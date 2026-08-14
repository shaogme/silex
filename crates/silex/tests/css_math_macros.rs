//! `css_min!` / `css_max!` / `css_clamp!` 必须能从**顶层** prelude 拿到。
//!
//! `#[macro_export]` 把宏放在 `silex_css` 的 crate 根，而不是 `types` 模块里，
//! 所以 `silex_css::prelude` 那句 `pub use crate::types::*` 带不走它们——漏掉
//! 单独的 re-export，宏在 `silex_css` 内部的单测里照样通过，只有用户会撞上
//! 「找不到 `css_min`」。这个文件就是那条链子的守卫。
//!
//! 产物文本由 `silex_css` 内部的单测断言（`Style::render()` 是 `pub(crate)`，
//! 跨 crate 拿不到 CSS）；这里只证明「宏在、且能用在属性上」。
use silex::prelude::*;

#[test]
fn the_math_macros_are_reachable_through_the_top_level_prelude() {
    assert_eq!(css_min!(px(600), pct(100)).to_string(), "min(600px, 100%)");
    assert_eq!(css_max!(vh(50), px(320)).to_string(), "max(50vh, 320px)");
    assert_eq!(
        css_clamp!(rem(1), vw(4), rem(2)).to_string(),
        "clamp(1rem, 4vw, 2rem)"
    );
}

/// 用在属性上时没有类型标注可写，量纲要完全由参数反推——这一条只有在真实
/// 调用点上才成立，`to_string()` 那几行证明不了
#[test]
fn the_math_macros_type_check_at_a_property_call_site() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let error_handler = scope
                .error_handler(|_| {})
                .expect("test error handler should register");
            let context = SilexContext::new(scope, error_handler);

            let _ = sty(context)
                .width(css_min!(px(600), pct(100)))
                .expect("width should accept the minimum expression")
                .height(css_max!(vh(50), px(320)))
                .expect("height should accept the maximum expression")
                .font_size(css_clamp!(rem(1), vw(4), rem(2)));
        })
        .expect("test context should initialize");
}
