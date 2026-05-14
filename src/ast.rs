use crate::token::Span;

/// Expression nodes. Each variant carries the data needed to evaluate it.
/// Spans are included on nodes that name variables so runtime errors can
/// report source locations in future milestones.
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

/// Statement nodes.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
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
}
