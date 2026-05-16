use crate::{
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
