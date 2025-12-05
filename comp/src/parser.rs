use std::collections::VecDeque;

use crate::{
    Compiler, HasLocation, SourceTextId,
    lexer::{Lexer, Token, TokenKind},
    syntax_tree::{
        ArrayTypeIdentifier, FieldInitilizationNode, FunctionHeaderNode,
        GenericParameterHeaderNode, GenericParameterNode, NamedTypeIdentifier, ParameterNode,
        Parsed, SyntaxNode, SyntaxTree, TypeIdentifier,
    },
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

    fn parse_until<U>(
        &mut self,
        end: TokenKind,
        compiler: &mut Compiler,
        body: impl Fn(&mut Parser, &mut Compiler) -> U,
    ) -> Vec<U> {
        let mut result = Vec::new();
        self.expected.push(end);
        while self.peek(0, compiler) != end && self.peek(0, compiler) != TokenKind::Eof {
            let position = self.lexer.position();
            result.push(body(self, compiler));
            if position == self.lexer.position() {
                self.consume(compiler);
            }
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
            TokenKind::ImplKeyword => self.parse_impl_block(compiler),
            TokenKind::CompKeyword => match self.peek(1, compiler) {
                TokenKind::FnKeyword => self.parse_function_declaration(compiler),
                TokenKind::StructKeyword => self.parse_struct_declaration(compiler),
                _ => todo!("Error handling!"),
            },
            TokenKind::FnKeyword => self.parse_function_declaration(compiler),
            TokenKind::StructKeyword => self.parse_struct_declaration(compiler),
            TokenKind::Error => SyntaxNode::<Parsed>::error(self.current(compiler).location()),
            err => {
                let location = self.current(compiler).location();
                compiler
                    .diagnostics
                    .report_invalid_top_level_statement(location, err);
                SyntaxNode::<Parsed>::error(self.current(compiler).location())
            }
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
        let expression = self.parse_expression_atom(compiler);
        let mut expression = self.parse_function_call_or_field_access(expression, compiler);
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
        let location = self.current(compiler).location();
        match self.peek(0, compiler) {
            TokenKind::Integer | TokenKind::TrueKeyword | TokenKind::FalseKeyword => {
                let literal = self.consume(compiler);
                SyntaxNode::<Parsed>::literal(literal)
            }
            TokenKind::Error => SyntaxNode::<Parsed>::error(location),
            unexpected => {
                compiler
                    .diagnostics
                    .report_cannot_parse_as_expression(location, unexpected);
                SyntaxNode::<Parsed>::error(location)
            }
        }
    }

    fn parse_variable_or_struct_literal(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let variable = self.expect(TokenKind::Identifier, compiler);
        if self.peek(0, compiler) == TokenKind::LBrace {
            let identifier = variable;
            let lbrace = self.expect(TokenKind::LBrace, compiler);
            let fields = self.parse_until(TokenKind::RBrace, compiler, |p, c| {
                p.parse_field_initialization(c)
            });
            let rbrace = self.expect(TokenKind::RBrace, compiler);
            SyntaxNode::<Parsed>::struct_literal(identifier, lbrace, fields, rbrace)
        } else {
            SyntaxNode::<Parsed>::variable(variable)
        }
    }

    fn parse_expression_atom(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        match self.peek(0, compiler) {
            TokenKind::Identifier => self.parse_variable_or_struct_literal(compiler),
            TokenKind::LBracket => self.parse_array_literal(compiler),
            _ => self.parse_literal(compiler),
        }
    }

    fn parse_array_literal(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let lbracket = self.expect(TokenKind::LBracket, compiler);
        let entries = self.parse_until(TokenKind::RBracket, compiler, |p, c| {
            let expression = p.parse_expression(c);
            let comma = p.maybe_expect(TokenKind::Comma, c);
            SyntaxNode::<Parsed>::commaed_expression(expression, comma)
        });
        if let Some(entry_without_comma) = entries[..entries.len() - 1]
            .iter()
            .position(|e| !e.kind.ends_with_comma())
        {
            compiler
                .diagnostics
                .report_missing_comma(entries[entry_without_comma].location);
        }
        let rbracket = self.expect(TokenKind::RBracket, compiler);
        SyntaxNode::<Parsed>::array_literal(lbracket, entries, rbracket)
    }

    fn parse_function_declaration(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let comp_keyword = self.maybe_expect(TokenKind::CompKeyword, compiler);
        let fn_keyword = self.expect(TokenKind::FnKeyword, compiler);
        let name = self.expect(TokenKind::Identifier, compiler);
        let function_header = self.parse_function_header(compiler);
        let body = self.parse_block_expression(compiler);
        SyntaxNode::<Parsed>::function_declaration(
            comp_keyword,
            fn_keyword,
            name,
            function_header,
            body,
        )
    }

    fn parse_struct_declaration(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let comp_keyword = self.maybe_expect(TokenKind::CompKeyword, compiler);
        let struct_keyword = self.expect(TokenKind::StructKeyword, compiler);
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let lbrace = self.expect(TokenKind::LBrace, compiler);
        let fields = self.parse_until(TokenKind::RBrace, compiler, |p, c| p.parse_parameter(c));
        if !fields.is_empty()
            && let Some(index) = fields[..fields.len() - 1]
                .iter()
                .position(|e| !e.ends_with_comma())
        {
            compiler
                .diagnostics
                .report_missing_comma(fields[index].location);
        }

        let rbrace = self.expect(TokenKind::RBrace, compiler);

        SyntaxNode::<Parsed>::struct_declaration(
            comp_keyword,
            struct_keyword,
            identifier,
            lbrace,
            fields,
            rbrace,
        )
    }
    fn maybe_expect(&mut self, comma: TokenKind, compiler: &mut Compiler) -> Option<Token> {
        if self.peek(0, compiler) == comma {
            Some(self.consume(compiler))
        } else {
            None
        }
    }

    fn parse_function_header(&mut self, compiler: &mut Compiler) -> FunctionHeaderNode<Parsed> {
        let generics = if self.peek(0, compiler) == TokenKind::LessThan {
            let less_than = self.expect(TokenKind::LessThan, compiler);
            let generic_parameter = self.parse_until(TokenKind::GreaterThan, compiler, |p, c| {
                p.parse_generic_parameter(c)
            });
            let greater_than = self.expect(TokenKind::GreaterThan, compiler);
            Some(GenericParameterHeaderNode::<Parsed>::new(
                less_than,
                generic_parameter,
                greater_than,
            ))
        } else {
            None
        };
        let lparen = self.expect(TokenKind::LParen, compiler);
        let parameters = self.parse_until(TokenKind::RParen, compiler, |p, c| p.parse_parameter(c));
        let rparen = self.expect(TokenKind::RParen, compiler);
        if !parameters.is_empty()
            && let Some(index) = parameters[..parameters.len() - 1]
                .iter()
                .position(|e| e.ends_with_comma())
        {
            compiler
                .diagnostics
                .report_missing_comma(parameters[index].location);
        }
        let return_type = if self.peek(0, compiler) == TokenKind::Colon {
            let colon = self.expect(TokenKind::Colon, compiler);
            let type_identifier = self.parse_type_identifier(compiler);
            Some((colon, type_identifier))
        } else {
            None
        };
        FunctionHeaderNode::<Parsed>::new(generics, lparen, parameters, rparen, return_type)
    }

    fn parse_parameter(&mut self, compiler: &mut Compiler) -> ParameterNode<Parsed> {
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let colon = self.expect(TokenKind::Colon, compiler);
        let type_identifier = self.parse_type_identifier(compiler);
        let comma = self.maybe_expect(TokenKind::Comma, compiler);
        ParameterNode::<Parsed>::new(identifier, colon, type_identifier, comma)
    }

    fn parse_type_identifier(&mut self, compiler: &mut Compiler) -> TypeIdentifier {
        let location = self.current(compiler).location();
        let ampersand = self.maybe_expect(TokenKind::Ampersand, compiler);
        match self.peek(0, compiler) {
            TokenKind::Identifier => {
                let identifier = self.expect(TokenKind::Identifier, compiler);
                TypeIdentifier::Named(NamedTypeIdentifier {
                    ampersand,
                    identifier,
                })
            }
            TokenKind::LBracket => {
                let lbracket = self.expect(TokenKind::LBracket, compiler);
                let type_ = self.parse_type_identifier(compiler);
                let semicolon = self.expect(TokenKind::Semicolon, compiler);
                let length = self.parse_expression(compiler);
                let rbracket = self.expect(TokenKind::RBracket, compiler);
                TypeIdentifier::Array(ArrayTypeIdentifier {
                    lbracket,
                    type_: Box::new(type_),
                    semicolon,
                    length: Box::new(length),
                    rbracket,
                })
            }
            TokenKind::Error => TypeIdentifier::Error(location),
            unexpected => {
                compiler
                    .diagnostics
                    .report_invalid_type_identifier_format(location, unexpected);
                TypeIdentifier::Error(location)
            }
        }
    }

    fn parse_block_expression(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let lbrace = self.expect(TokenKind::LBrace, compiler);
        let body = self.parse_until(TokenKind::RBrace, compiler, |p, c| p.parse_statement(c));
        let rbrace = self.expect(TokenKind::RBrace, compiler);
        SyntaxNode::<Parsed>::block_expression(lbrace, body, rbrace)
    }

    fn parse_statement(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        match self.peek(0, compiler) {
            _ => self.parse_expression_statement(compiler),
        }
    }

    fn parse_expression_statement(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let expression = self.parse_expression(compiler);
        if self.peek(0, compiler) == TokenKind::Equals {
            let equals = self.expect(TokenKind::Equals, compiler);
            let value = self.parse_expression(compiler);
            let semicolon = self.expect(TokenKind::Semicolon, compiler);
            SyntaxNode::<Parsed>::assignment_statement(expression, equals, value, semicolon)
        } else {
            let semicolon = self.maybe_expect(TokenKind::Semicolon, compiler);
            SyntaxNode::<Parsed>::expression_statement(expression, semicolon)
        }
    }

    fn parse_function_call_or_field_access(
        &mut self,
        expression: SyntaxNode<Parsed>,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Parsed> {
        let mut base = expression;
        while [TokenKind::LParen, TokenKind::Period].contains(&self.peek(0, compiler)) {
            match self.peek(0, compiler) {
                TokenKind::LParen => {
                    let lparen = self.expect(TokenKind::LParen, compiler);
                    let arguments = self.parse_until(TokenKind::RParen, compiler, |p, c| {
                        let argument = p.parse_expression(c);
                        let comma = p.maybe_expect(TokenKind::Comma, c);
                        SyntaxNode::<Parsed>::commaed_expression(argument, comma)
                    });
                    if !arguments.is_empty()
                        && let Some(index) = arguments[..arguments.len() - 1]
                            .iter()
                            .position(|e| !e.kind.ends_with_comma())
                    {
                        compiler
                            .diagnostics
                            .report_missing_comma(arguments[index].location);
                    }

                    let rparen = self.expect(TokenKind::RParen, compiler);
                    base = SyntaxNode::<Parsed>::function_call(base, lparen, arguments, rparen);
                }
                TokenKind::Period => {
                    let period = self.expect(TokenKind::Period, compiler);
                    let field = self.expect(TokenKind::Identifier, compiler);
                    base = SyntaxNode::<Parsed>::field_access(base, period, field);
                }
                _ => unreachable!(),
            }
        }
        base
    }

    fn parse_generic_parameter(&mut self, compiler: &mut Compiler) -> GenericParameterNode<Parsed> {
        let out = self.maybe_identifier("out", compiler);
        if let Some(out) = &out
            && [
                TokenKind::Colon,
                TokenKind::Comma,
                self.expected.last().copied().unwrap_or(TokenKind::Eof),
            ]
            .contains(&self.peek(0, compiler))
        {
            compiler
                .diagnostics
                .report_cannot_use_reserved_keyword(out.location(), "out");
        }
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let type_ = if self.peek(0, compiler) == TokenKind::Colon {
            let colon = self.expect(TokenKind::Colon, compiler);
            let type_ = self.parse_type_identifier(compiler);
            Some((colon, type_))
        } else {
            None
        };
        let comma = self.maybe_expect(TokenKind::Comma, compiler);

        let has_type = type_.is_some();
        let result = GenericParameterNode::<Parsed>::new(out, identifier, type_, comma);
        if !has_type && out.is_some() {
            compiler
                .diagnostics
                .report_cannot_declare_type_generics_as_out(result.location);
        }
        result
    }

    fn maybe_identifier(&mut self, lexeme: &str, compiler: &mut Compiler) -> Option<Token> {
        if self.peek(0, compiler) == TokenKind::Identifier
            && &compiler[self.buffer[0].location()] == lexeme
        {
            Some(self.consume(compiler))
        } else {
            None
        }
    }

    fn current(&mut self, compiler: &mut Compiler) -> &Token {
        self.peek(0, compiler);
        &self.buffer[0]
    }

    fn parse_field_initialization(
        &mut self,
        compiler: &mut Compiler,
    ) -> FieldInitilizationNode<Parsed> {
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let colon = self.expect(TokenKind::Colon, compiler);
        let expression = self.parse_expression(compiler);
        let comma = self.maybe_expect(TokenKind::Comma, compiler);
        FieldInitilizationNode::<Parsed>::new(identifier, colon, expression, comma)
    }

    fn parse_impl_block(&mut self, compiler: &mut Compiler) -> SyntaxNode<Parsed> {
        let impl_keyword = self.expect(TokenKind::ImplKeyword, compiler);
        let identifier = self.expect(TokenKind::Identifier, compiler);
        let lbrace = self.expect(TokenKind::LBrace, compiler);
        let body = self.parse_until(TokenKind::RBrace, compiler, |p, c| {
            p.parse_function_declaration(c)
        });
        let rbrace = self.expect(TokenKind::RBrace, compiler);
        SyntaxNode::<Parsed>::impl_block(impl_keyword, identifier, lbrace, body, rbrace)
    }
}
