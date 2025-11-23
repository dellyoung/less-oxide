use std::fmt::{self, Display};

/// 表示一份完整的 LESS 样式表。
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub statements: Vec<Statement>,
}

/// 树中的顶层语句。
#[derive(Debug, Clone)]
pub enum Statement {
    Import(ImportStatement),
    AtRule(AtRule),
    RuleSet(RuleSet),
    Variable(VariableDeclaration),
    MixinDefinition(MixinDefinition),
    MixinCall(MixinCall),
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct RuleSet {
    pub selectors: Vec<Selector>,
    pub body: Vec<RuleBody>,
}

#[derive(Debug, Clone)]
pub struct AtRule {
    pub name: String,
    pub params: String,
    pub body: Vec<RuleBody>,
}

#[derive(Debug, Clone)]
pub enum RuleBody {
    Declaration(Declaration),
    NestedRule(RuleSet),
    AtRule(AtRule),
    DetachedCall(DetachedCall),
    Variable(VariableDeclaration),
    MixinDefinition(MixinDefinition),
    MixinCall(MixinCall),
}

#[derive(Debug, Clone)]
pub struct Selector {
    pub value: String,
}

impl Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub name: String,
    pub value: Value,
    pub important: bool,
}

#[derive(Debug, Clone)]
pub struct Value {
    pub pieces: Vec<ValuePiece>,
}

impl Value {
    pub fn new(pieces: Vec<ValuePiece>) -> Self {
        Self { pieces }
    }
}

#[derive(Debug, Clone)]
pub enum ValuePiece {
    Literal(String),
    VariableRef(String),
}

impl Stylesheet {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }
}

#[derive(Debug, Clone)]
pub struct ImportStatement {
    pub raw: String,
    pub path: Option<String>,
    pub is_css: bool,
}

#[derive(Debug, Clone)]
pub struct MixinDefinition {
    pub name: String,
    pub params: Vec<MixinParam>,
    pub body: Vec<RuleBody>,
}

#[derive(Debug, Clone)]
pub struct MixinParam {
    pub name: String,
    pub default: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct MixinCall {
    pub name: String,
    pub args: Vec<MixinArgument>,
}

#[derive(Debug, Clone)]
pub enum MixinArgument {
    Value(Value),
    Ruleset(Vec<RuleBody>),
}

#[derive(Debug, Clone)]
pub struct DetachedCall {
    pub name: String,
}
