use std::marker::PhantomData;
use std::mem;
use std::panic::Location;
use std::ptr;

use silex_reactivity::{
    NodeId, get_debug_label, get_node_defined_at, is_signal_valid, notify_signal, register_derived,
    set_debug_label, signal as create_signal, store_value, track_signal, try_update_signal_silent,
    untrack as untrack_scoped,
};

use crate::reactivity::SignalSlice;
use crate::traits::*;
use crate::traits::{RxCloneData, RxData};
use crate::{Rx, RxValueKind};

/// 内部辅助函数：直接从运行时借用信号值。
/// 安全性：由 RxGuard 的生命周期和 Silex Arena 的地址稳定性保证。
pub(crate) unsafe fn rx_borrow_signal_unsafe<T: RxData>(id: NodeId) -> Option<&'static T> {
    unsafe { silex_reactivity::try_get_any_raw_untracked(id).map(|ptr| &*(ptr as *const T)) }
}

/// 内部辅助函数：直接从运行时借用 StoredValue。
unsafe fn rx_borrow_stored_value_unsafe<T: RxData>(id: NodeId) -> Option<&'static T> {
    unsafe { silex_reactivity::try_get_any_raw_untracked(id).map(|ptr| &*(ptr as *const T)) }
}

// --- Constant ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Constant<T>(pub T);

impl<T: RxData> RxValue for Constant<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Constant<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        None
    }
    #[inline(always)]
    fn track(&self) {}
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        false
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        Some("Constant".to_string())
    }
}

impl<T: RxData> RxInternal for Constant<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        Some(RxGuard::Borrowed {
            value: &self.0,
            token: None,
        })
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        Some(fun(&self.0))
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> IntoRx for Constant<T> {
    type RxType = Rx<T, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_constant(self.0)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> crate::traits::IntoSignal for Constant<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<T> {
        Signal::derive(Box::new(move || self.get()))
    }
}

// --- DerivedPayload ---

#[derive(Clone, Copy)]
pub struct DerivedPayload<Deps, F> {
    pub(crate) deps: Deps,
    pub(crate) func: F,
}

impl<D: std::fmt::Debug, F> std::fmt::Debug for DerivedPayload<D, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedPayload")
            .field("deps", &self.deps)
            .field("func", &"Fn(...)")
            .finish()
    }
}

impl<D, F> DerivedPayload<D, F> {
    pub const fn new(deps: D, func: F) -> Self {
        Self { deps, func }
    }
}

// --- RxInternal for DerivedPayloads ---

// Unary / Map implementation
impl<S, F, U> RxValue for DerivedPayload<S, F>
where
    S: RxValue,
    F: Fn(&S::Value) -> U + 'static,
{
    type Value = U;
}

impl<S, F, U> RxBase for DerivedPayload<S, F>
where
    S: RxBase + RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        self.deps.id()
    }
    #[inline(always)]
    fn track(&self) {
        self.deps.track();
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.deps.is_disposed()
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.deps.defined_at()
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.deps.debug_name()
    }
}

impl<S, F, U> RxInternal for DerivedPayload<S, F>
where
    S: RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    type ReadOutput<'a>
        = RxGuard<'a, U, U>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.deps
            .rx_try_with_untracked(|v| (self.func)(v))
            .map(RxGuard::Owned)
    }

    #[inline(always)]
    fn rx_try_with_untracked<URet>(&self, fun: impl FnOnce(&Self::Value) -> URet) -> Option<URet> {
        self.deps.rx_try_with_untracked(|v| {
            let u_val = (self.func)(v);
            fun(&u_val)
        })
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    fn rx_is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> IntoRx for DerivedPayload<S, F>
where
    S: RxInternal + Clone + 'static,
    F: Fn(&S::Value) -> U + 'static,
    U: RxCloneData, // 使用 RxCloneData 确保 .get() 可用
{
    type RxType = Rx<U, RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::derive(Box::new(move || {
            use crate::traits::RxGet;
            self.get()
        }))
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> crate::traits::IntoSignal for DerivedPayload<S, F>
where
    S: RxRead + Clone + 'static,
    for<'a> S::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&S::Value) -> U + 'static,
    U: RxCloneData,
{
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        use crate::traits::RxGet;
        Signal::derive(Box::new(move || self.get()))
    }
}

// --- OpPayload (Aggressive De-genericization) ---

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpPayloadHeader {
    pub read_to_ptr: unsafe fn(this: *const u8, out: *mut u8) -> bool,
    pub track: fn(this: *const u8),
    pub is_constant: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StaticMapPayload<OT> {
    pub header: OpPayloadHeader,
    pub input_id: NodeId,
    pub compute: unsafe fn(input_ptr: *const (), out_ptr: *mut (), mapper_ptr: *const ()),
    pub mapper_ptr: *const (),
    pub _marker: PhantomData<OT>,
}

impl<OT: RxData> StaticMapPayload<OT> {
    pub fn new<IT: RxData>(input_id: NodeId, mapper: fn(&IT) -> OT, is_constant: bool) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::thin_map_read_to_ptr,
                track: op_trampolines::track_unary,
                is_constant,
            },
            input_id,
            compute: op_trampolines::compute_map::<IT, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }

    pub fn new_unary<IT: RxData>(
        input_id: NodeId,
        mapper: fn(&IT) -> OT,
        is_constant: bool,
    ) -> Self {
        Self::new(input_id, mapper, is_constant)
    }

    pub fn new_with_track<IT: RxData>(
        input_id: NodeId,
        mapper: fn(&IT) -> OT,
        track_fn: fn(this: *const u8),
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::thin_map_read_to_ptr,
                track: track_fn,
                is_constant,
            },
            input_id,
            compute: op_trampolines::compute_map::<IT, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StaticMap2Payload<OT> {
    pub header: OpPayloadHeader,
    pub inputs: [NodeId; 2],
    pub compute: unsafe fn(i1: *const (), i2: *const (), out_ptr: *mut (), mapper_ptr: *const ()),
    pub mapper_ptr: *const (),
    pub _marker: PhantomData<OT>,
}

impl<OT: RxData> StaticMap2Payload<OT> {
    pub fn new<I1: RxData, I2: RxData>(
        inputs: [NodeId; 2],
        mapper: fn(&I1, &I2) -> OT,
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::thin_map2_read_to_ptr,
                track: op_trampolines::track_binary,
                is_constant,
            },
            inputs,
            compute: op_trampolines::compute_map2::<I1, I2, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StaticMap3Payload<OT> {
    pub header: OpPayloadHeader,
    pub inputs: [NodeId; 3],
    pub compute: unsafe fn(
        i1: *const (),
        i2: *const (),
        i3: *const (),
        out_ptr: *mut (),
        mapper_ptr: *const (),
    ),
    pub mapper_ptr: *const (),
    pub _marker: PhantomData<OT>,
}

impl<OT: RxData> StaticMap3Payload<OT> {
    pub fn new<I1: RxData, I2: RxData, I3: RxData>(
        inputs: [NodeId; 3],
        mapper: fn(&I1, &I2, &I3) -> OT,
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::thin_map3_read_to_ptr,
                track: op_trampolines::track_ternary,
                is_constant,
            },
            inputs,
            compute: op_trampolines::compute_map3::<I1, I2, I3, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpPayload<U, const N: usize> {
    pub header: OpPayloadHeader,
    pub inputs: [NodeId; N],
    pub raw_read_to_ptr: unsafe fn(inputs: &[NodeId], out: *mut u8) -> bool,
    pub raw_track: fn(inputs: &[NodeId]),
    pub _marker: PhantomData<U>,
}

impl<U: RxData, const N: usize> OpPayload<U, N> {
    pub fn new(
        inputs: [NodeId; N],
        read_to_ptr: unsafe fn(inputs: &[NodeId], out: *mut u8) -> bool,
        track: fn(inputs: &[NodeId]),
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::op_read_to_ptr_trampoline::<U, N>,
                track: op_trampolines::op_track_wrap::<U, N>,
                is_constant,
            },
            inputs,
            raw_read_to_ptr: read_to_ptr,
            raw_track: track,
            _marker: PhantomData,
        }
    }
}

impl<U: RxData, const N: usize> std::fmt::Debug for OpPayload<U, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpPayload")
            .field("inputs", &&self.inputs)
            .field(
                "read",
                &format_args!("{:p}", self.raw_read_to_ptr as *const ()),
            )
            .field("is_constant", &self.header.is_constant)
            .finish()
    }
}

impl<U: RxData, const N: usize> RxValue for OpPayload<U, N> {
    type Value = U;
}

impl<U: RxData, const N: usize> RxBase for OpPayload<U, N> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        None
    }
    #[inline(always)]
    fn track(&self) {
        (self.header.track)(self as *const Self as *const u8);
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        for i in 0..N {
            if !is_signal_valid(self.inputs[i]) {
                return true;
            }
        }
        false
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        None
    }
}

impl<U: RxData, const N: usize> RxInternal for OpPayload<U, N> {
    type ReadOutput<'a>
        = RxGuard<'a, U, U>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        let mut out = mem::MaybeUninit::<U>::uninit();
        unsafe {
            if (self.header.read_to_ptr)(
                self as *const Self as *const u8,
                out.as_mut_ptr() as *mut u8,
            ) {
                Some(RxGuard::Owned(out.assume_init()))
            } else {
                None
            }
        }
    }

    #[inline(always)]
    fn rx_try_with_untracked<URet>(&self, fun: impl FnOnce(&Self::Value) -> URet) -> Option<URet> {
        let mut out = mem::MaybeUninit::<U>::uninit();
        unsafe {
            if (self.header.read_to_ptr)(
                self as *const Self as *const u8,
                out.as_mut_ptr() as *mut u8,
            ) {
                let v = out.assume_init();
                Some(fun(&v))
            } else {
                None
            }
        }
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    fn rx_is_constant(&self) -> bool {
        self.header.is_constant
    }
}

impl<U: RxCloneData + 'static, const N: usize> IntoRx for OpPayload<U, N> {
    type RxType = Rx<U, RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::derive(Box::new(move || {
            use crate::traits::RxGet;
            self.get()
        }))
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.header.is_constant
    }
}

impl<U: RxCloneData, const N: usize> crate::traits::IntoSignal for OpPayload<U, N> {
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        use crate::traits::RxGet;
        Signal::derive(Box::new(move || self.get()))
    }
}

// Trampoline 辅助工具：用于在宏中生成虚函数表实例
pub mod op_trampolines {
    use super::*;

    pub unsafe fn op_read_to_ptr_trampoline<U: RxData, const N: usize>(
        this: *const u8,
        out: *mut u8,
    ) -> bool {
        let payload = unsafe { &*(this as *const OpPayload<U, N>) };
        unsafe { (payload.raw_read_to_ptr)(&payload.inputs, out) }
    }

    pub fn op_track_wrap<U: RxData, const N: usize>(this: *const u8) {
        let payload = unsafe { &*(this as *const OpPayload<U, N>) };
        (payload.raw_track)(&payload.inputs)
    }

    pub unsafe fn thin_map_read_to_ptr(this: *const u8, out: *mut u8) -> bool {
        let payload = unsafe { &*(this as *const StaticMapPayload<()>) }; // Any OT works for layout
        if let Some(input_ptr) =
            unsafe { silex_reactivity::try_get_any_raw_untracked(payload.input_id) }
        {
            unsafe { (payload.compute)(input_ptr, out as *mut (), payload.mapper_ptr) };
            true
        } else {
            false
        }
    }

    pub unsafe fn compute_map<IT: RxData, OT: RxData>(
        input: *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&IT) -> OT = unsafe { std::mem::transmute(mapper) };
        let val = mapper(unsafe { &*(input as *const IT) });
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    pub fn track_unary(this: *const u8) {
        let payload = unsafe { &*(this as *const StaticMapPayload<()>) };
        track_signal(payload.input_id);
    }

    pub unsafe fn thin_map2_read_to_ptr(this: *const u8, out: *mut u8) -> bool {
        let payload = unsafe { &*(this as *const StaticMap2Payload<()>) };
        if let Some(v1) = unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[0]) }
        {
            if let Some(v2) =
                unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[1]) }
            {
                unsafe { (payload.compute)(v1, v2, out as *mut (), payload.mapper_ptr) };
                return true;
            }
        }
        false
    }

    pub unsafe fn compute_map2<I1: RxData, I2: RxData, OT: RxData>(
        i1: *const (),
        i2: *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&I1, &I2) -> OT = unsafe { std::mem::transmute(mapper) };
        let val = mapper(unsafe { &*(i1 as *const I1) }, unsafe {
            &*(i2 as *const I2)
        });
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    pub fn track_binary(this: *const u8) {
        let payload = unsafe { &*(this as *const StaticMap2Payload<()>) };
        track_signal(payload.inputs[0]);
        track_signal(payload.inputs[1]);
    }

    pub unsafe fn thin_map3_read_to_ptr(this: *const u8, out: *mut u8) -> bool {
        let payload = unsafe { &*(this as *const StaticMap3Payload<()>) };
        if let Some(v1) = unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[0]) }
        {
            if let Some(v2) =
                unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[1]) }
            {
                if let Some(v3) =
                    unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[2]) }
                {
                    unsafe { (payload.compute)(v1, v2, v3, out as *mut (), payload.mapper_ptr) };
                    return true;
                }
            }
        }
        false
    }

    pub unsafe fn compute_map3<I1: RxData, I2: RxData, I3: RxData, OT: RxData>(
        i1: *const (),
        i2: *const (),
        i3: *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&I1, &I2, &I3) -> OT = unsafe { std::mem::transmute(mapper) };
        let val = mapper(
            unsafe { &*(i1 as *const I1) },
            unsafe { &*(i2 as *const I2) },
            unsafe { &*(i3 as *const I3) },
        );
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    pub fn track_ternary(this: *const u8) {
        let payload = unsafe { &*(this as *const StaticMap3Payload<()>) };
        track_signal(payload.inputs[0]);
        track_signal(payload.inputs[1]);
        track_signal(payload.inputs[2]);
    }

    pub fn track_inputs(inputs: &[NodeId]) {
        for &id in inputs {
            track_signal(id);
        }
    }

    /// 追踪打包在 StoredValue 中的 N 个信号
    pub fn track_tuple_meta<const N: usize>(this: *const u8) {
        let payload = unsafe { &*(this as *const StaticMapPayload<()>) };
        let meta_id = payload.input_id;
        let _ = silex_reactivity::try_with_stored_value(meta_id, |ids: &[NodeId; N]| {
            for &id in ids {
                track_signal(id);
            }
        });
    }

    // --- 元组读取 Mappers ---

    pub fn tuple_2_mapper<T0: Clone + 'static, T1: Clone + 'static>(i0: &T0, i1: &T1) -> (T0, T1) {
        (i0.clone(), i1.clone())
    }

    pub fn tuple_3_mapper<T0: Clone + 'static, T1: Clone + 'static, T2: Clone + 'static>(
        ids: &[NodeId; 3],
    ) -> (T0, T1, T2) {
        unsafe {
            (
                rx_borrow_signal_unsafe::<T0>(ids[0]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T1>(ids[1]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T2>(ids[2]).unwrap().clone(),
            )
        }
    }

    pub fn tuple_4_mapper<
        T0: Clone + 'static,
        T1: Clone + 'static,
        T2: Clone + 'static,
        T3: Clone + 'static,
    >(
        ids: &[NodeId; 4],
    ) -> (T0, T1, T2, T3) {
        unsafe {
            (
                rx_borrow_signal_unsafe::<T0>(ids[0]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T1>(ids[1]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T2>(ids[2]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T3>(ids[3]).unwrap().clone(),
            )
        }
    }

    pub fn tuple_5_mapper<
        T0: Clone + 'static,
        T1: Clone + 'static,
        T2: Clone + 'static,
        T3: Clone + 'static,
        T4: Clone + 'static,
    >(
        ids: &[NodeId; 5],
    ) -> (T0, T1, T2, T3, T4) {
        unsafe {
            (
                rx_borrow_signal_unsafe::<T0>(ids[0]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T1>(ids[1]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T2>(ids[2]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T3>(ids[3]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T4>(ids[4]).unwrap().clone(),
            )
        }
    }

    pub fn tuple_6_mapper<
        T0: Clone + 'static,
        T1: Clone + 'static,
        T2: Clone + 'static,
        T3: Clone + 'static,
        T4: Clone + 'static,
        T5: Clone + 'static,
    >(
        ids: &[NodeId; 6],
    ) -> (T0, T1, T2, T3, T4, T5) {
        unsafe {
            (
                rx_borrow_signal_unsafe::<T0>(ids[0]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T1>(ids[1]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T2>(ids[2]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T3>(ids[3]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T4>(ids[4]).unwrap().clone(),
                rx_borrow_signal_unsafe::<T5>(ids[5]).unwrap().clone(),
            )
        }
    }
}

// --- Signal 信号 Enum ---

pub enum Signal<T> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
    StoredConstant(NodeId, PhantomData<T>),
    #[allow(missing_docs)] // Internal optimization detail
    InlineConstant(u64, PhantomData<T>),
}

impl<T: RxData> std::fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(s) => f.debug_tuple("Read").field(s).finish(),
            Self::Derived(id, _) => f.debug_tuple("Derived").field(id).finish(),
            Self::StoredConstant(id, _) => f.debug_tuple("StoredConstant").field(id).finish(),
            Self::InlineConstant(val, _) => f.debug_tuple("InlineConstant").field(val).finish(),
        }
    }
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Read(l), Self::Read(r)) => l == r,
            (Self::Derived(l, _), Self::Derived(r, _)) => l == r,
            (Self::StoredConstant(l, _), Self::StoredConstant(r, _)) => l == r,
            (Self::InlineConstant(l, _), Self::InlineConstant(r, _)) => l == r,
            _ => false,
        }
    }
}

impl<T> Eq for Signal<T> {}

impl<T: RxData> RxValue for Signal<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Signal<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        match self {
            Signal::Read(s) => Some(s.id),
            Signal::Derived(id, _) => Some(*id),
            Signal::StoredConstant(id, _) => Some(*id),
            Signal::InlineConstant(_, _) => None,
        }
    }

    #[inline(always)]
    fn track(&self) {
        if let Some(id) = self.id() {
            track_signal(id);
        }
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.id().map(|id| !is_signal_valid(id)).unwrap_or(false)
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.id().and_then(get_node_defined_at)
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        let name = self.id().and_then(get_debug_label);
        if name.is_none() && self.is_constant() {
            Some("Constant".to_string())
        } else {
            name
        }
    }
}

impl<T: RxData> RxInternal for Signal<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        match self {
            Signal::Read(s) => s.rx_read_untracked(),
            Signal::Derived(id, _) => {
                // 不进行 track，仅获取当前值
                unsafe {
                    rx_borrow_signal_unsafe::<T>(*id).map(|v| RxGuard::Borrowed {
                        value: v,
                        token: Some(crate::NodeRef::from_id(*id)),
                    })
                }
            }
            Signal::StoredConstant(id, _) => unsafe {
                rx_borrow_stored_value_unsafe::<T>(*id).map(|v| RxGuard::Borrowed {
                    value: v,
                    token: Some(crate::NodeRef::from_id(*id)),
                })
            },
            Signal::InlineConstant(val, _) => {
                let val = unsafe { Self::unpack_inline(*val) };
                Some(RxGuard::Owned(val))
            }
        }
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match self {
            Signal::Read(s) => s.rx_try_with_untracked(fun),
            Signal::Derived(id, _) => unsafe { rx_borrow_signal_unsafe(*id).map(fun) },
            Signal::StoredConstant(id, _) => unsafe { rx_borrow_stored_value_unsafe(*id).map(fun) },
            Signal::InlineConstant(storage, _) => {
                let val = unsafe { Self::unpack_inline(*storage) };
                Some(fun(&val))
            }
        }
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        match self {
            Signal::Read(s) => s.rx_get_adaptive(),
            Signal::Derived(_, _) | Signal::StoredConstant(_, _) => self
                .rx_try_with_untracked(|v| {
                    use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
                    AdaptiveWrapper(v).maybe_clone()
                })
                .flatten(),
            Signal::InlineConstant(storage, _) => {
                let val = unsafe { Self::unpack_inline(*storage) };
                Some(val)
            }
        }
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<T: RxData> IntoRx for Signal<T> {
    type RxType = Rx<T, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_signal(self.ensure_node_id())
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<T: RxData> crate::traits::IntoSignal for Signal<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        self
    }
}

impl<T> std::hash::Hash for Signal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Read(s) => s.hash(state),
            Self::Derived(id, _) => id.hash(state),
            Self::StoredConstant(id, _) => id.hash(state),
            Self::InlineConstant(val, _) => val.hash(state),
        }
    }
}

// --- Generic Impl Block ---

impl<T: RxData> Signal<T> {
    #[track_caller]
    pub fn derive(f: Box<dyn Fn() -> T>) -> Self {
        let id = register_derived(f);
        Signal::Derived(id, PhantomData)
    }

    /// Internal helper to try inlining a value
    fn try_inline(value: T) -> Option<Self> {
        // Can only inline if it fits in u64 and doesn't implement Drop
        #[allow(clippy::manual_is_variant_and)] // we want explicit check
        if mem::size_of::<T>() <= mem::size_of::<u64>() && !mem::needs_drop::<T>() {
            unsafe {
                let mut storage = 0u64;
                let src_ptr = &value as *const T as *const u8;
                let dst_ptr = &mut storage as *mut u64 as *mut u8;
                ptr::copy_nonoverlapping(src_ptr, dst_ptr, mem::size_of::<T>());
                // Value is not dropped because we checked !needs_drop, so we can just forget it
                mem::forget(value);
                Some(Signal::InlineConstant(storage, PhantomData))
            }
        } else {
            None
        }
    }

    /// Internal helper to unpack an inlined value
    unsafe fn unpack_inline(storage: u64) -> T {
        unsafe {
            let mut value = mem::MaybeUninit::<T>::uninit();
            let src_ptr = &storage as *const u64 as *const u8;
            let dst_ptr = value.as_mut_ptr() as *mut u8;
            ptr::copy_nonoverlapping(src_ptr, dst_ptr, mem::size_of::<T>());
            value.assume_init()
        }
    }

    pub fn node_id(&self) -> Option<NodeId> {
        match self {
            Signal::Read(s) => Some(s.id),
            Signal::Derived(id, _) => Some(*id),
            Signal::StoredConstant(id, _) => Some(*id),
            Signal::InlineConstant(_, _) => None,
        }
    }

    /// 确保信号具有 NodeId。
    /// 如果是内联常量，则会将其提升为存储常量。
    pub fn ensure_node_id(&self) -> NodeId {
        if let Some(id) = self.node_id() {
            id
        } else if let Signal::InlineConstant(storage, _) = self {
            // 安全性：InlineConstant 保证了 T 不实现 Drop 且大小合适
            let value = unsafe { Self::unpack_inline(*storage) };
            store_value(value)
        } else {
            unreachable!("Signal must be either Read, Derived, StoredConstant or InlineConstant")
        }
    }

    pub fn is_constant(&self) -> bool {
        matches!(
            self,
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _)
        )
    }
}

impl<T: Default + RxCloneData> Default for Signal<T> {
    fn default() -> Self {
        T::default().into()
    }
}

impl<T: RxCloneData> Signal<T> {
    // derive moved to T: 'static block

    pub fn with_name(self, name: impl Into<String>) -> Self {
        match self {
            Signal::Read(s) => {
                s.with_name(name);
            }
            Signal::Derived(id, _) => set_debug_label(id, name),
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _) => {} // Constants usually don't need debug labels in the graph
        }
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static,
    {
        SignalSlice::new(self, getter)
    }
}

// Note: GetUntracked and Get methods are now provided as default methods in the RxRead trait.

impl<T: RxCloneData> From<T> for Signal<T> {
    #[track_caller]
    fn from(value: T) -> Self {
        if let Some(inline) = Self::try_inline(value.clone()) {
            return inline;
        }
        let id = store_value(value);
        Signal::StoredConstant(id, PhantomData)
    }
}

impl From<&str> for Signal<String> {
    #[track_caller]
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl<T: RxData> From<ReadSignal<T>> for Signal<T> {
    fn from(s: ReadSignal<T>) -> Self {
        Signal::Read(s)
    }
}

impl<T: RxData> From<RwSignal<T>> for Signal<T> {
    fn from(s: RwSignal<T>) -> Self {
        Signal::Read(s.read)
    }
}

// --- ReadSignal ---

pub struct ReadSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> ReadSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
        T: 'static,
    {
        SignalSlice::new(self, getter)
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! impl_signal_core_traits {
    ($($ty:ident),*) => {
        $(
            impl<T> std::fmt::Debug for $ty<T> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}({:?})", stringify!($ty), self.id)
                }
            }

            impl<T> Clone for $ty<T> {
                fn clone(&self) -> Self {
                    *self
                }
            }
            impl<T> Copy for $ty<T> {}

            impl<T> PartialEq for $ty<T> {
                fn eq(&self, other: &Self) -> bool {
                    self.id == other.id
                }
            }

            impl<T> Eq for $ty<T> {}

            impl<T> std::hash::Hash for $ty<T> {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    self.id.hash(state);
                }
            }
        )*
    };
}

impl_signal_core_traits!(ReadSignal);

// Note: GetUntracked and Get are now blanket-implemented via RxRead

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> WriteSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
        self
    }
}

impl_signal_core_traits!(WriteSignal);

impl<T: RxData> RxValue for WriteSignal<T> {
    type Value = T;
}

impl<T: RxData> RxBase for WriteSignal<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        Some(self.id)
    }
    #[inline(always)]
    fn track(&self) {
        track_signal(self.id);
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !is_signal_valid(self.id)
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        get_node_defined_at(self.id)
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        get_debug_label(self.id)
    }
}

impl<T: RxData> RxWrite for WriteSignal<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        try_update_signal_silent(self.id, fun)
    }

    #[inline(always)]
    fn rx_notify(&self) {
        notify_signal(self.id);
    }
}

// --- RwSignal ---

pub struct RwSignal<T> {
    pub read: ReadSignal<T>,
    pub write: WriteSignal<T>,
}

impl<T> Clone for RwSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for RwSignal<T> {}

impl<T> PartialEq for RwSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}

impl<T> Eq for RwSignal<T> {}

impl<T> std::hash::Hash for RwSignal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.read.hash(state);
        self.write.hash(state);
    }
}

impl<T: RxData> RwSignal<T> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        let (read, write) = signal(value);
        RwSignal { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<T> {
        self.write
    }

    pub fn split(&self) -> (ReadSignal<T>, WriteSignal<T>) {
        (self.read, self.write)
    }

    pub fn from_parts(read: ReadSignal<T>, write: WriteSignal<T>) -> Self {
        Self { read, write }
    }

    pub fn with_name(self, name: impl Into<String>) -> Self {
        self.read.with_name(name);
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
    {
        SignalSlice::new(self, getter)
    }
}

impl<T: 'static> RxWrite for RwSignal<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        self.write.rx_try_update_untracked(fun)
    }

    #[inline(always)]
    fn rx_notify(&self) {
        self.write.rx_notify();
    }
}

// --- Global Functions ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = create_signal(value);
    (
        ReadSignal {
            id,
            marker: PhantomData,
        },
        WriteSignal {
            id,
            marker: PhantomData,
        },
    )
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    untrack_scoped(f)
}

// 手动实现了 RxInternal，移除自动委托以避免冲突
// crate::impl_rx_delegate!(Signal, false);
crate::impl_rx_delegate!(ReadSignal, SignalID, false);
crate::impl_rx_delegate!(RwSignal, read, false);
// Constant 使用手动实现以获得更优性能
// crate::impl_rx_delegate!(Constant, true);

crate::impl_reactive_ops!(Signal);
crate::impl_reactive_ops!(ReadSignal);
crate::impl_reactive_ops!(RwSignal);
crate::impl_reactive_ops!(Constant);
