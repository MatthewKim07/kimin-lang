use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::bytecode::{BytecodeProgram, Chunk, Constant, Instruction};
use crate::error::CompileError;

pub struct BytecodeCompiler {
    chunk: Chunk,
    /// Tracks block nesting depth. 0 = global scope; > 0 = local scope.
    scope_depth: usize,
}

impl BytecodeCompiler {
    pub fn new() -> Self {
        BytecodeCompiler {
            chunk: Chunk::new(),
            scope_depth: 0,
        }
    }

    /// Lowers a parsed program to a flat bytecode chunk.
    pub fn compile(mut self, stmts: &[Stmt]) -> Result<BytecodeProgram, CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        self.chunk.emit(Instruction::Halt);
        Ok(BytecodeProgram::new(self.chunk))
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                self.compile_expr(value)?;
                if self.scope_depth == 0 {
                    self.chunk.emit(Instruction::DefineGlobal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::DefineLocal(name.clone()));
                }
            }

            Stmt::Assign { name, value, .. } => {
                self.compile_expr(value)?;
                if self.scope_depth == 0 {
                    self.chunk.emit(Instruction::StoreGlobal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::StoreLocal(name.clone()));
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
                self.scope_depth += 1;
                for s in stmts {
                    self.compile_stmt(s)?;
                }
                self.scope_depth -= 1;
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

            Stmt::FnDecl { name, .. } => {
                self.chunk
                    .emit(Instruction::Unsupported(format!("fn {}", name)));
            }

            Stmt::StateDecl { name, .. } => {
                self.chunk
                    .emit(Instruction::Unsupported(format!("state {}", name)));
            }

            Stmt::Transition {
                variable, target, ..
            } => {
                self.chunk.emit(Instruction::Unsupported(format!(
                    "transition {} -> {}",
                    variable, target
                )));
            }

            Stmt::Simulate { .. } => {
                self.chunk
                    .emit(Instruction::Unsupported("simulate".to_string()));
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
                if self.scope_depth == 0 {
                    self.chunk.emit(Instruction::LoadGlobal(name.clone()));
                } else {
                    self.chunk.emit(Instruction::LoadLocal(name.clone()));
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

            Expr::Call { callee, .. } => {
                let name = match callee.as_ref() {
                    Expr::Variable { name, .. } => name.clone(),
                    _ => "?".to_string(),
                };
                self.chunk
                    .emit(Instruction::Unsupported(format!("call {}", name)));
            }

            Expr::StateVariant {
                state_name,
                variant_name,
                ..
            } => {
                self.chunk.emit(Instruction::Unsupported(format!(
                    "{}.{}",
                    state_name, variant_name
                )));
            }
        }
        Ok(())
    }
}
