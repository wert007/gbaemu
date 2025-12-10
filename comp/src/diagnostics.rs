use std::fmt::Display;

use crate::{
    Location, SourceText, StringId, StringInterner,
    bind::BoundBinaryOperator,
    diagnostics::formatting::{BulletList, DiagnosticMessageComponent, Namespace, Parameter},
    lexer::TokenKind,
    typing::{TypeId, Types},
};

macro_rules! dgnst {
    ($($part:expr),*$(,)?) => {
        crate::diagnostics::DiagnosticMessage::Format(vec![$(Box::new($part),)*])
    };
}

mod formatting;

pub enum DiagnosticMessage {
    String(String),
    Format(Vec<Box<dyn DiagnosticMessageComponent>>),
}

pub struct DiagnosticMessageDisplay<'a, 'b, 'c, 'd> {
    types: &'a Types,
    strings: &'b StringInterner,
    source_texts: &'c SourceText,
    message: &'d DiagnosticMessage,
}

impl Display for DiagnosticMessageDisplay<'_, '_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.message {
            DiagnosticMessage::String(string) => write!(f, "{string}"),
            DiagnosticMessage::Format(format) => {
                for part in format {
                    part.fmt(f, self.types, self.strings, self.source_texts)?;
                }
                Ok(())
            }
        }
    }
}

impl DiagnosticMessage {
    fn display<'a, 'b, 'c, 'd>(
        &'d self,
        types: &'a Types,
        strings: &'b StringInterner,
        source_texts: &'c SourceText,
    ) -> DiagnosticMessageDisplay<'a, 'b, 'c, 'd> {
        DiagnosticMessageDisplay {
            types,
            strings,
            source_texts,
            message: self,
        }
    }
}

impl From<&str> for DiagnosticMessage {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}
impl From<String> for DiagnosticMessage {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

pub struct Diagnostic {
    location: Location,
    message: DiagnosticMessage,
}
impl Diagnostic {
    fn invalid_char(location: Location, ch: char) -> Diagnostic {
        Self {
            location,
            message: format!("Unexpected char {ch} in input!").into(),
        }
    }

    fn missing_comma(location: Location) -> Diagnostic {
        Self {
            location,
            message: "There is a comma missing here.".into(),
        }
    }

    fn cannot_declare_type_generics_as_out(location: Location) -> Diagnostic {
        Self {
            location,
            message: "Generic cannot be marked as out, since it has no type set.".into(),
        }
    }

    fn cannot_use_reserved_keyword(location: Location, keyword: &str) -> Diagnostic {
        Self {
            location,
            message: format!("Cannot use keyword `{keyword}` here.").into(),
        }
    }

    fn invalid_top_level_statement(location: Location, found: TokenKind) -> Diagnostic {
        Self {
            location,
            message: format!(
                "Only const declarations and function definitions are valid here. Found a/an {} instead.",
                found.diagnostic_name()
            ).into()
        }
    }

    fn cannot_parse_as_expression(location: Location, unexpected: TokenKind) -> Diagnostic {
        Self {
            location,
            message: format!(
                "Cannot parse a/an {} as an expression.",
                unexpected.diagnostic_name()
            )
            .into(),
        }
    }

    fn invalid_type_identifier_format(location: Location, unexpected: TokenKind) -> Diagnostic {
        Self {
            location,
            message: format!(
                "Type identifiers must be either written as `typeName` or as `[typeName; NUMBER]`, found a/an {} instead.",
                unexpected.diagnostic_name()
            ).into(),
        }
    }

    fn cannot_find_type(location: Location, name: StringId) -> Diagnostic {
        Self {
            location,
            message: dgnst!("No type named ", name, " could be found.",),
        }
    }

    fn non_const_value_in_type(location: Location) -> Diagnostic {
        Self {
            location,
            message: "Expression did not evaluate at compile time. Is it not const?".into(),
        }
    }

    fn cannot_find_variable_by_name(
        location: Location,
        namespaces: &[StringId],
        name: StringId,
    ) -> Diagnostic {
        if namespaces.is_empty() {
            Self {
                location,
                message: dgnst!(
                    "No variable named ",
                    name,
                    " could be found in the current scope."
                ),
            }
        } else {
            Self {
                location,
                message: dgnst!(
                    "No variable named ",
                    name,
                    " in this scope ",
                    Namespace(namespaces.into())
                ),
            }
        }
    }

    fn cannot_redeclare_variable(location: Location) -> Diagnostic {
        Self {
            location,
            message:
                "Variable with this name has been already declared and cannot be declared again."
                    .into(),
        }
    }

    fn previous_declaration_at(location: Location) -> Diagnostic {
        Self {
            location,
            message: "The previous conflicting declaration was here.".into(),
        }
    }

    fn non_const_value_in_const_declaration(location: Location) -> Diagnostic {
        Self {
            location,
            message: "The value cannot be computed at compile time and can therefore not be used in a const declaration.".into()
        }
    }

    fn cannot_parse_integer_literal_to(location: Location, expected: TypeId) -> Diagnostic {
        Self {
            location,
            message: dgnst!("Value is not a valid representation for ", expected),
        }
    }

    fn cannot_convert(location: Location, from: TypeId, to: TypeId) -> Diagnostic {
        Self {
            location,
            message: dgnst!("Cannot implicitly convert from ", from, " to ", to),
        }
    }

    fn invalid_binary_operation(
        location: Location,
        lhs_type: TypeId,
        op: BoundBinaryOperator,
        rhs_type: TypeId,
    ) -> Diagnostic {
        Self {
            location,
            message: dgnst!(
                "Cannot ",
                op.diagnostic_name(),
                " a/an ",
                lhs_type,
                " and a/an ",
                rhs_type
            ),
        }
    }

    fn cannot_find_field_with_this_name(
        location: Location,
        base: TypeId,
        field: StringId,
    ) -> Diagnostic {
        Self {
            location,
            message: dgnst!("No field named ", field, " found on ", base, "."),
        }
    }

    fn invalid_function_type(location: Location, type_: TypeId) -> Diagnostic {
        Self {
            location,
            message: dgnst!("Cannot call ", type_, " since it is not a function."),
        }
    }

    fn remove_semicolon(location: Location, expected: TypeId) -> Diagnostic {
        Self {
            location,
            message: dgnst!(
                "Hint: remove this semicolon, so this expression value is of type ",
                expected
            ),
        }
    }

    fn this_can_only_be_used_in_impl_block(location: Location) -> Diagnostic {
        Self {
            location,
            message: "this keyword can only be used for function that are inside of an impl block."
                .into(),
        }
    }

    fn missing_fields_in_struct_initialisation(
        location: Location,
        type_: TypeId,
        missing_fields: Vec<Parameter>,
    ) -> Diagnostic {
        Self {
            location,
            message: dgnst!(
                "Not all fields of ",
                type_,
                " has been initialised. The following fields are missing:",
                BulletList(location, missing_fields)
            ),
        }
    }

    fn definition_at(location: Location) -> Diagnostic {
        Self {
            location,
            message: dgnst!("Struct defined here."),
        }
    }

    fn unknown_fields_in_struct_initialisation(
        location: Location,
        type_: TypeId,
        too_much: Vec<StringId>,
    ) -> Diagnostic {
        if too_much.len() == 1 {
            Self {
                location,
                message: dgnst!(
                    "There is no field named ",
                    too_much[0],
                    " on type ",
                    type_,
                    "."
                ),
            }
        } else {
            Self {
                location,
                message: dgnst!(
                    "The following fields to not exist in ",
                    type_,
                    " and can therefore not be initialised.",
                    BulletList(location, too_much)
                ),
            }
        }
    }
}

pub struct Diagnostics {
    diagnostics: Vec<Diagnostic>,
}
impl Diagnostics {
    pub fn empty() -> Diagnostics {
        Diagnostics {
            diagnostics: Vec::new(),
        }
    }

    pub fn report_invalid_char(&mut self, location: Location, ch: char) {
        self.diagnostics
            .push(Diagnostic::invalid_char(location, ch));
    }

    pub fn report_missing_comma(&mut self, location: Location) {
        self.diagnostics.push(Diagnostic::missing_comma(location));
    }

    pub fn report_cannot_declare_type_generics_as_out(&mut self, location: Location) {
        self.diagnostics
            .push(Diagnostic::cannot_declare_type_generics_as_out(location));
    }

    pub fn report_cannot_use_reserved_keyword(&mut self, location: Location, keyword: &str) {
        self.diagnostics
            .push(Diagnostic::cannot_use_reserved_keyword(location, keyword));
    }

    pub fn report_invalid_top_level_statement(&mut self, location: Location, found: TokenKind) {
        self.diagnostics
            .push(Diagnostic::invalid_top_level_statement(location, found));
    }

    pub fn report_cannot_parse_as_expression(&mut self, location: Location, unexpected: TokenKind) {
        self.diagnostics
            .push(Diagnostic::cannot_parse_as_expression(location, unexpected));
    }

    pub fn report_invalid_type_identifier_format(
        &mut self,
        location: Location,
        unexpected: TokenKind,
    ) {
        self.diagnostics
            .push(Diagnostic::invalid_type_identifier_format(
                location, unexpected,
            ));
    }

    pub fn report_cannot_find_type(&mut self, location: Location, name: StringId) {
        self.diagnostics
            .push(Diagnostic::cannot_find_type(location, name))
    }

    pub fn report_non_const_value_in_type(&mut self, location: Location) {
        self.diagnostics
            .push(Diagnostic::non_const_value_in_type(location));
    }

    pub fn report_non_const_value_in_const_declaration(&mut self, location: Location) {
        self.diagnostics
            .push(Diagnostic::non_const_value_in_const_declaration(location));
    }

    pub fn report_cannot_find_variable_by_name(
        &mut self,
        location: Location,
        namespaces: &[StringId],
        identifier: StringId,
    ) {
        self.diagnostics
            .push(Diagnostic::cannot_find_variable_by_name(
                location, namespaces, identifier,
            ));
    }

    pub fn report_cannot_redeclare_variable(&mut self, location: Location, previous: Location) {
        self.diagnostics
            .push(Diagnostic::cannot_redeclare_variable(location));
        self.diagnostics
            .push(Diagnostic::previous_declaration_at(previous));
    }

    pub fn write_to(
        &self,
        out: &mut impl std::io::Write,
        files: &SourceText,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> Result<(), std::io::Error> {
        for diagnostic in &self.diagnostics {
            let m: DiagnosticMessageDisplay =
                diagnostic.message.display(&types, &strings, source_texts);
            writeln!(
                out,
                "[{}:{}:{}] {m}",
                files.file_name(diagnostic.location),
                files.line_number(diagnostic.location),
                files.column(diagnostic.location),
            )?;
        }
        Ok(())
    }

    pub fn report_cannot_parse_integer_literal_to(&mut self, location: Location, expected: TypeId) {
        self.diagnostics
            .push(Diagnostic::cannot_parse_integer_literal_to(
                location, expected,
            ));
    }

    pub fn report_cannot_convert(&mut self, location: Location, from: TypeId, to: TypeId) {
        self.diagnostics
            .push(Diagnostic::cannot_convert(location, from, to));
    }

    pub fn report_invalid_binary_operation(
        &mut self,
        location: Location,
        lhs_type: TypeId,
        op: BoundBinaryOperator,
        rhs_type: TypeId,
    ) {
        self.diagnostics.push(Diagnostic::invalid_binary_operation(
            location, lhs_type, op, rhs_type,
        ));
    }

    pub(crate) fn report_cannot_find_field_with_this_name(
        &mut self,
        location: Location,
        base: TypeId,
        field: StringId,
    ) {
        self.diagnostics
            .push(Diagnostic::cannot_find_field_with_this_name(
                location, base, field,
            ));
    }

    pub(crate) fn report_invalid_function_type(&mut self, location: Location, type_: TypeId) {
        self.diagnostics
            .push(Diagnostic::invalid_function_type(location, type_))
    }

    pub(crate) fn hint_remove_semicolon(&mut self, location: Location, expected: TypeId) {
        self.diagnostics
            .push(Diagnostic::remove_semicolon(location, expected));
    }

    pub(crate) fn report_this_can_only_be_used_in_impl_block(&mut self, location: Location) {
        self.diagnostics
            .push(Diagnostic::this_can_only_be_used_in_impl_block(location));
    }

    pub(crate) fn report_missing_fields_in_struct_initialisation(
        &mut self,
        location: Location,
        type_: TypeId,
        missing_fields: Vec<(StringId, TypeId)>,
        definition: Location,
    ) {
        self.diagnostics
            .push(Diagnostic::missing_fields_in_struct_initialisation(
                location,
                type_,
                missing_fields
                    .into_iter()
                    .map(|(n, t)| Parameter(n, t))
                    .collect(),
            ));
        self.diagnostics.push(Diagnostic::definition_at(definition));
    }

    pub(crate) fn report_unknown_fields_in_struct_initialisation(
        &mut self,
        location: Location,
        type_: TypeId,
        too_much: Vec<StringId>,
        definition: Location,
    ) {
        self.diagnostics
            .push(Diagnostic::unknown_fields_in_struct_initialisation(
                location, type_, too_much,
            ));
        self.diagnostics.push(Diagnostic::definition_at(definition));
    }
}
