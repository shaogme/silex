//! `css!` 里静态取值的三层校验（关键字 / 函数式取值 / 分量个数）。
//!
//! 这三层的判据在 `silex_macros::css::value_check` 里有单测，但那些单测直接调
//! `check_static_value`——证明不了「宏真的会因此编译失败」。这里的反例证明的是
//! 后者：错误从 `css!` 展开时抛出，位置指到源码里那条声明上。
//!
//! 与 `silex_css/tests/ui/` 的分工：那边是 `sty()` 构建器的类型错误（`ValidFor`
//! 的 `E0277`），这边是宏展开期的错误。两条路都要有反例，因为它们是两套判据。
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
