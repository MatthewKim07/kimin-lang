use std::collections::HashMap;

use crate::ast::{BinaryOp, Expr, Param, Stmt, TypeAnnotation, UnaryOp};
use crate::error::TypeError;
use crate::token::Span;

/// Static type representation used by the type checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Text,
    Bool,
    Nil,
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// Inferred or unannotated type — skips type checking on operations involving it.
    /// Used for functions without return-type annotations and for values whose type
    /// cannot be determined statically (e.g., calls to Unknown-returning functions).
    Unknown,
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::Number => "Number".into(),
            Type::Text => "Text".into(),
            Type::Bool => "Bool".into(),
            Type::Nil => "Nil".into(),
            Type::Function { .. } => "Function".into(),
            Type::Unknown => "Unknown".into(),
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }
}

/// Lexical scope stack for static types.
pub struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
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

    pub fn get(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t);
            }
        }
        None
    }

    pub fn define(&mut self, name: String, ty: Type) {
        self.scopes
            .last_mut()
            .expect("TypeEnv scope stack empty")
            .insert(name, ty);
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
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            env: TypeEnv::new(),
            current_fn_return_type: None,
        }
    }

    /// Type-check a program (top-level statement list).
    pub fn check(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        self.check_stmt_list(stmts)
    }

    /// Type-check a list of statements.
    /// Pre-registers all function declarations before checking any statement,
    /// so mutually recursive functions resolve correctly.
    fn check_stmt_list(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        // Pass 1: register function signatures so forward and mutual references work.
        for stmt in stmts {
            if let Stmt::FnDecl {
                name,
                params,
                return_type,
                ..
            } = stmt
            {
                let ty = build_fn_type(params, return_type.as_ref());
                self.env.define(name.clone(), ty);
            }
        }
        // Pass 2: check all statements.
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::Let {
                name,
                annotation,
                value,
                span,
            } => {
                let val_ty = self.check_expr(value, *span)?;
                if let Some(ann) = annotation {
                    let ann_ty = annotation_to_type(ann);
                    if !val_ty.is_unknown() && val_ty != ann_ty {
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
                    self.env.define(name.clone(), ann_ty);
                } else {
                    self.env.define(name.clone(), val_ty);
                }
                Ok(())
            }

            Stmt::Print { value } => {
                let ty = self.check_expr(value, Span { line: 0, col: 0 })?;
                // Function values are not printable.
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
                ..
            } => {
                // Signature already registered by the pre-pass in check_stmt_list.
                // Push a new scope, bind params, check body.
                let saved_ret = self.current_fn_return_type.take();
                self.current_fn_return_type = Some(
                    return_type
                        .as_ref()
                        .map(annotation_to_type)
                        .unwrap_or(Type::Unknown),
                );

                self.env.push_scope();
                for param in params {
                    self.env
                        .define(param.name.clone(), annotation_to_type(&param.ty));
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
                if !declared.is_unknown() && !ret_ty.is_unknown() && ret_ty != declared {
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
        }
    }

    /// Type-check an expression and return its static type.
    /// `context_span` is used for errors on nodes that carry no span themselves.
    fn check_expr(&mut self, expr: &Expr, context_span: Span) -> Result<Type, TypeError> {
        match expr {
            Expr::Number(_) => Ok(Type::Number),
            Expr::Str(_) => Ok(Type::Text),
            Expr::Bool(_) => Ok(Type::Bool),

            Expr::Variable { name, span } => self.env.get(name).cloned().ok_or_else(|| TypeError {
                msg: format!("undefined variable '{}'", name),
                line: span.line,
                col: span.col,
            }),

            Expr::Grouping(inner) => self.check_expr(inner, context_span),

            Expr::Unary { op, operand } => {
                let ty = self.check_expr(operand, context_span)?;
                if ty.is_unknown() {
                    return Ok(Type::Unknown);
                }
                match op {
                    UnaryOp::Neg => {
                        if ty != Type::Number {
                            return Err(TypeError {
                                msg: format!("unary '-' requires Number, got {}", ty.name()),
                                line: context_span.line,
                                col: context_span.col,
                            });
                        }
                        Ok(Type::Number)
                    }
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
                // Extract name for error messages when callee is a simple variable.
                let callee_name = if let Expr::Variable { name, .. } = callee.as_ref() {
                    name.as_str()
                } else {
                    "<expression>"
                };
                match callee_ty {
                    Type::Unknown => {
                        // Unknown callee — check args but don't error on callee type.
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
                            if !arg_ty.is_unknown() && !expected.is_unknown() && arg_ty != *expected
                            {
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
        // Unknown on either side → propagate Unknown without error.
        if lt.is_unknown() || rt.is_unknown() {
            return Ok(Type::Unknown);
        }

        match op {
            BinaryOp::Add => match (&lt, &rt) {
                (Type::Number, Type::Number) => Ok(Type::Number),
                (Type::Text, Type::Text) => Ok(Type::Text),
                _ => Err(TypeError {
                    msg: format!(
                        "operator '+' expected Number + Number or Text + Text, got {} + {}",
                        lt.name(),
                        rt.name()
                    ),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                let sym = match op {
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    _ => unreachable!(),
                };
                if lt == Type::Number && rt == Type::Number {
                    Ok(Type::Number)
                } else {
                    Err(TypeError {
                        msg: format!(
                            "operator '{}' requires Number operands, got {} and {}",
                            sym,
                            lt.name(),
                            rt.name()
                        ),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                if lt != rt {
                    let sym = if matches!(op, BinaryOp::Eq) {
                        "=="
                    } else {
                        "!="
                    };
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
                if lt == Type::Number && rt == Type::Number {
                    Ok(Type::Bool)
                } else {
                    Err(TypeError {
                        msg: format!(
                            "operator '{}' requires Number operands, got {} and {}",
                            sym,
                            lt.name(),
                            rt.name()
                        ),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

// --- helpers ---

pub fn annotation_to_type(ann: &TypeAnnotation) -> Type {
    match ann {
        TypeAnnotation::Number => Type::Number,
        TypeAnnotation::Text => Type::Text,
        TypeAnnotation::Bool => Type::Bool,
        TypeAnnotation::Nil => Type::Nil,
    }
}

pub fn build_fn_type(params: &[Param], return_type: Option<&TypeAnnotation>) -> Type {
    Type::Function {
        params: params.iter().map(|p| annotation_to_type(&p.ty)).collect(),
        ret: Box::new(return_type.map(annotation_to_type).unwrap_or(Type::Unknown)),
    }
}
