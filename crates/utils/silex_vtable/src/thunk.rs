use crate::{
    any_box::{AnyBox, InlineStorage},
    func_ptr::FuncPtr,
};
use alloc::boxed::Box;
use core::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ptr::{self, drop_in_place},
};

// --- ThunkBox<Args, R>: Generic type-erased Fn(Args) -> R ---

pub struct ThunkBoxVTable<Args, R> {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*const u8, Args) -> R>,
}

unsafe impl<Args, R> Sync for ThunkBoxVTable<Args, R> {}

pub struct ThunkBox<'a, Args, R> {
    inner: AnyBox<'a, ThunkBoxVTable<Args, R>>,
}

impl<'a, Args: 'a, R: 'a> ThunkBox<'a, Args, R> {
    pub fn new<F: Fn(Args) -> R + 'a>(f: F) -> Self {
        struct VGen<F, Args, R>(PhantomData<(F, Args, R)>);
        impl<F: Fn(Args) -> R, Args, R> VGen<F, Args, R> {
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

    /// 从已经手工填好的内联缓冲区构造。
    ///
    /// # Safety
    ///
    /// - `data` 必须使用 `vtable` 约定的确切布局初始化：内联分支存放完整的
    ///   闭包值，堆分支在偏移零处存放对应闭包的有效原始指针。
    /// - `vtable` 的 `drop`/`call` 函数必须针对同一个闭包类型以及完全匹配的
    ///   `Args`/`R` 实例化，不能只因为函数签名相同就复用其他闭包的 vtable。
    /// - `vtable` 地址必须在 `'a` 内有效，且 `data` 的所有权（包括析构责任）
    ///   由此 `ThunkBox` 接管；调用方不得通过 lifetime `transmute` 延长闭包或
    ///   参数载荷。
    pub unsafe fn from_raw(data: InlineStorage, vtable: &'a ThunkBoxVTable<Args, R>) -> Self {
        Self {
            inner: AnyBox {
                data,
                vtable,
                marker: PhantomData,
            },
        }
    }

    #[inline(always)]
    pub fn call(&self, args: Args) -> R {
        unsafe { (self.inner.vtable.call.as_fn())(self.inner.as_ptr(), args) }
    }
}

impl<'a, Args, R> Drop for ThunkBox<'a, Args, R> {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- OnceBox<Args, R>: Generic type-erased FnOnce(Args) -> R ---

pub struct OnceBoxVTable<Args, R> {
    pub drop: FuncPtr<unsafe fn(*mut u8)>,
    pub call: FuncPtr<unsafe fn(*mut u8, Args) -> R>,
}

unsafe impl<Args, R> Sync for OnceBoxVTable<Args, R> {}

pub struct OnceBox<'a, Args, R> {
    inner: AnyBox<'a, OnceBoxVTable<Args, R>>,
}

impl<'a, Args: 'a, R: 'a> OnceBox<'a, Args, R> {
    pub fn new<F: FnOnce(Args) -> R + 'a>(f: F) -> Self {
        struct VGen<F, Args, R>(PhantomData<(F, Args, R)>);
        impl<F: FnOnce(Args) -> R, Args, R> VGen<F, Args, R> {
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
        let mut this = ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe { (vtable.call.as_fn())(data_ptr, args) }
    }
}

impl<'a, Args, R> Drop for OnceBox<'a, Args, R> {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- Aliases & Wrappers ---

pub struct FnBox<'a>(ThunkBox<'a, (), ()>);

impl<'a> FnBox<'a> {
    pub fn new<F: Fn() + 'a>(f: F) -> Self {
        Self(ThunkBox::new(move |_| f()))
    }

    #[inline(always)]
    pub fn call(&self) {
        self.0.call(());
    }
}

pub struct FactoryBox<'a, R: 'a>(ThunkBox<'a, (), R>);

impl<'a, R: 'a> FactoryBox<'a, R> {
    pub fn new<F: Fn() -> R + 'a>(f: F) -> Self {
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
        drop_in_place(data as *mut T);
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
