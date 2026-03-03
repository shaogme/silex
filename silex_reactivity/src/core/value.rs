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
    type_id: TypeId,
    data: MaybeUninit<[usize; INLINE_WORDS]>,
}

struct AnyValueVTable {
    as_ptr: FuncPtr<unsafe fn(*const usize) -> *const ()>,
    as_mut_ptr: FuncPtr<unsafe fn(*mut usize) -> *mut ()>,
    drop: FuncPtr<unsafe fn(*mut usize)>,
    clone: Option<FuncPtr<unsafe fn(*const usize, TypeId, &'static AnyValueVTable) -> AnyValue>>,
    eq: Option<FuncPtr<unsafe fn(*const usize, *const usize) -> bool>>,
}

// Non-generic helpers to reduce bloat
#[inline(never)]
fn any_value_new_internal(
    data: [usize; INLINE_WORDS],
    vtable: &'static AnyValueVTable,
    type_id: TypeId,
) -> AnyValue {
    AnyValue {
        vtable,
        type_id,
        data: MaybeUninit::new(data),
    }
}

impl AnyValue {
    /// 创建一个普通的类型擦除值。不支持克隆和比较。
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();
        let type_id = TypeId::of::<T>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };

            // 物理原型共享：如果类型不需要 drop（如 Copy 类型），共享同一个 VTable
            if !mem::needs_drop::<T>() {
                any_value_new_internal(data, &COPY_NON_REACTIVE_VTABLE, type_id)
            } else {
                any_value_new_internal(data, &InlineVTable::<T>::VTABLE, type_id)
            }
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            any_value_new_internal(data, &BoxedVTable::<T>::VTABLE, type_id)
        }
    }

    /// 创建一个支持响应式操作（克隆、比较）的类型擦除值。
    pub(crate) fn new_reactive<T: Clone + PartialEq + 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();
        let type_id = TypeId::of::<T>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };

            // 物理原型共享：对于 Copy 且支持位比较的简单类型共享 VTable
            if !mem::needs_drop::<T>() && is_bitwise_equatable(type_id) {
                any_value_new_internal(data, &BITWISE_EQ_COPY_INLINE_VTABLE, type_id)
            } else {
                any_value_new_internal(data, &InlineReactiveVTable::<T>::VTABLE, type_id)
            }
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            any_value_new_internal(data, &BoxedReactiveVTable::<T>::VTABLE, type_id)
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Self> {
        self.vtable.clone.map(|f| unsafe {
            f.as_fn()(
                self.data.as_ptr() as *const usize,
                self.type_id,
                self.vtable,
            )
        })
    }

    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.type_id != other.type_id {
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
        if self.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = self.vtable.as_ptr.as_fn()(self.data.as_ptr() as *const usize);
                Some(&*(val_ptr as *const T))
            }
        } else {
            None
        }
    }

    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.type_id == TypeId::of::<T>() {
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

// --- Shared VTable Functions ---

unsafe fn shared_drop_noop(_: *mut usize) {}

unsafe fn shared_clone_bitwise(
    ptr: *const usize,
    type_id: TypeId,
    vtable: &'static AnyValueVTable,
) -> AnyValue {
    let mut data = [0usize; INLINE_WORDS];
    unsafe {
        ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), INLINE_WORDS);
    }
    AnyValue {
        vtable,
        type_id,
        data: MaybeUninit::new(data),
    }
}

unsafe fn shared_eq_bitwise(p1: *const usize, p2: *const usize) -> bool {
    let (s1, s2) = unsafe {
        (
            std::slice::from_raw_parts(p1, INLINE_WORDS),
            std::slice::from_raw_parts(p2, INLINE_WORDS),
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

struct InlineVTable<T>(std::marker::PhantomData<T>);
impl<T: 'static> InlineVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
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
        as_ptr: FuncPtr::new(|ptr| ptr as *const T as *const ()),
        as_mut_ptr: FuncPtr::new(|ptr| ptr as *mut T as *mut ()),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut T) }),
        clone: Some(FuncPtr::new(|ptr, type_id, vtable| {
            let mut data = [0usize; INLINE_WORDS];
            unsafe {
                let val = (*(ptr as *const T)).clone();
                ptr::write(data.as_mut_ptr() as *mut T, val);
            }
            AnyValue {
                vtable,
                type_id,
                data: MaybeUninit::new(data),
            }
        })),
        eq: Some(FuncPtr::new(|p1, p2| unsafe {
            *(p1 as *const T) == *(p2 as *const T)
        })),
    };
}

struct BoxedVTable<T>(std::marker::PhantomData<T>);
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

struct BoxedReactiveVTable<T>(std::marker::PhantomData<T>);
impl<T: Clone + PartialEq + 'static> BoxedReactiveVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        as_ptr: FuncPtr::new(|ptr| unsafe { (&**(ptr as *const Box<T>)) as *const T as *const () }),
        as_mut_ptr: FuncPtr::new(|ptr| unsafe {
            (&mut **(ptr as *mut Box<T>)) as *mut T as *mut ()
        }),
        drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<T>) }),
        clone: Some(FuncPtr::new(|ptr, type_id, vtable| {
            let mut data = [0usize; INLINE_WORDS];
            unsafe {
                let boxed = &**(ptr as *const Box<T>);
                let new_boxed = Box::new(boxed.clone());
                ptr::write(data.as_mut_ptr() as *mut Box<T>, new_boxed);
            }
            AnyValue {
                vtable,
                type_id,
                data: MaybeUninit::new(data),
            }
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

impl_thunk_vtable!(ThunkSimpleInlineVTable, Fn(), |f: &F, _rt| f());
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

    pub(crate) fn new_raw(data: [usize; INLINE_WORDS], vtable: &'static ThunkVTable) -> Self {
        thunk_value_new_internal(data, vtable)
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

// --- OnceThunk for FnOnce ---

pub(crate) struct OnceThunk {
    vtable: &'static OnceThunkVTable,
    data: MaybeUninit<[usize; INLINE_WORDS]>,
}

pub(crate) struct OnceThunkVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut usize)>,
    pub(crate) call_and_drop: FuncPtr<unsafe fn(*mut usize)>,
}

unsafe impl Sync for OnceThunkVTable {}

trait OnceThunkVTableGen {
    const VTABLE: OnceThunkVTable;
}

macro_rules! impl_once_thunk_vtable {
    ($name:ident, $call_logic:expr) => {
        struct $name<F>(std::marker::PhantomData<F>);
        impl<F: FnOnce() + 'static> OnceThunkVTableGen for $name<F> {
            const VTABLE: OnceThunkVTable = OnceThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut F) }),
                call_and_drop: FuncPtr::new(|ptr| unsafe {
                    let f = ptr::read(ptr as *mut F);
                    ($call_logic)(f);
                }),
            };
        }
    };
}

macro_rules! impl_once_thunk_boxed_vtable {
    ($name:ident, $call_logic:expr) => {
        struct $name<F>(std::marker::PhantomData<F>);
        impl<F: FnOnce() + 'static> OnceThunkVTableGen for $name<F> {
            const VTABLE: OnceThunkVTable = OnceThunkVTable {
                drop: FuncPtr::new(|ptr| unsafe { ptr::drop_in_place(ptr as *mut Box<F>) }),
                call_and_drop: FuncPtr::new(|ptr| unsafe {
                    let f = ptr::read(ptr as *mut Box<F>);
                    ($call_logic)(*f);
                }),
            };
        }
    };
}

impl_once_thunk_vtable!(OnceThunkSimpleInlineVTable, |f: F| f());
impl_once_thunk_boxed_vtable!(OnceThunkSimpleBoxedVTable, |f: F| f());

#[inline(never)]
fn once_thunk_new_internal(
    data: [usize; INLINE_WORDS],
    vtable: &'static OnceThunkVTable,
) -> OnceThunk {
    OnceThunk {
        vtable,
        data: MaybeUninit::new(data),
    }
}

impl OnceThunk {
    pub(crate) fn new<F: FnOnce() + 'static>(f: F) -> Self {
        let layout = Layout::new::<F>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            let mut data = [0usize; INLINE_WORDS];
            unsafe { ptr::write(data.as_mut_ptr() as *mut F, f) };
            once_thunk_new_internal(data, &OnceThunkSimpleInlineVTable::<F>::VTABLE)
        } else {
            let mut data = [0usize; INLINE_WORDS];
            let boxed = Box::new(f);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<F>, boxed) };
            once_thunk_new_internal(data, &OnceThunkSimpleBoxedVTable::<F>::VTABLE)
        }
    }

    pub(crate) fn call(mut self) {
        let vtable = self.vtable;
        let data_ptr = self.data.as_mut_ptr() as *mut usize;
        // 这一步很重要：手动标识已“调用”，防止 Drop 再跑一遍
        mem::forget(self);
        unsafe {
            (vtable.call_and_drop.as_fn())(data_ptr);
        }
    }
}

impl Drop for OnceThunk {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop.as_fn())(self.data.as_mut_ptr() as *mut usize);
        }
    }
}
