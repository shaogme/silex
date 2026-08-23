#[cfg(feature = "test-support")]
use silex_core::reactivity::{ReadSignal, Resource, SuspenseContext};
#[cfg(feature = "test-support")]
use silex_core::traits::{RxBase, RxRead, RxReadRefSource, RxValue};
use silex_core::{
    EffectPhase, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime, SilexError,
    SilexErrorKind, traits::RxGet,
};
#[cfg(feature = "test-support")]
use silex_core::{PromotionPlan, ReactiveSource, RuntimeScoped};
use std::cell::Cell;
use std::rc::Rc;

#[cfg(feature = "test-support")]
#[derive(Clone)]
struct FailingSource<'owner> {
    delegate: ReadSignal<'owner, u32>,
}

#[cfg(feature = "test-support")]
impl<'owner> RxValue for FailingSource<'owner> {
    type Owned = u32;
}

#[cfg(feature = "test-support")]
impl RxBase for FailingSource<'_> {
    fn track(&self) -> silex_core::SilexResult<()> {
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "test source failure".to_string(),
        )))
    }
}

#[cfg(feature = "test-support")]
impl RxRead for FailingSource<'_> {
    type ReadGuard<'a>
        = silex_core::BorrowedReadGuard<'a, u32>
    where
        Self: 'a;

    fn read(&self) -> silex_core::SilexResult<Self::ReadGuard<'_>> {
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "test source failure".to_string(),
        )))
    }

    fn read_untracked(&self) -> silex_core::SilexResult<Self::ReadGuard<'_>> {
        self.read()
    }
}

#[cfg(feature = "test-support")]
impl RxReadRefSource for FailingSource<'_> {
    type ViewGuard<'a>
        = silex_core::BorrowedReadGuard<'a, u32>
    where
        Self: 'a;

    fn read_ref(&self) -> silex_core::SilexResult<Self::ViewGuard<'_>> {
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "test source failure".to_string(),
        )))
    }

    fn read_ref_untracked(&self) -> silex_core::SilexResult<Self::ViewGuard<'_>> {
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "test source failure".to_string(),
        )))
    }
}

#[cfg(feature = "test-support")]
impl RxGet for FailingSource<'_> {
    fn get(&self) -> silex_core::SilexResult<Self::Owned> {
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "test source failure".to_string(),
        )))
    }

    fn get_untracked(&self) -> silex_core::SilexResult<Self::Owned> {
        self.get()
    }
}

#[cfg(feature = "test-support")]
impl RuntimeScoped for FailingSource<'_> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.delegate.owner_access()
    }
}

#[cfg(feature = "test-support")]
impl<'owner> ReactiveSource<'owner> for FailingSource<'owner> {
    fn into_promotion_plan(self) -> PromotionPlan<'owner, Self::Owned>
    where
        Self: Sized,
        Self::Owned: Sized + silex_core::traits::RxData + 'owner,
    {
        self.delegate.into_promotion_plan()
    }
}

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn same_runtime_child_scope_reads_are_reactive() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let source = owner.signal(1_i32).expect("source signal");
            let child = owner.create_child().expect("owned owner");
            let child_owner = child.access();
            let runs_in_effect = runs.clone();
            child_owner
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(child_owner),
                )
                .expect("effect should initialize");

            source.set(2).expect("source should update");
            assert_eq!(runs.get(), 2);
        })
        .expect("runtime child should initialize");
}

#[test]
fn foreign_tracked_reads_are_rejected() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    foreign_root.with_access(|foreign_scope| {
        let source = foreign_scope.signal(1_i32).expect("foreign source");
        let result = target_root.with_access(|target_scope| {
            target_scope
                .effect(
                    EffectPhase::Normal,
                    move || source.get().map(|_| ()),
                    handler(target_scope),
                )
                .map(|_| ())
        });
        assert!(matches!(
            result,
            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                ReactiveError::RuntimeMismatch,
            )))
        ));
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[test]
fn foreign_untracked_reads_are_allowed_without_subscription() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");
    let runs = Rc::new(Cell::new(0));

    foreign_root.with_access(|foreign_scope| {
        let source = foreign_scope.signal(1_i32).expect("foreign source");
        target_root.with_access(|target_scope| {
            let runs_in_effect = runs.clone();
            target_scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get_untracked()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(target_scope),
                )
                .expect("effect should initialize");
        });

        source.set(2).expect("foreign source should update");
        assert_eq!(runs.get(), 1);
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[cfg(feature = "test-support")]
#[test]
fn foreign_source_is_rejected_before_target_allocation() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    let foreign_scope = foreign_root.access();
    let source = foreign_scope.signal(1_u32).expect("foreign source");
    let target_scope = target_root.access();
    let error_handler = handler(target_scope);
    let before = target_scope.runtime_snapshot().expect("target snapshot");
    let result = Resource::builder(target_scope)
        .source(source)
        .fetch(|_| async { Ok::<u32, ()>(1) })
        .build(error_handler.view());

    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeMismatch,
        )))
    ));
    assert_eq!(
        target_scope.runtime_snapshot().expect("target snapshot"),
        before
    );
    drop(error_handler);

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[cfg(feature = "test-support")]
#[test]
fn foreign_suspense_is_rejected_before_target_allocation() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    let foreign_scope = foreign_root.access();
    let suspense = SuspenseContext::new(foreign_scope).expect("foreign suspense");
    let target_scope = target_root.access();
    let source = target_scope.signal(1_u32).expect("target source");
    let error_handler = handler(target_scope);
    let before = target_scope.runtime_snapshot().expect("target snapshot");
    let result = Resource::builder(target_scope)
        .source(source)
        .fetch(|_| async { Ok::<u32, ()>(1) })
        .suspense(suspense)
        .build(error_handler.view());

    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeMismatch,
        )))
    ));
    assert_eq!(
        target_scope.runtime_snapshot().expect("target snapshot"),
        before
    );
    assert_eq!(suspense.count.get_untracked().expect("suspense count"), 0);
    drop(error_handler);

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[cfg(feature = "test-support")]
#[test]
fn resource_init_failure_rolls_back_every_allocation() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    let scope = root.access();
    let delegate = scope.signal(1_u32).expect("source delegate");
    let source = FailingSource {
        delegate: delegate.read_signal(),
    };
    let error_handler = handler(scope);
    let before = scope.runtime_snapshot().expect("target snapshot");
    let result = Resource::builder(scope)
        .source(source)
        .fetch(|_| async { Ok::<u32, ()>(1) })
        .build(error_handler.view());

    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Framework(message)))
            if message == "test source failure"
    ));
    assert_eq!(scope.runtime_snapshot().expect("target snapshot"), before);
    drop(error_handler);
    root.close().expect("root disposal");
}
