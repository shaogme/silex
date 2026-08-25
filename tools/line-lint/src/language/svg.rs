use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageReport,
};
use crate::config::CountOptions;

pub struct SvgAnalyzer;

const EXTENSIONS: &[&str] = &["svg"];

impl LanguageAnalyzer for SvgAnalyzer {
    fn descriptor(&self) -> LanguageDescriptor {
        LanguageDescriptor::new(LanguageId::Svg, EXTENSIONS, &[])
    }

    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError> {
        let options = context.options();
        let skip_file = options.ignore_tests && is_test_path(context.path());
        let mut scanner = Scanner::default();

        while let Some(line) = context.next_line()? {
            scanner.consume_line(line, options, skip_file);
        }

        Ok(LanguageReport {
            language: LanguageId::Svg,
            line_count: scanner.line_count,
        })
    }
}

#[derive(Default)]
struct Scanner {
    state: LexState,
    line_count: usize,
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
}

impl Scanner {
    fn consume_line(&mut self, line: &str, options: CountOptions, skip_file: bool) {
        let flags = self.scan_line(line);
        if !skip_file && (flags.code || (!options.ignore_comments && flags.comment)) {
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
                    if false {
                        flags.comment = true;
                        self.state = LexState::LineComment;
                        index += 1;
                    } else if bytes[index..].starts_with(b"<!--") {
                        flags.comment = true;
                        self.state = LexState::BlockComment;
                        index += 4;
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
                        index += 1;
                    }
                }
                LexState::LineComment => {
                    flags.comment = true;
                    index = bytes.len();
                }
                LexState::BlockComment => {
                    flags.comment = true;
                    if bytes[index..].starts_with(b"-->") {
                        self.state = LexState::Code;
                        index += 3;
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
