use crate::any_box::AnyBox;
use crate::func_ptr::FuncPtr;
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::mem;
use core::ptr;

// --- FnBox: Type-erased Fn() ---

pub struct FnBoxVTable {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*const u8)>,
}

pub struct FnBox {
    inner: AnyBox<FnBoxVTable>,
}

impl FnBox {
    pub fn new<F: Fn() + 'static>(f: F) -> Self {
        struct VGen<F>(PhantomData<F>);
        impl<F: Fn() + 'static> VGen<F> {
            const STACK: FnBoxVTable = FnBoxVTable {
                drop: FuncPtr::new(drop_stack::<F>),
                call: FuncPtr::new(call_stack::<F>),
            };
            const HEAP: FnBoxVTable = FnBoxVTable {
                drop: FuncPtr::new(drop_heap::<F>),
                call: FuncPtr::new(call_heap::<F>),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F>::STACK, &VGen::<F>::HEAP),
        }
    }

    #[inline(always)]
    pub fn call(&self) {
        unsafe {
            (self.inner.vtable.call.as_fn())(self.inner.as_ptr());
        }
    }
}

impl Drop for FnBox {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- OnceBox: Type-erased FnOnce() ---

pub struct OnceBoxVTable {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*mut u8)>,
}

pub struct OnceBox {
    inner: AnyBox<OnceBoxVTable>,
}

impl OnceBox {
    pub fn new<F: FnOnce() + 'static>(f: F) -> Self {
        struct VGen<F>(PhantomData<F>);
        impl<F: FnOnce() + 'static> VGen<F> {
            const STACK: OnceBoxVTable = OnceBoxVTable {
                drop: FuncPtr::new(drop_stack::<F>),
                call: FuncPtr::new(call_once_stack::<F>),
            };
            const HEAP: OnceBoxVTable = OnceBoxVTable {
                drop: FuncPtr::new(drop_heap::<F>),
                call: FuncPtr::new(call_once_heap::<F>),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F>::STACK, &VGen::<F>::HEAP),
        }
    }

    #[inline(always)]
    pub fn call(self) {
        let mut this = mem::ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe {
            (vtable.call.as_fn())(data_ptr);
        }
    }
}

impl Drop for OnceBox {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- FactoryBox<R>: Type-erased Fn() -> R ---

pub struct FactoryBoxVTable<R: 'static> {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*const u8) -> R>,
}

pub struct FactoryBox<R: 'static> {
    inner: AnyBox<FactoryBoxVTable<R>>,
}

impl<R: 'static> FactoryBox<R> {
    pub fn new<F: Fn() -> R + 'static>(f: F) -> Self {
        struct VGen<F, R>(PhantomData<(F, R)>);
        impl<F: Fn() -> R + 'static, R: 'static> VGen<F, R> {
            const STACK: FactoryBoxVTable<R> = FactoryBoxVTable {
                drop: FuncPtr::new(drop_stack::<F>),
                call: FuncPtr::new(call_factory_stack::<F, R>),
            };
            const HEAP: FactoryBoxVTable<R> = FactoryBoxVTable {
                drop: FuncPtr::new(drop_heap::<F>),
                call: FuncPtr::new(call_factory_heap::<F, R>),
            };
        }

        Self {
            inner: AnyBox::new(f, &VGen::<F, R>::STACK, &VGen::<F, R>::HEAP),
        }
    }

    #[inline(always)]
    pub fn call(&self) -> R {
        unsafe { (self.inner.vtable.call.as_fn())(self.inner.as_ptr()) }
    }
}

impl<R> Drop for FactoryBox<R> {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
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

unsafe fn call_stack<F: Fn()>(data: *const u8) {
    unsafe {
        let f = &*(data as *const F);
        f();
    }
}

unsafe fn call_heap<F: Fn()>(data: *const u8) {
    unsafe {
        let ptr = *(data as *const *mut F);
        let f = &*ptr;
        f();
    }
}

unsafe fn call_once_stack<F: FnOnce()>(data: *mut u8) {
    unsafe {
        let f = ptr::read(data as *mut F);
        f();
    }
}

unsafe fn call_once_heap<F: FnOnce()>(data: *mut u8) {
    unsafe {
        let ptr = ptr::read(data as *mut *mut F);
        let f = *Box::from_raw(ptr);
        f();
    }
}

unsafe fn call_factory_stack<F: Fn() -> R, R>(data: *const u8) -> R {
    unsafe {
        let f = &*(data as *const F);
        f()
    }
}

unsafe fn call_factory_heap<F: Fn() -> R, R>(data: *const u8) -> R {
    unsafe {
        let ptr = *(data as *const *mut F);
        let f = &*ptr;
        f()
    }
}
