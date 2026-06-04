use crate::bytecode::{BytecodeProgram, Chunk, Constant, Instruction};

/// Returns a human-readable listing of a compiled program: main chunk followed by
/// each function chunk then each simulate body chunk in source order.
pub fn disassemble(program: &BytecodeProgram) -> String {
    let mut out = disassemble_chunk(&program.main, "main");
    for fc in &program.functions {
        out.push('\n');
        let header = format!("function {}/{}", fc.name, fc.arity);
        let mut section = disassemble_chunk(&fc.chunk, &header);
        // Inject params line after the section header.
        if !fc.params.is_empty() {
            let params_line = format!("params: {}\n", fc.params.join(", "));
            // Insert after the first line (the "=== ... ===" header).
            if let Some(nl) = section.find('\n') {
                section.insert_str(nl + 1, &params_line);
            }
        }
        out.push_str(&section);
    }
    for sc in &program.simulate_bodies {
        out.push('\n');
        let mut section = disassemble_chunk(&sc.chunk, &format!("simulate {}", sc.name));
        let params_line = "params: time\n";
        if let Some(nl) = section.find('\n') {
            section.insert_str(nl + 1, params_line);
        }
        out.push_str(&section);
    }
    for mc in &program.methods {
        out.push('\n');
        let header = format!("method {}::{}/{}", mc.struct_name, mc.method_name, mc.arity);
        let mut section = disassemble_chunk(&mc.chunk, &header);
        if !mc.params.is_empty() {
            let params_line = format!("params: {}\n", mc.params.join(", "));
            if let Some(nl) = section.find('\n') {
                section.insert_str(nl + 1, &params_line);
            }
        }
        out.push_str(&section);
    }
    out
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

fn fmt_path_steps(root: &str, steps: &[crate::ast::PathStep]) -> String {
    let mut out = root.to_string();
    for step in steps {
        match step {
            crate::ast::PathStep::Field(f) => out.push_str(&format!(".{}", f)),
            crate::ast::PathStep::Index => out.push_str("[?]"),
        }
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

pub fn disassemble_instruction(instr: &Instruction) -> String {
    fmt_instruction(instr)
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
        Instruction::LoadFunction(n) => format!("LOAD_FUNCTION {}", n),
        Instruction::Call { arg_count } => format!("CALL {}", arg_count),
        Instruction::Return => "RETURN".to_string(),
        Instruction::Halt => "HALT".to_string(),
        Instruction::DefineState {
            name,
            variants,
            transitions,
        } => {
            let vlist = variants.join(", ");
            let tlist = transitions
                .iter()
                .map(|(f, t)| format!("{}->{}", f, t))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "DEFINE_STATE {} variants=[{}] transitions=[{}]",
                name, vlist, tlist
            )
        }
        Instruction::LoadState {
            state_name,
            variant_name,
        } => format!("LOAD_STATE {}.{}", state_name, variant_name),
        Instruction::Transition { variable, target } => {
            format!("TRANSITION {} -> {}", variable, target)
        }
        Instruction::Simulate { body_idx } => format!("SIMULATE #{}", body_idx),
        Instruction::Array { count } => format!("ARRAY {}", count),
        Instruction::Map { count } => format!("MAP {}", count),
        Instruction::Index => "INDEX".to_string(),
        Instruction::Len => "LEN".to_string(),
        Instruction::Slice => "SLICE".to_string(),
        Instruction::ArrayPush(n) => format!("ARRAY_PUSH {}", n),
        Instruction::ArrayPop(n) => format!("ARRAY_POP {}", n),
        Instruction::SetIndex(n) => format!("SET_INDEX {}", n),
        Instruction::IndexCompoundAssign { name, op } => {
            use crate::ast::CompoundAssignOp;
            let op_str = match op {
                CompoundAssignOp::Add => "+=",
                CompoundAssignOp::Subtract => "-=",
                CompoundAssignOp::Multiply => "*=",
                CompoundAssignOp::Divide => "/=",
            };
            format!("INDEX_COMPOUND_ASSIGN {} {}", name, op_str)
        }
        Instruction::Contains => "CONTAINS".to_string(),
        Instruction::StartsWith => "STARTS_WITH".to_string(),
        Instruction::EndsWith => "ENDS_WITH".to_string(),
        Instruction::ToUpper => "TO_UPPER".to_string(),
        Instruction::ToLower => "TO_LOWER".to_string(),
        Instruction::Trim => "TRIM".to_string(),
        Instruction::ToString => "TO_STRING".to_string(),
        Instruction::ToNumber => "TO_NUMBER".to_string(),
        Instruction::ToBool => "TO_BOOL".to_string(),
        Instruction::Ln => "LN".to_string(),
        Instruction::Log2 => "LOG2".to_string(),
        Instruction::Log10 => "LOG10".to_string(),
        Instruction::Exp => "EXP".to_string(),
        Instruction::Sqrt => "SQRT".to_string(),
        Instruction::Pow => "POW".to_string(),
        Instruction::Abs => "ABS".to_string(),
        Instruction::Floor => "FLOOR".to_string(),
        Instruction::Ceil => "CEIL".to_string(),
        Instruction::Round => "ROUND".to_string(),
        Instruction::Min => "MIN".to_string(),
        Instruction::Max => "MAX".to_string(),
        Instruction::Sin => "SIN".to_string(),
        Instruction::Cos => "COS".to_string(),
        Instruction::Tan => "TAN".to_string(),
        Instruction::Split => "SPLIT".to_string(),
        Instruction::Join => "JOIN".to_string(),
        Instruction::HasKey => "HAS_KEY".to_string(),
        Instruction::Keys => "KEYS".to_string(),
        Instruction::Values => "VALUES".to_string(),
        Instruction::RemoveKey(name) => format!("REMOVE_KEY {}", name),
        Instruction::StructLiteral { name, fields } => {
            format!("STRUCT_LITERAL {} fields=[{}]", name, fields.join(", "))
        }
        Instruction::FieldAccess(field) => format!("FIELD_ACCESS {}", field),
        Instruction::SetPath { root, steps } => {
            let path = fmt_path_steps(root, steps);
            format!("SET_PATH {}", path)
        }
        Instruction::PathCompoundAssign { root, steps, op } => {
            use crate::ast::CompoundAssignOp;
            let path = fmt_path_steps(root, steps);
            let op_str = match op {
                CompoundAssignOp::Add => "+=",
                CompoundAssignOp::Subtract => "-=",
                CompoundAssignOp::Multiply => "*=",
                CompoundAssignOp::Divide => "/=",
            };
            format!("PATH_COMPOUND_ASSIGN {} {}", path, op_str)
        }
        Instruction::CallMethod { method, arg_count } => {
            format!("CALL_METHOD {} {}", method, arg_count)
        }
        Instruction::Unsupported(what) => format!("UNSUPPORTED({})", what),
    }
}
