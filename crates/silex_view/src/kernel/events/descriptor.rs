use silex_core::SilexResult;
pub use silex_dom::model::event::{
    DomEvent, DomRectData, EventDescriptor, EventKind, EventSpec, MouseEventData, PointerEventData,
    WindowEventRequest,
};
use std::borrow::Cow;
/// 事件 handler 的参数模式。
pub struct WithEventArg;
pub struct WithoutEventArg;

pub trait EventHandler<'scope, M> {
    fn into_handler(self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope>;
}

impl<'scope, F> EventHandler<'scope, WithEventArg> for F
where
    F: FnMut(DomEvent) -> SilexResult<()> + 'scope,
{
    fn into_handler(self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope> {
        Box::new(self)
    }
}

impl<'scope, F> EventHandler<'scope, WithoutEventArg> for F
where
    F: FnMut() -> SilexResult<()> + 'scope,
{
    fn into_handler(mut self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope> {
        Box::new(move |_| self())
    }
}

/// 常用事件描述符。事件 payload 统一是 backend-neutral [`DomEvent`]。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    name: &'static str,
    kind: EventKind,
}

impl Event {
    pub const fn new(name: &'static str, kind: EventKind) -> Self {
        Self { name, kind }
    }
}

impl EventDescriptor for Event {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(self.name)
    }

    fn spec(&self) -> EventSpec {
        EventSpec::new(self.name, self.kind)
    }
}

macro_rules! define_events {
    ($($name:ident : $event_name:literal => $kind:ident),* $(,)?) => {
        $( #[allow(non_upper_case_globals, dead_code)] pub const $name: Event = Event::new($event_name, EventKind::$kind); )*
    };
}

define_events!(
    click: "click" => Mouse,
    dblclick: "dblclick" => Mouse,
    input: "input" => Input,
    change: "change" => Form,
    keydown: "keydown" => Keyboard,
    keyup: "keyup" => Keyboard,
    focus: "focus" => Focus,
    blur: "blur" => Focus,
    mouseenter: "mouseenter" => Mouse,
    mouseleave: "mouseleave" => Mouse,
    pointerdown: "pointerdown" => Pointer,
    pointerup: "pointerup" => Pointer,
    pointermove: "pointermove" => Pointer,
    pointercancel: "pointercancel" => Pointer,
    submit: "submit" => Form,
    wheel: "wheel" => Wheel,
);
