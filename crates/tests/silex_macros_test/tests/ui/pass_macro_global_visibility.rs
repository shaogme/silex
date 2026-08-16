#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::global;

mod public_global {
    use super::global;

    global! {
        pub PublicGlobal<'owner>(owner: silex_core::OwnerAccess<'owner>) {
            body { color: red; }
        }
    }
}

mod crate_global {
    use super::global;

    global! {
        pub(crate) CrateGlobal<'owner>(owner: silex_core::OwnerAccess<'owner>) {
            body { color: green; }
        }
    }
}

mod super_global {
    use super::global;

    global! {
        pub(super) SuperGlobal<'owner>(owner: silex_core::OwnerAccess<'owner>) {
            body { color: blue; }
        }
    }
}

fn main() {
    let mut runtime = silex_core::Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let _ = public_global::PublicGlobal(owner);
        let _ = crate_global::CrateGlobal(owner);
        let _ = super_global::SuperGlobal(owner);
    });
}
