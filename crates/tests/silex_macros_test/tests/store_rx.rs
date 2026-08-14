use std::{cell::Cell, rc::Rc};

use silex_core::{Runtime, SilexContext, SilexError, SilexResult, rx};
use silex_macros::store;

#[derive(Clone)]
#[store]
struct Settings {
    theme: String,
    notifications: bool,
}

#[test]
fn explicit_store_field_source_tracks_only_selected_field() {
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| -> SilexResult<()> {
            let settings = SettingsStore::new(
                scope,
                Settings {
                    theme: "Light".to_string(),
                    notifications: false,
                },
            )
            .unwrap();
            let error_handler = scope.error_handler(|_: SilexError| {}).unwrap();
            let ctx = SilexContext::new(scope, error_handler);
            let theme = rx!(ctx; $(settings.theme).clone());
            let runs = Rc::new(Cell::new(0));
            let runs_for_effect = runs.clone();

            let _effect = scope
                .effect(
                    move || {
                        let _ = theme.get()?;
                        runs_for_effect.set(runs_for_effect.get() + 1);
                        Ok(())
                    },
                    error_handler,
                )
                .unwrap();

            assert_eq!(runs.get(), 1);
            settings.notifications.set(true).unwrap();
            assert_eq!(runs.get(), 1);

            settings.theme.set("Dark".to_string()).unwrap();
            assert_eq!(theme.get()?, "Dark");
            assert_eq!(runs.get(), 2);
            Ok(())
        })
        .unwrap()
        .unwrap();
}
