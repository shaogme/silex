//! `tw_verbose!` 的编译期诊断输出。
//!
//! 报告 §4.3：此前是一串 `eprintln!` 直接打到 stderr。`cargo build` 并行编译多个
//! crate 时这些行会与别的输出交织成一团，且 `--message-format=json` 消费不了——
//! 它既不是 rustc 诊断，也不是结构化数据。
//!
//! 现在正文落盘到 `<target>/silex-tw-debug/<hash>.txt`，stderr 只留一行指针。
//! 一行是原子的，不会被别的输出切碎；文件名按输入内容取哈希，重复构建覆盖同一个文件
//! 而不是越堆越多。落盘失败（只读文件系统、找不到 target 目录）时退回整段 stderr，
//! 诊断宏不该因为写不了文件就把编译搞失败。

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

/// 一段诊断的标题与正文
pub type Section<'a> = (&'a str, String);

/// 输出一次 `tw_verbose!` 诊断
pub fn emit(input_str: &str, sections: &[Section<'_>]) {
    let mut body = String::with_capacity(1024);
    let _ = writeln!(body, "# Silex tw_verbose! Compile-Time Diagnostics");
    let _ = writeln!(body, "\n## Macro Input\n{input_str}");
    for (title, content) in sections {
        let _ = writeln!(body, "\n## {title}\n{content}");
    }

    match write_to_file(input_str, &body) {
        Some(path) => eprintln!("[silex tw_verbose] {}", path.display()),
        None => eprintln!("{body}"),
    }
}

fn write_to_file(input_str: &str, body: &str) -> Option<PathBuf> {
    let dir = debug_dir()?;
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{:016x}.txt", stable_hash(input_str)));
    fs::write(&path, body).ok()?;
    Some(path)
}

/// 诊断文件的落点：`CARGO_TARGET_DIR`，否则从当前 crate 目录向上找最近的 `target/`
fn debug_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return Some(Path::new(&dir).join("silex-tw-debug"));
    }
    let mut dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").ok()?);
    loop {
        let candidate = dir.join("target");
        if candidate.is_dir() {
            return Some(candidate.join("silex-tw-debug"));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// 文件名用的稳定哈希——必须跨进程稳定，`DefaultHasher` 不保证这一点
fn stable_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = silex_hash::css::CssHasher::with_seed(0x9e3779b97f4a7c15);
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_across_calls_and_differs_per_input() {
        assert_eq!(stable_hash("p-4"), stable_hash("p-4"));
        assert_ne!(stable_hash("p-4"), stable_hash("p-8"));
    }

    /// 找不到 target 目录时不能 panic，只是退回 stderr
    #[test]
    fn emitting_never_panics() {
        emit("p-4", &[("Compiled Class Name", "slx-tw-x".to_string())]);
    }
}
