#![deny(warnings)]

use silex_core::{Constant, RxReadOption};

fn main() {
    let source = Constant::new(Some(1_u32));
    let mut escaped = None;
    RxReadOption::with(&source, |value| {
        escaped = value;
    });
    let _ = escaped;
}
