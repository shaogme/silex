use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageReport,
};
use crate::config::CountOptions;

pub struct TypeScriptAnalyzer;

const EXTENSIONS: &[&str] = &["ts", "tsx"];

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn descriptor(&self) -> LanguageDescriptor {
        LanguageDescriptor::new(LanguageId::TypeScript, EXTENSIONS, &[])
    }

    fn analyze(&self, context: &mut LanguageContext<'_>) -> Result<LanguageReport, LanguageError> {
        let options = context.options();
        let skip_file = options.ignore_tests && is_test_path(context.path());
        let mut scanner = Scanner::default();

        while let Some(line) = context.next_line()? {
            scanner.consume_line(line, options, skip_file);
        }

        Ok(LanguageReport {
            language: LanguageId::TypeScript,
            line_count: scanner.line_count,
        })
    }
}

#[derive(Default)]
struct Scanner {
    state: LexState,
    line_count: usize,
    skip_depth: isize,
    previous: Option<u8>,
    word: [u8; 6],
    word_len: usize,
    word_overflowed: bool,
    previous_word: PreviousWord,
}

#[derive(Default)]
enum LexState {
    #[default]
    Code,
    LineComment,
    BlockComment,
    Regex {
        escaped: bool,
        character_class: bool,
    },
    Quoted {
        quote: u8,
        escaped: bool,
    },
}

#[derive(Clone, Copy, Default)]
enum PreviousWord {
    #[default]
    None,
    Return,
    Throw,
    Case,
    Yield,
    Other,
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
        let mut skipped = false;
        if self.skip_depth > 0 {
            skipped = true;
            self.skip_depth += flags.brace_delta;
            if self.skip_depth <= 0 {
                self.skip_depth = 0;
            }
        } else if is_test_declaration(trimmed) {
            skipped = true;
            if flags.brace_delta > 0 {
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
                    if is_identifier_byte(bytes[index]) {
                        flags.code = true;
                        if self.word_len < self.word.len() {
                            self.word[self.word_len] = bytes[index];
                            self.word_len += 1;
                        } else {
                            self.word_overflowed = true;
                        }
                        self.previous = Some(bytes[index]);
                        index += 1;
                        continue;
                    }
                    self.finish_word();
                    if bytes[index..].starts_with(b"//") {
                        flags.comment = true;
                        self.state = LexState::LineComment;
                        index += 2;
                    } else if bytes[index..].starts_with(b"/*") {
                        flags.comment = true;
                        self.state = LexState::BlockComment;
                        index += 2;
                    } else if bytes[index] == b'/' && self.regex_literal_start() {
                        flags.code = true;
                        self.state = LexState::Regex {
                            escaped: false,
                            character_class: false,
                        };
                        index += 1;
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
                        if !bytes[index].is_ascii_whitespace() {
                            self.previous = Some(bytes[index]);
                            self.previous_word = PreviousWord::None;
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
                LexState::Regex {
                    mut escaped,
                    mut character_class,
                } => {
                    flags.code = true;
                    if escaped {
                        escaped = false;
                    } else if bytes[index] == b'\\' {
                        escaped = true;
                    } else if bytes[index] == b'[' {
                        character_class = true;
                    } else if bytes[index] == b']' {
                        character_class = false;
                    } else if bytes[index] == b'/' && !character_class {
                        self.state = LexState::Code;
                        self.previous = Some(b'/');
                    }
                    if !matches!(self.state, LexState::Code) {
                        self.state = LexState::Regex {
                            escaped,
                            character_class,
                        };
                    }
                    index += 1;
                }
                LexState::Quoted { quote, mut escaped } => {
                    flags.code = true;
                    if escaped {
                        escaped = false;
                    } else if bytes[index] == b'\\' {
                        escaped = true;
                    } else if bytes[index] == quote {
                        self.state = LexState::Code;
                        self.previous = Some(quote);
                    }
                    if !matches!(self.state, LexState::Code) {
                        self.state = LexState::Quoted { quote, escaped };
                    }
                    index += 1;
                }
            }
        }

        self.finish_word();

        if matches!(self.state, LexState::LineComment | LexState::Regex { .. }) {
            self.state = LexState::Code;
        }
        flags
    }

    fn regex_literal_start(&self) -> bool {
        let Some(previous) = self.previous else {
            return true;
        };
        matches!(
            previous,
            b'=' | b'('
                | b'['
                | b'{'
                | b','
                | b':'
                | b';'
                | b'!'
                | b'&'
                | b'|'
                | b'?'
                | b'+'
                | b'-'
                | b'*'
                | b'%'
                | b'^'
                | b'~'
                | b'<'
                | b'>'
        ) || matches!(
            self.previous_word,
            PreviousWord::Return | PreviousWord::Throw | PreviousWord::Case | PreviousWord::Yield
        )
    }

    fn finish_word(&mut self) {
        if self.word_len == 0 {
            return;
        }
        self.previous_word = if self.word_overflowed {
            PreviousWord::Other
        } else {
            match &self.word[..self.word_len] {
                b"return" => PreviousWord::Return,
                b"throw" => PreviousWord::Throw,
                b"case" => PreviousWord::Case,
                b"yield" => PreviousWord::Yield,
                _ => PreviousWord::Other,
            }
        };
        self.word_len = 0;
        self.word_overflowed = false;
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn is_test_declaration(line: &str) -> bool {
    line.starts_with("describe(")
        || line.starts_with("it(")
        || line.starts_with("test(")
        || line.starts_with("suite(")
        || line.starts_with("context(")
        || line.starts_with("Deno.test(")
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
