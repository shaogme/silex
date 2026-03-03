use crate::core::FuncPtr;
use silex_vtable::AnyBox;
use std::any::TypeId;
use std::marker::PhantomData;
use std::mem;
use std::ptr;

/// A type-erased value with Small Object Optimization (SOO).
pub(crate) struct AnyValue {
    inner: AnyBox<AnyValueVTable>,
}

type CloneFn = unsafe fn(*const u8, &'static AnyValueVTable) -> AnyValue;
type EqFn = unsafe fn(*const u8, *const u8) -> bool;

struct AnyValueVTable {
    type_id: TypeId,
    as_ptr: FuncPtr<unsafe fn(*const u8) -> *const ()>,
    as_mut_ptr: FuncPtr<unsafe fn(*mut u8) -> *mut ()>,
    drop: FuncPtr<unsafe fn(*mut u8)>,
    clone: Option<FuncPtr<CloneFn>>,
    eq: Option<FuncPtr<EqFn>>,
}

impl AnyValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        AnyValue {
            inner: AnyBox::new(value, &InlineVTable::<T>::VTABLE, &BoxedVTable::<T>::VTABLE),
        }
    }

    /// 创建一个支持响应式操作（克隆、比较）的类型擦除值。
    pub(crate) fn new_reactive<T: Clone + PartialEq + 'static>(value: T) -> Self {
        let is_bitwise = !mem::needs_drop::<T>() && is_bitwise_equatable(TypeId::of::<T>());
        let v_stack = if is_bitwise {
            &InlineReactiveVTable::<T>::BITWISE
        } else {
            &InlineReactiveVTable::<T>::NORMAL
        };

        AnyValue {
            inner: AnyBox::new(value, v_stack, &BoxedReactiveVTable::<T>::VTABLE),
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Self> {
        self.inner
            .vtable
            .clone
            .map(|f| unsafe { f.as_fn()(self.inner.as_ptr(), self.inner.vtable) })
    }

    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.inner.vtable.type_id != other.inner.vtable.type_id {
            return false;
        }
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

unsafe fn shared_clone_bitwise(ptr: *const u8, vtable: &'static AnyValueVTable) -> AnyValue {
    let mut inner = AnyBox {
        data: [0usize; 3],
        vtable,
    };
    unsafe {
        ptr::copy_nonoverlapping(ptr, inner.as_mut_ptr(), silex_vtable::SOO_CAPACITY);
    }
    AnyValue { inner }
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

// --- VTable Generators ---

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
        clone: None,
        eq: None,
    };
}

struct InlineReactiveVTable<T>(PhantomData<T>);
impl<T: Clone + PartialEq + 'static> InlineReactiveVTable<T> {
    const NORMAL: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: if mem::needs_drop::<T>() {
            FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) })
        } else {
            FuncPtr::new(shared_drop_noop)
        },
        clone: Some(FuncPtr::new(|ptr, vtable| {
            let val = unsafe { (*(ptr as *const T)).clone() };
            AnyValue {
                inner: AnyBox::new(val, vtable, &BoxedReactiveVTable::<T>::VTABLE),
            }
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            *(p1 as *const T) == *(p2 as *const T)
        })),
    };

    const BITWISE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(shared_drop_noop),
        clone: Some(FuncPtr::new(shared_clone_bitwise)),
        eq: Some(FuncPtr::new(shared_eq_bitwise)),
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
        clone: None,
        eq: None,
    };
}

struct BoxedReactiveVTable<T>(PhantomData<T>);
impl<T: Clone + PartialEq + 'static> BoxedReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        clone: Some(FuncPtr::new(|ptr, vtable| {
            let boxed = unsafe { &**(ptr as *const Box<T>) };
            let new_boxed = Box::new(boxed.clone());
            AnyValue {
                inner: AnyBox::new(new_boxed, vtable, vtable),
            }
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            (**(p1 as *const Box<T>)) == (**(p2 as *const Box<T>))
        })),
    };
}

// --- ThunkValue for Closures ---

pub(crate) type ThunkVTable = silex_vtable::ThunkBoxVTable<*const (), ()>;

pub(crate) struct ThunkValue(pub(crate) silex_vtable::ThunkBox<*const (), ()>);

impl ThunkValue {
    pub(crate) fn new_simple<F: Fn() + 'static>(f: F) -> Self {
        Self(silex_vtable::ThunkBox::new(move |_| f()))
    }

    pub(crate) fn new_raw(data: [usize; 3], vtable: &'static ThunkVTable) -> Self {
        Self(silex_vtable::ThunkBox::from_raw(data, vtable))
    }

    pub(crate) unsafe fn call(&self, rt: *const ()) {
        self.0.call(rt);
    }
}

// --- OnceThunk for FnOnce ---

pub(crate) struct OnceThunk(pub(crate) silex_vtable::OnceBox<(), ()>);

impl OnceThunk {
    pub(crate) fn new<F: FnOnce() + 'static>(f: F) -> Self {
        Self(silex_vtable::OnceBox::new(move |_| f()))
    }

    pub(crate) fn call(self) {
        self.0.call(());
    }
}
