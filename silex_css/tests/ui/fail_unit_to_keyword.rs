use silex_css::prelude::*;

fn main() {
    // 错误：display 应该只接受 DisplayKeyword 或 UnsafeCss，不应接受 Px
    let _ = Style::new().display(px(10));
}
