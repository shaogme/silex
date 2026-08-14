use crate::{
    CompletionSender, ErrorReporter, Scope, SilexError, SilexErrorKind,
    reactivity::{ReadSignal, StoredValue, WriteSignal},
    traits::{RxCloneData, RxData, RxError, RxRead, RxValue},
    unwind_safe,
};
use silex_reactivity::CallbackInvokeError;
use std::{cell::Cell, future::Future, pin::Pin, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
pub enum MutationState<T, E> {
    Idle,
    Pending,
    Success(T),
    Error(E),
}

impl<T, E> MutationState<T, E> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Success(value) => Some(value),
            _ => None,
        }
    }
}

fn resolve_mutation_result<T, E>(
    current_id: usize,
    id: usize,
    result: Result<T, E>,
) -> Option<MutationState<T, E>> {
    (current_id == id).then(|| match result {
        Ok(value) => MutationState::Success(value),
        Err(error) => MutationState::Error(error),
    })
}

type MutationFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'static>>;
type RegularMutationAction<'scope, Arg, T, E> = Rc<dyn Fn(Arg) -> MutationFuture<T, E> + 'scope>;
type PreparedMutationAction<'scope, Arg, T, E> =
    Rc<dyn Fn(Arg) -> Result<MutationFuture<T, E>, E> + 'scope>;

enum MutationAction<'scope, Arg, T, E> {
    Regular(RegularMutationAction<'scope, Arg, T, E>),
    Prepared(PreparedMutationAction<'scope, Arg, T, E>),
}

impl<'scope, Arg, T, E> Clone for MutationAction<'scope, Arg, T, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Regular(action) => Self::Regular(action.clone()),
            Self::Prepared(action) => Self::Prepared(action.clone()),
        }
    }
}

struct MutationInner<'scope, Arg, T, E> {
    action: MutationAction<'scope, Arg, T, E>,
    last_id: Rc<Cell<usize>>,
    completion: CompletionSender<(usize, Result<T, E>)>,
}

pub struct Mutation<'scope, Arg, T, E = SilexError> {
    pub state: ReadSignal<'scope, MutationState<T, E>>,
    set_state: WriteSignal<'scope, MutationState<T, E>>,
    inner: StoredValue<'scope, MutationInner<'scope, Arg, T, E>>,
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
}

impl<'scope, Arg, T, E> Copy for Mutation<'scope, Arg, T, E> {}

impl<'scope, Arg, T, E> Clone for Mutation<'scope, Arg, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, Arg, T, E> Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxData + 'static,
    E: RxError + 'static,
{
    pub fn new<F, Fut>(
        scope: Scope<'scope>,
        action: F,
        error_handler: ErrorReporter<'scope>,
    ) -> crate::SilexResult<Self>
    where
        F: Fn(Arg) -> Fut + 'scope,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = scope.signal(MutationState::Idle)?;
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion =
            scope.completion_sender(unwind_safe(move |(id, result): (usize, Result<T, E>)| {
                if let Some(next_state) =
                    resolve_mutation_result(last_id_for_callback.get(), id, result)
                {
                    set_state_for_callback.set(next_state)?;
                }
                Ok(())
            }))?;
        let inner = scope.stored(MutationInner {
            action: MutationAction::Regular(Rc::new(move |arg| Box::pin(action(arg)))),
            last_id,
            completion,
        })?;

        Ok(Self {
            state,
            set_state,
            inner,
            scope,
            error_handler,
        })
    }

    /// Create a mutation whose owned future is prepared before `Pending` is
    /// published. Preparation errors become `Error` without starting a task.
    pub fn new_with_prepare<F, Fut>(
        scope: Scope<'scope>,
        prepare: F,
        error_handler: ErrorReporter<'scope>,
    ) -> crate::SilexResult<Self>
    where
        F: Fn(Arg) -> Result<Fut, E> + 'scope,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = scope.signal(MutationState::Idle)?;
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion =
            scope.completion_sender(unwind_safe(move |(id, result): (usize, Result<T, E>)| {
                if let Some(next_state) =
                    resolve_mutation_result(last_id_for_callback.get(), id, result)
                {
                    set_state_for_callback.set(next_state)?;
                }
                Ok(())
            }))?;
        let inner = scope.stored(MutationInner {
            action: MutationAction::Prepared(Rc::new(move |arg| {
                prepare(arg).map(|future| Box::pin(future) as MutationFuture<T, E>)
            })),
            last_id,
            completion,
        })?;

        Ok(Self {
            state,
            set_state,
            inner,
            scope,
            error_handler,
        })
    }

    pub fn mutate(&self, arg: Arg) -> crate::SilexResult<()> {
        if !self.scope.is_active() {
            return Ok(());
        }

        let (action, last_id, completion) = self.inner.with(|inner| {
            (
                inner.action.clone(),
                inner.last_id.clone(),
                inner.completion.clone(),
            )
        })?;
        let (id, future) = match &action {
            MutationAction::Regular(action) => {
                let id = last_id.get().checked_add(1).ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Framework(
                        "Mutation request id exhausted".into(),
                    ))
                })?;
                last_id.set(id);
                self.set_state.set(MutationState::Pending)?;
                (id, action(arg))
            }
            MutationAction::Prepared(prepare) => {
                let future = match prepare(arg) {
                    Ok(future) => future,
                    Err(error) => {
                        let id = last_id.get().checked_add(1).ok_or_else(|| {
                            SilexError::fatal(SilexErrorKind::Framework(
                                "Mutation request id exhausted".into(),
                            ))
                        })?;
                        last_id.set(id);
                        self.set_state.set(MutationState::Error(error))?;
                        return Ok(());
                    }
                };
                let id = last_id.get().checked_add(1).ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Framework(
                        "Mutation request id exhausted".into(),
                    ))
                })?;
                last_id.set(id);
                self.set_state.set(MutationState::Pending)?;
                (id, future)
            }
        };
        // SAFETY: the completion destination rejects stale submissions before
        // this erased handler can be accessed after owner disposal.
        let error_handler = unsafe {
            std::mem::transmute::<ErrorReporter<'scope>, ErrorReporter<'static>>(self.error_handler)
        };
        self.scope.spawn_scoped(
            async move {
                match completion.submit((id, future.await)) {
                    Ok(_) => {}
                    Err(CallbackInvokeError::Runtime(error)) => {
                        let _ = error_handler.handle(SilexError::fatal(error));
                    }
                    Err(CallbackInvokeError::User(error)) => {
                        let _ = error_handler.handle(error);
                    }
                }
            },
            self.error_handler,
        )?;
        Ok(())
    }

    pub fn mutate_with<S>(&self, source: S) -> crate::SilexResult<()>
    where
        S: RxRead<Value = Arg>,
        Arg: Clone,
    {
        self.mutate(source.with(Clone::clone)?)
    }

    pub fn loading(&self) -> crate::SilexResult<bool> {
        self.state.with(MutationState::is_loading)
    }

    pub fn value(&self) -> crate::SilexResult<Option<T>>
    where
        T: Clone,
    {
        self.state.with(|state| state.value().cloned())
    }

    pub fn error(&self) -> crate::SilexResult<Option<E>>
    where
        E: Clone,
    {
        self.state.with(|state| match state {
            MutationState::Error(error) => Some(error.clone()),
            _ => None,
        })
    }
}

impl<'scope, Arg, T, E> RxValue for Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxData + 'scope,
    E: RxError + 'scope,
{
    type Value = Option<T>;
}

impl<'scope, Arg, T, E> RxRead for Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxCloneData + 'scope,
    E: RxError + 'scope,
{
    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> crate::SilexResult<U> {
        self.state.with(|state| f(&state.value().cloned()))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> crate::SilexResult<U> {
        self.state
            .with_untracked(|state| f(&state.value().cloned()))
    }
}

#[cfg(test)]
mod tests {
    use super::{MutationState, resolve_mutation_result};

    #[test]
    fn stale_mutation_result_does_not_replace_last_request() {
        assert_eq!(resolve_mutation_result(2, 1, Ok::<_, ()>("stale")), None);
        assert_eq!(
            resolve_mutation_result(2, 2, Ok::<_, ()>("current")),
            Some(MutationState::Success("current"))
        );
    }
}
