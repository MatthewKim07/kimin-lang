use crate::bytecode::{BytecodeProgram, Chunk, Constant, Instruction};

/// Returns a human-readable listing of a compiled program.
pub fn disassemble(program: &BytecodeProgram) -> String {
    disassemble_chunk(&program.chunk, "main")
}

pub fn disassemble_chunk(chunk: &Chunk, name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n", name));

    if !chunk.constants.is_empty() {
        out.push_str("constants:\n");
        for (i, c) in chunk.constants.iter().enumerate() {
            out.push_str(&format!("  {:04} {}\n", i, fmt_constant(c)));
        }
    }

    out.push_str("instructions:\n");
    for (i, instr) in chunk.instructions.iter().enumerate() {
        out.push_str(&format!("  {:04} {}\n", i, fmt_instruction(instr)));
    }

    out
}

fn fmt_constant(c: &Constant) -> String {
    match c {
        Constant::Number(n) => {
            if n.fract() == 0.0 {
                format!("Number({})", *n as i64)
            } else {
                format!("Number({})", n)
            }
        }
        Constant::Text(s) => format!("Text({:?})", s),
        Constant::Bool(b) => format!("Bool({})", b),
        Constant::Nil => "Nil".to_string(),
    }
}

fn fmt_instruction(instr: &Instruction) -> String {
    match instr {
        Instruction::Constant(i) => format!("CONSTANT #{}", i),
        Instruction::Nil => "NIL".to_string(),
        Instruction::True => "TRUE".to_string(),
        Instruction::False => "FALSE".to_string(),
        Instruction::LoadGlobal(n) => format!("LOAD_GLOBAL {}", n),
        Instruction::DefineGlobal(n) => format!("DEFINE_GLOBAL {}", n),
        Instruction::StoreGlobal(n) => format!("STORE_GLOBAL {}", n),
        Instruction::LoadLocal(n) => format!("LOAD_LOCAL {}", n),
        Instruction::DefineLocal(n) => format!("DEFINE_LOCAL {}", n),
        Instruction::StoreLocal(n) => format!("STORE_LOCAL {}", n),
        Instruction::Add => "ADD".to_string(),
        Instruction::Subtract => "SUBTRACT".to_string(),
        Instruction::Multiply => "MULTIPLY".to_string(),
        Instruction::Divide => "DIVIDE".to_string(),
        Instruction::Negate => "NEGATE".to_string(),
        Instruction::Not => "NOT".to_string(),
        Instruction::Equal => "EQUAL".to_string(),
        Instruction::NotEqual => "NOT_EQUAL".to_string(),
        Instruction::Less => "LESS".to_string(),
        Instruction::LessEqual => "LESS_EQUAL".to_string(),
        Instruction::Greater => "GREATER".to_string(),
        Instruction::GreaterEqual => "GREATER_EQUAL".to_string(),
        Instruction::Print => "PRINT".to_string(),
        Instruction::Pop => "POP".to_string(),
        Instruction::JumpIfFalse(target) => format!("JUMP_IF_FALSE @{}", target),
        Instruction::Jump(target) => format!("JUMP @{}", target),
        Instruction::BeginScope => "BEGIN_SCOPE".to_string(),
        Instruction::EndScope => "END_SCOPE".to_string(),
        Instruction::Return => "RETURN".to_string(),
        Instruction::Halt => "HALT".to_string(),
        Instruction::Unsupported(what) => format!("UNSUPPORTED({})", what),
    }
}
