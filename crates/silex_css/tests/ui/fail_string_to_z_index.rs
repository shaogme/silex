use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：z-index 是 `auto | <integer>`，裸字符串不再是它的合法取值
    let _ = Style::new(context).z_index("abc");
}

fn main() {}
