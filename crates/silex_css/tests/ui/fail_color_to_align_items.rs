use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(ctx: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：align-items 的语法里没有 <color>，此前它落在「什么都收」的
    // Shorthand 组里，`align-items: #ff0000` 能编译通过
    let _ = Style::new(ctx).align_items(hex("#ff0000"));
}

fn main() {}
