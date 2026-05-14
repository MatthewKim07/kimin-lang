use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::env::Env;
use crate::error::RuntimeError;
use crate::value::Value;

pub struct Interpreter {
    env: Env,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter { env: Env::new() }
    }

    pub fn run(&mut self, stmts: &[Stmt]) -> Result<(), RuntimeError> {
        for stmt in stmts {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.env.set(name.clone(), v);
            }
            Stmt::Print { value } => {
                let v = self.eval_expr(value)?;
                println!("{}", v);
            }
            Stmt::Expr(expr) => {
                self.eval_expr(expr)?;
            }
            Stmt::Block(stmts) => {
                self.env.push_scope();
                let result = self.run(stmts);
                self.env.pop_scope(); // restore scope even on error
                result?;
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.eval_expr(cond)?;
                if is_truthy(&cond_val) {
                    self.exec_stmt(then_block)?;
                } else if let Some(else_b) = else_block {
                    self.exec_stmt(else_b)?;
                }
            }
        }
        Ok(())
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
