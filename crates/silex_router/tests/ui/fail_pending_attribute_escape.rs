use silex_core::{Runtime, RxGet};
use silex_view::attribute::AttrOp;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    let attribute = runtime.with_transient(|owner| {
        let value = owner.signal(1_i32).expect("value signal should be created");
        AttrOp::new_scoped(move |_, _| {
            let _ = value.get();
            Ok(())
        })
    });
    require_static(attribute);
}
