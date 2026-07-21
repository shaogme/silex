use std::marker::PhantomData;

use crate::{
    impl_reactive_ops, impl_rx_delegate, impl_signal_core_traits,
    reactivity::{ReadSignal, Signal},
    traits::{RxCloneData, RxData},
};
use silex_reactivity::{NodeId, memo, set_debug_label};

// --- Memo ---

pub struct Memo<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl_signal_core_traits!(Memo);

impl<T: RxCloneData + PartialEq> Memo<T> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&T>) -> T + 'static,
    {
        let id = memo(f);
        Memo {
            id,
            marker: PhantomData,
        }
    }
}

impl<T> Memo<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
        self
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

impl<T: RxData> From<Memo<T>> for Signal<T> {
    fn from(m: Memo<T>) -> Self {
        Signal::Read(ReadSignal {
            id: m.id,
            marker: PhantomData,
        })
    }
}

impl_rx_delegate!(Memo, SignalID, false);

impl_reactive_ops!(Memo);
