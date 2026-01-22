use crate::dom::{Element, View, tag::a};
use crate::router::context::use_router;

/// `A` 组件结构体
pub struct A {
    href: String,
    inner: Element,
}

/// 创建一个链接组件，用于在应用内导航
///
/// 类似于 HTML 的 `<a>` 标签，但会拦截点击事件并使用 Router 导航，而不是刷新页面。
pub fn link(href: &str) -> A {
    let element = a().attr("href", href);
    A {
        href: href.to_string(),
        inner: element,
    }
}

impl A {
    /// 设置链接文本
    pub fn text(self, content: &str) -> Self {
        Self {
            inner: self.inner.text(content),
            ..self
        }
    }

    /// 设置 CSS 类
    pub fn class(self, name: &str) -> Self {
        Self {
            inner: self.inner.class(name),
            ..self
        }
    }

    /// 设置样式
    pub fn style(self, css: &str) -> Self {
        Self {
            inner: self.inner.style(css),
            ..self
        }
    }

    /// 添加子组件
    pub fn child<V: View>(self, view: V) -> Self {
        Self {
            inner: self.inner.child(view),
            ..self
        }
    }
}

impl View for A {
    fn mount(self, parent: &web_sys::Node) {
        let href = self.href.clone();

        // 绑定点击事件
        let element = self.inner.on_click(move |e: web_sys::MouseEvent| {
            // 阻止默认跳转行为
            e.prevent_default();

            // 使用 router 导航
            if let Some(ctx) = use_router() {
                ctx.navigator.push(&href);
            } else {
                // 如果没有 router，回退到普通跳转（或者是警告）
                let window = web_sys::window().unwrap();
                let _ = window.location().set_href(&href);
            }
        });

        element.mount(parent);
    }
}
