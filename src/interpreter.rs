use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::env::{Env, EnvRef};
use crate::error::RuntimeError;
use crate::value::{FunctionValue, Value};

/// Internal control-flow signal used to propagate `return` through nested statements.
/// This is not a runtime error — function calls catch Return; top-level run() turns it into an error.
enum ExecFlow {
    Continue,
    Return(Value),
}

pub struct Interpreter {
    env: EnvRef,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            env: Env::new_global(),
        }
    }

    /// Execute a program (top-level statement list). A Return reaching here is a runtime error.
    pub fn run(&mut self, stmts: &[Stmt]) -> Result<(), RuntimeError> {
        match self.exec_stmts(stmts)? {
            ExecFlow::Continue => Ok(()),
            ExecFlow::Return(_) => Err(RuntimeError {
                msg: "cannot return outside of a function".into(),
            }),
        }
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        self.env.borrow().get(name)
    }

    /// Run a list of statements, propagating any Return upward immediately.
    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<ExecFlow, RuntimeError> {
        for stmt in stmts {
            match self.exec_stmt(stmt)? {
                ExecFlow::Continue => {}
                flow @ ExecFlow::Return(_) => return Ok(flow),
            }
        }
        Ok(ExecFlow::Continue)
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<ExecFlow, RuntimeError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.env.borrow_mut().define(name.clone(), v);
                Ok(ExecFlow::Continue)
            }
            Stmt::Print { value } => {
                let v = self.eval_expr(value)?;
                println!("{}", v);
                Ok(ExecFlow::Continue)
            }
            Stmt::Expr(expr) => {
                self.eval_expr(expr)?;
                Ok(ExecFlow::Continue)
            }
            Stmt::Block(stmts) => {
                let outer = Rc::clone(&self.env);
                self.env = Env::new_child(Rc::clone(&self.env));
                let result = self.exec_stmts(stmts);
                self.env = outer; // restore even if execution errored
                result
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.eval_expr(cond)?;
                if is_truthy(&cond_val) {
                    self.exec_stmt(then_block)
                } else if let Some(else_b) = else_block {
                    self.exec_stmt(else_b)
                } else {
                    Ok(ExecFlow::Continue)
                }
            }
            Stmt::FnDecl {
                name, params, body, ..
            } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                let func = FunctionValue {
                    name: name.clone(),
                    params: param_names,
                    body: body.clone(),
                    // Capture the current env at declaration time (lexical scoping).
                    // Defining the function into this same env makes the name visible
                    // to recursive calls — the closure_env and the define target share
                    // the same Rc<RefCell<Env>>.
                    closure_env: Rc::clone(&self.env),
                };
                self.env
                    .borrow_mut()
                    .define(name.clone(), Value::Function(func));
                Ok(ExecFlow::Continue)
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Nil,
                };
                Ok(ExecFlow::Return(v))
            }

            Stmt::StateDecl { .. } => {
                // State machine declarations are purely static — no runtime work.
                Ok(ExecFlow::Continue)
            }

            Stmt::Transition {
                variable, target, ..
            } => {
                let current = self
                    .env
                    .borrow()
                    .get(variable)
                    .ok_or_else(|| RuntimeError {
                        msg: format!("undefined variable '{}'", variable),
                    })?;
                let state_name = match &current {
                    Value::StateValue { state_name, .. } => state_name.clone(),
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "'{}' is not a state value, got {}",
                                variable,
                                other.type_name()
                            ),
                        });
                    }
                };
                let new_val = Value::StateValue {
                    state_name,
                    variant_name: target.clone(),
                };
                let found = self.env.borrow_mut().assign_existing(variable, new_val);
                if !found {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", variable),
                    });
                }
                Ok(ExecFlow::Continue)
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),

            Expr::StateVariant {
                state_name,
                variant_name,
                ..
            } => Ok(Value::StateValue {
                state_name: state_name.clone(),
                variant_name: variant_name.clone(),
            }),

            Expr::Variable { name, .. } => {
                self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })
            }

            Expr::Grouping(inner) => self.eval_expr(inner),

            Expr::Unary { op, operand } => {
                let v = self.eval_expr(operand)?;
                match op {
                    UnaryOp::Neg => match &v {
                        Value::Number(n) => Ok(Value::Number(-*n)),
                        other => Err(RuntimeError {
                            msg: format!("cannot negate {}", other.type_name()),
                        }),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!is_truthy(&v))),
                }
            }

            Expr::Binary { op, left, right } => {
                let lv = self.eval_expr(left)?;
                let rv = self.eval_expr(right)?;
                eval_binary(op, lv, rv)
            }

            Expr::Call { callee, args, .. } => {
                let callee_val = self.eval_expr(callee)?;
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                match callee_val {
                    Value::Function(func) => self.call_function(&func, arg_vals),
                    other => Err(RuntimeError {
                        msg: format!("attempted to call non-function value {}", other.type_name()),
                    }),
                }
            }
        }
    }

    fn call_function(
        &mut self,
        func: &FunctionValue,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != func.params.len() {
            return Err(RuntimeError {
                msg: format!(
                    "function '{}' expected {} argument{} but got {}",
                    func.name,
                    func.params.len(),
                    if func.params.len() == 1 { "" } else { "s" },
                    args.len()
                ),
            });
        }

        // New frame whose parent is the closure's captured environment (lexical scoping).
        // The function sees its definition-site variables, not the call-site variables.
        let call_frame = Env::new_child(Rc::clone(&func.closure_env));
        for (param, arg) in func.params.iter().zip(args.into_iter()) {
            call_frame.borrow_mut().define(param.clone(), arg);
        }

        let outer = Rc::clone(&self.env);
        self.env = call_frame;
        let body: Vec<Stmt> = func.body.clone();
        let result = self.exec_stmts(&body);
        self.env = outer; // restore even if body errored

        match result? {
            ExecFlow::Continue => Ok(Value::Nil),
            ExecFlow::Return(v) => Ok(v),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Nil => false,
        _ => true,
    }
}

fn eval_binary(op: &BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    match op {
        BinaryOp::Add => match (&left, &right) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
            _ => Err(RuntimeError {
                msg: format!("cannot add {} and {}", left.type_name(), right.type_name()),
            }),
        },
        BinaryOp::Sub => numeric_op(&left, &right, "-", |a, b| a - b),
        BinaryOp::Mul => numeric_op(&left, &right, "*", |a, b| a * b),
        BinaryOp::Div => {
            if let (Value::Number(_), Value::Number(b)) = (&left, &right) {
                if *b == 0.0 {
                    return Err(RuntimeError {
                        msg: "division by zero".into(),
                    });
                }
            }
            numeric_op(&left, &right, "/", |a, b| a / b)
        }
        BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
        BinaryOp::NotEq => Ok(Value::Bool(!values_equal(&left, &right))),
        BinaryOp::Lt => numeric_cmp(&left, &right, "<", |a, b| a < b),
        BinaryOp::LtEq => numeric_cmp(&left, &right, "<=", |a, b| a <= b),
        BinaryOp::Gt => numeric_cmp(&left, &right, ">", |a, b| a > b),
        BinaryOp::GtEq => numeric_cmp(&left, &right, ">=", |a, b| a >= b),
    }
}

fn numeric_op(
    left: &Value,
    right: &Value,
    op: &str,
    f: impl Fn(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(*a, *b))),
        _ => Err(RuntimeError {
            msg: format!(
                "cannot apply '{}' to {} and {}",
                op,
                left.type_name(),
                right.type_name()
            ),
        }),
    }
}

fn numeric_cmp(
    left: &Value,
    right: &Value,
    op: &str,
    f: impl Fn(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(f(*a, *b))),
        _ => Err(RuntimeError {
            msg: format!(
                "cannot compare {} and {} with '{}'",
                left.type_name(),
                right.type_name(),
                op
            ),
        }),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}
