use crate::RxNodeKind;
use crate::reactivity::NodeId;
use silex_reactivity::{
    is_closure_valid, is_op_valid, is_signal_valid, is_stored_value_valid, track_signal,
    try_with_op,
};
use std::panic::Location;

/// 非泛型的 track 逻辑实现 (Dispatcher)。
/// 剥离了泛型分发，使所有类型的 Rx 共享相同的机器码。
#[inline(always)]
pub fn track(id: NodeId, kind: RxNodeKind) {
    match kind {
        RxNodeKind::Signal | RxNodeKind::Stored | RxNodeKind::Closure => {
            track_signal(id);
        }
        RxNodeKind::Op => {
            let _ = try_with_op(id, |buffer| {
                use crate::reactivity::OpPayloadHeader;
                let header = unsafe { &*(buffer.data.as_ptr() as *const OpPayloadHeader) };
                (header.track)(buffer.data.as_ptr());
            });
        }
    }
}

/// 非泛型的销毁状态检查 (Dispatcher)。
#[inline(always)]
pub fn is_disposed(id: NodeId, kind: RxNodeKind) -> bool {
    match kind {
        RxNodeKind::Signal => !is_signal_valid(id),
        RxNodeKind::Closure => !is_closure_valid(id),
        RxNodeKind::Op => !is_op_valid(id),
        RxNodeKind::Stored => !is_stored_value_valid(id),
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
pub unsafe fn read_to_ptr(id: NodeId, kind: RxNodeKind, out: *mut u8) -> bool {
    match kind {
        RxNodeKind::Signal | RxNodeKind::Stored => false,
        RxNodeKind::Op => silex_reactivity::try_with_op(id, |buffer| {
            use crate::reactivity::OpPayloadHeader;
            let header = unsafe { &*(buffer.data.as_ptr() as *const OpPayloadHeader) };
            unsafe { (header.read_to_ptr)(buffer.data.as_ptr(), out) }
        })
        .unwrap_or(false),
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
pub unsafe fn rx_read_node_untracked<'a, T: crate::traits::RxData>(
    id: NodeId,
    kind: RxNodeKind,
) -> Option<crate::traits::RxGuard<'a, T, T>> {
    match kind {
        RxNodeKind::Signal | RxNodeKind::Stored => unsafe {
            silex_reactivity::try_get_any_raw_untracked(id).map(|ptr| {
                crate::traits::RxGuard::Borrowed {
                    value: &*(ptr as *const T),
                    token: Some(crate::NodeRef::from_id(id)),
                }
            })
        },
        RxNodeKind::Op => {
            let mut out = std::mem::MaybeUninit::<T>::uninit();
            if unsafe { read_to_ptr(id, kind, out.as_mut_ptr() as *mut u8) } {
                Some(crate::traits::RxGuard::Owned(unsafe { out.assume_init() }))
            } else {
                None
            }
        }
        RxNodeKind::Closure => {
            silex_reactivity::try_with_closure::<Box<dyn Fn() -> T>, _>(id, |f| {
                crate::traits::RxGuard::Owned(f())
            })
        }
    }
}

/// 泛型助手：将节点访问逻辑收拢。
pub fn rx_try_with_node_untracked<T: crate::traits::RxData, U>(
    id: NodeId,
    kind: RxNodeKind,
    fun: impl FnOnce(&T) -> U,
) -> Option<U> {
    match kind {
        RxNodeKind::Signal | RxNodeKind::Stored => unsafe {
            silex_reactivity::try_get_any_raw_untracked(id).map(|ptr| fun(&*(ptr as *const T)))
        },
        RxNodeKind::Op => {
            let mut out = std::mem::MaybeUninit::<T>::uninit();
            if unsafe { read_to_ptr(id, kind, out.as_mut_ptr() as *mut u8) } {
                let v = unsafe { out.assume_init() };
                Some(fun(&v))
            } else {
                None
            }
        }
        RxNodeKind::Closure => {
            silex_reactivity::try_with_closure::<Box<dyn Fn() -> T>, _>(id, |f| fun(&f()))
        }
    }
}
