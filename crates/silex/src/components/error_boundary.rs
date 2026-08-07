use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use wasm_bindgen_futures::spawn_local;

use silex_core::{ErrorHandler, ErrorReporter, Scope, SilexError, rx};
use silex_dom::prelude::*;
use silex_dom::view::ViewOwner;
use silex_macros::component;

type ParentHandlerCell<'scope> = Rc<RefCell<Option<ErrorReporter<'scope>>>>;
type ErrorFactory<'scope> = Rc<dyn Fn(SilexError) -> AnyView<'scope> + 'scope>;
type RecordError<'scope> = Rc<dyn Fn(SilexError) + 'scope>;

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

    fn parent_handler(&self) -> ErrorReporter<'scope> {
        self.parent_handler
            .borrow()
            .as_ref()
            .expect("ErrorBoundary parent handler must be resolved during mount")
            .clone()
    }

    fn mount_inner(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let phase = self.phase;
        let child_handler = self.boundary_handler.clone();
        let parent_handler = self.parent_handler();
        let fallback = self.fallback.clone();
        let record_error = self.record_error.clone();
        let fallback_attrs = attrs.clone();
        let owner_token = owner.token();

        match phase {
            BoundaryPhase::Child => {
                let child_owner = owner_token.with_error_handler(child_handler);
                let result = self.view.mount(&child_owner, parent, attrs);
                match result {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        record_error(error.clone());
                        let fallback_owner = owner_token.with_error_handler(parent_handler.clone());
                        fallback(error).mount_owned(&fallback_owner, parent, fallback_attrs)
                    }
                }
            }
            BoundaryPhase::Fallback => {
                let fallback_owner = owner_token.with_error_handler(parent_handler);
                self.view.mount(&fallback_owner, parent, attrs)
            }
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
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        self.clone().mount_inner(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs)
    }
}

#[derive(Clone)]
struct ErrorBoundaryView<'scope> {
    view: AnyView<'scope>,
    phase_handler: ErrorReporter<'scope>,
    parent_handler: ParentHandlerCell<'scope>,
    parent_handler_override: Option<ErrorReporter<'scope>>,
}

impl<'scope> ApplyAttributes<'scope> for ErrorBoundaryView<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.view.apply_attributes(attrs);
    }
}

impl<'scope> View<'scope> for ErrorBoundaryView<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let parent_handler = self
            .parent_handler_override
            .clone()
            .unwrap_or_else(|| owner.token().error_handler());
        *self.parent_handler.borrow_mut() = Some(parent_handler);
        let token = owner.token().with_error_handler(self.phase_handler.clone());
        self.view.mount(&token, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        let parent_handler = self
            .parent_handler_override
            .clone()
            .unwrap_or_else(|| owner.token().error_handler());
        *self.parent_handler.borrow_mut() = Some(parent_handler);
        let token = owner.token().with_error_handler(self.phase_handler);
        self.view.mount_owned(&token, parent, attrs)
    }
}

/// Error boundary that routes descendant errors to its local fallback state.
#[component]
pub fn ErrorBoundary<'scope, FB, CH, V1, V2>(
    scope: Scope<'scope>,
    children: CH,
    #[chain] fallback: FB,
    #[chain(default)] parent_error_handler: Option<ErrorReporter<'scope>>,
) -> impl View<'scope>
where
    FB: Fn(SilexError) -> V1 + Clone + 'scope,
    CH: Fn() -> V2 + Clone + 'scope,
    V1: View<'scope> + 'scope,
    V2: View<'scope> + 'scope,
{
    let (error, set_error) = scope.signal(None::<SilexError>);
    let completion = scope.completion_sender(move |value| set_error.set(Some(value)));
    let reporter_completion = completion.clone();
    let boundary_handler = ErrorHandler::new(move |error| {
        let completion = reporter_completion.clone();
        spawn_local(async move {
            let _ = completion.submit(error);
        });
    });

    let parent_handler: ParentHandlerCell<'scope> = Rc::new(RefCell::new(None));
    let fallback = Rc::new(move |error: SilexError| fallback(error).into_any());
    let record_error = Rc::new(move |error: SilexError| set_error.set(Some(error)));
    let phase_handler = {
        let parent_handler = parent_handler.clone();
        let boundary_handler = boundary_handler.clone();
        ErrorHandler::new(move |error_value| {
            if error.try_get_untracked().ok().flatten().is_some() {
                parent_handler
                    .borrow()
                    .as_ref()
                    .expect("ErrorBoundary parent handler must be resolved during mount")
                    .handle(error_value);
            } else {
                boundary_handler.handle(error_value);
            }
        })
    };

    let fallback = fallback.clone();
    let children = children.clone();
    let parent_handler_for_view = parent_handler.clone();
    let view = rx!(scope; {
        if let Some(error) = (*$error).clone() {
            ErrorBoundaryBranch::fallback(
                fallback(error),
                boundary_handler.clone(),
                parent_handler_for_view.clone(),
                fallback.clone(),
                record_error.clone(),
            )
            .into_any()
        } else {
            let result = catch_unwind(AssertUnwindSafe({
                let children = children.clone();
                move || children().into_any()
            }));

            match result {
                Ok(view) => ErrorBoundaryBranch::child(
                    view,
                    boundary_handler.clone(),
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
                    let error = SilexError::Javascript(message);
                    spawn_local(async move {
                        let _ = completion.submit(error);
                    });
                    AnyView::Empty
                }
            }
        }
    });

    ErrorBoundaryView {
        view: view.into_any(),
        phase_handler,
        parent_handler,
        parent_handler_override: parent_error_handler,
    }
}
