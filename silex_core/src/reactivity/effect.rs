use std::cell::{Cell, RefCell};
use std::rc::Rc;

use silex_reactivity::NodeId;

// --- Effect ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Effect {
    pub(crate) id: NodeId,
}

impl Effect {
    pub fn new<T, F>(f: F) -> Self
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
    {
        let val = Rc::new(RefCell::new(None::<T>));
        let val_clone = val.clone();

        let id = silex_reactivity::effect(move || {
            let old = val_clone.borrow_mut().take();
            let new = f(old);
            *val_clone.borrow_mut() = Some(new);
        });
        Effect { id }
    }

    pub fn watch<W, T, C>(deps: W, callback: C, immediate: bool) -> Self
    where
        W: Fn() -> T + 'static,
        T: Clone + PartialEq + 'static,
        C: Fn(&T, Option<&T>, Option<()>) + 'static,
    {
        let first_run = Rc::new(Cell::new(true));
        let prev_deps = Rc::new(RefCell::new(None::<T>));

        Effect::new(move |_| {
            let new_val = deps();
            let mut p_borrow = prev_deps.borrow_mut();
            let old_val = p_borrow.clone();

            let is_first = first_run.get();
            if is_first {
                first_run.set(false);
                *p_borrow = Some(new_val.clone());
                if immediate {
                    callback(&new_val, old_val.as_ref(), None);
                }
            } else if old_val.as_ref() != Some(&new_val) {
                callback(&new_val, old_val.as_ref(), None);
                *p_borrow = Some(new_val);
            }
        })
    }
}
