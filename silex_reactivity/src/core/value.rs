use crate::core::FuncPtr;
use std::alloc::Layout;
use std::any::TypeId;
use std::mem::{self, MaybeUninit};
use std::ptr;

/// The size of the inline buffer in `usize` units.
const INLINE_WORDS: usize = 3;

/// A type-erased value with Small Object Optimization (SOO).
pub(crate) struct AnyValue {
    vtable: &'static AnyValueVTable,
    data: MaybeUninit<[usize; INLINE_WORDS]>,
}

struct AnyValueVTable {
    type_id: TypeId,
    as_ptr: FuncPtr<unsafe fn(*const usize) -> *const ()>,
    as_mut_ptr: FuncPtr<unsafe fn(*mut usize) -> *mut ()>,
    drop: FuncPtr<unsafe fn(*mut usize)>,
    clone: Option<FuncPtr<unsafe fn(*const usize) -> AnyValue>>,
    eq: Option<FuncPtr<unsafe fn(*const usize, *const usize) -> bool>>,
}

// Non-generic helpers to reduce bloat
#[inline(never)]
fn any_value_new_internal(
    data: [usize; INLINE_WORDS],
    vtable: &'static AnyValueVTable,
) -> AnyValue {
    AnyValue {
        vtable,
        data: MaybeUninit::new(data),
    }
}

impl AnyValue {
    /// 创建一个普通的类型擦除值。不支持克隆和比较。
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };
            any_value_new_internal(data, &InlineVTable::<T>::VTABLE)
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            any_value_new_internal(data, &BoxedVTable::<T>::VTABLE)
        }
    }

    /// 创建一个支持响应式操作（克隆、比较）的类型擦除值。
    pub(crate) fn new_reactive<T: Clone + PartialEq + 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };
            any_value_new_internal(data, &InlineReactiveVTable::<T>::VTABLE)
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            any_value_new_internal(data, &BoxedReactiveVTable::<T>::VTABLE)
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Self> {
        self.vtable
            .clone
            .map(|f| unsafe { f.as_fn()(self.data.as_ptr() as *const usize) })
    }

    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.vtable.type_id != other.vtable.type_id {
            return false;
        }
        self.vtable.eq.map_or(false, |f| unsafe {
            f.as_fn()(
                self.data.as_ptr() as *const usize,
                other.data.as_ptr() as *const usize,
            )
        })
    }

    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.vtable.as_ptr.as_fn()(self.data.as_ptr() as *const usize);
                Some(&*(val_ptr as *const T))
            }
        } else {
            None
        }
    }

    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.vtable.as_mut_ptr.as_fn()(self.data.as_mut_ptr() as *mut usize);
                Some(&mut *(val_ptr as *mut T))
            }
        } else {
            None
        }
    }

    pub(crate) unsafe fn as_ptr(&self) -> *const () {
        unsafe { self.vtable.as_ptr.as_fn()(self.data.as_ptr() as *const usize) }
    }
}

impl Drop for AnyValue {
    fn drop(&mut self) {
        unsafe {
            self.vtable.drop.as_fn()(self.data.as_mut_ptr() as *mut usize);
        }
    }
}

// --- VTable Generators ---

struct InlineVTable<T>(std::marker::PhantomData<T>);
impl<T: 'static> InlineVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) }),
        clone: None,
        eq: None,
    };
}

struct InlineReactiveVTable<T>(std::marker::PhantomData<T>);
impl<T: Clone + PartialEq + 'static> InlineReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) }),
        clone: Some(FuncPtr::new(|ptr| {
            AnyValue::new_reactive(unsafe { (*(ptr as *const T)).clone() })
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            *(p1 as *const T) == *(p2 as *const T)
        })),
    };
}

struct BoxedVTable<T>(std::marker::PhantomData<T>);
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

struct BoxedReactiveVTable<T>(std::marker::PhantomData<T>);
impl<T: Clone + PartialEq + 'static> BoxedReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        clone: Some(FuncPtr::new(|ptr| {
            AnyValue::new_reactive(unsafe { (**(ptr as *const Box<T>)).clone() })
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            (**(p1 as *const Box<T>)) == (**(p2 as *const Box<T>))
        })),
    };
}

// --- ThunkValue for Closures ---

pub(crate) struct ThunkValue {
    vtable: &'static ThunkVTable,
    data: MaybeUninit<[usize; INLINE_WORDS]>,
}

#[repr(C)]
pub(crate) struct ThunkVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut usize)>,
    pub(crate) call: FuncPtr<unsafe fn(*mut usize, *const ())>,
}

unsafe impl Sync for ThunkVTable {}

trait ThunkVTableGen {
    const VTABLE: ThunkVTable;
}

macro_rules! impl_thunk_vtable {
    ($name:ident, $t:path, $call_logic:expr) => {
        struct $name<F>(std::marker::PhantomData<F>);
        impl<F: $t + 'static> ThunkVTableGen for $name<F> {
            const VTABLE: ThunkVTable = ThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut F) }),
                call: FuncPtr::new(|ptr, rt| unsafe {
                    let f = &*(ptr as *const F);
                    ($call_logic)(f, rt);
                }),
            };
        }
    };
}

macro_rules! impl_thunk_boxed_vtable {
    ($name:ident, $t:path, $call_logic:expr) => {
        struct $name<F>(std::marker::PhantomData<F>);
        impl<F: $t + 'static> ThunkVTableGen for $name<F> {
            const VTABLE: ThunkVTable = ThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<F>) }),
                call: FuncPtr::new(|ptr, rt| unsafe {
                    let f = &**(ptr as *const Box<F>);
                    ($call_logic)(f, rt);
                }),
            };
        }
    };
}

impl_thunk_vtable!(ThunkInlineVTable, Fn(*const ()), |f: &F, rt| f(rt));
impl_thunk_vtable!(ThunkSimpleInlineVTable, Fn(), |f: &F, _rt| f());
impl_thunk_boxed_vtable!(ThunkBoxedVTable, Fn(*const ()), |f: &F, rt| f(rt));
impl_thunk_boxed_vtable!(ThunkSimpleBoxedVTable, Fn(), |f: &F, _rt| f());

#[inline(never)]
fn thunk_value_new_internal(
    data: [usize; INLINE_WORDS],
    vtable: &'static ThunkVTable,
) -> ThunkValue {
    ThunkValue {
        vtable,
        data: MaybeUninit::new(data),
    }
}

impl ThunkValue {
    pub(crate) fn new<F: Fn(*const ()) + 'static>(f: F) -> Self {
        Self::create::<F, ThunkInlineVTable<F>, ThunkBoxedVTable<F>>(f)
    }

    pub(crate) fn new_simple<F: Fn() + 'static>(f: F) -> Self {
        Self::create::<F, ThunkSimpleInlineVTable<F>, ThunkSimpleBoxedVTable<F>>(f)
    }

    #[inline(always)]
    fn create<F, IVT, BVT>(f: F) -> Self
    where
        F: 'static,
        IVT: ThunkVTableGen,
        BVT: ThunkVTableGen,
    {
        let layout = Layout::new::<F>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut F, f) };
            thunk_value_new_internal(data, &IVT::VTABLE)
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(f);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<F>, boxed) };
            thunk_value_new_internal(data, &BVT::VTABLE)
        }
    }

    pub(crate) unsafe fn call(&self, rt: *const ()) {
        unsafe {
            (self.vtable.call.as_fn())(self.data.as_ptr() as *mut usize, rt);
        }
    }
}

impl Drop for ThunkValue {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop.as_fn())(self.data.as_mut_ptr() as *mut usize);
        }
    }
}
