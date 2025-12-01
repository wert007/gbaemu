use crate::{Compiler, Location, SourceTextId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    location: Location,
    kind: TokenKind,
}
impl Token {
    fn char(location: Location, ch: char) -> Token {
        let kind = match ch {
            '\0' => TokenKind::Eof,
            '=' => TokenKind::Equals,
            ';' => TokenKind::Semicolon,
            _ => unreachable!(),
        };
        Self { location, kind }
    }

    fn integer(location: Location) -> Token {
        Self {
            location,
            kind: TokenKind::Integer,
        }
    }

    fn identifier(location: Location, lexeme: &str) -> Token {
        let kind = match lexeme {
            "const" => TokenKind::ConstKeyword,
            _ => TokenKind::Identifier,
        };
        Self { location, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    ConstKeyword,
    Identifier,
    Equals,
    Semicolon,
    Integer,
    Eof,
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
                _ => todo!(),
            }
        }
    }
}
