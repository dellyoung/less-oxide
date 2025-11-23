use crate::ast::*;
use crate::error::{LessError, LessResult};

/// LESS 解析器，负责把源码转换成 AST。
pub struct LessParser;

impl LessParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, input: &str) -> LessResult<Stylesheet> {
        let mut cursor = Cursor::new(input);
        let mut statements = Vec::new();

        while !cursor.is_eof() {
            cursor.skip_whitespace_and_comments();
            if cursor.is_eof() {
                break;
            }

            if cursor.starts_with('@') && cursor.lookahead_is_variable_decl()? {
                let var = self.parse_variable(&mut cursor)?;
                statements.push(Statement::Variable(var));
                continue;
            }

            if cursor.starts_with('@') && cursor.lookahead_is_import()? {
                let import = self.parse_import(&mut cursor)?;
                statements.push(Statement::Import(import));
                continue;
            }

            if cursor.starts_with('@') && cursor.lookahead_is_block_at_rule()? {
                let at_rule = self.parse_at_rule(&mut cursor)?;
                statements.push(Statement::AtRule(at_rule));
                continue;
            }

            if cursor.lookahead_is_mixin_definition()? {
                let mixin = self.parse_mixin_definition(&mut cursor)?;
                statements.push(Statement::MixinDefinition(mixin));
                continue;
            }

            if cursor.lookahead_is_mixin_call()? {
                let call = self.parse_mixin_call(&mut cursor)?;
                statements.push(Statement::MixinCall(call));
                continue;
            }

            let rule = self.parse_ruleset(&mut cursor)?;
            statements.push(Statement::RuleSet(rule));
        }

        Ok(Stylesheet::new(statements))
    }

    fn parse_variable(&self, cursor: &mut Cursor<'_>) -> LessResult<VariableDeclaration> {
        cursor.expect_char('@')?;
        let name = cursor.read_identifier();
        cursor.skip_whitespace_and_comments();
        cursor.expect_char(':')?;
        cursor.skip_whitespace_and_comments();

        let value = self.read_value(cursor, &[';'])?;
        if cursor.peek_char() == Some(';') {
            cursor.advance_char();
        }

        Ok(VariableDeclaration { name, value })
    }

    fn parse_ruleset(&self, cursor: &mut Cursor<'_>) -> LessResult<RuleSet> {
        cursor.skip_whitespace_and_comments();
        let selector_raw = cursor.read_until('{')?;
        let selectors = selector_raw
            .split(',')
            .map(|s| Selector {
                value: s.trim().to_string(),
            })
            .filter(|sel| !sel.value.is_empty())
            .collect::<Vec<_>>();

        if selectors.is_empty() {
            return Err(LessError::parse("缺少合法的选择器", cursor.position()));
        }

        cursor.expect_char('{')?;
        let mut body = Vec::new();

        loop {
            cursor.skip_whitespace_and_comments();
            if cursor.peek_char() == Some('}') {
                cursor.advance_char();
                break;
            }

            if cursor.is_eof() {
                return Err(LessError::parse("缺少匹配的 '}'", cursor.position()));
            }

            let item = self.parse_rule_body_item(cursor)?;
            body.push(item);
        }

        Ok(RuleSet { selectors, body })
    }

    fn parse_at_rule(&self, cursor: &mut Cursor<'_>) -> LessResult<AtRule> {
        cursor.expect_char('@')?;
        let name = cursor.read_identifier();
        if name.is_empty() {
            return Err(LessError::parse("at-rule 名称不能为空", cursor.position()));
        }
        cursor.skip_whitespace_and_comments();
        let mut params = String::new();
        let mut paren_depth = 0usize;
        while let Some(ch) = cursor.peek_char() {
            if ch == '{' && paren_depth == 0 {
                break;
            }
            match ch {
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                _ => {}
            }
            params.push(ch);
            cursor.advance_char();
        }
        cursor.skip_whitespace_and_comments();
        if cursor.starts_with_keyword("when") {
            cursor.consume_keyword("when");
            cursor.skip_whitespace_and_comments();
            cursor.skip_guard_condition();
            cursor.skip_whitespace_and_comments();
        }
        cursor.expect_char('{')?;
        let body = self.parse_at_rule_body(cursor)?;
        Ok(AtRule {
            name,
            params: params.trim().to_string(),
            body,
        })
    }

    fn parse_at_rule_body(&self, cursor: &mut Cursor<'_>) -> LessResult<Vec<RuleBody>> {
        let mut body = Vec::new();
        loop {
            cursor.skip_whitespace_and_comments();
            match cursor.peek_char() {
                Some('}') => {
                    cursor.advance_char();
                    break;
                }
                None => {
                    return Err(LessError::parse(
                        "at-rule 缺少匹配的 '}'",
                        cursor.position(),
                    ));
                }
                _ => {
                    let item = self.parse_rule_body_item(cursor)?;
                    body.push(item);
                }
            }
        }
        Ok(body)
    }

    fn parse_declaration(&self, cursor: &mut Cursor<'_>) -> LessResult<Declaration> {
        let name = cursor.read_property_name();
        cursor.skip_whitespace_and_comments();
        cursor.expect_char(':')?;
        cursor.skip_whitespace_and_comments();
        let value = self.read_value(cursor, &[';', '}'])?;
        let important = false;

        if cursor.peek_char() == Some(';') {
            cursor.advance_char();
        }

        Ok(Declaration {
            name,
            value,
            important,
        })
    }

    fn read_value(&self, cursor: &mut Cursor<'_>, terminators: &[char]) -> LessResult<Value> {
        let mut pieces = Vec::new();
        let mut current = String::new();

        let mut paren_depth = 0usize;

        while let Some(ch) = cursor.peek_char() {
            if terminators.contains(&ch) && paren_depth == 0 {
                break;
            }

            match ch {
                '\n' => {
                    current.push(ch);
                    cursor.advance_char();
                }
                '\'' | '"' => {
                    current.push(ch);
                    cursor.advance_char();
                    while let Some(next) = cursor.peek_char() {
                        current.push(next);
                        cursor.advance_char();
                        if next == ch {
                            break;
                        }
                        if next == '\\' {
                            if let Some(escaped) = cursor.peek_char() {
                                current.push(escaped);
                                cursor.advance_char();
                            }
                        }
                    }
                }
                '@' => {
                    if !current.is_empty() {
                        pieces.push(ValuePiece::Literal(current.clone()));
                        current.clear();
                    }
                    cursor.advance_char();
                    let name = cursor.read_identifier();
                    if name.is_empty() {
                        return Err(LessError::parse("变量名不能为空", cursor.position()));
                    }
                    pieces.push(ValuePiece::VariableRef(name));
                }
                '(' => {
                    paren_depth += 1;
                    current.push(ch);
                    cursor.advance_char();
                }
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                    current.push(ch);
                    cursor.advance_char();
                }
                _ => {
                    current.push(ch);
                    cursor.advance_char();
                }
            }
        }

        if !current.is_empty() {
            pieces.push(ValuePiece::Literal(current));
        }

        Ok(Value::new(pieces))
    }

    fn parse_import(&self, cursor: &mut Cursor<'_>) -> LessResult<ImportStatement> {
        cursor.expect_char('@')?;
        let ident = cursor.read_identifier();
        if !ident.eq_ignore_ascii_case("import") {
            return Err(LessError::parse("仅支持 @import 语句", cursor.position()));
        }

        let spec = cursor.read_until(';')?;
        cursor.expect_char(';')?;

        let mut remainder = spec.trim_start();
        let mut options = Vec::new();
        if remainder.starts_with('(') {
            if let Some(end) = remainder.find(')') {
                let opt_str = &remainder[1..end];
                options = opt_str
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_ascii_lowercase())
                    .collect();
                remainder = remainder[end + 1..].trim_start();
            } else {
                return Err(LessError::parse("不完整的 @import 选项", cursor.position()));
            }
        }

        let trimmed = remainder.trim();
        let path = Self::extract_import_path(trimmed);
        let mut is_css = options.iter().any(|opt| opt == "css");
        if !is_css {
            if let Some(ref target) = path {
                if target.ends_with(".css") {
                    is_css = true;
                }
            } else {
                // 无法解析路径时默认视为 CSS 导入
                is_css = true;
            }
        }

        let mut raw = String::from("@import ");
        raw.push_str(trimmed);
        raw.push(';');

        Ok(ImportStatement { raw, path, is_css })
    }

    fn extract_import_path(input: &str) -> Option<String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let first = trimmed.chars().next()?;
        if first == '"' || first == '\'' {
            if let Some(end) = trimmed[1..].find(first) {
                return Some(trimmed[1..1 + end].to_string());
            }
            return None;
        }
        if trimmed.starts_with("url(") {
            return None;
        }
        let token = trimmed
            .split_whitespace()
            .next()
            .map(|s| s.trim().to_string())?;
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    }

    fn parse_rule_body_item(&self, cursor: &mut Cursor<'_>) -> LessResult<RuleBody> {
        if cursor.starts_with('@') && cursor.lookahead_is_variable_decl()? {
            let var = self.parse_variable(cursor)?;
            return Ok(RuleBody::Variable(var));
        }

        if cursor.lookahead_is_mixin_definition()? {
            let mixin = self.parse_mixin_definition(cursor)?;
            return Ok(RuleBody::MixinDefinition(mixin));
        }

        if cursor.lookahead_is_mixin_call()? {
            let call = self.parse_mixin_call(cursor)?;
            return Ok(RuleBody::MixinCall(call));
        }

        if cursor.starts_with('@') {
            if cursor.lookahead_is_block_at_rule()? {
                let at_rule = self.parse_at_rule(cursor)?;
                return Ok(RuleBody::AtRule(at_rule));
            }
            if cursor.lookahead_is_detached_call()? {
                let call = self.parse_detached_call(cursor)?;
                return Ok(RuleBody::DetachedCall(call));
            }
        }

        match cursor.detect_body_kind() {
            Some(BodyKind::Declaration) => {
                let decl = self.parse_declaration(cursor)?;
                Ok(RuleBody::Declaration(decl))
            }
            Some(BodyKind::NestedRule) => {
                let nested = self.parse_ruleset(cursor)?;
                Ok(RuleBody::NestedRule(nested))
            }
            None => Err(LessError::parse(
                "无法判断声明或子选择器",
                cursor.position(),
            )),
        }
    }

    fn parse_mixin_definition(&self, cursor: &mut Cursor<'_>) -> LessResult<MixinDefinition> {
        let name = cursor.read_mixin_name()?;
        cursor.skip_whitespace_and_comments();
        let params = if cursor.peek_char() == Some('(') {
            self.parse_mixin_params(cursor)?
        } else {
            Vec::new()
        };
        cursor.skip_whitespace_and_comments();
        if cursor.starts_with_keyword("when") {
            cursor.consume_keyword("when");
            cursor.skip_whitespace_and_comments();
            cursor.skip_guard_condition();
            cursor.skip_whitespace_and_comments();
        }
        cursor.expect_char('{')?;
        let body = self.parse_mixin_body(cursor)?;
        Ok(MixinDefinition { name, params, body })
    }

    fn parse_mixin_body(&self, cursor: &mut Cursor<'_>) -> LessResult<Vec<RuleBody>> {
        let mut body = Vec::new();
        loop {
            cursor.skip_whitespace_and_comments();
            match cursor.peek_char() {
                Some('}') => {
                    cursor.advance_char();
                    break;
                }
                None => {
                    return Err(LessError::parse("mixin 缺少匹配的 '}'", cursor.position()));
                }
                _ => {
                    let item = self.parse_rule_body_item(cursor)?;
                    body.push(item);
                }
            }
        }
        Ok(body)
    }

    fn parse_mixin_params(&self, cursor: &mut Cursor<'_>) -> LessResult<Vec<MixinParam>> {
        let mut params = Vec::new();
        cursor.expect_char('(')?;
        loop {
            cursor.skip_whitespace_and_comments();
            if cursor.peek_char() == Some(')') {
                cursor.advance_char();
                break;
            }
            cursor.expect_char('@')?;
            let name = cursor.read_identifier();
            if name.is_empty() {
                return Err(LessError::parse("mixin 参数名不能为空", cursor.position()));
            }
            cursor.skip_whitespace_and_comments();
            let default = if cursor.peek_char() == Some(':') {
                cursor.advance_char();
                cursor.skip_whitespace_and_comments();
                let value = self.read_value(cursor, &[',', ')'])?;
                Some(value)
            } else {
                None
            };
            params.push(MixinParam { name, default });
            cursor.skip_whitespace_and_comments();
            match cursor.peek_char() {
                Some(',') => {
                    cursor.advance_char();
                }
                Some(')') => {
                    cursor.advance_char();
                    break;
                }
                _ => {
                    return Err(LessError::parse(
                        "mixin 参数列表缺少分隔符",
                        cursor.position(),
                    ));
                }
            }
        }
        Ok(params)
    }

    fn parse_mixin_call(&self, cursor: &mut Cursor<'_>) -> LessResult<MixinCall> {
        let name = cursor.read_mixin_name()?;
        cursor.skip_whitespace_and_comments();
        let args = if cursor.peek_char() == Some('(') {
            self.parse_mixin_arguments(cursor)?
        } else {
            Vec::new()
        };
        cursor.skip_whitespace_and_comments();
        cursor.expect_char(';')?;
        Ok(MixinCall { name, args })
    }

    fn parse_mixin_arguments(&self, cursor: &mut Cursor<'_>) -> LessResult<Vec<MixinArgument>> {
        let mut args = Vec::new();
        cursor.expect_char('(')?;
        loop {
            cursor.skip_whitespace_and_comments();
            if cursor.peek_char() == Some(')') {
                cursor.advance_char();
                break;
            }
            if cursor.peek_char() == Some('{') {
                cursor.expect_char('{')?;
                let body = self.parse_mixin_body(cursor)?;
                args.push(MixinArgument::Ruleset(body));
            } else {
                let value = self.read_value(cursor, &[',', ')'])?;
                args.push(MixinArgument::Value(value));
            }
            cursor.skip_whitespace_and_comments();
            match cursor.peek_char() {
                Some(',') => {
                    cursor.advance_char();
                }
                Some(')') => {
                    cursor.advance_char();
                    break;
                }
                _ => {
                    return Err(LessError::parse(
                        "mixin 参数调用缺少分隔符",
                        cursor.position(),
                    ))
                }
            }
        }
        Ok(args)
    }

    fn parse_detached_call(&self, cursor: &mut Cursor<'_>) -> LessResult<DetachedCall> {
        cursor.expect_char('@')?;
        let name = cursor.read_identifier();
        if name.is_empty() {
            return Err(LessError::parse(
                "期待可调用的规则集名称",
                cursor.position(),
            ));
        }
        cursor.skip_whitespace_and_comments();
        cursor.expect_char('(')?;
        cursor.skip_whitespace_and_comments();
        if cursor.peek_char() != Some(')') {
            return Err(LessError::parse(
                "暂不支持带参数的规则集调用",
                cursor.position(),
            ));
        }
        cursor.advance_char();
        cursor.skip_whitespace_and_comments();
        cursor.expect_char(';')?;
        Ok(DetachedCall { name })
    }
}

/// 带位置指针的输入游标，提供便捷的字符读取与回退功能。
struct Cursor<'a> {
    source: &'a str,
    len: usize,
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            len: source.len(),
            position: 0,
        }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn is_eof(&self) -> bool {
        self.position >= self.len
    }

    fn starts_with(&self, ch: char) -> bool {
        self.peek_char() == Some(ch)
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    fn expect_char(&mut self, expect: char) -> LessResult<()> {
        match self.advance_char() {
            Some(ch) if ch == expect => Ok(()),
            Some(ch) => Err(LessError::parse(
                format!("期待字符 '{expect}', 却得到 '{ch}'"),
                self.position,
            )),
            None => Err(LessError::parse(
                format!("期待字符 '{expect}'"),
                self.position,
            )),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.starts_with('/') {
                if self.consume_comment() {
                    continue;
                }
            }
            break;
        }
    }

    fn consume_comment(&mut self) -> bool {
        if self.match_str("//") {
            while let Some(ch) = self.peek_char() {
                self.advance_char();
                if ch == '\n' {
                    break;
                }
            }
            true
        } else if self.match_str("/*") {
            while let Some(_) = self.peek_char() {
                if self.match_str("*/") {
                    break;
                }
                self.advance_char();
            }
            true
        } else {
            false
        }
    }

    fn match_str(&mut self, prefix: &str) -> bool {
        if self.source[self.position..].starts_with(prefix) {
            self.position += prefix.len();
            true
        } else {
            false
        }
    }

    fn starts_with_keyword(&self, keyword: &str) -> bool {
        if !self.source[self.position..].starts_with(keyword) {
            return false;
        }
        let end = self.position + keyword.len();
        match self.source.get(end..) {
            Some(rest) => match rest.chars().next() {
                Some(ch) => !ch.is_alphanumeric() && ch != '-' && ch != '_',
                None => true,
            },
            None => true,
        }
    }

    fn consume_keyword(&mut self, keyword: &str) {
        self.position += keyword.len();
    }

    fn skip_guard_condition(&mut self) {
        let mut depth = 0usize;
        while let Some(ch) = self.peek_char() {
            if ch == '{' && depth == 0 {
                break;
            }
            match ch {
                '(' => depth += 1,
                ')' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                }
                _ => {}
            }
            self.advance_char();
        }
    }

    fn read_identifier(&mut self) -> String {
        let mut ident = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ident.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }
        ident
    }

    fn read_property_name(&mut self) -> String {
        let mut name = String::new();
        let mut pending_interpolation = false;
        while let Some(ch) = self.peek_char() {
            if ch == ':' || ch == ';' {
                break;
            }
            if ch == '{' && !pending_interpolation {
                break;
            }
            if ch.is_control() {
                break;
            }
            let ch = self.advance_char().unwrap();
            name.push(ch);
            if ch == '@' {
                pending_interpolation = true;
            } else if ch == '{' && pending_interpolation {
                while let Some(inner) = self.advance_char() {
                    name.push(inner);
                    if inner == '}' {
                        pending_interpolation = false;
                        break;
                    }
                }
            } else if !ch.is_whitespace() {
                pending_interpolation = false;
            }
        }
        name.trim().to_string()
    }

    fn read_until(&mut self, end: char) -> LessResult<String> {
        let mut result = String::new();
        while let Some(ch) = self.peek_char() {
            if ch == end {
                break;
            }
            result.push(ch);
            self.advance_char();
        }
        if self.peek_char() != Some(end) {
            return Err(LessError::parse(format!("期待字符 '{end}'"), self.position));
        }
        Ok(result)
    }

    fn lookahead_is_variable_decl(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        lookahead.expect_char('@')?;
        lookahead.read_identifier();
        lookahead.skip_whitespace();
        Ok(lookahead.peek_char() == Some(':'))
    }

    fn lookahead_is_import(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        if !lookahead.starts_with('@') {
            return Ok(false);
        }
        lookahead.expect_char('@')?;
        let ident = lookahead.read_identifier();
        Ok(ident.eq_ignore_ascii_case("import"))
    }

    fn lookahead_is_block_at_rule(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        if !lookahead.starts_with('@') {
            return Ok(false);
        }
        lookahead.advance_char();
        let ident = lookahead.read_identifier();
        if ident.is_empty() {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        let mut paren_depth = 0usize;
        while let Some(ch) = lookahead.peek_char() {
            match ch {
                '{' if paren_depth == 0 => return Ok(true),
                '(' => {
                    paren_depth += 1;
                    lookahead.advance_char();
                }
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                    lookahead.advance_char();
                }
                ';' => return Ok(false),
                _ => {
                    lookahead.advance_char();
                }
            }
        }
        Ok(false)
    }

    fn lookahead_is_mixin_definition(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        if !matches!(lookahead.peek_char(), Some('.') | Some('#')) {
            return Ok(false);
        }
        lookahead.advance_char();
        let ident = lookahead.read_identifier();
        if ident.is_empty() {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        if lookahead.peek_char() != Some('(') {
            return Ok(false);
        }
        lookahead.advance_char();
        let mut depth = 1;
        while let Some(ch) = lookahead.peek_char() {
            lookahead.advance_char();
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        if lookahead.starts_with_keyword("when") {
            lookahead.consume_keyword("when");
            lookahead.skip_whitespace_and_comments();
            lookahead.skip_guard_condition();
            lookahead.skip_whitespace_and_comments();
        }
        Ok(lookahead.peek_char() == Some('{'))
    }

    fn lookahead_is_mixin_call(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        if !matches!(lookahead.peek_char(), Some('.') | Some('#')) {
            return Ok(false);
        }
        lookahead.advance_char();
        let ident = lookahead.read_identifier();
        if ident.is_empty() {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        if lookahead.peek_char() == Some('(') {
            lookahead.advance_char();
            let mut depth = 1;
            while let Some(ch) = lookahead.peek_char() {
                lookahead.advance_char();
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if depth != 0 {
                return Ok(false);
            }
            lookahead.skip_whitespace_and_comments();
        }
        Ok(lookahead.peek_char() == Some(';'))
    }

    fn lookahead_is_detached_call(&self) -> LessResult<bool> {
        let mut lookahead = self.clone();
        if !lookahead.starts_with('@') {
            return Ok(false);
        }
        lookahead.advance_char();
        let ident = lookahead.read_identifier();
        if ident.is_empty() {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        if lookahead.peek_char() != Some('(') {
            return Ok(false);
        }
        lookahead.advance_char();
        let mut depth = 1;
        while let Some(ch) = lookahead.peek_char() {
            lookahead.advance_char();
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Ok(false);
        }
        lookahead.skip_whitespace_and_comments();
        Ok(lookahead.peek_char() == Some(';'))
    }

    fn read_mixin_name(&mut self) -> LessResult<String> {
        match self.peek_char() {
            Some('.') | Some('#') => {
                let prefix = self.advance_char().unwrap();
                let mut name = String::new();
                name.push(prefix);
                let ident = self.read_identifier();
                if ident.is_empty() {
                    return Err(LessError::parse("mixin 名称不合法", self.position()));
                }
                name.push_str(&ident);
                Ok(name)
            }
            _ => Err(LessError::parse("期待 mixin 名称", self.position())),
        }
    }

    /// 通过向前查看判断接下来的语句类型（声明或子规则）。
    fn detect_body_kind(&self) -> Option<BodyKind> {
        let mut iter = self.clone();
        iter.skip_whitespace_and_comments();
        let mut saw_colon = false;
        let mut pending_interpolation = false;
        while let Some(ch) = iter.peek_char() {
            match ch {
                '@' => {
                    pending_interpolation = true;
                    iter.advance_char();
                    continue;
                }
                '{' if pending_interpolation => {
                    iter.advance_char();
                    while let Some(inner) = iter.peek_char() {
                        let current = inner;
                        iter.advance_char();
                        if current == '}' {
                            break;
                        }
                    }
                    pending_interpolation = false;
                    continue;
                }
                '{' => return Some(BodyKind::NestedRule),
                ';' => return Some(BodyKind::Declaration),
                '}' => {
                    return if saw_colon {
                        Some(BodyKind::Declaration)
                    } else {
                        None
                    }
                }
                ':' => {
                    saw_colon = true;
                }
                _ => {
                    pending_interpolation = false;
                }
            }
            iter.advance_char();
        }
        if saw_colon {
            Some(BodyKind::Declaration)
        } else {
            None
        }
    }
}

impl<'a> Clone for Cursor<'a> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            len: self.len,
            position: self.position,
        }
    }
}

enum BodyKind {
    Declaration,
    NestedRule,
}
