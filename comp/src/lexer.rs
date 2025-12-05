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
            '.' => TokenKind::Period,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Asterisk,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '&' => TokenKind::Ampersand,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '<' => TokenKind::LessThan,
            '>' => TokenKind::GreaterThan,
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

    fn multi_char(location: Location, cur: &[char]) -> Token {
        todo!()
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
            "struct" => TokenKind::StructKeyword,
            "impl" => TokenKind::ImplKeyword,
            "this" => TokenKind::ThisKeyword,
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
    Asterisk,
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
    StructKeyword,
    LBrace,
    RBrace,
    LessThan,
    GreaterThan,
    Period,
    ImplKeyword,
    ThisKeyword,
    Ampersand,
}
impl TokenKind {
    pub(crate) fn binary_precedence(&self) -> Option<(usize, usize)> {
        Some(match self {
            TokenKind::Plus => (10, 20),
            TokenKind::Minus => (10, 20),
            TokenKind::Asterisk => (30, 40),
            TokenKind::Slash => (30, 40),
            TokenKind::Percent => (30, 40),
            _ => return None,
        })
    }

    pub fn diagnostic_name(&self) -> &'static str {
        match self {
            TokenKind::Eof => "end of file",
            TokenKind::Error => "invalid character",
            TokenKind::ConstKeyword => "const keyword",
            TokenKind::TrueKeyword => "true keyword",
            TokenKind::FalseKeyword => "false keyword",
            TokenKind::Identifier => "identifier",
            TokenKind::Equals => "=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Asterisk => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Semicolon => ";",
            TokenKind::Integer => "integer",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Period => ".",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::CompKeyword => "comp keyword",
            TokenKind::FnKeyword => "fn keyword",
            TokenKind::StructKeyword => "struct keyword",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LessThan => "<",
            TokenKind::GreaterThan => ">",
            TokenKind::ImplKeyword => "impl keyword",
            TokenKind::ThisKeyword => "this keyword",
            TokenKind::Ampersand => "&",
        }
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
            Comment,
            MultiCharStart(char),
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
        let result = loop {
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
                (LexState::Init, next @ Some(ch)) | (LexState::MultiCharStart(ch), next @ None) => {
                    if next.is_none() || followed_by(ch).is_empty() {
                        let location = location.with_end_at(self.position);
                        let token = Token::char(location, ch);
                        if token.kind == TokenKind::Error {
                            compiler.diagnostics.report_invalid_char(location, ch);
                        }
                        break Some(token);
                    } else {
                        lex_state = LexState::MultiCharStart(ch);
                        self.position += 1;
                    }
                }
                (LexState::MultiCharStart(prev), Some(cur)) => {
                    self.position += 1;
                    if prev == '/' && cur == '/' {
                        lex_state = LexState::Comment;
                        continue;
                    }
                    let location = location.with_end_at(self.position);
                    let token = Token::multi_char(location, &[prev, cur]);
                    if token.kind == TokenKind::Error {
                        compiler.diagnostics.report_invalid_char(location, cur);
                    }
                    break Some(token);
                }
                (LexState::Comment, None | Some('\n')) => {
                    // TODO: Do not loose comments in the future!
                    lex_state = LexState::Init;
                }
                (LexState::Comment, Some(_)) => {
                    location.span.start = self.position;
                    self.position += 1;
                }
            }
        };
        // dbg!(result);
        result
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }
}

fn followed_by(ch: char) -> &'static [char] {
    match ch {
        '/' => &['/'],
        _ => &[],
    }
}
