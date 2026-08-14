use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(ctx: C)
where
    C: SilexContextProvider<'scope>,
{
    let m = margin::all(px(16));
    // 错误：padding() 方法要求传入实现 ValidFor<props::Padding> 的类型，
    // MarginValue 仅实现了 ValidFor<props::Margin>
    let _ = Style::new(ctx).padding(m);
}

fn main() {}
