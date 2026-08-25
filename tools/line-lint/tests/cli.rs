use std::{fs, path::Path, process::Command};

use tempfile::{TempDir, tempdir};

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("write fixture");
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_line-lint"))
}

fn create_directory() -> TempDir {
    tempdir().expect("create temporary directory")
}

#[test]
fn list_sorts_by_count_and_honors_limit() {
    let directory = create_directory();
    write_file(&directory.path().join("short.rs"), "fn short() {}\n");
    write_file(
        &directory.path().join("long.rs"),
        "fn one() {}\nfn two() {}\nfn three() {}\n",
    );

    let output = binary()
        .args(["list", "-n", "1"])
        .arg(directory.path())
        .output()
        .expect("run line-lint");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "long.rs: 3\n");
}

#[test]
fn list_limit_zero_succeeds_without_output_when_reports_exist() {
    let directory = create_directory();
    write_file(&directory.path().join("source.rs"), "fn source() {}\n");

    let output = binary()
        .args(["list", "--limit", "0"])
        .arg(directory.path())
        .output()
        .expect("run line-lint with zero report limit");

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn list_output_is_stable_for_equal_counts() {
    let directory = create_directory();
    write_file(&directory.path().join("z-last.rs"), "fn last() {}\n");
    write_file(&directory.path().join("a-first.rs"), "fn first() {}\n");

    let output = binary()
        .args(["list"])
        .arg(directory.path())
        .output()
        .expect("run line-lint");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a-first.rs: 1\nz-last.rs: 1\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn check_preserves_success_exit_code_and_output() {
    let directory = create_directory();
    write_file(&directory.path().join("valid.rs"), "fn valid() {}\n");

    let output = binary()
        .args(["check", "--max-lines", "2"])
        .arg(directory.path())
        .output()
        .expect("run line-lint");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "valid.rs: 1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn check_fails_when_a_file_exceeds_maximum() {
    let directory = create_directory();
    write_file(
        &directory.path().join("long.rs"),
        "fn one() {}\nfn two() {}\nfn three() {}\n",
    );

    let output = binary()
        .args(["check", "--max-lines", "2"])
        .arg(directory.path())
        .output()
        .expect("run line-lint");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("long.rs: 3"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("line limits violated"));
}

#[test]
fn config_and_include_comments_flag_change_counting() {
    let directory = create_directory();
    let source_path = directory.path().join("source.py");
    write_file(&source_path, "# comment\nvalue = 1\n");
    let config_path = directory.path().join("custom.toml");
    write_file(&config_path, "ignore_comments = false\n");

    let configured = binary()
        .args(["list", "--config"])
        .arg(&config_path)
        .arg(&source_path)
        .output()
        .expect("run configured line-lint");
    assert!(configured.status.success());
    assert!(String::from_utf8_lossy(&configured.stdout).contains("source.py: 2"));

    let default = binary()
        .args(["list"])
        .arg(&source_path)
        .output()
        .expect("run default line-lint");
    assert!(default.status.success());
    assert!(String::from_utf8_lossy(&default.stdout).contains("source.py: 1"));

    let included = binary()
        .args(["--include-comments", "list"])
        .arg(&source_path)
        .output()
        .expect("run line-lint with comments");
    assert!(included.status.success());
    assert!(String::from_utf8_lossy(&included.stdout).contains("source.py: 2"));
}

#[test]
fn empty_supported_input_returns_failure_with_stable_diagnostic() {
    let directory = create_directory();
    write_file(&directory.path().join("README"), "plain text\n");

    let output = binary()
        .args(["list"])
        .arg(directory.path())
        .output()
        .expect("run line-lint");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "line-lint: input contains no text files: {}\n",
            directory.path().display()
        )
    );
}

#[test]
fn missing_input_returns_failure_with_error_prefix() {
    let directory = create_directory();
    let missing = directory.path().join("missing.rs");

    let output = binary()
        .args(["list"])
        .arg(&missing)
        .output()
        .expect("run line-lint");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("line-lint: "));
}

#[test]
fn jobs_one_and_many_have_identical_reports_and_exit_codes() {
    let directory = create_directory();
    for (name, source) in [
        ("first.rs", "fn first() {}\n"),
        ("second.rs", "fn one() {}\nfn two() {}\n"),
        ("third.py", "value = 1\nvalue = 2\nvalue = 3\n"),
    ] {
        write_file(&directory.path().join(name), source);
    }

    let one = binary()
        .args(["--jobs", "1", "list"])
        .arg(directory.path())
        .output()
        .expect("run line-lint with one job");
    let many = binary()
        .args(["--jobs", "4", "list"])
        .arg(directory.path())
        .output()
        .expect("run line-lint with many jobs");

    assert_eq!(one.status.success(), many.status.success());
    assert_eq!(one.stdout, many.stdout);
    assert_eq!(one.stderr, many.stderr);
}

#[test]
fn resource_error_has_identical_failure_for_one_and_many_jobs() {
    let directory = create_directory();
    write_file(
        &directory.path().join(".line-lint.toml"),
        "max_file_bytes = 12\nmax_line_bytes = 12\nrust_ast_max_bytes = 12\n",
    );
    write_file(&directory.path().join("normal.rs"), "fn x() {}\n");
    write_file(
        &directory.path().join("oversized.rs"),
        "fn one() {}\nfn two() {}\n",
    );

    let one = binary()
        .args(["--jobs", "1", "list"])
        .arg(directory.path())
        .output()
        .expect("run resource test with one job");
    let many = binary()
        .args(["--jobs", "4", "list"])
        .arg(directory.path())
        .output()
        .expect("run resource test with many jobs");

    assert!(!one.status.success());
    assert!(!many.status.success());
    assert_eq!(one.status.code(), many.status.code());
    assert_eq!(one.stdout, many.stdout);
    assert_eq!(one.stderr, many.stderr);
    assert!(String::from_utf8_lossy(&one.stderr).contains("oversized.rs"));
    assert!(String::from_utf8_lossy(&one.stderr).contains("exceeds configured limit"));
}

#[test]
fn jobs_zero_is_rejected_before_analysis() {
    let directory = create_directory();
    write_file(&directory.path().join("source.rs"), "fn main() {}\n");

    let output = binary()
        .args(["--jobs", "0", "list"])
        .arg(directory.path())
        .output()
        .expect("run invalid jobs command");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("jobs must be greater than zero"));
}
