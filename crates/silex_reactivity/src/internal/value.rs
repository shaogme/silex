use crate::internal::FuncPtr;
use silex_vtable::{AnyBox, OnceBox, ThunkBox};
use std::{any::TypeId, marker::PhantomData, mem, ptr};

/// A raw value with Small Object Optimization (SOO).
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
    /// `None` 表示这个值不参与相等性比较 —— [`AnyValue::try_eq`] 一律返回 `false`，
    /// 也就是“每次写入都算变化”。signal 与 `register_derived` 走的就是这条路
    /// （它们的 `T` 只有 `'static` 约束，根本没有 `PartialEq`），memo 则用
    /// [`AnyValue::new_reactive`] 带上真正的比较函数（AUDIT P10）。
    eq: Option<FuncPtr<EqFn>>,
}

impl AnyValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        AnyValue {
            inner: AnyBox::new(value, &InlineVTable::<T>::VTABLE, &BoxedVTable::<T>::VTABLE),
        }
    }

    /// 创建一个带相等性比较能力的类型擦除值（memo 的重算结果走这里）。
    pub(crate) fn new_reactive<T: PartialEq + 'static>(value: T) -> Self {
        AnyValue {
            inner: AnyBox::new(
                value,
                &InlineReactiveVTable::<T>::VTABLE,
                &BoxedReactiveVTable::<T>::VTABLE,
            ),
        }
    }

    /// 两个值是否相等。类型不同、或任一方不带比较函数时一律返回 `false`
    /// （即“当作变化处理”）。
    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.inner.vtable.type_id != other.inner.vtable.type_id {
            return false;
        }
        // SAFETY: 两侧的 type_id 已经比对过，因此 `eq` 拿到的两个指针指向的
        // 确实是同一个类型；指针由各自的 `AnyBox` 提供，在本表达式期间有效。
        self.inner
            .vtable
            .eq
            .is_some_and(|f| unsafe { f.as_fn()(self.inner.as_ptr(), other.inner.as_ptr()) })
    }

    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.inner.vtable.type_id == TypeId::of::<T>() {
            // SAFETY: type_id 相符即说明缓冲区里存的就是 `T`（见类型的不变量）；
            // `as_ptr` 会替我们区分内联表示与 `Box` 表示，返回指向 `T` 本身的指针。
            // 返回的引用绑定在 `&self` 上，不会比 `AnyValue` 活得更久。
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
            // SAFETY: 同 `downcast_ref`，独占性由 `&mut self` 保证。
            unsafe {
                let val_ptr = self.inner.vtable.as_mut_ptr.as_fn()(self.inner.as_mut_ptr());
                Some(&mut *(val_ptr as *mut T))
            }
        } else {
            None
        }
    }

    /// 直接交出内部值的裸指针，不做任何类型检查。
    ///
    /// # Safety
    ///
    /// 调用方必须自己知道里面存的是什么类型，并且保证在使用该指针期间这个
    /// `AnyValue` 既不被移动也不被写入（内联表示的地址随 `AnyValue` 一起移动）。
    pub(crate) unsafe fn as_ptr(&self) -> *const () {
        // SAFETY: 缓冲区与 vtable 是配对的，`as_ptr` 只是按存储形式解一次引用。
        unsafe { self.inner.vtable.as_ptr.as_fn()(self.inner.as_ptr()) }
    }
}

impl Drop for AnyValue {
    fn drop(&mut self) {
        // SAFETY: 值只在这里析构一次（`AnyValue` 不是 `Copy`，也没有别的路径
        // 会调用 vtable 的 drop），且 vtable 与缓冲区里的表示配对。
        unsafe {
            self.inner.vtable.drop.as_fn()(self.inner.as_mut_ptr());
        }
    }
}

// --- Shared VTable Functions ---

/// `!needs_drop::<T>()` 时用它顶替真正的析构函数，省掉一次单态化。
unsafe fn shared_drop_noop(_: *mut u8) {}

// 这里曾经有一条“按位”快路径：对 14 个原始类型用 `TypeId` 比对选中一组共享的
// clone/eq 函数，它们按固定的 `SOO_CAPACITY`（24 字节）整体复制/比较字节。
//
// 三个问题一并去掉了：
// 1. 按位比较 `bool` 这种 1 字节类型时会读到值以外的 23 个字节，正确性完全
//    依赖 `InlineStorage` “构造时整体零初始化”这条没有被任何地方强制的不变量
//    （AUDIT P19.1）；
// 2. 类型判定是**运行时**最多 14 次 `TypeId` 比较，每次 `new_reactive` 都要跑
//    一遍（AUDIT P19.2）；
// 3. 按位克隆随 AUDIT P9 一起失去了最后一个调用者 —— memo 不再克隆旧值。
//
// 而它换来的东西是负的：单态化出来的 `*(p as *const T) == *(p as *const T)`
// 比 24 字节 memcmp 更快，drop 也照样是 no-op。

// --- VTable Generators ---
//
// 下面四张表里的每一个闭包都是 vtable 的一项，它们收到的 `ptr` 永远是
// `AnyBox` 的数据区起始处。
//
// # Safety（对四张表统一成立）
//
// - `Inline*` 只会被装着 `T` 本体的 `AnyBox` 选中，`Boxed*` 只会被装着
//   `Box<T>` 的选中 —— 这是 `AnyBox::new` 按 `InlineStorage::fits::<T>()`
//   分派的结果，也是 `AnyValue` 的类型不变量；
// - 因此把 `ptr` 转成 `*mut T` / `*mut Box<T>` 再解引用是合法的；
// - 每张表的 `type_id` 就是 `T` 的 `TypeId`，调用方（`downcast_*` / `try_eq`）
//   必须先比对它才能使用这些函数。

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

// --- 计算闭包 ---

/// 一个节点的计算闭包。
///
/// 两种计算的**签名本来就不一样** —— effect 是 `FnMut()`，memo 是
/// `Fn(Option<&T>) -> T`。阶段三方案 B 之前，运行时强行让它们共用一个
/// `ThunkBox<*const (), ()>`：memo 的闭包被手工打包进 `InlineStorage`
/// （偏移 0 处塞一个 `*const MemoVTable`，其后才是闭包），由一个
/// “通用 runner” 在运行时把 vtable 读回来、从 `current_owner()` 反查自己是谁、
/// 再回调进运行时提交结果。整整一层间接，外加十来处 `unsafe`。
///
/// 那层间接存在的唯一理由是“`run_node` 只想认识一种 thunk 类型”。
/// 驱动循环把用户代码从运行时内部提到外面之后，**驱动本来就知道 id、
/// 也拿得到旧值**，于是可以直接分派 —— 一个两变体的枚举就够了。
pub(crate) enum Computation {
    /// 副作用：跑就完了，没有返回值。
    Effect(EffectThunk),
    /// 派生值：拿上一次的结果（首算时没有），算出新的一份。
    Memo(MemoThunk),
}

pub(crate) struct EffectThunk(ThunkBox<(), ()>);

impl EffectThunk {
    /// 从一个 `FnMut` 构造。
    ///
    /// effect 从前只接受 `Fn()`，想在 effect 里维护一点状态就得自己套
    /// `Cell` / `RefCell`（审计报告 §3.4）。`FnMut` 在这个模型下是安全的：
    /// 同一个节点在同一时刻只可能有一次执行 —— 运行前置的 `running` 标志
    /// 会让重入的那次直接返回 false（AUDIT P1）。
    ///
    /// 这里仍然用 `RefCell` 而不是 `UnsafeCell`：`running` 标志是运行时的不变量，
    /// 而这段代码在 `ThunkBox` 里，离那个不变量很远。真出现重入时 `RefCell`
    /// 给的是一句明确的 panic，`UnsafeCell` 给的是 UB —— 这正是本轮审计要消除的
    /// “靠注释维系独占性”。开销是一次标志检查，effect 不是热路径。
    pub(crate) fn new<F: FnMut() + 'static>(f: F) -> Self {
        let cell = std::cell::RefCell::new(f);
        Self(ThunkBox::new(move |()| {
            let mut f = cell
                .try_borrow_mut()
                .expect("effect 在自己的执行过程中被重入了：这是运行时的 bug");
            f()
        }))
    }

    /// 执行这个 effect。
    ///
    /// 不再需要一个 `*const Runtime` 参数：从前 memo 的 thunk 要靠它把运行时
    /// 转回来提交结果，而现在提交由驱动循环负责，effect 体本身通过
    /// 线程本地的运行时访问一切。
    #[inline]
    pub(crate) fn call(&self) {
        self.0.call(());
    }
}

/// memo / derived 的计算闭包：`Option<&AnyValue> -> AnyValue`。
///
/// 参数走裸指针而不是引用，因为 [`ThunkBox`] 的 `Args` 必须是 `'static`，
/// 装不下一个带生命周期的 `&'a AnyValue`。这是本类型仅有的一处 `unsafe`，
/// 而它顶替掉的是从前 `memo.rs` 里那三张手写 vtable 加 `runtime.rs` 里的
/// 通用 runner，一共十几处。
pub(crate) struct MemoThunk(ThunkBox<Option<*const AnyValue>, AnyValue>);

impl MemoThunk {
    /// 带相等性门控的 memo（`memo::create`）。
    pub(crate) fn new<T, F>(f: F) -> Self
    where
        T: PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        Self(ThunkBox::new(move |old: Option<*const AnyValue>| {
            // SAFETY: `old` 要么是 `None`，要么来自 `compute` 里一个当场还活着的
            // `&AnyValue`（旧值住在驱动循环的栈上），其存活期覆盖整个调用。
            let old_t = old.and_then(|p| unsafe { (*p).downcast_ref::<T>() });
            // 旧值按引用透传给用户闭包，绝不在这里克隆（AUDIT P9）。
            AnyValue::new_reactive(f(old_t))
        }))
    }

    /// 不做门控的派生节点（`memo::derived`）：`T` 只有 `'static`，没有
    /// `PartialEq` 可用，因此每一次重算都通知下游（AUDIT P10）。
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
