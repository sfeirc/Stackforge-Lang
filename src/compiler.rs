use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::*;
use crate::builtins;
use crate::chunk::{Chunk, FunctionObj, OpCode};
use crate::error::SfError;
use crate::value::Value;

/// One statically-known local variable slot. `depth` is the lexical block
/// nesting level it was declared at, used by `end_scope` to know which
/// locals just went out of scope.
struct Local {
    name: String,
    depth: u32,
}

/// Compiles a single function body (or the top-level script) into a
/// `Chunk`. Each function gets its own `FnCompiler` -- locals never leak
/// across function boundaries, matching the language's "no closures" rule
/// (see Honest scope in the README).
struct FnCompiler<'a> {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: u32,
    func_table: &'a HashMap<String, (usize, usize)>,
}

impl<'a> FnCompiler<'a> {
    fn new(func_table: &'a HashMap<String, (usize, usize)>, params: &[String]) -> Self {
        let locals = params
            .iter()
            .map(|p| Local {
                name: p.clone(),
                depth: 0,
            })
            .collect();
        FnCompiler {
            chunk: Chunk::default(),
            locals,
            scope_depth: 0,
            func_table,
        }
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rposition(|l| l.name == name)
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// Pops every local declared since the matching `begin_scope`, emitting
    /// one `Pop` per local so the operand stack actually shrinks back down
    /// at runtime (not just in the compiler's own bookkeeping).
    fn end_scope(&mut self, line: u32) {
        self.scope_depth -= 1;
        while let Some(last) = self.locals.last() {
            if last.depth > self.scope_depth {
                self.locals.pop();
                self.chunk.emit(OpCode::Pop, line);
            } else {
                break;
            }
        }
    }

    fn declare_local(&mut self, name: &str) {
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
        });
    }
