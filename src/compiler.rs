use std::collections::HashSet;

use crate::ast::{AssignTarget, BinaryOp, CompoundAssignOp, Expr, Param, PathStep, Stmt, UnaryOp};
use crate::bytecode::{
    BytecodeProgram, Chunk, Constant, FunctionChunk, Instruction, MethodChunk, SimulateChunk,
};
use crate::error::CompileError;

/// Tracks context for the nearest enclosing while loop during bytecode compilation.
/// Used to patch break/continue Jump placeholders and to compute scope-unwind depth.
struct LoopContext {
    /// Indices of placeholder Jump instructions emitted by `break` statements.
    break_jumps: Vec<usize>,
    /// Indices of placeholder Jump instructions emitted by `continue` statements.
    continue_jumps: Vec<usize>,
    /// `locals_stack.len()` at the point BEFORE the while body's BeginScope was emitted.
    /// Used to compute how many EndScope instructions to emit before a break/continue jump.
    scope_depth_before_body: usize,
}

pub struct BytecodeCompiler {
    chunk: Chunk,
    /// Names defined at global scope (depth 0). Used to correctly classify variable
    /// references inside blocks as GLOBAL or LOCAL regardless of current scope depth.
    globals: HashSet<String>,
    /// Names of declared state machine types. Used to distinguish state variant access
    /// (`Door.closed` → LoadState) from struct field access (`u.name` → Load + FieldAccess).
    state_types: HashSet<String>,
    /// Stack of locally-defined name sets, one entry per active block scope. The
    /// innermost scope is at the end. A name present in any layer here is LOCAL; a name
    /// in `globals` that is absent from all layers is GLOBAL.
    locals_stack: Vec<HashSet<String>>,
    /// Function chunks collected during compilation, in source order.
    functions: Vec<FunctionChunk>,
    /// Simulate body chunks collected during compilation, in source order.
    simulate_bodies: Vec<SimulateChunk>,
    /// Method chunks collected during compilation, keyed by (struct_name, method_name).
    methods: Vec<MethodChunk>,
    /// Stack of enclosing while-loop contexts, used to patch break/continue jumps.
    loop_stack: Vec<LoopContext>,
}

impl BytecodeCompiler {
    pub fn new() -> Self {
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals: HashSet::new(),
            state_types: HashSet::new(),
            locals_stack: Vec::new(),
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
            methods: Vec::new(),
            loop_stack: Vec::new(),
        }
    }

    /// Creates a compiler seeded for a function body. Parameters are pre-loaded as
    /// the innermost local scope so they resolve to LoadLocal inside the body.
    fn new_for_function(
        globals: HashSet<String>,
        state_types: HashSet<String>,
        params: &[Param],
    ) -> Self {
        let param_scope: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals,
            state_types,
            locals_stack: vec![param_scope],
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
            methods: Vec::new(),
            loop_stack: Vec::new(),
        }
    }

    fn new_for_simulate(globals: HashSet<String>, state_types: HashSet<String>) -> Self {
        let time_scope: HashSet<String> = ["time".to_string()].into_iter().collect();
        BytecodeCompiler {
            chunk: Chunk::new(),
            globals,
            state_types,
            locals_stack: vec![time_scope],
            functions: Vec::new(),
            simulate_bodies: Vec::new(),
            methods: Vec::new(),
            loop_stack: Vec::new(),
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
            self.methods,
        ))
    }

    /// Compiles a function body (no HALT). Appends NIL + RETURN if body does not
    /// already end with an explicit RETURN.
    fn compile_function_body(
        mut self,
        stmts: &[Stmt],
    ) -> Result<
        (
            Chunk,
            Vec<FunctionChunk>,
            Vec<SimulateChunk>,
            Vec<MethodChunk>,
        ),
        CompileError,
    > {
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
        Ok((
            self.chunk,
            self.functions,
            self.simulate_bodies,
            self.methods,
        ))
    }

    fn compile_simulate_body(
        mut self,
        stmts: &[Stmt],
    ) -> Result<
        (
            Chunk,
            Vec<FunctionChunk>,
            Vec<SimulateChunk>,
            Vec<MethodChunk>,
        ),
        CompileError,
    > {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        Ok((
            self.chunk,
            self.functions,
            self.simulate_bodies,
            self.methods,
        ))
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

            Stmt::IndexAssign {
                name, index, value, ..
            } => {
                // Stack layout before SetIndex: [..., index_value, new_value]
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.chunk.emit(Instruction::SetIndex(name.clone()));
            }

            Stmt::IndexCompoundAssign {
                name,
                index,
                op,
                value,
                ..
            } => {
                // Stack layout before IndexCompoundAssign: [..., index_value, rhs_value]
                // Index evaluated once here; VM reads old element internally.
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.chunk.emit(Instruction::IndexCompoundAssign {
                    name: name.clone(),
                    op: op.clone(),
                });
            }

            Stmt::TargetAssign { target, value, .. } => {
                // Compile index expressions left-to-right, then RHS, then SetPath.
                let (root, steps, index_exprs) = compiler_flatten_target(target);
                for expr in &index_exprs {
                    self.compile_expr(expr)?;
                }
                self.compile_expr(value)?;
                self.chunk.emit(Instruction::SetPath { root, steps });
            }

            Stmt::TargetCompoundAssign {
                target, op, value, ..
            } => {
                // Compile index expressions left-to-right, then RHS, then PathCompoundAssign.
                let (root, steps, index_exprs) = compiler_flatten_target(target);
                for expr in &index_exprs {
                    self.compile_expr(expr)?;
                }
                self.compile_expr(value)?;
                self.chunk.emit(Instruction::PathCompoundAssign {
                    root,
                    steps,
                    op: op.clone(),
                });
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

                let fn_compiler = BytecodeCompiler::new_for_function(
                    self.globals.clone(),
                    self.state_types.clone(),
                    params,
                );
                let (fn_chunk, nested_fns, nested_sims, nested_methods) =
                    fn_compiler.compile_function_body(body)?;

                self.functions.extend(nested_fns);
                self.simulate_bodies.extend(nested_sims);
                self.methods.extend(nested_methods);

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
                self.state_types.insert(name.clone());
                self.chunk.emit(Instruction::DefineState {
                    name: name.clone(),
                    variants: variants.iter().map(|v| v.name.clone()).collect(),
                    transitions: transitions
                        .iter()
                        .map(|t| (t.from.clone(), t.to.clone()))
                        .collect(),
                });
            }

            Stmt::StructDecl { .. } => {
                // Struct declarations are purely static — no bytecode needed.
            }

            Stmt::ImplBlock {
                struct_name,
                methods: method_stmts,
                ..
            } => {
                // Compile each method body into a MethodChunk; no runtime code emitted.
                for method_stmt in method_stmts {
                    if let Stmt::FnDecl {
                        name, params, body, ..
                    } = method_stmt
                    {
                        let method_compiler = BytecodeCompiler::new_for_function(
                            self.globals.clone(),
                            self.state_types.clone(),
                            params,
                        );
                        let (chunk, nested_fns, nested_sims, nested_methods) =
                            method_compiler.compile_function_body(body)?;
                        self.functions.extend(nested_fns);
                        self.simulate_bodies.extend(nested_sims);
                        self.methods.extend(nested_methods);
                        self.methods.push(MethodChunk {
                            struct_name: struct_name.clone(),
                            method_name: name.clone(),
                            params: params.iter().map(|p| p.name.clone()).collect(),
                            arity: params.len(),
                            chunk,
                        });
                    }
                }
            }

            Stmt::Transition {
                variable, target, ..
            } => {
                self.chunk.emit(Instruction::Transition {
                    variable: variable.clone(),
                    target: target.clone(),
                });
            }

            Stmt::While {
                condition, body, ..
            } => {
                // Layout:
                //   @loop_start: <condition>
                //                JumpIfFalse @loop_end
                //                BeginScope
                //                <body>           ← break emits EndScope(s)+Jump(@loop_end)
                //                                 ← continue emits EndScope(s)+Jump(@loop_start)
                //                EndScope
                //                Jump @loop_start
                //   @loop_end:
                let loop_start = self.chunk.instructions.len();
                self.compile_expr(condition)?;
                let jump_if_false_idx = self.chunk.emit(Instruction::JumpIfFalse(0));

                // Record scope depth BEFORE body BeginScope so break/continue know
                // how many EndScope instructions to emit to fully unwind to loop boundary.
                let scope_depth_before_body = self.locals_stack.len();
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.loop_stack.push(LoopContext {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                    scope_depth_before_body,
                });

                for s in body {
                    self.compile_stmt(s)?;
                }

                let ctx = self.loop_stack.pop().expect("loop_stack underflow");
                self.locals_stack.pop();
                self.chunk.emit(Instruction::EndScope);

                self.chunk.emit(Instruction::Jump(loop_start));

                let loop_end = self.chunk.instructions.len();
                self.chunk.instructions[jump_if_false_idx] = Instruction::JumpIfFalse(loop_end);

                // Patch all break jumps to point to loop_end (past the EndScope+Jump).
                for idx in ctx.break_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(loop_end);
                }
                // Patch all continue jumps to point to loop_start (re-evaluate condition).
                for idx in ctx.continue_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(loop_start);
                }
            }

            Stmt::Break { .. } => {
                // Emit EndScope for every scope opened inside (and including) the while body.
                // Then emit a Jump placeholder that will be patched to loop_end.
                let scope_depth_before_body = {
                    let ctx = self
                        .loop_stack
                        .last()
                        .expect("break outside loop (should have been caught by typechecker)");
                    ctx.scope_depth_before_body
                };
                let scopes_to_close = self.locals_stack.len() - scope_depth_before_body;
                for _ in 0..scopes_to_close {
                    self.chunk.emit(Instruction::EndScope);
                }
                let jump_idx = self.chunk.emit(Instruction::Jump(0)); // patched later
                self.loop_stack
                    .last_mut()
                    .expect("loop_stack empty")
                    .break_jumps
                    .push(jump_idx);
            }

            Stmt::Continue { .. } => {
                // Emit EndScope for every scope opened inside (and including) the while body.
                // Then emit a Jump placeholder that will be patched to loop_start.
                let scope_depth_before_body = {
                    let ctx = self
                        .loop_stack
                        .last()
                        .expect("continue outside loop (should have been caught by typechecker)");
                    ctx.scope_depth_before_body
                };
                let scopes_to_close = self.locals_stack.len() - scope_depth_before_body;
                for _ in 0..scopes_to_close {
                    self.chunk.emit(Instruction::EndScope);
                }
                let jump_idx = self.chunk.emit(Instruction::Jump(0)); // patched later
                self.loop_stack
                    .last_mut()
                    .expect("loop_stack empty")
                    .continue_jumps
                    .push(jump_idx);
            }

            Stmt::ForRange {
                var_name,
                start,
                end,
                body,
                ..
            } => {
                // Lowering layout:
                //   BEGIN_SCOPE (outer for scope — holds loop var and end sentinel)
                //     <start>
                //     DEFINE_LOCAL i
                //     <end>
                //     DEFINE_LOCAL __kimin_range_end_N
                //   @loop_start:
                //     LOAD_LOCAL i
                //     LOAD_LOCAL __kimin_range_end_N
                //     LESS
                //     JUMP_IF_FALSE @loop_end
                //     BEGIN_SCOPE (body)
                //       <body>          ← break → EndScope(s) + Jump(@loop_end)
                //                       ← continue → EndScope(s) + Jump(@increment)
                //     END_SCOPE (body)
                //   @increment:
                //     LOAD_LOCAL i
                //     CONSTANT 1
                //     ADD
                //     STORE_LOCAL i
                //     JUMP @loop_start
                //   @loop_end:
                //   END_SCOPE (outer)
                //
                // Note: @loop_end points to END_SCOPE(outer), so break correctly
                // closes both the body scope (via its own EndScopes) and the outer scope.

                // Use a counter to produce unique sentinel names even for nested for loops.
                let sentinel_name = format!("__kimin_range_end_{}", self.locals_stack.len());

                // Outer for scope.
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                // Define loop variable.
                self.compile_expr(start)?;
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(var_name.clone());
                self.chunk.emit(Instruction::DefineLocal(var_name.clone()));

                // Define end sentinel.
                self.compile_expr(end)?;
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(sentinel_name.clone());
                self.chunk
                    .emit(Instruction::DefineLocal(sentinel_name.clone()));

                // Condition check.
                let loop_start = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(var_name.clone()));
                self.chunk
                    .emit(Instruction::LoadLocal(sentinel_name.clone()));
                self.chunk.emit(Instruction::Less);
                let jump_if_false_idx = self.chunk.emit(Instruction::JumpIfFalse(0));

                // Body scope. Record scope_depth_before_body BEFORE body's BeginScope.
                let scope_depth_before_body = self.locals_stack.len();
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.loop_stack.push(LoopContext {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                    scope_depth_before_body,
                });

                for s in body {
                    self.compile_stmt(s)?;
                }

                let ctx = self.loop_stack.pop().expect("loop_stack underflow");
                self.locals_stack.pop();
                self.chunk.emit(Instruction::EndScope); // close body scope (normal exit)

                // Increment target — continue jumps here.
                let increment_target = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(var_name.clone()));
                let one_idx = self.chunk.add_constant(Constant::Number(1.0));
                self.chunk.emit(Instruction::Constant(one_idx));
                self.chunk.emit(Instruction::Add);
                self.chunk.emit(Instruction::StoreLocal(var_name.clone()));
                self.chunk.emit(Instruction::Jump(loop_start));

                // loop_end points to the outer END_SCOPE so break lands there and
                // the outer scope is closed by the normal END_SCOPE instruction.
                let loop_end = self.chunk.instructions.len();
                self.chunk.emit(Instruction::EndScope); // close outer for scope

                // Patch JumpIfFalse to loop_end.
                self.chunk.instructions[jump_if_false_idx] = Instruction::JumpIfFalse(loop_end);

                // Patch break jumps to loop_end.
                for idx in ctx.break_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(loop_end);
                }
                // Patch continue jumps to increment_target (not loop_start).
                for idx in ctx.continue_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(increment_target);
                }

                // Pop the outer for scope from locals_stack.
                self.locals_stack.pop();
            }

            Stmt::ForEach {
                var_name,
                iterable,
                body,
                ..
            } => {
                // Lowering layout:
                //   BEGIN_SCOPE (outer — holds array snapshot and index counter)
                //     <iterable>
                //     DEFINE_LOCAL __kimin_foreach_iter_N
                //     CONSTANT 0
                //     DEFINE_LOCAL __kimin_foreach_idx_N
                //   @loop_start:
                //     LOAD_LOCAL __kimin_foreach_idx_N
                //     LOAD_LOCAL __kimin_foreach_iter_N
                //     LEN
                //     LESS
                //     JUMP_IF_FALSE @loop_end
                //     BEGIN_SCOPE (body — holds loop variable)
                //       LOAD_LOCAL __kimin_foreach_iter_N
                //       LOAD_LOCAL __kimin_foreach_idx_N
                //       INDEX
                //       DEFINE_LOCAL var_name
                //       <body>  ← break → EndScope(s) + Jump(@loop_end)
                //               ← continue → EndScope(s) + Jump(@increment)
                //     END_SCOPE (body)
                //   @increment:
                //     LOAD_LOCAL __kimin_foreach_idx_N
                //     CONSTANT 1
                //     ADD
                //     STORE_LOCAL __kimin_foreach_idx_N
                //     JUMP @loop_start
                //   @loop_end:
                //   END_SCOPE (outer)

                let sentinel_n = self.locals_stack.len();
                let iter_name = format!("__kimin_foreach_iter_{}", sentinel_n);
                let idx_name = format!("__kimin_foreach_idx_{}", sentinel_n);

                // Outer scope: array snapshot + index counter.
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.compile_expr(iterable)?;
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(iter_name.clone());
                self.chunk.emit(Instruction::DefineLocal(iter_name.clone()));

                let zero_idx = self.chunk.add_constant(Constant::Number(0.0));
                self.chunk.emit(Instruction::Constant(zero_idx));
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(idx_name.clone());
                self.chunk.emit(Instruction::DefineLocal(idx_name.clone()));

                // Condition: idx < len(iter).
                let loop_start = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                self.chunk.emit(Instruction::LoadLocal(iter_name.clone()));
                self.chunk.emit(Instruction::Len);
                self.chunk.emit(Instruction::Less);
                let jump_if_false_idx = self.chunk.emit(Instruction::JumpIfFalse(0));

                // Body scope: loop variable.
                let scope_depth_before_body = self.locals_stack.len();
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.loop_stack.push(LoopContext {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                    scope_depth_before_body,
                });

                // Load iter[idx] and define as the loop variable.
                self.chunk.emit(Instruction::LoadLocal(iter_name.clone()));
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                self.chunk.emit(Instruction::Index);
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(var_name.clone());
                self.chunk.emit(Instruction::DefineLocal(var_name.clone()));

                for s in body {
                    self.compile_stmt(s)?;
                }

                let ctx = self.loop_stack.pop().expect("loop_stack underflow");
                self.locals_stack.pop();
                self.chunk.emit(Instruction::EndScope); // close body scope (normal exit)

                // @increment — continue jumps here.
                let increment_target = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                let one_idx = self.chunk.add_constant(Constant::Number(1.0));
                self.chunk.emit(Instruction::Constant(one_idx));
                self.chunk.emit(Instruction::Add);
                self.chunk.emit(Instruction::StoreLocal(idx_name.clone()));
                self.chunk.emit(Instruction::Jump(loop_start));

                // @loop_end — break jumps here; outer scope closed.
                let loop_end = self.chunk.instructions.len();
                self.chunk.emit(Instruction::EndScope); // close outer scope

                self.chunk.instructions[jump_if_false_idx] = Instruction::JumpIfFalse(loop_end);
                for idx in ctx.break_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(loop_end);
                }
                for idx in ctx.continue_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(increment_target);
                }

                self.locals_stack.pop();
            }

            Stmt::ForEachIndexed {
                index_name,
                var_name,
                iterable,
                body,
                ..
            } => {
                // Same layout as ForEach but body scope also defines index_name.
                //   BEGIN_SCOPE (outer — holds array snapshot and index counter)
                //     <iterable>
                //     DEFINE_LOCAL __kimin_foreach_iter_N
                //     CONSTANT 0
                //     DEFINE_LOCAL __kimin_foreach_idx_N
                //   @loop_start:
                //     LOAD_LOCAL __kimin_foreach_idx_N
                //     LOAD_LOCAL __kimin_foreach_iter_N
                //     LEN
                //     LESS
                //     JUMP_IF_FALSE @loop_end
                //     BEGIN_SCOPE (body — holds index_name and var_name)
                //       LOAD_LOCAL __kimin_foreach_idx_N
                //       DEFINE_LOCAL index_name
                //       LOAD_LOCAL __kimin_foreach_iter_N
                //       LOAD_LOCAL __kimin_foreach_idx_N
                //       INDEX
                //       DEFINE_LOCAL var_name
                //       <body>
                //     END_SCOPE (body)
                //   @increment:
                //     LOAD_LOCAL __kimin_foreach_idx_N
                //     CONSTANT 1
                //     ADD
                //     STORE_LOCAL __kimin_foreach_idx_N
                //     JUMP @loop_start
                //   @loop_end:
                //   END_SCOPE (outer)

                let sentinel_n = self.locals_stack.len();
                let iter_name = format!("__kimin_foreach_iter_{}", sentinel_n);
                let idx_name = format!("__kimin_foreach_idx_{}", sentinel_n);

                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.compile_expr(iterable)?;
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(iter_name.clone());
                self.chunk.emit(Instruction::DefineLocal(iter_name.clone()));

                let zero_idx = self.chunk.add_constant(Constant::Number(0.0));
                self.chunk.emit(Instruction::Constant(zero_idx));
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(idx_name.clone());
                self.chunk.emit(Instruction::DefineLocal(idx_name.clone()));

                let loop_start = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                self.chunk.emit(Instruction::LoadLocal(iter_name.clone()));
                self.chunk.emit(Instruction::Len);
                self.chunk.emit(Instruction::Less);
                let jump_if_false_idx = self.chunk.emit(Instruction::JumpIfFalse(0));

                let scope_depth_before_body = self.locals_stack.len();
                self.chunk.emit(Instruction::BeginScope);
                self.locals_stack.push(HashSet::new());

                self.loop_stack.push(LoopContext {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                    scope_depth_before_body,
                });

                // Define index variable (the 0-based counter).
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(index_name.clone());
                self.chunk
                    .emit(Instruction::DefineLocal(index_name.clone()));

                // Define element variable.
                self.chunk.emit(Instruction::LoadLocal(iter_name.clone()));
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                self.chunk.emit(Instruction::Index);
                self.locals_stack
                    .last_mut()
                    .unwrap()
                    .insert(var_name.clone());
                self.chunk.emit(Instruction::DefineLocal(var_name.clone()));

                for s in body {
                    self.compile_stmt(s)?;
                }

                let ctx = self.loop_stack.pop().expect("loop_stack underflow");
                self.locals_stack.pop();
                self.chunk.emit(Instruction::EndScope);

                let increment_target = self.chunk.instructions.len();
                self.chunk.emit(Instruction::LoadLocal(idx_name.clone()));
                let one_idx = self.chunk.add_constant(Constant::Number(1.0));
                self.chunk.emit(Instruction::Constant(one_idx));
                self.chunk.emit(Instruction::Add);
                self.chunk.emit(Instruction::StoreLocal(idx_name.clone()));
                self.chunk.emit(Instruction::Jump(loop_start));

                let loop_end = self.chunk.instructions.len();
                self.chunk.emit(Instruction::EndScope);

                self.chunk.instructions[jump_if_false_idx] = Instruction::JumpIfFalse(loop_end);
                for idx in ctx.break_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(loop_end);
                }
                for idx in ctx.continue_jumps {
                    self.chunk.instructions[idx] = Instruction::Jump(increment_target);
                }

                self.locals_stack.pop();
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
                let body_compiler = BytecodeCompiler::new_for_simulate(
                    self.globals.clone(),
                    self.state_types.clone(),
                );
                let (body_chunk, nested_fns, nested_sims, nested_methods) =
                    body_compiler.compile_simulate_body(body)?;

                self.functions.extend(nested_fns);
                self.simulate_bodies.extend(nested_sims);
                self.methods.extend(nested_methods);

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
                // Intercept builtins: len, push, pop.
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if name == "len" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::Len);
                        return Ok(());
                    }
                    if name == "push" && args.len() == 2 {
                        if let Expr::Variable { name: arr_name, .. } = &args[0] {
                            self.compile_expr(&args[1])?;
                            self.chunk.emit(Instruction::ArrayPush(arr_name.clone()));
                            return Ok(());
                        }
                    }
                    if name == "pop" && args.len() == 1 {
                        if let Expr::Variable { name: arr_name, .. } = &args[0] {
                            self.chunk.emit(Instruction::ArrayPop(arr_name.clone()));
                            return Ok(());
                        }
                    }
                    if name == "contains" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::Contains);
                        return Ok(());
                    }
                    if name == "starts_with" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::StartsWith);
                        return Ok(());
                    }
                    if name == "ends_with" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::EndsWith);
                        return Ok(());
                    }
                    if name == "to_upper" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::ToUpper);
                        return Ok(());
                    }
                    if name == "to_lower" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::ToLower);
                        return Ok(());
                    }
                    if name == "trim" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::Trim);
                        return Ok(());
                    }
                    if name == "split" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::Split);
                        return Ok(());
                    }
                    if name == "join" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::Join);
                        return Ok(());
                    }
                    if name == "has_key" && args.len() == 2 {
                        self.compile_expr(&args[0])?;
                        self.compile_expr(&args[1])?;
                        self.chunk.emit(Instruction::HasKey);
                        return Ok(());
                    }
                    if name == "keys" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::Keys);
                        return Ok(());
                    }
                    if name == "values" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::Values);
                        return Ok(());
                    }
                    if name == "remove" && args.len() == 2 {
                        if let Expr::Variable { name: var_name, .. } = &args[0] {
                            let var_name = var_name.clone();
                            self.compile_expr(&args[1])?;
                            self.chunk.emit(Instruction::RemoveKey(var_name));
                            return Ok(());
                        }
                    }
                    if name == "to_string" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::ToString);
                        return Ok(());
                    }
                    if name == "to_number" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::ToNumber);
                        return Ok(());
                    }
                    if name == "to_bool" && args.len() == 1 {
                        self.compile_expr(&args[0])?;
                        self.chunk.emit(Instruction::ToBool);
                        return Ok(());
                    }
                }
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

            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.compile_expr(elem)?;
                }
                self.chunk.emit(Instruction::Array {
                    count: elements.len(),
                });
            }

            Expr::MapLiteral { entries, .. } => {
                let count = entries.len();
                for (key, val) in entries {
                    self.compile_expr(key)?;
                    self.compile_expr(val)?;
                }
                self.chunk.emit(Instruction::Map { count });
            }

            Expr::Index { array, index, .. } => {
                self.compile_expr(array)?;
                self.compile_expr(index)?;
                self.chunk.emit(Instruction::Index);
            }

            Expr::Slice {
                array, start, end, ..
            } => {
                self.compile_expr(array)?;
                self.compile_expr(start)?;
                self.compile_expr(end)?;
                self.chunk.emit(Instruction::Slice);
            }

            Expr::StateVariant {
                state_name,
                variant_name,
                ..
            } => {
                if self.state_types.contains(state_name) {
                    // State machine variant literal (e.g. `Door.closed`).
                    self.chunk.emit(Instruction::LoadState {
                        state_name: state_name.clone(),
                        variant_name: variant_name.clone(),
                    });
                } else {
                    // Struct field access (e.g. `u.name`).
                    if self.is_local(state_name) {
                        self.chunk.emit(Instruction::LoadLocal(state_name.clone()));
                    } else {
                        self.chunk.emit(Instruction::LoadGlobal(state_name.clone()));
                    }
                    self.chunk
                        .emit(Instruction::FieldAccess(variant_name.clone()));
                }
            }

            Expr::StructLiteral { name, fields, .. } => {
                // Compile field values in source order; VM pops in LIFO then reverses.
                for (_, field_expr) in fields {
                    self.compile_expr(field_expr)?;
                }
                let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                self.chunk.emit(Instruction::StructLiteral {
                    name: name.clone(),
                    fields: field_names,
                });
            }

            Expr::FieldAccess { object, field, .. } => {
                self.compile_expr(object)?;
                self.chunk.emit(Instruction::FieldAccess(field.clone()));
            }

            Expr::MethodCall {
                object,
                method,
                args,
                ..
            } => {
                // Stack before CallMethod: [..., receiver, arg1, ..., argN]
                self.compile_expr(object)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.chunk.emit(Instruction::CallMethod {
                    method: method.clone(),
                    arg_count: args.len(),
                });
            }
        }
        Ok(())
    }
}

/// Decompose an AssignTarget into (root_name, bytecode_steps, index_exprs_in_source_order).
/// Index expressions must be compiled left-to-right before the RHS.
fn compiler_flatten_target(target: &AssignTarget) -> (String, Vec<PathStep>, Vec<Expr>) {
    match target {
        AssignTarget::Var(name) => (name.clone(), vec![], vec![]),
        AssignTarget::Field(inner, field) => {
            let (root, mut steps, exprs) = compiler_flatten_target(inner);
            steps.push(PathStep::Field(field.clone()));
            (root, steps, exprs)
        }
        AssignTarget::Index(inner, expr) => {
            let (root, mut steps, mut exprs) = compiler_flatten_target(inner);
            steps.push(PathStep::Index);
            exprs.push(expr.clone());
            (root, steps, exprs)
        }
    }
}
