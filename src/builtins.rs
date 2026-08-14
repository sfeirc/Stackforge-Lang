use crate::error::SfError;
use crate::value::Value;

/// Reserved native function names. User `fn` declarations with one of these
/// names are rejected at compile time (see `compiler::check_no_builtin_shadow`)
/// so the two execution back ends never have to disagree about which one
/// wins.
pub const BUILTIN_NAMES: &[&str] = &["len", "push", "pop", "keys"];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

pub fn builtin_id(name: &str) -> Option<usize> {
    BUILTIN_NAMES.iter().position(|n| *n == name)
}

pub fn builtin_name(id: usize) -> &'static str {
    BUILTIN_NAMES[id]
}

fn check_arity(name: &str, args: &[Value], expected: usize, line: u32) -> Result<(), SfError> {
    if args.len() != expected {
        return Err(SfError::runtime(
            format!("{}() expects {} argument(s), got {}", name, expected, args.len()),
            line,
        ));
    }
    Ok(())
}

/// Executes a builtin by name. Returns `Ok(None)` if `name` isn't a builtin
/// at all (caller should then look up a user function).
pub fn call_builtin(name: &str, args: &[Value], line: u32) -> Result<Option<Value>, SfError> {
    if !is_builtin(name) {
        return Ok(None);
    }
    Ok(Some(call_builtin_checked(name, args, line)?))
}

/// Same as `call_builtin` but assumes the caller already confirmed `name` is
/// a builtin (used by the VM, which dispatches by numeric id, not by name).
pub fn call_builtin_checked(name: &str, args: &[Value], line: u32) -> Result<Value, SfError> {
    Ok(match name {
        "len" => {
            check_arity(name, args, 1, line)?;
            match &args[0] {
                Value::Array(a) => Value::Number(a.borrow().len() as f64),
                Value::Str(s) => Value::Number(s.chars().count() as f64),
                Value::Map(m) => Value::Number(m.borrow().len() as f64),
                other => {
                    return Err(SfError::runtime(
                        format!(
                            "len() expects an array, string, or map, got {}",
                            other.type_name()
                        ),
                        line,
                    ))
                }
            }
        }
        "push" => {
            check_arity(name, args, 2, line)?;
            match &args[0] {
                Value::Array(a) => {
                    a.borrow_mut().push(args[1].clone());
                    Value::Nil
                }
                other => {
                    return Err(SfError::runtime(
                        format!(
                            "push() expects an array as its first argument, got {}",
                            other.type_name()
                        ),
                        line,
                    ))
                }
            }
        }
        "pop" => {
            check_arity(name, args, 1, line)?;
            match &args[0] {
                Value::Array(a) => a.borrow_mut().pop().unwrap_or(Value::Nil),
                other => {
                    return Err(SfError::runtime(
                        format!("pop() expects an array, got {}", other.type_name()),
                        line,
                    ))
                }
            }
        }
        "keys" => {
            check_arity(name, args, 1, line)?;
            match &args[0] {
                Value::Map(m) => {
                    let mut ks: Vec<String> = m.borrow().keys().cloned().collect();
                    ks.sort();
                    Value::array(ks.into_iter().map(Value::str).collect())
                }
                other => {
                    return Err(SfError::runtime(
                        format!("keys() expects a map, got {}", other.type_name()),
                        line,
                    ))
                }
            }
        }
        other => unreachable!("call_builtin_checked called with non-builtin name '{}'", other),
    })
}
