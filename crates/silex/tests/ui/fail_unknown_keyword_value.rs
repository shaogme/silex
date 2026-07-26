use silex::prelude::*;

fn main() {
    // A-1：`centre` 不在 `align-items` 的关键字表里。此前这一句编译通过、
    // 无警告，浏览器丢弃整条声明——元素默认按 `normal` 对齐，看不出错在哪
    let _ = css! {
        align-items: centre;
    };

    // 属性接受颜色时，拼错的颜色名同样要被拦住：具名颜色不在任何属性的
    // 关键字表里，靠 `COLOR` 位 + 全局颜色表判定
    let _ = css! {
        color: reed;
    };
}
