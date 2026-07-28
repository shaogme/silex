use crate::internal::FuncPtr;
use silex_vtable::{AnyBox, OnceBox, ThunkBox};
use std::{any::TypeId, marker::PhantomData, mem, ptr};

/// 支持小对象优化 (SOO) 的类型擦除值容器。
///
/// # 不变量
///
/// `inner.data` 里存的一定是 `inner.vtable.type_id` 所指的那个类型：小值直接
/// 内联在缓冲区里，大值（或对齐要求高的值）存的是一个 `Box<T>`，两种情形分别
/// 由 `Inline*VTable` 与 `Boxed*VTable` 处理。构造函数是唯一的写入口，
/// 它保证 vtable 与实际存放的表示配对，本类型所有 `unsafe` 都建立在这一点上。
pub(crate) struct AnyValue {
    inner: AnyBox<AnyValueVTable>,
}

type EqFn = unsafe fn(*const u8, *const u8) -> bool;

struct AnyValueVTable {
    type_id: TypeId,
    as_ptr: FuncPtr<unsafe fn(*const u8) -> *const ()>,
    as_mut_ptr: FuncPtr<unsafe fn(*mut u8) -> *mut ()>,
    drop: FuncPtr<unsafe fn(*mut u8)>,
    /// `None` 表示该值不参与相等性比较 —— [`AnyValue::try_eq`] 一律返回 `false`（视为变更）。
    /// Signal 与无 `PartialEq` 约束的 Derived 节点使用该类型；
    /// 带比较能力的 Memo 节点则使用 [`AnyValue::new_reactive`] 注入真正的比较逻辑。
    eq: Option<FuncPtr<EqFn>>,
}

impl AnyValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        AnyValue {
            inner: AnyBox::new(value, &InlineVTable::<T>::VTABLE, &BoxedVTable::<T>::VTABLE),
        }
    }

    /// 创建带相等性比较能力的类型擦除值（供 Memo 节点的计算结果使用）。
    pub(crate) fn new_reactive<T: PartialEq + 'static>(value: T) -> Self {
        AnyValue {
            inner: AnyBox::new(
                value,
                &InlineReactiveVTable::<T>::VTABLE,
                &BoxedReactiveVTable::<T>::VTABLE,
            ),
        }
    }

    /// 比较两个擦除值是否相等。若 TypeId 不匹配或任一方缺少比较函数则返回 `false`。
    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.inner.vtable.type_id != other.inner.vtable.type_id {
            return false;
        }
        // SAFETY: type_id 已经比对，`eq` 函数接收的两个指针类型保证一致。
        self.inner
            .vtable
            .eq
            .is_some_and(|f| unsafe { f.as_fn()(self.inner.as_ptr(), other.inner.as_ptr()) })
    }

    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.inner.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.inner.vtable.as_ptr.as_fn()(self.inner.as_ptr());
                Some(&*(val_ptr as *const T))
            }
        } else {
            None
        }
    }

    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.inner.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.inner.vtable.as_mut_ptr.as_fn()(self.inner.as_mut_ptr());
                Some(&mut *(val_ptr as *mut T))
            }
        } else {
            None
        }
    }

    /// 获取内部底层数据的裸指针。
    ///
    /// # Safety
    ///
    /// 调用方需保证类型匹配，且使用期间 `AnyValue` 不发生移动或改写。
    pub(crate) unsafe fn as_ptr(&self) -> *const () {
        unsafe { self.inner.vtable.as_ptr.as_fn()(self.inner.as_ptr()) }
    }
}

impl Drop for AnyValue {
    fn drop(&mut self) {
        unsafe {
            self.inner.vtable.drop.as_fn()(self.inner.as_mut_ptr());
        }
    }
}

// --- 虚表生成助手 ---

unsafe fn shared_drop_noop(_: *mut u8) {}

struct InlineVTable<T>(PhantomData<T>);
impl<T: 'static> InlineVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: if mem::needs_drop::<T>() {
            FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) })
        } else {
            FuncPtr::new(shared_drop_noop)
        },
        eq: None,
    };
}

struct InlineReactiveVTable<T>(PhantomData<T>);
impl<T: PartialEq + 'static> InlineReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: if mem::needs_drop::<T>() {
            FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) })
        } else {
            FuncPtr::new(shared_drop_noop)
        },
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            *(p1 as *const T) == *(p2 as *const T)
        })),
    };
}

struct BoxedVTable<T>(PhantomData<T>);
impl<T: 'static> BoxedVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        eq: None,
    };
}

struct BoxedReactiveVTable<T>(PhantomData<T>);
impl<T: PartialEq + 'static> BoxedReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            (**(p1 as *const Box<T>)) == (**(p2 as *const Box<T>))
        })),
    };
}

// --- 计算闭包容器 ---

/// 节点的计算闭包包装。
pub(crate) enum Computation {
    /// 副作用计算闭包 (Effect)。
    Effect(EffectThunk),
    /// 派生与缓存计算闭包 (Memo)。
    Memo(MemoThunk),
}

pub(crate) struct EffectThunk(ThunkBox<(), ()>);

impl EffectThunk {
    /// 从 `FnMut` 构造 Effect 计算闭包。
    ///
    /// 内部采用 `RefCell` 保护，若同节点内部发生意外重入借用将抛出明确的 panic。
    pub(crate) fn new<F: FnMut() + 'static>(f: F) -> Self {
        let cell = std::cell::RefCell::new(f);
        Self(ThunkBox::new(move |()| {
            let mut f = cell
                .try_borrow_mut()
                .expect("effect 在自己的执行过程中被重入了");
            f()
        }))
    }

    /// 执行 Effect 计算。
    #[inline]
    pub(crate) fn call(&self) {
        self.0.call(());
    }
}

/// Memo/Derived 计算闭包容器：接受上一次计算旧值的只读指针并返回新值。
pub(crate) struct MemoThunk(ThunkBox<Option<*const AnyValue>, AnyValue>);

impl MemoThunk {
    /// 构造带相等性门控的 Memo 计算闭包。
    pub(crate) fn new<T, F>(f: F) -> Self
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        Self(ThunkBox::new(move |old: Option<*const AnyValue>| {
            // SAFETY: `old` 来自驱动循环栈帧上的合法引用，在调用期间持续有效。
            let old_t = old.and_then(|p| unsafe { (*p).downcast_ref::<T>() });
            AnyValue::new_reactive(f(old_t))
        }))
    }

    /// 构造无相等性门控的 Derived 派生闭包。
    pub(crate) fn new_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> Self {
        Self(ThunkBox::new(move |_| AnyValue::new(f())))
    }

    /// 算一份新值。`old` 是上一次的结果（首算时为 `None`）。
    #[inline]
    pub(crate) fn compute(&self, old: Option<&AnyValue>) -> AnyValue {
        self.0.call(old.map(std::ptr::from_ref))
    }
}

// --- OnceThunk for FnOnce ---

pub(crate) struct OnceThunk(pub(crate) OnceBox<(), ()>);

impl OnceThunk {
    pub(crate) fn new<F: FnOnce() + 'static>(f: F) -> Self {
        Self(OnceBox::new(move |_| f()))
    }

    pub(crate) fn call(self) {
        self.0.call(());
    }
}

#[cfg(test)]
mod tests {
    //! `AnyValue` 的内联/装箱两条路径、比较语义与析构语义（AUDIT P18）。

    use super::*;
    use std::{cell::Cell, rc::Rc};

    /// 大到一定放不进内联缓冲区，走 `Box` 那条路径。
    #[derive(Clone, PartialEq, Debug)]
    struct Big([u64; 16]);

    #[test]
    fn downcast_only_succeeds_for_the_stored_type() {
        let mut v = AnyValue::new(7i32);
        assert_eq!(v.downcast_ref::<i32>(), Some(&7));
        assert_eq!(v.downcast_ref::<u32>(), None);
        assert_eq!(v.downcast_ref::<String>(), None);

        *v.downcast_mut::<i32>().expect("类型相符") += 1;
        assert_eq!(v.downcast_ref::<i32>(), Some(&8));
        assert!(v.downcast_mut::<u8>().is_none());
    }

    #[test]
    fn the_boxed_path_round_trips() {
        let big = Big([3; 16]);
        let v = AnyValue::new_reactive(big.clone());
        assert_eq!(v.downcast_ref::<Big>(), Some(&big));

        let same = AnyValue::new_reactive(big);
        let other = AnyValue::new_reactive(Big([4; 16]));
        assert!(v.try_eq(&same));
        assert!(!v.try_eq(&other));
    }

    #[test]
    fn values_without_a_comparator_never_compare_equal() {
        // `AnyValue::new`（signal / derived 用的构造函数）不带比较函数，
        // 因此 `try_eq` 恒为 false —— 也就是“每次写入都算变化”（AUDIT P10）。
        let a = AnyValue::new(1i32);
        let b = AnyValue::new(1i32);
        assert!(!a.try_eq(&b));

        // 而 memo 用的构造函数带比较函数。
        let c = AnyValue::new_reactive(1i32);
        let d = AnyValue::new_reactive(1i32);
        assert!(c.try_eq(&d));
    }

    #[test]
    fn comparing_across_types_is_false_not_a_transmute() {
        let a = AnyValue::new_reactive(1u8);
        let b = AnyValue::new_reactive(1u64);
        assert!(!a.try_eq(&b), "类型不同必须直接判不等，不能按位比较");
    }

    /// 小于一个字节的类型曾经走“按 24 字节整体比较”的快路径，正确性依赖
    /// 缓冲区被零初始化（AUDIT P19.1）。现在按 `T` 自己的 `PartialEq` 比较。
    #[test]
    fn one_byte_types_compare_by_value() {
        assert!(AnyValue::new_reactive(true).try_eq(&AnyValue::new_reactive(true)));
        assert!(!AnyValue::new_reactive(true).try_eq(&AnyValue::new_reactive(false)));
        assert!(AnyValue::new_reactive('x').try_eq(&AnyValue::new_reactive('x')));
    }

    struct DropSpy(Rc<Cell<usize>>);
    impl Drop for DropSpy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn dropping_runs_the_inner_destructor_exactly_once() {
        let hits = Rc::new(Cell::new(0));
        {
            let _v = AnyValue::new(DropSpy(hits.clone()));
            assert_eq!(hits.get(), 0);
        }
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn dropping_a_boxed_value_runs_its_destructor_too() {
        let hits = Rc::new(Cell::new(0));
        {
            // 连同一个大数组一起放进去，逼它走 `Box` 路径。
            let _v = AnyValue::new((DropSpy(hits.clone()), [0u64; 16]));
            assert_eq!(hits.get(), 0);
        }
        assert_eq!(hits.get(), 1);
    }
}
