#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| -> SilexResult<()> {
            let source = scope.rw_signal(String::from("Light"))?;
            let error_handler = scope.error_handler(|_| {})?;
            let _ = rx!(scope; error_handler; $(source.clone()));
            Ok(())
        })
        .unwrap()
        .unwrap();
}
