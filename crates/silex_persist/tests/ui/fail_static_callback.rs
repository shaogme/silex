use silex_persist::Persistent;

fn static_callback<'scope>(binding: Persistent<'scope, i32>) -> Box<dyn Fn() + 'static> {
    Box::new(move || {
        let _ = binding.get();
    })
}

fn main() {
    let _ = static_callback;
}
