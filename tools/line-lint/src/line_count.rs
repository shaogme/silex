use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    fs::File,
    io::{BufRead, BufReader, Cursor, Error as IoError, Read, Take},
    path::{Path, PathBuf},
};

use crate::{
    config::{CountOptions, MemoryLimits, ResourceLimit},
    language::{
        LanguageAnalyzer, LanguageContext, LanguageError, LanguageRegistry, LanguageReport,
    },
};

#[derive(Debug, Eq, PartialEq)]
pub struct FileReport {
    pub path: PathBuf,
    pub line_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AnalysisOutcome {
    Report(FileReport),
    Skip,
}

#[derive(Clone, Copy, Default)]
pub struct FileAnalyzer {
    registry: LanguageRegistry,
}

impl FileAnalyzer {
    pub fn new(registry: LanguageRegistry) -> Self {
        Self { registry }
    }

    pub fn analyze(
        &self,
        path: &Path,
        options: &CountOptions,
    ) -> Result<AnalysisOutcome, LineCountError> {
        self.analyze_with_limits(path, options, &MemoryLimits::default())
    }

    pub fn analyze_with_limits(
        &self,
        path: &Path,
        options: &CountOptions,
        limits: &MemoryLimits,
    ) -> Result<AnalysisOutcome, LineCountError> {
        let Some(analyzer) = route_analyzer(&self.registry, path)? else {
            return Ok(AnalysisOutcome::Skip);
        };
        let mut reader = open_bounded_reader(path, limits)?;
        let report = match analyze_reader(path, analyzer, &mut reader, options, *limits) {
            Ok(report) => report,
            Err(FileAnalysisError::InvalidUtf8 { .. }) => return Ok(AnalysisOutcome::Skip),
            Err(error) => return Err(error),
        };
        Ok(AnalysisOutcome::Report(FileReport {
            path: path.to_path_buf(),
            line_count: report.line_count,
        }))
    }

    pub fn analyze_source(
        &self,
        path: &Path,
        source: String,
        options: &CountOptions,
    ) -> Result<AnalysisOutcome, LineCountError> {
        self.analyze_source_with_limits(path, source, options, &MemoryLimits::default())
    }

    pub fn analyze_source_with_limits(
        &self,
        path: &Path,
        source: String,
        options: &CountOptions,
        limits: &MemoryLimits,
    ) -> Result<AnalysisOutcome, LineCountError> {
        let Some(analyzer) = route_analyzer(&self.registry, path)? else {
            return Ok(AnalysisOutcome::Skip);
        };
        validate_read_limits(path, limits)?;
        let mut reader = BufReader::new(Cursor::new(source.into_bytes()));
        let report = match analyze_reader(path, analyzer, &mut reader, options, *limits) {
            Ok(report) => report,
            Err(FileAnalysisError::InvalidUtf8 { .. }) => return Ok(AnalysisOutcome::Skip),
            Err(error) => return Err(error),
        };
        Ok(AnalysisOutcome::Report(FileReport {
            path: path.to_path_buf(),
            line_count: report.line_count,
        }))
    }
}

fn route_analyzer(
    registry: &LanguageRegistry,
    path: &Path,
) -> Result<Option<&'static dyn LanguageAnalyzer>, FileAnalysisError> {
    match registry.analyzer_for(path) {
        Ok(analyzer) => Ok(Some(analyzer)),
        Err(LanguageError::UnsupportedLanguage(_)) => Ok(None),
        Err(error) => Err(FileAnalysisError::language(path, error)),
    }
}

fn analyze_reader<R: BufRead>(
    path: &Path,
    analyzer: &dyn LanguageAnalyzer,
    reader: &mut R,
    options: &CountOptions,
    limits: MemoryLimits,
) -> Result<LanguageReport, FileAnalysisError> {
    let mut context = LanguageContext::new(path, reader, *options, limits)
        .map_err(|error| map_language_error(path, error))?;
    analyzer
        .analyze(&mut context)
        .map_err(|error| map_language_error(path, error))
}

fn map_language_error(path: &Path, error: LanguageError) -> FileAnalysisError {
    match error {
        LanguageError::InvalidUtf8 => FileAnalysisError::InvalidUtf8 {
            path: path.to_path_buf(),
        },
        LanguageError::ResourceLimit(limit) => FileAnalysisError::resource_limit(path, limit),
        LanguageError::ReadFailed(message) => FileAnalysisError::ReadFailed {
            path: path.to_path_buf(),
            message,
        },
        LanguageError::Configuration(message) => configuration_error(path, message),
        other => FileAnalysisError::language(path, other),
    }
}

fn open_bounded_reader(
    path: &Path,
    limits: &MemoryLimits,
) -> Result<BufReader<Take<File>>, FileAnalysisError> {
    let read_limits = validate_read_limits(path, limits)?;
    let file = File::open(path).map_err(|error| open_error(path, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| metadata_error(path, error))?;
    if metadata.len() > limits.max_file_bytes {
        return Err(FileAnalysisError::resource_limit(
            path,
            ResourceLimit::FileTooLarge {
                actual_bytes: metadata.len(),
                max_bytes: limits.max_file_bytes,
            },
        ));
    }
    let capacity = limits.max_line_bytes.saturating_add(2).min(64 * 1024);
    Ok(BufReader::with_capacity(
        capacity.max(1),
        file.take(read_limits.read_limit),
    ))
}

#[derive(Debug, Eq, PartialEq)]
#[cfg(test)]
enum SourceReadOutcome {
    Source(String),
    InvalidUtf8(FileAnalysisError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReadLimits {
    max_file_bytes: usize,
    read_limit: u64,
}

#[cfg(test)]
fn read_bounded_source(
    path: &Path,
    limits: &MemoryLimits,
) -> Result<SourceReadOutcome, FileAnalysisError> {
    let read_limits = validate_read_limits(path, limits)?;
    let file = File::open(path).map_err(|error| open_error(path, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| metadata_error(path, error))?;
    if metadata.len() > limits.max_file_bytes {
        return Err(FileAnalysisError::resource_limit(
            path,
            ResourceLimit::FileTooLarge {
                actual_bytes: metadata.len(),
                max_bytes: limits.max_file_bytes,
            },
        ));
    }

    let bytes = read_bounded_bytes(BufReader::new(file), read_limits.read_limit)
        .map_err(|error| read_error(path, error))?;
    if bytes.len() > read_limits.max_file_bytes {
        return Err(FileAnalysisError::resource_limit(
            path,
            ResourceLimit::FileTooLarge {
                actual_bytes: bytes_to_u64(path, bytes.len())?,
                max_bytes: limits.max_file_bytes,
            },
        ));
    }
    validate_source_bytes(path, &bytes, limits)?;

    match String::from_utf8(bytes) {
        Ok(source) => Ok(SourceReadOutcome::Source(source)),
        Err(_) => Ok(SourceReadOutcome::InvalidUtf8(
            FileAnalysisError::InvalidUtf8 {
                path: path.to_path_buf(),
            },
        )),
    }
}

#[cfg(test)]
fn read_bounded_bytes<R: Read>(reader: R, read_limit: u64) -> Result<Vec<u8>, IoError> {
    let mut reader = reader.take(read_limit);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn validate_read_limits(
    path: &Path,
    limits: &MemoryLimits,
) -> Result<ReadLimits, FileAnalysisError> {
    limits
        .validate()
        .map_err(|error| configuration_error(path, error.to_string()))?;
    let max_file_bytes = usize::try_from(limits.max_file_bytes).map_err(|_| {
        configuration_error(
            path,
            "max-file-bytes cannot be represented on this platform",
        )
    })?;
    let read_size = max_file_bytes.checked_add(1).ok_or_else(|| {
        configuration_error(
            path,
            "max-file-bytes cannot be increased for the read sentinel",
        )
    })?;
    let read_limit = u64::try_from(read_size).map_err(|_| {
        configuration_error(path, "bounded read size cannot be represented as a u64")
    })?;
    Ok(ReadLimits {
        max_file_bytes,
        read_limit,
    })
}

#[cfg(test)]
fn validate_source_bytes(
    path: &Path,
    source: &[u8],
    limits: &MemoryLimits,
) -> Result<(), FileAnalysisError> {
    let read_limits = validate_read_limits(path, limits)?;
    if source.len() > read_limits.max_file_bytes {
        return Err(FileAnalysisError::resource_limit(
            path,
            ResourceLimit::FileTooLarge {
                actual_bytes: bytes_to_u64(path, source.len())?,
                max_bytes: limits.max_file_bytes,
            },
        ));
    }

    let mut physical_lines = 0_usize;
    let mut line_bytes = 0_usize;
    let mut previous_was_carriage_return = false;
    for byte in source {
        if *byte == b'\n' {
            let actual_line_bytes = if previous_was_carriage_return {
                line_bytes
                    .checked_sub(1)
                    .ok_or_else(|| configuration_error(path, "line byte count underflowed"))?
            } else {
                line_bytes
            };
            let line_number = physical_lines
                .checked_add(1)
                .ok_or_else(|| configuration_error(path, "physical line number overflowed"))?;
            validate_line_bytes(path, line_number, actual_line_bytes, limits)?;
            physical_lines = line_number;
            if physical_lines > limits.max_source_lines {
                return Err(FileAnalysisError::resource_limit(
                    path,
                    ResourceLimit::TooManyLines {
                        actual_lines: physical_lines,
                        max_lines: limits.max_source_lines,
                    },
                ));
            }
            line_bytes = 0;
            previous_was_carriage_return = false;
        } else {
            line_bytes = line_bytes
                .checked_add(1)
                .ok_or_else(|| configuration_error(path, "line byte count overflowed"))?;
            previous_was_carriage_return = *byte == b'\r';
        }
    }

    if line_bytes > 0 {
        let line_number = physical_lines
            .checked_add(1)
            .ok_or_else(|| configuration_error(path, "physical line number overflowed"))?;
        validate_line_bytes(path, line_number, line_bytes, limits)?;
        physical_lines = line_number;
        if physical_lines > limits.max_source_lines {
            return Err(FileAnalysisError::resource_limit(
                path,
                ResourceLimit::TooManyLines {
                    actual_lines: physical_lines,
                    max_lines: limits.max_source_lines,
                },
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
fn validate_line_bytes(
    path: &Path,
    line_number: usize,
    actual_bytes: usize,
    limits: &MemoryLimits,
) -> Result<(), FileAnalysisError> {
    if actual_bytes > limits.max_line_bytes {
        return Err(FileAnalysisError::resource_limit(
            path,
            ResourceLimit::LineTooLong {
                line_number,
                actual_bytes,
                max_bytes: limits.max_line_bytes,
            },
        ));
    }
    Ok(())
}

#[cfg(test)]
fn bytes_to_u64(path: &Path, bytes: usize) -> Result<u64, FileAnalysisError> {
    u64::try_from(bytes)
        .map_err(|_| configuration_error(path, "byte count cannot be represented as a u64"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileAnalysisError {
    OpenFailed { path: PathBuf, message: String },
    MetadataFailed { path: PathBuf, message: String },
    ReadFailed { path: PathBuf, message: String },
    InvalidUtf8 { path: PathBuf },
    Configuration { path: PathBuf, message: String },
    ResourceLimit { path: PathBuf, limit: ResourceLimit },
    Language { path: PathBuf, message: String },
}

impl FileAnalysisError {
    pub fn resource_limit(path: &Path, limit: ResourceLimit) -> Self {
        Self::ResourceLimit {
            path: path.to_path_buf(),
            limit,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::OpenFailed { path, .. }
            | Self::MetadataFailed { path, .. }
            | Self::ReadFailed { path, .. }
            | Self::InvalidUtf8 { path }
            | Self::Configuration { path, .. }
            | Self::ResourceLimit { path, .. }
            | Self::Language { path, .. } => path,
        }
    }

    fn language(path: &Path, error: LanguageError) -> Self {
        Self::Language {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}

impl Display for FileAnalysisError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::OpenFailed { path, message } => {
                write!(
                    formatter,
                    "cannot analyze {}: failed to open file: {message}",
                    path.display()
                )
            }
            Self::MetadataFailed { path, message } => write!(
                formatter,
                "cannot analyze {}: failed to read file metadata: {message}",
                path.display()
            ),
            Self::ReadFailed { path, message } => write!(
                formatter,
                "cannot analyze {}: failed to read file: {message}",
                path.display()
            ),
            Self::InvalidUtf8 { path } => {
                write!(
                    formatter,
                    "cannot analyze {}: file is not valid UTF-8",
                    path.display()
                )
            }
            Self::Configuration { path, message } => write!(
                formatter,
                "cannot analyze {}: invalid resource configuration: {message}",
                path.display()
            ),
            Self::ResourceLimit { path, limit } => {
                write!(formatter, "cannot analyze {}: {limit}", path.display())
            }
            Self::Language { path, message } => write!(
                formatter,
                "cannot analyze {}: language analysis failed: {message}",
                path.display()
            ),
        }
    }
}

impl Error for FileAnalysisError {}

#[cfg(test)]
fn read_error(path: &Path, error: IoError) -> FileAnalysisError {
    FileAnalysisError::ReadFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn open_error(path: &Path, error: IoError) -> FileAnalysisError {
    FileAnalysisError::OpenFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn metadata_error(path: &Path, error: IoError) -> FileAnalysisError {
    FileAnalysisError::MetadataFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn configuration_error(path: &Path, message: impl Into<String>) -> FileAnalysisError {
    FileAnalysisError::Configuration {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

pub type LineCountError = FileAnalysisError;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Cursor,
        path::{Path, PathBuf},
    };

    use super::{
        AnalysisOutcome, FileAnalysisError, FileAnalyzer, FileReport, SourceReadOutcome,
        read_bounded_bytes, read_bounded_source, validate_source_bytes,
    };
    use crate::{
        config::{CountOptions, LintSettings, MemoryLimits, ResourceLimit},
        language::LanguageRegistry,
    };
    use tempfile::tempdir;

    fn count(source: &str, path: &Path, options: &CountOptions) -> usize {
        let outcome = FileAnalyzer::new(LanguageRegistry::new())
            .analyze_source(path, source.to_string(), options)
            .expect("analyze source");
        let AnalysisOutcome::Report(report) = outcome else {
            panic!("source should be supported");
        };
        report.line_count
    }

    fn byte_count(source: &[u8]) -> u64 {
        u64::try_from(source.len()).expect("test source length fits in u64")
    }

    #[test]
    fn ignores_blank_lines_and_comments_for_rust() {
        let source = "// comment\n\nfn main() { /* inline */ }\n/* block\ncomment */\n";

        assert_eq!(
            count(source, Path::new("main.rs"), &CountOptions::default()),
            1
        );
    }

    #[test]
    fn can_include_comments_without_counting_blank_lines() {
        let source = "# comment\n\nvalue = 1  # trailing\n";
        let options = CountOptions {
            ignore_comments: false,
            ..CountOptions::default()
        };

        assert_eq!(count(source, Path::new("main.py"), &options), 2);
    }

    #[test]
    fn skips_rust_test_items_but_keeps_cfg_any_production_items() {
        let source = include_str!("../fixtures/line_count.rs");

        assert_eq!(
            count(source, Path::new("line_count.rs"), &CountOptions::default()),
            9
        );
    }

    #[test]
    fn can_include_rust_tests_when_requested() {
        let source = include_str!("../fixtures/tests.rs");
        let options = CountOptions {
            ignore_tests: false,
            ..CountOptions::default()
        };

        assert_eq!(count(source, Path::new("tests.rs"), &options), 2);
    }

    #[test]
    fn skips_test_files_and_python_test_functions() {
        let test_file = "const value = 1;\n";
        assert_eq!(
            count(
                test_file,
                Path::new("tests/example.js"),
                &CountOptions::default()
            ),
            0
        );

        let source = "def keep():\n    return 1\n\ndef test_value():\n    assert 1 == 1\n";
        assert_eq!(
            count(source, Path::new("example.py"), &CountOptions::default()),
            2
        );
    }

    #[test]
    fn keeps_non_test_python_decorators_in_the_count() {
        let source = "@dataclass\nclass Value:\n    value: int\n";

        assert_eq!(
            count(source, Path::new("example.py"), &CountOptions::default()),
            3
        );
    }

    #[test]
    fn keeps_rust_lifetimes_from_masking_following_comments() {
        let source = "fn make<'a>() {\n    // comment\n}\n";

        assert_eq!(
            count(source, Path::new("main.rs"), &CountOptions::default()),
            2
        );
    }

    #[test]
    fn skips_unknown_and_invalid_utf8_inputs() {
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        assert_eq!(
            analyzer
                .analyze_source(
                    Path::new("README"),
                    "plain text".to_string(),
                    &CountOptions::default()
                )
                .expect("analyze unknown source"),
            AnalysisOutcome::Skip
        );
        assert_eq!(
            LintSettings::default().count_options(),
            CountOptions::default()
        );
    }

    #[test]
    fn formats_resource_diagnostics_with_path_and_limits() {
        let error = FileAnalysisError::resource_limit(
            Path::new("src/generated.rs"),
            ResourceLimit::FileTooLarge {
                actual_bytes: 73_400_321,
                max_bytes: 67_108_864,
            },
        );

        assert_eq!(
            error.to_string(),
            "cannot analyze src/generated.rs: file size 73400321 bytes exceeds configured limit 67108864 bytes"
        );
        assert_eq!(error.path(), Path::new("src/generated.rs"));
    }

    #[test]
    fn formats_each_file_analysis_error_boundary() {
        let path = Path::new("src/input.rs");
        let cases = [
            (
                FileAnalysisError::OpenFailed {
                    path: path.to_path_buf(),
                    message: "permission denied".to_string(),
                },
                "cannot analyze src/input.rs: failed to open file: permission denied",
            ),
            (
                FileAnalysisError::MetadataFailed {
                    path: path.to_path_buf(),
                    message: "metadata unavailable".to_string(),
                },
                "cannot analyze src/input.rs: failed to read file metadata: metadata unavailable",
            ),
            (
                FileAnalysisError::ReadFailed {
                    path: path.to_path_buf(),
                    message: "read interrupted".to_string(),
                },
                "cannot analyze src/input.rs: failed to read file: read interrupted",
            ),
            (
                FileAnalysisError::InvalidUtf8 {
                    path: path.to_path_buf(),
                },
                "cannot analyze src/input.rs: file is not valid UTF-8",
            ),
            (
                FileAnalysisError::Configuration {
                    path: path.to_path_buf(),
                    message: "sentinel overflow".to_string(),
                },
                "cannot analyze src/input.rs: invalid resource configuration: sentinel overflow",
            ),
            (
                FileAnalysisError::Language {
                    path: path.to_path_buf(),
                    message: "scanner failed".to_string(),
                },
                "cannot analyze src/input.rs: language analysis failed: scanner failed",
            ),
        ];

        for (error, diagnostic) in cases {
            assert_eq!(error.to_string(), diagnostic);
            assert_eq!(error.path(), path);
        }
    }

    #[test]
    fn reports_read_failures_with_a_stable_prefix() {
        let error = FileAnalyzer::new(LanguageRegistry::new())
            .analyze(Path::new("missing.rs"), &CountOptions::default())
            .expect_err("missing file must fail");

        assert!(matches!(error, FileAnalysisError::OpenFailed { .. }));
        assert!(
            error
                .to_string()
                .starts_with("cannot analyze missing.rs: failed to open file:")
        );
    }

    #[test]
    fn routes_unknown_paths_before_opening_them() {
        let directory = tempdir().expect("create temporary directory");
        let unknown_file = directory.path().join("large.bin");
        fs::write(&unknown_file, vec![b'x'; 128]).expect("write unknown large file");
        let tiny_limits = MemoryLimits {
            max_file_bytes: 1,
            max_line_bytes: 1,
            max_source_lines: 1,
            rust_ast_max_bytes: 1,
        };

        let outcome = FileAnalyzer::new(LanguageRegistry::new())
            .analyze_with_limits(&unknown_file, &CountOptions::default(), &tiny_limits)
            .expect("unknown path should be skipped before opening");

        assert_eq!(outcome, AnalysisOutcome::Skip);
    }

    #[test]
    fn accepts_file_size_equal_to_limit_and_rejects_one_byte_over() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("source.rs");
        let source = b"fn main() {}";
        fs::write(&path, source).expect("write source");
        let limits = MemoryLimits {
            max_file_bytes: byte_count(source),
            max_line_bytes: source.len(),
            max_source_lines: 1,
            rust_ast_max_bytes: byte_count(source),
        };

        assert!(matches!(
            FileAnalyzer::new(LanguageRegistry::new())
                .analyze_with_limits(&path, &CountOptions::default(), &limits)
                .expect("file at limit should be accepted"),
            AnalysisOutcome::Report(_)
        ));

        let mut oversized = source.to_vec();
        oversized.push(b' ');
        fs::write(&path, oversized).expect("write oversized source");
        let error = FileAnalyzer::new(LanguageRegistry::new())
            .analyze_with_limits(&path, &CountOptions::default(), &limits)
            .expect_err("file over limit must fail");

        assert!(matches!(
            error,
            FileAnalysisError::ResourceLimit {
                limit: ResourceLimit::FileTooLarge { .. },
                ..
            }
        ));
    }

    #[test]
    fn bounded_reader_detects_growth_after_a_small_metadata_snapshot() {
        let bytes = vec![b'x'; 6];
        let actual = read_bounded_bytes(Cursor::new(bytes), 6)
            .expect("bounded reader should read its sentinel byte");

        assert_eq!(actual.len(), 6);
        assert!(actual.len() > 5);
        let limits = MemoryLimits {
            max_file_bytes: 5,
            max_line_bytes: 5,
            max_source_lines: 1,
            rust_ast_max_bytes: 5,
        };
        let error = validate_source_bytes(Path::new("growing.rs"), &actual, &limits)
            .expect_err("the sentinel byte must be classified as file growth");
        assert!(matches!(
            error,
            FileAnalysisError::ResourceLimit {
                limit: ResourceLimit::FileTooLarge { .. },
                ..
            }
        ));
    }

    #[test]
    fn handles_empty_files_no_final_newline_and_crlf() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("source.rs");
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        let options = CountOptions::default();

        fs::write(&path, []).expect("write empty source");
        let empty = analyzer
            .analyze_with_limits(&path, &options, &MemoryLimits::default())
            .expect("empty source should be accepted");
        assert_eq!(
            empty,
            AnalysisOutcome::Report(FileReport {
                path: path.clone(),
                line_count: 0,
            })
        );

        let no_final_newline = b"fn main() {}";
        fs::write(&path, no_final_newline).expect("write source without final newline");
        let no_final_newline_limits = MemoryLimits {
            max_file_bytes: byte_count(no_final_newline),
            max_line_bytes: no_final_newline.len(),
            max_source_lines: 1,
            rust_ast_max_bytes: byte_count(no_final_newline),
        };
        assert!(matches!(
            analyzer
                .analyze_with_limits(&path, &options, &no_final_newline_limits)
                .expect("source without final newline should be accepted"),
            AnalysisOutcome::Report(_)
        ));

        let crlf = b"fn first() {}\r\nfn second() {}\r\n";
        fs::write(&path, crlf).expect("write CRLF source");
        let crlf_limits = MemoryLimits {
            max_file_bytes: byte_count(crlf),
            max_line_bytes: crlf.len(),
            max_source_lines: 2,
            rust_ast_max_bytes: byte_count(crlf),
        };
        let outcome = analyzer
            .analyze_with_limits(&path, &options, &crlf_limits)
            .expect("CRLF source should be accepted");
        assert!(matches!(
            outcome,
            AnalysisOutcome::Report(FileReport { line_count: 2, .. })
        ));
    }

    #[test]
    fn rejects_long_lines_and_too_many_physical_lines() {
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        let options = CountOptions::default();
        let long_line_limits = MemoryLimits {
            max_file_bytes: 32,
            max_line_bytes: 4,
            max_source_lines: 4,
            rust_ast_max_bytes: 32,
        };
        let line_error = analyzer
            .analyze_source_with_limits(
                Path::new("source.rs"),
                "12345".to_string(),
                &options,
                &long_line_limits,
            )
            .expect_err("long line must fail");
        assert!(matches!(
            line_error,
            FileAnalysisError::ResourceLimit {
                limit: ResourceLimit::LineTooLong { line_number: 1, .. },
                ..
            }
        ));

        let line_limits = MemoryLimits {
            max_file_bytes: 32,
            max_line_bytes: 4,
            max_source_lines: 2,
            rust_ast_max_bytes: 32,
        };
        let line_error = analyzer
            .analyze_source_with_limits(
                Path::new("source.rs"),
                "a\nb\nc".to_string(),
                &options,
                &line_limits,
            )
            .expect_err("too many physical lines must fail");
        assert!(matches!(
            line_error,
            FileAnalysisError::ResourceLimit {
                limit: ResourceLimit::TooManyLines {
                    actual_lines: 3,
                    max_lines: 2,
                },
                ..
            }
        ));
    }

    #[test]
    fn analyze_source_cannot_bypass_byte_or_line_limits() {
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        let options = CountOptions::default();
        let limits = MemoryLimits {
            max_file_bytes: 4,
            max_line_bytes: 4,
            max_source_lines: 4,
            rust_ast_max_bytes: 4,
        };

        let error = analyzer
            .analyze_source_with_limits(
                Path::new("source.rs"),
                "12345".to_string(),
                &options,
                &limits,
            )
            .expect_err("source entry point must enforce byte limits");
        assert!(matches!(
            error,
            FileAnalysisError::ResourceLimit {
                limit: ResourceLimit::FileTooLarge { .. },
                ..
            }
        ));
    }

    #[test]
    fn invalid_utf8_keeps_skip_semantics_and_has_a_diagnostic() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("invalid.rs");
        fs::write(&path, [0xff, 0xfe]).expect("write invalid UTF-8");
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());

        let outcome = analyzer
            .analyze_with_limits(&path, &CountOptions::default(), &MemoryLimits::default())
            .expect("invalid UTF-8 should be skipped");
        assert_eq!(outcome, AnalysisOutcome::Skip);

        let read = read_bounded_source(&path, &MemoryLimits::default())
            .expect("invalid UTF-8 read should be classified");
        assert!(matches!(
            read,
            SourceReadOutcome::InvalidUtf8(FileAnalysisError::InvalidUtf8 { .. })
        ));
    }

    #[test]
    fn does_not_treat_comment_markers_inside_strings_as_comments() {
        let source = "value = \"# not a comment\"\n";

        assert_eq!(
            count(source, Path::new("main.py"), &CountOptions::default()),
            1
        );
    }

    #[test]
    fn recognizes_multiline_raw_rust_strings() {
        let source = "let value = r#\"// not a comment\n# still text\"#;\nfn done() {}\n";

        assert_eq!(
            count(source, Path::new("main.rs"), &CountOptions::default()),
            3
        );
    }

    #[test]
    fn chooses_comment_rules_by_file_suffix() {
        let options = CountOptions::default();

        assert_eq!(
            count(
                "// comment\nconst value = 1;\n",
                Path::new("main.js"),
                &options
            ),
            1
        );
        assert_eq!(
            count(
                "<!-- comment -->\n<h1>Title</h1>\n",
                Path::new("page.html"),
                &options
            ),
            1
        );
        assert_eq!(
            count("-- comment\nSELECT 1;\n", Path::new("query.sql"), &options),
            1
        );
    }

    #[test]
    fn every_registry_analyzer_consumes_a_streaming_source() {
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        for language_analyzer in LanguageRegistry::new().analyzers() {
            let descriptor = language_analyzer.descriptor();
            let path = descriptor
                .extensions
                .first()
                .map(|extension| PathBuf::from(format!("source.{extension}")))
                .or_else(|| descriptor.file_names.first().map(PathBuf::from))
                .expect("every analyzer has a route");
            assert_eq!(
                analyzer
                    .analyze_source(&path, "value = 1\n".to_string(), &CountOptions::default())
                    .expect("stream source should analyze"),
                AnalysisOutcome::Report(FileReport {
                    path,
                    line_count: 1,
                })
            );
        }
    }
}
