use crate::ast::{
    BinaryOp, CompoundAssignOp, Expr, Param, StateTransition, StateVariant, Stmt, TypeAnnotation,
    UnaryOp,
};
use crate::error::ParseError;
use crate::token::{Span, Token, TokenKind};

/// Recursive-descent parser.
///
/// Grammar (Milestone 7A):
///   program         → stmt* EOF
///   stmt            → state_decl | transition_stmt | simulate_stmt | while_stmt | break_stmt | continue_stmt
///                   | fn_decl | return_stmt | let_stmt | assign_stmt | print_stmt | if_stmt | block | expr_stmt
///   simulate_stmt   → "simulate" expr "step" expr "{" stmt* "}"
///   while_stmt      → "while" expr "{" stmt* "}"
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
///   type_ann        → "Number" | "Text" | "Bool" | "Nil" | UNIT_NAME | IDENT
///   expr_stmt       → expr
///   expr            → equality
///   equality        → comparison (("==" | "!=") comparison)*
///   comparison      → term (("<" | "<=" | ">" | ">=") term)*
///   term            → factor (("+" | "-") factor)*
///   factor          → unary (("*" | "/") unary)*
///   unary           → ("-" | "!") unary | call
///   call            → primary ("(" args ")")*
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
        if matches!(self.current_kind(), TokenKind::State) {
            self.parse_state_decl()
        } else if matches!(self.current_kind(), TokenKind::Transition) {
            self.parse_transition_stmt()
        } else if matches!(self.current_kind(), TokenKind::Simulate) {
            self.parse_simulate_stmt()
        } else if matches!(self.current_kind(), TokenKind::While) {
            self.parse_while()
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
        } else if matches!(self.current_kind(), TokenKind::Ident(_))
            && matches!(self.peek_kind(), TokenKind::Eq)
        {
            self.parse_assign()
        } else if matches!(self.current_kind(), TokenKind::Ident(_))
            && matches!(
                self.peek_kind(),
                TokenKind::PlusEqual
                    | TokenKind::MinusEqual
                    | TokenKind::StarEqual
                    | TokenKind::SlashEqual
            )
        {
            self.parse_compound_assign()
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

        Ok(Param { name, ty, span })
    }

    /// Parse a type annotation: Number | Text | Bool | Nil | <unit name> | <state machine name>
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
            Err(self.error("expected type annotation (Number, Text, Bool, Nil, a known unit, or a state machine name)"))
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

    fn parse_compound_assign(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => unreachable!(),
        };
        self.advance(); // consume identifier
        let op = match self.current_kind() {
            TokenKind::PlusEqual => CompoundAssignOp::Add,
            TokenKind::MinusEqual => CompoundAssignOp::Subtract,
            TokenKind::StarEqual => CompoundAssignOp::Multiply,
            TokenKind::SlashEqual => CompoundAssignOp::Divide,
            _ => unreachable!(),
        };
        self.advance(); // consume compound operator
        let value = self.parse_expr()?;
        Ok(Stmt::CompoundAssign {
            name,
            op,
            value,
            span,
        })
    }

    fn parse_assign(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => unreachable!(),
        };
        self.advance(); // consume identifier
        self.advance(); // consume `=`
        let value = self.parse_expr()?;
        Ok(Stmt::Assign { name, value, span })
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
                // Check for state variant access: StateName.variant
                if matches!(self.current_kind(), TokenKind::Dot) {
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
