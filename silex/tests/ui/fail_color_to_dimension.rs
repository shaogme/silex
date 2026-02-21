use silex::prelude::*;

fn main() {
    // 错误：width 应该只接受维度（Px/Rem/Percent等），不应接受颜色 Hex
    let _ = Style::new().width(hex("#ff0000"));
}
