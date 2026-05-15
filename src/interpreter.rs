use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::env::Env;
use crate::error::RuntimeError;
use crate::value::{FunctionValue, Value};

/// Internal control-flow signal used to propagate `return` through nested statements.
/// This is not a runtime error — function calls catch Return; top-level run() turns it into an error.
enum ExecFlow {
    Continue,
    Return(Value),
}

pub struct Interpreter {
    env: Env,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter { env: Env::new() }
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
        self.env.get(name)
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
                self.env.set(name.clone(), v);
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
                self.env.push_scope();
                let result = self.exec_stmts(stmts);
                self.env.pop_scope(); // restore scope even on error
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
                let func = FunctionValue {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                self.env.set(name.clone(), Value::Function(func));
                Ok(ExecFlow::Continue)
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Nil,
                };
                Ok(ExecFlow::Return(v))
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),

            Expr::Variable { name, .. } => self.env.get(name).ok_or_else(|| RuntimeError {
                msg: format!("undefined variable '{}'", name),
            }),

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
                // Evaluate callee and all arguments before checking types.
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

        self.env.push_scope();
        for (param, arg) in func.params.iter().zip(args.into_iter()) {
            self.env.set(param.clone(), arg);
        }
        // Save body reference before executing so the borrow is clear.
        let body: Vec<Stmt> = func.body.clone();
        let result = self.exec_stmts(&body);
        self.env.pop_scope(); // restore even if execution errored

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
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil, Value::Nil) => true,
        _ => false,
    }
}
