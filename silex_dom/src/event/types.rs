use super::EventDescriptor;
use std::borrow::Cow;

macro_rules! generate_events {
    ($($name:ident : $type:ty),* $(,)?) => {
        $(
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone, Debug, Default)]
            pub struct $name;

            impl EventDescriptor for $name {
                type EventType = $type;
                fn name(&self) -> Cow<'static, str> {
                    stringify!($name).into()
                }
            }
        )*
    };
}

// === Mouse Events ===
generate_events! {
    click: web_sys::MouseEvent,
    dblclick: web_sys::MouseEvent,
    mousedown: web_sys::MouseEvent,
    mouseup: web_sys::MouseEvent,
    mousemove: web_sys::MouseEvent,
    mouseover: web_sys::MouseEvent,
    mouseout: web_sys::MouseEvent,
    mouseenter: web_sys::MouseEvent,
    mouseleave: web_sys::MouseEvent,
    contextmenu: web_sys::MouseEvent,
}

// === Keyboard Events ===
generate_events! {
    keydown: web_sys::KeyboardEvent,
    keypress: web_sys::KeyboardEvent,
    keyup: web_sys::KeyboardEvent,
}

// === Form Events ===
generate_events! {
    change: web_sys::Event, // 'change' target is useful, but the event itself is generic
    input: web_sys::InputEvent,
    submit: web_sys::SubmitEvent,
    reset: web_sys::Event,
    invalid: web_sys::Event,
}

// === Focus Events ===
generate_events! {
    focus: web_sys::FocusEvent,
    blur: web_sys::FocusEvent,
    focusin: web_sys::FocusEvent,
    focusout: web_sys::FocusEvent,
}

// === UI Events ===
generate_events! {
    scroll: web_sys::Event,
    resize: web_sys::UiEvent,
    load: web_sys::Event,
    unload: web_sys::Event,
    abort: web_sys::UiEvent,
    error: web_sys::ErrorEvent,
    select: web_sys::Event,
}

// === Pointer Events ===
generate_events! {
    pointerdown: web_sys::PointerEvent,
    pointermove: web_sys::PointerEvent,
    pointerup: web_sys::PointerEvent,
    pointercancel: web_sys::PointerEvent,
    pointerenter: web_sys::PointerEvent,
    pointerleave: web_sys::PointerEvent,
    pointerover: web_sys::PointerEvent,
    pointerout: web_sys::PointerEvent,
    gotpointercapture: web_sys::PointerEvent,
    lostpointercapture: web_sys::PointerEvent,
}

// === Drag Events ===
generate_events! {
    drag: web_sys::DragEvent,
    dragend: web_sys::DragEvent,
    dragenter: web_sys::DragEvent,
    dragexit: web_sys::DragEvent,
    dragleave: web_sys::DragEvent,
    dragover: web_sys::DragEvent,
    dragstart: web_sys::DragEvent,
    drop: web_sys::DragEvent,
}

// === Touch Events ===
generate_events! {
    touchstart: web_sys::TouchEvent,
    touchend: web_sys::TouchEvent,
    touchmove: web_sys::TouchEvent,
    touchcancel: web_sys::TouchEvent,
}

// === Wheel Events ===
generate_events! {
    wheel: web_sys::WheelEvent,
}

// === Animation & Transition ===
generate_events! {
    animationstart: web_sys::AnimationEvent,
    animationend: web_sys::AnimationEvent,
    animationiteration: web_sys::AnimationEvent,
    transitionend: web_sys::TransitionEvent,
}

// === Composition Events ===
generate_events! {
    compositionstart: web_sys::CompositionEvent,
    compositionupdate: web_sys::CompositionEvent,
    compositionend: web_sys::CompositionEvent,
}
