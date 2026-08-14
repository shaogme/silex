use futures_util::future::{AbortHandle, Abortable};
use std::{
    cell::RefCell,
    future::Future,
    mem::transmute,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use wasm_bindgen_futures::spawn_local;

/// A task cancellation capability retained by its owner.
///
/// Dropping this handle does not detach the task from its owner. The owner
/// registers its own cancellation hook and remains responsible for cleanup.
#[derive(Clone)]
pub struct TaskHandle<'scope> {
    state: Rc<RefCell<TaskState<'scope>>>,
}

struct TaskState<'scope> {
    future: Option<Pin<Box<dyn Future<Output = ()> + 'scope>>>,
    abort: Option<AbortHandle>,
}

struct TaskDriver {
    state: Rc<RefCell<TaskState<'static>>>,
}

impl Future for TaskDriver {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(mut future) = self.state.borrow_mut().future.take() else {
            return Poll::Ready(());
        };

        match future.as_mut().poll(context) {
            Poll::Pending => {
                let future = {
                    let mut state = self.state.borrow_mut();
                    if state
                        .abort
                        .as_ref()
                        .is_some_and(|abort| !abort.is_aborted())
                    {
                        state.future = Some(future);
                        None
                    } else {
                        Some(future)
                    }
                };
                drop(future);
                Poll::Pending
            }
            Poll::Ready(()) => {
                drop(future);
                Poll::Ready(())
            }
        }
    }
}

impl<'scope> TaskHandle<'scope> {
    pub(crate) fn inactive() -> Self {
        Self {
            state: Rc::new(RefCell::new(TaskState {
                future: None,
                abort: None,
            })),
        }
    }

    /// Request cancellation and synchronously release the owned future.
    /// Repeated calls are harmless.
    pub fn cancel(&self) {
        let (abort, future) = {
            let mut state = self.state.borrow_mut();
            (state.abort.clone(), state.future.take())
        };
        drop(future);
        if let Some(abort) = abort {
            abort.abort();
        }
    }

    /// Return whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.state
            .borrow()
            .abort
            .as_ref()
            .is_none_or(AbortHandle::is_aborted)
    }
}

pub(crate) fn start<'scope, F>(future: F) -> (TaskHandle<'scope>, impl FnOnce() + 'scope)
where
    F: Future<Output = ()> + 'scope,
{
    let (abort, registration) = AbortHandle::new_pair();
    let state = Rc::new(RefCell::new(TaskState {
        future: Some(Box::pin(future)),
        abort: Some(abort.clone()),
    }));
    let task = TaskHandle {
        state: state.clone(),
    };

    // SAFETY: the static driver may temporarily retain the erased scoped
    // future. Owner cleanup synchronously removes that future before the
    // lexical scope can release its captured data; after cleanup the erased
    // state contains no scoped data.
    let driver_state = unsafe {
        transmute::<Rc<RefCell<TaskState<'scope>>>, Rc<RefCell<TaskState<'static>>>>(state)
    };
    spawn_local(async move {
        let _ = Abortable::new(
            TaskDriver {
                state: driver_state,
            },
            registration,
        )
        .await;
    });

    let cancel_task = task.clone();
    (task, move || cancel_task.cancel())
}
