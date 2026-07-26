use silex_css::prelude::*;

fn main() {
    // 错误：`fr` 只在网格轨道尺寸里合法，`width: 1fr` 不是有效声明
    let _ = Style::new().width(fr(1));
}
