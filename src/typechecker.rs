use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::{
    BinaryOp, CompoundAssignOp, Expr, Param, StateTransition, StateVariant, Stmt, TypeAnnotation,
    UnaryOp,
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

/// Static description of a struct type registered by a `struct` declaration.
#[derive(Clone)]
pub struct StructInfo {
    pub name: String,
    /// Field name → static type, in BTreeMap (alphabetical) order.
    pub fields: BTreeMap<String, Type>,
}

/// Type and optional known-state-variant for a single variable binding.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    /// True when declared with `let mut`; false for immutable `let` bindings.
    pub mutable: bool,
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
    /// A fixed-size homogeneous array. The inner type is the element type.
    Array(Box<Type>),
    /// A map with Text keys and homogeneous values. First type is key (always Text in M12A),
    /// second type is value element type.
    Map(Box<Type>, Box<Type>),
    /// A struct value. The String is the struct name.
    Struct(String),
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
            Type::Array(elem) => format!("Array<{}>", elem.name()),
            Type::Map(k, v) => format!("Map<{}, {}>", k.name(), v.name()),
            Type::Struct(s) => s.clone(),
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

    /// Define an immutable variable with no known state variant.
    pub fn define(&mut self, name: String, ty: Type) {
        self.scopes
            .last_mut()
            .expect("TypeEnv scope stack empty")
            .insert(
                name,
                VarInfo {
                    ty,
                    mutable: false,
                    known_state_variant: None,
                },
            );
    }

    /// Define a variable with explicit mutability and an optional known state variant.
    pub fn define_with_variant(
        &mut self,
        name: String,
        ty: Type,
        variant: Option<String>,
        mutable: bool,
    ) {
        self.scopes
            .last_mut()
            .expect("TypeEnv scope stack empty")
            .insert(
                name,
                VarInfo {
                    ty,
                    mutable,
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
    /// Registry of declared struct types. Populated by struct declaration pre-pass.
    structs: HashMap<String, StructInfo>,
    /// Number of while loops currently enclosing the statement being checked.
    /// `break`/`continue` require this to be > 0.
    /// Reset to 0 on entry to a function or simulate body.
    loop_depth: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            env: TypeEnv::new(),
            current_fn_return_type: None,
            states: HashMap::new(),
            structs: HashMap::new(),
            loop_depth: 0,
        }
    }

    /// Type-check a program (top-level statement list).
    pub fn check(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        self.check_stmt_list(stmts)
    }

    /// Type-check a list of statements.
    ///
    /// Three-pass design:
    ///   Pass 1 — register all state machine and struct declarations
    ///   Pass 2 — register all function signatures (enables mutual recursion type-checking)
    ///   Pass 3 — check all statements
    fn check_stmt_list(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        // Pass 1: register state machines and struct types.
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
            if let Stmt::StructDecl { name, fields, span } = stmt {
                self.register_struct(name, fields, *span)?;
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

    fn register_struct(
        &mut self,
        name: &str,
        fields: &[(String, TypeAnnotation)],
        span: Span,
    ) -> Result<(), TypeError> {
        if self.structs.contains_key(name) {
            return Err(TypeError {
                msg: format!("duplicate struct '{}'", name),
                line: span.line,
                col: span.col,
            });
        }
        if fields.is_empty() {
            return Err(TypeError {
                msg: format!("struct '{}' must have at least one field", name),
                line: span.line,
                col: span.col,
            });
        }
        let mut field_map: BTreeMap<String, Type> = BTreeMap::new();
        for (field_name, ann) in fields {
            if field_map.contains_key(field_name.as_str()) {
                return Err(TypeError {
                    msg: format!("duplicate field '{}' in struct '{}'", field_name, name),
                    line: span.line,
                    col: span.col,
                });
            }
            let ty = self.resolve_annotation(ann, span)?;
            field_map.insert(field_name.clone(), ty);
        }
        self.structs.insert(
            name.to_string(),
            StructInfo {
                name: name.to_string(),
                fields: field_map,
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

            Stmt::StructDecl { .. } => {
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
                mutable,
                annotation,
                value,
                span,
            } => {
                // Pre-resolve annotation so we can pass the expected type to the expression
                // checker — this enables empty array literals when the annotation is Array<T>.
                let ann_ty_opt: Option<Type> = match annotation.as_ref() {
                    Some(ann) => Some(self.resolve_annotation(ann, *span)?),
                    None => None,
                };

                let val_ty = self.check_expr_with_expected(value, ann_ty_opt.as_ref(), *span)?;

                // Extract known variant when the initializer is a direct state variant expression.
                let known_variant = match value {
                    Expr::StateVariant { variant_name, .. } => Some(variant_name.clone()),
                    _ => None,
                };

                let (effective_ty, effective_variant) = if let Some(ann_ty) = ann_ty_opt {
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

                self.env.define_with_variant(
                    name.clone(),
                    effective_ty,
                    effective_variant,
                    *mutable,
                );
                Ok(())
            }

            Stmt::Assign { name, value, span } => {
                let (var_ty, var_mutable) = self
                    .env
                    .get(name)
                    .map(|vi| (vi.ty.clone(), vi.mutable))
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    })?;

                // State variables must use `transition`, not assignment.
                if matches!(var_ty, Type::State(_)) {
                    return Err(TypeError {
                        msg: "state variables must be changed with transition, not assignment"
                            .into(),
                        line: span.line,
                        col: span.col,
                    });
                }

                if !var_mutable {
                    return Err(TypeError {
                        msg: format!("cannot assign to immutable variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    });
                }

                // Pass the variable's type as expected so that `arr = []` is valid
                // when the variable is already typed as Array<T>.
                let val_ty = self.check_expr_with_expected(value, Some(&var_ty), *span)?;
                let compatible = val_ty.is_unknown()
                    || var_ty.is_unknown()
                    || val_ty == var_ty
                    || (matches!(&var_ty, Type::NumberWithUnit(_)) && val_ty == Type::Number);
                if !compatible {
                    return Err(TypeError {
                        msg: format!(
                            "variable '{}' has type {} but assigned value has type {}",
                            name,
                            var_ty.name(),
                            val_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(())
            }

            Stmt::CompoundAssign {
                name,
                op,
                value,
                span,
            } => {
                let (var_ty, var_mutable) = self
                    .env
                    .get(name)
                    .map(|vi| (vi.ty.clone(), vi.mutable))
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    })?;

                if matches!(var_ty, Type::State(_)) {
                    return Err(TypeError {
                        msg: "state variables must be changed with transition, not compound assignment".into(),
                        line: span.line,
                        col: span.col,
                    });
                }

                if !var_mutable {
                    return Err(TypeError {
                        msg: format!("cannot assign to immutable variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    });
                }

                let rhs_ty = self.check_expr(value, *span)?;
                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };
                let result_ty = self.check_binary(&binary_op, var_ty.clone(), rhs_ty, *span)?;
                let compatible = result_ty.is_unknown()
                    || var_ty.is_unknown()
                    || result_ty == var_ty
                    || (matches!(&var_ty, Type::NumberWithUnit(_)) && result_ty == Type::Number);
                if !compatible {
                    return Err(TypeError {
                        msg: format!(
                            "variable '{}' has type {} but compound assignment result has type {}",
                            name,
                            var_ty.name(),
                            result_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(())
            }

            Stmt::IndexAssign {
                name,
                index,
                value,
                span,
            } => {
                let (var_ty, var_mutable) = self
                    .env
                    .get(name)
                    .map(|vi| (vi.ty.clone(), vi.mutable))
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    })?;

                if !var_mutable {
                    return Err(TypeError {
                        msg: format!("cannot assign to immutable variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    });
                }

                match var_ty {
                    Type::Array(elem_ty) => {
                        let idx_ty = self.check_expr(index, *span)?;
                        if !idx_ty.is_unknown() && idx_ty != Type::Number {
                            return Err(TypeError {
                                msg: format!("array index must be Number, got {}", idx_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let val_ty = self.check_expr(value, *span)?;
                        let compatible = val_ty.is_unknown()
                            || elem_ty.is_unknown()
                            || val_ty == *elem_ty
                            || (matches!(&*elem_ty, Type::NumberWithUnit(_))
                                && val_ty == Type::Number);
                        if !compatible {
                            return Err(TypeError {
                                msg: format!(
                                    "array element has type {} but assigned value has type {}",
                                    elem_ty.name(),
                                    val_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                    }
                    Type::Map(_, val_ty) => {
                        let key_ty = self.check_expr(index, *span)?;
                        if !key_ty.is_unknown() && key_ty != Type::Text {
                            return Err(TypeError {
                                msg: format!("map index key must be Text, got {}", key_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let assigned_ty = self.check_expr(value, *span)?;
                        let compatible = assigned_ty.is_unknown()
                            || val_ty.is_unknown()
                            || assigned_ty == *val_ty
                            || (matches!(&*val_ty, Type::NumberWithUnit(_))
                                && assigned_ty == Type::Number);
                        if !compatible {
                            return Err(TypeError {
                                msg: format!(
                                    "map value has type {} but assigned value has type {}",
                                    val_ty.name(),
                                    assigned_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                    }
                    Type::Unknown => {
                        self.check_expr(index, *span)?;
                        self.check_expr(value, *span)?;
                    }
                    other => {
                        return Err(TypeError {
                            msg: format!("cannot index-assign into value of type {}", other.name()),
                            line: span.line,
                            col: span.col,
                        });
                    }
                }
                Ok(())
            }

            Stmt::IndexCompoundAssign {
                name,
                index,
                op,
                value,
                span,
            } => {
                let (var_ty, var_mutable) = self
                    .env
                    .get(name)
                    .map(|vi| (vi.ty.clone(), vi.mutable))
                    .ok_or_else(|| TypeError {
                        msg: format!("undefined variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    })?;

                if !var_mutable {
                    return Err(TypeError {
                        msg: format!("cannot assign to immutable variable '{}'", name),
                        line: span.line,
                        col: span.col,
                    });
                }

                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };

                match var_ty {
                    Type::Array(elem_ty) => {
                        let idx_ty = self.check_expr(index, *span)?;
                        if !idx_ty.is_unknown() && idx_ty != Type::Number {
                            return Err(TypeError {
                                msg: format!("array index must be Number, got {}", idx_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let rhs_ty = self.check_expr(value, *span)?;
                        let result_ty =
                            self.check_binary(&binary_op, *elem_ty.clone(), rhs_ty, *span)?;
                        let compatible = result_ty.is_unknown()
                            || elem_ty.is_unknown()
                            || result_ty == *elem_ty
                            || (matches!(&*elem_ty, Type::NumberWithUnit(_))
                                && result_ty == Type::Number);
                        if !compatible {
                            return Err(TypeError {
                                msg: format!(
                                    "array element has type {} but compound assignment result has type {}",
                                    elem_ty.name(),
                                    result_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                    }
                    Type::Map(_, val_ty) => {
                        let key_ty = self.check_expr(index, *span)?;
                        if !key_ty.is_unknown() && key_ty != Type::Text {
                            return Err(TypeError {
                                msg: format!("map index key must be Text, got {}", key_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let rhs_ty = self.check_expr(value, *span)?;
                        let result_ty =
                            self.check_binary(&binary_op, *val_ty.clone(), rhs_ty, *span)?;
                        let compatible = result_ty.is_unknown()
                            || val_ty.is_unknown()
                            || result_ty == *val_ty
                            || (matches!(&*val_ty, Type::NumberWithUnit(_))
                                && result_ty == Type::Number);
                        if !compatible {
                            return Err(TypeError {
                                msg: format!(
                                    "map value has type {} but compound assignment result has type {}",
                                    val_ty.name(),
                                    result_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                    }
                    Type::Unknown => {
                        self.check_expr(index, *span)?;
                        self.check_expr(value, *span)?;
                    }
                    other => {
                        return Err(TypeError {
                            msg: format!(
                                "cannot index-compound-assign into value of type {}",
                                other.name()
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                }
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

                // Functions cannot inherit outer break/continue context.
                let saved_loop_depth = self.loop_depth;
                self.loop_depth = 0;

                self.env.push_scope();
                for param in params {
                    let param_ty = self.resolve_annotation(&param.ty, param.span)?;
                    self.env.define(param.name.clone(), param_ty);
                }
                let result = self.check_stmt_list(body);
                self.env.pop_scope();

                self.loop_depth = saved_loop_depth;
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
                    Some(expr) => self.check_expr_with_expected(expr, Some(&declared), *span)?,
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

            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.check_expr(condition, *span)?;
                if !cond_ty.is_unknown() && cond_ty != Type::Bool {
                    return Err(TypeError {
                        msg: format!("while condition must be Bool, got {}", cond_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }
                self.loop_depth += 1;
                self.env.push_scope();
                let result = self.check_stmt_list(body);
                self.env.pop_scope();
                self.loop_depth -= 1;
                result
            }

            Stmt::Break { span } => {
                if self.loop_depth == 0 {
                    return Err(TypeError {
                        msg: "'break' used outside of a while loop".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(())
            }

            Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    return Err(TypeError {
                        msg: "'continue' used outside of a while loop".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(())
            }

            Stmt::ForRange {
                var_name,
                start,
                end,
                body,
                span,
            } => {
                let start_ty = self.check_expr(start, *span)?;
                if !start_ty.is_unknown() && start_ty != Type::Number {
                    return Err(TypeError {
                        msg: format!("range start must be Number, got {}", start_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }

                let end_ty = self.check_expr(end, *span)?;
                if !end_ty.is_unknown() && end_ty != Type::Number {
                    return Err(TypeError {
                        msg: format!("range end must be Number, got {}", end_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }

                // Loop variable is immutable Number, scoped to the loop body.
                self.env.push_scope();
                self.env.define_with_variant(
                    var_name.clone(),
                    Type::Number,
                    None,
                    false, // immutable
                );
                self.loop_depth += 1;
                let result = self.check_stmt_list(body);
                self.loop_depth -= 1;
                self.env.pop_scope();
                result
            }

            Stmt::ForEach {
                var_name,
                iterable,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iterable, *span)?;
                let elem_ty = match iter_ty {
                    Type::Array(elem) => *elem,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(TypeError {
                            msg: format!("for-each requires Array, got {}", other.name()),
                            line: span.line,
                            col: span.col,
                        })
                    }
                };
                self.env.push_scope();
                self.env
                    .define_with_variant(var_name.clone(), elem_ty, None, false);
                self.loop_depth += 1;
                let result = self.check_stmt_list(body);
                self.loop_depth -= 1;
                self.env.pop_scope();
                result
            }

            Stmt::ForEachIndexed {
                index_name,
                var_name,
                iterable,
                body,
                span,
            } => {
                if index_name == var_name {
                    return Err(TypeError {
                        msg: "indexed for-each variable names must be distinct".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                let iter_ty = self.check_expr(iterable, *span)?;
                let elem_ty = match iter_ty {
                    Type::Array(elem) => *elem,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(TypeError {
                            msg: format!("for-each requires Array, got {}", other.name()),
                            line: span.line,
                            col: span.col,
                        })
                    }
                };
                self.env.push_scope();
                self.env
                    .define_with_variant(index_name.clone(), Type::Number, None, false);
                self.env
                    .define_with_variant(var_name.clone(), elem_ty, None, false);
                self.loop_depth += 1;
                let result = self.check_stmt_list(body);
                self.loop_depth -= 1;
                self.env.pop_scope();
                result
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
                            "simulate duration must be a time unit (seconds, milliseconds, minutes, hours), got {}",
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

                // Simulate bodies cannot inherit outer break/continue context.
                let saved_loop_depth = self.loop_depth;
                self.loop_depth = 0;

                self.env.push_scope();
                self.env.define("time".to_string(), time_ty);
                let result = self.check_stmt_list(body);
                self.env.pop_scope();

                self.loop_depth = saved_loop_depth;
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
                // Check state machine registry first.
                if let Some(sm) = self.states.get(state_name) {
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
                    return Ok(Type::State(state_name.clone()));
                }

                // Check if it's a struct variable field access (e.g. `u.name`).
                if let Some(vi) = self.env.get(state_name) {
                    let obj_ty = vi.ty.clone();
                    if obj_ty.is_unknown() {
                        return Ok(Type::Unknown);
                    }
                    if let Type::Struct(ref sname) = obj_ty {
                        let sname = sname.clone();
                        let field_ty = self
                            .structs
                            .get(&sname)
                            .and_then(|si| si.fields.get(variant_name))
                            .cloned()
                            .ok_or_else(|| TypeError {
                                msg: format!("struct '{}' has no field '{}'", sname, variant_name),
                                line: span.line,
                                col: span.col,
                            })?;
                        return Ok(field_ty);
                    }
                    return Err(TypeError {
                        msg: format!(
                            "'{}' has type {}, which has no fields",
                            state_name,
                            obj_ty.name()
                        ),
                        line: span.line,
                        col: span.col,
                    });
                }

                Err(TypeError {
                    msg: format!("unknown state machine or struct variable '{}'", state_name),
                    line: span.line,
                    col: span.col,
                })
            }

            Expr::StructLiteral { name, fields, span } => {
                let struct_info = self
                    .structs
                    .get(name)
                    .ok_or_else(|| TypeError {
                        msg: format!("unknown struct '{}'", name),
                        line: span.line,
                        col: span.col,
                    })?
                    .clone();

                let declared_fields = struct_info.fields.clone();
                let mut provided: HashSet<String> = HashSet::new();

                for (field_name, field_expr) in fields {
                    if provided.contains(field_name.as_str()) {
                        return Err(TypeError {
                            msg: format!(
                                "duplicate field '{}' in struct '{}' literal",
                                field_name, name
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    let expected_ty =
                        declared_fields
                            .get(field_name.as_str())
                            .ok_or_else(|| TypeError {
                                msg: format!("struct '{}' has no field '{}'", name, field_name),
                                line: span.line,
                                col: span.col,
                            })?;
                    let actual_ty = self.check_expr(field_expr, *span)?;
                    let compatible = actual_ty.is_unknown()
                        || actual_ty == *expected_ty
                        || (matches!(expected_ty, Type::NumberWithUnit(_))
                            && actual_ty == Type::Number);
                    if !compatible {
                        return Err(TypeError {
                            msg: format!(
                                "field '{}' of struct '{}' expects {}, got {}",
                                field_name,
                                name,
                                expected_ty.name(),
                                actual_ty.name()
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    provided.insert(field_name.clone());
                }

                for field_name in declared_fields.keys() {
                    if !provided.contains(field_name.as_str()) {
                        return Err(TypeError {
                            msg: format!(
                                "missing field '{}' in struct '{}' literal",
                                field_name, name
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                }

                Ok(Type::Struct(name.clone()))
            }

            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let obj_ty = self.check_expr(object, *span)?;
                if obj_ty.is_unknown() {
                    return Ok(Type::Unknown);
                }
                if let Type::Struct(ref sname) = obj_ty {
                    let sname = sname.clone();
                    let field_ty = self
                        .structs
                        .get(&sname)
                        .and_then(|si| si.fields.get(field.as_str()))
                        .cloned()
                        .ok_or_else(|| TypeError {
                            msg: format!("struct '{}' has no field '{}'", sname, field),
                            line: span.line,
                            col: span.col,
                        })?;
                    return Ok(field_ty);
                }
                Err(TypeError {
                    msg: format!(
                        "cannot access field '{}' on value of type {}",
                        field,
                        obj_ty.name()
                    ),
                    line: span.line,
                    col: span.col,
                })
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

            Expr::ArrayLiteral { elements, span } => {
                if elements.is_empty() {
                    return Err(TypeError {
                        msg: "empty array literal requires an explicit Array<T> type annotation"
                            .into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                let first_ty = self.check_expr(&elements[0], *span)?;
                for elem in elements.iter().skip(1) {
                    let elem_ty = self.check_expr(elem, *span)?;
                    if !elem_ty.is_unknown() && !first_ty.is_unknown() && elem_ty != first_ty {
                        return Err(TypeError {
                            msg: format!(
                                "array elements must have the same type; expected {} but got {}",
                                first_ty.name(),
                                elem_ty.name()
                            ),
                            line: span.line,
                            col: span.col,
                        });
                    }
                }
                Ok(Type::Array(Box::new(first_ty)))
            }

            Expr::Index { array, index, span } => {
                let arr_ty = self.check_expr(array, *span)?;
                let idx_ty = self.check_expr(index, *span)?;
                match arr_ty {
                    Type::Map(_, val_ty) => {
                        if !idx_ty.is_unknown() && idx_ty != Type::Text {
                            return Err(TypeError {
                                msg: format!("map key must be Text, got {}", idx_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        Ok(*val_ty)
                    }
                    Type::Array(elem) => {
                        if !idx_ty.is_unknown() && idx_ty != Type::Number {
                            return Err(TypeError {
                                msg: format!("index must be Number, got {}", idx_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        Ok(*elem)
                    }
                    Type::Text => {
                        if !idx_ty.is_unknown() && idx_ty != Type::Number {
                            return Err(TypeError {
                                msg: format!("index must be Number, got {}", idx_ty.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        Ok(Type::Text)
                    }
                    Type::Unknown => Ok(Type::Unknown),
                    other => Err(TypeError {
                        msg: format!("cannot index into value of type {}", other.name()),
                        line: span.line,
                        col: span.col,
                    }),
                }
            }

            Expr::MapLiteral { entries, span } => {
                if entries.is_empty() {
                    return Err(TypeError {
                        msg: "empty map literal requires an explicit Map type annotation"
                            .to_string(),
                        line: span.line,
                        col: span.col,
                    });
                }
                let mut val_ty: Option<Type> = None;
                for (key_expr, val_expr) in entries {
                    let k = self.check_expr(key_expr, *span)?;
                    if !k.is_unknown() && k != Type::Text {
                        return Err(TypeError {
                            msg: format!("map key must be Text, got {}", k.name()),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    let v = self.check_expr(val_expr, *span)?;
                    if matches!(v, Type::Map(..)) {
                        return Err(TypeError {
                            msg: "nested maps are not supported yet".to_string(),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    match &val_ty {
                        None => val_ty = Some(v),
                        Some(existing) => {
                            if !v.is_unknown() && !existing.is_unknown() && &v != existing {
                                return Err(TypeError {
                                    msg: format!(
                                        "map values must be homogeneous: expected {}, got {}",
                                        existing.name(),
                                        v.name()
                                    ),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                            if existing.is_unknown() && !v.is_unknown() {
                                val_ty = Some(v);
                            }
                        }
                    }
                }
                let vt = val_ty.unwrap_or(Type::Unknown);
                Ok(Type::Map(Box::new(Type::Text), Box::new(vt)))
            }

            Expr::Slice {
                array,
                start,
                end,
                span,
            } => {
                let arr_ty = self.check_expr(array, *span)?;
                let is_text = matches!(arr_ty, Type::Text);
                match &arr_ty {
                    Type::Array(_) | Type::Text | Type::Unknown => {}
                    other => {
                        return Err(TypeError {
                            msg: format!(
                                "slice target must be Array or Text, got {}",
                                other.name()
                            ),
                            line: span.line,
                            col: span.col,
                        })
                    }
                }
                let start_ty = self.check_expr(start, *span)?;
                if !start_ty.is_unknown() && start_ty != Type::Number {
                    return Err(TypeError {
                        msg: format!("slice start must be Number, got {}", start_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }
                let end_ty = self.check_expr(end, *span)?;
                if !end_ty.is_unknown() && end_ty != Type::Number {
                    return Err(TypeError {
                        msg: format!("slice end must be Number, got {}", end_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }
                if is_text {
                    return Ok(Type::Text);
                }
                let elem_ty = match arr_ty {
                    Type::Array(elem) => *elem,
                    _ => Type::Unknown,
                };
                Ok(Type::Array(Box::new(elem_ty)))
            }

            Expr::Call { callee, args, span } => {
                // `len` builtin: len(array) -> Number
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if name == "len" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("len() expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let arg_ty = self.check_expr(&args[0], *span)?;
                        match arg_ty {
                            Type::Array(_) | Type::Text | Type::Unknown => return Ok(Type::Number),
                            other => {
                                return Err(TypeError {
                                    msg: format!(
                                        "len() requires Array or Text, got {}",
                                        other.name()
                                    ),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        }
                    }

                    // `push` builtin: push(arr, value) -> Nil
                    if name == "push" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("push() expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let arr_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(TypeError {
                                    msg: "push() first argument must be a mutable array variable"
                                        .into(),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        let (arr_ty, arr_mutable) = self
                            .env
                            .get(&arr_name)
                            .map(|vi| (vi.ty.clone(), vi.mutable))
                            .ok_or_else(|| TypeError {
                                msg: format!("undefined variable '{}'", arr_name),
                                line: span.line,
                                col: span.col,
                            })?;
                        if !arr_mutable {
                            return Err(TypeError {
                                msg: format!("cannot mutate immutable variable '{}'", arr_name),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let elem_ty = match arr_ty {
                            Type::Array(elem) => *elem,
                            Type::Unknown => Type::Unknown,
                            other => {
                                return Err(TypeError {
                                    msg: format!("push() requires Array, got {}", other.name()),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        let val_ty = self.check_expr(&args[1], *span)?;
                        let compatible = val_ty.is_unknown()
                            || elem_ty.is_unknown()
                            || val_ty == elem_ty
                            || (matches!(&elem_ty, Type::NumberWithUnit(_))
                                && val_ty == Type::Number);
                        if !compatible {
                            return Err(TypeError {
                                msg: format!(
                                    "push() value has type {} but array element type is {}",
                                    val_ty.name(),
                                    elem_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Nil);
                    }

                    // `pop` builtin: pop(arr) -> T
                    if name == "pop" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("pop() expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let arr_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(TypeError {
                                    msg: "pop() argument must be a mutable array variable".into(),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        let (arr_ty, arr_mutable) = self
                            .env
                            .get(&arr_name)
                            .map(|vi| (vi.ty.clone(), vi.mutable))
                            .ok_or_else(|| TypeError {
                                msg: format!("undefined variable '{}'", arr_name),
                                line: span.line,
                                col: span.col,
                            })?;
                        if !arr_mutable {
                            return Err(TypeError {
                                msg: format!("cannot mutate immutable variable '{}'", arr_name),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let elem_ty = match arr_ty {
                            Type::Array(elem) => *elem,
                            Type::Unknown => Type::Unknown,
                            other => {
                                return Err(TypeError {
                                    msg: format!("pop() requires Array, got {}", other.name()),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        return Ok(elem_ty);
                    }

                    // `contains` builtin: contains(text, pattern) -> Bool
                    if name == "contains" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("contains() expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[0], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "contains() first argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t2 = self.check_expr(&args[1], *span)?;
                        if !t2.is_unknown() && t2 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "contains() second argument must be Text, got {}",
                                    t2.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Bool);
                    }

                    // `starts_with` builtin: starts_with(text, prefix) -> Bool
                    if name == "starts_with" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!(
                                    "starts_with() expects 2 arguments, got {}",
                                    args.len()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[0], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "starts_with() first argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t2 = self.check_expr(&args[1], *span)?;
                        if !t2.is_unknown() && t2 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "starts_with() second argument must be Text, got {}",
                                    t2.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Bool);
                    }

                    // `ends_with` builtin: ends_with(text, suffix) -> Bool
                    if name == "ends_with" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("ends_with() expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[0], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "ends_with() first argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t2 = self.check_expr(&args[1], *span)?;
                        if !t2.is_unknown() && t2 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "ends_with() second argument must be Text, got {}",
                                    t2.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Bool);
                    }

                    // `to_upper` builtin: to_upper(text) -> Text
                    if name == "to_upper" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("to_upper() expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t = self.check_expr(&args[0], *span)?;
                        if !t.is_unknown() && t != Type::Text {
                            return Err(TypeError {
                                msg: format!("to_upper() argument must be Text, got {}", t.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Text);
                    }

                    // `to_lower` builtin: to_lower(text) -> Text
                    if name == "to_lower" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("to_lower() expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t = self.check_expr(&args[0], *span)?;
                        if !t.is_unknown() && t != Type::Text {
                            return Err(TypeError {
                                msg: format!("to_lower() argument must be Text, got {}", t.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Text);
                    }

                    // `trim` builtin: trim(text) -> Text
                    if name == "trim" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("trim() expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t = self.check_expr(&args[0], *span)?;
                        if !t.is_unknown() && t != Type::Text {
                            return Err(TypeError {
                                msg: format!("trim() argument must be Text, got {}", t.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Text);
                    }

                    // `split` builtin: split(text, delimiter) -> Array<Text>
                    if name == "split" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("split() expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t0 = self.check_expr(&args[0], *span)?;
                        if !t0.is_unknown() && t0 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "split() first argument must be Text, got {}",
                                    t0.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[1], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "split() second argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Array(Box::new(Type::Text)));
                    }

                    // `join` builtin: join(Array<Text>, delimiter) -> Text
                    if name == "join" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("join() expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t0 = self.check_expr(&args[0], *span)?;
                        if !t0.is_unknown() && t0 != Type::Array(Box::new(Type::Text)) {
                            return Err(TypeError {
                                msg: format!(
                                    "join() first argument must be Array<Text>, got {}",
                                    t0.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[1], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "join() second argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Text);
                    }

                    // `has_key` builtin: has_key(Map<Text,V>, Text) -> Bool
                    if name == "has_key" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("has_key expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t0 = self.check_expr(&args[0], *span)?;
                        if !t0.is_unknown() && !matches!(t0, Type::Map(_, _)) {
                            return Err(TypeError {
                                msg: format!(
                                    "has_key() first argument must be Map, got {}",
                                    t0.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t1 = self.check_expr(&args[1], *span)?;
                        if !t1.is_unknown() && t1 != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "has_key() second argument must be Text, got {}",
                                    t1.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Bool);
                    }

                    // `keys` builtin: keys(Map<Text,V>) -> Array<Text>
                    if name == "keys" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("keys expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t0 = self.check_expr(&args[0], *span)?;
                        if !t0.is_unknown() && !matches!(t0, Type::Map(_, _)) {
                            return Err(TypeError {
                                msg: format!("keys() argument must be Map, got {}", t0.name()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(Type::Array(Box::new(Type::Text)));
                    }

                    // `values` builtin: values(Map<Text,V>) -> Array<V>
                    if name == "values" {
                        if args.len() != 1 {
                            return Err(TypeError {
                                msg: format!("values expects 1 argument, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let t0 = self.check_expr(&args[0], *span)?;
                        match &t0 {
                            Type::Map(_, v) => {
                                return Ok(Type::Array(v.clone()));
                            }
                            other if other.is_unknown() => {
                                return Ok(Type::Unknown);
                            }
                            other => {
                                return Err(TypeError {
                                    msg: format!(
                                        "values() argument must be Map, got {}",
                                        other.name()
                                    ),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        }
                    }

                    // `remove` builtin: remove(map, key) -> V
                    if name == "remove" {
                        if args.len() != 2 {
                            return Err(TypeError {
                                msg: format!("remove expects 2 arguments, got {}", args.len()),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let map_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(TypeError {
                                    msg: "remove() first argument must be a mutable map variable"
                                        .into(),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        let (map_ty, map_mutable) = self
                            .env
                            .get(&map_name)
                            .map(|vi| (vi.ty.clone(), vi.mutable))
                            .ok_or_else(|| TypeError {
                                msg: format!("undefined variable '{}'", map_name),
                                line: span.line,
                                col: span.col,
                            })?;
                        if !map_mutable {
                            return Err(TypeError {
                                msg: format!("cannot remove from immutable map '{}'", map_name),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        let val_ty = match map_ty {
                            Type::Map(_, v) => *v,
                            Type::Unknown => Type::Unknown,
                            other => {
                                return Err(TypeError {
                                    msg: format!(
                                        "remove() first argument must be Map, got {}",
                                        other.name()
                                    ),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        };
                        let key_ty = self.check_expr(&args[1], *span)?;
                        if !key_ty.is_unknown() && key_ty != Type::Text {
                            return Err(TypeError {
                                msg: format!(
                                    "remove() second argument must be Text, got {}",
                                    key_ty.name()
                                ),
                                line: span.line,
                                col: span.col,
                            });
                        }
                        return Ok(val_ty);
                    }
                }

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
                            let arg_ty =
                                self.check_expr_with_expected(arg, Some(expected), *span)?;
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
                } else if self.structs.contains_key(name) {
                    Ok(Type::Struct(name.clone()))
                } else {
                    Err(TypeError {
                        msg: format!("unknown type '{}'", name),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
            TypeAnnotation::Array(inner) => {
                let inner_ty = self.resolve_annotation(inner, span)?;
                Ok(Type::Array(Box::new(inner_ty)))
            }
            TypeAnnotation::Map(key_ann, val_ann) => {
                let key_ty = self.resolve_annotation(key_ann, span)?;
                if key_ty != Type::Text && !key_ty.is_unknown() {
                    return Err(TypeError {
                        msg: format!("map key type must be Text, got {}", key_ty.name()),
                        line: span.line,
                        col: span.col,
                    });
                }
                let val_ty = self.resolve_annotation(val_ann, span)?;
                if matches!(val_ty, Type::Map(..)) {
                    return Err(TypeError {
                        msg: "nested maps are not supported".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
                Ok(Type::Map(Box::new(Type::Text), Box::new(val_ty)))
            }
        }
    }

    /// Check an expression, allowing an empty array literal when the expected type is known.
    ///
    /// Only special-cases `Expr::ArrayLiteral { elements: [] }` — all other expressions
    /// (including non-empty array literals) delegate to `check_expr`.
    fn check_expr_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
        context_span: Span,
    ) -> Result<Type, TypeError> {
        if let Expr::ArrayLiteral { elements, span } = expr {
            if elements.is_empty() {
                return match expected {
                    Some(Type::Array(elem_ty)) => Ok(Type::Array(elem_ty.clone())),
                    _ => Err(TypeError {
                        msg: "empty array literal requires an explicit Array<T> type annotation"
                            .into(),
                        line: span.line,
                        col: span.col,
                    }),
                };
            }
        }
        if let Expr::MapLiteral { entries, span } = expr {
            if entries.is_empty() {
                return match expected {
                    Some(Type::Map(_, val_ty)) => {
                        Ok(Type::Map(Box::new(Type::Text), val_ty.clone()))
                    }
                    _ => Err(TypeError {
                        msg: "empty map literal requires an explicit Map<Text, V> type annotation"
                            .into(),
                        line: span.line,
                        col: span.col,
                    }),
                };
            }
        }
        self.check_expr(expr, context_span)
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

/// Returns true if `ty` is a single-exponent-1 time unit (seconds, milliseconds, minutes, hours).
fn is_time_unit(ty: &Type) -> bool {
    const TIME_UNITS: [&str; 4] = ["seconds", "milliseconds", "minutes", "hours"];
    match ty {
        Type::NumberWithUnit(dim) => {
            dim.exponents.len() == 1 && TIME_UNITS.iter().any(|&u| dim.exponents.get(u) == Some(&1))
        }
        _ => false,
    }
}
