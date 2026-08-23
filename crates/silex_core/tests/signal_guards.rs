#[cfg(target_arch = "wasm32")]
use silex_core::Resource;
use silex_core::reactivity::MutationState;
use silex_core::traits::{RxRead, RxWrite};
use silex_core::{
    Constant, ErrorHandlerToken, Mutation, OwnerAccess, ReactiveError, Runtime, RxGet,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn core_signal_guards_expose_direct_payload_access() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal((1_i32, 2_i32)).expect("signal creation");
            let read = signal.read().expect("tracked read guard");
            assert_eq!(read.0, 1);
            read.finish().expect("read finish");

            let mut write = signal.write().expect("write guard");
            write.0 = 3;
            write.commit().expect("write commit");
            assert_eq!(signal.get().expect("signal read"), (3, 2));

            let second = scope.signal((4_i32, 5_i32)).expect("second signal");
            let mut direct_write = second.write().expect("second write guard");
            direct_write.1 = 6;
            direct_write.commit().expect("direct write commit");
        })
        .expect("runtime scope");
}

#[test]
fn core_read_guards_cover_computed_stored_rx_and_constant_sources() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("source signal");
            let computed = scope
                .computed(|| source.get().map(|value| value + 1), handler(scope))
                .expect("computed source");
            let computed_guard = computed.read().expect("computed guard");
            assert_eq!(*computed_guard, 2);
            computed_guard.finish().expect("computed finish");

            let stored = scope.stored(3_i32).expect("stored source");
            let stored_guard = stored.read().expect("stored guard");
            assert_eq!(*stored_guard, 3);
            stored_guard.finish().expect("stored finish");
            let mut stored_write = stored.write().expect("stored write guard");
            *stored_write = 4;
            stored_write.commit().expect("stored write commit");

            let rx = source.into_rx();
            let rx_guard = rx.read_untracked().expect("rx guard");
            assert_eq!(*rx_guard, 1);
            rx_guard.finish().expect("rx finish");

            let constant = Constant::new(5_i32);
            let constant_guard = constant.read().expect("constant guard");
            assert_eq!(*constant_guard, 5);
        })
        .expect("runtime scope");
}

#[test]
fn core_guard_reads_preserve_tracking_and_slice_mapping() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let source = scope.signal((1_i32, 2_i32)).expect("source signal");
            let slice = source.slice(|value| &value.1);
            let slice_guard = slice.read().expect("slice guard");
            assert_eq!(*slice_guard, 2);
            assert!(source.write_signal().write().is_err());
            drop(slice_guard);
            source.set((3, 4)).expect("source update");
        })
        .expect("runtime scope");
}

#[test]
fn owned_guards_snapshot_resource_mutation_and_tuple_values() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let mutation = Mutation::new(scope, |_: ()| async { Ok::<i32, ()>(1) }, handler(scope))
                .expect("mutation creation");
            mutation
                .state
                .set(MutationState::Success(9))
                .expect("mutation value");
            let mutation_guard = mutation.read().expect("mutation guard");
            assert_eq!(*mutation_guard, Some(9));
            mutation
                .state
                .set(MutationState::Success(10))
                .expect("mutation update");
            assert_eq!(*mutation_guard, Some(9));

            let left = scope.signal(1_i32).expect("left signal");
            let right = scope.signal(2_i32).expect("right signal");
            let tuple = (left, right);
            let tuple_guard = tuple.read().expect("tuple guard");
            left.set(3).expect("left update");
            assert_eq!(*tuple_guard, (1, 2));
        })
        .expect("runtime scope");
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test(async)]
async fn resource_read_guards_snapshot_values() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("source signal");
            let resource = Resource::builder(scope)
                .source(source)
                .fetch(|_| async { Ok::<i32, ()>(1) })
                .build(handler(scope))
                .expect("resource creation");
            resource.set(7).expect("resource value");
            let resource_guard = resource.read().expect("resource guard");
            assert_eq!(*resource_guard, Some(7));
            resource.set(8).expect("resource update");
            assert_eq!(*resource_guard, Some(7));
        })
        .expect("runtime scope");
}

#[allow(dead_code)]
fn _error_type_is_still_reactive() -> Result<(), ReactiveError> {
    Ok(())
}
