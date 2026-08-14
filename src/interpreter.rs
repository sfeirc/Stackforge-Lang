use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

use crate::ast::*;
use crate::error::SfError;
use crate::value::Value;

/// Tree-walking interpreter: evaluates the AST directly, no compilation
/// step. This is the "obvious" implementation strategy and exists
/// primarily as (a) a second, independent execution path that must agree
/// with the bytecode VM on every test program, and (b) the baseline the
/// VM's benchmark numbers are measured against.
pub struct Interpreter<'a> {
    functions: HashMap<String, Rc<FunctionDecl>>,
    out: &'a mut dyn Write,
}

/// Non-local control flow. `Return` unwinds through nested blocks/loops up
/// to the enclosing function call (or the top level, ending the script).
enum Flow {
    Normal,
    Return(Value),
}

type Scopes = Vec<HashMap<String, Value>>;

impl<'a> Interpreter<'a> {
    pub fn new(out: &'a mut dyn Write) -> Self {
        Interpreter {
            functions: HashMap::new(),
            out,
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<(), SfError> {
        for f in &program.functions {
            self.functions.insert(f.name.clone(), Rc::new(f.clone()));
        }
        let mut scopes: Scopes = vec![HashMap::new()];
        self.exec_block(&program.main, &mut scopes)?;
        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Stmt], scopes: &mut Scopes) -> Result<Flow, SfError> {
        for stmt in stmts {
            match self.exec_stmt(stmt, scopes)? {
                Flow::Normal => {}
                ret @ Flow::Return(_) => return Ok(ret),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_scoped(&mut self, stmts: &[Stmt], scopes: &mut Scopes) -> Result<Flow, SfError> {
        scopes.push(HashMap::new());
        let result = self.exec_block(stmts, scopes);
        scopes.pop();
        result
    }

    fn exec_stmt(&mut self, stmt: &Stmt, scopes: &mut Scopes) -> Result<Flow, SfError> {
        match stmt {
            Stmt::Let(name, expr, _line) => {
                let value = self.eval(expr, scopes)?;
                scopes.last_mut().unwrap().insert(name.clone(), value);
                Ok(Flow::Normal)
            }
            Stmt::ExprStmt(expr) => {
                self.eval(expr, scopes)?;
                Ok(Flow::Normal)
            }
            Stmt::Print(exprs, _line) => {
                let mut parts = Vec::with_capacity(exprs.len());
                for e in exprs {
                    parts.push(self.eval(e, scopes)?.to_string());
                }
                writeln!(self.out, "{}", parts.join(" ")).ok();
                Ok(Flow::Normal)
            }
            Stmt::If(cond, then_b, else_b) => {
                if self.eval(cond, scopes)?.is_truthy() {
                    self.exec_scoped(then_b, scopes)
                } else {
                    self.exec_scoped(else_b, scopes)
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond, scopes)?.is_truthy() {
                    match self.exec_scoped(body, scopes)? {
                        Flow::Normal => {}
                        ret @ Flow::Return(_) => return Ok(ret),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Block(stmts) => self.exec_scoped(stmts, scopes),
            Stmt::Return(expr, _line) => {
                let value = match expr {
                    Some(e) => self.eval(e, scopes)?,
                    None => Value::Nil,
                };
                Ok(Flow::Return(value))
            }
        }
    }

    fn lookup(&self, scopes: &Scopes, name: &str, line: u32) -> Result<Value, SfError> {
        for scope in scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Ok(v.clone());
            }
        }
        Err(SfError::runtime(format!("undefined variable '{}'", name), line))
    }

    fn assign(&self, scopes: &mut Scopes, name: &str, value: Value, line: u32) -> Result<(), SfError> {
        for scope in scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(SfError::runtime(
            format!("assignment to undefined variable '{}'", name),
            line,
        ))
    }
