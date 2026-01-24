use std::marker::PhantomData;
use std::panic::Location;

use silex_reactivity::NodeId;

use crate::traits::*;

// --- Memo ---

pub struct Memo<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Memo({:?})", self.id)
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Memo<T> {}

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

impl<T> DefinedAt for Memo<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for Memo<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Track for Memo<T> {
    fn track(&self) {
        silex_reactivity::track_signal(self.id);
    }
}

impl<T: 'static> WithUntracked for Memo<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_signal_untracked(self.id, fun)
    }
}

impl<T: Clone + PartialEq + 'static> Accessor<T> for Memo<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: 'static> From<Memo<T>> for crate::reactivity::Signal<T> {
    fn from(m: Memo<T>) -> Self {
        crate::reactivity::Signal::Read(crate::reactivity::ReadSignal {
            id: m.id,
            marker: PhantomData,
        })
    }
}
