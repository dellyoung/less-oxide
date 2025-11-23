use crate::ast::{
    AtRule, Declaration, MixinArgument, MixinCall, MixinDefinition, RuleBody, RuleSet, Statement,
    Stylesheet, Value, ValuePiece,
};
use crate::color;
use crate::error::{LessError, LessResult};
use crate::CompileOptions;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

/// 经过语义求值后的规则信息。
#[derive(Debug, Clone)]
pub struct EvaluatedStylesheet {
    pub imports: Vec<String>,
    pub nodes: Vec<EvaluatedNode>,
}

#[derive(Debug, Clone)]
pub enum EvaluatedNode {
    Rule(EvaluatedRule),
    AtRule(EvaluatedAtRule),
}

#[derive(Debug, Clone)]
pub struct EvaluatedRule {
    pub selectors: Vec<String>,
    pub declarations: Vec<EvaluatedDeclaration>,
}

#[derive(Debug, Clone)]
pub struct EvaluatedAtRule {
    pub name: String,
    pub params: String,
    pub declarations: Vec<EvaluatedDeclaration>,
    pub children: Vec<EvaluatedNode>,
}

#[derive(Debug, Clone)]
pub struct EvaluatedDeclaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

/// 负责维护变量与 mixin 作用域并输出扁平化 CSS 规则。
pub struct Evaluator {
    scopes: Vec<IndexMap<String, VariableValue>>,
    mixin_scopes: Vec<IndexMap<String, MixinDefinition>>,
}

impl Evaluator {
    pub fn new(options: CompileOptions) -> Self {
        let _ = options;
        Self {
            scopes: vec![IndexMap::new()],
            mixin_scopes: vec![IndexMap::new()],
        }
    }

    pub fn evaluate(&mut self, stylesheet: Stylesheet) -> LessResult<EvaluatedStylesheet> {
        let mut imports = Vec::new();
        let mut nodes = Vec::new();
        for statement in stylesheet.statements {
            match statement {
                Statement::Import(import) => {
                    imports.push(import.raw);
                }
                Statement::Variable(var) => {
                    let value = self.eval_value(&var.value)?;
                    self.set_variable_text(var.name, value);
                }
                Statement::RuleSet(rule) => {
                    let mut produced = self.eval_ruleset(rule, &[])?;
                    nodes.append(&mut produced);
                }
                Statement::AtRule(at_rule) => {
                    let evaluated = self.eval_at_rule(at_rule, &[])?;
                    nodes.push(EvaluatedNode::AtRule(evaluated));
                }
                Statement::MixinDefinition(def) => {
                    self.set_mixin(def);
                }
                Statement::MixinCall(call) => {
                    let mut declarations = Vec::new();
                    let mut produced = Vec::new();
                    self.expand_mixin(call, &[], &mut declarations, &mut produced)?;
                    if !declarations.is_empty() {
                        return Err(LessError::eval("顶层 mixin 调用产生了无法附加的声明"));
                    }
                    nodes.extend(produced);
                }
            }
        }
        Ok(EvaluatedStylesheet { imports, nodes })
    }

    fn eval_ruleset(
        &mut self,
        rule: RuleSet,
        parent_selectors: &[String],
    ) -> LessResult<Vec<EvaluatedNode>> {
        self.push_scope();
        self.push_mixin_scope();

        let selectors = self.combine_selectors(parent_selectors, &rule.selectors);
        let mut declarations = Vec::new();
        let mut pending_nodes: Vec<EvaluatedNode> = Vec::new();

        for item in rule.body {
            self.handle_rule_body_item(item, &selectors, &mut declarations, &mut pending_nodes)?;
        }

        let mut output = Vec::new();
        if !declarations.is_empty() {
            output.push(EvaluatedNode::Rule(EvaluatedRule {
                selectors: selectors.clone(),
                declarations,
            }));
        }

        output.extend(pending_nodes);

        self.pop_mixin_scope();
        self.pop_scope();
        Ok(output)
    }

    fn handle_rule_body_item(
        &mut self,
        item: RuleBody,
        selectors: &[String],
        declarations: &mut Vec<EvaluatedDeclaration>,
        pending_nodes: &mut Vec<EvaluatedNode>,
    ) -> LessResult<()> {
        match item {
            RuleBody::Variable(var) => {
                let value = self.eval_value(&var.value)?;
                self.set_variable_text(var.name, value);
            }
            RuleBody::Declaration(decl) => {
                let evaluated = self.eval_declaration(decl)?;
                declarations.push(evaluated);
            }
            RuleBody::NestedRule(nested) => {
                let nested_output = self.eval_ruleset(nested, selectors)?;
                pending_nodes.extend(nested_output);
            }
            RuleBody::MixinDefinition(def) => {
                self.set_mixin(def);
            }
            RuleBody::MixinCall(call) => {
                self.expand_mixin(call, selectors, declarations, pending_nodes)?;
            }
            RuleBody::AtRule(at_rule) => {
                let evaluated = self.eval_at_rule(at_rule, selectors)?;
                pending_nodes.push(EvaluatedNode::AtRule(evaluated));
            }
            RuleBody::DetachedCall(call) => {
                self.invoke_detached_ruleset(&call.name, selectors, declarations, pending_nodes)?;
            }
        }
        Ok(())
    }

    fn expand_mixin(
        &mut self,
        call: MixinCall,
        selectors: &[String],
        declarations: &mut Vec<EvaluatedDeclaration>,
        pending_nodes: &mut Vec<EvaluatedNode>,
    ) -> LessResult<()> {
        let definition = self.resolve_mixin(&call.name)?;
        if call.args.len() > definition.params.len() {
            return Err(LessError::eval(format!(
                "mixin {} 参数过多: 期望 {} 个，实际 {} 个",
                call.name,
                definition.params.len(),
                call.args.len()
            )));
        }

        self.push_scope();
        self.push_mixin_scope();

        for (arg_value, param) in call.args.iter().zip(definition.params.iter()) {
            match arg_value {
                MixinArgument::Value(value) => {
                    let evaluated = self.eval_value(value)?;
                    self.set_variable_text(param.name.clone(), evaluated);
                }
                MixinArgument::Ruleset(body) => {
                    self.set_variable_ruleset(param.name.clone(), body.clone());
                }
            }
        }

        if call.args.len() < definition.params.len() {
            for param in definition.params.iter().skip(call.args.len()) {
                if let Some(default) = &param.default {
                    let evaluated = self.eval_value(default)?;
                    self.set_variable_text(param.name.clone(), evaluated);
                } else {
                    self.pop_mixin_scope();
                    self.pop_scope();
                    return Err(LessError::eval(format!(
                        "mixin {} 缺少必填参数 @{}",
                        definition.name, param.name
                    )));
                }
            }
        }

        for body_item in definition.body {
            self.handle_rule_body_item(body_item, selectors, declarations, pending_nodes)?;
        }

        self.pop_mixin_scope();
        self.pop_scope();
        Ok(())
    }

    fn invoke_detached_ruleset(
        &mut self,
        name: &str,
        selectors: &[String],
        declarations: &mut Vec<EvaluatedDeclaration>,
        pending_nodes: &mut Vec<EvaluatedNode>,
    ) -> LessResult<()> {
        let body = self.resolve_ruleset_variable(name)?;
        for item in body {
            self.handle_rule_body_item(item, selectors, declarations, pending_nodes)?;
        }
        Ok(())
    }

    fn eval_at_rule(
        &mut self,
        at_rule: AtRule,
        selectors: &[String],
    ) -> LessResult<EvaluatedAtRule> {
        self.push_scope();
        self.push_mixin_scope();

        let mut scoped_declarations = Vec::new();
        let mut at_rule_declarations = Vec::new();
        let mut children: Vec<EvaluatedNode> = Vec::new();

        for item in at_rule.body {
            match item {
                RuleBody::Variable(var) => {
                    let value = self.eval_value(&var.value)?;
                    self.set_variable_text(var.name, value);
                }
                RuleBody::Declaration(decl) => {
                    let evaluated = self.eval_declaration(decl)?;
                    if selectors.is_empty() {
                        at_rule_declarations.push(evaluated);
                    } else {
                        scoped_declarations.push(evaluated);
                    }
                }
                RuleBody::NestedRule(nested) => {
                    let nested_output = self.eval_ruleset(nested, selectors)?;
                    children.extend(nested_output);
                }
                RuleBody::MixinDefinition(def) => {
                    self.set_mixin(def);
                }
                RuleBody::MixinCall(call) => {
                    if selectors.is_empty() {
                        self.expand_mixin(
                            call,
                            selectors,
                            &mut at_rule_declarations,
                            &mut children,
                        )?;
                    } else {
                        self.expand_mixin(
                            call,
                            selectors,
                            &mut scoped_declarations,
                            &mut children,
                        )?;
                    }
                }
                RuleBody::AtRule(inner) => {
                    let evaluated = self.eval_at_rule(inner, selectors)?;
                    children.push(EvaluatedNode::AtRule(evaluated));
                }
                RuleBody::DetachedCall(call) => {
                    if selectors.is_empty() {
                        self.invoke_detached_ruleset(
                            &call.name,
                            selectors,
                            &mut at_rule_declarations,
                            &mut children,
                        )?;
                    } else {
                        self.invoke_detached_ruleset(
                            &call.name,
                            selectors,
                            &mut scoped_declarations,
                            &mut children,
                        )?;
                    }
                }
            }
        }

        let mut scoped_nodes = Vec::new();
        if !selectors.is_empty() && !scoped_declarations.is_empty() {
            scoped_nodes.push(EvaluatedNode::Rule(EvaluatedRule {
                selectors: selectors.to_vec(),
                declarations: scoped_declarations,
            }));
        }
        scoped_nodes.extend(children);

        self.pop_mixin_scope();
        self.pop_scope();

        Ok(EvaluatedAtRule {
            name: at_rule.name,
            params: at_rule.params,
            declarations: if selectors.is_empty() {
                at_rule_declarations
            } else {
                Vec::new()
            },
            children: scoped_nodes,
        })
    }

    fn eval_declaration(&mut self, decl: Declaration) -> LessResult<EvaluatedDeclaration> {
        let name = self.interpolate_property_name(&decl.name)?;
        let mut value = self.eval_value(&decl.value)?;
        let mut important = decl.important;
        if !important {
            if let Some(stripped) = Self::strip_important(&value) {
                value = stripped;
                important = true;
            }
        }
        Ok(EvaluatedDeclaration {
            name,
            value,
            important,
        })
    }

    fn interpolate_property_name(&self, raw: &str) -> LessResult<String> {
        if !raw.contains("@{") {
            return Ok(raw.trim().to_string());
        }
        let mut chars = raw.chars().peekable();
        let mut output = String::new();
        while let Some(ch) = chars.next() {
            if ch == '@' && chars.peek() == Some(&'{') {
                chars.next();
                let mut name = String::new();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                if name.is_empty() {
                    return Err(LessError::eval("属性插值缺少变量名"));
                }
                let value = self.resolve_variable_text(&name)?;
                output.push_str(value.trim());
            } else {
                output.push(ch);
            }
        }
        Ok(output.trim().to_string())
    }

    fn eval_value(&mut self, value: &Value) -> LessResult<String> {
        let mut buffer = String::new();
        for piece in &value.pieces {
            match piece {
                ValuePiece::Literal(text) => buffer.push_str(text),
                ValuePiece::VariableRef(name) => {
                    let resolved = self.resolve_variable_text(name)?;
                    buffer.push_str(&resolved);
                }
            }
        }
        self.compute_value(buffer.trim())
    }

    fn compute_value(&mut self, input: &str) -> LessResult<String> {
        if input.is_empty() {
            return Ok(String::new());
        }
        if let Some(color) = self.evaluate_color_function(input)? {
            return Ok(color);
        }
        if let Some(inline) = self.replace_inline_color_functions(input)? {
            return Ok(inline);
        }
        if input.contains("var(") {
            return Ok(input.to_string());
        }
        if input.contains("url(") {
            return Ok(input.to_string());
        }
        if input.contains("unit(") {
            return Ok(input.to_string());
        }
        if input.contains("calc(") {
            return Ok(input.to_string());
        }
        match self.evaluate_arithmetic(input) {
            Ok(Some(value)) => return Ok(value),
            Ok(None) => {}
            Err(_) => return Ok(input.to_string()),
        }
        Ok(input.to_string())
    }

    fn evaluate_color_function(&mut self, input: &str) -> LessResult<Option<String>> {
        static COLOR_FN_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?ix)^(?P<name>lighten|darken|fade)\s*\(\s*(?P<color>[^,]+)\s*,\s*(?P<amount>[^)]+)\)$")
                .expect("颜色函数正则编译失败")
        });

        if let Some(result) = self.evaluate_overlay_function(input)? {
            return Ok(Some(result));
        }

        if let Some(caps) = COLOR_FN_RE.captures(input) {
            let name = caps.name("name").unwrap().as_str().to_ascii_lowercase();
            let color_arg = caps.name("color").unwrap().as_str().trim();
            let amount_arg = caps.name("amount").unwrap().as_str().trim();

            let color = color::parse_color(color_arg)
                .ok_or_else(|| LessError::eval(format!("无法解析颜色参数: {color_arg}")))?;
            let amount = Self::parse_percentage(amount_arg)?;

            let result = match name.as_str() {
                "lighten" => color::lighten(color, amount),
                "darken" => color::darken(color, amount),
                "fade" => color::fade(color, amount),
                _ => return Ok(None),
            };

            let output = if name == "fade" {
                color::format_rgba(result)
            } else {
                color::format_hex(result)
            };

            return Ok(Some(output));
        }
        Ok(None)
    }

    fn evaluate_overlay_function(&self, input: &str) -> LessResult<Option<String>> {
        let trimmed = input.trim();
        if !trimmed.to_ascii_lowercase().starts_with("overlay(") {
            return Ok(None);
        }
        let start = trimmed
            .find('(')
            .ok_or_else(|| LessError::eval("overlay 函数缺少 '('"))?
            + 1;
        let end = trimmed
            .rfind(')')
            .ok_or_else(|| LessError::eval("overlay 函数缺少 ')'"))?;
        let body = &trimmed[start..end];
        let (first, second) = Self::split_overlay_args(body)?;
        let top_color = color::parse_color(first.trim())
            .ok_or_else(|| LessError::eval(format!("无法解析颜色参数: {first}")))?;
        let bottom_color = color::parse_color(second.trim())
            .ok_or_else(|| LessError::eval(format!("无法解析颜色参数: {second}")))?;
        let blended = color::overlay(top_color, bottom_color);
        Ok(Some(color::format_hex(blended)))
    }

    fn split_overlay_args(input: &str) -> LessResult<(String, String)> {
        let mut depth = 0i32;
        let mut split = None;
        for (idx, ch) in input.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                }
                ',' if depth == 0 => {
                    split = Some(idx);
                    break;
                }
                _ => {}
            }
        }
        let idx = split.ok_or_else(|| LessError::eval("overlay 函数参数不完整"))?;
        let first = input[..idx].to_string();
        let second = input[idx + 1..].to_string();
        Ok((first, second))
    }

    fn replace_inline_color_functions(&mut self, input: &str) -> LessResult<Option<String>> {
        static INLINE_COLOR_FN_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(
                r"(?xi)(lighten|darken|fade)\s*\(\s*((?:[^()]+|\([^()]*\))+?)\s*,\s*([^)]+)\)",
            )
            .expect("颜色函数正则编译失败")
        });

        let mut output = String::with_capacity(input.len());
        let mut last = 0;
        let mut changed = false;

        for caps in INLINE_COLOR_FN_RE.captures_iter(input) {
            let matched = caps.get(0).unwrap();
            output.push_str(&input[last..matched.start()]);

            let name = caps.get(1).unwrap().as_str().to_ascii_lowercase();
            let color_arg = caps.get(2).unwrap().as_str().trim();
            let amount_arg = caps.get(3).unwrap().as_str().trim();

            let color = color::parse_color(color_arg)
                .ok_or_else(|| LessError::eval(format!("无法解析颜色参数: {color_arg}")))?;
            let amount = Self::parse_percentage(amount_arg)?;

            let replacement = match name.as_str() {
                "lighten" => color::format_hex(color::lighten(color, amount)),
                "darken" => color::format_hex(color::darken(color, amount)),
                "fade" => color::format_rgba(color::fade(color, amount)),
                _ => unreachable!(),
            };

            output.push_str(&replacement);
            last = matched.end();
            changed = true;
        }

        if !changed {
            return Ok(None);
        }

        output.push_str(&input[last..]);
        Ok(Some(output))
    }

    fn parse_percentage(raw: &str) -> LessResult<f64> {
        let cleaned = raw.trim();
        if cleaned.ends_with('%') {
            let number = cleaned[..cleaned.len() - 1].trim();
            let value: f64 = number
                .parse()
                .map_err(|_| LessError::eval(format!("无法解析百分比: {raw}")))?;
            Ok((value / 100.0).clamp(0.0, 1.0))
        } else {
            let value: f64 = cleaned
                .parse()
                .map_err(|_| LessError::eval(format!("无法解析数值: {raw}")))?;
            Ok(value.clamp(0.0, 1.0))
        }
    }

    fn evaluate_arithmetic(&self, input: &str) -> LessResult<Option<String>> {
        let cleaned = input.replace(['(', ')'], " ");
        let expression = Self::strip_outer_parentheses(cleaned.trim());
        if expression.is_empty() || !Self::contains_operator(expression) {
            return Ok(None);
        }

        let tokens = self.tokenize_expression(expression)?;
        if tokens.is_empty() {
            return Ok(None);
        }

        let mut iter = tokens.into_iter();
        let mut current = match iter.next() {
            Some(Token::Quantity(q)) => q,
            _ => return Err(LessError::eval("算术表达式缺少初始数值".to_string())),
        };

        let mut results: Vec<Quantity> = Vec::new();

        while let Some(token) = iter.next() {
            match token {
                Token::Operator(op) => {
                    let rhs = match iter.next() {
                        Some(Token::Quantity(q)) => q,
                        _ => return Err(LessError::eval("算术表达式缺少右侧数值".to_string())),
                    };
                    current = Self::apply_operator(current, op, rhs)?;
                }
                Token::Quantity(next_qty) => {
                    results.push(current);
                    current = next_qty;
                }
            }
        }

        results.push(current);

        let output = results
            .into_iter()
            .map(Self::format_quantity)
            .collect::<Vec<_>>()
            .join(" ");

        Ok(Some(output))
    }

    fn tokenize_expression(&self, input: &str) -> LessResult<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut prev_was_operator = true;

        for ch in input.chars() {
            if ch.is_whitespace() {
                let trimmed_current = current.trim();
                if trimmed_current == "-" || trimmed_current == "+" {
                    continue;
                }

                if !current.is_empty() {
                    Self::push_token(&mut tokens, &mut current)?;
                }
                continue;
            }

            if Self::is_operator(ch) {
                if ch == '-' && prev_was_operator {
                    current.push(ch);
                    continue;
                }
                if !current.is_empty() {
                    Self::push_token(&mut tokens, &mut current)?;
                }
                tokens.push(Token::Operator(ch));
                prev_was_operator = true;
            } else {
                current.push(ch);
                prev_was_operator = false;
            }
        }

        if !current.is_empty() {
            Self::push_token(&mut tokens, &mut current)?;
        }

        Ok(tokens)
    }

    fn push_token(tokens: &mut Vec<Token>, current: &mut String) -> LessResult<()> {
        let trimmed = current.trim();
        if trimmed.is_empty() {
            current.clear();
            return Ok(());
        }

        if trimmed == "-" || trimmed == "+" {
            return Err(LessError::eval("算术表达式缺少数值内容".to_string()));
        }

        if trimmed.len() == 1 && Self::is_operator(trimmed.chars().next().unwrap()) {
            tokens.push(Token::Operator(trimmed.chars().next().unwrap()));
        } else {
            let quantity = Self::parse_quantity(trimmed)?;
            tokens.push(Token::Quantity(quantity));
        }

        current.clear();
        Ok(())
    }

    fn parse_quantity(token: &str) -> LessResult<Quantity> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(LessError::eval("缺少数值内容".to_string()));
        }

        let mut value_part = String::new();
        let mut unit_part = String::new();
        for ch in trimmed.chars() {
            if ch.is_ascii_digit()
                || ch == '.'
                || ((ch == '-' || ch == '+') && value_part.is_empty())
            {
                value_part.push(ch);
            } else if ch.is_ascii_alphabetic() || ch == '%' {
                unit_part.push(ch);
            } else if ch.is_whitespace() {
                continue;
            } else {
                return Err(LessError::eval(format!("无法解析数值片段: {token}")));
            }
        }

        if value_part.is_empty() {
            return Err(LessError::eval(format!("缺少数值部分: {token}")));
        }

        let value: f64 = value_part
            .parse()
            .map_err(|_| LessError::eval(format!("无法解析数值 {value_part}")))?;

        Ok(Quantity {
            value,
            unit: unit_part,
        })
    }

    fn apply_operator(lhs: Quantity, op: char, rhs: Quantity) -> LessResult<Quantity> {
        match op {
            '+' | '-' => {
                if lhs.unit != rhs.unit {
                    return Err(LessError::eval(format!(
                        "不同单位无法相加/相减: {}{} 与 {}{}",
                        lhs.value, lhs.unit, rhs.value, rhs.unit
                    )));
                }
                let value = if op == '+' {
                    lhs.value + rhs.value
                } else {
                    lhs.value - rhs.value
                };
                Ok(Quantity {
                    value,
                    unit: lhs.unit,
                })
            }
            '*' => {
                if !lhs.unit.is_empty() && !rhs.unit.is_empty() {
                    return Err(LessError::eval("暂不支持两个带单位数值相乘".to_string()));
                }
                let value = lhs.value * rhs.value;
                let unit = if lhs.unit.is_empty() {
                    rhs.unit
                } else {
                    lhs.unit
                };
                Ok(Quantity { value, unit })
            }
            '/' => {
                if rhs.value.abs() < f64::EPSILON {
                    return Err(LessError::eval("除法分母不能为 0".to_string()));
                }
                if !rhs.unit.is_empty() {
                    return Err(LessError::eval("暂不支持被除数携带单位".to_string()));
                }
                Ok(Quantity {
                    value: lhs.value / rhs.value,
                    unit: lhs.unit,
                })
            }
            _ => Err(LessError::eval(format!("未知的运算符 {op}"))),
        }
    }

    fn format_quantity(quantity: Quantity) -> String {
        let mut value = quantity.value;
        if value.abs() < 1e-9 {
            value = 0.0;
        }
        let mut formatted = format!("{value:.4}");
        while formatted.contains('.') && formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
        if quantity.unit.is_empty() {
            formatted
        } else {
            format!("{formatted}{}", quantity.unit)
        }
    }

    fn strip_outer_parentheses<'a>(input: &'a str) -> &'a str {
        let mut trimmed = input.trim();
        loop {
            if trimmed.starts_with('(') && trimmed.ends_with(')') {
                let mut depth = 0;
                let mut balanced = true;
                for (idx, ch) in trimmed.chars().enumerate() {
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth == 0 && idx != trimmed.len() - 1 {
                            balanced = false;
                            break;
                        }
                    }
                }
                if balanced && depth == 0 && trimmed.len() > 2 {
                    trimmed = trimmed[1..trimmed.len() - 1].trim();
                    continue;
                }
            }
            return trimmed;
        }
    }

    fn contains_operator(input: &str) -> bool {
        let chars: Vec<char> = input.chars().collect();
        for (idx, &ch) in chars.iter().enumerate() {
            if !Self::is_operator(ch) {
                continue;
            }
            if ch == '-' {
                if chars.get(idx + 1) == Some(&'-') {
                    continue;
                }
            }

            let prev = idx.checked_sub(1).and_then(|i| chars.get(i)).copied();
            let next = chars.get(idx + 1).copied();

            let prev_ok = prev.map_or(true, |c| {
                c.is_whitespace()
                    || c.is_ascii_digit()
                    || matches!(c, '(' | ')' | '+' | '-' | '*' | '/')
            });

            let next_ok = next.map_or(true, |c| {
                c.is_whitespace()
                    || c.is_ascii_digit()
                    || c == '@'
                    || matches!(c, '(' | ')' | '+' | '-' | '*' | '/')
            });

            if prev_ok && next_ok {
                return true;
            }
        }
        false
    }

    fn is_operator(ch: char) -> bool {
        matches!(ch, '+' | '-' | '*' | '/')
    }

    fn resolve_variable_text(&self, name: &str) -> LessResult<String> {
        match self.lookup_variable(name)? {
            VariableValue::Text(value) => Ok(value),
            VariableValue::DetachedRuleset(_) => Err(LessError::eval(format!(
                "变量 @{name} 不是可作为文本使用的值"
            ))),
        }
    }

    fn resolve_ruleset_variable(&self, name: &str) -> LessResult<Vec<RuleBody>> {
        match self.lookup_variable(name)? {
            VariableValue::DetachedRuleset(body) => Ok(body),
            VariableValue::Text(_) => {
                Err(LessError::eval(format!("变量 @{name} 不是可调用的规则集")))
            }
        }
    }

    fn lookup_variable(&self, name: &str) -> LessResult<VariableValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Ok(value.clone());
            }
        }
        Err(LessError::eval(format!("未定义的变量 @{name}")))
    }

    fn set_variable_text(&mut self, name: String, value: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, VariableValue::Text(value));
        }
    }

    fn set_variable_ruleset(&mut self, name: String, body: Vec<RuleBody>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, VariableValue::DetachedRuleset(body));
        }
    }

    fn set_mixin(&mut self, definition: MixinDefinition) {
        if let Some(scope) = self.mixin_scopes.last_mut() {
            scope.insert(definition.name.clone(), definition);
        }
    }

    fn resolve_mixin(&self, name: &str) -> LessResult<MixinDefinition> {
        for scope in self.mixin_scopes.iter().rev() {
            if let Some(def) = scope.get(name) {
                return Ok(def.clone());
            }
        }
        Err(LessError::eval(format!("未定义的 mixin {name}")))
    }

    fn push_scope(&mut self) {
        self.scopes.push(IndexMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn push_mixin_scope(&mut self) {
        self.mixin_scopes.push(IndexMap::new());
    }

    fn pop_mixin_scope(&mut self) {
        self.mixin_scopes.pop();
    }

    /// 合并父子选择器，支持 `&` 占位符。
    fn combine_selectors(
        &self,
        parents: &[String],
        current: &[crate::ast::Selector],
    ) -> Vec<String> {
        if parents.is_empty() {
            return current.iter().map(|s| s.value.clone()).collect();
        }

        let mut result = Vec::new();
        for parent in parents {
            for child in current {
                let selector = if child.value.contains('&') {
                    child.value.replace('&', parent).trim().to_string()
                } else {
                    format!("{} {}", parent.trim(), child.value.trim())
                };
                result.push(selector);
            }
        }
        result
    }

    /// 检测并剥离 `!important` 标记，返回去除后的值。
    fn strip_important(value: &str) -> Option<String> {
        let trimmed = value.trim_end();
        if trimmed.ends_with("!important") {
            let idx = trimmed.len() - "!important".len();
            let without = trimmed[..idx].trim_end();
            return Some(without.to_string());
        }
        None
    }
}

#[derive(Debug, Clone)]
struct Quantity {
    value: f64,
    unit: String,
}

#[derive(Debug)]
enum Token {
    Quantity(Quantity),
    Operator(char),
}

#[derive(Debug, Clone)]
enum VariableValue {
    Text(String),
    DetachedRuleset(Vec<RuleBody>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompileOptions;

    #[test]
    fn hyphenated_words_are_not_arithmetic() {
        assert!(!Evaluator::contains_operator("inline-flex"));
        assert!(!Evaluator::contains_operator("border-radius"));
    }

    #[test]
    fn overlay_function_is_evaluated() {
        let mut evaluator = Evaluator::new(CompileOptions::default());
        let value = evaluator
            .evaluate_color_function("overlay(rgba(255, 255, 255, 0.05), #2c2c2c)")
            .unwrap();
        assert_eq!(value, Some("#373737".to_string()));
    }
}
