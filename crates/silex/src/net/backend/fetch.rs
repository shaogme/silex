use std::{cell::Cell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, Headers, Request, RequestInit, Response};

use crate::net::{
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

        let headers = Headers::new().map_err(NetError::from)?;
        for (name, value) in &spec.headers {
            headers
                .append(name, value)
                .map_err(|err| NetError::JsError(format!("{err:?}")))?;
        }
        init.set_headers(headers.as_ref());

        let mut timeout_handle: Option<i32> = None;
        let mut timeout_guard: Option<Closure<dyn FnMut()>> = None;
        let timed_out = Rc::new(Cell::new(false));

        if let Some(timeout) = spec.timeout {
            let controller = AbortController::new().map_err(NetError::from)?;
            let signal = controller.signal();
            init.set_signal(Some(&signal));

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

            timeout_handle = Some(handle);
            timeout_guard = Some(timeout_closure);
        }

        match &spec.body {
            RequestBody::Empty => {}
            RequestBody::Text(text) | RequestBody::Json(text) => {
                let body = JsValue::from_str(text);
                init.set_body(&body);
            }
            RequestBody::Form(fields) => {
                let form = web_sys::FormData::new().map_err(NetError::from)?;
                for (name, value) in fields {
                    form.append_with_str(name, value).map_err(NetError::from)?;
                }
                init.set_body(form.as_ref());
            }
        }

        let request = Request::new_with_str_and_init(&spec.url, &init).map_err(NetError::from)?;
        let response_value = JsFuture::from(window.fetch_with_request(&request)).await;

        if let Some(handle) = timeout_handle {
            window.clear_timeout_with_handle(handle);
        }
        drop(timeout_guard);

        let response_value = match response_value {
            Ok(value) => value,
            Err(_err) => {
                return Err(if timed_out.get() {
                    NetError::Timeout
                } else {
                    NetError::TransportUnavailable
                });
            }
        };

        let response: Response = response_value
            .dyn_into()
            .map_err(|err| NetError::JsError(format!("{err:?}")))?;

        let raw_body = JsFuture::from(response.text().map_err(NetError::from)?)
            .await
            .map_err(NetError::from)?
            .as_string()
            .unwrap_or_default();

        let status = response.status();
        let status_text = response.status_text();
        let url = response.url();

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
