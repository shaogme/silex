use silex_css::prelude::*;

fn main() {
    // 错误：`TransformBuilder` 的参数曾只约束 `Display`，于是
    // `transform().translate(hex("#fff"), rgb(1,2,3)).rotate("banana")`
    // 是合法 Rust——与整个 crate 的「强类型 CSS」定位割裂
    let _ = transform().rotate("banana");
}
