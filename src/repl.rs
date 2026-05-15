use std::io::{self, Write};

use crate::error::KiminError;
use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typechecker::TypeChecker;

pub fn run_repl() {
    let mut interp = Interpreter::new();
    let mut tc = TypeChecker::new();
    println!("Kimin REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'exit' or press Ctrl-C to quit.\n");

    loop {
        print!(">>> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }

        let input = line.trim();
        if input == "exit" || input == "quit" {
            break;
        }
        if input.is_empty() {
            continue;
        }

        if let Err(e) = exec_line(input, &mut interp, &mut tc) {
            eprintln!("{}", e);
        }
    }
}

fn exec_line(
    source: &str,
    interp: &mut Interpreter,
    tc: &mut TypeChecker,
) -> Result<(), KiminError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    tc.check(&stmts)?;
    interp.run(&stmts)?;
    Ok(())
}
