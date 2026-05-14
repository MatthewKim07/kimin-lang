use std::fmt;

/// Runtime value representation. All values are cloned on assignment;
/// reference semantics can be added in a later milestone.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Str(String),
    Bool(bool),
    Nil,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "Number",
            Value::Str(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Nil => "Nil",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Print whole numbers without a decimal point.
            Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => {
                write!(f, "{}", *n as i64)
            }
            Value::Number(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
        }
    }
}
