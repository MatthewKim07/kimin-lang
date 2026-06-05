use std::collections::BTreeMap;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{AssignTarget, BinaryOp, CompoundAssignOp, Expr, Stmt, UnaryOp};
use crate::env::{Env, EnvRef};
use crate::error::RuntimeError;
use crate::value::{FunctionValue, Value};

enum ExecFlow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter {
    env: EnvRef,
    /// Method registry: struct_name → method_name → FunctionValue (with self as first param).
    methods: HashMap<String, HashMap<String, FunctionValue>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            env: Env::new_global(),
            methods: HashMap::new(),
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

                // Read current variable, dispatch on Array vs Map.
                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;

                match current {
                    Value::Map(mut map) => {
                        let key = match idx_val {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "map index key must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        map.insert(key, new_elem);
                        if !self.env.borrow_mut().assign_existing(name, Value::Map(map)) {
                            return Err(RuntimeError {
                                msg: format!("undefined variable '{}'", name),
                            });
                        }
                        return Ok(ExecFlow::Normal);
                    }
                    Value::Array(_) => {}
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "cannot index-assign into value of type {}",
                                other.type_name()
                            ),
                        })
                    }
                }

                // Array path: re-read to get Vec.
                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;
                let mut elems = match current {
                    Value::Array(v) => v,
                    _ => unreachable!("already checked above"),
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
                // Evaluate index/key first, then dispatch on target type.
                let idx_val = self.eval_expr(index)?;

                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };

                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;

                match current {
                    Value::Map(mut map) => {
                        let key = match idx_val {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "map index key must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let old_val = map.get(&key).cloned().ok_or_else(|| RuntimeError {
                            msg: format!("map key '{}' not found", key),
                        })?;
                        let rhs = self.eval_expr(value)?;
                        let new_val = eval_binary(&binary_op, old_val, rhs)?;
                        map.insert(key, new_val);
                        if !self.env.borrow_mut().assign_existing(name, Value::Map(map)) {
                            return Err(RuntimeError {
                                msg: format!("undefined variable '{}'", name),
                            });
                        }
                        return Ok(ExecFlow::Normal);
                    }
                    Value::Array(_) => {}
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "cannot index-compound-assign into value of type {}",
                                other.type_name()
                            ),
                        })
                    }
                }

                // Array path: re-read to get Vec.
                let current = self.env.borrow().get(name).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", name),
                })?;
                let mut elems = match current {
                    Value::Array(v) => v,
                    _ => unreachable!("already checked above"),
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

            Stmt::StructDecl { .. } => {
                // Struct declarations are purely static — no runtime work.
                Ok(ExecFlow::Normal)
            }

            Stmt::ImplBlock {
                struct_name,
                methods: method_stmts,
                ..
            } => {
                // Register each method as a FunctionValue (self is the first param).
                for method_stmt in method_stmts {
                    if let Stmt::FnDecl {
                        name, params, body, ..
                    } = method_stmt
                    {
                        let fv = FunctionValue {
                            name: name.clone(),
                            params: params.iter().map(|p| p.name.clone()).collect(),
                            body: body.clone(),
                            closure_env: Rc::clone(&self.env),
                        };
                        self.methods
                            .entry(struct_name.clone())
                            .or_default()
                            .insert(name.clone(), fv);
                    }
                }
                Ok(ExecFlow::Normal)
            }

            Stmt::TargetAssign { target, value, .. } => {
                let (root, steps, index_exprs) = interp_flatten_target(target);
                // Evaluate index expressions in source order.
                let index_vals: Vec<Value> = index_exprs
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<_, _>>()?;
                // Evaluate RHS.
                let rhs = self.eval_expr(value)?;
                // Read root, update at path, write back.
                let root_val = self.env.borrow().get(&root).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", root),
                })?;
                let updated = interp_update_path(root_val, &steps, &index_vals, &mut 0, rhs)?;
                if !self.env.borrow_mut().assign_existing(&root, updated) {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", root),
                    });
                }
                Ok(ExecFlow::Normal)
            }

            Stmt::TargetCompoundAssign {
                target, op, value, ..
            } => {
                let binary_op = match op {
                    CompoundAssignOp::Add => BinaryOp::Add,
                    CompoundAssignOp::Subtract => BinaryOp::Sub,
                    CompoundAssignOp::Multiply => BinaryOp::Mul,
                    CompoundAssignOp::Divide => BinaryOp::Div,
                };
                let (root, steps, index_exprs) = interp_flatten_target(target);
                // Evaluate index expressions first.
                let index_vals: Vec<Value> = index_exprs
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<_, _>>()?;
                // Read root and extract old leaf value (before RHS is evaluated).
                let root_val = self.env.borrow().get(&root).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", root),
                })?;
                let old_val = interp_read_path(&root_val, &steps, &index_vals, &mut 0)?;
                // Evaluate RHS.
                let rhs = self.eval_expr(value)?;
                let new_val = eval_binary(&binary_op, old_val, rhs)?;
                // Re-read root (RHS may have modified it) and update at path.
                let root_val2 = self.env.borrow().get(&root).ok_or_else(|| RuntimeError {
                    msg: format!("undefined variable '{}'", root),
                })?;
                let updated = interp_update_path(root_val2, &steps, &index_vals, &mut 0, new_val)?;
                if !self.env.borrow_mut().assign_existing(&root, updated) {
                    return Err(RuntimeError {
                        msg: format!("undefined variable '{}'", root),
                    });
                }
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

            Stmt::ForEach {
                var_name,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iterable)?;
                let elements = match iter_val {
                    Value::Array(elems) => elems,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("for-each requires Array, got {}", other.type_name()),
                        })
                    }
                };
                // Snapshot: `elements` is already a clone from eval_expr.
                let outer = Rc::clone(&self.env);
                let mut loop_result = ExecFlow::Normal;

                for elem in elements {
                    // Fresh body env per iteration; loop variable defined inside.
                    let body_env = Env::new_child(Rc::clone(&outer));
                    body_env.borrow_mut().define(var_name.clone(), elem);
                    self.env = Rc::clone(&body_env);
                    let result = self.exec_stmts(body);
                    self.env = Rc::clone(&outer);

                    match result? {
                        ExecFlow::Normal => {}
                        ExecFlow::Break => {
                            loop_result = ExecFlow::Normal;
                            break;
                        }
                        ExecFlow::Continue => {}
                        flow @ ExecFlow::Return(_) => {
                            loop_result = flow;
                            break;
                        }
                    }
                }

                self.env = outer;
                Ok(loop_result)
            }

            Stmt::ForEachIndexed {
                index_name,
                var_name,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iterable)?;
                let elements = match iter_val {
                    Value::Array(elems) => elems,
                    other => {
                        return Err(RuntimeError {
                            msg: format!("for-each requires Array, got {}", other.type_name()),
                        })
                    }
                };
                let outer = Rc::clone(&self.env);
                let mut loop_result = ExecFlow::Normal;

                for (idx, elem) in elements.into_iter().enumerate() {
                    let body_env = Env::new_child(Rc::clone(&outer));
                    body_env
                        .borrow_mut()
                        .define(index_name.clone(), Value::Number(idx as f64));
                    body_env.borrow_mut().define(var_name.clone(), elem);
                    self.env = Rc::clone(&body_env);
                    let result = self.exec_stmts(body);
                    self.env = Rc::clone(&outer);

                    match result? {
                        ExecFlow::Normal => {}
                        ExecFlow::Break => {
                            loop_result = ExecFlow::Normal;
                            break;
                        }
                        ExecFlow::Continue => {}
                        flow @ ExecFlow::Return(_) => {
                            loop_result = flow;
                            break;
                        }
                    }
                }

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
            } => {
                // If `state_name` is a variable in the current env, treat as struct field access.
                if let Some(val) = self.env.borrow().get(state_name) {
                    return match val {
                        Value::Struct { fields, .. } => fields
                            .get(variant_name)
                            .cloned()
                            .ok_or_else(|| RuntimeError {
                                msg: format!("struct has no field '{}'", variant_name),
                            }),
                        other => Err(RuntimeError {
                            msg: format!(
                                "cannot access field '{}' on {}",
                                variant_name,
                                other.type_name()
                            ),
                        }),
                    };
                }
                // Otherwise it's a state machine variant literal.
                Ok(Value::StateValue {
                    state_name: state_name.clone(),
                    variant_name: variant_name.clone(),
                })
            }

            Expr::StructLiteral { name, fields, .. } => {
                let mut field_map = std::collections::BTreeMap::new();
                for (field_name, field_expr) in fields {
                    let val = self.eval_expr(field_expr)?;
                    field_map.insert(field_name.clone(), val);
                }
                Ok(Value::Struct {
                    name: name.clone(),
                    fields: field_map,
                })
            }

            Expr::FieldAccess { object, field, .. } => {
                let val = self.eval_expr(object)?;
                match val {
                    Value::Struct { fields, .. } => {
                        fields.get(field).cloned().ok_or_else(|| RuntimeError {
                            msg: format!("struct has no field '{}'", field),
                        })
                    }
                    other => Err(RuntimeError {
                        msg: format!("cannot access field '{}' on {}", field, other.type_name()),
                    }),
                }
            }

            Expr::MethodCall {
                object,
                method,
                args,
                ..
            } => {
                let receiver = self.eval_expr(object)?;
                let struct_name = match &receiver {
                    Value::Struct { name, .. } => name.clone(),
                    other => {
                        return Err(RuntimeError {
                            msg: format!(
                                "cannot call method '{}' on {}",
                                method,
                                other.type_name()
                            ),
                        })
                    }
                };
                // Clone FunctionValue to release borrow on self.methods before calling.
                let fv = self
                    .methods
                    .get(&struct_name)
                    .and_then(|m| m.get(method))
                    .cloned()
                    .ok_or_else(|| RuntimeError {
                        msg: format!("struct '{}' has no method '{}'", struct_name, method),
                    })?;

                // Build args: receiver (self) first, then explicit args.
                let mut call_args = vec![receiver];
                for arg in args {
                    call_args.push(self.eval_expr(arg)?);
                }
                self.call_function(&fv, call_args)
            }

            Expr::Variable { name, .. } => {
                // Builtin constants.
                if name == "PI" {
                    return Ok(Value::Number(std::f64::consts::PI));
                }
                if name == "E" {
                    return Ok(Value::Number(std::f64::consts::E));
                }
                if name == "TAU" {
                    return Ok(Value::Number(std::f64::consts::TAU));
                }
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

            Expr::MapLiteral { entries, .. } => {
                let mut map = BTreeMap::new();
                for (key_expr, val_expr) in entries {
                    let key = match self.eval_expr(key_expr)? {
                        Value::Str(s) => s,
                        other => {
                            return Err(RuntimeError {
                                msg: format!("map key must be Text, got {}", other.type_name()),
                            })
                        }
                    };
                    let val = self.eval_expr(val_expr)?;
                    map.insert(key, val);
                }
                Ok(Value::Map(map))
            }

            Expr::Index { array, index, .. } => {
                let arr_val = self.eval_expr(array)?;
                let idx_val = self.eval_expr(index)?;
                match arr_val {
                    Value::Map(map) => {
                        let key = match idx_val {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!("map key must be Text, got {}", other.type_name()),
                                })
                            }
                        };
                        map.get(&key).cloned().ok_or_else(|| RuntimeError {
                            msg: format!("map key '{}' not found", key),
                        })
                    }
                    arr_val => {
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
                            Value::Array(elems) => {
                                elems.get(i).cloned().ok_or_else(|| RuntimeError {
                                    msg: format!(
                                        "array index out of bounds: index {} but length is {}",
                                        i,
                                        elems.len()
                                    ),
                                })
                            }
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
                                msg: format!(
                                    "cannot index into value of type {}",
                                    other.type_name()
                                ),
                            }),
                        }
                    }
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

                    if name == "has_key" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("has_key expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let map = match self.eval_expr(&args[0])? {
                            Value::Map(m) => m,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "has_key() first argument must be Map, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let key = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "has_key() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        return Ok(Value::Bool(map.contains_key(&key)));
                    }

                    if name == "keys" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("keys expects 1 argument, got {}", args.len()),
                            });
                        }
                        let map = match self.eval_expr(&args[0])? {
                            Value::Map(m) => m,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "keys() argument must be Map, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let ks: Vec<Value> = map.keys().map(|k| Value::Str(k.clone())).collect();
                        return Ok(Value::Array(ks));
                    }

                    if name == "values" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("values expects 1 argument, got {}", args.len()),
                            });
                        }
                        let map = match self.eval_expr(&args[0])? {
                            Value::Map(m) => m,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "values() argument must be Map, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let vs: Vec<Value> = map.values().cloned().collect();
                        return Ok(Value::Array(vs));
                    }

                    if name == "remove" {
                        if args.len() != 2 {
                            return Err(RuntimeError {
                                msg: format!("remove expects 2 arguments, got {}", args.len()),
                            });
                        }
                        let map_name = match &args[0] {
                            Expr::Variable { name, .. } => name.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    msg: "remove() first argument must be a mutable map variable"
                                        .into(),
                                })
                            }
                        };
                        let key = match self.eval_expr(&args[1])? {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "remove() second argument must be Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let current =
                            self.env
                                .borrow()
                                .get(&map_name)
                                .ok_or_else(|| RuntimeError {
                                    msg: format!("undefined variable '{}'", map_name),
                                })?;
                        let mut map = match current {
                            Value::Map(m) => m,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "remove() first argument must be Map, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let removed = map.remove(&key).ok_or_else(|| RuntimeError {
                            msg: format!("map key '{}' not found", key),
                        })?;
                        self.env
                            .borrow_mut()
                            .assign_existing(&map_name, Value::Map(map));
                        return Ok(removed);
                    }

                    // `to_string` builtin: to_string(value) -> Text
                    if name == "to_string" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("to_string() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let val = self.eval_expr(&args[0])?;
                        return Ok(Value::Str(format!("{}", val)));
                    }

                    // `to_number` builtin: to_number(text) -> Number
                    if name == "to_number" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("to_number() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let val = self.eval_expr(&args[0])?;
                        let text = match val {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "to_number() expects Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let n = crate::value::parse_number_from_text(&text)
                            .map_err(|e| RuntimeError { msg: e })?;
                        return Ok(Value::Number(n));
                    }

                    // `to_bool` builtin: to_bool(text) -> Bool
                    if name == "to_bool" {
                        if args.len() != 1 {
                            return Err(RuntimeError {
                                msg: format!("to_bool() expects 1 argument, got {}", args.len()),
                            });
                        }
                        let val = self.eval_expr(&args[0])?;
                        let text = match val {
                            Value::Str(s) => s,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "to_bool() expects Text, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let b = crate::value::parse_bool_from_text(&text)
                            .map_err(|e| RuntimeError { msg: e })?;
                        return Ok(Value::Bool(b));
                    }

                    // ln / log2 / log10 / exp: Number -> Number
                    if matches!(name.as_str(), "ln" | "log2" | "log10" | "exp") && args.len() == 1 {
                        let val = self.eval_expr(&args[0])?;
                        let n = match val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let result = match name.as_str() {
                            "ln" => {
                                if n <= 0.0 {
                                    return Err(RuntimeError {
                                        msg: format!("ln requires positive Number, got {}", n),
                                    });
                                }
                                n.ln()
                            }
                            "log2" => {
                                if n <= 0.0 {
                                    return Err(RuntimeError {
                                        msg: format!("log2 requires positive Number, got {}", n),
                                    });
                                }
                                n.log2()
                            }
                            "log10" => {
                                if n <= 0.0 {
                                    return Err(RuntimeError {
                                        msg: format!("log10 requires positive Number, got {}", n),
                                    });
                                }
                                n.log10()
                            }
                            "exp" => {
                                let r = n.exp();
                                if !r.is_finite() {
                                    return Err(RuntimeError {
                                        msg: format!("exp result is not finite (exp({}))", n),
                                    });
                                }
                                r
                            }
                            _ => unreachable!(),
                        };
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("{} result is not finite", name),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // sin / cos / tan: Number -> Number (radians)
                    if matches!(name.as_str(), "sin" | "cos" | "tan") && args.len() == 1 {
                        let val = self.eval_expr(&args[0])?;
                        let n = match val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if !n.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("{} input is not finite", name),
                            });
                        }
                        let result = match name.as_str() {
                            "sin" => n.sin(),
                            "cos" => n.cos(),
                            "tan" => n.tan(),
                            _ => unreachable!(),
                        };
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("{} result is not finite", name),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // clamp: Number, Number, Number -> Number
                    if name == "clamp" && args.len() == 3 {
                        let n_val = self.eval_expr(&args[0])?;
                        let n = match n_val {
                            Value::Number(v) => v,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "clamp() argument 1 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let lo_val = self.eval_expr(&args[1])?;
                        let lo = match lo_val {
                            Value::Number(v) => v,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "clamp() argument 2 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let hi_val = self.eval_expr(&args[2])?;
                        let hi = match hi_val {
                            Value::Number(v) => v,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "clamp() argument 3 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if !n.is_finite() || !lo.is_finite() || !hi.is_finite() {
                            return Err(RuntimeError {
                                msg: "clamp input is not finite".into(),
                            });
                        }
                        if lo > hi {
                            return Err(RuntimeError {
                                msg: "clamp lower bound cannot be greater than upper bound".into(),
                            });
                        }
                        let result = if n < lo {
                            lo
                        } else if n > hi {
                            hi
                        } else {
                            n
                        };
                        return Ok(Value::Number(result));
                    }

                    // hypot: Number, Number -> Number (Euclidean magnitude)
                    if name == "hypot" && args.len() == 2 {
                        let a_val = self.eval_expr(&args[0])?;
                        let a = match a_val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "hypot() argument 1 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let b_val = self.eval_expr(&args[1])?;
                        let b = match b_val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "hypot() argument 2 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if !a.is_finite() || !b.is_finite() {
                            return Err(RuntimeError {
                                msg: "hypot input is not finite".into(),
                            });
                        }
                        let result = a.hypot(b);
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: "hypot result is not finite".into(),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // asin / acos / atan: Number -> Number (radians)
                    if matches!(name.as_str(), "asin" | "acos" | "atan") && args.len() == 1 {
                        let val = self.eval_expr(&args[0])?;
                        let n = match val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if !n.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("{} input is not finite", name),
                            });
                        }
                        let result = match name.as_str() {
                            "asin" => {
                                if n < -1.0 || n > 1.0 {
                                    return Err(RuntimeError {
                                        msg: format!("asin requires input in [-1, 1], got {}", n),
                                    });
                                }
                                n.asin()
                            }
                            "acos" => {
                                if n < -1.0 || n > 1.0 {
                                    return Err(RuntimeError {
                                        msg: format!("acos requires input in [-1, 1], got {}", n),
                                    });
                                }
                                n.acos()
                            }
                            "atan" => n.atan(),
                            _ => unreachable!(),
                        };
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("{} result is not finite", name),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // atan2: Number, Number -> Number (radians)
                    if name == "atan2" && args.len() == 2 {
                        let y_val = self.eval_expr(&args[0])?;
                        let y = match y_val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "atan2() argument 1 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let x_val = self.eval_expr(&args[1])?;
                        let x = match x_val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "atan2() argument 2 expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if !y.is_finite() || !x.is_finite() {
                            return Err(RuntimeError {
                                msg: "atan2 input is not finite".into(),
                            });
                        }
                        let result = y.atan2(x);
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: "atan2 result is not finite".into(),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // sqrt: Number -> Number (non-negative)
                    if name == "sqrt" && args.len() == 1 {
                        let val = self.eval_expr(&args[0])?;
                        let n = match val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "sqrt() expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        if n < 0.0 {
                            return Err(RuntimeError {
                                msg: format!("sqrt requires non-negative Number, got {}", n),
                            });
                        }
                        let result = n.sqrt();
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: "sqrt result is not finite".to_string(),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // pow: Number, Number -> Number
                    if name == "pow" && args.len() == 2 {
                        let v0 = self.eval_expr(&args[0])?;
                        let v1 = self.eval_expr(&args[1])?;
                        let base = match v0 {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "pow() first argument expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let exp = match v1 {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "pow() second argument expects Number, got {}",
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let result = base.powf(exp);
                        if !result.is_finite() {
                            return Err(RuntimeError {
                                msg: format!("pow result is not finite (pow({}, {}))", base, exp),
                            });
                        }
                        return Ok(Value::Number(result));
                    }

                    // abs / floor / ceil / round: Number -> Number
                    if matches!(name.as_str(), "abs" | "floor" | "ceil" | "round")
                        && args.len() == 1
                    {
                        let val = self.eval_expr(&args[0])?;
                        let n = match val {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let result = match name.as_str() {
                            "abs" => n.abs(),
                            "floor" => n.floor(),
                            "ceil" => n.ceil(),
                            "round" => n.round(),
                            _ => unreachable!(),
                        };
                        return Ok(Value::Number(result));
                    }

                    // min / max: Number, Number -> Number
                    if (name == "min" || name == "max") && args.len() == 2 {
                        let v0 = self.eval_expr(&args[0])?;
                        let v1 = self.eval_expr(&args[1])?;
                        let a = match v0 {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() first argument expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let b = match v1 {
                            Value::Number(n) => n,
                            other => {
                                return Err(RuntimeError {
                                    msg: format!(
                                        "{}() second argument expects Number, got {}",
                                        name,
                                        other.type_name()
                                    ),
                                })
                            }
                        };
                        let result = if name == "min" { a.min(b) } else { a.max(b) };
                        return Ok(Value::Number(result));
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

// ---- Target path helpers ----

/// Step kind for interpreter path traversal.
enum InterpStep {
    Field(String),
    Index, // value supplied from index_vals
}

/// Decompose an AssignTarget into (root_name, steps, index_exprs) all in source order.
fn interp_flatten_target(target: &AssignTarget) -> (String, Vec<InterpStep>, Vec<Expr>) {
    match target {
        AssignTarget::Var(name) => (name.clone(), vec![], vec![]),
        AssignTarget::Field(inner, field) => {
            let (root, mut steps, exprs) = interp_flatten_target(inner);
            steps.push(InterpStep::Field(field.clone()));
            (root, steps, exprs)
        }
        AssignTarget::Index(inner, expr) => {
            let (root, mut steps, mut exprs) = interp_flatten_target(inner);
            steps.push(InterpStep::Index);
            exprs.push(expr.clone());
            (root, steps, exprs)
        }
    }
}

/// Read the leaf value at the end of a path without mutating anything.
fn interp_read_path(
    val: &Value,
    steps: &[InterpStep],
    index_vals: &[Value],
    idx_pos: &mut usize,
) -> Result<Value, RuntimeError> {
    if steps.is_empty() {
        return Ok(val.clone());
    }
    match &steps[0] {
        InterpStep::Field(field) => match val {
            Value::Struct { fields, .. } => {
                let inner = fields.get(field.as_str()).ok_or_else(|| RuntimeError {
                    msg: format!("struct has no field '{}'", field),
                })?;
                interp_read_path(inner, &steps[1..], index_vals, idx_pos)
            }
            other => Err(RuntimeError {
                msg: format!("cannot access field on {}", other.type_name()),
            }),
        },
        InterpStep::Index => {
            let idx_val = &index_vals[*idx_pos];
            *idx_pos += 1;
            match val {
                Value::Array(elems) => {
                    let i = interp_array_index(idx_val, elems.len())?;
                    interp_read_path(&elems[i], &steps[1..], index_vals, idx_pos)
                }
                Value::Map(map) => {
                    let key = interp_map_key(idx_val)?;
                    let inner = map.get(&key).ok_or_else(|| RuntimeError {
                        msg: format!("map key '{}' not found", key),
                    })?;
                    interp_read_path(inner, &steps[1..], index_vals, idx_pos)
                }
                other => Err(RuntimeError {
                    msg: format!("cannot index into {}", other.type_name()),
                }),
            }
        }
    }
}

/// Clone `val`, update the leaf at the end of `steps`, and return the updated clone.
fn interp_update_path(
    val: Value,
    steps: &[InterpStep],
    index_vals: &[Value],
    idx_pos: &mut usize,
    new_val: Value,
) -> Result<Value, RuntimeError> {
    if steps.is_empty() {
        return Ok(new_val);
    }
    match &steps[0] {
        InterpStep::Field(field) => match val {
            Value::Struct {
                name: sn,
                mut fields,
            } => {
                if !fields.contains_key(field.as_str()) {
                    return Err(RuntimeError {
                        msg: format!("struct '{}' has no field '{}'", sn, field),
                    });
                }
                let old = fields.remove(field).unwrap();
                let updated = interp_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                fields.insert(field.clone(), updated);
                Ok(Value::Struct { name: sn, fields })
            }
            other => Err(RuntimeError {
                msg: format!("cannot assign field on {}", other.type_name()),
            }),
        },
        InterpStep::Index => {
            let idx_val = index_vals[*idx_pos].clone();
            *idx_pos += 1;
            match val {
                Value::Array(mut elems) => {
                    let i = interp_array_index(&idx_val, elems.len())?;
                    let old = elems[i].clone();
                    elems[i] = interp_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                    Ok(Value::Array(elems))
                }
                Value::Map(mut map) => {
                    let key = interp_map_key(&idx_val)?;
                    let old = map.get(&key).cloned().ok_or_else(|| RuntimeError {
                        msg: format!("map key '{}' not found", key),
                    })?;
                    let updated =
                        interp_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                    map.insert(key, updated);
                    Ok(Value::Map(map))
                }
                other => Err(RuntimeError {
                    msg: format!("cannot index into {}", other.type_name()),
                }),
            }
        }
    }
}

fn interp_array_index(idx_val: &Value, len: usize) -> Result<usize, RuntimeError> {
    let n = match idx_val {
        Value::Number(n) => *n,
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
    if i >= len {
        return Err(RuntimeError {
            msg: format!(
                "array index out of bounds: index {} but length is {}",
                i, len
            ),
        });
    }
    Ok(i)
}

fn interp_map_key(idx_val: &Value) -> Result<String, RuntimeError> {
    match idx_val {
        Value::Str(s) => Ok(s.clone()),
        other => Err(RuntimeError {
            msg: format!("map index key must be Text, got {}", other.type_name()),
        }),
    }
}
