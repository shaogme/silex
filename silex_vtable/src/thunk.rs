use crate::any_box::AnyBox;
use crate::func_ptr::FuncPtr;
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::mem;
use core::ptr;

// --- ThunkBox<Args, R>: Generic type-erased Fn(Args) -> R ---

pub struct ThunkBoxVTable<Args: 'static, R: 'static> {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*const u8, Args) -> R>,
}

unsafe impl<Args: 'static, R: 'static> Sync for ThunkBoxVTable<Args, R> {}

pub struct ThunkBox<Args: 'static, R: 'static> {
    inner: AnyBox<ThunkBoxVTable<Args, R>>,
}

impl<Args: 'static, R: 'static> ThunkBox<Args, R> {
    pub fn new<F: Fn(Args) -> R + 'static>(f: F) -> Self {
        struct VGen<F, Args, R>(PhantomData<(F, Args, R)>);
        impl<F: Fn(Args) -> R + 'static, Args: 'static, R: 'static> VGen<F, Args, R> {
            const STACK: ThunkBoxVTable<Args, R> = ThunkBoxVTable {
                drop: FuncPtr::new(drop_stack::<F>),
                call: FuncPtr::new(call_thunk_stack::<F, Args, R>),
            };
            const HEAP: ThunkBoxVTable<Args, R> = ThunkBoxVTable {
                drop: FuncPtr::new(drop_heap::<F>),
                call: FuncPtr::new(call_thunk_heap::<F, Args, R>),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F, Args, R>::STACK, &VGen::<F, Args, R>::HEAP),
        }
    }

    pub fn from_raw(data: [usize; 3], vtable: &'static ThunkBoxVTable<Args, R>) -> Self {
        Self {
            inner: AnyBox { data, vtable },
        }
    }

    #[inline(always)]
    pub fn call(&self, args: Args) -> R {
        unsafe { (self.inner.vtable.call.as_fn())(self.inner.as_ptr(), args) }
    }
}

impl<Args: 'static, R: 'static> Drop for ThunkBox<Args, R> {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- OnceBox<Args, R>: Generic type-erased FnOnce(Args) -> R ---

pub struct OnceBoxVTable<Args: 'static, R: 'static> {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*mut u8, Args) -> R>,
}

unsafe impl<Args: 'static, R: 'static> Sync for OnceBoxVTable<Args, R> {}

pub struct OnceBox<Args: 'static, R: 'static> {
    inner: AnyBox<OnceBoxVTable<Args, R>>,
}

impl<Args: 'static, R: 'static> OnceBox<Args, R> {
    pub fn new<F: FnOnce(Args) -> R + 'static>(f: F) -> Self {
        struct VGen<F, Args, R>(PhantomData<(F, Args, R)>);
        impl<F: FnOnce(Args) -> R + 'static, Args: 'static, R: 'static> VGen<F, Args, R> {
            const STACK: OnceBoxVTable<Args, R> = OnceBoxVTable {
                drop: FuncPtr::new(drop_stack::<F>),
                call: FuncPtr::new(call_once_thunk_stack::<F, Args, R>),
            };
            const HEAP: OnceBoxVTable<Args, R> = OnceBoxVTable {
                drop: FuncPtr::new(drop_heap::<F>),
                call: FuncPtr::new(call_once_thunk_heap::<F, Args, R>),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F, Args, R>::STACK, &VGen::<F, Args, R>::HEAP),
        }
    }

    #[inline(always)]
    pub fn call(self, args: Args) -> R {
        let mut this = mem::ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe { (vtable.call.as_fn())(data_ptr, args) }
    }
}

impl<Args: 'static, R: 'static> Drop for OnceBox<Args, R> {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- Aliases & Wrappers ---

pub struct FnBox(ThunkBox<(), ()>);

impl FnBox {
    pub fn new<F: Fn() + 'static>(f: F) -> Self {
        Self(ThunkBox::new(move |_| f()))
    }

    #[inline(always)]
    pub fn call(&self) {
        self.0.call(());
    }
}

pub struct FactoryBox<R: 'static>(ThunkBox<(), R>);

impl<R: 'static> FactoryBox<R> {
    pub fn new<F: Fn() -> R + 'static>(f: F) -> Self {
        Self(ThunkBox::new(move |_| f()))
    }

    #[inline(always)]
    pub fn call(&self) -> R {
        self.0.call(())
    }
}

// --- Glue Functions ---

unsafe fn drop_stack<T>(data: *mut u8) {
    unsafe {
        ptr::drop_in_place(data as *mut T);
    }
}

unsafe fn drop_heap<T>(data: *mut u8) {
    unsafe {
        let ptr = ptr::read(data as *mut *mut T);
        let _ = Box::from_raw(ptr);
    }
}

unsafe fn call_thunk_stack<F: Fn(Args) -> R, Args, R>(data: *const u8, args: Args) -> R {
    unsafe {
        let f = &*(data as *const F);
        f(args)
    }
}

unsafe fn call_thunk_heap<F: Fn(Args) -> R, Args, R>(data: *const u8, args: Args) -> R {
    unsafe {
        let ptr = *(data as *const *mut F);
        let f = &*ptr;
        f(args)
    }
}

unsafe fn call_once_thunk_stack<F: FnOnce(Args) -> R, Args, R>(data: *mut u8, args: Args) -> R {
    unsafe {
        let f = ptr::read(data as *mut F);
        f(args)
    }
}

unsafe fn call_once_thunk_heap<F: FnOnce(Args) -> R, Args, R>(data: *mut u8, args: Args) -> R {
    unsafe {
        let ptr = ptr::read(data as *mut *mut F);
        let f = *Box::from_raw(ptr);
        f(args)
    }
}
