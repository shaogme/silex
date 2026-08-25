//! 不依赖具体浏览器类型的 View 辅助函数。

use crate::context::MountContext;
use crate::event::{EventDescriptor, EventHandler, bind_event};
use crate::owner::MountOwnerToken;
use silex_core::{ErrorHandlerInput, SilexResult};
use silex_dom::{
    model::{DomDocument, DomElement, DomNode, event::DomEvent},
    runtime::DomContext,
};

/// 从显式 DOM context 获取 document。
pub fn document(context: &DomContext) -> SilexResult<DomDocument> {
    context.document().map_err(Into::into)
}

/// 返回 opaque event 的目标节点。
pub fn event_target(event: &DomEvent) -> &DomNode {
    event.target()
}

/// 在当前 owner 下注册一个物理事件监听器。
pub fn listen<'scope, E, F, M, H>(
    context: &MountContext<'scope>,
    element: &DomElement,
    event: E,
    callback: F,
    error_handler: H,
) -> SilexResult<()>
where
    E: EventDescriptor,
    F: EventHandler<'scope, M> + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    bind_event(context, element, event, callback, error_handler)
}

/// 验证 owner 当前仍可创建宿主资源。
pub fn ensure_owner_active<'scope>(owner: &MountOwnerToken<'scope>) -> SilexResult<()> {
    if owner.is_active()? {
        Ok(())
    } else {
        Err(silex_core::SilexError::fatal(
            silex_core::SilexErrorKind::Reactivity(silex_core::ReactiveError::NoSuchNode),
        ))
    }
}
