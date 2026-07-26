use silex_css::prelude::*;

fn main() {
    // 错误：量纲标记要挡住的正是这个——长度与时间不能相加
    let _ = px(10) + sec(1);
}
