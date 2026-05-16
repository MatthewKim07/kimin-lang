use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::{
    BinaryOp, Expr, Param, StateTransition, StateVariant, Stmt, TypeAnnotation, UnaryOp,
};
use crate::error::TypeError;
use crate::token::Span;

/// Structured representation of a physical unit dimension.
///
/// Internally a sparse map from canonical unit name → integer exponent.
///   `meters`        → `{ "meters": 1 }`
///   `meters/seconds`→ `{ "meters": 1, "seconds": -1 }`
///   `meters^2`      → `{ "meters": 2 }`
///   dimensionless   → `{}` (treated as plain Number by the type checker)
///
/// BTreeMap gives deterministic alphabetical key ordering for display and equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDimension {
    exponents: BTreeMap<String, i32>,
}

impl UnitDimension {
    /// Base unit with exponent 1 (e.g., `UnitDimension::base("meters")`).
    pub fn base(unit: &str) -> Self {
        let mut exponents = BTreeMap::new();
        exponents.insert(unit.to_string(), 1);
        UnitDimension { exponents }
    }

    /// Dimensionless unit (empty map). Used as the `1` in `Number / unit = 1/unit`.
    pub fn dimensionless() -> Self {
        UnitDimension {
            exponents: BTreeMap::new(),
        }
    }

    /// Multiply two dimensions by adding exponents. Zero exponents are removed.
    pub fn mul(&self, other: &Self) -> Self {
        let mut exponents = self.exponents.clone();
        for (k, v) in &other.exponents {
            let e = exponents.entry(k.clone()).or_insert(0);
            *e += v;
        }
        exponents.retain(|_, v| *v != 0);
        UnitDimension { exponents }
    }

    /// Divide two dimensions by subtracting the other's exponents. Zero exponents are removed.
    pub fn div(&self, other: &Self) -> Self {
        let mut exponents = self.exponents.clone();
        for (k, v) in &other.exponents {
            let e = exponents.entry(k.clone()).or_insert(0);
            *e -= v;
        }
        exponents.retain(|_, v| *v != 0);
        UnitDimension { exponents }
    }

    /// True when all exponents have cancelled — equivalent to plain Number.
    pub fn is_dimensionless(&self) -> bool {
        self.exponents.is_empty()
    }

    /// Human-readable display string.
    ///
    /// - `{ meters: 1 }`                          → `"meters"`
    /// - `{ meters: 2 }`                          → `"meters^2"`
    /// - `{ meters: 1, seconds: -1 }`             → `"meters/seconds"`
    /// - `{ seconds: -1 }`                        → `"1/seconds"`
    /// - `{ kilograms: 1, meters: 1, seconds: -2 }` → `"kilograms*meters/seconds^2"`
    ///
    /// Positive exponents: numerator, sorted alphabetically, joined with `*`.
    /// Negative exponents: denominator, sorted alphabetically, joined with `*`.
    pub fn display_name(&self) -> String {
        if self.exponents.is_empty() {
            return "Number".to_string();
        }
        let mut num: Vec<String> = Vec::new();
        let mut den: Vec<String> = Vec::new();
        for (unit, &exp) in &self.exponents {
            if exp > 0 {
                if exp == 1 {
                    num.push(unit.clone());
                } else {
                    num.push(format!("{}^{}", unit, exp));
                }
            } else {
                let abs_exp = -exp;
                if abs_exp == 1 {
                    den.push(unit.clone());
                } else {
                    den.push(format!("{}^{}", unit, abs_exp));
                }
            }
        }
        match (num.is_empty(), den.is_empty()) {
            (false, true) => num.join("*"),
            (true, false) => format!("1/{}", den.join("*")),
            (false, false) => format!("{}/{}", num.join("*"), den.join("*")),
            (true, true) => "Number".to_string(),
        }
    }
}

/// Static description of a state machine registered by a `state` declaration.
pub struct StateMachineType {
    pub name: String,
    pub variants: HashSet<String>,
    /// Set of (from_variant, to_variant) allowed transitions.
    pub transitions: HashSet<(String, String)>,
}

/// Type and optional known-state-variant for a single variable binding.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    /// Statically known current variant for state-typed variables, when determinable.
    pub known_state_variant: Option<String>,
}

/// Static type representation used by the type checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    /// A numeric value with a physical unit dimension.
    NumberWithUnit(UnitDimension),
    Text,
    Bool,
    Nil,
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// A state machine value. The String is the state machine name.
    State(String),
    /// Inferred or unannotated type — skips type checking on operations involving it.
    Unknown,
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::Number => "Number".into(),
            Type::NumberWithUnit(dim) => dim.display_name(),
            Type::Text => "Text".into(),
            Type::Bool => "Bool".into(),
            Type::Nil => "Nil".into(),
            Type::Function { .. } => "Function".into(),
            Type::State(s) => s.clone(),
            Type::Unknown => "Unknown".into(),
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }
}

/// Lexical scope stack for static types.
pub struct TypeEnv {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn get(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    /// Define a variable with no known state variant (normal non-state variables).
    pub fn define(&mut self, name: String, ty: Type) {
        self.scopes
            .last_mut()
            .expect("TypeEnv scope stack empty")
            .insert(
                name,
                VarInfo {
                    ty,
                    known_state_variant: None,
                },
            );
    }

    /// Define a variable with an optional known state variant.
    pub fn define_with_variant(&mut self, name: String, ty: Type, variant: Option<String>) {
        self.scopes
            .last_mut()
            .expect("TypeEnv scope stack empty")
            .insert(
                name,
                VarInfo {
                    ty,
                    known_state_variant: variant,
                },
            );
    }

    /// Update the known state variant for an existing variable, searching from inner to outer scope.
    pub fn update_variant(&mut self, name: &str, variant: String) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(info) = scope.get_mut(name) {
                info.known_state_variant = Some(variant);
                return;
            }
        }
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TypeChecker {
    pub env: TypeEnv,
    /// Return type of the function currently being checked. None at top level.
    current_fn_return_type: Option<Type>,
    /// Registry of declared state machines. Populated by state declaration pre-pass.
    states: HashMap<String, StateMachineType>,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            env: TypeEnv::new(),
            current_fn_return_type: None,
            states: HashMap::new(),
        }
    }

    /// Type-check a program (top-level statement list).
    pub fn check(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        self.check_stmt_list(stmts)
    }

    /// Type-check a list of statements.
    ///
    /// Three-pass design:
    ///   Pass 1 — register all state machine declarations (so function signatures can reference them)
    ///   Pass 2 — register all function signatures (enables mutual recursion type-checking)
    ///   Pass 3 — check all statements
    fn check_stmt_list(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        // Pass 1: register state machines.
        for stmt in stmts {
            if let Stmt::StateDecl {
                name,
                variants,
                transitions,
                span,
            } = stmt
            {
                self.register_state(name, variants, transitions, *span)?;
            }
        }
        // Pass 2: register function signatures.
        for stmt in stmts {
            if let Stmt::FnDecl {
                name,
                params,
                return_type,
                span,
                ..
            } = stmt
            {
                let ty = self.build_fn_type(params, return_type.as_ref(), *span)?;
                self.env.define(name.clone(), ty);
            }
        }
        // Pass 3: check all statements.
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn register_state(
        &mut self,
        name: &str,
        variants: &[StateVariant],
        transitions: &[StateTransition],
        span: Span,
    ) -> Result<(), TypeError> {
        if self.states.contains_key(name) {
            return Err(TypeError {
                msg: format!("duplicate state machine '{}'", name),
                line: span.line,
                col: span.col,
            });
        }

        // Check for duplicate variant names.
        let mut variant_set = HashSet::new();
        for v in variants {
            if !variant_set.insert(v.name.clone()) {
                return Err(TypeError {
                    msg: format!("duplicate variant '{}' in state machine '{}'", v.name, name),
                    line: v.span.line,
                    col: v.span.col,
                });
            }
        }

        // Validate transitions reference known variants.
        let mut transition_set = HashSet::new();
        for t in transitions {
            if !variant_set.contains(&t.from) {
                return Err(TypeError {
                    msg: format!(
                        "transition 'from' variant '{}' is not declared in state machine '{}'",
                        t.from, name
                    ),
                    line: t.span.line,
                    col: t.span.col,
                });
            }
            if !variant_set.contains(&t.to) {
                return Err(TypeError {
                    msg: format!(
                        "transition 'to' variant '{}' is not declared in state machine '{}'",
                        t.to, name
                    ),
                    line: t.span.line,
                    col: t.span.col,
                });
            }
            transition_set.insert((t.from.clone(), t.to.clone()));
        }

        self.states.insert(
            name.to_string(),
            StateMachineType {
                name: name.to_string(),
                variants: variant_set,
                transitions: transition_set,
            },
        );
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::StateDecl { .. } => {
                // Already registered in the pre-pass; nothing more to do.
                Ok(())
            }

            Stmt::Transition {
                variable,
                target,
                span,
            } => {
                // Clone what we need before any mutable borrows.
                let (ty, known_variant) = self
                    .env
                    .get(variable)
                    .map(|vi| (vi.ty.clone(), vi.known_state_variant.clone()))
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", variable),
                        line: span.line,
                        col: span.col,
                    })?;

                let state_name = match ty {
                    Type::State(s) => s,
                    other => {
                        return Err(TypeError {
                            msg: format!(
                                "'{}' has type {}, not a state machine; transition requires a state variable",
                                variable,
                                other.name()
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                };

                let sm = self
                    .states
                    .get(&state_name)
                    .expect("Type::State references unregistered state machine");

                if !sm.variants.contains(target) {
                    return Err(TypeError {
                        msg: format!(
                            "unknown variant '{}' for state machine '{}'",
                            target, state_name
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }

                if let Some(current) = known_variant {
                    if !sm.transitions.contains(&(current.clone(), target.clone())) {
                        return Err(TypeError {
                            msg: format!(
                                "invalid transition for {}: {} -> {}",
                                state_name, current, target
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                }
                // Valid — update the tracked known variant.
                self.env.update_variant(variable, target.clone());
                Ok(())
            }

            Stmt::Let {
                name,
                annotation,
                value,
                span,
            } => {
                let val_ty = self.check_expr(value, *span)?;

                // Extract known variant when the initializer is a direct state variant expression.
                let known_variant = match value {
                    Expr::StateVariant { variant_name, .. } => Some(variant_name.clone()),
                    _ => None,
                };

                let (effective_ty, effective_variant) = if let Some(ann) = annotation {
                    let ann_ty = self.resolve_annotation(ann, *span)?;
                    let compatible = val_ty.is_unknown()
                        || val_ty == ann_ty
                        || (matches!(&ann_ty, Type::NumberWithUnit(_)) && val_ty == Type::Number)
                        || (matches!(&ann_ty, Type::State(_)) && val_ty.is_unknown());
                    if !compatible {
                        return Err(TypeError {
                            msg: format!(
                                "variable '{}' declared as {} but initializer has type {}",
                                name,
                                ann_ty.name(),
                                val_ty.name()
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    let variant = if matches!(ann_ty, Type::State(_)) {
                        known_variant
                    } else {
                        None
                    };
                    (ann_ty, variant)
                } else {
                    let variant = if matches!(val_ty, Type::State(_)) {
                        known_variant
                    } else {
                        None
                    };
                    (val_ty, variant)
                };

                self.env
                    .define_with_variant(name.clone(), effective_ty, effective_variant);
                Ok(())
            }

            Stmt::Print { value } => {
                let ty = self.check_expr(value, Span { line: 0, col: 0 })?;
                if matches!(ty, Type::Function { .. }) {
                    return Err(TypeError {
                        msg: "cannot print a Function value".into(),
                        line: 0,
                        col: 0,
                    });
                }
                Ok(())
            }

            Stmt::Expr(expr) => {
                self.check_expr(expr, Span { line: 0, col: 0 })?;
                Ok(())
            }

            Stmt::Block(stmts) => {
                self.env.push_scope();
                let result = self.check_stmt_list(stmts);
                self.env.pop_scope();
                result
            }

            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_ty = self.check_expr(cond, Span { line: 0, col: 0 })?;
                if !cond_ty.is_unknown() && cond_ty != Type::Bool {
                    return Err(TypeError {
                        msg: format!("if condition must be Bool, got {}", cond_ty.name()),
                        line: 0,
                        col: 0,
                    });
                }
                self.check_stmt(then_block)?;
                if let Some(else_b) = else_block {
                    self.check_stmt(else_b)?;
                }
                Ok(())
            }

            Stmt::FnDecl {
                params,
                return_type,
                body,
                span,
                ..
            } => {
                // Signature already registered by the pre-pass.
                let saved_ret = self.current_fn_return_type.take();
                self.current_fn_return_type = Some(match return_type.as_ref() {
                    Some(ann) => self.resolve_annotation(ann, *span)?,
                    None => Type::Unknown,
                });

                self.env.push_scope();
                for param in params {
                    let param_ty = self.resolve_annotation(&param.ty, param.span)?;
                    self.env.define(param.name.clone(), param_ty);
                }
                let result = self.check_stmt_list(body);
                self.env.pop_scope();

                self.current_fn_return_type = saved_ret;
                result
            }

            Stmt::Return { value, span } => {
                if self.current_fn_return_type.is_none() {
                    return Err(TypeError {
                        msg: "cannot return outside of a function".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                let declared = self
                    .current_fn_return_type
                    .as_ref()
                    .expect("checked above")
                    .clone();
                let ret_ty = match value {
                    Some(expr) => self.check_expr(expr, *span)?,
                    None => Type::Nil,
                };
                let compatible = declared.is_unknown()
                    || ret_ty.is_unknown()
                    || ret_ty == declared
                    || (matches!(&declared, Type::NumberWithUnit(_)) && ret_ty == Type::Number);
                if !compatible {
                    return Err(TypeError {
                        msg: format!(
                            "function declared return type {} but returned {}",
                            declared.name(),
                            ret_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(())
            }

            Stmt::Simulate {
                duration,
                step,
                body,
                span,
            } => {
                let dur_ty = self.check_expr(duration, *span)?;
                let step_ty = self.check_expr(step, *span)?;

                // Determine the `time` variable type injected into the body.
                let time_ty = if dur_ty.is_unknown() {
                    Type::Unknown
                } else if is_time_unit(&dur_ty) {
                    dur_ty.clone()
                } else {
                    return Err(TypeError {
                        msg: format!(
                            "simulate duration must be a time unit (seconds), got {}",
                            dur_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                };

                // Step must match the duration type (both unknown is fine; mismatch is not).
                if !step_ty.is_unknown() && !time_ty.is_unknown() && step_ty != time_ty {
                    return Err(TypeError {
                        msg: format!(
                            "simulate step must have the same unit as duration ({}), got {}",
                            time_ty.name(),
                            step_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }

                self.env.push_scope();
                self.env.define("time".to_string(), time_ty);
                let result = self.check_stmt_list(body);
                self.env.pop_scope();
                result
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, context_span: Span) -> Result<Type, TypeError> {
        match expr {
            Expr::Number(_) => Ok(Type::Number),
            Expr::Str(_) => Ok(Type::Text),
            Expr::Bool(_) => Ok(Type::Bool),

            Expr::Variable { name, span } => {
                self.env
                    .get(name)
                    .map(|vi| vi.ty.clone())
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    })
            }

            Expr::StateVariant {
                state_name,
                variant_name,
                span,
            } => {
                let sm = self.states.get(state_name).ok_or_else(|| TypeError {
                    msg: format!("unknown state machine '{}'", state_name),
                    line: span.line,
                    col: span.col,
                })?;
                if !sm.variants.contains(variant_name) {
                    return Err(TypeError {
                        msg: format!(
                            "unknown variant '{}' for state machine '{}'",
                            variant_name, state_name
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(Type::State(state_name.clone()))
            }

            Expr::Grouping(inner) => self.check_expr(inner, context_span),

            Expr::Unary { op, operand } => {
                let ty = self.check_expr(operand, context_span)?;
                if ty.is_unknown() {
                    return Ok(Type::Unknown);
                }
                match op {
                    UnaryOp::Neg => match &ty {
                        Type::Number => Ok(Type::Number),
                        Type::NumberWithUnit(u) => Ok(Type::NumberWithUnit(u.clone())),
                        _ => Err(TypeError {
                            msg: format!("unary '-' requires Number, got {}", ty.name()),
                            line: context_span.line,
                            col: context_span.col,
                        }),
                    },
                    UnaryOp::Not => {
                        if ty != Type::Bool {
                            return Err(TypeError {
                                msg: format!("unary '!' requires Bool, got {}", ty.name()),
                                line: context_span.line,
                                col: context_span.col,
                            });
                        }
                        Ok(Type::Bool)
                    }
                }
            }

            Expr::Binary { op, left, right } => {
                let lt = self.check_expr(left, context_span)?;
                let rt = self.check_expr(right, context_span)?;
                self.check_binary(op, lt, rt, context_span)
            }

            Expr::Call { callee, args, span } => {
                let callee_ty = self.check_expr(callee, *span)?;
                let callee_name = if let Expr::Variable { name, .. } = callee.as_ref() {
                    name.as_str()
                } else {
                    "<expression>"
                };
                match callee_ty {
                    Type::Unknown => {
                        for arg in args {
                            self.check_expr(arg, *span)?;
                        }
                        Ok(Type::Unknown)
                    }
                    Type::Function {
                        params: param_types,
                        ret,
                    } => {
                        if args.len() != param_types.len() {
                            return Err(TypeError {
                                msg: format!(
                                    "function '{}' expected {} argument{} but got {}",
                                    callee_name,
                                    param_types.len(),
                                    if param_types.len() == 1 { "" } else { "s" },
                                    args.len()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        for (i, (arg, expected)) in args.iter().zip(param_types.iter()).enumerate()
                        {
                            let arg_ty = self.check_expr(arg, *span)?;
                            let compatible = arg_ty.is_unknown()
                                || expected.is_unknown()
                                || arg_ty == *expected
                                || (matches!(expected, Type::NumberWithUnit(_))
                                    && arg_ty == Type::Number);
                            if !compatible {
                                return Err(TypeError {
                                    msg: format!(
                                        "function '{}' argument {} expected {} but got {}",
                                        callee_name,
                                        i + 1,
                                        expected.name(),
                                        arg_ty.name()
                                    ),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        }
                        Ok(*ret)
                    }
                    other => Err(TypeError {
                        msg: format!(
                            "cannot call '{}': value has type {}, not Function",
                            callee_name,
                            other.name()
                        ),
                        line: span.line,
                        col: span.col,
                    }),
                }
            }
        }
    }

    fn check_binary(
        &self,
        op: &BinaryOp,
        lt: Type,
        rt: Type,
        span: Span,
    ) -> Result<Type, TypeError> {
        if lt.is_unknown() || rt.is_unknown() {
            return Ok(Type::Unknown);
        }

        match op {
            BinaryOp::Add => match (&lt, &rt) {
                (Type::Number, Type::Number) => Ok(Type::Number),
                (Type::Text, Type::Text) => Ok(Type::Text),
                (Type::NumberWithUnit(u), Type::NumberWithUnit(v)) => {
                    if u == v {
                        Ok(Type::NumberWithUnit(u.clone()))
                    } else {
                        Err(TypeError {
                            msg: format!(
                                "cannot add {} and {}",
                                u.display_name(),
                                v.display_name()
                            ),
                            line: span.line,
                            col: span.col,
                        })
                    }
                }
                _ => Err(TypeError {
                    msg: format!(
                        "operator '+' expected Number + Number, Text + Text, or same-unit + same-unit, got {} + {}",
                        lt.name(),
                        rt.name()
                    ),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinaryOp::Sub => match (&lt, &rt) {
                (Type::Number, Type::Number) => Ok(Type::Number),
                (Type::NumberWithUnit(u), Type::NumberWithUnit(v)) => {
                    if u == v {
                        Ok(Type::NumberWithUnit(u.clone()))
                    } else {
                        Err(TypeError {
                            msg: format!(
                                "cannot subtract {} and {}",
                                u.display_name(),
                                v.display_name()
                            ),
                            line: span.line,
                            col: span.col,
                        })
                    }
                }
                _ => Err(TypeError {
                    msg: format!(
                        "operator '-' requires Number or same-unit operands, got {} and {}",
                        lt.name(),
                        rt.name()
                    ),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinaryOp::Mul => match (&lt, &rt) {
                (Type::Number, Type::Number) => Ok(Type::Number),
                (Type::Number, Type::NumberWithUnit(u)) => Ok(Type::NumberWithUnit(u.clone())),
                (Type::NumberWithUnit(u), Type::Number) => Ok(Type::NumberWithUnit(u.clone())),
                (Type::NumberWithUnit(u), Type::NumberWithUnit(v)) => {
                    let result = u.mul(v);
                    if result.is_dimensionless() {
                        Ok(Type::Number)
                    } else {
                        Ok(Type::NumberWithUnit(result))
                    }
                }
                _ => Err(TypeError {
                    msg: format!(
                        "operator '*' requires Number operands, got {} and {}",
                        lt.name(),
                        rt.name()
                    ),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinaryOp::Div => match (&lt, &rt) {
                (Type::Number, Type::Number) => Ok(Type::Number),
                (Type::NumberWithUnit(u), Type::Number) => Ok(Type::NumberWithUnit(u.clone())),
                (Type::NumberWithUnit(u), Type::NumberWithUnit(v)) => {
                    let result = u.div(v);
                    if result.is_dimensionless() {
                        Ok(Type::Number)
                    } else {
                        Ok(Type::NumberWithUnit(result))
                    }
                }
                (Type::Number, Type::NumberWithUnit(v)) => {
                    let result = UnitDimension::dimensionless().div(v);
                    Ok(Type::NumberWithUnit(result))
                }
                _ => Err(TypeError {
                    msg: format!(
                        "operator '/' requires Number operands, got {} and {}",
                        lt.name(),
                        rt.name()
                    ),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinaryOp::Eq | BinaryOp::NotEq => {
                if lt != rt {
                    let sym = if matches!(op, BinaryOp::Eq) { "==" } else { "!=" };
                    return Err(TypeError {
                        msg: format!(
                            "operator '{}' requires same-type operands, got {} and {}",
                            sym,
                            lt.name(),
                            rt.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(Type::Bool)
            }
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                let sym = match op {
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    _ => unreachable!(),
                };
                match (&lt, &rt) {
                    (Type::Number, Type::Number) => Ok(Type::Bool),
                    (Type::NumberWithUnit(u), Type::NumberWithUnit(v)) => {
                        if u == v {
                            Ok(Type::Bool)
                        } else {
                            Err(TypeError {
                                msg: format!(
                                    "operator '{}' cannot compare {} and {}",
                                    sym,
                                    u.display_name(),
                                    v.display_name()
                                ),
                                line: span.line,
                                col: span.col,
                            })
                        }
                    }
                    _ => Err(TypeError {
                        msg: format!(
                            "operator '{}' requires Number operands, got {} and {}",
                            sym,
                            lt.name(),
                            rt.name()
                        ),
                        line: span.line,
                        col: span.col,
                    }),
                }
            }
        }
    }

    /// Resolve a type annotation to a static Type.
    /// For `TypeAnnotation::Named`, looks up the state machine registry.
    fn resolve_annotation(&self, ann: &TypeAnnotation, span: Span) -> Result<Type, TypeError> {
        match ann {
            TypeAnnotation::Number => Ok(Type::Number),
            TypeAnnotation::NumberWithUnit(u) => Ok(Type::NumberWithUnit(UnitDimension::base(u))),
            TypeAnnotation::Text => Ok(Type::Text),
            TypeAnnotation::Bool => Ok(Type::Bool),
            TypeAnnotation::Nil => Ok(Type::Nil),
            TypeAnnotation::Named(name) => {
                if self.states.contains_key(name) {
                    Ok(Type::State(name.clone()))
                } else {
                    Err(TypeError {
                        msg: format!("unknown type '{}'", name),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
        }
    }

    fn build_fn_type(
        &self,
        params: &[Param],
        return_type: Option<&TypeAnnotation>,
        span: Span,
    ) -> Result<Type, TypeError> {
        let mut param_types = Vec::new();
        for param in params {
            param_types.push(self.resolve_annotation(&param.ty, span)?);
        }
        let ret = match return_type {
            Some(ann) => self.resolve_annotation(ann, span)?,
            None => Type::Unknown,
        };
        Ok(Type::Function {
            params: param_types,
            ret: Box::new(ret),
        })
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns true if `ty` is exactly `seconds` (the only supported time unit in M6A).
fn is_time_unit(ty: &Type) -> bool {
    match ty {
        Type::NumberWithUnit(dim) => {
            dim.exponents.len() == 1 && dim.exponents.get("seconds") == Some(&1)
        }
        _ => false,
    }
}
