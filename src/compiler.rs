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

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), SfError> {
        match expr {
            Expr::Number(n) => {
                let idx = self.chunk.add_constant(Value::Number(*n));
                self.chunk.emit(OpCode::PushConst(idx), 0);
            }
            Expr::Str(s) => {
                let idx = self.chunk.add_constant(Value::str(s.clone()));
                self.chunk.emit(OpCode::PushConst(idx), 0);
            }
            Expr::Bool(true) => {
                self.chunk.emit(OpCode::PushTrue, 0);
            }
            Expr::Bool(false) => {
                self.chunk.emit(OpCode::PushFalse, 0);
            }
            Expr::Nil => {
                self.chunk.emit(OpCode::PushNil, 0);
            }
            Expr::Ident(name, line) => match self.resolve_local(name) {
                Some(slot) => {
                    self.chunk.emit(OpCode::GetLocal(slot), *line);
                }
                None => return Err(SfError::compile(format!("undefined variable '{}'", name), *line)),
            },
            Expr::Array(items) => {
                for it in items {
                    self.compile_expr(it)?;
                }
                self.chunk.emit(OpCode::BuildArray(items.len()), 0);
            }
            Expr::Map(entries) => {
                for (k, v) in entries {
                    let idx = self.chunk.add_constant(Value::str(k.clone()));
                    self.chunk.emit(OpCode::PushConst(idx), 0);
                    self.compile_expr(v)?;
                }
                self.chunk.emit(OpCode::BuildMap(entries.len()), 0);
            }
            Expr::Index(base, idx, line) => {
                self.compile_expr(base)?;
                self.compile_expr(idx)?;
                self.chunk.emit(OpCode::IndexGet, *line);
            }
            Expr::Unary(op, operand, line) => {
                self.compile_expr(operand)?;
                match op {
                    UnOp::Neg => {
                        self.chunk.emit(OpCode::Neg, *line);
                    }
                    UnOp::Not => {
                        self.chunk.emit(OpCode::Not, *line);
                    }
                }
            }
            Expr::Binary(op, lhs, rhs, line) => {
                self.compile_expr(lhs)?;
                self.compile_expr(rhs)?;
                let opcode = match op {
                    BinOp::Add => OpCode::Add,
                    BinOp::Sub => OpCode::Sub,
                    BinOp::Mul => OpCode::Mul,
                    BinOp::Div => OpCode::Div,
                    BinOp::Mod => OpCode::Mod,
                    BinOp::Eq => OpCode::Eq,
                    BinOp::NotEq => OpCode::NotEq,
                    BinOp::Lt => OpCode::Lt,
                    BinOp::LtEq => OpCode::LtEq,
                    BinOp::Gt => OpCode::Gt,
                    BinOp::GtEq => OpCode::GtEq,
                };
                self.chunk.emit(opcode, *line);
            }
            Expr::Logical(op, lhs, rhs) => {
                self.compile_expr(lhs)?;
                match op {
                    LogicOp::And => {
                        let jump = self.chunk.emit(OpCode::JumpIfFalsePeek(0), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                        self.compile_expr(rhs)?;
                        let target = self.chunk.here();
                        self.chunk.patch_jump(jump, target);
                    }
                    LogicOp::Or => {
                        let jump = self.chunk.emit(OpCode::JumpIfTruePeek(0), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                        self.compile_expr(rhs)?;
                        let target = self.chunk.here();
                        self.chunk.patch_jump(jump, target);
                    }
                }
            }
            Expr::Assign(name, value, line) => {
                self.compile_expr(value)?;
                match self.resolve_local(name) {
                    Some(slot) => {
                        self.chunk.emit(OpCode::SetLocal(slot), *line);
                    }
                    None => {
                        return Err(SfError::compile(
                            format!("assignment to undefined variable '{}'", name),
                            *line,
                        ))
                    }
                }
            }
            Expr::IndexAssign(base, idx, value, line) => {
                self.compile_expr(base)?;
                self.compile_expr(idx)?;
                self.compile_expr(value)?;
                self.chunk.emit(OpCode::IndexSet, *line);
            }
            Expr::Call(name, args, line) => {
                if let Some(id) = builtins::builtin_id(name) {
                    for a in args {
                        self.compile_expr(a)?;
                    }
                    self.chunk.emit(OpCode::CallBuiltin(id, args.len() as u8), *line);
                } else if let Some((idx, arity)) = self.func_table.get(name).copied() {
                    if arity != args.len() {
                        return Err(SfError::compile(
                            format!(
                                "function '{}' expects {} argument(s), got {}",
                                name,
                                arity,
                                args.len()
                            ),
                            *line,
                        ));
                    }
                    for a in args {
                        self.compile_expr(a)?;
                    }
                    self.chunk.emit(OpCode::Call(idx, args.len() as u8), *line);
                } else {
                    return Err(SfError::compile(format!("undefined function '{}'", name), *line));
                }
            }
        }
        Ok(())
    }
}

/// Compiles a whole program into a flat function table: index 0 is always
/// the compiled top-level script, indices 1.. are user `fn` declarations in
/// declaration order. Two passes: first register every function's name and
/// arity (so forward references and mutual recursion resolve), then compile
/// each body against that fully-populated table.
pub fn compile(program: &Program) -> Result<Vec<Rc<FunctionObj>>, SfError> {
    let mut func_table: HashMap<String, (usize, usize)> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if builtins::is_builtin(&f.name) {
            return Err(SfError::compile(
                format!("function '{}' shadows a builtin of the same name", f.name),
                f.line,
            ));
        }
        if func_table.contains_key(&f.name) {
            return Err(SfError::compile(
                format!("function '{}' is already defined", f.name),
                f.line,
            ));
        }
        func_table.insert(f.name.clone(), (i + 1, f.params.len()));
    }

    let mut functions: Vec<Rc<FunctionObj>> = Vec::with_capacity(program.functions.len() + 1);
    functions.push(Rc::new(FunctionObj {
        name: "<script>".to_string(),
        arity: 0,
        chunk: Chunk::default(),
    }));

    for f in &program.functions {
        let mut fc = FnCompiler::new(&func_table, &f.params);
        fc.compile_block(&f.body)?;
        fc.chunk.emit(OpCode::PushNil, f.line);
        fc.chunk.emit(OpCode::Return, f.line);
        functions.push(Rc::new(FunctionObj {
            name: f.name.clone(),
            arity: f.params.len(),
            chunk: fc.chunk,
        }));
    }

    let mut main_fc = FnCompiler::new(&func_table, &[]);
    main_fc.compile_block(&program.main)?;
    main_fc.chunk.emit(OpCode::PushNil, 0);
    main_fc.chunk.emit(OpCode::Return, 0);
    functions[0] = Rc::new(FunctionObj {
        name: "<script>".to_string(),
        arity: 0,
        chunk: main_fc.chunk,
    });

    Ok(functions)
}
