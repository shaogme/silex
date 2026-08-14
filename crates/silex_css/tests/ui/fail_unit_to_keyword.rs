use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：border_bottom_style 应该只接受 Keyword 或 CssUnsafe，不应接受 Px
    let _ = Style::new(context).border_bottom_style(px(10));
}

fn main() {}
