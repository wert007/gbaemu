use crate::{Compiler, HasLocation, Location, SourceTextId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    location: Location,
    pub kind: TokenKind,
    is_generated: bool,
}

impl HasLocation for Token {
    fn location(&self) -> Location {
        self.location
    }
}

impl Token {
    fn char(location: Location, ch: char) -> Token {
        let kind = match ch {
            '\0' => TokenKind::Eof,
            '=' => TokenKind::Equals,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            ',' => TokenKind::Comma,
            _ => TokenKind::Error,
        };
        Self {
            location,
            kind,
            is_generated: false,
        }
    }

    fn integer(location: Location) -> Token {
        Self {
            location,
            kind: TokenKind::Integer,
            is_generated: false,
        }
    }

    fn identifier(location: Location, lexeme: &str) -> Token {
        let kind = match lexeme {
            "const" => TokenKind::ConstKeyword,
            "true" => TokenKind::TrueKeyword,
            "false" => TokenKind::FalseKeyword,
            "comp" => TokenKind::CompKeyword,
            "fn" => TokenKind::FnKeyword,
            _ => TokenKind::Identifier,
        };
        Self {
            location,
            kind,
            is_generated: false,
        }
    }

    pub(crate) fn generate(kind: TokenKind, location: Location) -> Token {
        Self {
            location,
            kind,
            is_generated: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Eof,
    Error,
    ConstKeyword,
    TrueKeyword,
    FalseKeyword,
    Identifier,
    Equals,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Semicolon,
    Integer,
    LBracket,
    RBracket,
    Comma,
    Colon,
    LParen,
    RParen,
    CompKeyword,
    FnKeyword,
    LBrace,
    RBrace,
}
impl TokenKind {
    pub(crate) fn binary_precedence(&self) -> Option<(usize, usize)> {
        Some(match self {
            TokenKind::Plus => (10, 20),
            TokenKind::Minus => (10, 20),
            TokenKind::Star => (30, 40),
            TokenKind::Slash => (30, 40),
            TokenKind::Percent => (30, 40),
            _ => return None,
        })
    }
}

pub struct Lexer {
    file: SourceTextId,
    position: usize,
}

impl Lexer {
    pub fn new(file: SourceTextId) -> Self {
        Self { file, position: 0 }
    }

    pub fn lex(&mut self, compiler: &mut Compiler) -> Option<Token> {
        #[derive(Debug, Clone, Copy)]
        enum LexState {
            Init,
            Identifier,
            Integer,
            // Done(Option<Token>),
        }
        let mut lex_state = LexState::Init;

        let mut location = Location {
            span: crate::Span {
                start: self.position,
                len: 0,
            },
            file: self.file,
        };
        self.position += 1;
        loop {
            if self.position - 1 > compiler[self.file].len() {
                return None;
            }
            match (
                lex_state,
                compiler[self.file].content[self.position - 1..]
                    .chars()
                    .next(),
            ) {
                (LexState::Init, None) => {
                    break Some(Token::char(location.with_end_at(self.position), '\0'));
                }
                (LexState::Init, Some(ch @ ('=' | ';'))) => {
                    break Some(Token::char(location.with_end_at(self.position), ch));
                }

                (LexState::Init, Some(' ' | '\n' | '\t' | '\r')) => {
                    self.position += 1;
                    location.span.start += 1;
                }
                (LexState::Init | LexState::Integer, Some('0'..='9')) => {
                    self.position += 1;
                    lex_state = LexState::Integer;
                }
                (LexState::Integer, _) => {
                    self.position -= 1;
                    break Some(Token::integer(location.with_end_at(self.position)));
                }
                (LexState::Init | LexState::Identifier, Some('a'..='z' | 'A'..='Z' | '_'))
                | (LexState::Identifier, Some('0'..='9')) => {
                    self.position += 1;
                    lex_state = LexState::Identifier;
                }
                (LexState::Identifier, _) => {
                    self.position -= 1;
                    let location = location.with_end_at(self.position);
                    let lexeme = &compiler[location];
                    break Some(Token::identifier(location, lexeme));
                }
                (LexState::Init, Some(ch)) => {
                    let location = location.with_end_at(self.position);
                    compiler.diagnostics.report_invalid_char(location, ch);
                    break Some(Token::char(location, ch));
                }
            }
        }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }
}
