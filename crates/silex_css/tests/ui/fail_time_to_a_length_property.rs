use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：时间单位只对接受 `<time>` 的属性合法，不能当长度用
    let _ = Style::new(context).width(sec(1));
}

fn main() {}
