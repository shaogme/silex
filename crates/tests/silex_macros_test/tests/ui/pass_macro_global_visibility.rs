#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::global;

mod public_global {
    use super::global;

    global! {
        pub PublicGlobal<'scope>(scope: silex_core::Scope<'scope>) {
            body { color: red; }
        }
    }
}

mod crate_global {
    use super::global;

    global! {
        pub(crate) CrateGlobal<'scope>(scope: silex_core::Scope<'scope>) {
            body { color: green; }
        }
    }
}

mod super_global {
    use super::global;

    global! {
        pub(super) SuperGlobal<'scope>(scope: silex_core::Scope<'scope>) {
            body { color: blue; }
        }
    }
}

fn main() {
    let mut runtime = silex_core::Runtime::new();
    runtime.child(|scope| {
        let _ = public_global::PublicGlobal(scope);
        let _ = crate_global::CrateGlobal(scope);
        let _ = super_global::SuperGlobal(scope);
    });
}
