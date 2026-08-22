use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use silex_core::{
    ErrorHandlerToken, ErrorReporter, HandlerLease, ReactiveError, SilexContextProvider,
    SilexError, SilexErrorKind, SilexResult, WriteSignal,
};
use silex_dom::prelude::*;
use silex_dom::view::{
    BranchEvaluation, BranchRenderContext, MountContext, MountState, SharedCell,
    mount_branch_stable_cached,
};
use silex_macros::component;

struct ParentHandler<'scope> {
    lease: HandlerLease<'scope>,
}

type ParentHandlerCell<'scope> = SharedCell<Option<MountState<'scope, ParentHandler<'scope>>>>;
type ErrorFactory<'scope> = Rc<dyn Fn(SilexError) -> AnyView<'scope> + 'scope>;

#[derive(Clone)]
enum BoundaryState {
    Child { generation: u64 },
    Switching { error: SilexError, generation: u64 },
    Fallback { error: SilexError, generation: u64 },
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundaryBranchKey {
    Child(u64),
    Fallback(u64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BranchSlotPhase {
    Child,
    Switching,
    Fallback,
    Closed,
}

#[derive(Clone, Copy)]
struct BranchSlotSnapshot {
    phase: BranchSlotPhase,
    generation: u64,
}

#[derive(Clone)]
struct ErrorBoundarySlot {
    state: Rc<Cell<BranchSlotSnapshot>>,
}

impl ErrorBoundarySlot {
    fn new() -> Self {
        Self {
            state: Rc::new(Cell::new(BranchSlotSnapshot {
                phase: BranchSlotPhase::Child,
                generation: 0,
            })),
        }
    }

    fn mount_child(&self, generation: u64) {
        self.state.set(BranchSlotSnapshot {
            phase: BranchSlotPhase::Child,
            generation,
        });
    }

    fn dispose_current(&self, generation: u64) {
        self.state.set(BranchSlotSnapshot {
            phase: BranchSlotPhase::Switching,
            generation,
        });
    }

    fn replace_with_fallback(&self, generation: u64) {
        self.state.set(BranchSlotSnapshot {
            phase: BranchSlotPhase::Fallback,
            generation,
        });
    }

    fn close(&self) {
        let generation = self.state.get().generation;
        self.state.set(BranchSlotSnapshot {
            phase: BranchSlotPhase::Closed,
            generation,
        });
    }

    fn generation(&self) -> u64 {
        self.state.get().generation
    }

    fn is_closed(&self) -> bool {
        self.state.get().phase == BranchSlotPhase::Closed
    }

    fn is_fallback_active(&self) -> bool {
        self.state.get().phase == BranchSlotPhase::Fallback
    }
}

type BranchKeyProvider<'scope> =
    Rc<dyn Fn() -> SilexResult<BranchEvaluation<BoundaryBranchKey, BoundaryState>> + 'scope>;
type BranchRenderer<'scope> = Rc<
    dyn Fn(
            BranchEvaluation<BoundaryBranchKey, BoundaryState>,
            BranchRenderContext<'scope>,
        ) -> AnyView<'scope>
        + 'scope,
>;

#[derive(Clone)]
struct ErrorBoundaryBranchView<'scope> {
    key: BranchKeyProvider<'scope>,
    render: BranchRenderer<'scope>,
}

impl<'scope> View<'scope> for ErrorBoundaryBranchView<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let key = self.key.clone();
        let render = self.render.clone();
        mount_branch_stable_cached(
            context,
            move || key(),
            move |evaluation, context| render(evaluation, context),
        )
    }
}

#[derive(Clone)]
struct ErrorBoundaryView<'scope> {
    view: ErrorBoundaryBranchView<'scope>,
    phase_handler: ErrorReporter<'scope>,
    parent_handler: ParentHandlerCell<'scope>,
    fallback: ErrorFactory<'scope>,
    state: WriteSignal<'scope, BoundaryState>,
    slot: ErrorBoundarySlot,
    _boundary_handler: ErrorHandlerToken<'scope>,
    _boundary_lease: HandlerLease<'scope>,
    _phase_handler: ErrorHandlerToken<'scope>,
}

impl<'scope> View<'scope> for ErrorBoundaryView<'scope> {
    fn mount(
        &self,
        context: &MountContext<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        debug_assert!(!self.slot.is_closed());
        let owner = context.owner();
        let error_handler = context.error_handler();
        let token = owner.clone();
        let lease = error_handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        let parent_state = token.owner_state(ParentHandler { lease })?;
        self.parent_handler.set(Some(parent_state));
        let slot = self.slot.clone();
        let cleanup_slot = slot.clone();
        let closed_state = self.state;
        owner.on_cleanup(
            Box::new(move || {
                cleanup_slot.close();
                let _ = closed_state.set(BoundaryState::Closed);
                Ok(())
            }),
            error_handler,
        )?;
        let phase_context = context.with_error_handler(self.phase_handler);
        match self.view.mount(&phase_context) {
            Ok(instance) => Ok(instance),
            Err(error @ SilexError::Recoverable(_)) => {
                let generation = slot.generation().saturating_add(1);
                self.state.set(BoundaryState::Fallback {
                    error: error.clone(),
                    generation,
                })?;
                slot.replace_with_fallback(generation);
                (self.fallback)(error).mount(context)
            }
            Err(error) => Err(error),
        }
    }
}

/// Error boundary that routes descendant errors to its local fallback state.
///
/// The child factory receives the boundary handler so scope-bound services can
/// bind their construction-time effects to this boundary before mount.
/// Recoverable errors from an already-mounted child are dispatched during the
/// deferred phase at the end of the current runtime flush. The child branch is
/// disposed before the fallback branch is mounted. Errors while constructing or
/// mounting the fallback are routed to the parent handler and terminate this
/// boundary; they are not sent back to the failed child branch.
///
/// Callers should observe the resulting DOM or owner cleanup when coordinating
/// with a boundary. They should not depend on a fixed number of JavaScript
/// microtasks, and custom views should pass the supplied error reporter to
/// their owner-bound effects and cleanups rather than forwarding errors through
/// a completion endpoint.
#[component]
pub fn ErrorBoundary<'scope, Ctx, FB, CH, V1, V2>(
    #[ctx] ctx: Ctx,
    children: CH,
    #[chain] fallback: FB,
) -> impl View<'scope>
where
    Ctx: SilexContextProvider<'scope>,
    FB: Fn(SilexError) -> V1 + Clone + 'scope,
    CH: Fn(Ctx) -> V2 + Clone + 'scope,
    V1: View<'scope> + 'scope,
    V2: View<'scope> + 'scope,
{
    let (state, set_state) = owner.signal(BoundaryState::Child { generation: 0 })?;
    let state_for_boundary = state;
    let set_state_for_boundary = set_state;
    let boundary_handler = owner.error_handler(move |error| {
        let Ok(BoundaryState::Child { generation }) = state_for_boundary.get_untracked() else {
            return;
        };
        let generation = generation.saturating_add(1);
        set_state_for_boundary
            .set(BoundaryState::Switching { error, generation })
            .expect("boundary state should remain active while handling an error");
    })?;
    let boundary_lease = boundary_handler
        .view()
        .lease()
        .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
    let boundary_handler_view = boundary_handler.view();

    let parent_handler: ParentHandlerCell<'scope> = SharedCell::new(None);
    let fallback = Rc::new(move |error: SilexError| fallback(error).into_any());
    let slot = ErrorBoundarySlot::new();
    let boundary_handler_for_phase = boundary_lease.clone();
    let set_state_for_phase = set_state;
    let phase_handler = {
        let parent_handler = parent_handler.clone();
        let phase_slot = slot.clone();
        owner.error_handler(move |error_value| {
            let snapshot = state.get_untracked();
            let child_active = snapshot
                .as_ref()
                .is_ok_and(|state| matches!(state, BoundaryState::Child { .. }));
            if child_active {
                let _ = boundary_handler_for_phase.handle(error_value);
            } else {
                if snapshot.as_ref().is_ok_and(|state| {
                    matches!(
                        state,
                        BoundaryState::Switching { .. } | BoundaryState::Fallback { .. }
                    )
                }) {
                    phase_slot.close();
                    let _ = set_state_for_phase.set(BoundaryState::Closed);
                }
                let parent = parent_handler.with(|state| {
                    state
                        .as_ref()
                        .and_then(|state| state.with(|handler| handler.lease.clone()).ok())
                });
                if let Some(parent) = parent {
                    let _ = parent.handle(error_value);
                } else {
                    let _ = boundary_handler_for_phase.handle(error_value);
                }
            }
        })?
    };

    let key_state = state;
    let key = Rc::new(move || {
        let snapshot = key_state.get()?;
        let key = match &snapshot {
            BoundaryState::Child { generation } => BoundaryBranchKey::Child(*generation),
            BoundaryState::Switching { generation, .. }
            | BoundaryState::Fallback { generation, .. } => {
                BoundaryBranchKey::Fallback(*generation)
            }
            BoundaryState::Closed => BoundaryBranchKey::Fallback(u64::MAX),
        };
        Ok(BranchEvaluation::new(key, snapshot))
    });

    let branch_slot = slot.clone();
    let branch_fallback = fallback.clone();
    let branch_children = children.clone();
    let branch_ctx = SilexContextProvider::with_error_reporter(ctx, boundary_handler_view);
    let branch_set_state = set_state;
    let render = Rc::new(
        move |evaluation: BranchEvaluation<BoundaryBranchKey, BoundaryState>,
              _context: BranchRenderContext<'scope>| {
            let (key, snapshot) = evaluation.into_parts();
            match (key, snapshot) {
                (BoundaryBranchKey::Child(generation), BoundaryState::Child { .. }) => {
                    branch_slot.mount_child(generation);
                    let result = catch_unwind(AssertUnwindSafe({
                        let children = branch_children.clone();
                        let child_ctx = branch_ctx;
                        move || children(child_ctx).into_any()
                    }));
                    match result {
                        Ok(view) => view,
                        Err(payload) => {
                            let message = if let Some(value) = payload.downcast_ref::<&str>() {
                                format!("Panic: {value}")
                            } else if let Some(value) = payload.downcast_ref::<String>() {
                                format!("Panic: {value}")
                            } else {
                                "Unknown Panic".to_string()
                            };
                            let error = SilexError::fatal(SilexErrorKind::Javascript(message));
                            let next_generation = generation.saturating_add(1);
                            let _ = branch_set_state.set(BoundaryState::Switching {
                                error,
                                generation: next_generation,
                            });
                            AnyView::Empty
                        }
                    }
                }
                (
                    BoundaryBranchKey::Fallback(generation),
                    BoundaryState::Switching { error, .. },
                ) => {
                    branch_slot.dispose_current(generation);
                    let view = branch_fallback(error.clone());
                    let _ = branch_set_state.set(BoundaryState::Fallback { error, generation });
                    branch_slot.replace_with_fallback(generation);
                    view
                }
                (
                    BoundaryBranchKey::Fallback(generation),
                    BoundaryState::Fallback { error, .. },
                ) => {
                    if branch_slot.is_closed() || branch_slot.is_fallback_active() {
                        return AnyView::Empty;
                    }
                    branch_slot.replace_with_fallback(generation);
                    branch_fallback(error)
                }
                (_, BoundaryState::Closed) => AnyView::Empty,
                _ => AnyView::Empty,
            }
        },
    );

    let view = ErrorBoundaryBranchView { key, render };

    Ok(ErrorBoundaryView {
        view,
        phase_handler: phase_handler.view(),
        parent_handler,
        fallback,
        state: set_state,
        slot,
        _boundary_handler: boundary_handler,
        _boundary_lease: boundary_lease,
        _phase_handler: phase_handler,
    })
}
