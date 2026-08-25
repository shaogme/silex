use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    io::Error as IoError,
    path::{Path, PathBuf},
    vec::IntoIter,
};

use ignore::{DirEntry, Error as IgnoreError, Walk, WalkBuilder};

use crate::config::{DiscoveryOptions, LintSettings};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourcePath(PathBuf);

impl SourcePath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileSet(Vec<SourcePath>);

impl FileSet {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourcePath> {
        self.0.iter()
    }
}

impl IntoIterator for FileSet {
    type Item = SourcePath;
    type IntoIter = IntoIter<SourcePath>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct FileWalker {
    walk: Walk,
}

impl Iterator for FileWalker {
    type Item = Result<SourcePath, FileError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = self.walk.next()?;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => return Some(Err(walk_error(error))),
            };
            if entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                return Some(Ok(SourcePath::new(entry.into_path())));
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FileCollector {
    options: DiscoveryOptions,
}

impl FileCollector {
    pub fn new(options: DiscoveryOptions) -> Self {
        Self { options }
    }

    pub fn from_settings(settings: &LintSettings) -> Self {
        Self::new(settings.discovery_options())
    }

    pub fn walk(&self, input: &Path) -> Result<FileWalker, FileError> {
        let metadata = fs::symlink_metadata(input).map_err(|error| io_error(input, error))?;
        if metadata.file_type().is_symlink() {
            return Ok(FileWalker {
                walk: WalkBuilder::empty().build(),
            });
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(FileError(format!(
                "input is not a file or directory: {}",
                input.display()
            )));
        }

        let mut builder = WalkBuilder::new(input);
        builder
            .standard_filters(false)
            .hidden(self.options.hide_hidden)
            .parents(self.options.load_parents)
            .ignore(self.options.load_ignore)
            .git_ignore(self.options.load_gitignore)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .follow_links(false)
            .sort_by_file_path(|left, right| left.cmp(right))
            .filter_entry(skip_git_directory);

        Ok(FileWalker {
            walk: builder.build(),
        })
    }

    pub fn collect(&self, input: &Path) -> Result<FileSet, FileError> {
        let mut files = self.walk(input)?.collect::<Result<Vec<_>, _>>()?;
        files.sort();
        Ok(FileSet(files))
    }
}

#[derive(Debug)]
pub struct FileError(String);

impl Display for FileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for FileError {}

fn skip_git_directory(entry: &DirEntry) -> bool {
    !entry
        .path()
        .components()
        .any(|component| component.as_os_str() == ".git")
}

fn io_error(path: &Path, error: IoError) -> FileError {
    FileError(format!("{}: {error}", path.display()))
}

fn walk_error(error: IgnoreError) -> FileError {
    FileError(format!("cannot walk input: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use tempfile::tempdir;

    use super::{FileCollector, SourcePath};
    use crate::config::DiscoveryOptions;

    fn write_file(path: &Path, content: &str) {
        fs::write(path, content).expect("write fixture");
    }

    #[test]
    fn loads_ignore_and_gitignore_files_by_default() {
        let directory = tempdir().expect("create temporary directory");
        write_file(&directory.path().join(".ignore"), "ignored-by-ignore.rs\n");
        write_file(
            &directory.path().join(".gitignore"),
            "ignored-by-gitignore.rs\n",
        );
        write_file(&directory.path().join(".hidden.rs"), "fn hidden() {}\n");
        write_file(&directory.path().join("kept.rs"), "fn kept() {}\n");
        write_file(
            &directory.path().join("ignored-by-ignore.rs"),
            "fn ignored() {}\n",
        );
        write_file(
            &directory.path().join("ignored-by-gitignore.rs"),
            "fn ignored() {}\n",
        );

        let files = FileCollector::new(DiscoveryOptions::default())
            .collect(directory.path())
            .expect("collect files");
        let names = files
            .iter()
            .filter_map(|path| path.as_path().file_name())
            .collect::<Vec<_>>();

        assert!(names.iter().any(|name| *name == "kept.rs"));
        assert!(!names.iter().any(|name| *name == "ignored-by-ignore.rs"));
        assert!(!names.iter().any(|name| *name == "ignored-by-gitignore.rs"));
        assert!(!names.iter().any(|name| *name == ".hidden.rs"));
    }

    #[test]
    fn can_disable_each_ignore_source_and_hidden_filter() {
        let directory = tempdir().expect("create temporary directory");
        write_file(&directory.path().join(".ignore"), "ignored.rs\n");
        write_file(&directory.path().join(".gitignore"), "gitignored.rs\n");
        write_file(&directory.path().join(".hidden.rs"), "fn hidden() {}\n");
        write_file(&directory.path().join("ignored.rs"), "fn ignored() {}\n");
        write_file(&directory.path().join("gitignored.rs"), "fn ignored() {}\n");

        let options = DiscoveryOptions {
            load_ignore: false,
            load_gitignore: false,
            hide_hidden: false,
            ..DiscoveryOptions::default()
        };
        let files = FileCollector::new(options)
            .collect(directory.path())
            .expect("collect files");
        let names = files
            .iter()
            .filter_map(|path| path.as_path().file_name())
            .collect::<Vec<_>>();

        assert!(names.iter().any(|name| *name == ".hidden.rs"));
        assert!(names.iter().any(|name| *name == "ignored.rs"));
        assert!(names.iter().any(|name| *name == "gitignored.rs"));
    }

    #[test]
    fn parent_ignore_rules_can_be_disabled() {
        let directory = tempdir().expect("create temporary directory");
        let child = directory.path().join("child");
        fs::create_dir(&child).expect("create child directory");
        write_file(&directory.path().join(".ignore"), "from-parent.rs\n");
        write_file(&child.join("from-parent.rs"), "fn kept() {}\n");

        let options = DiscoveryOptions {
            load_parents: false,
            ..DiscoveryOptions::default()
        };
        let files = FileCollector::new(options)
            .collect(&child)
            .expect("collect files");

        assert_eq!(
            files.iter().map(SourcePath::as_path).collect::<Vec<_>>(),
            vec![child.join("from-parent.rs").as_path()]
        );
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_file_symlinks() {
        let directory = tempdir().expect("create temporary directory");
        let target = directory.path().join("target.rs");
        let link = directory.path().join("link.rs");
        write_file(&target, "fn target() {}\n");
        symlink(&target, &link).expect("create file symlink");

        let files = FileCollector::new(DiscoveryOptions::default())
            .collect(directory.path())
            .expect("collect files");

        assert!(files.iter().any(|path| path.as_path() == target));
        assert!(!files.iter().any(|path| path.as_path() == link));
    }
}
