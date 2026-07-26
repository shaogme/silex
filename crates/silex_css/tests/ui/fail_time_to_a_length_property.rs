use silex_css::prelude::*;

fn main() {
    // 错误：时间单位只对接受 `<time>` 的属性合法，不能当长度用
    let _ = Style::new().width(sec(1));
}
