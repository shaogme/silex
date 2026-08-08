use ref_str::LocalStaticRefStr;
use silex_core::{ErrorReporter, ReactiveError, Runtime, RuntimeInputs, RxGet, Scope, SilexResult};
use silex_persist::{
    BackendEvent, BackendEventSink, BackendSubscribeError, BackendSubscription, DecodePolicy,
    NoDefault, ParseCodec, PersistCodec, PersistMode, PersistenceBackend, PersistenceError,
    PersistenceState, Persistent, PersistentBuilder, RemovePolicy, SyncStrategy, WriteDefault,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

type SubscriptionMap = Rc<RefCell<HashMap<LocalStaticRefStr, Vec<(usize, BackendEventSink)>>>>;

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {})
}

#[derive(Clone, Default)]
struct MockBackend {
    state: Rc<RefCell<HashMap<String, String>>>,
    removed: Rc<RefCell<Vec<String>>>,
    writes: Rc<RefCell<Vec<(String, String)>>>,
    subscriptions: SubscriptionMap,
    next_id: Rc<Cell<usize>>,
    fail_writes: Rc<RefCell<bool>>,
    fail_removes: Rc<RefCell<bool>>,
    fail_subscription: Rc<RefCell<Option<PersistenceError>>>,
    subscription_event: Rc<RefCell<Option<BackendEvent>>>,
}

impl MockBackend {
    fn with_value(key: &str, value: &str) -> Self {
        let mut state = HashMap::new();
        state.insert(key.to_string(), value.to_string());
        Self {
            state: Rc::new(RefCell::new(state)),
            ..Self::default()
        }
    }

    fn failing_writes() -> Self {
        Self {
            fail_writes: Rc::new(RefCell::new(true)),
            ..Self::default()
        }
    }

    fn failing_removes() -> Self {
        Self {
            fail_removes: Rc::new(RefCell::new(true)),
            ..Self::default()
        }
    }

    fn failing_subscription() -> Self {
        Self {
            fail_subscription: Rc::new(RefCell::new(Some(PersistenceError::InvalidConfiguration(
                "mock subscription configuration failure".to_string(),
            )))),
            ..Self::default()
        }
    }

    fn with_subscription_event(event: BackendEvent) -> Self {
        Self {
            subscription_event: Rc::new(RefCell::new(Some(event))),
            ..Self::default()
        }
    }

    fn emit(&self, key: &str, event: BackendEvent) {
        let callbacks = self
            .subscriptions
            .borrow()
            .get(key)
            .map(|subscribers| {
                subscribers
                    .iter()
                    .map(|(_, sink)| sink.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for callback in callbacks {
            callback(event.clone());
        }
    }
}

impl<'scope> PersistenceBackend<'scope> for MockBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        Ok(self.state.borrow().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        if *self.fail_writes.borrow() {
            return Err(PersistenceError::WriteFailed(
                "mock backend write failure".to_string(),
            ));
        }
        self.state
            .borrow_mut()
            .insert(key.to_string(), value.to_string());
        self.writes
            .borrow_mut()
            .push((key.to_string(), value.to_string()));
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        if *self.fail_removes.borrow() {
            return Err(PersistenceError::RemoveFailed(
                "mock backend remove failure".to_string(),
            ));
        }
        self.state.borrow_mut().remove(key);
        self.removed.borrow_mut().push(key.to_string());
        Ok(())
    }

    fn subscribe(
        &self,
        _scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        _error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        if let Some(error) = self.fail_subscription.borrow().clone() {
            return Err(BackendSubscribeError::new(error));
        }
        let key = key.into();
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let subscription_event = self.subscription_event.borrow_mut().take();
        let sink_for_event = sink.clone();
        self.subscriptions
            .borrow_mut()
            .entry(key.clone())
            .or_default()
            .push((id, sink));
        if let Some(event) = subscription_event {
            sink_for_event(event);
        }
        let subscriptions = self.subscriptions.clone();
        Ok(BackendSubscription::new(move || {
            let mut subscriptions = subscriptions.borrow_mut();
            if let Some(subscribers) = subscriptions.get_mut(&key) {
                subscribers.retain(|(subscriber_id, _)| *subscriber_id != id);
                if subscribers.is_empty() {
                    subscriptions.remove(&key);
                }
            }
        }))
    }
}

#[derive(Clone)]
struct CallbackCodec<'scope> {
    on_encode: Rc<dyn Fn() + 'scope>,
    on_decode: Rc<dyn Fn() + 'scope>,
    on_should_remove: Rc<dyn Fn() + 'scope>,
}

impl<'scope> PersistCodec<i32> for CallbackCodec<'scope> {
    fn encode(&self, value: &i32) -> Result<String, String> {
        (self.on_encode)();
        Ok(value.to_string())
    }

    fn decode(&self, raw: &str) -> Result<i32, String> {
        (self.on_decode)();
        raw.parse()
            .map_err(|error: std::num::ParseIntError| error.to_string())
    }

    fn should_remove(&self, _value: &i32) -> bool {
        (self.on_should_remove)();
        false
    }
}

#[derive(Clone)]
struct FailingEncodeCodec {
    fail: Rc<Cell<bool>>,
}

impl PersistCodec<i32> for FailingEncodeCodec {
    fn encode(&self, value: &i32) -> Result<String, String> {
        if self.fail.get() {
            Err("mock codec encode failure".to_string())
        } else {
            Ok(value.to_string())
        }
    }

    fn decode(&self, raw: &str) -> Result<i32, String> {
        raw.parse()
            .map_err(|error: std::num::ParseIntError| error.to_string())
    }
}

#[derive(Clone)]
struct CallbackBackend<'scope> {
    inner: MockBackend,
    on_get: Option<Rc<dyn Fn() + 'scope>>,
    on_set: Option<Rc<dyn Fn() + 'scope>>,
    on_remove: Option<Rc<dyn Fn() + 'scope>>,
}

impl<'scope> PersistenceBackend<'scope> for CallbackBackend<'scope> {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        if let Some(callback) = &self.on_get {
            callback();
        }
        self.inner.get(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        if let Some(callback) = &self.on_set {
            callback();
        }
        self.inner.set(key, value)
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        if let Some(callback) = &self.on_remove {
            callback();
        }
        self.inner.remove(key)
    }

    fn subscribe(
        &self,
        scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        self.inner.subscribe(scope, key, sink, error_handler)
    }
}

#[derive(Clone, Default)]
struct FailingSubscriptionResourceBackend {
    inner: MockBackend,
    active_resources: Rc<Cell<usize>>,
    cleanup_calls: Rc<Cell<usize>>,
}

impl<'scope> PersistenceBackend<'scope> for FailingSubscriptionResourceBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        self.inner.get(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        self.inner.set(key, value)
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        self.inner.remove(key)
    }

    fn subscribe(
        &self,
        _scope: Scope<'scope>,
        _key: impl Into<LocalStaticRefStr>,
        _sink: BackendEventSink,
        _error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        self.active_resources.set(self.active_resources.get() + 1);
        let active_resources = self.active_resources.clone();
        let cleanup_calls = self.cleanup_calls.clone();
        let cleanup = BackendSubscription::new(move || {
            active_resources.set(active_resources.get() - 1);
            cleanup_calls.set(cleanup_calls.get() + 1);
        });
        Err(BackendSubscribeError::with_cleanup(
            PersistenceError::InvalidConfiguration(
                "resource backend subscription failure".to_string(),
            ),
            cleanup,
        ))
    }
}

#[derive(Clone)]
struct ForeignRuntimeBackend {
    inputs: RuntimeInputs,
    _marker: Rc<()>,
    subscribe_calls: Rc<Cell<usize>>,
    active_subscriptions: Rc<Cell<usize>>,
}

impl<'scope> PersistenceBackend<'scope> for ForeignRuntimeBackend {
    fn get(&self, _key: &str) -> Result<Option<String>, PersistenceError> {
        Ok(None)
    }

    fn set(&self, _key: &str, _value: &str) -> Result<(), PersistenceError> {
        Ok(())
    }

    fn remove(&self, _key: &str) -> Result<(), PersistenceError> {
        Ok(())
    }

    fn runtime_inputs(&self) -> RuntimeInputs {
        self.inputs.clone()
    }

    fn subscribe(
        &self,
        _scope: Scope<'scope>,
        _key: impl Into<LocalStaticRefStr>,
        _sink: BackendEventSink,
        _error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        self.subscribe_calls.set(self.subscribe_calls.get() + 1);
        self.active_subscriptions
            .set(self.active_subscriptions.get() + 1);
        let active_subscriptions = self.active_subscriptions.clone();
        Ok(BackendSubscription::new(move || {
            active_subscriptions.set(active_subscriptions.get() - 1);
        }))
    }
}

#[derive(Clone, PartialEq)]
struct MarkerValue(Rc<()>);

#[derive(Clone, Copy)]
struct MarkerCodec;

impl PersistCodec<MarkerValue> for MarkerCodec {
    fn encode(&self, _value: &MarkerValue) -> Result<String, String> {
        Ok("marker".to_string())
    }

    fn decode(&self, raw: &str) -> Result<MarkerValue, String> {
        if raw == "marker" {
            Ok(MarkerValue(Rc::new(())))
        } else {
            Err("unexpected marker value".to_string())
        }
    }
}

fn parse_builder<'scope, B>(
    scope: Scope<'scope>,
    backend: B,
    key: &str,
) -> PersistentBuilder<'scope, B, ParseCodec<i32>, i32, NoDefault>
where
    B: PersistenceBackend<'scope>,
{
    Persistent::builder(scope, key.to_string(), test_handler(scope))
        .backend(backend)
        .parse::<i32>()
}

#[test]
fn write_default_if_missing_persists_default() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::default();
        let value = parse_builder(scope, backend.clone(), "counter")
            .default(7)
            .build();
        assert_eq!(value.get_untracked(), 7);
        assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("7".to_string())
        );
    });
}

#[test]
fn decode_error_remove_and_use_default_keeps_decode_error_state() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("counter", "bad");
        let value = parse_builder(scope, backend.clone(), "counter")
            .on_decode_error(DecodePolicy::RemoveAndUseDefault)
            .default(5)
            .build();
        assert_eq!(value.get_untracked(), 5);
        assert_eq!(backend.get("counter").unwrap(), None);
        assert_eq!(
            backend.removed.borrow().as_slice(),
            &["counter".to_string()]
        );
        assert!(matches!(
            value.state().get_untracked(),
            PersistenceState::DecodeError(_)
        ));
    });
}

#[test]
fn decode_error_use_default_preserves_invalid_raw() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("counter", "bad");
        let value = parse_builder(scope, backend.clone(), "counter")
            .on_decode_error(DecodePolicy::UseDefault)
            .default(11)
            .build();
        assert_eq!(value.get_untracked(), 11);
        assert_eq!(backend.get("counter").unwrap(), Some("bad".to_string()));
        assert!(matches!(
            value.state().get_untracked(),
            PersistenceState::DecodeError(_)
        ));
    });
}

#[test]
fn write_default_always_normalizes_existing_raw() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("counter", "007");
        let _value = parse_builder(scope, backend.clone(), "counter")
            .write_default(WriteDefault::Always)
            .default(5)
            .build();
        assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
        assert_eq!(
            backend.writes.borrow().as_slice(),
            &[("counter".to_string(), "7".to_string())]
        );
    });
}

#[test]
fn initial_default_write_failure_is_visible() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::failing_writes();
        let value = parse_builder(scope, backend, "counter").default(3).build();
        assert_eq!(value.get_untracked(), 3);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock backend write failure".to_string())
        );
    });
}

#[test]
fn manual_encode_failure_sets_write_error_for_effect_and_flush() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::default();
        let codec = FailingEncodeCodec {
            fail: Rc::new(Cell::new(false)),
        };
        let fail = codec.fail.clone();
        let value = Persistent::builder(scope, "manual-encode-failure", test_handler(scope))
            .backend(backend.clone())
            .custom_codec::<i32, _>(codec)
            .mode(PersistMode::Manual)
            .sync(SyncStrategy::None)
            .default(1)
            .build();

        fail.set(true);
        value.set(2);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock codec encode failure".to_string())
        );

        value.set(3);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock codec encode failure".to_string())
        );
        assert_eq!(
            value.flush(),
            Err(PersistenceError::EncodeFailed(
                "mock codec encode failure".to_string()
            ))
        );
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock codec encode failure".to_string())
        );
    });
}

#[test]
fn optional_none_flush_removes_backend_key() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("name", "alice");
        let value = Persistent::builder(scope, "name", test_handler(scope))
            .backend(backend.clone())
            .string()
            .optional()
            .build();
        assert_eq!(value.get_untracked(), Some("alice".to_string()));
        value.set(None);
        value.flush().unwrap();
        assert_eq!(backend.get("name").unwrap(), None);
        assert_eq!(backend.removed.borrow().as_slice(), &["name".to_string()]);
    });
}

#[test]
fn external_remove_uses_default_without_rewriting_backend() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("counter", "7");
        let value = parse_builder(scope, backend.clone(), "counter")
            .default(5)
            .build();
        backend.state.borrow_mut().remove("counter");
        backend.emit(
            "counter",
            BackendEvent::Removed {
                key: "counter".into(),
            },
        );
        assert_eq!(value.get_untracked(), 5);
        assert_eq!(backend.get("counter").unwrap(), None);
        assert!(backend.removed.borrow().is_empty());
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
    });
}

#[test]
fn explicit_remove_does_not_skip_the_next_immediate_or_manual_write() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let immediate_backend = MockBackend::default();
        let immediate = parse_builder(scope, immediate_backend.clone(), "immediate-remove")
            .default(5)
            .build();
        immediate.remove().unwrap();
        immediate.set(9);
        assert_eq!(
            immediate_backend.get("immediate-remove").unwrap(),
            Some("9".to_string())
        );

        let manual_backend = MockBackend::default();
        let manual = parse_builder(scope, manual_backend.clone(), "manual-remove")
            .mode(PersistMode::Manual)
            .sync(SyncStrategy::None)
            .default(5)
            .build();
        manual.remove().unwrap();
        manual.set(9);
        assert_eq!(
            manual.state().get_untracked(),
            PersistenceState::Dirty("9".to_string())
        );
        manual.flush().unwrap();
        assert_eq!(
            manual_backend.get("manual-remove").unwrap(),
            Some("9".to_string())
        );
    });
}

#[test]
fn missing_reload_with_default_value_does_not_skip_the_next_write() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::default();
        let value = parse_builder(scope, backend.clone(), "reload-missing")
            .write_default(WriteDefault::Never)
            .default(5)
            .build();
        value.reload().unwrap();
        value.set(6);
        assert_eq!(
            backend.get("reload-missing").unwrap(),
            Some("6".to_string())
        );
    });
}

#[test]
fn external_remove_with_default_value_does_not_skip_the_next_write() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("same-default", "5");
        let value = parse_builder(scope, backend.clone(), "same-default")
            .default(5)
            .build();
        backend.state.borrow_mut().remove("same-default");
        backend.emit(
            "same-default",
            BackendEvent::Removed {
                key: "same-default".into(),
            },
        );
        value.set(6);
        assert_eq!(backend.get("same-default").unwrap(), Some("6".to_string()));
    });
}

#[test]
fn ignored_external_remove_does_not_skip_the_next_write() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("ignored-remove", "7");
        let value = parse_builder(scope, backend.clone(), "ignored-remove")
            .on_remove(RemovePolicy::Ignore)
            .default(5)
            .build();
        backend.state.borrow_mut().remove("ignored-remove");
        backend.emit(
            "ignored-remove",
            BackendEvent::Removed {
                key: "ignored-remove".into(),
            },
        );
        assert_eq!(value.get_untracked(), 7);
        value.set(8);
        assert_eq!(
            backend.get("ignored-remove").unwrap(),
            Some("8".to_string())
        );
    });
}

#[test]
fn local_write_after_external_fallback_before_effect_flush_is_persisted() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("same-transaction", "7");
        let value = parse_builder(scope, backend.clone(), "same-transaction")
            .default(5)
            .build();
        let (trigger, set_trigger) = scope.signal(false);
        let value_for_effect = value;
        let backend_for_effect = backend.clone();
        scope
            .effect(
                move || -> SilexResult<()> {
                    if trigger.try_get()? {
                        backend_for_effect
                            .state
                            .borrow_mut()
                            .remove("same-transaction");
                        backend_for_effect.emit(
                            "same-transaction",
                            BackendEvent::Removed {
                                key: "same-transaction".into(),
                            },
                        );
                        value_for_effect.set(6);
                    }
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("persistence test effect can be registered");

        set_trigger.set(true);
        assert_eq!(
            backend.get("same-transaction").unwrap(),
            Some("6".to_string())
        );
    });
}

#[test]
fn codec_callbacks_can_reenter_the_same_binding() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("codec-reentry", "1");
        let binding_slot: Rc<RefCell<Option<Persistent<'_, i32>>>> = Rc::new(RefCell::new(None));
        let encode_called = Rc::new(Cell::new(false));
        let decode_called = Rc::new(Cell::new(false));
        let should_remove_called = Rc::new(Cell::new(false));

        let slot_for_encode = binding_slot.clone();
        let encode_called_for_callback = encode_called.clone();
        let on_encode = Rc::new(move || {
            let binding = slot_for_encode.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !encode_called_for_callback.replace(true)
            {
                binding.flush().unwrap();
            }
        });
        let slot_for_decode = binding_slot.clone();
        let decode_called_for_callback = decode_called.clone();
        let on_decode = Rc::new(move || {
            let binding = slot_for_decode.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !decode_called_for_callback.replace(true)
            {
                binding.reload().unwrap();
            }
        });
        let slot_for_should_remove = binding_slot.clone();
        let should_remove_called_for_callback = should_remove_called.clone();
        let on_should_remove = Rc::new(move || {
            let binding = slot_for_should_remove.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !should_remove_called_for_callback.replace(true)
            {
                binding.flush().unwrap();
            }
        });
        let codec = CallbackCodec {
            on_encode,
            on_decode,
            on_should_remove,
        };
        let binding = Persistent::builder(scope, "codec-reentry", test_handler(scope))
            .backend(backend.clone())
            .custom_codec::<i32, _>(codec)
            .default(1)
            .build();
        *binding_slot.borrow_mut() = Some(binding);

        binding.reload().unwrap();
        binding.set(2);
        assert!(decode_called.get());
        assert!(encode_called.get());
        assert!(should_remove_called.get());
        assert_eq!(backend.get("codec-reentry").unwrap(), Some("2".to_string()));
    });
}

#[test]
fn default_callback_can_reenter_the_same_binding() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("default-reentry", "7");
        let binding_slot: Rc<RefCell<Option<Persistent<'_, i32>>>> = Rc::new(RefCell::new(None));
        let called = Rc::new(Cell::new(false));
        let slot_for_default = binding_slot.clone();
        let called_for_default = called.clone();
        let binding = parse_builder(scope, backend.clone(), "default-reentry")
            .default_with(move || {
                let binding = slot_for_default.borrow().as_ref().copied();
                if let Some(binding) = binding
                    && !called_for_default.replace(true)
                {
                    binding.flush().unwrap();
                }
                5
            })
            .build();
        *binding_slot.borrow_mut() = Some(binding);

        backend.state.borrow_mut().remove("default-reentry");
        backend.emit(
            "default-reentry",
            BackendEvent::Removed {
                key: "default-reentry".into(),
            },
        );
        assert!(called.get());
        assert_eq!(binding.get_untracked(), 5);
        assert_eq!(
            binding.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
    });
}

#[test]
fn backend_callbacks_can_reenter_the_same_binding() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let inner = MockBackend::with_value("backend-reentry", "1");
        let binding_slot: Rc<RefCell<Option<Persistent<'_, i32>>>> = Rc::new(RefCell::new(None));
        let get_called = Rc::new(Cell::new(false));
        let set_called = Rc::new(Cell::new(false));
        let remove_called = Rc::new(Cell::new(false));

        let slot_for_get = binding_slot.clone();
        let get_called_for_callback = get_called.clone();
        let on_get = Rc::new(move || {
            let binding = slot_for_get.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !get_called_for_callback.replace(true)
            {
                binding.reload().unwrap();
            }
        });
        let slot_for_set = binding_slot.clone();
        let set_called_for_callback = set_called.clone();
        let on_set = Rc::new(move || {
            let binding = slot_for_set.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !set_called_for_callback.replace(true)
            {
                binding.flush().unwrap();
            }
        });
        let slot_for_remove = binding_slot.clone();
        let remove_called_for_callback = remove_called.clone();
        let on_remove = Rc::new(move || {
            let binding = slot_for_remove.borrow().as_ref().copied();
            if let Some(binding) = binding
                && !remove_called_for_callback.replace(true)
            {
                binding.remove().unwrap();
            }
        });
        let backend = CallbackBackend {
            inner: inner.clone(),
            on_get: Some(on_get),
            on_set: Some(on_set),
            on_remove: Some(on_remove),
        };
        let binding = Persistent::builder(scope, "backend-reentry", test_handler(scope))
            .backend(backend)
            .parse::<i32>()
            .default(1)
            .build();
        *binding_slot.borrow_mut() = Some(binding);

        binding.reload().unwrap();
        binding.set(2);
        binding.remove().unwrap();
        assert!(get_called.get());
        assert!(set_called.get());
        assert!(remove_called.get());
        assert_eq!(inner.get("backend-reentry").unwrap(), None);
    });
}

#[test]
fn panicking_codec_callback_restores_the_controller_payload() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("codec-panic", "1");
        let should_panic = Rc::new(Cell::new(false));
        let panic_for_callback = should_panic.clone();
        let codec = CallbackCodec {
            on_encode: Rc::new(move || {
                if panic_for_callback.replace(false) {
                    panic!("codec callback panic");
                }
            }),
            on_decode: Rc::new(|| {}),
            on_should_remove: Rc::new(|| {}),
        };
        let binding = Persistent::builder(scope, "codec-panic", test_handler(scope))
            .backend(backend.clone())
            .custom_codec::<i32, _>(codec)
            .default(1)
            .build();

        should_panic.set(true);
        let panic = catch_unwind(AssertUnwindSafe(|| binding.set(2)));
        assert!(panic.is_err());
        binding.set(3);
        assert_eq!(backend.get("codec-panic").unwrap(), Some("3".to_string()));
    });
}

#[test]
fn subscription_is_removed_when_scope_is_disposed() {
    let backend = MockBackend::with_value("counter", "7");
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _value = parse_builder(scope, backend.clone(), "counter")
            .default(5)
            .build();
        assert_eq!(backend.subscriptions.borrow().len(), 1);
    });
    assert!(backend.subscriptions.borrow().is_empty());
}

#[test]
fn stale_persistent_operations_return_no_such_node_during_root_cleanup() {
    let backend = MockBackend::default();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let errors = Rc::new(RefCell::new(Vec::new()));

    root.with_scope(|scope| {
        let value = parse_builder(scope, backend.clone(), "stale-cleanup")
            .default(1)
            .build();
        let errors_for_cleanup = errors.clone();
        scope
            .on_cleanup(
                move || -> SilexResult<()> {
                    errors_for_cleanup
                        .borrow_mut()
                        .push(value.remove().expect_err("stale remove must fail"));
                    errors_for_cleanup
                        .borrow_mut()
                        .push(value.reload().expect_err("stale reload must fail"));
                    errors_for_cleanup
                        .borrow_mut()
                        .push(value.flush().expect_err("stale flush must fail"));
                    assert_eq!(
                        value.try_set(2),
                        Err(PersistenceError::Reactivity(ReactiveError::NoSuchNode))
                    );
                    value.reset();
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("stale cleanup can be registered");
    });

    let writes_before_dispose = backend.writes.borrow().len();
    let removes_before_dispose = backend.removed.borrow().len();
    root.dispose().expect("root cleanup should succeed");

    assert_eq!(
        errors.borrow().as_slice(),
        &[
            PersistenceError::Reactivity(ReactiveError::NoSuchNode),
            PersistenceError::Reactivity(ReactiveError::NoSuchNode),
            PersistenceError::Reactivity(ReactiveError::NoSuchNode),
        ]
    );
    assert_eq!(backend.writes.borrow().len(), writes_before_dispose);
    assert_eq!(backend.removed.borrow().len(), removes_before_dispose);
}

#[test]
fn subscription_configuration_failure_does_not_create_binding_nodes() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::failing_subscription();
        let marker = Rc::new(());
        let marker_count_before = Rc::strong_count(&marker);
        let result = parse_builder(scope, backend.clone(), "invalid-subscription")
            .default_with({
                let marker = marker.clone();
                move || {
                    let _ = &marker;
                    1
                }
            })
            .try_build();

        assert!(matches!(
            result,
            Err(PersistenceError::InvalidConfiguration(message))
                if message == "mock subscription configuration failure"
        ));
        assert_eq!(Rc::strong_count(&marker), marker_count_before);
        assert!(backend.subscriptions.borrow().is_empty());
    });
}

#[test]
fn subscription_error_rolls_back_resources_created_before_failure() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = FailingSubscriptionResourceBackend::default();
        let marker = Rc::new(());
        let marker_count_before = Rc::strong_count(&marker);
        let result = parse_builder(scope, backend.clone(), "resource-failure")
            .default_with({
                let marker = marker.clone();
                move || {
                    let _ = &marker;
                    1
                }
            })
            .try_build();

        assert!(matches!(
            result,
            Err(PersistenceError::InvalidConfiguration(message))
                if message == "resource backend subscription failure"
        ));
        assert_eq!(backend.active_resources.get(), 0);
        assert_eq!(backend.cleanup_calls.get(), 1);
        assert_eq!(Rc::strong_count(&marker), marker_count_before);
    });
}

#[test]
fn foreign_runtime_try_build_leaves_no_binding_resources() {
    let mut foreign_runtime = Runtime::new();
    let foreign_inputs = foreign_runtime.child(|scope| {
        let (_, write) = scope.signal(1_i32);
        write.runtime_inputs()
    });

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let marker = Rc::new(());
        let backend = ForeignRuntimeBackend {
            inputs: foreign_inputs.clone(),
            _marker: marker.clone(),
            subscribe_calls: Rc::new(Cell::new(0)),
            active_subscriptions: Rc::new(Cell::new(0)),
        };
        let marker_count_without_builder = Rc::strong_count(&marker);
        let builder = Persistent::builder(scope, "foreign-runtime", test_handler(scope))
            .backend(backend.clone())
            .custom_codec::<MarkerValue, _>(MarkerCodec)
            .default_with({
                let marker = marker.clone();
                move || MarkerValue(marker.clone())
            });

        let result = builder.try_build();

        assert!(matches!(
            result,
            Err(PersistenceError::InvalidConfiguration(_))
        ));
        assert_eq!(backend.subscribe_calls.get(), 0);
        assert_eq!(backend.active_subscriptions.get(), 0);
        assert_eq!(Rc::strong_count(&marker), marker_count_without_builder);
    });
}

#[test]
fn synchronous_subscription_event_is_replayed_after_initialization() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_subscription_event(BackendEvent::Set {
            key: "synchronous-event".into(),
            value: "9".to_string(),
        });
        let value = parse_builder(scope, backend, "synchronous-event")
            .default(1)
            .build();

        assert_eq!(value.get_untracked(), 9);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("9".to_string())
        );
    });
}

#[test]
fn manual_mode_marks_value_dirty_until_flush() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::default();
        let value = parse_builder(scope, backend.clone(), "counter")
            .mode(PersistMode::Manual)
            .sync(SyncStrategy::None)
            .default(1)
            .build();
        value.set(2);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Dirty("2".to_string())
        );
        value.flush().unwrap();
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("2".to_string())
        );
    });
}

#[test]
fn initial_decode_removal_failure_sets_write_error() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::failing_removes();
        backend
            .state
            .borrow_mut()
            .insert("counter".to_string(), "bad".to_string());
        let value = parse_builder(scope, backend.clone(), "counter")
            .write_default(WriteDefault::Always)
            .on_decode_error(DecodePolicy::RemoveAndUseDefault)
            .default(1)
            .build();
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock backend remove failure".to_string())
        );
        assert_eq!(backend.get("counter").unwrap(), Some("bad".to_string()));
    });
}

#[test]
fn external_decode_removal_failure_sets_write_error() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::with_value("counter", "1");
        let value = parse_builder(scope, backend.clone(), "counter")
            .on_decode_error(DecodePolicy::RemoveAndUseDefault)
            .default(1)
            .build();
        *backend.fail_removes.borrow_mut() = true;
        backend.emit(
            "counter",
            BackendEvent::Set {
                key: "counter".into(),
                value: "bad".to_string(),
            },
        );
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock backend remove failure".to_string())
        );
    });
}

#[test]
fn write_default_never_and_immediate_none_cover_missing_and_existing_values() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let missing_backend = MockBackend::default();
        let missing = parse_builder(scope, missing_backend.clone(), "matrix-missing")
            .write_default(WriteDefault::Never)
            .on_remove(RemovePolicy::Ignore)
            .sync(SyncStrategy::None)
            .default(5)
            .build();
        assert_eq!(missing.get_untracked(), 5);
        assert_eq!(missing_backend.get("matrix-missing").unwrap(), None);
        assert!(missing_backend.subscriptions.borrow().is_empty());
        assert_eq!(
            missing.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );

        missing.set(5);
        assert_eq!(
            missing_backend.get("matrix-missing").unwrap(),
            Some("5".to_string())
        );
        assert_eq!(
            missing.state().get_untracked(),
            PersistenceState::Ready("5".to_string())
        );

        missing.set(6);
        assert_eq!(
            missing_backend.get("matrix-missing").unwrap(),
            Some("6".to_string())
        );
        assert_eq!(
            missing_backend.writes.borrow().as_slice(),
            &[
                ("matrix-missing".to_string(), "5".to_string()),
                ("matrix-missing".to_string(), "6".to_string()),
            ]
        );
        assert_eq!(
            missing.state().get_untracked(),
            PersistenceState::Ready("6".to_string())
        );

        let existing_backend = MockBackend::with_value("matrix-existing", "7");
        let existing = parse_builder(scope, existing_backend.clone(), "matrix-existing")
            .write_default(WriteDefault::Never)
            .on_remove(RemovePolicy::Ignore)
            .sync(SyncStrategy::None)
            .default(5)
            .build();
        assert_eq!(existing.get_untracked(), 7);
        assert_eq!(
            existing.state().get_untracked(),
            PersistenceState::Ready("7".to_string())
        );
        assert!(existing_backend.writes.borrow().is_empty());
        existing_backend
            .state
            .borrow_mut()
            .remove("matrix-existing");
        existing_backend.emit(
            "matrix-existing",
            BackendEvent::Removed {
                key: "matrix-existing".into(),
            },
        );
        assert_eq!(existing.get_untracked(), 7);
        assert_eq!(
            existing.state().get_untracked(),
            PersistenceState::Ready("7".to_string())
        );

        existing.set(8);
        assert_eq!(
            existing_backend.get("matrix-existing").unwrap(),
            Some("8".to_string())
        );
        assert_eq!(
            existing_backend.writes.borrow().as_slice(),
            &[("matrix-existing".to_string(), "8".to_string())]
        );
    });
}

#[test]
fn remove_ignore_matrix_clears_external_state_without_skipping_local_writes() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let non_default_backend = MockBackend::with_value("ignore-non-default", "7");
        let non_default = parse_builder(scope, non_default_backend.clone(), "ignore-non-default")
            .on_remove(RemovePolicy::Ignore)
            .default(5)
            .build();
        non_default_backend
            .state
            .borrow_mut()
            .remove("ignore-non-default");
        non_default_backend.emit(
            "ignore-non-default",
            BackendEvent::Removed {
                key: "ignore-non-default".into(),
            },
        );
        assert_eq!(non_default.get_untracked(), 7);
        assert_eq!(
            non_default.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
        non_default.set(8);
        assert_eq!(
            non_default_backend.get("ignore-non-default").unwrap(),
            Some("8".to_string())
        );

        let default_backend = MockBackend::with_value("ignore-default", "5");
        let default = parse_builder(scope, default_backend.clone(), "ignore-default")
            .write_default(WriteDefault::Never)
            .on_remove(RemovePolicy::Ignore)
            .default(5)
            .build();
        default_backend.state.borrow_mut().remove("ignore-default");
        default_backend.emit(
            "ignore-default",
            BackendEvent::Removed {
                key: "ignore-default".into(),
            },
        );
        assert_eq!(default.get_untracked(), 5);
        assert_eq!(
            default.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
        default.set(6);
        assert_eq!(
            default_backend.get("ignore-default").unwrap(),
            Some("6".to_string())
        );

        let manual_backend = MockBackend::with_value("ignore-manual", "7");
        let manual = parse_builder(scope, manual_backend.clone(), "ignore-manual")
            .write_default(WriteDefault::Never)
            .mode(PersistMode::Manual)
            .sync(SyncStrategy::CrossContext)
            .on_remove(RemovePolicy::Ignore)
            .default(5)
            .build();
        manual_backend.state.borrow_mut().remove("ignore-manual");
        manual_backend.emit(
            "ignore-manual",
            BackendEvent::Removed {
                key: "ignore-manual".into(),
            },
        );
        assert_eq!(
            manual.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
        manual.set(8);
        assert_eq!(
            manual.state().get_untracked(),
            PersistenceState::Dirty("8".to_string())
        );
        manual.flush().unwrap();
        assert_eq!(
            manual_backend.get("ignore-manual").unwrap(),
            Some("8".to_string())
        );
        assert_eq!(
            manual.state().get_untracked(),
            PersistenceState::Ready("8".to_string())
        );
    });
}

#[test]
fn write_default_never_manual_none_stays_ready_until_a_local_change() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let backend = MockBackend::default();
        let value = parse_builder(scope, backend.clone(), "never-manual")
            .write_default(WriteDefault::Never)
            .mode(PersistMode::Manual)
            .sync(SyncStrategy::None)
            .on_remove(RemovePolicy::Ignore)
            .default(1)
            .build();
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
        assert_eq!(backend.get("never-manual").unwrap(), None);

        value.set(2);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Dirty("2".to_string())
        );
        value.flush().unwrap();
        assert_eq!(backend.get("never-manual").unwrap(), Some("2".to_string()));
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("2".to_string())
        );
    });
}
