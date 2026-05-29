use crate::ast::{
    AssignTarget, BinaryOp, CompoundAssignOp, Expr, Param, StateTransition, StateVariant, Stmt,
    TypeAnnotation, UnaryOp,
};
// Note: MethodCall and ImplBlock are used via Expr:: and Stmt:: below.
use crate::error::ParseError;
use crate::token::{Span, Token, TokenKind};

/// Recursive-descent parser.
///
/// Grammar (Milestone 10D):
///   program         → stmt* EOF
///   stmt            → state_decl | transition_stmt | simulate_stmt | while_stmt | for_stmt | break_stmt | continue_stmt
///                   | fn_decl | return_stmt | let_stmt | assign_stmt | print_stmt | if_stmt | block | expr_stmt
///   simulate_stmt   → "simulate" expr "step" expr "{" stmt* "}"
///   while_stmt      → "while" expr "{" stmt* "}"
///   for_stmt        → "for" IDENT "in" "range" "(" expr "," expr ")" "{" stmt* "}"
///   state_decl      → "state" IDENT "{" (variant_decl | transition_decl)* "}"
///   variant_decl    → IDENT
///   transition_decl → "transition" IDENT "->" IDENT
///   transition_stmt → "transition" IDENT "->" IDENT
///   fn_decl         → "fn" IDENT "(" typed_params ")" ("->" type_ann)? fn_body
///   typed_params    → (IDENT ":" type_ann ("," IDENT ":" type_ann)*)?
///   return_stmt     → "return" expr?
///   let_stmt        → "let" "mut"? IDENT (":" type_ann)? "=" expr
///   assign_stmt           → IDENT "=" expr      (lookahead: Ident followed by single "=")
///   compound_assign_stmt  → IDENT ("+=" | "-=" | "*=" | "/=") expr
///   print_stmt      → "print" "(" expr ")"
///   if_stmt         → "if" expr block ("else" block)?
///   block           → "{" stmt* "}"
///   fn_body         → "{" stmt* "}"
///   type_ann        → "Number" | "Text" | "Bool" | "Nil" | UNIT_NAME | "Array" "<" type_ann ">" | IDENT
///   expr_stmt       → expr
///   expr            → equality
///   equality        → comparison (("==" | "!=") comparison)*
///   comparison      → term (("<" | "<=" | ">" | ">=") term)*
///   term            → factor (("+" | "-") factor)*
///   factor          → unary (("*" | "/") unary)*
///   unary           → ("-" | "!") unary | call
///   call            → primary ( "(" args ")" | "[" expr "]" | "[" expr ".." expr "]" )*
///   primary         → NUMBER | STRING | "true" | "false" | IDENT ("." IDENT)? | "(" expr ")"
///   args            → (expr ("," expr)*)?
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    // --- statements ---

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        if matches!(self.current_kind(), TokenKind::Struct) {
            self.parse_struct_decl()
        } else if matches!(self.current_kind(), TokenKind::State) {
            self.parse_state_decl()
        } else if matches!(self.current_kind(), TokenKind::Transition) {
            self.parse_transition_stmt()
        } else if matches!(self.current_kind(), TokenKind::Simulate) {
            self.parse_simulate_stmt()
        } else if matches!(self.current_kind(), TokenKind::While) {
            self.parse_while()
        } else if matches!(self.current_kind(), TokenKind::For) {
            self.parse_for_stmt()
        } else if matches!(self.current_kind(), TokenKind::Break) {
            self.parse_break()
        } else if matches!(self.current_kind(), TokenKind::Continue) {
            self.parse_continue()
        } else if matches!(self.current_kind(), TokenKind::Fn) {
            self.parse_fn_decl()
        } else if matches!(self.current_kind(), TokenKind::Return) {
            self.parse_return()
        } else if matches!(self.current_kind(), TokenKind::Let) {
            self.parse_let()
        } else if matches!(self.current_kind(), TokenKind::Print) {
            self.parse_print()
        } else if matches!(self.current_kind(), TokenKind::If) {
            self.parse_if()
        } else if matches!(self.current_kind(), TokenKind::LBrace) {
            self.parse_block()
        } else if matches!(self.current_kind(), TokenKind::Impl) {
            self.parse_impl_block()
        } else if matches!(self.current_kind(), TokenKind::Ident(_)) {
            self.parse_target_assign_or_expr()
        } else {
            Ok(Stmt::Expr(self.parse_expr()?))
        }
    }

    fn parse_state_decl(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `state`

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected state machine name after 'state'")),
        };
        self.advance(); // consume name

        self.expect_kind(TokenKind::LBrace, "expected '{' after state machine name")?;

        let mut variants = Vec::new();
        let mut transitions = Vec::new();

        while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
            if matches!(self.current_kind(), TokenKind::Transition) {
                let t_span = self.current_span();
                self.advance(); // consume `transition`
                let from = match self.current_kind() {
                    TokenKind::Ident(n) => n.clone(),
                    _ => return Err(self.error("expected variant name after 'transition'")),
                };
                self.advance();
                self.expect_kind(TokenKind::Arrow, "expected '->' in transition declaration")?;
                let to = match self.current_kind() {
                    TokenKind::Ident(n) => n.clone(),
                    _ => return Err(self.error("expected variant name after '->'")),
                };
                self.advance();
                transitions.push(StateTransition {
                    from,
                    to,
                    span: t_span,
                });
            } else if matches!(self.current_kind(), TokenKind::Ident(_)) {
                let v_span = self.current_span();
                let v_name = match self.current_kind() {
                    TokenKind::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                self.advance();
                variants.push(StateVariant {
                    name: v_name,
                    span: v_span,
                });
            } else {
                return Err(self.error("expected variant name or 'transition' in state body"));
            }
        }

        self.expect_kind(TokenKind::RBrace, "expected '}' after state body")?;

        Ok(Stmt::StateDecl {
            name,
            variants,
            transitions,
            span,
        })
    }

    fn parse_struct_decl(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `struct`

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected struct name after 'struct'")),
        };
        self.advance(); // consume name

        self.expect_kind(TokenKind::LBrace, "expected '{' after struct name")?;

        let mut fields: Vec<(String, TypeAnnotation)> = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
            let field_name = match self.current_kind() {
                TokenKind::Ident(n) => n.clone(),
                _ => return Err(self.error("expected field name in struct declaration")),
            };
            self.advance(); // consume field name
            self.expect_kind(TokenKind::Colon, "expected ':' after field name in struct")?;
            let type_ann = self.parse_type_annotation()?;
            fields.push((field_name, type_ann));
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance(); // optional trailing comma
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected '}' after struct fields")?;

        Ok(Stmt::StructDecl { name, fields, span })
    }

    fn parse_impl_block(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `impl`

        let struct_name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected struct name after 'impl'")),
        };
        self.advance(); // consume struct name

        self.expect_kind(
            TokenKind::LBrace,
            "expected '{' after struct name in impl block",
        )?;

        let mut methods: Vec<Stmt> = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
            if !matches!(self.current_kind(), TokenKind::Fn) {
                return Err(self.error("impl block may only contain method declarations (fn ...)"));
            }
            methods.push(self.parse_method_decl(&struct_name)?);
        }
        if methods.is_empty() {
            return Err(self.error("impl block must contain at least one method"));
        }
        self.expect_kind(TokenKind::RBrace, "expected '}' after impl block")?;

        Ok(Stmt::ImplBlock {
            struct_name,
            methods,
            span,
        })
    }

    fn parse_method_decl(&mut self, struct_name: &str) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `fn`

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected method name after 'fn'")),
        };
        self.advance(); // consume method name

        self.expect_kind(TokenKind::LParen, "expected '(' after method name")?;
        let params = self.parse_method_params(struct_name)?;
        self.expect_kind(TokenKind::RParen, "expected ')' after method parameters")?;

        let return_type = if matches!(self.current_kind(), TokenKind::Arrow) {
            self.advance(); // consume `->`
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_fn_body()?;

        Ok(Stmt::FnDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    /// Parse method parameters: first param is `self` or `mut self`; rest use normal typed syntax.
    /// `self` gets a `Named(struct_name)` annotation so the typechecker can resolve it.
    fn parse_method_params(&mut self, struct_name: &str) -> Result<Vec<Param>, ParseError> {
        // Optional `mut` before `self` makes self a mutable local copy.
        let self_mutable = if matches!(self.current_kind(), TokenKind::Mut) {
            self.advance(); // consume `mut`
            true
        } else {
            false
        };

        // First param must be bare `self` (no type annotation).
        if !matches!(self.current_kind(), TokenKind::Ident(s) if s == "self") {
            return Err(self.error("method must have 'self' as first parameter"));
        }
        let self_span = self.current_span();
        self.advance(); // consume `self`

        let mut params = vec![Param {
            name: "self".to_string(),
            ty: TypeAnnotation::Named(struct_name.to_string()),
            span: self_span,
            mutable: self_mutable,
        }];

        // Additional typed params.
        while matches!(self.current_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            params.push(self.parse_typed_param()?);
        }

        Ok(params)
    }

    fn parse_transition_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `transition`

        let variable = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected variable name after 'transition'")),
        };
        self.advance();

        self.expect_kind(
            TokenKind::Arrow,
            "expected '->' after variable name in transition",
        )?;

        let target = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected target variant name after '->'")),
        };
        self.advance();

        Ok(Stmt::Transition {
            variable,
            target,
            span,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `while`

        if !self.can_start_expr() {
            return Err(self.error("expected condition expression after 'while'"));
        }
        let condition = self.parse_expr()?;
        let body = self.parse_fn_body()?;

        Ok(Stmt::While {
            condition,
            body,
            span,
        })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `for`

        // Loop variable identifier.
        let var_name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected loop variable name after 'for'")),
        };
        self.advance(); // consume identifier

        // Check for indexed for-each: `for i, item in ...`
        if matches!(self.current_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            let item_name = match self.current_kind() {
                TokenKind::Ident(n) => n.clone(),
                _ => {
                    return Err(
                        self.error("expected item variable name after ',' in 'for' statement")
                    )
                }
            };
            self.advance(); // consume item ident
            if !matches!(self.current_kind(), TokenKind::In) {
                return Err(self.error("expected 'in' after loop variables in 'for' statement"));
            }
            self.advance(); // consume `in`
            let iterable = self.parse_expr()?;
            let body = self.parse_fn_body()?;
            return Ok(Stmt::ForEachIndexed {
                index_name: var_name,
                var_name: item_name,
                iterable,
                body,
                span,
            });
        }

        // `in` keyword.
        if !matches!(self.current_kind(), TokenKind::In) {
            return Err(self.error("expected 'in' after loop variable in 'for' statement"));
        }
        self.advance(); // consume `in`

        // Dispatch: `range(` → ForRange; anything else → ForEach.
        let is_range = matches!(self.current_kind(), TokenKind::Ident(n) if n == "range")
            && matches!(self.peek_kind(), TokenKind::LParen);

        if is_range {
            self.advance(); // consume `range`
            self.expect_kind(TokenKind::LParen, "expected '(' after 'range'")?;
            let start = self.parse_expr()?;
            self.expect_kind(TokenKind::Comma, "expected ',' between range arguments")?;
            let end = self.parse_expr()?;
            // Reject three-argument range.
            if matches!(self.current_kind(), TokenKind::Comma) {
                return Err(self.error(
                    "range takes exactly 2 arguments (start, end); 3-argument range is not supported",
                ));
            }
            self.expect_kind(TokenKind::RParen, "expected ')' after range arguments")?;
            let body = self.parse_fn_body()?;
            Ok(Stmt::ForRange {
                var_name,
                start,
                end,
                body,
                span,
            })
        } else {
            let iterable = self.parse_expr()?;
            let body = self.parse_fn_body()?;
            Ok(Stmt::ForEach {
                var_name,
                iterable,
                body,
                span,
            })
        }
    }

    fn parse_break(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `break`
        Ok(Stmt::Break { span })
    }

    fn parse_continue(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `continue`
        Ok(Stmt::Continue { span })
    }

    fn parse_simulate_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `simulate`

        let duration = self.parse_expr()?;

        if !matches!(self.current_kind(), TokenKind::Step) {
            return Err(self.error("expected 'step' after duration expression in simulate"));
        }
        self.advance(); // consume `step`

        let step = self.parse_expr()?;

        let body = self.parse_fn_body()?;

        Ok(Stmt::Simulate {
            duration,
            step,
            body,
            span,
        })
    }

    fn parse_fn_decl(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `fn`

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected function name after 'fn'")),
        };
        self.advance(); // consume name

        self.expect_kind(TokenKind::LParen, "expected '(' after function name")?;
        let params = self.parse_typed_params()?;
        self.expect_kind(TokenKind::RParen, "expected ')' after parameters")?;

        // Optional return type: `-> TypeAnnotation`
        let return_type = if matches!(self.current_kind(), TokenKind::Arrow) {
            self.advance(); // consume `->`
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_fn_body()?;

        Ok(Stmt::FnDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    /// Parse `(IDENT ":" type_ann ("," IDENT ":" type_ann)*)?`
    fn parse_typed_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.current_kind(), TokenKind::RParen) {
            return Ok(params);
        }

        params.push(self.parse_typed_param()?);
        while matches!(self.current_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            params.push(self.parse_typed_param()?);
        }
        Ok(params)
    }

    fn parse_typed_param(&mut self) -> Result<Param, ParseError> {
        let span = self.current_span();
        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected parameter name")),
        };
        self.advance(); // consume name

        self.expect_kind(TokenKind::Colon, "expected ':' after parameter name (parameters require a type annotation, e.g. x: Number)")?;
        let ty = self.parse_type_annotation()?;

        Ok(Param {
            name,
            ty,
            span,
            mutable: false,
        })
    }

    /// Parse a type annotation: Number | Text | Bool | Nil | <unit name> | <state machine name> | Array<T>
    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        if matches!(self.current_kind(), TokenKind::Ident(_)) {
            let name = match self.current_kind() {
                TokenKind::Ident(s) => s.clone(),
                _ => unreachable!(),
            };
            self.advance();
            match name.as_str() {
                "Number" => Ok(TypeAnnotation::Number),
                "Text" => Ok(TypeAnnotation::Text),
                "Bool" => Ok(TypeAnnotation::Bool),
                "Nil" => Ok(TypeAnnotation::Nil),
                "Array" => {
                    if !matches!(self.current_kind(), TokenKind::Lt) {
                        return Err(self.error(
                            "expected '<' after 'Array' in type annotation, e.g. Array<Number>",
                        ));
                    }
                    self.advance(); // consume `<`
                    if matches!(self.current_kind(), TokenKind::Gt) {
                        return Err(
                            self.error("Array type requires an element type, e.g. Array<Number>")
                        );
                    }
                    let inner = self.parse_type_annotation()?;
                    if matches!(inner, TypeAnnotation::Array(_)) {
                        return Err(self.error(
                            "nested arrays are not supported; use Array<T> with a non-array element type",
                        ));
                    }
                    if !matches!(self.current_kind(), TokenKind::Gt) {
                        return Err(
                            self.error("expected '>' after element type in Array<T> annotation")
                        );
                    }
                    self.advance(); // consume `>`
                    Ok(TypeAnnotation::Array(Box::new(inner)))
                }
                "Map" => {
                    if !matches!(self.current_kind(), TokenKind::Lt) {
                        return Err(self.error(
                            "expected '<' after 'Map' in type annotation, e.g. Map<Text, Number>",
                        ));
                    }
                    self.advance(); // consume `<`
                    let key_ann = self.parse_type_annotation()?;
                    if !matches!(self.current_kind(), TokenKind::Comma) {
                        return Err(self
                            .error("expected ',' in Map type annotation, e.g. Map<Text, Number>"));
                    }
                    self.advance(); // consume `,`
                    if matches!(self.current_kind(), TokenKind::Gt) {
                        return Err(self.error("expected map value type after ',' in Map<Text, V>"));
                    }
                    let val_ann = self.parse_type_annotation()?;
                    if matches!(val_ann, TypeAnnotation::Map(..)) {
                        return Err(self.error("nested maps are not supported"));
                    }
                    if !matches!(self.current_kind(), TokenKind::Gt) {
                        return Err(
                            self.error("expected '>' after value type in Map<Text, V> annotation")
                        );
                    }
                    self.advance(); // consume `>`
                    Ok(TypeAnnotation::Map(Box::new(key_ann), Box::new(val_ann)))
                }
                other => {
                    if let Some(canonical) = resolve_unit(other) {
                        Ok(TypeAnnotation::NumberWithUnit(canonical.to_string()))
                    } else {
                        // Defer to the type checker — may be a state machine name.
                        Ok(TypeAnnotation::Named(other.to_string()))
                    }
                }
            }
        } else {
            Err(self.error("expected type annotation (Number, Text, Bool, Nil, a known unit, Array<T>, Map<Text, V>, or a state machine name)"))
        }
    }

    /// Parse `{ stmt* }` for a function body, returning inner statements directly.
    fn parse_fn_body(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect_kind(TokenKind::LBrace, "expected '{' before function body")?;
        let mut stmts = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_kind(TokenKind::RBrace, "expected '}' after function body")?;
        Ok(stmts)
    }

    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `return`

        let value = if self.can_start_expr() {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Stmt::Return { value, span })
    }

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume `let`

        // Optional `mut`
        let mutable = if matches!(self.current_kind(), TokenKind::Mut) {
            self.advance(); // consume `mut`
            true
        } else {
            false
        };

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected identifier after 'let'")),
        };
        let span = self.current_span();
        self.advance(); // consume identifier

        // Optional `: TypeAnnotation`
        let annotation = if matches!(self.current_kind(), TokenKind::Colon) {
            self.advance(); // consume `:`
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        self.expect_kind(TokenKind::Eq, "expected '=' after variable name")?;

        let value = self.parse_expr()?;

        Ok(Stmt::Let {
            name,
            mutable,
            annotation,
            value,
            span,
        })
    }

    /// Unified assignment/expression parser for all identifier-led statements.
    ///
    /// Attempts to parse an assignment target path (chained `.field` and `[index]` steps),
    /// then checks for `=` or `op=`. If found, produces the appropriate assignment statement.
    /// Otherwise backtracks and parses the entire thing as an expression statement.
    ///
    /// Produces existing statement types for backward-compatible shapes:
    ///   `u = v`        → Stmt::Assign
    ///   `u op= v`      → Stmt::CompoundAssign
    ///   `arr[i] = v`   → Stmt::IndexAssign   (root-only index)
    ///   `arr[i] op= v` → Stmt::IndexCompoundAssign
    ///   `u.f = v`      → Stmt::TargetAssign  (path ending in field)
    ///   `u.f op= v`    → Stmt::TargetCompoundAssign
    ///   `arr[i].f = v` → Stmt::TargetAssign
    ///   `u.f.g = v`    → Stmt::TargetAssign
    fn parse_target_assign_or_expr(&mut self) -> Result<Stmt, ParseError> {
        let saved_pos = self.pos;
        let span = self.current_span();

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Ok(Stmt::Expr(self.parse_expr()?)),
        };
        self.advance(); // consume name

        let mut target = AssignTarget::Var(name);

        // Extend target with chained .field and [index] steps.
        // Stop on function call `(`, DotDot inside `[]`, or any non-target token.
        loop {
            if matches!(self.current_kind(), TokenKind::Dot) {
                self.advance(); // consume .
                let field = match self.current_kind() {
                    TokenKind::Ident(f) => f.clone(),
                    _ => {
                        // Unexpected token after . — backtrack and parse as expression.
                        self.pos = saved_pos;
                        return Ok(Stmt::Expr(self.parse_expr()?));
                    }
                };
                self.advance(); // consume field name
                target = AssignTarget::Field(Box::new(target), field);
            } else if matches!(self.current_kind(), TokenKind::LBracket) {
                self.advance(); // consume [
                                // Immediately DotDot → open-start slice, not a valid index step.
                if matches!(self.current_kind(), TokenKind::DotDot) {
                    self.pos = saved_pos;
                    return Ok(Stmt::Expr(self.parse_expr()?));
                }
                // Parse index expression.
                let index = self.parse_expr()?;
                // DotDot after index → slice expression, not assignment target.
                if matches!(self.current_kind(), TokenKind::DotDot) {
                    self.pos = saved_pos;
                    return Ok(Stmt::Expr(self.parse_expr()?));
                }
                if !matches!(self.current_kind(), TokenKind::RBracket) {
                    return Err(self.error("expected ']' after index expression"));
                }
                self.advance(); // consume ]
                target = AssignTarget::Index(Box::new(target), index);
            } else {
                break;
            }
        }

        // Check for assignment operator.
        let is_compound = matches!(
            self.current_kind(),
            TokenKind::PlusEqual
                | TokenKind::MinusEqual
                | TokenKind::StarEqual
                | TokenKind::SlashEqual
        );

        if matches!(self.current_kind(), TokenKind::Eq) {
            self.advance(); // consume =
            let value = self.parse_expr()?;
            return Ok(match target {
                AssignTarget::Var(n) => Stmt::Assign {
                    name: n,
                    value,
                    span,
                },
                AssignTarget::Index(inner, idx) if matches!(*inner, AssignTarget::Var(_)) => {
                    let n = match *inner {
                        AssignTarget::Var(n) => n,
                        _ => unreachable!(),
                    };
                    Stmt::IndexAssign {
                        name: n,
                        index: idx,
                        value,
                        span,
                    }
                }
                other => Stmt::TargetAssign {
                    target: other,
                    value,
                    span,
                },
            });
        }

        if is_compound {
            let op = match self.current_kind() {
                TokenKind::PlusEqual => CompoundAssignOp::Add,
                TokenKind::MinusEqual => CompoundAssignOp::Subtract,
                TokenKind::StarEqual => CompoundAssignOp::Multiply,
                TokenKind::SlashEqual => CompoundAssignOp::Divide,
                _ => unreachable!(),
            };
            self.advance(); // consume op=
            let value = self.parse_expr()?;
            return Ok(match target {
                AssignTarget::Var(n) => Stmt::CompoundAssign {
                    name: n,
                    op,
                    value,
                    span,
                },
                AssignTarget::Index(inner, idx) if matches!(*inner, AssignTarget::Var(_)) => {
                    let n = match *inner {
                        AssignTarget::Var(n) => n,
                        _ => unreachable!(),
                    };
                    Stmt::IndexCompoundAssign {
                        name: n,
                        index: idx,
                        op,
                        value,
                        span,
                    }
                }
                other => Stmt::TargetCompoundAssign {
                    target: other,
                    op,
                    value,
                    span,
                },
            });
        }

        // Not an assignment — backtrack and parse as expression statement.
        self.pos = saved_pos;
        Ok(Stmt::Expr(self.parse_expr()?))
    }

    fn parse_print(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume `print`
        self.expect_kind(TokenKind::LParen, "expected '(' after 'print'")?;
        let value = self.parse_expr()?;
        self.expect_kind(TokenKind::RParen, "expected ')' after print argument")?;
        Ok(Stmt::Print { value })
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume `if`
        let cond = self.parse_expr()?;
        let then_block = Box::new(self.parse_block()?);

        let else_block = if matches!(self.current_kind(), TokenKind::Else) {
            self.advance(); // consume `else`
            Some(Box::new(self.parse_block()?))
        } else {
            None
        };

        Ok(Stmt::If {
            cond,
            then_block,
            else_block,
        })
    }

    fn parse_block(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(TokenKind::LBrace, "expected '{'")?;
        let mut stmts = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_kind(TokenKind::RBrace, "expected '}'")?;
        Ok(Stmt::Block(stmts))
    }

    // --- expressions (precedence climbing) ---

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::BangEq => BinaryOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::LtEq => BinaryOp::LtEq,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::GtEq => BinaryOp::GtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.current_kind(), TokenKind::Minus) {
            self.advance();
            let operand = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            })
        } else if matches!(self.current_kind(), TokenKind::Bang) {
            self.advance();
            let operand = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            })
        } else {
            self.parse_call()
        }
    }

    fn parse_call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if matches!(self.current_kind(), TokenKind::LParen) {
                let span = self.current_span();
                self.advance(); // consume `(`
                let args = self.parse_args()?;
                self.expect_kind(TokenKind::RParen, "expected ')' after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else if matches!(self.current_kind(), TokenKind::Dot) {
                // Chained `.ident` or `.method(args)` on any expression.
                // Plain `ident.ident` on a simple variable is handled in parse_primary.
                let span = self.current_span();
                self.advance(); // consume `.`
                let ident = match self.current_kind() {
                    TokenKind::Ident(n) => n.clone(),
                    _ => return Err(self.error("expected field or method name after '.'")),
                };
                self.advance(); // consume field/method name

                if matches!(self.current_kind(), TokenKind::LParen) {
                    // Method call: expr.method(args)
                    self.advance(); // consume `(`
                    let args = self.parse_args()?;
                    self.expect_kind(TokenKind::RParen, "expected ')' after method arguments")?;
                    expr = Expr::MethodCall {
                        object: Box::new(expr),
                        method: ident,
                        args,
                        span,
                    };
                } else {
                    // Field access: expr.field
                    expr = Expr::FieldAccess {
                        object: Box::new(expr),
                        field: ident,
                        span,
                    };
                }
            } else if matches!(self.current_kind(), TokenKind::LBrace)
                && matches!(&expr, Expr::Variable { .. })
                && matches!(self.peek_kind(), TokenKind::Ident(_))
                && matches!(self.peek_kind_2(), TokenKind::Colon)
            {
                // Struct literal: TypeName { field: value, ... }
                // Disambiguation: { followed by Ident : is unambiguously a struct literal.
                let struct_name = match &expr {
                    Expr::Variable { name, .. } => name.clone(),
                    _ => unreachable!(),
                };
                let span = self.current_span();
                self.advance(); // consume `{`
                let mut fields: Vec<(String, Expr)> = Vec::new();
                while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
                    let field_name = match self.current_kind() {
                        TokenKind::Ident(n) => n.clone(),
                        _ => return Err(self.error("expected field name in struct literal")),
                    };
                    self.advance(); // consume field name
                    self.expect_kind(
                        TokenKind::Colon,
                        "expected ':' after field name in struct literal",
                    )?;
                    let val = self.parse_expr()?;
                    fields.push((field_name, val));
                    if matches!(self.current_kind(), TokenKind::Comma) {
                        self.advance(); // optional trailing comma
                    } else {
                        break;
                    }
                }
                self.expect_kind(
                    TokenKind::RBrace,
                    "expected '}' after struct literal fields",
                )?;
                expr = Expr::StructLiteral {
                    name: struct_name,
                    fields,
                    span,
                };
            } else if matches!(self.current_kind(), TokenKind::LBracket) {
                let span = self.current_span();
                self.advance(); // consume `[`
                if matches!(self.current_kind(), TokenKind::RBracket) {
                    return Err(self.error("index expression requires an index value"));
                }
                if matches!(self.current_kind(), TokenKind::DotDot) {
                    return Err(self.error(
                        "open-ended slices are not supported; provide a start expression before '..'",
                    ));
                }
                let first = self.parse_expr()?;
                if matches!(self.current_kind(), TokenKind::DotDot) {
                    self.advance(); // consume `..`
                    if matches!(self.current_kind(), TokenKind::RBracket) {
                        return Err(self.error(
                            "open-ended slices are not supported; provide an end expression after '..'",
                        ));
                    }
                    let end = self.parse_expr()?;
                    self.expect_kind(TokenKind::RBracket, "expected ']' after slice end")?;
                    expr = Expr::Slice {
                        array: Box::new(expr),
                        start: Box::new(first),
                        end: Box::new(end),
                        span,
                    };
                } else {
                    self.expect_kind(TokenKind::RBracket, "expected ']' after index")?;
                    expr = Expr::Index {
                        array: Box::new(expr),
                        index: Box::new(first),
                        span,
                    };
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if matches!(self.current_kind(), TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while matches!(self.current_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        let tok = self.current().clone();
        match tok.kind {
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Ident(name) => {
                self.advance();
                // `name.ident(` → method call on simple variable.
                // `name.ident`  → state variant (existing; typechecker resolves Struct vs State).
                if matches!(self.current_kind(), TokenKind::Dot)
                    && matches!(self.peek_kind(), TokenKind::Ident(_))
                    && matches!(self.peek_kind_2(), TokenKind::LParen)
                {
                    let method_span = self.current_span();
                    self.advance(); // consume `.`
                    let method = match self.current_kind() {
                        TokenKind::Ident(m) => m.clone(),
                        _ => unreachable!(),
                    };
                    self.advance(); // consume method name
                    self.advance(); // consume `(`
                    let args = self.parse_args()?;
                    self.expect_kind(TokenKind::RParen, "expected ')' after method arguments")?;
                    Ok(Expr::MethodCall {
                        object: Box::new(Expr::Variable { name, span }),
                        method,
                        args,
                        span: method_span,
                    })
                } else if matches!(self.current_kind(), TokenKind::Dot) {
                    self.advance(); // consume `.`
                    let variant_name = match self.current_kind() {
                        TokenKind::Ident(v) => v.clone(),
                        _ => return Err(self.error("expected variant name after '.'")),
                    };
                    self.advance();
                    Ok(Expr::StateVariant {
                        state_name: name,
                        variant_name,
                        span,
                    })
                } else {
                    Ok(Expr::Variable { name, span })
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::RParen, "expected ')' after expression")?;
                Ok(Expr::Grouping(Box::new(expr)))
            }
            TokenKind::LBracket => {
                self.advance(); // consume `[`
                if matches!(self.current_kind(), TokenKind::RBracket) {
                    self.advance(); // consume `]`
                    return Ok(Expr::ArrayLiteral {
                        elements: vec![],
                        span,
                    });
                }
                let mut elements = vec![self.parse_expr()?];
                while matches!(self.current_kind(), TokenKind::Comma) {
                    self.advance(); // consume `,`
                    if matches!(self.current_kind(), TokenKind::RBracket) {
                        break; // allow trailing comma
                    }
                    elements.push(self.parse_expr()?);
                }
                self.expect_kind(TokenKind::RBracket, "expected ']' after array elements")?;
                Ok(Expr::ArrayLiteral { elements, span })
            }
            TokenKind::LBrace => {
                self.advance(); // consume `{`
                let mut entries: Vec<(Expr, Expr)> = Vec::new();
                while !matches!(self.current_kind(), TokenKind::RBrace) && !self.is_at_end() {
                    let key = self.parse_expr()?;
                    self.expect_kind(TokenKind::Colon, "expected ':' after map key")?;
                    let val = self.parse_expr()?;
                    entries.push((key, val));
                    if matches!(self.current_kind(), TokenKind::Comma) {
                        self.advance(); // consume `,`
                    } else {
                        break;
                    }
                }
                self.expect_kind(TokenKind::RBrace, "expected '}' after map entries")?;
                Ok(Expr::MapLiteral { entries, span })
            }
            _ => Err(self.error("expected expression")),
        }
    }

    // --- helpers ---

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn current_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn current_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn peek_kind(&self) -> &TokenKind {
        let next = self.pos + 1;
        if next < self.tokens.len() {
            &self.tokens[next].kind
        } else {
            &self.tokens[self.pos].kind
        }
    }

    fn peek_kind_2(&self) -> &TokenKind {
        let next2 = self.pos + 2;
        if next2 < self.tokens.len() {
            &self.tokens[next2].kind
        } else {
            &self.tokens[self.pos].kind
        }
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn can_start_expr(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::Number(_)
                | TokenKind::String(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Ident(_)
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Minus
                | TokenKind::Bang
        )
    }

    fn expect_kind(&mut self, kind: TokenKind, msg: &str) -> Result<(), ParseError> {
        if std::mem::discriminant(self.current_kind()) == std::mem::discriminant(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(msg))
        }
    }

    fn error(&self, msg: &str) -> ParseError {
        let span = &self.tokens[self.pos].span;
        ParseError {
            msg: msg.to_string(),
            line: span.line,
            col: span.col,
        }
    }
}

/// Returns the canonical unit name if `name` is a known unit name or alias, else None.
///
/// Supported units and their aliases:
///   meters    (m)
///   seconds   (s)
///   kilograms (kg)
///   amperes   (A, amps)
///   kelvin    (K)
///   moles     (mol)
///   candela   (cd)
///   radians   (rad)
///   degrees   (deg)
///   volts     (V)
///   watts     (W)
///   joules    (J)
///   newtons   (N)
pub fn resolve_unit(name: &str) -> Option<&'static str> {
    match name {
        "m" | "meters" => Some("meters"),
        "s" | "seconds" => Some("seconds"),
        "ms" | "milliseconds" => Some("milliseconds"),
        "min" | "minutes" => Some("minutes"),
        "h" | "hours" => Some("hours"),
        "kg" | "kilograms" => Some("kilograms"),
        "A" | "amps" | "amperes" => Some("amperes"),
        "K" | "kelvin" => Some("kelvin"),
        "mol" | "moles" => Some("moles"),
        "cd" | "candela" => Some("candela"),
        "rad" | "radians" => Some("radians"),
        "deg" | "degrees" => Some("degrees"),
        "V" | "volts" => Some("volts"),
        "W" | "watts" => Some("watts"),
        "J" | "joules" => Some("joules"),
        "N" | "newtons" => Some("newtons"),
        _ => None,
    }
}
