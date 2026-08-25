use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageReport,
};
use crate::config::CountOptions;

pub struct TomlAnalyzer;

const EXTENSIONS: &[&str] = &["toml"];

impl LanguageAnalyzer for TomlAnalyzer {
    fn descriptor(&self) -> LanguageDescriptor {
        LanguageDescriptor::new(LanguageId::Toml, EXTENSIONS, &[])
    }

    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError> {
        let options = context.options();
        let skip_file = options.ignore_tests && is_test_path(context.path());
        let mut scanner = Scanner::default();

        while let Some(line) = context.next_line()? {
            scanner.consume_line(line, options, skip_file);
        }

        Ok(LanguageReport {
            language: LanguageId::Toml,
            line_count: scanner.line_count,
        })
    }
}

#[derive(Default)]
struct Scanner {
    state: LexState,
    line_count: usize,
    skip_depth: isize,
    skip_indent: Option<usize>,
    pending_annotation: bool,
}

#[derive(Default)]
enum LexState {
    #[default]
    Code,
    LineComment,
    Quoted {
        quote: u8,
        escaped: bool,
    },
    Triple {
        quote: u8,
    },
}

#[derive(Clone, Copy, Default)]
struct LineFlags {
    code: bool,
    comment: bool,
    brace_delta: isize,
}

impl Scanner {
    fn consume_line(&mut self, line: &str, options: CountOptions, skip_file: bool) {
        let flags = self.scan_line(line);
        if skip_file {
            return;
        }
        if !options.ignore_tests {
            self.add_line(flags, false, options);
            return;
        }

        let trimmed = line.trim_start();
        let blank = trimmed.is_empty();
        let mut skipped = false;

        if self.skip_depth > 0 {
            skipped = true;
            self.skip_depth += flags.brace_delta;
            if self.skip_depth <= 0 {
                self.skip_depth = 0;
            }
        } else if let Some(base_indent) = self.skip_indent {
            if blank || indentation(line) > base_indent {
                skipped = true;
            } else {
                self.skip_indent = None;
            }
        }

        if !skipped {
            if self.pending_annotation && !blank {
                skipped = true;
                self.pending_annotation = false;
                if flags.brace_delta > 0 {
                    self.skip_depth = flags.brace_delta;
                } else if is_indented_test(trimmed) {
                    self.skip_indent = Some(indentation(line));
                }
            } else if is_test_declaration(trimmed) {
                skipped = true;
                if is_annotation(trimmed) {
                    self.pending_annotation = true;
                } else if flags.brace_delta > 0 {
                    self.skip_depth = flags.brace_delta;
                } else if is_indented_test(trimmed) {
                    self.skip_indent = Some(indentation(line));
                }
            }
        }

        self.add_line(flags, skipped, options);
    }

    fn add_line(&mut self, flags: LineFlags, skipped: bool, options: CountOptions) {
        if !skipped && (flags.code || (!options.ignore_comments && flags.comment)) {
            self.line_count += 1;
        }
    }

    fn scan_line(&mut self, line: &str) -> LineFlags {
        let bytes = line.as_bytes();
        let mut flags = LineFlags::default();
        let mut index = 0;

        while index < bytes.len() {
            match self.state {
                LexState::Code => {
                    if bytes[index] == b'#' {
                        flags.comment = true;
                        self.state = LexState::LineComment;
                        index += 1;
                    } else if bytes[index..].starts_with(b"'''")
                        || bytes[index..].starts_with(b"\"\"\"")
                    {
                        flags.code = true;
                        self.state = LexState::Triple {
                            quote: bytes[index],
                        };
                        index += 3;
                    } else if matches!(bytes[index], b'\'' | b'"') {
                        flags.code = true;
                        self.state = LexState::Quoted {
                            quote: bytes[index],
                            escaped: false,
                        };
                        index += 1;
                    } else {
                        if !bytes[index].is_ascii_whitespace() {
                            flags.code = true;
                        }
                        if bytes[index] == b'{' {
                            flags.brace_delta += 1;
                        } else if bytes[index] == b'}' {
                            flags.brace_delta -= 1;
                        }
                        index += 1;
                    }
                }
                LexState::LineComment => {
                    flags.comment = true;
                    index = bytes.len();
                }
                LexState::Quoted { quote, mut escaped } => {
                    flags.code = true;
                    if escaped {
                        escaped = false;
                    } else if bytes[index] == b'\\' {
                        escaped = true;
                    } else if bytes[index] == quote {
                        self.state = LexState::Code;
                    }
                    if !matches!(self.state, LexState::Code) {
                        self.state = LexState::Quoted { quote, escaped };
                    }
                    index += 1;
                }
                LexState::Triple { quote } => {
                    flags.code = true;
                    if bytes[index..].starts_with(&[quote, quote, quote]) {
                        self.state = LexState::Code;
                        index += 3;
                    } else {
                        index += 1;
                    }
                }
            }
        }

        if matches!(self.state, LexState::LineComment) {
            self.state = LexState::Code;
        }
        flags
    }
}

fn is_test_declaration(_line: &str) -> bool {
    false
}

fn is_indented_test(line: &str) -> bool {
    line.starts_with("def test_")
        || line.starts_with("async def test_")
        || line.starts_with("class Test")
}

fn is_annotation(line: &str) -> bool {
    line.starts_with('@')
        || line.starts_with("@pytest.mark")
        || line.starts_with("@ParameterizedTest")
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

fn is_test_path(path: &Path) -> bool {
    if path.components().any(|component| {
        matches!(
            component
                .as_os_str()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            "test" | "tests" | "__tests__" | "spec" | "specs"
        )
    }) {
        return true;
    }
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    let stem = lower
        .rsplit_once('.')
        .map_or(lower.as_str(), |(stem, _)| stem);
    stem == "test"
        || stem == "tests"
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.contains(".test")
        || stem.contains(".spec")
}
