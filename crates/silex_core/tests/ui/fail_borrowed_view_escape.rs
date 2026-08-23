#![deny(warnings)]

use silex_core::{Constant, RxReadRef};

fn main() {
    let source = Constant::new(1_u32);
    let mut escaped = None;
    RxReadRef::with(&source, |value| {
        escaped = Some(value);
    });
    let _ = escaped;
}
