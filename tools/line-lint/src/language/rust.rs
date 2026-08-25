use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageReport,
};
use crate::config::CountOptions;

pub struct RustAnalyzer;

impl LanguageAnalyzer for RustAnalyzer {
    fn descriptor(&self) -> LanguageDescriptor {
        LanguageDescriptor::new(LanguageId::Rust, &["rs"], &[])
    }

    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError> {
        let options = context.options();
        let skip_file = options.ignore_tests && is_test_path(context.path());
        let mut scanner = Scanner::default();

        while let Some(line) = context.next_line()? {
            scanner.consume_line(line, options, skip_file);
        }

        Ok(LanguageReport {
            language: LanguageId::Rust,
            line_count: scanner.line_count,
        })
    }
}

#[derive(Default)]
struct Scanner {
    state: LexState,
    line_count: usize,
    skip_depth: isize,
    pending_test_attribute: bool,
}

#[derive(Clone, Copy, Default)]
enum LexState {
    #[default]
    Code,
    LineComment,
    BlockComment {
        depth: usize,
    },
    Quoted {
        quote: u8,
        escaped: bool,
    },
    RawString {
        hashes: usize,
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
        } else if self.pending_test_attribute && !blank {
            skipped = true;
            self.pending_test_attribute = false;
            if flags.brace_delta > 0 {
                self.skip_depth = flags.brace_delta;
            }
        } else if is_test_attribute(trimmed) {
            skipped = true;
            self.pending_test_attribute = true;
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
                        self.state = LexState::BlockComment { depth: 1 };
                        index += 2;
                    } else if let Some(hashes) = raw_string_start(bytes, index) {
                        flags.code = true;
                        self.state = LexState::RawString { hashes };
                        index += hashes + 2;
                    } else if is_quote_start(bytes, index) {
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
                LexState::BlockComment { mut depth } => {
                    flags.comment = true;
                    if bytes[index..].starts_with(b"*/") {
                        depth -= 1;
                        index += 2;
                        if depth == 0 {
                            self.state = LexState::Code;
                        } else {
                            self.state = LexState::BlockComment { depth };
                        }
                    } else if bytes[index..].starts_with(b"/*") {
                        depth += 1;
                        index += 2;
                        self.state = LexState::BlockComment { depth };
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
                LexState::RawString { hashes } => {
                    flags.code = true;
                    if is_raw_string_end(bytes, index, hashes) {
                        self.state = LexState::Code;
                        index += hashes + 1;
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

fn raw_string_start(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes[index] != b'r' {
        return None;
    }
    let mut cursor = index + 1;
    while cursor < bytes.len() && bytes[cursor] == b'#' {
        cursor += 1;
    }
    (cursor < bytes.len() && bytes[cursor] == b'"').then_some(cursor - index - 1)
}

fn is_raw_string_end(bytes: &[u8], index: usize, hashes: usize) -> bool {
    bytes[index] == b'"'
        && bytes
            .get(index + 1..index + 1 + hashes)
            .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
}

fn is_quote_start(bytes: &[u8], index: usize) -> bool {
    if !matches!(bytes[index], b'"' | b'\'') {
        return false;
    }
    if bytes[index] == b'"' {
        return true;
    }
    let mut cursor = index + 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        if !escaped && bytes[cursor] == b'\'' {
            return true;
        }
        escaped = !escaped && bytes[cursor] == b'\\';
        cursor += 1;
    }
    false
}

fn is_test_attribute(line: &str) -> bool {
    if line.starts_with("#[test]") {
        return true;
    }
    let Some(cfg) = line.strip_prefix("#[cfg(") else {
        return false;
    };
    if cfg.contains("not(") {
        return false;
    }
    if cfg.contains("any(") && cfg.contains("feature") {
        return false;
    }
    cfg.contains("test")
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

#[cfg(test)]
mod tests {
    use std::{
        io::{BufReader, Cursor},
        path::Path,
    };

    use super::RustAnalyzer;
    use crate::{
        config::{CountOptions, MemoryLimits},
        language::{LanguageAnalyzer, LanguageContext, LanguageId, LanguageReport},
    };

    fn analyze(source: &str) -> LanguageReport {
        let mut reader = BufReader::new(Cursor::new(source.as_bytes()));
        let mut context = LanguageContext::new(
            Path::new("main.rs"),
            &mut reader,
            CountOptions::default(),
            MemoryLimits::default(),
        )
        .expect("create context");
        RustAnalyzer
            .analyze(&mut context)
            .expect("analyze Rust source")
    }

    #[test]
    fn skips_streamed_rust_test_items_and_keeps_cfg_any_production_items() {
        let source = include_str!("../../fixtures/line_count.rs");
        let report = analyze(source);

        assert_eq!(report.language, LanguageId::Rust);
        assert_eq!(report.line_count, 9);
    }

    #[test]
    fn parses_rust_test_cfg_forms() {
        let source = "#[cfg(any(test, feature = \"x\"))]\nfn test_item() {}\nfn keep() {}\n";
        let report = analyze(source);

        assert_eq!(report.line_count, 3);
    }

    #[test]
    fn handles_raw_strings_without_repeated_hash_allocations() {
        let source = "let value = r###\"// text\n### still text\"###;\nfn done() {}\n";
        let report = analyze(source);

        assert_eq!(report.line_count, 3);
    }
}
