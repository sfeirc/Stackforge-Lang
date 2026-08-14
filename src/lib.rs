//! Stackforge: lexer -> Pratt parser -> AST, executed either by a
//! tree-walking interpreter or by a compiler-to-bytecode + stack-based VM.
//!
//! See the crate's README for the language grammar, architecture, and
//! benchmark numbers. This module just wires the phases together.

pub mod ast;
pub mod builtins;
pub mod chunk;
pub mod compiler;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod value;
pub mod vm;

use std::io::Write;

pub use error::{SfError, SfResult};

/// Parses and runs `src` with the tree-walking interpreter, writing every
/// `print` line to `out`.
pub fn run_ast(src: &str, out: &mut dyn Write) -> SfResult<()> {
    let program = parser::parse(src)?;
    let mut interp = interpreter::Interpreter::new(out);
    interp.run(&program)
}

/// Parses, compiles to bytecode, and runs `src` on the stack-based VM,
/// writing every `print` line to `out`.
pub fn run_vm(src: &str, out: &mut dyn Write) -> SfResult<()> {
    let program = parser::parse(src)?;
    let functions = compiler::compile(&program)?;
    let mut vm = vm::Vm::new(functions, out);
    vm.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree-walking interpreter and the bytecode VM are two independent
    /// implementations of the same language semantics; this differential
    /// test runs a batch of programs through both and asserts they print
    /// byte-for-byte identical output. Divergence here means one of the two
    /// back ends has a semantic bug.
    fn assert_backends_agree(src: &str) {
        let mut ast_out = Vec::new();
        let mut vm_out = Vec::new();
        run_ast(src, &mut ast_out).unwrap_or_else(|e| panic!("ast backend failed on {:?}: {}", src, e));
        run_vm(src, &mut vm_out).unwrap_or_else(|e| panic!("vm backend failed on {:?}: {}", src, e));
        assert_eq!(
            String::from_utf8(ast_out).unwrap(),
            String::from_utf8(vm_out).unwrap(),
            "ast interpreter and vm disagree on program:\n{}",
            src
        );
    }

    #[test]
    fn both_backends_agree_on_recursive_fibonacci() {
        assert_backends_agree(
            r#"
            fn fib(n) {
                if (n < 2) { return n; }
                return fib(n - 1) + fib(n - 2);
            }
            print fib(15);
            "#,
        );
    }

    #[test]
    fn both_backends_agree_on_loops_and_arrays_and_maps() {
        assert_backends_agree(
            r#"
            let arr = [];
            for (let i = 0; i < 5; i = i + 1) {
                push(arr, i * i);
            }
            let m = {"sum": 0};
            for (let i = 0; i < len(arr); i = i + 1) {
                m["sum"] = m["sum"] + arr[i];
            }
            print arr, m["sum"];
            "#,
        );
    }

    #[test]
    fn both_backends_agree_on_short_circuit_logic() {
        assert_backends_agree(
            r#"
            fn noisy_true() { print "called true"; return true; }
            fn noisy_false() { print "called false"; return false; }
            if (false && noisy_true()) { print "unreachable"; }
            if (true || noisy_false()) { print "reached"; }
            "#,
        );
    }
}
