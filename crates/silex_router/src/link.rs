use crate::{ToRoute, context::RouterContext};
use silex_core::SilexResult;
use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_dom::view::MountContext;
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

#[derive(Clone, Default)]
pub struct LinkBehavior {
    pub target: String,
    pub download: bool,
}

#[derive(Clone, Copy)]
struct ClickModifiers {
    button: i16,
    ctrl: bool,
    meta: bool,
    shift: bool,
    alt: bool,
}

fn click_modifiers(event: &web_sys::MouseEvent) -> ClickModifiers {
    ClickModifiers {
        button: event.button(),
        ctrl: event.ctrl_key(),
        meta: event.meta_key(),
        shift: event.shift_key(),
        alt: event.alt_key(),
    }
}

fn is_same_origin_internal_href(href: &str) -> bool {
    if href.starts_with('/') && !href.starts_with("//") {
        return true;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = href;
        false
    }
    #[cfg(target_arch = "wasm32")]
    {
        if !(href.starts_with("http://") || href.starts_with("https://")) {
            return false;
        }
        let Some(window) = web_sys::window() else {
            return false;
        };
        let location = window.location();
        let Ok(origin) = location.origin() else {
            return false;
        };
        let Ok(base) = location.href() else {
            return false;
        };
        let Ok(url) = web_sys::Url::new_with_base(href, &base) else {
            return false;
        };
        url.origin() == origin
    }
}

fn should_intercept_click(modifiers: ClickModifiers, behavior: &LinkBehavior, href: &str) -> bool {
    modifiers.button == 0
        && !modifiers.ctrl
        && !modifiers.meta
        && !modifiers.shift
        && !modifiers.alt
        && behavior.target.is_empty()
        && !behavior.download
        && is_same_origin_internal_href(href)
}

#[derive(Clone)]
pub struct LinkView<'scope> {
    view: SilexResult<AnyView<'scope>>,
}

impl<'scope> ApplyAttributes<'scope> for LinkView<'scope> {}

impl<'scope> View<'scope> for LinkView<'scope> {
    fn mount(
        &self,
        context: &MountContext<'scope>,
        attrs: Vec<silex_dom::attribute::AttrOp<'scope>>,
    ) -> SilexResult<silex_dom::view::MountInstance<'scope>> {
        match &self.view {
            Ok(view) => view.mount(context, attrs),
            Err(error) => Err(error.clone()),
        }
    }
}

/// 创建一个链接组件，用于在应用内导航
///
/// 类似于 HTML 的 `<a>` 标签，但会拦截点击事件并使用 Router 导航，而不是刷新页面。
#[component]
pub fn Link<'scope, T: ToRoute + Clone + 'scope>(
    #[ctx] router_ctx: RouterContext<'scope>,
    to: T,
    #[chain] children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    active_class: String,
    #[chain(default)] behavior: LinkBehavior,
) -> LinkView<'scope> {
    let view = (|| -> SilexResult<AnyView<'scope>> {
        let href = to.to_route();

        // 计算实际显示在 DOM 上的 href (包含 base_path 处理)。
        let base_path_str = router_ctx.base_path.get_untracked()?;
        let display_href =
            if !base_path_str.is_empty() && base_path_str != "/" && href.starts_with('/') {
                format!("{}{}", base_path_str.trim_end_matches('/'), href)
            } else {
                href.clone()
            };

        // 如果指定了 active_class，创建响应式类名绑定。
        let is_active_class = if !active_class.is_empty() {
            let path_signal = router_ctx.path;
            let href_for_rx = href.clone();
            let class_name = active_class.clone();

            let is_active = router_ctx.owner().computed(
                move || {
                    let current_path = path_signal.get()?;
                    Ok(is_active_path(&current_path, &href_for_rx))
                },
                router_ctx.error_reporter(),
            )?;
            let is_active = is_active.into_rx();
            Some((class_name, is_active))
        } else {
            None
        };

        // 点击导航逻辑。
        let href_for_click = href.clone();
        let navigator = router_ctx.navigator;
        let behavior_for_click = behavior.clone();

        Ok(a(children)
            .attr("href", display_href)
            .attr("target", behavior.target.clone())
            .attr("download", behavior.download.then_some(String::new()))
            .class(is_active_class)
            .on_click(move |e: web_sys::MouseEvent| -> SilexResult<()> {
                if !should_intercept_click(
                    click_modifiers(&e),
                    &behavior_for_click,
                    &href_for_click,
                ) {
                    return Ok(());
                }
                e.prevent_default();
                navigator.push(href_for_click.as_str())
            })
            .into_any())
    })();
    LinkView { view }
}

#[cfg(test)]
mod tests {
    use super::{ClickModifiers, LinkBehavior, is_active_path, should_intercept_click};

    #[test]
    fn active_matching_respects_path_segments() {
        assert!(is_active_path("/", "/"));
        assert!(is_active_path("/users", "/users"));
        assert!(is_active_path("/users/42", "/users"));
        assert!(is_active_path("/users/42", "/users/"));
        assert!(!is_active_path("/username", "/user"));
        assert!(!is_active_path("/users2", "/users"));
    }

    #[test]
    fn click_interception_only_handles_plain_internal_primary_clicks() {
        let plain = ClickModifiers {
            button: 0,
            ctrl: false,
            meta: false,
            shift: false,
            alt: false,
        };
        assert!(should_intercept_click(
            plain,
            &LinkBehavior::default(),
            "/users"
        ));
        assert!(!should_intercept_click(
            ClickModifiers { button: 1, ..plain },
            &LinkBehavior::default(),
            "/users",
        ));
        assert!(!should_intercept_click(
            ClickModifiers {
                ctrl: true,
                ..plain
            },
            &LinkBehavior::default(),
            "/users",
        ));
        assert!(!should_intercept_click(
            plain,
            &LinkBehavior {
                target: String::from("_blank"),
                download: false,
            },
            "/users",
        ));
        assert!(!should_intercept_click(
            plain,
            &LinkBehavior {
                target: String::new(),
                download: true,
            },
            "/users",
        ));
        assert!(!should_intercept_click(
            plain,
            &LinkBehavior::default(),
            "https://external.example/users",
        ));
    }
}
