#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
#[silex_component(builder = ExplicitBuilder, render = render_explicit)]
struct MissingProductProps {
    value: String,
}

fn render_explicit(_: MissingProductProps) {}

fn main() {}
