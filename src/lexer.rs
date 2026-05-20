use crate::error::LexError;
use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();

            if self.is_at_end() {
                break;
            }

            let tok = self.next_token()?;
            tokens.push(tok);
        }

        tokens.push(Token::new(TokenKind::Eof, self.line, self.col));
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let line = self.line;
        let col = self.col;
        let c = self.advance();

        let kind = match c {
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            ':' => TokenKind::Colon,
            '-' => {
                if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::MinusEqual
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '"' => self.lex_string(line, col)?,
            c if c.is_ascii_digit() => self.lex_number(c, line, col)?,
            c if c.is_alphabetic() || c == '_' => self.lex_ident_or_keyword(c),
            other => {
                return Err(LexError {
                    msg: format!("unexpected character '{}'", other),
                    line,
                    col,
                });
            }
        };

        Ok(Token::new(kind, line, col))
    }

    fn lex_string(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(LexError {
                        msg: "unterminated string literal".into(),
                        line,
                        col,
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some(_) => {
                    s.push(self.advance());
                }
            }
        }
        Ok(TokenKind::String(s))
    }

    fn lex_number(&mut self, first: char, line: usize, col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::from(first);
        let mut saw_dot = false;

        loop {
            match self.peek() {
                Some(c) if c.is_ascii_digit() => {
                    s.push(c);
                    self.advance();
                }
                Some('.') if !saw_dot => {
                    // Only consume the dot if the next character is a digit.
                    if matches!(self.peek_second(), Some(d) if d.is_ascii_digit()) {
                        saw_dot = true;
                        s.push('.');
                        self.advance();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        s.parse::<f64>()
            .map(TokenKind::Number)
            .map_err(|_| LexError {
                msg: format!("invalid number literal '{}'", s),
                line,
                col,
            })
    }

    fn lex_ident_or_keyword(&mut self, first: char) -> TokenKind {
        let mut s = String::from(first);
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
            s.push(self.advance());
        }
        match s.as_str() {
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "print" => TokenKind::Print,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "state" => TokenKind::State,
            "transition" => TokenKind::Transition,
            "simulate" => TokenKind::Simulate,
            "step" => TokenKind::Step,
            "mut" => TokenKind::Mut,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            _ => TokenKind::Ident(s),
        }
    }

    /// Skip spaces, tabs, carriage returns, newlines, and `//` line comments.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip horizontal/vertical whitespace.
            while matches!(self.peek(), Some(c) if c.is_whitespace()) {
                self.advance();
            }
            // Skip line comments.
            if self.peek() == Some('/') && self.peek_second() == Some('/') {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    // --- low-level character helpers ---

    fn advance(&mut self) -> char {
        let c = self.source[self.pos];
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn peek_second(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }
}
