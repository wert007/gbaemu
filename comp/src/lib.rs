use std::{ops::Index, path::Path};

use crate::{
    bind::Binder,
    diagnostics::Diagnostics,
    lexer::{Lexer, Token},
    memory::Memoryblock,
    parser::Parser,
    syntax_tree::{Bound, NamespacedIdentifier, Parsed, SyntaxNode, SyntaxTree},
    traits::{TraitImplementors, Traits},
    typing::Types,
    value::Value,
    variables::{VariableId, Variables},
};

pub mod debug;

mod bind;
mod const_evaluator;
mod diagnostics;
mod lexer;
mod memory;
mod parser;
mod pattern;
mod syntax_tree;
mod traits;
mod typing;
mod value;
mod variables;

pub struct Compiler {
    pub files: SourceText,
    pub diagnostics: Diagnostics,
    pub strings: StringInterner,
    // Things that are mostly for the binder
    pub nodes: BoundTree,
    pub types: Types,
    pub const_memory: Memoryblock<memory::Bound>,
    pub variables: Variables,
    pub traits: Traits,
    pub trait_implementors: TraitImplementors,
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
        let mut strings = StringInterner::new();
        Self {
            files: SourceText::empty(),
            diagnostics: Diagnostics::empty(),
            types: Types::new(&mut strings),
            strings,
            nodes: BoundTree::new(),
            const_memory: Memoryblock::new(),
            variables: Variables::new(),
            trait_implementors: TraitImplementors::new(),
            traits: Traits::new(),
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
        Binder::new(file).bind(self);
    }

    pub fn intern(&mut self, string: impl Into<String>) -> StringId {
        self.strings.intern(string)
    }

    pub fn intern_location(&mut self, location: Location) -> StringId {
        self.strings.intern(&self.files[location])
    }

    pub fn write_diagnostics(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        self.diagnostics
            .write_to(out, &self.files, &self.types, &self.strings, &self.files)
    }
}

pub fn intern_namespaced_identifier(
    namespaced_identifier: NamespacedIdentifier,
    compiler: &mut Compiler,
) -> (Vec<StringId>, StringId) {
    let namespaces: Vec<_> = namespaced_identifier
        .namespaces
        .into_iter()
        .map(|n| compiler.intern_location(n.0.location()))
        .collect();
    let identifier = compiler.intern_location(namespaced_identifier.identifier.location());
    (namespaces, identifier)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(usize);

impl StringId {
    pub const UNDERSCORE: StringId = StringId(0);
}

pub struct StringInterner {
    strings: Vec<String>,
}
impl StringInterner {
    fn new() -> Self {
        let mut result = Self {
            strings: Vec::new(),
        };
        assert_eq!(result.intern("_"), StringId::UNDERSCORE);

        result
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

impl Index<VariableId> for StringInterner {
    type Output = str;

    fn index(&self, index: VariableId) -> &Self::Output {
        &self[index.1]
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

pub trait HasLocation {
    fn location(&self) -> Location;
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

    pub fn file_name(&self, location: Location) -> &str {
        &self[location.file].file_name
    }

    pub fn line_number(&self, location: Location) -> usize {
        self[location.file].line_number(location.span.start)
    }

    pub fn column(&self, location: Location) -> usize {
        self[location.file].column(location.span.start)
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
        line_starts.push(0);
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

    fn line_number(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(it) => it + 1,
            Err(it) => it,
        }
    }

    fn column(&self, offset: usize) -> usize {
        let line_number = self.line_number(offset);
        offset - self.line_starts[line_number - 1]
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
impl BoundId {
    pub unsafe fn from_raw(id: usize) -> BoundId {
        Self(id)
    }
}

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
        while self.elements.len() <= id.0 {
            self.reserved -= 1;
            self.elements
                .push(unsafe { SyntaxNode::<Bound>::empty(BoundId(0)) });
        }
        self.elements[id.0] = node;
    }

    unsafe fn silent_set(&mut self, id: BoundId, node: SyntaxNode<Bound>) {
        while self.elements.len() <= id.0 {
            self.reserved -= 1;
            self.elements
                .push(unsafe { SyntaxNode::<Bound>::empty(BoundId(0)) });
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
    fn location_of(&self, id: BoundId) -> Location {
        self[id].location()
    }

    unsafe fn free_last(&mut self) {
        // self.reserved -= 1;
        self.elements.pop();
    }
}

impl Index<BoundId> for BoundTree {
    type Output = SyntaxNode<Bound>;

    fn index(&self, index: BoundId) -> &Self::Output {
        &self.elements[index.0]
    }
}
