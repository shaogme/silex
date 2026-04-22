use crate::logic::Map;
use crate::reactivity::{StaticMap2Payload, StaticMap3Payload};
use crate::traits::RxRead;
use crate::{Rx, RxValueKind};

/// 使用 @fn 时，显式调用单信号静态映射。
#[inline(always)]
pub fn map1_static<S, U>(s: S, f: fn(&S::Value) -> U) -> Rx<U, RxValueKind>
where
    S: Map + Clone + RxRead + 'static,
    S::Value: Sized + 'static,
    U: 'static,
{
    s.map_fn(f)
}

/// 使用 @fn 时，显式调用双信号静态映射。
#[inline(always)]
pub fn map2_static<I1, I2, U>(
    i1: I1,
    i2: I2,
    f: fn(&I1::Value, &I2::Value) -> U,
) -> Rx<U, RxValueKind>
where
    I1: Map + Clone + RxRead + 'static,
    I2: Map + Clone + RxRead + 'static,
    I1::Value: Sized + 'static,
    I2::Value: Sized + 'static,
    U: 'static,
{
    if let (Some(id1), Some(id2)) = (i1.id(), i2.id()) {
        let op = StaticMap2Payload::new2([id1, id2], f, false);
        Rx::new_op(op)
    } else {
        let s1 = i1.clone();
        let s2 = i2.clone();
        Rx::derive(Box::new(move || s1.with(|v1| s2.with(|v2| f(v1, v2)))))
    }
}

/// 使用 @fn 时，显式调用三信号静态映射。
#[inline(always)]
pub fn map3_static<I1, I2, I3, U>(
    i1: I1,
    i2: I2,
    i3: I3,
    f: fn(&I1::Value, &I2::Value, &I3::Value) -> U,
) -> Rx<U, RxValueKind>
where
    I1: Map + Clone + RxRead + 'static,
    I2: Map + Clone + RxRead + 'static,
    I3: Map + Clone + RxRead + 'static,
    I1::Value: Sized + 'static,
    I2::Value: Sized + 'static,
    I3::Value: Sized + 'static,
    U: 'static,
{
    if let (Some(id1), Some(id2), Some(id3)) = (i1.id(), i2.id(), i3.id()) {
        let op = StaticMap3Payload::new3([id1, id2, id3], f, false);
        Rx::new_op(op)
    } else {
        let s1 = i1.clone();
        let s2 = i2.clone();
        let s3 = i3.clone();
        Rx::derive(Box::new(move || {
            s1.with(|v1| s2.with(|v2| s3.with(|v3| f(v1, v2, v3))))
        }))
    }
}
