use silex_core::{ReadSignal, WriteSignal};
use silex_dom::{
    attribute::GlobalEventAttributes,
    element::Element,
    helpers::{debounce_owned, set_timeout_owned, window_event_listener_owned},
    view::ViewOwnerToken,
};
use std::time::Duration;

#[allow(dead_code)]
fn compile_element<'scope, 'run>(
    read: ReadSignal<'scope, 'run, i32>,
    write: WriteSignal<'scope, 'run, i32>,
    borrowed_ref: &'scope str,
) -> Element<'scope, 'run> {
    Element::new("button").on_click(move |_| {
        let _ = borrowed_ref;
        write.set(read.get() + 1);
    })
}

#[allow(dead_code)]
fn compile_owned<'scope, 'run>(
    token: &ViewOwnerToken<'scope, 'run>,
    read: ReadSignal<'scope, 'run, i32>,
    write: WriteSignal<'scope, 'run, i32>,
    borrowed_ref: &'scope str,
) {
    let _timeout = set_timeout_owned(
        token,
        move || write.set(read.get() + borrowed_ref.len() as i32),
        Duration::from_millis(1),
    );
    let _listener = window_event_listener_owned(token, silex_dom::event::click, move |_| {
        let _ = borrowed_ref;
        write.set(read.get() + 1);
    });
    let _debounced = debounce_owned(token, Duration::from_millis(1), move |_: i32| {
        write.set(read.get() + 1);
    });
}

fn main() {}
