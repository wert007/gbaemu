use crate::plugins::debugger::commands::{Address, Command};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq)]
pub enum Token {
    Text(String),
    Int(u32),
    LBracket,
    RBracket,
    Error,
    Eof,
}

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
}
impl Parser {
    fn new(text: String) -> Self {
        let mut tokens = parse_tokens(text);
        tokens.push(Token::Eof);
        tokens.reverse();

        Self { tokens }
    }

    fn peek(&self) -> &Token {
        self.tokens.last().unwrap_or(&Token::Eof)
    }

    fn eat(&mut self) -> Token {
        self.tokens.pop().unwrap_or(Token::Eof)
    }
}

enum Ast {
    Literal(u32),
    MemoryRead(Box<Ast>),
}
impl Ast {
    fn expect_address(self) -> Result<Address, ()> {
        match self {
            Ast::Literal(u) => Ok(Address::Literal(u)),
            Ast::MemoryRead(ast) => {
                todo!()
            }
        }
    }
}

pub fn parse_command(text: String) -> Result<Command, ()> {
    let mut p = Parser::new(text);
    match p.peek() {
        Token::Text(c) => parse_named_command(&c.clone(), p),
        Token::Error => Err(()),
        Token::Eof => Ok(Command::Step),
        _ => {
            let ast = parse_expression(&mut p)?;
            match ast {
                Ast::Literal(u) => Ok(Command::Echo(u.to_string())),
                Ast::MemoryRead(ast) => {
                    let addr = ast.expect_address()?;
                    Ok(Command::ReadMemory(addr))
                }
            }
        }
    }
}

fn parse_expression(p: &mut Parser) -> Result<Ast, ()> {
    match p.eat() {
        Token::Int(u) => Ok(Ast::Literal(u)),
        Token::LBracket => {
            let addr = parse_expression(p)?;
            if p.eat() != Token::RBracket {
                return Err(());
            }
            Ok(Ast::MemoryRead(Box::new(addr)))
        }
        Token::Text(_) | Token::RBracket | Token::Error | Token::Eof => Err(()),
    }
}

fn parse_named_command(c: &str, p: Parser) -> Result<Command, ()> {
    match c {
        "b" | "back" => Ok(Command::GoBack),
        "c" | "cont" | "continue" => Ok(Command::Continue),
        "h" | "help" | "?" => Ok(Command::Help),
        "jr" | "jump-return" => Ok(Command::JumpOut),
        "r" | "reg" | "registers" => Ok(Command::ShowRegisters),
        "stackframe" | "sf" => Ok(Command::ShowStackFrame),
        "show-asm" => Ok(Command::StartEmittingAssembly),
        "hide-asm" => Ok(Command::StopEmittingAssembly),
        "n" | "next" | "step" | "s" => Ok(Command::Step),
        hmm => todo!("{hmm}"),
    }
}

pub fn parse_tokens(text: String) -> Vec<Token> {
    let mut result = Vec::new();
    let mut is_int = false;
    let mut is_hex = false;
    let mut is_text = false;
    let mut last_multi_char_token_start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '[' => {
                if is_text {
                    result.push(Token::Text(
                        text[last_multi_char_token_start..i].to_string(),
                    ));
                } else if is_int || is_hex {
                    let radix = if is_hex { 16 } else { 10 };
                    result.push(
                        u32::from_str_radix(
                            &text[last_multi_char_token_start..i].trim_start_matches("0x"),
                            radix,
                        )
                        .map(|u| Token::Int(u))
                        .unwrap_or(Token::Error),
                    );
                }
                result.push(Token::LBracket)
            }
            ']' => {
                if is_text {
                    result.push(Token::Text(
                        text[last_multi_char_token_start..i].to_string(),
                    ));
                } else if is_int || is_hex {
                    let radix = if is_hex { 16 } else { 10 };
                    result.push(
                        u32::from_str_radix(
                            &text[last_multi_char_token_start..i].trim_start_matches("0x"),
                            radix,
                        )
                        .map(|u| Token::Int(u))
                        .unwrap_or(Token::Error),
                    );
                }

                result.push(Token::RBracket)
            }
            ws if ws.is_whitespace() => {}
            d if !is_text && d.is_ascii_digit() => {
                if is_int || is_hex {
                } else {
                    last_multi_char_token_start = i;
                    is_int = true;
                }
            }
            d if is_text && d.is_ascii_digit() => {}
            '_' | '?' if is_text => {}
            'x' if !is_text && is_int && &text[last_multi_char_token_start..i + 1] == "0x" => {
                is_int = false;
                is_hex = true;
            }
            xd if xd.is_ascii_hexdigit() && is_hex => {}
            a if a.is_alphabetic() && !is_int && !is_hex
                || a == '-' && is_text
                || a == '?' && !is_text && !is_int && !is_hex =>
            {
                if is_text {
                } else {
                    last_multi_char_token_start = i;
                    is_text = true;
                }
            }
            c => {
                if is_text && c.is_whitespace() {
                    result.push(Token::Text(
                        text[last_multi_char_token_start..i].to_string(),
                    ));
                } else if is_int || is_hex {
                    let radix = if is_hex { 16 } else { 10 };
                    result.push(
                        u32::from_str_radix(&text[last_multi_char_token_start..i], radix)
                            .map(|u| Token::Int(u))
                            .unwrap_or(Token::Error),
                    );
                    match c {
                        '[' => result.push(Token::LBracket),
                        ']' => result.push(Token::RBracket),
                        ws if ws.is_whitespace() => {}
                        _ => todo!("{c}"),
                    }
                } else {
                    result.push(Token::Error);
                }
            }
        }
    }
    if is_text {
        result.push(Token::Text(text[last_multi_char_token_start..].to_string()));
    } else if is_int || is_hex {
        let radix = if is_hex { 16 } else { 10 };
        result.push(
            u32::from_str_radix(&text[last_multi_char_token_start..], radix)
                .map(|u| Token::Int(u))
                .unwrap_or(Token::Error),
        );
    }
    result
}
