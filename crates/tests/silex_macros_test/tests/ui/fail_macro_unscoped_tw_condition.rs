#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::tw;

fn main() {
    let condition = true;
    let _ = tw!(
        "inline-flex",
        (condition, "bg-blue-500", "bg-slate-500")
    );
}
