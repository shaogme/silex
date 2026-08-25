use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageReport,
};
use crate::config::CountOptions;

pub struct CppAnalyzer;

const EXTENSIONS: &[&str] = &["cc", "cpp", "cxx"];

impl LanguageAnalyzer for CppAnalyzer {
    fn descriptor(&self) -> LanguageDescriptor {
        LanguageDescriptor::new(LanguageId::Cpp, EXTENSIONS, &[])
    }

    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError> {
        let options = context.options();
        let skip_file = options.ignore_tests && is_test_path(context.path());
        let mut scanner = Scanner::default();

        while let Some(line) = context.next_line()? {
            scanner.consume_line(line, options, skip_file);
        }

        Ok(LanguageReport {
            language: LanguageId::Cpp,
            line_count: scanner.line_count,
        })
    }
}

#[derive(Default)]
struct Scanner {
    state: LexState,
    line_count: usize,
    skip_depth: isize,
    pending_annotation: bool,
}

#[derive(Default)]
enum LexState {
    #[default]
    Code,
    LineComment,
    BlockComment,
    Quoted {
        quote: u8,
        escaped: bool,
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
        } else if self.pending_annotation && !blank {
            skipped = true;
            self.pending_annotation = false;
            if flags.brace_delta > 0 {
                self.skip_depth = flags.brace_delta;
            }
        } else if is_test_declaration(trimmed) {
            skipped = true;
            if is_annotation(trimmed) {
                self.pending_annotation = true;
            } else if flags.brace_delta > 0 {
                self.skip_depth = flags.brace_delta;
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
                    if bytes[index..].starts_with(b"//") {
                        flags.comment = true;
                        self.state = LexState::LineComment;
                        index += 2;
                    } else if bytes[index..].starts_with(b"/*") {
                        flags.comment = true;
                        self.state = LexState::BlockComment;
                        index += 2;
                    } else if matches!(bytes[index], b'\'' | b'"') || bytes[index] == 0x60 {
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
                LexState::BlockComment => {
                    flags.comment = true;
                    if bytes[index..].starts_with(b"*/") {
                        self.state = LexState::Code;
                        index += 2;
                    } else {
                        index += 1;
                    }
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
            }
        }

        if matches!(self.state, LexState::LineComment) {
            self.state = LexState::Code;
        }
        flags
    }
}

fn is_test_declaration(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("TEST(") || trimmed.starts_with("TEST_F(") || trimmed.starts_with("TEST_P(")
}

fn is_annotation(line: &str) -> bool {
    line.starts_with('@') || line.starts_with('[')
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
