use ref_str::LocalStaticRefStr;
use silex_core::{ErrorHandlerToken, ErrorReporter, OwnerAccess, Runtime, SilexError, SilexResult};
use silex_persist::{
    BackendEventSink, BackendSubscribeError, BackendSubscription, PersistExternalSync,
    PersistenceBackend, PersistenceError, Persistent, WriteDefault,
};
use std::{cell::RefCell, collections::HashMap, error::Error, rc::Rc};

#[derive(Clone, Default)]
struct MemoryBackend {
    values: Rc<RefCell<HashMap<String, String>>>,
}

impl<'scope> PersistenceBackend<'scope> for MemoryBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        Ok(self.values.borrow().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        self.values
            .borrow_mut()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        self.values.borrow_mut().remove(key);
        Ok(())
    }

    fn subscribe(
        &self,
        _owner: OwnerAccess<'scope>,
        _key: impl Into<LocalStaticRefStr>,
        _sink: BackendEventSink,
        _error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        Ok(BackendSubscription::new(|| {}))
    }
}

fn handler<'scope>(owner: OwnerAccess<'scope>) -> SilexResult<ErrorHandlerToken<'scope>> {
    owner.error_handler(|_| {})
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();
    let backend = MemoryBackend::default();

    runtime
        .with_transient(|owner| {
            let binding = Persistent::builder(owner, "counter", handler(owner)?)
                .backend(backend.clone())
                .parse::<i32>()
                .write_default(WriteDefault::Never)
                .external_sync(PersistExternalSync::Disabled)
                .default(0)
                .build()?;

            binding.set(2)?;
            assert_eq!(binding.get_untracked()?, 2);
            assert!(binding.has_persisted_value()?);
            assert_eq!(backend.get("counter")?, Some("2".to_string()));
            Ok::<(), SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
