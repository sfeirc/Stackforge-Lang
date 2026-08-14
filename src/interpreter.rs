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

    fn eval(&mut self, expr: &Expr, scopes: &mut Scopes) -> Result<Value, SfError> {
        Ok(match expr {
            Expr::Number(n) => Value::Number(*n),
            Expr::Str(s) => Value::str(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Nil => Value::Nil,
            Expr::Ident(name, line) => self.lookup(scopes, name, *line)?,
            Expr::Array(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for it in items {
                    vals.push(self.eval(it, scopes)?);
                }
                Value::array(vals)
            }
            Expr::Map(entries) => {
                let mut vals = Vec::with_capacity(entries.len());
                for (k, v) in entries {
                    vals.push((k.clone(), self.eval(v, scopes)?));
                }
                Value::map(vals)
            }
            Expr::Index(base, idx, line) => {
                let base_v = self.eval(base, scopes)?;
                let idx_v = self.eval(idx, scopes)?;
                index_get(&base_v, &idx_v, *line)?
            }
            Expr::Unary(op, operand, line) => {
                let v = self.eval(operand, scopes)?;
                match op {
                    UnOp::Neg => Value::Number(-expect_number(&v, *line)?),
                    UnOp::Not => Value::Bool(!v.is_truthy()),
                }
            }
            Expr::Binary(op, lhs, rhs, line) => {
                let l = self.eval(lhs, scopes)?;
                let r = self.eval(rhs, scopes)?;
                eval_binary(op, &l, &r, *line)?
            }
            Expr::Logical(op, lhs, rhs) => {
                let l = self.eval(lhs, scopes)?;
                match op {
                    LogicOp::And => {
                        if !l.is_truthy() {
                            l
                        } else {
                            self.eval(rhs, scopes)?
                        }
                    }
                    LogicOp::Or => {
                        if l.is_truthy() {
                            l
                        } else {
                            self.eval(rhs, scopes)?
                        }
                    }
                }
            }
            Expr::Assign(name, value_expr, line) => {
                let v = self.eval(value_expr, scopes)?;
                self.assign(scopes, name, v.clone(), *line)?;
                v
            }
            Expr::IndexAssign(base, idx, value_expr, line) => {
                let base_v = self.eval(base, scopes)?;
                let idx_v = self.eval(idx, scopes)?;
                let value = self.eval(value_expr, scopes)?;
                index_set(&base_v, &idx_v, value.clone(), *line)?;
                value
            }
            Expr::Call(name, arg_exprs, line) => {
                let mut args = Vec::with_capacity(arg_exprs.len());
                for a in arg_exprs {
                    args.push(self.eval(a, scopes)?);
                }
                self.call(name, args, *line)?
            }
        })
    }

    fn call(&mut self, name: &str, args: Vec<Value>, line: u32) -> Result<Value, SfError> {
        if let Some(v) = crate::builtins::call_builtin(name, &args, line)? {
            return Ok(v);
        }
        let func = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| SfError::runtime(format!("undefined function '{}'", name), line))?;
        if func.params.len() != args.len() {
            return Err(SfError::runtime(
                format!(
                    "function '{}' expects {} argument(s), got {}",
                    name,
                    func.params.len(),
                    args.len()
                ),
                line,
            ));
        }
        let mut scopes: Scopes = vec![HashMap::new()];
        for (param, arg) in func.params.iter().zip(args) {
            scopes[0].insert(param.clone(), arg);
        }
        match self.exec_block(&func.body, &mut scopes)? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Value::Nil),
        }
    }
}

fn expect_number(v: &Value, line: u32) -> Result<f64, SfError> {
    v.as_number()
        .ok_or_else(|| SfError::runtime(format!("expected a number, got {}", v.type_name()), line))
}
