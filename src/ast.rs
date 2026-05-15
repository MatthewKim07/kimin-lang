use crate::token::Span;

/// Expression nodes.
#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Str(String),
    Bool(bool),
    Variable {
        name: String,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Grouping(Box<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

/// User-facing static type annotation (written in source code).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeAnnotation {
    Number,
    Text,
    Bool,
    Nil,
}

/// A typed function parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeAnnotation,
    pub span: Span,
}

/// Statement nodes.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        /// Optional `: TypeAnnotation` — if absent the type is inferred.
        annotation: Option<TypeAnnotation>,
        value: Expr,
        span: Span,
    },
    Print {
        value: Expr,
    },
    Expr(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then_block: Box<Stmt>,
        else_block: Option<Box<Stmt>>,
    },
    FnDecl {
        name: String,
        params: Vec<Param>,
        /// Optional `-> TypeAnnotation` return type.
        return_type: Option<TypeAnnotation>,
        body: Vec<Stmt>,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
}
