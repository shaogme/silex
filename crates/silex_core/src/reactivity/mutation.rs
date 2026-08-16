use crate::{
    CompletionSender, ErrorHandlerInput, ErrorReporter, OwnerAccess, SilexError, SilexErrorKind,
    reactivity::{ReadSignal, StoredValue, WriteSignal},
    traits::{RxCloneData, RxData, RxError, RxRead, RxValue},
    unwind_safe,
};
use silex_reactivity::{CallbackInvokeError, ReactiveError};
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

type MutationFuture<'owner, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'owner>>;
type RegularMutationAction<'owner, Arg, T, E> =
    Rc<dyn Fn(Arg) -> MutationFuture<'owner, T, E> + 'owner>;
type PreparedMutationAction<'owner, Arg, T, E> =
    Rc<dyn Fn(Arg) -> Result<MutationFuture<'owner, T, E>, E> + 'owner>;

enum MutationAction<'owner, Arg, T, E> {
    Regular(RegularMutationAction<'owner, Arg, T, E>),
    Prepared(PreparedMutationAction<'owner, Arg, T, E>),
}

impl<'owner, Arg, T, E> Clone for MutationAction<'owner, Arg, T, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Regular(action) => Self::Regular(action.clone()),
            Self::Prepared(action) => Self::Prepared(action.clone()),
        }
    }
}

struct MutationInner<'owner, Arg, T, E> {
    action: MutationAction<'owner, Arg, T, E>,
    last_id: Rc<Cell<usize>>,
    completion: CompletionSender<(usize, Result<T, E>)>,
}

pub struct Mutation<'owner, Arg, T, E = SilexError> {
    pub state: ReadSignal<'owner, MutationState<T, E>>,
    set_state: WriteSignal<'owner, MutationState<T, E>>,
    inner: StoredValue<'owner, MutationInner<'owner, Arg, T, E>>,
    owner: OwnerAccess<'owner>,
    error_handler: ErrorReporter<'owner>,
}

impl<'owner, Arg, T, E> Copy for Mutation<'owner, Arg, T, E> {}

impl<'owner, Arg, T, E> Clone for Mutation<'owner, Arg, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, Arg, T, E> Mutation<'owner, Arg, T, E>
where
    Arg: RxData + 'owner,
    T: RxData + 'static,
    E: RxError + 'static,
{
    pub fn new<F, Fut, H>(
        owner: OwnerAccess<'owner>,
        action: F,
        error_handler: H,
    ) -> crate::SilexResult<Self>
    where
        F: Fn(Arg) -> Fut + 'owner,
        Fut: Future<Output = Result<T, E>> + 'owner,
        H: Clone + ErrorHandlerInput<'owner>,
    {
        let handler_owner = error_handler.clone();
        let error_handler = error_handler.handler_ref();
        owner.on_cleanup(
            move || {
                drop(handler_owner);
                Ok::<(), SilexError>(())
            },
            error_handler,
        )?;
        let (state, set_state) = owner.signal(MutationState::Idle)?;
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion =
            owner.completion_sender(unwind_safe(move |(id, result): (usize, Result<T, E>)| {
                if let Some(next_state) =
                    resolve_mutation_result(last_id_for_callback.get(), id, result)
                {
                    set_state_for_callback.set(next_state)?;
                }
                Ok(())
            }))?;
        let inner = owner.stored(MutationInner {
            action: MutationAction::Regular(Rc::new(move |arg| Box::pin(action(arg)))),
            last_id,
            completion,
        })?;

        Ok(Self {
            state,
            set_state,
            inner,
            owner,
            error_handler,
        })
    }

    /// Create a mutation whose owned future is prepared before `Pending` is
    /// published. Preparation errors become `Error` without starting a task.
    pub fn new_with_prepare<F, Fut, H>(
        owner: OwnerAccess<'owner>,
        prepare: F,
        error_handler: H,
    ) -> crate::SilexResult<Self>
    where
        F: Fn(Arg) -> Result<Fut, E> + 'owner,
        Fut: Future<Output = Result<T, E>> + 'owner,
        H: Clone + ErrorHandlerInput<'owner>,
    {
        let handler_owner = error_handler.clone();
        let error_handler = error_handler.handler_ref();
        owner.on_cleanup(
            move || {
                drop(handler_owner);
                Ok::<(), SilexError>(())
            },
            error_handler,
        )?;
        let (state, set_state) = owner.signal(MutationState::Idle)?;
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion =
            owner.completion_sender(unwind_safe(move |(id, result): (usize, Result<T, E>)| {
                if let Some(next_state) =
                    resolve_mutation_result(last_id_for_callback.get(), id, result)
                {
                    set_state_for_callback.set(next_state)?;
                }
                Ok(())
            }))?;
        let inner = owner.stored(MutationInner {
            action: MutationAction::Prepared(Rc::new(move |arg| {
                prepare(arg).map(|future| Box::pin(future) as MutationFuture<'owner, T, E>)
            })),
            last_id,
            completion,
        })?;

        Ok(Self {
            state,
            set_state,
            inner,
            owner,
            error_handler,
        })
    }

    pub fn mutate(&self, arg: Arg) -> crate::SilexResult<()> {
        if !self.owner.is_active() {
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
        let error_handler = self.error_handler;
        self.owner.spawn_scoped(
            async move {
                match completion.submit((id, future.await)) {
                    Ok(_) => {}
                    Err(CallbackInvokeError::Runtime(error)) => {
                        let _ = error_handler.handle(SilexError::fatal(error));
                    }
                    Err(CallbackInvokeError::User(error)) => {
                        let _ = error_handler.handle(error);
                    }
                    Err(CallbackInvokeError::Handler(error)) => {
                        let _ =
                            error_handler.handle(SilexError::fatal(ReactiveError::Handler(error)));
                    }
                    Err(CallbackInvokeError::Close(error)) => {
                        let _ =
                            error_handler.handle(SilexError::fatal(SilexErrorKind::Close(error)));
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

impl<'owner, Arg, T, E> RxValue for Mutation<'owner, Arg, T, E>
where
    Arg: RxData + 'owner,
    T: RxData + 'owner,
    E: RxError + 'owner,
{
    type Value = Option<T>;
}

impl<'owner, Arg, T, E> RxRead for Mutation<'owner, Arg, T, E>
where
    Arg: RxData + 'owner,
    T: RxCloneData + 'owner,
    E: RxError + 'owner,
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
