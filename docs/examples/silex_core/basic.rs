use silex_core::{
    EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime, SilexError, SilexResult, traits::RxGet,
};
use std::{cell::Cell, error::Error, rc::Rc};

fn handler<'owner>(owner: OwnerAccess<'owner>) -> SilexResult<ErrorHandlerToken<'owner>> {
    owner.error_handler(|_error| {})
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0_i32));

    runtime
        .with_transient(|owner| {
            let source = owner.signal(1_i32)?;
            let doubled = owner.computed(
                move || Ok::<_, SilexError>(source.get()? * 2),
                handler(owner)?,
            )?;
            let seen_for_effect = seen.clone();

            owner.effect(
                EffectPhase::Normal,
                move || {
                    seen_for_effect.set(doubled.get()?);
                    Ok::<(), SilexError>(())
                },
                handler(owner)?,
            )?;

            source.set(2)?;
            assert_eq!(seen.get(), 4);
            Ok::<(), SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
