#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let source = scope.rw_signal(String::from("Light"));
        let _ = rx!(scope; $(source.clone()));
    });
}
