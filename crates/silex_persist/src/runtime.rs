use gloo_timers::future::TimeoutFuture;
use silex_core::{
    ErrorHandlerInput, OwnerAccess, ReactiveError, SilexError, SilexResult, TaskHandle,
};
use std::{cell::Cell, mem, rc::Rc, time::Duration};

pub(crate) struct PersistTimer<'scope> {
    task: TaskHandle<'scope>,
    active: Rc<Cell<bool>>,
}

impl<'scope> PersistTimer<'scope> {
    pub(crate) fn cancel(&self) {
        self.active.set(false);
        self.task.cancel();
    }

    pub(crate) fn finish(&self) {
        self.active.set(false);
    }
}

pub(crate) fn schedule_timer<'scope, H>(
    owner: OwnerAccess<'scope>,
    task: impl FnOnce() -> SilexResult<()> + 'scope,
    duration: Duration,
    error_handler: H,
) -> SilexResult<PersistTimer<'scope>>
where
    H: ErrorHandlerInput<'scope>,
{
    let lease = error_handler
        .handler_ref()
        .lease()
        .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
    let active = Rc::new(Cell::new(true));
    let active_for_task = active.clone();
    let milliseconds = duration.as_millis().try_into().unwrap_or(u32::MAX);
    let task = owner.spawn_scoped(
        async move {
            TimeoutFuture::new(milliseconds).await;
            active_for_task.set(false);
            if let Err(error) = task() {
                let _ = lease.handle(error);
            }
        },
        error_handler,
    )?;
    Ok(PersistTimer { task, active })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WriteToken {
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriteOrigin {
    Bootstrap,
    LocalMutation,
    ExternalSnapshot,
    ExplicitFlush,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriteRequest {
    pub(crate) token: WriteToken,
    pub(crate) raw: Option<String>,
    pub(crate) origin: WriteOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimePhase {
    Bootstrap,
    Idle,
    Scheduled(WriteRequest),
    Flushing(WriteRequest),
    Failed {
        request: WriteRequest,
        message: String,
    },
    Closed,
}

pub(crate) struct PersistRuntime<'scope> {
    phase: RuntimePhase,
    timer: Option<PersistTimer<'scope>>,
    next_revision: u64,
    last_backend_raw: Option<String>,
    last_origin: WriteOrigin,
}

impl<'scope> PersistRuntime<'scope> {
    pub(crate) fn new() -> Self {
        Self {
            phase: RuntimePhase::Bootstrap,
            timer: None,
            next_revision: 0,
            last_backend_raw: None,
            last_origin: WriteOrigin::Bootstrap,
        }
    }

    pub(crate) fn initialize_snapshot(&mut self, raw: Option<String>) {
        if matches!(self.phase, RuntimePhase::Closed) {
            return;
        }
        self.phase = RuntimePhase::Idle;
        self.last_backend_raw = raw;
        self.last_origin = WriteOrigin::Bootstrap;
    }

    pub(crate) fn last_backend_raw(&self) -> Option<String> {
        self.last_backend_raw.clone()
    }

    pub(crate) fn begin_request(
        &mut self,
        raw: Option<String>,
        origin: WriteOrigin,
    ) -> Option<(WriteToken, Option<PersistTimer<'scope>>)> {
        if matches!(self.phase, RuntimePhase::Closed) {
            return None;
        }
        let previous_timer = self.timer.take();
        let token = self.next_token();
        self.phase = RuntimePhase::Scheduled(WriteRequest { token, raw, origin });
        self.last_origin = origin;
        Some((token, previous_timer))
    }

    pub(crate) fn attach_timer(
        &mut self,
        token: WriteToken,
        timer: PersistTimer<'scope>,
    ) -> Option<PersistTimer<'scope>> {
        if matches!(
            &self.phase,
            RuntimePhase::Scheduled(current) if current.token == token
        ) {
            self.timer.replace(timer)
        } else {
            Some(timer)
        }
    }

    pub(crate) fn claim_timer(&mut self, token: WriteToken) -> Option<WriteRequest> {
        self.claim_scheduled(token, true)
    }

    pub(crate) fn claim_request(&mut self, token: WriteToken) -> Option<WriteRequest> {
        self.claim_scheduled(token, false)
    }

    fn claim_scheduled(&mut self, token: WriteToken, finish_timer: bool) -> Option<WriteRequest> {
        let phase = mem::replace(&mut self.phase, RuntimePhase::Idle);
        match phase {
            RuntimePhase::Scheduled(request) if request.token == token => {
                if finish_timer {
                    if let Some(timer) = self.timer.take() {
                        timer.finish();
                    }
                } else {
                    debug_assert!(self.timer.is_none());
                }
                self.phase = RuntimePhase::Flushing(request.clone());
                Some(request)
            }
            phase => {
                self.phase = phase;
                None
            }
        }
    }

    pub(crate) fn mark_schedule_failed(
        &mut self,
        token: WriteToken,
        message: String,
    ) -> (bool, Option<PersistTimer<'scope>>) {
        let phase = mem::replace(&mut self.phase, RuntimePhase::Idle);
        match phase {
            RuntimePhase::Scheduled(request) if request.token == token => {
                self.phase = RuntimePhase::Failed { request, message };
                (true, self.timer.take())
            }
            phase => {
                self.phase = phase;
                (false, None)
            }
        }
    }

    pub(crate) fn mark_write_succeeded(&mut self, token: WriteToken) -> bool {
        let phase = mem::replace(&mut self.phase, RuntimePhase::Idle);
        match phase {
            RuntimePhase::Flushing(request) if request.token == token => {
                self.last_backend_raw = request.raw;
                self.last_origin = request.origin;
                true
            }
            phase => {
                self.phase = phase;
                false
            }
        }
    }

    pub(crate) fn mark_write_failed(&mut self, token: WriteToken, message: String) -> bool {
        let phase = mem::replace(&mut self.phase, RuntimePhase::Idle);
        match phase {
            RuntimePhase::Flushing(request) if request.token == token => {
                self.phase = RuntimePhase::Failed { request, message };
                true
            }
            phase => {
                self.phase = phase;
                false
            }
        }
    }

    pub(crate) fn apply_external_snapshot(
        &mut self,
        raw: Option<String>,
    ) -> Option<PersistTimer<'scope>> {
        if matches!(self.phase, RuntimePhase::Closed) {
            return None;
        }
        let timer = self.timer.take();
        self.phase = RuntimePhase::Idle;
        self.last_backend_raw = raw;
        self.last_origin = WriteOrigin::ExternalSnapshot;
        timer
    }

    pub(crate) fn invalidate(&mut self) -> Option<PersistTimer<'scope>> {
        if matches!(self.phase, RuntimePhase::Closed) {
            return None;
        }
        self.phase = RuntimePhase::Idle;
        self.timer.take()
    }

    pub(crate) fn close(&mut self) -> Option<PersistTimer<'scope>> {
        if matches!(self.phase, RuntimePhase::Closed) {
            return None;
        }
        self.phase = RuntimePhase::Closed;
        self.timer.take()
    }

    #[cfg(test)]
    pub(crate) fn phase(&self) -> &RuntimePhase {
        &self.phase
    }

    #[cfg(test)]
    pub(crate) fn last_origin(&self) -> WriteOrigin {
        self.last_origin
    }

    fn next_token(&mut self) -> WriteToken {
        self.next_revision = self.next_revision.wrapping_add(1);
        WriteToken {
            revision: self.next_revision,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::arithmetic_side_effects, clippy::expect_used)]

    use super::*;

    fn request(runtime: &mut PersistRuntime<'static>, raw: &str) -> WriteToken {
        runtime
            .begin_request(Some(raw.to_string()), WriteOrigin::LocalMutation)
            .expect("runtime should accept request")
            .0
    }

    #[test]
    fn bootstrap_snapshot_is_idle_without_a_timer() {
        let mut runtime = PersistRuntime::new();
        runtime.initialize_snapshot(Some("default".to_string()));

        assert_eq!(runtime.phase(), &RuntimePhase::Idle);
        assert_eq!(runtime.last_backend_raw(), Some("default".to_string()));
        assert_eq!(runtime.last_origin(), WriteOrigin::Bootstrap);
    }

    #[test]
    fn consecutive_requests_keep_only_the_latest_raw_snapshot() {
        let mut runtime = PersistRuntime::new();
        let first = request(&mut runtime, "first");
        let latest = request(&mut runtime, "latest");

        assert_ne!(first, latest);
        assert!(runtime.claim_timer(first).is_none());
        let current = runtime
            .claim_timer(latest)
            .expect("latest request should be claimable");
        assert_eq!(current.raw, Some("latest".to_string()));
        assert!(runtime.mark_write_succeeded(latest));
        assert_eq!(runtime.phase(), &RuntimePhase::Idle);
        assert_eq!(runtime.last_backend_raw(), Some("latest".to_string()));
    }

    #[test]
    fn schedule_failure_preserves_request_for_explicit_retry() {
        let mut runtime = PersistRuntime::new();
        let token = request(&mut runtime, "retry");

        let (current, timer) = runtime.mark_schedule_failed(token, "timer unavailable".to_string());
        assert!(current);
        assert!(timer.is_none());
        assert!(matches!(
            runtime.phase(),
            RuntimePhase::Failed { request, message }
                if request.token == token && request.raw == Some("retry".to_string())
                    && message == "timer unavailable"
        ));

        let retry = request(&mut runtime, "retry");
        assert_ne!(token, retry);
        assert!(
            matches!(runtime.phase(), RuntimePhase::Scheduled(request) if request.token == retry)
        );
    }

    #[test]
    fn failed_write_can_be_replaced_and_closed_callbacks_are_gated() {
        let mut runtime = PersistRuntime::new();
        let failed = request(&mut runtime, "failed");
        assert!(runtime.claim_timer(failed).is_some());
        assert!(runtime.mark_write_failed(failed, "backend unavailable".to_string()));
        assert!(matches!(runtime.phase(), RuntimePhase::Failed { .. }));

        let next = request(&mut runtime, "next");
        assert!(runtime.claim_timer(next).is_some());
        assert!(runtime.mark_write_succeeded(next));
        assert!(runtime.claim_timer(failed).is_none());

        runtime.close();
        assert!(
            runtime
                .begin_request(Some("late".to_string()), WriteOrigin::LocalMutation)
                .is_none()
        );
        assert!(runtime.claim_timer(next).is_none());
        assert!(runtime.close().is_none());
    }

    #[test]
    fn external_snapshot_invalidates_old_request_and_updates_missing_baseline() {
        let mut runtime = PersistRuntime::new();
        let old = request(&mut runtime, "old");
        runtime.apply_external_snapshot(None);

        assert!(runtime.claim_timer(old).is_none());
        assert_eq!(runtime.last_backend_raw(), None);
        assert_eq!(runtime.last_origin(), WriteOrigin::ExternalSnapshot);

        let local = request(&mut runtime, "local");
        assert!(runtime.claim_timer(local).is_some());
        assert!(runtime.mark_write_succeeded(local));
    }
}
