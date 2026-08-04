use silex_core::Runtime;
use silex_dom::attribute::PendingAttribute;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    let attribute = runtime.child(|scope| {
        let (value, _) = scope.signal(1_i32);
        PendingAttribute::new_scoped(move |_, _| {
            let _ = value.get();
        })
    });
    require_static(attribute);
}
