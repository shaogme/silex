use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(ctx: C)
where
    C: SilexContextProvider<'scope>,
{
    // 错误：animation-delay 接受 <time>，不接受长度
    let _ = Style::new(ctx).animation_delay(px(10));
}

fn main() {}
