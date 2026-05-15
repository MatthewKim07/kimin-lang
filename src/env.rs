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
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Env({} bindings)", self.bindings.len())
    }
}
