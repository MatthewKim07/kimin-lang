use std::collections::BTreeMap;
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
    /// A fixed-size homogeneous array.
    Array(Vec<Value>),
    /// A map with Text keys and homogeneous values.
    /// BTreeMap gives deterministic alphabetical key ordering for display and equality.
    Map(BTreeMap<String, Value>),
    /// A struct value. Fields stored in BTreeMap for deterministic alphabetical display.
    Struct {
        name: String,
        fields: BTreeMap<String, Value>,
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
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
            Value::Struct { .. } => "Struct",
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
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (
                Value::Struct {
                    name: n1,
                    fields: f1,
                },
                Value::Struct {
                    name: n2,
                    fields: f2,
                },
            ) => n1 == n2 && f1 == f2,
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
            Value::Array(elems) => {
                let parts: Vec<String> = elems.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Map(map) => {
                let parts: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Value::Struct { name, fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} {{ {} }}", name, parts.join(", "))
            }
        }
    }
}

/// Parse a Text value into a Bool.
///
/// - Trims leading/trailing whitespace.
/// - Accepts exactly "true" → true and "false" → false (case-sensitive).
/// - Rejects all other strings including case variants, numeric strings, and empty.
pub fn parse_bool_from_text(s: &str) -> Result<bool, String> {
    let trimmed = s.trim();
    match trimmed {
        "true" => Ok(true),
        "false" => Ok(false),
        "" => Err("cannot convert '' to Bool (empty string)".to_string()),
        other => Err(format!("cannot convert '{}' to Bool", other)),
    }
}

/// Parse a Text value into a finite f64.
///
/// - Trims leading/trailing whitespace.
/// - Rejects empty strings.
/// - Rejects non-numeric strings.
/// - Rejects non-finite results (NaN, ±Infinity).
pub fn parse_number_from_text(s: &str) -> Result<f64, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("cannot convert '' to Number (empty string)".to_string());
    }
    let n: f64 = trimmed
        .parse()
        .map_err(|_| format!("cannot convert '{}' to Number", trimmed))?;
    if !n.is_finite() {
        return Err(format!("cannot convert '{}' to Number", trimmed));
    }
    Ok(n)
}

/// Apply `{}` placeholder substitution.
///
/// Counts non-overlapping `{}` substrings in `template`; each is replaced
/// left-to-right with the display of the corresponding arg.  Returns an error
/// string if the placeholder count does not equal `args.len()`.
pub fn format_template(template: &str, args: &[Value]) -> Result<String, String> {
    // Count {} placeholders first
    let chars: Vec<char> = template.chars().collect();
    let mut placeholder_count = 0usize;
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '}' {
            placeholder_count += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    if placeholder_count != args.len() {
        return Err(format!(
            "format expected {} value{} for placeholders, got {}",
            placeholder_count,
            if placeholder_count == 1 { "" } else { "s" },
            args.len()
        ));
    }
    // Build result string
    let mut result = String::new();
    let mut arg_idx = 0usize;
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '}' {
            result.push_str(&format!("{}", args[arg_idx]));
            arg_idx += 1;
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    Ok(result)
}
