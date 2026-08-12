#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::store;
use silex_persist::Persistent;

#[derive(Clone, Debug, PartialEq)]
#[store]
struct User {
    name: String,
    age: i32,
}

#[derive(Clone, Debug, PartialEq)]
#[store]
struct Generic<'model, T>
where
    T: Clone,
{
    value: T,
    label: &'model str,
}

#[derive(Clone, Debug, PartialEq)]
#[store]
struct Settings {
    pub theme: String,
    pub notifications: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[store]
struct Fixed<const N: usize> {
    values: [u8; N],
}

fn persistent_field_store<'scope>(
    scope: Scope<'scope>,
    theme: Persistent<'scope, String>,
    notifications: RwSignal<'scope, bool>,
) {
    let _settings: SettingsStore<'scope> =
        SettingsStore::from_handles(scope, theme, notifications).unwrap();
    let _settings: SettingsStore<'scope> =
        SettingsStore::from_handles(scope, theme, notifications).unwrap();
}

fn typed_persistent_field_store<'scope>(
    scope: Scope<'scope>,
    theme: Persistent<'scope, String>,
    notifications: RwSignal<'scope, bool>,
) {
    let _settings: SettingsStore<'scope, Persistent<'scope, String>, RwSignal<'scope, bool>> =
        SettingsStore::from_typed_handles(scope, theme, notifications).unwrap();
    let _settings: SettingsStore<'scope, Persistent<'scope, String>, RwSignal<'scope, bool>> =
        SettingsStore::from_typed_handles(scope, theme, notifications).unwrap();
}

fn main() {
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| -> SilexResult<()> {
        let error_handler = scope.error_handler(|_: SilexError| {}).unwrap();
        let user = UserStore::new(
            scope,
            User {
                name: "Alice".to_string(),
                age: 25,
            },
        )
        .unwrap();

        user.name.set("Bob".to_string()).unwrap();
        user.age.update(|age| *age += 1).unwrap();
        assert_eq!(user.snapshot().unwrap().name, "Bob");
        assert_eq!(user.snapshot_untracked().unwrap().age, 26);
        assert!(user.scope() == scope);
        let copied = user;
        assert_eq!(copied.snapshot().unwrap().name, "Bob");

        let name = scope.rw_signal("Carol".to_string()).unwrap();
        let age = scope.rw_signal(30).unwrap();
        let from_handles: UserStore<'_> = UserStore::from_handles(scope, name, age).unwrap();
        assert_eq!(from_handles.snapshot().unwrap().name, "Carol");

        let settings = SettingsStore::new(
            scope,
            Settings {
                theme: "Light".to_string(),
                notifications: false,
            },
        )
        .unwrap();
        let theme = rx!(scope; error_handler; $(settings.theme).clone());
        let label = rx!(scope; error_handler; format!("Theme: {}", $(settings.theme)));
        assert_eq!(theme.get()?, "Light");
        assert_eq!(label.get()?, "Theme: Light");

        let value = scope.rw_signal(7u32).unwrap();
        let label = "generic";
        let generic = GenericStore::new(scope, Generic { value: 7, label }).unwrap();
        let _ = (value, generic.snapshot().unwrap());

        let fixed = FixedStore::new(scope, Fixed { values: [1, 2] }).unwrap();
        assert_eq!(fixed.snapshot().unwrap().values, [1, 2]);
        let _ = (persistent_field_store, typed_persistent_field_store);
        Ok(())
    })
    .unwrap()
    .unwrap();
}
