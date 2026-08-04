use silex_core::reactivity::{Memo, Signal, StoredValue};

fn main() {
    let _ = Signal::pair(1_i32);
    let _ = Memo::new(|_| 1_i32);
    let _ = StoredValue::new(1_i32);
}
