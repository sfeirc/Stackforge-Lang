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
