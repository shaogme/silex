use std::{
    cmp::Ordering as CompareOrdering,
    collections::BinaryHeap,
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::sync_channel,
    },
    thread,
};

use rayon::iter::{ParallelBridge, ParallelIterator};
use rayon::{ThreadPool, ThreadPoolBuilder};

pub mod config;
pub mod files;
pub mod language;
pub mod line_count;

pub use config::{
    CliOverrides, ConfigError, ConfigLoader, CountOptions, DiscoveryOptions, LineLimits,
    LintSettings, MAX_JOBS, MemoryLimits, ParallelOptions, ResourceLimit, SettingsBuilder,
};
pub use language::{
    LanguageAnalyzer, LanguageContext, LanguageDescriptor, LanguageError, LanguageId,
    LanguageRegistry, LanguageReport, SourceDocument,
};
pub use line_count::{
    AnalysisOutcome, FileAnalysisError, FileAnalyzer, FileReport, LineCountError,
};

pub type Settings = LintSettings;

pub const MAX_REPORTS: usize = 100_000;
const MAX_ERROR_DIAGNOSTICS: usize = 64;

#[derive(Debug)]
pub struct LintError(String);

impl Display for LintError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for LintError {}

#[derive(Clone)]
pub struct LintEngine {
    settings: LintSettings,
    collector: files::FileCollector,
    analyzer: FileAnalyzer,
}

impl LintEngine {
    pub fn new(settings: LintSettings) -> Self {
        let collector = files::FileCollector::from_settings(&settings);
        let analyzer = FileAnalyzer::new(LanguageRegistry::new());
        Self {
            settings,
            collector,
            analyzer,
        }
    }

    pub fn run(&self, input: &Path) -> Result<Vec<FileReport>, LintError> {
        self.run_with_sink(input, ReportSinkMode::Complete(MAX_REPORTS))
    }

    pub fn run_with_report_limit(
        &self,
        input: &Path,
        limit: usize,
    ) -> Result<Vec<FileReport>, LintError> {
        if limit > MAX_REPORTS {
            return Err(LintError(format!(
                "report limit {limit} exceeds the maximum of {MAX_REPORTS}"
            )));
        }
        self.run_with_sink(input, ReportSinkMode::TopN(limit))
    }

    fn run_with_sink(
        &self,
        input: &Path,
        sink_mode: ReportSinkMode,
    ) -> Result<Vec<FileReport>, LintError> {
        let walker = self
            .collector
            .walk(input)
            .map_err(|error| LintError(error.to_string()))?;
        let pool = build_thread_pool(self.settings.parallel_options().jobs)?;
        let queue_capacity = effective_jobs(self.settings.parallel_options().jobs)
            .saturating_mul(2)
            .max(1);
        let (sender, receiver) = sync_channel(queue_capacity);
        let cancelled = Arc::new(AtomicBool::new(false));
        let discovered_file = Arc::new(AtomicBool::new(false));
        let walker_error = Arc::new(Mutex::new(None));
        let sink = Arc::new(ReportSink::new(sink_mode, Arc::clone(&cancelled)));
        let sink_failed = Arc::new(AtomicBool::new(false));
        let producer_cancelled = Arc::clone(&cancelled);
        let producer_discovered_file = Arc::clone(&discovered_file);
        let producer_error = Arc::clone(&walker_error);
        let producer = thread::spawn(move || {
            for result in walker {
                if producer_cancelled.load(Ordering::Acquire) {
                    break;
                }
                match result {
                    Ok(source_path) => {
                        producer_discovered_file.store(true, Ordering::Release);
                        if sender.send(source_path).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        if let Ok(mut slot) = producer_error.lock() {
                            *slot = Some(error);
                        }
                        producer_cancelled.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        });

        let analyzer = self.analyzer;
        let options = self.settings.count_options();
        let limits = self.settings.memory_limits();
        let worker_cancelled = Arc::clone(&cancelled);
        let worker_sink = Arc::clone(&sink);
        let worker_sink_failed = Arc::clone(&sink_failed);
        pool.install(|| {
            receiver.into_iter().par_bridge().for_each(|source_path| {
                if worker_cancelled.load(Ordering::Acquire) {
                    return;
                }
                let result = analyzer.analyze_with_limits(source_path.as_path(), &options, &limits);
                let item = match result {
                    Ok(AnalysisOutcome::Report(report)) => SinkItem::Report(report),
                    Ok(AnalysisOutcome::Skip) => SinkItem::Skip,
                    Err(error) => SinkItem::Error(error),
                };
                if worker_sink.submit(item).is_err() {
                    worker_sink_failed.store(true, Ordering::Release);
                    worker_cancelled.store(true, Ordering::Release);
                }
            });
        });
        drop(worker_sink);
        producer
            .join()
            .map_err(|_| LintError("file walker thread panicked".to_string()))?;

        let walker_error = walker_error
            .lock()
            .map_err(|_| LintError("file walker state was poisoned".to_string()))?
            .take();
        if let Some(error) = walker_error {
            return Err(LintError(error.to_string()));
        }

        if sink_failed.load(Ordering::Acquire) {
            return Err(LintError("report sink state was poisoned".to_string()));
        }

        let sink = Arc::try_unwrap(sink)
            .map_err(|_| LintError("report sink still has active workers".to_string()))?;
        let output = sink.into_output()?;
        if let Some(error) = output.error {
            return Err(LintError(error.to_string()));
        }
        if output.report_limit_exceeded {
            return Err(LintError(format!(
                "report count exceeds the configured maximum of {MAX_REPORTS}"
            )));
        }

        let mut reports = output.reports;
        reports.sort_by(|left, right| left.path.cmp(&right.path));
        if reports.is_empty() {
            if output.report_seen {
                return Ok(reports);
            }
            let message = if discovered_file.load(Ordering::Acquire) {
                "input contains no text files"
            } else {
                "input contains no files"
            };
            return Err(LintError(format!("{message}: {}", input.display())));
        }
        Ok(reports)
    }
}

fn build_thread_pool(requested_jobs: usize) -> Result<ThreadPool, LintError> {
    ThreadPoolBuilder::new()
        .num_threads(effective_jobs(requested_jobs))
        .build()
        .map_err(|error| LintError(format!("cannot create analysis thread pool: {error}")))
}

fn effective_jobs(requested_jobs: usize) -> usize {
    let available_jobs = thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1);
    requested_jobs.min(available_jobs).clamp(1, MAX_JOBS)
}

pub fn lint(input: &Path, settings: &LintSettings) -> Result<Vec<FileReport>, LintError> {
    LintEngine::new(settings.clone()).run(input)
}

pub fn lint_with_report_limit(
    input: &Path,
    settings: &LintSettings,
    limit: usize,
) -> Result<Vec<FileReport>, LintError> {
    LintEngine::new(settings.clone()).run_with_report_limit(input, limit)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReportSorter;

impl ReportSorter {
    pub fn sort(reports: &mut [FileReport]) {
        reports.sort_by(|left, right| {
            right
                .line_count
                .cmp(&left.line_count)
                .then_with(|| left.path.cmp(&right.path))
        });
    }
}

pub fn sort_reports(reports: &mut [FileReport]) {
    ReportSorter::sort(reports);
}

#[derive(Clone, Copy, Debug)]
enum ReportSinkMode {
    TopN(usize),
    Complete(usize),
}

#[derive(Debug, Eq, PartialEq)]
struct ReportEntry(FileReport);

impl Ord for ReportEntry {
    fn cmp(&self, other: &Self) -> CompareOrdering {
        self.0
            .line_count
            .cmp(&other.0.line_count)
            .reverse()
            .then_with(|| self.0.path.cmp(&other.0.path))
    }
}

impl PartialOrd for ReportEntry {
    fn partial_cmp(&self, other: &Self) -> Option<CompareOrdering> {
        Some(self.cmp(other))
    }
}

enum ReportStore {
    TopN {
        reports: BinaryHeap<ReportEntry>,
        limit: usize,
    },
    Complete {
        reports: Vec<FileReport>,
        max_reports: usize,
    },
}

struct ReportSinkState {
    reports: ReportStore,
    report_seen: bool,
    report_limit_exceeded: bool,
    errors: ErrorStore,
}

struct ReportSink {
    state: Mutex<ReportSinkState>,
    cancelled: Arc<AtomicBool>,
}

impl ReportSink {
    fn new(mode: ReportSinkMode, cancelled: Arc<AtomicBool>) -> Self {
        let reports = match mode {
            ReportSinkMode::TopN(limit) => ReportStore::TopN {
                reports: BinaryHeap::with_capacity(limit),
                limit,
            },
            ReportSinkMode::Complete(max_reports) => ReportStore::Complete {
                reports: Vec::with_capacity(max_reports),
                max_reports,
            },
        };
        Self {
            state: Mutex::new(ReportSinkState {
                reports,
                report_seen: false,
                report_limit_exceeded: false,
                errors: ErrorStore::default(),
            }),
            cancelled,
        }
    }

    fn submit(&self, item: SinkItem) -> Result<(), ()> {
        let resource_error = matches!(
            &item,
            SinkItem::Error(FileAnalysisError::ResourceLimit { .. })
        );
        if resource_error {
            self.cancelled.store(true, Ordering::Release);
        }

        let mut state = self.state.lock().map_err(|_| ())?;
        match item {
            SinkItem::Report(report) => {
                state.report_seen = true;
                let limit_exceeded = match &mut state.reports {
                    ReportStore::TopN { reports, limit } => {
                        insert_top_report(reports, *limit, report);
                        false
                    }
                    ReportStore::Complete {
                        reports,
                        max_reports,
                    } => {
                        if reports.len() >= *max_reports {
                            true
                        } else {
                            reports.push(report);
                            false
                        }
                    }
                };
                if limit_exceeded {
                    state.report_limit_exceeded = true;
                    self.cancelled.store(true, Ordering::Release);
                }
            }
            SinkItem::Skip => {}
            SinkItem::Error(error) => state.errors.record(error),
        }
        Ok(())
    }

    fn into_output(self) -> Result<SinkOutput, LintError> {
        let state = self
            .state
            .into_inner()
            .map_err(|_| LintError("report sink state was poisoned".to_string()))?;
        Ok(SinkOutput {
            reports: state.reports.into_reports(),
            report_seen: state.report_seen,
            report_limit_exceeded: state.report_limit_exceeded,
            error: state.errors.selected(),
        })
    }
}

enum SinkItem {
    Report(FileReport),
    Skip,
    Error(FileAnalysisError),
}

struct SinkOutput {
    reports: Vec<FileReport>,
    report_seen: bool,
    report_limit_exceeded: bool,
    error: Option<FileAnalysisError>,
}

impl ReportStore {
    fn into_reports(self) -> Vec<FileReport> {
        match self {
            Self::TopN { reports, .. } => reports.into_iter().map(|entry| entry.0).collect(),
            Self::Complete { reports, .. } => reports,
        }
    }
}

fn insert_top_report(reports: &mut BinaryHeap<ReportEntry>, limit: usize, report: FileReport) {
    if reports.len() < limit {
        reports.push(ReportEntry(report));
        return;
    }
    let Some(worst) = reports.peek() else {
        return;
    };
    if report_precedes(&report, &worst.0) {
        reports.pop();
        reports.push(ReportEntry(report));
    }
}

fn report_precedes(left: &FileReport, right: &FileReport) -> bool {
    right
        .line_count
        .cmp(&left.line_count)
        .then_with(|| left.path.cmp(&right.path))
        == CompareOrdering::Less
}

#[derive(Default)]
struct ErrorStore {
    first: Option<FileAnalysisError>,
    resource: Option<FileAnalysisError>,
    candidates: Vec<FileAnalysisError>,
}

impl ErrorStore {
    fn record(&mut self, error: FileAnalysisError) {
        if self.first.is_none() {
            self.first = Some(error.clone());
        }
        if matches!(&error, FileAnalysisError::ResourceLimit { .. }) {
            replace_error(&mut self.resource, error.clone());
        }
        if self.candidates.len() < MAX_ERROR_DIAGNOSTICS {
            self.candidates.push(error);
            return;
        }
        let Some((worst_index, worst)) = self
            .candidates
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| error_order(left, right))
        else {
            return;
        };
        if error_order(&error, worst) == CompareOrdering::Less {
            self.candidates[worst_index] = error;
        }
    }

    fn selected(self) -> Option<FileAnalysisError> {
        let mut selected = self.first;
        if let Some(error) = self.resource {
            replace_error(&mut selected, error);
        }
        for error in self.candidates {
            replace_error(&mut selected, error);
        }
        selected
    }
}

fn replace_error(slot: &mut Option<FileAnalysisError>, candidate: FileAnalysisError) {
    let should_replace = match slot.as_ref() {
        None => true,
        Some(current) => error_order(&candidate, current) == CompareOrdering::Less,
    };
    if should_replace {
        *slot = Some(candidate);
    }
}

fn error_order(left: &FileAnalysisError, right: &FileAnalysisError) -> CompareOrdering {
    left.path()
        .cmp(right.path())
        .then_with(|| left.to_string().cmp(&right.to_string()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use tempfile::tempdir;

    use super::{
        CliOverrides, FileAnalysisError, FileReport, LintEngine, LintSettings,
        MAX_ERROR_DIAGNOSTICS, MAX_REPORTS, ReportSink, ReportSinkMode, ResourceLimit,
        SettingsBuilder, SinkItem, lint, sort_reports,
    };

    fn settings_with_jobs(jobs: usize) -> LintSettings {
        let mut builder = SettingsBuilder::new();
        builder.apply_cli(&CliOverrides {
            jobs: Some(jobs),
            ..CliOverrides::default()
        });
        builder.build().expect("valid parallel settings")
    }

    fn settings_with_limits(jobs: usize) -> LintSettings {
        let mut builder = SettingsBuilder::new();
        builder.apply_cli(&CliOverrides {
            jobs: Some(jobs),
            max_file_bytes: Some(12),
            max_line_bytes: Some(12),
            rust_ast_max_bytes: Some(12),
            ..CliOverrides::default()
        });
        builder.build().expect("valid resource settings")
    }

    #[test]
    fn public_lint_interface_analyzes_different_suffixes() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(directory.path().join("main.rs"), "fn main() {}\n").expect("write Rust file");
        fs::write(directory.path().join("main.py"), "# comment\nvalue = 1\n")
            .expect("write Python file");

        let mut reports = lint(directory.path(), &LintSettings::default()).expect("lint files");
        sort_reports(&mut reports);

        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].line_count, 1);
        assert_eq!(reports[1].line_count, 1);
        assert!(
            reports
                .iter()
                .any(|report| report.path.ends_with("main.rs"))
        );
        assert!(
            reports
                .iter()
                .any(|report| report.path.ends_with("main.py"))
        );
    }

    #[test]
    fn unknown_files_are_skipped_and_reported_as_empty_input() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(directory.path().join("README"), "plain text\n").expect("write file");

        let error = lint(directory.path(), &LintSettings::default()).expect_err("unknown file");

        assert_eq!(
            error.to_string(),
            "input contains no text files: ".to_string() + &directory.path().display().to_string()
        );
    }

    #[test]
    fn empty_directory_reports_no_files() {
        let directory = tempdir().expect("create temporary directory");

        let error = lint(directory.path(), &LintSettings::default()).expect_err("empty input");

        assert_eq!(
            error.to_string(),
            "input contains no files: ".to_string() + &directory.path().display().to_string()
        );
    }

    #[test]
    fn jobs_one_and_many_have_identical_reports() {
        let directory = tempdir().expect("create temporary directory");
        for index in 0..8 {
            let path = directory.path().join(format!("source-{index}.rs"));
            let source = "fn line() {}\n".repeat(index + 1);
            fs::write(path, source).expect("write source file");
        }

        let one = lint(directory.path(), &settings_with_jobs(1)).expect("lint with one job");
        let many = lint(directory.path(), &settings_with_jobs(4)).expect("lint with many jobs");

        assert_eq!(one, many);
    }

    #[test]
    fn resource_error_cancels_parallel_work_and_keeps_error_semantics() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(directory.path().join("a-too-large.rs"), "1234567890123")
            .expect("write oversized source");
        fs::write(directory.path().join("b-normal.rs"), "fn main() {}\n")
            .expect("write normal source");

        let one = lint(directory.path(), &settings_with_limits(1))
            .expect_err("one job should report the resource error");
        let many = lint(directory.path(), &settings_with_limits(4))
            .expect_err("many jobs should report the resource error");

        assert_eq!(one.to_string(), many.to_string());
        assert!(one.to_string().contains("a-too-large.rs"));
        assert!(one.to_string().contains("exceeds configured limit"));
    }

    #[test]
    fn unknown_file_is_skipped_before_parallel_file_analysis() {
        let directory = tempdir().expect("create temporary directory");
        fs::write(directory.path().join("large.bin"), vec![b'x'; 128]).expect("write unknown file");
        fs::write(directory.path().join("source.rs"), "fn x() {}\n").expect("write source file");

        let reports = lint(directory.path(), &settings_with_limits(4)).expect("lint files");

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].path, directory.path().join("source.rs"));
    }

    fn report(path: &str, line_count: usize) -> FileReport {
        FileReport {
            path: PathBuf::from(path),
            line_count,
        }
    }

    #[test]
    fn top_n_sink_keeps_only_reports_that_can_be_output() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = ReportSink::new(ReportSinkMode::TopN(2), Arc::clone(&cancelled));
        sink.submit(SinkItem::Report(report("low.rs", 1)))
            .expect("submit low report");
        sink.submit(SinkItem::Report(report("z-high.rs", 30)))
            .expect("submit high report");
        sink.submit(SinkItem::Report(report("a-high.rs", 30)))
            .expect("submit tied high report");
        sink.submit(SinkItem::Report(report("middle.rs", 20)))
            .expect("submit middle report");

        let output = sink.into_output().expect("finish top-n sink");
        let mut reports = output.reports;
        sort_reports(&mut reports);

        assert_eq!(
            reports,
            vec![report("a-high.rs", 30), report("z-high.rs", 30)]
        );
        assert!(!cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn top_n_sink_with_zero_capacity_still_records_report_input() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = ReportSink::new(ReportSinkMode::TopN(0), Arc::clone(&cancelled));
        sink.submit(SinkItem::Report(report("source.rs", 1)))
            .expect("submit report to zero-capacity sink");

        let output = sink.into_output().expect("finish zero-capacity sink");

        assert!(output.report_seen);
        assert!(output.reports.is_empty());
        assert!(!output.report_limit_exceeded);
        assert!(!cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn complete_sink_fails_after_its_fixed_report_limit() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = ReportSink::new(ReportSinkMode::Complete(2), Arc::clone(&cancelled));
        for (path, line_count) in [("a.rs", 1), ("b.rs", 2), ("c.rs", 3)] {
            sink.submit(SinkItem::Report(report(path, line_count)))
                .expect("submit complete report");
        }

        let output = sink.into_output().expect("finish complete sink");

        assert_eq!(output.reports.len(), 2);
        assert!(output.report_limit_exceeded);
        assert!(cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn error_selection_is_stable_and_keeps_resource_errors() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = ReportSink::new(ReportSinkMode::TopN(0), Arc::clone(&cancelled));
        sink.submit(SinkItem::Error(FileAnalysisError::Language {
            path: PathBuf::from("z.rs"),
            message: "late error".to_string(),
        }))
        .expect("submit first error");
        sink.submit(SinkItem::Error(FileAnalysisError::ResourceLimit {
            path: PathBuf::from("a.rs"),
            limit: ResourceLimit::FileTooLarge {
                actual_bytes: 2,
                max_bytes: 1,
            },
        }))
        .expect("submit resource error");
        sink.submit(SinkItem::Error(FileAnalysisError::Language {
            path: PathBuf::from("m.rs"),
            message: "middle error".to_string(),
        }))
        .expect("submit middle error");

        let output = sink.into_output().expect("finish error sink");

        assert_eq!(
            output.error.expect("an error").path(),
            PathBuf::from("a.rs")
        );
        assert!(cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn error_candidates_have_a_fixed_capacity() {
        let mut errors = super::ErrorStore::default();
        for index in 0..(MAX_ERROR_DIAGNOSTICS + 10) {
            errors.record(FileAnalysisError::Language {
                path: PathBuf::from(format!("{index:03}.rs")),
                message: "analysis error".to_string(),
            });
        }

        assert_eq!(errors.candidates.len(), MAX_ERROR_DIAGNOSTICS);
    }

    #[test]
    fn public_report_limit_rejects_values_above_the_memory_bound() {
        let engine = LintEngine::new(LintSettings::default());

        let error = engine
            .run_with_report_limit(Path::new("unused"), MAX_REPORTS + 1)
            .expect_err("report limit must be bounded");

        assert_eq!(
            error.to_string(),
            format!(
                "report limit {} exceeds the maximum of {MAX_REPORTS}",
                MAX_REPORTS + 1
            )
        );
    }
}
