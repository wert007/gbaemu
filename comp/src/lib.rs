use std::{ops::Index, path::Path};

use crate::{
    bind::Binder,
    lexer::{Lexer, Token},
    parser::Parser,
    syntax_tree::{Bound, Parsed, SyntaxNode, SyntaxTree},
    value::Value,
};

mod bind;
mod const_evaluator;
mod lexer;
mod parser;
mod syntax_tree;
mod typing;
mod value;

pub struct Compiler {
    pub files: SourceText,
    pub diagnostics: Diagnostics,
    pub strings: StringInterner,
    pub nodes: BoundTree,
}

impl Index<SourceTextId> for Compiler {
    type Output = SourceTextFile;

    fn index(&self, index: SourceTextId) -> &Self::Output {
        &self.files[index]
    }
}

impl Index<Location> for Compiler {
    type Output = str;

    fn index(&self, index: Location) -> &Self::Output {
        &self.files[index]
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            files: SourceText::empty(),
            diagnostics: Diagnostics::empty(),
            strings: StringInterner::new(),
            nodes: BoundTree::new(),
        }
    }

    pub fn add_file(&mut self, path: impl AsRef<Path>) -> Result<SourceTextId, std::io::Error> {
        self.files.add(path)
    }

    pub fn lex(&mut self, file: SourceTextId) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut lexer = Lexer::new(file);
        while let Some(token) = lexer.lex(self) {
            tokens.push(token);
        }
        tokens
    }

    pub fn parse(&mut self, file: SourceTextId) -> SyntaxTree<Parsed> {
        Parser::new(file).parse(self)
    }

    pub fn bind(&mut self, file: SourceTextId) {
        Binder::new(file, self).bind(self);
    }

    pub fn intern(&mut self, string: impl Into<String>) -> StringId {
        self.strings.intern(string)
    }

    pub fn intern_location(&mut self, location: Location) -> StringId {
        self.strings.intern(&self.files[location])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringId(usize);

pub struct StringInterner {
    strings: Vec<String>,
}
impl StringInterner {
    fn new() -> Self {
        Self {
            strings: Vec::new(),
        }
    }

    fn intern(&mut self, string: impl Into<String>) -> StringId {
        let string = string.into();
        if let Some(index) = self.strings.iter().position(|s| s == &string) {
            StringId(index)
        } else {
            let index = self.strings.len();
            self.strings.push(string);
            StringId(index)
        }
    }
}

impl Index<StringId> for StringInterner {
    type Output = str;

    fn index(&self, index: StringId) -> &Self::Output {
        &self.strings[index.0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    start: usize,
    len: usize,
}
impl Span {
    fn combine(self, span: Span) -> Span {
        Self::new(self.start.min(span.start), self.end().max(span.end()))
    }

    fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            len: end - start,
        }
    }

    fn end(&self) -> usize {
        self.start + self.len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    span: Span,
    file: SourceTextId,
}

impl Location {
    fn with_end_at(self, end: usize) -> Self {
        Self {
            span: Span {
                start: self.span.start,
                len: end - self.span.start,
            },
            ..self
        }
    }

    fn combine(self, other: impl Into<Option<Location>>) -> Location {
        let other = other.into();
        if let Some(other) = other {
            assert_eq!(self.file, other.file);
            Self {
                span: self.span.combine(other.span),
                ..self
            }
        } else {
            self
        }
    }

    fn from_vec<L: HasLocation>(locations: &[L]) -> Option<Location> {
        if locations.is_empty() {
            None
        } else {
            let mut result = locations[0].location();
            for l in locations {
                result = result.combine(l.location());
            }
            Some(result)
        }
    }

    unsafe fn zero() -> Location {
        Location {
            span: Span { start: 0, len: 0 },
            file: SourceTextId(0),
        }
    }
}

trait HasLocation {
    fn location(&self) -> Location;
}

pub struct DiagnosticMessage(String);

pub struct Diagnostic {
    location: Location,
    message: DiagnosticMessage,
}
impl Diagnostic {
    fn invalid_char(location: Location, ch: char) -> Diagnostic {
        Self {
            location,
            message: DiagnosticMessage(format!("Unexpected char {ch} in input!")),
        }
    }
}

pub struct Diagnostics {
    diagnostics: Vec<Diagnostic>,
}
impl Diagnostics {
    fn empty() -> Diagnostics {
        Diagnostics {
            diagnostics: Vec::new(),
        }
    }

    fn report_invalid_char(&mut self, location: Location, ch: char) {
        self.diagnostics
            .push(Diagnostic::invalid_char(location, ch));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTextId(usize);

pub struct SourceText {
    files: Vec<SourceTextFile>,
}

impl Index<SourceTextId> for SourceText {
    type Output = SourceTextFile;

    fn index(&self, index: SourceTextId) -> &Self::Output {
        &self.files[index.0]
    }
}

impl Index<Location> for SourceText {
    type Output = str;

    fn index(&self, index: Location) -> &Self::Output {
        &self[index.file][index.span]
    }
}

impl SourceText {
    fn empty() -> SourceText {
        Self { files: Vec::new() }
    }

    fn add(&mut self, path: impl AsRef<Path>) -> Result<SourceTextId, std::io::Error> {
        let id = SourceTextId(self.files.len());
        self.files.push(SourceTextFile::from_file(path)?);
        Ok(id)
    }
}

pub struct SourceTextFile {
    file_name: String,
    content: String,
    line_starts: Vec<usize>,
}
impl SourceTextFile {
    fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .ok_or(std::io::Error::from(std::io::ErrorKind::InvalidFilename))?
            .to_string_lossy()
            .into_owned();
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_string(file_name, content))
    }

    fn from_string(file_name: String, content: String) -> SourceTextFile {
        let line_starts = Self::collect_line_starts(&content);
        Self {
            file_name,
            content,
            line_starts,
        }
    }

    fn collect_line_starts(content: &str) -> Vec<usize> {
        let mut line_starts = Vec::new();
        for (i, ch) in content.char_indices() {
            match ch {
                '\n' => {
                    line_starts.push(i + 1);
                }
                _ => {}
            }
        }
        line_starts
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl Index<Span> for SourceTextFile {
    type Output = str;

    fn index(&self, index: Span) -> &Self::Output {
        &self.content[index.start..][..index.len]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundId(usize);

#[derive(Debug)]

pub struct BoundTree {
    // root: BoundId,
    elements: Vec<SyntaxNode<Bound>>,
    reserved: usize,
}
impl BoundTree {
    fn new() -> Self {
        Self {
            elements: Vec::new(),
            reserved: 0,
        }
    }

    pub(crate) fn constant_value(&self, expression: BoundId) -> Option<&Value> {
        self[expression].stage.constant_value.as_ref()
    }

    pub(crate) fn set_constant_value(&mut self, expression: BoundId, value: Option<Value>) {
        self.elements[expression.0].stage.constant_value = value;
    }

    unsafe fn set(&mut self, id: BoundId, node: SyntaxNode<Bound>) {
        assert!(id.0 < self.elements.len() + self.reserved);
        self.reserved -= 1;
        while self.elements.len() <= id.0 {
            self.elements.push(SyntaxNode::<Bound>::error(
                unsafe { Location::zero() },
                BoundId(0),
            ));
        }
        self.elements[id.0] = node;
    }

    unsafe fn prepare_id(&mut self) -> BoundId {
        let id = self.elements.len() + self.reserved;
        self.reserved += 1;
        BoundId(id)
    }

    fn type_of(&self, id: BoundId) -> typing::TypeId {
        self[id].stage.type_
    }
}

impl Index<BoundId> for BoundTree {
    type Output = SyntaxNode<Bound>;

    fn index(&self, index: BoundId) -> &Self::Output {
        &self.elements[index.0]
    }
}
