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

impl AnyValue {
    /// 创建一个普通的类型擦除值。不支持克隆和比较。
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        let mut data = MaybeUninit::<[usize; INLINE_WORDS]>::uninit();
        let vtable: &'static AnyValueVTable = if fits_inline {
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };
            &InlineVTable::<T>::VTABLE
        } else {
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            &BoxedVTable::<T>::VTABLE
        };

        AnyValue { vtable, data }
    }

    /// 创建一个支持响应式操作（克隆、比较）的类型擦除值。
    pub(crate) fn new_reactive<T: Clone + PartialEq + 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        let mut data = MaybeUninit::<[usize; INLINE_WORDS]>::uninit();
        let vtable: &'static AnyValueVTable = if fits_inline {
            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };
            &InlineReactiveVTable::<T>::VTABLE
        } else {
            let boxed = Box::new(value);
            unsafe { ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed) };
            &BoxedReactiveVTable::<T>::VTABLE
        };

        AnyValue { vtable, data }
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
