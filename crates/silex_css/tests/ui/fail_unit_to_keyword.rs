use silex_css::prelude::*;

fn main() {
    // 错误：border_bottom_style 应该只接受 Keyword 或 UnsafeCss，不应接受 Px
    let _ = Style::new().border_bottom_style(px(10));
}
