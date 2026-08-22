#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn ReactiveInputComponent<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(default)] signal: Rx<'owner, String>,
    #[chain(default)] read: ReadSignal<'owner, i32>,
    #[chain(default)] rw: Signal<'owner, bool>,
    #[chain(default)] computed: Computed<'owner, f64>,
    #[chain(default)] stored: StoredValue<'owner, char>,
    #[chain(default)] rx: Rx<'owner, usize>,
) -> impl View<'owner> {
    let _ = (owner, signal, read, rw, computed, stored, rx);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| -> SilexResult<()> {
            let read_string = owner.signal(String::from("source"))?;
            let read_int = owner.signal(1_i32)?;
            let rw_bool = owner.signal(false)?;
            let error_handler = owner.error_handler(|_| {})?;
            let ctx = SilexContext::new(owner, error_handler.view());
            let memo_float = owner.computed(|| Ok(1.0_f64), error_handler.view())?;
            let stored_char = owner.stored('s')?;
            let rx_usize = owner.constant(1_usize)?;

            let _builder = ReactiveInputComponent(ctx, AnyView::Empty)
                .signal("constant")?
                .signal(read_string)?
                .read(2_i32)?
                .read(read_int)?
                .rw(true)?
                .rw(rw_bool)?
                .computed(2.0_f64)?
                .computed(memo_float)?
                .stored('c')?
                .stored(stored_char)?
                .rx(2_usize)?
                .rx(rx_usize)?;
            let _ = _builder;
            Ok(())
        })
        .unwrap()
        .unwrap();
}
