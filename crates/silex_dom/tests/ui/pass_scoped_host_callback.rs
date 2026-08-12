use silex_core::{ReadSignal, SilexResult, WriteSignal};
use silex_dom::{
    attribute::GlobalEventAttributes,
    element::Element,
    helpers::{
        debounce, queue_microtask, request_animation_frame, request_idle_callback, set_interval,
        set_timeout, window_event_listener_untyped,
    },
    view::ViewOwnerToken,
};
use std::time::Duration;

#[allow(dead_code)]
fn compile_element<'scope>(
    read: ReadSignal<'scope, i32>,
    write: WriteSignal<'scope, i32>,
    borrowed_ref: &'scope str,
) -> Element<'scope> {
    Element::new("button").on_click(move |_| {
        let _ = borrowed_ref;
        write.set(read.get()? + 1)?;
        Ok(())
    })
}

#[allow(dead_code)]
fn compile_owned<'scope>(
    token: &ViewOwnerToken<'scope>,
    read: ReadSignal<'scope, i32>,
    write: WriteSignal<'scope, i32>,
    borrowed_ref: &'scope str,
) -> SilexResult<()> {
    let _timeout = set_timeout(
        token,
        move || {
            write.set(read.get()? + borrowed_ref.len() as i32)?;
            Ok(())
        },
        Duration::from_millis(1),
    );
    let _interval = set_interval(
        token,
        move || {
            write.set(read.get()? + borrowed_ref.len() as i32)?;
            Ok(())
        },
        Duration::from_millis(1),
    );
    let _frame = request_animation_frame(token, move || {
        write.set(read.get()? + borrowed_ref.len() as i32)?;
        Ok(())
    });
    let _idle = request_idle_callback(token, move || {
        write.set(read.get()? + borrowed_ref.len() as i32)?;
        Ok(())
    });
    let _microtask = queue_microtask(token, move || {
        write.set(read.get()? + borrowed_ref.len() as i32)?;
        Ok(())
    });
    let _listener = window_event_listener_untyped(token, "click", move |_| {
        let _ = borrowed_ref;
        write.set(read.get()? + 1)?;
        Ok(())
    });
    let _debounced = debounce(token, Duration::from_millis(1), move |_: i32| {
        write.set(read.get()? + 1)?;
        Ok(())
    })?;
    Ok(())
}

fn main() {}
