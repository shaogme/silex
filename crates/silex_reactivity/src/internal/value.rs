//! Safe type-erased values and scoped computation payloads.
//!
//! Values are optimized with `InlineStorage` (SOO) for small payloads (<=24 bytes),
//! eliminating heap allocations for standard reactive types (`i32`, `bool`, `f64`, etc.).

use silex_vtable::{any_box::InlineStorage, func_ptr::FuncPtr};
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    ptr,
};

pub(crate) struct AnyValueVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
    pub(crate) type_id: FuncPtr<fn() -> TypeId>,
    pub(crate) as_ptr: FuncPtr<unsafe fn(*const u8) -> *const u8>,
    pub(crate) as_mut_ptr: FuncPtr<unsafe fn(*mut u8) -> *mut u8>,
    pub(crate) equals: FuncPtr<unsafe fn(*const u8, *const u8) -> bool>,
}

unsafe impl Sync for AnyValueVTable {}
unsafe impl Send for AnyValueVTable {}

struct VGen<T>(PhantomData<T>);

impl<T: 'static> VGen<T> {
    const STACK: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_stack::<T>),
        type_id: FuncPtr::new(type_id::<T>),
        as_ptr: FuncPtr::new(as_ptr_stack::<T>),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_stack::<T>),
        equals: FuncPtr::new(equals_none),
    };

    const HEAP: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_heap::<T>),
        type_id: FuncPtr::new(type_id::<T>),
        as_ptr: FuncPtr::new(as_ptr_heap::<T>),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_heap::<T>),
        equals: FuncPtr::new(equals_none),
    };
}

struct VGenEq<T>(PhantomData<T>);

impl<T: PartialEq + 'static> VGenEq<T> {
    const STACK: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_stack::<T>),
        type_id: FuncPtr::new(type_id::<T>),
        as_ptr: FuncPtr::new(as_ptr_stack::<T>),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_stack::<T>),
        equals: FuncPtr::new(equals_typed::<T>),
    };

    const HEAP: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_heap::<T>),
        type_id: FuncPtr::new(type_id::<T>),
        as_ptr: FuncPtr::new(as_ptr_heap::<T>),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_heap::<T>),
        equals: FuncPtr::new(equals_typed::<T>),
    };
}

/// # Safety
///
/// `data` 必须指向在偏移 0 处存有有效 `T` 实例的 `InlineStorage` 内存，且该实例未被提前 Drop。
unsafe fn drop_stack<T>(data: *mut u8) {
    // SAFETY: data 指向内联存储在 InlineStorage 内部的有效 T 实例，能够安全执行 drop_in_place。
    unsafe {
        ptr::drop_in_place(data as *mut T);
    }
}

/// # Safety
///
/// `data` 必须指向在偏移 0 处存有由 `Box::into_raw` 分配的有效 `*mut T` 指针的 `InlineStorage` 内存。
unsafe fn drop_heap<T>(data: *mut u8) {
    // SAFETY: data 偏移 0 处存有由 Box::into_raw 分配在堆上的 *mut T 指针，read 读取后再通过 Box::from_raw 释放堆内存。
    unsafe {
        let heap_ptr = ptr::read(data as *mut *mut T);
        let _ = Box::from_raw(heap_ptr);
    }
}

fn type_id<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}

/// # Safety
///
/// `_data` 必须指向存有有效 `T` 内联实例的 `InlineStorage` 内存。
unsafe fn as_ptr_stack<T>(_data: *const u8) -> *const u8 {
    _data
}

/// # Safety
///
/// `_data` 必须指向存有有效 `T` 内联实例的 `InlineStorage` 内存。
unsafe fn as_mut_ptr_stack<T>(_data: *mut u8) -> *mut u8 {
    _data
}

/// # Safety
///
/// `data` 必须指向在偏移 0 处存有由 `Box::into_raw` 分配的有效 `*mut T` 指针的内存。
unsafe fn as_ptr_heap<T>(data: *const u8) -> *const u8 {
    // SAFETY: data 指向 InlineStorage 内部存储的 *mut T 裸指针，读取后转换为 *const u8 表达式数据首地址。
    unsafe {
        let heap_ptr = ptr::read(data as *const *mut T);
        heap_ptr as *const u8
    }
}

/// # Safety
///
/// `data` 必须指向在偏移 0 处存有由 `Box::into_raw` 分配的有效 `*mut T` 指针的内存。
unsafe fn as_mut_ptr_heap<T>(data: *mut u8) -> *mut u8 {
    // SAFETY: data 指向 InlineStorage 内部存储的 *mut T 裸指针，读取后转换为 *mut u8 表达式数据可变首地址。
    unsafe {
        let heap_ptr = ptr::read(data as *const *mut T);
        heap_ptr as *mut u8
    }
}

/// # Safety
///
/// 擦除比对的空实现，调用时指针参数必须合法。
unsafe fn equals_none(_data1: *const u8, _data2: *const u8) -> bool {
    false
}

/// # Safety
///
/// `data1` 与 `data2` 必须为已通过 `as_ptr` 解出的指向类型 `T` 有效实例的合法指针。
unsafe fn equals_typed<T: PartialEq>(data1: *const u8, data2: *const u8) -> bool {
    // SAFETY: data1 与 data2 为由同类型 vtable.as_ptr 提取的合法 *const T 实例指针，安全解引用比对。
    unsafe {
        let val1 = &*(data1 as *const T);
        let val2 = &*(data2 as *const T);
        val1 == val2
    }
}


pub(crate) struct AnyValue {
    data: InlineStorage,
    vtable: &'static AnyValueVTable,
}

impl AnyValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let fits_inline = InlineStorage::fits::<T>(0);
        let mut data = InlineStorage::zeroed();
        if fits_inline {
            // SAFETY: 已验证 T 满足 size <= 24 且 align <= align_of::<usize>()，在 0 偏移处写入安全。
            unsafe { data.write(0, value) };
            Self {
                data,
                vtable: &VGen::<T>::STACK,
            }
        } else {
            // SAFETY: 写入 Box::into_raw 分配的指针（占据 1 个 usize），InlineStorage 必可容纳。
            unsafe { data.write(0, Box::into_raw(Box::new(value))) };
            Self {
                data,
                vtable: &VGen::<T>::HEAP,
            }
        }
    }

    pub(crate) fn new_reactive<T: PartialEq + 'static>(value: T) -> Self {
        let fits_inline = InlineStorage::fits::<T>(0);
        let mut data = InlineStorage::zeroed();
        if fits_inline {
            // SAFETY: 已验证 T 满足 size <= 24 且 align <= align_of::<usize>()，在 0 偏移处写入安全。
            unsafe { data.write(0, value) };
            Self {
                data,
                vtable: &VGenEq::<T>::STACK,
            }
        } else {
            // SAFETY: 写入 Box::into_raw 分配的指针（占据 1 个 usize），InlineStorage 必可容纳。
            unsafe { data.write(0, Box::into_raw(Box::new(value))) };
            Self {
                data,
                vtable: &VGenEq::<T>::HEAP,
            }
        }
    }

    #[inline(always)]
    pub(crate) fn value_type_id(&self) -> TypeId {
        (self.vtable.type_id.as_fn())()
    }

    #[inline(always)]
    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if self.value_type_id() != other.value_type_id() {
            return false;
        }
        // SAFETY: self 与 other 均保存合法数据且 TypeId 完全一致，可以提取指针并调用底层 equals。
        unsafe {
            let ptr1 = (self.vtable.as_ptr.as_fn())(self.data.as_ptr());
            let ptr2 = (other.vtable.as_ptr.as_fn())(other.data.as_ptr());
            (self.vtable.equals.as_fn())(ptr1, ptr2)
        }
    }

    #[inline(always)]
    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.value_type_id() == TypeId::of::<T>() {
            // SAFETY: TypeId 匹配保证底层存储确为 T 类型，vtable.as_ptr 返回有效的 *const T 指针。
            unsafe {
                let ptr = (self.vtable.as_ptr.as_fn())(self.data.as_ptr());
                Some(&*(ptr as *const T))
            }
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.value_type_id() == TypeId::of::<T>() {
            // SAFETY: TypeId 匹配保证底层存储确为 T 类型，vtable.as_mut_ptr 返回有效的 *mut T 指针。
            unsafe {
                let ptr = (self.vtable.as_mut_ptr.as_fn())(self.data.as_mut_ptr());
                Some(&mut *(ptr as *mut T))
            }
        } else {
            None
        }
    }
}

impl Drop for AnyValue {
    fn drop(&mut self) {
        // SAFETY: vtable.drop 指向类型特定且存储路径相符的析构函数（STACK 为 drop_in_place，HEAP 为 Box 释放）。
        unsafe {
            (self.vtable.drop.as_fn())(self.data.as_mut_ptr());
        }
    }
}



pub(crate) enum Computation<'scope> {
    Effect(EffectThunk<'scope>),
    Memo(MemoThunk<'scope>),
}

pub(crate) struct EffectThunk<'scope> {
    callback: Box<dyn FnMut() + 'scope>,
}

impl<'scope> EffectThunk<'scope> {
    pub(crate) fn new<F>(callback: F) -> Self
    where
        F: FnMut() + 'scope,
    {
        Self {
            callback: Box::new(callback),
        }
    }

    /// # Safety
    ///
    /// 调用者必须保证此 `EffectThunk` 仅存储在生命周期为 `'scope` 的 `ScopeFrame` 对应的 `ScopeState` 中。
    /// 当 `'scope` 作用域退出时，`ScopeFrame::dispose()` 会被调用并 Drop 内部所有的 `EffectThunk`，
    /// 从而保证闭包绝对不会在 `'scope` 作用域之外被访问或调用。
    pub(crate) unsafe fn extend_lifetime<'run>(self) -> EffectThunk<'run> {
        // SAFETY: 调用者保证 self 的真正生存期由 ScopeFrame 控制，直接 transmute 更改泛型参数生命周期。
        unsafe { std::mem::transmute(self) }
    }

    pub(crate) fn call(&mut self) {
        (self.callback)();
    }
}

pub(crate) struct MemoThunk<'scope> {
    callback: MemoCallback<'scope>,
}

type MemoCallback<'scope> = Box<dyn FnMut(Option<&AnyValue>) -> AnyValue + 'scope>;

impl<'scope> MemoThunk<'scope> {
    pub(crate) fn new<T, F>(callback: F) -> Self
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move |old| {
                let old = old.and_then(|value| value.downcast_ref::<T>());
                AnyValue::new_reactive(callback(old))
            }),
        }
    }

    pub(crate) fn new_derived<T, F>(callback: F) -> Self
    where
        T: 'static,
        F: FnMut() -> T + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move |_| AnyValue::new(callback())),
        }
    }

    /// # Safety
    ///
    /// 调用者必须保证此 `MemoThunk` 仅存储在生命周期为 `'scope` 的 `ScopeFrame` 对应的 `ScopeState` 中。
    /// 当 `'scope` 作用域退出时，`ScopeFrame::dispose()` 会被调用并 Drop 内部所有的 `MemoThunk`，
    /// 从而保证闭包绝对不会在 `'scope` 作用域之外被访问或调用。
    pub(crate) unsafe fn extend_lifetime<'run>(self) -> MemoThunk<'run> {
        // SAFETY: 调用者保证 self 的真正生存期由 ScopeFrame 控制，直接 transmute 更改泛型参数生命周期。
        unsafe { std::mem::transmute(self) }
    }

    pub(crate) fn compute(&mut self, old: Option<&AnyValue>) -> AnyValue {
        (self.callback)(old)
    }
}

pub(crate) struct OnceThunk<'scope> {
    callback: Option<Box<dyn FnOnce() + 'scope>>,
}

impl<'scope> OnceThunk<'scope> {
    pub(crate) fn new<F>(callback: F) -> Self
    where
        F: FnOnce() + 'scope,
    {
        Self {
            callback: Some(Box::new(callback)),
        }
    }

    /// # Safety
    ///
    /// 调用者必须保证此 `OnceThunk` 仅存储在生命周期为 `'scope` 的 `ScopeFrame` 对应的 `ScopeState` 中。
    /// 当 `'scope` 作用域退出时，`ScopeFrame::dispose()` 会被调用并 Drop 内部所有的 `OnceThunk`，
    /// 从而保证闭包绝对不会在 `'scope` 作用域之外被访问或调用。
    pub(crate) unsafe fn extend_lifetime<'run>(self) -> OnceThunk<'run> {
        // SAFETY: 调用者保证 self 的真正生存期由 ScopeFrame 控制，直接 transmute 更改泛型参数生命周期。
        unsafe { std::mem::transmute(self) }
    }

    pub(crate) fn call(mut self) {
        if let Some(callback) = self.callback.take() {
            callback();
        }
    }
}

pub(crate) struct CallbackThunk<'scope> {
    callback: Box<dyn FnMut(Box<dyn Any>) + 'scope>,
}

impl<'scope> CallbackThunk<'scope> {
    pub(crate) fn new<F>(callback: F) -> Self
    where
        F: FnMut(Box<dyn Any>) + 'scope,
    {
        Self {
            callback: Box::new(callback),
        }
    }

    /// # Safety
    ///
    /// 调用者必须保证此 `CallbackThunk` 仅存储在生命周期为 `'scope` 的 `ScopeFrame` 对应的 `ScopeState` 中。
    /// 当 `'scope` 作用域退出时，`ScopeFrame::dispose()` 会被调用并 Drop 内部所有的 `CallbackThunk`，
    /// 从而保证闭包绝对不会在 `'scope` 作用域之外被访问或调用。
    pub(crate) unsafe fn extend_lifetime<'run>(self) -> CallbackThunk<'run> {
        // SAFETY: 调用者保证 self 的真正生存期由 ScopeFrame 控制，直接 transmute 更改泛型参数生命周期。
        unsafe { std::mem::transmute(self) }
    }

    pub(crate) fn call(&mut self, arg: Box<dyn Any>) {
        (self.callback)(arg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_value_try_eq() {
        // 1. Stack 路径 PartialEq
        let v1 = AnyValue::new_reactive(10i32);
        let v2 = AnyValue::new_reactive(10i32);
        let v3 = AnyValue::new_reactive(20i32);

        assert!(v1.try_eq(&v2));
        assert!(!v1.try_eq(&v3));

        // 2. Heap 路径 PartialEq ([u8; 32])
        let h1 = AnyValue::new_reactive([1u8; 32]);
        let h2 = AnyValue::new_reactive([1u8; 32]);
        let h3 = AnyValue::new_reactive([2u8; 32]);

        assert!(h1.try_eq(&h2));
        assert!(!h1.try_eq(&h3));

        // 3. 不同类型比对
        assert!(!v1.try_eq(&h1));

        // 4. 非 PartialEq 类型 (new 构造)
        #[allow(dead_code)]
        struct NonEq(i32);

        let n1 = AnyValue::new(NonEq(10));
        let n2 = AnyValue::new(NonEq(10));
        assert!(!n1.try_eq(&n2));
    }

    #[test]
    fn test_any_value_downcast_mut() {
        let mut v = AnyValue::new(10i32);
        if let Some(val) = v.downcast_mut::<i32>() {
            *val = 42;
        }
        assert_eq!(v.downcast_ref::<i32>(), Some(&42));
        assert_eq!(v.downcast_ref::<u32>(), None);
    }
}

