#![deny(warnings)]

use silex_core::{Constant, RxReadTuple2};

fn main() {
    let source = (Constant::new(1_u32), Constant::new(2_u32));
    let mut escaped = None;
    RxReadTuple2::with(&source, |value| {
        escaped = Some(value);
    });
    let _ = escaped;
}
