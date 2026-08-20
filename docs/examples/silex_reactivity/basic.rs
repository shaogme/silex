use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, ErrorHandlerToken, OwnerAccess, ReactiveError,
    Runtime,
};

fn handler<'scope, E: 'scope>(
    scope: OwnerAccess<'scope>,
) -> Result<ErrorHandlerToken<'scope, E>, ReactiveError> {
    scope.error_handler(|_| {})
}

fn computation_error(error: ComputationInitError<ReactiveError>) -> ReactiveError {
    match error {
        ComputationInitError::Registration(error) | ComputationInitError::Initial(error) => error,
    }
}

fn invoke_error(error: CallbackInvokeError<ReactiveError>) -> ReactiveError {
    match error {
        CallbackInvokeError::Runtime(error) | CallbackInvokeError::User(error) => error,
        CallbackInvokeError::Handler(error) => ReactiveError::Handler(error),
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(1_i32)?;
            let doubled = scope
                .computed(
                    move || source.get().map(|value| value * 2),
                    handler::<ReactiveError>(scope)?,
                )
                .map_err(computation_error)?;

            scope
                .effect(
                    move || {
                        println!("{}", doubled.get().map_err(invoke_error)?);
                        Ok::<(), ReactiveError>(())
                    },
                    handler::<ReactiveError>(scope)?,
                )
                .map_err(computation_error)?;

            set_source.set(2)?;
            Ok::<(), ReactiveError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)??;

    Ok(())
}
