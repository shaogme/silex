#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::global;

mod public_global {
    use super::global;

    global! {
        pub {
            body { color: red; }
        }
    }
}

mod crate_global {
    use super::global;

    global! {
        pub(crate) {
            body { color: green; }
        }
    }
}

mod super_global {
    use super::global;

    global! {
        pub(super) {
            body { color: blue; }
        }
    }
}

fn main() {
    let _ = public_global::GlobalStyles();
    let _ = crate_global::GlobalStyles();
    let _ = super_global::GlobalStyles();
}
