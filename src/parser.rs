use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::error::ParseError;
use crate::token::{Span, Token, TokenKind};

/// Recursive-descent parser.
///
/// Grammar (informal):
///   program    → stmt* EOF
///   stmt       → fn_decl | return_stmt | let_stmt | print_stmt | if_stmt | block | expr_stmt
///   fn_decl    → "fn" IDENT "(" params ")" fn_body
///   return_stmt → "return" expr?
///   let_stmt   → "let" IDENT "=" expr
///   print_stmt → "print" "(" expr ")"
///   if_stmt    → "if" expr block ("else" block)?
///   block      → "{" stmt* "}"
///   fn_body    → "{" stmt* "}"        (same syntax, different AST treatment)
///   params     → (IDENT ("," IDENT)*)?
///   expr_stmt  → expr
///   expr       → equality
///   equality   → comparison (("==" | "!=") comparison)*
///   comparison → term (("<" | "<=" | ">" | ">=") term)*
///   term       → factor (("+" | "-") factor)*
///   factor     → unary (("*" | "/") unary)*
///   unary      → ("-" | "!") unary | call
///   call       → primary ("(" args ")")*
///   primary    → NUMBER | STRING | "true" | "false" | IDENT | "(" expr ")"
///   args       → (expr ("," expr)*)?
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
        if matches!(self.current_kind(), TokenKind::Fn) {
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
        } else {
            Ok(Stmt::Expr(self.parse_expr()?))
        }
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
        let params = self.parse_params()?;
        self.expect_kind(TokenKind::RParen, "expected ')' after parameters")?;

        let body = self.parse_fn_body()?;

        Ok(Stmt::FnDecl {
            name,
            params,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.current_kind(), TokenKind::RParen) {
            return Ok(params);
        }
        let first = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected parameter name")),
        };
        params.push(first);
        self.advance();

        while matches!(self.current_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            let param = match self.current_kind() {
                TokenKind::Ident(n) => n.clone(),
                _ => return Err(self.error("expected parameter name after ','")),
            };
            params.push(param);
            self.advance();
        }
        Ok(params)
    }

    /// Parse `{ stmt* }` for a function body, returning the inner statements directly.
    /// The caller (call_function) manages the scope, so we do not wrap in Stmt::Block.
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

        let name = match self.current_kind() {
            TokenKind::Ident(n) => n.clone(),
            _ => return Err(self.error("expected identifier after 'let'")),
        };
        let span = self.current_span();
        self.advance(); // consume identifier

        self.expect_kind(TokenKind::Eq, "expected '=' after variable name")?;

        let value = self.parse_expr()?;

        Ok(Stmt::Let { name, value, span })
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
                Ok(Expr::Variable { name, span })
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

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    /// Returns true if the current token can begin an expression.
    /// Used to decide whether `return` has a value.
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

    /// Consume the current token if it matches `kind`, otherwise return an error.
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
