use crate::{AppHost, AppHostError, BootstrapError, HostState, UnmountOutcome};
use js_sys::{Array, Object, Reflect};
use silex_core::{
    CleanupDiagnostic, CleanupPayloadKind, CloseError, SilexError, log::console_error,
};
use silex_dom::{CleanupFailure, CleanupOrigin, CleanupReport};
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

/// An opaque JavaScript owner for an application-specific Rust mount.
///
/// The wrapper deliberately exposes no generic mount method. Rust application glue must
/// construct and mount an [`AppHost`] before transferring it to JavaScript.
#[wasm_bindgen]
pub struct JsAppHost {
    /// A private heap handle keeps the wasm-bindgen receiver free of the host's interior state.
    host: usize,
}

impl JsAppHost {
    /// Transfer an already configured host to the JavaScript-facing owner.
    pub fn from_app_host(host: AppHost) -> Self {
        Self {
            host: Box::into_raw(Box::new(host)) as usize,
        }
    }

    fn host(&self) -> &AppHost {
        debug_assert_ne!(self.host, 0);
        // SAFETY: `from_app_host` creates exactly one boxed host, and `Drop` releases it only
        // after the JavaScript wrapper is no longer reachable.
        unsafe { &*(self.host as *const AppHost) }
    }

    fn host_mut(&mut self) -> &mut AppHost {
        debug_assert_ne!(self.host, 0);
        // SAFETY: the wrapper owns the unique boxed host, so an exclusive Rust borrow is valid.
        unsafe { &mut *(self.host as *mut AppHost) }
    }
}

impl Drop for JsAppHost {
    fn drop(&mut self) {
        if self.host == 0 {
            return;
        }

        let host = self.host as *mut AppHost;
        let result = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: the pointer was produced by `Box::into_raw` in `from_app_host` and is
            // consumed exactly once here.
            unsafe { drop(Box::from_raw(host)) };
        }));
        if result.is_err() {
            console_error("Silex JS host drop panicked");
        }
    }
}

#[wasm_bindgen]
impl JsAppHost {
    /// Return whether the wrapped host currently owns an active application.
    pub fn is_active(&self) -> Result<bool, JsValue> {
        self.host()
            .is_active()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Return the stable lowercase host state used by the JavaScript API.
    pub fn state(&self) -> String {
        host_state_name(self.host().state()).to_string()
    }

    /// Dispose the application and convert failures into a structured JavaScript object.
    ///
    /// Rust keeps `UnmountOutcome` intact. The JavaScript-facing operation is intentionally
    /// idempotent, so both `Disposed` and `AlreadyUnmounted` resolve successfully.
    pub fn unmount(&mut self) -> Result<(), JsValue> {
        let result = catch_unwind(AssertUnwindSafe(|| self.host_mut().unmount()));
        match result {
            Ok(Ok(UnmountOutcome::Disposed | UnmountOutcome::AlreadyUnmounted)) => Ok(()),
            Ok(Err(error)) => Err(app_host_error_to_js(&error)?),
            Err(panic) => Err(panic_to_js("unmount", panic)?),
        }
    }
}

fn panic_to_js(operation: &str, panic: Box<dyn Any + Send>) -> Result<JsValue, JsValue> {
    let close_error = CloseError::from_panic(panic);
    let object = Object::new();
    set_property(&object, "code", JsValue::from_str("panic"))?;
    set_property(
        &object,
        "message",
        JsValue::from_str(&format!(
            "{operation} panicked: {}",
            close_error.diagnostic().message()
        )),
    )?;
    set_property(&object, "state", JsValue::from_str("poisoned"))?;
    set_property(&object, "operation", JsValue::from_str(operation))?;
    set_property(
        &object,
        "diagnostic",
        cleanup_diagnostic_to_js(close_error.diagnostic())?,
    )?;
    Ok(object.into())
}

fn host_state_name(state: HostState) -> &'static str {
    match state {
        HostState::Ready => "ready",
        HostState::Mounting => "mounting",
        HostState::Active => "active",
        HostState::Disposing => "disposing",
        HostState::Poisoned => "poisoned",
    }
}

/// Convert a bootstrap adapter failure into the stable JavaScript error object shape.
pub fn bootstrap_error_to_js(error: &BootstrapError) -> Result<JsValue, JsValue> {
    match error {
        BootstrapError::Host(error) => app_host_error_to_js(error),
        BootstrapError::TargetNotFound(target) => simple_bootstrap_error_to_js(
            "target-not-found",
            &format!("bootstrap target not found: {target}"),
            Some(("target", JsValue::from_str(target))),
        ),
        BootstrapError::Lifecycle(message) => {
            simple_bootstrap_error_to_js("lifecycle", message, None)
        }
        BootstrapError::Listener(error) => {
            simple_bootstrap_error_to_js("listener", &error.to_string(), None)
        }
    }
}

fn app_host_error_to_js(error: &AppHostError) -> Result<JsValue, JsValue> {
    let object = Object::new();
    set_property(
        &object,
        "code",
        JsValue::from_str(app_host_error_code(error)),
    )?;
    set_property(&object, "message", JsValue::from_str(&error.to_string()))?;
    set_property(&object, "primary", JsValue::UNDEFINED)?;
    set_property(&object, "rollback", JsValue::UNDEFINED)?;

    match error {
        AppHostError::Mount(error) => {
            set_property(&object, "primary", silex_error_to_js(error.primary())?)?;
            set_property(&object, "rollback", cleanup_report_to_js(error.rollback())?)?;
        }
        AppHostError::Dispose(error) => {
            let report = cleanup_report_to_js(error.report())?;
            set_property(&object, "rollback", report.clone())?;
            set_property(&object, "report", report)?;
        }
        AppHostError::InvalidState { state } => {
            set_property(&object, "state", JsValue::from_str(host_state_name(*state)))?;
        }
        AppHostError::AlreadyMounted
        | AppHostError::NotMounted
        | AppHostError::ReentrantOperation
        | AppHostError::Poisoned => {}
    }

    Ok(object.into())
}

fn simple_bootstrap_error_to_js(
    code: &str,
    message: &str,
    extra: Option<(&str, JsValue)>,
) -> Result<JsValue, JsValue> {
    let object = Object::new();
    set_property(&object, "code", JsValue::from_str(code))?;
    set_property(&object, "message", JsValue::from_str(message))?;
    set_property(&object, "primary", JsValue::UNDEFINED)?;
    set_property(&object, "rollback", JsValue::UNDEFINED)?;
    if let Some((name, value)) = extra {
        set_property(&object, name, value)?;
    }
    Ok(object.into())
}

fn app_host_error_code(error: &AppHostError) -> &'static str {
    match error {
        AppHostError::AlreadyMounted => "already-mounted",
        AppHostError::NotMounted => "not-mounted",
        AppHostError::InvalidState { .. } => "invalid-state",
        AppHostError::Mount(_) => "mount",
        AppHostError::Dispose(_) => "dispose",
        AppHostError::ReentrantOperation => "reentrant",
        AppHostError::Poisoned => "poisoned",
    }
}

fn cleanup_report_to_js(report: &CleanupReport) -> Result<JsValue, JsValue> {
    let object = Object::new();
    set_property(&object, "clean", JsValue::from_bool(report.is_clean()))?;

    let cleanup = Array::new();
    for failure in report.cleanup_failures() {
        cleanup.push(&cleanup_failure_to_js(failure)?);
    }
    set_property(&object, "cleanupFailures", cleanup.into())?;

    let boundary = Array::new();
    for error in report.boundary_errors() {
        boundary.push(&silex_error_to_js(error)?);
    }
    set_property(&object, "boundaryErrors", boundary.into())?;

    Ok(object.into())
}

fn cleanup_failure_to_js(failure: &CleanupFailure) -> Result<JsValue, JsValue> {
    let object = Object::new();
    set_property(
        &object,
        "origin",
        JsValue::from_str(cleanup_origin_name(failure.origin)),
    )?;
    set_property(
        &object,
        "diagnostic",
        cleanup_diagnostic_to_js(failure.error.diagnostic())?,
    )?;
    Ok(object.into())
}

fn cleanup_diagnostic_to_js(diagnostic: &CleanupDiagnostic) -> Result<JsValue, JsValue> {
    let object = Object::new();
    set_property(&object, "message", JsValue::from_str(diagnostic.message()))?;
    set_property(
        &object,
        "payloadKind",
        JsValue::from_str(cleanup_payload_kind_name(diagnostic.payload_kind())),
    )?;
    Ok(object.into())
}

fn silex_error_to_js(error: &SilexError) -> Result<JsValue, JsValue> {
    let (strategy, kind) = match error {
        SilexError::Recoverable(kind) => ("recoverable", kind),
        SilexError::Fatal(kind) => ("fatal", kind),
    };
    let object = Object::new();
    set_property(&object, "strategy", JsValue::from_str(strategy))?;
    set_property(&object, "kind", JsValue::from_str(kind.as_str()))?;
    set_property(&object, "message", JsValue::from_str(&error.to_string()))?;
    Ok(object.into())
}

fn set_property(object: &Object, name: &str, value: JsValue) -> Result<(), JsValue> {
    Reflect::set(object.as_ref(), &JsValue::from_str(name), &value).and_then(|set| {
        if set {
            Ok(())
        } else {
            Err(JsValue::from_str(
                "JavaScript error object property was not set",
            ))
        }
    })
}

fn cleanup_origin_name(origin: CleanupOrigin) -> &'static str {
    match origin {
        CleanupOrigin::Root => "root",
        CleanupOrigin::ProvisionalOwner => "provisional-owner",
        CleanupOrigin::MountBoundary => "mount-boundary",
    }
}

fn cleanup_payload_kind_name(kind: CleanupPayloadKind) -> &'static str {
    match kind {
        CleanupPayloadKind::String => "string",
        CleanupPayloadKind::StaticStr => "static-str",
        CleanupPayloadKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod unwind_safety_tests {
    use super::JsAppHost;
    use std::panic::RefUnwindSafe;

    #[test]
    fn js_host_receiver_is_ref_unwind_safe() {
        fn assert_ref_unwind_safe<T: RefUnwindSafe>() {}

        assert_ref_unwind_safe::<JsAppHost>();
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use silex_core::SilexErrorKind;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn app_host_errors_are_structured_without_stringifying_reports() {
        let error = app_host_error_to_js(&AppHostError::AlreadyMounted)
            .expect("error object should be created");
        assert_eq!(
            Reflect::get(&error, &JsValue::from_str("code"))
                .expect("code property should exist")
                .as_string()
                .as_deref(),
            Some("already-mounted")
        );
        assert_eq!(
            Reflect::get(&error, &JsValue::from_str("message"))
                .expect("message property should exist")
                .as_string()
                .as_deref(),
            Some("application host already has a mounted app")
        );
        assert!(
            Reflect::get(&error, &JsValue::from_str("primary"))
                .expect("primary property should exist")
                .is_undefined()
        );
    }

    #[wasm_bindgen_test]
    fn silex_errors_keep_strategy_separate_from_kind_in_javascript() {
        let fatal = silex_error_to_js(&SilexError::fatal(SilexErrorKind::Framework(
            "fatal child failure".to_string(),
        )))
        .expect("fatal error object should be created");
        assert_eq!(
            Reflect::get(&fatal, &JsValue::from_str("strategy"))
                .expect("strategy property should exist")
                .as_string()
                .as_deref(),
            Some("fatal")
        );
        assert_eq!(
            Reflect::get(&fatal, &JsValue::from_str("kind"))
                .expect("kind property should exist")
                .as_string()
                .as_deref(),
            Some("framework")
        );

        let recoverable = silex_error_to_js(&SilexError::recoverable(SilexErrorKind::Framework(
            "recoverable child failure".to_string(),
        )))
        .expect("recoverable error object should be created");
        assert_eq!(
            Reflect::get(&recoverable, &JsValue::from_str("strategy"))
                .expect("strategy property should exist")
                .as_string()
                .as_deref(),
            Some("recoverable")
        );
        assert_eq!(
            Reflect::get(&recoverable, &JsValue::from_str("kind"))
                .expect("kind property should exist")
                .as_string()
                .as_deref(),
            Some("framework")
        );
    }
}
