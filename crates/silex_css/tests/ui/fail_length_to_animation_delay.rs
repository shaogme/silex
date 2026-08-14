use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：animation-delay 接受 <time>，不接受长度
    let _ = Style::new(context).animation_delay(px(10));
}

fn main() {}
