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
    /// A state variant value expression: `StateName.variant`
    StateVariant {
        state_name: String,
        variant_name: String,
        span: Span,
    },
    /// A fixed-size homogeneous array literal: `[e1, e2, e3]`
    ArrayLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    /// Array index expression: `array[index]`
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Array slice expression: `array[start..end]`
    /// Returns a new array with elements from start (inclusive) to end (exclusive).
    Slice {
        array: Box<Expr>,
        start: Box<Expr>,
        end: Box<Expr>,
        span: Span,
    },
    /// Map literal expression: `{"key": value, ...}`
    /// Keys must be Text expressions; values must be homogeneous.
    MapLiteral {
        entries: Vec<(Expr, Expr)>,
        span: Span,
    },
    /// Struct construction expression: `TypeName { field: value, ... }`
    /// Field names are unquoted identifiers; all declared fields must be provided.
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// Field access on an arbitrary expression: `expr.field`
    /// Used for chained access like `arr[0].field` or `make().field`.
    /// Simple `var.field` patterns are parsed as `Expr::StateVariant` in `parse_primary`.
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    /// A method call on a struct receiver: `expr.method(arg1, arg2, ...)`.
    /// Receiver is evaluated; receiver type must be a struct; method looked up by struct name.
    /// Receiver value is passed as the implicit first argument (`self`).
    MethodCall {
        object: Box<Expr>,
        method: String,
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
    /// A numeric value annotated with a physical unit (e.g., `meters`, `seconds`).
    /// The String is the canonical unit name as returned by the parser's unit registry.
    NumberWithUnit(String),
    Text,
    Bool,
    Nil,
    /// An identifier that is not a built-in type or known unit.
    /// Resolved to a state machine type by the type checker.
    Named(String),
    /// A homogeneous array type annotation: `Array<T>`.
    Array(Box<TypeAnnotation>),
    /// A map type annotation: `Map<Text, V>`.
    /// Key must be Text; value can be any supported non-map type.
    Map(Box<TypeAnnotation>, Box<TypeAnnotation>),
}

/// A typed function parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeAnnotation,
    /// True when declared `mut self` in a method — makes the self binding mutable.
    /// Always false for regular typed params and global function params.
    pub mutable: bool,
    pub span: Span,
}

/// A named variant in a state machine declaration.
#[derive(Debug, Clone)]
pub struct StateVariant {
    pub name: String,
    pub span: Span,
}

/// An allowed transition between two state variants.
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub span: Span,
}

/// Operator used in a compound assignment statement.
#[derive(Debug, Clone, PartialEq)]
pub enum CompoundAssignOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// A recursive assignment target for path-based mutation.
///
/// Represents the left-hand side of `target = expr` and `target op= expr`
/// where the target can be a nested chain of field accesses and index operations.
///
/// Examples:
///   `u.score`                    → Field(Var("u"), "score")
///   `users[0].score`             → Field(Index(Var("users"), 0_expr), "score")
///   `company.owner.name`         → Field(Field(Var("company"), "owner"), "name")
///   `scores["alice"].value`      → Field(Index(Var("scores"), "alice"_expr), "value")
#[derive(Debug, Clone)]
pub enum AssignTarget {
    /// A root mutable variable.
    Var(String),
    /// Field access step: `target.field`.
    Field(Box<AssignTarget>, String),
    /// Index step: `target[expr]` — array index (Number) or map key (Text).
    Index(Box<AssignTarget>, Expr),
}

/// A single step in a compiled assignment path (index steps consume a stack value).
#[derive(Debug, Clone, PartialEq)]
pub enum PathStep {
    /// Access a named struct field. No stack value consumed.
    Field(String),
    /// Array/map index step. Consumes one value from the index-value sequence.
    Index,
}

/// Statement nodes.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        /// True when declared with `let mut`.
        mutable: bool,
        /// Optional `: TypeAnnotation` — if absent the type is inferred.
        annotation: Option<TypeAnnotation>,
        value: Expr,
        span: Span,
    },
    /// Variable reassignment: `x = expr`. Only valid for `let mut` bindings.
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    /// Compound assignment: `x += expr`, `x -= expr`, `x *= expr`, `x /= expr`.
    CompoundAssign {
        name: String,
        op: CompoundAssignOp,
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
    /// A state machine declaration: `state Name { variants... transitions... }`
    StateDecl {
        name: String,
        variants: Vec<StateVariant>,
        transitions: Vec<StateTransition>,
        span: Span,
    },
    /// A controlled state transition statement: `transition var -> target_variant`
    Transition {
        variable: String,
        target: String,
        span: Span,
    },
    /// A deterministic simulation block: `simulate <duration> step <step> { ... }`
    Simulate {
        duration: Expr,
        step: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// A while loop: `while <condition> { ... }`
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Exit the nearest enclosing while loop.
    Break {
        span: Span,
    },
    /// Skip the rest of the current while-body iteration; re-evaluate the condition.
    Continue {
        span: Span,
    },
    /// A numeric range-based for loop: `for <var> in range(<start>, <end>) { ... }`
    /// Iterates `i` from `start` (inclusive) to `end` (exclusive) by 1.
    /// The loop variable is immutable and loop-local.
    ForRange {
        var_name: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// An array for-each loop: `for <var> in <array_expr> { ... }`
    /// Evaluates the iterable once (snapshot); iterates over each element in order.
    /// The loop variable is immutable and loop-local.
    ForEach {
        var_name: String,
        iterable: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Indexed array for-each loop: `for <index>, <var> in <array_expr> { ... }`
    /// Evaluates the iterable once (snapshot); provides 0-based index and element per iteration.
    /// Both variables are immutable and loop-local; index is Number.
    ForEachIndexed {
        index_name: String,
        var_name: String,
        iterable: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Array element assignment: `arr[index] = value`. Only valid for `let mut` arrays.
    /// The target must be an identifier naming a mutable array variable.
    IndexAssign {
        name: String,
        index: Expr,
        value: Expr,
        span: Span,
    },
    /// Array element compound assignment: `arr[index] op= value`. Sugar for `arr[i] = arr[i] op value`.
    /// Only valid for `let mut` arrays. Index is evaluated once.
    IndexCompoundAssign {
        name: String,
        index: Expr,
        op: CompoundAssignOp,
        value: Expr,
        span: Span,
    },
    /// A struct type declaration: `struct Name { field: Type, ... }`
    StructDecl {
        name: String,
        fields: Vec<(String, TypeAnnotation)>,
        span: Span,
    },
    /// An impl block declaring methods for a named struct.
    /// Each method is a `Stmt::FnDecl` where the first parameter is `self` (by value).
    ImplBlock {
        struct_name: String,
        methods: Vec<Stmt>,
        span: Span,
    },
    /// Path-based field mutation: `target.field = expr`.
    /// `target` can be a variable, nested field path, array index, or map index.
    /// The root variable must be mutable. Only valid as a statement.
    TargetAssign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    /// Path-based field compound mutation: `target.field op= expr`.
    TargetCompoundAssign {
        target: AssignTarget,
        op: CompoundAssignOp,
        value: Expr,
        span: Span,
    },
}
