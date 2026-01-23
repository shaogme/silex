use crate::router::context::use_router;
use silex_core::dom::WithText;
use silex_core::dom::element::TypedElement;
use silex_core::dom::tags::A as TagA;
use silex_core::dom::{View, tag::a};

/// `A` 组件结构体
pub struct A {
    href: String,
    inner: TypedElement<TagA>,
}

/// 创建一个链接组件，用于在应用内导航
///
/// 类似于 HTML 的 `<a>` 标签，但会拦截点击事件并使用 Router 导航，而不是刷新页面。
#[allow(non_snake_case)]
pub fn Link(href: &str) -> A {
    let element = a(()).attr("href", href);
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

    /// 设置激活时的 CSS 类 (当当前路径匹配 href 时添加)
    pub fn active_class(self, name: &str) -> Self {
        // 尝试获取 Router 上下文中的 path 信号
        if let Some(router) = use_router() {
            let path_signal = router.path;
            let href = self.href.clone();
            let class_name = name.to_string();

            let is_active = move || {
                let current_path = path_signal.get();
                if href == "/" {
                    current_path == "/"
                } else if current_path == href {
                    true
                } else if current_path.starts_with(&href) {
                    // 确保是路径段匹配，避免 /user 匹配 /users
                    if href.ends_with('/') {
                        true
                    } else {
                        current_path.chars().nth(href.len()) == Some('/')
                    }
                } else {
                    false
                }
            };

            Self {
                inner: self.inner.class((class_name, is_active)),
                ..self
            }
        } else {
            self
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

        // 在绑定事件前，根据 Router 的 base_path 更新 DOM 元素的 href 属性
        // 这样可以保证原生行为（如右键打开新标签页）指向正确的物理路径
        if let Some(ctx) = use_router() {
            if !ctx.base_path.is_empty() && ctx.base_path != "/" && href.starts_with('/') {
                let base = ctx.base_path.trim_end_matches('/');
                let full_href = format!("{}{}", base, href);
                let _ = self.inner.dom_element.set_attribute("href", &full_href);
            }
        }

        // 绑定点击事件
        let element = self.inner.on_click(move |e: web_sys::MouseEvent| {
            // 阻止默认跳转行为
            e.prevent_default();

            // 使用 router 导航
            if let Some(ctx) = use_router() {
                // 注意：这里仍然传递逻辑路径 (href)，Navigator 会自动处理 base_path
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
