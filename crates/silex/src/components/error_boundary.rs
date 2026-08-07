use std::panic::{AssertUnwindSafe, catch_unwind};

use wasm_bindgen_futures::spawn_local;

use silex_core::{ErrorReporter, Scope, SilexError, rx};
use silex_dom::prelude::*;
use silex_dom::view::ViewOwner;
use silex_macros::component;

#[derive(Clone)]
struct ErrorBoundaryView<'scope> {
    view: AnyView<'scope>,
    reporter: ErrorReporter<'scope>,
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
        let token = owner.token().with_error_reporter(self.reporter.clone());
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
        let token = owner.token().with_error_reporter(self.reporter);
        self.view.mount_owned(&token, parent, attrs)
    }
}

/// Error boundary that routes descendant deferred errors to its local fallback state.
#[component]
pub fn ErrorBoundary<'scope, FB, CH, V1, V2>(
    scope: Scope<'scope>,
    children: CH,
    #[chain] fallback: FB,
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
    let reporter = ErrorReporter::new(move |error| {
        let completion = reporter_completion.clone();
        spawn_local(async move {
            let _ = completion.submit(error);
        });
    });

    let fallback = fallback.clone();
    let children = children.clone();
    let view = rx!(scope; {
        if let Some(error) = (*$error).clone() {
            fallback(error).into_any()
        } else {
            let result = catch_unwind(AssertUnwindSafe({
                let children = children.clone();
                move || children().into_any()
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
        reporter,
    }
}
