//! “稍后填充”的宿主元素引用（DOM 节点等）。
//!
//! 就是一个存着 `Option<T>` 的 [`store`] 节点，只是句柄的种类不同
//! —— 这样 [`set`] 与 [`store::try_update`] 不会互相
//! 串门。

use crate::{NodeRefId, ReactiveResult, store};

/// 注册一个尚未填充的元素引用。
#[track_caller]
pub fn create<T: 'static>() -> NodeRefId {
    NodeRefId::from_raw(store::create_raw::<Option<T>>(None))
}

/// 取出里面存放的元素（尚未 [`set`] 时为 `Ok(None)`）。
pub fn try_get<T: Clone + 'static>(id: NodeRefId) -> ReactiveResult<Option<T>> {
    store::with_raw::<Option<T>, _>(id.raw(), Clone::clone)
}

/// 便捷形式：节点没了、类型不对、尚未填充，一律 `None`。
#[inline(always)]
pub fn get<T: Clone + 'static>(id: NodeRefId) -> Option<T> {
    try_get(id).ok().flatten()
}

/// 填充一个元素引用。
pub fn set<T: 'static>(id: NodeRefId, element: T) -> ReactiveResult<()> {
    store::with_raw_mut::<Option<T>, _>(id.raw(), |slot| *slot = Some(element))
}
