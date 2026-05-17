use std::fmt;

use crate::ast::Stmt;
use crate::env::EnvRef;

/// Runtime representation of a named function, capturing its definition-site environment.
#[derive(Clone)]
pub struct FunctionValue {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    /// Lexical environment captured at the point of function declaration.
    /// The function sees variables from this env and its ancestors, not from the call site.
    pub closure_env: EnvRef,
}

impl fmt::Debug for FunctionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionValue")
            .field("name", &self.name)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

/// Runtime value representation. All values are cloned on assignment.
#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Str(String),
    Bool(bool),
    Nil,
    Function(FunctionValue),
    /// A bytecode function value capturing its lexical definition environment.
    BytecodeFunction {
        name: String,
        env: EnvRef,
    },
    StateValue {
        state_name: String,
        variant_name: String,
    },
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "Number",
            Value::Str(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Nil => "Nil",
            Value::Function(_) => "Function",
            Value::BytecodeFunction { .. } => "Function",
            Value::StateValue { .. } => "StateValue",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::BytecodeFunction { name: a, .. }, Value::BytecodeFunction { name: b, .. }) => {
                a == b
            }
            (
                Value::StateValue {
                    state_name: s1,
                    variant_name: v1,
                },
                Value::StateValue {
                    state_name: s2,
                    variant_name: v2,
                },
            ) => s1 == s2 && v1 == v2,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => {
                write!(f, "{}", *n as i64)
            }
            Value::Number(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::Function(func) => write!(f, "<fn {}>", func.name),
            Value::BytecodeFunction { name, .. } => write!(f, "<fn {}>", name),
            Value::StateValue {
                state_name,
                variant_name,
            } => {
                write!(f, "{}.{}", state_name, variant_name)
            }
        }
    }
}
