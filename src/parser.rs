use crate::ast::*;
use crate::error::SfError;
use crate::lexer::lex;
use crate::token::{Token, TokenKind};

/// Recursive-descent statement parser + Pratt (binding-power) expression
/// parser. Statements (`let`, `if`, `while`, `for`, `fn`, `return`, blocks)
/// are parsed by dedicated recursive-descent functions, one per grammar
/// production; expressions are parsed by a single `parse_expr_bp` loop keyed
/// off a binding-power table, which is what actually buys operator
/// precedence without a cascade of one-production-per-precedence-level
/// functions.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

/// Left/right binding power for each infix operator. Left-associative
/// operators use `(n, n+1)`; a strictly higher `n` means "binds tighter".
fn infix_binding_power(kind: &TokenKind) -> Option<(u8, u8)> {
    use TokenKind::*;
    Some(match kind {
        OrOr => (1, 2),
        AndAnd => (3, 4),
        EqEq | NotEq => (5, 6),
        Lt | LtEq | Gt | GtEq => (7, 8),
        Plus | Minus => (9, 10),
        Star | Slash | Percent => (11, 12),
        _ => return None,
    })
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn line(&self) -> u32 {
        self.peek().line
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if !self.is_eof() {
            self.pos += 1;
        }
        tok
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn match_tok(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<Token, SfError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(SfError::parse(
                format!("expected {} but found {:?}", what, self.peek().kind),
                self.line(),
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, SfError> {
        match &self.peek().kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            other => Err(SfError::parse(
                format!("expected identifier but found {:?}", other),
                self.line(),
            )),
        }
    }

    // ---------------------------------------------------------------- //
    // Program / declarations
    // ---------------------------------------------------------------- //

    pub fn parse_program(&mut self) -> Result<Program, SfError> {
        let mut program = Program::default();
        while !self.is_eof() {
            if self.check(&TokenKind::Fn) {
                program.functions.push(self.parse_function_decl()?);
            } else {
                program.main.push(self.parse_statement()?);
            }
        }
        Ok(program)
    }

    fn parse_function_decl(&mut self) -> Result<FunctionDecl, SfError> {
        let line = self.line();
        self.advance(); // 'fn'
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LParen, "'(' after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if !self.match_tok(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "')' after parameter list")?;
        let body = self.parse_block()?;
        Ok(FunctionDecl {
            name,
            params,
            body,
            line,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, SfError> {
        self.expect(&TokenKind::LBrace, "'{'")?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_eof() {
            stmts.push(self.parse_statement()?);
        }
        self.expect(&TokenKind::RBrace, "'}'")?;
        Ok(stmts)
    }

    // ---------------------------------------------------------------- //
    // Statements
    // ---------------------------------------------------------------- //

    fn parse_statement(&mut self) -> Result<Stmt, SfError> {
        match &self.peek().kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::Print => self.parse_print(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, SfError> {
        let line = self.line();
        self.advance(); // 'let'
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Assign, "'=' in let binding")?;
        let value = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "';' after let binding")?;
        Ok(Stmt::Let(name, value, line))
    }

    fn parse_print(&mut self) -> Result<Stmt, SfError> {
        let line = self.line();
        self.advance(); // 'print'
        let mut args = vec![self.parse_expression()?];
        while self.match_tok(&TokenKind::Comma) {
            args.push(self.parse_expression()?);
        }
        self.expect(&TokenKind::Semicolon, "';' after print statement")?;
        Ok(Stmt::Print(args, line))
    }

    fn parse_if(&mut self) -> Result<Stmt, SfError> {
        self.advance(); // 'if'
        self.expect(&TokenKind::LParen, "'(' after if")?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "')' after if condition")?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.match_tok(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                vec![self.parse_if()?]
            } else {
                self.parse_block()?
            }
        } else {
            Vec::new()
        };
        Ok(Stmt::If(cond, then_branch, else_branch))
    }

    fn parse_while(&mut self) -> Result<Stmt, SfError> {
        self.advance(); // 'while'
        self.expect(&TokenKind::LParen, "'(' after while")?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "')' after while condition")?;
        let body = self.parse_block()?;
        Ok(Stmt::While(cond, body))
    }

    /// Desugars `for (init; cond; post) { body }` into
    /// `{ init; while (cond) { body; post; } }` so the compiler and
    /// interpreter only ever have to know about `While` and `Block`.
    fn parse_for(&mut self) -> Result<Stmt, SfError> {
        self.advance(); // 'for'
        self.expect(&TokenKind::LParen, "'(' after for")?;
        let init = if self.check(&TokenKind::Let) {
            self.parse_let()?
        } else {
            self.parse_expr_stmt()?
        };
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "';' after for condition")?;
        let post = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "')' after for clauses")?;
        let mut body = self.parse_block()?;
        body.push(Stmt::ExprStmt(post));
        Ok(Stmt::Block(vec![init, Stmt::While(cond, body)]))
    }

    fn parse_return(&mut self) -> Result<Stmt, SfError> {
        let line = self.line();
        self.advance(); // 'return'
        let value = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::Semicolon, "';' after return")?;
        Ok(Stmt::Return(value, line))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, SfError> {
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "';' after expression")?;
        Ok(Stmt::ExprStmt(expr))
    }

    // ---------------------------------------------------------------- //
    // Expressions
    // ---------------------------------------------------------------- //

    fn parse_expression(&mut self) -> Result<Expr, SfError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, SfError> {
        let line = self.line();
        let target = self.parse_expr_bp(1)?;
        if self.check(&TokenKind::Assign) {
            self.advance();
            let value = self.parse_assignment()?; // right-associative
            return match target {
                Expr::Ident(name, _) => Ok(Expr::Assign(name, Box::new(value), line)),
                Expr::Index(arr, idx, iline) => Ok(Expr::IndexAssign(arr, idx, Box::new(value), iline)),
                _ => Err(SfError::parse("invalid assignment target", line)),
            };
        }
        Ok(target)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, SfError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let kind = self.peek().kind.clone();
            let (l_bp, r_bp) = match infix_binding_power(&kind) {
                Some(bp) => bp,
                None => break,
            };
            if l_bp < min_bp {
                break;
            }
            let line = self.line();
            self.advance();
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = match kind {
                TokenKind::OrOr => Expr::Logical(LogicOp::Or, Box::new(lhs), Box::new(rhs)),
                TokenKind::AndAnd => Expr::Logical(LogicOp::And, Box::new(lhs), Box::new(rhs)),
                TokenKind::EqEq => Expr::Binary(BinOp::Eq, Box::new(lhs), Box::new(rhs), line),
                TokenKind::NotEq => Expr::Binary(BinOp::NotEq, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Lt => Expr::Binary(BinOp::Lt, Box::new(lhs), Box::new(rhs), line),
                TokenKind::LtEq => Expr::Binary(BinOp::LtEq, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Gt => Expr::Binary(BinOp::Gt, Box::new(lhs), Box::new(rhs), line),
                TokenKind::GtEq => Expr::Binary(BinOp::GtEq, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Plus => Expr::Binary(BinOp::Add, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Minus => Expr::Binary(BinOp::Sub, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Star => Expr::Binary(BinOp::Mul, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Slash => Expr::Binary(BinOp::Div, Box::new(lhs), Box::new(rhs), line),
                TokenKind::Percent => Expr::Binary(BinOp::Mod, Box::new(lhs), Box::new(rhs), line),
                _ => unreachable!("infix_binding_power and this match must stay in sync"),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, SfError> {
        let line = self.line();
        if self.match_tok(&TokenKind::Bang) {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary(UnOp::Not, Box::new(operand), line));
        }
        if self.match_tok(&TokenKind::Minus) {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary(UnOp::Neg, Box::new(operand), line));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, SfError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.check(&TokenKind::LBracket) {
                let line = self.line();
                self.advance();
                let index = self.parse_expression()?;
                self.expect(&TokenKind::RBracket, "']' after index expression")?;
                expr = Expr::Index(Box::new(expr), Box::new(index), line);
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, SfError> {
        let line = self.line();
        let tok = self.peek().kind.clone();
        match tok {
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(Expr::Nil)
            }
            TokenKind::Ident(name) => {
                self.advance();
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&TokenKind::RParen, "')' after call arguments")?;
                    Ok(Expr::Call(name, args, line))
                } else {
                    Ok(Expr::Ident(name, line))
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RParen, "')' after parenthesized expression")?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let items = self.parse_args()?;
                self.expect(&TokenKind::RBracket, "']' after array literal")?;
                Ok(Expr::Array(items))
            }
            TokenKind::LBrace => {
                self.advance();
                let mut entries = Vec::new();
                if !self.check(&TokenKind::RBrace) {
                    loop {
                        let key = match &self.peek().kind {
                            TokenKind::Str(s) => s.clone(),
                            other => {
                                return Err(SfError::parse(
                                    format!("expected string key in map literal, found {:?}", other),
                                    self.line(),
                                ))
                            }
                        };
                        self.advance();
                        self.expect(&TokenKind::Colon, "':' after map key")?;
                        let value = self.parse_expression()?;
                        entries.push((key, value));
                        if !self.match_tok(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBrace, "'}' after map literal")?;
                Ok(Expr::Map(entries))
            }
            other => Err(SfError::parse(format!("unexpected token {:?}", other), line)),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, SfError> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) && !self.check(&TokenKind::RBracket) {
            loop {
                args.push(self.parse_expression()?);
                if !self.match_tok(&TokenKind::Comma) {
                    break;
                }
            }
        }
        Ok(args)
    }
}

pub fn parse(src: &str) -> Result<Program, SfError> {
    let tokens = lex(src)?;
    Parser::new(tokens).parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arithmetic_precedence_correctly() {
        // 1 + 2 * 3 must parse as 1 + (2 * 3), not (1 + 2) * 3.
        let prog = parse("let x = 1 + 2 * 3;").unwrap();
        match &prog.main[0] {
            Stmt::Let(name, Expr::Binary(BinOp::Add, lhs, rhs, _), _) => {
                assert_eq!(name, "x");
                assert_eq!(**lhs, Expr::Number(1.0));
                assert_eq!(
                    **rhs,
                    Expr::Binary(
                        BinOp::Mul,
                        Box::new(Expr::Number(2.0)),
                        Box::new(Expr::Number(3.0)),
                        1
                    )
                );
            }
            other => panic!("unexpected AST: {:?}", other),
        }
    }

    #[test]
    fn parses_comparison_binding_looser_than_arithmetic() {
        let prog = parse("let x = 1 + 2 < 3 * 4;").unwrap();
        match &prog.main[0] {
            Stmt::Let(_, Expr::Binary(BinOp::Lt, lhs, rhs, _), _) => {
                assert_eq!(
                    **lhs,
                    Expr::Binary(
                        BinOp::Add,
                        Box::new(Expr::Number(1.0)),
                        Box::new(Expr::Number(2.0)),
                        1
                    )
                );
                assert_eq!(
                    **rhs,
                    Expr::Binary(
                        BinOp::Mul,
                        Box::new(Expr::Number(3.0)),
                        Box::new(Expr::Number(4.0)),
                        1
                    )
                );
            }
            other => panic!("unexpected AST: {:?}", other),
        }
    }

    #[test]
    fn parses_logical_and_looser_than_equality_but_tighter_than_or() {
        let prog = parse("let x = a == 1 || b == 2 && c == 3;").unwrap();
        // Expect: a==1 || (b==2 && c==3)
        match &prog.main[0] {
            Stmt::Let(_, Expr::Logical(LogicOp::Or, lhs, rhs), _) => {
                assert_eq!(
                    **lhs,
                    Expr::Binary(
                        BinOp::Eq,
                        Box::new(Expr::Ident("a".into(), 1)),
                        Box::new(Expr::Number(1.0)),
                        1
                    )
                );
                match &**rhs {
                    Expr::Logical(LogicOp::And, _, _) => {}
                    other => panic!("expected && on the right of ||, got {:?}", other),
                }
            }
            other => panic!("unexpected AST: {:?}", other),
        }
    }

    #[test]
    fn parses_right_associative_assignment_chain() {
        let prog = parse("y = x = 5;").unwrap();
        match &prog.main[0] {
            Stmt::ExprStmt(Expr::Assign(name, inner, _)) => {
                assert_eq!(name, "y");
                assert_eq!(**inner, Expr::Assign("x".into(), Box::new(Expr::Number(5.0)), 1));
            }
            other => panic!("unexpected AST: {:?}", other),
        }
    }

    #[test]
    fn parses_function_declaration_with_params_and_return() {
        let prog = parse("fn add(a, b) { return a + b; }").unwrap();
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.params, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(f.body.len(), 1);
    }

    #[test]
    fn parses_array_and_index_expressions() {
        let prog = parse("let a = [1, 2, 3]; let b = a[0];").unwrap();
        assert_eq!(
            prog.main[0],
            Stmt::Let(
                "a".into(),
                Expr::Array(vec![Expr::Number(1.0), Expr::Number(2.0), Expr::Number(3.0)]),
                1
            )
        );
        match &prog.main[1] {
            Stmt::Let(_, Expr::Index(arr, idx, _), _) => {
                assert_eq!(**arr, Expr::Ident("a".into(), 1));
                assert_eq!(**idx, Expr::Number(0.0));
            }
            other => panic!("unexpected AST: {:?}", other),
        }
    }

    #[test]
    fn for_loop_desugars_to_block_with_init_and_while() {
        let prog = parse("for (let i = 0; i < 3; i = i + 1) { print i; }").unwrap();
        match &prog.main[0] {
            Stmt::Block(stmts) => {
                assert_eq!(stmts.len(), 2);
                assert!(matches!(stmts[0], Stmt::Let(..)));
                match &stmts[1] {
                    Stmt::While(_, body) => assert_eq!(body.len(), 2), // print + post-increment
                    other => panic!("expected While, got {:?}", other),
                }
            }
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn missing_semicolon_reports_syntax_error_with_line_number() {
        let err = parse("let x = 1\nlet y = 2;").unwrap_err();
        match err {
            SfError::Parse { line, .. } => assert_eq!(line, 2),
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn unclosed_paren_reports_syntax_error() {
        let err = parse("let x = (1 + 2;").unwrap_err();
        assert!(matches!(err, SfError::Parse { .. }));
    }
}
