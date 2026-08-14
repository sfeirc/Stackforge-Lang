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
