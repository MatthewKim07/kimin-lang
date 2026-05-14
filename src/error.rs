use std::fmt;

#[derive(Debug)]
pub struct LexError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LexError at line {}, column {}: {}",
            self.line, self.col, self.msg
        )
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ParseError at line {}, column {}: {}",
            self.line, self.col, self.msg
        )
    }
}

#[derive(Debug)]
pub struct RuntimeError {
    pub msg: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuntimeError: {}", self.msg)
    }
}

/// Top-level error type covering all phases of execution.
#[derive(Debug)]
pub enum KiminError {
    Lex(LexError),
    Parse(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for KiminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KiminError::Lex(e) => write!(f, "{}", e),
            KiminError::Parse(e) => write!(f, "{}", e),
            KiminError::Runtime(e) => write!(f, "{}", e),
        }
    }
}

impl From<LexError> for KiminError {
    fn from(e: LexError) -> Self {
        KiminError::Lex(e)
    }
}

impl From<ParseError> for KiminError {
    fn from(e: ParseError) -> Self {
        KiminError::Parse(e)
    }
}

impl From<RuntimeError> for KiminError {
    fn from(e: RuntimeError) -> Self {
        KiminError::Runtime(e)
    }
}
