use silex_css::prelude::*;

fn main() {
    // 错误：border_left_width 应该只接受维度（Px/Rem/Percent等），不应接受颜色 Hex
    let _ = Style::new().border_left_width(hex("#ff0000"));
}
