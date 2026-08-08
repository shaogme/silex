#![cfg(feature = "bootstrap")]

use silex::bootstrap::{
    AppHost, BootstrapError, HostState, LifecycleReporter, PageController, PageLifecyclePolicy,
    UnmountOutcome,
};

#[test]
fn bootstrap_api_is_exported_under_the_feature_gated_namespace() {
    let _ = std::mem::size_of::<AppHost>();
    let _ = std::mem::size_of::<PageController>();
    let _ = std::mem::size_of::<BootstrapError>();
    let _: Option<LifecycleReporter> = None;
    let _: HostState = HostState::Ready;
    let _: PageLifecyclePolicy = PageLifecyclePolicy::Manual;
    let _: UnmountOutcome = UnmountOutcome::AlreadyUnmounted;
}
