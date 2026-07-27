//! 非响应式的保管值：把一个值交给运行时，让它随所属 scope 一起销毁。
//!
//! 与 signal 的区别：它**不是**响应式的 —— 没有订阅者，读写都不会触发任何调度。
//!
//! # 它同时是 `callback` 与 `node_ref` 的底座
//!
//! 从前这三者（外加已经删掉的 `register_closure` / `register_op`）是
//! `ExtraData` 枚举的五个变体，五套几乎一模一样的“取出—downcast—用”的代码，
//! 外加五个 `is_*_valid` 探测函数（审计报告 §3.1 / §3.2 / §2.4）。变体的唯一
//! 作用是当运行时的种类 tag，而种类现在写在句柄的类型里
//! （[`Handle<K>`](crate::Handle)）。于是全部收敛成本模块的这一套泛型实现，
//! [`callback`](crate::callback) 与 [`node_ref`](crate::node_ref) 只是它上面的
//! 两层薄封装。

use crate::{
    RawId, ReactiveError, ReactiveResult, StoredId,
    internal::value::AnyValue,
    runtime::{drive, with_rt},
};

/// 把一个值交给运行时保管，返回它的句柄。
///
/// 它的生命周期与所属的 scope 绑定：父节点销毁时一并销毁。
#[track_caller]
pub fn create<T: 'static>(value: T) -> StoredId {
    StoredId::from_raw(create_raw(value))
}

/// 借用保管的值。
///
/// # 契约
///
/// 值在 `f` 执行期间被**移出**节点（节点里暂时是空的），运行时因此不必在
/// 用户代码执行期间持有任何指向该条目的引用 —— 从前这里直接把
/// `SparseSecondaryMap::get_mut` 交出来的引用递给 `f`，`f` 里碰一下任何别的
/// 节点就会在 Stacked Borrows 下作废它（审计报告 §2.1）。
///
/// 代价是：**不允许在 `f` 内部访问同一个节点**，那样会拿到
/// [`Reentrant`](ReactiveError::Reentrant)。
pub fn try_with<T: 'static, R>(id: StoredId, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
    with_raw(id.raw(), f)
}

/// 就地修改保管的值。不触发任何调度。契约同 [`try_with`]。
pub fn try_update<T: 'static, R>(id: StoredId, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
    with_raw_mut(id.raw(), f)
}

/// 直接借出内部的引用，跳过 `Clone`。
///
/// # Safety
///
/// 返回的 `&'static T` 是**伪造**的：数据实际存活在运行时的 arena 里，与
/// `'static` 毫无关系。调用方必须保证在该引用的整个使用期间，下列操作一次都
/// 不发生 —— 任何一条都会让引用悬垂，读它即为未定义行为（AUDIT P6）：
///
/// - [`dispose`](crate::scope::dispose) 该节点或它的任一祖先（释放 arena 槽位）；
/// - [`try_update`] / [`try_with`]：它们会把值**移出**节点，期间节点里是空的；
/// - 任何会重入运行时并执行用户代码的调用（effect 体、cleanup、`batch` 收尾），
///   因为用户代码可以做上面两件事中的任意一件。
///
/// 换句话说：把返回值当作一个只在“紧接着的、不重入运行时的表达式”里有效的
/// 借用来用。需要更长的存活期就改用 [`try_with`] 或克隆一份出来。
pub unsafe fn try_value_ref<T: 'static>(id: StoredId) -> Option<&'static T> {
    // SAFETY: 契约（引用不得跨越上面列出的任何一种操作）由本函数的调用方承担，
    // 原样转嫁给 `payload_value_unchecked`。指针要走出 `with_rt` 的借用再解引用，
    // 而“伪造出来的 `'static` 有多久有效”正是上面那份契约的内容。
    let ptr = with_rt(|rt| unsafe { rt.payload_value_unchecked(id.raw()) }.map(std::ptr::from_ref))
        .ok()
        .flatten()?;
    unsafe { &*ptr }.downcast_ref::<T>()
}

// --- 供 `callback` / `node_ref` 复用的泛型底座 ---

#[track_caller]
pub(crate) fn create_raw<T: 'static>(value: T) -> RawId {
    drive::store_payload(AnyValue::new(value)).expect("刚建出来的运行时可用")
}

pub(crate) fn with_raw<T: 'static, R>(raw: RawId, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
    drive::with_payload(raw, |value| downcast(value, f))?
}

pub(crate) fn with_raw_mut<T: 'static, R>(
    raw: RawId,
    f: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    drive::with_payload_mut(raw, |value| {
        value
            .downcast_mut::<T>()
            .map(f)
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

fn downcast<T: 'static, R>(value: &AnyValue, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
    value
        .downcast_ref::<T>()
        .map(f)
        .ok_or(ReactiveError::TypeMismatch)
}
