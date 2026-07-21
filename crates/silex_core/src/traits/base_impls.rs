use crate::reactivity::NodeId;
use crate::traits::{RxBase, RxValue};

impl RxValue for () {
    type Value = ();
}

impl RxBase for () {
    fn id(&self) -> Option<NodeId> {
        None
    }
    fn track(&self) {}
    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
}

macro_rules! impl_rx_base_for_constant {
    ($($t:ty),*) => {
        $(
            impl RxBase for $t {
                fn id(&self) -> Option<NodeId> { None }
                fn track(&self) {}
                fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> { None }
            }
        )*
    };
}

impl_rx_base_for_constant!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool, String, &str
);
