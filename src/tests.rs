use crate::{
    error::ForgeError, interpreter::Interpreter, lexer::Lexer, parser::Parser, token::TokenKind,
    value::Value,
};

// --- test helpers ---

fn tokenize(source: &str) -> Vec<TokenKind> {
    Lexer::new(source)
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

fn run(source: &str) -> Result<Interpreter, ForgeError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    let mut interp = Interpreter::new();
    interp.run(&stmts)?;
    Ok(interp)
}

fn check(source: &str) -> Result<(), ForgeError> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::new(tokens).parse()?;
    Ok(())
}

// --- lexer tests ---

#[test]
fn lex_integer() {
    let kinds = tokenize("42");
    assert!(matches!(kinds[0], TokenKind::Number(n) if n == 42.0));
}

#[test]
fn lex_float() {
    let kinds = tokenize("3.14");
    assert!(matches!(kinds[0], TokenKind::Number(n) if (n - 3.14).abs() < 1e-10));
}

#[test]
fn lex_string() {
    let kinds = tokenize(r#""hello world""#);
    assert!(matches!(&kinds[0], TokenKind::String(s) if s == "hello world"));
}

#[test]
fn lex_identifier() {
    let kinds = tokenize("foo bar_baz _x");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "foo"));
    assert!(matches!(&kinds[1], TokenKind::Ident(s) if s == "bar_baz"));
    assert!(matches!(&kinds[2], TokenKind::Ident(s) if s == "_x"));
}

#[test]
fn lex_keywords() {
    let kinds = tokenize("let if else print true false");
    assert_eq!(kinds[0], TokenKind::Let);
    assert_eq!(kinds[1], TokenKind::If);
    assert_eq!(kinds[2], TokenKind::Else);
    assert_eq!(kinds[3], TokenKind::Print);
    assert_eq!(kinds[4], TokenKind::True);
    assert_eq!(kinds[5], TokenKind::False);
}

#[test]
fn lex_arithmetic_operators() {
    let kinds = tokenize("+ - * /");
    assert_eq!(kinds[0], TokenKind::Plus);
    assert_eq!(kinds[1], TokenKind::Minus);
    assert_eq!(kinds[2], TokenKind::Star);
    assert_eq!(kinds[3], TokenKind::Slash);
}

#[test]
fn lex_comparison_operators() {
    let kinds = tokenize("== != < <= > >= ! =");
    assert_eq!(kinds[0], TokenKind::EqEq);
    assert_eq!(kinds[1], TokenKind::BangEq);
    assert_eq!(kinds[2], TokenKind::Lt);
    assert_eq!(kinds[3], TokenKind::LtEq);
    assert_eq!(kinds[4], TokenKind::Gt);
    assert_eq!(kinds[5], TokenKind::GtEq);
    assert_eq!(kinds[6], TokenKind::Bang);
    assert_eq!(kinds[7], TokenKind::Eq);
}

#[test]
fn lex_line_comment_skipped() {
    let kinds = tokenize("42 // this is ignored\n99");
    assert!(matches!(kinds[0], TokenKind::Number(n) if n == 42.0));
    assert!(matches!(kinds[1], TokenKind::Number(n) if n == 99.0));
}

// --- parser / precedence tests ---

#[test]
fn parse_arithmetic_precedence_mul_before_add() {
    // 1 + 2 * 3 = 7, not 9
    let interp = run("let r = 1 + 2 * 3").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn parse_grouping_overrides_precedence() {
    let interp = run("let r = (1 + 2) * 3").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(9.0)));
}

// --- arithmetic evaluation ---

#[test]
fn eval_addition() {
    let interp = run("let r = 3 + 4").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn eval_subtraction() {
    let interp = run("let r = 10 - 3").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn eval_multiplication() {
    let interp = run("let r = 4 * 5").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(20.0)));
}

#[test]
fn eval_division() {
    let interp = run("let r = 10 / 4").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(2.5)));
}

#[test]
fn eval_unary_negation() {
    let interp = run("let r = -5").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(-5.0)));
}

#[test]
fn eval_boolean_not() {
    let interp = run("let r = !true").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(false)));
}

// --- variable assignment ---

#[test]
fn let_number() {
    let interp = run("let x = 42").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(42.0)));
}

#[test]
fn let_string() {
    let interp = run(r#"let name = "Forge""#).unwrap();
    assert_eq!(
        interp.get_var("name"),
        Some(Value::Str("Forge".to_string()))
    );
}

#[test]
fn let_bool() {
    let interp = run("let flag = true").unwrap();
    assert_eq!(interp.get_var("flag"), Some(Value::Bool(true)));
}

#[test]
fn let_expression() {
    let interp = run("let x = 2 + 3 * 4").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(14.0)));
}

// --- block scope ---

#[test]
fn block_inner_variable_does_not_leak() {
    let interp = run("let x = 5\n{ let inner = 10 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(5.0)));
    assert_eq!(interp.get_var("inner"), None);
}

#[test]
fn block_can_read_outer_variable() {
    run("let x = 5\n{ let y = x }").unwrap();
}

#[test]
fn block_inner_shadow_does_not_change_outer() {
    // After the block, outer x should still be 1.
    let interp = run("let x = 1\n{ let x = 99 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(1.0)));
}

// --- if / else ---

#[test]
fn if_true_executes_then() {
    run("if true { let x = 1 }").unwrap();
}

#[test]
fn if_false_skips_then() {
    run("if false { let x = 1 }").unwrap();
}

#[test]
fn if_else_condition_true() {
    run("if true { let a = 1 } else { let b = 2 }").unwrap();
}

#[test]
fn if_else_condition_false() {
    run("if false { let a = 1 } else { let b = 2 }").unwrap();
}

#[test]
fn if_comparison_true_branch() {
    run("let score = 12\nif score > 10 { let high = true }").unwrap();
}

// --- runtime errors ---

#[test]
fn error_undefined_variable() {
    match run("print(not_defined)") {
        Err(ForgeError::Runtime(e)) => {
            assert!(
                e.msg.contains("not_defined"),
                "expected 'not_defined' in: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected RuntimeError, got Ok"),
        Err(e) => panic!("expected RuntimeError, got: {}", e),
    }
}

#[test]
fn error_add_number_and_bool() {
    match run("let x = 1 + true") {
        Err(ForgeError::Runtime(e)) => {
            assert!(
                e.msg.contains("Number") && e.msg.contains("Bool"),
                "expected type names in error, got: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected RuntimeError, got Ok"),
        Err(e) => panic!("expected RuntimeError, got: {}", e),
    }
}

#[test]
fn error_division_by_zero() {
    let result = run("let x = 5 / 0");
    assert!(matches!(result, Err(ForgeError::Runtime(_))));
}

// --- check command (parse only) ---

#[test]
fn check_valid_let() {
    assert!(check("let x = 1 + 2").is_ok());
}

#[test]
fn check_valid_if_else() {
    assert!(check(r#"if true { print("hi") } else { print("bye") }"#).is_ok());
}

#[test]
fn check_missing_ident_after_let() {
    // `let = 5` is a syntax error
    assert!(matches!(check("let = 5"), Err(ForgeError::Parse(_))));
}

#[test]
fn check_missing_condition_in_if() {
    // `if { }` — `{` is not a valid expression
    assert!(matches!(check("if { }"), Err(ForgeError::Parse(_))));
}
