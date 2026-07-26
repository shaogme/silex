use silex::prelude::*;

fn main() {
    // A-3：`color` 的语法是 `<color>`，没有多分量形式。这一句大概是想写
    // `border: 1px solid red`——属性名写错的那一半由 `resolve_property_type`
    // 管不了，因为 `color` 是个真实属性
    let _ = css! {
        color: 1px solid red;
    };

    // `z-index` 只接受一个 `<integer>`
    let _ = css! {
        z-index: 1 2;
    };
}
