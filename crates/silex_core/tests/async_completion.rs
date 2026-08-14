#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    ErrorHandler, Runtime, Scope, SilexError,
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

struct ReadyFuture {
    dropped: Rc<Cell<usize>>,
}

impl Future for ReadyFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

impl Drop for ReadyFuture {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

async fn wait_for_tasks(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[wasm_bindgen_test(async)]
async fn resource_enters_loading_and_reloading_states() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1u32).expect("signal should initialize");
            let suspense = SuspenseContext::new(scope).expect("suspense should initialize");
            let resource = Resource::new(
                scope,
                source,
                |_| async { Ok::<_, ()>(1u32) },
                Some(suspense),
                handler(scope),
            )
            .expect("resource should initialize");

            assert!(matches!(
                resource
                    .state
                    .get()
                    .expect("resource state should be readable"),
                ResourceState::Loading
            ));
            assert_eq!(
                suspense
                    .count
                    .get()
                    .expect("suspense count should be readable"),
                1
            );
            resource.set(1).expect("resource value should be writable");
            set_source.set(2).expect("source should be writable");
            assert!(matches!(
                resource
                    .state
                    .get()
                    .expect("resource state should be readable"),
                ResourceState::Reloading(1)
            ));
            assert_eq!(
                suspense
                    .count
                    .get()
                    .expect("suspense count should be readable"),
                1
            );
        })
        .expect("child scope should run");

    wait_for_tasks(0).await;
}

#[wasm_bindgen_test(async)]
async fn resource_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1u32).expect("signal should initialize");
            let dropped_for_fetcher = dropped.clone();
            let calls_for_fetcher = calls.clone();
            let resource = Resource::new(
                scope,
                source,
                move |_| {
                    calls_for_fetcher.set(calls_for_fetcher.get() + 1);
                    PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone())
                },
                None,
                handler(scope),
            )
            .expect("resource should initialize");

            assert!(
                resource
                    .loading()
                    .expect("resource state should be readable")
            );
            set_source.set(2).expect("source should be writable");
            assert!(
                resource
                    .loading()
                    .expect("resource state should be readable")
            );
        })
        .expect("child scope should run");
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

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1u32).expect("signal should initialize");
            let suspense = SuspenseContext::new(scope).expect("suspense should initialize");
            let first_dropped_for_fetcher = first_dropped.clone();
            let second_dropped_for_fetcher = second_dropped.clone();
            let resource = Resource::new(
                scope,
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
                handler(scope),
            )
            .expect("resource should initialize");

            assert_eq!(
                suspense
                    .count
                    .get()
                    .expect("suspense count should be readable"),
                1
            );
            set_source.set(2).expect("source should be writable");
            assert!(matches!(
                resource
                    .state
                    .get()
                    .expect("resource state should be readable"),
                ResourceState::Loading
            ));
            count_after_replacement.set(
                suspense
                    .count
                    .get()
                    .expect("suspense count should be readable"),
            );
        })
        .expect("child scope should run");

    wait_for_tasks(10).await;
    assert_eq!(count_after_replacement.get(), 1);
    assert_eq!(first_dropped.get(), 1);
    assert_eq!(second_dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn resource_scope_capability_survives_async_replacement() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    root.with_scope(|scope| async move {
        let (source, set_source) = scope.signal(1_u32).expect("signal should initialize");
        let resource = Resource::new(
            scope,
            source,
            |value| async move { Ok::<_, ()>(value) },
            None,
            handler(scope),
        )
        .expect("resource should initialize");

        wait_for_tasks(0).await;
        assert!(matches!(
            resource.state.get().expect("resource state should be readable"),
            ResourceState::Ready(value) if value == 1
        ));
        set_source.set(2).expect("source should be writable");
        wait_for_tasks(0).await;
        assert!(matches!(
            resource.state.get().expect("resource state should be readable"),
            ResourceState::Ready(value) if value == 2
        ));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| {
            let dropped_for_action = dropped.clone();
            let calls_for_action = calls.clone();
            let mutation = Mutation::new(
                scope,
                move |value: u32| {
                    calls_for_action.set(calls_for_action.get() + 1);
                    let _ = value;
                    PendingFuture::<Result<u32, ()>>::new(dropped_for_action.clone())
                },
                handler(scope),
            )
            .expect("mutation should initialize");
            let copied = mutation;

            mutation.mutate(1).expect("first mutation should start");
            copied.mutate(2).expect("second mutation should start");
            assert!(matches!(
                mutation
                    .state
                    .get()
                    .expect("mutation state should be readable"),
                MutationState::Pending
            ));
        })
        .expect("child scope should run");
    wait_for_tasks(20).await;
    assert_eq!(calls.get(), 2);
    assert_eq!(dropped.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn mutation_prepare_error_invalidates_previous_completion() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    root.with_scope(|scope| async move {
        let mutation = Mutation::new_with_prepare(
            scope,
            |value: u32| {
                if value == 1 {
                    Ok(async {
                        TimeoutFuture::new(10).await;
                        Ok::<u32, &'static str>(1)
                    })
                } else {
                    Err("prepare failed")
                }
            },
            handler(scope),
        )
        .expect("mutation should initialize");

        mutation.mutate(1).expect("first mutation should start");
        mutation.mutate(2).expect("second mutation should start");
        assert!(matches!(
            mutation
                .state
                .get()
                .expect("mutation state should be readable"),
            MutationState::Error("prepare failed")
        ));

        wait_for_tasks(20).await;
        assert!(matches!(
            mutation
                .state
                .get()
                .expect("mutation state should be readable"),
            MutationState::Error("prepare failed")
        ));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn scoped_task_cancels_and_drops_its_future() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let task = {
        let scope = root.scope();
        scope
            .spawn_scoped(PendingFuture::<()>::new(dropped.clone()), handler(scope))
            .expect("task should start")
    };

    assert!(!task.is_cancelled());
    task.cancel();
    task.cancel();
    assert!(task.is_cancelled());
    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    drop(task);
    root.dispose().expect("root disposal should succeed");
}

#[wasm_bindgen_test(async)]
async fn scope_disposal_drops_scoped_task_future_immediately() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            scope
                .spawn_scoped(PendingFuture::<()>::new(dropped.clone()), handler(scope))
                .expect("task should start");
        })
        .expect("child scope should run");

    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn owned_scope_disposal_drops_scoped_task_future_immediately() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let owned = root
        .scope()
        .owned_scope()
        .expect("owned scope should start");
    let task = owned
        .spawn_scoped(
            PendingFuture::<()>::new(dropped.clone()),
            handler(root.scope()),
        )
        .expect("task should start");

    owned
        .dispose()
        .expect("owned scope disposal should succeed");
    assert_eq!(dropped.get(), 1);
    assert!(task.is_cancelled());
    drop(task);
    drop(owned);
    root.dispose().expect("root disposal should succeed");
}

#[wasm_bindgen_test(async)]
async fn completed_scoped_task_drops_its_future_once() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let task = root
        .scope()
        .spawn_scoped(
            ReadyFuture {
                dropped: dropped.clone(),
            },
            handler(root.scope()),
        )
        .expect("task should start");

    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    assert!(!task.is_cancelled());
    drop(task);
    root.dispose().expect("root disposal should succeed");
    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn child_scope_cancels_resource_without_reactivating_parent() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| {
            scope
                .child(|child| {
                    let (source, _) = child.signal(1u32).expect("signal should initialize");
                    let dropped_for_fetcher = dropped.clone();
                    let resource = Resource::new(
                        child,
                        source,
                        move |_| PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone()),
                        None,
                        handler(child),
                    )
                    .expect("resource should initialize");
                    assert!(
                        resource
                            .loading()
                            .expect("resource state should be readable")
                    );
                })
                .expect("child scope should run");
        })
        .expect("root child scope should run");

    wait_for_tasks(10).await;
    assert_eq!(dropped.get(), 1);
}
