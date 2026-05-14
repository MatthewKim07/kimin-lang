use std::io::{self, Write};

use crate::error::KiminError;
use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;

pub fn run_repl() {
    let mut interp = Interpreter::new();
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

        if let Err(e) = exec_line(input, &mut interp) {
            eprintln!("{}", e);
        }
    }
}

fn exec_line(source: &str, interp: &mut Interpreter) -> Result<(), KiminError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    interp.run(&stmts)?;
    Ok(())
}
