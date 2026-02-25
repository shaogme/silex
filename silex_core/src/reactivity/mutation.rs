use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::SilexError;
use crate::reactivity::signal::{ReadSignal, WriteSignal, signal};
use crate::reactivity::stored_value::StoredValue;
use crate::traits::*;
use crate::traits::{RxCloneData, RxData};
use std::panic::Location;

// --- MutationState ---

#[derive(Clone, Debug, PartialEq)]
pub enum MutationState<T, E> {
    /// No mutation has been triggered yet.
    Idle,
    /// A mutation is currently in progress.
    Pending,
    /// The last mutation completed successfully.
    Success(T),
    /// The last mutation failed.
    Error(E),
}

impl<T, E> MutationState<T, E> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Success(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_option(&self) -> Option<&T> {
        self.value()
    }
}

// --- Mutation ---
type MutationFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

struct MutationInner<Arg, T, E> {
    // 使用 Rc 而非 Box，以便我们可以克隆 action 并提取出 `mutate` 作用域，
    // 从而避免在执行用户提供的 `f` 时发生 RefCell 重入 panic（如果 `f` 内部也访问了 StoredValue）。
    action: Rc<dyn Fn(Arg) -> MutationFuture<T, E>>,
    last_id: Cell<usize>,
}

pub struct Mutation<Arg, T, E = SilexError> {
    pub state: ReadSignal<MutationState<T, E>>,
    set_state: WriteSignal<MutationState<T, E>>,
    // Use StoredValue to hold the closure and ID, making Mutation pure Copy
    inner: StoredValue<MutationInner<Arg, T, E>>,
}

impl<Arg, T, E> Clone for Mutation<Arg, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Arg, T, E> Copy for Mutation<Arg, T, E> {}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> Mutation<Arg, T, E> {
    /// Create a new Mutation with the given async handler.
    ///
    /// The handler `f` takes an argument `Arg` and returns a Future resolving to `Result<T, E>`.
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(Arg) -> Fut + 'static,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = signal(MutationState::Idle);

        // Wrap the user provided future in a Box to erase the type.
        let action = Rc::new(move |arg| Box::pin(f(arg)) as MutationFuture<T, E>);

        let inner_val = MutationInner {
            action,
            last_id: Cell::new(0),
        };

        let inner = StoredValue::new(inner_val);

        Self {
            state,
            set_state,
            inner,
        }
    }

    /// Trigger the mutation with the given argument.
    ///
    /// This will update the state to `Pending`, execute the future,
    /// and then update to `Success` or `Error`.
    /// If `mutate` is called again while a request is pending, the previous request's
    /// result will be ignored (last-one-wins).
    pub fn mutate(&self, arg: Arg) {
        // Increment ID and set pending state
        let (current_id, action) = match self.inner.try_with_untracked(|inner| {
            let next_id = inner.last_id.get().wrapping_add(1);
            inner.last_id.set(next_id);
            (next_id, inner.action.clone())
        }) {
            Some(v) => v,
            None => {
                crate::warn!("Mutation triggered after disposal");
                return;
            }
        };

        self.set_state.set(MutationState::Pending);

        // Execute action outside of StoredValue borrow lock to avoid panic
        // if the user's function tries to access other StoredValues.
        let future = action(arg);

        // Spawn
        let set_state = self.set_state;
        let inner_handle = self.inner;

        wasm_bindgen_futures::spawn_local(async move {
            let result = future.await;

            // Check ID
            let is_latest = inner_handle
                .try_with_untracked(|inner| inner.last_id.get() == current_id)
                .unwrap_or(false);

            if is_latest {
                set_state.update(|s| {
                    *s = match result {
                        Ok(data) => MutationState::Success(data),
                        Err(err) => MutationState::Error(err),
                    };
                });
            }
        });
    }

    pub fn mutate_with<A>(&self, arg_accessor: A)
    where
        A: RxRead<Value = Arg>,
        Arg: Clone,
    {
        self.mutate(arg_accessor.with(Clone::clone));
    }

    /// Helper to check if the mutation is currently `Pending`.
    pub fn loading(&self) -> bool {
        self.state.with(|s: &MutationState<T, E>| s.is_loading())
    }

    /// Helper to get the last successful value, if any.
    pub fn value(&self) -> Option<T> {
        self.state
            .with(|s: &MutationState<T, E>| s.value().cloned())
    }

    /// Helper to get the last error, if any.
    pub fn error(&self) -> Option<E> {
        self.state.with(|s: &MutationState<T, E>| match s {
            MutationState::Error(e) => Some(e.clone()),
            _ => None,
        })
    }
}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> RxValue for Mutation<Arg, T, E> {
    type Value = Option<T>;
}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> RxBase for Mutation<Arg, T, E> {
    #[inline(always)]
    fn id(&self) -> Option<crate::reactivity::NodeId> {
        self.state.id()
    }

    #[inline(always)]
    fn track(&self) {
        self.state.track();
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.state.is_disposed()
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.state.defined_at()
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.state.debug_name()
    }
}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> RxInternal for Mutation<Arg, T, E> {
    type ReadOutput<'a>
        = RxGuard<'a, Option<T>, Option<T>>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.state
            .try_with_untracked(|s| RxGuard::Owned(s.value().cloned()))
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with_untracked(|s: &MutationState<T, E>| {
            let val = s.value().cloned();
            fun(&val)
        })
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::AdaptiveWrapper;
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        false
    }
}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> IntoRx for Mutation<Arg, T, E> {
    type RxType = crate::Rx<Self, crate::RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        crate::Rx(self, std::marker::PhantomData)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        false
    }
}

impl<Arg: RxData, T: RxCloneData, E: RxCloneData> crate::traits::IntoSignal
    for Mutation<Arg, T, E>
{
    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<Option<T>>
    where
        Self: 'static,
        T: Clone,
    {
        crate::reactivity::Signal::derive(move || self.get())
    }
}
