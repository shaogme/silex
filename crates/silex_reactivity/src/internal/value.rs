//! Safe type-erased values and scoped computation payloads.
//!
//! Values are optimized with `InlineStorage` (SOO) for small payloads (<=24 bytes),
//! eliminating heap allocations for standard reactive types (`i32`, `bool`, `f64`, etc.).

use crate::{
    ReactiveError,
    error::{ErrorEvent, ErrorHandler, InitialErrorSlot},
};
use silex_vtable::{any_box::InlineStorage, func_ptr::FuncPtr};
use std::{marker::PhantomData, ptr};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TypeIdToken {
    name: &'static str,
}

fn type_id_token<T: ?Sized>() -> TypeIdToken {
    TypeIdToken {
        name: std::any::type_name::<T>(),
    }
}

impl PartialEq for TypeIdToken {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.name.as_ptr(), other.name.as_ptr())
            && self.name.len() == other.name.len()
            && self.name == other.name
    }
}

impl Eq for TypeIdToken {}

pub(crate) struct AnyValueVTable {
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
    pub(crate) type_id: FuncPtr<fn() -> TypeIdToken>,
    pub(crate) as_ptr: FuncPtr<unsafe fn(*const u8) -> *const u8>,
    pub(crate) as_mut_ptr: FuncPtr<unsafe fn(*mut u8) -> *mut u8>,
    pub(crate) equals: FuncPtr<unsafe fn(*const u8, *const u8) -> bool>,
}

unsafe impl Sync for AnyValueVTable {}
unsafe impl Send for AnyValueVTable {}

struct VGen<T: ?Sized>(PhantomData<fn() -> T>);

impl<T> VGen<T> {
    const STACK: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_stack::<T>),
        type_id: FuncPtr::new(type_id_token::<T>),
        as_ptr: FuncPtr::new(as_ptr_stack),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_stack),
        equals: FuncPtr::new(equals_none),
    };

    const HEAP: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_heap::<T>),
        type_id: FuncPtr::new(type_id_token::<T>),
        as_ptr: FuncPtr::new(as_ptr_heap::<T>),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_heap::<T>),
        equals: FuncPtr::new(equals_none),
    };
}

struct VGenEq<T: ?Sized>(PhantomData<fn() -> T>);

impl<T: PartialEq> VGenEq<T> {
    const STACK: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_stack::<T>),
        type_id: FuncPtr::new(type_id_token::<T>),
        as_ptr: FuncPtr::new(as_ptr_stack),
        as_mut_ptr: FuncPtr::new(as_mut_ptr_stack),
        equals: FuncPtr::new(equals_typed::<T>),
    };

    const HEAP: AnyValueVTable = AnyValueVTable {
        drop: FuncPtr::new(drop_heap::<T>),
        type_id: FuncPtr::new(type_id_token::<T>),
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

/// # Safety
///
/// `data` 必须指向存有有效内联实例的 `InlineStorage` 内存。
unsafe fn as_ptr_stack(data: *const u8) -> *const u8 {
    data
}

/// # Safety
///
/// `data` 必须指向存有有效内联实例的 `InlineStorage` 内存。
unsafe fn as_mut_ptr_stack(data: *mut u8) -> *mut u8 {
    data
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

pub(crate) struct AnyValue<'a> {
    data: InlineStorage,
    vtable: &'static AnyValueVTable,
    is_heap: bool,
    _marker: PhantomData<*mut &'a ()>,
}

impl<'a> AnyValue<'a> {
    pub(crate) fn new<T: 'a>(value: T) -> Self {
        let fits_inline = InlineStorage::fits::<T>(0);
        let mut data = InlineStorage::zeroed();
        if fits_inline {
            // SAFETY: 已验证 T 满足 size <= 24 且 align <= align_of::<usize>()，在 0 偏移处写入安全。
            unsafe { data.write(0, value) };
            Self {
                data,
                vtable: &VGen::<T>::STACK,
                is_heap: false,
                _marker: PhantomData,
            }
        } else {
            // SAFETY: 写入 Box::into_raw 分配的指针（占据 1 个 usize），InlineStorage 必可容纳。
            unsafe { data.write(0, Box::into_raw(Box::new(value))) };
            Self {
                data,
                vtable: &VGen::<T>::HEAP,
                is_heap: true,
                _marker: PhantomData,
            }
        }
    }

    pub(crate) fn new_reactive<T: PartialEq + 'a>(value: T) -> Self {
        let fits_inline = InlineStorage::fits::<T>(0);
        let mut data = InlineStorage::zeroed();
        if fits_inline {
            // SAFETY: 已验证 T 满足 size <= 24 且 align <= align_of::<usize>()，在 0 偏移处写入安全。
            unsafe { data.write(0, value) };
            Self {
                data,
                vtable: &VGenEq::<T>::STACK,
                is_heap: false,
                _marker: PhantomData,
            }
        } else {
            // SAFETY: 写入 Box::into_raw 分配的指针（占据 1 个 usize），InlineStorage 必可容纳。
            unsafe { data.write(0, Box::into_raw(Box::new(value))) };
            Self {
                data,
                vtable: &VGenEq::<T>::HEAP,
                is_heap: true,
                _marker: PhantomData,
            }
        }
    }

    #[inline(always)]
    pub(crate) fn value_type_id(&self) -> TypeIdToken {
        (self.vtable.type_id.as_fn())()
    }

    #[inline(always)]
    pub(crate) fn try_eq(&self, other: &Self) -> bool {
        if !std::ptr::eq(self.vtable, other.vtable) || self.value_type_id() != other.value_type_id()
        {
            return false;
        }
        // SAFETY: self 与 other 使用同一 vtable 且保存合法数据，可以提取指针并调用底层 equals。
        unsafe {
            let ptr1 = (self.vtable.as_ptr.as_fn())(self.data.as_ptr());
            let ptr2 = (other.vtable.as_ptr.as_fn())(other.data.as_ptr());
            (self.vtable.equals.as_fn())(ptr1, ptr2)
        }
    }

    #[inline(always)]
    /// Downcast to a value whose complete type, including lifetimes, is known
    /// to match the value used to construct this container.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `T` is exactly the erased value type. The
    /// runtime type token intentionally cannot encode lifetimes, so matching a
    /// type constructor with a different lifetime is not sufficient.
    pub(crate) unsafe fn downcast_ref<T>(&self) -> Option<&T> {
        if self.value_type_id() == type_id_token::<T>() {
            // SAFETY: 调用者保证 exact-type 合约，vtable.as_ptr 返回有效的 *const T 指针。
            unsafe {
                let ptr = (self.vtable.as_ptr.as_fn())(self.data.as_ptr());
                Some(&*(ptr as *const T))
            }
        } else {
            None
        }
    }

    #[inline(always)]
    /// Downcast to a mutable value with the exact erased type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `T` is exactly the erased value type,
    /// including all lifetime parameters.
    pub(crate) unsafe fn downcast_mut<T>(&mut self) -> Option<&mut T> {
        if self.value_type_id() == type_id_token::<T>() {
            // SAFETY: 调用者保证 exact-type 合约，vtable.as_mut_ptr 返回有效的 *mut T 指针。
            unsafe {
                let ptr = (self.vtable.as_mut_ptr.as_fn())(self.data.as_mut_ptr());
                Some(&mut *(ptr as *mut T))
            }
        } else {
            None
        }
    }

    #[inline(always)]
    /// Take ownership of the erased value after an exact-type downcast.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `T` is exactly the erased value type,
    /// including all lifetime parameters.
    pub(crate) unsafe fn downcast<T>(mut self) -> Option<T> {
        if self.value_type_id() == type_id_token::<T>() {
            // SAFETY: 调用者保证 exact-type 合约，当前 vtable 与存储路径匹配。
            // 使用 ptr::read 将 T 从底层内存中提取出来，并使用 mem::forget 防止 AnyValue 的 drop 再次释放底层对象。
            unsafe {
                let val = if self.is_heap {
                    let heap_ptr = ptr::read(self.data.as_mut_ptr().cast::<*mut T>());
                    *Box::from_raw(heap_ptr)
                } else {
                    let ptr = (self.vtable.as_mut_ptr.as_fn())(self.data.as_mut_ptr()) as *mut T;
                    ptr::read(ptr)
                };
                std::mem::forget(self);
                Some(val)
            }
        } else {
            None
        }
    }
}

impl Drop for AnyValue<'_> {
    fn drop(&mut self) {
        // SAFETY: vtable.drop 指向类型特定且存储路径相符的析构函数（STACK 为 drop_in_place，HEAP 为 Box 释放）。
        unsafe {
            (self.vtable.drop.as_fn())(self.data.as_mut_ptr());
        }
    }
}

pub(crate) enum Computation<'scope> {
    Effect(EffectThunk<'scope>),
    Previous(PreviousThunk<'scope>),
    Watch(WatchThunk<'scope>),
    Memo(MemoThunk<'scope>),
}

pub(crate) struct EffectThunk<'scope> {
    callback: Box<dyn FnMut() -> Result<(), ErrorEvent<'scope>> + 'scope>,
}

impl<'scope> EffectThunk<'scope> {
    pub(crate) fn new<E, F>(
        callback: F,
        handler: ErrorHandler<'scope, E>,
        initial_slot: InitialErrorSlot<E>,
    ) -> Self
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move || {
                callback().map_err(|error| ErrorEvent::new(error, handler, initial_slot.clone()))
            }),
        }
    }

    pub(crate) fn call(&mut self) -> Result<(), ErrorEvent<'scope>> {
        (self.callback)()
    }
}

pub(crate) struct PreviousThunk<'scope> {
    callback: PreviousCallback<'scope>,
}

type PreviousCallback<'scope> = Box<
    dyn FnMut(Option<&AnyValue<'scope>>) -> Result<AnyValue<'scope>, ErrorEvent<'scope>> + 'scope,
>;

impl<'scope> PreviousThunk<'scope> {
    pub(crate) fn new<T, E, F>(
        callback: F,
        handler: ErrorHandler<'scope, E>,
        initial_slot: InitialErrorSlot<E>,
    ) -> Self
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move |old| {
                let old = old.map(|value| unsafe {
                    value
                        .downcast_ref::<T>()
                        .expect("previous computation value type must match")
                });
                callback(old)
                    .map(AnyValue::new)
                    .map_err(|error| ErrorEvent::new(error, handler, initial_slot.clone()))
            }),
        }
    }

    pub(crate) fn compute(
        &mut self,
        old: Option<&AnyValue<'scope>>,
    ) -> Result<AnyValue<'scope>, ErrorEvent<'scope>> {
        (self.callback)(old)
    }
}

pub(crate) struct WatchThunk<'scope> {
    getter: Box<dyn FnMut() -> Result<AnyValue<'scope>, ErrorEvent<'scope>> + 'scope>,
    callback: WatchCallback<'scope>,
    initialized: bool,
    immediate: bool,
    once: bool,
}

type WatchCallback<'scope> = Box<
    dyn FnMut(&AnyValue<'scope>, Option<&AnyValue<'scope>>) -> Result<(), ErrorEvent<'scope>>
        + 'scope,
>;

impl<'scope> WatchThunk<'scope> {
    pub(crate) fn new<T, E, G, C>(
        getter: G,
        callback: C,
        handler: ErrorHandler<'scope, E>,
        initial_slot: InitialErrorSlot<E>,
        immediate: bool,
        once: bool,
    ) -> Self
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        let mut getter = getter;
        let mut callback = callback;
        let getter_handler = handler;
        let getter_slot = initial_slot.clone();
        Self {
            getter: Box::new(move || {
                getter()
                    .map(AnyValue::new_reactive)
                    .map_err(|error| ErrorEvent::new(error, getter_handler, getter_slot.clone()))
            }),
            callback: Box::new(move |new, old| {
                let new = unsafe { new.downcast_ref::<T>() }
                    .expect("watch getter and callback value types must match");
                let old = old.map(|value| unsafe {
                    value
                        .downcast_ref::<T>()
                        .expect("watch getter and callback value types must match")
                });
                callback(new, old)
                    .map_err(|error| ErrorEvent::new(error, handler, initial_slot.clone()))
            }),
            initialized: false,
            immediate,
            once,
        }
    }

    pub(crate) fn get(&mut self) -> Result<AnyValue<'scope>, ErrorEvent<'scope>> {
        (self.getter)()
    }

    pub(crate) fn call(
        &mut self,
        new: &AnyValue<'scope>,
        old: Option<&AnyValue<'scope>>,
    ) -> Result<(), ErrorEvent<'scope>> {
        (self.callback)(new, old)
    }

    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    pub(crate) fn immediate(&self) -> bool {
        self.immediate
    }

    pub(crate) fn once(&self) -> bool {
        self.once
    }
}

pub(crate) struct MemoThunk<'scope> {
    callback: MemoCallback<'scope>,
}

type MemoCallback<'scope> = Box<dyn FnMut(Option<&AnyValue<'scope>>) -> AnyValue<'scope> + 'scope>;

impl<'scope> MemoThunk<'scope> {
    pub(crate) fn new<T, F>(callback: F) -> Self
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move |old| {
                let old = old.and_then(|value| unsafe { value.downcast_ref::<T>() });
                AnyValue::new_reactive(callback(old))
            }),
        }
    }

    pub(crate) fn new_derived<T, F>(callback: F) -> Self
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move |_| AnyValue::new(callback())),
        }
    }

    pub(crate) fn compute(&mut self, old: Option<&AnyValue<'scope>>) -> AnyValue<'scope> {
        (self.callback)(old)
    }
}

pub(crate) struct CleanupThunk<'scope> {
    callback: Option<Box<dyn FnOnce() -> Result<(), ErrorEvent<'scope>> + 'scope>>,
}

impl<'scope> CleanupThunk<'scope> {
    pub(crate) fn new<E, F>(callback: F, handler: ErrorHandler<'scope, E>) -> Self
    where
        E: 'scope,
        F: FnOnce() -> Result<(), E> + 'scope,
    {
        Self {
            callback: Some(Box::new(move || {
                callback().map_err(|error| ErrorEvent::deferred(error, handler))
            })),
        }
    }

    pub(crate) fn call(mut self) -> Result<(), ErrorEvent<'scope>> {
        self.callback.take().expect("cleanup thunk called twice")()
    }
}

pub(crate) enum CallbackThunkError<'scope> {
    Runtime(ReactiveError),
    User(AnyValue<'scope>),
}

pub(crate) struct CallbackThunk<'scope> {
    callback: Box<dyn FnMut(AnyValue<'scope>) -> Result<(), CallbackThunkError<'scope>> + 'scope>,
}

impl<'scope> CallbackThunk<'scope> {
    fn new<F>(callback: F) -> Self
    where
        F: FnMut(AnyValue<'scope>) -> Result<(), CallbackThunkError<'scope>> + 'scope,
    {
        Self {
            callback: Box::new(callback),
        }
    }

    pub(crate) fn new_typed_fallible<T, E, F>(callback: F) -> Self
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + 'scope,
    {
        let mut callback = callback;
        Self::new(move |value| {
            let value = unsafe { value.downcast::<T>() }
                .ok_or(CallbackThunkError::Runtime(ReactiveError::TypeMismatch))?;
            callback(value).map_err(|error| CallbackThunkError::User(AnyValue::new(error)))
        })
    }

    pub(crate) fn new_typed_infallible<T, F>(callback: F) -> Self
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        let mut callback = callback;
        Self::new(move |value| {
            let value = unsafe { value.downcast::<T>() }
                .ok_or(CallbackThunkError::Runtime(ReactiveError::TypeMismatch))?;
            callback(value);
            Ok(())
        })
    }

    pub(crate) fn call(&mut self, arg: AnyValue<'scope>) -> Result<(), CallbackThunkError<'scope>> {
        (self.callback)(arg)
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
        if let Some(val) = unsafe { v.downcast_mut::<i32>() } {
            *val = 42;
        }
        assert_eq!(unsafe { v.downcast_ref::<i32>() }, Some(&42));
        assert_eq!(unsafe { v.downcast_ref::<u32>() }, None);
    }

    #[test]
    fn test_any_value_heap_downcast_returns_owned_value() {
        let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
        struct HeapValue {
            dropped: std::rc::Rc<std::cell::Cell<i32>>,
            _padding: [u8; 32],
        }
        impl Drop for HeapValue {
            fn drop(&mut self) {
                self.dropped.set(self.dropped.get() + 1);
            }
        }

        let value = AnyValue::new(HeapValue {
            dropped: dropped.clone(),
            _padding: [0; 32],
        });
        let value = unsafe { value.downcast::<HeapValue>() }.expect("exact type should downcast");
        drop(value);
        assert_eq!(dropped.get(), 1);
    }

    #[test]
    fn test_any_value_non_static_lifetime() {
        #[derive(Debug, PartialEq, Eq)]
        struct BorrowedData<'a>(&'a str);

        let s = String::from("hello world");
        let b1 = BorrowedData(s.as_str());
        let b2 = BorrowedData(s.as_str());

        let v1 = AnyValue::new_reactive(b1);
        let v2 = AnyValue::new_reactive(b2);

        assert!(v1.try_eq(&v2));
        assert_eq!(
            unsafe { v1.downcast_ref::<BorrowedData>() },
            Some(&BorrowedData("hello world"))
        );
    }
}
