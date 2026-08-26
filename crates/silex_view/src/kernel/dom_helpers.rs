use silex_dom::model::{DomNode, event::DomEvent};

/// 返回 opaque event 的目标节点。
pub fn event_target(event: &DomEvent) -> &DomNode {
    event.target()
}
