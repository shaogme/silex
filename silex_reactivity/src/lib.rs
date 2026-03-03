mod core;
pub use crate::core::algorithm::NodeState;
pub use crate::core::arena::{Arena, Index as NodeId, SparseSecondaryMap};
pub(crate) use crate::core::list::List;

pub(crate) type NodeList = List<NodeId>;
pub(crate) type DependencyList = List<(NodeId, u32)>;

mod runtime;
use runtime::RUNTIME;

mod primitive;
pub use primitive::*;

/// 具有 16 字节对齐要求的 64 字节固定宽度缓冲区。
/// 用于跨 crate 安全地传递和存储类型擦除后的 Payload。
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct RawOpBuffer {
    pub data: [u8; 64],
}

impl Default for RawOpBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RawOpBuffer {
    pub fn new() -> Self {
        Self { data: [0u8; 64] }
    }
}

pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    RUNTIME.with(|rt| rt.batch(f))
}

pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.with(|rt| rt.create_scope(f))
}

pub fn dispose(id: NodeId) {
    RUNTIME.with(|rt| rt.dispose(id));
}

pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.with(|rt| rt.on_cleanup(f));
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|rt| rt.untrack(f))
}

/// 获取任何响应式节点的原始指针（不区分 Signal 或 StoredValue）。
/// 用于 Silex Core 的高级去泛型化优化。
///
/// # Safety
///
/// 调用者必须确保返回的指针在当前上下文中有效。
/// 如果节点被销毁，该指针将失效。
pub unsafe fn try_get_any_raw_untracked(id: NodeId) -> Option<*const ()> {
    RUNTIME.with(|rt| unsafe { rt.get_any_raw_ptr_untracked(id) })
}

pub fn get_node_defined_at(_id: NodeId) -> Option<&'static std::panic::Location<'static>> {
    #[cfg(debug_assertions)]
    {
        RUNTIME.with(|rt| {
            if let Some(node) = rt.storage.graph.get(_id) {
                return node.defined_at;
            }
            None
        })
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
        RUNTIME.with(|rt| {
            if let Some(aux) = rt.storage.try_aux_mut(_id) {
                aux.debug_label = Some(label);
            }
        })
    }
}

pub fn get_debug_label(_id: NodeId) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        RUNTIME.with(|rt| {
            if let Some(aux) = rt.storage.node_aux.get(_id)
                && let Some(label) = &aux.debug_label
            {
                return Some(label.clone());
            }
            // Check dead labels
            rt.storage.dead_node_labels.get(_id).cloned()
        })
    }
    #[cfg(not(debug_assertions))]
    {
        return None;
    }
}
