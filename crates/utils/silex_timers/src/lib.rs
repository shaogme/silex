//! Web timer futures used by Silex.
//!
//! This local replacement keeps the `gloo_timers::future` API used by the
//! workspace while making every wasm Closure unwind-safe under
//! `panic=unwind`.

#![deny(missing_docs, missing_debug_implementations)]

#[cfg(feature = "futures")]
/// Future-backed timers.
pub mod future {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };

    #[cfg(target_arch = "wasm32")]
    use std::{cell::RefCell, rc::Rc, task::Waker};

    /// A future that resolves once a Web timeout fires.
    #[cfg_attr(not(target_arch = "wasm32"), derive(Default))]
    pub struct TimeoutFuture {
        #[cfg(target_arch = "wasm32")]
        state: Rc<RefCell<TimeoutState>>,
        #[cfg(target_arch = "wasm32")]
        handle: Option<i32>,
        #[cfg(target_arch = "wasm32")]
        closure: Option<wasm_bindgen::closure::Closure<dyn FnMut()>>,
    }

    #[cfg(target_arch = "wasm32")]
    struct TimeoutState {
        fired: bool,
        waker: Option<Waker>,
    }

    impl std::fmt::Debug for TimeoutFuture {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("TimeoutFuture")
                .finish_non_exhaustive()
        }
    }

    impl TimeoutFuture {
        /// Create a timeout future measured in milliseconds.
        #[cfg(target_arch = "wasm32")]
        pub fn new(milliseconds: u32) -> Self {
            use std::panic::AssertUnwindSafe;
            use wasm_bindgen::{JsCast, UnwrapThrowExt, closure::Closure};

            let state = Rc::new(RefCell::new(TimeoutState {
                fired: false,
                waker: None,
            }));
            let state_for_callback = state.clone();
            let closure: Closure<dyn FnMut()> = Closure::once(AssertUnwindSafe(move || {
                let waker = {
                    let mut state = state_for_callback.borrow_mut();
                    state.fired = true;
                    state.waker.take()
                };
                if let Some(waker) = waker {
                    waker.wake();
                }
            }));
            let window = web_sys::window().expect_throw("window is unavailable");
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    milliseconds.min(i32::MAX as u32) as i32,
                )
                .unwrap_throw();

            Self {
                state,
                handle: Some(handle),
                closure: Some(closure),
            }
        }

        /// Create an immediately-ready fallback on non-wasm targets.
        #[cfg(not(target_arch = "wasm32"))]
        pub fn new(_milliseconds: u32) -> Self {
            Self {}
        }
    }

    #[cfg(target_arch = "wasm32")]
    impl Drop for TimeoutFuture {
        fn drop(&mut self) {
            if let Some(handle) = self.handle.take()
                && let Some(window) = web_sys::window()
            {
                window.clear_timeout_with_handle(handle);
            }
            let _ = self.closure.take();
        }
    }

    impl Future for TimeoutFuture {
        type Output = ();

        #[cfg(target_arch = "wasm32")]
        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
            let mut state = self.state.borrow_mut();
            if state.fired {
                Poll::Ready(())
            } else {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(())
        }
    }

    /// Wait for a duration using a timeout future.
    pub fn sleep(duration: Duration) -> TimeoutFuture {
        let milliseconds: u32 = duration
            .as_millis()
            .try_into()
            .expect("timer duration exceeds the u32 millisecond range");
        TimeoutFuture::new(milliseconds)
    }
}
