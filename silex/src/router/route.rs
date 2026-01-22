use crate::dom::view::{AnyView, IntoAnyView, View};
use std::rc::Rc;

/// 路由定义节点
#[derive(Clone)]
pub struct Route {
    pub(crate) path: String,
    pub(crate) children: Vec<Route>,
    pub(crate) view: Rc<dyn Fn() -> AnyView>,
}

impl Route {
    /// 创建一个新的路由节点
    ///
    /// # Arguments
    /// * `path` - 路径模式 (e.g. "/", "users", ":id")
    /// * `view_fn` - 渲染该路由的组件函数
    pub fn new<V, F>(path: &str, view_fn: F) -> Self
    where
        V: View + 'static,
        F: Fn() -> V + 'static,
    {
        Self {
            path: path.to_string(),
            children: Vec::new(),
            view: Rc::new(move || view_fn().into_any()),
        }
    }

    /// 添加子路由
    pub fn children(mut self, children: Vec<Route>) -> Self {
        self.children = children;
        self
    }
}
