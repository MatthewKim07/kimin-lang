use std::collections::{BTreeMap, HashMap, HashSet};
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
    global_env: EnvRef,
    states: HashMap<String, RuntimeStateMachine>,
    /// Method registry: struct_name → method_name → index into program.methods.
    method_registry: HashMap<String, HashMap<String, usize>>,
    output: Vec<String>,
}

impl Vm {
    pub fn new(program: BytecodeProgram) -> Self {
        let mut method_registry: HashMap<String, HashMap<String, usize>> = HashMap::new();
        for (idx, mc) in program.methods.iter().enumerate() {
            method_registry
                .entry(mc.struct_name.clone())
                .or_default()
                .insert(mc.method_name.clone(), idx);
        }
        Vm {
            program,
            global_env: Env::new_global(),
            states: HashMap::new(),
            method_registry,
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
                // Stack-based call (M8G): callee is on stack below the arguments.
                // Layout before CALL n: [..., callee, arg1, ..., argN]
                Instruction::Call { arg_count } => {
                    // Pop args in reverse order, then reverse to restore original order.
                    let mut args: Vec<Value> = (0..arg_count)
                        .map(|_| pop(stack))
                        .collect::<Result<Vec<_>, _>>()?;
                    args.reverse();

                    // Pop callee from below the args.
                    let callee = pop(stack)?;

                    let (fn_chunk, fn_params, fn_arity, fn_name, captured_env) = match callee {
                        Value::BytecodeFunction {
                            name: fn_name,
                            env: captured_env,
                        } => {
                            // Clone chunk data before the recursive execute_chunk call
                            // to release the borrow on self.program.
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
                            (chunk, params, arity, fn_name, captured_env)
                        }
                        other => {
                            return Err(runtime_err(&format!(
                                "attempted to call non-function value of type {}",
                                other.type_name()
                            )))
                        }
                    };

                    if args.len() != fn_arity {
                        return Err(runtime_err(&format!(
                            "function '{}' expects {} argument(s), got {}",
                            fn_name,
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

                // ── Arrays ───────────────────────────────────────────────────────
                Instruction::Array { count } => {
                    let mut elements: Vec<Value> = (0..count)
                        .map(|_| pop(stack))
                        .collect::<Result<Vec<_>, _>>()?;
                    // Elements were pushed left-to-right, so reverse to restore order.
                    elements.reverse();
                    stack.push(Value::Array(elements));
                }

                Instruction::Map { count } => {
                    // Pairs were pushed key1,val1,key2,val2,... left-to-right.
                    // Pop count pairs (top-of-stack = last pair), then reverse to source order.
                    let mut pairs: Vec<(String, Value)> = Vec::with_capacity(count);
                    for _ in 0..count {
                        let val = pop(stack)?;
                        let key = match pop(stack)? {
                            Value::Str(s) => s,
                            other => {
                                return Err(runtime_err(&format!(
                                    "map key must be Text, got {}",
                                    other.type_name()
                                )))
                            }
                        };
                        pairs.push((key, val));
                    }
                    // Reverse to restore source order, then insert so later duplicates win.
                    pairs.reverse();
                    let mut map = BTreeMap::new();
                    for (k, v) in pairs {
                        map.insert(k, v);
                    }
                    stack.push(Value::Map(map));
                }

                Instruction::Index => {
                    let idx_val = pop(stack)?;
                    let arr_val = pop(stack)?;
                    match arr_val {
                        Value::Map(map) => {
                            let key = match idx_val {
                                Value::Str(s) => s,
                                other => {
                                    return Err(runtime_err(&format!(
                                        "map key must be Text, got {}",
                                        other.type_name()
                                    )))
                                }
                            };
                            match map.get(&key) {
                                Some(v) => stack.push(v.clone()),
                                None => {
                                    return Err(runtime_err(&format!(
                                        "map key '{}' not found",
                                        key
                                    )))
                                }
                            }
                        }
                        arr_val => {
                            let n = match idx_val {
                                Value::Number(n) => n,
                                _ => return Err(runtime_err("index must be a Number")),
                            };
                            if n.fract() != 0.0 {
                                return Err(runtime_err("index must be an integer"));
                            }
                            if n < 0.0 {
                                return Err(runtime_err("index out of bounds: index is negative"));
                            }
                            let i = n as usize;
                            match arr_val {
                                Value::Array(arr) => {
                                    if i >= arr.len() {
                                        return Err(runtime_err(&format!(
                                            "array index out of bounds: index {} but length is {}",
                                            i,
                                            arr.len()
                                        )));
                                    }
                                    stack.push(arr[i].clone());
                                }
                                Value::Str(s) => {
                                    let chars: Vec<char> = s.chars().collect();
                                    if i >= chars.len() {
                                        return Err(runtime_err(&format!(
                                            "string index out of bounds: index {} but length is {}",
                                            i,
                                            chars.len()
                                        )));
                                    }
                                    stack.push(Value::Str(chars[i].to_string()));
                                }
                                other => {
                                    return Err(runtime_err(&format!(
                                        "cannot index into value of type {}",
                                        other.type_name()
                                    )))
                                }
                            }
                        }
                    }
                }

                Instruction::Len => {
                    let val = pop(stack)?;
                    match val {
                        Value::Array(v) => stack.push(Value::Number(v.len() as f64)),
                        Value::Str(s) => stack.push(Value::Number(s.chars().count() as f64)),
                        other => {
                            return Err(runtime_err(&format!(
                                "len() requires Array or Text, got {}",
                                other.type_name()
                            )))
                        }
                    }
                }

                Instruction::Slice => {
                    let end_val = pop(stack)?;
                    let start_val = pop(stack)?;
                    let arr_val = pop(stack)?;
                    let s = match start_val {
                        Value::Number(n) => n,
                        _ => return Err(runtime_err("slice start must be Number")),
                    };
                    if s.fract() != 0.0 {
                        return Err(runtime_err(&format!(
                            "slice start must be an integer, got {}",
                            s
                        )));
                    }
                    if s < 0.0 {
                        return Err(runtime_err("slice start must be non-negative"));
                    }
                    let e = match end_val {
                        Value::Number(n) => n,
                        _ => return Err(runtime_err("slice end must be Number")),
                    };
                    if e.fract() != 0.0 {
                        return Err(runtime_err(&format!(
                            "slice end must be an integer, got {}",
                            e
                        )));
                    }
                    if e < 0.0 {
                        return Err(runtime_err("slice end must be non-negative"));
                    }
                    let si = s as usize;
                    let ei = e as usize;
                    if si > ei {
                        return Err(runtime_err(&format!(
                            "slice start {} is greater than end {}",
                            si, ei
                        )));
                    }
                    match arr_val {
                        Value::Array(elems) => {
                            if ei > elems.len() {
                                return Err(runtime_err(&format!(
                                    "slice end {} is out of bounds for array of length {}",
                                    ei,
                                    elems.len()
                                )));
                            }
                            stack.push(Value::Array(elems[si..ei].to_vec()));
                        }
                        Value::Str(str_val) => {
                            let chars: Vec<char> = str_val.chars().collect();
                            if ei > chars.len() {
                                return Err(runtime_err(&format!(
                                    "slice end {} is out of bounds for string of length {}",
                                    ei,
                                    chars.len()
                                )));
                            }
                            stack.push(Value::Str(chars[si..ei].iter().collect()));
                        }
                        other => {
                            return Err(runtime_err(&format!(
                                "slice target must be Array or Text, got {}",
                                other.type_name()
                            )))
                        }
                    }
                }

                Instruction::SetIndex(name) => {
                    // Stack before: [..., index_value, new_value]
                    let new_elem = pop(stack)?;
                    let idx_val = pop(stack)?;

                    // Read current variable; dispatch on Array vs Map.
                    let current = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;

                    match current {
                        Value::Map(mut map) => {
                            let key = match idx_val {
                                Value::Str(s) => s,
                                other => {
                                    return Err(runtime_err(&format!(
                                        "map index key must be Text, got {}",
                                        other.type_name()
                                    )))
                                }
                            };
                            map.insert(key, new_elem);
                            if !current_env
                                .borrow_mut()
                                .assign_existing(&name, Value::Map(map))
                            {
                                return Err(runtime_err(&format!("undefined variable '{}'", name)));
                            }
                        }
                        Value::Array(_) => {
                            // Re-read to get the Vec (borrow already dropped above).
                            let current2 = current_env.borrow().get(&name).ok_or_else(|| {
                                runtime_err(&format!("undefined variable '{}'", name))
                            })?;
                            let mut elems = match current2 {
                                Value::Array(v) => v,
                                _ => unreachable!(),
                            };
                            let n = match idx_val {
                                Value::Number(n) => n,
                                _ => return Err(runtime_err("array index must be a Number")),
                            };
                            if n.fract() != 0.0 {
                                return Err(runtime_err("array index must be an integer"));
                            }
                            if n < 0.0 {
                                return Err(runtime_err(
                                    "array index out of bounds: index is negative",
                                ));
                            }
                            let i = n as usize;
                            if i >= elems.len() {
                                return Err(runtime_err(&format!(
                                    "array index out of bounds: index {} but length is {}",
                                    i,
                                    elems.len()
                                )));
                            }
                            elems[i] = new_elem;
                            if !current_env
                                .borrow_mut()
                                .assign_existing(&name, Value::Array(elems))
                            {
                                return Err(runtime_err(&format!("undefined variable '{}'", name)));
                            }
                        }
                        other => {
                            return Err(runtime_err(&format!(
                                "cannot index-assign into value of type {}",
                                other.type_name()
                            )));
                        }
                    }
                }

                Instruction::IndexCompoundAssign { name, op } => {
                    // Stack before: [..., index_value, rhs_value]
                    let rhs = pop(stack)?;
                    let idx_val = pop(stack)?;

                    let current = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;

                    match current {
                        Value::Map(mut map) => {
                            let key = match idx_val {
                                Value::Str(s) => s,
                                other => {
                                    return Err(runtime_err(&format!(
                                        "map index key must be Text, got {}",
                                        other.type_name()
                                    )))
                                }
                            };
                            let old_val = map.get(&key).cloned().ok_or_else(|| {
                                runtime_err(&format!("map key '{}' not found", key))
                            })?;
                            let new_val = apply_compound_op(&op, old_val, rhs)?;
                            map.insert(key, new_val);
                            if !current_env
                                .borrow_mut()
                                .assign_existing(&name, Value::Map(map))
                            {
                                return Err(runtime_err(&format!("undefined variable '{}'", name)));
                            }
                        }
                        Value::Array(_) => {
                            // Re-read to get the Vec (borrow already dropped above).
                            let current2 = current_env.borrow().get(&name).ok_or_else(|| {
                                runtime_err(&format!("undefined variable '{}'", name))
                            })?;
                            let mut elems = match current2 {
                                Value::Array(v) => v,
                                _ => unreachable!(),
                            };
                            let n = match idx_val {
                                Value::Number(n) => n,
                                _ => return Err(runtime_err("array index must be a Number")),
                            };
                            if n.fract() != 0.0 {
                                return Err(runtime_err("array index must be an integer"));
                            }
                            if n < 0.0 {
                                return Err(runtime_err(
                                    "array index out of bounds: index is negative",
                                ));
                            }
                            let i = n as usize;
                            if i >= elems.len() {
                                return Err(runtime_err(&format!(
                                    "array index out of bounds: index {} but length is {}",
                                    i,
                                    elems.len()
                                )));
                            }
                            let old_elem = elems[i].clone();
                            let new_elem = apply_compound_op(&op, old_elem, rhs)?;
                            elems[i] = new_elem;
                            if !current_env
                                .borrow_mut()
                                .assign_existing(&name, Value::Array(elems))
                            {
                                return Err(runtime_err(&format!("undefined variable '{}'", name)));
                            }
                        }
                        other => {
                            return Err(runtime_err(&format!(
                                "cannot index-compound-assign into value of type {}",
                                other.type_name()
                            )));
                        }
                    }
                }

                Instruction::ArrayPush(name) => {
                    let new_elem = pop(stack)?;
                    let current = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;
                    let mut elems = match current {
                        Value::Array(v) => v,
                        other => {
                            return Err(runtime_err(&format!(
                                "push() requires Array, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    elems.push(new_elem);
                    if !current_env
                        .borrow_mut()
                        .assign_existing(&name, Value::Array(elems))
                    {
                        return Err(runtime_err(&format!("undefined variable '{}'", name)));
                    }
                    stack.push(Value::Nil);
                }

                Instruction::ArrayPop(name) => {
                    let current = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;
                    let mut elems = match current {
                        Value::Array(v) => v,
                        other => {
                            return Err(runtime_err(&format!(
                                "pop() requires Array, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    if elems.is_empty() {
                        return Err(runtime_err("cannot pop from empty array"));
                    }
                    let popped = elems.pop().unwrap();
                    if !current_env
                        .borrow_mut()
                        .assign_existing(&name, Value::Array(elems))
                    {
                        return Err(runtime_err(&format!("undefined variable '{}'", name)));
                    }
                    stack.push(popped);
                }

                Instruction::Contains => {
                    let pattern = pop(stack)?;
                    let text = pop(stack)?;
                    let t = match text {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "contains() first argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let p = match pattern {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "contains() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Bool(t.contains(p.as_str())));
                }

                Instruction::StartsWith => {
                    let prefix = pop(stack)?;
                    let text = pop(stack)?;
                    let t = match text {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "starts_with() first argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let p = match prefix {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "starts_with() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Bool(t.starts_with(p.as_str())));
                }

                Instruction::EndsWith => {
                    let suffix = pop(stack)?;
                    let text = pop(stack)?;
                    let t = match text {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "ends_with() first argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let s = match suffix {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "ends_with() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Bool(t.ends_with(s.as_str())));
                }

                Instruction::ToUpper => {
                    let val = pop(stack)?;
                    let s = match val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "to_upper() argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Str(s.to_uppercase()));
                }

                Instruction::ToLower => {
                    let val = pop(stack)?;
                    let s = match val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "to_lower() argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Str(s.to_lowercase()));
                }

                Instruction::Trim => {
                    let val = pop(stack)?;
                    let s = match val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "trim() argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Str(s.trim().to_string()));
                }

                Instruction::ToString => {
                    let val = pop(stack)?;
                    stack.push(Value::Str(format!("{}", val)));
                }

                Instruction::ToNumber => {
                    let val = pop(stack)?;
                    let text = match val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "to_number() expects Text, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let n =
                        crate::value::parse_number_from_text(&text).map_err(|e| runtime_err(&e))?;
                    stack.push(Value::Number(n));
                }

                Instruction::ToBool => {
                    let val = pop(stack)?;
                    let text = match val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "to_bool() expects Text, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let b =
                        crate::value::parse_bool_from_text(&text).map_err(|e| runtime_err(&e))?;
                    stack.push(Value::Bool(b));
                }

                Instruction::Ln => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "ln() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if n <= 0.0 {
                        return Err(runtime_err(&format!(
                            "ln requires positive Number, got {}",
                            n
                        )));
                    }
                    let r = n.ln();
                    if !r.is_finite() {
                        return Err(runtime_err("ln result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Log2 => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "log2() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if n <= 0.0 {
                        return Err(runtime_err(&format!(
                            "log2 requires positive Number, got {}",
                            n
                        )));
                    }
                    let r = n.log2();
                    if !r.is_finite() {
                        return Err(runtime_err("log2 result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Log10 => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "log10() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if n <= 0.0 {
                        return Err(runtime_err(&format!(
                            "log10 requires positive Number, got {}",
                            n
                        )));
                    }
                    let r = n.log10();
                    if !r.is_finite() {
                        return Err(runtime_err("log10 result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Exp => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "exp() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let r = n.exp();
                    if !r.is_finite() {
                        return Err(runtime_err(&format!(
                            "exp result is not finite (exp({}))",
                            n
                        )));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Pi => {
                    stack.push(Value::Number(std::f64::consts::PI));
                }

                Instruction::EConst => {
                    stack.push(Value::Number(std::f64::consts::E));
                }

                Instruction::Tau => {
                    stack.push(Value::Number(std::f64::consts::TAU));
                }

                Instruction::Phi => {
                    stack.push(Value::Number((1.0 + 5.0_f64.sqrt()) / 2.0));
                }

                Instruction::Sin => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "sin() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("sin input is not finite"));
                    }
                    let r = n.sin();
                    if !r.is_finite() {
                        return Err(runtime_err("sin result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Cos => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "cos() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("cos input is not finite"));
                    }
                    let r = n.cos();
                    if !r.is_finite() {
                        return Err(runtime_err("cos result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Tan => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "tan() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("tan input is not finite"));
                    }
                    let r = n.tan();
                    if !r.is_finite() {
                        return Err(runtime_err("tan result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Clamp => {
                    let hi_val = pop(stack)?;
                    let lo_val = pop(stack)?;
                    let n_val = pop(stack)?;
                    let hi = match hi_val {
                        Value::Number(v) => v,
                        other => {
                            return Err(runtime_err(&format!(
                                "clamp() argument 3 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let lo = match lo_val {
                        Value::Number(v) => v,
                        other => {
                            return Err(runtime_err(&format!(
                                "clamp() argument 2 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let n = match n_val {
                        Value::Number(v) => v,
                        other => {
                            return Err(runtime_err(&format!(
                                "clamp() argument 1 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() || !lo.is_finite() || !hi.is_finite() {
                        return Err(runtime_err("clamp input is not finite"));
                    }
                    if lo > hi {
                        return Err(runtime_err(
                            "clamp lower bound cannot be greater than upper bound",
                        ));
                    }
                    let result = if n < lo {
                        lo
                    } else if n > hi {
                        hi
                    } else {
                        n
                    };
                    stack.push(Value::Number(result));
                }

                Instruction::Hypot => {
                    let b_val = pop(stack)?;
                    let a_val = pop(stack)?;
                    let b = match b_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "hypot() argument 2 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let a = match a_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "hypot() argument 1 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !a.is_finite() || !b.is_finite() {
                        return Err(runtime_err("hypot input is not finite"));
                    }
                    let r = a.hypot(b);
                    if !r.is_finite() {
                        return Err(runtime_err("hypot result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Asin => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "asin() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("asin input is not finite"));
                    }
                    if n < -1.0 || n > 1.0 {
                        return Err(runtime_err(&format!(
                            "asin requires input in [-1, 1], got {}",
                            n
                        )));
                    }
                    let r = n.asin();
                    if !r.is_finite() {
                        return Err(runtime_err("asin result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Acos => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "acos() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("acos input is not finite"));
                    }
                    if n < -1.0 || n > 1.0 {
                        return Err(runtime_err(&format!(
                            "acos requires input in [-1, 1], got {}",
                            n
                        )));
                    }
                    let r = n.acos();
                    if !r.is_finite() {
                        return Err(runtime_err("acos result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Atan => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "atan() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !n.is_finite() {
                        return Err(runtime_err("atan input is not finite"));
                    }
                    let r = n.atan();
                    if !r.is_finite() {
                        return Err(runtime_err("atan result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Atan2 => {
                    let x_val = pop(stack)?;
                    let y_val = pop(stack)?;
                    let x = match x_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "atan2() argument 2 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let y = match y_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "atan2() argument 1 expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if !y.is_finite() || !x.is_finite() {
                        return Err(runtime_err("atan2 input is not finite"));
                    }
                    let r = y.atan2(x);
                    if !r.is_finite() {
                        return Err(runtime_err("atan2 result is not finite"));
                    }
                    stack.push(Value::Number(r));
                }

                Instruction::Sqrt => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "sqrt() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if n < 0.0 {
                        return Err(runtime_err(&format!(
                            "sqrt requires non-negative Number, got {}",
                            n
                        )));
                    }
                    let result = n.sqrt();
                    if !result.is_finite() {
                        return Err(runtime_err("sqrt result is not finite"));
                    }
                    stack.push(Value::Number(result));
                }

                Instruction::Pow => {
                    // Stack: [..., base, exp] — exp on top.
                    let exp_val = pop(stack)?;
                    let base_val = pop(stack)?;
                    let base = match base_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "pow() first argument expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let exp = match exp_val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "pow() second argument expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let result = base.powf(exp);
                    if !result.is_finite() {
                        return Err(runtime_err(&format!(
                            "pow result is not finite (pow({}, {}))",
                            base, exp
                        )));
                    }
                    stack.push(Value::Number(result));
                }

                Instruction::Abs => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "abs() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    stack.push(Value::Number(n.abs()));
                }

                Instruction::Floor => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "floor() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    stack.push(Value::Number(n.floor()));
                }

                Instruction::Ceil => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "ceil() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    stack.push(Value::Number(n.ceil()));
                }

                Instruction::Round => {
                    let val = pop(stack)?;
                    let n = match val {
                        Value::Number(n) => n,
                        other => {
                            return Err(runtime_err(&format!(
                                "round() expects Number, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    stack.push(Value::Number(n.round()));
                }

                Instruction::Min => {
                    // Stack: [..., a, b] — b on top.
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    let (an, bn) = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => (x, y),
                        _ => return Err(runtime_err("min() requires two Number arguments")),
                    };
                    stack.push(Value::Number(an.min(bn)));
                }

                Instruction::Max => {
                    let b = pop(stack)?;
                    let a = pop(stack)?;
                    let (an, bn) = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => (x, y),
                        _ => return Err(runtime_err("max() requires two Number arguments")),
                    };
                    stack.push(Value::Number(an.max(bn)));
                }

                Instruction::Format { arg_count } => {
                    // Pop arg_count args in reverse, then pop template
                    let mut fmt_args: Vec<Value> = (0..arg_count)
                        .map(|_| pop(stack))
                        .collect::<Result<Vec<_>, _>>()?;
                    fmt_args.reverse(); // restore source order
                    let tmpl_val = pop(stack)?;
                    let template = match tmpl_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "format() expects template Text, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    let result = crate::value::format_template(&template, &fmt_args)
                        .map_err(|msg| runtime_err(&msg))?;
                    stack.push(Value::Str(result));
                }

                Instruction::Split => {
                    let delim_val = pop(stack)?;
                    let text_val = pop(stack)?;
                    let delimiter = match delim_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "split() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let text = match text_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "split() first argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let parts: Vec<Value> = if delimiter.is_empty() {
                        text.chars().map(|c| Value::Str(c.to_string())).collect()
                    } else {
                        text.split(delimiter.as_str())
                            .map(|p| Value::Str(p.to_string()))
                            .collect()
                    };
                    stack.push(Value::Array(parts));
                }

                Instruction::Join => {
                    let delim_val = pop(stack)?;
                    let parts_val = pop(stack)?;
                    let delimiter = match delim_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "join() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let parts = match parts_val {
                        Value::Array(elems) => elems,
                        other => {
                            return Err(runtime_err(&format!(
                                "join() first argument must be Array<Text>, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let strs: Result<Vec<String>, KiminError> = parts
                        .iter()
                        .map(|v| match v {
                            Value::Str(s) => Ok(s.clone()),
                            other => Err(runtime_err(&format!(
                                "join() array element must be Text, got {}",
                                other.type_name()
                            ))),
                        })
                        .collect();
                    stack.push(Value::Str(strs?.join(delimiter.as_str())));
                }

                Instruction::HasKey => {
                    let key_val = pop(stack)?;
                    let map_val = pop(stack)?;
                    let key = match key_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "has_key() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let map = match map_val {
                        Value::Map(m) => m,
                        other => {
                            return Err(runtime_err(&format!(
                                "has_key() first argument must be Map, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    stack.push(Value::Bool(map.contains_key(&key)));
                }

                Instruction::Keys => {
                    let map_val = pop(stack)?;
                    let map = match map_val {
                        Value::Map(m) => m,
                        other => {
                            return Err(runtime_err(&format!(
                                "keys() argument must be Map, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let ks: Vec<Value> = map.keys().map(|k| Value::Str(k.clone())).collect();
                    stack.push(Value::Array(ks));
                }

                Instruction::Values => {
                    let map_val = pop(stack)?;
                    let map = match map_val {
                        Value::Map(m) => m,
                        other => {
                            return Err(runtime_err(&format!(
                                "values() argument must be Map, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let vs: Vec<Value> = map.values().cloned().collect();
                    stack.push(Value::Array(vs));
                }

                Instruction::RemoveKey(name) => {
                    let key_val = pop(stack)?;
                    let key = match key_val {
                        Value::Str(s) => s,
                        other => {
                            return Err(runtime_err(&format!(
                                "remove() second argument must be Text, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let current = current_env
                        .borrow()
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?;
                    let mut map = match current {
                        Value::Map(m) => m,
                        other => {
                            return Err(runtime_err(&format!(
                                "remove() first argument must be Map, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let removed = map
                        .remove(&key)
                        .ok_or_else(|| runtime_err(&format!("map key '{}' not found", key)))?;
                    current_env
                        .borrow_mut()
                        .assign_existing(&name, Value::Map(map));
                    stack.push(removed);
                }

                Instruction::StructLiteral { name, fields } => {
                    let count = fields.len();
                    let mut vals: Vec<Value> = Vec::with_capacity(count);
                    for _ in 0..count {
                        vals.push(pop(stack)?);
                    }
                    vals.reverse(); // restore source order
                    let mut field_map = std::collections::BTreeMap::new();
                    for (field_name, val) in fields.iter().zip(vals.into_iter()) {
                        field_map.insert(field_name.clone(), val);
                    }
                    stack.push(Value::Struct {
                        name: name.clone(),
                        fields: field_map,
                    });
                }

                Instruction::FieldAccess(field) => {
                    let val = pop(stack)?;
                    match val {
                        Value::Struct { fields, .. } => {
                            let v = fields.get(&field).ok_or_else(|| {
                                runtime_err(&format!("struct has no field '{}'", field))
                            })?;
                            stack.push(v.clone());
                        }
                        other => {
                            return Err(runtime_err(&format!(
                                "cannot access field '{}' on {}",
                                field,
                                other.type_name()
                            )));
                        }
                    }
                }

                Instruction::SetPath { root, steps } => {
                    // Stack: [..., index0_val, ..., indexN_val, new_value]
                    let n_index = steps
                        .iter()
                        .filter(|s| matches!(s, crate::ast::PathStep::Index))
                        .count();
                    let new_val = pop(stack)?;
                    // Pop index values in stack order (top = last compiled = rightmost index).
                    let mut idx_vals_rev: Vec<Value> =
                        (0..n_index).map(|_| pop(stack)).collect::<Result<_, _>>()?;
                    idx_vals_rev.reverse(); // restore left-to-right (source) order
                    let root_val = current_env
                        .borrow()
                        .get(&root)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", root)))?;
                    let updated = vm_update_path(root_val, &steps, &idx_vals_rev, &mut 0, new_val)?;
                    if !current_env.borrow_mut().assign_existing(&root, updated) {
                        return Err(runtime_err(&format!("undefined variable '{}'", root)));
                    }
                }

                Instruction::PathCompoundAssign { root, steps, op } => {
                    // Stack: [..., index0_val, ..., indexN_val, rhs_value]
                    // RHS is evaluated first (compiler order), so it is on top.
                    let n_index = steps
                        .iter()
                        .filter(|s| matches!(s, crate::ast::PathStep::Index))
                        .count();
                    let rhs = pop(stack)?;
                    let mut idx_vals_rev: Vec<Value> =
                        (0..n_index).map(|_| pop(stack)).collect::<Result<_, _>>()?;
                    idx_vals_rev.reverse();
                    let root_val = current_env
                        .borrow()
                        .get(&root)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", root)))?;
                    // Read old value at path (struct was already modified by RHS side effects if any).
                    let old_val = vm_read_path(&root_val, &steps, &idx_vals_rev, &mut 0)?;
                    let new_val = apply_compound_op(&op, old_val, rhs)?;
                    let updated = vm_update_path(root_val, &steps, &idx_vals_rev, &mut 0, new_val)?;
                    if !current_env.borrow_mut().assign_existing(&root, updated) {
                        return Err(runtime_err(&format!("undefined variable '{}'", root)));
                    }
                }

                Instruction::CallMethod { method, arg_count } => {
                    // Stack: [..., receiver, arg1, ..., argN]
                    let mut args_rev: Vec<Value> = (0..arg_count)
                        .map(|_| pop(stack))
                        .collect::<Result<_, _>>()?;
                    args_rev.reverse(); // restore source order
                    let receiver = pop(stack)?;

                    let struct_name = match &receiver {
                        Value::Struct { name, .. } => name.clone(),
                        other => {
                            return Err(runtime_err(&format!(
                                "cannot call method '{}' on {}",
                                method,
                                other.type_name()
                            )));
                        }
                    };

                    let method_idx = self
                        .method_registry
                        .get(&struct_name)
                        .and_then(|m| m.get(&method))
                        .copied()
                        .ok_or_else(|| {
                            runtime_err(&format!(
                                "struct '{}' has no method '{}'",
                                struct_name, method
                            ))
                        })?;

                    // Clone chunk data before recursive execute_chunk to avoid borrow conflict.
                    let mc_chunk = self.program.methods[method_idx].chunk.clone();
                    let mc_params = self.program.methods[method_idx].params.clone();

                    // Build method env as child of global env.
                    let method_env = Env::new_child(Rc::clone(&self.global_env));
                    {
                        let mut env_borrow = method_env.borrow_mut();
                        // params[0] is "self"
                        env_borrow.define("self".to_string(), receiver);
                        for (param, val) in mc_params.iter().skip(1).zip(args_rev.iter()) {
                            env_borrow.define(param.clone(), val.clone());
                        }
                    }

                    let mut method_stack: Vec<Value> = Vec::new();
                    let ret = self.execute_chunk(&mc_chunk, &mut method_stack, method_env, true)?;
                    stack.push(ret.unwrap_or(Value::Nil));
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

fn apply_compound_op(
    op: &crate::ast::CompoundAssignOp,
    a: Value,
    b: Value,
) -> Result<Value, KiminError> {
    use crate::ast::CompoundAssignOp;
    match op {
        CompoundAssignOp::Add => match (a, b) {
            (Value::Number(x), Value::Number(y)) => Ok(Value::Number(x + y)),
            (Value::Str(x), Value::Str(y)) => Ok(Value::Str(x + &y)),
            _ => Err(runtime_err("'+=' requires two numbers or two strings")),
        },
        CompoundAssignOp::Subtract => match (a, b) {
            (Value::Number(x), Value::Number(y)) => Ok(Value::Number(x - y)),
            _ => Err(runtime_err("'-=' requires numbers")),
        },
        CompoundAssignOp::Multiply => match (a, b) {
            (Value::Number(x), Value::Number(y)) => Ok(Value::Number(x * y)),
            _ => Err(runtime_err("'*=' requires numbers")),
        },
        CompoundAssignOp::Divide => match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                if y == 0.0 {
                    return Err(runtime_err("division by zero"));
                }
                Ok(Value::Number(x / y))
            }
            _ => Err(runtime_err("'/=' requires numbers")),
        },
    }
}

// ---- Path read/update helpers for SetPath / PathCompoundAssign ----

fn vm_read_path(
    val: &Value,
    steps: &[crate::ast::PathStep],
    index_vals: &[Value],
    idx_pos: &mut usize,
) -> Result<Value, KiminError> {
    if steps.is_empty() {
        return Ok(val.clone());
    }
    match &steps[0] {
        crate::ast::PathStep::Field(field) => match val {
            Value::Struct { fields, .. } => {
                let inner = fields
                    .get(field.as_str())
                    .ok_or_else(|| runtime_err(&format!("struct has no field '{}'", field)))?;
                vm_read_path(inner, &steps[1..], index_vals, idx_pos)
            }
            other => Err(runtime_err(&format!(
                "cannot access field on {}",
                other.type_name()
            ))),
        },
        crate::ast::PathStep::Index => {
            let idx_val = &index_vals[*idx_pos];
            *idx_pos += 1;
            match val {
                Value::Array(elems) => {
                    let i = vm_array_index(idx_val, elems.len())?;
                    vm_read_path(&elems[i], &steps[1..], index_vals, idx_pos)
                }
                Value::Map(map) => {
                    let key = vm_map_key(idx_val)?;
                    let inner = map
                        .get(&key)
                        .ok_or_else(|| runtime_err(&format!("map key '{}' not found", key)))?;
                    vm_read_path(inner, &steps[1..], index_vals, idx_pos)
                }
                other => Err(runtime_err(&format!(
                    "cannot index into {}",
                    other.type_name()
                ))),
            }
        }
    }
}

fn vm_update_path(
    val: Value,
    steps: &[crate::ast::PathStep],
    index_vals: &[Value],
    idx_pos: &mut usize,
    new_val: Value,
) -> Result<Value, KiminError> {
    if steps.is_empty() {
        return Ok(new_val);
    }
    match &steps[0] {
        crate::ast::PathStep::Field(field) => match val {
            Value::Struct {
                name: sn,
                mut fields,
            } => {
                if !fields.contains_key(field.as_str()) {
                    return Err(runtime_err(&format!(
                        "struct '{}' has no field '{}'",
                        sn, field
                    )));
                }
                let old = fields.remove(field).unwrap();
                let updated = vm_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                fields.insert(field.clone(), updated);
                Ok(Value::Struct { name: sn, fields })
            }
            other => Err(runtime_err(&format!(
                "cannot assign field on {}",
                other.type_name()
            ))),
        },
        crate::ast::PathStep::Index => {
            let idx_val = index_vals[*idx_pos].clone();
            *idx_pos += 1;
            match val {
                Value::Array(mut elems) => {
                    let i = vm_array_index(&idx_val, elems.len())?;
                    let old = elems[i].clone();
                    elems[i] = vm_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                    Ok(Value::Array(elems))
                }
                Value::Map(mut map) => {
                    let key = vm_map_key(&idx_val)?;
                    let old = map
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| runtime_err(&format!("map key '{}' not found", key)))?;
                    let updated = vm_update_path(old, &steps[1..], index_vals, idx_pos, new_val)?;
                    map.insert(key, updated);
                    Ok(Value::Map(map))
                }
                other => Err(runtime_err(&format!(
                    "cannot index into {}",
                    other.type_name()
                ))),
            }
        }
    }
}

fn vm_array_index(idx_val: &Value, len: usize) -> Result<usize, KiminError> {
    let n = match idx_val {
        Value::Number(n) => *n,
        other => {
            return Err(runtime_err(&format!(
                "array index must be Number, got {}",
                other.type_name()
            )))
        }
    };
    if n.fract() != 0.0 {
        return Err(runtime_err(&format!(
            "array index must be an integer, got {}",
            n
        )));
    }
    if n < 0.0 {
        return Err(runtime_err(&format!(
            "array index out of bounds: {} is negative",
            n as i64
        )));
    }
    let i = n as usize;
    if i >= len {
        return Err(runtime_err(&format!(
            "array index out of bounds: index {} but length is {}",
            i, len
        )));
    }
    Ok(i)
}

fn vm_map_key(idx_val: &Value) -> Result<String, KiminError> {
    match idx_val {
        Value::Str(s) => Ok(s.clone()),
        other => Err(runtime_err(&format!(
            "map index key must be Text, got {}",
            other.type_name()
        ))),
    }
}
