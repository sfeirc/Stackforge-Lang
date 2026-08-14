use std::io::Write;
use std::rc::Rc;

use crate::ast::BinOp;
use crate::builtins;
use crate::chunk::{FunctionObj, OpCode};
use crate::error::SfError;
use crate::interpreter::{eval_binary, index_get, index_set};
use crate::value::Value;

/// One activation record. `base` is the stack index of local slot 0 for
/// this call -- parameters occupy `base..base+arity`, and any further
/// `let`s inside the function get slots `base+arity`, `base+arity+1`, ...
struct CallFrame {
    function: Rc<FunctionObj>,
    ip: usize,
    base: usize,
}

/// A stack-based bytecode VM: one growable `Vec<Value>` operand stack, one
/// call-frame stack for function activation records, and a flat
/// fetch-decode-execute loop over `OpCode`. Deliberately dynamically typed
/// (matching the tree-walking interpreter) and deliberately simple: no
/// register allocation, no inline caching, no GC beyond `Rc` refcounting.
pub struct Vm<'a> {
    functions: Vec<Rc<FunctionObj>>,
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    out: &'a mut dyn Write,
}

impl<'a> Vm<'a> {
    pub fn new(functions: Vec<Rc<FunctionObj>>, out: &'a mut dyn Write) -> Self {
        Vm {
            functions,
            stack: Vec::new(),
            frames: Vec::new(),
            out,
        }
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Result<Value, SfError> {
        self.stack
            .pop()
            .ok_or_else(|| SfError::runtime("internal error: operand stack underflow", 0))
    }

    /// Runs the program starting at `functions[0]` (the compiled top-level
    /// script) to completion. On success, the operand stack is guaranteed
    /// back to empty -- see `vm::tests::stack_is_balanced_after_a_full_program`
    /// for the regression test that would catch a leak here.
    pub fn run(&mut self) -> Result<(), SfError> {
        let main = self.functions[0].clone();
        self.frames.push(CallFrame {
            function: main,
            ip: 0,
            base: 0,
        });

        loop {
            let frame_idx = self.frames.len() - 1;
            let (op, line) = {
                let frame = &self.frames[frame_idx];
                let code = &frame.function.chunk.code;
                if frame.ip >= code.len() {
                    return Err(SfError::runtime(
                        "internal error: fell off the end of a chunk without Return",
                        0,
                    ));
                }
                (code[frame.ip].clone(), frame.function.chunk.lines[frame.ip])
            };
            self.frames[frame_idx].ip += 1;

            match op {
                OpCode::PushConst(i) => {
                    let v = self.frames[frame_idx].function.chunk.constants[i].clone();
                    self.push(v);
                }
                OpCode::PushNil => self.push(Value::Nil),
                OpCode::PushTrue => self.push(Value::Bool(true)),
                OpCode::PushFalse => self.push(Value::Bool(false)),
                OpCode::Pop => {
                    self.pop()?;
                }

                OpCode::Add => self.binary(BinOp::Add, line)?,
                OpCode::Sub => self.binary(BinOp::Sub, line)?,
                OpCode::Mul => self.binary(BinOp::Mul, line)?,
                OpCode::Div => self.binary(BinOp::Div, line)?,
                OpCode::Mod => self.binary(BinOp::Mod, line)?,
                OpCode::Eq => self.binary(BinOp::Eq, line)?,
                OpCode::NotEq => self.binary(BinOp::NotEq, line)?,
                OpCode::Lt => self.binary(BinOp::Lt, line)?,
                OpCode::LtEq => self.binary(BinOp::LtEq, line)?,
                OpCode::Gt => self.binary(BinOp::Gt, line)?,
                OpCode::GtEq => self.binary(BinOp::GtEq, line)?,

                OpCode::Neg => {
                    let v = self.pop()?;
                    let n = v.as_number().ok_or_else(|| {
                        SfError::runtime(format!("expected a number, got {}", v.type_name()), line)
                    })?;
                    self.push(Value::Number(-n));
                }
                OpCode::Not => {
                    let v = self.pop()?;
                    self.push(Value::Bool(!v.is_truthy()));
                }

                OpCode::GetLocal(slot) => {
                    let base = self.frames[frame_idx].base;
                    self.push(self.stack[base + slot].clone());
                }
                OpCode::SetLocal(slot) => {
                    let base = self.frames[frame_idx].base;
                    let v = self
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| SfError::runtime("internal error: operand stack underflow", 0))?;
                    self.stack[base + slot] = v;
                }

                OpCode::Jump(target) => {
                    self.frames[frame_idx].ip = target;
                }
                OpCode::JumpIfFalse(target) => {
                    let v = self.pop()?;
                    if !v.is_truthy() {
                        self.frames[frame_idx].ip = target;
                    }
                }
                OpCode::JumpIfFalsePeek(target) => {
                    let truthy = self.stack.last().map(|v| v.is_truthy()).unwrap_or(false);
                    if !truthy {
                        self.frames[frame_idx].ip = target;
                    }
                }
                OpCode::JumpIfTruePeek(target) => {
                    let truthy = self.stack.last().map(|v| v.is_truthy()).unwrap_or(false);
                    if truthy {
                        self.frames[frame_idx].ip = target;
                    }
                }

                OpCode::BuildArray(count) => {
                    let start = self.stack.len() - count;
                    let items: Vec<Value> = self.stack.split_off(start);
                    self.push(Value::array(items));
                }
                OpCode::BuildMap(count) => {
                    let mut entries = Vec::with_capacity(count);
                    for _ in 0..count {
                        let val = self.pop()?;
                        let key = self.pop()?;
                        let key_str = match key {
                            Value::Str(s) => s.to_string(),
                            other => {
                                return Err(SfError::runtime(
                                    format!("internal error: map key compiled to a {}", other.type_name()),
                                    line,
                                ))
                            }
                        };
                        entries.push((key_str, val));
                    }
                    self.push(Value::map(entries));
                }
                OpCode::IndexGet => {
                    let idx = self.pop()?;
                    let base = self.pop()?;
                    let v = index_get(&base, &idx, line)?;
                    self.push(v);
                }
                OpCode::IndexSet => {
                    let value = self.pop()?;
                    let idx = self.pop()?;
                    let base = self.pop()?;
                    index_set(&base, &idx, value.clone(), line)?;
                    self.push(value);
                }

                OpCode::Call(func_idx, argc) => {
                    let func = self.functions[func_idx].clone();
                    let argc = argc as usize;
                    if func.arity != argc {
                        return Err(SfError::runtime(
                            format!(
                                "function '{}' expects {} argument(s), got {}",
                                func.name, func.arity, argc
                            ),
                            line,
                        ));
                    }
                    let base = self.stack.len() - argc;
                    self.frames.push(CallFrame {
                        function: func,
                        ip: 0,
                        base,
                    });
                }
                OpCode::CallBuiltin(id, argc) => {
                    let argc = argc as usize;
                    let start = self.stack.len() - argc;
                    let args: Vec<Value> = self.stack.split_off(start);
                    let name = builtins::builtin_name(id);
                    let v = builtins::call_builtin_checked(name, &args, line)?;
                    self.push(v);
                }
                OpCode::Print(argc) => {
                    let start = self.stack.len() - argc as usize;
                    let vals: Vec<Value> = self.stack.split_off(start);
                    let line_str = vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
                    writeln!(self.out, "{}", line_str).ok();
                }
                OpCode::Return => {
                    let value = self.pop()?;
                    let frame = self.frames.pop().unwrap();
                    self.stack.truncate(frame.base);
                    if self.frames.is_empty() {
                        // The outermost (script) frame returned: nothing is
                        // left to consume this value, so it is discarded
                        // rather than pushed back -- this is what keeps the
                        // operand stack balanced at program end.
                        return Ok(());
                    }
                    self.push(value);
                }
            }
        }
    }

    fn binary(&mut self, op: BinOp, line: u32) -> Result<(), SfError> {
        let r = self.pop()?;
        let l = self.pop()?;
        let v = eval_binary(&op, &l, &r, line)?;
        self.push(v);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::parser::parse;

    fn run_capture(src: &str) -> String {
        let program = parse(src).unwrap();
        let functions = compile(&program).unwrap();
        let mut out = Vec::new();
        {
            let mut vm = Vm::new(functions, &mut out);
            vm.run().unwrap();
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn runs_straight_line_arithmetic() {
        assert_eq!(run_capture("print 1 + 2 * 3;"), "7\n");
    }

    #[test]
    fn runs_recursive_fibonacci() {
        let src = r#"
            fn fib(n) {
                if (n < 2) { return n; }
                return fib(n - 1) + fib(n - 2);
            }
            print fib(10);
        "#;
        assert_eq!(run_capture(src), "55\n");
    }

    #[test]
    fn stack_is_balanced_after_a_full_program() {
        let src = r#"
            fn add(a, b) { return a + b; }
            let total = 0;
            for (let i = 0; i < 10; i = i + 1) {
                total = add(total, i);
            }
            let arr = [1, 2, 3];
            arr[0] = 99;
            let m = {"a": 1, "b": 2};
            if (total > 0 && len(arr) == 3) {
                print total, arr[0], m["a"];
            }
        "#;
        let program = parse(src).unwrap();
        let functions = compile(&program).unwrap();
        let mut out = Vec::new();
        let mut vm = Vm::new(functions, &mut out);
        vm.run().unwrap();
        assert_eq!(vm.stack.len(), 0, "operand stack leaked values: {:?}", vm.stack);
    }

    #[test]
    fn division_by_zero_is_a_runtime_error_with_line() {
        let program = parse("let x = 1;\nlet y = x / 0;").unwrap();
        let functions = compile(&program).unwrap();
        let mut out = Vec::new();
        let mut vm = Vm::new(functions, &mut out);
        let err = vm.run().unwrap_err();
        match err {
            SfError::Runtime { line, .. } => assert_eq!(line, 2),
            other => panic!("expected Runtime error, got {:?}", other),
        }
    }

    #[test]
    fn array_out_of_bounds_is_a_runtime_error() {
        let program = parse("let a = [1, 2]; let x = a[5];").unwrap();
        let functions = compile(&program).unwrap();
        let mut out = Vec::new();
        let mut vm = Vm::new(functions, &mut out);
        assert!(matches!(vm.run().unwrap_err(), SfError::Runtime { .. }));
    }
}
