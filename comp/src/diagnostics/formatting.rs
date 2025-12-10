use std::fmt::Display;

use crate::{
    Location, SourceText, StringId, StringInterner,
    typing::{TypeId, Types},
};

pub struct DiagnosticMessageComponentToString<'a, 'b, 'c, 'd, T> {
    types: &'a Types,
    strings: &'b StringInterner,
    source_texts: &'c SourceText,
    value: &'d T,
}

impl<T: DiagnosticMessageComponent> Display
    for DiagnosticMessageComponentToString<'_, '_, '_, '_, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value
            .fmt(f, self.types, self.strings, self.source_texts)
    }
}

pub trait DiagnosticMessageComponent {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> std::fmt::Result;
}

pub trait DiagnosticMessageComponentToStringTrait {
    fn to_string(
        &self,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> String;
}

impl<T> DiagnosticMessageComponentToStringTrait for T
where
    T: DiagnosticMessageComponent + Sized,
{
    fn to_string(
        &self,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> String {
        format!(
            "{}",
            DiagnosticMessageComponentToString {
                types,
                source_texts,
                strings,
                value: self
            }
        )
    }
}

pub struct BulletList<T>(pub(super) Location, pub(super) Vec<T>);

impl<T: DiagnosticMessageComponent> DiagnosticMessageComponent for BulletList<T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> std::fmt::Result {
        let length = self.0.to_string(types, strings, source_texts).len() + 1;
        for entry in &self.1 {
            writeln!(f)?;
            for _ in 0..length {
                write!(f, " ")?;
            }
            write!(f, " - ")?;
            entry.fmt(f, types, strings, source_texts)?;
        }
        Ok(())
    }
}

pub struct Parameter(pub(super) StringId, pub(super) TypeId);

impl DiagnosticMessageComponent for Parameter {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> std::fmt::Result {
        self.0.fmt(f, types, strings, source_texts)?;
        write!(f, ": ")?;
        self.1.fmt(f, types, strings, source_texts)?;
        Ok(())
    }
}

pub struct Namespace(pub(super) Vec<StringId>);

impl DiagnosticMessageComponent for Namespace {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        types: &Types,
        strings: &StringInterner,
        source_texts: &SourceText,
    ) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        for namespace in &self.0 {
            namespace.fmt(f, types, strings, source_texts)?;
            write!(f, "::")?;
        }
        Ok(())
    }
}

impl DiagnosticMessageComponent for StringId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        _types: &Types,
        strings: &StringInterner,
        _source_texts: &SourceText,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(&strings[*self], f)
    }
}
impl DiagnosticMessageComponent for &str {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        _types: &Types,
        _strings: &StringInterner,
        _source_texts: &SourceText,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}

impl DiagnosticMessageComponent for String {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        _types: &Types,
        _strings: &StringInterner,
        _source_texts: &SourceText,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}

impl DiagnosticMessageComponent for TypeId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        types: &Types,
        strings: &StringInterner,
        _source_texts: &SourceText,
    ) -> std::fmt::Result {
        types.fmt_type(*self, f, strings)
    }
}

impl DiagnosticMessageComponent for Location {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        _types: &Types,
        _strings: &StringInterner,
        source_texts: &SourceText,
    ) -> std::fmt::Result {
        write!(
            f,
            "[{}:{}:{}]",
            source_texts.file_name(*self),
            source_texts.line_number(*self),
            source_texts.column(*self)
        )
    }
}
