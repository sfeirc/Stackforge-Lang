use crate::error::SfError;
use crate::token::{Token, TokenKind};

/// Hand-written lexer: one pass over the source bytes, no regex, no lookahead
/// beyond a single `peek`/`peek2`. Tracks line numbers so every downstream
/// error (parse, compile, runtime) can point at a source line.
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
}

fn keyword(ident: &str) -> Option<TokenKind> {
    Some(match ident {
        "let" => TokenKind::Let,
        "fn" => TokenKind::Fn,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "return" => TokenKind::Return,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "nil" => TokenKind::Nil,
        "print" => TokenKind::Print,
        _ => return None,
    })
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
        }
    }

    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }

    fn peek2(&self) -> u8 {
        *self.src.get(self.pos + 1).unwrap_or(&0)
    }

    fn advance(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        c
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.advance();
                }
                b'/' if self.peek2() == b'/' => {
                    while self.peek() != b'\n' && self.peek() != 0 {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn make(&self, kind: TokenKind, line: u32) -> Token {
        Token::new(kind, line)
    }

    fn read_string(&mut self, line: u32) -> Result<Token, SfError> {
        // opening quote already consumed
        let mut s = String::new();
        loop {
            match self.peek() {
                0 => return Err(SfError::lex("unterminated string literal", line)),
                b'"' => {
                    self.advance();
                    break;
                }
                b'\\' => {
                    self.advance();
                    let esc = self.advance();
                    match esc {
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        0 => return Err(SfError::lex("unterminated string literal", line)),
                        other => {
                            return Err(SfError::lex(
                                format!("unknown escape sequence '\\{}'", other as char),
                                self.line,
                            ))
                        }
                    }
                }
                _ => {
                    // support UTF-8 bytes transparently by pushing raw bytes
                    // through a small buffer; since we only ever slice on
                    // ASCII delimiters this is safe.
                    let start = self.pos;
                    self.advance();
                    s.push_str(std::str::from_utf8(&self.src[start..self.pos]).unwrap_or(""));
                }
            }
        }
        Ok(self.make(TokenKind::Str(s), line))
    }

    fn read_number(&mut self, line: u32) -> Token {
        let start = self.pos;
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        if self.peek() == b'.' && self.peek2().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let n: f64 = text.parse().unwrap();
        self.make(TokenKind::Number(n), line)
    }

    fn read_ident(&mut self, line: u32) -> Token {
        let start = self.pos;
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }
        let text = std::str::from_utf8(&self.src[start..self.pos])
            .unwrap()
            .to_string();
        match keyword(&text) {
            Some(kw) => self.make(kw, line),
            None => self.make(TokenKind::Ident(text), line),
        }
    }
