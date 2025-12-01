use std::{ops::Index, path::Path};

use crate::lexer::{Lexer, Token};

mod lexer;

pub struct Compiler {
    pub files: SourceText,
    pub diagnostics: Diagnostics,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    start: usize,
    len: usize,
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
}

pub struct DiagnosticMessage(String);

pub struct Diagnostic {
    location: Location,
    message: DiagnosticMessage,
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
