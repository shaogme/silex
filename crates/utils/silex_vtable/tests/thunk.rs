use silex_vtable::{OnceBox, SOO_CAPACITY, ThunkBox};
use std::{cell::Cell, rc::Rc};

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn thunk_calls_and_drops_inline_capture() {
    let drops = Rc::new(Cell::new(0));
    {
        let probe = DropProbe(drops.clone());
        let thunk = ThunkBox::new(move |value: usize| {
            let _ = probe.0.get();
            value + 1
        });

        assert_eq!(thunk.call(41), 42);
        assert_eq!(thunk.call(1), 2);
    }

    assert_eq!(drops.get(), 1);
}

#[test]
fn thunk_calls_and_drops_heap_capture() {
    let drops = Rc::new(Cell::new(0));
    {
        let capture = ([7u8; SOO_CAPACITY + 1], DropProbe(drops.clone()));
        let thunk = ThunkBox::new(move |value: usize| {
            let _ = capture.1.0.get();
            value + usize::from(capture.0[0])
        });

        assert_eq!(thunk.call(35), 42);
    }

    assert_eq!(drops.get(), 1);
}

#[test]
fn thunk_accepts_borrowed_capture() {
    let text = String::from("scoped");
    let thunk = ThunkBox::new(|suffix: &str| format!("{text}-{suffix}"));

    assert_eq!(thunk.call("value"), "scoped-value");
}

#[test]
fn once_box_calls_and_drops_borrowed_capture() {
    let drops = Rc::new(Cell::new(0));
    let text = String::from("scoped");
    {
        let probe = DropProbe(drops.clone());
        let once = OnceBox::new(|suffix: &str| {
            let _ = probe.0.get();
            format!("{text}-{suffix}")
        });

        assert_eq!(once.call("value"), "scoped-value");
    }

    assert_eq!(drops.get(), 1);
}
