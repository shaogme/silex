use std::{
    cmp::Ordering,
    collections::BTreeSet,
    env,
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    io::Error as IoError,
    path::{Path, PathBuf},
    process::ExitCode,
};

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::{
    Attribute, File, ForeignItem, ImplItem, Item, Meta, Token, TraitItem,
    punctuated::Punctuated,
    spanned::Spanned,
    visit::{self, Visit},
};

const MAX_PRODUCTION_LINES: usize = 650;

#[derive(Debug)]
struct ToolError(String);

impl Display for ToolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for ToolError {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Position {
    line: usize,
    column: usize,
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.line, self.column).cmp(&(other.line, other.column))
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy)]
struct SpanRange {
    start: Position,
    end: Position,
}

#[derive(Debug, Eq, PartialEq)]
struct FileReport {
    production_lines: BTreeSet<usize>,
    skipped_test_items: usize,
    pure_test_file: bool,
}

impl FileReport {
    fn production_line_count(&self) -> usize {
        self.production_lines.len()
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dom-module-size: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), ToolError> {
    let mut arguments = env::args_os();
    let program = arguments.next().unwrap_or_default();
    let root = arguments.next().ok_or_else(|| {
        ToolError(format!(
            "usage: {} <crates/silex_dom/src>",
            Path::new(&program).display()
        ))
    })?;
    if arguments.next().is_some() {
        return Err(ToolError(
            "expected exactly one source directory".to_string(),
        ));
    }

    let root = PathBuf::from(root);
    let files = collect_rust_files(&root)?;
    if files.is_empty() {
        return Err(ToolError(format!(
            "source directory contains no Rust files: {}",
            root.display()
        )));
    }

    let mut oversized = Vec::new();
    for path in files {
        let report = analyze_file(&path)?;
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let kind = if report.pure_test_file {
            "pure test file"
        } else {
            "production file"
        };
        println!(
            "{}: {} production lines, skipped {} test items ({kind})",
            relative.display(),
            report.production_line_count(),
            report.skipped_test_items
        );
        if report.production_line_count() > MAX_PRODUCTION_LINES {
            oversized.push((relative.to_path_buf(), report.production_line_count()));
        }
    }

    if oversized.is_empty() {
        return Ok(());
    }

    let details = oversized
        .into_iter()
        .map(|(path, lines)| format!("{} ({lines})", path.display()))
        .collect::<Vec<_>>()
        .join(", ");
    Err(ToolError(format!(
        "production line limit of {MAX_PRODUCTION_LINES} exceeded: {details}"
    )))
}

fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>, ToolError> {
    let metadata = fs::metadata(root).map_err(|error| io_error(root, error))?;
    if metadata.is_file() {
        if root.extension().is_some_and(|extension| extension == "rs") {
            return Ok(vec![root.to_path_buf()]);
        }
        return Err(ToolError(format!(
            "input is not a Rust file: {}",
            root.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(ToolError(format!(
            "input is not a file or directory: {}",
            root.display()
        )));
    }

    let mut files = Vec::new();
    collect_rust_files_recursively(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files_recursively(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), ToolError> {
    for entry in fs::read_dir(root).map_err(|error| io_error(root, error))? {
        let entry = entry.map_err(|error| io_error(root, error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| io_error(&path, error))?;
        if file_type.is_dir() {
            collect_rust_files_recursively(&path, files)?;
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn io_error(path: &Path, error: IoError) -> ToolError {
    ToolError(format!("{}: {error}", path.display()))
}

fn analyze_file(path: &Path) -> Result<FileReport, ToolError> {
    let source = fs::read_to_string(path).map_err(|error| io_error(path, error))?;
    analyze_source(&source, path)
}

fn analyze_source(source: &str, path: &Path) -> Result<FileReport, ToolError> {
    let syntax = syn::parse_file(source)
        .map_err(|error| ToolError(format!("{}: {error}", path.display())))?;
    let mut collector = TestItemCollector::default();
    for item in &syntax.items {
        collector.visit_item(item);
    }

    let pure_test_file = is_pure_test_file(path, &syntax);
    let production_lines = if pure_test_file {
        BTreeSet::new()
    } else {
        let tokens = source
            .parse::<TokenStream>()
            .map_err(|error| ToolError(format!("{}: {error}", path.display())))?;
        collect_token_lines(&tokens, &collector.skipped_ranges)
    };

    Ok(FileReport {
        production_lines,
        skipped_test_items: collector.skipped_test_items,
        pure_test_file,
    })
}

fn is_pure_test_file(path: &Path, syntax: &File) -> bool {
    if !path.file_name().is_some_and(|name| name == "tests.rs") {
        return false;
    }
    syntax.items.iter().all(|item| {
        matches!(item, Item::Use(_) | Item::ExternCrate(_))
            || item_attributes(item).iter().any(attribute_marks_test_only)
    })
}

#[derive(Default)]
struct TestItemCollector {
    skipped_ranges: Vec<SpanRange>,
    skipped_test_items: usize,
}

impl TestItemCollector {
    fn skip_if_test_only(&mut self, attrs: &[Attribute], span: Span) -> bool {
        if !attrs.iter().any(attribute_marks_test_only) {
            return false;
        }
        if let Some(range) = item_span_range(attrs, span) {
            self.skipped_ranges.push(range);
        }
        self.skipped_test_items += 1;
        true
    }
}

impl<'ast> Visit<'ast> for TestItemCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.skip_if_test_only(item_attributes(item), item.span()) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if self.skip_if_test_only(impl_item_attributes(item), item.span()) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'ast ForeignItem) {
        if self.skip_if_test_only(foreign_item_attributes(item), item.span()) {
            return;
        }
        visit::visit_foreign_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if self.skip_if_test_only(trait_item_attributes(item), item.span()) {
            return;
        }
        visit::visit_trait_item(self, item);
    }
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_item_attributes(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_item_attributes(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn foreign_item_attributes(item: &ForeignItem) -> &[Attribute] {
    match item {
        ForeignItem::Fn(item) => &item.attrs,
        ForeignItem::Static(item) => &item.attrs,
        ForeignItem::Type(item) => &item.attrs,
        ForeignItem::Macro(item) => &item.attrs,
        ForeignItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn attribute_marks_test_only(attribute: &Attribute) -> bool {
    if attribute.path().is_ident("test") {
        return true;
    }
    attribute.path().is_ident("cfg") && cfg_meta_is_test_only(&attribute.meta)
}

fn cfg_meta_is_test_only(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("cfg") => {
            let nested = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .unwrap_or_default();
            nested.len() == 1 && nested.iter().any(cfg_meta_is_test_only)
        }
        Meta::List(list) if list.path.is_ident("all") => {
            let nested = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .unwrap_or_default();
            nested.iter().any(cfg_meta_is_test_only)
        }
        Meta::List(_) | Meta::NameValue(_) => false,
    }
}

fn span_range(span: Span) -> Option<SpanRange> {
    let start = span.start();
    let end = span.end();
    let start = Position {
        line: start.line,
        column: start.column,
    };
    let end = Position {
        line: end.line,
        column: end.column,
    };
    (start < end).then_some(SpanRange { start, end })
}

fn item_span_range(attrs: &[Attribute], span: Span) -> Option<SpanRange> {
    let mut range = span_range(span)?;
    for attribute in attrs {
        if let Some(attribute_range) = span_range(attribute.span()) {
            range.start = range.start.min(attribute_range.start);
            range.end = range.end.max(attribute_range.end);
        }
    }
    Some(range)
}

fn collect_token_lines(tokens: &TokenStream, skipped_ranges: &[SpanRange]) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    for token in tokens.clone() {
        match token {
            TokenTree::Group(group) => {
                add_span_lines(group.span_open(), skipped_ranges, &mut lines);
                lines.extend(collect_token_lines(&group.stream(), skipped_ranges));
                add_span_lines(group.span_close(), skipped_ranges, &mut lines);
            }
            TokenTree::Ident(ident) => add_span_lines(ident.span(), skipped_ranges, &mut lines),
            TokenTree::Punct(punct) => add_span_lines(punct.span(), skipped_ranges, &mut lines),
            TokenTree::Literal(literal) => {
                add_span_lines(literal.span(), skipped_ranges, &mut lines)
            }
        }
    }
    lines
}

fn add_span_lines(span: Span, skipped_ranges: &[SpanRange], lines: &mut BTreeSet<usize>) {
    let Some(range) = span_range(span) else {
        return;
    };
    if skipped_ranges
        .iter()
        .any(|skipped| ranges_overlap(range, *skipped))
    {
        return;
    }
    for line in range.start.line..=range.end.line {
        lines.insert(line);
    }
}

fn ranges_overlap(left: SpanRange, right: SpanRange) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::{Path, analyze_source};

    #[test]
    fn fixture_counts_tokens_and_skips_test_items() {
        let source = include_str!("../fixtures/line_count.rs");
        let report = analyze_source(source, Path::new("line_count.rs")).expect("fixture parses");

        assert_eq!(report.production_line_count(), 9);
        assert_eq!(report.skipped_test_items, 3);
        assert!(report.production_lines.contains(&3));
        assert!(!report.production_lines.contains(&1));
        assert!(!report.production_lines.contains(&15));
        assert!(!report.pure_test_file);
    }

    #[test]
    fn fixture_skips_pure_test_files() {
        let source = include_str!("../fixtures/tests.rs");
        let report = analyze_source(source, Path::new("tests.rs")).expect("fixture parses");

        assert_eq!(report.production_line_count(), 0);
        assert_eq!(report.skipped_test_items, 1);
        assert!(report.pure_test_file);
    }
}
