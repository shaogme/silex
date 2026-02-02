use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::SilexError;
use crate::reactivity::signal::{ReadSignal, WriteSignal, signal};
use crate::reactivity::stored_value::StoredValue;
use crate::traits::*;

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

struct MutationInner<Arg, T, E> {
    // 使用 Rc 而非 Box，以便我们可以克隆 action 并提取出 `mutate` 作用域，
    // 从而避免在执行用户提供的 `f` 时发生 RefCell 重入 panic（如果 `f` 内部也访问了 StoredValue）。
    action: Rc<dyn Fn(Arg) -> Pin<Box<dyn Future<Output = Result<T, E>>>>>,
    last_id: Cell<usize>,
}

pub struct Mutation<Arg, T, E = SilexError>
where
    Arg: 'static,
    T: 'static,
    E: 'static,
{
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

impl<Arg: 'static, T: Clone + 'static, E: Clone + 'static> Mutation<Arg, T, E> {
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
        let action = Rc::new(move |arg| {
            let fut = f(arg);
            Box::pin(async move { fut.await }) as Pin<Box<dyn Future<Output = Result<T, E>>>>
        });

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
        let (current_id, action) = match self.inner.try_with_value(|inner| {
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
                .try_with_value(|inner| inner.last_id.get() == current_id)
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

    /// Helper to check if the mutation is currently `Pending`.
    pub fn loading(&self) -> bool {
        self.state.with(|s| s.is_loading())
    }

    /// Helper to get the last successful value, if any.
    pub fn value(&self) -> Option<T> {
        self.state.with(|s| s.value().cloned())
    }

    /// Helper to get the last error, if any.
    pub fn error(&self) -> Option<E> {
        self.state.with(|s| match s {
            MutationState::Error(e) => Some(e.clone()),
            _ => None,
        })
    }
}
