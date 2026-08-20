use std::cell::RefCell;
use std::rc::Rc;

use silex_core::{SilexError, SilexErrorKind};
use std::time::Duration;

use crate::state::RetryPolicy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OperationId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Closing,
    Closed,
}

struct ControllerState {
    current: Option<OperationId>,
    next: u64,
    phase: Phase,
}

/// Owner-local arbiter for asynchronous work. A completion can commit only
/// while its id is current and the controller is still running.
#[derive(Clone)]
pub(crate) struct OperationController {
    state: Rc<RefCell<ControllerState>>,
}

impl OperationController {
    pub(crate) fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(ControllerState {
                current: None,
                next: 0,
                phase: Phase::Idle,
            })),
        }
    }

    pub(crate) fn begin(&self) -> Result<CommitGuard, SilexError> {
        let mut state = self.state.borrow_mut();
        let next = state.next.checked_add(1).ok_or_else(|| {
            SilexError::fatal(SilexErrorKind::Framework(
                "silex_net operation id exhausted".to_string(),
            ))
        });
        let id = match next {
            Ok(value) => {
                state.next = value;
                OperationId(value)
            }
            Err(error) => {
                return Err(error);
            }
        };
        state.current = Some(id);
        state.phase = Phase::Running;
        drop(state);
        Ok(CommitGuard {
            controller: self.clone(),
            id,
        })
    }

    pub(crate) fn close(&self) {
        let mut state = self.state.borrow_mut();
        if state.phase != Phase::Closed {
            state.phase = Phase::Closing;
            state.current = None;
            state.phase = Phase::Closed;
        }
    }

    pub(crate) fn invalidate(&self) {
        let mut state = self.state.borrow_mut();
        if state.phase == Phase::Running {
            state.current = None;
            state.phase = Phase::Idle;
        }
    }

    fn is_current_running(&self, id: OperationId) -> bool {
        let state = self.state.borrow();
        let valid = state.phase == Phase::Running && state.current == Some(id);
        valid
    }
}

#[derive(Clone)]
pub(crate) struct CommitGuard {
    controller: OperationController,
    id: OperationId,
}

impl CommitGuard {
    pub(crate) fn id(&self) -> OperationId {
        self.id
    }

    pub(crate) fn is_current(&self) -> bool {
        self.controller.is_current_running(self.id)
    }
}

/// Shared lifecycle and retry bookkeeping for browser-backed connections.
/// The transport-specific modules only decide how to create and close a host
/// registration; failure consumption and operation invalidation live here.
pub(crate) struct ConnectionDriver {
    controller: OperationController,
    current: Option<OperationId>,
    failure_consumed: bool,
    retry_attempt: u32,
    retry_started_at: Option<f64>,
}

impl ConnectionDriver {
    pub(crate) fn new() -> Self {
        Self {
            controller: OperationController::new(),
            current: None,
            failure_consumed: false,
            retry_attempt: 0,
            retry_started_at: None,
        }
    }

    pub(crate) fn begin(&mut self) -> Result<OperationId, SilexError> {
        let operation = self.controller.begin()?.id();
        self.current = Some(operation);
        self.failure_consumed = false;
        Ok(operation)
    }

    pub(crate) fn is_current(&self, operation: OperationId) -> bool {
        self.current == Some(operation) && self.controller.is_current_running(operation)
    }

    pub(crate) fn consume_failure(&mut self, operation: OperationId) -> bool {
        if !self.is_current(operation) || self.failure_consumed {
            return false;
        }
        self.failure_consumed = true;
        true
    }

    pub(crate) fn consume_current_failure(&mut self) -> bool {
        let Some(operation) = self.current else {
            return false;
        };
        self.consume_failure(operation)
    }

    pub(crate) fn reset_retry_window(&mut self) {
        self.retry_attempt = 0;
        self.retry_started_at = None;
    }

    pub(crate) fn recovered(&mut self) {
        self.failure_consumed = false;
        self.reset_retry_window();
    }

    pub(crate) fn next_retry(
        &mut self,
        operation: OperationId,
        policy: RetryPolicy,
        now: f64,
    ) -> Option<(OperationId, Duration)> {
        if !self.is_current(operation) {
            return None;
        }
        let next_attempt = self.retry_attempt.checked_add(1)?;
        if next_attempt > policy.max_retries {
            return None;
        }
        let started_at = *self.retry_started_at.get_or_insert(now);
        let delay = policy.delay_for_attempt(next_attempt);
        if policy.max_elapsed.is_some_and(|limit| {
            let elapsed = Duration::from_millis((now - started_at).max(0.0) as u64);
            elapsed >= limit || elapsed.saturating_add(delay) > limit
        }) {
            return None;
        }
        self.retry_attempt = next_attempt;
        Some((operation, delay))
    }

    pub(crate) fn next_current_retry(
        &mut self,
        policy: RetryPolicy,
        now: f64,
    ) -> Option<(OperationId, Duration)> {
        self.current
            .and_then(|operation| self.next_retry(operation, policy, now))
    }

    pub(crate) fn invalidate(&mut self) {
        self.current = None;
        self.failure_consumed = false;
        self.controller.invalidate();
    }

    pub(crate) fn close(&mut self) {
        self.invalidate();
        self.controller.close();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ConnectionDriver, OperationController};
    use crate::state::RetryPolicy;

    #[test]
    fn only_latest_operation_can_commit() {
        let controller = OperationController::new();
        let first = controller.begin().expect("first operation");
        let second = controller.begin().expect("second operation");
        assert!(!first.is_current());
        assert!(second.is_current());
        controller.close();
        assert!(!second.is_current());
    }

    #[test]
    fn retry_budget_excludes_initial_connection() {
        let mut driver = ConnectionDriver::new();
        driver.begin().expect("initial operation");
        let policy = RetryPolicy::new(3, Duration::ZERO).no_jitter();
        assert!(driver.next_current_retry(policy, 0.0).is_some());
        assert!(driver.next_current_retry(policy, 0.0).is_some());
        assert!(driver.next_current_retry(policy, 0.0).is_some());
        assert!(driver.next_current_retry(policy, 0.0).is_none());
    }

    #[test]
    fn zero_retry_budget_stops_after_initial_connection() {
        let mut driver = ConnectionDriver::new();
        driver.begin().expect("initial operation");
        assert!(
            driver
                .next_current_retry(RetryPolicy::new(0, Duration::ZERO), 0.0)
                .is_none()
        );
    }
}
