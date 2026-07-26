use silex_css::prelude::*;

fn main() {
    // 错误：`with_value` 能把工厂函数刚建立起来的不变量一行抹掉
    // （`hex("#fff").with_value("javascript:alert(1)")`），已移除
    let _ = hex("#ffffff").with_value("javascript:alert(1)");
}
