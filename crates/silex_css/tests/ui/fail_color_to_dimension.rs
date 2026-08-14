use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：border_left_width 应该只接受维度（Px/Rem/Percent等），不应接受颜色 Hex
    let _ = Style::new(context).border_left_width(hex("#ff0000"));
}

fn main() {}
