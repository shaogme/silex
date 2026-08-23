//! Staged multi-signal transaction coordination for the reactive runtime.
//!
//! A transaction never lends a live write lease to user code. It clones each
//! signal value into a typed entry, validates every target before applying any
//! entry, and only publishes after all staged payloads have been written.

use crate::{
    ReactiveError, ReactiveResult,
    borrow::SharedCell,
    handle::NodeKindTag,
    internal::NodeId,
    owner::{OwnerAccess, Signal},
    root::{CleanupFailure, CloseError},
    runtime::{
        self, CloseReportQueue, GlobalScheduler, OwnerId, ScopeState, validate_active_scheduler,
    },
    unsafe_boundary::ActiveOwnerProof,
};
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// The explicit lifecycle phase of a reactive transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionPhase {
    Active,
    Preparing,
    Applying,
    Committed,
    Aborted,
    Poisoned,
}

/// Structured runtime failure returned by transaction operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionError {
    primary: ReactiveError,
    rollback: Vec<ReactiveError>,
    phase: TransactionPhase,
}

impl TransactionError {
    fn new(primary: ReactiveError, phase: TransactionPhase) -> Self {
        Self {
            primary,
            rollback: Vec::new(),
            phase,
        }
    }

    fn with_rollback(
        primary: ReactiveError,
        rollback: Vec<ReactiveError>,
        phase: TransactionPhase,
    ) -> Self {
        Self {
            primary,
            rollback,
            phase,
        }
    }

    /// Return the primary runtime failure.
    pub fn primary(&self) -> ReactiveError {
        self.primary
    }

    /// Return failures collected while restoring staged payloads.
    pub fn rollback_failures(&self) -> &[ReactiveError] {
        &self.rollback
    }

    /// Return the phase in which the failure was observed.
    pub fn phase(&self) -> TransactionPhase {
        self.phase
    }
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "事务在 {:?} 阶段失败：{}",
            self.phase, self.primary
        )?;
        if !self.rollback.is_empty() {
            write!(formatter, "（回滚失败 {} 项）", self.rollback.len())?;
        }
        Ok(())
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}

/// Separates an operation's user error from a runtime transaction error.
#[derive(Debug, PartialEq, Eq)]
pub enum TransactionOperationError<E> {
    Runtime(TransactionError),
    User(E),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TransactionKey {
    owner_id: OwnerId,
    node: NodeId,
    kind: NodeKindTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TargetSnapshot {
    key: TransactionKey,
    version: u32,
    updated_epoch: u64,
    last_computed_epoch: u64,
    scheduler_epoch: u64,
}

struct TransactionContext<'scope> {
    state: ScopeState<'scope>,
    scheduler: SharedCell<GlobalScheduler>,
    owner_id: OwnerId,
}

impl<'scope> TransactionContext<'scope> {
    fn new(state: ScopeState<'scope>) -> Result<Self, TransactionError> {
        let (owner_id, scheduler) = state
            .try_borrow()
            .map(|state| (state.owner_id, state.scheduler.clone()))
            .map_err(|error| TransactionError::new(error, TransactionPhase::Active))?;
        let context = Self {
            state,
            scheduler,
            owner_id,
        };
        context
            .ensure_active()
            .map(|()| context)
            .map_err(|error| TransactionError::new(error, TransactionPhase::Active))
    }

    fn ensure_active(&self) -> ReactiveResult<()> {
        let state = self.state.try_borrow()?;
        if !state.is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        validate_active_scheduler(&self.scheduler)
    }

    fn key<T>(&self, target: &Signal<'scope, T>) -> TransactionKey {
        TransactionKey {
            owner_id: self.owner_id,
            node: target.write.handle.raw(),
            kind: NodeKindTag::Signal,
        }
    }

    fn validate_signal<T>(
        &self,
        target: &Signal<'scope, T>,
    ) -> ReactiveResult<ActiveOwnerProof<'scope>> {
        self.ensure_active()?;
        let target_state = target.write.handle.state();
        if !Rc::ptr_eq(self.state.inner(), target_state.inner()) {
            return Err(ReactiveError::RuntimeMismatch);
        }
        let target_owner = target_state.try_borrow()?.owner_id;
        if target_owner != self.owner_id {
            return Err(ReactiveError::RuntimeMismatch);
        }
        let proof = ActiveOwnerProof::from_state(&self.state)?;
        let slot = proof.restore_typed_slot(
            &self.state,
            target.write.handle.raw(),
            NodeKindTag::Signal,
            target.write.value.pointer(),
        )?;
        let lease = slot.try_write(self.scheduler.clone())?;
        drop(lease);
        Ok(proof)
    }

    fn capture_target<T>(
        &self,
        target: &Signal<'scope, T>,
    ) -> ReactiveResult<(ActiveOwnerProof<'scope>, TargetSnapshot)> {
        let proof = self.validate_signal(target)?;
        let key = self.key(target);
        let node = self
            .state
            .try_borrow()?
            .nodes
            .get(key.node)
            .copied()
            .ok_or(ReactiveError::NoSuchNode)?;
        let scheduler_epoch = self.scheduler.try_borrow()?.current_epoch();
        Ok((
            proof,
            TargetSnapshot {
                key,
                version: node.version,
                updated_epoch: node.updated_epoch,
                last_computed_epoch: node.last_computed_epoch,
                scheduler_epoch,
            },
        ))
    }

    fn validate_snapshot<T>(
        &self,
        target: &Signal<'scope, T>,
        expected: TargetSnapshot,
    ) -> ReactiveResult<()> {
        let (_, current) = self.capture_target(target)?;
        if current != expected {
            return Err(ReactiveError::InvariantViolation);
        }
        Ok(())
    }

    fn read_signal<T>(
        &self,
        target: &Signal<'scope, T>,
        proof: &ActiveOwnerProof<'scope>,
    ) -> ReactiveResult<T>
    where
        T: Clone,
    {
        let slot = proof.restore_typed_slot(
            &self.state,
            target.write.handle.raw(),
            NodeKindTag::Signal,
            target.write.value.pointer(),
        )?;
        let lease = slot.try_read(self.scheduler.clone())?.into_initialized()?;
        Ok((*lease).clone())
    }

    fn write_signal<T>(&self, target: &Signal<'scope, T>, value: T) -> ReactiveResult<()>
    where
        T: 'scope,
    {
        let proof = ActiveOwnerProof::from_state(&self.state)?;
        let slot = proof.restore_typed_slot(
            &self.state,
            target.write.handle.raw(),
            NodeKindTag::Signal,
            target.write.value.pointer(),
        )?;
        let mut lease = slot.try_write(self.scheduler.clone())?.into_initialized()?;
        *lease = value;
        drop(lease);
        Ok(())
    }
}

trait TransactionEntry<'scope> {
    fn target_key(&self) -> TransactionKey;
    fn validate(&self, context: &TransactionContext<'scope>) -> ReactiveResult<()>;
    fn apply(&mut self, context: &TransactionContext<'scope>) -> ReactiveResult<()>;
    fn restore(&mut self, context: &TransactionContext<'scope>) -> ReactiveResult<()>;
}

struct TypedEntry<'scope, T> {
    key: TransactionKey,
    target: Signal<'scope, T>,
    snapshot: TargetSnapshot,
    original: T,
    staged: T,
}

impl<'scope, T: Clone + 'scope> TransactionEntry<'scope> for TypedEntry<'scope, T> {
    fn target_key(&self) -> TransactionKey {
        self.key
    }

    fn validate(&self, context: &TransactionContext<'scope>) -> ReactiveResult<()> {
        context.validate_snapshot(&self.target, self.snapshot)
    }

    fn apply(&mut self, context: &TransactionContext<'scope>) -> ReactiveResult<()> {
        context.write_signal(&self.target, self.staged.clone())
    }

    fn restore(&mut self, context: &TransactionContext<'scope>) -> ReactiveResult<()> {
        context.write_signal(&self.target, self.original.clone())
    }
}

struct TransactionDepthGuard {
    scheduler: SharedCell<GlobalScheduler>,
    close_reports: Rc<CloseReportQueue>,
    active: bool,
}

impl TransactionDepthGuard {
    fn begin(scheduler: SharedCell<GlobalScheduler>) -> Result<Self, ReactiveError> {
        let mut scheduler_ref = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        scheduler_ref.transaction_depth = scheduler_ref.transaction_depth.saturating_add(1);
        let close_reports = scheduler_ref.close_reports.clone();
        drop(scheduler_ref);
        Ok(Self {
            close_reports,
            scheduler,
            active: true,
        })
    }
}

impl Drop for TransactionDepthGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.scheduler.try_borrow_mut() {
            Ok(mut scheduler) => {
                scheduler.transaction_depth = scheduler.transaction_depth.saturating_sub(1);
            }
            Err(_) => {
                let error = CloseError::from_failures(vec![CleanupFailure::Runtime(
                    ReactiveError::BorrowConflict,
                )]);
                if let Some(error) = error {
                    self.close_reports.push(error);
                }
            }
        }
        self.active = false;
    }
}

/// A runtime transaction containing typed staged signal values.
pub struct ReactiveTransaction<'scope> {
    context: TransactionContext<'scope>,
    phase: TransactionPhase,
    entries: Vec<Box<dyn TransactionEntry<'scope> + 'scope>>,
    targets: HashSet<TransactionKey>,
    applied: usize,
    #[cfg(test)]
    apply_failure_at: Option<usize>,
    #[cfg(test)]
    restore_failure_at: Option<usize>,
}

impl<'scope> ReactiveTransaction<'scope> {
    pub(crate) fn new(state: ScopeState<'scope>) -> Result<Self, TransactionError> {
        Ok(Self {
            context: TransactionContext::new(state)?,
            phase: TransactionPhase::Active,
            entries: Vec::new(),
            targets: HashSet::new(),
            applied: 0,
            #[cfg(test)]
            apply_failure_at: None,
            #[cfg(test)]
            restore_failure_at: None,
        })
    }

    /// Return the current transaction phase.
    pub fn phase(&self) -> TransactionPhase {
        self.phase
    }

    fn phase_error(&self) -> TransactionError {
        TransactionError::new(ReactiveError::InvariantViolation, self.phase)
    }

    fn ensure_active(&self) -> Result<(), TransactionError> {
        if self.phase != TransactionPhase::Active {
            return Err(self.phase_error());
        }
        self.context
            .ensure_active()
            .map_err(|error| TransactionError::new(error, self.phase))
    }

    fn register_target<T>(
        &mut self,
        target: &Signal<'scope, T>,
    ) -> Result<(TransactionKey, ActiveOwnerProof<'scope>, TargetSnapshot), TransactionError> {
        self.ensure_active()?;
        let (proof, snapshot) = self
            .context
            .capture_target(target)
            .map_err(|error| TransactionError::new(error, self.phase))?;
        let key = snapshot.key;
        if !self.targets.insert(key) {
            self.phase = TransactionPhase::Poisoned;
            return Err(TransactionError::new(
                ReactiveError::DuplicateTarget,
                TransactionPhase::Active,
            ));
        }
        Ok((key, proof, snapshot))
    }

    fn original<T>(
        &mut self,
        target: &Signal<'scope, T>,
    ) -> Result<(TransactionKey, TargetSnapshot, T), TransactionError>
    where
        T: Clone,
    {
        let (key, proof, snapshot) = self.register_target(target)?;
        match self.context.read_signal(target, &proof) {
            Ok(value) => Ok((key, snapshot, value)),
            Err(error) => {
                self.targets.remove(&key);
                self.phase = TransactionPhase::Poisoned;
                Err(TransactionError::new(error, TransactionPhase::Active))
            }
        }
    }

    /// Read an untracked clone of a signal's current value.
    pub fn snapshot<T>(&self, source: Signal<'scope, T>) -> Result<T, TransactionError>
    where
        T: Clone,
    {
        self.ensure_active()?;
        let proof = self
            .context
            .validate_signal(&source)
            .map_err(|error| TransactionError::new(error, self.phase))?;
        self.context
            .read_signal(&source, &proof)
            .map_err(|error| TransactionError::new(error, self.phase))
    }

    /// Stage one typed update and return the user's operation result.
    pub fn update<T, R, E, F>(
        &mut self,
        target: Signal<'scope, T>,
        f: F,
    ) -> Result<R, TransactionOperationError<E>>
    where
        T: Clone + 'scope,
        F: FnOnce(&mut T) -> Result<R, E>,
    {
        let (key, snapshot, original) = self
            .original(&target)
            .map_err(TransactionOperationError::Runtime)?;
        let mut staged = original.clone();
        let result = match f(&mut staged) {
            Ok(result) => result,
            Err(error) => {
                self.targets.remove(&key);
                self.phase = TransactionPhase::Poisoned;
                return Err(TransactionOperationError::User(error));
            }
        };
        self.entries.push(Box::new(TypedEntry {
            key,
            target,
            snapshot,
            original,
            staged,
        }));
        Ok(result)
    }

    /// Stage a replacement value for one signal.
    pub fn set<T>(&mut self, target: Signal<'scope, T>, value: T) -> Result<(), TransactionError>
    where
        T: Clone + 'scope,
    {
        let (key, snapshot, original) = self.original(&target)?;
        self.entries.push(Box::new(TypedEntry {
            key,
            target,
            snapshot,
            original,
            staged: value,
        }));
        Ok(())
    }

    fn prepare(&mut self) -> Result<(), TransactionError> {
        self.phase = TransactionPhase::Preparing;
        for entry in &self.entries {
            if let Err(error) = entry.validate(&self.context) {
                return Err(TransactionError::new(error, self.phase));
            }
        }
        Ok(())
    }

    fn rollback_applied(&mut self) -> Vec<ReactiveError> {
        let mut failures = Vec::new();
        for (index, entry) in self.entries.iter_mut().take(self.applied).enumerate().rev() {
            #[cfg(test)]
            if self.restore_failure_at == Some(index) {
                failures.push(ReactiveError::BorrowConflict);
                continue;
            }
            #[cfg(not(test))]
            let _ = index;
            if let Err(error) = entry.restore(&self.context) {
                failures.push(error);
            }
        }
        self.applied = 0;
        failures
    }

    fn report_rollback(&self, error: &TransactionError) {
        if error.rollback_failures().is_empty() {
            return;
        }
        report_transaction_error(&self.context.state, error.clone());
    }

    fn fail_with_depth(
        &mut self,
        primary: ReactiveError,
        depth: TransactionDepthGuard,
    ) -> TransactionError {
        let rollback = self.rollback_applied();
        let phase = self.phase;
        self.phase = if rollback.is_empty() {
            TransactionPhase::Aborted
        } else {
            TransactionPhase::Poisoned
        };
        drop(depth);
        let error = TransactionError::with_rollback(primary, rollback, phase);
        self.report_rollback(&error);
        error
    }

    fn apply_entries(&mut self) -> ReactiveResult<()> {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            self.applied = index.saturating_add(1);
            #[cfg(test)]
            if self.apply_failure_at == Some(index) {
                return Err(ReactiveError::BorrowConflict);
            }
            entry.apply(&self.context)?;
        }
        Ok(())
    }

    fn publish_entries(&self) -> ReactiveResult<()> {
        let ids: Vec<_> = self
            .entries
            .iter()
            .map(|entry| entry.target_key().node)
            .collect();
        runtime::preflight_signal_publications(&self.context.state, &ids)?;
        for id in ids {
            runtime::commit_signal(&self.context.state, id)?;
        }
        Ok(())
    }

    /// Apply and publish every staged entry, or restore all applied values.
    pub fn commit(mut self) -> Result<(), TransactionError> {
        self.prepare()?;
        let depth = TransactionDepthGuard::begin(self.context.scheduler.clone())
            .map_err(|error| TransactionError::new(error, self.phase))?;
        self.phase = TransactionPhase::Applying;

        let applied = catch_unwind(AssertUnwindSafe(|| self.apply_entries()));
        match applied {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(self.fail_with_depth(error, depth)),
            Err(panic) => {
                let rollback = self.rollback_applied();
                let phase = self.phase;
                self.phase = if rollback.is_empty() {
                    TransactionPhase::Aborted
                } else {
                    TransactionPhase::Poisoned
                };
                drop(depth);
                if !rollback.is_empty() {
                    report_transaction_error(
                        &self.context.state,
                        TransactionError::with_rollback(
                            ReactiveError::InvariantViolation,
                            rollback,
                            phase,
                        ),
                    );
                }
                resume_unwind(panic);
            }
        }

        if let Err(error) = self.publish_entries() {
            return Err(self.fail_with_depth(error, depth));
        }
        self.phase = TransactionPhase::Committed;
        self.applied = 0;
        drop(depth);
        runtime::flush_if_idle(&self.context.state)
            .map_err(|error| TransactionError::new(error, self.phase))
    }

    /// Discard all staged values and restore an in-progress apply if needed.
    pub fn abort(mut self) -> Result<(), TransactionError> {
        match self.phase {
            TransactionPhase::Applying => {
                let rollback = self.rollback_applied();
                let phase = self.phase;
                self.phase = if rollback.is_empty() {
                    TransactionPhase::Aborted
                } else {
                    TransactionPhase::Poisoned
                };
                if rollback.is_empty() {
                    Ok(())
                } else {
                    Err(TransactionError::with_rollback(
                        ReactiveError::InvariantViolation,
                        rollback,
                        phase,
                    ))
                }
            }
            TransactionPhase::Committed => Err(self.phase_error()),
            _ => {
                self.entries.clear();
                self.applied = 0;
                self.phase = TransactionPhase::Aborted;
                Ok(())
            }
        }
    }
}

impl Drop for ReactiveTransaction<'_> {
    fn drop(&mut self) {
        match self.phase {
            TransactionPhase::Active | TransactionPhase::Preparing => {
                self.entries.clear();
                self.phase = TransactionPhase::Aborted;
            }
            TransactionPhase::Applying => {
                let rollback = self.rollback_applied();
                let phase = self.phase;
                self.phase = if rollback.is_empty() {
                    TransactionPhase::Aborted
                } else {
                    TransactionPhase::Poisoned
                };
                if !rollback.is_empty() {
                    report_transaction_error(
                        &self.context.state,
                        TransactionError::with_rollback(
                            ReactiveError::InvariantViolation,
                            rollback,
                            phase,
                        ),
                    );
                }
            }
            TransactionPhase::Committed
            | TransactionPhase::Aborted
            | TransactionPhase::Poisoned => {}
        }
    }
}

impl<'scope> OwnerAccess<'scope> {
    /// Create a low-level transaction bound to this owner's runtime and scope.
    #[doc(hidden)]
    pub fn reactive_transaction(&self) -> Result<ReactiveTransaction<'scope>, TransactionError> {
        ReactiveTransaction::new(self.state())
    }
}

pub(crate) fn report_transaction_error<'scope>(
    state: &ScopeState<'scope>,
    error: TransactionError,
) {
    let reporter = state.try_borrow().ok().and_then(|state_ref| {
        state_ref
            .scheduler
            .try_borrow()
            .ok()
            .map(|scheduler| scheduler.close_reports.clone())
    });
    if let Some(reporter) = reporter
        && let Some(close_error) =
            CloseError::from_failures(vec![CleanupFailure::Transaction(error)])
    {
        reporter.push(close_error);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]

    use super::*;
    use crate::{EffectPhase, ErrorHandlerToken, Runtime};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn handler<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
        owner.error_handler(|_| {}).expect("handler registration")
    }

    #[test]
    fn successful_transaction_publishes_multiple_signal_values() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let left = owner.signal(1_i32).expect("left signal");
                let right = owner.signal(String::from("a")).expect("right signal");
                let runs = Rc::new(Cell::new(0_usize));
                let runs_in_effect = runs.clone();
                let observations = Rc::new(RefCell::new(Vec::new()));
                let observations_in_effect = observations.clone();
                let left_in_effect = left;
                let right_in_effect = right;
                owner
                    .effect(
                        EffectPhase::Normal,
                        move || {
                            let left_value = left_in_effect.get().expect("left read");
                            let right_value = right_in_effect.get().expect("right read");
                            observations_in_effect
                                .borrow_mut()
                                .push((left_value, right_value));
                            runs_in_effect.set(runs_in_effect.get().saturating_add(1));
                            Ok(())
                        },
                        handler(owner),
                    )
                    .expect("effect");
                assert_eq!(runs.get(), 1);

                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction
                    .update(left, |value| {
                        *value += 2;
                        Ok::<_, ()>(())
                    })
                    .expect("left update");
                transaction
                    .set(right, String::from("b"))
                    .expect("right update");
                transaction.commit().expect("commit");

                assert_eq!(left.get(), Ok(3));
                assert_eq!(right.get(), Ok(String::from("b")));
                assert_eq!(runs.get(), 2);
                assert_eq!(
                    &*observations.borrow(),
                    &[(1, String::from("a")), (3, String::from("b"))]
                );
            })
            .expect("scope");
    }

    #[test]
    fn failed_update_drops_all_staged_values() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let first = owner.signal(1_i32).expect("first signal");
                let second = owner.signal(2_i32).expect("second signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction
                    .update(first, |value| {
                        *value = 10;
                        Ok::<_, ()>(())
                    })
                    .expect("first update");
                let error = transaction
                    .update(second, |_value| Err::<(), _>("user failure"))
                    .expect_err("user failure");
                assert_eq!(error, TransactionOperationError::User("user failure"));
                drop(transaction);
                assert_eq!(first.get(), Ok(1));
                assert_eq!(second.get(), Ok(2));
            })
            .expect("scope");
    }

    #[test]
    fn duplicate_target_is_rejected_without_publishing() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(1_i32).expect("signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction.set(signal, 2).expect("first set");
                let error = transaction.set(signal, 3).expect_err("duplicate target");
                assert_eq!(error.primary(), ReactiveError::DuplicateTarget);
                drop(transaction);
                assert_eq!(signal.get(), Ok(1));
            })
            .expect("scope");
    }

    #[test]
    fn dropping_transaction_does_not_publish_staged_values() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(1_i32).expect("signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction.set(signal, 2).expect("set");
                drop(transaction);
                assert_eq!(signal.get(), Ok(1));
            })
            .expect("scope");
    }

    #[test]
    fn panic_in_update_drops_staged_values_and_releases_transaction_depth() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(1_i32).expect("signal");
                let scheduler = signal
                    .write
                    .handle
                    .state()
                    .try_borrow()
                    .expect("state")
                    .scheduler
                    .clone();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut transaction = owner.reactive_transaction().expect("transaction");
                    let _ = transaction.update(signal, |_value| -> Result<(), ()> {
                        panic!("user panic");
                    });
                }));
                assert!(result.is_err());
                assert_eq!(signal.get(), Ok(1));
                assert_eq!(
                    scheduler.try_borrow().expect("scheduler").transaction_depth,
                    0
                );
            })
            .expect("scope");
    }

    #[test]
    fn prepare_borrow_conflict_does_not_change_the_target() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(1_i32).expect("signal");
                let read = signal.read.read().expect("read lease");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                let error = transaction.set(signal, 2).expect_err("borrow conflict");
                assert_eq!(error.primary(), ReactiveError::BorrowConflict);
                drop(transaction);
                drop(read);
                assert_eq!(signal.get(), Ok(1));
            })
            .expect("scope");
    }

    #[test]
    fn prepare_rejects_live_mutation_after_staging() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(1_i32).expect("signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction
                    .update(signal, |value| {
                        signal.set(9)?;
                        *value = 2;
                        Ok::<_, ReactiveError>(())
                    })
                    .expect("staged update");

                let error = transaction.commit().expect_err("live mutation conflict");
                assert_eq!(error.primary(), ReactiveError::InvariantViolation);
                assert_eq!(signal.get(), Ok(9));
            })
            .expect("scope");
    }

    #[test]
    fn publication_preflight_keeps_source_metadata_on_dependent_borrow_conflict() {
        let mut runtime = Runtime::new();
        let owner = runtime.owner().expect("owner");
        let access = owner.access();
        let source = access.signal(1_i32).expect("source signal");
        let child = owner.create_child().expect("child owner");
        let child_access = child.access();
        let child_signal = child_access.signal(0_i32).expect("child signal");
        child_access
            .effect(
                EffectPhase::Normal,
                move || {
                    source.get().map_err(|_| ())?;
                    Ok(())
                },
                handler(access),
            )
            .expect("cross-scope effect");
        let child_state = child_signal.write.handle.state();
        let held_state = child_state.try_borrow().expect("child state read");
        let source_state = source.write.handle.state();
        let source_node = source.write.handle.raw();
        let before_version = source_state
            .try_borrow()
            .expect("source state read")
            .nodes
            .get(source_node)
            .expect("source node")
            .version;

        let mut transaction = access.reactive_transaction().expect("transaction");
        transaction.set(source, 2).expect("staged source");
        let error = transaction.commit().expect_err("dependent borrow conflict");
        assert_eq!(error.primary(), ReactiveError::BorrowConflict);
        assert_eq!(source.get(), Ok(1));
        let after_version = source_state
            .try_borrow()
            .expect("source state read")
            .nodes
            .get(source_node)
            .expect("source node")
            .version;
        assert_eq!(after_version, before_version);

        drop(held_state);
        child.close().expect("child close");
        owner.close().expect("owner close");
    }

    #[test]
    fn apply_failure_restores_already_applied_entries() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let first = owner.signal(1_i32).expect("first signal");
                let second = owner.signal(2_i32).expect("second signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction.set(first, 10).expect("first set");
                transaction.set(second, 20).expect("second set");
                transaction.apply_failure_at = Some(1);
                let error = transaction.commit().expect_err("apply failure");
                assert_eq!(error.primary(), ReactiveError::BorrowConflict);
                assert!(error.rollback_failures().is_empty());
                assert_eq!(first.get(), Ok(1));
                assert_eq!(second.get(), Ok(2));
            })
            .expect("scope");
    }

    #[test]
    fn restore_failure_preserves_primary_error_and_reports_diagnostics() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let first = owner.signal(1_i32).expect("first signal");
                let second = owner.signal(2_i32).expect("second signal");
                let mut transaction = owner.reactive_transaction().expect("transaction");
                transaction.set(first, 10).expect("first set");
                transaction.set(second, 20).expect("second set");
                transaction.apply_failure_at = Some(1);
                transaction.restore_failure_at = Some(0);

                let error = transaction.commit().expect_err("apply failure");
                assert_eq!(error.primary(), ReactiveError::BorrowConflict);
                assert_eq!(error.rollback_failures(), &[ReactiveError::BorrowConflict]);
                assert_eq!(first.get(), Ok(10));
                assert_eq!(second.get(), Ok(2));
            })
            .expect("scope");

        let reports = runtime
            .take_unhandled_close_errors()
            .expect("close reports should be readable");
        assert!(reports.iter().any(|report| {
            report.failures().iter().any(|failure| {
                matches!(failure, CleanupFailure::Transaction(error)
                    if error.rollback_failures() == [ReactiveError::BorrowConflict])
            })
        }));
    }

    #[test]
    fn closed_owner_is_rejected_before_staging() {
        let mut runtime = Runtime::new();
        let owner = runtime.owner().expect("owner");
        let access = owner.access();
        let signal = access.signal(1_i32).expect("signal");
        let mut transaction = access.reactive_transaction().expect("transaction");
        owner.close().expect("close");
        let error = transaction.set(signal, 2).expect_err("closed owner");
        assert_eq!(error.primary(), ReactiveError::NoSuchNode);
        drop(transaction);
    }
}
