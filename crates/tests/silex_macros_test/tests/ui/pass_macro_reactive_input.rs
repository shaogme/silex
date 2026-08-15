#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn ReactiveInputComponent<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain(default)] signal: Signal<'scope, String>,
    #[chain(default)] read: ReadSignal<'scope, i32>,
    #[chain(default)] rw: RwSignal<'scope, bool>,
    #[chain(default)] memo: Memo<'scope, f64>,
    #[chain(default)] stored: StoredValue<'scope, char>,
    #[chain(default)] rx: Rx<'scope, usize>,
) -> impl View<'scope> {
    let _ = (scope, signal, read, rw, memo, stored, rx);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| -> SilexResult<()> {
            let (read_string, _) = scope.signal(String::from("source"))?;
            let (read_int, _) = scope.signal(1_i32)?;
            let rw_bool = scope.rw_signal(false)?;
            let error_handler = scope.error_handler(|_| {})?;
            let ctx = SilexContext::new(scope, error_handler.view());
            let memo_float = scope.memo(|_| Ok(1.0_f64), error_handler.view())?;
            let stored_char = scope.stored('s')?;
            let rx_usize = scope.constant(1_usize)?;

            let _builder = ReactiveInputComponent(ctx, AnyView::Empty)
                .signal("constant")?
                .signal(read_string)?
                .read(2_i32)?
                .read(read_int)?
                .rw(true)?
                .rw(rw_bool)?
                .memo(2.0_f64)?
                .memo(memo_float)?
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
