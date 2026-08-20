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

fn persistent_field_store<'owner>(
    owner: OwnerAccess<'owner>,
    theme: Persistent<'owner, String>,
    notifications: RwSignal<'owner, bool>,
) {
    let _settings: SettingsStore<'owner> =
        SettingsStore::from_handles(owner, theme, notifications).unwrap();
    let _settings: SettingsStore<'owner> =
        SettingsStore::from_handles(owner, theme, notifications).unwrap();
}

fn typed_persistent_field_store<'owner>(
    owner: OwnerAccess<'owner>,
    theme: Persistent<'owner, String>,
    notifications: RwSignal<'owner, bool>,
) {
    let _settings: SettingsStore<'owner, Persistent<'owner, String>, RwSignal<'owner, bool>> =
        SettingsStore::from_typed_handles(owner, theme, notifications).unwrap();
    let _settings: SettingsStore<'owner, Persistent<'owner, String>, RwSignal<'owner, bool>> =
        SettingsStore::from_typed_handles(owner, theme, notifications).unwrap();
}

fn main() {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| -> SilexResult<()> {
        let error_handler = owner.error_handler(|_: SilexError| {}).unwrap();
        let user = UserStore::new(
            owner,
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
        assert!(user.owner() == owner);
        let copied = user;
        assert_eq!(copied.snapshot().unwrap().name, "Bob");

        let name = owner.rw_signal("Carol".to_string()).unwrap();
        let age = owner.rw_signal(30).unwrap();
        let from_handles: UserStore<'_> = UserStore::from_handles(owner, name, age).unwrap();
        assert_eq!(from_handles.snapshot().unwrap().name, "Carol");

        let settings = SettingsStore::new(
            owner,
            Settings {
                theme: "Light".to_string(),
                notifications: false,
            },
        )
        .unwrap();
        let ctx = SilexContext::new(owner, error_handler.view());
        let theme = rx!(ctx; $(settings.theme).clone())?;
        let label = rx!(ctx; format!("Theme: {}", $(settings.theme)))?;
        assert_eq!(theme.get()?, "Light");
        assert_eq!(label.get()?, "Theme: Light");

        let value = owner.rw_signal(7u32).unwrap();
        let label = "generic";
        let generic = GenericStore::new(owner, Generic { value: 7, label }).unwrap();
        let _ = (value, generic.snapshot().unwrap());

        let fixed = FixedStore::new(owner, Fixed { values: [1, 2] }).unwrap();
        assert_eq!(fixed.snapshot().unwrap().values, [1, 2]);
        let _ = (persistent_field_store, typed_persistent_field_store);
        Ok(())
    })
    .unwrap()
    .unwrap();
}
