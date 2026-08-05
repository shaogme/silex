use crate::{
    Scope, SilexError,
    reactivity::{ReadSignal, WriteSignal},
    traits::{RxBase, RxCloneData, RxData, RxError, RxRead, RxValue},
};
use silex_reactivity::CompletionSender;
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

pub struct Mutation<'scope, Arg, T, E = SilexError> {
    pub state: ReadSignal<'scope, MutationState<T, E>>,
    set_state: WriteSignal<'scope, MutationState<T, E>>,
    action: MutationAction<'scope, Arg, T, E>,
    last_id: Rc<Cell<usize>>,
    completion: CompletionSender<(usize, Result<T, E>)>,
    scope: Scope<'scope>,
}

impl<'scope, Arg, T, E> Clone for Mutation<'scope, Arg, T, E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            set_state: self.set_state,
            action: self.action.clone(),
            last_id: self.last_id.clone(),
            completion: self.completion.clone(),
            scope: self.scope,
        }
    }
}

impl<'scope, Arg, T, E> Mutation<'scope, Arg, T, E>
where
    Arg: RxData,
    T: RxData + 'static,
    E: RxError + 'static,
{
    pub fn new<F, Fut>(scope: Scope<'scope>, action: F) -> Self
    where
        F: Fn(Arg) -> Fut + 'scope,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = scope.signal(MutationState::Idle);
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion = scope.completion_sender(move |(id, result): (usize, Result<T, E>)| {
            if let Some(next_state) =
                resolve_mutation_result(last_id_for_callback.get(), id, result)
            {
                set_state_for_callback.set(next_state);
            }
        });

        Self {
            state,
            set_state,
            action: MutationAction::Regular(Rc::new(move |arg| Box::pin(action(arg)))),
            last_id,
            completion,
            scope,
        }
    }

    /// Create a mutation whose owned future is prepared before `Pending` is
    /// published. Preparation errors become `Error` without starting a task.
    pub fn new_with_prepare<F, Fut>(scope: Scope<'scope>, prepare: F) -> Self
    where
        F: Fn(Arg) -> Result<Fut, E> + 'scope,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = scope.signal(MutationState::Idle);
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion = scope.completion_sender(move |(id, result): (usize, Result<T, E>)| {
            if let Some(next_state) =
                resolve_mutation_result(last_id_for_callback.get(), id, result)
            {
                set_state_for_callback.set(next_state);
            }
        });

        Self {
            state,
            set_state,
            action: MutationAction::Prepared(Rc::new(move |arg| {
                prepare(arg).map(|future| Box::pin(future) as MutationFuture<T, E>)
            })),
            last_id,
            completion,
            scope,
        }
    }

    pub fn mutate(&self, arg: Arg) {
        if !self.scope.is_active() {
            return;
        }

        let (id, future) = match &self.action {
            MutationAction::Regular(action) => {
                let id = self
                    .last_id
                    .get()
                    .checked_add(1)
                    .expect("Mutation request id exhausted");
                self.last_id.set(id);
                self.set_state.set(MutationState::Pending);
                (id, action(arg))
            }
            MutationAction::Prepared(prepare) => {
                let future = match prepare(arg) {
                    Ok(future) => future,
                    Err(error) => {
                        let id = self
                            .last_id
                            .get()
                            .checked_add(1)
                            .expect("Mutation request id exhausted");
                        self.last_id.set(id);
                        self.set_state.set(MutationState::Error(error));
                        return;
                    }
                };
                let id = self
                    .last_id
                    .get()
                    .checked_add(1)
                    .expect("Mutation request id exhausted");
                self.last_id.set(id);
                self.set_state.set(MutationState::Pending);
                (id, future)
            }
        };
        let completion = self.completion.clone();
        self.scope.spawn_scoped(async move {
            let _ = completion.submit((id, future.await));
        });
    }

    pub fn mutate_with<S>(&self, source: S)
    where
        S: RxRead<Value = Arg>,
        Arg: Clone,
    {
        self.mutate(source.with(Clone::clone));
    }

    pub fn loading(&self) -> bool {
        self.state.with(MutationState::is_loading)
    }

    pub fn value(&self) -> Option<T>
    where
        T: Clone,
    {
        self.state.with(|state| state.value().cloned())
    }

    pub fn error(&self) -> Option<E>
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

impl<'scope, Arg, T, E> RxBase for Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxData + 'scope,
    E: RxError + 'scope,
{
    fn track(&self) {
        self.state.track();
    }

    fn is_alive(&self) -> bool {
        self.state.is_alive()
    }
}

impl<'scope, Arg, T, E> RxRead for Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxCloneData + 'scope,
    E: RxError + 'scope,
{
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with(|state| f(&state.value().cloned()))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state
            .try_with_untracked(|state| f(&state.value().cloned()))
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
