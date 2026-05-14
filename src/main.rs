use std::fs;
use std::process;

use clap::{Parser, Subcommand};

use forge::{
    error::ForgeError, interpreter::Interpreter, lexer::Lexer, parser::Parser as ForgeParser, repl,
};

#[derive(Parser)]
#[command(name = "forge", version, about = "The Forge programming language")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute a .forge source file
    Run {
        /// Path to the .forge file
        file: String,
    },
    /// Check syntax of a .forge file without executing it
    Check {
        /// Path to the .forge file
        file: String,
    },
    /// Start the interactive REPL
    Repl,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { file } => {
            let source = read_file(&file);
            if let Err(e) = run_source(&source) {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
        Command::Check { file } => {
            let source = read_file(&file);
            match check_source(&source) {
                Ok(()) => println!("Syntax OK: {}", file),
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            }
        }
        Command::Repl => {
            repl::run_repl();
        }
    }
}

fn read_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error reading '{}': {}", path, e);
        process::exit(1);
    })
}

fn run_source(source: &str) -> Result<(), ForgeError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = ForgeParser::new(tokens).parse()?;
    let mut interp = Interpreter::new();
    interp.run(&stmts)?;
    Ok(())
}

fn check_source(source: &str) -> Result<(), ForgeError> {
    let tokens = Lexer::new(source).tokenize()?;
    ForgeParser::new(tokens).parse()?;
    Ok(())
}
