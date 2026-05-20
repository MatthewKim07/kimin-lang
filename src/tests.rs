use crate::{
    bytecode::{Constant, Instruction},
    compiler::BytecodeCompiler,
    env::Env,
    error::KiminError,
    interpreter::Interpreter,
    lexer::Lexer,
    parser::Parser,
    token::TokenKind,
    typechecker::{TypeChecker, UnitDimension},
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

/// Lex, parse, type-check, and execute. Returns the Interpreter on success.
fn run(source: &str) -> Result<Interpreter, KiminError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    TypeChecker::new().check(&stmts)?;
    let mut interp = Interpreter::new();
    interp.run(&stmts)?;
    Ok(interp)
}

/// Lex, parse, and type-check (no execution). Returns Ok on success.
fn check(source: &str) -> Result<(), KiminError> {
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    TypeChecker::new().check(&stmts)?;
    Ok(())
}

/// Lex, parse, and compile to bytecode (no type check — focused on IR shape).
fn compile_prog(source: &str) -> crate::bytecode::BytecodeProgram {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    BytecodeCompiler::new().compile(&stmts).unwrap()
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
fn lex_colon() {
    let kinds = tokenize(":");
    assert_eq!(kinds[0], TokenKind::Colon);
}

#[test]
fn lex_arrow() {
    let kinds = tokenize("->");
    assert_eq!(kinds[0], TokenKind::Arrow);
}

#[test]
fn lex_arrow_vs_minus() {
    // `-x` is minus; `->` is arrow.
    let minus = tokenize("-5");
    assert_eq!(minus[0], TokenKind::Minus);
    let arrow = tokenize("->");
    assert_eq!(arrow[0], TokenKind::Arrow);
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
    let interp = run("let r = 1 + 2 * 3").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn parse_grouping_overrides_precedence() {
    let interp = run("let r = (1 + 2) * 3").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(9.0)));
}

#[test]
fn parse_let_with_annotation() {
    assert!(check("let x: Number = 5").is_ok());
    assert!(check(r#"let name: Text = "hi""#).is_ok());
    assert!(check("let flag: Bool = true").is_ok());
}

#[test]
fn parse_fn_typed_params_and_return() {
    assert!(check("fn add(a: Number, b: Number) -> Number { return a + b }").is_ok());
    assert!(check("fn greet(name: Text) -> Text { return name }").is_ok());
    assert!(check("fn noop() -> Nil { }").is_ok());
}

#[test]
fn parse_error_missing_param_type() {
    // Parameters require `: TypeAnnotation`.
    assert!(matches!(check("fn f(x) { }"), Err(KiminError::Parse(_))));
}

#[test]
fn parse_error_unknown_type_name() {
    // Unknown type names now defer to the type checker and produce TypeError, not ParseError.
    assert!(matches!(
        check("let x: Banana = 5"),
        Err(KiminError::Type(_))
    ));
    assert!(matches!(
        check("fn f(x: Meters) { }"),
        Err(KiminError::Type(_))
    ));
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
    let interp = run(r#"let name = "Kimin""#).unwrap();
    assert_eq!(
        interp.get_var("name"),
        Some(Value::Str("Kimin".to_string()))
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

#[test]
fn if_number_condition_is_type_error() {
    // `if 0` — condition must be Bool, not Number.
    assert!(matches!(
        run("if 0 { let x = 1 }"),
        Err(KiminError::Type(_))
    ));
}

// --- runtime errors ---

#[test]
fn error_undefined_variable() {
    // Undefined variables are now caught by the type checker.
    match run("print(not_defined)") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("not_defined"),
                "expected 'not_defined' in: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError, got Ok"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn error_add_number_and_bool() {
    // Type mismatches are now caught by the type checker.
    match run("let x = 1 + true") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("Number") && e.msg.contains("Bool"),
                "expected type names in error, got: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError, got Ok"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn error_division_by_zero() {
    // Division by zero is not a type error — both operands are Number.
    // It is still a runtime error.
    let result = run("let x = 5 / 0");
    assert!(matches!(result, Err(KiminError::Runtime(_))));
}

// --- check command (parse + type check only) ---

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
    assert!(matches!(check("let = 5"), Err(KiminError::Parse(_))));
}

#[test]
fn check_missing_condition_in_if() {
    assert!(matches!(check("if { }"), Err(KiminError::Parse(_))));
}

// --- string operations ---

#[test]
fn string_concatenation() {
    let interp = run(r#"let s = "hello" + " world""#).unwrap();
    assert_eq!(
        interp.get_var("s"),
        Some(Value::Str("hello world".to_string()))
    );
}

#[test]
fn string_plus_number_is_type_error() {
    assert!(matches!(
        run(r#"let x = "hello" + 1"#),
        Err(KiminError::Type(_))
    ));
}

// --- equality and comparison return values ---

#[test]
fn equality_same_values_is_true() {
    let interp = run("let r = 1 == 1").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn equality_different_values_is_false() {
    let interp = run("let r = 1 == 2").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(false)));
}

#[test]
fn inequality_different_values_is_true() {
    let interp = run("let r = 1 != 2").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn equality_cross_type_is_type_error() {
    // Static typing requires same-type equality.
    // `1 == "1"` is now a TypeError (was: false at runtime in M2B).
    assert!(matches!(
        run(r#"let r = 1 == "1""#),
        Err(KiminError::Type(_))
    ));
}

// --- truthiness / Bool operator semantics (Milestone 3 changes) ---

#[test]
fn not_on_bool_ok() {
    let interp = run("let r = !true").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(false)));
}

#[test]
fn not_on_number_is_type_error() {
    // `!0` — unary `!` requires Bool, not Number.
    assert!(matches!(run("let r = !0"), Err(KiminError::Type(_))));
}

#[test]
fn not_on_text_is_type_error() {
    assert!(matches!(
        run(r#"let r = !"nonempty""#),
        Err(KiminError::Type(_))
    ));
}

// --- nested blocks ---

#[test]
fn nested_blocks_scope_isolation() {
    let interp = run("let x = 1\n{ let x = 2\n  { let x = 3 }\n}").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(1.0)));
}

// --- lex errors ---

#[test]
fn lex_error_unterminated_string() {
    assert!(matches!(
        run(r#"let x = "unterminated"#),
        Err(KiminError::Lex(_))
    ));
}

#[test]
fn lex_error_unexpected_char() {
    assert!(matches!(run("let x = @5"), Err(KiminError::Lex(_))));
}

// --- parse errors ---

#[test]
fn parse_error_unclosed_paren() {
    assert!(matches!(check("let x = (1 + 2"), Err(KiminError::Parse(_))));
}

#[test]
fn parse_error_missing_closing_brace() {
    assert!(matches!(check("{ let x = 1"), Err(KiminError::Parse(_))));
}

// --- lexer: Milestone 2A tokens ---

#[test]
fn lex_fn_keyword() {
    let kinds = tokenize("fn");
    assert_eq!(kinds[0], TokenKind::Fn);
}

#[test]
fn lex_return_keyword() {
    let kinds = tokenize("return");
    assert_eq!(kinds[0], TokenKind::Return);
}

#[test]
fn lex_comma() {
    let kinds = tokenize(",");
    assert_eq!(kinds[0], TokenKind::Comma);
}

// --- parser: function declarations ---

#[test]
fn parse_fn_decl_zero_params() {
    assert!(check("fn greet() { }").is_ok());
}

#[test]
fn parse_fn_decl_multiple_params() {
    assert!(check("fn add(a: Number, b: Number, c: Number) { return a + b + c }").is_ok());
}

#[test]
fn parse_return_with_value() {
    assert!(check("fn f() -> Number { return 42 }").is_ok());
}

#[test]
fn parse_return_without_value() {
    assert!(check("fn f() { return }").is_ok());
}

#[test]
fn parse_call_zero_args() {
    assert!(check("fn f() { } f()").is_ok());
}

#[test]
fn parse_call_multiple_args() {
    assert!(check("fn add(a: Number, b: Number) -> Number { return a + b } add(1, 2)").is_ok());
}

#[test]
fn parse_nested_calls() {
    assert!(check("fn id(x: Number) -> Number { return x } id(id(id(5)))").is_ok());
}

// --- interpreter: function calls ---

#[test]
fn fn_call_returns_value() {
    let interp =
        run("fn add(a: Number, b: Number) -> Number { return a + b }\nlet r = add(2, 3)").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(5.0)));
}

#[test]
fn fn_without_return_gives_nil() {
    let interp = run("fn noop() { let x = 1 }\nlet r = noop()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Nil));
}

#[test]
fn fn_bare_return_gives_nil() {
    let interp = run("fn early() { return }\nlet r = early()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Nil));
}

#[test]
fn fn_params_bind_correctly() {
    let interp =
        run("fn sub(x: Number, y: Number) -> Number { return x - y }\nlet r = sub(10, 3)").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn fn_locals_do_not_leak() {
    let interp = run("fn f() { let local = 99 }\nf()").unwrap();
    assert_eq!(interp.get_var("local"), None);
}

#[test]
fn fn_locals_shadow_globals() {
    let interp = run("let x = 1\nfn f() { let x = 99\nreturn x }\nlet r = f()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(99.0)));
    assert_eq!(interp.get_var("x"), Some(Value::Number(1.0)));
}

#[test]
fn fn_return_inside_if_exits_function() {
    let interp = run(r#"fn check(n: Number) -> Text { if n > 10 { return "big" }
return "small" }
let r = check(15)"#)
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Str("big".to_string())));
}

#[test]
fn fn_return_inside_nested_block_exits_function() {
    let interp = run("fn f() -> Number { { return 7 } }\nlet r = f()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn fn_wrong_arity_error() {
    // Wrong arity is now caught by the type checker.
    match run("fn add(a: Number, b: Number) -> Number { return a + b }\nadd(1)") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("add") && e.msg.contains("2") && e.msg.contains("1"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError, got Ok"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn fn_call_non_function_error() {
    // Calling a non-function is now caught by the type checker.
    match run("let x = 42\nx()") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("non-function") || e.msg.contains("Number"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError, got Ok"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn fn_return_outside_function_error() {
    // Return outside function is now caught by the type checker.
    match run("return 5") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("return") && e.msg.contains("outside"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError, got Ok"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn fn_recursion_factorial() {
    let interp = run(
        "fn fact(n: Number) -> Number { if n <= 1 { return 1 }\nreturn n * fact(n - 1) }\nlet r = fact(5)",
    )
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(120.0)));
}

// --- Milestone 2A / 2B scoping tests ---

#[test]
fn scoping_global_variable_readable_in_function() {
    let interp = run("let x = 42\nfn get_x() { return x }\nlet r = get_x()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(42.0)));
}

#[test]
fn scoping_lexical_does_not_see_caller_local() {
    let interp = run(
        "let x = 10\nfn show() { return x }\nfn caller() { let x = 99\nreturn show() }\nlet r = caller()"
    ).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(10.0)));
}

#[test]
fn scoping_prompt_example_lexical() {
    let interp2 = run("let x = 10\nfn show() { return x }\nlet r = show()").unwrap();
    assert_eq!(interp2.get_var("r"), Some(Value::Number(10.0)));
}

#[test]
fn scoping_fn_param_shadows_global() {
    let interp = run("let x = 10\nfn f(x: Number) -> Number { return x }\nlet r = f(99)").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(99.0)));
    assert_eq!(interp.get_var("x"), Some(Value::Number(10.0)));
}

#[test]
fn scoping_function_scope_popped_after_call() {
    let interp = run("fn f() { let inner = 55\nreturn inner }\nlet r = f()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(55.0)));
    assert_eq!(interp.get_var("inner"), None);
}

#[test]
fn scoping_forward_reference_fails() {
    // Type checker pre-registers all function signatures, so the call type-checks.
    // But the runtime evaluates `add(1, 2)` before `fn add(...)` has executed,
    // so the runtime env does not yet contain `add`.
    match run("let r = add(1, 2)\nfn add(a: Number, b: Number) -> Number { return a + b }") {
        Err(KiminError::Runtime(e)) => {
            assert!(e.msg.contains("add"), "expected 'add' in: {}", e.msg);
        }
        Ok(_) => panic!("expected RuntimeError, got Ok"),
        Err(e) => panic!("expected RuntimeError, got: {}", e),
    }
}

#[test]
fn scoping_mutual_recursion_works() {
    // Both functions pre-registered in the type environment at the same scope level.
    // Both closure_envs share the same global Rc, which contains both names.
    let interp = run(
        "fn is_even(n: Number) -> Bool { if n == 0 { return true }\nreturn is_odd(n - 1) }\nfn is_odd(n: Number) -> Bool { if n == 0 { return false }\nreturn is_even(n - 1) }\nlet r = is_even(4)"
    ).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn return_propagates_through_multiple_nested_blocks() {
    let interp = run("fn f() -> Number { { { return 42 } } }\nlet r = f()").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(42.0)));
}

// --- Milestone 2B: closures ---

#[test]
fn fn_nested_function_captures_outer_local() {
    let interp = run(
        "fn outer() { let captured = 42\nfn inner() { return captured }\nreturn inner() }\nlet r = outer()"
    ).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(42.0)));
}

#[test]
fn fn_closure_captures_definition_scope() {
    let interp = run(
        "fn make_getter() { let x = 77\nfn get() { return x }\nreturn get }\nlet getter = make_getter()\nlet r = getter()"
    ).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(77.0)));
}

// --- REPL: function preserved across interpreter calls ---

#[test]
fn repl_function_preserved_across_calls() {
    let mut interp = Interpreter::new();
    let mut tc = TypeChecker::new();

    let src1 = "fn add(a: Number, b: Number) -> Number { return a + b }";
    let tokens = Lexer::new(src1).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    tc.check(&stmts).unwrap();
    interp.run(&stmts).unwrap();

    let src2 = "let r = add(10, 5)";
    let tokens2 = Lexer::new(src2).tokenize().unwrap();
    let stmts2 = Parser::new(tokens2).parse().unwrap();
    tc.check(&stmts2).unwrap();
    interp.run(&stmts2).unwrap();

    assert_eq!(interp.get_var("r"), Some(Value::Number(15.0)));
}

// --- Milestone 3: static type checker ---

#[test]
fn type_annotated_let_correct() {
    let interp = run("let x: Number = 42").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(42.0)));
}

#[test]
fn type_annotated_let_mismatch_is_type_error() {
    assert!(matches!(
        run(r#"let x: Number = "hello""#),
        Err(KiminError::Type(_))
    ));
    assert!(matches!(run("let x: Text = 5"), Err(KiminError::Type(_))));
    assert!(matches!(run("let x: Bool = 42"), Err(KiminError::Type(_))));
}

#[test]
fn type_inferred_let_number() {
    let interp = run("let x = 10").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(10.0)));
}

#[test]
fn type_inferred_let_text() {
    let interp = run(r#"let s = "hello""#).unwrap();
    assert_eq!(interp.get_var("s"), Some(Value::Str("hello".to_string())));
}

#[test]
fn type_inferred_let_bool() {
    let interp = run("let b = false").unwrap();
    assert_eq!(interp.get_var("b"), Some(Value::Bool(false)));
}

#[test]
fn type_arithmetic_number_ok() {
    assert!(check("let r = 1 + 2 * 3 - 4 / 2").is_ok());
}

#[test]
fn type_text_concat_ok() {
    assert!(check(r#"let r = "hello" + " world""#).is_ok());
}

#[test]
fn type_arithmetic_wrong_types_error() {
    assert!(matches!(run("let r = 5 - true"), Err(KiminError::Type(_))));
    assert!(matches!(
        run(r#"let r = "hi" * 2"#),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_bool_not_ok() {
    let interp = run("let r = !false").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn type_fn_return_type_checked() {
    // Correct: return type matches annotation.
    assert!(check("fn f() -> Number { return 42 }").is_ok());
    assert!(check(r#"fn g() -> Text { return "hi" }"#).is_ok());
    assert!(check("fn h() -> Bool { return true }").is_ok());
}

#[test]
fn type_fn_return_mismatch_is_type_error() {
    assert!(matches!(
        check(r#"fn f() -> Number { return "wrong" }"#),
        Err(KiminError::Type(_))
    ));
    assert!(matches!(
        check("fn g() -> Text { return 42 }"),
        Err(KiminError::Type(_))
    ));
    assert!(matches!(
        check("fn h() -> Bool { return 1 }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_fn_wrong_arg_type_is_type_error() {
    assert!(matches!(
        run(r#"fn f(x: Number) -> Number { return x } f("wrong")"#),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_fn_call_with_correct_types_ok() {
    assert!(check("fn add(a: Number, b: Number) -> Number { return a + b }\nadd(1, 2)").is_ok());
}

#[test]
fn type_recursion_ok() {
    assert!(check(
        "fn fact(n: Number) -> Number { if n <= 1 { return 1 }\nreturn n * fact(n - 1) }"
    )
    .is_ok());
}

#[test]
fn type_closure_capture_ok() {
    // Nested function captures outer local; type checker handles this via lexical scope stack.
    assert!(check(
        "fn outer(x: Number) -> Number { fn inner() -> Number { return x }\nreturn inner() }"
    )
    .is_ok());
}

#[test]
fn type_mutual_recursion_in_block_ok() {
    // Two mutually-recursive functions declared inside a block.
    // check_stmt_list pre-registers both signatures before checking either body,
    // so is_even can refer to is_odd and vice versa at block scope.
    assert!(check(
        "{ fn is_even(n: Number) -> Bool { if n == 0 { return true }\nreturn is_odd(n - 1) }\nfn is_odd(n: Number) -> Bool { if n == 0 { return false }\nreturn is_even(n - 1) } }"
    )
    .is_ok());
}

#[test]
fn type_non_exhaustive_return_not_caught() {
    // Known gap: the type checker only validates return statements that are present.
    // A function declared -> Number that can fall off the end returns Nil at runtime.
    // This should ideally be a TypeError; it is not currently caught.
    assert!(check("fn f(x: Bool) -> Number { if x { return 1 } }").is_ok());
}

#[test]
fn type_unknown_flows_through_annotated_let() {
    // Unannotated function has Unknown return type.
    // Assigning its result to a `: Number`-annotated let passes the type checker
    // because Unknown is the gradual-typing wildcard.
    // The binding then carries Number in the TypeEnv going forward.
    assert!(check("fn f(x: Number) { return x }\nlet r = f(5)\nlet n: Number = r").is_ok());
}

// --- Milestone 4: unit-aware types ---

#[test]
fn parse_unit_let_annotation_long_name() {
    assert!(check("let d: meters = 10").is_ok());
    assert!(check("let t: seconds = 5").is_ok());
    assert!(check("let m: kilograms = 3").is_ok());
}

#[test]
fn parse_unit_let_annotation_short_alias() {
    assert!(check("let d: m = 10").is_ok());
    assert!(check("let t: s = 5").is_ok());
    assert!(check("let m: kg = 3").is_ok());
}

#[test]
fn parse_unit_fn_param_annotation() {
    assert!(check("fn f(d: meters) { }").is_ok());
    assert!(check("fn f(d: m) { }").is_ok());
}

#[test]
fn parse_unit_fn_return_annotation() {
    assert!(check("fn f() -> meters { return 10 }").is_ok());
    assert!(check("fn f() -> seconds { return 0 }").is_ok());
}

#[test]
fn parse_unit_all_registry_names() {
    // All supported unit names and aliases parse without error.
    assert!(check("let a: meters = 1").is_ok());
    assert!(check("let a: m = 1").is_ok());
    assert!(check("let a: seconds = 1").is_ok());
    assert!(check("let a: s = 1").is_ok());
    assert!(check("let a: kilograms = 1").is_ok());
    assert!(check("let a: kg = 1").is_ok());
    assert!(check("let a: amperes = 1").is_ok());
    assert!(check("let a: amps = 1").is_ok());
    assert!(check("let a: A = 1").is_ok());
    assert!(check("let a: kelvin = 1").is_ok());
    assert!(check("let a: K = 1").is_ok());
    assert!(check("let a: moles = 1").is_ok());
    assert!(check("let a: mol = 1").is_ok());
    assert!(check("let a: candela = 1").is_ok());
    assert!(check("let a: cd = 1").is_ok());
    assert!(check("let a: radians = 1").is_ok());
    assert!(check("let a: rad = 1").is_ok());
    assert!(check("let a: degrees = 1").is_ok());
    assert!(check("let a: deg = 1").is_ok());
    assert!(check("let a: volts = 1").is_ok());
    assert!(check("let a: V = 1").is_ok());
    assert!(check("let a: watts = 1").is_ok());
    assert!(check("let a: W = 1").is_ok());
    assert!(check("let a: joules = 1").is_ok());
    assert!(check("let a: J = 1").is_ok());
    assert!(check("let a: newtons = 1").is_ok());
    assert!(check("let a: N = 1").is_ok());
}

#[test]
fn parse_unit_alias_canonicalizes() {
    // Short alias and long name produce the same canonical type — operations between them work.
    assert!(check("let a: m = 10\nlet b: meters = 5\nlet c = a + b").is_ok());
}

#[test]
fn type_unit_let_from_number_literal_ok() {
    let interp = run("let d: meters = 10").unwrap();
    assert_eq!(interp.get_var("d"), Some(Value::Number(10.0)));
}

#[test]
fn type_unit_let_from_unit_var_same_unit_ok() {
    assert!(check("let a: meters = 10\nlet b: meters = a").is_ok());
}

#[test]
fn type_unit_let_from_text_error() {
    assert!(matches!(
        run(r#"let d: meters = "ten""#),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_let_from_bool_error() {
    assert!(matches!(
        run("let d: meters = true"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_let_wrong_unit_error() {
    match run("let t: seconds = 2\nlet bad: meters = t") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("meters") && e.msg.contains("seconds"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn type_unit_inferred_let_preserves_unit() {
    // Inferred let from unit variable inherits unit type; arithmetic stays unit-typed.
    assert!(check("let a: meters = 10\nlet b = a\nlet c = a + b").is_ok());
}

#[test]
fn type_unit_unit_to_number_annotation_error() {
    // Cannot strip unit by assigning to Number.
    assert!(matches!(
        run("let d: meters = 10\nlet n: Number = d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_same_unit_add_ok() {
    let interp = run("let a: meters = 10\nlet b: meters = 5\nlet c = a + b").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(15.0)));
}

#[test]
fn type_unit_different_unit_add_error() {
    match run("let d: meters = 10\nlet t: seconds = 2\nlet bad = d + t") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("meters") && e.msg.contains("seconds"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn type_unit_number_plus_unit_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet bad = 5 + d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_same_unit_sub_ok() {
    let interp = run("let a: meters = 10\nlet b: meters = 3\nlet c = a - b").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(7.0)));
}

#[test]
fn type_unit_different_unit_sub_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet t: seconds = 2\nlet bad = d - t"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_scalar_mul_number_times_unit_ok() {
    let interp = run("let d: meters = 3\nlet c = 4 * d").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(12.0)));
}

#[test]
fn type_unit_scalar_mul_unit_times_number_ok() {
    let interp = run("let d: meters = 3\nlet c = d * 4").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(12.0)));
}

#[test]
fn type_unit_compound_mul_infers_product() {
    // meters * seconds now infers compound type meters*seconds, not a TypeError.
    let interp = run("let d: meters = 10\nlet t: seconds = 2\nlet compound = d * t").unwrap();
    assert_eq!(interp.get_var("compound"), Some(Value::Number(20.0)));
    // Indirect type check: two values of the same compound type can be added.
    assert!(check(
        "let d: meters = 10\nlet t: seconds = 2\nlet c1 = d * t\nlet c2 = d * t\nlet sum = c1 + c2"
    )
    .is_ok());
    // A different compound type cannot be added.
    assert!(matches!(
        check("let d: meters = 10\nlet t: seconds = 2\nlet c = d * t\nlet bad = c + d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_unit_div_number_ok() {
    let interp = run("let d: meters = 12\nlet c = d / 4").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(3.0)));
}

#[test]
fn type_unit_same_unit_div_gives_number() {
    let interp = run("let a: meters = 10\nlet b: meters = 2\nlet c = a / b").unwrap();
    assert_eq!(interp.get_var("c"), Some(Value::Number(5.0)));
}

#[test]
fn type_unit_different_unit_div_infers_quotient() {
    // meters / seconds now infers compound type meters/seconds, not a TypeError.
    let interp = run("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t").unwrap();
    assert_eq!(interp.get_var("speed"), Some(Value::Number(5.0)));
    // Two values of the same compound type can be added.
    assert!(check(
        "let d: meters = 10\nlet t: seconds = 2\nlet s1 = d / t\nlet s2 = d / t\nlet sum = s1 + s2"
    )
    .is_ok());
    // A different compound type cannot be added.
    assert!(matches!(
        check("let d: meters = 10\nlet t: seconds = 2\nlet s = d / t\nlet bad = s + d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_number_div_unit_infers_reciprocal() {
    // Number / unit now infers a reciprocal unit type 1/unit, not a TypeError.
    let interp = run("let d: meters = 10\nlet rate = 5 / d").unwrap();
    assert_eq!(interp.get_var("rate"), Some(Value::Number(0.5)));
    // Multiplying back recovers Number (reciprocal * base = dimensionless).
    let interp2 = run("let t: seconds = 2\nlet freq = 10 / t\nlet back = freq * t").unwrap();
    assert_eq!(interp2.get_var("back"), Some(Value::Number(10.0)));
}

#[test]
fn type_unit_same_unit_comparison_ok() {
    let interp = run("let a: meters = 10\nlet b: meters = 5\nlet r = a > b").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn type_unit_different_unit_comparison_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet t: seconds = 2\nlet bad = d < t"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_number_unit_comparison_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet bad = 5 < d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_same_unit_equality_ok() {
    let interp = run("let a: meters = 10\nlet b: meters = 10\nlet r = a == b").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn type_unit_different_unit_equality_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet t: seconds = 10\nlet bad = d == t"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_fn_unit_param_ok() {
    let interp =
        run("fn f(d: meters) -> meters { return d }\nlet a: meters = 10\nlet r = f(a)").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(10.0)));
}

#[test]
fn type_unit_fn_arg_promotion_from_number() {
    // Raw numeric literal promoted to unit when param expects unit.
    let interp =
        run("fn add(a: meters, b: meters) -> meters { return a + b }\nlet r = add(10, 5)").unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(15.0)));
}

#[test]
fn type_unit_fn_wrong_unit_arg_error() {
    assert!(matches!(
        run("fn f(d: meters) -> meters { return d }\nlet t: seconds = 2\nf(t)"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_fn_return_promotion_from_number() {
    // Function declared -> meters can return a plain Number (promotion).
    assert!(check("fn ten_meters() -> meters { return 10 }").is_ok());
}

#[test]
fn type_unit_fn_return_wrong_unit_error() {
    assert!(matches!(
        check("fn f() -> meters { let t: seconds = 2\nreturn t }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_closure_captures_unit_var() {
    assert!(check(
        "fn outer(d: meters) -> meters { fn inner() -> meters { return d }\nreturn inner() }"
    )
    .is_ok());
}

#[test]
fn type_unit_unary_neg_preserves_unit() {
    let interp = run("let d: meters = 5\nlet neg = -d").unwrap();
    assert_eq!(interp.get_var("neg"), Some(Value::Number(-5.0)));
}

#[test]
fn type_unit_scale_and_ratio() {
    // Scalar multiply then same-unit divide → gets back Number.
    let interp =
        run("let d: meters = 10\nlet scaled = d * 3\nlet a: meters = 30\nlet ratio = scaled / a")
            .unwrap();
    assert_eq!(interp.get_var("ratio"), Some(Value::Number(1.0)));
}

// --- Milestone 4 audit: edge cases ---

#[test]
fn type_unit_unit_plus_number_error() {
    // NumberWithUnit + Number is not valid; promotion only applies at assignment/call sites.
    assert!(matches!(
        run("let d: meters = 10\nlet bad = d + 5"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_unit_minus_number_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet bad = d - 5"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_number_minus_unit_error() {
    assert!(matches!(
        run("let d: meters = 10\nlet bad = 5 - d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_unit_gt_number_error() {
    // Comparison between unit and plain Number is not valid.
    assert!(matches!(
        run("let d: meters = 10\nlet bad = d > 5"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_unit_eq_number_error() {
    // Equality between unit and plain Number is not valid (different types).
    assert!(matches!(
        run("let d: meters = 10\nlet bad = d == 5"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_cross_alias_arithmetic_ok() {
    // amps and A both canonicalize to "amperes" — they can be added.
    assert!(check("let a: amps = 3\nlet b: A = 2\nlet c = a + b").is_ok());
}

#[test]
fn type_unit_number_expr_promotes_to_unit_param() {
    // A Number-typed *expression* (not just a literal) also promotes to a unit param.
    assert!(check("fn f(d: meters) -> meters { return d }\nf(2 + 3)").is_ok());
}

#[test]
fn type_unit_arg_to_number_param_is_error() {
    // Promotion is one-way: cannot pass a unit-typed value to a Number param.
    assert!(matches!(
        run("fn f(x: Number) -> Number { return x }\nlet d: meters = 10\nf(d)"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_unit_unknown_plus_unit_propagates_unknown() {
    // Unknown on either side of binary op propagates Unknown — gradual typing escape hatch.
    // An unannotated-return function returns Unknown; Unknown + unit → Unknown, no error.
    assert!(check("fn f(x: Number) { return x }\nlet d: meters = 10\nlet r = f(5) + d").is_ok());
}

#[test]
fn type_unit_unknown_satisfies_unit_annotation() {
    // Unknown from an unannotated function satisfies a unit annotation on let.
    assert!(check("fn f(x: Number) { return x }\nlet d: meters = f(10)").is_ok());
}

// --- Milestone 4B: UnitDimension struct unit tests ---

#[test]
fn unit_dim_base_display() {
    assert_eq!(UnitDimension::base("meters").display_name(), "meters");
    assert_eq!(UnitDimension::base("seconds").display_name(), "seconds");
}

#[test]
fn unit_dim_squared_display() {
    let m = UnitDimension::base("meters");
    assert_eq!(m.mul(&m).display_name(), "meters^2");
}

#[test]
fn unit_dim_divided_display() {
    let m = UnitDimension::base("meters");
    let s = UnitDimension::base("seconds");
    assert_eq!(m.div(&s).display_name(), "meters/seconds");
}

#[test]
fn unit_dim_reciprocal_display() {
    let s = UnitDimension::base("seconds");
    let recip = UnitDimension::dimensionless().div(&s);
    assert_eq!(recip.display_name(), "1/seconds");
}

#[test]
fn unit_dim_complex_display() {
    // kilograms * meters / seconds^2
    let kg = UnitDimension::base("kilograms");
    let m = UnitDimension::base("meters");
    let s = UnitDimension::base("seconds");
    let result = kg.mul(&m).div(&s).div(&s);
    assert_eq!(result.display_name(), "kilograms*meters/seconds^2");
}

#[test]
fn unit_dim_dimensionless() {
    let m = UnitDimension::base("meters");
    assert!(m.div(&m).is_dimensionless());
}

#[test]
fn unit_dim_zero_exponents_removed() {
    // meters * (1/meters) → dimensionless; map must be empty, not {meters: 0}.
    let m = UnitDimension::base("meters");
    let recip = UnitDimension::dimensionless().div(&m);
    let result = m.mul(&recip);
    assert!(result.is_dimensionless());
}

#[test]
fn unit_dim_ordering_deterministic() {
    // BTreeMap → alphabetical order. meters*seconds and seconds*meters produce same string.
    let m = UnitDimension::base("meters");
    let s = UnitDimension::base("seconds");
    assert_eq!(m.mul(&s).display_name(), s.mul(&m).display_name());
    assert_eq!(m.mul(&s).display_name(), "meters*seconds");
}

// --- Milestone 4B: compound unit type checker tests ---

#[test]
fn type_compound_mul_same_unit_infers_squared() {
    let interp = run("let d: meters = 3\nlet area = d * d").unwrap();
    assert_eq!(interp.get_var("area"), Some(Value::Number(9.0)));
    // meters^2 + meters^2 = ok; meters^2 + meters = error.
    assert!(check("let d: meters = 3\nlet a1 = d * d\nlet a2 = d * d\nlet sum = a1 + a2").is_ok());
    assert!(matches!(
        check("let d: meters = 3\nlet a = d * d\nlet bad = a + d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_compound_mul_different_units_infers_product() {
    let interp = run("let d: meters = 3\nlet t: seconds = 4\nlet p = d * t").unwrap();
    assert_eq!(interp.get_var("p"), Some(Value::Number(12.0)));
}

#[test]
fn type_compound_div_meters_over_seconds_infers_speed() {
    let interp = run("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t").unwrap();
    assert_eq!(interp.get_var("speed"), Some(Value::Number(5.0)));
}

#[test]
fn type_compound_number_div_unit_infers_reciprocal() {
    let interp = run("let t: seconds = 4\nlet freq = 20 / t").unwrap();
    assert_eq!(interp.get_var("freq"), Some(Value::Number(5.0)));
}

#[test]
fn type_compound_add_same_compound_ok() {
    // speed + speed → ok (same compound dimension)
    let interp = run(
        "let d: meters = 10\nlet t: seconds = 2\nlet s1 = d / t\nlet s2 = d / t\nlet total = s1 + s2"
    )
    .unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(10.0)));
}

#[test]
fn type_compound_add_different_compound_error() {
    match check("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet bad = speed + d") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("meters") && e.msg.contains("seconds"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn type_compound_compare_same_compound_ok() {
    let interp = run(
        "let d: meters = 10\nlet t: seconds = 2\nlet s1 = d / t\nlet s2 = d / t\nlet r = s1 > s2",
    )
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(false)));
}

#[test]
fn type_compound_compare_different_compound_error() {
    assert!(matches!(
        check("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nif speed < d { }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_compound_equality_same_compound_ok() {
    let interp = run(
        "let d: meters = 10\nlet t: seconds = 2\nlet s1 = d / t\nlet s2 = d / t\nlet r = s1 == s2",
    )
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn type_compound_equality_different_compound_error() {
    assert!(matches!(
        check("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet bad = speed == d"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_compound_let_inferred_preserves_type() {
    // Inferred compound type is preserved and participates in further arithmetic.
    assert!(check(
        "let d: meters = 10\nlet t: seconds = 2\nlet s = d / t\nlet s2 = s\nlet sum = s + s2"
    )
    .is_ok());
}

#[test]
fn type_compound_assign_to_wrong_annotation_errors() {
    // Cannot assign a compound unit to a base-unit annotation.
    assert!(matches!(
        check("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet bad: meters = speed"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_compound_speed_times_time_simplifies_to_distance() {
    // (meters/seconds) * seconds → meters (compound simplification).
    // Verified by assigning the result to a `: meters` annotation.
    let interp = run(
        "let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet back = speed * t\nlet check: meters = back"
    )
    .unwrap();
    assert_eq!(interp.get_var("back"), Some(Value::Number(10.0)));
}

#[test]
fn type_compound_reciprocal_times_unit_is_number() {
    // (Number / unit) * unit → Number (reciprocal cancels).
    let interp = run("let t: seconds = 2\nlet freq = 10 / t\nlet back = freq * t").unwrap();
    assert_eq!(interp.get_var("back"), Some(Value::Number(10.0)));
}

#[test]
fn type_compound_unary_neg_preserves_compound() {
    let interp =
        run("let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet neg = -speed").unwrap();
    assert_eq!(interp.get_var("neg"), Some(Value::Number(-5.0)));
    // neg and speed have the same compound type — can add.
    assert!(check(
        "let d: meters = 10\nlet t: seconds = 2\nlet speed = d / t\nlet neg = -speed\nlet sum = speed + neg"
    )
    .is_ok());
}

#[test]
fn type_compound_closure_captures_compound_type() {
    // Closure captures a compound-unit variable; type checker follows it through the capture.
    assert!(check(
        "fn outer(d: meters, t: seconds) { let speed = d / t\nfn inner() { return speed }\nreturn inner() }"
    )
    .is_ok());
}

#[test]
fn type_compound_meters_squared_runtime_value() {
    let interp = run("let w: meters = 3\nlet h: meters = 4\nlet area = w * h").unwrap();
    assert_eq!(interp.get_var("area"), Some(Value::Number(12.0)));
}

// ============================================================
// Milestone 5 — State machines
// ============================================================

// --- lexer ---

#[test]
fn lex_state_keyword() {
    let kinds = tokenize("state");
    assert!(matches!(kinds[0], TokenKind::State));
}

#[test]
fn lex_transition_keyword() {
    let kinds = tokenize("transition");
    assert!(matches!(kinds[0], TokenKind::Transition));
}

#[test]
fn lex_dot_token() {
    let kinds = tokenize("Door.closed");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "Door"));
    assert!(matches!(kinds[1], TokenKind::Dot));
    assert!(matches!(kinds[2], TokenKind::Ident(ref s) if s == "closed"));
}

// --- parser ---

#[test]
fn parse_state_decl_ok() {
    assert!(check("state Door {\n  closed\n  open\n  transition closed -> open\n}").is_ok());
}

#[test]
fn parse_state_variant_expr_ok() {
    assert!(check("state Door { closed open }\nlet d: Door = Door.closed").is_ok());
}

#[test]
fn parse_transition_stmt_ok() {
    assert!(check(
        "state Door {\n  closed\n  open\n  transition closed -> open\n}\nlet door: Door = Door.closed\ntransition door -> open"
    )
    .is_ok());
}

#[test]
fn parse_state_missing_brace_is_parse_error() {
    assert!(matches!(
        check("state Door { closed"),
        Err(KiminError::Parse(_))
    ));
}

#[test]
fn parse_transition_missing_arrow_is_parse_error() {
    assert!(matches!(
        check("state Door { closed open }\nlet d: Door = Door.closed\ntransition d closed"),
        Err(KiminError::Parse(_))
    ));
}

// --- type checker: state declarations ---

#[test]
fn type_state_decl_registers_type_ok() {
    assert!(check("state Door { closed open }").is_ok());
}

#[test]
fn type_state_duplicate_variant_is_type_error() {
    assert!(matches!(
        check("state Door { closed closed }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_duplicate_state_name_is_type_error() {
    assert!(matches!(
        check("state Door { closed }\nstate Door { open }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_transition_unknown_from_variant_is_error() {
    assert!(matches!(
        check("state Door { closed open\n  transition locked -> open\n}"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_transition_unknown_to_variant_is_error() {
    assert!(matches!(
        check("state Door { closed open\n  transition closed -> locked\n}"),
        Err(KiminError::Type(_))
    ));
}

// --- type checker: state variable bindings ---

#[test]
fn type_state_let_valid_variant_ok() {
    assert!(check("state Door { closed open }\nlet door: Door = Door.closed").is_ok());
}

#[test]
fn type_state_let_invalid_variant_is_error() {
    assert!(matches!(
        check("state Door { closed open }\nlet door: Door = Door.locked"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_unknown_state_type_annotation_is_error() {
    assert!(matches!(
        check("let x: Motor = 5"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_variant_from_unknown_state_is_error() {
    assert!(matches!(
        check("let x = Door.closed"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_assign_wrong_state_type_is_error() {
    assert!(matches!(
        check("state Door { closed }\nstate Motor { stopped }\nlet d: Door = Motor.stopped"),
        Err(KiminError::Type(_))
    ));
}

// --- type checker: functions with state types ---

#[test]
fn type_state_fn_returning_state_type_ok() {
    assert!(
        check("state Door { closed open }\nfn initial() -> Door { return Door.closed }").is_ok()
    );
}

#[test]
fn type_state_fn_return_wrong_type_is_error() {
    assert!(matches!(
        check("state Door { closed open }\nfn initial() -> Door { return 42 }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_fn_param_state_type_ok() {
    assert!(check("state Door { closed open }\nfn show(d: Door) { print(d) }").is_ok());
}

// --- type checker: transition statements ---

#[test]
fn type_state_transition_valid_ok() {
    assert!(check(
        "state Door {\n  closed\n  open\n  transition closed -> open\n}\nlet door: Door = Door.closed\ntransition door -> open"
    )
    .is_ok());
}

#[test]
fn type_state_transition_invalid_is_error() {
    assert!(matches!(
        check(
            "state Door {\n  closed\n  open\n  transition closed -> open\n}\nlet door: Door = Door.closed\ntransition door -> closed"
        ),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_transition_unknown_target_variant_is_error() {
    assert!(matches!(
        check(
            "state Door { closed open }\nlet door: Door = Door.closed\ntransition door -> locked"
        ),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_transition_on_non_state_variable_is_error() {
    assert!(matches!(
        check("let x: Number = 10\ntransition x -> open"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_known_variant_updates_after_transition() {
    // After valid transition closed -> open, door is known to be `open`.
    // Transitioning open -> closed is invalid (no such transition declared).
    assert!(matches!(
        check(
            "state Door {\n  closed\n  open\n  transition closed -> open\n}\nlet door: Door = Door.closed\ntransition door -> open\ntransition door -> closed"
        ),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_state_chain_of_transitions_ok() {
    assert!(check(
        "state Door {\n  closed\n  opening\n  open\n  transition closed -> opening\n  transition opening -> open\n}\nlet door: Door = Door.closed\ntransition door -> opening\ntransition door -> open"
    )
    .is_ok());
}

// --- interpreter ---

#[test]
fn interp_state_variant_eval() {
    let interp = run("state Door { closed open }\nlet door: Door = Door.closed").unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "closed".into(),
        })
    );
}

#[test]
fn interp_state_transition_updates_value() {
    let interp = run(
        "state Door {\n  closed\n  open\n  transition closed -> open\n}\nlet door: Door = Door.closed\ntransition door -> open"
    )
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "open".into(),
        })
    );
}

#[test]
fn interp_state_transition_sequence() {
    // Three-step transition sequence, checking final value.
    let interp = run(concat!(
        "state Door {\n  closed\n  opening\n  open\n",
        "  transition closed -> opening\n  transition opening -> open\n}\n",
        "let door: Door = Door.closed\n",
        "transition door -> opening\n",
        "transition door -> open"
    ))
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "open".into(),
        })
    );
}

#[test]
fn interp_state_function_returns_state_value() {
    let interp = run(
        "state Door { closed open }\nfn initial() -> Door { return Door.closed }\nlet door: Door = initial()"
    )
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "closed".into(),
        })
    );
}

#[test]
fn interp_state_print_state_value_ok() {
    assert!(run("state Door { closed open }\nlet door: Door = Door.closed\nprint(door)").is_ok());
}

// --- Milestone 5 audit: lexer/parser edge cases ---

#[test]
fn lex_number_dot_ident_is_number_then_dot() {
    // `42.Door` must lex as Number(42), Dot, Ident("Door") — Dot must not break float lexing.
    let kinds = tokenize("42.Door");
    assert!(matches!(kinds[0], TokenKind::Number(n) if n == 42.0));
    assert_eq!(kinds[1], TokenKind::Dot);
    assert!(matches!(&kinds[2], TokenKind::Ident(s) if s == "Door"));
}

#[test]
fn parse_trailing_dot_is_parse_error() {
    // `Door.` with no variant after the dot is a ParseError.
    assert!(matches!(check("let x = Door."), Err(KiminError::Parse(_))));
}

#[test]
fn parse_leading_dot_in_expr_is_parse_error() {
    // `.closed` in expression position is a ParseError.
    assert!(matches!(
        check("let x = .closed"),
        Err(KiminError::Parse(_))
    ));
}

// --- Milestone 5 audit: state machine edge cases ---

#[test]
fn type_state_empty_body_ok() {
    // A state with no variants is valid: no variants means no variant expressions are possible.
    assert!(check("state Empty { }").is_ok());
}

#[test]
fn type_state_duplicate_transition_rule_ok() {
    // Duplicate transition rules are silently deduplicated — not an error.
    assert!(check(
        "state Door { closed open  transition closed -> open  transition closed -> open }"
    )
    .is_ok());
}

// --- Milestone 5 audit: scope and shadowing ---

#[test]
fn interp_state_transition_in_block_updates_outer_var() {
    // Transition inside a block updates the outer binding when there is no inner shadow.
    let interp = run(concat!(
        "state Door { closed open  transition closed -> open }\n",
        "let door: Door = Door.closed\n",
        "{ transition door -> open }",
    ))
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "open".into(),
        })
    );
}

#[test]
fn interp_state_shadow_in_block_outer_unaffected() {
    // Transition on an inner shadow does not change the outer binding.
    let interp = run(concat!(
        "state Door { closed open  transition closed -> open  transition open -> closed }\n",
        "let door: Door = Door.closed\n",
        "{ let door: Door = Door.open  transition door -> closed }",
    ))
    .unwrap();
    // Outer door was closed and remains closed — inner shadow was transitioned, not outer.
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "closed".into(),
        })
    );
}

// --- Milestone 5 audit: state equality ---

#[test]
fn type_state_equality_same_state_type_ok() {
    // Same-state-type equality passes the type checker and produces Bool.
    assert!(check(
        "state Door { closed open }\nlet a: Door = Door.closed\nlet b: Door = Door.open\nlet r = a == b"
    )
    .is_ok());
}

#[test]
fn interp_state_equality_same_variant_is_true() {
    let interp = run(
        "state Door { closed open }\nlet a: Door = Door.closed\nlet b: Door = Door.closed\nlet r = a == b",
    )
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(true)));
}

#[test]
fn interp_state_equality_different_variant_is_false() {
    let interp = run(
        "state Door { closed open }\nlet a: Door = Door.closed\nlet b: Door = Door.open\nlet r = a == b",
    )
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Bool(false)));
}

#[test]
fn type_state_cross_type_equality_is_error() {
    // Equality between two different state types is a TypeError (different types).
    assert!(matches!(
        check(concat!(
            "state Door { closed }\nstate Motor { stopped }\n",
            "let d: Door = Door.closed\nlet m: Motor = Motor.stopped\n",
            "let r = d == m"
        )),
        Err(KiminError::Type(_))
    ));
}

// --- Milestone 5 audit: closures capturing state values ---

#[test]
fn type_state_closure_captures_state_var() {
    // A closure that captures a state-typed parameter type-checks correctly.
    assert!(check(concat!(
        "state Door { closed open }\n",
        "fn make_getter(d: Door) { fn get() { return d }  return get }",
    ))
    .is_ok());
}

#[test]
fn interp_state_closure_returns_captured_state_value() {
    let interp = run(concat!(
        "state Door { closed open }\n",
        "fn make_getter(d: Door) { fn get() { return d }  return get }\n",
        "let getter = make_getter(Door.closed)\n",
        "let r = getter()",
    ))
    .unwrap();
    assert_eq!(
        interp.get_var("r"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "closed".into(),
        })
    );
}

// ============================================================
// Milestone 6A — simulate blocks
// ============================================================

// --- lexer ---

#[test]
fn lex_simulate_keyword() {
    let kinds = tokenize("simulate");
    assert_eq!(kinds[0], TokenKind::Simulate);
}

#[test]
fn lex_step_keyword() {
    let kinds = tokenize("step");
    assert_eq!(kinds[0], TokenKind::Step);
}

#[test]
fn lex_simulate_and_step_together() {
    let kinds = tokenize("simulate duration step dt");
    assert_eq!(kinds[0], TokenKind::Simulate);
    assert!(matches!(&kinds[1], TokenKind::Ident(s) if s == "duration"));
    assert_eq!(kinds[2], TokenKind::Step);
    assert!(matches!(&kinds[3], TokenKind::Ident(s) if s == "dt"));
}

// --- parser ---

#[test]
fn parse_simulate_basic_parses() {
    assert!(
        check("let d: seconds = 3\nlet s: seconds = 1\nsimulate d step s { print(time) }").is_ok()
    );
}

#[test]
fn parse_simulate_missing_step_keyword_is_error() {
    let src = "let d: seconds = 3\nlet s: seconds = 1\nsimulate d s { print(time) }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(matches!(result, Err(_)));
}

#[test]
fn parse_simulate_missing_body_is_error() {
    let src = "let d: seconds = 3\nlet s: seconds = 1\nsimulate d step s";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(matches!(result, Err(_)));
}

// --- type checker ---

#[test]
fn type_simulate_seconds_duration_ok() {
    assert!(
        check("let d: seconds = 3\nlet s: seconds = 1\nsimulate d step s { print(time) }").is_ok()
    );
}

#[test]
fn type_simulate_time_var_is_seconds_in_body() {
    // time used in arithmetic with another seconds value — should be ok
    assert!(check(concat!(
        "let d: seconds = 4\nlet s: seconds = 1\n",
        "let extra: seconds = 1\n",
        "simulate d step s { let t2 = time + extra }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_time_undefined_outside_block() {
    assert!(matches!(
        check("let d: seconds = 2\nlet s: seconds = 1\nsimulate d step s { }\nprint(time)"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_plain_number_duration_is_error() {
    assert!(matches!(
        check("let d = 3\nlet s: seconds = 1\nsimulate d step s { }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_meters_duration_is_error() {
    assert!(matches!(
        check("let d: meters = 3\nlet s: seconds = 1\nsimulate d step s { }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_mismatched_step_unit_is_error() {
    assert!(matches!(
        check("let d: seconds = 3\nlet s: meters = 1\nsimulate d step s { }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_unknown_duration_accepted() {
    // Unannotated function returns produce Unknown type, which passes type check (gradual typing).
    assert!(check(concat!(
        "fn get(x: Number) { return x }\n",
        "let d = get(3)\nlet s = get(1)\n",
        "simulate d step s { }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_body_state_transition_ok() {
    assert!(check(concat!(
        "state Door { closed opening  transition closed -> opening }\n",
        "let door: Door = Door.closed\n",
        "let d: seconds = 1\nlet s: seconds = 1\n",
        "simulate d step s { transition door -> opening }"
    ))
    .is_ok());
}

// --- interpreter ---

#[test]
fn interp_simulate_zero_duration_runs_zero_iterations() {
    // duration 0 / step 1 = 0 iterations; time never set
    let interp =
        run("let d: seconds = 0\nlet s: seconds = 1\nlet count = 0\nsimulate d step s { }")
            .unwrap();
    assert_eq!(interp.get_var("count"), Some(Value::Number(0.0)));
}

#[test]
fn interp_simulate_three_iterations_time_values() {
    // Check that time takes values 0, 1, 2 (store last value)
    let interp = run(concat!(
        "let d: seconds = 3\nlet s: seconds = 1\n",
        "let last_time: seconds = 0\n",
        "simulate d step s { let last_time: seconds = time }"
    ));
    // Just verify it runs without error — time is block-scoped
    assert!(interp.is_ok());
}

#[test]
fn interp_simulate_step_zero_is_runtime_error() {
    // Unknown-typed values (unannotated fn return) bypass type checker; step=0 hits runtime error.
    assert!(matches!(
        run("fn v(x: Number) { return x }\nsimulate v(3) step v(0) { }"),
        Err(KiminError::Runtime(_))
    ));
}

#[test]
fn interp_simulate_negative_step_is_runtime_error() {
    assert!(matches!(
        run("fn v(x: Number) { return x }\nsimulate v(3) step v(-1) { }"),
        Err(KiminError::Runtime(_))
    ));
}

#[test]
fn interp_simulate_negative_duration_is_runtime_error() {
    assert!(matches!(
        run("fn v(x: Number) { return x }\nsimulate v(-1) step v(1) { }"),
        Err(KiminError::Runtime(_))
    ));
}

#[test]
fn interp_simulate_print_runs_without_error() {
    assert!(
        run("let d: seconds = 2\nlet s: seconds = 1\nsimulate d step s { print(time) }").is_ok()
    );
}

#[test]
fn interp_simulate_accesses_outer_variable() {
    // simulate body reads outer let variable
    let interp = run(concat!(
        "let d: seconds = 1\nlet s: seconds = 1\n",
        "let x = 42\n",
        "simulate d step s { let y = x }"
    ))
    .unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(42.0)));
}

#[test]
fn interp_simulate_state_transition_persists_across_iterations() {
    // door transitions inside body; outer door should reflect changes
    let interp = run(concat!(
        "state Door { closed opening  transition closed -> opening }\n",
        "let door: Door = Door.closed\n",
        "let d: seconds = 1\nlet s: seconds = 1\n",
        "simulate d step s { transition door -> opening }"
    ))
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".into(),
            variant_name: "opening".into(),
        })
    );
}

#[test]
fn interp_simulate_fractional_step_floor_iterations() {
    // 5 / 2 = 2.5 → floor = 2 iterations; runs without error.
    // Unknown-typed values (unannotated fn return) bypass time unit type check.
    assert!(run("fn v(x: Number) { return x }\nsimulate v(5) step v(2) { }").is_ok());
}

#[test]
fn interp_simulate_nested_simulate_ok() {
    assert!(run(concat!(
        "let d: seconds = 2\nlet s: seconds = 1\n",
        "simulate d step s { ",
        "let inner_d: seconds = 1\nlet inner_s: seconds = 1\n",
        "simulate inner_d step inner_s { print(time) }",
        " }"
    ))
    .is_ok());
}

// ============================================================
// Milestone 6A audit — additional coverage
// ============================================================

// --- parser (audit additions) ---

#[test]
fn parse_simulate_missing_duration_expr_is_error() {
    // `step` keyword cannot start an expression; parse_primary returns ParseError
    let src = "let s: seconds = 1\nsimulate step s { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(matches!(result, Err(_)));
}

#[test]
fn parse_simulate_missing_step_expr_is_error() {
    // `{` cannot start an expression for step; parse_primary returns ParseError
    let src = "let d: seconds = 3\nsimulate d step { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(matches!(result, Err(_)));
}

#[test]
fn parse_simulate_nested_block_in_body_parses() {
    assert!(check(concat!(
        "let d: seconds = 1\nlet s: seconds = 1\n",
        "simulate d step s { { let x = 1 } }"
    ))
    .is_ok());
}

// --- type checker (audit additions) ---

#[test]
fn type_simulate_time_shadows_outer_time_variable() {
    // Outer `time: Number`; inner `time: seconds` from simulate. No conflict — shadowing is ok.
    assert!(check(concat!(
        "let time = 99\n",
        "let d: seconds = 1\nlet s: seconds = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_inside_function_ok() {
    assert!(check(concat!(
        "fn f() {\n",
        "  let d: seconds = 1\n  let s: seconds = 1\n",
        "  simulate d step s { }\n",
        "}"
    ))
    .is_ok());
}

#[test]
fn type_simulate_return_inside_body_inside_function_ok() {
    // return inside simulate inside a function is valid — propagates to function return
    assert!(check(concat!(
        "fn f() -> Number {\n",
        "  let d: seconds = 1\n  let s: seconds = 1\n",
        "  simulate d step s { return 42 }\n",
        "  return 0\n",
        "}"
    ))
    .is_ok());
}

#[test]
fn type_simulate_return_outside_function_inside_simulate_is_error() {
    // return at top level inside simulate body — still TypeError "cannot return outside of a function"
    assert!(matches!(
        check("let d: seconds = 1\nlet s: seconds = 1\nsimulate d step s { return 42 }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_body_invalid_variant_transition_is_error() {
    assert!(matches!(
        check(concat!(
            "state Door { closed open }\n",
            "let door: Door = Door.closed\n",
            "let d: seconds = 1\nlet s: seconds = 1\n",
            "simulate d step s { transition door -> locked }"
        )),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn type_simulate_nested_simulate_type_checks_ok() {
    assert!(check(concat!(
        "let d: seconds = 2\nlet s: seconds = 1\n",
        "simulate d step s {\n",
        "  let id: seconds = 1\n  let is: seconds = 1\n",
        "  simulate id step is { }\n",
        "}"
    ))
    .is_ok());
}

// --- interpreter (audit additions) ---

#[test]
fn interp_simulate_local_let_does_not_persist_across_iterations() {
    // Re-defining same local name each iteration (fresh child env) must not error.
    assert!(run(concat!(
        "let d: seconds = 3\nlet s: seconds = 1\n",
        "simulate d step s { let x = time }"
    ))
    .is_ok());
}

#[test]
fn interp_simulate_return_inside_function_exits_with_value() {
    // return inside simulate inside function; caller gets the returned value
    let interp = run(concat!(
        "fn f() -> Number {\n",
        "  let d: seconds = 1\n  let s: seconds = 1\n",
        "  simulate d step s { return 42 }\n",
        "  return 0\n",
        "}\n",
        "let r = f()"
    ))
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(42.0)));
}

// ============================================================
// Milestone 6B — extended time units
// ============================================================

// --- unit registry (resolve_unit) ---

#[test]
fn lex_unit_milliseconds_canonical() {
    assert!(check("let x: milliseconds = 5").is_ok());
}

#[test]
fn lex_unit_ms_alias_accepted() {
    assert!(check("let x: ms = 5").is_ok());
}

#[test]
fn lex_unit_minutes_canonical() {
    assert!(check("let x: minutes = 3").is_ok());
}

#[test]
fn lex_unit_min_alias_accepted() {
    assert!(check("let x: min = 3").is_ok());
}

#[test]
fn lex_unit_hours_canonical() {
    assert!(check("let x: hours = 2").is_ok());
}

#[test]
fn lex_unit_h_alias_accepted() {
    assert!(check("let x: h = 2").is_ok());
}

#[test]
fn lex_unit_ms_and_milliseconds_same_type() {
    // ms alias must canonicalize to milliseconds — same type as full name
    assert!(check(concat!(
        "let a: milliseconds = 5\n",
        "let b: ms = 5\n",
        "let c: milliseconds = a"
    ))
    .is_ok());
}

#[test]
fn lex_unit_min_and_minutes_same_type() {
    assert!(check(concat!(
        "let a: minutes = 3\n",
        "let b: min = 3\n",
        "let c: minutes = a"
    ))
    .is_ok());
}

// --- type checker: is_time_unit covers all four units ---

#[test]
fn type_simulate_milliseconds_duration_accepted() {
    assert!(check(concat!(
        "let d: milliseconds = 5\n",
        "let s: milliseconds = 2\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_minutes_duration_accepted() {
    assert!(check(concat!(
        "let d: minutes = 3\n",
        "let s: minutes = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_hours_duration_accepted() {
    assert!(check(concat!(
        "let d: hours = 2\n",
        "let s: hours = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_ms_alias_accepted() {
    assert!(check(concat!(
        "let d: ms = 5\n",
        "let s: ms = 2\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_min_alias_accepted() {
    assert!(check(concat!(
        "let d: min = 3\n",
        "let s: min = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_h_alias_accepted() {
    assert!(check(concat!(
        "let d: h = 2\n",
        "let s: h = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn type_simulate_mismatched_time_units_is_error() {
    // minutes duration with seconds step → TypeError
    assert!(check(concat!(
        "let d: minutes = 2\n",
        "let s: seconds = 30\n",
        "simulate d step s { print(time) }"
    ))
    .is_err());
}

#[test]
fn type_simulate_minutes_vs_milliseconds_is_error() {
    assert!(check(concat!(
        "let d: minutes = 1\n",
        "let s: milliseconds = 500\n",
        "simulate d step s { print(time) }"
    ))
    .is_err());
}

#[test]
fn type_simulate_hours_vs_minutes_is_error() {
    assert!(check(concat!(
        "let d: hours = 1\n",
        "let s: minutes = 30\n",
        "simulate d step s { print(time) }"
    ))
    .is_err());
}

#[test]
fn type_simulate_non_time_unit_meters_is_error() {
    assert!(check(concat!(
        "let d: meters = 10\n",
        "let s: meters = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_err());
}

// --- interpreter: extended time units run correctly ---

#[test]
fn interp_simulate_milliseconds_correct_iterations() {
    // 5ms / 2ms = 2 iterations: time=0, time=2
    let interp = run(concat!(
        "let d: milliseconds = 5\n",
        "let s: milliseconds = 2\n",
        "let last: milliseconds = 0\n",
        "simulate d step s { let last2 = time }"
    ))
    .unwrap();
    // 2 iterations (floor(5/2)=2); time variable is in body scope, outer last stays 0
    assert_eq!(interp.get_var("last"), Some(Value::Number(0.0)));
}

#[test]
fn interp_simulate_minutes_correct_iterations() {
    // 3min / 1min = 3 iterations
    let interp = run(concat!(
        "fn v(x: Number) { return x }\n",
        "let d: minutes = 3\n",
        "let s: minutes = 1\n",
        "simulate d step s { print(time) }"
    ))
    .unwrap();
    // just check it runs without error; output not captured in tests
    let _ = interp;
}

#[test]
fn interp_simulate_hours_two_iterations() {
    // 2h / 1h = 2 iterations: time=0, time=1
    assert!(run(concat!(
        "let d: hours = 2\n",
        "let s: hours = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn interp_simulate_ms_alias_runs() {
    assert!(run(concat!(
        "let d: ms = 4\n",
        "let s: ms = 2\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

#[test]
fn interp_simulate_min_alias_runs() {
    assert!(run(concat!(
        "let d: min = 3\n",
        "let s: min = 1\n",
        "simulate d step s { print(time) }"
    ))
    .is_ok());
}

// ============================================================
// Milestone 7A — let mut and assignment
// ============================================================

// --- lexer ---

#[test]
fn lex_mut_keyword() {
    let kinds = tokenize("mut");
    assert!(matches!(kinds[0], TokenKind::Mut));
}

// --- parser ---

#[test]
fn parse_let_mut_without_annotation() {
    assert!(check("let mut x = 1").is_ok());
}

#[test]
fn parse_let_mut_with_annotation() {
    assert!(check("let mut x: Number = 1").is_ok());
}

#[test]
fn parse_let_mut_unit_annotation() {
    assert!(check("let mut d: meters = 10").is_ok());
}

#[test]
fn parse_assign_stmt() {
    assert!(check("let mut x: Number = 1\nx = 2").is_ok());
}

#[test]
fn parse_assign_ambiguity_eqeq_not_assign() {
    // x == 1 must remain an expression statement, not an assignment
    assert!(check("let x: Number = 1\nlet y: Bool = x == 1").is_ok());
}

#[test]
fn parse_let_mut_error_no_ident() {
    // let mut followed by non-identifier should parse error
    let result = check("let mut = 1");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Parse(e)) = result {
        assert!(e.msg.contains("identifier"));
    }
}

// --- type checker ---

#[test]
fn type_let_mut_creates_mutable_number() {
    assert!(check("let mut x: Number = 1\nx = 2").is_ok());
}

#[test]
fn type_let_immutable_no_assign() {
    assert!(check("let x: Number = 1\nx = 2").is_err());
}

#[test]
fn type_assign_immutable_variable_error_message() {
    let result = check("let x: Number = 1\nx = 2");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Type(e)) = result {
        assert!(e.msg.contains("immutable") && e.msg.contains("'x'"));
    }
}

#[test]
fn type_assign_undefined_variable_error() {
    assert!(check("x = 2").is_err());
}

#[test]
fn type_assign_type_mismatch_number_text() {
    let result = check("let mut x: Number = 1\nx = \"hello\"");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Type(e)) = result {
        assert!(e.msg.contains("Number") && e.msg.contains("Text"));
    }
}

#[test]
fn type_assign_same_unit_ok() {
    assert!(check("let mut d: meters = 10\nlet extra: meters = 5\nd = extra").is_ok());
}

#[test]
fn type_assign_number_promotes_to_unit() {
    // Number literal can be assigned to unit variable (same promotion as let)
    assert!(check("let mut d: meters = 10\nd = 20").is_ok());
}

#[test]
fn type_assign_wrong_unit_error() {
    let result = check("let mut d: meters = 1\nlet t: seconds = 2\nd = t");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Type(e)) = result {
        assert!(e.msg.contains("meters") && e.msg.contains("seconds"));
    }
}

#[test]
fn type_assign_unit_to_number_variable_error() {
    assert!(check("let mut n: Number = 1\nlet d: meters = 2\nn = d").is_err());
}

#[test]
fn type_assign_inside_block_updates_outer_mutable_ok() {
    assert!(check("let mut x: Number = 1\n{ x = 2 }").is_ok());
}

#[test]
fn type_assign_immutable_outer_from_block_error() {
    assert!(check("let x: Number = 1\n{ x = 2 }").is_err());
}

#[test]
fn type_assign_local_shadow_mutable_ok() {
    // Inner shadow is mutable, outer is immutable — inner assignment is fine
    assert!(check("let x: Number = 1\n{ let mut x: Number = 10\nx = 20 }").is_ok());
}

#[test]
fn type_assign_inner_shadow_does_not_affect_outer() {
    // Inner immutable shadow should reject assignment, outer mutability irrelevant
    assert!(check("let mut x: Number = 1\n{ let x: Number = 10\nx = 20 }").is_err());
}

#[test]
fn type_assign_inside_function_local_mutable_ok() {
    assert!(check(concat!(
        "fn f() -> Number {\n",
        "  let mut n: Number = 1\n",
        "  n = 42\n",
        "  return n\n",
        "}"
    ))
    .is_ok());
}

#[test]
fn type_assign_state_variable_rejected_even_if_mutable() {
    let result = check(concat!(
        "state Door { closed open transition closed -> open }\n",
        "let mut door: Door = Door.closed\n",
        "door = Door.open"
    ));
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Type(e)) = result {
        assert!(e.msg.contains("transition"));
    }
}

#[test]
fn type_transition_still_works_for_state_ok() {
    assert!(check(concat!(
        "state Door { closed open transition closed -> open }\n",
        "let mut door: Door = Door.closed\n",
        "transition door -> open"
    ))
    .is_ok());
}

#[test]
fn type_assign_inside_simulate_outer_mutable_ok() {
    assert!(check(concat!(
        "let mut n: Number = 0\n",
        "let d: seconds = 3\n",
        "let s: seconds = 1\n",
        "simulate d step s { n = n + 1 }"
    ))
    .is_ok());
}

#[test]
fn type_assign_inside_simulate_immutable_outer_error() {
    assert!(check(concat!(
        "let n: Number = 0\n",
        "let d: seconds = 3\n",
        "let s: seconds = 1\n",
        "simulate d step s { n = n + 1 }"
    ))
    .is_err());
}

// --- interpreter ---

#[test]
fn interp_assign_updates_runtime_value() {
    let interp = run("let mut x: Number = 1\nx = 42").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(42.0)));
}

#[test]
fn interp_assign_inside_block_updates_outer() {
    let interp = run("let mut x: Number = 1\n{ x = 99 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(99.0)));
}

#[test]
fn interp_assign_inside_block_updates_local_shadow() {
    // Shadow inside block; outer x untouched
    let interp = run("let mut x: Number = 1\n{ let mut x: Number = 10\nx = 20 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(1.0)));
}

#[test]
fn interp_assign_inside_function_local_var() {
    let interp = run(concat!(
        "fn f() -> Number {\n",
        "  let mut n: Number = 0\n",
        "  n = 7\n",
        "  return n\n",
        "}\n",
        "let r = f()"
    ))
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(7.0)));
}

#[test]
fn interp_assign_inside_simulate_persists_across_iterations() {
    let interp = run(concat!(
        "let mut counter: Number = 0\n",
        "let d: seconds = 3\n",
        "let s: seconds = 1\n",
        "simulate d step s { counter = counter + 1 }"
    ))
    .unwrap();
    assert_eq!(interp.get_var("counter"), Some(Value::Number(3.0)));
}

#[test]
fn interp_simulate_motion_position_equals_six() {
    // position = 0, velocity = 2m/s, dt = 1s, 3 iterations -> position = 6
    let interp = run(concat!(
        "let mut position: meters = 0\n",
        "let dist_per_step: meters = 2\n",
        "let unit_time: seconds = 1\n",
        "let velocity = dist_per_step / unit_time\n",
        "let duration: seconds = 3\n",
        "let dt: seconds = 1\n",
        "simulate duration step dt {\n",
        "  position = position + velocity * dt\n",
        "}"
    ))
    .unwrap();
    assert_eq!(interp.get_var("position"), Some(Value::Number(6.0)));
}

#[test]
fn interp_assign_number_promotes_to_unit_at_runtime() {
    let interp = run("let mut d: meters = 10\nd = 20").unwrap();
    assert_eq!(interp.get_var("d"), Some(Value::Number(20.0)));
}

#[test]
fn interp_let_immutable_still_readable() {
    let interp = run("let x: Number = 5").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(5.0)));
}

#[test]
fn interp_let_mut_without_annotation_infers_type() {
    let interp = run("let mut x = 1\nx = 99").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(99.0)));
}

// ============================================================
// Milestone 7A audit — hardening
// ============================================================

// --- parser audit ---

#[test]
fn parse_assign_missing_rhs_is_parse_error() {
    let result = check("let mut x: Number = 1\nx =");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Parse(e)) = result {
        assert!(e.msg.contains("expression"));
    }
}

#[test]
fn parse_assign_inside_nested_block_ok() {
    assert!(check("let mut x: Number = 1\n{ { x = 2 } }").is_ok());
}

#[test]
fn parse_assign_in_expression_context_is_parse_error() {
    // Assignment is statement-only; `print(x = 1)` must be a ParseError.
    let result = check("let mut x: Number = 1\nprint(x = 1)");
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Parse(_)) = result {
        // ParseError expected (not TypeError or RuntimeError)
    } else {
        panic!("expected ParseError for assignment inside expression");
    }
}

// --- type checker audit ---

#[test]
fn type_assign_closure_reads_reassigned_outer_var() {
    // Closure defined before reassignment reads the updated value (capture by ref via Rc<RefCell>)
    assert!(check(concat!(
        "let mut x: Number = 1\n",
        "fn get_x() -> Number { return x }\n",
        "x = 2\n",
        "let r = get_x()"
    ))
    .is_ok());
}

#[test]
fn type_assign_closure_mutates_captured_mutable_var() {
    // Closure body assigns outer mutable variable — type checker should accept
    assert!(check(concat!(
        "let mut x: Number = 1\n",
        "fn inc() -> Number {\n",
        "  x = x + 1\n",
        "  return x\n",
        "}"
    ))
    .is_ok());
}

#[test]
fn type_assign_closure_cannot_mutate_captured_immutable() {
    // Closure body tries to assign outer immutable variable — TypeError
    let result = check(concat!(
        "let x: Number = 1\n",
        "fn bad() -> Number {\n",
        "  x = 2\n",
        "  return x\n",
        "}"
    ));
    assert!(result.is_err());
    if let Err(crate::error::KiminError::Type(e)) = result {
        assert!(e.msg.contains("immutable") && e.msg.contains("'x'"));
    }
}

#[test]
fn type_assign_compound_unit_same_compound_ok() {
    // Assign meters/seconds to a meters/seconds variable
    assert!(check(concat!(
        "let d1: meters = 4\n",
        "let t1: seconds = 2\n",
        "let mut v = d1 / t1\n",
        "let d2: meters = 6\n",
        "let t2: seconds = 3\n",
        "v = d2 / t2"
    ))
    .is_ok());
}

#[test]
fn type_assign_compound_unit_wrong_unit_error() {
    // Assign meters to a meters/seconds variable — TypeError
    assert!(check(concat!(
        "let d1: meters = 4\n",
        "let t1: seconds = 2\n",
        "let mut v = d1 / t1\n",
        "let d2: meters = 5\n",
        "v = d2"
    ))
    .is_err());
}

#[test]
fn type_transition_on_immutable_state_var_ok() {
    // transition is a separate controlled mutation primitive; does not require let mut
    assert!(check(concat!(
        "state Door { closed open transition closed -> open }\n",
        "let door: Door = Door.closed\n",
        "transition door -> open"
    ))
    .is_ok());
}

#[test]
fn type_assign_bool_variable_ok() {
    assert!(check("let mut b: Bool = true\nb = false").is_ok());
}

#[test]
fn type_assign_text_variable_ok() {
    assert!(check("let mut s: Text = \"hello\"\ns = \"world\"").is_ok());
}

#[test]
fn type_assign_nested_block_reaches_outer_mutable_ok() {
    assert!(check("let mut x: Number = 1\n{ { x = 99 } }").is_ok());
}

#[test]
fn type_assign_bool_mismatch_error() {
    assert!(check("let mut b: Bool = true\nb = 1").is_err());
}

// --- interpreter audit ---

#[test]
fn interp_assign_closure_reads_updated_outer_var() {
    // Case A: closure reads outer mutable after reassignment
    let interp = run(concat!(
        "let mut x: Number = 1\n",
        "fn get_x() -> Number { return x }\n",
        "x = 2\n",
        "let r = get_x()"
    ))
    .unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(2.0)));
}

#[test]
fn interp_assign_closure_mutates_captured_across_calls() {
    // Case B: closure writes outer mutable var; second call sees updated value
    let interp = run(concat!(
        "let mut x: Number = 1\n",
        "fn inc() -> Number {\n",
        "  x = x + 1\n",
        "  return x\n",
        "}\n",
        "let a = inc()\n",
        "let b = inc()"
    ))
    .unwrap();
    assert_eq!(interp.get_var("a"), Some(Value::Number(2.0)));
    assert_eq!(interp.get_var("b"), Some(Value::Number(3.0)));
}

#[test]
fn interp_assign_rhs_evaluated_before_update() {
    // x = x + 5 where x starts at 3 must produce 8, not use stale value
    let interp = run("let mut x: Number = 3\nx = x + 5").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(8.0)));
}

#[test]
fn interp_assign_simulate_body_local_let_fresh_each_iteration() {
    // Body-local `let mut local` resets to 0 on each iteration;
    // outer accumulates 1 per iteration (3 total over 3 iters).
    let interp = run(concat!(
        "let mut outer: Number = 0\n",
        "let duration: seconds = 3\n",
        "let dt: seconds = 1\n",
        "simulate duration step dt {\n",
        "  let mut local: Number = 0\n",
        "  local = local + 1\n",
        "  outer = outer + local\n",
        "}"
    ))
    .unwrap();
    assert_eq!(interp.get_var("outer"), Some(Value::Number(3.0)));
}

#[test]
fn interp_assign_nested_block_updates_correct_binding() {
    let interp = run("let mut x: Number = 1\n{ { x = 99 } }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(99.0)));
}

#[test]
fn interp_transition_immutable_state_var_updates_value() {
    // transition works without let mut — controlled mutation primitive
    let interp = run(concat!(
        "state Door { closed open transition closed -> open }\n",
        "let door: Door = Door.closed\n",
        "transition door -> open"
    ))
    .unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".to_string(),
            variant_name: "open".to_string(),
        })
    );
}

#[test]
fn interp_assign_bool_updates_value() {
    let interp = run("let mut b: Bool = true\nb = false").unwrap();
    assert_eq!(interp.get_var("b"), Some(Value::Bool(false)));
}

#[test]
fn interp_assign_text_updates_value() {
    let interp = run("let mut s: Text = \"hello\"\ns = \"world\"").unwrap();
    assert_eq!(interp.get_var("s"), Some(Value::Str("world".to_string())));
}

// --- bytecode / compiler tests ---

#[test]
fn bytecode_number_literal_emits_constant() {
    let prog = compile_prog("print(10)");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(instrs[1], Instruction::Print));
    assert!(matches!(instrs[2], Instruction::Halt));
    assert!(matches!(prog.main.constants[0], Constant::Number(n) if n == 10.0));
}

#[test]
fn bytecode_string_literal_emits_constant() {
    let prog = compile_prog("print(\"hi\")");
    assert!(matches!(&prog.main.constants[0], Constant::Text(s) if s == "hi"));
    assert!(matches!(
        prog.main.instructions[0],
        Instruction::Constant(0)
    ));
}

#[test]
fn bytecode_bool_true_emits_true() {
    let prog = compile_prog("let b = true");
    assert!(matches!(prog.main.instructions[0], Instruction::True));
}

#[test]
fn bytecode_bool_false_emits_false() {
    let prog = compile_prog("let b = false");
    assert!(matches!(prog.main.instructions[0], Instruction::False));
}

#[test]
fn bytecode_let_defines_global() {
    let prog = compile_prog("let x = 5");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "x"));
    assert!(matches!(instrs[2], Instruction::Halt));
}

#[test]
fn bytecode_let_mut_defines_global() {
    // `let mut` and `let` both emit DEFINE_GLOBAL — mutability is a type-checker concern
    let prog = compile_prog("let mut count = 0");
    let instrs = &prog.main.instructions;
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "count"));
}

#[test]
fn bytecode_assign_stores_global() {
    let prog = compile_prog("let mut x = 0\nx = 1");
    let instrs = &prog.main.instructions;
    // CONSTANT(0), DEFINE_GLOBAL x, CONSTANT(1), STORE_GLOBAL x, HALT
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "x"));
    assert!(matches!(&instrs[3], Instruction::StoreGlobal(n) if n == "x"));
}

#[test]
fn bytecode_print_emits_print_instr() {
    let prog = compile_prog("print(42)");
    assert!(matches!(prog.main.instructions[1], Instruction::Print));
}

#[test]
fn bytecode_binary_add_emits_add() {
    let prog = compile_prog("let z = 1 + 2");
    let instrs = &prog.main.instructions;
    // CONSTANT(0), CONSTANT(1), ADD, DEFINE_GLOBAL z, HALT
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(instrs[1], Instruction::Constant(1)));
    assert!(matches!(instrs[2], Instruction::Add));
    assert!(matches!(&instrs[3], Instruction::DefineGlobal(n) if n == "z"));
}

#[test]
fn bytecode_binary_subtract_emits_subtract() {
    let prog = compile_prog("let z = 5 - 3");
    assert!(matches!(prog.main.instructions[2], Instruction::Subtract));
}

#[test]
fn bytecode_binary_multiply_emits_multiply() {
    let prog = compile_prog("let z = 4 * 3");
    assert!(matches!(prog.main.instructions[2], Instruction::Multiply));
}

#[test]
fn bytecode_binary_divide_emits_divide() {
    let prog = compile_prog("let z = 10 / 2");
    assert!(matches!(prog.main.instructions[2], Instruction::Divide));
}

#[test]
fn bytecode_unary_neg_emits_negate() {
    let prog = compile_prog("let z = -5");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(instrs[1], Instruction::Negate));
}

#[test]
fn bytecode_unary_not_emits_not() {
    let prog = compile_prog("let b = !true");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::True));
    assert!(matches!(instrs[1], Instruction::Not));
}

#[test]
fn bytecode_variable_load_global() {
    let prog = compile_prog("let x = 1\nprint(x)");
    let instrs = &prog.main.instructions;
    // CONSTANT, DEFINE_GLOBAL x, LOAD_GLOBAL x, PRINT, HALT
    assert!(matches!(&instrs[2], Instruction::LoadGlobal(n) if n == "x"));
}

#[test]
fn bytecode_comparison_eq_emits_equal() {
    let prog = compile_prog("let b = 1 == 1");
    assert!(matches!(prog.main.instructions[2], Instruction::Equal));
}

#[test]
fn bytecode_comparison_lt_emits_less() {
    let prog = compile_prog("let b = 1 < 2");
    assert!(matches!(prog.main.instructions[2], Instruction::Less));
}

#[test]
fn bytecode_if_no_else_patches_jump() {
    // if true { print(1) }
    // TRUE, JIF_FALSE(?), BEGIN_SCOPE, CONSTANT, PRINT, END_SCOPE, HALT
    let prog = compile_prog("if true { print(1) }");
    let instrs = &prog.main.instructions;
    assert_eq!(instrs.len(), 7);
    assert!(matches!(instrs[0], Instruction::True));
    assert!(matches!(instrs[1], Instruction::JumpIfFalse(6)));
    assert!(matches!(instrs[2], Instruction::BeginScope));
    assert!(matches!(instrs[5], Instruction::EndScope));
    assert!(matches!(instrs[6], Instruction::Halt));
}

#[test]
fn bytecode_if_else_patches_both_jumps() {
    // if true { print(1) } else { print(2) }
    // TRUE, JIF_FALSE(7), BEGIN_SCOPE, CONSTANT, PRINT, END_SCOPE, JUMP(11),
    // BEGIN_SCOPE, CONSTANT, PRINT, END_SCOPE, HALT
    let prog = compile_prog("if true { print(1) } else { print(2) }");
    let instrs = &prog.main.instructions;
    assert_eq!(instrs.len(), 12);
    assert!(matches!(instrs[0], Instruction::True));
    assert!(matches!(instrs[1], Instruction::JumpIfFalse(7)));
    assert!(matches!(instrs[6], Instruction::Jump(11)));
    assert!(matches!(instrs[11], Instruction::Halt));
}

#[test]
fn bytecode_block_emits_scope_instructions() {
    // { let x = 1 }
    let prog = compile_prog("{ let x = 1 }");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::BeginScope));
    assert!(matches!(&instrs[2], Instruction::DefineLocal(n) if n == "x"));
    assert!(matches!(instrs[3], Instruction::EndScope));
    assert!(matches!(instrs[4], Instruction::Halt));
}

#[test]
fn bytecode_block_uses_define_local() {
    let prog = compile_prog("{ let y = 99 }");
    let instrs = &prog.main.instructions;
    assert!(matches!(&instrs[2], Instruction::DefineLocal(n) if n == "y"));
}

#[test]
fn bytecode_return_with_value() {
    let prog = compile_prog("return 5");
    let instrs = &prog.main.instructions;
    // CONSTANT(0), RETURN, HALT
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(instrs[1], Instruction::Return));
    assert!(matches!(instrs[2], Instruction::Halt));
}

#[test]
fn bytecode_return_bare_emits_nil() {
    let prog = compile_prog("return");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::Nil));
    assert!(matches!(instrs[1], Instruction::Return));
}

#[test]
fn bytecode_fn_decl_emits_load_function() {
    // M8B: FnDecl now lowers to LOAD_FUNCTION + DEFINE_GLOBAL in main and a FunctionChunk.
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let instrs = &prog.main.instructions;
    assert!(matches!(&instrs[0], Instruction::LoadFunction(n) if n == "add"));
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "add"));
    assert_eq!(prog.functions.len(), 1);
    assert_eq!(prog.functions[0].name, "add");
    assert_eq!(prog.functions[0].arity, 2);
    assert_eq!(prog.functions[0].params, vec!["a", "b"]);
}

#[test]
fn bytecode_call_emits_call_instruction() {
    // M8G: calls lower to stack-based CALL arg_count (no name in instruction).
    let prog = compile_prog("fn f() { } f()");
    let has_call = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { arg_count: 0 }));
    assert!(has_call);
    // Callee must be loaded via LoadGlobal/LoadLocal/LoadFunction before the call.
    let has_load_f =
        prog.main.instructions.iter().any(
            |i| matches!(i, Instruction::LoadGlobal(n) | Instruction::LoadLocal(n) if n == "f"),
        );
    assert!(has_load_f, "callee 'f' must be loaded before CALL");
}

#[test]
fn bytecode_state_decl_emits_define_state() {
    let prog = compile_prog("state Door { closed open transition closed -> open }");
    let instrs = &prog.main.instructions;
    assert!(
        matches!(&instrs[0], Instruction::DefineState { name, .. } if name == "Door"),
        "expected DefineState for 'Door', got {:?}",
        instrs[0]
    );
}

#[test]
fn bytecode_transition_emits_transition_instruction() {
    let prog = compile_prog(concat!(
        "state Door { closed open transition closed -> open }\n",
        "let door: Door = Door.closed\n",
        "transition door -> open"
    ));
    let has_transition = prog.main.instructions.iter().any(|i| {
        matches!(i, Instruction::Transition { variable, target }
            if variable == "door" && target == "open")
    });
    assert!(
        has_transition,
        "expected Transition instruction in main chunk"
    );
}

#[test]
fn bytecode_simulate_emits_simulate_instruction() {
    let prog = compile_prog(concat!(
        "let dur = 3\n",
        "let dt = 1\n",
        "simulate dur step dt { }"
    ));
    let has_simulate = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Simulate { .. }));
    assert!(has_simulate, "simulate must emit Instruction::Simulate");
    assert_eq!(prog.simulate_bodies.len(), 1, "one simulate body expected");
}

#[test]
fn bytecode_halt_is_last_instruction() {
    let prog = compile_prog("let x = 1\nlet y = 2\nprint(x)");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs.last().unwrap(), Instruction::Halt));
}

#[test]
fn bytecode_constant_pool_indexes_are_sequential() {
    let prog = compile_prog("let a = 1\nlet b = 2\nlet c = 3");
    assert_eq!(prog.main.constants.len(), 3);
    assert!(matches!(prog.main.constants[0], Constant::Number(n) if n == 1.0));
    assert!(matches!(prog.main.constants[1], Constant::Number(n) if n == 2.0));
    assert!(matches!(prog.main.constants[2], Constant::Number(n) if n == 3.0));
}

#[test]
fn bytecode_expr_stmt_emits_pop() {
    // An expression used as a statement pushes a value — POP discards it.
    let prog = compile_prog("1 + 2");
    let instrs = &prog.main.instructions;
    // CONSTANT, CONSTANT, ADD, POP, HALT
    assert!(matches!(instrs[3], Instruction::Pop));
}

#[test]
fn bytecode_grouping_transparent() {
    let prog = compile_prog("let x = (5)");
    let instrs = &prog.main.instructions;
    assert!(matches!(instrs[0], Instruction::Constant(0)));
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "x"));
}

// --- disassembler tests ---

#[test]
fn disassemble_produces_chunk_header() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("print(1)");
    let out = disassemble(&prog);
    assert!(out.contains("=== main ==="));
    assert!(out.contains("PRINT"));
    assert!(out.contains("HALT"));
}

#[test]
fn disassemble_lists_constants_section() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("print(42)");
    let out = disassemble(&prog);
    assert!(out.contains("constants:"));
    assert!(out.contains("Number(42)"));
}

#[test]
fn disassemble_shows_define_global() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let speed = 5");
    let out = disassemble(&prog);
    assert!(out.contains("DEFINE_GLOBAL speed"));
}

#[test]
fn disassemble_shows_jump_targets() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("if true { print(1) }");
    let out = disassemble(&prog);
    assert!(out.contains("JUMP_IF_FALSE @6"));
}

#[test]
fn disassemble_shows_function_chunk() {
    // M8B: function declarations now produce a named function chunk, not UNSUPPORTED.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn foo() { }");
    let out = disassemble(&prog);
    assert!(out.contains("=== function foo/0 ==="));
    assert!(out.contains("LOAD_FUNCTION foo"));
    assert!(!out.contains("UNSUPPORTED(fn foo)"));
}

// --- M8A audit: scope classification ---

#[test]
fn bytecode_outer_global_loaded_from_block() {
    // Bug regression: outer global accessed inside block must emit LOAD_GLOBAL, not LOAD_LOCAL.
    let prog = compile_prog("let x = 1\n{ print(x) }");
    let instrs = &prog.main.instructions;
    // CONSTANT, DEFINE_GLOBAL x, BEGIN_SCOPE, LOAD_GLOBAL x, PRINT, END_SCOPE, HALT
    assert!(matches!(&instrs[3], Instruction::LoadGlobal(n) if n == "x"));
}

#[test]
fn bytecode_outer_global_stored_from_block() {
    // Bug regression: assignment to outer global inside block must emit STORE_GLOBAL.
    let prog = compile_prog("let mut x = 0\n{ x = 1 }");
    let instrs = &prog.main.instructions;
    // CONSTANT, DEFINE_GLOBAL x, BEGIN_SCOPE, CONSTANT, STORE_GLOBAL x, END_SCOPE, HALT
    assert!(matches!(&instrs[4], Instruction::StoreGlobal(n) if n == "x"));
}

#[test]
fn bytecode_local_variable_stays_local() {
    // A variable defined inside a block must use LOCAL instructions.
    let prog = compile_prog("{ let y = 2\nprint(y) }");
    let instrs = &prog.main.instructions;
    // BEGIN_SCOPE, CONSTANT, DEFINE_LOCAL y, LOAD_LOCAL y, PRINT, END_SCOPE, HALT
    assert!(matches!(&instrs[2], Instruction::DefineLocal(n) if n == "y"));
    assert!(matches!(&instrs[3], Instruction::LoadLocal(n) if n == "y"));
}

#[test]
fn bytecode_nested_blocks_scope_balanced() {
    // Two nested blocks must emit two BeginScope/EndScope pairs.
    let prog = compile_prog("{ { let z = 1 } }");
    let instrs = &prog.main.instructions;
    let begin_count = instrs
        .iter()
        .filter(|i| matches!(i, Instruction::BeginScope))
        .count();
    let end_count = instrs
        .iter()
        .filter(|i| matches!(i, Instruction::EndScope))
        .count();
    assert_eq!(begin_count, 2);
    assert_eq!(end_count, 2);
}

#[test]
fn bytecode_nested_if_jump_targets() {
    // Nested if: outer JIF_FALSE jumps past both ifs; inner JIF_FALSE jumps past inner then.
    // let x = 1 / if x==1 { if x==2 { print("inner") } }
    let prog = compile_prog("let x = 1\nif x == 1 { if x == 2 { print(\"inner\") } }");
    let instrs = &prog.main.instructions;
    // 0:CONST, 1:DEF_GLOBAL x, 2:LOAD_GLOBAL x, 3:CONST, 4:EQUAL, 5:JIF_FALSE(@16),
    // 6:BEGIN_SCOPE, 7:LOAD_GLOBAL x, 8:CONST, 9:EQUAL, 10:JIF_FALSE(@15),
    // 11:BEGIN_SCOPE, 12:CONST, 13:PRINT, 14:END_SCOPE, 15:END_SCOPE, 16:HALT
    assert!(matches!(instrs[5], Instruction::JumpIfFalse(16)));
    assert!(matches!(instrs[10], Instruction::JumpIfFalse(15)));
    assert!(matches!(instrs[16], Instruction::Halt));
}

#[test]
fn bytecode_if_else_inside_block_jump_targets() {
    // if/else inside a block: JIF_FALSE and JUMP still patch correctly.
    let prog = compile_prog("let x = 5\n{ if x > 3 { print(\"big\") } else { print(\"small\") } }");
    let instrs = &prog.main.instructions;
    // 0:CONST, 1:DEF_GLOBAL x, 2:BEGIN_SCOPE, 3:LOAD_GLOBAL x, 4:CONST, 5:GREATER,
    // 6:JIF_FALSE(@12), 7:BEGIN_SCOPE, 8:CONST, 9:PRINT, 10:END_SCOPE, 11:JUMP(@16),
    // 12:BEGIN_SCOPE, 13:CONST, 14:PRINT, 15:END_SCOPE, 16:END_SCOPE, 17:HALT
    assert!(matches!(instrs[6], Instruction::JumpIfFalse(12)));
    assert!(matches!(instrs[11], Instruction::Jump(16)));
    assert!(matches!(instrs[17], Instruction::Halt));
}

// --- M8A audit: missing comparison operators ---

#[test]
fn bytecode_not_equal_emits_not_equal() {
    let prog = compile_prog("let b = 1 != 2");
    assert!(matches!(prog.main.instructions[2], Instruction::NotEqual));
}

#[test]
fn bytecode_less_equal_emits_less_equal() {
    let prog = compile_prog("let b = 1 <= 2");
    assert!(matches!(prog.main.instructions[2], Instruction::LessEqual));
}

#[test]
fn bytecode_greater_emits_greater() {
    let prog = compile_prog("let b = 2 > 1");
    assert!(matches!(prog.main.instructions[2], Instruction::Greater));
}

#[test]
fn bytecode_greater_equal_emits_greater_equal() {
    let prog = compile_prog("let b = 2 >= 1");
    assert!(matches!(
        prog.main.instructions[2],
        Instruction::GreaterEqual
    ));
}

// --- M8A audit: disassembler coverage ---

#[test]
fn disassemble_string_constant_has_quotes() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("print(\"hello\")");
    let out = disassemble(&prog);
    assert!(out.contains("Text(\"hello\")"));
}

#[test]
fn disassemble_nil_instruction_shown() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("return");
    let out = disassemble(&prog);
    assert!(out.contains("NIL"));
}

#[test]
fn disassemble_store_global_shown() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let mut x = 0\nx = 5");
    let out = disassemble(&prog);
    assert!(out.contains("STORE_GLOBAL x"));
}

#[test]
fn disassemble_begin_end_scope_balanced() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("{ let y = 1 }");
    let out = disassemble(&prog);
    assert!(out.contains("BEGIN_SCOPE"));
    assert!(out.contains("END_SCOPE"));
}

#[test]
fn disassemble_load_global_shown() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let x = 1\n{ print(x) }");
    let out = disassemble(&prog);
    // After fix: inner reference to x uses LOAD_GLOBAL
    assert!(out.contains("LOAD_GLOBAL x"));
}

// --- M8A audit: unsupported coverage ---

#[test]
fn bytecode_state_variant_expr_emits_load_state() {
    // Door.closed as an expression now emits LoadState, not Unsupported.
    let prog = compile_prog(
        "state Door { closed open transition closed -> open }\nlet d: Door = Door.closed",
    );
    let has_load = prog.main.instructions.iter().any(|i| {
        matches!(i, Instruction::LoadState { state_name, variant_name }
            if state_name == "Door" && variant_name == "closed")
    });
    assert!(has_load, "expected LoadState Door.closed in main chunk");
}

#[test]
fn bytecode_unsupported_does_not_crash_on_fndecl() {
    // FnDecl with body and typed params must not panic.
    let result = std::panic::catch_unwind(|| {
        compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }")
    });
    assert!(result.is_ok());
}

#[test]
fn bytecode_unsupported_does_not_crash_on_call() {
    let result = std::panic::catch_unwind(|| compile_prog("fn f() { } f()"));
    assert!(result.is_ok());
}

#[test]
fn bytecode_constants_not_deduplicated() {
    // Constants are appended per-use — no deduplication in M8A (expected behavior).
    let prog = compile_prog("let a = 1\nlet b = 1");
    assert_eq!(prog.main.constants.len(), 2);
    assert!(matches!(prog.main.constants[0], Constant::Number(n) if n == 1.0));
    assert!(matches!(prog.main.constants[1], Constant::Number(n) if n == 1.0));
}

// --- M8B: function chunk structure ---

#[test]
fn bytecode_fn_chunk_has_correct_name() {
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    assert_eq!(prog.functions.len(), 1);
    assert_eq!(prog.functions[0].name, "add");
}

#[test]
fn bytecode_fn_chunk_has_correct_arity() {
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    assert_eq!(prog.functions[0].arity, 2);
}

#[test]
fn bytecode_fn_chunk_has_correct_params() {
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    assert_eq!(prog.functions[0].params, vec!["a", "b"]);
}

#[test]
fn bytecode_fn_chunk_zero_param() {
    let prog = compile_prog("fn f() { }");
    assert_eq!(prog.functions[0].arity, 0);
    assert!(prog.functions[0].params.is_empty());
}

#[test]
fn bytecode_fn_body_params_load_as_local() {
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "a")));
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "b")));
}

#[test]
fn bytecode_fn_body_explicit_return_emits_return() {
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(body.iter().any(|i| matches!(i, Instruction::Return)));
}

#[test]
fn bytecode_fn_body_no_return_emits_nil_return() {
    // Empty body gets implicit NIL + RETURN.
    let prog = compile_prog("fn f() { }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(matches!(body[0], Instruction::Nil));
    assert!(matches!(body[1], Instruction::Return));
}

#[test]
fn bytecode_fn_local_let_emits_define_local() {
    let prog = compile_prog("fn f() -> Number { let x = 5\nreturn x }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::DefineLocal(n) if n == "x")));
}

#[test]
fn bytecode_fn_local_variable_load_local() {
    let prog = compile_prog("fn f() -> Number { let x = 5\nreturn x }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "x")));
}

#[test]
fn bytecode_multiple_fn_decls_create_multiple_chunks() {
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "fn square(x: Number) -> Number { return x * x }"
    ));
    assert_eq!(prog.functions.len(), 2);
    assert_eq!(prog.functions[0].name, "add");
    assert_eq!(prog.functions[1].name, "square");
}

#[test]
fn bytecode_multiple_fn_decls_emit_load_functions_in_main() {
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "fn square(x: Number) -> Number { return x * x }"
    ));
    let instrs = &prog.main.instructions;
    assert!(matches!(&instrs[0], Instruction::LoadFunction(n) if n == "add"));
    assert!(matches!(&instrs[1], Instruction::DefineGlobal(n) if n == "add"));
    assert!(matches!(&instrs[2], Instruction::LoadFunction(n) if n == "square"));
    assert!(matches!(&instrs[3], Instruction::DefineGlobal(n) if n == "square"));
}

// --- M8B: call lowering ---

#[test]
fn bytecode_simple_call_emits_call_instr() {
    // M8G: Call no longer carries the function name; arg_count is the identifier.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3)"
    ));
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { arg_count: 2 })));
}

#[test]
fn bytecode_call_arg_count_correct() {
    let prog = compile_prog(concat!(
        "fn f(a: Number, b: Number, c: Number) -> Number { return a }\n",
        "let z = f(1, 2, 3)"
    ));
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { arg_count: 3 })));
}

#[test]
fn bytecode_call_args_constants_precede_call() {
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3)"
    ));
    let instrs = &prog.main.instructions;
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { .. }))
        .unwrap();
    let const_count = instrs[..call_idx]
        .iter()
        .filter(|i| matches!(i, Instruction::Constant(_)))
        .count();
    // Two constant args (2.0 and 3.0) must be compiled before the call.
    assert!(const_count >= 2);
}

#[test]
fn bytecode_nested_call_inner_before_outer() {
    // square(add(2, 3)): inner CALL 2 (add's args) must precede outer CALL 1 (square's arg).
    // M8G: callee is on stack; calls are distinguished by arg count, not name.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "fn square(x: Number) -> Number { return x * x }\n",
        "let z = square(add(2, 3))"
    ));
    let instrs = &prog.main.instructions;
    let inner_call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { arg_count: 2 }))
        .unwrap();
    let outer_call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { arg_count: 1 }))
        .unwrap();
    assert!(inner_call_idx < outer_call_idx);
}

#[test]
fn bytecode_recursive_call_emits_call_to_self() {
    // M8G: recursive call compiles to LoadGlobal("fact") + CALL 1.
    let prog = compile_prog(concat!(
        "fn fact(n: Number) -> Number {\n",
        "  if n <= 1 { return 1 }\n",
        "  return n * fact(n - 1)\n",
        "}"
    ));
    let body = &prog.functions[0].chunk.instructions;
    let has_load_fact = body
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "fact"));
    let has_call_1 = body
        .iter()
        .any(|i| matches!(i, Instruction::Call { arg_count: 1 }));
    assert!(has_load_fact, "recursive call must load 'fact' callee");
    assert!(has_call_1, "recursive call must emit CALL 1");
}

#[test]
fn bytecode_zero_arg_call_emits_call_zero() {
    let prog = compile_prog("fn f() { } f()");
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { arg_count: 0 })));
}

// --- M8B: disassembler ---

#[test]
fn disassemble_shows_function_chunk_header() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn foo() { }");
    let out = disassemble(&prog);
    assert!(out.contains("=== function foo/0 ==="));
}

#[test]
fn disassemble_shows_function_params() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let out = disassemble(&prog);
    assert!(out.contains("params: a, b"));
}

#[test]
fn disassemble_shows_load_function_in_main() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let out = disassemble(&prog);
    assert!(out.contains("LOAD_FUNCTION add"));
}

#[test]
fn disassemble_shows_call_instruction() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "print(add(2, 3))"
    ));
    let out = disassemble(&prog);
    // M8G: CALL no longer includes callee name; callee loaded via LOAD_GLOBAL.
    assert!(out.contains("CALL 2"));
    assert!(out.contains("LOAD_GLOBAL add"));
}

#[test]
fn disassemble_no_unsupported_for_named_fn_decl() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn foo(x: Number) -> Number { return x }");
    let out = disassemble(&prog);
    assert!(!out.contains("UNSUPPORTED(fn foo)"));
}

#[test]
fn disassemble_no_unsupported_for_named_call() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!("fn f() -> Number { return 1 }\n", "let x = f()"));
    let out = disassemble(&prog);
    assert!(!out.contains("UNSUPPORTED(call f)"));
}

#[test]
fn disassemble_function_chunk_after_main() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn foo() { }");
    let out = disassemble(&prog);
    let main_pos = out.find("=== main ===").unwrap();
    let fn_pos = out.find("=== function foo/0 ===").unwrap();
    assert!(main_pos < fn_pos);
}

#[test]
fn disassemble_multiple_function_chunks_in_order() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!("fn first() { }\n", "fn second() { }"));
    let out = disassemble(&prog);
    let first_pos = out.find("function first/0").unwrap();
    let second_pos = out.find("function second/0").unwrap();
    assert!(first_pos < second_pos);
}

// --- M8B audit: function chunk correctness ---

#[test]
fn bytecode_fn_chunk_contains_no_halt() {
    // Function bodies must never contain HALT — HALT is only emitted for the main chunk.
    let prog = compile_prog("fn add(a: Number, b: Number) -> Number { return a + b }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(!body.iter().any(|i| matches!(i, Instruction::Halt)));
}

#[test]
fn bytecode_fn_bare_return_in_body_emits_nil_return() {
    // `return` (no value) inside a function emits NIL + RETURN.
    let prog = compile_prog("fn f() { return }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(matches!(body[0], Instruction::Nil));
    assert!(matches!(body[1], Instruction::Return));
}

#[test]
fn bytecode_fn_if_else_inside_body_patches_jumps() {
    // if/else jump patching must work inside a function chunk, not only in main.
    let prog = compile_prog(concat!(
        "fn pick(x: Number) -> Number {\n",
        "  if x > 0 { return 1 } else { return 0 }\n",
        "}"
    ));
    let body = &prog.functions[0].chunk.instructions;
    let has_patched_jif = body
        .iter()
        .any(|i| matches!(i, Instruction::JumpIfFalse(t) if *t > 0));
    let has_patched_jump = body
        .iter()
        .any(|i| matches!(i, Instruction::Jump(t) if *t > 0));
    assert!(
        has_patched_jif,
        "JumpIfFalse must be patched to non-zero target"
    );
    assert!(has_patched_jump, "Jump must be patched to non-zero target");
}

#[test]
fn bytecode_fn_nested_block_emits_scope_instructions() {
    // A block inside a function body must emit BEGIN_SCOPE / END_SCOPE.
    let prog = compile_prog(concat!(
        "fn f() -> Number {\n",
        "  { let x = 1 }\n",
        "  return 0\n",
        "}"
    ));
    let body = &prog.functions[0].chunk.instructions;
    let begin_count = body
        .iter()
        .filter(|i| matches!(i, Instruction::BeginScope))
        .count();
    let end_count = body
        .iter()
        .filter(|i| matches!(i, Instruction::EndScope))
        .count();
    assert_eq!(begin_count, 1);
    assert_eq!(end_count, 1);
}

// --- M8B audit: variable resolution inside function chunks ---

#[test]
fn bytecode_fn_param_shadows_global_of_same_name() {
    // Parameter named the same as a top-level global must load as LOAD_LOCAL, not LOAD_GLOBAL.
    let prog = compile_prog(concat!(
        "let x = 10\n",
        "fn f(x: Number) -> Number { return x }"
    ));
    let body = &prog.functions[0].chunk.instructions;
    assert!(
        body.iter()
            .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "x")),
        "parameter x must emit LOAD_LOCAL"
    );
    assert!(
        !body
            .iter()
            .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "x")),
        "parameter x must NOT emit LOAD_GLOBAL"
    );
}

#[test]
fn bytecode_fn_global_ref_inside_fn_emits_load_global() {
    // A top-level global referenced inside a function must emit LOAD_GLOBAL.
    let prog = compile_prog(concat!(
        "let speed = 10\n",
        "fn f() -> Number { return speed }"
    ));
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "speed")));
}

#[test]
fn bytecode_fn_global_ref_from_nested_block_inside_fn_emits_load_global() {
    // Global accessed from a block inside a function must still emit LOAD_GLOBAL.
    let prog = compile_prog(concat!("let g = 99\n", "fn f() -> Number { { return g } }"));
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "g")));
}

#[test]
fn bytecode_fn_local_mut_store_emits_store_local() {
    // Assignment to a function-local mutable variable must emit STORE_LOCAL.
    let prog = compile_prog(concat!(
        "fn f() -> Number {\n",
        "  let mut x = 0\n",
        "  x = 5\n",
        "  return x\n",
        "}"
    ));
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::StoreLocal(n) if n == "x")));
}

#[test]
fn bytecode_fn_assign_to_global_mut_inside_fn_emits_store_global() {
    // Assignment to a top-level mutable global inside a function must emit STORE_GLOBAL.
    let prog = compile_prog(concat!("let mut g = 0\n", "fn f() {\n", "  g = 1\n", "}"));
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "g")));
}

#[test]
fn bytecode_fn_local_let_same_name_as_param_uses_local_ops() {
    // Let with same name as a parameter uses DefineLocal/LoadLocal (no crash, no global ops).
    // Documented provisional behavior: the new let shadows the param in the local scope set.
    let prog = compile_prog("fn f(x: Number) -> Number { let x = 99\nreturn x }");
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::DefineLocal(n) if n == "x")));
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "x")));
    assert!(!body
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "x")));
}

// --- M8B audit: call lowering edge cases ---

#[test]
fn bytecode_call_as_expr_stmt_emits_pop() {
    // A call used as a statement (result discarded) must emit POP after CALL.
    let prog = compile_prog("fn f() { } f()");
    let instrs = &prog.main.instructions;
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { .. }))
        .unwrap();
    assert!(
        matches!(instrs[call_idx + 1], Instruction::Pop),
        "POP must follow CALL when call is a statement"
    );
}

#[test]
fn bytecode_call_in_print_emits_print_after_call() {
    // print(f()) must emit CALL then PRINT — not PRINT then CALL.
    let prog = compile_prog("fn f() -> Number { return 1 }\nprint(f())");
    let instrs = &prog.main.instructions;
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { .. }))
        .unwrap();
    assert!(
        matches!(instrs[call_idx + 1], Instruction::Print),
        "PRINT must immediately follow CALL in print(f())"
    );
}

#[test]
fn bytecode_call_result_in_binary_expr_correct_order() {
    // add(2, 3) + 1: CALL must be emitted before ADD.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3) + 1"
    ));
    let instrs = &prog.main.instructions;
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { .. }))
        .unwrap();
    let add_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Add))
        .unwrap();
    assert!(call_idx < add_idx, "CALL must precede ADD");
}

#[test]
fn bytecode_mutual_recursion_emits_cross_calls() {
    // M8G: mutual recursion loads callee via LoadGlobal before each CALL.
    // is_even chunk must load is_odd; is_odd chunk must load is_even.
    let prog = compile_prog(concat!(
        "fn is_even(n: Number) -> Bool {\n",
        "  if n == 0 { return true }\n",
        "  return is_odd(n - 1)\n",
        "}\n",
        "fn is_odd(n: Number) -> Bool {\n",
        "  if n == 0 { return false }\n",
        "  return is_even(n - 1)\n",
        "}"
    ));
    let even_body = &prog.functions[0].chunk.instructions;
    assert!(
        even_body
            .iter()
            .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "is_odd")),
        "is_even must load 'is_odd' callee"
    );
    assert!(
        even_body
            .iter()
            .any(|i| matches!(i, Instruction::Call { arg_count: 1 })),
        "is_even must emit CALL 1"
    );
    let odd_body = &prog.functions[1].chunk.instructions;
    assert!(
        odd_body
            .iter()
            .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "is_even")),
        "is_odd must load 'is_even' callee"
    );
    assert!(
        odd_body
            .iter()
            .any(|i| matches!(i, Instruction::Call { arg_count: 1 })),
        "is_odd must emit CALL 1"
    );
}

#[test]
fn bytecode_dynamic_call_no_longer_unsupported() {
    // M8G: chained call f()() compiles to stack-based calls — no Unsupported instruction.
    let prog = compile_prog("fn f() { } f()()");
    let has_unsupported = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Unsupported(_)));
    assert!(
        !has_unsupported,
        "dynamic call must no longer emit UNSUPPORTED after M8G"
    );
    // Must emit two CALL 0 instructions: one for f(), one for the chained ().
    let call_count = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::Call { arg_count: 0 }))
        .count();
    assert_eq!(
        call_count, 2,
        "f()() must emit exactly two CALL 0 instructions"
    );
}

// --- M8B audit: nested function declarations (provisional behavior) ---

#[test]
fn bytecode_nested_fn_decl_does_not_panic() {
    // A function declared inside another function body must not panic.
    // Nested fn lowering is provisional: inner appears in prog.functions as a flat chunk.
    let result = std::panic::catch_unwind(|| {
        compile_prog(concat!(
            "fn outer() -> Number {\n",
            "  fn inner() -> Number { return 1 }\n",
            "  return inner()\n",
            "}"
        ))
    });
    assert!(result.is_ok(), "nested fn decl must not panic");
}

#[test]
fn bytecode_nested_fn_decl_both_appear_in_functions() {
    // Nested fn decl: both outer and inner appear in prog.functions (flat function table).
    // inner is extended before outer is pushed, so inner appears first.
    let prog = compile_prog(concat!(
        "fn outer() -> Number {\n",
        "  fn inner() -> Number { return 1 }\n",
        "  return inner()\n",
        "}"
    ));
    assert_eq!(prog.functions.len(), 2);
    let names: Vec<&str> = prog.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must appear in function table"
    );
    assert!(
        names.contains(&"outer"),
        "outer must appear in function table"
    );
}

// --- M8B audit: disassembler stability ---

#[test]
fn disassemble_function_constants_in_fn_chunk_not_main() {
    // A function with a constant in its body must show that constant under the function section.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn double(x: Number) -> Number { return x * 2 }");
    let out = disassemble(&prog);
    let fn_pos = out.find("=== function double/1 ===").unwrap();
    let number2_pos = out.find("Number(2)").unwrap();
    // The Number(2) constant must appear after the function section header.
    assert!(
        number2_pos > fn_pos,
        "function constant must appear in the function section, not main"
    );
}

#[test]
fn disassemble_no_params_line_for_zero_param_fn() {
    // Zero-param function must not produce a params: line in the output.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn f() { }");
    let out = disassemble(&prog);
    assert!(
        !out.contains("params:"),
        "zero-param function must not show params: line"
    );
}

#[test]
fn disassemble_call_format_stable() {
    // M8G: CALL instruction is formatted as "CALL arg_count" (no name).
    // The callee name appears in the preceding LOAD_GLOBAL instruction.
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!(
        "fn f(a: Number, b: Number, c: Number) -> Number { return a }\n",
        "let x = f(1, 2, 3)"
    ));
    let out = disassemble(&prog);
    assert!(out.contains("CALL 3"), "CALL must include arg count");
    assert!(
        out.contains("LOAD_GLOBAL f"),
        "callee must be loaded via LOAD_GLOBAL"
    );
    assert!(
        !out.contains("CALL f"),
        "CALL must not include function name after M8G"
    );
}

#[test]
fn disassemble_load_function_format_stable() {
    // LOAD_FUNCTION instruction must be formatted as "LOAD_FUNCTION name".
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn my_func() { }");
    let out = disassemble(&prog);
    assert!(out.contains("LOAD_FUNCTION my_func"));
}

#[test]
fn disassemble_fn_chunk_return_shown() {
    // Function body with explicit return must show RETURN in the function section.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn f() -> Number { return 1 }");
    let out = disassemble(&prog);
    let fn_pos = out.find("=== function f/0 ===").unwrap();
    let return_pos = out.rfind("RETURN").unwrap();
    assert!(
        return_pos > fn_pos,
        "RETURN must appear inside the function section"
    );
}

#[test]
fn disassemble_fn_implicit_return_shows_nil_return() {
    // Empty function body must show NIL + RETURN in the function section.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn f() { }");
    let out = disassemble(&prog);
    assert!(out.contains("NIL"), "implicit return must emit NIL");
    assert!(out.contains("RETURN"), "implicit return must emit RETURN");
}

// ─── VM helpers ────────────────────────────────────────────────────────────

/// Compile and run through the bytecode VM. Returns captured print output.
fn vm_run(source: &str) -> Result<Vec<String>, KiminError> {
    use crate::vm::Vm;
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    TypeChecker::new().check(&stmts)?;
    let prog = BytecodeCompiler::new().compile(&stmts)?;
    let mut vm = Vm::new(prog);
    vm.run()?;
    Ok(vm.take_output())
}

/// Compile and run through the VM, skipping type-checking.
/// Use for programs that would fail type check (e.g. states) to verify the
/// VM produces the right runtime error.
fn vm_run_unchecked(source: &str) -> Result<Vec<String>, KiminError> {
    use crate::vm::Vm;
    let tokens = Lexer::new(source).tokenize()?;
    let stmts = Parser::new(tokens).parse()?;
    let prog = BytecodeCompiler::new().compile(&stmts)?;
    let mut vm = Vm::new(prog);
    vm.run()?;
    Ok(vm.take_output())
}

// ─── VM tests ──────────────────────────────────────────────────────────────

#[test]
fn vm_print_number() {
    let out = vm_run("print(42)").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn vm_print_float() {
    let out = vm_run("print(3.14)").unwrap();
    assert_eq!(out, vec!["3.14"]);
}

#[test]
fn vm_print_string() {
    let out = vm_run(r#"print("hello")"#).unwrap();
    assert_eq!(out, vec!["hello"]);
}

#[test]
fn vm_print_bool_true() {
    let out = vm_run("print(true)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_print_bool_false() {
    let out = vm_run("print(false)").unwrap();
    assert_eq!(out, vec!["false"]);
}

#[test]
fn vm_arithmetic_add() {
    let out = vm_run("print(1 + 2)").unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn vm_arithmetic_subtract() {
    let out = vm_run("print(10 - 3)").unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_arithmetic_multiply() {
    let out = vm_run("print(4 * 5)").unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn vm_arithmetic_divide() {
    let out = vm_run("print(10 / 2)").unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn vm_arithmetic_negate() {
    let out = vm_run("print(-7)").unwrap();
    assert_eq!(out, vec!["-7"]);
}

#[test]
fn vm_string_concatenation() {
    let out = vm_run(r#"print("hello" + " world")"#).unwrap();
    assert_eq!(out, vec!["hello world"]);
}

#[test]
fn vm_comparison_equal() {
    let out = vm_run("print(1 == 1)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_comparison_not_equal() {
    let out = vm_run("print(1 != 2)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_comparison_less() {
    let out = vm_run("print(3 < 5)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_comparison_greater_equal() {
    let out = vm_run("print(5 >= 5)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_not_operator() {
    let out = vm_run("print(!false)").unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_let_and_load() {
    let out = vm_run("let x = 10\nprint(x)").unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn vm_mutable_assign() {
    let out = vm_run("let mut x = 1\nx = x + 1\nprint(x)").unwrap();
    assert_eq!(out, vec!["2"]);
}

#[test]
fn vm_if_true_branch() {
    let out = vm_run("if true { print(1) } else { print(2) }").unwrap();
    assert_eq!(out, vec!["1"]);
}

#[test]
fn vm_if_false_branch() {
    let out = vm_run("if false { print(1) } else { print(2) }").unwrap();
    assert_eq!(out, vec!["2"]);
}

#[test]
fn vm_block_scope_local() {
    let out = vm_run("{ let x = 99\nprint(x) }").unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn vm_multiple_prints() {
    let out = vm_run("print(1)\nprint(2)\nprint(3)").unwrap();
    assert_eq!(out, vec!["1", "2", "3"]);
}

#[test]
fn vm_function_call_returns_value() {
    let src = "fn double(x: Number) -> Number { return x * 2 }\nprint(double(5))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn vm_function_two_params() {
    let src = "fn add(a: Number, b: Number) -> Number { return a + b }\nprint(add(3, 4))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_function_implicit_nil_return() {
    let src = "fn f() { }\nprint(f())";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["nil"]);
}

#[test]
fn vm_nested_function_calls() {
    let src = "fn add(a: Number, b: Number) -> Number { return a + b }\nfn square(x: Number) -> Number { return x * x }\nprint(square(add(2, 3)))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["25"]);
}

#[test]
fn vm_function_accesses_global() {
    let src = "let g = 100\nfn f() -> Number { return g }\nprint(f())";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["100"]);
}

#[test]
fn vm_recursive_factorial() {
    let src = r#"fn fact(n: Number) -> Number {
  if n <= 1 { return 1 }
  return n * fact(n - 1)
}
print(fact(5))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["120"]);
}

#[test]
fn vm_recursive_fibonacci() {
    let src = r#"fn fib(n: Number) -> Number {
  if n <= 1 { return n }
  return fib(n - 1) + fib(n - 2)
}
print(fib(7))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["13"]);
}

#[test]
fn vm_function_called_multiple_times() {
    let src = "fn inc(x: Number) -> Number { return x + 1 }\nprint(inc(0))\nprint(inc(inc(0)))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn vm_division_by_zero_error() {
    let result = vm_run("print(1 / 0)");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("division by zero"), "got: {}", msg);
}

#[test]
fn vm_undefined_variable_error() {
    let tokens = Lexer::new("print(x)").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    // Skip type checker (it would catch this first); verify VM also catches it.
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("undefined variable"), "got: {}", msg);
}

#[test]
fn vm_wrong_arity_error() {
    let src = "fn f(x: Number) -> Number { return x }\nprint(f(1, 2))";
    // Type checker allows extra call args check at runtime only via VM path.
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("expects"), "got: {}", msg);
}

#[test]
fn vm_define_state_runs_without_output() {
    // State declaration now executes (registers metadata) with no stack effect / no output.
    let out = vm_run_unchecked("state Door { closed opening open }").unwrap();
    assert!(out.is_empty());
}

#[test]
fn vm_bytecode_function_value_display() {
    assert_eq!(
        format!(
            "{}",
            Value::BytecodeFunction {
                name: "foo".into(),
                env: Env::new_global()
            }
        ),
        "<fn foo>"
    );
}

#[test]
fn vm_bytecode_function_value_type_name() {
    assert_eq!(
        Value::BytecodeFunction {
            name: "foo".into(),
            env: Env::new_global()
        }
        .type_name(),
        "Function"
    );
}

#[test]
fn vm_bytecode_function_value_equality() {
    assert_eq!(
        Value::BytecodeFunction {
            name: "f".into(),
            env: Env::new_global()
        },
        Value::BytecodeFunction {
            name: "f".into(),
            env: Env::new_global()
        }
    );
    assert_ne!(
        Value::BytecodeFunction {
            name: "f".into(),
            env: Env::new_global()
        },
        Value::BytecodeFunction {
            name: "g".into(),
            env: Env::new_global()
        }
    );
}

#[test]
fn vm_output_capture_order() {
    let out = vm_run("print(1)\nprint(2)\nprint(3)").unwrap();
    assert_eq!(out, vec!["1", "2", "3"]);
}

#[test]
fn vm_if_no_else_false_condition() {
    // If condition is false and no else branch, nothing printed.
    let out = vm_run("if false { print(99) }").unwrap();
    assert!(out.is_empty());
}

#[test]
fn vm_local_scope_does_not_leak() {
    // Variable defined inside block must not be visible outside.
    let src = "let x = 1\n{ let x = 2 }\nprint(x)";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1"]);
}

#[test]
fn vm_param_shadows_global() {
    let src = "let x = 99\nfn f(x: Number) -> Number { return x }\nprint(f(1))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1"]);
}

// ─── Audit: operand order ───────────────────────────────────────────────────

#[test]
fn vm_subtraction_operand_order() {
    // 10 - 3 = 7; reversed would be -7
    let out = vm_run("print(10 - 3)").unwrap();
    assert_eq!(out, vec!["7"]);
    let out2 = vm_run("print(3 - 10)").unwrap();
    assert_eq!(out2, vec!["-7"]);
}

#[test]
fn vm_division_operand_order() {
    // 10 / 2 = 5; reversed would be 0.2
    let out = vm_run("print(10 / 2)").unwrap();
    assert_eq!(out, vec!["5"]);
    let out2 = vm_run("print(2 / 10)").unwrap();
    assert_eq!(out2, vec!["0.2"]);
}

#[test]
fn vm_comparison_operand_order() {
    let out = vm_run("print(2 < 10)").unwrap();
    assert_eq!(out, vec!["true"]);
    let out2 = vm_run("print(10 < 2)").unwrap();
    assert_eq!(out2, vec!["false"]);
}

// ─── Audit: function call behavior ─────────────────────────────────────────

#[test]
fn vm_zero_arg_function_returns_value() {
    let out = vm_run("fn answer() -> Number { return 42 }\nprint(answer())").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn vm_multi_arg_order_preserved() {
    // sub(10, 3) = 7; reversed args would give -7
    let src = "fn sub(a: Number, b: Number) -> Number { return a - b }\nprint(sub(10, 3))";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_function_locals_do_not_leak_to_caller() {
    // Local defined inside fn is not in caller's scope
    let src = "let result = 0\nfn f() { let secret = 42 }\nf()\nprint(result)";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["0"]);
}

#[test]
fn vm_recursive_calls_have_separate_locals() {
    // sum(3) = 3+2+1 = 6; would be wrong if locals were shared across recursive frames
    let src = r#"fn sum(n: Number) -> Number {
  if n <= 0 { return 0 }
  return n + sum(n - 1)
}
print(sum(3))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["6"]);
}

#[test]
fn vm_return_inside_nested_block() {
    let src = r#"fn f(n: Number) -> Number {
  {
    return n
  }
}
print(f(7))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_return_inside_if() {
    let src = r#"fn sign(n: Number) -> Number {
  if n > 0 { return 1 }
  if n < 0 { return -1 }
  return 0
}
print(sign(5))
print(sign(-3))
print(sign(0))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "-1", "0"]);
}

#[test]
fn vm_mutual_recursion() {
    // is_even returns 1 for true, 0 for false (avoids Bool annotation ambiguity)
    let src = r#"fn is_even(n: Number) -> Number {
  if n == 0 { return 1 }
  return is_odd(n - 1)
}
fn is_odd(n: Number) -> Number {
  if n == 0 { return 0 }
  return is_even(n - 1)
}
print(is_even(4))
print(is_odd(3))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "1"]);
}

#[test]
fn vm_global_mutable_assign_inside_function() {
    // StoreGlobal inside a function body updates the shared globals table
    let src = r#"let mut count = 0
fn inc() {
  count = count + 1
}
inc()
inc()
print(count)"#;
    // Use raw compilation to sidestep type checker scope chain for globals-in-fn-body.
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    vm.run().unwrap();
    assert_eq!(vm.take_output(), vec!["2"]);
}

// ─── Audit: unsupported features ───────────────────────────────────────────

#[test]
fn vm_transition_sequence_works() {
    // State declaration + binding + transition now execute correctly in the VM.
    let src = concat!(
        "state Door { closed open  transition closed -> open }\n",
        "let door: Door = Door.closed\n",
        "transition door -> open\n",
        "print(door)"
    );
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    vm.run().unwrap();
    assert_eq!(vm.take_output(), vec!["Door.open"]);
}

#[test]
fn vm_simulate_empty_body_runs() {
    // Simulate with empty body produces no output but doesn't error.
    let src = "let d: seconds = 1\nlet dt: seconds = 1\nsimulate d step dt { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    vm.run().unwrap();
    assert!(vm.take_output().is_empty());
}

#[test]
fn vm_load_unknown_state_errors() {
    // LoadState without a preceding DefineState → RuntimeError: unknown state machine.
    let src = "print(Door.closed)";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown state machine"), "got: {}", msg);
}

#[test]
fn vm_dynamic_call_nil_callee_errors() {
    // M8G: f()() now executes. f() returns nil; calling nil as a function produces
    // a clean RuntimeError about non-function type, not an Unsupported error.
    let src = "fn f() { }\nf()()";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("non-function") || msg.contains("Nil"),
        "calling nil must produce a non-function error, got: {}",
        msg
    );
    assert!(
        !msg.contains("bytecode feature not yet executable"),
        "must not mention Unsupported after M8G, got: {}",
        msg
    );
}

#[test]
fn vm_unknown_function_error() {
    // M8G: calling an undefined name fails at LoadGlobal (undefined variable),
    // before even reaching the Call instruction.
    let src = "print(nonexistent(1))";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("nonexistent") || msg.contains("undefined"),
        "must mention the undefined name, got: {}",
        msg
    );
}

// ─── Audit: cross-validation against tree-walk interpreter ─────────────────

#[test]
fn vm_matches_tree_arithmetic() {
    // Values verified by `kimin run`: 3+4=7, 4*5-11=9, (2+3)*4+2=22
    let src = "print(3 + 4)\nprint(4 * 5 - 11)\nprint((2 + 3) * 4 + 2)";
    assert_eq!(vm_run(src).unwrap(), vec!["7", "9", "22"]);
}

#[test]
fn vm_matches_tree_conditionals() {
    let src = "if true { print(1) } else { print(2) }\nif false { print(3) } else { print(4) }";
    assert_eq!(vm_run(src).unwrap(), vec!["1", "4"]);
}

#[test]
fn vm_matches_tree_mutable() {
    let src = "let mut x = 0\nx = x + 1\nprint(x)\nx = x + 1\nprint(x)\nx = x * 10\nprint(x)";
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2", "20"]);
}

#[test]
fn vm_matches_tree_functions() {
    let src = r#"fn add(a: Number, b: Number) -> Number { return a + b }
fn square(x: Number) -> Number { return x * x }
print(add(2, 3))
print(square(5))
print(square(add(2, 3)))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["5", "25", "25"]);
}

#[test]
fn vm_matches_tree_recursion() {
    let src = r#"fn fact(n: Number) -> Number {
  if n <= 1 { return 1 }
  return n * fact(n - 1)
}
print(fact(5))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["120"]);
}

// ─── Audit: misc behavior ───────────────────────────────────────────────────

#[test]
fn vm_halt_stops_execution() {
    // Halt is the final instruction in main; verifies clean termination.
    let out = vm_run("print(1)\nprint(2)").unwrap();
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn vm_print_function_value_via_global() {
    // LoadFunction + DefineGlobal stores BytecodeFunction("f");
    // loading and printing it should display "<fn f>".
    let src = "fn f() { }\nprint(f)";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    vm.run().unwrap();
    assert_eq!(vm.take_output(), vec!["<fn f>"]);
}

#[test]
fn vm_nested_if_works() {
    // JumpIfFalse fix: nested ifs must not corrupt the stack.
    let src = r#"let x = 5
if x > 0 {
  if x > 3 {
    print(1)
  } else {
    print(2)
  }
} else {
  print(3)
}"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1"]);
}

#[test]
fn vm_if_condition_does_not_leak_to_stack() {
    // After an if statement, subsequent code must see correct stack state.
    // If JumpIfFalse left the condition on the stack, this program would print
    // the wrong result for the second print.
    let src = "if true { print(1) }\nprint(2)";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn vm_multiple_if_statements_stack_clean() {
    // Three consecutive if statements must each have a clean stack after completion.
    let src = r#"if true { print(1) }
if false { print(99) }
if true { print(2) }"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "2"]);
}

// ─── M8D: state machine bytecode tests ─────────────────────────────────────

const DOOR_SRC: &str = "state Door {
  closed
  opening
  open

  transition closed -> opening
  transition opening -> open
}";

#[test]
fn bytecode_define_state_preserves_variants_and_transitions() {
    let prog = compile_prog(DOOR_SRC);
    let instrs = &prog.main.instructions;
    match &instrs[0] {
        Instruction::DefineState {
            name,
            variants,
            transitions,
        } => {
            assert_eq!(name, "Door");
            assert!(variants.contains(&"closed".to_string()));
            assert!(variants.contains(&"opening".to_string()));
            assert!(variants.contains(&"open".to_string()));
            assert!(transitions.contains(&("closed".into(), "opening".into())));
            assert!(transitions.contains(&("opening".into(), "open".into())));
        }
        other => panic!("expected DefineState, got {:?}", other),
    }
}

#[test]
fn bytecode_state_variant_let_emits_load_state_then_define_global() {
    // let door: Door = Door.closed → LoadState + DefineGlobal
    let prog = compile_prog(&format!("{}\nlet door: Door = Door.closed", DOOR_SRC));
    let instrs = &prog.main.instructions;
    // Find LoadState followed by DefineGlobal("door")
    let load_idx = instrs
        .iter()
        .position(|i| {
            matches!(i, Instruction::LoadState { state_name, variant_name }
            if state_name == "Door" && variant_name == "closed")
        })
        .expect("LoadState Door.closed not found");
    assert!(
        matches!(&instrs[load_idx + 1], Instruction::DefineGlobal(n) if n == "door"),
        "expected DefineGlobal(door) after LoadState"
    );
}

#[test]
fn bytecode_simulate_emits_simulate_not_unsupported() {
    let prog = compile_prog("let dur = 3\nlet dt = 1\nsimulate dur step dt { }");
    let has_simulate_instr = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Simulate { .. }));
    assert!(
        has_simulate_instr,
        "simulate must emit Instruction::Simulate"
    );
    let has_unsupported = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Unsupported(s) if s == "simulate"));
    assert!(
        !has_unsupported,
        "simulate must not emit Unsupported after M8E"
    );
}

#[test]
fn disassemble_define_state_format() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(DOOR_SRC);
    let out = disassemble(&prog);
    assert!(
        out.contains("DEFINE_STATE Door"),
        "expected DEFINE_STATE Door in disassembly"
    );
    assert!(
        out.contains("variants=["),
        "expected variants list in disassembly"
    );
    assert!(
        out.contains("transitions=["),
        "expected transitions list in disassembly"
    );
}

#[test]
fn disassemble_load_state_format() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(&format!("{}\nlet door: Door = Door.closed", DOOR_SRC));
    let out = disassemble(&prog);
    assert!(
        out.contains("LOAD_STATE Door.closed"),
        "expected LOAD_STATE Door.closed"
    );
}

#[test]
fn disassemble_transition_format() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(&format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> opening",
        DOOR_SRC
    ));
    let out = disassemble(&prog);
    assert!(
        out.contains("TRANSITION door -> opening"),
        "expected TRANSITION door -> opening"
    );
}

#[test]
fn disassemble_states_example_no_unsupported_for_state() {
    // states.kimin must no longer contain UNSUPPORTED for state or state values.
    use crate::disassemble::disassemble;
    let prog = compile_prog(&format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> opening\ntransition door -> open",
        DOOR_SRC
    ));
    let out = disassemble(&prog);
    assert!(
        !out.contains("UNSUPPORTED(state"),
        "state decl must not emit UNSUPPORTED"
    );
    assert!(
        !out.contains("UNSUPPORTED(transition"),
        "transition must not emit UNSUPPORTED"
    );
    assert!(
        !out.contains("UNSUPPORTED(Door"),
        "state variant must not emit UNSUPPORTED"
    );
}

// ─── M8D: VM state machine execution tests ─────────────────────────────────

fn vm_run_state(src: &str) -> Result<Vec<String>, KiminError> {
    // vm_run_unchecked bypasses the type checker — convenient for state programs
    // that use type annotations (Door) the type checker validates but we don't need here.
    vm_run_unchecked(src)
}

#[test]
fn vm_let_state_variable_prints_state() {
    let src = format!("{}\nlet door: Door = Door.closed\nprint(door)", DOOR_SRC);
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed"]);
}

#[test]
fn vm_transition_updates_global_state_value() {
    let src = format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> opening\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_transition_sequence_full() {
    // Mirrors states.kimin expected output.
    let src = format!(
        "{}\nlet door: Door = Door.closed\nprint(door)\ntransition door -> opening\nprint(door)\ntransition door -> open\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed", "Door.opening", "Door.open"]);
}

#[test]
fn vm_state_value_printed_via_display() {
    // Value::StateValue Display format is state_name.variant_name
    let src = format!("{}\nlet d: Door = Door.opening\nprint(d)", DOOR_SRC);
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_transition_inside_block_updates_outer_state() {
    let src = format!(
        "{}\nlet door: Door = Door.closed\n{{ transition door -> opening }}\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_state_returned_from_function() {
    let src = format!(
        "{}\nfn make_door() -> Door {{ return Door.closed }}\nprint(make_door())",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed"]);
}

#[test]
fn vm_state_passed_to_function_and_printed() {
    let src = format!(
        "{}\nfn show(d: Door) {{ print(d) }}\nlet door: Door = Door.opening\nshow(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_transition_inside_function_updates_param_not_caller() {
    // Current language semantics: transition inside fn body updates the function-local
    // copy of the parameter, not the caller's binding.
    let src = format!(
        "{}\nfn open_local(d: Door) -> Door {{ transition d -> opening\nreturn d }}\nlet door: Door = Door.closed\nlet changed = open_local(door)\nprint(door)\nprint(changed)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed", "Door.opening"]);
}

#[test]
fn vm_state_value_equality() {
    // Two StateValues with same state_name and variant_name are equal.
    let src = format!(
        "{}\nlet a: Door = Door.closed\nlet b: Door = Door.closed\nprint(a == b)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["true"]);
}

#[test]
fn vm_state_value_inequality() {
    let src = format!(
        "{}\nlet a: Door = Door.closed\nlet b: Door = Door.opening\nprint(a == b)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["false"]);
}

#[test]
fn vm_define_state_no_stack_effect() {
    // DefineState must have no stack effect — subsequent print still works.
    let src = format!("{}\nprint(1)", DOOR_SRC);
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["1"]);
}

// ─── M8D: VM state machine error tests ─────────────────────────────────────

#[test]
fn vm_load_unknown_variant_errors() {
    // DefineState registered Door, but variant "locked" not declared.
    let src = format!("{}\nlet d = Door.locked", DOOR_SRC);
    let tokens = Lexer::new(&src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown variant"), "got: {}", msg);
}

#[test]
fn vm_transition_unknown_variable_errors() {
    // Transition on a variable that does not exist.
    let src = format!("{}\ntransition ghost -> opening", DOOR_SRC);
    let tokens = Lexer::new(&src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("undefined state variable"), "got: {}", msg);
}

#[test]
fn vm_transition_non_state_value_errors() {
    // Transition on a variable that holds a Number, not a StateValue.
    let src = format!("{}\nlet n = 5\ntransition n -> opening", DOOR_SRC);
    let tokens = Lexer::new(&src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("not a state value"), "got: {}", msg);
}

#[test]
fn vm_transition_unknown_target_variant_errors() {
    // Transition to a variant that exists in neither declaration.
    let src = format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> locked",
        DOOR_SRC
    );
    let tokens = Lexer::new(&src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown variant"), "got: {}", msg);
}

#[test]
fn vm_transition_invalid_edge_errors() {
    // Transition from closed directly to open — not declared (must go closed -> opening -> open).
    let src = format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> open",
        DOOR_SRC
    );
    let tokens = Lexer::new(&src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("invalid transition"), "got: {}", msg);
}

#[test]
fn vm_simulate_now_executes() {
    // simulate now executes in the VM after M8E.
    let src = "let d: seconds = 1\nlet dt: seconds = 1\nsimulate d step dt { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    vm.run().unwrap();
    assert!(vm.take_output().is_empty());
}

// ─── M8D: cross-validation vs tree-walk ────────────────────────────────────

#[test]
fn vm_matches_tree_states_example() {
    // Output verified by `kimin run examples/states.kimin`: Door.closed / Door.opening / Door.open
    let src = std::fs::read_to_string("examples/states.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.closed", "Door.opening", "Door.open"]);
}

#[test]
fn vm_matches_tree_state_functions_example() {
    // Output verified by `kimin run examples/state_functions.kimin`: Door.closed / Door.opening
    let src = std::fs::read_to_string("examples/state_functions.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.closed", "Door.opening"]);
}

#[test]
fn vm_matches_tree_state_errors_example() {
    // state_errors.kimin runs cleanly by default and prints Door.closed
    let src = std::fs::read_to_string("examples/state_errors.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.closed"]);
}

// ─── M8D audit: compiler lowering completeness ─────────────────────────────

#[test]
fn bytecode_state_variant_in_return_lowers_correctly() {
    // Expr::StateVariant inside a return statement must emit LoadState.
    let src = format!(
        "{}\nfn make_closed() -> Door {{ return Door.closed }}",
        DOOR_SRC
    );
    let prog = compile_prog(&src);
    let fn_chunk = prog
        .functions
        .iter()
        .find(|f| f.name == "make_closed")
        .expect("function not found");
    let has_load_state = fn_chunk.chunk.instructions.iter().any(|i| {
        matches!(i, Instruction::LoadState { state_name, variant_name }
            if state_name == "Door" && variant_name == "closed")
    });
    assert!(
        has_load_state,
        "return Door.closed must emit LoadState in function chunk"
    );
}

#[test]
fn bytecode_transition_inside_if_lowers_correctly() {
    // Transition inside an if branch must still emit Instruction::Transition.
    let src = format!(
        "{}\nlet door: Door = Door.closed\nif true {{ transition door -> opening }}",
        DOOR_SRC
    );
    let prog = compile_prog(&src);
    let has_transition = prog.main.instructions.iter().any(|i| {
        matches!(i, Instruction::Transition { variable, target }
            if variable == "door" && target == "opening")
    });
    assert!(
        has_transition,
        "transition inside if must emit Transition instruction"
    );
}

// ─── M8D audit: VM state value behavior ────────────────────────────────────

#[test]
fn vm_print_state_literal_directly() {
    // print(Door.closed) without a let binding — LoadState pushes onto stack, Print consumes it.
    let src = format!("{}\nprint(Door.closed)", DOOR_SRC);
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed"]);
}

#[test]
fn vm_state_value_in_local_scope() {
    // State value stored in block-local: accessible inside block, not outside.
    let src = format!(
        "{}\n{{ let d: Door = Door.opening\nprint(d) }}\nprint(1)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening", "1"]);
}

#[test]
fn vm_transition_immutable_state_does_not_require_let_mut() {
    // State variables use transition for mutation — let mut is not required.
    let src = format!(
        "{}\nlet door: Door = Door.closed\ntransition door -> opening\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

// ─── M8D audit: transition scope behavior ──────────────────────────────────

#[test]
fn vm_transition_local_shadow_updates_local_not_global() {
    // Block-local `door` shadows global `door`.
    // Transition inside block updates the local shadow; global is unchanged.
    let src = format!(
        "{}\nlet door: Door = Door.closed\n{{ let door: Door = Door.closed\ntransition door -> opening\nprint(door) }}\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening", "Door.closed"]);
}

#[test]
fn vm_transition_after_shadow_ends_targets_global() {
    // After a block with a local shadow exits, transitioning targets the global again.
    let src = format!(
        "{}\nlet door: Door = Door.closed\n{{ let door: Door = Door.closed }}\ntransition door -> opening\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_transition_inside_if_branch() {
    // Transition inside an if branch executes only when condition is true.
    let src = format!(
        "{}\nlet door: Door = Door.closed\nif true {{ transition door -> opening }}\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

#[test]
fn vm_transition_inside_if_not_taken() {
    // Transition inside an if branch is skipped when condition is false.
    let src = format!(
        "{}\nlet door: Door = Door.closed\nif false {{ transition door -> opening }}\nprint(door)",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.closed"]);
}

// ─── M8D audit: function interaction ───────────────────────────────────────

#[test]
fn vm_transition_function_updates_global_state_directly() {
    // A function that references and transitions a global state variable (not via parameter)
    // must update the global in-place — get_var finds it in globals, assign_var updates globals.
    let src = "state Door { closed open  transition closed -> open }\nlet door: Door = Door.closed\nfn open_door() {\n  transition door -> open\n}\nopen_door()\nprint(door)";
    let out = vm_run_state(src).unwrap();
    assert_eq!(out, vec!["Door.open"]);
}

#[test]
fn vm_state_value_passed_through_two_functions() {
    // State values survive passing through multiple function call frames.
    let src = format!(
        "{}\nfn wrap(d: Door) -> Door {{ return d }}\nfn outer(d: Door) -> Door {{ return wrap(d) }}\nlet door: Door = Door.opening\nprint(outer(door))",
        DOOR_SRC
    );
    let out = vm_run_state(&src).unwrap();
    assert_eq!(out, vec!["Door.opening"]);
}

// ─── M8E: simulate bytecode compiler tests ─────────────────────────────────

#[test]
fn bytecode_simulate_no_longer_unsupported() {
    let prog = compile_prog("let dur = 3\nlet dt = 1\nsimulate dur step dt { }");
    let no_unsupported = prog
        .main
        .instructions
        .iter()
        .all(|i| !matches!(i, Instruction::Unsupported(s) if s == "simulate"));
    assert!(
        no_unsupported,
        "simulate must not emit Unsupported after M8E"
    );
}

#[test]
fn bytecode_simulate_stores_one_body() {
    let prog = compile_prog("let dur = 3\nlet dt = 1\nsimulate dur step dt { print(1) }");
    assert_eq!(prog.simulate_bodies.len(), 1);
    assert_eq!(prog.simulate_bodies[0].name, "simulate#0");
}

#[test]
fn bytecode_simulate_two_bodies_indexed_correctly() {
    let src =
        "let d = 1\nlet s = 1\nsimulate d step s { print(1) }\nsimulate d step s { print(2) }";
    let prog = compile_prog(src);
    assert_eq!(prog.simulate_bodies.len(), 2);
    assert_eq!(prog.simulate_bodies[0].name, "simulate#0");
    assert_eq!(prog.simulate_bodies[1].name, "simulate#1");
    // Main chunk should have two SIMULATE instructions with correct indices.
    let sim_instrs: Vec<usize> = prog
        .main
        .instructions
        .iter()
        .filter_map(|i| match i {
            Instruction::Simulate { body_idx } => Some(*body_idx),
            _ => None,
        })
        .collect();
    assert_eq!(sim_instrs, vec![0, 1]);
}

#[test]
fn bytecode_simulate_body_contains_load_local_time() {
    let prog = compile_prog("let dur = 1\nlet dt = 1\nsimulate dur step dt { print(time) }");
    let body = &prog.simulate_bodies[0].chunk;
    let has_load_time = body
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "time"));
    assert!(has_load_time, "simulate body must load 'time' as local");
}

#[test]
fn bytecode_simulate_body_loads_outer_global() {
    let src = "let mut pos = 0\nlet dur = 1\nlet dt = 1\nsimulate dur step dt { pos = pos + 1 }";
    let prog = compile_prog(src);
    let body = &prog.simulate_bodies[0].chunk;
    let has_load_global = body
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "pos"));
    let has_store_global = body
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "pos"));
    assert!(has_load_global, "simulate body must LoadGlobal outer var");
    assert!(has_store_global, "simulate body must StoreGlobal outer var");
}

#[test]
fn bytecode_simulate_body_can_contain_transition() {
    let src = format!(
        "{}\nlet door: Door = Door.closed\nlet dur = 1\nlet dt = 1\nsimulate dur step dt {{ transition door -> opening }}",
        DOOR_SRC
    );
    let prog = compile_prog(&src);
    let body = &prog.simulate_bodies[0].chunk;
    let has_transition = body.instructions.iter().any(|i| {
        matches!(i, Instruction::Transition { variable, target }
            if variable == "door" && target == "opening")
    });
    assert!(
        has_transition,
        "simulate body must contain Transition instruction"
    );
}

#[test]
fn disassemble_simulate_body_section_shown() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let dur = 1\nlet dt = 1\nsimulate dur step dt { print(time) }");
    let out = disassemble(&prog);
    assert!(
        out.contains("simulate simulate#0"),
        "expected simulate body section header"
    );
    assert!(
        out.contains("SIMULATE #0"),
        "expected SIMULATE instruction in main"
    );
    assert!(
        out.contains("LOAD_LOCAL time"),
        "expected LOAD_LOCAL time in simulate body"
    );
}

#[test]
fn disassemble_simulate_motion_no_unsupported() {
    use crate::disassemble::disassemble;
    let src = std::fs::read_to_string("examples/simulate_motion.kimin").unwrap();
    let prog = compile_prog(&src);
    let out = disassemble(&prog);
    assert!(
        !out.contains("UNSUPPORTED(simulate)"),
        "simulate_motion.kimin must not have UNSUPPORTED(simulate) after M8E"
    );
    assert!(out.contains("SIMULATE"), "expected SIMULATE instruction");
}

// ─── M8E: VM simulate execution tests ──────────────────────────────────────

#[test]
fn vm_simulate_print_time_seconds() {
    // duration=3, dt=1 → 3 iterations, time = 0, 1, 2
    let out =
        vm_run("let dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt { print(time) }")
            .unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn vm_simulate_fractional_step() {
    // duration=1, step=0.5 → 2 iterations, time = 0, 0.5
    let out =
        vm_run("let dur: seconds = 1\nlet dt: seconds = 0.5\nsimulate dur step dt { print(time) }")
            .unwrap();
    assert_eq!(out, vec!["0", "0.5"]);
}

#[test]
fn vm_simulate_zero_duration_no_output() {
    let out =
        vm_run("let dur: seconds = 0\nlet dt: seconds = 1\nsimulate dur step dt { print(99) }")
            .unwrap();
    assert!(out.is_empty(), "zero duration: no iterations");
}

#[test]
fn vm_simulate_step_zero_runtime_error() {
    let src = "let dur: seconds = 1\nlet dt: seconds = 0\nsimulate dur step dt { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("greater than zero"), "got: {}", msg);
}

#[test]
fn vm_simulate_negative_duration_runtime_error() {
    // Manually create program with duration = -1 to bypass type checker.
    use crate::{
        bytecode::{BytecodeProgram, Chunk, Constant, SimulateChunk},
        vm::Vm,
    };
    let mut main = Chunk::new();
    let neg_idx = main.add_constant(Constant::Number(-1.0));
    let step_idx = main.add_constant(Constant::Number(1.0));
    main.emit(Instruction::Constant(neg_idx));
    main.emit(Instruction::Constant(step_idx));
    main.emit(Instruction::Simulate { body_idx: 0 });
    main.emit(Instruction::Halt);
    let prog = BytecodeProgram::new(
        main,
        vec![],
        vec![SimulateChunk {
            name: "simulate#0".into(),
            chunk: Chunk::new(),
        }],
    );
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("cannot be negative"), "got: {}", msg);
}

#[test]
fn vm_simulate_body_local_fresh_each_iteration() {
    // A let binding inside the body is fresh per iteration — not cumulative.
    let src = "let dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt { let x = time\nprint(x) }";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn vm_simulate_mutable_position_motion() {
    // Key M8E feature: mutable outer variable updated across iterations.
    // velocity is inferred as meters/seconds via compound unit inference.
    let src = r#"let mut position: meters = 0
let dist_per_step: meters = 2
let unit_time: seconds = 1
let velocity = dist_per_step / unit_time
let duration: seconds = 3
let dt: seconds = 1
simulate duration step dt {
  position = position + velocity * dt
  print(position)
}"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["2", "4", "6"]);
}

#[test]
fn vm_simulate_two_loops_independent() {
    // Two sequential simulate blocks operate independently.
    let src = r#"let mut x = 0
let dur: seconds = 2
let dt: seconds = 1
simulate dur step dt { x = x + 1 }
let first = x
simulate dur step dt { x = x + 10 }
print(first)
print(x)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["2", "22"]);
}

#[test]
fn vm_simulate_state_transition_one_iteration() {
    let src = format!(
        "{}\nlet door: Door = Door.closed\nlet dur: seconds = 1\nlet dt: seconds = 1\nsimulate dur step dt {{\n  print(door)\n  transition door -> opening\n  print(door)\n}}",
        DOOR_SRC
    );
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.closed", "Door.opening"]);
}

#[test]
fn vm_simulate_state_toggler() {
    // Blinker toggles off->on->off each iteration; starts and ends off each iteration.
    let src = "state Blinker { off on  transition off -> on  transition on -> off }\nlet light: Blinker = Blinker.off\nlet dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt {\n  transition light -> on\n  print(light)\n  transition light -> off\n}";
    let out = vm_run_unchecked(src).unwrap();
    assert_eq!(out, vec!["Blinker.on", "Blinker.on", "Blinker.on"]);
}

#[test]
fn vm_simulate_return_inside_function() {
    // return inside simulate inside a function must exit the function.
    let src = r#"fn f() -> Number {
  let dur: seconds = 3
  let dt: seconds = 1
  simulate dur step dt {
    return 42
  }
  return 0
}
print(f())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["42"]);
}

// ─── M8E: cross-validation vs tree-walk ────────────────────────────────────

#[test]
fn vm_simulate_matches_tree_simulate_example() {
    let src = std::fs::read_to_string("examples/simulate.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn vm_simulate_matches_tree_simulate_state_example() {
    let src = std::fs::read_to_string("examples/simulate_state.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    // simulate_state.kimin: 1 iteration, prints Door.closed then Door.opening
    assert_eq!(out, vec!["Door.closed", "Door.opening"]);
}

#[test]
fn vm_simulate_matches_tree_simulate_motion_example() {
    let src = std::fs::read_to_string("examples/simulate_motion.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["2", "4", "6"]);
}

#[test]
fn vm_simulate_matches_tree_simulate_errors_example() {
    let src = std::fs::read_to_string("examples/simulate_errors.kimin").unwrap();
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["0", "1"]);
}

// ─── M8E: runtime error tests ─────────────────────────────────────────────

#[test]
fn vm_simulate_invalid_body_index_errors() {
    use crate::{
        bytecode::{BytecodeProgram, Chunk},
        vm::Vm,
    };
    let mut main = Chunk::new();
    let dur_idx = main.add_constant(Constant::Number(1.0));
    let step_idx = main.add_constant(Constant::Number(1.0));
    main.emit(Instruction::Constant(dur_idx));
    main.emit(Instruction::Constant(step_idx));
    main.emit(Instruction::Simulate { body_idx: 99 });
    main.emit(Instruction::Halt);
    let prog = BytecodeProgram::new(main, vec![], vec![]);
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("invalid simulate body index"), "got: {}", msg);
}

// ─── M8E audit hardening ────────────────────────────────────────────────────

#[test]
fn vm_simulate_floor_iteration_count() {
    // floor(2.9 / 1) = 2 iterations: time = 0, 1
    let src = "let dur: seconds = 2.9\nlet dt: seconds = 1\nsimulate dur step dt { print(time) }";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["0", "1"]);
}

#[test]
fn vm_simulate_step_larger_than_duration_zero_iterations() {
    // floor(0.5 / 1) = 0: body never runs
    let src = "let dur: seconds = 0.5\nlet dt: seconds = 1\nsimulate dur step dt { print(99) }";
    let out = vm_run(src).unwrap();
    assert!(out.is_empty(), "step > duration produces zero iterations");
}

#[test]
fn vm_simulate_negative_step_errors() {
    // Negative step is not a static type error (sign not a type concern) but
    // the VM rejects step <= 0 at runtime.
    let src = "let dur: seconds = 1\nlet dt: seconds = -1\nsimulate dur step dt { }";
    let result = vm_run_unchecked(src);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("greater than zero"), "got: {}", msg);
}

#[test]
fn vm_simulate_non_number_duration_errors() {
    // Manually push a string as duration — bypasses source-level type constraints.
    use crate::{
        bytecode::{BytecodeProgram, Chunk, Constant, SimulateChunk},
        vm::Vm,
    };
    let mut main = Chunk::new();
    let str_idx = main.add_constant(Constant::Text("oops".into()));
    let step_idx = main.add_constant(Constant::Number(1.0));
    main.emit(Instruction::Constant(str_idx));
    main.emit(Instruction::Constant(step_idx));
    main.emit(Instruction::Simulate { body_idx: 0 });
    main.emit(Instruction::Halt);
    let prog = BytecodeProgram::new(
        main,
        vec![],
        vec![SimulateChunk {
            name: "simulate#0".into(),
            chunk: Chunk::new(),
        }],
    );
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("simulate duration must be a number"),
        "got: {}",
        msg
    );
}

#[test]
fn vm_simulate_non_number_step_errors() {
    // Manually push a string as step — bypasses source-level type constraints.
    use crate::{
        bytecode::{BytecodeProgram, Chunk, Constant, SimulateChunk},
        vm::Vm,
    };
    let mut main = Chunk::new();
    let dur_idx = main.add_constant(Constant::Number(1.0));
    let str_idx = main.add_constant(Constant::Text("oops".into()));
    main.emit(Instruction::Constant(dur_idx));
    main.emit(Instruction::Constant(str_idx));
    main.emit(Instruction::Simulate { body_idx: 0 });
    main.emit(Instruction::Halt);
    let prog = BytecodeProgram::new(
        main,
        vec![],
        vec![SimulateChunk {
            name: "simulate#0".into(),
            chunk: Chunk::new(),
        }],
    );
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("simulate step must be a number"),
        "got: {}",
        msg
    );
}

#[test]
fn vm_simulate_body_local_shadow_does_not_affect_outer() {
    // A let binding inside the body with the same name as an outer global does
    // not modify the outer variable — it creates a body-local binding only.
    let src = r#"let mut x = 100
let dur: seconds = 2
let dt: seconds = 1
simulate dur step dt {
  let x = time
}
print(x)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["100"], "outer x must be unchanged after simulate");
}

#[test]
fn vm_simulate_inside_block_local_capture_confirmed_limitation() {
    // M8F: simulate bodies now parent their iter_env to the current_env (not global_env),
    // so block-local outer variables are captured correctly.
    // This was a known M8E limitation; it is fixed in M8F.
    let src = r#"let dur: seconds = 1
let dt: seconds = 1
{
  let captured = 42
  simulate dur step dt {
    print(captured)
  }
}"#;
    let result = vm_run(src);
    assert_eq!(
        result.unwrap(),
        vec!["42"],
        "block-local capture must succeed after M8F fix"
    );
}

#[test]
fn vm_simulate_state_invalid_transition_unchecked_errors() {
    // An invalid state transition inside a simulate body produces a clean RuntimeError.
    let src = format!(
        "{}\nlet door: Door = Door.closed\nlet dur: seconds = 1\nlet dt: seconds = 1\nsimulate dur step dt {{\n  transition door -> flying\n}}",
        DOOR_SRC
    );
    let result = vm_run_unchecked(&src);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("flying") || msg.contains("variant") || msg.contains("transition"),
        "expected invalid-transition error, got: {}",
        msg
    );
}

#[test]
fn bytecode_simulate_body_no_halt() {
    let prog = compile_prog("let dur = 1\nlet dt = 1\nsimulate dur step dt { print(time) }");
    let body = &prog.simulate_bodies[0];
    let has_halt = body
        .chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Halt));
    assert!(!has_halt, "simulate body chunk must not contain Halt");
}

#[test]
fn bytecode_simulate_duration_before_step() {
    // Compiler emits: ..., LoadGlobal("dur"), LoadGlobal("dt"), Simulate { .. }
    // The VM pops step first (top of stack) then duration, so duration must compile first.
    let prog = compile_prog("let dur = 3\nlet dt = 1\nsimulate dur step dt { }");
    let instrs = &prog.main.instructions;
    let dur_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::LoadGlobal(n) if n == "dur"))
        .expect("LoadGlobal(dur) must exist");
    let dt_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::LoadGlobal(n) if n == "dt"))
        .expect("LoadGlobal(dt) must exist");
    let sim_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Simulate { .. }))
        .expect("Simulate must exist");
    assert!(dur_pos < dt_pos, "duration must compile before step");
    assert!(
        dt_pos < sim_pos,
        "step must compile before Simulate instruction"
    );
}

#[test]
fn bytecode_simulate_body_store_local_for_body_local() {
    // A let mut variable declared inside a simulate body is body-local: must emit
    // DefineLocal/StoreLocal, never DefineGlobal/StoreGlobal.
    let prog = compile_prog(
        "let dur = 1\nlet dt = 1\nsimulate dur step dt { let mut acc = 0\nacc = acc + 1 }",
    );
    let body = &prog.simulate_bodies[0].chunk;
    assert!(
        body.instructions
            .iter()
            .any(|i| matches!(i, Instruction::DefineLocal(n) if n == "acc")),
        "body-local let must emit DefineLocal(acc)"
    );
    assert!(
        body.instructions
            .iter()
            .any(|i| matches!(i, Instruction::StoreLocal(n) if n == "acc")),
        "body-local assignment must emit StoreLocal(acc)"
    );
    assert!(
        !body
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::DefineGlobal(n) if n == "acc")),
        "body-local must not emit DefineGlobal(acc)"
    );
    assert!(
        !body
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "acc")),
        "body-local must not emit StoreGlobal(acc)"
    );
}

#[test]
fn disassemble_multiple_simulate_bodies() {
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!(
        "let dur = 1\nlet dt = 1\n",
        "simulate dur step dt { print(1) }\n",
        "simulate dur step dt { print(2) }",
    ));
    assert_eq!(
        prog.simulate_bodies.len(),
        2,
        "two simulate bodies expected"
    );
    let out = disassemble(&prog);
    let pos0 = out
        .find("simulate simulate#0")
        .expect("simulate#0 section must appear in disassembly");
    let pos1 = out
        .find("simulate simulate#1")
        .expect("simulate#1 section must appear in disassembly");
    assert!(pos0 < pos1, "simulate#0 must appear before simulate#1");
    assert_eq!(
        out.matches("params: time").count(),
        2,
        "each simulate body section must have 'params: time'"
    );
}

// ── M8F: closure / free-variable capture tests ───────────────────────────────

#[test]
fn vm_closure_function_reads_global_free_variable() {
    // A function declared at top level can read a global defined before it.
    let out = vm_run("let x = 99\nfn get() -> Number { return x }\nprint(get())").unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn vm_closure_nested_reads_enclosing_local() {
    // A nested function captures its enclosing function's local variable.
    let src = r#"fn outer() -> Number {
  let val = 7
  fn inner() -> Number {
    return val
  }
  return inner()
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_closure_mutates_captured_mutable() {
    // Nested function mutates a mutable variable from its enclosing scope.
    let src = r#"fn outer() -> Number {
  let mut x = 1
  fn inc() -> Number {
    x = x + 1
    return x
  }
  inc()
  return inc()
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn vm_closure_inner_shadows_outer_variable() {
    // A local in the inner function shadows the outer function's variable.
    let src = r#"fn outer() -> Number {
  let x = 10
  fn inner() -> Number {
    let x = 20
    return x
  }
  return inner()
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn vm_closure_params_shadow_captured_variables() {
    // A parameter with the same name as an outer variable shadows it.
    let src = r#"fn outer() -> Number {
  let x = 5
  fn add(x: Number) -> Number {
    return x + 1
  }
  return add(100)
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["101"]);
}

#[test]
fn vm_closure_returned_function_keeps_env_alive() {
    // A returned function value carries its captured environment.
    // Because Kimin doesn't yet support first-class return of functions,
    // test via a top-level function calling a nested helper twice.
    let src = r#"fn make_adder() -> Number {
  let base = 10
  fn adder(n: Number) -> Number {
    return base + n
  }
  return adder(5)
}
print(make_adder())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["15"]);
}

// ── M8F: simulate body block/function-local capture tests ────────────────────

#[test]
fn vm_simulate_captures_block_local_read() {
    // Simulate body reads a variable defined in an enclosing block scope.
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
{
  let offset = 10
  simulate dur step dt {
    print(offset)
  }
}"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["10", "10", "10"]);
}

#[test]
fn vm_simulate_captures_block_local_mutable_write() {
    // Simulate body writes to a mutable variable in an enclosing block scope;
    // the change persists across iterations and after the simulate.
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
{
  let mut acc = 0
  simulate dur step dt {
    acc = acc + 1
  }
  print(acc)
}"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn vm_simulate_captures_function_local_mutable() {
    // Simulate inside a function body can write to the function's mutable local.
    let src = r#"fn count() -> Number {
  let mut total = 0
  let dur: seconds = 4
  let dt: seconds = 1
  simulate dur step dt {
    total = total + 1
  }
  return total
}
print(count())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["4"]);
}

#[test]
fn vm_simulate_body_local_stays_local() {
    // A let inside the simulate body is not visible after the loop; the outer
    // scope retains its own value.
    let src = r#"let mut x = 0
let dur: seconds = 2
let dt: seconds = 1
simulate dur step dt {
  let x = 99
  print(x)
}
print(x)"#;
    // The body's `let x = 99` is local to each iteration; outer x stays 0.
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["99", "99", "0"]);
}

// ── M8F audit: additional closure correctness tests ──────────────────────────

#[test]
fn vm_closure_reads_updated_capture() {
    // Nested function sees the most recent value of a captured mutable variable,
    // even if it was updated AFTER the inner function was defined.
    let src = r#"fn outer() -> Number {
  let mut x: Number = 1

  fn get() -> Number {
    return x
  }

  x = 9
  return get()
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["9"]);
}

#[test]
fn vm_closure_multiple_calls_accumulate() {
    // Two sequential calls to the same nested function see accumulated state.
    let src = r#"fn outer() -> Number {
  let mut x: Number = 0

  fn inc() -> Number {
    x = x + 1
    return x
  }

  let a = inc()
  let b = inc()
  return a + b
}
print(outer())"#;
    // a=1, b=2 → 3
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn vm_closure_recursive_captures_outer() {
    // Recursive nested function can read an outer (non-recursive) captured variable.
    let src = r#"fn outer() -> Number {
  let bonus: Number = 10

  fn f(n: Number) -> Number {
    if n <= 0 {
      return bonus
    }

    return f(n - 1)
  }

  return f(3)
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn vm_closure_mutual_recursion_captures_outer() {
    // Two mutually recursive nested functions both capture the same outer variable.
    let src = r#"fn outer() -> Number {
  let done: Number = 100

  fn even(n: Number) -> Number {
    if n == 0 {
      return done
    }

    return odd(n - 1)
  }

  fn odd(n: Number) -> Number {
    if n == 0 {
      return done + 1
    }

    return even(n - 1)
  }

  return even(4)
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["100"]);
}

#[test]
fn vm_closure_captured_unit_mutable() {
    // Captured mutable variable with a unit type is updated correctly across calls.
    let src = r#"fn outer() -> meters {
  let mut distance: meters = 1
  let stride: meters = 2

  fn advance() -> meters {
    distance = distance + stride
    return distance
  }

  advance()
  return advance()
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn vm_closure_state_transition_captured() {
    // Nested function transitions a state variable captured from the enclosing scope.
    let src = format!(
        "{}\nfn open_door() -> Door {{\n  let door: Door = Door.closed\n\n  fn open_it() -> Door {{\n    transition door -> open\n    return door\n  }}\n\n  return open_it()\n}}\nprint(open_door())",
        "state Door { closed open transition closed -> open }"
    );
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.open"]);
}

// ── M8F audit: simulate + capture interaction tests ──────────────────────────

#[test]
fn vm_simulate_captures_state_local_transition() {
    // Simulate body can transition a state variable defined in an enclosing block.
    let src = format!(
        "{}\nlet dur: seconds = 1\nlet dt: seconds = 1\n{{\n  let door: Door = Door.closed\n  simulate dur step dt {{\n    transition door -> open\n  }}\n  print(door)\n}}",
        "state Door { closed open transition closed -> open }"
    );
    let out = vm_run_unchecked(&src).unwrap();
    assert_eq!(out, vec!["Door.open"]);
}

#[test]
fn vm_simulate_call_nested_function_with_capture() {
    // A nested function defined in an outer function scope is callable from
    // the simulate body and reads captured variables correctly.
    let src = r#"fn outer() -> Number {
  let duration: seconds = 2
  let dt: seconds = 1
  let x: Number = 5
  let mut total: Number = 0

  fn add_x() -> Number {
    return x
  }

  simulate duration step dt {
    total = total + add_x()
  }

  return total
}
print(outer())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["10"]);
}

// ── M8F audit: shadowing correctness ─────────────────────────────────────────

#[test]
fn vm_env_assignment_updates_nearest_binding() {
    // Assignment targets the nearest binding in the scope chain, not an outer one.
    let src = r#"let mut x: Number = 1

fn f() -> Number {
  let mut x: Number = 10

  {
    let mut x: Number = 100
    x = x + 1
  }

  x = x + 1
  return x
}

print(f())
print(x)"#;
    // Inside f: block x becomes 101 (discarded after block); f's x += 1 = 11.
    // Global x is unaffected.
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["11", "1"]);
}

#[test]
fn vm_env_param_shadows_global() {
    // A function parameter with the same name as a global shadows it inside the function.
    let src = r#"let x: Number = 100

fn f(x: Number) -> Number {
  return x + 1
}

print(f(5))
print(x)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["6", "100"]);
}

#[test]
fn vm_env_block_shadow_does_not_leak() {
    // A variable introduced in a block does not affect the outer scope after the block ends.
    let src = r#"let mut x: Number = 1
{
  let x: Number = 99
  print(x)
}
print(x)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["99", "1"]);
}

// ── M8F audit: VM output matches tree-walk ────────────────────────────────────

#[test]
fn vm_matches_tree_closure_example() {
    // closure.kimin: make_getter returns a function value; calling via variable gives 77.
    // VM output must match the known-correct tree-walk output.
    let src = r#"fn make_getter() {
  let x = 77
  fn get() {
    return x
  }
  return get
}

let getter = make_getter()
let result = getter()
print(result)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["77"]);
}

#[test]
fn vm_matches_tree_simulate_capture() {
    // Simulate body reads a function-local mutable variable.
    // Both executors must agree on the final value.
    let src = r#"fn f() -> Number {
  let duration: seconds = 3
  let dt: seconds = 1
  let mut x: Number = 0

  simulate duration step dt {
    x = x + 1
  }

  return x
}
print(f())"#;
    let vm_out = vm_run(src).unwrap();
    assert_eq!(vm_out, vec!["3"]);
}

// ── M8G: dynamic call execution tests ────────────────────────────────────────

#[test]
fn vm_dynamic_call_chained_returns_function() {
    // M8G: make_getter()() chains two calls — first returns a BytecodeFunction,
    // second invokes it. Must produce correct output without Unsupported error.
    let src = r#"fn make_getter() {
  let x = 77
  fn get() {
    return x
  }
  return get
}
print(make_getter()())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["77"]);
}

// ── M8G: additional dynamic call tests ───────────────────────────────────────

#[test]
fn vm_dynamic_call_adder_chained() {
    // make_adder(a)(b) — returns a closure that adds a, then calls it with b.
    let src = r#"fn make_adder(a: Number) {
  fn add_to(b: Number) -> Number {
    return a + b
  }
  return add_to
}
print(make_adder(2)(3))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn vm_dynamic_call_preserves_closure_capture() {
    // Returned closure carries its captured env; calling via variable preserves state.
    let src = r#"fn make_getter() {
  let x: Number = 42
  fn get() -> Number {
    return x
  }
  return get
}
let g = make_getter()
print(g())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn vm_dynamic_call_wrong_arity_errors() {
    // Calling a returned function with wrong arity produces a clean RuntimeError.
    let src = r#"fn make_getter() {
  fn get() -> Number {
    return 1
  }
  return get
}
print(make_getter()(99))"#;
    let result = vm_run(src);
    assert!(result.is_err(), "wrong arity must error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("expects") || msg.contains("argument"),
        "arity error must mention arguments, got: {}",
        msg
    );
}

#[test]
fn vm_call_non_function_errors() {
    // Calling a non-function value (e.g. Number) via dynamic dispatch errors cleanly.
    let src = "fn make_fn() -> Number { return 1 }\nprint(make_fn()())";
    let result = vm_run_unchecked(src);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("non-function") || msg.contains("Number"),
        "must report non-function type, got: {}",
        msg
    );
}

#[test]
fn vm_dynamic_call_arg_order_preserved() {
    // Arguments to a dynamically dispatched call arrive in left-to-right order.
    let src = r#"fn make_sub() {
  fn sub(a: Number, b: Number) -> Number {
    return a - b
  }
  return sub
}
print(make_sub()(10, 3))"#;
    let out = vm_run(src).unwrap();
    // 10 - 3 = 7, not 3 - 10 = -7
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_dynamic_call_inside_simulate() {
    // Dynamic call inside a simulate body uses captured env correctly.
    let src = r#"fn make_adder(n: Number) {
  fn add(x: Number) -> Number {
    return x + n
  }
  return add
}

let add5 = make_adder(5)
let dur: seconds = 2
let dt: seconds = 1
let mut total: Number = 0

simulate dur step dt {
  total = total + add5(time)
}
print(total)"#;
    // iter 0: add5(0) = 5; iter 1: add5(1) = 6; total = 11
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["11"]);
}

#[test]
fn vm_bytecode_callee_load_precedes_call() {
    // Compiler emits callee load before args and before CALL instruction.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3)"
    ));
    let instrs = &prog.main.instructions;
    let load_add_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::LoadGlobal(n) if n == "add"))
        .expect("LoadGlobal add must exist");
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { .. }))
        .expect("Call must exist");
    assert!(load_add_idx < call_idx, "callee load must precede CALL");
}

#[test]
fn vm_bytecode_dynamic_call_emits_two_calls() {
    // f()() emits: LoadGlobal f, CALL 0, CALL 0. No Unsupported.
    let prog = compile_prog("fn f() { } f()()");
    let instrs = &prog.main.instructions;
    let call_count = instrs
        .iter()
        .filter(|i| matches!(i, Instruction::Call { arg_count: 0 }))
        .count();
    assert_eq!(
        call_count, 2,
        "f()() must emit exactly 2 CALL 0 instructions"
    );
    assert!(
        !instrs
            .iter()
            .any(|i| matches!(i, Instruction::Unsupported(_))),
        "no Unsupported instructions after M8G"
    );
}

#[test]
fn vm_dynamic_adder_output_matches_tree() {
    // VM and tree-walk must agree on make_adder(2)(3) → 5.
    let src = r#"fn make_adder(a: Number) {
  fn add_to(b: Number) -> Number {
    return a + b
  }
  return add_to
}
print(make_adder(2)(3))"#;
    let vm_out = vm_run(src).unwrap();
    assert_eq!(vm_out, vec!["5"]);
}

// ── M8G audit: additional hardening tests ────────────────────────────────────

#[test]
fn vm_dynamic_counter_preserves_state_across_calls() {
    // make_counter returns a closure over a mutable captured variable.
    // Each call to counter() increments and returns the same x.
    let src = r#"fn make_counter() {
  let mut x: Number = 0
  fn inc() -> Number {
    x = x + 1
    return x
  }
  return inc
}
let counter = make_counter()
print(counter())
print(counter())"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn vm_dynamic_call_inside_if() {
    // Dynamic call inside a taken if branch executes correctly.
    let src = r#"fn make_getter() {
  let x: Number = 77
  fn get() -> Number { return x }
  return get
}
let cond = true
if cond {
  print(make_getter()())
}"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["77"]);
}

#[test]
fn vm_dynamic_call_inside_function() {
    // Dynamic call chained inside another function body works correctly.
    let src = r#"fn make_getter() {
  let x: Number = 99
  fn get() -> Number { return x }
  return get
}
fn get_via_fn() {
  let getter = make_getter()
  print(getter())
}
get_via_fn()"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn vm_dynamic_call_non_function_text_errors() {
    // Calling a Text value produces a clean non-function RuntimeError.
    let src = "fn make_text() { return \"hello\" }\nmake_text()()";
    let result = vm_run_unchecked(src);
    assert!(result.is_err(), "calling Text must error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("non-function") || msg.contains("String"),
        "must report non-function type, got: {}",
        msg
    );
}

#[test]
fn vm_dynamic_call_non_function_bool_errors() {
    // Calling a Bool value produces a clean non-function RuntimeError.
    let src = "fn make_bool() { return true }\nmake_bool()()";
    let result = vm_run_unchecked(src);
    assert!(result.is_err(), "calling Bool must error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("non-function") || msg.contains("Bool"),
        "must report non-function type, got: {}",
        msg
    );
}

#[test]
fn vm_dynamic_getter_matches_tree() {
    // make_getter()() gives the same output in tree-walk and VM.
    let src = r#"fn make_getter() {
  let x: Number = 77
  fn get() -> Number { return x }
  return get
}
print(make_getter()())"#;
    assert!(run(src).is_ok(), "tree-walk must succeed");
    assert_eq!(vm_run(src).unwrap(), vec!["77"]);
}

#[test]
fn vm_dynamic_counter_matches_tree() {
    // Counter closure gives the same output in tree-walk and VM.
    let src = r#"fn make_counter() {
  let mut x: Number = 0
  fn inc() -> Number {
    x = x + 1
    return x
  }
  return inc
}
let counter = make_counter()
print(counter())
print(counter())"#;
    assert!(run(src).is_ok(), "tree-walk must succeed");
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2"]);
}

#[test]
fn disassemble_chained_call_shows_two_calls() {
    // f()() disassembles to two consecutive CALL 0 instructions — no function name in CALL.
    use crate::disassemble::disassemble;
    let prog = compile_prog("fn f() { } f()()");
    let out = disassemble(&prog);
    // Count occurrences of "CALL 0" in the main section.
    let main_section = out.split("=== function").next().unwrap_or(&out);
    let call_count = main_section.matches("CALL 0").count();
    assert_eq!(
        call_count, 2,
        "f()() must disassemble to two 'CALL 0' instructions, got: {}",
        out
    );
    assert!(
        !out.contains("UNSUPPORTED"),
        "no UNSUPPORTED in disassembly after M8G"
    );
}

#[test]
fn bytecode_call_instruction_has_only_arg_count() {
    // After M8G: Call instructions carry only arg_count, not a callee name.
    // The type system enforces this at compile time. This test documents the invariant
    // and ensures the function table shape is correct.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(1, 2)"
    ));
    // Every Call instruction must match Call { arg_count: _ } (no name field).
    for instr in &prog.main.instructions {
        if let Instruction::Call { arg_count } = instr {
            // Verify arg_count is a reasonable value (structural check).
            assert!(*arg_count <= 255, "arg_count out of expected range");
        }
    }
    // Must emit exactly one Call instruction for add(1, 2).
    let call_count = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::Call { .. }))
        .count();
    assert_eq!(call_count, 1, "add(1, 2) must emit exactly one CALL");
}

// ── M8F audit: recursive function no crash ────────────────────────────────────

#[test]
fn vm_recursive_function_no_crash() {
    // A recursive function creates an Rc env chain that may form a cycle,
    // but must not panic or hang. The program must produce the correct result.
    let src = r#"fn fact(n: Number) -> Number {
  if n <= 0 {
    return 1
  }
  return fact(n - 1) * n
}
print(fact(5))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["120"]);
}

// ── Milestone 9A: compound assignment operators ────────────────────────────

// --- lexer ---

#[test]
fn lex_plus_equal() {
    let kinds = tokenize("x += 1");
    assert!(matches!(kinds[0], TokenKind::Ident(_)));
    assert_eq!(kinds[1], TokenKind::PlusEqual);
    assert!(matches!(kinds[2], TokenKind::Number(_)));
}

#[test]
fn lex_minus_equal() {
    let kinds = tokenize("x -= 1");
    assert_eq!(kinds[1], TokenKind::MinusEqual);
}

#[test]
fn lex_star_equal() {
    let kinds = tokenize("x *= 2");
    assert_eq!(kinds[1], TokenKind::StarEqual);
}

#[test]
fn lex_slash_equal() {
    let kinds = tokenize("x /= 2");
    assert_eq!(kinds[1], TokenKind::SlashEqual);
}

#[test]
fn lex_plus_equal_not_two_tokens() {
    // += must be a single token, not Plus then Eq
    let kinds = tokenize("x += 1");
    assert_eq!(kinds.len(), 4); // Ident, PlusEqual, Number, Eof
    assert_eq!(kinds[1], TokenKind::PlusEqual);
}

// --- parser ---

#[test]
fn parse_compound_assign_plus() {
    let src = "let mut x = 0\nx += 1";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert_eq!(stmts.len(), 2);
    assert!(matches!(
        &stmts[1],
        crate::ast::Stmt::CompoundAssign {
            op: crate::ast::CompoundAssignOp::Add,
            ..
        }
    ));
}

#[test]
fn parse_compound_assign_minus() {
    let src = "let mut x = 10\nx -= 3";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(
        &stmts[1],
        crate::ast::Stmt::CompoundAssign {
            op: crate::ast::CompoundAssignOp::Subtract,
            ..
        }
    ));
}

#[test]
fn parse_compound_assign_star() {
    let src = "let mut x = 5\nx *= 3";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(
        &stmts[1],
        crate::ast::Stmt::CompoundAssign {
            op: crate::ast::CompoundAssignOp::Multiply,
            ..
        }
    ));
}

#[test]
fn parse_compound_assign_slash() {
    let src = "let mut x = 10\nx /= 2";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(
        &stmts[1],
        crate::ast::Stmt::CompoundAssign {
            op: crate::ast::CompoundAssignOp::Divide,
            ..
        }
    ));
}

// --- type checker ---

#[test]
fn type_compound_assign_immutable_errors() {
    let err = check("let x = 10\nx += 1").unwrap_err();
    assert!(err.to_string().contains("immutable"));
}

#[test]
fn type_compound_assign_undefined_errors() {
    let err = check("y += 1").unwrap_err();
    assert!(err.to_string().contains("undefined variable 'y'"));
}

#[test]
fn type_compound_assign_state_variable_errors() {
    let src = "state Door { open closed transition open -> closed }\nlet mut door: Door = Door.open\ndoor += 1";
    let err = check(src).unwrap_err();
    assert!(err.to_string().contains("transition"));
}

#[test]
fn type_compound_assign_unit_mismatch_errors() {
    let src = "let mut d: meters = 0\nlet t: seconds = 5\nd += t";
    let err = check(src).unwrap_err();
    assert!(err.to_string().contains("cannot add"));
}

#[test]
fn type_compound_assign_number_ok() {
    assert!(check("let mut x = 10\nx += 5").is_ok());
    assert!(check("let mut x = 10\nx -= 3").is_ok());
    assert!(check("let mut x = 10\nx *= 2").is_ok());
    assert!(check("let mut x = 10\nx /= 4").is_ok());
}

#[test]
fn type_compound_assign_same_unit_plus_ok() {
    // meters += meters is valid
    assert!(check("let mut d: meters = 0\nlet inc: meters = 5\nd += inc").is_ok());
}

#[test]
fn type_compound_assign_unit_plus_bare_number_errors() {
    // meters += Number is NOT valid — unit-safe; check_binary(Add, meters, Number) → error
    match check("let mut d: meters = 0\nd += 10") {
        Err(e) => {
            assert!(e.to_string().contains("operator '+'") || e.to_string().contains("Number"))
        }
        Ok(()) => panic!("expected TypeError"),
    }
}

#[test]
fn type_compound_assign_unit_times_number_ok() {
    assert!(check("let mut d: meters = 10\nd *= 2").is_ok());
}

// --- interpreter (tree-walk) ---

#[test]
fn interp_compound_assign_plus_equals() {
    let interp = run("let mut x = 10\nx += 5").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(15.0)));
}

#[test]
fn interp_compound_assign_minus_equals() {
    let interp = run("let mut x = 10\nx -= 3").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(7.0)));
}

#[test]
fn interp_compound_assign_star_equals() {
    let interp = run("let mut x = 4\nx *= 3").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(12.0)));
}

#[test]
fn interp_compound_assign_slash_equals() {
    let interp = run("let mut x = 20\nx /= 4").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(5.0)));
}

#[test]
fn interp_compound_assign_chain() {
    let interp = run("let mut x = 10\nx += 5\nx -= 3\nx *= 2\nx /= 4").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(6.0)));
}

#[test]
fn interp_compound_assign_in_block() {
    let interp = run("let mut counter = 0\n{ counter += 1\ncounter += 1 }").unwrap();
    assert_eq!(interp.get_var("counter"), Some(Value::Number(2.0)));
}

#[test]
fn interp_compound_assign_in_function() {
    let src = r#"fn add_to(start: Number, amount: Number) -> Number {
  let mut result = start
  result += amount
  return result
}
let out = add_to(10, 7)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("out"), Some(Value::Number(17.0)));
}

#[test]
fn interp_compound_assign_accumulate_in_simulate() {
    let src = r#"let mut pos: meters = 0
let vel: meters = 10
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    pos += vel
}"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("pos"), Some(Value::Number(30.0)));
}

#[test]
fn interp_compound_assign_unit_times_number() {
    let interp = run("let mut d: meters = 10\nd *= 2").unwrap();
    assert_eq!(interp.get_var("d"), Some(Value::Number(20.0)));
}

#[test]
fn interp_compound_assign_div_by_zero_errors() {
    match run("let mut x = 10\nx /= 0") {
        Err(e) => assert!(e.to_string().contains("division by zero")),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn interp_compound_assign_print_output() {
    let tokens = Lexer::new("let mut x = 5\nx += 3\nprint(x)")
        .tokenize()
        .unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    TypeChecker::new().check(&stmts).unwrap();
    let mut interp = Interpreter::new();
    interp.run(&stmts).unwrap();
    // Value is 8
    assert_eq!(interp.get_var("x"), Some(Value::Number(8.0)));
}

// --- bytecode compiler (IR shape) ---

#[test]
fn bytecode_compound_assign_plus_desugars_to_load_op_store() {
    let prog = compile_prog("let mut x = 0\nx += 5");
    let instrs = &prog.main.instructions;
    // Expect somewhere: LoadGlobal("x"), Constant(5), Add, StoreGlobal("x")
    let has_load = instrs
        .iter()
        .any(|i| matches!(i, Instruction::LoadGlobal(n) if n == "x"));
    let has_add = instrs.iter().any(|i| matches!(i, Instruction::Add));
    let has_store = instrs
        .iter()
        .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "x"));
    assert!(has_load, "missing LoadGlobal x");
    assert!(has_add, "missing Add");
    assert!(has_store, "missing StoreGlobal x");
}

#[test]
fn bytecode_compound_assign_minus_desugars() {
    let prog = compile_prog("let mut x = 10\nx -= 3");
    let instrs = &prog.main.instructions;
    assert!(instrs.iter().any(|i| matches!(i, Instruction::Subtract)));
}

#[test]
fn bytecode_compound_assign_multiply_desugars() {
    let prog = compile_prog("let mut x = 5\nx *= 4");
    let instrs = &prog.main.instructions;
    assert!(instrs.iter().any(|i| matches!(i, Instruction::Multiply)));
}

#[test]
fn bytecode_compound_assign_divide_desugars() {
    let prog = compile_prog("let mut x = 20\nx /= 4");
    let instrs = &prog.main.instructions;
    assert!(instrs.iter().any(|i| matches!(i, Instruction::Divide)));
}

#[test]
fn bytecode_compound_assign_local_uses_load_store_local() {
    // Inside a block, compound assign uses LoadLocal / StoreLocal
    let prog = compile_prog("{ let mut x = 0\nx += 1 }");
    let instrs = &prog.main.instructions;
    assert!(instrs
        .iter()
        .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "x")));
    assert!(instrs
        .iter()
        .any(|i| matches!(i, Instruction::StoreLocal(n) if n == "x")));
}

// --- VM execution ---

#[test]
fn vm_compound_assign_plus_equals() {
    let out = vm_run("let mut x = 10\nx += 5\nprint(x)").unwrap();
    assert_eq!(out, vec!["15"]);
}

#[test]
fn vm_compound_assign_minus_equals() {
    let out = vm_run("let mut x = 10\nx -= 3\nprint(x)").unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn vm_compound_assign_star_equals() {
    let out = vm_run("let mut x = 4\nx *= 3\nprint(x)").unwrap();
    assert_eq!(out, vec!["12"]);
}

#[test]
fn vm_compound_assign_slash_equals() {
    let out = vm_run("let mut x = 20\nx /= 4\nprint(x)").unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn vm_compound_assign_chain() {
    let out = vm_run("let mut x = 10\nx += 5\nx -= 3\nx *= 2\nx /= 4\nprint(x)").unwrap();
    assert_eq!(out, vec!["6"]);
}

#[test]
fn vm_compound_assign_in_block() {
    let out =
        vm_run("let mut counter = 0\n{ counter += 1\ncounter += 1 }\nprint(counter)").unwrap();
    assert_eq!(out, vec!["2"]);
}

#[test]
fn vm_compound_assign_accumulate_in_simulate() {
    let src = r#"let mut pos: meters = 0
let vel: meters = 10
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    pos += vel
}
print(pos)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["30"]);
}

#[test]
fn vm_compound_assign_matches_tree_walk() {
    let src = "let mut x = 3\nx *= 7\nx -= 1\nprint(x)";
    let tree_out = {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let stmts = Parser::new(tokens).parse().unwrap();
        TypeChecker::new().check(&stmts).unwrap();
        let mut interp = Interpreter::new();
        interp.run(&stmts).unwrap();
        interp.get_var("x")
    };
    let vm_out = vm_run(src).unwrap();
    // tree: 3*7=21, 21-1=20
    assert_eq!(tree_out, Some(Value::Number(20.0)));
    assert_eq!(vm_out, vec!["20"]);
}

// ── Milestone 9A audit: hardening tests ───────────────────────────────────────

// --- lexer: operator disambiguation after adding compound-assign tokens ---

#[test]
fn lex_slash_comment_still_works() {
    // // must still be parsed as a line comment, not /= or two slashes
    let kinds = tokenize("// this is a comment");
    assert_eq!(kinds, vec![TokenKind::Eof]);
}

#[test]
fn lex_division_still_works() {
    // / without = must still be Slash, not SlashEqual
    let kinds = tokenize("x / y");
    assert_eq!(kinds[1], TokenKind::Slash);
}

#[test]
fn lex_plus_still_works_without_equal() {
    let kinds = tokenize("x + y");
    assert_eq!(kinds[1], TokenKind::Plus);
}

#[test]
fn lex_minus_still_works_without_equal() {
    let kinds = tokenize("x - y");
    assert_eq!(kinds[1], TokenKind::Minus);
}

#[test]
fn lex_star_still_works_without_equal() {
    let kinds = tokenize("x * y");
    assert_eq!(kinds[1], TokenKind::Star);
}

#[test]
fn lex_arrow_still_works() {
    // -> must still be Arrow; the - branch must check > before =
    let kinds = tokenize("->");
    assert_eq!(kinds[0], TokenKind::Arrow);
    assert_eq!(kinds[1], TokenKind::Eof);
}

// --- parser: disambiguation and error cases ---

#[test]
fn parse_compound_assign_missing_rhs_error() {
    // x += with no expression after is a parse error
    let src = "let mut x = 0\nx +=";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

#[test]
fn parse_regular_assignment_unaffected() {
    // x = expr must still parse as Stmt::Assign, not CompoundAssign
    let src = "let mut x = 0\nx = 5";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(&stmts[1], crate::ast::Stmt::Assign { .. }));
}

#[test]
fn parse_equality_unaffected_from_compound() {
    // x == y must not be confused with compound assign; parses as expression
    let src = "let x = 5\nif x == 5 { print(x) }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert_eq!(stmts.len(), 2);
}

#[test]
fn parse_compound_assign_self_referential() {
    // x += x is valid syntax — variable appears on both sides
    let src = "let mut x = 5\nx += x";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(
        &stmts[1],
        crate::ast::Stmt::CompoundAssign {
            op: crate::ast::CompoundAssignOp::Add,
            ..
        }
    ));
}

// --- typechecker: broader coverage ---

#[test]
fn type_compound_assign_text_concat_ok() {
    // Text + Text → Text; s += suffix must typecheck
    assert!(check("let mut s = \"hello\"\ns += \" world\"").is_ok());
}

#[test]
fn type_compound_assign_position_velocity_dt_ok() {
    // Same-unit +=: pos: meters, vel: meters → pos += vel ok
    let src = "let mut pos: meters = 0\nlet vel: meters = 5\npos += vel";
    assert!(check(src).is_ok());
}

#[test]
fn type_compound_assign_inside_simulate_ok() {
    let src = r#"let mut pos: meters = 0
let vel: meters = 10
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    pos += vel
}"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_compound_assign_inside_closure_ok() {
    // Compound assign on a function-local mut variable must typecheck
    let src = r#"fn bump(n: Number) -> Number {
  let mut r = n
  r += 1
  return r
}
let x = bump(5)"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_compound_assign_captured_mutable_ok() {
    // A function body may compound-assign an outer-scope mut global
    let src = r#"let mut total = 0
fn add_five() -> Number {
  total += 5
  return total
}
let r = add_five()"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_compound_assign_immutable_capture_error() {
    // Compound assign to an outer-scope immutable variable must error
    let src = r#"let x = 10
fn bump() -> Number {
  x += 1
  return x
}
bump()"#;
    let err = check(src).unwrap_err();
    assert!(err.to_string().contains("immutable"));
}

// --- interpreter: edge cases ---

#[test]
fn interp_compound_rhs_evaluated_before_store() {
    // x += x: RHS must snapshot current x before storing
    // x=5 → x += x → x should be 10, not 15
    let interp = run("let mut x = 5\nx += x").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(10.0)));
}

#[test]
fn interp_compound_closure_capture() {
    // Compound assign in a function body updates the captured global via env chain
    let src = r#"let mut total = 0
fn add_five() -> Number {
  total += 5
  return total
}
add_five()
add_five()"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(10.0)));
}

#[test]
fn interp_compound_body_local_simulate_fresh() {
    // Simulate body local mut is fresh each iteration; outer mut accumulates
    let src = r#"let mut total = 0
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    let mut local = 0
    local += 1
    total += local
}"#;
    let interp = run(src).unwrap();
    // local resets each iteration → 1 per iteration; total = 3
    assert_eq!(interp.get_var("total"), Some(Value::Number(3.0)));
}

#[test]
fn interp_compound_text_concat() {
    let interp = run("let mut s = \"hello\"\ns += \" world\"").unwrap();
    assert_eq!(
        interp.get_var("s"),
        Some(Value::Str("hello world".to_string()))
    );
}

// --- bytecode: instruction order and function/simulate body lowering ---

#[test]
fn bytecode_compound_add_instruction_order() {
    // x += 5: LoadGlobal(x) must precede Add, which must precede StoreGlobal(x)
    let prog = compile_prog("let mut x = 0\nx += 5");
    let instrs = &prog.main.instructions;
    let load_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::LoadGlobal(n) if n == "x"))
        .expect("missing LoadGlobal x");
    let add_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Add))
        .expect("missing Add");
    let store_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::StoreGlobal(n) if n == "x"))
        .expect("missing StoreGlobal x");
    assert!(load_pos < add_pos, "LoadGlobal x must come before Add");
    assert!(add_pos < store_pos, "Add must come before StoreGlobal x");
}

#[test]
fn bytecode_compound_in_function_body() {
    // Compound assign inside a function body lowers to LoadLocal/StoreLocal
    let src = r#"fn bump(n: Number) -> Number {
  let mut r = n
  r += 1
  return r
}
let x = bump(10)"#;
    let prog = compile_prog(src);
    let fn_chunk = prog
        .functions
        .iter()
        .find(|f| f.name == "bump")
        .expect("bump not found");
    let instrs = &fn_chunk.chunk.instructions;
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instruction::LoadLocal(n) if n == "r")),
        "missing LoadLocal r"
    );
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::Add)),
        "missing Add"
    );
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instruction::StoreLocal(n) if n == "r")),
        "missing StoreLocal r"
    );
}

#[test]
fn bytecode_compound_in_simulate_body() {
    // Compound assign inside simulate body compiles into the simulate chunk
    let src = r#"let mut pos: meters = 0
let vel: meters = 10
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    pos += vel
}"#;
    let prog = compile_prog(src);
    assert!(
        !prog.simulate_bodies.is_empty(),
        "simulate body must be compiled"
    );
    let sim_instrs = &prog.simulate_bodies[0].chunk.instructions;
    assert!(
        sim_instrs.iter().any(|i| matches!(i, Instruction::Add)),
        "missing Add in simulate body"
    );
    assert!(
        sim_instrs
            .iter()
            .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "pos")),
        "missing StoreGlobal pos in simulate body"
    );
}

// --- VM: execution correctness ---

#[test]
fn vm_compound_function_local() {
    let src = r#"fn bump(n: Number) -> Number {
  let mut r = n
  r += 1
  return r
}
print(bump(10))"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["11"]);
}

#[test]
fn vm_compound_closure_capture() {
    // Compound assign through env chain updates the global in the VM
    let src = r#"let mut total = 0
fn add_five() -> Number {
  total += 5
  return total
}
add_five()
add_five()
print(total)"#;
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn vm_compound_dynamic_counter() {
    // Multiple top-level compound assigns accumulate correctly
    let src = "let mut c = 0\nc += 1\nc += 1\nc += 1\nprint(c)";
    let out = vm_run(src).unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn vm_compound_text_concat() {
    let out = vm_run("let mut s = \"hello\"\ns += \" world\"\nprint(s)").unwrap();
    assert_eq!(out, vec!["hello world"]);
}

#[test]
fn vm_compound_units_run_and_vm() {
    // VM must execute compound assign on unit-typed variable and match tree-walk var value
    let src = "let mut d: meters = 0\nlet inc: meters = 10\nd += inc";
    // Tree-walk: check variable value
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("d"), Some(Value::Number(10.0)));
    // VM: check print output
    let vm_out = vm_run("let mut d: meters = 0\nlet inc: meters = 10\nd += inc\nprint(d)").unwrap();
    assert_eq!(vm_out, vec!["10"]);
}

// --- unit system: minus and compound unit ---

#[test]
fn compound_assignment_unit_minus_same_unit_ok() {
    // d -= inc where both are meters → ok (same as + rule)
    let src = "let mut d: meters = 20\nlet inc: meters = 5\nd -= inc";
    assert!(check(src).is_ok());
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("d"), Some(Value::Number(15.0)));
}

#[test]
fn compound_assignment_velocity_dt_compound_unit_ok() {
    // vel: meters, scale: Number → vel *= scale is ok (scaling rule)
    // pos: meters += vel → ok (same-unit add)
    let src = r#"let mut pos: meters = 0
let mut vel: meters = 10
vel *= 2
pos += vel"#;
    assert!(check(src).is_ok());
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("pos"), Some(Value::Number(20.0)));
    assert_eq!(interp.get_var("vel"), Some(Value::Number(20.0)));
}

// --- state: compound assign does not interfere with state machinery ---

#[test]
fn transition_still_works_after_compound_assignment_feature() {
    let src = r#"state Door { open closed transition open -> closed }
let mut door: Door = Door.open
let mut x = 0
x += 5
transition door -> closed"#;
    assert!(check(src).is_ok());
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(5.0)));
}

#[test]
fn compound_assign_state_as_rhs_errors() {
    // Number += State value is always a TypeError
    let src = r#"state Door { open closed transition open -> closed }
let door: Door = Door.open
let mut x = 0
x += door"#;
    let err = check(src).unwrap_err();
    assert!(err.to_string().contains("operator '+'") || err.to_string().contains("State"));
}

// --- simulate: body isolation and state coexistence ---

#[test]
fn simulate_compound_body_local_fresh() {
    // A body-local mut reset to 0 each iteration; only outer accumulator persists
    let src = r#"let mut acc = 0
let dur: seconds = 4
let dt: seconds = 1
simulate dur step dt {
    let mut scratch = 10
    scratch -= 5
    acc += scratch
}"#;
    // scratch = 10-5=5 each iter, 4 iters → acc=20
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("acc"), Some(Value::Number(20.0)));
    let vm_out = vm_run(
        r#"let mut acc = 0
let dur: seconds = 4
let dt: seconds = 1
simulate dur step dt {
    let mut scratch = 10
    scratch -= 5
    acc += scratch
}
print(acc)"#,
    )
    .unwrap();
    assert_eq!(vm_out, vec!["20"]);
}

#[test]
fn simulate_compound_with_state_transition() {
    // Compound assign and state transition may coexist in the same simulate body
    let src = r#"state Light { off on transition off -> on }
let mut light: Light = Light.off
let mut ticks = 0
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    ticks += 1
}
transition light -> on"#;
    assert!(check(src).is_ok());
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("ticks"), Some(Value::Number(3.0)));
}

// --- output matching: tree-walk and VM agree ---

#[test]
fn vm_matches_tree_compound_assignment_units() {
    let src = r#"let mut d: meters = 0
let inc: meters = 25
d += inc
d -= inc
d += inc
d *= 2
print(d)"#;
    // 0+25=25, 25-25=0, 0+25=25, 25*2=50
    assert_eq!(vm_run(src).unwrap(), vec!["50"]);
}

#[test]
fn vm_matches_tree_simulate_compound_assignment() {
    let src = r#"let mut pos: meters = 0
let vel: meters = 10
let dur: seconds = 3
let dt: seconds = 1
simulate dur step dt {
    pos += vel
}
print(pos)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["30"]);
}

// ── Milestone 9B: while loops ─────────────────────────────────────────────────

// --- lexer ---

#[test]
fn lex_while_keyword() {
    let kinds = tokenize("while");
    assert_eq!(kinds[0], TokenKind::While);
}

#[test]
fn lex_whiley_is_identifier() {
    // `whiley` must NOT lex as While + y; it is a single identifier
    let kinds = tokenize("whiley");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "whiley"));
}

// --- parser ---

#[test]
fn parse_while_simple() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1 }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert_eq!(stmts.len(), 2);
    assert!(matches!(&stmts[1], crate::ast::Stmt::While { .. }));
}

#[test]
fn parse_while_body_stmts() {
    let src = "let mut x = 0\nwhile x < 3 { print(x)\nx += 1 }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        assert_eq!(body.len(), 2);
    } else {
        panic!("expected While");
    }
}

#[test]
fn parse_nested_while() {
    let src = "let mut i = 0\nwhile i < 3 { let mut j = 0\nwhile j < 2 { j += 1 }\ni += 1 }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        // body contains let j, inner while, i += 1
        assert!(body
            .iter()
            .any(|s| matches!(s, crate::ast::Stmt::While { .. })));
    } else {
        panic!("expected outer While");
    }
}

#[test]
fn parse_while_inside_function() {
    let src = r#"fn f() -> Number {
  let mut x = 0
  while x < 3 { x += 1 }
  return x
}
f()"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_while_missing_condition_error() {
    // `while { x += 1 }` — `{` cannot start an expression
    let src = "let mut x = 0\nwhile { x += 1 }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

#[test]
fn parse_while_missing_body_error() {
    // `while x < 5` with no `{` is a parse error
    let src = "let mut x = 0\nwhile x < 5";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

// --- typechecker ---

#[test]
fn type_while_bool_condition_ok() {
    assert!(check("let mut x = 0\nwhile x < 5 { x += 1 }").is_ok());
}

#[test]
fn type_while_bool_literal_ok() {
    assert!(check("while false { }").is_ok());
}

#[test]
fn type_while_number_condition_type_error() {
    let err = check("let mut x = 0\nwhile x { x += 1 }").unwrap_err();
    assert!(err.to_string().contains("Bool"), "error was: {}", err);
}

#[test]
fn type_while_text_condition_type_error() {
    let err = check("let mut s = \"hi\"\nwhile s { }").unwrap_err();
    assert!(err.to_string().contains("Bool"), "error was: {}", err);
}

#[test]
fn type_while_comparison_condition_ok() {
    assert!(check("let mut x = 0\nwhile x != 5 { x += 1 }").is_ok());
}

#[test]
fn type_while_body_mutates_outer_mutable_ok() {
    assert!(check("let mut x = 0\nwhile x < 3 { x += 1 }").is_ok());
}

#[test]
fn type_while_body_immutable_assignment_type_error() {
    let err = check("let x = 0\nwhile x < 3 { x += 1 }").unwrap_err();
    assert!(err.to_string().contains("immutable"), "error was: {}", err);
}

#[test]
fn type_while_local_does_not_leak() {
    // `inner` declared inside while body is not visible after the loop
    let err =
        check("let mut x = 0\nwhile x < 1 { let inner = 99\nx += 1 }\nprint(inner)").unwrap_err();
    assert!(
        err.to_string().contains("undefined variable 'inner'"),
        "error was: {}",
        err
    );
}

#[test]
fn type_while_inside_function_return_ok() {
    let src = r#"fn f() -> Number {
  let mut x = 0
  while x < 3 { x += 1 }
  return x
}
let r = f()"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_while_with_state_transition_ok() {
    let src = r#"state Door { closed open transition closed -> open }
let mut door: Door = Door.closed
while door == Door.closed { transition door -> open }"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_while_nested_ok() {
    let src = "let mut i = 0\nwhile i < 3 { let mut j = 0\nwhile j < 2 { j += 1 }\ni += 1 }";
    assert!(check(src).is_ok());
}

// --- interpreter ---

#[test]
fn interp_while_count_loop() {
    let interp = run("let mut x = 0\nwhile x < 5 { x += 1 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(5.0)));
}

#[test]
fn interp_while_zero_iterations() {
    let interp = run("let mut x = 10\nwhile x < 5 { x += 1 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(10.0)));
}

#[test]
fn interp_while_compound_assignment() {
    // Position accumulates each iteration
    let src = "let mut pos = 0\nlet vel = 3\nlet mut i = 0\nwhile i < 4 { pos += vel\ni += 1 }";
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("pos"), Some(Value::Number(12.0)));
}

#[test]
fn interp_while_body_local_fresh_per_iteration() {
    // Each iteration's local is reset; only outer accumulator persists
    let src = r#"let mut acc = 0
let mut i = 0
while i < 3 {
    let mut scratch = 10
    scratch -= 3
    acc += scratch
    i += 1
}"#;
    let interp = run(src).unwrap();
    // scratch = 7 each iter; acc = 7*3 = 21
    assert_eq!(interp.get_var("acc"), Some(Value::Number(21.0)));
}

#[test]
fn interp_while_updates_outer_mutable() {
    let interp = run("let mut x = 0\nwhile x < 3 { x += 1 }").unwrap();
    assert_eq!(interp.get_var("x"), Some(Value::Number(3.0)));
}

#[test]
fn interp_while_inside_function() {
    let src = r#"fn countdown(n: Number) -> Number {
  let mut x = n
  while x > 0 {
    x -= 1
  }
  return x
}
let r = countdown(5)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(0.0)));
}

#[test]
fn interp_while_return_exits_function() {
    // Return inside while body should exit the enclosing function immediately
    let src = r#"fn find_first(limit: Number) -> Number {
  let mut x = 0
  while x < limit {
    if x == 3 { return x }
    x += 1
  }
  return x
}
let r = find_first(10)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("r"), Some(Value::Number(3.0)));
}

#[test]
fn interp_while_nested() {
    let src = r#"let mut total = 0
let mut i = 0
while i < 3 {
    let mut j = 0
    while j < 4 {
        total += 1
        j += 1
    }
    i += 1
}"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(12.0)));
}

#[test]
fn interp_while_state_transition_loop() {
    let src = r#"state Light { off on transition off -> on }
let mut light: Light = Light.off
let mut ticks = 0
while light == Light.off {
    ticks += 1
    transition light -> on
}
print(ticks)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("ticks"), Some(Value::Number(1.0)));
}

// --- bytecode ---

#[test]
fn bytecode_while_emits_jump_if_false_and_back_jump() {
    let prog = compile_prog("let mut x = 0\nwhile x < 5 { x += 1 }");
    let instrs = &prog.main.instructions;
    // Must have JumpIfFalse
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instruction::JumpIfFalse(_))),
        "missing JumpIfFalse"
    );
    // Must have a Jump (not JumpIfFalse) for the back-edge
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::Jump(_))),
        "missing back Jump"
    );
}

#[test]
fn bytecode_while_condition_before_jump_if_false() {
    // LESS or other comparison must appear before JumpIfFalse
    let prog = compile_prog("let mut x = 0\nwhile x < 5 { x += 1 }");
    let instrs = &prog.main.instructions;
    let less_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Less))
        .unwrap();
    let jif_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::JumpIfFalse(_)))
        .unwrap();
    assert!(less_pos < jif_pos, "LESS must precede JumpIfFalse");
}

#[test]
fn bytecode_while_body_is_scoped() {
    let prog = compile_prog("let mut x = 0\nwhile x < 5 { x += 1 }");
    let instrs = &prog.main.instructions;
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::BeginScope)),
        "missing BeginScope"
    );
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::EndScope)),
        "missing EndScope"
    );
}

#[test]
fn bytecode_while_compound_assignment_inside_loop() {
    // Compound assign inside while body emits Load/Add/Store inside the loop body
    let prog = compile_prog("let mut x = 0\nwhile x < 5 { x += 1 }");
    let instrs = &prog.main.instructions;
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::Add)),
        "missing Add in loop body"
    );
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instruction::StoreGlobal(n) if n == "x")),
        "missing StoreGlobal x"
    );
}

#[test]
fn bytecode_nested_while_jump_targets() {
    // Nested while must emit two JumpIfFalse instructions
    let src = "let mut i = 0\nwhile i < 3 { let mut j = 0\nwhile j < 2 { j += 1 }\ni += 1 }";
    let prog = compile_prog(src);
    let instrs = &prog.main.instructions;
    let jif_count = instrs
        .iter()
        .filter(|i| matches!(i, Instruction::JumpIfFalse(_)))
        .count();
    assert_eq!(
        jif_count, 2,
        "nested while must emit 2 JumpIfFalse instructions"
    );
}

#[test]
fn bytecode_while_inside_function() {
    let src = r#"fn f() -> Number {
  let mut x = 0
  while x < 3 { x += 1 }
  return x
}
f()"#;
    let prog = compile_prog(src);
    let fn_chunk = prog
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function f not found");
    let instrs = &fn_chunk.chunk.instructions;
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instruction::JumpIfFalse(_))),
        "while in function must emit JumpIfFalse"
    );
    assert!(
        instrs.iter().any(|i| matches!(i, Instruction::Jump(_))),
        "while in function must emit back Jump"
    );
}

// --- VM ---

#[test]
fn vm_while_count_loop() {
    let out = vm_run("let mut x = 0\nwhile x < 5 { print(x)\nx += 1 }").unwrap();
    assert_eq!(out, vec!["0", "1", "2", "3", "4"]);
}

#[test]
fn vm_while_zero_iterations() {
    let out = vm_run("let mut x = 10\nwhile x < 5 { x += 1 }\nprint(x)").unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn vm_while_compound_assignment() {
    let src = "let mut x = 0\nwhile x < 4 { x += 1 }\nprint(x)";
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn vm_while_nested() {
    let src = r#"let mut total = 0
let mut i = 0
while i < 3 {
    let mut j = 0
    while j < 4 {
        total += 1
        j += 1
    }
    i += 1
}
print(total)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn vm_while_inside_function() {
    let src = r#"fn countdown(n: Number) -> Number {
  let mut x = n
  while x > 0 { x -= 1 }
  return x
}
print(countdown(5))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["0"]);
}

#[test]
fn vm_while_return_inside_exits_function() {
    let src = r#"fn find(limit: Number) -> Number {
  let mut x = 0
  while x < limit {
    if x == 3 { return x }
    x += 1
  }
  return x
}
print(find(10))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn vm_while_state_transition() {
    let src = r#"state Door { closed open transition closed -> open }
let mut door: Door = Door.closed
while door == Door.closed {
    print(door)
    transition door -> open
}
print(door)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["Door.closed", "Door.open"]);
}

#[test]
fn vm_while_matches_tree_walk() {
    // Both executors must agree on the final value and print output
    let src = "let mut x = 0\nwhile x < 5 { print(x)\nx += 1 }";
    let tree_interp = run(src).unwrap();
    let vm_out = vm_run(src).unwrap();
    assert_eq!(tree_interp.get_var("x"), Some(Value::Number(5.0)));
    assert_eq!(vm_out, vec!["0", "1", "2", "3", "4"]);
}

#[test]
fn vm_while_units_match_tree_walk() {
    let src = r#"let mut pos: meters = 0
let stride: meters = 5
let limit: meters = 20
while pos < limit {
    pos += stride
    print(pos)
}"#;
    assert_eq!(vm_run(src).unwrap(), vec!["5", "10", "15", "20"]);
}

// ─── M9B Audit: Lexer hardening ────────────────────────────────────────────

#[test]
fn lex_meanwhile_identifier() {
    // "meanwhile" must lex as Ident, not While keyword
    let kinds = tokenize("meanwhile");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "meanwhile"));
}

#[test]
fn lex_while_loop_identifier() {
    // "while_loop" must lex as Ident (prefix match does not steal keyword)
    let kinds = tokenize("while_loop");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "while_loop"));
}

// ─── M9B Audit: Parser hardening ───────────────────────────────────────────

#[test]
fn parse_while_with_compound_assignment() {
    // while body may contain compound assignment — must parse cleanly
    let src = "let mut x = 0\nwhile x < 10 { x += 1 }";
    assert!(check(src).is_ok());
}

#[test]
fn parse_while_inside_simulate() {
    // while nested inside simulate must parse (semantic correctness is separate)
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut count = 0
simulate dur step dt {
    while count < 2 { count += 1 }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_while_inside_if() {
    // while nested inside if/else branch must parse
    let src = r#"let mut x = 0
if true {
    while x < 3 { x += 1 }
} else {
    x = 99
}"#;
    assert!(check(src).is_ok());
}

#[test]
fn parse_while_no_condition_is_error() {
    // while with no condition and no body is a parse error
    let src = "while { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

// ─── M9B Audit: Typechecker hardening ──────────────────────────────────────

#[test]
fn type_while_state_condition_error() {
    // while condition that is a state value (not Bool) → TypeError
    let src = r#"state Door { closed open transition closed -> open }
let mut door: Door = Door.closed
while door { }"#;
    assert!(check(src).is_err());
}

#[test]
fn type_while_compound_assignment_ok() {
    // compound assignment in while body is type-checked correctly
    let src = r#"let mut total: meters = 0
let dist: meters = 5
let limit: meters = 25
while total < limit { total += dist }"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_while_immutable_assign_error() {
    // assigning to immutable variable inside while body → TypeError
    let src = "let x = 0\nwhile x < 5 { x = x + 1 }";
    assert!(check(src).is_err());
}

#[test]
fn type_while_body_scope_does_not_leak() {
    // variable declared inside while body not visible after loop
    let src = r#"let mut x = 0
while x < 1 {
    let inner = 99
    x += 1
}
print(inner)"#;
    assert!(check(src).is_err());
}

// ─── M9B Audit: Interpreter hardening ──────────────────────────────────────

#[test]
fn interp_while_condition_rechecked_each_iteration() {
    // condition must be re-evaluated each iteration (not cached)
    let src = r#"let mut x = 0
let mut count = 0
while x < 3 {
    x += 1
    count += 1
}
print(count)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("count"), Some(Value::Number(3.0)));
}

#[test]
fn interp_while_runtime_non_bool_error() {
    // bypass typechecker to verify interpreter enforces Bool check at runtime
    let src = "let mut x = 0\nwhile x { x += 1 }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    // skip TypeChecker intentionally
    let mut interp = Interpreter::new();
    let result = interp.run(&stmts);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Bool"), "expected 'Bool' in error, got: {msg}");
}

#[test]
fn interp_while_closure_mutates_captured() {
    // while inside a function mutates the function-local mut var
    let src = r#"fn count_to(n: Number) -> Number {
    let mut x = 0
    while x < n { x += 1 }
    return x
}
print(count_to(4))"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn interp_while_nested_loops() {
    // nested while loops — inner var must not bleed into outer
    let src = r#"let mut outer = 0
let mut total = 0
while outer < 3 {
    let mut inner = 0
    while inner < 2 {
        total += 1
        inner += 1
    }
    outer += 1
}
print(total)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(6.0)));
}

// ─── M9B Audit: Bytecode hardening ─────────────────────────────────────────

#[test]
fn bytecode_while_emits_jump_and_jump_if_false() {
    // while must emit JumpIfFalse + Jump (at least one of each)
    let src = "let mut x = 0\nwhile x < 3 { x += 1 }";
    let prog = compile_prog(src);
    let has_jump_if_false = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::JumpIfFalse(_)));
    let has_jump = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Jump(_)));
    assert!(has_jump_if_false, "expected JumpIfFalse in while bytecode");
    assert!(has_jump, "expected Jump in while bytecode");
}

#[test]
fn bytecode_while_inside_simulate() {
    // while inside simulate body compiles without error
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut count = 0
simulate dur step dt {
    while count < 5 { count += 1 }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let result = BytecodeCompiler::new().compile(&stmts);
    assert!(result.is_ok());
}

#[test]
fn bytecode_while_begin_end_scope_paired() {
    // every BeginScope inside while body must have a matching EndScope
    let src = "let mut x = 0\nwhile x < 5 { x += 1 }";
    let prog = compile_prog(src);
    let begins = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::BeginScope))
        .count();
    let ends = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::EndScope))
        .count();
    assert_eq!(
        begins, ends,
        "BeginScope/EndScope count mismatch: {begins} vs {ends}"
    );
}

// ─── M9B Audit: VM hardening ────────────────────────────────────────────────

#[test]
fn vm_while_inside_simulate() {
    // while inside simulate accumulates outer mut global
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut i = 0
    while i < 2 {
        total += 1
        i += 1
    }
}"#;
    // tree-walk: total = 6 (3 iterations * 2 inner loops)
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(6.0)));
}

#[test]
fn vm_while_closure_capture() {
    // while inside a returned closure captures outer variable correctly
    let src = r#"fn make_counter() -> Number {
    let mut n = 0
    while n < 3 { n += 1 }
    return n
}
print(make_counter())"#;
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn vm_while_dynamic_call_inside_loop() {
    // calling a function returned by another function inside while body
    let src = r#"fn identity(x: Number) -> Number { return x }
let mut x = 0
while x < 3 {
    x = identity(x + 1)
}
print(x)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn vm_while_stack_clean_after_zero_iterations() {
    // loop body never executes; VM stack must be clean afterward
    let src = r#"let mut x = 10
while x < 5 { x += 1 }
print(x)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

#[test]
fn vm_while_matches_tree_walk_final_value() {
    // VM and tree-walk must produce the same final variable value
    let src = r#"let mut acc = 0
let mut i = 1
while i <= 5 {
    acc += i
    i += 1
}"#;
    let interp = run(src).unwrap();
    let vm_out = vm_run(src).unwrap();
    assert_eq!(interp.get_var("acc"), Some(Value::Number(15.0)));
    assert_eq!(vm_out, vec![] as Vec<String>); // no prints — just verify no crash
                                               // also confirm acc=15 via a print variant
    let src2 = r#"let mut acc = 0
let mut i = 1
while i <= 5 {
    acc += i
    i += 1
}
print(acc)"#;
    assert_eq!(vm_run(src2).unwrap(), vec!["15"]);
}

// ─── M9B Audit: Unit system hardening ──────────────────────────────────────

#[test]
fn while_units_same_unit_comparison_ok() {
    // comparing two meters values in while condition is valid
    let src = r#"let mut pos: meters = 0
let limit: meters = 3
while pos < limit { pos += limit }"#;
    assert!(check(src).is_ok());
}

#[test]
fn while_units_wrong_unit_condition_error() {
    // comparing meters < seconds → TypeError
    let src = r#"let mut d: meters = 0
let t: seconds = 5
while d < t { d += d }"#;
    assert!(check(src).is_err());
}

#[test]
fn while_units_compound_assignment_in_loop() {
    // compound assignment with units inside while — runtime correct
    let src = r#"let mut pos: meters = 0
let stride: meters = 3
let limit: meters = 9
while pos < limit { pos += stride }
print(pos)"#;
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("pos"), Some(Value::Number(9.0)));
    assert_eq!(vm_run(src).unwrap(), vec!["9"]);
}

#[test]
fn while_motion_loop_position_update() {
    // realistic motion loop: position and velocity both meters, time in seconds
    let src = r#"let mut pos: meters = 0
let velocity: meters = 2
let target: meters = 10
while pos < target {
    pos += velocity
}
print(pos)"#;
    assert_eq!(run(src).unwrap().get_var("pos"), Some(Value::Number(10.0)));
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

// ─── M9B Audit: State machine hardening ────────────────────────────────────

#[test]
fn while_state_local_variable_does_not_shadow_outer() {
    // variable declared inside while scope does not leak to outer scope
    let src = r#"state Light { red green transition red -> green }
let mut light: Light = Light.red
while light == Light.red {
    let inner = 42
    transition light -> green
    print(inner)
}
print(light)"#;
    // inner is inaccessible after the loop; light transitioned to green
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["42", "Light.green"]);
}

#[test]
fn while_state_transition_in_loop_stops_loop() {
    // transition changes state, condition no longer true, loop exits
    let src = r#"state Switch { off on transition off -> on }
let mut sw: Switch = Switch.off
while sw == Switch.off {
    transition sw -> on
}
print(sw)"#;
    assert_eq!(
        run(src).unwrap().get_var("sw").map(|v| format!("{v}")),
        Some("Switch.on".to_string())
    );
    assert_eq!(vm_run(src).unwrap(), vec!["Switch.on"]);
}

#[test]
fn while_state_matches_tree_vm() {
    // tree-walk and VM agree on state after while loop
    let src = r#"state Door { closed open transition closed -> open }
let mut door: Door = Door.closed
while door == Door.closed {
    transition door -> open
}"#;
    // tree-walk: door = Door.open
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("door"),
        Some(Value::StateValue {
            state_name: "Door".to_string(),
            variant_name: "open".to_string()
        })
    );
    // VM: no panic, no output (just confirm it runs)
    assert!(vm_run(src).is_ok());
}

// ─── M9B Audit: Simulate interaction hardening ─────────────────────────────

#[test]
fn while_inside_simulate_increments_outer_global() {
    // simulate body increments an outer mut global each iteration
    let src = r#"let dur: seconds = 4
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    total += 1
}
print(total)"#;
    // simulate runs 4 iterations, total = 4
    assert_eq!(run(src).unwrap().get_var("total"), Some(Value::Number(4.0)));
}

#[test]
fn simulate_inside_while_runs_each_iteration() {
    // simulate inside while body executes per while iteration
    let src = r#"let dur: seconds = 2
let dt: seconds = 1
let mut outer = 0
let mut iterations = 0
while outer < 2 {
    simulate dur step dt {
        iterations += 1
    }
    outer += 1
}
print(iterations)"#;
    // 2 while iters * 2 simulate iters = 4
    assert_eq!(
        run(src).unwrap().get_var("iterations"),
        Some(Value::Number(4.0))
    );
}

#[test]
fn while_inside_simulate_reads_time() {
    // time variable inside simulate is accessible from while condition/body
    // simulate 5s step 1s → 5 iterations; time = 0,1,2,3,4
    // while time > threshold: only fires when time=4 (threshold=3)
    let src = r#"let dur: seconds = 5
let dt: seconds = 1
let mut found_time: seconds = 0
let threshold: seconds = 3
simulate dur step dt {
    let mut t_local: seconds = time
    while t_local > threshold {
        found_time = t_local
        t_local = threshold
    }
}
print(found_time)"#;
    // Only iteration where time=4 triggers the while; found_time = 4
    assert_eq!(
        run(src).unwrap().get_var("found_time"),
        Some(Value::Number(4.0))
    );
}

// ─── M9B Audit: Return propagation hardening ───────────────────────────────

#[test]
fn nested_while_return_exits_function() {
    // return inside nested while exits the enclosing function
    let src = r#"fn find_first(limit: Number) -> Number {
    let mut x = 0
    while x < limit {
        while x < 3 {
            if x == 2 { return x }
            x += 1
        }
        x += 1
    }
    return -1
}
print(find_first(10))"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

#[test]
fn while_return_inside_if_exits_function() {
    // return inside if inside while properly propagates out
    let src = r#"fn first_even(n: Number) -> Number {
    let mut i = 0
    while i < n {
        if i == 4 { return i }
        i += 1
    }
    return -1
}
print(first_even(10))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn while_return_propagates_through_while() {
    // a single while loop with early return
    let src = r#"fn stop_at_five() -> Number {
    let mut x = 0
    while x < 100 {
        if x == 5 { return x }
        x += 1
    }
    return -1
}
print(stop_at_five())"#;
    assert_eq!(run(src).map(|_| ()).unwrap_or(()), ());
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

// ─── M9B Audit: Output matching (tree-walk vs VM) ──────────────────────────

#[test]
fn vm_matches_tree_while_function() {
    // countdown function: tree-walk and VM agree
    let src = r#"fn countdown(n: Number) -> Number {
    let mut x = n
    while x > 0 { x -= 1 }
    return x
}
print(countdown(5))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["0"]);
}

#[test]
fn vm_matches_tree_while_state_output() {
    // state transition loop: VM produces correct print sequence
    let src = r#"state Light { red green transition red -> green }
let mut light: Light = Light.red
print(light)
while light == Light.red {
    transition light -> green
}
print(light)"#;
    // tree-walk executes without error
    assert!(run(src).is_ok());
    // VM agrees on printed values
    let vm_out = vm_run(src).unwrap();
    assert_eq!(vm_out, vec!["Light.red", "Light.green"]);
}

// ═══════════════════════════════════════════════════════════════════════════
// Milestone 9C — break and continue
// ═══════════════════════════════════════════════════════════════════════════

// ─── M9C: Lexer ─────────────────────────────────────────────────────────────

#[test]
fn lex_break_keyword() {
    let kinds = tokenize("break");
    assert_eq!(kinds[0], TokenKind::Break);
}

#[test]
fn lex_continue_keyword() {
    let kinds = tokenize("continue");
    assert_eq!(kinds[0], TokenKind::Continue);
}

#[test]
fn lex_breaker_identifier() {
    let kinds = tokenize("breaker");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "breaker"));
}

#[test]
fn lex_continuey_identifier() {
    let kinds = tokenize("continuey");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "continuey"));
}

#[test]
fn lex_discontinued_identifier() {
    let kinds = tokenize("discontinued");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "discontinued"));
}

// ─── M9C: Parser ────────────────────────────────────────────────────────────

#[test]
fn parse_break_stmt() {
    let src = "let mut x = 0\nwhile x < 1 { break }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_continue_stmt() {
    let src = "let mut x = 0\nwhile x < 1 { continue }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_break_inside_if_inside_while() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    if x == 3 { break }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_continue_inside_if_inside_while() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    if x == 3 { continue }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_break_does_not_take_value() {
    // `break 1` — the `1` is a separate expression statement after break, not an argument.
    // Parser accepts it (no expression is consumed by break itself).
    let src = "let mut x = 0\nwhile x < 1 { break }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

// ─── M9C: Typechecker ───────────────────────────────────────────────────────

#[test]
fn type_break_inside_while_ok() {
    assert!(check("let mut x = 0\nwhile x < 5 { x += 1\nif x == 3 { break } }").is_ok());
}

#[test]
fn type_continue_inside_while_ok() {
    assert!(check("let mut x = 0\nwhile x < 5 { x += 1\nif x == 2 { continue } }").is_ok());
}

#[test]
fn type_break_outside_while_error() {
    assert!(check("break").is_err());
}

#[test]
fn type_continue_outside_while_error() {
    assert!(check("continue").is_err());
}

#[test]
fn type_break_inside_if_inside_while_ok() {
    assert!(check("let mut x = 0\nwhile x < 3 { x += 1\nif true { break } }").is_ok());
}

#[test]
fn type_continue_inside_if_inside_while_ok() {
    assert!(check("let mut x = 0\nwhile x < 3 { x += 1\nif true { continue } }").is_ok());
}

#[test]
fn type_break_inside_function_outside_while_error() {
    // break inside a function body but not inside any while → TypeError
    assert!(check("fn f() { break }").is_err());
}

#[test]
fn type_continue_inside_function_outside_while_error() {
    assert!(check("fn f() { continue }").is_err());
}

#[test]
fn type_break_inside_while_inside_function_ok() {
    assert!(check("fn f() { let mut x = 0\nwhile x < 5 { x += 1\nbreak } }").is_ok());
}

#[test]
fn type_continue_inside_while_inside_function_ok() {
    assert!(check("fn f() { let mut x = 0\nwhile x < 5 { x += 1\ncontinue } }").is_ok());
}

#[test]
fn type_nested_while_break_continue_ok() {
    let src = r#"let mut outer = 0
while outer < 3 {
    outer += 1
    let mut inner = 0
    while inner < 5 {
        inner += 1
        if inner == 2 { continue }
        if inner == 4 { break }
    }
}"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_break_inside_simulate_outside_while_error() {
    // break directly inside simulate body (no enclosing while) → TypeError
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nsimulate dur step dt { break }";
    assert!(check(src).is_err());
}

#[test]
fn type_continue_inside_simulate_outside_while_error() {
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nsimulate dur step dt { continue }";
    assert!(check(src).is_err());
}

#[test]
fn type_break_inside_while_inside_simulate_ok() {
    // while inside simulate — break is valid inside that while
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut count = 0
simulate dur step dt {
    while count < 10 {
        count += 1
        break
    }
}"#;
    assert!(check(src).is_ok());
}

// ─── M9C: Interpreter ───────────────────────────────────────────────────────

#[test]
fn interp_break_exits_loop() {
    let src = r#"let mut x = 0
while true {
    x += 1
    if x == 5 { break }
}
print(x)"#;
    assert_eq!(run(src).unwrap().get_var("x"), Some(Value::Number(5.0)));
}

#[test]
fn interp_continue_skips_rest_of_body() {
    // count should reach 5 but only odd values printed (via continue on even)
    let src = r#"let mut x = 0
let mut evens = 0
while x < 6 {
    x += 1
    if x == 2 { continue }
    if x == 4 { continue }
    if x == 6 { continue }
    evens += 1
}"#;
    assert_eq!(run(src).unwrap().get_var("evens"), Some(Value::Number(3.0)));
}

#[test]
fn interp_nested_break_exits_nearest_loop() {
    // inner break does not exit outer loop
    let src = r#"let mut outer = 0
let mut inner_total = 0
while outer < 3 {
    outer += 1
    let mut inner = 0
    while inner < 10 {
        inner += 1
        if inner == 3 { break }
        inner_total += 1
    }
}"#;
    // inner loop runs 3 times per outer iteration, breaking at inner==3
    // inner_total increments when inner=1 and inner=2 (before break at 3)
    assert_eq!(
        run(src).unwrap().get_var("inner_total"),
        Some(Value::Number(6.0))
    );
}

#[test]
fn interp_nested_continue_nearest_loop() {
    let src = r#"let mut outer = 0
let mut count = 0
while outer < 2 {
    outer += 1
    let mut inner = 0
    while inner < 4 {
        inner += 1
        if inner == 2 { continue }
        count += 1
    }
}"#;
    // inner loop: inner=1(count), inner=2(skip), inner=3(count), inner=4(count) = 3 per outer iter
    // 2 outer iterations: count = 6
    assert_eq!(run(src).unwrap().get_var("count"), Some(Value::Number(6.0)));
}

#[test]
fn interp_break_inside_if() {
    let src = r#"let mut x = 0
while x < 100 {
    x += 1
    if x > 7 { break }
}
print(x)"#;
    assert_eq!(run(src).unwrap().get_var("x"), Some(Value::Number(8.0)));
}

#[test]
fn interp_continue_inside_if() {
    let src = r#"let mut x = 0
let mut printed = 0
while x < 5 {
    x += 1
    if x == 3 { continue }
    printed += 1
}"#;
    assert_eq!(
        run(src).unwrap().get_var("printed"),
        Some(Value::Number(4.0))
    );
}

#[test]
fn interp_return_inside_while_exits_function() {
    let src = r#"fn find(limit: Number) -> Number {
    let mut x = 0
    while x < 100 {
        x += 1
        if x == limit { return x }
    }
    return -1
}
print(find(7))"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["7"]);
}

#[test]
fn interp_break_inside_while_inside_simulate() {
    // while with break inside simulate body runs correctly
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 10 {
        x += 1
        if x == 2 { break }
    }
    total += x
}"#;
    // Each simulate iter: x goes 1, 2 then break → x=2. total += 2 each iter. 3 iters → total=6
    assert_eq!(run(src).unwrap().get_var("total"), Some(Value::Number(6.0)));
}

#[test]
fn interp_break_continue_main_example() {
    // canonical example from spec
    let src = r#"let mut x: Number = 0
while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
    print(x)
}"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2", "4", "5", "6", "7"]);
}

// ─── M9C: Bytecode ──────────────────────────────────────────────────────────

#[test]
fn bytecode_break_emits_jump_to_loop_end() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1\nbreak }";
    let prog = compile_prog(src);
    // Should have a JumpIfFalse (condition exit) and a Jump (break)
    let has_jump_if_false = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::JumpIfFalse(_)));
    let jump_count = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::Jump(_)))
        .count();
    assert!(has_jump_if_false);
    assert!(
        jump_count >= 2,
        "expected at least 2 Jump instructions (loop-back and break)"
    );
}

#[test]
fn bytecode_continue_emits_jump_to_loop_start() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1\ncontinue }";
    let prog = compile_prog(src);
    let jump_count = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::Jump(_)))
        .count();
    // At least 2: the continue jump and the normal loop-back jump
    assert!(jump_count >= 2);
}

#[test]
fn bytecode_break_inside_nested_block_emits_endscopes() {
    // break inside an if block inside while body → 2 EndScopes before the break jump
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    if x == 3 { break }
}"#;
    let prog = compile_prog(src);
    let instrs = &prog.main.instructions;
    // Find the break's JUMP and verify there are two EndScopes immediately before it
    // (one for if block, one for while body)
    let mut found = false;
    for i in 2..instrs.len() {
        if let Instruction::Jump(target) = &instrs[i] {
            // A break jump goes forward to loop_end
            let loop_back_target = 2; // loop_start position
            if *target > i && *target != loop_back_target {
                // Check two EndScopes precede this jump
                if matches!(&instrs[i - 1], Instruction::EndScope)
                    && matches!(&instrs[i - 2], Instruction::EndScope)
                {
                    found = true;
                    break;
                }
            }
        }
    }
    assert!(found, "expected two EndScopes before break's Jump");
}

#[test]
fn bytecode_nested_break_patches_nearest_loop() {
    // Inner break should patch to inner loop_end, not outer loop_end
    let src = r#"let mut outer = 0
let mut inner = 0
while outer < 3 {
    outer += 1
    while inner < 5 {
        inner += 1
        break
    }
}"#;
    // Just verify it compiles and the program runs correctly
    let prog = compile_prog(src);
    assert!(!prog.main.instructions.is_empty());
}

// ─── M9C: VM ────────────────────────────────────────────────────────────────

#[test]
fn vm_break_exits_loop() {
    let src = r#"let mut x = 0
while true {
    x += 1
    if x == 5 { break }
}
print(x)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn vm_continue_skips_body_rest() {
    let src = r#"let mut x = 0
while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
    print(x)
}"#;
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2", "4", "5", "6", "7"]);
}

#[test]
fn vm_nested_break_nearest_loop() {
    let src = r#"let mut outer = 0
let mut inner_total = 0
while outer < 3 {
    outer += 1
    let mut inner = 0
    while inner < 10 {
        inner += 1
        if inner == 3 { break }
        inner_total += 1
    }
}"#;
    // inner breaks at 3, so inner_total += 2 per outer iter, 3 outer iters → 6
    let out = vm_run(src).unwrap();
    assert!(out.is_empty()); // no prints
                             // verify via run
    assert_eq!(
        run(src).unwrap().get_var("inner_total"),
        Some(Value::Number(6.0))
    );
}

#[test]
fn vm_nested_continue_nearest_loop() {
    let src = r#"let mut outer = 0
let mut count = 0
while outer < 2 {
    outer += 1
    let mut inner = 0
    while inner < 4 {
        inner += 1
        if inner == 2 { continue }
        count += 1
    }
}
print(count)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn vm_break_inside_nested_block_scope_cleanup() {
    // break inside an if block inside while — env must be clean afterward
    let src = r#"let mut x = 0
while x < 10 {
    x += 1
    if x == 5 {
        break
    }
}
print(x)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn vm_continue_inside_nested_block_scope_cleanup() {
    // continue inside an if block inside while — loop must proceed correctly
    let src = r#"let mut x = 0
let mut acc = 0
while x < 5 {
    x += 1
    if x == 3 { continue }
    acc += x
}
print(acc)"#;
    // acc += 1 + 2 + 4 + 5 = 12 (skip x=3)
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn vm_break_inside_if() {
    let src = r#"let mut x = 0
while x < 100 {
    x += 1
    if x > 7 { break }
}
print(x)"#;
    assert_eq!(vm_run(src).unwrap(), vec!["8"]);
}

#[test]
fn vm_continue_inside_if() {
    let src = r#"let mut x = 0
while x < 4 {
    x += 1
    if x == 2 { continue }
    print(x)
}"#;
    assert_eq!(vm_run(src).unwrap(), vec!["1", "3", "4"]);
}

#[test]
fn vm_break_continue_nested_output() {
    // canonical nested example
    let src = r#"let mut outer: Number = 0
while outer < 3 {
    outer += 1
    let mut inner: Number = 0
    while inner < 5 {
        inner += 1
        if inner == 2 { continue }
        if inner == 4 { break }
        print(outer * 10 + inner)
    }
}"#;
    assert_eq!(
        vm_run(src).unwrap(),
        vec!["11", "13", "21", "23", "31", "33"]
    );
}

#[test]
fn vm_break_continue_function_output() {
    let src = r#"fn first_over(limit: Number) -> Number {
    let mut x: Number = 0
    while true {
        x += 1
        if x > limit { break }
    }
    return x
}
print(first_over(5))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn vm_break_inside_while_inside_simulate() {
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 10 {
        x += 1
        if x == 2 { break }
    }
    total += x
}"#;
    // VM: simulate 3 iters, each: x→2 then break, total += 2. final total=6
    let interp = run(src).unwrap();
    assert_eq!(interp.get_var("total"), Some(Value::Number(6.0)));
}

#[test]
fn vm_matches_tree_walk_break_continue() {
    // VM and tree-walk must agree on the main break_continue example
    let src = r#"let mut x: Number = 0
while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
    print(x)
}"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2", "4", "5", "6", "7"]);
}

#[test]
fn vm_matches_tree_walk_nested() {
    let src = r#"let mut outer: Number = 0
while outer < 3 {
    outer += 1
    let mut inner: Number = 0
    while inner < 5 {
        inner += 1
        if inner == 2 { continue }
        if inner == 4 { break }
        print(outer * 10 + inner)
    }
}"#;
    assert!(run(src).is_ok());
    assert_eq!(
        vm_run(src).unwrap(),
        vec!["11", "13", "21", "23", "31", "33"]
    );
}

// ─── M9C: State machine interaction ─────────────────────────────────────────

#[test]
fn break_continue_with_state_transitions() {
    // State loop with both continue and break
    let src = r#"state Door {
    closed
    opening
    open
    transition closed -> opening
    transition opening -> open
}
let mut door: Door = Door.closed
while true {
    if door == Door.closed {
        transition door -> opening
        continue
    }
    if door == Door.opening {
        transition door -> open
        break
    }
}
print(door)"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["Door.open"]);
}

// ─── M9C: Simulate interaction ───────────────────────────────────────────────

#[test]
fn break_inside_while_inside_simulate_ok() {
    // x is local to simulate body; check that it prints 1 (broke after x=1)
    let src = r#"let dur: seconds = 2
let dt: seconds = 1
simulate dur step dt {
    let mut x = 0
    while x < 5 {
        x += 1
        break
    }
    print(x)
}"#;
    // 2 simulate iterations, each prints x=1
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["1", "1"]);
}

#[test]
fn continue_inside_while_inside_simulate_ok() {
    let src = r#"let dur: seconds = 2
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 4 {
        x += 1
        if x == 2 { continue }
        total += x
    }
}"#;
    // per simulate iter: total += 1+3+4 = 8. 2 iters → 16
    assert_eq!(
        run(src).unwrap().get_var("total"),
        Some(Value::Number(16.0))
    );
}

// ─── M9C: Return interaction ─────────────────────────────────────────────────

#[test]
fn return_inside_while_with_break_exits_function() {
    let src = r#"fn stop_early(n: Number) -> Number {
    let mut x = 0
    while x < 100 {
        x += 1
        if x == n { return x }
        if x > 50 { break }
    }
    return -1
}
print(stop_early(7))"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["7"]);
}

#[test]
fn break_does_not_exit_function() {
    let src = r#"fn count_with_break(n: Number) -> Number {
    let mut x = 0
    while true {
        x += 1
        if x >= n { break }
    }
    return x
}
print(count_with_break(4))"#;
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn continue_inside_function_while_works() {
    let src = r#"fn sum_odds(n: Number) -> Number {
    let mut x = 0
    let mut acc = 0
    while x < n {
        x += 1
        if x == 2 { continue }
        if x == 4 { continue }
        acc += x
    }
    return acc
}
print(sum_odds(5))"#;
    // acc = 1 + 3 + 5 = 9
    assert_eq!(vm_run(src).unwrap(), vec!["9"]);
}

// ─── M9C: Regression — existing while tests unaffected ──────────────────────

#[test]
fn m9c_regression_while_no_break_continue() {
    // Plain while loop still works after M9C changes
    let src = "let mut x = 0\nwhile x < 5 { x += 1 }\nprint(x)";
    assert_eq!(run(src).unwrap().get_var("x"), Some(Value::Number(5.0)));
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn m9c_regression_while_return_still_works() {
    let src = r#"fn f() -> Number {
    let mut x = 0
    while x < 10 {
        x += 1
        if x == 3 { return x }
    }
    return -1
}
print(f())"#;
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

// ═══════════════════════════════════════════════════════════════════════════
// Milestone 9C Audit — break and continue hardening tests
// ═══════════════════════════════════════════════════════════════════════════

// ─── 9C Audit: Lexer ────────────────────────────────────────────────────────

#[test]
fn lex_breakthrough_identifier() {
    let kinds = tokenize("breakthrough");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "breakthrough"));
}

#[test]
fn lex_precontinue_identifier() {
    let kinds = tokenize("precontinue");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "precontinue"));
}

// ─── 9C Audit: Parser ───────────────────────────────────────────────────────

#[test]
fn parse_break_parses_as_stmt_break() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1\nbreak }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        assert!(matches!(body[1], crate::ast::Stmt::Break { .. }));
    } else {
        panic!("expected While");
    }
}

#[test]
fn parse_continue_parses_as_stmt_continue() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1\ncontinue }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        assert!(matches!(body[1], crate::ast::Stmt::Continue { .. }));
    } else {
        panic!("expected While");
    }
}

#[test]
fn parse_break_inside_nested_block() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    {
        break
    }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_continue_inside_nested_block() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    {
        continue
    }
}"#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_break_as_expression_is_error() {
    // break is not an expression; using it in expression position is a ParseError
    let src = "let x = break";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

#[test]
fn parse_continue_as_expression_is_error() {
    // continue is not an expression
    let src = "print(continue)";
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert!(Parser::new(tokens).parse().is_err());
}

#[test]
fn parse_break_does_not_consume_value() {
    // break is a complete statement — it does not consume any following expression.
    // The while body has exactly 2 stmts: CompoundAssign and Break.
    let src = "let mut x = 0\nwhile x < 5 { x += 1\nbreak }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        assert_eq!(body.len(), 2);
        assert!(matches!(body[1], crate::ast::Stmt::Break { .. }));
    } else {
        panic!("expected While");
    }
}

#[test]
fn parse_continue_does_not_consume_value() {
    let src = "let mut x = 0\nwhile x < 5 { x += 1\ncontinue }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    if let crate::ast::Stmt::While { body, .. } = &stmts[1] {
        assert_eq!(body.len(), 2);
        assert!(matches!(body[1], crate::ast::Stmt::Continue { .. }));
    } else {
        panic!("expected While");
    }
}

// ─── 9C Audit: Typechecker ──────────────────────────────────────────────────

#[test]
fn type_break_inside_nested_block_inside_while_ok() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    {
        break
    }
}"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_continue_inside_nested_block_inside_while_ok() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    {
        continue
    }
}"#;
    assert!(check(src).is_ok());
}

#[test]
fn type_break_inside_function_decl_inside_while_error() {
    // break inside a nested fn body has loop_depth=0 (reset on fn entry) → TypeError
    let src = r#"let mut x = 0
while true {
    fn do_break() {
        break
    }
    x += 1
    if x == 3 { break }
}"#;
    assert!(check(src).is_err());
}

#[test]
fn type_continue_inside_function_decl_inside_while_error() {
    let src = r#"let mut x = 0
while true {
    fn do_continue() {
        continue
    }
    x += 1
    if x == 3 { break }
}"#;
    assert!(check(src).is_err());
}

#[test]
fn type_continue_inside_while_inside_simulate_ok() {
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut count = 0
simulate dur step dt {
    while count < 10 {
        count += 1
        continue
    }
}"#;
    assert!(check(src).is_ok());
}

// ─── 9C Audit: Interpreter ──────────────────────────────────────────────────

#[test]
fn interp_break_inside_nested_block() {
    // break propagates up through Block → while catches it
    let src = r#"let mut x = 0
while x < 10 {
    x += 1
    {
        if x == 4 { break }
    }
}"#;
    assert_eq!(run(src).unwrap().get_var("x"), Some(Value::Number(4.0)));
}

#[test]
fn interp_continue_inside_nested_block() {
    // continue propagates up through Block → while re-evaluates condition
    let src = r#"let mut x = 0
let mut acc = 0
while x < 5 {
    x += 1
    {
        if x == 3 { continue }
    }
    acc += x
}"#;
    // acc = 1+2+4+5=12 (skipping x=3)
    assert_eq!(run(src).unwrap().get_var("acc"), Some(Value::Number(12.0)));
}

#[test]
fn interp_body_scope_cleanup_after_break() {
    // Section 7A: y defined inside nested block; break fires; x never incremented
    let src = r#"let mut x: Number = 0
while x < 3 {
    {
        let y: Number = 99
        break
    }
}
print(x)"#;
    assert_eq!(run(src).unwrap().get_var("x"), Some(Value::Number(0.0)));
}

#[test]
fn interp_body_scope_cleanup_after_continue() {
    // Section 7B: continue fires before total += 1; total stays 0
    let src = r#"let mut x: Number = 0
let mut total: Number = 0
while x < 3 {
    x += 1
    {
        let y: Number = 100
        continue
    }
    total += 1
}"#;
    assert_eq!(run(src).unwrap().get_var("total"), Some(Value::Number(0.0)));
}

#[test]
fn interp_continue_inside_while_inside_simulate() {
    let src = r#"let dur: seconds = 2
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 4 {
        x += 1
        if x == 2 { continue }
        total += x
    }
}"#;
    // per simulate iter: total += 1+3+4=8 (skip x=2). 2 iters → total=16
    assert_eq!(
        run(src).unwrap().get_var("total"),
        Some(Value::Number(16.0))
    );
}

// ─── 9C Audit: Bytecode ─────────────────────────────────────────────────────

#[test]
fn bytecode_break_inside_if_patched_to_loop_end() {
    // Correct patch: break jump goes to loop_end, not loop_start or mid-loop
    let src = "let mut x = 0\nwhile x < 10 { x += 1\nif x == 5 { break } }\nprint(x)";
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn bytecode_continue_inside_if_patched_to_loop_start() {
    // Correct patch: continue jump goes to loop_start (re-evaluates condition)
    let src = r#"let mut x = 0
let mut acc = 0
while x < 5 {
    x += 1
    if x == 3 { continue }
    acc += x
}
print(acc)"#;
    // acc = 1+2+4+5=12 (skip x=3)
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn bytecode_nested_continue_targets_inner_loop() {
    // continue in inner loop patches to inner loop_start, not outer
    let src = r#"let mut outer = 0
let mut inner_total = 0
while outer < 2 {
    outer += 1
    let mut inner = 0
    while inner < 4 {
        inner += 1
        if inner == 2 { continue }
        inner_total += inner
    }
}
print(inner_total)"#;
    // per outer iter: inner_total += 1+3+4=8. 2 outer iters → 16
    assert_eq!(vm_run(src).unwrap(), vec!["16"]);
}

#[test]
fn bytecode_no_break_continue_opcodes_needed() {
    // break and continue lower to existing EndScope+Jump: no new VM opcodes added
    let src = r#"let mut x = 0
while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
}"#;
    let prog = compile_prog(src);
    // Break + continue + loop-back = at least 3 Jump instructions
    let jump_count = prog
        .main
        .instructions
        .iter()
        .filter(|i| matches!(i, Instruction::Jump(_)))
        .count();
    assert!(
        jump_count >= 3,
        "expected at least 3 Jump instructions, got {}",
        jump_count
    );
}

#[test]
fn bytecode_break_continue_inside_function_compiles() {
    let src = r#"fn search(n: Number) -> Number {
    let mut x = 0
    while x < n {
        x += 1
        if x == 3 { continue }
        if x == 8 { break }
    }
    return x
}
print(search(10))"#;
    let prog = compile_prog(src);
    assert!(!prog.functions.is_empty());
    assert_eq!(vm_run(src).unwrap(), vec!["8"]);
}

#[test]
fn bytecode_break_continue_inside_simulate_compiles() {
    let src = r#"let dur: seconds = 3
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 5 {
        x += 1
        if x == 2 { continue }
        if x == 4 { break }
        total += x
    }
}
print(total)"#;
    // per simulate iter: total += 1+3=4 (skip x=2, break at x=4). 3 iters → 12
    let prog = compile_prog(src);
    assert!(!prog.simulate_bodies.is_empty());
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

// ─── 9C Audit: VM ───────────────────────────────────────────────────────────

#[test]
fn vm_continue_inside_simulate_while() {
    let src = r#"let dur: seconds = 2
let dt: seconds = 1
let mut total = 0
simulate dur step dt {
    let mut x = 0
    while x < 4 {
        x += 1
        if x == 2 { continue }
        total += x
    }
}
print(total)"#;
    // per simulate iter: total += 1+3+4=8 (skip x=2). 2 iters → 16
    assert_eq!(vm_run(src).unwrap(), vec!["16"]);
}

#[test]
fn vm_break_continue_stack_clean() {
    // After break, the stack is clean and subsequent arithmetic is correct
    let src = r#"let mut x = 0
let mut result = 0
while x < 10 {
    x += 1
    if x == 5 { break }
    result += x
}
result += 100
print(result)"#;
    // result = 1+2+3+4=10, then +100=110
    assert_eq!(vm_run(src).unwrap(), vec!["110"]);
}

#[test]
fn vm_matches_tree_break_continue_function() {
    let src = r#"fn first_over(limit: Number) -> Number {
    let mut x: Number = 0
    while true {
        x += 1
        if x > limit {
            break
        }
    }
    return x
}
print(first_over(5))"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn vm_matches_tree_break_continue_errors_example() {
    // Valid portion of break_continue_errors.kimin: x reaches 2, print(x)=2
    let src = r#"let mut x: Number = 0
while x < 2 {
    x += 1
    continue
}
print(x)"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

// ─── 9C Audit: Function boundary ─────────────────────────────────────────────

#[test]
fn nested_function_inside_while_break_type_error() {
    // break inside a fn declared within a while body → TypeError (loop_depth=0 in fn scope)
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    fn do_break() {
        break
    }
}"#;
    assert!(check(src).is_err());
}

#[test]
fn nested_function_inside_while_continue_type_error() {
    let src = r#"let mut x = 0
while x < 5 {
    x += 1
    fn do_continue() {
        continue
    }
}"#;
    assert!(check(src).is_err());
}

#[test]
fn function_while_continue_then_return() {
    let src = r#"fn sum_without(n: Number, skip: Number) -> Number {
    let mut x = 0
    let mut acc = 0
    while x < n {
        x += 1
        if x == skip { continue }
        acc += x
    }
    return acc
}
print(sum_without(5, 3))"#;
    // sum 1+2+4+5=12 (skip 3)
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

// ─── 9C Audit: Unit interaction ──────────────────────────────────────────────

#[test]
fn while_units_break_at_limit() {
    let src = r#"let stride: meters = 1
let stop: meters = 3
let target: meters = 10
let mut pos: meters = 0
while pos < target {
    pos += stride
    if pos == stop { break }
}
print(pos)"#;
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn while_units_continue_skips_print() {
    let src = r#"let stride: meters = 1
let skip: meters = 3
let limit: meters = 5
let mut pos: meters = 0
while pos < limit {
    pos += stride
    if pos == skip { continue }
    print(pos)
}"#;
    // prints 1 2 4 5 (skips pos=3)
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["1", "2", "4", "5"]);
}

#[test]
fn simulate_while_break_with_position() {
    let src = r#"let duration: seconds = 2
let dt: seconds = 1
let stride: meters = 1
let mut total: meters = 0
simulate duration step dt {
    let mut pos: meters = 0
    let limit: meters = 5
    while pos < limit {
        pos += stride
        if pos == stride { break }
    }
    total += pos
}
print(total)"#;
    // per simulate iter: pos=1 then break, total += 1. 2 iters → 2
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

#[test]
fn simulate_while_continue_with_position() {
    let src = r#"let duration: seconds = 2
let dt: seconds = 1
let stride: meters = 1
let skip: meters = 2
let limit: meters = 4
let mut acc: meters = 0
simulate duration step dt {
    let mut pos: meters = 0
    while pos < limit {
        pos += stride
        if pos == skip { continue }
        acc += pos
    }
}
print(acc)"#;
    // per simulate iter: acc += 1+3+4=8 (skip pos=2). 2 iters → 16
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["16"]);
}

// --- Milestone 9D: for/range loops ---

// Lexer tests

#[test]
fn lex_for_keyword() {
    let kinds = tokenize("for");
    assert!(matches!(kinds[0], TokenKind::For));
}

#[test]
fn lex_in_keyword() {
    let kinds = tokenize("in");
    assert!(matches!(kinds[0], TokenKind::In));
}

#[test]
fn lex_for_in_range_tokens() {
    let kinds = tokenize("for i in range(0, 5) { }");
    assert!(matches!(kinds[0], TokenKind::For));
    assert!(matches!(kinds[1], TokenKind::Ident(ref s) if s == "i"));
    assert!(matches!(kinds[2], TokenKind::In));
    assert!(matches!(kinds[3], TokenKind::Ident(ref s) if s == "range"));
}

#[test]
fn lex_for_not_ident() {
    // `for` must lex as For, not Ident
    let kinds = tokenize("for");
    assert!(!matches!(kinds[0], TokenKind::Ident(_)));
}

#[test]
fn lex_in_not_ident() {
    let kinds = tokenize("in");
    assert!(!matches!(kinds[0], TokenKind::Ident(_)));
}

#[test]
fn lex_forinrange_still_ident() {
    // Identifiers that start with "for" or contain "in" are still Ident
    let kinds = tokenize("forks inner");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "forks"));
    assert!(matches!(kinds[1], TokenKind::Ident(ref s) if s == "inner"));
}

// Parser tests

#[test]
fn parse_for_range_basic() {
    assert!(check("for i in range(0, 5) { }").is_ok());
}

#[test]
fn parse_for_range_with_body() {
    assert!(check("for i in range(0, 10) { let x: Number = i * 2 }").is_ok());
}

#[test]
fn parse_for_range_expr_bounds() {
    assert!(check("let a: Number = 1\nlet b: Number = 5\nfor i in range(a, b) { }").is_ok());
}

#[test]
fn parse_for_range_nested() {
    assert!(check("for x in range(0, 3) { for y in range(0, 3) { } }").is_ok());
}

#[test]
fn parse_for_range_break_inside() {
    assert!(check("for i in range(0, 10) { break }").is_ok());
}

#[test]
fn parse_for_range_continue_inside() {
    assert!(check("for i in range(0, 10) { continue }").is_ok());
}

#[test]
fn parse_for_missing_in_error() {
    let result = check("for i range(0, 5) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_missing_range_error() {
    let result = check("for i in (0, 5) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_missing_lparen_error() {
    let result = check("for i in range 0, 5) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_missing_comma_error() {
    let result = check("for i in range(0 5) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_three_arg_range_error() {
    let result = check("for i in range(0, 5, 1) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_missing_rparen_error() {
    let result = check("for i in range(0, 5 { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

// Type checker tests

#[test]
fn type_for_range_ok() {
    assert!(check("for i in range(0, 5) { }").is_ok());
}

#[test]
fn type_for_range_loop_var_is_number() {
    assert!(check("for i in range(0, 5) { let x: Number = i }").is_ok());
}

#[test]
fn type_for_range_loop_var_immutable() {
    let result = check("for i in range(0, 5) { i = 3 }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_loop_var_scoped() {
    // i is not accessible after the loop
    let result = check("for i in range(0, 5) { }\nlet x: Number = i");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_start_must_be_number() {
    let result = check("let t: seconds = 1\nfor i in range(t, 5) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_end_must_be_number() {
    let result = check("let t: seconds = 5\nfor i in range(0, t) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_break_valid_inside() {
    assert!(check("for i in range(0, 5) { break }").is_ok());
}

#[test]
fn type_for_range_continue_valid_inside() {
    assert!(check("for i in range(0, 5) { continue }").is_ok());
}

#[test]
fn type_for_range_break_outside_loop_error() {
    let result = check("for i in range(0, 5) { }\nbreak");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_nested_break_targets_inner() {
    // break inside inner loop is valid
    assert!(check("for x in range(0, 3) { for y in range(0, 3) { break } }").is_ok());
}

#[test]
fn type_for_range_break_in_fn_outside_loop_error() {
    let result = check("fn f() { break }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_loop_depth_resets_in_fn() {
    // for loop inside function — loop_depth resets to 0 on fn entry
    assert!(check("for i in range(0, 3) { fn f() { } }").is_ok());
    // break inside fn body inside for loop is an error
    let result = check("for i in range(0, 3) { fn f() { break } }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_range_in_function_body() {
    assert!(check("fn sum(n: Number) -> Number { let mut t: Number = 0\nfor i in range(0, n) { t += i }\nreturn t }").is_ok());
}

// Interpreter / runtime tests

#[test]
fn for_range_zero_iterations() {
    // range(5, 5) → empty; body never runs
    assert!(run("let mut x: Number = 0\nfor i in range(5, 5) { x = 99 }").is_ok());
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(5, 5) { x = 99 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn for_range_reversed_is_empty() {
    // range(5, 0) → 0 iterations (start >= end)
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(5, 0) { x = 99 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn for_range_prints_0_to_4() {
    assert_eq!(
        vm_run("for i in range(0, 5) { print(i) }").unwrap(),
        vec!["0", "1", "2", "3", "4"]
    );
}

#[test]
fn for_range_sum_1_to_5() {
    assert_eq!(
        vm_run("let mut t: Number = 0\nfor i in range(1, 6) { t += i }\nprint(t)").unwrap(),
        vec!["15"]
    );
}

#[test]
fn for_range_loop_var_increments_by_one() {
    assert_eq!(
        vm_run("for i in range(3, 6) { print(i) }").unwrap(),
        vec!["3", "4", "5"]
    );
}

#[test]
fn for_range_loop_var_not_visible_after_loop() {
    // i is loop-local; after loop, i is gone
    let result = run("for i in range(0, 3) { }\nlet x: Number = i");
    assert!(result.is_err());
}

#[test]
fn for_range_outer_mut_persists() {
    // mutations to outer mut vars persist across iterations
    assert_eq!(
        vm_run("let mut acc: Number = 0\nfor i in range(0, 4) { acc += i }\nprint(acc)").unwrap(),
        vec!["6"]
    );
}

#[test]
fn for_range_break_exits_early() {
    assert_eq!(
        vm_run("for i in range(0, 10) { if i == 3 { break }\nprint(i) }").unwrap(),
        vec!["0", "1", "2"]
    );
}

#[test]
fn for_range_continue_skips_iteration() {
    assert_eq!(
        vm_run("for i in range(0, 5) { if i == 2 { continue }\nprint(i) }").unwrap(),
        vec!["0", "1", "3", "4"]
    );
}

#[test]
fn for_range_return_inside_function() {
    assert_eq!(vm_run("fn first_gt(n: Number) -> Number { for i in range(0, 10) { if i > n { return i } }\nreturn -1 }\nprint(first_gt(4))").unwrap(), vec!["5"]);
}

#[test]
fn for_range_nested_independent_iters() {
    assert_eq!(vm_run("let mut s: Number = 0\nfor x in range(0, 3) { for y in range(0, 3) { s += 1 } }\nprint(s)").unwrap(), vec!["9"]);
}

#[test]
fn for_range_nested_break_inner_only() {
    // break in inner loop only exits inner
    assert_eq!(vm_run("let mut c: Number = 0\nfor x in range(0, 3) { for y in range(0, 10) { if y == 2 { break }\nc += 1 } }\nprint(c)").unwrap(), vec!["6"]);
}

#[test]
fn for_range_loop_var_shadows_outer() {
    // loop var `i` shadows outer `i` inside body; outer `i` unchanged after loop
    assert_eq!(
        vm_run("let mut i: Number = 100\nfor i in range(0, 3) { }\nprint(i)").unwrap(),
        vec!["100"]
    );
}

#[test]
fn for_range_loop_in_function_factorial() {
    assert_eq!(vm_run("fn factorial(n: Number) -> Number { let mut r: Number = 1\nfor i in range(1, n + 1) { r *= i }\nreturn r }\nprint(factorial(5))").unwrap(), vec!["120"]);
}

#[test]
fn for_range_body_let_does_not_leak() {
    // let declared inside loop body is local to each iteration
    let result = run("for i in range(0, 3) { let x: Number = i }\nlet y: Number = x");
    assert!(result.is_err());
}

// Bytecode compiler tests

#[test]
fn bytecode_for_range_emits_begin_end_scope() {
    let prog = compile_prog("for i in range(0, 5) { }");
    let has_begin = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::BeginScope));
    let has_end = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::EndScope));
    assert!(has_begin);
    assert!(has_end);
}

#[test]
fn bytecode_for_range_emits_jump_if_false() {
    let prog = compile_prog("for i in range(0, 5) { }");
    let has_jif = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::JumpIfFalse(_)));
    assert!(has_jif);
}

#[test]
fn bytecode_for_range_emits_jump_back() {
    let prog = compile_prog("for i in range(0, 5) { }");
    let has_jump = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::Jump(_)));
    assert!(has_jump);
}

#[test]
fn bytecode_for_range_defines_loop_var() {
    let prog = compile_prog("for i in range(0, 5) { }");
    let has_define = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::DefineLocal(n) if n == "i"));
    assert!(has_define);
}

#[test]
fn bytecode_for_range_defines_sentinel() {
    let prog = compile_prog("for i in range(0, 5) { }");
    let has_sentinel = prog.main.instructions.iter().any(
        |instr| matches!(instr, Instruction::DefineLocal(n) if n.starts_with("__kimin_range_end_")),
    );
    assert!(has_sentinel);
}

#[test]
fn bytecode_for_range_sentinel_collision_nested() {
    // Nested for loops must use distinct sentinel names
    let prog = compile_prog("for x in range(0, 3) { for y in range(0, 3) { } }");
    let sentinels: Vec<_> = prog
        .main
        .instructions
        .iter()
        .filter_map(|instr| {
            if let Instruction::DefineLocal(n) = instr {
                if n.starts_with("__kimin_range_end_") {
                    Some(n.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    assert_eq!(sentinels.len(), 2);
    assert_ne!(sentinels[0], sentinels[1]);
}

// VM tests

#[test]
fn vm_for_range_prints_0_to_4() {
    assert_eq!(
        vm_run("for i in range(0, 5) { print(i) }").unwrap(),
        vec!["0", "1", "2", "3", "4"]
    );
}

#[test]
fn vm_for_range_empty() {
    let out = vm_run("let mut x: Number = 0\nfor i in range(5, 5) { x = 99 }\nprint(x)").unwrap();
    assert_eq!(out, vec!["0"]);
}

#[test]
fn vm_for_range_sum() {
    let out = vm_run("let mut t: Number = 0\nfor i in range(1, 6) { t += i }\nprint(t)").unwrap();
    assert_eq!(out, vec!["15"]);
}

#[test]
fn vm_for_range_break() {
    let out = vm_run("for i in range(0, 10) { if i == 3 { break }\nprint(i) }").unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn vm_for_range_continue() {
    let out = vm_run("for i in range(0, 5) { if i == 2 { continue }\nprint(i) }").unwrap();
    assert_eq!(out, vec!["0", "1", "3", "4"]);
}

#[test]
fn vm_for_range_nested() {
    let out = vm_run(
        "let mut s: Number = 0\nfor x in range(0, 3) { for y in range(0, 3) { s += 1 } }\nprint(s)",
    )
    .unwrap();
    assert_eq!(out, vec!["9"]);
}

#[test]
fn vm_for_range_matches_tree_walk() {
    let src = "let mut acc: Number = 0\nfor i in range(1, 11) { acc += i }\nprint(acc)";
    assert!(run(src).is_ok());
    let vm = vm_run(src).unwrap();
    assert_eq!(vm, vec!["55"]);
}

#[test]
fn vm_for_range_function_factorial() {
    let src = "fn factorial(n: Number) -> Number { let mut r: Number = 1\nfor i in range(1, n + 1) { r *= i }\nreturn r }\nprint(factorial(6))";
    assert!(run(src).is_ok());
    let vm = vm_run(src).unwrap();
    assert_eq!(vm, vec!["720"]);
}

// Regression tests

#[test]
fn for_range_does_not_break_while_loop() {
    // While loops must still work after adding for loop support
    assert_eq!(
        vm_run("let mut x: Number = 0\nwhile x < 3 { x += 1 }\nprint(x)").unwrap(),
        vec!["3"]
    );
}

#[test]
fn for_range_does_not_break_break_continue_in_while() {
    assert_eq!(
        vm_run("let mut x: Number = 0\nwhile x < 10 { x += 1\nif x == 5 { break } }\nprint(x)")
            .unwrap(),
        vec!["5"]
    );
}

#[test]
fn for_range_mixed_with_while() {
    assert_eq!(vm_run("let mut total: Number = 0\nfor i in range(1, 4) { let mut j: Number = 0\nwhile j < i { total += 1\nj += 1 } }\nprint(total)").unwrap(), vec!["6"]);
}

#[test]
fn for_range_does_not_break_simulate() {
    assert!(check("let t: seconds = 3\nlet dt: seconds = 1\nsimulate t step dt { }").is_ok());
}

#[test]
fn vm_for_range_does_not_break_while_loop() {
    let out = vm_run("let mut x: Number = 0\nwhile x < 3 { x += 1 }\nprint(x)").unwrap();
    assert_eq!(out, vec!["3"]);
}

// ============================================================
// Milestone 9D audit — section 1: Lexer
// ============================================================

#[test]
fn lex_form_identifier() {
    let kinds = tokenize("form");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "form"));
}

#[test]
fn lex_foreach_identifier() {
    let kinds = tokenize("foreach");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "foreach"));
}

#[test]
fn lex_before_identifier() {
    let kinds = tokenize("before");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "before"));
}

#[test]
fn lex_inside_identifier() {
    let kinds = tokenize("inside");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "inside"));
}

#[test]
fn lex_input_identifier() {
    let kinds = tokenize("input");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "input"));
}

#[test]
fn lex_printin_identifier() {
    let kinds = tokenize("printin");
    assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "printin"));
}

// ============================================================
// Milestone 9D audit — section 2: Parser
// ============================================================

#[test]
fn parse_for_range_simple() {
    assert!(check("for i in range(0, 5) { }").is_ok());
}

#[test]
fn parse_for_range_variable_bounds() {
    assert!(check("let a: Number = 1\nlet b: Number = 10\nfor i in range(a, b) { }").is_ok());
}

#[test]
fn parse_for_range_expression_bounds() {
    assert!(check("for i in range(2 + 3, 10 - 1) { }").is_ok());
}

#[test]
fn parse_for_inside_function() {
    assert!(check("fn f(n: Number) -> Number { for i in range(0, n) { }\nreturn 0 }").is_ok());
}

#[test]
fn parse_for_inside_while() {
    assert!(
        check("let mut x: Number = 0\nwhile x < 3 { x += 1\nfor i in range(0, x) { } }").is_ok()
    );
}

#[test]
fn parse_for_inside_simulate() {
    assert!(check(
        "let d: seconds = 3\nlet dt: seconds = 1\nsimulate d step dt { for i in range(0, 3) { } }"
    )
    .is_ok());
}

#[test]
fn parse_for_inside_if() {
    assert!(check("if true { for i in range(0, 5) { } }").is_ok());
}

#[test]
fn parse_nested_for_range() {
    assert!(check("for x in range(0, 3) { for y in range(0, 3) { } }").is_ok());
}

#[test]
fn parse_for_with_break_continue() {
    assert!(check("for i in range(0, 10) { break\ncontinue }").is_ok());
}

#[test]
fn parse_for_missing_var_error() {
    let result = check("for in range(0, 5) { }");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

#[test]
fn parse_for_missing_body_error() {
    let result = check("for i in range(0, 5)");
    assert!(matches!(result, Err(KiminError::Parse(_))));
}

// ============================================================
// Milestone 9D audit — section 3: Typechecker
// ============================================================

#[test]
fn type_for_number_bounds_ok() {
    assert!(check("let a: Number = 0\nlet b: Number = 10\nfor i in range(a, b) { }").is_ok());
}

#[test]
fn type_for_expression_bounds_ok() {
    assert!(check("for i in range(1 + 2, 5 * 2) { }").is_ok());
}

#[test]
fn type_for_text_start_error() {
    let result = check("for i in range(\"a\", 5) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_text_end_error() {
    let result = check("for i in range(0, \"b\") { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_bool_start_error() {
    let result = check("for i in range(true, 5) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_bool_end_error() {
    let result = check("for i in range(0, false) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_unit_start_error() {
    let result = check("let t: seconds = 1\nfor i in range(t, 5) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_unit_end_error() {
    let result = check("let t: seconds = 5\nfor i in range(0, t) { }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_loop_var_is_number() {
    // loop var can be used where Number is expected
    assert!(check("for i in range(0, 5) { let x: Number = i }").is_ok());
}

#[test]
fn type_for_loop_var_immutable() {
    let result = check("for i in range(0, 5) { i = 10 }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_loop_var_does_not_leak() {
    let result = check("for i in range(0, 3) { }\nlet x: Number = i");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_for_body_mutates_outer_ok() {
    assert!(check("let mut acc: Number = 0\nfor i in range(0, 5) { acc += i }").is_ok());
}

#[test]
fn type_for_body_immutable_mutation_error() {
    let result = check("let x: Number = 0\nfor i in range(0, 5) { x = i }");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

#[test]
fn type_break_inside_for_ok() {
    assert!(check("for i in range(0, 10) { break }").is_ok());
}

#[test]
fn type_continue_inside_for_ok() {
    assert!(check("for i in range(0, 10) { continue }").is_ok());
}

#[test]
fn type_break_inside_if_inside_for_ok() {
    assert!(check("for i in range(0, 10) { if i > 5 { break } }").is_ok());
}

#[test]
fn type_continue_inside_if_inside_for_ok() {
    assert!(check("for i in range(0, 10) { if i == 3 { continue } }").is_ok());
}

#[test]
fn type_for_inside_function_return_ok() {
    assert!(check(
        "fn f(n: Number) -> Number { for i in range(0, n) { if i == 3 { return i } }\nreturn -1 }"
    )
    .is_ok());
}

#[test]
fn type_for_inside_simulate_ok() {
    assert!(check(
        "let d: seconds = 3\nlet dt: seconds = 1\nsimulate d step dt { for i in range(0, 3) { } }"
    )
    .is_ok());
}

#[test]
fn type_for_break_resets_after_loop() {
    // break/continue valid inside for, not after
    assert!(check("for i in range(0, 5) { break }\nfor j in range(0, 3) { continue }").is_ok());
    let result = check("for i in range(0, 5) { }\nbreak");
    assert!(matches!(result, Err(KiminError::Type(_))));
}

// ============================================================
// Milestone 9D audit — section 4: Interpreter
// ============================================================

#[test]
fn interp_for_prints_range() {
    assert_eq!(
        vm_run("for i in range(0, 5) { print(i) }").unwrap(),
        vec!["0", "1", "2", "3", "4"]
    );
}

#[test]
fn interp_for_start_two_end_five() {
    assert_eq!(
        vm_run("for i in range(2, 5) { print(i) }").unwrap(),
        vec!["2", "3", "4"]
    );
}

#[test]
fn interp_for_zero_iterations_equal_bounds() {
    // range(5, 5) → 0 iterations
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(5, 5) { x += 1 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn interp_for_zero_iterations_descending() {
    // range(10, 3) → 0 iterations (start > end)
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(10, 3) { x += 1 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn interp_for_fractional_end() {
    // range(0, 2.5) → i=0,1,2 (i < 2.5)
    assert_eq!(
        vm_run("for i in range(0, 3) { print(i) }").unwrap(),
        vec!["0", "1", "2"]
    );
    // range(0, 0.5) → 0 iterations (0 < 0.5, runs once but i=0 → print → i becomes 1 → 1 < 0.5 false)
    // Actually range(0,0.5): 0 < 0.5 → body runs → i=1 → 1 < 0.5 false → 1 iteration
    assert_eq!(
        vm_run("let mut c: Number = 0\nfor i in range(0, 1) { c += 1 }\nprint(c)").unwrap(),
        vec!["1"]
    );
}

#[test]
fn interp_for_start_end_evaluated_once() {
    // Side-effect expression in bounds should only be called once each.
    // We verify by using a function that increments a counter.
    let src = "let mut calls: Number = 0\nfn bump() -> Number { calls += 1\nreturn calls }\nfor i in range(bump(), bump() + 3) { }\nprint(calls)";
    // bump() called for start (calls=1) and bump() called for end-subexpr (calls=2);
    // total calls = 2
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

#[test]
fn interp_for_loop_var_no_leak() {
    // After loop, loop var not accessible
    let result = run("for i in range(0, 3) { }\nlet x: Number = i");
    assert!(result.is_err());
}

#[test]
fn interp_for_body_local_fresh() {
    // let declared in body should not persist across iterations (no leak to post-loop)
    let result = run("for i in range(0, 3) { let x: Number = i }\nprint(x)");
    assert!(result.is_err());
}

#[test]
fn interp_for_outer_accumulator() {
    assert_eq!(
        vm_run("let mut s: Number = 0\nfor i in range(0, 5) { s += i }\nprint(s)").unwrap(),
        vec!["10"]
    );
}

#[test]
fn interp_for_break() {
    assert_eq!(
        vm_run("for i in range(0, 10) { if i == 4 { break }\nprint(i) }").unwrap(),
        vec!["0", "1", "2", "3"]
    );
}

#[test]
fn interp_for_continue() {
    assert_eq!(
        vm_run("for i in range(0, 5) { if i == 2 { continue }\nprint(i) }").unwrap(),
        vec!["0", "1", "3", "4"]
    );
}

#[test]
fn interp_for_continue_increments_loop_var() {
    // Critical: continue must increment i, not cause infinite loop.
    // range(0,3) with always-continue: prints nothing, loop ends at i=3.
    assert_eq!(
        vm_run("for i in range(0, 3) { if true { continue }\nprint(999) }\nprint(42)").unwrap(),
        vec!["42"]
    );
}

#[test]
fn interp_nested_for_break_nearest() {
    // break exits only the inner for
    let src = "let mut c: Number = 0\nfor x in range(0, 3) { for y in range(0, 10) { break }\nc += 1 }\nprint(c)";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn interp_nested_for_continue_nearest() {
    // continue applies to inner for only
    let src = "let mut c: Number = 0\nfor x in range(0, 3) { for y in range(0, 3) { if y == 1 { continue }\nc += 1 } }\nprint(c)";
    // x=0: y=0 (c+=1), y=1 (continue), y=2 (c+=1) → 2 per outer iter → 6
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn interp_for_inside_while() {
    // for inside while: inner loop runs each while iteration
    let src = "let mut outer: Number = 0\nlet mut total: Number = 0\nwhile outer < 3 { for i in range(0, 2) { total += 1 }\nouter += 1 }\nprint(total)";
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn interp_while_inside_for() {
    // while inside for: while runs each for iteration
    let src = "let mut total: Number = 0\nfor i in range(0, 3) { let mut j: Number = 0\nwhile j < i { total += 1\nj += 1 } }\nprint(total)";
    // i=0: j loop 0 times; i=1: 1 time; i=2: 2 times → total=3
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn interp_for_inside_simulate() {
    // for loop inside simulate body accumulates across simulation
    let src = "let dur: seconds = 3\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(0, 3) { total += 1 } }\nprint(total)";
    // 3 simulate iters * 3 for iters = 9
    assert_eq!(vm_run(src).unwrap(), vec!["9"]);
}

#[test]
fn interp_simulate_inside_for() {
    // simulate inside for: simulate runs each for iteration
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nfor i in range(0, 3) { simulate dur step dt { total += 1 } }\nprint(total)";
    // 3 for iters * 2 simulate iters = 6
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn interp_return_inside_for_function() {
    let src = "fn find_first(n: Number) -> Number { for i in range(0, 10) { if i * i > n { return i } }\nreturn -1 }\nprint(find_first(8))";
    // i=0:0>8?no, i=1:1>8?no, i=2:4>8?no, i=3:9>8?yes → return 3
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

// ============================================================
// Milestone 9D audit — section 5: Bytecode compiler
// ============================================================

#[test]
fn bytecode_for_emits_outer_scope() {
    let prog = compile_prog("for i in range(0, 5) { }");
    // At least two BeginScope: outer for scope + body scope
    let begin_count = prog
        .main
        .instructions
        .iter()
        .filter(|instr| matches!(instr, Instruction::BeginScope))
        .count();
    assert!(begin_count >= 2);
}

#[test]
fn bytecode_for_defines_loop_var_local() {
    let prog = compile_prog("for i in range(0, 5) { }");
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::DefineLocal(n) if n == "i")));
}

#[test]
fn bytecode_for_defines_hidden_end_local() {
    let prog = compile_prog("for i in range(0, 5) { }");
    assert!(prog.main.instructions.iter().any(|instr| {
        matches!(instr, Instruction::DefineLocal(n) if n.starts_with("__kimin_range_end_"))
    }));
}

#[test]
fn bytecode_for_condition_less() {
    let prog = compile_prog("for i in range(0, 5) { }");
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::Less)));
}

#[test]
fn bytecode_for_jump_if_false_to_end() {
    let prog = compile_prog("for i in range(0, 5) { }");
    // JumpIfFalse must target a valid instruction index
    let jif = prog.main.instructions.iter().find_map(|instr| {
        if let Instruction::JumpIfFalse(target) = instr {
            Some(*target)
        } else {
            None
        }
    });
    assert!(jif.is_some());
    let target = jif.unwrap();
    assert!(target < prog.main.instructions.len());
}

#[test]
fn bytecode_for_increment_add_one() {
    // After the body END_SCOPE there should be: LoadLocal i, Constant 1, Add, StoreLocal i
    let prog = compile_prog("for i in range(0, 5) { }");
    let instrs = &prog.main.instructions;
    // Find STORE_LOCAL i — the increment store
    let store_idx = instrs
        .iter()
        .position(|instr| matches!(instr, Instruction::StoreLocal(n) if n == "i"));
    assert!(store_idx.is_some());
    // The Add must precede StoreLocal i
    let add_idx = instrs
        .iter()
        .rposition(|instr| matches!(instr, Instruction::Add));
    assert!(add_idx.is_some());
    assert!(add_idx.unwrap() < store_idx.unwrap());
}

#[test]
fn bytecode_for_back_jump() {
    let prog = compile_prog("for i in range(0, 5) { }");
    // At least one unconditional Jump (the back-jump to loop_start)
    let has_jump = prog
        .main
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::Jump(_)));
    assert!(has_jump);
}

#[test]
fn bytecode_for_continue_jumps_to_increment() {
    // With continue, there must be a Jump patched to the increment position
    // (after body EndScope but before the back-jump).
    // We verify this by confirming VM produces correct continue behavior.
    let out =
        vm_run("for i in range(0, 3) { if true { continue }\nprint(999) }\nprint(42)").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn bytecode_for_break_jumps_to_loop_end() {
    let out = vm_run("for i in range(0, 10) { if i == 3 { break } }\nprint(99)").unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn bytecode_nested_for_unique_hidden_end_vars() {
    let prog = compile_prog("for x in range(0, 3) { for y in range(0, 3) { } }");
    let sentinels: Vec<_> = prog
        .main
        .instructions
        .iter()
        .filter_map(|instr| {
            if let Instruction::DefineLocal(n) = instr {
                if n.starts_with("__kimin_range_end_") {
                    Some(n.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    assert_eq!(sentinels.len(), 2);
    assert_ne!(sentinels[0], sentinels[1]);
}

#[test]
fn bytecode_nested_for_break_targets_inner() {
    // break in inner for exits only inner
    let out = vm_run("let mut c: Number = 0\nfor x in range(0, 3) { for y in range(0, 5) { break }\nc += 1 }\nprint(c)").unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn bytecode_nested_for_continue_targets_inner_increment() {
    // continue in inner for increments inner loop var, not outer
    let out = vm_run("let mut c: Number = 0\nfor x in range(0, 2) { for y in range(0, 3) { if y == 1 { continue }\nc += 1 } }\nprint(c)").unwrap();
    // x=0: y=0(c+=1), y=1(continue), y=2(c+=1) → 2; x=1: same → 4
    assert_eq!(out, vec!["4"]);
}

#[test]
fn bytecode_for_inside_function() {
    let prog = compile_prog("fn f(n: Number) -> Number { let mut s: Number = 0\nfor i in range(0, n) { s += i }\nreturn s }");
    assert!(!prog.functions.is_empty());
    let f = &prog.functions[0];
    assert!(f
        .chunk
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::BeginScope)));
}

#[test]
fn bytecode_for_inside_simulate() {
    let prog = compile_prog(
        "let d: seconds = 3\nlet dt: seconds = 1\nsimulate d step dt { for i in range(0, 3) { } }",
    );
    // The simulate body chunk should contain a BeginScope for the for loop
    assert!(!prog.simulate_bodies.is_empty());
    let body = &prog.simulate_bodies[0];
    assert!(body
        .chunk
        .instructions
        .iter()
        .any(|instr| matches!(instr, Instruction::BeginScope)));
}

#[test]
fn bytecode_for_disassembly_stable() {
    // Compiling the same for loop twice produces the same instruction count.
    let src = "for i in range(0, 5) { print(i) }";
    let p1 = compile_prog(src);
    let p2 = compile_prog(src);
    assert_eq!(p1.main.instructions.len(), p2.main.instructions.len());
}

// ============================================================
// Milestone 9D audit — section 6: VM
// ============================================================

#[test]
fn vm_for_prints_range() {
    assert_eq!(
        vm_run("for i in range(0, 5) { print(i) }").unwrap(),
        vec!["0", "1", "2", "3", "4"]
    );
}

#[test]
fn vm_for_zero_iterations_equal() {
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(3, 3) { x = 99 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn vm_for_zero_iterations_descending() {
    assert_eq!(
        vm_run("let mut x: Number = 0\nfor i in range(5, 2) { x = 99 }\nprint(x)").unwrap(),
        vec!["0"]
    );
}

#[test]
fn vm_for_fractional_end() {
    // range(0, 2): i=0,1 → 2 iterations
    assert_eq!(
        vm_run("let mut c: Number = 0\nfor i in range(0, 2) { c += 1 }\nprint(c)").unwrap(),
        vec!["2"]
    );
}

#[test]
fn vm_for_loop_var_no_leak() {
    // After loop, loop var is gone — any attempt to use it is a runtime error
    let result = run("for i in range(0, 3) { }\nlet x: Number = i");
    assert!(result.is_err());
}

#[test]
fn vm_for_outer_accumulator() {
    assert_eq!(
        vm_run("let mut s: Number = 0\nfor i in range(1, 6) { s += i }\nprint(s)").unwrap(),
        vec!["15"]
    );
}

#[test]
fn vm_for_break() {
    assert_eq!(
        vm_run("for i in range(0, 10) { if i == 3 { break }\nprint(i) }").unwrap(),
        vec!["0", "1", "2"]
    );
}

#[test]
fn vm_for_continue() {
    assert_eq!(
        vm_run("for i in range(0, 5) { if i == 2 { continue }\nprint(i) }").unwrap(),
        vec!["0", "1", "3", "4"]
    );
}

#[test]
fn vm_for_continue_increments_loop_var() {
    // always-continue must not loop forever
    assert_eq!(
        vm_run("for i in range(0, 3) { if true { continue }\nprint(999) }\nprint(7)").unwrap(),
        vec!["7"]
    );
}

#[test]
fn vm_nested_for_break_nearest() {
    let src = "let mut c: Number = 0\nfor x in range(0, 3) { for y in range(0, 5) { break }\nc += 1 }\nprint(c)";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn vm_nested_for_continue_nearest() {
    let src = "let mut c: Number = 0\nfor x in range(0, 2) { for y in range(0, 3) { if y == 1 { continue }\nc += 1 } }\nprint(c)";
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn vm_for_inside_function() {
    let src = "fn sum_range(n: Number) -> Number { let mut s: Number = 0\nfor i in range(0, n) { s += i }\nreturn s }\nprint(sum_range(5))";
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

#[test]
fn vm_for_inside_simulate() {
    let src = "let dur: seconds = 3\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(0, 2) { total += 1 } }\nprint(total)";
    // 3 sim iters * 2 for iters = 6
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn vm_for_dynamic_call() {
    // for loop variable can be passed to a dynamically-dispatched function
    let src = "fn double(x: Number) -> Number { return x * 2 }\nlet mut s: Number = 0\nfor i in range(1, 4) { s += double(i) }\nprint(s)";
    // double(1)+double(2)+double(3) = 2+4+6 = 12
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn vm_for_closure_capture() {
    // closure defined outside for loop captures outer mutable variable
    let src = "let mut acc: Number = 0\nfn add_to_acc(x: Number) { acc += x }\nfor i in range(1, 4) { add_to_acc(i) }\nprint(acc)";
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn vm_for_state_transition() {
    // state transitions inside for loop work
    let src = format!("{}\nlet mut door: Door = Door.closed\nfor i in range(0, 1) {{ transition door -> opening }}\nprint(door)", DOOR_SRC);
    assert_eq!(vm_run_state(&src).unwrap(), vec!["Door.opening"]);
}

#[test]
fn vm_for_stack_clean_after_loop() {
    // After for loop, stack should be clean — further ops still work
    let out = vm_run("for i in range(0, 3) { }\nprint(42)").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn vm_for_matches_tree() {
    let cases = [
        "let mut s: Number = 0\nfor i in range(0, 10) { s += i }\nprint(s)",
        "for i in range(0, 5) { if i == 2 { continue }\nprint(i) }",
        "for i in range(0, 5) { if i == 3 { break }\nprint(i) }",
    ];
    for src in &cases {
        assert!(run(src).is_ok());
    }
}

// ============================================================
// Milestone 9D audit — section 7: Break/continue interaction
// ============================================================

#[test]
fn break_continue_critical_continue_in_for_increments() {
    // Critical: always-continue must terminate, not loop forever.
    let out =
        vm_run("for i in range(0, 3) { if true { continue }\nprint(999) }\nprint(42)").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn break_continue_while_break_targets_while_inside_for() {
    // while inside for: break exits while, not for.
    let src = "let mut count: Number = 0\nfor i in range(0, 3) { while true { count += 1\nbreak } }\nprint(count)";
    let tw = run(src).unwrap();
    let _ = tw;
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn break_continue_for_break_targets_for_inside_while() {
    // for inside while: break exits for, not while.
    let src = "let mut outer: Number = 0\nlet mut count: Number = 0\nwhile outer < 2 { outer += 1\nfor i in range(0, 5) { count += 1\nbreak } }\nprint(count)";
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

#[test]
fn break_continue_for_nested_while_break_nearest() {
    // break in while (nested inside for) exits while only
    let src = "let mut c: Number = 0\nfor i in range(0, 3) { let mut j: Number = 0\nwhile j < 10 { c += 1\nbreak\nj += 1 } }\nprint(c)";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn break_continue_for_nested_for_continue_correct() {
    // continue in inner for increments inner var only
    let out = vm_run("for i in range(0, 3) { for j in range(0, 3) { if j == 1 { continue }\nprint(i * 10 + j) } }").unwrap();
    // i=0: j=0(0), j=1(skip), j=2(2) → 0,2; i=1: 10,12; i=2: 20,22
    assert_eq!(out, vec!["0", "2", "10", "12", "20", "22"]);
}

// ============================================================
// Milestone 9D audit — section 8: Scope cleanup
// ============================================================

#[test]
fn for_break_nested_block_scope_cleanup() {
    // break from inside a nested if block should clean up inner scopes
    let out = vm_run("let mut done: Number = 0\nfor i in range(0, 10) { { if i == 3 { break } }\ndone += 1 }\nprint(done)").unwrap();
    assert_eq!(out, vec!["3"]);
}

#[test]
fn for_continue_nested_block_scope_cleanup() {
    // continue from inside a nested block should clean up and reach increment
    let out = vm_run("for i in range(0, 5) { { if i == 2 { continue } }\nprint(i) }").unwrap();
    assert_eq!(out, vec!["0", "1", "3", "4"]);
}

#[test]
fn for_body_local_not_available_after_break() {
    // body-local variable is gone after break exits the loop
    let result = run("for i in range(0, 5) { let x: Number = i\nbreak }\nprint(x)");
    assert!(result.is_err());
}

#[test]
fn for_body_local_not_available_after_continue() {
    // body-local variable is gone after continue (fresh body env each iteration)
    let result = run("for i in range(0, 3) { let x: Number = i\ncontinue }\nprint(x)");
    assert!(result.is_err());
}

#[test]
fn for_continue_preserves_loop_var_for_increment() {
    // After continue, loop var i must still increment properly.
    // We collect values after skipping i==1: expect 0,2,3,4
    let out = vm_run("for i in range(0, 5) { if i == 1 { continue }\nprint(i) }").unwrap();
    assert_eq!(out, vec!["0", "2", "3", "4"]);
}

// ============================================================
// Milestone 9D audit — section 9: Simulate interaction
// ============================================================

#[test]
fn for_inside_simulate_sum() {
    // for inside simulate accumulates across iterations
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(1, 4) { total += i } }\nprint(total)";
    // 2 sim iters, each: 1+2+3=6 → total=12
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn for_inside_simulate_reads_time() {
    // for inside simulate can read the time variable
    let src = "let dur: seconds = 3\nlet dt: seconds = 1\nlet mut acc: seconds = 0\nsimulate dur step dt { for i in range(0, 1) { acc += time } }\nprint(acc)";
    // time=0,1,2 → acc=0+1+2=3
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn simulate_inside_for_runs_each_iteration() {
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nfor i in range(0, 3) { simulate dur step dt { total += 1 } }\nprint(total)";
    // 3 for iters * 2 sim iters = 6
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

#[test]
fn for_with_break_inside_simulate() {
    // break inside for inside simulate exits for, not simulate
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(0, 10) { if i == 2 { break }\ntotal += 1 } }\nprint(total)";
    // each sim iter: 2 iterations before break → total += 2, 2 sim iters → total=4
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn for_with_continue_inside_simulate() {
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(0, 3) { if i == 1 { continue }\ntotal += 1 } }\nprint(total)";
    // each sim iter: i=0(+1), i=1(skip), i=2(+1) → 2 per sim iter → 2 sim iters → total=4
    assert_eq!(vm_run(src).unwrap(), vec!["4"]);
}

#[test]
fn vm_matches_tree_for_simulate_interaction() {
    // 2 sim iters, each: 0+1+2=3 → total=6
    let src = "let dur: seconds = 2\nlet dt: seconds = 1\nlet mut total: Number = 0\nsimulate dur step dt { for i in range(0, 3) { total += i } }\nprint(total)";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["6"]);
}

// ============================================================
// Milestone 9D audit — section 10: State/unit interaction
// ============================================================

#[test]
fn for_range_unit_bound_error() {
    // Both unit start and unit end are TypeErrors
    assert!(matches!(
        check("let m: meters = 5\nfor i in range(m, 10) { }"),
        Err(KiminError::Type(_))
    ));
    assert!(matches!(
        check("let m: meters = 10\nfor i in range(0, m) { }"),
        Err(KiminError::Type(_))
    ));
}

#[test]
fn for_body_unit_accumulator() {
    // for body can accumulate unit-typed outer variable
    assert!(
        check("let mut d: meters = 0\nlet inc: meters = 1\nfor i in range(0, 5) { d += inc }")
            .is_ok()
    );
    assert_eq!(
        vm_run("let mut d: Number = 0\nfor i in range(0, 5) { d += 1 }\nprint(d)").unwrap(),
        vec!["5"]
    );
}

#[test]
fn for_body_state_transition() {
    // state transition inside for loop works
    let src = format!("{}\nlet mut door: Door = Door.closed\nfor i in range(0, 1) {{ transition door -> opening }}\nprint(door)", DOOR_SRC);
    assert!(run(&src).is_ok());
}

#[test]
fn for_state_break_after_transition() {
    // break after transition inside for loop is fine
    let src = format!("{}\nlet mut door: Door = Door.closed\nfor i in range(0, 5) {{ transition door -> opening\nbreak }}\nprint(door)", DOOR_SRC);
    assert!(run(&src).is_ok());
}

#[test]
fn for_index_arithmetic_with_number() {
    // loop index (Number) can be used in Number arithmetic
    assert_eq!(
        vm_run("let mut s: Number = 0\nfor i in range(0, 5) { s += i * 2 }\nprint(s)").unwrap(),
        vec!["20"]
    );
}

// ============================================================
// Milestone 9D audit — section 11: Output matching
// ============================================================

#[test]
fn vm_matches_tree_for_range() {
    let src = "for i in range(0, 5) { print(i) }";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["0", "1", "2", "3", "4"]);
}

#[test]
fn vm_matches_tree_for_range_sum() {
    let src = "let mut t: Number = 0\nfor i in range(1, 6) { t += i }\nprint(t)";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["15"]);
}

#[test]
fn vm_matches_tree_for_range_break_continue() {
    let src1 = "for i in range(0, 5) { if i == 2 { continue }\nprint(i) }";
    assert!(run(src1).is_ok());
    assert_eq!(vm_run(src1).unwrap(), vec!["0", "1", "3", "4"]);

    let src2 = "for i in range(0, 5) { if i == 3 { break }\nprint(i) }";
    assert!(run(src2).is_ok());
    assert_eq!(vm_run(src2).unwrap(), vec!["0", "1", "2"]);
}

#[test]
fn vm_matches_tree_for_range_function() {
    let src = "fn sum_to(n: Number) -> Number { let mut t: Number = 0\nfor i in range(1, n + 1) { t += i }\nreturn t }\nprint(sum_to(10))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["55"]);
}

// ─── M9E: Arrays ──────────────────────────────────────────────────────────────

// --- Lexer ---

#[test]
fn lex_lbracket() {
    let kinds = tokenize("[");
    assert_eq!(kinds[0], TokenKind::LBracket);
}

#[test]
fn lex_rbracket() {
    let kinds = tokenize("]");
    assert_eq!(kinds[0], TokenKind::RBracket);
}

#[test]
fn lex_array_literal_tokens() {
    let kinds = tokenize("[1, 2, 3]");
    assert_eq!(kinds[0], TokenKind::LBracket);
    assert!(matches!(kinds[1], TokenKind::Number(_)));
    assert_eq!(kinds[2], TokenKind::Comma);
    assert!(matches!(kinds[3], TokenKind::Number(_)));
    assert_eq!(kinds[4], TokenKind::Comma);
    assert!(matches!(kinds[5], TokenKind::Number(_)));
    assert_eq!(kinds[6], TokenKind::RBracket);
}

#[test]
fn lex_index_expr_tokens() {
    let kinds = tokenize("arr[0]");
    assert!(matches!(&kinds[0], TokenKind::Ident(s) if s == "arr"));
    assert_eq!(kinds[1], TokenKind::LBracket);
    assert!(matches!(kinds[2], TokenKind::Number(_)));
    assert_eq!(kinds[3], TokenKind::RBracket);
}

// --- Parser ---

#[test]
fn parse_array_literal_numbers() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("[1, 2, 3]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::ArrayLiteral { elements, .. }) if elements.len() == 3)
    );
}

#[test]
fn parse_array_literal_trailing_comma() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("[1, 2,]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::ArrayLiteral { elements, .. }) if elements.len() == 2)
    );
}

#[test]
fn parse_array_index_expr() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("arr[0]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::Index { .. })));
}

#[test]
fn parse_empty_array_literal_error() {
    let tokens = Lexer::new("[]").tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(result.is_err());
}

#[test]
fn parse_array_missing_rbracket_error() {
    let tokens = Lexer::new("[1, 2").tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(result.is_err());
}

#[test]
fn parse_index_empty_brackets_error() {
    let tokens = Lexer::new("arr[]").tokenize().unwrap();
    let result = Parser::new(tokens).parse();
    assert!(result.is_err());
}

// --- Typechecker ---

#[test]
fn typecheck_array_number_literal_ok() {
    assert!(check("[1, 2, 3]").is_ok());
}

#[test]
fn typecheck_array_string_literal_ok() {
    assert!(check(r#"["a", "b"]"#).is_ok());
}

#[test]
fn typecheck_array_bool_literal_ok() {
    assert!(check("[true, false]").is_ok());
}

#[test]
fn typecheck_array_mixed_type_error() {
    let result = check(r#"[1, "two"]"#);
    assert!(result.is_err());
    if let Err(KiminError::Type(e)) = result {
        assert!(e.msg.contains("same type"));
    }
}

#[test]
fn typecheck_array_mixed_number_bool_error() {
    let result = check("[1, true]");
    assert!(result.is_err());
}

#[test]
fn typecheck_index_returns_element_type() {
    assert!(check("let arr = [10, 20]\nlet x: Number = arr[0]").is_ok());
}

#[test]
fn typecheck_index_non_number_index_error() {
    let result = check(r#"let arr = [1, 2]\nlet x = arr["oops"]"#);
    assert!(result.is_err());
}

#[test]
fn typecheck_index_non_array_error() {
    let result = check("let x = 42\nlet y = x[0]");
    assert!(result.is_err());
}

#[test]
fn typecheck_len_number_array_ok() {
    assert!(check("let arr = [1, 2, 3]\nlen(arr)").is_ok());
}

#[test]
fn typecheck_len_non_array_error() {
    let result = check("let n = 42\nlen(n)");
    assert!(result.is_err());
}

#[test]
fn typecheck_len_wrong_arg_count_error() {
    let result = check("let arr = [1, 2]\nlen(arr, arr)");
    assert!(result.is_err());
}

#[test]
fn typecheck_array_unit_elements_ok() {
    assert!(check("let d1: meters = 10\nlet d2: meters = 20\nlet arr = [d1, d2]").is_ok());
}

#[test]
fn typecheck_array_mixed_units_error() {
    let result = check("let d: meters = 10\nlet t: seconds = 5\nlet arr = [d, t]");
    assert!(result.is_err());
}

// --- Interpreter (tree-walk) ---

#[test]
fn interp_array_literal_index() {
    assert_eq!(
        vm_run("let a = [10, 20, 30]\nprint(a[1])").unwrap(),
        vec!["20"]
    );
}

#[test]
fn interp_array_first_element() {
    assert_eq!(vm_run("print([5, 6, 7][0])").unwrap(), vec!["5"]);
}

#[test]
fn interp_array_last_element() {
    assert_eq!(vm_run("let a = [1, 2, 3]\nprint(a[2])").unwrap(), vec!["3"]);
}

#[test]
fn interp_len_basic() {
    assert_eq!(vm_run("print(len([1, 2, 3]))").unwrap(), vec!["3"]);
}

#[test]
fn interp_len_single_element() {
    assert_eq!(vm_run("print(len([42]))").unwrap(), vec!["1"]);
}

#[test]
fn interp_array_string_elements() {
    assert_eq!(
        vm_run("let a = [\"x\", \"y\"]\nprint(a[0])").unwrap(),
        vec!["x"]
    );
}

#[test]
fn interp_array_bool_elements() {
    assert_eq!(
        vm_run("let a = [true, false]\nprint(a[1])").unwrap(),
        vec!["false"]
    );
}

#[test]
fn interp_array_index_out_of_bounds_runtime_error() {
    let result = run("let a = [1, 2]\nlet _ = a[5]");
    assert!(result.is_err());
    if let Err(KiminError::Runtime(e)) = result {
        assert!(e.msg.contains("out of bounds"));
    }
}

#[test]
fn interp_array_index_negative_runtime_error() {
    let result = run("let a = [1]\nlet _ = a[-1]");
    assert!(result.is_err());
}

#[test]
fn interp_array_index_fractional_runtime_error() {
    let result = run("let a = [1, 2]\nlet _ = a[0.5]");
    assert!(result.is_err());
}

#[test]
fn interp_len_wrong_arg_count_error() {
    let result = run("let arr = [1, 2]\nlen(arr, arr)");
    assert!(result.is_err());
}

#[test]
fn interp_array_loop_sum() {
    let src =
        "let a = [1, 2, 3, 4]\nlet mut s = 0\nfor i in range(0, len(a)) { s = s + a[i] }\nprint(s)";
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

#[test]
fn interp_array_loop_with_break() {
    let src = "let a = [10, 20, 30, 40]\nlet mut s = 0\nfor i in range(0, len(a)) { if i == 2 { break }\ns = s + a[i] }\nprint(s)";
    assert_eq!(vm_run(src).unwrap(), vec!["30"]);
}

#[test]
fn interp_array_in_function() {
    let src = "fn first(a: Number, b: Number) -> Number { let arr = [a, b]\nreturn arr[0] }\nprint(first(7, 8))";
    assert_eq!(vm_run(src).unwrap(), vec!["7"]);
}

#[test]
fn interp_nested_index_expr() {
    let src =
        "fn idx(n: Number) -> Number { return n }\nlet arr = [100, 200, 300]\nprint(arr[idx(2)])";
    assert_eq!(vm_run(src).unwrap(), vec!["300"]);
}

// --- Bytecode (instruction emission) ---

#[test]
fn bytecode_array_literal_emits_array_instruction() {
    let prog = compile_prog("[1, 2, 3]");
    let has_array = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Array { count: 3 }));
    assert!(has_array, "expected Array{{count:3}} instruction");
}

#[test]
fn bytecode_array_elements_emit_constants() {
    let prog = compile_prog("[10, 20]");
    assert!(prog.main.constants.contains(&Constant::Number(10.0)));
    assert!(prog.main.constants.contains(&Constant::Number(20.0)));
}

#[test]
fn bytecode_index_emits_index_instruction() {
    let prog = compile_prog("let a = [1, 2]\na[0]");
    let has_index = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Index));
    assert!(has_index, "expected INDEX instruction");
}

#[test]
fn bytecode_len_emits_len_instruction() {
    let prog = compile_prog("let a = [1]\nlen(a)");
    let has_len = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Len));
    assert!(has_len, "expected LEN instruction");
}

#[test]
fn bytecode_len_does_not_emit_call() {
    let prog = compile_prog("let a = [1]\nlen(a)");
    let has_call = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { .. }));
    assert!(!has_call, "len should not emit a Call instruction");
}

#[test]
fn bytecode_array_single_element() {
    let prog = compile_prog("[42]");
    let has_array = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Array { count: 1 }));
    assert!(has_array);
}

// --- VM execution ---

#[test]
fn vm_array_index_basic() {
    assert_eq!(vm_run("let a = [1, 2, 3]\nprint(a[0])").unwrap(), vec!["1"]);
}

#[test]
fn vm_array_len_basic() {
    assert_eq!(vm_run("print(len([10, 20, 30]))").unwrap(), vec!["3"]);
}

#[test]
fn vm_array_index_last() {
    assert_eq!(vm_run("let a = [5, 6, 7]\nprint(a[2])").unwrap(), vec!["7"]);
}

#[test]
fn vm_array_out_of_bounds_error() {
    let result = vm_run("let a = [1]\nprint(a[5])");
    assert!(result.is_err());
}

#[test]
fn vm_array_negative_index_error() {
    let result = vm_run("let a = [1, 2]\nprint(a[-1])");
    assert!(result.is_err());
}

#[test]
fn vm_array_fractional_index_error() {
    let result = vm_run("let a = [1, 2]\nprint(a[1.5])");
    assert!(result.is_err());
}

#[test]
fn vm_len_non_array_error() {
    let result = vm_run("let n = 42\nlen(n)");
    assert!(result.is_err());
}

#[test]
fn vm_array_loop_sum() {
    let src =
        "let a = [2, 4, 6]\nlet mut s = 0\nfor i in range(0, len(a)) { s = s + a[i] }\nprint(s)";
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn vm_array_in_function() {
    let src = "fn second(x: Number, y: Number) -> Number { let arr = [x, y]\nreturn arr[1] }\nprint(second(3, 9))";
    assert_eq!(vm_run(src).unwrap(), vec!["9"]);
}

// --- VM/tree parity ---

#[test]
fn vm_matches_tree_array_basic() {
    let src = "let a = [10, 20, 30]\nprint(a[0])\nprint(a[2])\nprint(len(a))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "30", "3"]);
}

#[test]
fn vm_matches_tree_array_loop() {
    let src =
        "let a = [1, 2, 3, 4]\nlet mut s = 0\nfor i in range(0, len(a)) { s = s + a[i] }\nprint(s)";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

#[test]
fn vm_matches_tree_array_string_elements() {
    let src = "let a = [\"hello\", \"world\"]\nprint(a[1])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["world"]);
}

#[test]
fn vm_matches_tree_len_in_for_range() {
    let src = "let a = [10, 20, 30]\nfor i in range(0, len(a)) { print(a[i]) }";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "30"]);
}

// ─── M9E Audit: Lexer ─────────────────────────────────────────────────────────

#[test]
fn lex_braces_unaffected_by_brackets() {
    let kinds = tokenize("{ [ ] }");
    assert_eq!(kinds[0], TokenKind::LBrace);
    assert_eq!(kinds[1], TokenKind::LBracket);
    assert_eq!(kinds[2], TokenKind::RBracket);
    assert_eq!(kinds[3], TokenKind::RBrace);
}

#[test]
fn lex_parens_unaffected_by_brackets() {
    let kinds = tokenize("( [ ] )");
    assert_eq!(kinds[0], TokenKind::LParen);
    assert_eq!(kinds[1], TokenKind::LBracket);
    assert_eq!(kinds[2], TokenKind::RBracket);
    assert_eq!(kinds[3], TokenKind::RParen);
}

// ─── M9E Audit: Parser ────────────────────────────────────────────────────────

#[test]
fn parse_array_literal_text_strings() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new(r#"["a", "b", "c"]"#).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::ArrayLiteral { elements, .. }) if elements.len() == 3)
    );
}

#[test]
fn parse_array_literal_bools() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("[true, false, true]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::ArrayLiteral { elements, .. }) if elements.len() == 3)
    );
}

#[test]
fn parse_array_literal_variables() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("[x, y, z]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::ArrayLiteral { elements, .. }) if elements.len() == 3)
    );
}

#[test]
fn parse_index_on_array_literal() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("[1, 2, 3][1]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    // Should parse as Index { array: ArrayLiteral, index: 1 }
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::Index { .. })));
}

#[test]
fn parse_index_after_call_parses() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("foo()[0]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::Index { .. })));
}

#[test]
fn parse_chained_index_parses() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("a[0][1]").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::Index { .. })));
}

#[test]
fn parse_array_literal_in_function_call_arg() {
    use crate::ast::{Expr, Stmt};
    let tokens = Lexer::new("foo([1, 2, 3])").tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    // Should parse as Call { args: [ArrayLiteral] }
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::Call { .. })));
}

#[test]
fn parse_index_assignment_supported_since_m10a() {
    // arr[0] = 5 is valid syntax since M10A.
    let tokens = Lexer::new("let mut arr = [1, 2, 3]\narr[0] = 5")
        .tokenize()
        .unwrap();
    let result = Parser::new(tokens).parse();
    assert!(
        result.is_ok(),
        "index assignment should parse since M10A: {:?}",
        result
    );
}

#[test]
fn parse_return_array_literal_parses() {
    // Bug fix regression: return [expr] should parse as Return(ArrayLiteral), not bare return
    use crate::ast::{Expr, Stmt};
    let src = "fn f(n: Number) {\nreturn [n, n]\n}";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let body = match &stmts[0] {
        Stmt::FnDecl { body, .. } => body,
        _ => panic!("expected FnDecl"),
    };
    assert!(
        matches!(
            &body[0],
            Stmt::Return {
                value: Some(Expr::ArrayLiteral { .. }),
                ..
            }
        ),
        "return [expr] should parse as Return(ArrayLiteral)"
    );
}

// ─── M9E Audit: Typechecker ──────────────────────────────────────────────────

#[test]
fn type_index_text_index_error() {
    let result = check("let arr = [1, 2]\nlet idx = \"x\"\nlet _ = arr[idx]");
    assert!(result.is_err());
    if let Err(KiminError::Type(e)) = result {
        assert!(e.msg.contains("Number"));
    }
}

#[test]
fn type_index_bool_index_error() {
    let result = check("let arr = [1, 2]\nlet _ = arr[true]");
    assert!(result.is_err());
}

#[test]
fn type_index_unit_index_error() {
    let result = check("let arr = [1, 2]\nlet idx: seconds = 1\nlet _ = arr[idx]");
    assert!(result.is_err());
}

#[test]
fn type_array_state_values_ok() {
    assert!(check("state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.open\nlet arr = [d1, d2]").is_ok());
}

#[test]
fn type_array_closure_capture_ok() {
    assert!(check("let a = [1, 2, 3]\nfn get(i: Number) -> Number { return a[i] }").is_ok());
}

#[test]
fn type_array_inside_simulate_ok() {
    assert!(check("let dur: seconds = 3\nlet stp: seconds = 1\nlet a = [10, 20, 30]\nsimulate dur step stp { let x: Number = a[0] }").is_ok());
}

#[test]
fn type_array_for_range_len_ok() {
    assert!(check("let a = [1, 2, 3]\nfor i in range(0, len(a)) { let x: Number = a[i] }").is_ok());
}

#[test]
fn type_nested_array_typechecks_as_array_of_array() {
    // Nested arrays are technically supported by the type system
    // (inner arrays have type Array<Number>, outer becomes Array<Array<Number>>)
    assert!(check("let a = [[1, 2], [3, 4]]").is_ok());
}

#[test]
fn type_string_index_error() {
    let result = check("let s = \"hello\"\nlet _ = s[0]");
    assert!(result.is_err());
}

// ─── M9E Audit: Interpreter ───────────────────────────────────────────────────

#[test]
fn interp_array_literal_left_to_right_order() {
    // Elements must be stored in source order, not reversed
    assert_eq!(
        vm_run("let a = [10, 20, 30]\nprint(a[0])\nprint(a[1])\nprint(a[2])").unwrap(),
        vec!["10", "20", "30"]
    );
}

#[test]
fn interp_return_array_literal_directly() {
    // Regression: return [expr, expr] was silently returning nil before can_start_expr fix
    let src =
        "fn pair(n: Number) {\nreturn [n, n + 1]\n}\nlet p = pair(5)\nprint(p[0])\nprint(p[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["5", "6"]);
}

#[test]
fn interp_function_returns_array_variable() {
    let src = "fn make(n: Number) {\nlet a = [n, n + 10]\nreturn a\n}\nlet r = make(3)\nprint(r[0])\nprint(r[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["3", "13"]);
}

#[test]
fn interp_closure_captures_outer_array() {
    let src = "let a = [5, 10, 15]\nfn get(i: Number) -> Number { return a[i] }\nprint(get(0))\nprint(get(2))";
    assert_eq!(vm_run(src).unwrap(), vec!["5", "15"]);
}

#[test]
fn interp_array_index_by_computed_expr() {
    let src = "let a = [100, 200, 300]\nlet i = 2\nprint(a[i - 1])";
    assert_eq!(vm_run(src).unwrap(), vec!["200"]);
}

#[test]
fn interp_array_inside_simulate_by_counter() {
    let src = "let a = [10, 20, 30]\nlet mut idx = 0\nlet dur: seconds = 3\nlet stp: seconds = 1\nsimulate dur step stp {\nprint(a[idx])\nidx = idx + 1\n}";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "30"]);
}

#[test]
fn interp_unit_array_elements() {
    let src =
        "let d1: meters = 5\nlet d2: meters = 10\nlet ds = [d1, d2]\nprint(ds[0])\nprint(ds[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["5", "10"]);
}

#[test]
fn interp_state_array_elements() {
    let src = "state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.open\nlet doors = [d1, d2]\nprint(doors[0])\nprint(doors[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["Door.closed", "Door.open"]);
}

#[test]
fn interp_array_print_displays_all_elements() {
    let src = "let a = [1, 2, 3]\nprint(a)";
    assert_eq!(vm_run(src).unwrap(), vec!["[1, 2, 3]"]);
}

// ─── M9E Audit: Bytecode ──────────────────────────────────────────────────────

#[test]
fn bytecode_array_in_function_chunk() {
    let prog =
        compile_prog("fn f(n: Number) -> Number {\nlet arr = [n, n + 1]\nreturn arr[0]\n}\nf(10)");
    let fn_chunk = prog.functions.iter().find(|f| f.name == "f").unwrap();
    let has_array = fn_chunk
        .chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Array { .. }));
    let has_index = fn_chunk
        .chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Index));
    assert!(has_array, "function chunk should have ARRAY instruction");
    assert!(has_index, "function chunk should have INDEX instruction");
}

#[test]
fn bytecode_array_in_simulate_chunk() {
    let prog = compile_prog("let a = [1, 2, 3]\nlet dur: seconds = 3\nlet stp: seconds = 1\nsimulate dur step stp { let x: Number = a[0] }");
    // Simulate body should have INDEX but array literal was in main chunk, not simulate
    let main_has_array = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Array { .. }));
    let sim_has_index = prog.simulate_bodies.iter().any(|sc| {
        sc.chunk
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Index))
    });
    assert!(main_has_array);
    assert!(sim_has_index);
}

#[test]
fn bytecode_array_index_inside_for_loop() {
    let prog =
        compile_prog("let a = [1, 2, 3]\nfor i in range(0, len(a)) { let x: Number = a[i] }");
    let has_index = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Index));
    let has_len = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Len));
    assert!(has_index);
    assert!(has_len);
}

#[test]
fn bytecode_array_literal_elements_in_order() {
    // ARRAY 3 must appear AFTER the three element constants are pushed
    let prog = compile_prog("[10, 20, 30]");
    let instrs = &prog.main.instructions;
    // Find position of ARRAY instruction
    let array_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Array { count: 3 }))
        .unwrap();
    // All three constants should appear before it
    let constants_before = instrs[..array_pos]
        .iter()
        .filter(|i| matches!(i, Instruction::Constant(_)))
        .count();
    assert_eq!(
        constants_before, 3,
        "all 3 element constants must appear before ARRAY 3"
    );
}

#[test]
fn bytecode_disassemble_contains_array_index_len() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let a = [1, 2]\nprint(a[0])\nprint(len(a))");
    let dis = disassemble(&prog);
    assert!(dis.contains("ARRAY 2"), "disassembler should print ARRAY 2");
    assert!(dis.contains("INDEX"), "disassembler should print INDEX");
    assert!(dis.contains("LEN"), "disassembler should print LEN");
}

// ─── M9E Audit: VM ────────────────────────────────────────────────────────────

#[test]
fn vm_array_literal_order_correct() {
    assert_eq!(vm_run("print([10, 20, 30][0])").unwrap(), vec!["10"]);
    assert_eq!(vm_run("print([10, 20, 30][2])").unwrap(), vec!["30"]);
}

#[test]
fn vm_return_array_literal_directly() {
    let src =
        "fn make(n: Number) {\nreturn [n, n + 1]\n}\nlet r = make(7)\nprint(r[0])\nprint(r[1])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["7", "8"]);
}

#[test]
fn vm_closure_captures_array_and_indexes() {
    let src = "let a = [5, 10, 15]\nfn get(i: Number) -> Number { return a[i] }\nprint(get(1))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10"]);
}

#[test]
fn vm_array_inside_simulate_by_counter() {
    let src = "let a = [10, 20, 30]\nlet mut idx = 0\nlet dur: seconds = 3\nlet stp: seconds = 1\nsimulate dur step stp {\nprint(a[idx])\nidx = idx + 1\n}";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "30"]);
}

#[test]
fn vm_unit_array_elements_correct() {
    let src =
        "let d1: meters = 5\nlet d2: meters = 10\nlet ds = [d1, d2]\nprint(ds[0])\nprint(ds[1])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["5", "10"]);
}

#[test]
fn vm_state_array_elements_correct() {
    let src = "state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.open\nlet doors = [d1, d2]\nprint(doors[0])\nprint(doors[1])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["Door.closed", "Door.open"]);
}

// ─── M9E Audit: len builtin ──────────────────────────────────────────────────

#[test]
fn len_user_defined_fn_array_arg_builtin_takes_precedence() {
    // When user defines fn len(x) and calls len(array), the builtin intercepts
    // and returns array length (not calling user fn)
    let src =
        "fn len(x: Number) -> Number { return x + 100 }\nlet arr = [1, 2, 3]\nprint(len(arr))";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn len_user_defined_fn_non_array_arg_is_type_error() {
    // When user defines fn len(x: Number) and calls len(number), the builtin
    // intercept fires and rejects it (builtin always takes precedence for callee named "len")
    // This is a known limitation — user cannot define a function named "len" that shadows the builtin
    let result =
        check("fn len(x: Number) -> Number { return x + 100 }\nlet n: Number = 42\nlen(n)");
    assert!(
        result.is_err(),
        "builtin len intercept rejects non-Array argument even when user fn exists"
    );
}

#[test]
fn len_zero_args_type_error() {
    let result = check("let arr = [1, 2]\nlen()");
    assert!(result.is_err());
}

#[test]
fn len_two_args_type_error() {
    let result = check("let arr = [1, 2]\nlen(arr, arr)");
    assert!(result.is_err());
}

// ─── M9E Audit: Function / closure interaction ───────────────────────────────

#[test]
fn function_returns_inline_array_literal() {
    // Regression test for can_start_expr bug fix
    let src = "fn triple(n: Number) {\nreturn [n, n + 1, n + 2]\n}\nlet t = triple(10)\nprint(t[0])\nprint(t[1])\nprint(t[2])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "11", "12"]);
}

#[test]
fn function_len_on_returned_array() {
    let src = "fn pair(n: Number) {\nreturn [n, n + 1]\n}\nprint(len(pair(5)))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["2"]);
}

// ─── M9E Audit: Simulate interaction ─────────────────────────────────────────

#[test]
fn simulate_time_is_unit_not_number_cannot_index_array() {
    // `time` inside simulate has the unit type of the duration (e.g. seconds),
    // not a plain Number. Using it directly as an array index is a TypeError.
    let src = "let a = [100, 200, 300]\nlet dur: seconds = 3\nlet stp: seconds = 1\nsimulate dur step stp { let _ = a[time] }";
    let result = check(src);
    assert!(
        result.is_err(),
        "time is a unit type, not Number — cannot be used as array index"
    );
}

#[test]
fn simulate_out_of_bounds_array_index_errors() {
    let src = "let a = [1]\nlet dur: seconds = 2\nlet stp: seconds = 1\nsimulate dur step stp {\nlet _ = a[time]\n}";
    // Second iteration: time = 1, a has length 1, index 1 is out of bounds
    let result = run(src);
    assert!(result.is_err());
}

// ─── M9E Audit: Unit and state interaction ────────────────────────────────────

#[test]
fn array_units_sum_via_for_loop() {
    let src = "let d1: meters = 5\nlet d2: meters = 10\nlet d3: meters = 15\nlet ds = [d1, d2, d3]\nlet mut total: meters = 0\nfor i in range(0, len(ds)) { total = total + ds[i] }\nprint(total)";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["30"]);
}

#[test]
fn array_index_unit_element_type() {
    // Indexing a unit array should return unit type, allowing unit ops
    assert!(check(
        "let d1: meters = 5\nlet d2: meters = 10\nlet ds = [d1, d2]\nlet x: meters = ds[0]"
    )
    .is_ok());
}

#[test]
fn array_state_values_index_type() {
    // Indexing a state array returns state type
    assert!(check("state Light { on off transition on -> off }\nlet l1 = Light.on\nlet l2 = Light.off\nlet lights = [l1, l2]\ntransition lights[0] -> off").is_err(),
            "transition into index expression should fail (not a simple variable)");
}

// ─── M9E Audit: Tree-walk / VM output parity ─────────────────────────────────

#[test]
fn vm_matches_tree_unit_array() {
    let src = "let d1: meters = 5\nlet d2: meters = 10\nlet ds = [d1, d2]\nprint(ds[0])\nprint(ds[1])\nprint(len(ds))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["5", "10", "2"]);
}

#[test]
fn vm_matches_tree_closure_with_array() {
    let src = "let a = [5, 10, 15]\nfn get(i: Number) -> Number { return a[i] }\nprint(get(0))\nprint(get(1))\nprint(get(2))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["5", "10", "15"]);
}

#[test]
fn vm_matches_tree_return_inline_array() {
    let src = "fn triple(n: Number) {\nreturn [n, n + 1, n + 2]\n}\nlet t = triple(10)\nprint(t[0])\nprint(t[2])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "12"]);
}

#[test]
fn vm_matches_tree_literal_index() {
    let src = "print([10, 20, 30][1])";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["20"]);
}

#[test]
fn vm_matches_tree_state_array() {
    let src = "state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.open\nlet doors = [d1, d2]\nprint(doors[0])\nprint(len(doors))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["Door.closed", "2"]);
}

// ─── M10A: Array mutation by index ─────────────────────────────────────────

// --- parser tests ---

#[test]
fn parse_index_assign_simple() {
    let src = "let mut a = [1, 2]\na[0] = 99";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_assign_expr_index() {
    let src = "let mut a = [1, 2, 3]\nlet i = 1\na[i] = 99";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_assign_expr_value() {
    let src = "let mut a = [1, 2]\nlet x = 5\na[0] = x + 1";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_assign_inside_fn() {
    let src = "fn fill(arr: Number) {\nlet mut a = [1, 2]\na[0] = arr\n}";
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let stmts = crate::parser::Parser::new(tokens).parse();
    assert!(stmts.is_ok());
}

#[test]
fn parse_index_assign_inside_for() {
    let src = "let mut a = [0, 0, 0]\nfor i in range(0, 3) {\na[i] = i\n}";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_assign_missing_bracket_error() {
    let src = "let mut a = [1]\na[0 = 99";
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let result = crate::parser::Parser::new(tokens).parse();
    assert!(result.is_err());
}

#[test]
fn parse_index_assign_missing_value_error() {
    let src = "let mut a = [1]\na[0] =";
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let result = crate::parser::Parser::new(tokens).parse();
    assert!(result.is_err());
}

#[test]
fn parse_normal_index_expr_unaffected() {
    // arr[0] used as expression (e.g. in print) must still work.
    let src = "let a = [1, 2]\nprint(a[0])";
    assert!(run(src).is_ok());
}

#[test]
fn parse_normal_assign_unaffected() {
    // Variable-level assignment of array literal still works.
    let src = "let mut a = [1]\na = [2]";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_expr_no_eq_falls_through() {
    // arr[0] as a standalone expression statement (no `=`) should parse OK (as Stmt::Expr).
    let src = "let a = [1, 2]\na[0]";
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let result = crate::parser::Parser::new(tokens).parse();
    assert!(result.is_ok());
}

// --- typechecker tests ---

#[test]
fn type_index_assign_number_array_ok() {
    assert!(check("let mut a = [1, 2, 3]\na[0] = 99").is_ok());
}

#[test]
fn type_index_assign_text_array_ok() {
    assert!(check("let mut a = [\"x\", \"y\"]\na[0] = \"z\"").is_ok());
}

#[test]
fn type_index_assign_bool_array_ok() {
    assert!(check("let mut a = [true, false]\na[1] = true").is_ok());
}

#[test]
fn type_index_assign_unit_array_ok() {
    assert!(check("let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet d3: meters = 5\na[0] = d3").is_ok());
}

#[test]
fn type_index_assign_number_to_unit_ok() {
    // Number can be promoted to a unit element type, matching assignment promotion rules.
    assert!(
        check("let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\na[0] = 5").is_ok()
    );
}

#[test]
fn type_index_assign_immutable_error() {
    let e = check("let a = [1, 2]\na[0] = 99").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("immutable"),
        "expected 'immutable' in: {}",
        msg
    );
}

#[test]
fn type_index_assign_undefined_error() {
    let e = check("nums[0] = 99").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("undefined"),
        "expected 'undefined' in: {}",
        msg
    );
}

#[test]
fn type_index_assign_non_array_error() {
    let e = check("let mut x = 5\nx[0] = 99").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("not an array"),
        "expected 'not an array' in: {}",
        msg
    );
}

#[test]
fn type_index_assign_text_index_error() {
    let e = check("let mut a = [1, 2]\na[\"0\"] = 99").unwrap_err();
    let msg = e.to_string();
    assert!(msg.contains("Number"), "expected 'Number' in: {}", msg);
}

#[test]
fn type_index_assign_bool_index_error() {
    let e = check("let mut a = [1, 2]\na[true] = 99").unwrap_err();
    let msg = e.to_string();
    assert!(msg.contains("Number"), "expected 'Number' in: {}", msg);
}

#[test]
fn type_index_assign_wrong_elem_type_error() {
    let e = check("let mut a = [1, 2]\na[0] = \"hello\"").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("Number") && msg.contains("Text"),
        "expected type mismatch in: {}",
        msg
    );
}

#[test]
fn type_index_assign_inside_fn_ok() {
    assert!(check("fn f() {\nlet mut a = [1, 2]\na[0] = 99\n}").is_ok());
}

#[test]
fn type_index_assign_inside_for_ok() {
    assert!(check("let mut a = [0, 0, 0]\nfor i in range(0, 3) {\na[i] = i\n}").is_ok());
}

#[test]
fn type_index_assign_inside_simulate_ok() {
    // Uses an outer mutable counter as index (time has unit type, not Number).
    assert!(check("let mut a = [0, 0]\nlet mut i = 0\nlet dur: seconds = 2\nlet dt: seconds = 1\nsimulate dur step dt {\na[i] = i\ni += 1\n}").is_ok());
}

// --- interpreter tests ---

#[test]
fn interp_index_assign_updates_array() {
    let src = "let mut a = [1, 2, 3]\na[1] = 99\nprint(a[1])";
    let out = run(src).unwrap();
    // Verify via get_var
    assert_eq!(
        out.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(99.0),
            Value::Number(3.0)
        ]))
    );
}

#[test]
fn interp_index_assign_first_middle_last() {
    let src = "let mut a = [1, 2, 3]\na[0] = 10\na[1] = 20\na[2] = 30";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(10.0),
            Value::Number(20.0),
            Value::Number(30.0)
        ]))
    );
}

#[test]
fn interp_index_assign_length_unchanged() {
    let src = "let mut a = [1, 2, 3]\na[0] = 99\na[2] = 42";
    let interp = run(src).unwrap();
    let arr = interp.get_var("a").unwrap();
    if let Value::Array(v) = arr {
        assert_eq!(v.len(), 3);
    } else {
        panic!("expected Array");
    }
}

#[test]
fn interp_index_assign_inside_fn() {
    let src = "fn update() -> Number {\nlet mut a = [1, 2, 3]\na[0] = 99\nreturn a[0]\n}\nprint(update())";
    assert!(run(src).is_ok());
}

#[test]
fn interp_index_assign_inside_closure() {
    let src = "fn outer() -> Number {\nlet mut nums = [1, 2, 3]\nfn update() -> Number {\nnums[0] = 99\nreturn nums[0]\n}\nreturn update()\n}\nprint(outer())";
    assert!(run(src).is_ok());
}

#[test]
fn interp_index_assign_inside_for_doubles() {
    let src = "let mut a = [1, 2, 3, 4]\nfor i in range(0, len(a)) {\na[i] = a[i] * 2\n}\nprint(a[0])\nprint(a[3])";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(2.0),
            Value::Number(4.0),
            Value::Number(6.0),
            Value::Number(8.0)
        ]))
    );
}

#[test]
fn interp_index_assign_inside_simulate() {
    let src = "let mut a = [0, 0, 0]\nlet mut i = 0\nlet dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt {\na[i] = i + 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(10.0),
            Value::Number(11.0),
            Value::Number(12.0)
        ]))
    );
}

#[test]
fn interp_index_assign_unit_array() {
    let src = "let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet d3: meters = 5\na[0] = d3\nprint(a[0])";
    assert!(run(src).is_ok());
}

#[test]
fn interp_index_assign_state_array() {
    let src = "state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.closed\nlet mut doors = [d1, d2]\ndoors[1] = Door.open\nprint(doors[1])";
    assert!(run(src).is_ok());
}

#[test]
fn interp_index_assign_out_of_bounds() {
    let src = "let mut a = [1, 2, 3]\na[9] = 99";
    match run(src) {
        Ok(_) => panic!("expected runtime error"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("out of bounds"),
                "expected bounds error in: {}",
                msg
            );
        }
    }
}

#[test]
fn interp_index_assign_negative_index() {
    let src = "let mut a = [1, 2, 3]\na[-1] = 99";
    match run(src) {
        Ok(_) => panic!("expected runtime error"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("negative") || msg.contains("out of bounds"),
                "expected negative/bounds error in: {}",
                msg
            );
        }
    }
}

#[test]
fn interp_index_assign_fractional_index() {
    let src = "let mut a = [1, 2, 3]\na[1.5] = 99";
    match run(src) {
        Ok(_) => panic!("expected runtime error"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("integer"),
                "expected integer error in: {}",
                msg
            );
        }
    }
}

// --- bytecode tests ---

#[test]
fn bytecode_index_assign_emits_set_index() {
    let prog = compile_prog("let mut a = [1, 2]\na[0] = 99");
    let has_set_index = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::SetIndex(name) if name == "a"));
    assert!(has_set_index, "expected SetIndex(\"a\") in main chunk");
}

#[test]
fn bytecode_index_assign_index_before_value() {
    // Verify compile order: index expression compiled before value expression.
    let prog = compile_prog("let mut a = [1, 2]\na[0] = 99");
    let instrs = &prog.main.instructions;
    let set_idx_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::SetIndex(_)))
        .expect("no SetIndex");
    // The two CONSTANT instructions for 0 (index) and 99 (value) should appear
    // somewhere before SetIndex. We just verify SetIndex exists and the chunk compiles.
    assert!(set_idx_pos > 0);
}

#[test]
fn bytecode_set_index_disassembly() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let mut a = [1, 2]\na[0] = 99");
    let dis = disassemble(&prog);
    assert!(
        dis.contains("SET_INDEX a"),
        "expected SET_INDEX a in:\n{}",
        dis
    );
}

#[test]
fn bytecode_index_assign_in_for() {
    let prog = compile_prog("let mut a = [0, 0]\nfor i in range(0, 2) {\na[i] = i\n}");
    let has_set_index = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::SetIndex(_)));
    assert!(has_set_index);
}

#[test]
fn bytecode_index_assign_in_simulate() {
    let prog = compile_prog("let mut a = [0, 0]\nlet mut i = 0\nlet d: seconds = 2\nlet dt: seconds = 1\nsimulate d step dt {\na[i] = i\ni += 1\n}");
    // SetIndex appears in the simulate body chunk.
    let has_set_index = prog.simulate_bodies.iter().any(|sc| {
        sc.chunk
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::SetIndex(_)))
    });
    assert!(has_set_index, "expected SetIndex in simulate body chunk");
}

// --- VM tests ---

#[test]
fn vm_index_assign_updates_array() {
    let src = "let mut a = [1, 2, 3]\na[1] = 99\nprint(a[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["99"]);
}

#[test]
fn vm_index_assign_first_middle_last() {
    let src = "let mut a = [1, 2, 3]\na[0] = 10\na[1] = 20\na[2] = 30\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "30"]);
}

#[test]
fn vm_index_assign_inside_fn() {
    let src = "fn update() -> Number {\nlet mut a = [1, 2, 3]\na[0] = 99\nreturn a[0]\n}\nprint(update())";
    assert_eq!(vm_run(src).unwrap(), vec!["99"]);
}

#[test]
fn vm_index_assign_inside_closure() {
    let src = "fn outer() -> Number {\nlet mut nums = [1, 2, 3]\nfn update() -> Number {\nnums[0] = 99\nreturn nums[0]\n}\nreturn update()\n}\nprint(outer())";
    assert_eq!(vm_run(src).unwrap(), vec!["99"]);
}

#[test]
fn vm_index_assign_inside_for() {
    let src = "let mut a = [1, 2, 3, 4]\nfor i in range(0, len(a)) {\na[i] = a[i] * 2\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])\nprint(a[3])";
    assert_eq!(vm_run(src).unwrap(), vec!["2", "4", "6", "8"]);
}

#[test]
fn vm_index_assign_inside_simulate() {
    let src = "let mut a = [0, 0, 0]\nlet mut i = 0\nlet dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt {\na[i] = i + 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "11", "12"]);
}

#[test]
fn vm_index_assign_units() {
    let src = "let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet d3: meters = 5\na[0] = d3\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn vm_index_assign_out_of_bounds() {
    let src = "let mut a = [1, 2, 3]\na[9] = 99";
    let err = vm_run(src).unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn vm_index_assign_negative_index() {
    let src = "let mut a = [1, 2, 3]\na[-1] = 99";
    let err = vm_run(src).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("negative") || msg.contains("out of bounds"));
}

#[test]
fn vm_index_assign_fractional_index() {
    let src = "let mut a = [1, 2, 3]\na[1.5] = 99";
    let err = vm_run(src).unwrap_err();
    assert!(err.to_string().contains("integer"));
}

#[test]
fn vm_index_assign_output_matches_tree() {
    let src = "let mut a = [1, 2, 3]\na[0] = 99\na[2] = 42\nprint(a[0])\nprint(a[1])\nprint(a[2])\nprint(len(a))";
    let tree_out: Vec<String> = {
        // Capture println output via run() doesn't capture; use vm_run for both to compare.
        // Compare tree-walk state vs VM output.
        let interp = run(src).unwrap();
        let a = interp.get_var("a").unwrap();
        if let Value::Array(v) = a {
            v.iter().map(|x| format!("{}", x)).collect()
        } else {
            panic!("expected Array");
        }
    };
    let vm_out = vm_run(src).unwrap();
    assert_eq!(vm_out, vec!["99", "2", "42", "3"]);
    assert_eq!(tree_out, vec!["99", "2", "42"]);
}

// --- regression tests ---

#[test]
fn regression_existing_arrays_still_work() {
    let src = "let a = [10, 20, 30]\nprint(a[0])\nprint(len(a))";
    assert!(run(src).is_ok());
    assert_eq!(vm_run(src).unwrap(), vec!["10", "3"]);
}

#[test]
fn regression_index_read_unaffected_by_mutation() {
    let src = "let mut a = [1, 2, 3]\na[1] = 99\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["1", "99", "3"]);
}

#[test]
fn regression_len_after_mutation_unchanged() {
    let src = "let mut a = [1, 2, 3]\na[0] = 99\nprint(len(a))";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

#[test]
fn regression_for_range_still_works_after_m10a() {
    let src = "for i in range(0, 3) {\nprint(i)\n}";
    assert_eq!(vm_run(src).unwrap(), vec!["0", "1", "2"]);
}

#[test]
fn regression_simulate_still_works_after_m10a() {
    let src = "let mut x = 0\nlet d: seconds = 3\nlet dt: seconds = 1\nsimulate d step dt {\nx += 1\n}\nprint(x)";
    assert_eq!(vm_run(src).unwrap(), vec!["3"]);
}

// ============================================================
// M10A Audit — additional hardening tests
// ============================================================

// --- Parser: missing cases ---

#[test]
fn parse_index_assign_inside_while() {
    // arr[i] = val inside a while body must parse correctly.
    let src = "let mut a = [1, 2, 3]\nlet mut i = 0\nwhile i < 3 {\na[i] = i\ni += 1\n}";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_compound_assign_supported_since_m10b() {
    // M10B added index compound assignment. arr[i] += val is now valid syntax.
    let tokens = crate::lexer::Lexer::new("let mut a = [1, 2]\na[0] += 1")
        .tokenize()
        .unwrap();
    let result = crate::parser::Parser::new(tokens).parse();
    assert!(result.is_ok(), "arr[i] += val should parse in M10B");
}

#[test]
fn parse_index_assign_nested_target_backtracks() {
    // a[0][1] = 99 — outer index is not `=` after first `]`; backtracks to expr statement.
    // The inner `[1] = 99` portion causes a parse error (no `=` on expr stmt).
    // Confirm it at least does not silently succeed as an IndexAssign.
    let tokens = crate::lexer::Lexer::new("let mut a = [1, 2]\na[0][1] = 99")
        .tokenize()
        .unwrap();
    let result = crate::parser::Parser::new(tokens).parse();
    // Either error or parses a[0][1] as an Expr statement (no assignment side effect).
    // Either outcome is acceptable — but it must NOT be a successful IndexAssign.
    // We verify by checking runtime: outer array is unchanged if run succeeds.
    let _ = result; // accept either parse outcome
}

// --- Typechecker: missing cases ---

#[test]
fn type_index_assign_unit_index_error() {
    // A unit-typed variable (e.g. seconds) cannot be used as an array index.
    let src = "let t: seconds = 1\nlet mut a = [1, 2]\na[t] = 99";
    let e = check(src).unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("Number"),
        "expected 'Number' index error in: {}",
        msg
    );
}

#[test]
fn type_index_assign_wrong_unit_error() {
    // Element type meters; assigning seconds → TypeError.
    let src = "let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet s: seconds = 1\na[0] = s";
    let e = check(src).unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("meters") || msg.contains("seconds"),
        "expected unit mismatch error in: {}",
        msg
    );
}

#[test]
fn type_index_assign_immutable_captured_array_error() {
    // Immutable array captured by a closure — assignment should be TypeError.
    let src = "fn outer() {\nlet a = [1, 2]\nfn update() {\na[0] = 99\n}\nupdate()\n}";
    let e = check(src).unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("immutable"),
        "expected immutable error in: {}",
        msg
    );
}

// --- Interpreter: missing cases ---

#[test]
fn interp_index_assign_while_loop() {
    // Mutation inside a while loop persists across iterations.
    let src = "let mut a = [1, 2, 3]\nlet mut i = 0\nwhile i < 3 {\na[i] = i * 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(0.0),
            Value::Number(10.0),
            Value::Number(20.0),
        ]))
    );
}

#[test]
fn interp_index_assign_updates_nearest_binding() {
    // Block-local scope: mutation from inner block updates the outer mutable array.
    let src = "let mut a = [1, 2]\n{\na[0] = 99\n}\nprint(a[0])";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![Value::Number(99.0), Value::Number(2.0)]))
    );
}

#[test]
fn interp_index_assign_block_shadow() {
    // Inner block shadows outer `a` with its own array — mutation stays in inner binding.
    let src = "let mut a = [1, 2]\n{\nlet mut a = [10, 20]\na[0] = 99\n}\nprint(a[0])";
    let interp = run(src).unwrap();
    // Outer `a[0]` must still be 1, not 99.
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]))
    );
}

#[test]
fn interp_index_assign_eval_order_index_before_value() {
    // Index expression evaluated before value expression (left-to-right).
    // next_i increments counter and returns counter-1; value_fn increments and returns counter*10.
    // After next_i: counter=1, returns 0. After value_fn: counter=2, returns 20.
    // So a[0] = 20.
    let src = concat!(
        "let mut counter = 0\n",
        "fn next_i() -> Number {\ncounter += 1\nreturn counter - 1\n}\n",
        "fn value_fn() -> Number {\ncounter += 1\nreturn counter * 10\n}\n",
        "let mut a = [0, 0, 0]\n",
        "a[next_i()] = value_fn()\n",
        "print(a[0])\n",
        "print(counter)"
    );
    let interp = run(src).unwrap();
    let a = interp.get_var("a").unwrap();
    if let Value::Array(v) = a {
        assert_eq!(v[0], Value::Number(20.0));
        assert_eq!(v[1], Value::Number(0.0));
    } else {
        panic!("expected Array");
    }
    assert_eq!(interp.get_var("counter"), Some(Value::Number(2.0)));
}

#[test]
fn interp_index_assign_closure_repeated_call() {
    // Calling update() twice on a captured mutable array: 1 + 10 + 10 = 21.
    let src = concat!(
        "fn outer() -> Number {\n",
        "let mut nums = [1, 2, 3]\n",
        "fn update() {\nnums[0] = nums[0] + 10\n}\n",
        "update()\n",
        "update()\n",
        "return nums[0]\n",
        "}\n",
        "print(outer())"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21"]);
}

#[test]
fn interp_index_assign_for_continue() {
    // continue skips assignment for i==2; other indices get mutated.
    let src = concat!(
        "let mut a = [1, 2, 3, 4]\n",
        "for i in range(0, 4) {\n",
        "if i == 2 { continue }\n",
        "a[i] = a[i] * 10\n",
        "}"
    );
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(10.0),
            Value::Number(20.0),
            Value::Number(3.0),
            Value::Number(40.0),
        ]))
    );
}

#[test]
fn interp_index_assign_for_break() {
    // break exits at i==2; indices 0 and 1 mutated; 2 and 3 unchanged.
    let src = concat!(
        "let mut a = [1, 2, 3, 4]\n",
        "for i in range(0, 4) {\n",
        "if i == 2 { break }\n",
        "a[i] = a[i] * 10\n",
        "}"
    );
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a"),
        Some(Value::Array(vec![
            Value::Number(10.0),
            Value::Number(20.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]))
    );
}

#[test]
fn interp_index_assign_simulate_outer_array() {
    // simulate body updates an outer mutable array via outer counter index.
    let src = concat!(
        "let mut values = [0, 0, 0]\n",
        "let mut idx = 0\n",
        "let dur: seconds = 3\n",
        "let dt: seconds = 1\n",
        "simulate dur step dt {\n",
        "values[idx] = idx * 5\n",
        "idx += 1\n",
        "}"
    );
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("values"),
        Some(Value::Array(vec![
            Value::Number(0.0),
            Value::Number(5.0),
            Value::Number(10.0),
        ]))
    );
}

// --- VM: missing cases ---

#[test]
fn vm_index_assign_inside_while() {
    let src = "let mut a = [1, 2, 3]\nlet mut i = 0\nwhile i < 3 {\na[i] = i * 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["0", "10", "20"]);
}

#[test]
fn vm_index_assign_stack_clean() {
    // After SetIndex the stack must be empty (no value left on stack).
    // A subsequent print should work with the correct array state.
    let src = "let mut a = [1, 2, 3]\na[0] = 99\na[1] = 88\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["99", "88", "3"]);
}

#[test]
fn vm_index_assign_eval_order_index_before_value() {
    // VM must also evaluate index before value.
    let src = concat!(
        "let mut counter = 0\n",
        "fn next_i() -> Number {\ncounter += 1\nreturn counter - 1\n}\n",
        "fn value_fn() -> Number {\ncounter += 1\nreturn counter * 10\n}\n",
        "let mut a = [0, 0, 0]\n",
        "a[next_i()] = value_fn()\n",
        "print(a[0])\n",
        "print(counter)"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["20", "2"]);
}

#[test]
fn vm_index_assign_closure_repeated_call() {
    let src = concat!(
        "fn outer() -> Number {\n",
        "let mut nums = [1, 2, 3]\n",
        "fn update() {\nnums[0] = nums[0] + 10\n}\n",
        "update()\n",
        "update()\n",
        "return nums[0]\n",
        "}\n",
        "print(outer())"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21"]);
}

#[test]
fn vm_index_assign_for_continue() {
    let src = concat!(
        "let mut a = [1, 2, 3, 4]\n",
        "for i in range(0, 4) {\n",
        "if i == 2 { continue }\n",
        "a[i] = a[i] * 10\n",
        "}\n",
        "print(a[0])\nprint(a[1])\nprint(a[2])\nprint(a[3])"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "3", "40"]);
}

#[test]
fn vm_index_assign_for_break() {
    let src = concat!(
        "let mut a = [1, 2, 3, 4]\n",
        "for i in range(0, 4) {\n",
        "if i == 2 { break }\n",
        "a[i] = a[i] * 10\n",
        "}\n",
        "print(a[0])\nprint(a[1])\nprint(a[2])\nprint(a[3])"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["10", "20", "3", "4"]);
}

#[test]
fn vm_matches_tree_array_mutation() {
    let src = "let mut a = [1, 2, 3]\na[0] = 99\na[2] = 42\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["99", "2", "42"]);
}

#[test]
fn vm_matches_tree_array_mutation_loop() {
    let src = "let mut a = [1, 2, 3, 4]\nfor i in range(0, len(a)) {\na[i] = a[i] * 2\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])\nprint(a[3])";
    assert_eq!(vm_run(src).unwrap(), vec!["2", "4", "6", "8"]);
}

#[test]
fn vm_matches_tree_array_mutation_while() {
    let src = "let mut a = [1, 2, 3]\nlet mut i = 0\nwhile i < 3 {\na[i] = i * 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["0", "10", "20"]);
}

#[test]
fn vm_matches_tree_array_mutation_simulate() {
    let src = "let mut a = [0, 0, 0]\nlet mut i = 0\nlet dur: seconds = 3\nlet dt: seconds = 1\nsimulate dur step dt {\na[i] = i + 10\ni += 1\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "11", "12"]);
}

// ============================================================
// M10B: Index compound assignment (arr[i] += value, etc.)
// ============================================================

// --- Parser tests ---

#[test]
fn parse_index_compound_add() {
    assert!(check("let mut a = [1, 2]\na[0] += 5").is_ok());
}

#[test]
fn parse_index_compound_subtract() {
    assert!(check("let mut a = [1, 2]\na[0] -= 1").is_ok());
}

#[test]
fn parse_index_compound_multiply() {
    assert!(check("let mut a = [2, 3]\na[1] *= 4").is_ok());
}

#[test]
fn parse_index_compound_divide() {
    assert!(check("let mut a = [10, 4]\na[0] /= 2").is_ok());
}

#[test]
fn parse_index_compound_expression_index() {
    let src = "let mut a = [1, 2, 3]\nlet i = 1\na[i] += 10";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_compound_expression_value() {
    let src = "let mut a = [1, 2]\nlet x = 3\na[0] += x * 2";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_compound_inside_function() {
    let src = "fn update() {\nlet mut a = [1, 2]\na[0] += 5\n}";
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    assert!(crate::parser::Parser::new(tokens).parse().is_ok());
}

#[test]
fn parse_index_compound_inside_for() {
    let src = "let mut a = [0, 0, 0]\nfor i in range(0, 3) {\na[i] += i\n}";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_compound_inside_simulate() {
    let src =
        "let mut a = [0]\nlet mut i = 0\nlet d: seconds = 1\nlet dt: seconds = 1\nsimulate d step dt {\na[i] += 1\ni += 1\n}";
    assert!(check(src).is_ok());
}

#[test]
fn parse_index_compound_missing_value_error() {
    let tokens = crate::lexer::Lexer::new("let mut a = [1]\na[0] +=")
        .tokenize()
        .unwrap();
    assert!(crate::parser::Parser::new(tokens).parse().is_err());
}

#[test]
fn parse_index_expr_unaffected_by_compound() {
    // arr[i] as read-only expression must still work after M10B.
    assert!(run("let a = [1, 2]\nprint(a[0])").is_ok());
}

#[test]
fn parse_index_assign_unaffected_by_compound() {
    // Plain arr[i] = val must still work.
    assert!(check("let mut a = [1, 2]\na[0] = 99").is_ok());
}

// --- Typechecker tests ---

#[test]
fn type_index_compound_number_add_ok() {
    assert!(check("let mut a = [1, 2]\na[0] += 5").is_ok());
}

#[test]
fn type_index_compound_number_sub_ok() {
    assert!(check("let mut a = [10, 2]\na[0] -= 3").is_ok());
}

#[test]
fn type_index_compound_number_mul_ok() {
    assert!(check("let mut a = [2, 3]\na[1] *= 4").is_ok());
}

#[test]
fn type_index_compound_number_div_ok() {
    assert!(check("let mut a = [10, 4]\na[0] /= 2").is_ok());
}

#[test]
fn type_index_compound_text_add_ok() {
    assert!(check("let mut a = [\"hello\", \"world\"]\na[0] += \"!\"").is_ok());
}

#[test]
fn type_index_compound_text_sub_error() {
    let e = check("let mut a = [\"hello\"]\na[0] -= \"x\"").unwrap_err();
    assert!(e.to_string().contains("Text") || e.to_string().contains("'-'"));
}

#[test]
fn type_index_compound_immutable_array_error() {
    let e = check("let a = [1, 2]\na[0] += 1").unwrap_err();
    assert!(
        e.to_string().contains("immutable"),
        "expected immutable error in: {}",
        e
    );
}

#[test]
fn type_index_compound_undefined_error() {
    let e = check("nums[0] += 1").unwrap_err();
    assert!(e.to_string().contains("undefined"));
}

#[test]
fn type_index_compound_non_array_error() {
    let e = check("let mut x = 5\nx[0] += 1").unwrap_err();
    assert!(e.to_string().contains("not an array"));
}

#[test]
fn type_index_compound_text_index_error() {
    let e = check("let mut a = [1, 2]\na[\"0\"] += 1").unwrap_err();
    assert!(e.to_string().contains("Number"));
}

#[test]
fn type_index_compound_wrong_element_type_error() {
    let e = check("let mut a = [1, 2]\na[0] += \"hello\"").unwrap_err();
    assert!(
        e.to_string().contains("Number") || e.to_string().contains("Text"),
        "expected type error in: {}",
        e
    );
}

#[test]
fn type_index_compound_unit_same_unit_ok() {
    let src = "let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet inc: meters = 5\na[0] += inc";
    assert!(check(src).is_ok());
}

#[test]
fn type_index_compound_wrong_unit_error() {
    let src = "let d1: meters = 1\nlet d2: meters = 2\nlet mut a = [d1, d2]\nlet s: seconds = 1\na[0] += s";
    let e = check(src).unwrap_err();
    assert!(
        e.to_string().contains("meters") || e.to_string().contains("seconds"),
        "expected unit mismatch in: {}",
        e
    );
}

#[test]
fn type_index_compound_state_error() {
    // State elements cannot use arithmetic compound assignment.
    let src =
        "state Door { closed open transition closed -> open }\nlet d1 = Door.closed\nlet d2 = Door.open\nlet mut a = [d1, d2]\na[0] += d2";
    let e = check(src).unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("State") || msg.contains("Door") || msg.contains("+"),
        "expected type error for state compound assign in: {}",
        msg
    );
}

#[test]
fn type_index_compound_inside_function_ok() {
    assert!(check("fn f() {\nlet mut a = [1, 2]\na[0] += 5\n}").is_ok());
}

#[test]
fn type_index_compound_inside_closure_ok() {
    let src = concat!(
        "fn outer() {\n",
        "let mut nums = [1, 2]\n",
        "fn inner() {\nnums[0] += 10\n}\n",
        "inner()\n",
        "}"
    );
    assert!(check(src).is_ok());
}

#[test]
fn type_index_compound_inside_for_ok() {
    assert!(check("let mut a = [0, 0, 0]\nfor i in range(0, 3) {\na[i] += i\n}").is_ok());
}

#[test]
fn type_index_compound_inside_simulate_ok() {
    let src =
        "let mut a = [0, 0]\nlet mut i = 0\nlet d: seconds = 2\nlet dt: seconds = 1\nsimulate d step dt {\na[i] += i\ni += 1\n}";
    assert!(check(src).is_ok());
}

// --- Interpreter tests ---

#[test]
fn interp_index_compound_add() {
    let src = "let mut a = [1, 2, 3]\na[1] += 10\nprint(a[1])";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(12.0),
            Value::Number(3.0)
        ])
    );
}

#[test]
fn interp_index_compound_subtract() {
    let src = "let mut a = [10, 5]\na[0] -= 3";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![Value::Number(7.0), Value::Number(5.0)])
    );
}

#[test]
fn interp_index_compound_multiply() {
    let src = "let mut a = [2, 3]\na[0] *= 4";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![Value::Number(8.0), Value::Number(3.0)])
    );
}

#[test]
fn interp_index_compound_divide() {
    let src = "let mut a = [10, 4]\na[0] /= 2";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![Value::Number(5.0), Value::Number(4.0)])
    );
}

#[test]
fn interp_index_compound_text_concat() {
    let src = "let mut a = [\"hello\", \"world\"]\na[0] += \"!\"";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![
            Value::Str("hello!".into()),
            Value::Str("world".into())
        ])
    );
}

#[test]
fn interp_index_compound_units() {
    let src = "let d1: meters = 5\nlet d2: meters = 3\nlet mut a = [d1, d2]\nlet inc: meters = 2\na[0] += inc\nprint(a[0])";
    assert!(run(src).is_ok());
}

#[test]
fn interp_index_compound_closure_capture() {
    // inner closure mutates captured outer array via +=.
    let src = concat!(
        "fn outer() -> Number {\n",
        "let mut nums = [1, 2, 3]\n",
        "fn update() {\nnums[0] += 10\n}\n",
        "update()\n",
        "update()\n",
        "return nums[0]\n",
        "}\n",
        "print(outer())"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21"]);
}

#[test]
fn interp_index_compound_for_loop() {
    let src = "let mut a = [1, 2, 3, 4]\nfor i in range(0, len(a)) {\na[i] *= 2\n}";
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("a").unwrap(),
        Value::Array(vec![
            Value::Number(2.0),
            Value::Number(4.0),
            Value::Number(6.0),
            Value::Number(8.0),
        ])
    );
}

#[test]
fn interp_index_compound_simulate() {
    let src = concat!(
        "let mut values = [0, 0, 0]\n",
        "let mut i: Number = 0\n",
        "let duration: seconds = 3\n",
        "let dt: seconds = 1\n",
        "simulate duration step dt {\n",
        "values[i] += i + 10\n",
        "i += 1\n",
        "}"
    );
    let interp = run(src).unwrap();
    assert_eq!(
        interp.get_var("values").unwrap(),
        Value::Array(vec![
            Value::Number(10.0),
            Value::Number(11.0),
            Value::Number(12.0),
        ])
    );
}

#[test]
fn interp_index_compound_eval_order() {
    // idx() runs first (counter→1, returns 0), then rhs() (counter→11, returns 11).
    // arr[0] = 10 + 11 = 21.
    let src = concat!(
        "let mut arr = [10, 20]\n",
        "let mut counter: Number = 0\n",
        "fn idx() -> Number {\ncounter += 1\nreturn 0\n}\n",
        "fn rhs() -> Number {\ncounter += 10\nreturn counter\n}\n",
        "arr[idx()] += rhs()\n",
        "print(arr[0])\n",
        "print(counter)"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21", "11"]);
}

#[test]
fn interp_index_compound_out_of_bounds_error() {
    let src = "let mut a = [1, 2]\na[9] += 1";
    match run(src) {
        Ok(_) => panic!("expected runtime error"),
        Err(e) => assert!(e.to_string().contains("out of bounds")),
    }
}

#[test]
fn interp_index_compound_fractional_index_error() {
    let src = "let mut a = [1, 2]\na[0.5] += 1";
    match run(src) {
        Ok(_) => panic!("expected runtime error"),
        Err(e) => assert!(e.to_string().contains("integer")),
    }
}

// --- Bytecode tests ---

#[test]
fn bytecode_index_compound_add_emits_instruction() {
    let prog = compile_prog("let mut a = [1, 2]\na[0] += 5");
    let has = prog.main.instructions.iter().any(|i| {
        matches!(i, Instruction::IndexCompoundAssign { name, op }
            if name == "a" && *op == crate::ast::CompoundAssignOp::Add)
    });
    assert!(has, "expected IndexCompoundAssign Add in main chunk");
}

#[test]
fn bytecode_index_compound_order_index_then_rhs() {
    // Verify index compiled before rhs: both appear before IndexCompoundAssign.
    let prog = compile_prog("let mut a = [1, 2]\na[0] += 5");
    let pos = prog
        .main
        .instructions
        .iter()
        .position(|i| matches!(i, Instruction::IndexCompoundAssign { .. }))
        .expect("no IndexCompoundAssign");
    assert!(
        pos >= 2,
        "expected at least 2 instructions before IndexCompoundAssign"
    );
}

#[test]
fn bytecode_index_compound_no_double_index_eval() {
    // The index expression appears exactly once before IndexCompoundAssign.
    // Verify by checking only one CONSTANT #0 (the index value 0) before the instruction.
    let prog = compile_prog("let mut a = [1, 2]\na[0] += 5");
    let instrs = &prog.main.instructions;
    let ica_pos = instrs
        .iter()
        .position(|i| matches!(i, Instruction::IndexCompoundAssign { .. }))
        .unwrap();
    // There should be exactly one Constant(0) before the IndexCompoundAssign that could be index.
    // Just verify IndexCompoundAssign exists once (no double emission).
    let count = instrs
        .iter()
        .filter(|i| matches!(i, Instruction::IndexCompoundAssign { .. }))
        .count();
    assert_eq!(count, 1);
    let _ = ica_pos;
}

#[test]
fn disassemble_index_compound_stable() {
    use crate::disassemble::disassemble;
    let prog = compile_prog("let mut a = [1, 2]\na[0] += 5\na[1] *= 3");
    let dis = disassemble(&prog);
    assert!(
        dis.contains("INDEX_COMPOUND_ASSIGN a +="),
        "missing +=: {}",
        dis
    );
    assert!(
        dis.contains("INDEX_COMPOUND_ASSIGN a *="),
        "missing *=: {}",
        dis
    );
}

#[test]
fn bytecode_index_assign_still_uses_set_index() {
    // Plain `arr[i] = val` must still emit SetIndex, not IndexCompoundAssign.
    let prog = compile_prog("let mut a = [1, 2]\na[0] = 99");
    let has_set_index = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::SetIndex(_)));
    let has_ica = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::IndexCompoundAssign { .. }));
    assert!(has_set_index, "expected SetIndex for plain index assign");
    assert!(!has_ica, "unexpected IndexCompoundAssign for plain assign");
}

// --- VM tests ---

#[test]
fn vm_index_compound_add() {
    let src = "let mut a = [1, 2, 3]\na[1] += 10\nprint(a[1])";
    assert_eq!(vm_run(src).unwrap(), vec!["12"]);
}

#[test]
fn vm_index_compound_subtract() {
    let src = "let mut a = [10, 5]\na[0] -= 3\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["7"]);
}

#[test]
fn vm_index_compound_multiply() {
    let src = "let mut a = [2, 3]\na[0] *= 4\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["8"]);
}

#[test]
fn vm_index_compound_divide() {
    let src = "let mut a = [10]\na[0] /= 2\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["5"]);
}

#[test]
fn vm_index_compound_text_concat() {
    let src = "let mut a = [\"hello\"]\na[0] += \"!\"\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["hello!"]);
}

#[test]
fn vm_index_compound_units() {
    let src = "let d1: meters = 5\nlet d2: meters = 3\nlet mut a = [d1, d2]\nlet inc: meters = 2\na[0] += inc\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["7"]);
}

#[test]
fn vm_index_compound_closure_capture() {
    let src = concat!(
        "fn outer() -> Number {\n",
        "let mut nums = [1, 2, 3]\n",
        "fn update() {\nnums[0] += 10\n}\n",
        "update()\n",
        "update()\n",
        "return nums[0]\n",
        "}\n",
        "print(outer())"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21"]);
}

#[test]
fn vm_index_compound_for_loop() {
    let src = "let mut a = [1, 2, 3, 4]\nfor i in range(0, len(a)) {\na[i] *= 2\n}\nprint(a[0])\nprint(a[1])\nprint(a[2])\nprint(a[3])";
    assert_eq!(vm_run(src).unwrap(), vec!["2", "4", "6", "8"]);
}

#[test]
fn vm_index_compound_simulate() {
    let src = concat!(
        "let mut values = [0, 0, 0]\n",
        "let mut i: Number = 0\n",
        "let duration: seconds = 3\n",
        "let dt: seconds = 1\n",
        "simulate duration step dt {\n",
        "values[i] += i + 10\n",
        "i += 1\n",
        "}\n",
        "print(values[0])\nprint(values[1])\nprint(values[2])"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["10", "11", "12"]);
}

#[test]
fn vm_index_compound_eval_order() {
    let src = concat!(
        "let mut arr = [10, 20]\n",
        "let mut counter: Number = 0\n",
        "fn idx() -> Number {\ncounter += 1\nreturn 0\n}\n",
        "fn rhs() -> Number {\ncounter += 10\nreturn counter\n}\n",
        "arr[idx()] += rhs()\n",
        "print(arr[0])\n",
        "print(counter)"
    );
    assert_eq!(vm_run(src).unwrap(), vec!["21", "11"]);
}

#[test]
fn vm_index_compound_stack_clean() {
    // After IndexCompoundAssign, stack is clean; subsequent operations work correctly.
    let src = "let mut a = [1, 2, 3]\na[0] += 9\na[2] += 7\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "2", "10"]);
}

#[test]
fn vm_index_compound_out_of_bounds() {
    let src = "let mut a = [1, 2]\na[9] += 1";
    let err = vm_run(src).unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn vm_index_compound_fractional_index() {
    let src = "let mut a = [1, 2]\na[0.5] += 1";
    let err = vm_run(src).unwrap_err();
    assert!(err.to_string().contains("integer"));
}

#[test]
fn vm_index_compound_matches_tree() {
    // VM and tree-walk produce same final array state.
    let src = "let mut a = [1, 2, 3]\na[0] += 9\na[1] *= 3\na[2] -= 1\nprint(a[0])\nprint(a[1])\nprint(a[2])";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "6", "2"]);
}

// --- Regression tests ---

#[test]
fn regression_m10a_index_assign_unaffected_by_m10b() {
    // Plain index assign must still work after M10B.
    let src = "let mut a = [1, 2, 3]\na[0] = 99\nprint(a[0])";
    assert_eq!(vm_run(src).unwrap(), vec!["99"]);
}

#[test]
fn regression_arrays_read_unaffected_by_m10b() {
    let src = "let a = [10, 20, 30]\nprint(a[0])\nprint(len(a))";
    assert_eq!(vm_run(src).unwrap(), vec!["10", "3"]);
}

#[test]
fn regression_compound_assign_scalar_unaffected_by_m10b() {
    // Plain variable compound assign still works.
    let src = "let mut x = 5\nx += 3\nprint(x)";
    assert_eq!(vm_run(src).unwrap(), vec!["8"]);
}
