use crate::{
    RxNodeKind,
    reactivity::{OpPayloadHeader, RawId},
    traits::{RxData, RxGuard},
};
use silex_reactivity::{SignalId, StoredId, signal, store, try_get_any_raw_untracked};
use std::{mem::MaybeUninit, panic::Location};

/// Op 载荷在运行时里的实际存储类型。
///
/// 从前是一个 64 字节的 `RawOpBuffer`（`Copy` + 永不析构，审计报告 §2.4）；
/// 现在就是一个普通的 stored value，只是这一层仍然用裸指针去读它的头部 ——
/// 具体的载荷类型 `P` 在这里已经被擦掉了，只剩下 `OpPayloadHeader` 这份布局约定。
///
/// # Safety
///
/// 读取者必须保证节点里存的确实是一个以 [`OpPayloadHeader`] 开头的载荷。
/// 这条契约由 `Rx::new_op` 的调用方维持（`silex_rx` 宏生成的代码）。
unsafe fn with_op_ptr<R>(id: StoredId, f: impl FnOnce(*const u8) -> R) -> Option<R> {
    // SAFETY: 契约转嫁给调用方；指针在本表达式内立刻用掉，其间不重入运行时。
    let ptr = unsafe { try_get_any_raw_untracked(id.raw()) }?;
    Some(f(ptr as *const u8))
}

/// 非泛型的 track 逻辑实现 (Dispatcher)。
/// 剥离了泛型分发，使所有类型的 Rx 共享相同的机器码。
#[inline(always)]
pub fn track(id: RawId, kind: RxNodeKind) {
    match kind {
        RxNodeKind::Signal => signal::track(SignalId::from_raw_unchecked(id)),
        RxNodeKind::Stored | RxNodeKind::Closure => {}
        RxNodeKind::Op => {
            // SAFETY: `RxNodeKind::Op` 保证载荷以 `OpPayloadHeader` 开头。
            let _ = unsafe {
                with_op_ptr(StoredId::from_raw_unchecked(id), |ptr| {
                    let header = &*(ptr as *const OpPayloadHeader);
                    (header.track)(ptr);
                })
            };
        }
    }
}

/// 非泛型的销毁状态检查 (Dispatcher)。
#[inline(always)]
pub fn is_disposed(id: RawId, kind: RxNodeKind) -> bool {
    // 六个 `is_*_valid` 自由函数已经收敛成一个 `Handle::<K>::is_alive()`
    // （审计报告 §3.1）。这一层的种类在 `RxNodeKind` 里，所以在这里断言回去。
    match kind {
        RxNodeKind::Signal => !SignalId::from_raw_unchecked(id).is_alive(),
        RxNodeKind::Closure | RxNodeKind::Op | RxNodeKind::Stored => {
            !StoredId::from_raw_unchecked(id).is_alive()
        }
    }
}

/// 统一的 Panic 报告分发器。
/// 避免在每个泛型实例中生成冗长的字符串格式化代码。
#[cold]
#[inline(never)]
pub fn report_disposed(
    defined_at: Option<&'static Location<'static>>,
    debug_name: Option<String>,
    location: &'static Location<'static>,
) -> ! {
    if let Some(name) = debug_name {
        if let Some(defined_at) = defined_at {
            panic!(
                "At {location}, you tried to access a reactive value \"{name}\" which was \
                 defined at {defined_at}, but it has already been disposed."
            )
        } else {
            panic!(
                "At {location}, you tried to access a reactive value \"{name}\", but it has \
                 already been disposed."
            )
        }
    } else if let Some(defined_at) = defined_at {
        panic!(
            "At {location}, you tried to access a reactive value which was \
             defined at {defined_at}, but it has already been disposed."
        )
    } else {
        panic!(
            "At {location}, you tried to access a reactive value, but it has \
             already been disposed."
        )
    }
}

/// 核心分发：将数据读取到原始指针。
///
/// # Safety
///
/// 调用者必须确保 out 指向的内存有足够的空间存储 T，且 id 对应的类型确实是 T。
pub unsafe fn read_to_ptr(id: RawId, kind: RxNodeKind, out: *mut u8) -> bool {
    match kind {
        RxNodeKind::Signal | RxNodeKind::Stored => false,
        // SAFETY: `RxNodeKind::Op` 保证载荷以 `OpPayloadHeader` 开头；
        // `out` 的容量由本函数的调用方保证。
        RxNodeKind::Op => unsafe {
            with_op_ptr(StoredId::from_raw_unchecked(id), |ptr| {
                let header = &*(ptr as *const OpPayloadHeader);
                (header.read_to_ptr)(ptr, out)
            })
            .unwrap_or(false)
        },
        RxNodeKind::Closure => {
            // 闭包目前不支持 read_to_ptr，因为它通常返回 T 的所有权。
            // 由调用者通过 try_with_closure 处理。
            false
        }
    }
}

/// 泛型助手：将节点读取逻辑收拢。
/// 虽然此函数本身是泛型的，但它通过调用非泛型分发器来减少调用方的代码体积。
///
/// # Safety
///
/// 调用者必须确保 `id` 对应的节点确实存储了类型 `T`。
pub unsafe fn rx_read_node_untracked<'a, T: RxData>(
    id: RawId,
    kind: RxNodeKind,
) -> Option<RxGuard<'a, T, T>> {
    match kind {
        RxNodeKind::Signal => unsafe {
            signal::try_value_ref::<T>(SignalId::from_raw_unchecked(id)).map(|value| {
                RxGuard::Borrowed {
                    value,
                    token: Some(id),
                }
            })
        },
        RxNodeKind::Stored => unsafe {
            store::try_value_ref::<T>(StoredId::from_raw_unchecked(id)).map(|value| {
                RxGuard::Borrowed {
                    value,
                    token: Some(id),
                }
            })
        },
        RxNodeKind::Op => {
            let mut out = MaybeUninit::<T>::uninit();
            if unsafe { read_to_ptr(id, kind, out.as_mut_ptr() as *mut u8) } {
                Some(RxGuard::Owned(unsafe { out.assume_init() }))
            } else {
                None
            }
        }
        RxNodeKind::Closure => {
            store::try_with::<Box<dyn Fn() -> T>, _>(StoredId::from_raw_unchecked(id), |f| {
                RxGuard::Owned(f())
            })
            .ok()
        }
    }
}

/// 泛型助手：将节点访问逻辑收拢。
pub fn rx_try_with_node_untracked<T: RxData, U>(
    id: RawId,
    kind: RxNodeKind,
    fun: impl FnOnce(&T) -> U,
) -> Option<U> {
    match kind {
        RxNodeKind::Signal => {
            signal::try_with_untracked::<T, U>(SignalId::from_raw_unchecked(id), fun).ok()
        }
        RxNodeKind::Stored => store::try_with::<T, U>(StoredId::from_raw_unchecked(id), fun).ok(),
        RxNodeKind::Op => {
            let mut out = MaybeUninit::<T>::uninit();
            if unsafe { read_to_ptr(id, kind, out.as_mut_ptr() as *mut u8) } {
                let v = unsafe { out.assume_init() };
                Some(fun(&v))
            } else {
                None
            }
        }
        RxNodeKind::Closure => store::try_with::<Box<dyn Fn() -> T>, _>(
            StoredId::from_raw_unchecked(id),
            |f| fun(&f()),
        )
        .ok(),
    }
}
