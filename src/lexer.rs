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

    /// Tokenize the whole input, ending with a single `Eof` token.
    pub fn tokenize(mut self) -> Result<Vec<Token>, SfError> {
        let mut out = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let line = self.line;
            let c = self.peek();
            if c == 0 {
                out.push(self.make(TokenKind::Eof, line));
                break;
            }
            let tok = match c {
                b'(' => {
                    self.advance();
                    self.make(TokenKind::LParen, line)
                }
                b')' => {
                    self.advance();
                    self.make(TokenKind::RParen, line)
                }
                b'{' => {
                    self.advance();
                    self.make(TokenKind::LBrace, line)
                }
                b'}' => {
                    self.advance();
                    self.make(TokenKind::RBrace, line)
                }
                b'[' => {
                    self.advance();
                    self.make(TokenKind::LBracket, line)
                }
                b']' => {
                    self.advance();
                    self.make(TokenKind::RBracket, line)
                }
                b',' => {
                    self.advance();
                    self.make(TokenKind::Comma, line)
                }
                b';' => {
                    self.advance();
                    self.make(TokenKind::Semicolon, line)
                }
                b':' => {
                    self.advance();
                    self.make(TokenKind::Colon, line)
                }
                b'+' => {
                    self.advance();
                    self.make(TokenKind::Plus, line)
                }
                b'-' => {
                    self.advance();
                    self.make(TokenKind::Minus, line)
                }
                b'*' => {
                    self.advance();
                    self.make(TokenKind::Star, line)
                }
                b'/' => {
                    self.advance();
                    self.make(TokenKind::Slash, line)
                }
                b'%' => {
                    self.advance();
                    self.make(TokenKind::Percent, line)
                }
                b'=' => {
                    self.advance();
                    if self.peek() == b'=' {
                        self.advance();
                        self.make(TokenKind::EqEq, line)
                    } else {
                        self.make(TokenKind::Assign, line)
                    }
                }
                b'!' => {
                    self.advance();
                    if self.peek() == b'=' {
                        self.advance();
                        self.make(TokenKind::NotEq, line)
                    } else {
                        self.make(TokenKind::Bang, line)
                    }
                }
                b'<' => {
                    self.advance();
                    if self.peek() == b'=' {
                        self.advance();
                        self.make(TokenKind::LtEq, line)
                    } else {
                        self.make(TokenKind::Lt, line)
                    }
                }
                b'>' => {
                    self.advance();
                    if self.peek() == b'=' {
                        self.advance();
                        self.make(TokenKind::GtEq, line)
                    } else {
                        self.make(TokenKind::Gt, line)
                    }
                }
                b'&' => {
                    self.advance();
                    if self.peek() == b'&' {
                        self.advance();
                        self.make(TokenKind::AndAnd, line)
                    } else {
                        return Err(SfError::lex(
                            "unexpected character '&' (did you mean '&&'?)",
                            line,
                        ));
                    }
                }
                b'|' => {
                    self.advance();
                    if self.peek() == b'|' {
                        self.advance();
                        self.make(TokenKind::OrOr, line)
                    } else {
                        return Err(SfError::lex(
                            "unexpected character '|' (did you mean '||'?)",
                            line,
                        ));
                    }
                }
                b'"' => {
                    self.advance();
                    self.read_string(line)?
                }
                c if c.is_ascii_digit() => self.read_number(line),
                c if c.is_ascii_alphabetic() || c == b'_' => self.read_ident(line),
                other => {
                    return Err(SfError::lex(
                        format!("unexpected character '{}'", other as char),
                        line,
                    ))
                }
            };
            out.push(tok);
        }
        Ok(out)
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, SfError> {
    Lexer::new(src).tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_arithmetic_expression_with_correct_kinds() {
        let toks = lex("1 + 2 * (3 - 4) / 5;").unwrap();
        let kinds: Vec<TokenKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Number(1.0),
                TokenKind::Plus,
                TokenKind::Number(2.0),
                TokenKind::Star,
                TokenKind::LParen,
                TokenKind::Number(3.0),
                TokenKind::Minus,
                TokenKind::Number(4.0),
                TokenKind::RParen,
                TokenKind::Slash,
                TokenKind::Number(5.0),
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_keywords_and_identifiers_distinctly() {
        let toks = lex("let fib = fn;").unwrap();
        let kinds: Vec<TokenKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Let,
                TokenKind::Ident("fib".to_string()),
                TokenKind::Assign,
                TokenKind::Fn,
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_string_with_escapes() {
        let toks = lex("\"hello\\nworld\"").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Str("hello\nworld".to_string()));
    }

    #[test]
    fn tokenizes_comparison_and_logical_operators() {
        let toks = lex("a <= b && c != d || !e").unwrap();
        let kinds: Vec<TokenKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("a".into()),
                TokenKind::LtEq,
                TokenKind::Ident("b".into()),
                TokenKind::AndAnd,
                TokenKind::Ident("c".into()),
                TokenKind::NotEq,
                TokenKind::Ident("d".into()),
                TokenKind::OrOr,
                TokenKind::Bang,
                TokenKind::Ident("e".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn line_numbers_advance_across_newlines() {
        let toks = lex("let a = 1;\nlet b = 2;\nlet c = 3;").unwrap();
        // find token for 'c' (3rd let statement) -> should be on line 3
        let c_tok = toks
            .iter()
            .find(|t| t.kind == TokenKind::Ident("c".to_string()))
            .unwrap();
        assert_eq!(c_tok.line, 3);
    }

    #[test]
    fn comments_are_skipped() {
        let toks = lex("1 // this is a comment\n+ 2").unwrap();
        let kinds: Vec<TokenKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Number(1.0),
                TokenKind::Plus,
                TokenKind::Number(2.0),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn unterminated_string_reports_lex_error_with_line() {
        let err = lex("\"unterminated").unwrap_err();
        match err {
            SfError::Lex { line, .. } => assert_eq!(line, 1),
            _ => panic!("expected Lex error"),
        }
    }

    #[test]
    fn unexpected_character_reports_lex_error() {
        let err = lex("let x = 1 @ 2;").unwrap_err();
        assert!(matches!(err, SfError::Lex { .. }));
    }
}
