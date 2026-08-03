use futures_util::future::{AbortHandle, Abortable};
use std::future::Future;
use wasm_bindgen_futures::spawn_local;

/// A task cancellation capability retained by its owner.
///
/// Dropping this handle does not detach the task from its owner. The owner
/// registers its own cancellation hook and remains responsible for cleanup.
#[derive(Clone)]
pub struct TaskHandle {
    abort: Option<AbortHandle>,
}

impl TaskHandle {
    pub(crate) fn inactive() -> Self {
        Self { abort: None }
    }

    /// Request cancellation. Repeated calls are harmless.
    pub fn cancel(&self) {
        if let Some(abort) = &self.abort {
            abort.abort();
        }
    }

    /// Return whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.abort.as_ref().is_none_or(AbortHandle::is_aborted)
    }
}

pub(crate) fn start<F>(future: F) -> (TaskHandle, impl FnOnce())
where
    F: Future<Output = ()> + 'static,
{
    let (abort, registration) = AbortHandle::new_pair();
    let task = TaskHandle {
        abort: Some(abort.clone()),
    };
    spawn_local(async move {
        let _ = Abortable::new(future, registration).await;
    });
    (task, move || abort.abort())
}
