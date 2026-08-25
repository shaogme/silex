use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const DEFAULT_MAX_LINES: usize = 650;
pub const DEFAULT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_SOURCE_LINES: usize = 1_000_000;
pub const DEFAULT_RUST_AST_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const DEFAULT_JOBS: usize = 8;
pub const MAX_JOBS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryLimits {
    pub max_file_bytes: u64,
    pub max_line_bytes: usize,
    pub max_source_lines: usize,
    pub rust_ast_max_bytes: u64,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            max_source_lines: DEFAULT_MAX_SOURCE_LINES,
            rust_ast_max_bytes: DEFAULT_RUST_AST_MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLimit {
    FileTooLarge {
        actual_bytes: u64,
        max_bytes: u64,
    },
    LineTooLong {
        line_number: usize,
        actual_bytes: usize,
        max_bytes: usize,
    },
    TooManyLines {
        actual_lines: usize,
        max_lines: usize,
    },
    RustAstTooLarge {
        actual_bytes: u64,
        max_bytes: u64,
    },
}

impl Display for ResourceLimit {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::FileTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "file size {actual_bytes} bytes exceeds configured limit {max_bytes} bytes"
            ),
            Self::LineTooLong {
                line_number,
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "line {line_number} is {actual_bytes} bytes, exceeds configured limit {max_bytes} bytes"
            ),
            Self::TooManyLines {
                actual_lines,
                max_lines,
            } => write!(
                formatter,
                "physical line count {actual_lines} exceeds configured limit {max_lines} lines"
            ),
            Self::RustAstTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "Rust AST input size {actual_bytes} bytes exceeds configured limit {max_bytes} bytes"
            ),
        }
    }
}

impl Error for ResourceLimit {}

impl MemoryLimits {
    pub fn validate(self) -> Result<(), ConfigError> {
        if self.max_file_bytes == 0 {
            return Err(ConfigError(
                "max-file-bytes must be greater than zero".to_string(),
            ));
        }
        if self.max_line_bytes == 0 {
            return Err(ConfigError(
                "max-line-bytes must be greater than zero".to_string(),
            ));
        }
        if self.max_source_lines == 0 {
            return Err(ConfigError(
                "max-source-lines must be greater than zero".to_string(),
            ));
        }
        if self.rust_ast_max_bytes == 0 {
            return Err(ConfigError(
                "rust-ast-max-bytes must be greater than zero".to_string(),
            ));
        }
        if self.max_file_bytes == u64::MAX {
            return Err(ConfigError(
                "max-file-bytes must be less than 18446744073709551615".to_string(),
            ));
        }
        let max_line_bytes = match u64::try_from(self.max_line_bytes) {
            Ok(value) => value,
            Err(_) => {
                return Err(ConfigError(
                    "max-line-bytes cannot be represented as bytes".to_string(),
                ));
            }
        };
        if max_line_bytes > self.max_file_bytes {
            return Err(ConfigError(
                "max-line-bytes cannot be greater than max-file-bytes".to_string(),
            ));
        }
        if self.max_file_bytes.checked_add(1).is_none() {
            return Err(ConfigError(
                "max-file-bytes must allow a one-byte overflow sentinel".to_string(),
            ));
        }
        if self.rust_ast_max_bytes > self.max_file_bytes {
            return Err(ConfigError(
                "rust-ast-max-bytes cannot be greater than max-file-bytes".to_string(),
            ));
        }
        if usize::try_from(self.max_file_bytes).is_err()
            || usize::try_from(self.rust_ast_max_bytes).is_err()
        {
            return Err(ConfigError(
                "memory byte limits cannot be represented on this platform".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParallelOptions {
    pub jobs: usize,
}

impl Default for ParallelOptions {
    fn default() -> Self {
        Self { jobs: DEFAULT_JOBS }
    }
}

impl ParallelOptions {
    pub const fn new(jobs: usize) -> Self {
        Self { jobs }
    }

    pub fn validate(self) -> Result<(), ConfigError> {
        if self.jobs == 0 {
            return Err(ConfigError("jobs must be greater than zero".to_string()));
        }
        if self.jobs > MAX_JOBS {
            return Err(ConfigError(format!(
                "jobs cannot be greater than the product limit of {MAX_JOBS}"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountOptions {
    pub ignore_comments: bool,
    pub ignore_tests: bool,
}

impl Default for CountOptions {
    fn default() -> Self {
        Self {
            ignore_comments: true,
            ignore_tests: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscoveryOptions {
    pub load_gitignore: bool,
    pub load_ignore: bool,
    pub hide_hidden: bool,
    pub load_parents: bool,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            load_gitignore: true,
            load_ignore: true,
            hide_hidden: true,
            load_parents: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineLimits {
    pub max_lines: Option<usize>,
    pub min_lines: Option<usize>,
}

impl LineLimits {
    pub fn validate(self) -> Result<(), ConfigError> {
        if let (Some(min_lines), Some(max_lines)) = (self.min_lines, self.max_lines)
            && min_lines > max_lines
        {
            return Err(ConfigError(
                "min-lines cannot be greater than max-lines".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LintSettings {
    count: CountOptions,
    discovery: DiscoveryOptions,
    limits: LineLimits,
    memory: MemoryLimits,
    parallel: ParallelOptions,
}

impl Default for LintSettings {
    fn default() -> Self {
        Self {
            count: CountOptions::default(),
            discovery: DiscoveryOptions::default(),
            limits: LineLimits {
                max_lines: Some(DEFAULT_MAX_LINES),
                min_lines: None,
            },
            memory: MemoryLimits::default(),
            parallel: ParallelOptions::default(),
        }
    }
}

impl LintSettings {
    pub fn count_options(&self) -> CountOptions {
        self.count
    }

    pub fn discovery_options(&self) -> DiscoveryOptions {
        self.discovery
    }

    pub fn line_limits(&self) -> LineLimits {
        self.limits
    }

    pub fn max_lines(&self) -> Option<usize> {
        self.limits.max_lines
    }

    pub fn min_lines(&self) -> Option<usize> {
        self.limits.min_lines
    }

    pub fn memory_limits(&self) -> MemoryLimits {
        self.memory
    }

    pub fn parallel_options(&self) -> ParallelOptions {
        self.parallel
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.limits.validate()?;
        self.memory.validate()?;
        self.parallel.validate()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CliOverrides {
    pub include_comments: bool,
    pub ignore_comments: bool,
    pub include_tests: bool,
    pub ignore_tests: bool,
    pub gitignore: bool,
    pub no_gitignore: bool,
    pub load_ignore: bool,
    pub no_ignore: bool,
    pub hidden: bool,
    pub show_hidden: bool,
    pub parents: bool,
    pub no_parents: bool,
    pub max_lines: Option<usize>,
    pub min_lines: Option<usize>,
    pub max_file_bytes: Option<u64>,
    pub max_line_bytes: Option<usize>,
    pub max_source_lines: Option<usize>,
    pub rust_ast_max_bytes: Option<u64>,
    pub jobs: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsBuilder {
    settings: LintSettings,
}

impl SettingsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_settings(settings: LintSettings) -> Self {
        Self { settings }
    }

    fn apply(&mut self, values: &ConfigValues, layer: &str) -> Result<(), ConfigError> {
        values.validate_conflicts(layer)?;
        if let Some(value) = values.max_lines {
            self.settings.limits.max_lines = Some(value);
        }
        if let Some(value) = values.min_lines {
            self.settings.limits.min_lines = Some(value);
        }
        if let Some(value) = values.max_file_bytes {
            self.settings.memory.max_file_bytes = value;
        }
        if let Some(value) = values.max_line_bytes {
            self.settings.memory.max_line_bytes = value;
        }
        if let Some(value) = values.max_source_lines {
            self.settings.memory.max_source_lines = value;
        }
        if let Some(value) = values.rust_ast_max_bytes {
            self.settings.memory.rust_ast_max_bytes = value;
        }
        if let Some(value) = values.jobs {
            self.settings.parallel.jobs = value;
        }
        if let Some(value) = values.ignore_comments {
            self.settings.count.ignore_comments = value;
        }
        if let Some(value) = values.include_comments {
            self.settings.count.ignore_comments = !value;
        }
        if let Some(value) = values.ignore_tests {
            self.settings.count.ignore_tests = value;
        }
        if let Some(value) = values.include_tests {
            self.settings.count.ignore_tests = !value;
        }
        if let Some(value) = values.load_gitignore {
            self.settings.discovery.load_gitignore = value;
        }
        if let Some(value) = values.load_ignore {
            self.settings.discovery.load_ignore = value;
        }
        if let Some(value) = values.hide_hidden {
            self.settings.discovery.hide_hidden = value;
        }
        if let Some(value) = values.load_parents {
            self.settings.discovery.load_parents = value;
        }
        Ok(())
    }

    pub fn apply_cli(&mut self, overrides: &CliOverrides) {
        if overrides.include_comments {
            self.settings.count.ignore_comments = false;
        }
        if overrides.ignore_comments {
            self.settings.count.ignore_comments = true;
        }
        if overrides.include_tests {
            self.settings.count.ignore_tests = false;
        }
        if overrides.ignore_tests {
            self.settings.count.ignore_tests = true;
        }
        if overrides.gitignore {
            self.settings.discovery.load_gitignore = true;
        }
        if overrides.no_gitignore {
            self.settings.discovery.load_gitignore = false;
        }
        if overrides.load_ignore {
            self.settings.discovery.load_ignore = true;
        }
        if overrides.no_ignore {
            self.settings.discovery.load_ignore = false;
        }
        if overrides.hidden {
            self.settings.discovery.hide_hidden = true;
        }
        if overrides.show_hidden {
            self.settings.discovery.hide_hidden = false;
        }
        if overrides.parents {
            self.settings.discovery.load_parents = true;
        }
        if overrides.no_parents {
            self.settings.discovery.load_parents = false;
        }
        if let Some(max_lines) = overrides.max_lines {
            self.settings.limits.max_lines = Some(max_lines);
        }
        if let Some(min_lines) = overrides.min_lines {
            self.settings.limits.min_lines = Some(min_lines);
        }
        if let Some(max_file_bytes) = overrides.max_file_bytes {
            self.settings.memory.max_file_bytes = max_file_bytes;
        }
        if let Some(max_line_bytes) = overrides.max_line_bytes {
            self.settings.memory.max_line_bytes = max_line_bytes;
        }
        if let Some(max_source_lines) = overrides.max_source_lines {
            self.settings.memory.max_source_lines = max_source_lines;
        }
        if let Some(rust_ast_max_bytes) = overrides.rust_ast_max_bytes {
            self.settings.memory.rust_ast_max_bytes = rust_ast_max_bytes;
        }
        if let Some(jobs) = overrides.jobs {
            self.settings.parallel.jobs = jobs;
        }
    }

    pub fn build(self) -> Result<LintSettings, ConfigError> {
        self.settings.validate()?;
        Ok(self.settings)
    }
}

#[derive(Debug)]
pub struct ConfigError(String);

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for ConfigError {}

#[derive(Debug, Default, Deserialize)]
struct TomlConfig {
    #[serde(flatten)]
    root: ConfigValues,
    #[serde(default)]
    settings: ConfigValues,
    #[serde(default, rename = "line-lint", alias = "line_lint")]
    line_lint: ConfigValues,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigValues {
    #[serde(alias = "max-lines")]
    max_lines: Option<usize>,
    #[serde(alias = "min-lines")]
    min_lines: Option<usize>,
    #[serde(alias = "max-file-bytes")]
    max_file_bytes: Option<u64>,
    #[serde(alias = "max-line-bytes")]
    max_line_bytes: Option<usize>,
    #[serde(alias = "max-source-lines")]
    max_source_lines: Option<usize>,
    #[serde(alias = "rust-ast-max-bytes")]
    rust_ast_max_bytes: Option<u64>,
    jobs: Option<usize>,
    #[serde(alias = "comments", alias = "ignore-comments")]
    ignore_comments: Option<bool>,
    #[serde(alias = "include-comments")]
    include_comments: Option<bool>,
    #[serde(alias = "tests", alias = "ignore-tests")]
    ignore_tests: Option<bool>,
    #[serde(alias = "include-tests")]
    include_tests: Option<bool>,
    #[serde(alias = "gitignore", alias = "use_gitignore", alias = "load-gitignore")]
    load_gitignore: Option<bool>,
    #[serde(alias = "ignore_file", alias = "use_ignore", alias = "load-ignore")]
    load_ignore: Option<bool>,
    #[serde(alias = "hidden", alias = "hidden_files", alias = "hide_hidden")]
    hide_hidden: Option<bool>,
    #[serde(alias = "parents", alias = "parent_ignore", alias = "load-parents")]
    load_parents: Option<bool>,
}

impl ConfigValues {
    fn validate_conflicts(&self, layer: &str) -> Result<(), ConfigError> {
        if self.ignore_comments.is_some() && self.include_comments.is_some() {
            return Err(ConfigError(format!(
                "{layer} sets both ignore-comments and include-comments"
            )));
        }
        if self.ignore_tests.is_some() && self.include_tests.is_some() {
            return Err(ConfigError(format!(
                "{layer} sets both ignore-tests and include-tests"
            )));
        }
        Ok(())
    }
}

pub struct ConfigLoader {
    input: PathBuf,
    explicit_path: Option<PathBuf>,
    overrides: CliOverrides,
}

impl ConfigLoader {
    pub fn new(input: &Path) -> Self {
        Self {
            input: input.to_path_buf(),
            explicit_path: None,
            overrides: CliOverrides::default(),
        }
    }

    pub fn explicit_path(mut self, path: Option<&Path>) -> Self {
        self.explicit_path = path.map(Path::to_path_buf);
        self
    }

    pub fn overrides(mut self, overrides: CliOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    pub fn load(self) -> Result<LintSettings, ConfigError> {
        let config_path = self.explicit_path.or_else(|| find_config(&self.input));
        let Some(config_path) = config_path else {
            let mut builder = SettingsBuilder::new();
            builder.apply_cli(&self.overrides);
            return builder.build();
        };

        let source = fs::read_to_string(&config_path).map_err(|error| {
            ConfigError(format!("cannot read {}: {error}", config_path.display()))
        })?;
        let config: TomlConfig = toml::from_str(&source).map_err(|error| {
            ConfigError(format!("cannot parse {}: {error}", config_path.display()))
        })?;
        let mut builder = SettingsBuilder::new();
        builder.apply(&config.root, "root configuration layer")?;
        builder.apply(&config.settings, "settings configuration layer")?;
        builder.apply(&config.line_lint, "line-lint configuration layer")?;
        builder.apply_cli(&self.overrides);
        builder.build()
    }
}

pub fn load(input: &Path, explicit_path: Option<&Path>) -> Result<LintSettings, ConfigError> {
    ConfigLoader::new(input).explicit_path(explicit_path).load()
}

fn find_config(input: &Path) -> Option<PathBuf> {
    let start = input_directory(input);
    let mut current = Some(start.as_path());
    while let Some(directory) = current {
        for file_name in [".line-lint.toml", "line-lint.toml"] {
            let candidate = directory.join(file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        current = directory.parent();
    }
    None
}

fn input_directory(input: &Path) -> PathBuf {
    match fs::metadata(input) {
        Ok(metadata) if metadata.is_dir() => input.to_path_buf(),
        _ => input
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        CliOverrides, ConfigLoader, CountOptions, DEFAULT_JOBS, DEFAULT_MAX_FILE_BYTES,
        DEFAULT_MAX_LINE_BYTES, DEFAULT_MAX_LINES, DEFAULT_MAX_SOURCE_LINES,
        DEFAULT_RUST_AST_MAX_BYTES, LintSettings, MAX_JOBS, MemoryLimits, ParallelOptions,
        ResourceLimit, SettingsBuilder, load,
    };
    use tempfile::tempdir;

    #[test]
    fn defaults_match_documented_behavior() {
        let settings = LintSettings::default();

        assert_eq!(settings.max_lines(), Some(DEFAULT_MAX_LINES));
        assert_eq!(settings.min_lines(), None);
        assert_eq!(settings.count_options(), CountOptions::default());
        assert!(settings.discovery_options().load_gitignore);
        assert!(settings.discovery_options().load_ignore);
        assert!(settings.discovery_options().hide_hidden);
        assert!(settings.discovery_options().load_parents);
        assert_eq!(settings.memory_limits(), MemoryLimits::default());
        assert_eq!(settings.parallel_options(), ParallelOptions::default());
        assert_eq!(
            settings.memory_limits().max_file_bytes,
            DEFAULT_MAX_FILE_BYTES
        );
        assert_eq!(
            settings.memory_limits().max_line_bytes,
            DEFAULT_MAX_LINE_BYTES
        );
        assert_eq!(
            settings.memory_limits().max_source_lines,
            DEFAULT_MAX_SOURCE_LINES
        );
        assert_eq!(
            settings.memory_limits().rust_ast_max_bytes,
            DEFAULT_RUST_AST_MAX_BYTES
        );
        assert_eq!(settings.parallel_options().jobs, DEFAULT_JOBS);
        settings.validate().expect("default settings are valid");
    }

    #[test]
    fn loads_flat_and_nested_toml_settings() {
        let directory = tempdir().expect("create temporary directory");
        let config_path = directory.path().join("line-lint.toml");
        fs::write(
            &config_path,
            "max_lines = 900\n\n[settings]\nmin_lines = 20\nignore_comments = false\n\n[line_lint]\nhide_hidden = false\nload_gitignore = false\n",
        )
        .expect("write configuration");

        let settings = load(directory.path(), None).expect("load configuration");

        assert_eq!(settings.max_lines(), Some(900));
        assert_eq!(settings.min_lines(), Some(20));
        assert!(!settings.count_options().ignore_comments);
        assert!(!settings.discovery_options().hide_hidden);
        assert!(!settings.discovery_options().load_gitignore);
        assert!(settings.discovery_options().load_ignore);
    }

    #[test]
    fn explicit_config_path_takes_precedence() {
        let directory = tempdir().expect("create temporary directory");
        let auto_path = directory.path().join(".line-lint.toml");
        let explicit_path = directory.path().join("custom.toml");
        fs::write(&auto_path, "max_lines = 100\n").expect("write automatic config");
        fs::write(&explicit_path, "max_lines = 200\n").expect("write explicit config");

        let settings = ConfigLoader::new(directory.path())
            .explicit_path(Some(&explicit_path))
            .load()
            .expect("load config");

        assert_eq!(settings.max_lines(), Some(200));
    }

    #[test]
    fn rejects_invalid_line_range() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(
            directory.path().join("line-lint.toml"),
            "min_lines = 20\nmax_lines = 10\n",
        )
        .expect("write configuration");

        let error = load(directory.path(), None).expect_err("invalid range must fail");

        assert_eq!(
            error.to_string(),
            "min-lines cannot be greater than max-lines"
        );
    }

    #[test]
    fn rejects_conflicting_options_in_one_layer() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(
            directory.path().join("line-lint.toml"),
            "ignore_comments = true\ninclude_comments = true\n",
        )
        .expect("write configuration");

        let error = load(directory.path(), None).expect_err("conflict must fail");

        assert!(
            error
                .to_string()
                .contains("both ignore-comments and include-comments")
        );
    }

    #[test]
    fn cli_overrides_are_applied_after_configuration() {
        let mut builder = SettingsBuilder::new();
        builder.apply_cli(&CliOverrides {
            include_comments: true,
            max_lines: Some(12),
            max_file_bytes: Some(4096),
            max_line_bytes: Some(1024),
            rust_ast_max_bytes: Some(2048),
            jobs: Some(2),
            ..CliOverrides::default()
        });
        let settings = builder.build().expect("build settings");

        assert!(!settings.count_options().ignore_comments);
        assert_eq!(settings.max_lines(), Some(12));
        assert_eq!(settings.memory_limits().max_file_bytes, 4096);
        assert_eq!(settings.parallel_options().jobs, 2);
    }

    #[test]
    fn loads_resource_options_from_line_lint_layer() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(
            directory.path().join("line-lint.toml"),
            "[line-lint]\nmax-file-bytes = 4096\nmax-line-bytes = 512\nmax-source-lines = 99\nrust-ast-max-bytes = 2048\njobs = 2\n",
        )
        .expect("write configuration");

        let settings = load(directory.path(), None).expect("load resource configuration");

        assert_eq!(
            settings.memory_limits(),
            MemoryLimits {
                max_file_bytes: 4096,
                max_line_bytes: 512,
                max_source_lines: 99,
                rust_ast_max_bytes: 2048,
            }
        );
        assert_eq!(settings.parallel_options(), ParallelOptions::new(2));
    }

    #[test]
    fn rejects_zero_and_conflicting_resource_limits() {
        let limits = MemoryLimits {
            max_file_bytes: 0,
            ..MemoryLimits::default()
        };
        assert_eq!(
            limits
                .validate()
                .expect_err("zero file limit must fail")
                .to_string(),
            "max-file-bytes must be greater than zero"
        );

        let limits = MemoryLimits {
            max_line_bytes: usize::try_from(DEFAULT_MAX_FILE_BYTES)
                .expect("default file limit fits usize")
                + 1,
            ..MemoryLimits::default()
        };
        assert_eq!(
            limits
                .validate()
                .expect_err("line limit must fit file limit")
                .to_string(),
            "max-line-bytes cannot be greater than max-file-bytes"
        );
    }

    #[test]
    fn rejects_u64_max_file_limit_before_checked_add() {
        let limits = MemoryLimits {
            max_file_bytes: u64::MAX,
            ..MemoryLimits::default()
        };

        assert_eq!(
            limits
                .validate()
                .expect_err("u64::MAX cannot provide a sentinel byte")
                .to_string(),
            "max-file-bytes must be less than 18446744073709551615"
        );
    }

    #[test]
    fn rejects_invalid_parallel_options() {
        assert_eq!(
            ParallelOptions::new(0)
                .validate()
                .expect_err("zero jobs must fail")
                .to_string(),
            "jobs must be greater than zero"
        );
        assert_eq!(
            ParallelOptions::new(MAX_JOBS + 1)
                .validate()
                .expect_err("too many jobs must fail")
                .to_string(),
            "jobs cannot be greater than the product limit of 64"
        );
    }

    #[test]
    fn formats_resource_limit_diagnostics() {
        assert_eq!(
            ResourceLimit::LineTooLong {
                line_number: 4,
                actual_bytes: 2049,
                max_bytes: 2048,
            }
            .to_string(),
            "line 4 is 2049 bytes, exceeds configured limit 2048 bytes"
        );
        assert_eq!(
            ResourceLimit::TooManyLines {
                actual_lines: 101,
                max_lines: 100,
            }
            .to_string(),
            "physical line count 101 exceeds configured limit 100 lines"
        );
        assert_eq!(
            ResourceLimit::RustAstTooLarge {
                actual_bytes: 8193,
                max_bytes: 8192,
            }
            .to_string(),
            "Rust AST input size 8193 bytes exceeds configured limit 8192 bytes"
        );
    }
}
