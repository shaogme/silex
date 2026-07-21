use crate::attribute::PendingAttribute;
use crate::view::View;
use silex_core::reactivity::create_scope;
use web_sys::Node;

/// 一个特殊的视图，它为其子视图创建一个新的响应式作用域。
pub struct ScopeView<V: View> {
    view: V,
}

impl<V: View> ScopeView<V> {
    pub fn new(view: V) -> Self {
        Self { view }
    }
}

impl<V: View> crate::view::ApplyAttributes for ScopeView<V> {}

impl<V: View> View for ScopeView<V> {
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let view = &self.view;
        create_scope(move || {
            view.mount(parent, attrs);
        });
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        create_scope(move || {
            self.view.mount_owned(parent, attrs);
        });
    }
}
