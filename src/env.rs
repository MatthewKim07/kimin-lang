use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::value::Value;

/// A shared, interior-mutable reference to an environment frame.
pub type EnvRef = Rc<RefCell<Env>>;

/// A single frame in the lexical environment chain.
/// Each frame holds its own bindings and an optional link to its enclosing frame.
pub struct Env {
    bindings: HashMap<String, Value>,
    parent: Option<EnvRef>,
}

impl Env {
    /// Create the global (root) environment with no parent.
    pub fn new_global() -> EnvRef {
        Rc::new(RefCell::new(Env {
            bindings: HashMap::new(),
            parent: None,
        }))
    }

    /// Create a child frame with `parent` as its enclosing environment.
    pub fn new_child(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Env {
            bindings: HashMap::new(),
            parent: Some(parent),
        }))
    }

    /// Look up a variable, walking from this frame up through all parent frames.
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.bindings.get(name) {
            return Some(v.clone());
        }
        match &self.parent {
            Some(parent) => parent.borrow().get(name),
            None => None,
        }
    }

    /// Define a variable in this frame (innermost scope only).
    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    /// Update an existing variable binding, searching from this frame up through parents.
    /// Returns true if the variable was found and updated, false if not found.
    /// Used exclusively by `transition` statements — not for general assignment.
    pub fn assign_existing(&mut self, name: &str, value: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), value);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().assign_existing(name, value)
        } else {
            false
        }
    }
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Env({} bindings)", self.bindings.len())
    }
}
