use silex_css::prelude::*;

fn main() {
    // 错误：z-index 是 `auto | <integer>`，裸字符串不再是它的合法取值
    let _ = Style::new().z_index("abc");
}
