mod core;
mod primitive;
mod runtime;

pub(crate) use crate::core::list::List;

/// 响应式节点的句柄。这是本 crate 唯一对外暴露的容器相关类型。
///
/// `Arena` / `SparseSecondaryMap` / `NodeState` 曾经也是 `pub` 的，但它们的
/// `get_mut(&self) -> Option<&mut T>` 允许安全代码两行就造出两个同时存活的
/// `&mut`（AUDIT P7）。用注释约束的契约必须由类型系统或 `unsafe` 表达，
/// 在此之前它们只能留在 crate 内部，由运行时自己保证独占访问。
pub use crate::core::arena::Index as NodeId;

use runtime::RUNTIME;
pub(crate) use runtime::Runtime;
use std::panic::Location;

pub use primitive::*;

pub(crate) type NodeList = List<NodeId>;
pub(crate) type DependencyList = List<(NodeId, u32)>;

/// 具有 16 字节对齐要求的 64 字节固定宽度缓冲区。
/// 用于跨 crate 安全地传递和存储类型擦除后的 Payload。
///
/// 缓冲区用 `MaybeUninit<u8>` 而不是 `u8`：Payload 里通常含有函数指针和数据指针，
/// 而整数类型的读写会擦除指针 provenance，按值搬运 `[u8; 64]` 之后再把这些字节
/// 当指针解引用即为未定义行为（AUDIT P3）。字节级复制则保留 provenance。
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct RawOpBuffer {
    data: [std::mem::MaybeUninit<u8>; 64],
}

impl Default for RawOpBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RawOpBuffer {
    pub const CAPACITY: usize = 64;
    pub const ALIGNMENT: usize = 16;

    /// 全零初始化的缓冲区。
    pub fn new() -> Self {
        Self {
            data: [std::mem::MaybeUninit::new(0); Self::CAPACITY],
        }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr().cast()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr().cast()
    }
}

pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    RUNTIME.get_or(Runtime::new).batch(f)
}

#[track_caller]
pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.get_or(Runtime::new).create_scope(f)
}

pub fn dispose(id: NodeId) {
    RUNTIME.get_or(Runtime::new).dispose(id);
}

pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.get_or(Runtime::new).on_cleanup(f);
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.get_or(Runtime::new).untrack(f)
}

/// 获取任何响应式节点的原始指针（不区分 Signal 或 StoredValue）。
/// 用于 Silex Core 的高级去泛型化优化。
///
/// # Safety
///
/// 调用者必须确保返回的指针在当前上下文中有效。
/// 如果节点被销毁，该指针将失效。
pub unsafe fn try_get_any_raw_untracked(id: NodeId) -> Option<*const ()> {
    let rt = RUNTIME.get()?;
    unsafe { rt.get_any_raw_ptr_untracked(id) }
}

pub fn get_node_defined_at(_id: NodeId) -> Option<&'static Location<'static>> {
    #[cfg(debug_assertions)]
    {
        let rt = RUNTIME.get()?;
        if let Some(node) = rt.storage.graph.get(_id) {
            return node.defined_at;
        }
        None
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

// --- Debugging API ---

pub fn set_debug_label(_id: NodeId, _label: impl Into<String>) {
    #[cfg(debug_assertions)]
    {
        let label = _label.into();
        let rt = RUNTIME.get_or(Runtime::new);
        if let Some(aux) = rt.storage.try_aux_mut(_id) {
            aux.debug_label = Some(label);
        }
    }
}

pub fn get_debug_label(_id: NodeId) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        let rt = RUNTIME.get()?;
        if let Some(aux) = rt.storage.node_aux.get(_id)
            && let Some(label) = &aux.debug_label
        {
            return Some(label.clone());
        }
        // Check dead labels
        rt.storage.dead_node_labels.get(_id).cloned()
    }
    #[cfg(not(debug_assertions))]
    {
        return None;
    }
}
