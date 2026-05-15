use crate::{
    error::KiminError, interpreter::Interpreter, lexer::Lexer, parser::Parser, token::TokenKind,
    typechecker::TypeChecker, value::Value,
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
    assert!(matches!(
        check("let x: Banana = 5"),
        Err(KiminError::Parse(_))
    ));
    assert!(matches!(
        check("fn f(x: Meters) { }"),
        Err(KiminError::Parse(_))
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
fn type_unit_compound_mul_error() {
    match run("let d: meters = 10\nlet t: seconds = 2\nlet bad = d * t") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("compound")
                    || (e.msg.contains("meters") && e.msg.contains("seconds")),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
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
fn type_unit_different_unit_div_error() {
    match run("let d: meters = 10\nlet t: seconds = 2\nlet bad = d / t") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("compound")
                    || (e.msg.contains("meters") && e.msg.contains("seconds")),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
}

#[test]
fn type_unit_number_div_unit_error() {
    match run("let d: meters = 10\nlet bad = 5 / d") {
        Err(KiminError::Type(e)) => {
            assert!(
                e.msg.contains("reciprocal") || e.msg.contains("meters"),
                "unexpected error: {}",
                e.msg
            );
        }
        Ok(_) => panic!("expected TypeError"),
        Err(e) => panic!("expected TypeError, got: {}", e),
    }
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
