#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::{css, global, inject_css, tw, tw_variants};

tw_variants! {
    pub struct NumericVariants {
        base: "block",
        variants: {
            size: {
                "1x": "p-1",
                sm: "p-2",
            }
        },
        default_variants: { size: "1x" }
    }
}

global! {
    pub StaticGlobal<'scope>(scope: silex_core::Scope<'scope>) {
        body { color: red; }
    }
}

fn main() {
    let _ = css! { color: red; };
    let _ = tw!("inline-flex items-center");
    let _ = NumericVariants::new().get_checked("1x");
    inject_css! { :root { --test-color: red; } };
}
