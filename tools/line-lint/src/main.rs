use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{ArgAction, Args, Parser, Subcommand};

use line_lint::{
    CliOverrides, ConfigLoader, FileReport, LineLimits, LintSettings, lint, lint_with_report_limit,
    sort_reports,
};

#[derive(Debug)]
struct ToolError(String);

impl Display for ToolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for ToolError {}

#[derive(Debug, Parser)]
#[command(name = "line-lint", version, about = "Lint file line counts")]
struct Cli {
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short = 'j', long = "jobs", global = true, value_name = "N")]
    jobs: Option<usize>,

    #[arg(
        long = "include-comments",
        alias = "no-ignore-comments",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "ignore_comments"
    )]
    include_comments: bool,

    #[arg(
        long = "ignore-comments",
        global = true,
        action = ArgAction::SetTrue,
        hide = true,
        conflicts_with = "include_comments"
    )]
    ignore_comments: bool,

    #[arg(
        long = "include-tests",
        alias = "no-ignore-tests",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "ignore_tests"
    )]
    include_tests: bool,

    #[arg(
        long = "ignore-tests",
        global = true,
        action = ArgAction::SetTrue,
        hide = true,
        conflicts_with = "include_tests"
    )]
    ignore_tests: bool,

    #[arg(
        long = "gitignore",
        alias = "load-gitignore",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "no_gitignore"
    )]
    gitignore: bool,

    #[arg(
        long = "no-gitignore",
        global = true,
        action = ArgAction::SetTrue,
        hide = true,
        conflicts_with = "gitignore"
    )]
    no_gitignore: bool,

    #[arg(
        long = "load-ignore",
        alias = "use-ignore",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "no_ignore"
    )]
    load_ignore: bool,

    #[arg(
        long = "no-ignore",
        global = true,
        action = ArgAction::SetTrue,
        hide = true,
        conflicts_with = "load_ignore"
    )]
    no_ignore: bool,

    #[arg(
        long = "hidden",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "show_hidden"
    )]
    hidden: bool,

    #[arg(
        long = "show-hidden",
        alias = "no-hidden",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "hidden"
    )]
    show_hidden: bool,

    #[arg(
        long = "parents",
        alias = "load-parents",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "no_parents"
    )]
    parents: bool,

    #[arg(
        long = "no-parents",
        global = true,
        action = ArgAction::SetTrue,
        hide = true,
        conflicts_with = "parents"
    )]
    no_parents: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List(ListArgs),
    Check(CheckArgs),
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    input: PathBuf,
    #[arg(short = 'n', long = "limit", value_name = "N")]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    input: PathBuf,
    #[arg(long = "max-lines", value_name = "N")]
    max_lines: Option<usize>,
    #[arg(long = "min-lines", value_name = "N")]
    min_lines: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
struct LimitOverrides {
    max_lines: Option<usize>,
    min_lines: Option<usize>,
}

struct CommandRunner;

impl CommandRunner {
    fn run(cli: &Cli) -> Result<(), ToolError> {
        match &cli.command {
            Command::List(arguments) => ListCommand::run(cli, arguments),
            Command::Check(arguments) => CheckCommand::run(cli, arguments),
        }
    }
}

struct ListCommand;

impl ListCommand {
    fn run(cli: &Cli, arguments: &ListArgs) -> Result<(), ToolError> {
        let settings = load_settings(cli, &arguments.input, LimitOverrides::default())?;
        let mut reports = match arguments.limit {
            Some(limit) => lint_with_report_limit(&arguments.input, &settings, limit),
            None => lint(&arguments.input, &settings),
        }
        .map_err(|error| ToolError(error.to_string()))?;
        sort_reports(&mut reports);
        print_reports(&reports, &arguments.input);
        Ok(())
    }
}

struct CheckCommand;

impl CheckCommand {
    fn run(cli: &Cli, arguments: &CheckArgs) -> Result<(), ToolError> {
        let settings = load_settings(
            cli,
            &arguments.input,
            LimitOverrides {
                max_lines: arguments.max_lines,
                min_lines: arguments.min_lines,
            },
        )?;
        let mut reports =
            lint(&arguments.input, &settings).map_err(|error| ToolError(error.to_string()))?;
        sort_reports(&mut reports);
        print_reports(&reports, &arguments.input);

        let policy = CheckPolicy::new(settings.line_limits());
        let violations = reports
            .iter()
            .filter(|report| policy.is_violation(report))
            .count();
        if violations > 0 {
            return Err(ToolError(format!(
                "line limits violated by {violations} file(s)"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckPolicy {
    limits: LineLimits,
}

impl CheckPolicy {
    fn new(limits: LineLimits) -> Self {
        Self { limits }
    }

    fn is_violation(&self, report: &FileReport) -> bool {
        self.limits
            .max_lines
            .is_some_and(|max_lines| report.line_count > max_lines)
            || self
                .limits
                .min_lines
                .is_some_and(|min_lines| report.line_count < min_lines)
    }
}

fn load_settings(
    cli: &Cli,
    input: &Path,
    limits: LimitOverrides,
) -> Result<LintSettings, ToolError> {
    let overrides = CliOverrides {
        include_comments: cli.include_comments,
        ignore_comments: cli.ignore_comments,
        include_tests: cli.include_tests,
        ignore_tests: cli.ignore_tests,
        gitignore: cli.gitignore,
        no_gitignore: cli.no_gitignore,
        load_ignore: cli.load_ignore,
        no_ignore: cli.no_ignore,
        hidden: cli.hidden,
        show_hidden: cli.show_hidden,
        parents: cli.parents,
        no_parents: cli.no_parents,
        max_lines: limits.max_lines,
        min_lines: limits.min_lines,
        max_file_bytes: None,
        max_line_bytes: None,
        max_source_lines: None,
        rust_ast_max_bytes: None,
        jobs: cli.jobs,
    };
    ConfigLoader::new(input)
        .explicit_path(cli.config.as_deref())
        .overrides(overrides)
        .load()
        .map_err(|error| ToolError(error.to_string()))
}

fn print_reports(reports: &[FileReport], input: &Path) {
    for report in reports {
        println!(
            "{}: {}",
            display_path(&report.path, input).display(),
            report.line_count
        );
    }
}

fn display_path(path: &Path, input: &Path) -> PathBuf {
    if input.is_dir() {
        path.strip_prefix(input)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match CommandRunner::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("line-lint: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::{
        CheckArgs, CheckPolicy, Cli, Command, FileReport, LineLimits, ListArgs, sort_reports,
    };

    #[test]
    fn parses_list_and_check_commands() {
        let cli = Cli::try_parse_from(["line-lint", "list", "-n", "30", "src"])
            .expect("parse list command");
        assert!(matches!(
            cli.command,
            Command::List(ListArgs {
                limit: Some(30),
                ..
            })
        ));

        let cli = Cli::try_parse_from([
            "line-lint",
            "check",
            "--max-lines",
            "650",
            "--min-lines",
            "50",
            "src",
        ])
        .expect("parse check command");
        assert!(matches!(
            cli.command,
            Command::Check(CheckArgs {
                max_lines: Some(650),
                min_lines: Some(50),
                ..
            })
        ));

        let cli = Cli::try_parse_from(["line-lint", "-j", "2", "list", "src"])
            .expect("parse jobs option");
        assert_eq!(cli.jobs, Some(2));
    }

    #[test]
    fn sorts_by_line_count_then_path() {
        let mut reports = vec![
            FileReport {
                path: PathBuf::from("b.rs"),
                line_count: 10,
            },
            FileReport {
                path: PathBuf::from("a.rs"),
                line_count: 10,
            },
            FileReport {
                path: PathBuf::from("c.rs"),
                line_count: 20,
            },
        ];

        sort_reports(&mut reports);

        assert_eq!(
            reports
                .iter()
                .map(|report| report.path.as_path())
                .collect::<Vec<_>>(),
            [
                PathBuf::from("c.rs"),
                PathBuf::from("a.rs"),
                PathBuf::from("b.rs")
            ]
            .iter()
            .map(PathBuf::as_path)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn checks_both_limits() {
        let policy = CheckPolicy::new(LineLimits {
            max_lines: Some(10),
            min_lines: Some(5),
        });
        assert!(policy.is_violation(&FileReport {
            path: PathBuf::from("long.rs"),
            line_count: 11,
        }));
        assert!(policy.is_violation(&FileReport {
            path: PathBuf::from("short.rs"),
            line_count: 4,
        }));
        assert!(!policy.is_violation(&FileReport {
            path: PathBuf::from("valid.rs"),
            line_count: 7,
        }));
    }
}
