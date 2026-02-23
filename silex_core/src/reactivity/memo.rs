use std::marker::PhantomData;

use silex_reactivity::NodeId;

// --- Memo ---

pub struct Memo<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

crate::impl_signal_core_traits!(Memo);

impl<T: Clone + PartialEq + 'static> Memo<T> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&T>) -> T + 'static,
    {
        let id = silex_reactivity::memo(f);
        Memo {
            id,
            marker: PhantomData,
        }
    }
}

impl<T> Memo<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

impl<T: 'static> From<Memo<T>> for crate::reactivity::Signal<T> {
    fn from(m: Memo<T>) -> Self {
        crate::reactivity::Signal::Read(crate::reactivity::ReadSignal {
            id: m.id,
            marker: PhantomData,
        })
    }
}

crate::impl_rx_delegate!(Memo, SignalID, false);

crate::impl_reactive_ops!(Memo);
