#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    Runtime,
    reactivity::{Mutation, MutationState, Resource, ResourceState, SuspenseContext},
};
use std::{cell::Cell, rc::Rc};

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Clone)]
struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

async fn wait_for_tasks(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

#[wasm_bindgen_test(async)]
async fn resource_enters_loading_and_reloading_states() {
    let mut runtime = Runtime::new();
    runtime.run_scoped(|scope| {
        let (source, set_source) = scope.signal(1u32);
        let suspense = SuspenseContext::new(scope);
        let resource = Resource::new(
            scope,
            source,
            |_| async { Ok::<_, ()>(1u32) },
            Some(suspense),
        );

        assert!(matches!(resource.state.get(), ResourceState::Loading));
        assert_eq!(suspense.count.get(), 1);
        resource.set(1);
        set_source.set(2);
        assert!(matches!(resource.state.get(), ResourceState::Reloading(1)));
        assert_eq!(suspense.count.get(), 2);
    });

    wait_for_tasks(0).await;
}

#[wasm_bindgen_test(async)]
async fn resource_future_completion_is_discarded_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.run_scoped(|scope| {
        let (source, set_source) = scope.signal(1u32);
        let dropped_for_fetcher = dropped.clone();
        let calls_for_fetcher = calls.clone();
        let resource = Resource::new(
            scope,
            source,
            move |_| {
                calls_for_fetcher.set(calls_for_fetcher.get() + 1);
                let dropped = dropped_for_fetcher.clone();
                async move {
                    TimeoutFuture::new(0).await;
                    Ok::<_, ()>(DropProbe(dropped))
                }
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
async fn mutation_future_completion_is_discarded_after_scope_dispose() {
    let dropped = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.run_scoped(|scope| {
        let dropped_for_action = dropped.clone();
        let calls_for_action = calls.clone();
        let mutation = Mutation::new(scope, move |value: u32| {
            calls_for_action.set(calls_for_action.get() + 1);
            let dropped = dropped_for_action.clone();
            async move {
                TimeoutFuture::new(if value == 1 { 10 } else { 0 }).await;
                Ok::<_, ()>(DropProbe(dropped))
            }
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
async fn child_scope_drops_late_resource_completion_without_reactivating_parent() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.run_scoped(|scope| {
        scope.scope(|child| {
            let (source, _) = child.signal(1u32);
            let dropped_for_fetcher = dropped.clone();
            let resource = Resource::new(
                child,
                source,
                move |_| {
                    let dropped = dropped_for_fetcher.clone();
                    async move {
                        TimeoutFuture::new(0).await;
                        Ok::<_, ()>(DropProbe(dropped))
                    }
                },
                None,
            );
            assert!(resource.loading());
        });
    });

    wait_for_tasks(10).await;
    assert_eq!(dropped.get(), 1);
}
