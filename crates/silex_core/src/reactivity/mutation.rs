use crate::{
    Rx, Scope, SilexError,
    reactivity::{ReadSignal, Signal, WriteSignal},
    traits::{IntoRx, IntoSignal, RxBase, RxCloneData, RxData, RxError, RxRead, RxValue},
};
use silex_reactivity::CompletionToken;
use std::{cell::Cell, future::Future, pin::Pin, rc::Rc};
use wasm_bindgen_futures::spawn_local;

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

pub struct Mutation<'scope, 'run, Arg, T, E = SilexError> {
    pub state: ReadSignal<'scope, 'run, MutationState<T, E>>,
    set_state: WriteSignal<'scope, 'run, MutationState<T, E>>,
    action: Rc<dyn Fn(Arg) -> MutationFuture<T, E> + 'scope>,
    last_id: Rc<Cell<usize>>,
    completion: CompletionToken<(usize, Result<T, E>)>,
}

impl<'scope, 'run, Arg, T, E> Clone for Mutation<'scope, 'run, Arg, T, E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            set_state: self.set_state,
            action: self.action.clone(),
            last_id: self.last_id.clone(),
            completion: self.completion.clone(),
        }
    }
}

impl<'scope, 'run, Arg, T, E> Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData,
    T: RxData + 'static,
    E: RxError + 'static,
{
    pub fn new<F, Fut>(scope: &Scope<'scope, 'run>, action: F) -> Self
    where
        F: Fn(Arg) -> Fut + 'scope,
        Fut: Future<Output = Result<T, E>> + 'static,
    {
        let (state, set_state) = scope.signal(MutationState::Idle);
        let last_id = Rc::new(Cell::new(0usize));
        let last_id_for_callback = last_id.clone();
        let set_state_for_callback = set_state;
        let completion = scope.completion_scoped(move |(id, result): (usize, Result<T, E>)| {
            if let Some(next_state) =
                resolve_mutation_result(last_id_for_callback.get(), id, result)
            {
                set_state_for_callback.set(next_state);
            }
        });

        Self {
            state,
            set_state,
            action: Rc::new(move |arg| Box::pin(action(arg))),
            last_id,
            completion,
        }
    }

    pub fn mutate(&self, arg: Arg) {
        let id = self
            .last_id
            .get()
            .checked_add(1)
            .expect("Mutation request id exhausted");
        self.last_id.set(id);
        self.set_state.set(MutationState::Pending);
        let future = (self.action)(arg);
        let completion = self.completion.clone();
        spawn_local(async move {
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

impl<'scope, 'run, Arg, T, E> RxValue for Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData + 'run,
    T: RxData + 'run,
    E: RxError + 'run,
{
    type Value = Option<T>;
}

impl<'scope, 'run, Arg, T, E> RxBase for Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData + 'run,
    T: RxData + 'run,
    E: RxError + 'run,
{
    fn track(&self) {
        self.state.track();
    }

    fn is_alive(&self) -> bool {
        self.state.is_alive()
    }
}

impl<'scope, 'run, Arg, T, E> RxRead for Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData + 'run,
    T: RxCloneData + 'run,
    E: RxError + 'run,
{
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with(|state| f(&state.value().cloned()))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state
            .try_with_untracked(|state| f(&state.value().cloned()))
    }
}

impl<'scope, 'run, Arg, T, E> IntoRx<'scope, 'run> for Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData + 'run,
    T: RxCloneData + 'static,
    E: RxError + 'static,
{
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Option<T>> {
        let scope = *scope;
        scope.derived(move || self.value())
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, Arg, T, E> IntoSignal<'scope, 'run> for Mutation<'scope, 'run, Arg, T, E>
where
    Arg: RxData + 'run,
    T: RxCloneData + 'static,
    E: RxError + 'static,
{
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, Option<T>> {
        self.into_rx(scope).into_signal(scope)
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
