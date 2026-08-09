#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::store;

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
        let from_handles = UserStore::try_from_handles(scope, name, age).unwrap();
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
    });
}
