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

    fn compile_block(&mut self, stmts: &[Stmt]) -> Result<(), SfError> {
        for s in stmts {
            self.compile_stmt(s)?;
        }
        Ok(())
    }

    fn compile_scoped_block(&mut self, stmts: &[Stmt], line: u32) -> Result<(), SfError> {
        self.begin_scope();
        self.compile_block(stmts)?;
        self.end_scope(line);
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), SfError> {
        match stmt {
            Stmt::Let(name, expr, _line) => {
                self.compile_expr(expr)?;
                self.declare_local(name);
            }
            Stmt::ExprStmt(expr) => {
                self.compile_expr(expr)?;
                self.chunk.emit(OpCode::Pop, 0);
            }
            Stmt::Print(exprs, line) => {
                for e in exprs {
                    self.compile_expr(e)?;
                }
                self.chunk.emit(OpCode::Print(exprs.len() as u8), *line);
            }
            Stmt::If(cond, then_b, else_b) => {
                self.compile_expr(cond)?;
                let else_jump = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
                self.compile_scoped_block(then_b, 0)?;
                let end_jump = self.chunk.emit(OpCode::Jump(0), 0);
                let else_target = self.chunk.here();
                self.chunk.patch_jump(else_jump, else_target);
                self.compile_scoped_block(else_b, 0)?;
                let end_target = self.chunk.here();
                self.chunk.patch_jump(end_jump, end_target);
            }
            Stmt::While(cond, body) => {
                let loop_start = self.chunk.here();
                self.compile_expr(cond)?;
                let exit_jump = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
                self.compile_scoped_block(body, 0)?;
                self.chunk.emit(OpCode::Jump(loop_start), 0);
                let exit_target = self.chunk.here();
                self.chunk.patch_jump(exit_jump, exit_target);
            }
            Stmt::Block(stmts) => self.compile_scoped_block(stmts, 0)?,
            Stmt::Return(expr, line) => {
                match expr {
                    Some(e) => self.compile_expr(e)?,
                    None => {
                        self.chunk.emit(OpCode::PushNil, *line);
                    }
                }
                self.chunk.emit(OpCode::Return, *line);
            }
        }
        Ok(())
    }
