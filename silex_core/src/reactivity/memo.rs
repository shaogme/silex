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

impl<T> Memo<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }
}

impl<T> DefinedAt for Memo<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn debug_name(&self) -> Option<String> {
        silex_reactivity::get_debug_label(self.id)
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

impl<T: Clone + PartialEq + 'static> GetUntracked for Memo<T> {
    type Value = T;
    fn try_get_untracked(&self) -> Option<T> {
        self.try_with_untracked(Clone::clone)
    }
}

impl<T: Clone + PartialEq + 'static> Get for Memo<T> {
    type Value = T;
    fn try_get(&self) -> Option<T> {
        self.try_with(Clone::clone)
    }
}

impl<T: Clone + PartialEq + 'static> Map for Memo<T> {
    type Value = T;

    fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(Self::Value) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| f(self.get()))
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

use crate::impl_reactive_ops;
impl_reactive_ops!(Memo);
