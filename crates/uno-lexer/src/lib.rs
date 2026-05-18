
use uno_syntax::span::Span;
use uno_syntax::token::{Token, TokenKind};

pub struct Lexer {
    source: String,
    start: usize,
    current: usize,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
        }
    }

    fn src(&self) -> &[u8] {
        self.source.as_bytes()
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            b'\0'
        } else {
            self.src()[self.current]
        }
    }

    fn peek_next(&self) -> u8 {
        if self.current + 1 >= self.source.len() {
            b'\0'
        } else {
            self.src()[self.current + 1]
        }
    }

    fn advance(&mut self) -> u8 {
        let c = self.src()[self.current];
        self.current += 1;
        c
    }

    fn emit(&self, kind: TokenKind) -> Token {
        Token::new(kind, Span::new(self.start, self.current))
    }

    fn emit_err(&self, msg: impl Into<String>) -> Token {
        Token::new(
            TokenKind::Error(msg.into()),
            Span::new(self.start, self.current),
        )
    }

    fn skip_line(&mut self) {
        while !self.is_at_end() && self.peek() != b'\n' {
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        loop {
            if self.is_at_end() {
                return;
            }
            if self.peek() == b'*' && self.peek_next() == b'/' {
                self.advance();
                self.advance();
                return;
            }
            self.advance();
        }
    }

    fn read_number(&mut self, first: u8) -> Token {
        if first == b'0' && self.peek() == b'x' {
            self.advance();
            while self.peek().is_ascii_hexdigit() {
                self.advance();
            }
            return self.emit(TokenKind::Integer(
                self.source[self.start..self.current].to_string(),
            ));
        }

        while self.peek().is_ascii_digit() {
            self.advance();
        }

        if self.peek() == b'_' {
            self.advance();
            let suffix_start = self.current;
            while self.peek().is_ascii_alphanumeric() {
                self.advance();
            }
            let suffix = &self.source[suffix_start..self.current];
            let num = self.source[self.start..suffix_start - 1].to_string();
            return match suffix {
                "u8" | "u16" | "u32" | "u64" | "u128" | "u256" => {
                    self.emit(TokenKind::Integer(num))
                }
                _ => self.emit_err(format!("unknown integer suffix '{}'", suffix)),
            };
        }

        self.emit(TokenKind::Felt(
            self.source[self.start..self.current].to_string(),
        ))
    }

    fn read_ident(&mut self) -> Token {
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].to_string();
        let kind = match text.as_str() {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "mut" => TokenKind::Mut,
            "pub" => TokenKind::Pub,
            "while" => TokenKind::While,
            "loop" => TokenKind::Loop,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "match" => TokenKind::Match,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "impl" => TokenKind::Impl,
            "use" => TokenKind::Use,
            "bool" => TokenKind::BoolType,
            "u8" => TokenKind::Uint(8),
            "u16" => TokenKind::Uint(16),
            "u32" => TokenKind::Uint(32),
            "u64" => TokenKind::Uint(64),
            "u128" => TokenKind::Uint(128),
            "u256" => TokenKind::Uint(256),
            _ => TokenKind::Ident(text),
        };
        self.emit(kind)
    }

    pub fn next_token(&mut self) -> Token {
        loop {
            self.start = self.current;

            if self.is_at_end() {
                return self.emit(TokenKind::Eof);
            }

            let c = self.advance();

            if c.is_ascii_whitespace() {
                if c == b'\n' {
                    continue;
                }
                continue;
            }

            if c.is_ascii_digit() {
                return self.read_number(c);
            }

            if c.is_ascii_alphabetic() || c == b'_' {
                return self.read_ident();
            }

            return match c {
                b'(' => self.emit(TokenKind::LParen),
                b')' => self.emit(TokenKind::RParen),
                b'{' => self.emit(TokenKind::LBrace),
                b'}' => self.emit(TokenKind::RBrace),
                b'[' => self.emit(TokenKind::LBracket),
                b']' => self.emit(TokenKind::RBracket),
                b',' => self.emit(TokenKind::Comma),
                b';' => self.emit(TokenKind::Semicolon),
                b':' => {
                    if self.peek() == b':' {
                        self.advance();
                        self.emit(TokenKind::Colon)
                    } else {
                        self.emit(TokenKind::Colon)
                    }
                }
                b'.' => self.emit(TokenKind::Dot),
                b'_' => self.emit(TokenKind::Underscore),
                b'#' => self.emit(TokenKind::Hash),

                b'+' => self.emit(TokenKind::Plus),
                b'-' => {
                    if self.peek() == b'>' {
                        self.advance();
                        self.emit(TokenKind::Arrow)
                    } else {
                        self.emit(TokenKind::Minus)
                    }
                }
                b'*' => self.emit(TokenKind::Star),
                b'/' => {
                    if self.peek() == b'/' {
                        self.skip_line();
                        continue;
                    }
                    if self.peek() == b'*' {
                        self.advance();
                        self.skip_block_comment();
                        continue;
                    }
                    self.emit(TokenKind::Slash)
                }
                b'%' => self.emit(TokenKind::Percent),

                b'=' => {
                    if self.peek() == b'=' {
                        self.advance();
                        self.emit(TokenKind::EqualsEquals)
                    } else {
                        self.emit(TokenKind::Equals)
                    }
                }
                b'!' => {
                    if self.peek() == b'=' {
                        self.advance();
                        self.emit(TokenKind::NotEquals)
                    } else {
                        self.emit(TokenKind::Not)
                    }
                }
                b'<' => {
                    if self.peek() == b'=' {
                        self.advance();
                        self.emit(TokenKind::LessEquals)
                    } else {
                        self.emit(TokenKind::Less)
                    }
                }
                b'>' => {
                    if self.peek() == b'=' {
                        self.advance();
                        self.emit(TokenKind::GreaterEquals)
                    } else {
                        self.emit(TokenKind::Greater)
                    }
                }
                b'&' => {
                    if self.peek() == b'&' {
                        self.advance();
                        self.emit(TokenKind::AndAnd)
                    } else {
                        self.emit_err("expected '&&'")
                    }
                }
                b'|' => {
                    if self.peek() == b'|' {
                        self.advance();
                        self.emit(TokenKind::OrOr)
                    } else {
                        self.emit_err("expected '||'")
                    }
                }

                _ => self.emit_err(format!("unexpected character '{}'", c as char)),
            };
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }
}
