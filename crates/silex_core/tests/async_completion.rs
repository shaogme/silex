#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    Runtime,
    reactivity::{Mutation, MutationState, Resource, ResourceState, SuspenseContext},
};
use std::{
    cell::Cell,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct PendingFuture<T> {
    dropped: Rc<Cell<usize>>,
    marker: PhantomData<fn() -> T>,
}

impl<T> PendingFuture<T> {
    fn new(dropped: Rc<Cell<usize>>) -> Self {
        Self {
            dropped,
            marker: PhantomData,
        }
    }
}

impl<T> Future for PendingFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl<T> Drop for PendingFuture<T> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

async fn wait_for_tasks(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

#[wasm_bindgen_test(async)]
async fn resource_enters_loading_and_reloading_states() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1u32);
        let suspense = SuspenseContext::new(&scope);
        let resource = Resource::new(
            &scope,
            source,
            |_| async { Ok::<_, ()>(1u32) },
            Some(suspense),
        );

        assert!(matches!(resource.state.get(), ResourceState::Loading));
        assert_eq!(suspense.count.get(), 1);
        resource.set(1);
        set_source.set(2);
        assert!(matches!(resource.state.get(), ResourceState::Reloading(1)));
        assert_eq!(suspense.count.get(), 1);
    });

    wait_for_tasks(0).await;
}

#[wasm_bindgen_test(async)]
async fn resource_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1u32);
        let dropped_for_fetcher = dropped.clone();
        let calls_for_fetcher = calls.clone();
        let resource = Resource::new(
            &scope,
            source,
            move |_| {
                calls_for_fetcher.set(calls_for_fetcher.get() + 1);
                PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone())
            },
            None,
        );

        assert!(resource.loading());
        set_source.set(2);
        assert!(resource.loading());
    });
    wait_for_tasks(10).await;
    assert_eq!(calls.get(), 2);
    assert_eq!(dropped.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn resource_replacement_keeps_only_the_new_suspense_request() {
    let first_dropped = Rc::new(Cell::new(0));
    let second_dropped = Rc::new(Cell::new(0));
    let count_after_replacement = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1u32);
        let suspense = SuspenseContext::new(&scope);
        let first_dropped_for_fetcher = first_dropped.clone();
        let second_dropped_for_fetcher = second_dropped.clone();
        let resource = Resource::new(
            &scope,
            source,
            move |value| {
                if value == 1 {
                    Box::pin(PendingFuture::<Result<u32, ()>>::new(
                        first_dropped_for_fetcher.clone(),
                    )) as Pin<Box<dyn Future<Output = Result<u32, ()>>>>
                } else {
                    Box::pin(PendingFuture::<Result<u32, ()>>::new(
                        second_dropped_for_fetcher.clone(),
                    )) as Pin<Box<dyn Future<Output = Result<u32, ()>>>>
                }
            },
            Some(suspense),
        );

        assert_eq!(suspense.count.get(), 1);
        set_source.set(2);
        assert!(matches!(resource.state.get(), ResourceState::Loading));
        count_after_replacement.set(suspense.count.get());
    });

    wait_for_tasks(10).await;
    assert_eq!(count_after_replacement.get(), 1);
    assert_eq!(first_dropped.get(), 1);
    assert_eq!(second_dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn mutation_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let dropped_for_action = dropped.clone();
        let calls_for_action = calls.clone();
        let mutation = Mutation::new(&scope, move |value: u32| {
            calls_for_action.set(calls_for_action.get() + 1);
            let _ = value;
            PendingFuture::<Result<u32, ()>>::new(dropped_for_action.clone())
        });

        mutation.mutate(1);
        mutation.mutate(2);
        assert!(matches!(mutation.state.get(), MutationState::Pending));
    });
    wait_for_tasks(20).await;
    assert_eq!(calls.get(), 2);
    assert_eq!(dropped.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn scoped_task_cancels_and_drops_its_future() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let mut root = runtime.run(|_| {});
    let task = root
        .scope()
        .spawn_scoped(PendingFuture::<()>::new(dropped.clone()));

    assert!(!task.is_cancelled());
    task.cancel();
    task.cancel();
    assert!(task.is_cancelled());
    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    root.dispose().expect("root disposal should succeed");
}

#[wasm_bindgen_test(async)]
async fn child_scope_cancels_resource_without_reactivating_parent() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        scope.child(|child| {
            let (source, _) = child.signal(1u32);
            let dropped_for_fetcher = dropped.clone();
            let resource = Resource::new(
                &child,
                source,
                move |_| PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone()),
                None,
            );
            assert!(resource.loading());
        });
    });

    wait_for_tasks(10).await;
    assert_eq!(dropped.get(), 1);
}
