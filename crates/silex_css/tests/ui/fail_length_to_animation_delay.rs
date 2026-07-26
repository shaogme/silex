use silex_css::prelude::*;

fn main() {
    // 错误：animation-delay 接受 <time>，不接受长度
    let _ = Style::new().animation_delay(px(10));
}
