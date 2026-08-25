use std::{cell::Cell, rc::Rc};

use silex_dom::{
    adapters::ssr::SsrDom,
    model::{
        event::{DomEventBridge, EventKind, EventSpec, PhysicalEventRequest},
        node::ElementSpec,
    },
};

#[test]
fn ssr_listener_lease_is_recorded_and_retractable() {
    let dom = SsrDom::new();
    let context = dom.context();
    let element = context
        .create_element(ElementSpec::new("button"))
        .expect("button should exist");
    let callback_calls = Rc::new(Cell::new(0));
    let callback_calls_for_bridge = callback_calls.clone();
    let bridge: Rc<dyn DomEventBridge> = Rc::new(move |_| {
        callback_calls_for_bridge.set(callback_calls_for_bridge.get() + 1);
        Ok(())
    });
    let resource = context
        .listen(
            PhysicalEventRequest::new(&element, EventSpec::new("click", EventKind::Mouse))
                .with_bridge(bridge),
        )
        .expect("SSR listener should create a lease");
    assert!(resource.is_active());
    assert_eq!(callback_calls.get(), 0);
    let record = dom.event_records().pop().expect("event record");
    assert!(record.id > 0);
    resource.cancel().expect("SSR lease should be cancellable");
    resource
        .cancel()
        .expect("repeated SSR lease cancellation should be inert");
    assert_eq!(dom.event_records().len(), 0);
}
