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
        SettingsStore::try_from_handles(scope, theme, notifications).unwrap();
    let _settings: SettingsStore<'scope> =
        SettingsStore::from_handles(scope, theme, notifications);
}

fn typed_persistent_field_store<'scope>(
    scope: Scope<'scope>,
    theme: Persistent<'scope, String>,
    notifications: RwSignal<'scope, bool>,
) {
    let _settings: SettingsStore<
        'scope,
        Persistent<'scope, String>,
        RwSignal<'scope, bool>,
    > = SettingsStore::try_from_typed_handles(scope, theme, notifications).unwrap();
    let _settings: SettingsStore<'scope, Persistent<'scope, String>, RwSignal<'scope, bool>> =
        SettingsStore::from_typed_handles(scope, theme, notifications);
}

fn main() {
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let user = UserStore::new(
            scope,
            User {
                name: "Alice".to_string(),
                age: 25,
            },
        );

        user.name.set("Bob".to_string());
        user.age.update(|age| *age += 1);
        assert_eq!(user.snapshot().name, "Bob");
        assert_eq!(user.snapshot_untracked().age, 26);
        assert!(user.scope() == scope);
        let copied = user;
        assert_eq!(copied.snapshot().name, "Bob");

        let name = scope.rw_signal("Carol".to_string());
        let age = scope.rw_signal(30);
        let from_handles: UserStore<'_> = UserStore::try_from_handles(scope, name, age).unwrap();
        assert_eq!(from_handles.snapshot().name, "Carol");

        let value = scope.rw_signal(7u32);
        let label = "generic";
        let generic = GenericStore::new(
            scope,
            Generic {
                value: 7,
                label,
            },
        );
        let _ = (value, generic.snapshot());

        let fixed = FixedStore::new(scope, Fixed { values: [1, 2] });
        assert_eq!(fixed.snapshot().values, [1, 2]);
        let _ = (persistent_field_store, typed_persistent_field_store);
    });
}
