use std::collections::HashMap;

use crate::value::Value;

/// Lexical environment implemented as a scope stack.
/// push_scope / pop_scope bracket each block; the global scope is never popped.
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        // Never pop the global scope.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Look up a variable, searching from innermost to outermost scope.
    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    /// Define or shadow a variable in the current (innermost) scope.
    pub fn set(&mut self, name: String, value: Value) {
        self.scopes
            .last_mut()
            .expect("scope stack is empty — this is a bug in Forge")
            .insert(name, value);
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}
