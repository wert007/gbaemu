use std::collections::VecDeque;

use crate::{
    Compiler, HasLocation, SourceTextId,
    lexer::{Lexer, Token, TokenKind},
    syntax_tree::{Parsed, SyntaxNode, SyntaxTree},
};

pub struct Parser {
    lexer: Lexer,
    buffer: VecDeque<Token>,
    expected: Vec<TokenKind>,
}

impl Parser {
    pub fn new(file: SourceTextId) -> Self {
        Parser {
            lexer: Lexer::new(file),
            buffer: VecDeque::new(),
            expected: Vec::new(),
        }
    }

    pub fn parse(&mut self, compiler: &mut Compiler) -> SyntaxTree<Parsed> {
        self.parse_program(compiler)
    }

    fn parse_program(&mut self, compiler: &mut Compiler) -> SyntaxTree<Parsed> {
        let statements = self.parse_until(TokenKind::Eof, compiler, |p, c| {
            p.parse_top_level_statement(c)
        });
        let eof = self.expect(TokenKind::Eof, compiler);
        SyntaxTree {
            node: SyntaxNode::<Parsed>::program(statements, eof),
        }
    }

    fn parse_until(
        &mut self,
        end: TokenKind,
        compiler: &mut Compiler,
        body: impl Fn(&mut Parser, &mut Compiler) -> SyntaxNode<Parsed>,
    ) -> Vec<SyntaxNode<Parsed>> {
        let mut result = Vec::new();
        self.expected.push(end);
        while self.peek(0, compiler) != end && self.peek(0, compiler) != TokenKind::Eof {
            result.push(body(self, compiler));
        }
        self.expected.pop();
        result
    }

    fn peek(&mut self, offset: usize, compiler: &mut Compiler) -> TokenKind {
        if offset >= self.buffer.len() {
            self.fill_buffer_until(offset, compiler);
        }
        self.buffer
            .get(offset)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn fill_buffer_until(&mut self, len: usize, compiler: &mut Compiler) {
        while self.buffer.len() <= len
            && let Some(token) = self.lexer.lex(compiler)
        {
            self.buffer.push_back(token);
        }
    }

    fn consume(&mut self, compiler: &mut Compiler) -> Token {
        self.fill_buffer_until(0, compiler);
        let token = self.buffer.pop_front().expect("Should be valid");
        self.expected.pop_if(|e| *e == token.kind);
        token
    }

    fn expect(&mut self, expected: TokenKind, compiler: &mut Compiler) -> Token {
        self.expected.pop_if(|e| *e == expected);
        self.fill_buffer_until(0, compiler);
        let top = self.buffer.front().unwrap();
        if top.kind == expected && !self.buffer.is_empty() {
            if top.kind == TokenKind::Eof {
                top.clone()
            } else {
                self.buffer.pop_front().unwrap()
            }
        } else {
            Token::generate(expected, top.location())
        }
    }

    fn parse_top_level_statement(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        match self.peek(0, compiler) {
            TokenKind::ConstKeyword => self.parse_const_declaration(compiler),
            _ => todo!("Error handling!"),
        }
    }

    fn parse_const_declaration(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let const_keyword = self.expect(TokenKind::ConstKeyword, compiler);
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let equals = self.expect(TokenKind::Equals, compiler);
        self.expected.push(TokenKind::Semicolon);
        let expression = self.parse_expression(compiler);
        let semicolon = self.expect(TokenKind::Semicolon, compiler);
        SyntaxNode::<Parsed>::const_declaration(
            const_keyword,
            identifier,
            equals,
            expression,
            semicolon,
        )
    }

    fn parse_expression(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        self.parse_binary(0, compiler)
    }

    fn parse_binary(
        &mut self,
        minimal_precedence: usize,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Parsed> {
        let mut expression = self.parse_expression_atom(compiler);
        while let Some((lhs, rhs)) = self.peek(0, compiler).binary_precedence() {
            if lhs < minimal_precedence {
                break;
            }
            let op = self.consume(compiler);
            let rhs = self.parse_binary(rhs, compiler);
            expression = SyntaxNode::<Parsed>::binary(expression, op, rhs);
        }

        expression
    }

    fn parse_literal(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        match self.peek(0, compiler) {
            TokenKind::Integer | TokenKind::TrueKeyword | TokenKind::FalseKeyword => {
                let literal = self.consume(compiler);
                SyntaxNode::<Parsed>::literal(literal)
            }
            _ => {
                todo!("Error handling!")
            }
        }
    }

    fn parse_variable(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let variable = self.expect(TokenKind::Identifier, compiler);
        SyntaxNode::<Parsed>::variable(variable)
    }

    fn parse_expression_atom(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        match self.peek(0, compiler) {
            TokenKind::Identifier => self.parse_variable(compiler),
            TokenKind::LBracket => self.parse_array_literal(compiler),
            _ => self.parse_literal(compiler),
        }
    }

    fn parse_array_literal(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let lbracket = self.expect(TokenKind::LBracket, compiler);
        let entries = self.parse_until(TokenKind::RBracket, compiler, |p, c| {
            let expression = p.parse_expression(c);
            let comma = if p.peek(0, c) == TokenKind::Comma {
                Some(p.consume(c))
            } else {
                None
            };
            SyntaxNode::<Parsed>::commaed_expression(expression, comma)
        });
        assert!(
            entries[..entries.len() - 1]
                .iter()
                .all(|e| e.kind.ends_with_comma()),
            "TODO: Error handling"
        );
        let rbracket = self.expect(TokenKind::RBracket, compiler);
        SyntaxNode::<Parsed>::array_literal(lbracket, entries, rbracket)
    }
}
