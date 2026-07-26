//! 底层数据结构：句柄化的 arena、紧凑的小列表、类型擦除的值。
//!
//! 这个模块从前叫 `core`，于是 crate 内部写 `core::mem::...` 会解析到它自己
//! 而不是标准库（审计报告 §3.4）。里面的图算法（`algorithm.rs`）已经随阶段三
//! 搬去了 [`crate::runtime::graph`] —— 那一层抽象只有一个实现者，代价却是
//! 把订阅表与依赖表强行物化进 `Vec`（审计报告 §3.3）。

pub(crate) mod arena;
pub(crate) mod list;
pub(crate) mod value;

pub(crate) use silex_vtable::func_ptr::FuncPtr;
