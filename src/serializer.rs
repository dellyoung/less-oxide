use crate::evaluator::{
    EvaluatedAtRule, EvaluatedDeclaration, EvaluatedNode, EvaluatedRule, EvaluatedStylesheet,
};
use crate::utils::{collapse_whitespace, indent};

/// 负责将扁平化的规则转换为最终 CSS 文本。
pub struct Serializer {
    minify: bool,
}

impl Serializer {
    pub fn new(minify: bool) -> Self {
        Self { minify }
    }

    pub fn to_css(&self, stylesheet: &EvaluatedStylesheet) -> String {
        if self.minify {
            self.render_minified(stylesheet)
        } else {
            self.render_pretty(stylesheet)
        }
    }

    fn render_pretty(&self, stylesheet: &EvaluatedStylesheet) -> String {
        let mut output = String::new();
        for import in &stylesheet.imports {
            output.push_str(import.trim());
            output.push('\n');
        }
        if !stylesheet.imports.is_empty() && !stylesheet.nodes.is_empty() {
            output.push('\n');
        }
        for (idx, node) in stylesheet.nodes.iter().enumerate() {
            self.render_node_pretty(node, 0, &mut output);
            if idx + 1 < stylesheet.nodes.len() {
                output.push('\n');
            }
        }
        output.trim().to_string()
    }

    fn render_minified(&self, stylesheet: &EvaluatedStylesheet) -> String {
        let mut output = String::new();
        for import in &stylesheet.imports {
            output.push_str(import.trim());
            output.push('\n');
        }
        for node in &stylesheet.nodes {
            self.render_node_minified(node, &mut output);
        }
        while output.ends_with('\n') {
            output.pop();
        }
        output
    }

    fn format_declaration(&self, decl: &EvaluatedDeclaration) -> String {
        let mut result = format!("{}: {}", decl.name.trim(), decl.value.trim());
        if decl.important {
            result.push_str(" !important");
        }
        result.push(';');
        result
    }

    fn format_declaration_minified(&self, decl: &EvaluatedDeclaration) -> String {
        let mut result = format!("{}:{}", decl.name.trim(), collapse_whitespace(&decl.value));
        if decl.important {
            result.push_str("!important");
        }
        result
    }

    fn render_node_pretty(&self, node: &EvaluatedNode, level: usize, output: &mut String) {
        match node {
            EvaluatedNode::Rule(rule) => self.render_rule_pretty(rule, level, output),
            EvaluatedNode::AtRule(at_rule) => self.render_at_rule_pretty(at_rule, level, output),
        }
    }

    fn render_rule_pretty(&self, rule: &EvaluatedRule, level: usize, output: &mut String) {
        if rule.declarations.is_empty() {
            return;
        }
        output.push_str(&indent(level));
        output.push_str(&rule.selectors.join(", "));
        output.push_str(" {\n");
        for decl in &rule.declarations {
            output.push_str(&indent(level + 1));
            output.push_str(&self.format_declaration(decl));
            output.push('\n');
        }
        output.push_str(&indent(level));
        output.push_str("}\n");
    }

    fn render_at_rule_pretty(&self, at_rule: &EvaluatedAtRule, level: usize, output: &mut String) {
        output.push_str(&indent(level));
        output.push('@');
        output.push_str(&at_rule.name);
        if !at_rule.params.is_empty() {
            output.push(' ');
            output.push_str(at_rule.params.trim());
        }
        output.push_str(" {\n");
        for decl in &at_rule.declarations {
            output.push_str(&indent(level + 1));
            output.push_str(&self.format_declaration(decl));
            output.push('\n');
        }
        for child in &at_rule.children {
            self.render_node_pretty(child, level + 1, output);
            if !output.ends_with('\n') {
                output.push('\n');
            }
        }
        output.push_str(&indent(level));
        output.push_str("}\n");
    }

    fn render_node_minified(&self, node: &EvaluatedNode, output: &mut String) {
        match node {
            EvaluatedNode::Rule(rule) => self.render_rule_minified(rule, output),
            EvaluatedNode::AtRule(at_rule) => self.render_at_rule_minified(at_rule, output),
        }
    }

    fn render_rule_minified(&self, rule: &EvaluatedRule, output: &mut String) {
        if rule.declarations.is_empty() {
            return;
        }
        output.push_str(&rule.selectors.join(","));
        output.push('{');
        for (idx, decl) in rule.declarations.iter().enumerate() {
            if idx > 0 {
                output.push(';');
            }
            output.push_str(&self.format_declaration_minified(decl));
        }
        output.push('}');
    }

    fn render_at_rule_minified(&self, at_rule: &EvaluatedAtRule, output: &mut String) {
        output.push('@');
        output.push_str(&at_rule.name);
        if !at_rule.params.trim().is_empty() {
            output.push(' ');
            output.push_str(&collapse_whitespace(&at_rule.params));
        }
        output.push('{');
        for (idx, decl) in at_rule.declarations.iter().enumerate() {
            if idx > 0 {
                output.push(';');
            }
            output.push_str(&self.format_declaration_minified(decl));
        }
        for child in &at_rule.children {
            self.render_node_minified(child, output);
        }
        output.push('}');
    }
}
