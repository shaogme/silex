use silex_css::prelude::*;

fn main() {
    // 错误：`translateZ()` 只接受 `<length>`——百分比在这里是无效的。
    // 区分 `<length>` 与 `<length-percentage>` 就是为了挡住这一类。
    let _ = transform().translate_z(pct(50));
}
