use std::{cell::Cell, rc::Rc};

use js_sys::Reflect;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, Headers, Request, RequestInit, Response, Window};

use crate::{
    NetError,
    backend::TransportFuture,
    state::{HttpResponse, RequestBody, RequestSpec},
};

pub trait Transport: 'static {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BrowserTransport;

#[derive(Clone, Copy, Debug, Default)]
pub struct HttpBackend;

struct FetchAbortGuard {
    controller: AbortController,
    window: Window,
    timeout_handle: Option<i32>,
    timeout_closure: Option<Closure<dyn FnMut()>>,
    timed_out: Rc<Cell<bool>>,
    completed: bool,
}

impl FetchAbortGuard {
    fn new(controller: AbortController, window: Window, timed_out: Rc<Cell<bool>>) -> Self {
        Self {
            controller,
            window,
            timeout_handle: None,
            timeout_closure: None,
            timed_out,
            completed: false,
        }
    }

    fn set_timeout(&mut self, handle: i32, closure: Closure<dyn FnMut()>) {
        self.timeout_handle = Some(handle);
        self.timeout_closure = Some(closure);
    }

    fn timed_out(&self) -> bool {
        self.timed_out.get()
    }

    fn complete(&mut self) {
        self.completed = true;
        if let Some(handle) = self.timeout_handle.take() {
            self.window.clear_timeout_with_handle(handle);
        }
        self.timeout_closure.take();
    }
}

impl Drop for FetchAbortGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.timeout_handle.take() {
            self.window.clear_timeout_with_handle(handle);
        }
        self.timeout_closure.take();
        if !self.completed {
            self.controller.abort();
        }
    }
}

impl Transport for BrowserTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        Box::pin(async move { Self::send(spec).await })
    }
}

impl Transport for HttpBackend {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        Box::pin(async move { Self::send(spec).await })
    }
}

impl HttpBackend {
    pub async fn send(spec: RequestSpec) -> Result<HttpResponse, NetError> {
        BrowserTransport::send(spec).await
    }
}

impl BrowserTransport {
    pub async fn send(spec: RequestSpec) -> Result<HttpResponse, NetError> {
        let window = web_sys::window().ok_or(NetError::BrowserUnavailable)?;

        let init = RequestInit::new();
        init.set_method(spec.method.as_str());

        let controller = AbortController::new().map_err(NetError::from)?;
        let signal = controller.signal();
        init.set_signal(Some(&signal));
        let timed_out = Rc::new(Cell::new(false));
        let mut abort_guard =
            FetchAbortGuard::new(controller.clone(), window.clone(), timed_out.clone());

        let headers = Headers::new().map_err(NetError::from)?;
        for (name, value) in &spec.headers {
            headers
                .append(name, value)
                .map_err(|err| NetError::JsError(format!("{err:?}")))?;
        }
        init.set_headers(headers.as_ref());

        if let Some(timeout) = spec.timeout {
            let timed_out_flag = timed_out.clone();
            let abort_controller = controller.clone();
            let timeout_closure = Closure::wrap(Box::new(move || {
                timed_out_flag.set(true);
                abort_controller.abort();
            }) as Box<dyn FnMut()>);

            let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    timeout_closure.as_ref().unchecked_ref(),
                    millis,
                )
                .map_err(NetError::from)?;
            abort_guard.set_timeout(handle, timeout_closure);
        }

        match &spec.body {
            RequestBody::Empty => {}
            RequestBody::Text(text) | RequestBody::Json(text) => {
                init.set_body(&JsValue::from_str(text));
            }
            RequestBody::Form(fields) => {
                let body = fields
                    .iter()
                    .map(|(name, value)| {
                        format!("{}={}", form_component(name), form_component(value))
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                init.set_body(&JsValue::from_str(&body));
            }
        }

        let request = Request::new_with_str_and_init(&spec.url, &init).map_err(NetError::from)?;
        let response_value = JsFuture::from(window.fetch_with_request(&request)).await;

        let response_value = match response_value {
            Ok(value) => value,
            Err(error) => {
                let timed_out = abort_guard.timed_out();
                let aborted = is_abort_error(&error);
                abort_guard.complete();
                return Err(if timed_out {
                    NetError::Timeout
                } else if aborted {
                    NetError::Aborted
                } else {
                    NetError::TransportUnavailable
                });
            }
        };

        let response: Response = match response_value.dyn_into() {
            Ok(response) => response,
            Err(error) => {
                abort_guard.complete();
                return Err(NetError::JsError(format!("{error:?}")));
            }
        };

        let body_promise = match response.text() {
            Ok(promise) => promise,
            Err(error) => {
                abort_guard.complete();
                return Err(NetError::from(error));
            }
        };
        let raw_body = match JsFuture::from(body_promise).await {
            Ok(value) => value.as_string().unwrap_or_default(),
            Err(error) => {
                let timed_out = abort_guard.timed_out();
                let aborted = is_abort_error(&error);
                abort_guard.complete();
                return Err(if timed_out {
                    NetError::Timeout
                } else if aborted {
                    NetError::Aborted
                } else {
                    NetError::JsError(format!("{error:?}"))
                });
            }
        };

        let status = response.status();
        let status_text = response.status_text();
        let url = response.url();
        abort_guard.complete();

        if !response.ok() {
            return Err(NetError::HttpStatus {
                status,
                body: raw_body,
            });
        }

        Ok(HttpResponse {
            url,
            status,
            status_text,
            raw_body,
        })
    }
}

fn form_component(value: &str) -> String {
    crate::builder::helper::encode_component(value).replace("%20", "+")
}

fn is_abort_error(error: &JsValue) -> bool {
    Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string())
        .is_some_and(|name| name == "AbortError")
}
