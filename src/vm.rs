use std::collections::HashMap;

use crate::bytecode::{BytecodeProgram, Chunk, Constant, Instruction};
use crate::error::{KiminError, RuntimeError};
use crate::value::Value;

/// Minimal stack-based bytecode VM for Kimin.
///
/// Executes `BytecodeProgram` produced by `BytecodeCompiler`. The tree-walk
/// interpreter (`Interpreter`) remains the source of truth for `kimin run`.
/// This VM is reachable via `kimin vm <file>`.
pub struct Vm {
    program: BytecodeProgram,
    globals: HashMap<String, Value>,
    output: Vec<String>,
}

impl Vm {
    pub fn new(program: BytecodeProgram) -> Self {
        Vm {
            program,
            globals: HashMap::new(),
            output: Vec::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), KiminError> {
        let main = self.program.main.clone();
        let mut stack: Vec<Value> = Vec::new();
        let mut locals: Vec<HashMap<String, Value>> = Vec::new();
        self.execute_chunk(&main, &mut stack, &mut locals, false)?;
        Ok(())
    }

    /// Returns all lines that were printed during execution, in order.
    pub fn take_output(self) -> Vec<String> {
        self.output
    }

    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        stack: &mut Vec<Value>,
        locals: &mut Vec<HashMap<String, Value>>,
        is_fn: bool,
    ) -> Result<Option<Value>, KiminError> {
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

                Instruction::DefineGlobal(name) => {
                    let val = pop(stack)?;
                    self.globals.insert(name, val);
                }
                Instruction::LoadGlobal(name) => {
                    let val = self
                        .globals
                        .get(&name)
                        .ok_or_else(|| runtime_err(&format!("undefined variable '{}'", name)))?
                        .clone();
                    stack.push(val);
                }
                Instruction::StoreGlobal(name) => {
                    let val = pop(stack)?;
                    if !self.globals.contains_key(&name) {
                        return Err(runtime_err(&format!("undefined variable '{}'", name)));
                    }
                    self.globals.insert(name, val);
                }

                Instruction::DefineLocal(name) => {
                    let val = pop(stack)?;
                    match locals.last_mut() {
                        Some(frame) => {
                            frame.insert(name, val);
                        }
                        None => return Err(runtime_err("DefineLocal outside any scope")),
                    }
                }
                Instruction::LoadLocal(name) => {
                    let val = load_local(locals, &name)?;
                    stack.push(val);
                }
                Instruction::StoreLocal(name) => {
                    let val = pop(stack)?;
                    store_local(locals, &name, val)?;
                }

                Instruction::BeginScope => {
                    locals.push(HashMap::new());
                }
                Instruction::EndScope => {
                    locals.pop();
                }

                Instruction::Jump(target) => {
                    ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    // Pop the condition — peek-without-pop leaks the condition onto the stack.
                    let val = pop(stack)?;
                    if !is_truthy(&val) {
                        ip = target;
                    }
                }

                Instruction::LoadFunction(name) => {
                    stack.push(Value::BytecodeFunction(name));
                }

                Instruction::Call { name, arg_count } => {
                    let mut args: Vec<Value> = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(pop(stack)?);
                    }
                    args.reverse();

                    // Clone chunk data out of self.program before recursive call
                    // to avoid holding an immutable borrow across the &mut self call.
                    let (fn_chunk, fn_params, fn_arity) = {
                        let fc = self
                            .program
                            .functions
                            .iter()
                            .find(|f| f.name == name)
                            .ok_or_else(|| runtime_err(&format!("unknown function '{}'", name)))?;
                        (fc.chunk.clone(), fc.params.clone(), fc.arity)
                    };

                    if args.len() != fn_arity {
                        return Err(runtime_err(&format!(
                            "function '{}' expects {} argument(s), got {}",
                            name,
                            fn_arity,
                            args.len()
                        )));
                    }

                    let mut fn_frame: HashMap<String, Value> = HashMap::new();
                    for (param, val) in fn_params.iter().zip(args) {
                        fn_frame.insert(param.clone(), val);
                    }
                    let mut fn_locals: Vec<HashMap<String, Value>> = vec![fn_frame];
                    let mut fn_stack: Vec<Value> = Vec::new();

                    let ret = self.execute_chunk(&fn_chunk, &mut fn_stack, &mut fn_locals, true)?;
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
                    // Return at top level is a no-op (implicit return from main)
                }

                Instruction::Halt => {
                    return Ok(None);
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

fn load_local(locals: &[HashMap<String, Value>], name: &str) -> Result<Value, KiminError> {
    for frame in locals.iter().rev() {
        if let Some(v) = frame.get(name) {
            return Ok(v.clone());
        }
    }
    Err(runtime_err(&format!("undefined local variable '{}'", name)))
}

fn store_local(
    locals: &mut [HashMap<String, Value>],
    name: &str,
    val: Value,
) -> Result<(), KiminError> {
    for frame in locals.iter_mut().rev() {
        if frame.contains_key(name) {
            frame.insert(name.to_string(), val);
            return Ok(());
        }
    }
    Err(runtime_err(&format!("undefined local variable '{}'", name)))
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
