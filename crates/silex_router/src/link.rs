use crate::{ToRoute, context::RouterContext};
use silex_core::SilexResult;
use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_html::a;
use silex_macros::component;

pub(crate) fn is_active_path(current_path: &str, href: &str) -> bool {
    if href == "/" {
        current_path == "/"
    } else if current_path == href {
        true
    } else if current_path.starts_with(href) {
        href.ends_with('/') || current_path.as_bytes().get(href.len()) == Some(&b'/')
    } else {
        false
    }
}

/// 创建一个链接组件，用于在应用内导航
///
/// 类似于 HTML 的 `<a>` 标签，但会拦截点击事件并使用 Router 导航，而不是刷新页面。
#[component]
pub fn Link<'scope, T: ToRoute + Clone + 'scope>(
    router_ctx: RouterContext<'scope>,
    to: T,
    #[chain] children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    active_class: String,
) -> impl View<'scope> {
    let href = to.to_route();

    // 计算实际显示在 DOM 上的 href (包含 base_path 处理)。
    let base_path_str = router_ctx.base_path.get_untracked();
    let display_href = if !base_path_str.is_empty() && base_path_str != "/" && href.starts_with('/')
    {
        format!("{}{}", base_path_str.trim_end_matches('/'), href)
    } else {
        href.clone()
    };

    // 如果指定了 active_class，创建响应式类名绑定。
    let is_active_class = if !active_class.is_empty() {
        let path_signal = router_ctx.path;
        let href_for_rx = href.clone();
        let class_name = active_class.clone();

        let is_active = silex_core::rx!(router_ctx.scope(); {
            let current_path = $path_signal;
            is_active_path(current_path, &href_for_rx)
        });
        Some((class_name, is_active))
    } else {
        None
    };

    // 点击导航逻辑。
    let href_for_click = href.clone();
    let navigator = router_ctx.navigator;

    a(children)
        .attr("href", display_href)
        .class(is_active_class)
        .on_click(move |e: web_sys::MouseEvent| -> SilexResult<()> {
            // 阻止默认跳转行为
            e.prevent_default();
            navigator.push(href_for_click.as_str());
            Ok(())
        })
}

#[cfg(test)]
mod tests {
    use super::is_active_path;

    #[test]
    fn active_matching_respects_path_segments() {
        assert!(is_active_path("/", "/"));
        assert!(is_active_path("/users", "/users"));
        assert!(is_active_path("/users/42", "/users"));
        assert!(is_active_path("/users/42", "/users/"));
        assert!(!is_active_path("/username", "/user"));
        assert!(!is_active_path("/users2", "/users"));
    }
}
