//! 惰性求值的派生节点：[`create`]（带相等性门控）与 [`derived`]（不带）。
//!
//! 两者的读取都走 [`signal`](crate::signal) 模块 —— [`MemoId`] 与 [`DerivedId`]
//! 都实现了 [`Readable`](crate::Readable)。

use crate::{
    DerivedId, MemoId,
    core::{FuncPtr, value::AnyValue},
    runtime::{MemoVTable, RUNTIME, Runtime},
};
use silex_vtable::InlineStorage;
use std::{marker::PhantomData, mem::size_of, ptr::drop_in_place};

/// memo 内联载荷的布局：`[&'static MemoVTable][F 或 Box<F>]`。
///
/// vtable 以真正的指针形式写入缓冲区，**不做** `as usize` 往返 ——
/// 整数往返会擦除 provenance，之后再解引用即为未定义行为（AUDIT P3）。
const MEMO_PAYLOAD_OFFSET: usize = size_of::<usize>();

/// 把 vtable 指针与闭包打包进内联缓冲区。
/// 闭包放不下时改为存放 `Box<F>`，由对应的 vtable 分支负责解引用与析构。
fn build_memo_payload<F: 'static>(
    inline_vtable: &'static MemoVTable,
    boxed_vtable: &'static MemoVTable,
    f: F,
) -> InlineStorage {
    let mut data = InlineStorage::zeroed();
    // SAFETY: 偏移 0 处写入一个指针必然放得下；载荷写在 MEMO_PAYLOAD_OFFSET 处，
    // 由 `InlineStorage::fits` 判定是否内联，放不下则退化为一个 `Box<F>` 指针。
    unsafe {
        if InlineStorage::fits::<F>(MEMO_PAYLOAD_OFFSET) {
            data.write(0, inline_vtable as *const MemoVTable);
            data.write(MEMO_PAYLOAD_OFFSET, f);
        } else {
            data.write(0, boxed_vtable as *const MemoVTable);
            data.write(MEMO_PAYLOAD_OFFSET, Box::new(f));
        }
    }
    data
}

/// 创建一个惰性求值、带相等性门控的派生节点。
///
/// - **惰性**：依赖变化只把它标脏，真正的重算发生在下一次读取（或下游 effect
///   被调度）时。
/// - **门控**：重算后用 `PartialEq` 与旧值比较，只有真的变了才通知下游。
///   一条 memo 链因此能把上游的抖动挡在中途。
/// - 计算闭包拿到的 `Option<&T>` 是**上一次的结果**，首次计算时为 `None`。
///   它是借来的，不是克隆来的 —— 需要拥有一份请自己 `clone`（AUDIT P9）。
///
/// # 契约
///
/// 不允许在 `f` 内部读取这个 memo 自己：旧值在 `f` 执行期间被移出了节点，
/// 此时节点里放的是占位值。旧值请从参数拿。
#[track_caller]
pub fn create<T, F>(f: F) -> MemoId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    let id = RUNTIME.get_or(Runtime::new).register_node();
    internal_init_memo::<T, F>(id, f);
    MemoId::from_raw(id)
}

#[inline(never)]
fn internal_init_memo<T, F>(id: crate::RawNodeId, f: F)
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    let data = build_memo_payload(
        &MemoInlineVTable::<T, F>::VTABLE,
        &MemoBoxedVTable::<T, F>::VTABLE,
        f,
    );

    // SAFETY: 载荷由 `build_memo_payload` 按 `MemoVTable` 约定的布局构造。
    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo(id, data) };
}

/// 创建一个惰性求值但**不做相等性门控**的派生节点。
///
/// 与 [`create`] 的唯一区别就在门控：`T` 只有 `'static` 约束，运行时没有
/// `PartialEq` 可用，因此每一次重算都会通知下游，哪怕算出来的值和上次一样
/// （AUDIT P10）。它换来的是对 `T` 不作任何要求 —— 这正是上层框架的
/// `Signal::derive` 需要的：任意闭包都能包成一个可读节点。
///
/// 值本身仍然是缓存的：没有依赖变化时读它不会重新执行闭包。
#[track_caller]
pub fn derived<T: 'static>(f: Box<dyn Fn() -> T>) -> DerivedId {
    let id = RUNTIME.get_or(Runtime::new).register_node();
    internal_init_derived::<T>(id, f);
    DerivedId::from_raw(id)
}

#[inline(never)]
fn internal_init_derived<T: 'static>(id: crate::RawNodeId, f: Box<dyn Fn() -> T>) {
    // `Box<dyn Fn() -> T>` 是两个机器字的胖指针，恰好放得下；
    // `DerivedVTable` 只有内联这一种布局，所以这里必须真的内联。
    const {
        assert!(
            InlineStorage::fits::<Box<dyn Fn()>>(MEMO_PAYLOAD_OFFSET),
            "derived payload must fit inline"
        );
    }
    let mut data = InlineStorage::zeroed();
    // SAFETY: 上面的 const 断言保证了胖指针放得下；布局与 `DerivedVTable` 一致。
    unsafe {
        data.write(0, &DerivedVTable::<T>::VTABLE as *const MemoVTable);
        data.write(MEMO_PAYLOAD_OFFSET, f);
    }

    // SAFETY: 载荷按 `DerivedVTable` 约定的布局构造。
    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo(id, data) };
}

struct MemoInlineVTable<T, F>(PhantomData<(T, F)>);
impl<T: Clone + PartialEq + 'static, F: Fn(Option<&T>) -> T + 'static> MemoInlineVTable<T, F> {
    const VTABLE: MemoVTable = MemoVTable {
        // 旧值按引用透传给用户闭包，绝不在这里克隆（AUDIT P9）。
        compute: FuncPtr::new(|ptr, old| {
            let f = unsafe { &*(ptr as *const F) };
            let new_t = f(old.and_then(|any| any.downcast_ref::<T>()));
            AnyValue::new_reactive(new_t)
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut F) }),
    };
}

struct MemoBoxedVTable<T, F>(PhantomData<(T, F)>);
impl<T: Clone + PartialEq + 'static, F: Fn(Option<&T>) -> T + 'static> MemoBoxedVTable<T, F> {
    const VTABLE: MemoVTable = MemoVTable {
        compute: FuncPtr::new(|ptr, old| {
            let f = unsafe { &**(ptr as *const Box<F>) };
            let new_t = f(old.and_then(|any| any.downcast_ref::<T>()));
            AnyValue::new_reactive(new_t)
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut Box<F>) }),
    };
}

struct DerivedVTable<T>(PhantomData<T>);
impl<T: 'static> DerivedVTable<T> {
    const VTABLE: MemoVTable = MemoVTable {
        compute: FuncPtr::new(|ptr, _| {
            let f = unsafe { &**(ptr as *const Box<dyn Fn() -> T>) };
            AnyValue::new(f())
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut Box<dyn Fn() -> T>) }),
    };
}
