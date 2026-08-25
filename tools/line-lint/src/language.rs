use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    io::BufRead,
    ops::Range,
    path::{Path, PathBuf},
    str,
};

use crate::config::{CountOptions, MemoryLimits, ResourceLimit};

pub mod c;
pub mod cmake;
pub mod cpp;
pub mod csharp;
pub mod css;
pub mod dart;
pub mod dockerfile;
pub mod elixir;
pub mod fish;
pub mod go;
pub mod groovy;
pub mod haskell;
pub mod html;
pub mod ini;
pub mod java;
pub mod javascript;
pub mod julia;
pub mod kotlin;
pub mod less;
pub mod makefile;
pub mod markdown;
pub mod perl;
pub mod php;
pub mod python;
pub mod r;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod scss;
pub mod shell;
pub mod sql;
pub mod svg;
pub mod swift;
pub mod toml;
pub mod typescript;
pub mod vue;
pub mod xml;
pub mod yaml;
pub mod zig;

mod registry;

pub use registry::LanguageRegistry;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LanguageId {
    Rust,
    Python,
    C,
    Cpp,
    CSharp,
    Dart,
    Go,
    Groovy,
    Java,
    JavaScript,
    Kotlin,
    Php,
    Scala,
    Swift,
    TypeScript,
    Zig,
    Elixir,
    Fish,
    Ini,
    Julia,
    Perl,
    R,
    Ruby,
    Shell,
    Toml,
    Yaml,
    Sql,
    Haskell,
    Css,
    Less,
    Scss,
    Vue,
    Html,
    Xml,
    Svg,
    Markdown,
    Dockerfile,
    Makefile,
    Cmake,
}

impl Display for LanguageId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let name = match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Dart => "dart",
            Self::Go => "go",
            Self::Groovy => "groovy",
            Self::Java => "java",
            Self::JavaScript => "javascript",
            Self::Kotlin => "kotlin",
            Self::Php => "php",
            Self::Scala => "scala",
            Self::Swift => "swift",
            Self::TypeScript => "typescript",
            Self::Zig => "zig",
            Self::Elixir => "elixir",
            Self::Fish => "fish",
            Self::Ini => "ini",
            Self::Julia => "julia",
            Self::Perl => "perl",
            Self::R => "r",
            Self::Ruby => "ruby",
            Self::Shell => "shell",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Sql => "sql",
            Self::Haskell => "haskell",
            Self::Css => "css",
            Self::Less => "less",
            Self::Scss => "scss",
            Self::Vue => "vue",
            Self::Html => "html",
            Self::Xml => "xml",
            Self::Svg => "svg",
            Self::Markdown => "markdown",
            Self::Dockerfile => "dockerfile",
            Self::Makefile => "makefile",
            Self::Cmake => "cmake",
        };
        formatter.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageDescriptor {
    pub id: LanguageId,
    pub extensions: &'static [&'static str],
    pub file_names: &'static [&'static str],
}

impl LanguageDescriptor {
    pub const fn new(
        id: LanguageId,
        extensions: &'static [&'static str],
        file_names: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            extensions,
            file_names,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocument {
    path: PathBuf,
    source: String,
    line_ranges: Vec<Range<usize>>,
}

impl SourceDocument {
    pub fn new<S>(path: &Path, source: S) -> Self
    where
        S: Into<String>,
    {
        let source = source.into();
        let line_ranges = line_ranges(&source);
        Self {
            path: path.to_path_buf(),
            source,
            line_ranges,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn line_count(&self) -> usize {
        self.line_ranges.len()
    }

    pub fn line(&self, number: usize) -> Option<&str> {
        self.line_ranges
            .get(number.checked_sub(1)?)
            .map(|range| &self.source[range.clone()])
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.line_ranges
            .iter()
            .map(|range| &self.source[range.clone()])
    }
}

fn line_ranges(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let mut end = index;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        ranges.push(start..end);
        start = index + 1;
    }
    if start < bytes.len() {
        ranges.push(start..bytes.len());
    }
    ranges
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageReport {
    pub language: LanguageId,
    pub line_count: usize,
}

pub struct LanguageContext<'a> {
    path: &'a Path,
    reader: &'a mut dyn BufRead,
    options: CountOptions,
    limits: MemoryLimits,
    line: Vec<u8>,
    line_number: usize,
    bytes_read: u64,
    pending_carriage_return: bool,
}

impl<'a> LanguageContext<'a> {
    pub fn new(
        path: &'a Path,
        reader: &'a mut dyn BufRead,
        options: CountOptions,
        limits: MemoryLimits,
    ) -> Result<Self, LanguageError> {
        limits
            .validate()
            .map_err(|error| LanguageError::Configuration(error.to_string()))?;
        let capacity = limits.max_line_bytes.clamp(1, 64 * 1024);
        Ok(Self {
            path,
            reader,
            options,
            limits,
            line: Vec::with_capacity(capacity),
            line_number: 0,
            bytes_read: 0,
            pending_carriage_return: false,
        })
    }

    pub fn path(&self) -> &Path {
        self.path
    }

    pub fn options(&self) -> CountOptions {
        self.options
    }

    pub fn limits(&self) -> MemoryLimits {
        self.limits
    }

    pub fn line_number(&self) -> usize {
        self.line_number
    }

    pub fn next_line(&mut self) -> Result<Option<&str>, LanguageError> {
        self.line.clear();
        loop {
            let byte = {
                let buffer = self
                    .reader
                    .fill_buf()
                    .map_err(|error| LanguageError::ReadFailed(error.to_string()))?;
                if buffer.is_empty() {
                    if self.pending_carriage_return {
                        self.pending_carriage_return = false;
                        self.append_byte(b'\r')?;
                    }
                    if self.line.is_empty() {
                        return Ok(None);
                    }
                    return self.finish_line();
                }
                let byte = buffer[0];
                self.reader.consume(1);
                byte
            };

            self.bytes_read = self.bytes_read.checked_add(1).ok_or_else(|| {
                LanguageError::Configuration("file byte count overflowed".to_string())
            })?;
            if self.bytes_read > self.limits.max_file_bytes {
                return Err(LanguageError::ResourceLimit(ResourceLimit::FileTooLarge {
                    actual_bytes: self.bytes_read,
                    max_bytes: self.limits.max_file_bytes,
                }));
            }

            if self.pending_carriage_return {
                if byte == b'\n' {
                    self.pending_carriage_return = false;
                    return self.finish_line();
                }
                self.pending_carriage_return = false;
                self.append_byte(b'\r')?;
            }
            if byte == b'\n' {
                return self.finish_line();
            }
            if byte == b'\r' {
                self.pending_carriage_return = true;
            } else {
                self.append_byte(byte)?;
            }
        }
    }

    fn append_byte(&mut self, byte: u8) -> Result<(), LanguageError> {
        if self.line.len() >= self.limits.max_line_bytes {
            let actual_bytes = self.line.len().checked_add(1).ok_or_else(|| {
                LanguageError::Configuration("line byte count overflowed".to_string())
            })?;
            return Err(LanguageError::ResourceLimit(ResourceLimit::LineTooLong {
                line_number: self.line_number.checked_add(1).ok_or_else(|| {
                    LanguageError::Configuration("line number overflowed".to_string())
                })?,
                actual_bytes,
                max_bytes: self.limits.max_line_bytes,
            }));
        }
        self.line.push(byte);
        Ok(())
    }

    fn finish_line(&mut self) -> Result<Option<&str>, LanguageError> {
        self.line_number = self
            .line_number
            .checked_add(1)
            .ok_or_else(|| LanguageError::Configuration("line number overflowed".to_string()))?;
        if self.line_number > self.limits.max_source_lines {
            return Err(LanguageError::ResourceLimit(ResourceLimit::TooManyLines {
                actual_lines: self.line_number,
                max_lines: self.limits.max_source_lines,
            }));
        }
        let line = str::from_utf8(&self.line).map_err(|_| LanguageError::InvalidUtf8)?;
        Ok(Some(line))
    }
}

pub trait LanguageAnalyzer: Sync {
    fn descriptor(&self) -> LanguageDescriptor;
    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum LanguageError {
    UnsupportedLanguage(PathBuf),
    Analysis(String),
    Configuration(String),
    InvalidUtf8,
    ReadFailed(String),
    ResourceLimit(ResourceLimit),
}

impl Display for LanguageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnsupportedLanguage(path) => {
                write!(formatter, "unsupported language: {}", path.display())
            }
            Self::Analysis(message) => formatter.write_str(message),
            Self::Configuration(message) => write!(formatter, "configuration error: {message}"),
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8"),
            Self::ReadFailed(message) => write!(formatter, "read failed: {message}"),
            Self::ResourceLimit(limit) => Display::fmt(limit, formatter),
        }
    }
}

impl Error for LanguageError {}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufReader, Cursor},
        path::{Path, PathBuf},
    };

    use super::{LanguageContext, LanguageError, LanguageId, LanguageRegistry, SourceDocument};
    use crate::config::{CountOptions, MemoryLimits, ResourceLimit};

    #[test]
    fn source_document_has_stable_crlf_boundaries() {
        let document = SourceDocument::new(Path::new("source.rs"), "one\r\ntwo\n".to_string());

        assert_eq!(document.line_count(), 2);
        assert_eq!(document.line(1), Some("one"));
        assert_eq!(document.line(2), Some("two"));
        assert_eq!(document.line(3), None);
    }

    #[test]
    fn registry_routes_case_insensitive_names_and_extensions() {
        let registry = LanguageRegistry::new();

        assert_eq!(
            registry
                .analyzer_for(Path::new("DockerFile"))
                .expect("Dockerfile analyzer")
                .descriptor()
                .id,
            LanguageId::Dockerfile
        );
        assert_eq!(
            registry
                .analyzer_for(Path::new("main.TSX"))
                .expect("TypeScript analyzer")
                .descriptor()
                .id,
            LanguageId::TypeScript
        );
        assert!(registry.analyzer_for(Path::new("README")).is_err());
    }

    #[test]
    fn registry_index_covers_every_descriptor_entry() {
        let registry = LanguageRegistry::new();

        for analyzer in registry.analyzers() {
            let descriptor = analyzer.descriptor();
            for extension in descriptor.extensions {
                let path = PathBuf::from(format!("source.{extension}"));
                assert_eq!(
                    registry
                        .analyzer_for(&path)
                        .expect("extension must be indexed")
                        .descriptor()
                        .id,
                    descriptor.id
                );
            }
            for file_name in descriptor.file_names {
                assert_eq!(
                    registry
                        .analyzer_for(Path::new(file_name))
                        .expect("file name must be indexed")
                        .descriptor()
                        .id,
                    descriptor.id
                );
            }
        }

        assert_eq!(registry.analyzers().len(), 39);
    }

    fn limits(max_file_bytes: u64, max_line_bytes: usize, max_source_lines: usize) -> MemoryLimits {
        MemoryLimits {
            max_file_bytes,
            max_line_bytes,
            max_source_lines,
            rust_ast_max_bytes: max_file_bytes,
        }
    }

    #[test]
    fn context_streams_empty_no_final_newline_crlf_and_utf8_lines() {
        let mut empty_reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
        let mut empty = LanguageContext::new(
            Path::new("empty.rs"),
            &mut empty_reader,
            CountOptions::default(),
            limits(16, 16, 4),
        )
        .expect("create empty context");
        assert_eq!(empty.next_line().expect("read empty file"), None);

        let source = "α\r\nβ".as_bytes().to_vec();
        let mut reader = BufReader::new(Cursor::new(source));
        let mut context = LanguageContext::new(
            Path::new("utf8.rs"),
            &mut reader,
            CountOptions::default(),
            limits(8, 2, 2),
        )
        .expect("create UTF-8 context");
        assert_eq!(context.next_line().expect("read first line"), Some("α"));
        assert_eq!(context.next_line().expect("read second line"), Some("β"));
        assert_eq!(context.next_line().expect("read EOF"), None);
    }

    #[test]
    fn context_accepts_exact_limits_and_rejects_each_overflow_boundary() {
        let mut exact_reader = BufReader::new(Cursor::new(b"abcd\n".to_vec()));
        let mut exact = LanguageContext::new(
            Path::new("exact.rs"),
            &mut exact_reader,
            CountOptions::default(),
            limits(5, 4, 1),
        )
        .expect("create exact context");
        assert_eq!(exact.next_line().expect("read exact line"), Some("abcd"));
        assert_eq!(exact.next_line().expect("read exact EOF"), None);

        let mut file_reader = BufReader::new(Cursor::new(b"abcd".to_vec()));
        let mut file_context = LanguageContext::new(
            Path::new("file.rs"),
            &mut file_reader,
            CountOptions::default(),
            limits(3, 3, 1),
        )
        .expect("create file limit context");
        assert!(matches!(
            file_context.next_line(),
            Err(LanguageError::ResourceLimit(ResourceLimit::FileTooLarge {
                actual_bytes: 4,
                max_bytes: 3,
            }))
        ));

        let mut line_reader = BufReader::new(Cursor::new(b"abcde".to_vec()));
        let mut line_context = LanguageContext::new(
            Path::new("line.rs"),
            &mut line_reader,
            CountOptions::default(),
            limits(8, 4, 1),
        )
        .expect("create line limit context");
        assert!(matches!(
            line_context.next_line(),
            Err(LanguageError::ResourceLimit(ResourceLimit::LineTooLong {
                line_number: 1,
                actual_bytes: 5,
                max_bytes: 4,
            }))
        ));

        let mut count_reader = BufReader::new(Cursor::new(b"a\nb\nc".to_vec()));
        let mut count_context = LanguageContext::new(
            Path::new("count.rs"),
            &mut count_reader,
            CountOptions::default(),
            limits(8, 1, 2),
        )
        .expect("create line count context");
        assert_eq!(count_context.next_line().expect("read line one"), Some("a"));
        assert_eq!(count_context.next_line().expect("read line two"), Some("b"));
        assert!(matches!(
            count_context.next_line(),
            Err(LanguageError::ResourceLimit(ResourceLimit::TooManyLines {
                actual_lines: 3,
                max_lines: 2,
            }))
        ));
    }

    #[test]
    fn context_reports_invalid_utf8_and_bounds_line_buffer() {
        let mut utf8_reader = BufReader::new(Cursor::new(vec![0xff]));
        let mut utf8_context = LanguageContext::new(
            Path::new("invalid.rs"),
            &mut utf8_reader,
            CountOptions::default(),
            limits(4, 4, 1),
        )
        .expect("create invalid UTF-8 context");
        assert_eq!(utf8_context.next_line(), Err(LanguageError::InvalidUtf8));

        let mut long_reader = BufReader::new(Cursor::new(vec![b'x'; 100_000]));
        let mut long_context = LanguageContext::new(
            Path::new("long.rs"),
            &mut long_reader,
            CountOptions::default(),
            limits(100_000, 8, 1),
        )
        .expect("create long-line context");
        assert!(matches!(
            long_context.next_line(),
            Err(LanguageError::ResourceLimit(
                ResourceLimit::LineTooLong { .. }
            ))
        ));
        assert!(long_context.line.capacity() <= 8);
    }
}
