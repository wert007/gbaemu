use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

use crate::{
    plugins::debugger::commands::{Address, Command},
    registers::{RegisterIndex, Registers},
};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq)]
pub enum Token {
    Text(String),
    Int(u32),
    Register(RegisterIndex),
    Equals,
    LBracket,
    RBracket,
    Error,
    Eof,
}
impl Token {
    fn text(t: &str) -> Token {
        if let Ok(r) = RegisterIndex::try_from(t.to_uppercase().as_str()) {
            Token::Register(r)
        } else {
            Token::Text(t.to_string())
        }
    }

    fn as_u32(&self, registers: Registers) -> Option<u32> {
        match self {
            Token::Int(u) => Some(*u),
            Token::Register(r) => Some(registers.read(*r)),
            _ => None,
        }
    }
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
    Register(RegisterIndex),
    MemoryRead(Box<Ast>),
}
impl Ast {
    fn expect_address(self) -> Result<Address, ()> {
        match self {
            Ast::Literal(u) => Ok(Address::Literal(u)),
            Ast::Register(r) => Ok(Address::Register(r)),
            Ast::MemoryRead(ast) => Ok(Address::Indirect(Box::new(ast.expect_address()?))),
        }
    }
}

pub fn parse_command(text: String, registers: Registers) -> Result<Command, ()> {
    let mut p = Parser::new(text);
    match p.peek() {
        Token::Text(c) => parse_named_command(&c.clone(), p),
        Token::Error => Err(()),
        Token::Eof => Ok(Command::Step),
        _ => {
            let ast = parse_expression(&mut p)?;
            match ast {
                Ast::Literal(u) => Ok(Command::Echo(u.to_string())),
                Ast::Register(r) => Ok(Command::Echo(registers.read(r).to_string())),
                Ast::MemoryRead(ast) => {
                    let addr = ast.expect_address()?;
                    if p.peek() == &Token::Equals {
                        p.eat();
                        let Some(v) = p.eat().as_u32(registers) else {
                            return Err(());
                        };
                        Ok(Command::WriteMemory(addr, v))
                    } else {
                        Ok(Command::ReadMemory(addr))
                    }
                }
            }
        }
    }
}

fn parse_expression(p: &mut Parser) -> Result<Ast, ()> {
    match p.eat() {
        Token::Int(u) => Ok(Ast::Literal(u)),
        Token::Register(r) => Ok(Ast::Register(r)),
        Token::LBracket => {
            let addr = parse_expression(p)?;
            if p.eat() != Token::RBracket {
                return Err(());
            }
            Ok(Ast::MemoryRead(Box::new(addr)))
        }
        Token::Text(_) | Token::RBracket | Token::Equals | Token::Error | Token::Eof => Err(()),
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
        _ => Err(()),
    }
}

pub fn parse_tokens(text: String) -> Vec<Token> {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, strum::EnumIs)]
    enum State {
        Int,
        Hex,
        Text,
        None,
    }
    let result = RefCell::new(Vec::new());
    let state = RefCell::new(State::None);
    let last_multi_char_token_start = AtomicUsize::new(0);
    let finish = |i| {
        let last_multi_char_token_start =
            last_multi_char_token_start.load(std::sync::atomic::Ordering::SeqCst);
        if state.borrow().is_text() {
            result
                .borrow_mut()
                .push(Token::text(&text[last_multi_char_token_start..i]));
        } else if state.borrow().is_int() || state.borrow().is_hex() {
            let radix = if state.borrow().is_hex() { 16 } else { 10 };
            result.borrow_mut().push(
                u32::from_str_radix(
                    &text[last_multi_char_token_start..i].trim_start_matches("0x"),
                    radix,
                )
                .map(|u| Token::Int(u))
                .unwrap_or(Token::Error),
            );
        } else {
            *state.borrow_mut() = State::None;

            return false;
        }
        *state.borrow_mut() = State::None;

        true
    };
    let is_int = || state.borrow().is_int();
    let is_hex = || state.borrow().is_hex();
    for (i, ch) in text.char_indices() {
        match ch {
            '[' => {
                finish(i);
                result.borrow_mut().push(Token::LBracket)
            }
            ']' => {
                finish(i);
                result.borrow_mut().push(Token::RBracket)
            }
            '=' => {
                finish(i);
                result.borrow_mut().push(Token::Equals)
            }
            ws if ws.is_whitespace() => {}
            d if !state.borrow().is_text() && d.is_ascii_digit() => {
                if is_int() || is_hex() {
                } else {
                    last_multi_char_token_start.store(i, SeqCst);
                    *state.borrow_mut() = State::Int;
                }
            }
            d if state.borrow().is_text() && d.is_ascii_digit() => {}
            '_' | '?' if state.borrow().is_text() => {}
            'x' if !state.borrow().is_text()
                && is_int()
                && &text[last_multi_char_token_start.load(std::sync::atomic::Ordering::SeqCst)
                    ..i + 1]
                    == "0x" =>
            {
                *state.borrow_mut() = State::Hex;
            }
            xd if xd.is_ascii_hexdigit() && is_hex() => {}
            a if a.is_alphabetic() && !is_int() && !is_hex()
                || a == '-' && state.borrow().is_text()
                || a == '?' && !state.borrow().is_text() && !is_int() && !is_hex() =>
            {
                if state.borrow().is_text() {
                } else {
                    last_multi_char_token_start.store(i, SeqCst);
                    *state.borrow_mut() = State::Text;
                }
            }
            c => {
                if !finish(i) {
                    result.borrow_mut().push(Token::Error);
                }
            }
        }
    }
    finish(text.len());
    result.take()
}
