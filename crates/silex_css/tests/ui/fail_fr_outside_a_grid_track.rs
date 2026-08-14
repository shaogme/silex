use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：`fr` 只在网格轨道尺寸里合法，`width: 1fr` 不是有效声明
    let _ = Style::new(context).width(fr(1));
}

fn main() {}
