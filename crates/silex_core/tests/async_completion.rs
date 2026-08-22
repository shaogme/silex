#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    ErrorHandlerToken, OwnerAccess, Runtime,
    reactivity::{Mutation, MutationState, Resource, ResourceState, SuspenseContext},
    traits::RxGet,
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

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[wasm_bindgen_test(async)]
async fn resource_enters_loading_and_reloading_states() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let source = owner.signal(1u32).expect("signal should initialize");
            let suspense = SuspenseContext::new(owner).expect("suspense should initialize");
            let resource = Resource::builder(owner)
                .source(source)
                .fetch(|_| async { Ok::<_, ()>(1u32) })
                .suspense(suspense)
                .build(handler(owner))
                .expect("resource should initialize");

            assert!(matches!(
                resource
                    .state()
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
            source.set(2).expect("source should be writable");
            assert!(matches!(
                resource
                    .state()
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
        .expect("child owner should run");

    wait_for_tasks(0).await;
}

#[wasm_bindgen_test(async)]
async fn resource_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let source = owner.signal(1u32).expect("signal should initialize");
            let dropped_for_fetcher = dropped.clone();
            let calls_for_fetcher = calls.clone();
            let resource = Resource::builder(owner)
                .source(source)
                .fetch(move |_| {
                    calls_for_fetcher.set(calls_for_fetcher.get() + 1);
                    PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone())
                })
                .build(handler(owner))
                .expect("resource should initialize");

            assert!(
                resource
                    .loading()
                    .expect("resource state should be readable")
            );
            source.set(2).expect("source should be writable");
            assert!(
                resource
                    .loading()
                    .expect("resource state should be readable")
            );
        })
        .expect("child owner should run");
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
        .with_transient(|owner| {
            let source = owner.signal(1u32).expect("signal should initialize");
            let suspense = SuspenseContext::new(owner).expect("suspense should initialize");
            let first_dropped_for_fetcher = first_dropped.clone();
            let second_dropped_for_fetcher = second_dropped.clone();
            let resource = Resource::builder(owner)
                .source(source)
                .fetch(move |value| {
                    if value == 1 {
                        Box::pin(PendingFuture::<Result<u32, ()>>::new(
                            first_dropped_for_fetcher.clone(),
                        )) as Pin<Box<dyn Future<Output = Result<u32, ()>>>>
                    } else {
                        Box::pin(PendingFuture::<Result<u32, ()>>::new(
                            second_dropped_for_fetcher.clone(),
                        )) as Pin<Box<dyn Future<Output = Result<u32, ()>>>>
                    }
                })
                .suspense(suspense)
                .build(handler(owner))
                .expect("resource should initialize");

            assert_eq!(
                suspense
                    .count
                    .get()
                    .expect("suspense count should be readable"),
                1
            );
            source.set(2).expect("source should be writable");
            assert!(matches!(
                resource
                    .state()
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
        .expect("child owner should run");

    wait_for_tasks(10).await;
    assert_eq!(count_after_replacement.get(), 1);
    assert_eq!(first_dropped.get(), 1);
    assert_eq!(second_dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn resource_scope_capability_survives_async_replacement() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    root.with_access_async(|owner| {
        Box::pin(async move {
            let source = owner.signal(1_u32).expect("signal should initialize");
            let resource = Resource::builder(owner)
                .source(source)
                .fetch(|value| async move { Ok::<_, ()>(value) })
                .build(handler(owner))
                .expect("resource should initialize");

            wait_for_tasks(0).await;
            assert!(matches!(
                resource
                    .state()
                    .get()
                    .expect("resource state should be readable"),
                ResourceState::Ready(value) if value == 1
            ));
            source.set(2).expect("source should be writable");
            wait_for_tasks(0).await;
            assert!(matches!(
                resource
                    .state()
                    .get()
                    .expect("resource state should be readable"),
                ResourceState::Ready(value) if value == 2
            ));
        })
    })
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_future_is_cancelled_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let dropped_for_action = dropped.clone();
            let calls_for_action = calls.clone();
            let mutation = Mutation::new(
                owner,
                move |value: u32| {
                    calls_for_action.set(calls_for_action.get() + 1);
                    let _ = value;
                    PendingFuture::<Result<u32, ()>>::new(dropped_for_action.clone())
                },
                handler(owner),
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
        .expect("child owner should run");
    wait_for_tasks(20).await;
    assert_eq!(calls.get(), 2);
    assert_eq!(dropped.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn mutation_prepare_error_invalidates_previous_completion() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    root.with_access_async(|owner| {
        Box::pin(async move {
            let mutation = Mutation::new_with_prepare(
                owner,
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
                handler(owner),
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
    })
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn scoped_task_cancels_and_drops_its_future() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let owner = root.access();
    let task = owner
        .spawn_scoped(PendingFuture::<()>::new(dropped.clone()), handler(owner))
        .expect("task should start");

    assert!(!task.is_cancelled());
    task.cancel();
    task.cancel();
    assert!(task.is_cancelled());
    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    drop(task);
    root.close().expect("root disposal should succeed");
}

#[wasm_bindgen_test(async)]
async fn scope_disposal_drops_scoped_task_future_immediately() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            owner
                .spawn_scoped(PendingFuture::<()>::new(dropped.clone()), handler(owner))
                .expect("task should start");
        })
        .expect("child owner should run");

    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn owned_scope_disposal_drops_scoped_task_future_immediately() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let owned = root.create_child().expect("owned owner should start");
    let owner = owned.access();
    let task = owner
        .spawn_scoped(PendingFuture::<()>::new(dropped.clone()), handler(owner))
        .expect("task should start");

    owned.close().expect("owned owner disposal should succeed");
    assert_eq!(dropped.get(), 1);
    assert!(task.is_cancelled());
    drop(task);
    drop(owned);
    root.close().expect("root disposal should succeed");
}

#[wasm_bindgen_test(async)]
async fn completed_scoped_task_drops_its_future_once() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let owner = root.access();
    let task = owner
        .spawn_scoped(
            ReadyFuture {
                dropped: dropped.clone(),
            },
            handler(owner),
        )
        .expect("task should start");

    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    assert!(!task.is_cancelled());
    drop(task);
    root.close().expect("root disposal should succeed");
    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn child_scope_cancels_resource_without_reactivating_parent() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            owner
                .with_transient(|child| {
                    let source = child.signal(1u32).expect("signal should initialize");
                    let dropped_for_fetcher = dropped.clone();
                    let resource = Resource::builder(child)
                        .source(source)
                        .fetch(move |_| {
                            PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone())
                        })
                        .build(handler(child))
                        .expect("resource should initialize");
                    assert!(
                        resource
                            .loading()
                            .expect("resource state should be readable")
                    );
                })
                .expect("child owner should run");
        })
        .expect("root child owner should run");

    wait_for_tasks(10).await;
    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn resource_copy_handles_do_not_own_the_child_scope() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let owner = root.access();
    let source = owner.signal(1_u32).expect("source should initialize");
    let dropped_for_fetcher = dropped.clone();
    let resource = Resource::builder(owner)
        .source(source)
        .fetch(move |_| PendingFuture::<Result<u32, ()>>::new(dropped_for_fetcher.clone()))
        .build(handler(owner))
        .expect("resource should initialize");
    {
        let first = resource;
        let second = resource;
        let third = first.clone();
        assert!(first.loading().expect("first handle should be readable"));
        assert!(second.loading().expect("second handle should be readable"));
        assert!(third.loading().expect("third handle should be readable"));
        let _ = (first, second, third);
    }
    assert!(
        resource
            .loading()
            .expect("resource should outlive all copied handles")
    );
    assert_eq!(dropped.get(), 0);

    root.close().expect("owner close should reclaim resource");
    wait_for_tasks(0).await;
    assert_eq!(dropped.get(), 1);
    assert!(resource.state().get().is_err());
    assert!(resource.refetch().is_err());
    assert!(resource.loading().is_err());
}
