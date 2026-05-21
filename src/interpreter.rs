use std::rc::Rc;

use crate::ast::{BinaryOp, CompoundAssignOp, Expr, Stmt, UnaryOp};
use crate::env::{Env, EnvRef};
use crate::error::RuntimeError;
use crate::value::{FunctionValue, Value};

/// Internal control-flow signal used to propagate `return`, `break`, and `continue`
/// through nested statements.
/// - Normal:   keep executing statements (was previously named `Continue`)
/// - Return:   propagate return value out to the enclosing function call
/// - Break:    exit the nearest enclosing while loop
/// - Continue: skip remainder of current while-body iteration
enum ExecFlow {
    Normal,
    Return(Value),
    Break,
    Continue,
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
            ExecFlow::Normal => Ok(()),
            ExecFlow::Return(_) => Err(RuntimeError {
                msg: "cannot return outside of a function".into(),
            }),
            ExecFlow::Break => Err(RuntimeError {
                msg: "'break' used outside of a while loop".into(),
            }),
            ExecFlow::Continue => Err(RuntimeError {
                msg: "'continue' used outside of a while loop".into(),
            }),
        }
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        self.env.borrow().get(name)
    }

    /// Run a list of statements, propagating Return/Break/Continue upward immediately.
    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<ExecFlow, RuntimeError> {
        for stmt in stmts {
            match self.exec_stmt(stmt)? {
                ExecFlow::Normal => {}
                flow => return Ok(flow),
            }
        }
        Ok(ExecFlow::Normal)
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<ExecFlow, RuntimeError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.env.borrow_mut().define(name.clone(), v);
                Ok(ExecFlow::Normal)
            }
            Stmt::Assign { name, value, .. } => {
                let v = self.eval_expr(value)?;
                let found = self.env.borrow_mut().assign_existing(name, v);
                if !found {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", name),
                    });
                }
                Ok(ExecFlow::Normal)
            }
            Stmt::CompoundAssign {
                name, op, value, ..
            } => {
                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;
                let rhs = self.eval_expr(value)?;
                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };
                let result = eval_binary(&binary_op, current, rhs)?;
                let found = self.env.borrow_mut().assign_existing(name, result);
                if !found {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", name),
                    });
                }
                Ok(ExecFlow::Normal)
            }
            Stmt::IndexAssign {
                name, index, value, ..
            } => {
                // Evaluate index first, then value (source order).
                let idx_val = self.eval_expr(index)?;
                let new_elem = self.eval_expr(value)?;

                // Read current array value, clone its Vec, update, assign back.
                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;
                let mut elems = match current {
                    Value::Array(v) => v,
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "index assignment target '{}' is not an array, got {}",
                                name,
                                other.type_name()
                            ),
                        })
                    }
                };

                let n = match idx_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("array index must be Number, got {}", other.type_name()),
                        })
                    }
                };
                if n.fract() != 0.0 {
                    return Err(RuntimeError {
                        msg: format!("array index must be an integer, got {}", n),
                    });
                }
                if n < 0.0 {
                    return Err(RuntimeError {
                        msg: format!("array index out of bounds: index {} is negative", n as i64),
                    });
                }
                let i = n as usize;
                if i >= elems.len() {
                    return Err(RuntimeError {
                        msg: format!(
                            "array index out of bounds: index {} but length is {}",
                            i,
                            elems.len()
                        ),
                    });
                }
                elems[i] = new_elem;
                let found = self
                    .env
                    .borrow_mut()
                    .assign_existing(name, Value::Array(elems));
                if !found {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", name),
                    });
                }
                Ok(ExecFlow::Normal)
            }

            Stmt::IndexCompoundAssign {
                name,
                index,
                op,
                value,
                ..
            } => {
                // Evaluate index first, then read array, then evaluate rhs.
                let idx_val = self.eval_expr(index)?;

                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;
                let mut elems = match current {
                    Value::Array(v) => v,
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "index compound assignment target '{}' is not an array, got {}",
                                name,
                                other.type_name()
                            ),
                        })
                    }
                };

                let n = match idx_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("array index must be Number, got {}", other.type_name()),
                        })
                    }
                };
                if n.fract() != 0.0 {
                    return Err(RuntimeError {
                        msg: format!("array index must be an integer, got {}", n),
                    });
                }
                if n < 0.0 {
                    return Err(RuntimeError {
                        msg: format!("array index out of bounds: index {} is negative", n as i64),
                    });
                }
                let i = n as usize;
                if i >= elems.len() {
                    return Err(RuntimeError {
                        msg: format!(
                            "array index out of bounds: index {} but length is {}",
                            i,
                            elems.len()
                        ),
                    });
                }

                let old_elem = elems[i].clone();
                let rhs = self.eval_expr(value)?;
                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };
                let new_elem = eval_binary(&binary_op, old_elem, rhs)?;
                elems[i] = new_elem;
                let found = self
                    .env
                    .borrow_mut()
                    .assign_existing(name, Value::Array(elems));
                if !found {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", name),
                    });
                }
                Ok(ExecFlow::Normal)
            }

            Stmt::Print { value } => {
                let v = self.eval_expr(value)?;
                println!("{}", v);
                Ok(ExecFlow::Normal)
            }
            Stmt::Expr(expr) => {
                self.eval_expr(expr)?;
                Ok(ExecFlow::Normal)
            }
            Stmt::Block(stmts) => {
                let outer = Rc::clone(&self.env);
                self.env = Env::new_child(Rc::clone(&self.env));
                let result = self.exec_stmts(stmts);
                self.env = outer; // restore even if execution errored
                result // propagates Normal, Return, Break, Continue
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.eval_expr(cond)?;
                if is_truthy(&cond_val) {
                    self.exec_stmt(then_block) // propagates all flows including Break/Continue
                } else if let Some(else_b) = else_block {
                    self.exec_stmt(else_b)
                } else {
                    Ok(ExecFlow::Normal)
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
                Ok(ExecFlow::Normal)
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
                Ok(ExecFlow::Normal)
            }

            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    let cond_val = self.eval_expr(condition)?;
                    let keep_going = match cond_val {
                        Value::Bool(b) => b,
                        other => {
                            return Err(RuntimeError {
                                msg: format!(
                                    "while condition must be Bool, got {}",
                                    other.type_name()
                                ),
                            })
                        }
                    };
                    if !keep_going {
                        break;
                    }
                    let outer = Rc::clone(&self.env);
                    self.env = Env::new_child(Rc::clone(&self.env));
                    let result = self.exec_stmts(body);
                    self.env = outer;
                    match result? {
                        ExecFlow::Normal => {}
                        ExecFlow::Break => break,
                        ExecFlow::Continue => continue,
                        flow @ ExecFlow::Return(_) => return Ok(flow),
                    }
                }
                Ok(ExecFlow::Normal)
            }

            Stmt::Break { .. } => Ok(ExecFlow::Break),

            Stmt::Continue { .. } => Ok(ExecFlow::Continue),

            Stmt::ForRange {
                var_name,
                start,
                end,
                body,
                ..
            } => {
                let start_val = self.eval_expr(start)?;
                let end_val = self.eval_expr(end)?;

                let start_num = match start_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("range start must be a Number, got {}", other.type_name()),
                        })
                    }
                };
                let end_num = match end_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("range end must be a Number, got {}", other.type_name()),
                        })
                    }
                };

                // Create a loop-level env that holds the loop variable.
                // A fresh body env is created per iteration as a child.
                let loop_env = Env::new_child(Rc::clone(&self.env));
                let outer = Rc::clone(&self.env);
                self.env = Rc::clone(&loop_env);
                self.env
                    .borrow_mut()
                    .define(var_name.clone(), Value::Number(start_num));

                let mut i = start_num;
                let mut loop_result = ExecFlow::Normal;

                while i < end_num {
                    // Update loop variable for this iteration.
                    loop_env
                        .borrow_mut()
                        .assign_existing(var_name, Value::Number(i));

                    // Execute body in a fresh child of the loop env.
                    let body_env = Env::new_child(Rc::clone(&loop_env));
                    self.env = Rc::clone(&body_env);
                    let result = self.exec_stmts(body);
                    self.env = Rc::clone(&loop_env);

                    match result? {
                        ExecFlow::Normal => {}
                        ExecFlow::Break => {
                            loop_result = ExecFlow::Normal;
                            break;
                        }
                        ExecFlow::Continue => {
                            // Fall through to increment.
                        }
                        flow @ ExecFlow::Return(_) => {
                            loop_result = flow;
                            break;
                        }
                    }

                    i += 1.0;
                }

                // Restore the enclosing environment.
                self.env = outer;

                Ok(loop_result)
            }

            Stmt::Simulate {
                duration,
                step,
                body,
                ..
            } => {
                let dur_val = self.eval_expr(duration)?;
                let step_val = self.eval_expr(step)?;

                let dur_num = match dur_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "simulate duration must be a number, got {}",
                                other.type_name()
                            ),
                        })
                    }
                };
                let step_num = match step_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "simulate step must be a number, got {}",
                                other.type_name()
                            ),
                        })
                    }
                };

                if step_num <= 0.0 {
                    return Err(RuntimeError {
                        msg: "simulate step must be greater than zero".into(),
                    });
                }
                if dur_num < 0.0 {
                    return Err(RuntimeError {
                        msg: "simulate duration cannot be negative".into(),
                    });
                }

                let iterations = (dur_num / step_num).floor() as usize;
                for i in 0..iterations {
                    let t = i as f64 * step_num;
                    let outer = Rc::clone(&self.env);
                    self.env = Env::new_child(Rc::clone(&self.env));
                    self.env
                        .borrow_mut()
                        .define("time".to_string(), Value::Number(t));
                    let result = self.exec_stmts(body);
                    self.env = outer;
                    match result? {
                        ExecFlow::Normal => {}
                        flow @ ExecFlow::Return(_) => return Ok(flow),
                        // Break/Continue should not escape simulate (typechecker prevents it),
                        // but propagate Return-like so they don't silently disappear.
                        ExecFlow::Break | ExecFlow::Continue => {}
                    }
                }
                Ok(ExecFlow::Normal)
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
                Ok(ExecFlow::Normal)
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

            Expr::ArrayLiteral { elements, .. } => {
                let mut vals = Vec::with_capacity(elements.len());
                for elem in elements {
                    vals.push(self.eval_expr(elem)?);
                }
                Ok(Value::Array(vals))
            }

            Expr::Index { array, index, .. } => {
                let arr_val = self.eval_expr(array)?;
                let idx_val = self.eval_expr(index)?;
                let n = match idx_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("index must be Number, got {}", other.type_name()),
                        })
                    }
                };
                if n.fract() != 0.0 {
                    return Err(RuntimeError {
                        msg: format!("index must be an integer, got {}", n),
                    });
                }
                if n < 0.0 {
                    return Err(RuntimeError {
                        msg: format!("index out of bounds: index {} is negative", n as i64),
                    });
                }
                let i = n as usize;
                match arr_val {
                    Value::Array(elems) => elems.get(i).cloned().ok_or_else(|| RuntimeError {
                        msg: format!(
                            "array index out of bounds: index {} but length is {}",
                            i,
                            elems.len()
                        ),
                    }),
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        chars
                            .get(i)
                            .map(|c| Value::Str(c.to_string()))
                            .ok_or_else(|| RuntimeError {
                                msg: format!(
                                    "string index out of bounds: index {} but length is {}",
                                    i,
                                    chars.len()
                                ),
                            })
                    }
                    other => Err(RuntimeError {
                        msg: format!("cannot index into value of type {}", other.type_name()),
                    }),
                }
            }

            Expr::Slice {
                array, start, end, ..
            } => {
                let arr_val = self.eval_expr(array)?;
                let start_val = self.eval_expr(start)?;
                let end_val = self.eval_expr(end)?;
                let s = match start_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("slice start must be Number, got {}", other.type_name()),
                        })
                    }
                };
                if s.fract() != 0.0 {
                    return Err(RuntimeError {
                        msg: format!("slice start must be an integer, got {}", s),
                    });
                }
                if s < 0.0 {
                    return Err(RuntimeError {
                        msg: "slice start must be non-negative".into(),
                    });
                }
                let e = match end_val {
                    Value::Number(n) => n,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("slice end must be Number, got {}", other.type_name()),
                        })
                    }
                };
                if e.fract() != 0.0 {
                    return Err(RuntimeError {
                        msg: format!("slice end must be an integer, got {}", e),
                    });
                }
                if e < 0.0 {
                    return Err(RuntimeError {
                        msg: "slice end must be non-negative".into(),
                    });
                }
                let si = s as usize;
                let ei = e as usize;
                if si > ei {
                    return Err(RuntimeError {
                        msg: format!("slice start {} is greater than end {}", si, ei),
                    });
                }
                match arr_val {
                    Value::Array(elems) => {
                        if ei > elems.len() {
                            return Err(RuntimeError {
                                msg: format!(
                                    "slice end {} is out of bounds for array of length {}",
                                    ei,
                                    elems.len()
                                ),
                            });
                        }
                        Ok(Value::Array(elems[si..ei].to_vec()))
                    }
                    Value::Str(s_str) => {
                        let chars: Vec<char> = s_str.chars().collect();
                        if ei > chars.len() {
                            return Err(RuntimeError {
                                msg: format!(
                                    "slice end {} is out of bounds for string of length {}",
                                    ei,
                                    chars.len()
                                ),
                            });
                        }
                        Ok(Value::Str(chars[si..ei].iter().collect()))
                    }
                    other => Err(RuntimeError {
                        msg: format!(
                            "slice target must be Array or Text, got {}",
                            other.type_name()
                        ),
                    }),
                }
            }

            Expr::Call { callee, args, .. } => {
                // `len` builtin: len(array) -> Number
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if name == "len" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("len() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let arg_val = self.eval_expr(&args[0])?;
                        return match arg_val {
                            Value::Array(v) => Ok(Value::Number(v.len() as f64)),
                            Value::Str(s) => Ok(Value::Number(s.chars().count() as f64)),
                            other => Err(RuntimeError {
                                msg: format!(
                                    "len() requires Array or Text, got {}",
                                    other.type_name()
                                ),
                            }),
                        };
                    }

                    if name == "push" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("push() expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let arr_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    msg: "push() first argument must be a mutable array variable"
                                        .into(),
                                })
                            }
                        };
                        let new_elem = self.eval_expr(&args[1])?;
                        let current =
                            self.env
                                .borrow()
                                .get(&arr_name)
                                .ok_or_else(|| RuntimeError {
                                    msg: format!("undefined variable '{}'", arr_name),
                                })?;
                        let mut elems = match current {
                            Value::Array(v) => v,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "push() requires Array, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        elems.push(new_elem);
                        if !self
                            .env
                            .borrow_mut()
                            .assign_existing(&arr_name, Value::Array(elems))
                        {
                            return Err(RuntimeError {
                                msg: format!("undefined variable '{}'", arr_name),
                            });
                        }
                        return Ok(Value::Nil);
                    }

                    if name == "pop" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("pop() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let arr_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    msg: "pop() argument must be a mutable array variable".into(),
                                })
                            }
                        };
                        let current =
                            self.env
                                .borrow()
                                .get(&arr_name)
                                .ok_or_else(|| RuntimeError {
                                    msg: format!("undefined variable '{}'", arr_name),
                                })?;
                        let mut elems = match current {
                            Value::Array(v) => v,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!("pop() requires Array, got {}", other.type_name()),
                                })
                            }
                        };
                        if elems.is_empty() {
                            return Err(RuntimeError {
                                msg: "cannot pop from empty array".into(),
                            });
                        }
                        let popped = elems.pop().unwrap();
                        if !self
                            .env
                            .borrow_mut()
                            .assign_existing(&arr_name, Value::Array(elems))
                        {
                            return Err(RuntimeError {
                                msg: format!("undefined variable '{}'", arr_name),
                            });
                        }
                        return Ok(popped);
                    }

                    if name == "contains" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("contains() expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "contains() first argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let pattern = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "contains() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Bool(text.contains(pattern.as_str())));
                    }

                    if name == "starts_with" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!(
                                    "starts_with() expects 2 arguments, got {}",
                                    args.len()
                                ),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "starts_with() first argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let prefix = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "starts_with() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Bool(text.starts_with(prefix.as_str())));
                    }

                    if name == "ends_with" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("ends_with() expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "ends_with() first argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let suffix = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "ends_with() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Bool(text.ends_with(suffix.as_str())));
                    }

                    if name == "to_upper" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("to_upper() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "to_upper() argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Str(text.to_uppercase()));
                    }

                    if name == "to_lower" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("to_lower() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "to_lower() argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Str(text.to_lowercase()));
                    }

                    if name == "trim" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("trim() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "trim() argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Str(text.trim().to_string()));
                    }

                    if name == "split" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("split() expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let text = match self.eval_expr(&args[0])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "split() first argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let delimiter = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "split() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let parts: Vec<Value> = if delimiter.is_empty() {
                            text.chars().map(|c| Value::Str(c.to_string())).collect()
                        } else {
                            text.split(delimiter.as_str())
                                .map(|p| Value::Str(p.to_string()))
                                .collect()
                        };
                        return Ok(Value::Array(parts));
                    }

                    if name == "join" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("join() expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let parts = match self.eval_expr(&args[0])? {
                            Value::Array(elems) => elems,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "join() first argument must be Array<Text>, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let delimiter = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "join() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let strs: Result<Vec<String>, RuntimeError> = parts
                            .iter()
                            .map(|v| match v {
                                Value::Str(s) => Ok(s.clone()),
                                other => Err(RuntimeError {
                                    msg: format!(
                                        "join() array element must be Text, got {}",
                                        other.type_name()
                                    ),
                                }),
                            })
                            .collect();
                        return Ok(Value::Str(strs?.join(delimiter.as_str())));
                    }
                }

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
            ExecFlow::Normal => Ok(Value::Nil),
            ExecFlow::Return(v) => Ok(v),
            ExecFlow::Break | ExecFlow::Continue => {
                // Typechecker prevents break/continue from escaping function bodies.
                Err(RuntimeError {
                    msg: "break/continue escaped function boundary (compiler bug)".into(),
                })
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
    a == b
}
