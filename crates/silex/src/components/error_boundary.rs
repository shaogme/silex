use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorHandlerToken, ErrorReporter, HandlerLease,
    ReactiveError, SilexContextProvider, SilexError, SilexErrorKind, SilexResult, rx, unwind_safe,
};
use silex_dom::prelude::*;
use silex_dom::view::{MountOwner, MountState, SharedCell};
use silex_macros::component;

struct ParentHandler<'scope> {
    reporter: ErrorReporter<'scope>,
    lease: HandlerLease<'scope>,
}

type ParentHandlerCell<'scope> = SharedCell<Option<MountState<'scope, ParentHandler<'scope>>>>;
type ErrorFactory<'scope> = Rc<dyn Fn(SilexError) -> AnyView<'scope> + 'scope>;
type RecordError<'scope> = Rc<dyn Fn(SilexError) + 'scope>;

fn submit_boundary_error<'scope>(
    completion: &CompletionSender<SilexError>,
    error: SilexError,
    error_handler: ErrorReporter<'scope>,
) {
    let result = completion.submit(error);
    let Err(error) = result else {
        return;
    };
    let error = match error {
        CallbackInvokeError::Runtime(error) => SilexError::fatal(SilexErrorKind::Reactivity(error)),
        CallbackInvokeError::User(error) => error,
        CallbackInvokeError::Handler(error) => {
            SilexError::fatal(SilexErrorKind::Reactivity(ReactiveError::Handler(error)))
        }
    };
    let handler_result = catch_unwind(AssertUnwindSafe(|| error_handler.handle(error)));
    if let Err(handler_panic) = handler_result {
        let _ = catch_unwind(AssertUnwindSafe(|| completion.cancel()));
        resume_unwind(handler_panic);
    }
}

#[derive(Clone, Copy)]
enum BoundaryPhase {
    Child,
    Fallback,
}

#[derive(Clone)]
struct ErrorBoundaryBranch<'scope> {
    view: AnyView<'scope>,
    phase: BoundaryPhase,
    boundary_handler: ErrorReporter<'scope>,
    parent_handler: ParentHandlerCell<'scope>,
    fallback: ErrorFactory<'scope>,
    record_error: RecordError<'scope>,
}

impl<'scope> ErrorBoundaryBranch<'scope> {
    fn child(
        view: AnyView<'scope>,
        boundary_handler: ErrorReporter<'scope>,
        parent_handler: ParentHandlerCell<'scope>,
        fallback: ErrorFactory<'scope>,
        record_error: RecordError<'scope>,
    ) -> Self {
        Self {
            view,
            phase: BoundaryPhase::Child,
            boundary_handler,
            parent_handler,
            fallback,
            record_error,
        }
    }

    fn fallback(
        view: AnyView<'scope>,
        boundary_handler: ErrorReporter<'scope>,
        parent_handler: ParentHandlerCell<'scope>,
        fallback: ErrorFactory<'scope>,
        record_error: RecordError<'scope>,
    ) -> Self {
        Self {
            view,
            phase: BoundaryPhase::Fallback,
            boundary_handler,
            parent_handler,
            fallback,
            record_error,
        }
    }

    fn parent_handler(&self) -> SilexResult<ErrorReporter<'scope>> {
        let handler = self.parent_handler.with(|state| {
            state
                .as_ref()
                .and_then(|state| state.with(|handler| handler.reporter).ok())
        });
        handler.ok_or_else(|| {
            SilexError::fatal(SilexErrorKind::Framework(
                "ErrorBoundary parent handler must be resolved during mount".to_string(),
            ))
        })
    }

    fn mount_inner(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ErrorReporter<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        let phase = self.phase;
        let child_handler = self.boundary_handler;
        let parent_handler = self.parent_handler()?;
        let fallback = self.fallback.clone();
        let record_error = self.record_error.clone();
        let fallback_attrs = attrs.clone();
        match phase {
            BoundaryPhase::Child => {
                let result = self.view.mount(owner, parent, attrs, child_handler);
                match result {
                    Ok(instance) => Ok(instance),
                    Err(error @ SilexError::Recoverable(_)) => {
                        record_error(error.clone());
                        fallback(error).mount(owner, parent, fallback_attrs, parent_handler)
                    }
                    Err(error @ SilexError::Fatal(_)) => Err(error),
                }
            }
            BoundaryPhase::Fallback => self.view.mount(owner, parent, attrs, parent_handler),
        }
    }
}

impl<'scope> ApplyAttributes<'scope> for ErrorBoundaryBranch<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.view.apply_attributes(attrs);
    }
}

impl<'scope> View<'scope> for ErrorBoundaryBranch<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        self.clone()
            .mount_inner(owner, parent, attrs, error_handler)
    }
}

#[derive(Clone)]
struct ErrorBoundaryView<'scope> {
    view: AnyView<'scope>,
    phase_handler: ErrorReporter<'scope>,
    parent_handler: ParentHandlerCell<'scope>,
    _boundary_handler: ErrorHandlerToken<'scope>,
    _phase_handler: ErrorHandlerToken<'scope>,
    _completion_error_handler: ErrorHandlerToken<'scope>,
}

impl<'scope> ApplyAttributes<'scope> for ErrorBoundaryView<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.view.apply_attributes(attrs);
    }
}

impl<'scope> View<'scope> for ErrorBoundaryView<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        let token = owner.token();
        let lease = error_handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        let parent_state = token.owner_state(ParentHandler {
            reporter: error_handler,
            lease,
        })?;
        self.parent_handler.set(Some(parent_state));
        self.view.mount(owner, parent, attrs, self.phase_handler)
    }
}

/// Error boundary that routes descendant errors to its local fallback state.
///
/// The child factory receives the boundary handler so scope-bound services can
/// bind their construction-time effects to this boundary before mount.
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
    let (error, set_error) = scope.signal(None::<SilexError>)?;
    let completion = scope.completion_sender(unwind_safe(move |value| {
        let _ = set_error.set(Some(value));
        Ok(())
    }))?;
    let completion_error_handler = scope.error_handler(move |error| {
        let _ = set_error.set(Some(error));
    })?;
    let completion_error_handler_for_boundary = completion_error_handler.clone();
    let reporter_completion = completion.clone();
    let boundary_handler = scope.error_handler(move |error| {
        let completion = reporter_completion.clone();
        let error_handler = completion_error_handler_for_boundary.view();
        let _ = scope.spawn_scoped(
            async move {
                submit_boundary_error(&completion, error, error_handler);
            },
            error_handler,
        );
    })?;
    let boundary_handler_view = boundary_handler.view();

    let parent_handler: ParentHandlerCell<'scope> = SharedCell::new(None);
    let fallback = Rc::new(move |error: SilexError| fallback(error).into_any());
    let record_error = Rc::new(move |error: SilexError| {
        let _ = set_error.set(Some(error));
    });
    let boundary_handler_for_phase = boundary_handler.clone();
    let phase_handler = {
        let parent_handler = parent_handler.clone();
        scope.error_handler(move |error_value| {
            if error.get_untracked().ok().flatten().is_some() {
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
            } else {
                let _ = boundary_handler_for_phase.handle(error_value);
            }
        })?
    };

    let fallback = fallback.clone();
    let children = children.clone();
    let child_ctx = SilexContextProvider::with_error_reporter(ctx, boundary_handler_view);
    let parent_handler_for_view = parent_handler.clone();
    let phase_handler_view = phase_handler.view();
    let phase_ctx = SilexContextProvider::with_error_reporter(ctx, phase_handler_view);
    let completion_error_handler_view = completion_error_handler.view();
    let view = rx!(phase_ctx; {
        if let Some(error) = (*$error).clone() {
            ErrorBoundaryBranch::fallback(
                fallback(error),
                boundary_handler_view,
                parent_handler_for_view.clone(),
                fallback.clone(),
                record_error.clone(),
            )
            .into_any()
        } else {
            let result = catch_unwind(AssertUnwindSafe({
                let children = children.clone();
                move || children(child_ctx).into_any()
            }));

            match result {
                Ok(view) => ErrorBoundaryBranch::child(
                    view,
                    boundary_handler_view,
                    parent_handler_for_view.clone(),
                    fallback.clone(),
                    record_error.clone(),
                )
                .into_any(),
                Err(payload) => {
                    let message = if let Some(value) = payload.downcast_ref::<&str>() {
                        format!("Panic: {value}")
                    } else if let Some(value) = payload.downcast_ref::<String>() {
                        format!("Panic: {value}")
                    } else {
                        "Unknown Panic".to_string()
                    };
                    let completion = completion.clone();
                    let error = SilexError::fatal(SilexErrorKind::Javascript(message));
                    let error_handler = completion_error_handler_view;
                    let _ = scope.spawn_scoped(
                        async move {
                            submit_boundary_error(&completion, error, error_handler);
                        },
                        error_handler,
                    );
                    AnyView::Empty
                }
            }
        }
    });

    Ok(ErrorBoundaryView {
        view: view.into_any(),
        phase_handler: phase_handler_view,
        parent_handler,
        _boundary_handler: boundary_handler,
        _phase_handler: phase_handler,
        _completion_error_handler: completion_error_handler,
    })
}
