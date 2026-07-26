use silex::prelude::*;

fn main() {
    // A-2：`rgb()` 产出颜色，而 `align-items` 的语法里没有 `<color>`。
    // `align-items: #ff0000` 早就被 `ValidFor` 拦住了（`Hex` 定得了型），
    // 但换成函数写法就绕过了整个类型系统
    let _ = css! {
        align-items: rgb(0 0 0);
    };

    // 渐变也是 `<image>`：`z-index` 只接受整数
    let _ = css! {
        z-index: linear-gradient(red, blue);
    };
}
