use crate::traits::*;
use crate::{Rx, RxValueKind};
use silex_reactivity::{NodeId, is_signal_valid, track_signals_batch};
use std::marker::PhantomData;
use std::mem;
use std::panic::Location;

// --- OpPayload (Aggressive De-genericization) ---

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpPayloadHeader {
    pub read_to_ptr: unsafe fn(this: *const u8, out: *mut u8) -> bool,
    pub track: fn(this: *const u8),
    pub is_constant: bool,
    pub input_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UnifiedStaticMapPayload<OT> {
    pub header: OpPayloadHeader,
    pub compute: unsafe fn(inputs: *const *const (), out_ptr: *mut (), mapper_ptr: *const ()),
    pub mapper_ptr: *const (),
    pub inputs: [NodeId; 3],
    pub _marker: PhantomData<OT>,
}

pub type StaticMapPayload<OT> = UnifiedStaticMapPayload<OT>;
pub type StaticMap2Payload<OT> = UnifiedStaticMapPayload<OT>;
pub type StaticMap3Payload<OT> = UnifiedStaticMapPayload<OT>;

impl<OT: RxData> UnifiedStaticMapPayload<OT> {
    pub fn new1<IT: RxData>(input_id: NodeId, mapper: fn(&IT) -> OT, is_constant: bool) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::unified_map_read_to_ptr,
                track: op_trampolines::unified_track,
                is_constant,
                input_count: 1,
            },
            inputs: [
                input_id,
                NodeId {
                    index: 0,
                    generation: 0,
                },
                NodeId {
                    index: 0,
                    generation: 0,
                },
            ],
            compute: op_trampolines::compute_map_1::<IT, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }

    pub fn new1_with_track<IT: RxData>(
        input_id: NodeId,
        mapper: fn(&IT) -> OT,
        track_fn: fn(this: *const u8),
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::unified_map_read_to_ptr,
                track: track_fn,
                is_constant,
                input_count: 1,
            },
            inputs: [
                input_id,
                NodeId {
                    index: 0,
                    generation: 0,
                },
                NodeId {
                    index: 0,
                    generation: 0,
                },
            ],
            compute: op_trampolines::compute_map_1::<IT, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }

    pub fn new2<I1: RxData, I2: RxData>(
        inputs: [NodeId; 2],
        mapper: fn(&I1, &I2) -> OT,
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::unified_map_read_to_ptr,
                track: op_trampolines::unified_track,
                is_constant,
                input_count: 2,
            },
            inputs: [
                inputs[0],
                inputs[1],
                NodeId {
                    index: 0,
                    generation: 0,
                },
            ],
            compute: op_trampolines::compute_map_2::<I1, I2, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }

    pub fn new3<I1: RxData, I2: RxData, I3: RxData>(
        inputs: [NodeId; 3],
        mapper: fn(&I1, &I2, &I3) -> OT,
        is_constant: bool,
    ) -> Self {
        Self {
            header: OpPayloadHeader {
                read_to_ptr: op_trampolines::unified_map_read_to_ptr,
                track: op_trampolines::unified_track,
                is_constant,
                input_count: 3,
            },
            inputs,
            compute: op_trampolines::compute_map_3::<I1, I2, I3, OT>,
            mapper_ptr: mapper as *const (),
            _marker: PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpPayload<U, const N: usize> {
    pub header: OpPayloadHeader,
    pub raw_read_to_ptr: unsafe fn(inputs: &[NodeId], out: *mut u8) -> bool,
    pub raw_track: fn(inputs: &[NodeId]),
    pub inputs: [NodeId; N],
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
                input_count: N as u32,
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
        = crate::traits::RxGuard<'a, U, U>
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
                Some(crate::traits::RxGuard::Owned(out.assume_init()))
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
    fn into_signal(self) -> super::Signal<Self::Value> {
        use crate::traits::RxGet;
        super::Signal::derive(Box::new(move || self.get()))
    }
}

// Trampoline 辅助工具
pub mod op_trampolines {
    use super::*;

    /// # Safety
    ///
    /// This function is unsafe because it performs raw pointer dereferencing.
    /// The caller must ensure that `this` points to a valid `OpPayload<U, N>`
    /// and `out` points to a valid memory location for `U`.
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

    /// # Safety
    ///
    /// The caller must ensure that `this` points to a valid `UnifiedStaticMapPayload<()>`
    /// and `out` points to a valid memory location for the output type.
    pub unsafe fn unified_map_read_to_ptr(this: *const u8, out: *mut u8) -> bool {
        let payload = unsafe { &*(this as *const UnifiedStaticMapPayload<()>) };
        let mut input_ptrs = [std::ptr::null(); 3];
        for (i, item) in input_ptrs
            .iter_mut()
            .enumerate()
            .take(payload.header.input_count as usize)
        {
            if let Some(ptr) =
                unsafe { silex_reactivity::try_get_any_raw_untracked(payload.inputs[i]) }
            {
                *item = ptr;
            } else {
                return false;
            }
        }
        unsafe { (payload.compute)(input_ptrs.as_ptr(), out as *mut (), payload.mapper_ptr) };
        true
    }

    pub fn unified_track(this: *const u8) {
        let payload = unsafe { &*(this as *const UnifiedStaticMapPayload<()>) };
        track_signals_batch(&payload.inputs[..payload.header.input_count as usize]);
    }

    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `inputs` points to an array of at least 1 valid pointer to `IT`.
    /// - `out_ptr` points to a valid memory location for `OT`.
    /// - `mapper` is a valid function pointer of type `fn(&IT) -> OT`.
    pub unsafe fn compute_map_1<IT: RxData, OT: RxData>(
        inputs: *const *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&IT) -> OT = unsafe { std::mem::transmute(mapper) };
        let i0 = unsafe { &*(*inputs as *const IT) };
        let val = mapper(i0);
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `inputs` points to an array of at least 2 valid pointers to `I1` and `I2`.
    /// - `out_ptr` points to a valid memory location for `OT`.
    /// - `mapper` is a valid function pointer of type `fn(&I1, &I2) -> OT`.
    pub unsafe fn compute_map_2<I1: RxData, I2: RxData, OT: RxData>(
        inputs: *const *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&I1, &I2) -> OT = unsafe { std::mem::transmute(mapper) };
        let i0 = unsafe { &*(*inputs as *const I1) };
        let i1 = unsafe { &*(*inputs.add(1) as *const I2) };
        let val = mapper(i0, i1);
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `inputs` points to an array of at least 3 valid pointers to `I1`, `I2`, and `I3`.
    /// - `out_ptr` points to a valid memory location for `OT`.
    /// - `mapper` is a valid function pointer of type `fn(&I1, &I2, &I3) -> OT`.
    pub unsafe fn compute_map_3<I1: RxData, I2: RxData, I3: RxData, OT: RxData>(
        inputs: *const *const (),
        out_ptr: *mut (),
        mapper: *const (),
    ) {
        let mapper: fn(&I1, &I2, &I3) -> OT = unsafe { std::mem::transmute(mapper) };
        let i0 = unsafe { &*(*inputs as *const I1) };
        let i1 = unsafe { &*(*inputs.add(1) as *const I2) };
        let i2 = unsafe { &*(*inputs.add(2) as *const I3) };
        let val = mapper(i0, i1, i2);
        unsafe { std::ptr::write(out_ptr as *mut OT, val) };
    }

    pub fn track_inputs(inputs: &[NodeId]) {
        track_signals_batch(inputs);
    }

    pub fn track_tuple_meta_slice(this: *const u8) {
        let payload = unsafe { &*(this as *const UnifiedStaticMapPayload<()>) };
        let meta_id = payload.inputs[0];
        let _ = silex_reactivity::try_with_stored_value(meta_id, |ids: &Vec<NodeId>| {
            track_signals_batch(ids);
        });
    }

    pub fn track_tuple_meta<const N: usize>(this: *const u8) {
        track_tuple_meta_slice(this);
    }

    pub fn tuple_2_mapper<T0: Clone + 'static, T1: Clone + 'static>(i0: &T0, i1: &T1) -> (T0, T1) {
        (i0.clone(), i1.clone())
    }

    pub fn tuple_3_mapper<T0: Clone + 'static, T1: Clone + 'static, T2: Clone + 'static>(
        ids: &[NodeId; 3],
    ) -> (T0, T1, T2) {
        unsafe {
            (
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[0]).unwrap() as *const T0))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[1]).unwrap() as *const T1))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[2]).unwrap() as *const T2))
                    .clone(),
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
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[0]).unwrap() as *const T0))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[1]).unwrap() as *const T1))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[2]).unwrap() as *const T2))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[3]).unwrap() as *const T3))
                    .clone(),
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
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[0]).unwrap() as *const T0))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[1]).unwrap() as *const T1))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[2]).unwrap() as *const T2))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[3]).unwrap() as *const T3))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[4]).unwrap() as *const T4))
                    .clone(),
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
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[0]).unwrap() as *const T0))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[1]).unwrap() as *const T1))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[2]).unwrap() as *const T2))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[3]).unwrap() as *const T3))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[4]).unwrap() as *const T4))
                    .clone(),
                (&*(silex_reactivity::try_get_any_raw_untracked(ids[5]).unwrap() as *const T5))
                    .clone(),
            )
        }
    }
}
