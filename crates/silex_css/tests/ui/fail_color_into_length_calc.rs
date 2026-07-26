use silex_css::prelude::*;
use silex_css::types::{CalcValue, IntoCalc, LengthMark};

fn main() {
    // 错误：`IntoCalc<LengthMark>` 曾是 `impl<T: Display>` 的 blanket impl，
    // 任何能打印的东西都能变成长度
    let _: CalcValue<LengthMark> = hex("#ffffff").into_calc();
}
