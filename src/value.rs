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
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Nil => write!(f, "nil"),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, v) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                let map = m.borrow();
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}: {}", k, map[*k])?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_integral_numbers_without_decimal_point() {
        assert_eq!(Value::Number(3.0).to_string(), "3");
        assert_eq!(Value::Number(3.5).to_string(), "3.5");
    }

    #[test]
    fn truthiness_matches_ruby_style_rules() {
        assert!(Value::Number(0.0).is_truthy());
        assert!(Value::str("").is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(!Value::Nil.is_truthy());
    }

    #[test]
    fn arrays_are_reference_types() {
        let a = Value::array(vec![Value::Number(1.0)]);
        let b = a.clone();
        if let Value::Array(cell) = &b {
            cell.borrow_mut().push(Value::Number(2.0));
        }
        if let Value::Array(cell) = &a {
            assert_eq!(cell.borrow().len(), 2);
        } else {
            panic!("expected array");
        }
    }
}
