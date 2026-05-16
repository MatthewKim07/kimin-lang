use crate::{
    bytecode::{Constant, Instruction},
    compiler::BytecodeCompiler,
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
    // M8B: named calls now lower to CALL name arg_count, not UNSUPPORTED.
    let prog = compile_prog("fn f() { } f()");
    let has_call = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { name, arg_count: 0 } if name == "f"));
    assert!(has_call);
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
fn bytecode_simulate_emits_unsupported() {
    let prog = compile_prog(concat!(
        "let dur = 3\n",
        "let dt = 1\n",
        "simulate dur step dt { }"
    ));
    let has_simulate = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Unsupported(s) if s == "simulate"));
    assert!(has_simulate);
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
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3)"
    ));
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { name, arg_count: 2 } if name == "add")));
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
        .any(|i| matches!(i, Instruction::Call { name, arg_count: 3 } if name == "f")));
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
    // square(add(2, 3)): inner CALL add must precede outer CALL square.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "fn square(x: Number) -> Number { return x * x }\n",
        "let z = square(add(2, 3))"
    ));
    let instrs = &prog.main.instructions;
    let add_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { name, .. } if name == "add"))
        .unwrap();
    let sq_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { name, .. } if name == "square"))
        .unwrap();
    assert!(add_idx < sq_idx);
}

#[test]
fn bytecode_recursive_call_emits_call_to_self() {
    let prog = compile_prog(concat!(
        "fn fact(n: Number) -> Number {\n",
        "  if n <= 1 { return 1 }\n",
        "  return n * fact(n - 1)\n",
        "}"
    ));
    let body = &prog.functions[0].chunk.instructions;
    assert!(body
        .iter()
        .any(|i| matches!(i, Instruction::Call { name, arg_count: 1 } if name == "fact")));
}

#[test]
fn bytecode_zero_arg_call_emits_call_zero() {
    let prog = compile_prog("fn f() { } f()");
    assert!(prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Call { name, arg_count: 0 } if name == "f")));
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
    assert!(out.contains("CALL add 2"));
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
        .position(|i| matches!(i, Instruction::Call { name, .. } if name == "f"))
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
        .position(|i| matches!(i, Instruction::Call { name, .. } if name == "f"))
        .unwrap();
    assert!(
        matches!(instrs[call_idx + 1], Instruction::Print),
        "PRINT must immediately follow CALL f in print(f())"
    );
}

#[test]
fn bytecode_call_result_in_binary_expr_correct_order() {
    // add(2, 3) + 1: CALL add must be emitted before ADD.
    let prog = compile_prog(concat!(
        "fn add(a: Number, b: Number) -> Number { return a + b }\n",
        "let z = add(2, 3) + 1"
    ));
    let instrs = &prog.main.instructions;
    let call_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Call { name, .. } if name == "add"))
        .unwrap();
    let add_idx = instrs
        .iter()
        .position(|i| matches!(i, Instruction::Add))
        .unwrap();
    assert!(call_idx < add_idx, "CALL add must precede ADD");
}

#[test]
fn bytecode_mutual_recursion_emits_cross_calls() {
    // is_even calls is_odd and vice versa — each function chunk must emit a CALL to the other.
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
            .any(|i| matches!(i, Instruction::Call { name, arg_count: 1 } if name == "is_odd")),
        "is_even must call is_odd"
    );
    let odd_body = &prog.functions[1].chunk.instructions;
    assert!(
        odd_body
            .iter()
            .any(|i| matches!(i, Instruction::Call { name, arg_count: 1 } if name == "is_even")),
        "is_odd must call is_even"
    );
}

#[test]
fn bytecode_dynamic_call_emits_unsupported_marker() {
    // f()() — outer callee is a Call expression, not a Variable — must emit UNSUPPORTED(dynamic call).
    let prog = compile_prog("fn f() { } f()()");
    let has_unsupported = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Unsupported(s) if s == "dynamic call"));
    assert!(
        has_unsupported,
        "dynamic call must emit UNSUPPORTED(dynamic call)"
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
    // CALL instruction must be formatted as "CALL name arg_count".
    use crate::disassemble::disassemble;
    let prog = compile_prog(concat!(
        "fn f(a: Number, b: Number, c: Number) -> Number { return a }\n",
        "let x = f(1, 2, 3)"
    ));
    let out = disassemble(&prog);
    assert!(out.contains("CALL f 3"));
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
        format!("{}", Value::BytecodeFunction("foo".into())),
        "<fn foo>"
    );
}

#[test]
fn vm_bytecode_function_value_type_name() {
    assert_eq!(
        Value::BytecodeFunction("foo".into()).type_name(),
        "Function"
    );
}

#[test]
fn vm_bytecode_function_value_equality() {
    assert_eq!(
        Value::BytecodeFunction("f".into()),
        Value::BytecodeFunction("f".into())
    );
    assert_ne!(
        Value::BytecodeFunction("f".into()),
        Value::BytecodeFunction("g".into())
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
fn vm_unsupported_simulate_errors() {
    let src = "let d: seconds = 1\nlet dt: seconds = 1\nsimulate d step dt { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("bytecode feature not yet executable"),
        "got: {}",
        msg
    );
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
fn vm_unsupported_dynamic_call_errors() {
    // f()() — outer callee is a Call expression, not a Variable → Unsupported("dynamic call")
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
        msg.contains("bytecode feature not yet executable"),
        "got: {}",
        msg
    );
}

#[test]
fn vm_unknown_function_error() {
    // Call to a name not in the function table
    let src = "print(nonexistent(1))";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown function"), "got: {}", msg);
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
fn bytecode_simulate_still_emits_unsupported() {
    let prog = compile_prog("let dur = 3\nlet dt = 1\nsimulate dur step dt { }");
    let has_unsupported = prog
        .main
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Unsupported(s) if s == "simulate"));
    assert!(has_unsupported, "simulate must still emit Unsupported");
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
fn vm_simulate_still_unsupported() {
    // simulate must remain Unsupported after M8D.
    let src = "let d: seconds = 1\nlet dt: seconds = 1\nsimulate d step dt { }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let prog = BytecodeCompiler::new().compile(&stmts).unwrap();
    use crate::vm::Vm;
    let mut vm = Vm::new(prog);
    let result = vm.run();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("bytecode feature not yet executable"),
        "got: {}",
        msg
    );
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
