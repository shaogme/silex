use silex_css::prelude::*;

fn main() {
    // `css_min!` 放宽的只是「参数必须同型」，量纲标记依旧挡在那里：
    // 长度和时间取不出一个 min
    let _ = css_min!(px(1), sec(1));
}
