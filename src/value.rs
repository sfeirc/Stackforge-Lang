use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// A runtime value. Shared, byte-for-byte, between the tree-walking
/// interpreter and the bytecode VM -- both back ends must agree on what a
/// value *is*, even though they disagree on how they get produced.
///
/// Arrays and maps are reference types (`Rc<RefCell<_>>`), matching the
/// language's semantics: `let b = a; b[0] = 9;` mutates the same array `a`
/// points at. This is a deliberate, disclosed simplification -- see
/// "Honest scope" in the README for what that does and doesn't buy us.
#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(Rc<String>),
    Array(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<String, Value>>>),
    Nil,
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(Rc::new(s.into()))
    }

    pub fn array(items: Vec<Value>) -> Value {
        Value::Array(Rc::new(RefCell::new(items)))
    }

    pub fn map(entries: Vec<(String, Value)>) -> Value {
        Value::Map(Rc::new(RefCell::new(entries.into_iter().collect())))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
            Value::Nil => "nil",
        }
    }

    /// Truthiness used by `if`/`while`/`&&`/`||`: everything is truthy
    /// except `false` and `nil` (Ruby/Lua-style), not "0 is falsy"
    /// (Python/C-style) -- a deliberate choice, documented in the README,
    /// so `if (0) { ... }` runs the branch (0 is a perfectly good number).
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Bool(false) | Value::Nil)
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
