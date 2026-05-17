use std::collections::HashSet;

use crate::ast::{BinaryOp, CompoundAssignOp, Expr, Param, Stmt, UnaryOp};
use crate::bytecode::{
    BytecodeProgram, Chunk, Constant, FunctionChunk, Instruction, SimulateChunk,
};
use crate::error::CompileError;

pub struct BytecodeCompiler {
    chunk: Chunk,
    /// Names defined at global scope (depth 0). Used to correctly classify variable
    /// references inside blocks as GLOBAL or LOCAL regardless of current scope depth.
    globals: HashSet<String>,
    /// Stack of locally-defined name sets, one entry per active block scope. The
    /// innermost scope is at the end. A name present in any layer here is LOCAL; a name
    /// in `globals` that is absent from all layers is GLOBAL.
    locals_stack: Vec<HashSet<String>>,
    /// Function chunks collected during compilation, in source order.
    functions: Vec<FunctionChunk>,
    /// Simulate body chunks collected during compilation, in source order.
    simulate_bodies: Vec<SimulateChunk>,
}

impl BytecodeCompiler {
    pub fn new() -> Self {
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals: HashSet::new(),
            locals_stack: Vec::new(),
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
        }
    }

    /// Creates a compiler seeded for a function body. Parameters are pre-loaded as
    /// the innermost local scope so they resolve to LoadLocal inside the body.
    fn new_for_function(globals: HashSet<String>, params: &[Param]) -> Self {
        let param_scope: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals,
            locals_stack: vec![param_scope],
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
        }
    }

    /// Creates a compiler seeded for a simulate body. `"time"` is pre-loaded as
    /// the innermost local scope so it resolves to LoadLocal inside the body.
    /// Outer variables (from globals) remain accessible as globals.
    /// Note: variables from enclosing block scopes (outer locals) are not
    /// accessible — only top-level (global) outer variables work.
    fn new_for_simulate(globals: HashSet<String>) -> Self {
        let time_scope: HashSet<String> = ["time".to_string()].into_iter().collect();
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals,
            locals_stack: vec![time_scope],
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
        }
    }

    /// Lowers a parsed program to bytecode. Emits HALT at end.
    pub fn compile(mut self, stmts: &[Stmt]) -> Result<BytecodeProgram, CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        self.chunk.emit(Instruction::Halt);
        Ok(BytecodeProgram::new(
            self.chunk,
            self.functions,
            self.simulate_bodies,
        ))
    }

    /// Compiles a function body (no HALT). Appends NIL + RETURN if body does not
    /// already end with an explicit RETURN.
    fn compile_function_body(
        mut self,
        stmts: &[Stmt],
    ) -> Result<(Chunk, Vec<FunctionChunk>, Vec<SimulateChunk>), CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        let needs_return = self
            .chunk
            .instructions
            .last()
            .map(|i| !matches!(i, Instruction::Return))
            .unwrap_or(true);
        if needs_return {
            self.chunk.emit(Instruction::Nil);
            self.chunk.emit(Instruction::Return);
        }
        Ok((self.chunk, self.functions, self.simulate_bodies))
    }

    /// Compiles a simulate body. No HALT or RETURN appended.
    fn compile_simulate_body(
        mut self,
        stmts: &[Stmt],
    ) -> Result<(Chunk, Vec<FunctionChunk>, Vec<SimulateChunk>), CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        Ok((self.chunk, self.functions, self.simulate_bodies))
    }

    /// Returns true if `name` resolves to a local variable in any active block scope.
    /// A name that is NOT local is classified as global.
    fn is_local(&self, name: &str) -> bool {
        self.locals_stack
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                self.compile_expr(value)?;
                if self.locals_stack.is_empty() {
                    // Top-level (global) scope.
                    self.globals.insert(name.clone());
                    self.chunk.emit(Instruction::DefineGlobal(name.clone()));
                } else {
                    // Inside a block or function body — register as local.
                    self.locals_stack.last_mut().unwrap().insert(name.clone());
                    self.chunk.emit(Instruction::DefineLocal(name.clone()));
                }
            }

            Stmt::Assign { name, value, .. } => {
                self.compile_expr(value)?;
                if self.is_local(name) {
                    self.chunk.emit(Instruction::StoreLocal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::StoreGlobal(name.clone()));
                }
            }

            Stmt::CompoundAssign {
                name, op, value, ..
            } => {
                // Desugar: Load(var) → compile(rhs) → Op → Store(var)
                if self.is_local(name) {
                    self.chunk.emit(Instruction::LoadLocal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::LoadGlobal(name.clone()));
                }
                self.compile_expr(value)?;
                let instr = match op {
                    CompoundAssignOp::Add => Instruction::Add,
                    CompoundAssignOp::Subtract => Instruction::Subtract,
                    CompoundAssignOp::Multiply => Instruction::Multiply,
                    CompoundAssignOp::Divide => Instruction::Divide,
                };
                self.chunk.emit(instr);
                if self.is_local(name) {
                    self.chunk.emit(Instruction::StoreLocal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::StoreGlobal(name.clone()));
                }
            }

            Stmt::Print { value } => {
                self.compile_expr(value)?;
                self.chunk.emit(Instruction::Print);
            }

            Stmt::Expr(expr) => {
                self.compile_expr(expr)?;
                self.chunk.emit(Instruction::Pop);
            }

            Stmt::Block(stmts) => {
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());
                for s in stmts {
                    self.compile_stmt(s)?;
                }
                self.locals_stack.pop();
                self.chunk.emit(Instruction::EndScope);
            }

            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                self.compile_expr(cond)?;
                let jump_if_false_idx = self.chunk.emit(Instruction::JumpIfFalse(0));

                self.compile_stmt(then_block)?;

                if let Some(else_blk) = else_block {
                    let jump_idx = self.chunk.emit(Instruction::Jump(0));
                    let else_start = self.chunk.instructions.len();
                    self.chunk.instructions[jump_if_false_idx] =
                        Instruction::JumpIfFalse(else_start);

                    self.compile_stmt(else_blk)?;

                    let after_else = self.chunk.instructions.len();
                    self.chunk.instructions[jump_idx] = Instruction::Jump(after_else);
                } else {
                    let after_then = self.chunk.instructions.len();
                    self.chunk.instructions[jump_if_false_idx] =
                        Instruction::JumpIfFalse(after_then);
                }
            }

            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.chunk.emit(Instruction::Nil);
                }
                self.chunk.emit(Instruction::Return);
            }

            Stmt::FnDecl {
                name, params, body, ..
            } => {
                // Register in globals before compiling body so recursive calls resolve correctly.
                self.globals.insert(name.clone());

                let fn_compiler = BytecodeCompiler::new_for_function(self.globals.clone(), params);
                let (fn_chunk, nested_fns, nested_sims) =
                    fn_compiler.compile_function_body(body)?;

                // Collect any function/simulate chunks emitted within the body.
                self.functions.extend(nested_fns);
                self.simulate_bodies.extend(nested_sims);

                self.functions.push(FunctionChunk {
                    name: name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    arity: params.len(),
                    chunk: fn_chunk,
                });

                self.chunk.emit(Instruction::LoadFunction(name.clone()));
                // Top-level functions go into global scope; nested functions into the
                // current local scope so each call site captures its own closure.
                if self.locals_stack.is_empty() {
                    self.chunk.emit(Instruction::DefineGlobal(name.clone()));
                } else {
                    self.locals_stack.last_mut().unwrap().insert(name.clone());
                    self.chunk.emit(Instruction::DefineLocal(name.clone()));
                }
            }

            Stmt::StateDecl {
                name,
                variants,
                transitions,
                ..
            } => {
                self.chunk.emit(Instruction::DefineState {
                    name: name.clone(),
                    variants: variants.iter().map(|v| v.name.clone()).collect(),
                    transitions: transitions
                        .iter()
                        .map(|t| (t.from.clone(), t.to.clone()))
                        .collect(),
                });
            }

            Stmt::Transition {
                variable, target, ..
            } => {
                self.chunk.emit(Instruction::Transition {
                    variable: variable.clone(),
                    target: target.clone(),
                });
            }

            Stmt::Simulate {
                duration,
                step,
                body,
                ..
            } => {
                // Duration and step compile inline; body goes into a separate SimulateChunk.
                self.compile_expr(duration)?;
                self.compile_expr(step)?;

                // Compile body with a child compiler that knows about globals and has
                // "time" as a pre-seeded local. Variables from enclosing block-local scopes
                // are not accessible from simulate bodies — only top-level globals are.
                let body_compiler = BytecodeCompiler::new_for_simulate(self.globals.clone());
                let (body_chunk, nested_fns, nested_sims) =
                    body_compiler.compile_simulate_body(body)?;

                // Nested functions and simulate bodies from inside the body come first.
                self.functions.extend(nested_fns);
                self.simulate_bodies.extend(nested_sims);

                // This simulate body's index is its position after adding nested bodies.
                let body_idx = self.simulate_bodies.len();
                self.simulate_bodies.push(SimulateChunk {
                    name: format!("simulate#{}", body_idx),
                    chunk: body_chunk,
                });

                self.chunk.emit(Instruction::Simulate { body_idx });
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Number(n) => {
                let idx = self.chunk.add_constant(Constant::Number(*n));
                self.chunk.emit(Instruction::Constant(idx));
            }

            Expr::Str(s) => {
                let idx = self.chunk.add_constant(Constant::Text(s.clone()));
                self.chunk.emit(Instruction::Constant(idx));
            }

            Expr::Bool(b) => {
                if *b {
                    self.chunk.emit(Instruction::True);
                } else {
                    self.chunk.emit(Instruction::False);
                }
            }

            Expr::Variable { name, .. } => {
                if self.is_local(name) {
                    self.chunk.emit(Instruction::LoadLocal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::LoadGlobal(name.clone()));
                }
            }

            Expr::Unary { op, operand } => {
                self.compile_expr(operand)?;
                match op {
                    UnaryOp::Neg => {
                        self.chunk.emit(Instruction::Negate);
                    }
                    UnaryOp::Not => {
                        self.chunk.emit(Instruction::Not);
                    }
                }
            }

            Expr::Binary { op, left, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                let instr = match op {
                    BinaryOp::Add => Instruction::Add,
                    BinaryOp::Sub => Instruction::Subtract,
                    BinaryOp::Mul => Instruction::Multiply,
                    BinaryOp::Div => Instruction::Divide,
                    BinaryOp::Eq => Instruction::Equal,
                    BinaryOp::NotEq => Instruction::NotEqual,
                    BinaryOp::Lt => Instruction::Less,
                    BinaryOp::LtEq => Instruction::LessEqual,
                    BinaryOp::Gt => Instruction::Greater,
                    BinaryOp::GtEq => Instruction::GreaterEqual,
                };
                self.chunk.emit(instr);
            }

            Expr::Grouping(inner) => {
                self.compile_expr(inner)?;
            }

            Expr::Call { callee, args, .. } => {
                // Compile callee first (pushes function value onto stack),
                // then arguments left-to-right, then emit stack-based Call.
                // This handles named calls, returned closures, and chained calls uniformly.
                self.compile_expr(callee)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.chunk.emit(Instruction::Call {
                    arg_count: args.len(),
                });
            }

            Expr::StateVariant {
                state_name,
                variant_name,
                ..
            } => {
                self.chunk.emit(Instruction::LoadState {
                    state_name: state_name.clone(),
                    variant_name: variant_name.clone(),
                });
            }
        }
        Ok(())
    }
}
