#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| -> SilexResult<()> {
            let source = owner.signal(String::from("Light"))?;
            let error_handler = owner.error_handler(|_| {})?;
            let _ = rx!(owner; error_handler; $(source.clone()));
            Ok(())
        })
        .unwrap()
        .unwrap();
}
