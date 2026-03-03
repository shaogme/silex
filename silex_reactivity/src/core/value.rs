use crate::core::FuncPtr;
use silex_vtable::AnyBox;
use std::any::TypeId;
use std::marker::PhantomData;
use std::mem;
use std::ptr;

/// A type-erased value with Small Object Optimization (SOO).
pub(crate) struct AnyValue {
    inner: AnyBox<AnyValueVTable>,
    type_id: TypeId,
}

type CloneFn = unsafe fn(*const u8, TypeId, &'static AnyValueVTable) -> AnyValue;
type EqFn = unsafe fn(*const u8, *const u8) -> bool;

struct AnyValueVTable {
    as_ptr: FuncPtr<unsafe fn(*const u8) -> *const ()>,
    as_mut_ptr: FuncPtr<unsafe fn(*mut u8) -> *mut ()>,
    drop: FuncPtr<unsafe fn(*mut u8)>,
    clone: Option<FuncPtr<CloneFn>>,
    eq: Option<FuncPtr<EqFn>>,
}

impl AnyValue {
    /// 创建一个普通的类型擦除值。不支持克隆和比较。
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let type_id = TypeId::of::<T>();

        // 物理原型共享：如果类型不需要 drop（如 Copy 类型），共享同一个 VTable
        let (v_stack, v_heap) = if !mem::needs_drop::<T>() {
            (&COPY_NON_REACTIVE_VTABLE, &BoxedVTable::<T>::VTABLE)
        } else {
            (&InlineVTable::<T>::VTABLE, &BoxedVTable::<T>::VTABLE)
        };

        AnyValue {
            inner: AnyBox::new(value, v_stack, v_heap),
            type_id,
        }
    }

    /// 创建一个支持响应式操作（克隆、比较）的类型擦除值。
    pub(crate) fn new_reactive<T: Clone + PartialEq + 'static>(value: T) -> Self {
        let type_id = TypeId::of::<T>();

        let (v_stack, v_heap) = if !mem::needs_drop::<T>() && is_bitwise_equatable(type_id) {
            (
                &BITWISE_EQ_COPY_INLINE_VTABLE,
                &BoxedReactiveVTable::<T>::VTABLE,
            )
        } else {
            (
                &InlineReactiveVTable::<T>::VTABLE,
                &BoxedReactiveVTable::<T>::VTABLE,
            )
        };

        AnyValue {
            inner: AnyBox::new(value, v_stack, v_heap),
            type_id,
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Self> {
        self.inner
            .vtable
            .clone
            .map(|f| unsafe { f.as_fn()(self.inner.as_ptr(), self.type_id, self.inner.vtable) })
    }

    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.type_id != other.type_id {
            return false;
        }
        self.inner
            .vtable
            .eq
            .is_some_and(|f| unsafe { f.as_fn()(self.inner.as_ptr(), other.inner.as_ptr()) })
    }

    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.inner.vtable.as_ptr.as_fn()(self.inner.as_ptr());
                Some(&*(val_ptr as *const T))
            }
        } else {
            None
        }
    }

    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.inner.vtable.as_mut_ptr.as_fn()(self.inner.as_mut_ptr());
                Some(&mut *(val_ptr as *mut T))
            }
        } else {
            None
        }
    }

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

// --- Shared VTable Functions ---

unsafe fn shared_drop_noop(_: *mut u8) {}

unsafe fn shared_clone_bitwise(
    ptr: *const u8,
    type_id: TypeId,
    vtable: &'static AnyValueVTable,
) -> AnyValue {
    let mut inner = AnyBox {
        data: [0usize; 3],
        vtable,
    };
    unsafe {
        ptr::copy_nonoverlapping(ptr, inner.as_mut_ptr(), silex_vtable::SOO_CAPACITY);
    }
    AnyValue { inner, type_id }
}

unsafe fn shared_eq_bitwise(p1: *const u8, p2: *const u8) -> bool {
    let (s1, s2) = unsafe {
        (
            std::slice::from_raw_parts(p1, silex_vtable::SOO_CAPACITY),
            std::slice::from_raw_parts(p2, silex_vtable::SOO_CAPACITY),
        )
    };
    s1 == s2
}

fn is_bitwise_equatable(id: TypeId) -> bool {
    id == TypeId::of::<i8>()
        || id == TypeId::of::<i16>()
        || id == TypeId::of::<i32>()
        || id == TypeId::of::<i64>()
        || id == TypeId::of::<i128>()
        || id == TypeId::of::<u8>()
        || id == TypeId::of::<u16>()
        || id == TypeId::of::<u32>()
        || id == TypeId::of::<u64>()
        || id == TypeId::of::<u128>()
        || id == TypeId::of::<usize>()
        || id == TypeId::of::<isize>()
        || id == TypeId::of::<bool>()
        || id == TypeId::of::<char>()
}

// --- Shared VTables ---

static COPY_NON_REACTIVE_VTABLE: AnyValueVTable = AnyValueVTable {
    as_ptr: FuncPtr::new(|ptr| ptr as *const ()),
    as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut ()),
    drop: FuncPtr::new(shared_drop_noop),
    clone: None,
    eq: None,
};

static BITWISE_EQ_COPY_INLINE_VTABLE: AnyValueVTable = AnyValueVTable {
    as_ptr: FuncPtr::new(|ptr| ptr as *const ()),
    as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut ()),
    drop: FuncPtr::new(shared_drop_noop),
    clone: Some(FuncPtr::new(shared_clone_bitwise)),
    eq: Some(FuncPtr::new(shared_eq_bitwise)),
};

// --- VTable Generators ---

struct InlineVTable<T>(PhantomData<T>);
impl<T: 'static> InlineVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) }),
        clone: None,
        eq: None,
    };
}

struct InlineReactiveVTable<T>(PhantomData<T>);
impl<T: Clone + PartialEq + 'static> InlineReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) }),
        clone: Some(FuncPtr::new(|ptr, type_id, vtable| {
            let val = unsafe { (*(ptr as *const T)).clone() };
            AnyValue {
                inner: AnyBox::new(val, vtable, vtable), // Note: this logic might need care if vtable is not the same for heap
                type_id,
            }
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            *(p1 as *const T) == *(p2 as *const T)
        })),
    };
}

struct BoxedVTable<T>(PhantomData<T>);
impl<T: 'static> BoxedVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        clone: None,
        eq: None,
    };
}

struct BoxedReactiveVTable<T>(PhantomData<T>);
impl<T: Clone + PartialEq + 'static> BoxedReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        clone: Some(FuncPtr::new(|ptr, type_id, vtable| {
            let boxed = unsafe { &**(ptr as *const Box<T>) };
            let new_boxed = Box::new(boxed.clone());
            AnyValue {
                inner: AnyBox::new(new_boxed, vtable, vtable),
                type_id,
            }
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            (**(p1 as *const Box<T>)) == (**(p2 as *const Box<T>))
        })),
    };
}

// --- ThunkValue for Closures ---

pub(crate) struct ThunkValue {
    inner: AnyBox<ThunkVTable>,
}

#[repr(C)]
pub(crate) struct ThunkVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
    pub(crate) call: FuncPtr<unsafe fn(*mut u8, *const ())>,
}

unsafe impl Sync for ThunkVTable {}

impl ThunkValue {
    pub(crate) fn new_simple<F: Fn() + 'static>(f: F) -> Self {
        struct VGen<F>(PhantomData<F>);
        impl<F: Fn() + 'static> VGen<F> {
            const STACK: ThunkVTable = ThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut F) }),
                call: FuncPtr::new(|ptr, _| unsafe {
                    let f = &*(ptr as *const F);
                    f();
                }),
            };
            const HEAP: ThunkVTable = ThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<F>) }),
                call: FuncPtr::new(|ptr, _| unsafe {
                    let f = &**(ptr as *const Box<F>);
                    f();
                }),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F>::STACK, &VGen::<F>::HEAP),
        }
    }

    pub(crate) fn new_raw(data: [usize; 3], vtable: &'static ThunkVTable) -> Self {
        Self {
            inner: AnyBox { data, vtable },
        }
    }

    pub(crate) unsafe fn call(&self, rt: *const ()) {
        unsafe {
            (self.inner.vtable.call.as_fn())(self.inner.as_ptr() as *mut u8, rt);
        }
    }
}

impl Drop for ThunkValue {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- OnceThunk for FnOnce ---

pub(crate) struct OnceThunk {
    inner: AnyBox<OnceThunkVTable>,
}

pub(crate) struct OnceThunkVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
    pub(crate) call_and_drop: FuncPtr<unsafe fn(*mut u8)>,
}

unsafe impl Sync for OnceThunkVTable {}

impl OnceThunk {
    pub(crate) fn new<F: FnOnce() + 'static>(f: F) -> Self {
        struct VGen<F>(PhantomData<F>);
        impl<F: FnOnce() + 'static> VGen<F> {
            const STACK: OnceThunkVTable = OnceThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut F) }),
                call_and_drop: FuncPtr::new(|ptr| unsafe {
                    let f = ptr::read(ptr as *mut F);
                    f();
                }),
            };
            const HEAP: OnceThunkVTable = OnceThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<F>) }),
                call_and_drop: FuncPtr::new(|ptr| unsafe {
                    let f = ptr::read(ptr as *mut Box<F>);
                    (*f)();
                }),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F>::STACK, &VGen::<F>::HEAP),
        }
    }

    pub(crate) fn call(self) {
        let mut this = mem::ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe {
            (vtable.call_and_drop.as_fn())(data_ptr);
        }
    }
}

impl Drop for OnceThunk {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}
