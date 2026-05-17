use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::bytecode::{BytecodeProgram, Chunk, Constant, Instruction};
use crate::env::{Env, EnvRef};
use crate::error::{KiminError, RuntimeError};
use crate::value::Value;

/// State machine metadata registered at runtime by DefineState.
/// The state name is the HashMap key; it is not duplicated here.
#[derive(Debug, Clone)]
struct RuntimeStateMachine {
    variants: HashSet<String>,
    transitions: HashSet<(String, String)>,
}

/// Minimal stack-based bytecode VM for Kimin.
///
/// Uses an `Env` chain (same type as the tree-walk interpreter) so that nested
/// functions and simulate bodies can correctly capture their lexical environments.
/// Globals are stored in a root `EnvRef`; each block / function call / simulate
/// iteration creates a child env.
///
/// The tree-walk interpreter (`Interpreter`) remains the source of truth for
/// `kimin run`. This VM is reachable via `kimin vm <file>`.
pub struct Vm {
    program: BytecodeProgram,
    /// Root environment holding top-level (global) variables and functions.
    global_env: EnvRef,
    /// State machine metadata registered by DefineState instructions.
    states: HashMap<String, RuntimeStateMachine>,
    output: Vec<String>,
}

impl Vm {
    pub fn new(program: BytecodeProgram) -> Self {
        Vm {
            program,
            global_env: Env::new_global(),
            states: HashMap::new(),
            output: Vec::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), KiminError> {
        let main = self.program.main.clone();
        let mut stack: Vec<Value> = Vec::new();
        self.execute_chunk(&main, &mut stack, Rc::clone(&self.global_env), false)?;
        Ok(())
    }

    /// Returns all lines that were printed during execution, in order.
    pub fn take_output(self) -> Vec<String> {
        self.output
    }

    /// Execute `chunk` using `env` as the starting lexical environment.
    ///
    /// `env` is taken by value so that `BeginScope`/`EndScope` can cheaply
    /// rebind `current_env` to a child or parent without touching the caller's Rc.
    /// The caller's Rc reference count is incremented by one for the duration of
    /// the call and decremented when `current_env` is reassigned or dropped.
    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        stack: &mut Vec<Value>,
        env: EnvRef,
        is_fn: bool,
    ) -> Result<Option<Value>, KiminError> {
        let mut current_env = env;
        let mut ip = 0;

        while ip < chunk.instructions.len() {
            let instr = chunk.instructions[ip].clone();
            ip += 1;
            match instr {
                Instruction::Constant(idx) => {
                    stack.push(const_to_val(&chunk.constants[idx]));
                }
                Instruction::Nil => stack.push(Value::Nil),
                Instruction::True => stack.push(Value::Bool(true)),
                Instruction::False => stack.push(Value::Bool(false)),

                Instruction::Pop => {
                    pop(stack)?;
                }
                Instruction::Print => {
                    let val = pop(stack)?;
                    let line = format!("{}", val);
                    println!("{}", line);
                    self.output.push(line);
                }

                Instruction::Negate => {
                    let v = pop(stack)?;
                    match v {
                        Value::Number(n) => stack.push(Value::Number(-n)),
                        _ => return Err(runtime_err("unary '-' requires a number")),
                    }
                }
                Instruction::Not => {
                    let v = pop(stack)?;
                    stack.push(Value::Bool(!is_truthy(&v)));
                }

                Instruction::Add => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Number(x + y)),
                        (Value::Str(x), Value::Str(y)) => stack.push(Value::Str(x + &y)),
                        _ => return Err(runtime_err("'+' requires two numbers or two strings")),
                    }
                }
                Instruction::Subtract => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Number(x - y)),
                        _ => return Err(runtime_err("'-' requires numbers")),
                    }
                }
                Instruction::Multiply => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Number(x * y)),
                        _ => return Err(runtime_err("'*' requires numbers")),
                    }
                }
                Instruction::Divide => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => {
                            if y == 0.0 {
                                return Err(runtime_err("division by zero"));
                            }
                            stack.push(Value::Number(x / y));
                        }
                        _ => return Err(runtime_err("'/' requires numbers")),
                    }
                }

                Instruction::Equal => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    stack.push(Value::Bool(a == b));
                }
                Instruction::NotEqual => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    stack.push(Value::Bool(a != b));
                }
                Instruction::Less => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Bool(x < y)),
                        _ => return Err(runtime_err("'<' requires numbers")),
                    }
                }
                Instruction::LessEqual => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Bool(x <= y)),
                        _ => return Err(runtime_err("'<=' requires numbers")),
                    }
                }
                Instruction::Greater => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Bool(x > y)),
                        _ => return Err(runtime_err("'>' requires numbers")),
                    }
                }
                Instruction::GreaterEqual => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => stack.push(Value::Bool(x >= y)),
                        _ => return Err(runtime_err("'>=' requires numbers")),
                    }
                }

                // ── Variable operations ───────────────────────────────────────────
                //
                // After M8F all loads and stores use the env chain so that free
                // variables from enclosing scopes are found regardless of whether
                // the compiler classified them as Local or Global.
                //
                // DefineGlobal: always binds in the root (global) env.
                // DefineLocal:  binds in the innermost (current) env.
                // LoadGlobal / LoadLocal: both walk the chain from current_env.
                // StoreGlobal / StoreLocal: both walk the chain via assign_existing.
                Instruction::DefineGlobal(name) => {
                    let val = pop(stack)?;
                    self.global_env.borrow_mut().define(name, val);
                }
                Instruction::LoadGlobal(name) => {
                    let val = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;
                    stack.push(val);
                }
                Instruction::StoreGlobal(name) => {
                    let val = pop(stack)?;
                    if !current_env.borrow_mut().assign_existing(&name, val) {
                        return Err(runtime_err(&format!("undefined variable '{}'", name)));
                    }
                }

                Instruction::DefineLocal(name) => {
                    let val = pop(stack)?;
                    current_env.borrow_mut().define(name, val);
                }
                Instruction::LoadLocal(name) => {
                    let val = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;
                    stack.push(val);
                }
                Instruction::StoreLocal(name) => {
                    let val = pop(stack)?;
                    if !current_env.borrow_mut().assign_existing(&name, val) {
                        return Err(runtime_err(&format!("undefined variable '{}'", name)));
                    }
                }

                // ── Scope management ─────────────────────────────────────────────
                Instruction::BeginScope => {
                    current_env = Env::new_child(Rc::clone(&current_env));
                }
                Instruction::EndScope => {
                    let parent = current_env
                        .borrow()
                        .parent_ref()
                        .ok_or_else(|| runtime_err("EndScope with no enclosing scope"))?;
                    current_env = parent;
                }

                // ── Control flow ─────────────────────────────────────────────────
                Instruction::Jump(target) => {
                    ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    let val = pop(stack)?;
                    if !is_truthy(&val) {
                        ip = target;
                    }
                }

                // ── Functions ────────────────────────────────────────────────────

                // Capture the current lexical environment into the function value.
                Instruction::LoadFunction(name) => {
                    stack.push(Value::BytecodeFunction {
                        name,
                        env: Rc::clone(&current_env),
                    });
                }

                // Named call: resolve the function value via the env chain, then
                // call it using its captured environment as parent (lexical scope).
                Instruction::Call { name, arg_count } => {
                    let mut args: Vec<Value> = (0..arg_count)
                        .map(|_| pop(stack))
                        .collect::<Result<Vec<_>, _>>()?;
                    args.reverse();

                    // Resolve the function value through the current env chain.
                    let fn_val = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("unknown function '{}'", name)))?;

                    let (fn_chunk, fn_params, fn_arity, captured_env) = match fn_val {
                        Value::BytecodeFunction {
                            name: fn_name,
                            env: captured_env,
                        } => {
                            // Clone chunk data out of self.program before the
                            // recursive execute_chunk call to release the borrow.
                            let (chunk, params, arity) = {
                                let fc = self
                                    .program
                                    .functions
                                    .iter()
                                    .find(|f| f.name == fn_name)
                                    .ok_or_else(|| {
                                        runtime_err(&format!(
                                            "unknown function chunk '{}'",
                                            fn_name
                                        ))
                                    })?;
                                (fc.chunk.clone(), fc.params.clone(), fc.arity)
                            };
                            (chunk, params, arity, captured_env)
                        }
                        _ => return Err(runtime_err(&format!("'{}' is not a function", name))),
                    };

                    if args.len() != fn_arity {
                        return Err(runtime_err(&format!(
                            "function '{}' expects {} argument(s), got {}",
                            name,
                            fn_arity,
                            args.len()
                        )));
                    }

                    // Fresh call env parented to the function's captured env
                    // (lexical scoping — NOT to the call-site env).
                    let call_env = Env::new_child(captured_env);
                    for (param, val) in fn_params.iter().zip(args) {
                        call_env.borrow_mut().define(param.clone(), val);
                    }

                    let mut fn_stack: Vec<Value> = Vec::new();
                    let ret = self.execute_chunk(&fn_chunk, &mut fn_stack, call_env, true)?;
                    stack.push(ret.unwrap_or(Value::Nil));
                }

                Instruction::Return => {
                    if is_fn {
                        let val = if stack.is_empty() {
                            Value::Nil
                        } else {
                            pop(stack)?
                        };
                        return Ok(Some(val));
                    }
                    // Return at top level is a no-op (main program ends at Halt).
                }

                Instruction::Halt => {
                    return Ok(None);
                }

                // ── State machines ───────────────────────────────────────────────
                Instruction::DefineState {
                    name,
                    variants,
                    transitions,
                } => {
                    let rsm = RuntimeStateMachine {
                        variants: variants.into_iter().collect(),
                        transitions: transitions.into_iter().collect(),
                    };
                    self.states.insert(name, rsm);
                    // No stack effect.
                }

                Instruction::LoadState {
                    state_name,
                    variant_name,
                } => {
                    if !self.states.contains_key(&state_name) {
                        return Err(runtime_err(&format!(
                            "unknown state machine '{}'",
                            state_name
                        )));
                    }
                    if !self.states[&state_name].variants.contains(&variant_name) {
                        return Err(runtime_err(&format!(
                            "unknown variant '{}' for state '{}'",
                            variant_name, state_name
                        )));
                    }
                    stack.push(Value::StateValue {
                        state_name,
                        variant_name,
                    });
                }

                Instruction::Transition { variable, target } => {
                    // Read current value — end the borrow before calling borrow_mut.
                    let (state_name, current_variant) = {
                        let val = current_env.borrow().get(&variable).ok_or_else(|| {
                            runtime_err(&format!("undefined state variable '{}'", variable))
                        })?;
                        match val {
                            Value::StateValue {
                                state_name,
                                variant_name,
                            } => (state_name, variant_name),
                            _ => {
                                return Err(runtime_err(&format!(
                                    "transition target '{}' is not a state value",
                                    variable
                                )))
                            }
                        }
                    };

                    // Validate the transition edge (extract bools to release the borrow).
                    let (has_variant, valid_edge) = match self.states.get(&state_name) {
                        None => {
                            return Err(runtime_err(&format!(
                                "unknown state machine '{}'",
                                state_name
                            )))
                        }
                        Some(sm) => (
                            sm.variants.contains(&target),
                            sm.transitions
                                .contains(&(current_variant.clone(), target.clone())),
                        ),
                    };

                    if !has_variant {
                        return Err(runtime_err(&format!(
                            "unknown variant '{}' for state '{}'",
                            target, state_name
                        )));
                    }
                    if !valid_edge {
                        return Err(runtime_err(&format!(
                            "invalid transition for {}: {} -> {}",
                            state_name, current_variant, target
                        )));
                    }

                    let new_val = Value::StateValue {
                        state_name,
                        variant_name: target,
                    };
                    if !current_env.borrow_mut().assign_existing(&variable, new_val) {
                        return Err(runtime_err(&format!(
                            "undefined state variable '{}'",
                            variable
                        )));
                    }
                }

                // ── Simulate ─────────────────────────────────────────────────────
                Instruction::Simulate { body_idx } => {
                    let step = match pop(stack)? {
                        Value::Number(n) => n,
                        _ => return Err(runtime_err("simulate step must be a number")),
                    };
                    let duration = match pop(stack)? {
                        Value::Number(n) => n,
                        _ => return Err(runtime_err("simulate duration must be a number")),
                    };
                    if step <= 0.0 {
                        return Err(runtime_err("simulate step must be greater than zero"));
                    }
                    if duration < 0.0 {
                        return Err(runtime_err("simulate duration cannot be negative"));
                    }

                    // Clone body chunk to release the borrow on self.program.
                    let body_chunk = self
                        .program
                        .simulate_bodies
                        .get(body_idx)
                        .ok_or_else(|| {
                            runtime_err(&format!("invalid simulate body index {}", body_idx))
                        })?
                        .chunk
                        .clone();

                    let iterations = (duration / step).floor() as usize;
                    for i in 0..iterations {
                        // Each iteration env is a child of the CURRENT env so the
                        // body can read/write block-local and function-local outer
                        // variables (fixes the M8E block-local capture limitation).
                        let iter_env = Env::new_child(Rc::clone(&current_env));
                        iter_env
                            .borrow_mut()
                            .define("time".to_string(), Value::Number(i as f64 * step));

                        let mut iter_stack: Vec<Value> = Vec::new();
                        let ret =
                            self.execute_chunk(&body_chunk, &mut iter_stack, iter_env, is_fn)?;

                        // Propagate a return that originated inside a function.
                        if ret.is_some() {
                            return Ok(ret);
                        }
                    }
                }

                Instruction::Unsupported(feature) => {
                    return Err(runtime_err(&format!(
                        "bytecode feature not yet executable: {}",
                        feature
                    )));
                }
            }
        }
        Ok(None)
    }
}

fn pop(stack: &mut Vec<Value>) -> Result<Value, KiminError> {
    stack.pop().ok_or_else(|| runtime_err("stack underflow"))
}

fn const_to_val(c: &Constant) -> Value {
    match c {
        Constant::Number(n) => Value::Number(*n),
        Constant::Text(s) => Value::Str(s.clone()),
        Constant::Bool(b) => Value::Bool(*b),
        Constant::Nil => Value::Nil,
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Nil => false,
        _ => true,
    }
}

fn runtime_err(msg: &str) -> KiminError {
    KiminError::Runtime(RuntimeError {
        msg: msg.to_string(),
    })
}
