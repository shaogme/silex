//! 见 `Cargo.toml`：本 crate 以 `my_silex` 的名字依赖 `silex`，
//! 编译通过即证明宏展开出的路径是按调用方实际依赖名解析的，而不是写死的 `::silex::`。
//!
//! 覆盖面按"展开里出现过绝对路径"来选，每个宏至少踩一次。

use my_silex::prelude::*;

/// `tw!`（含条件分支，会展开 `rx!` / `cx!`）与 `css!`
#[component]
pub fn Badge(#[prop(into)] label: String, #[chain(default)] wide: bool) -> impl View {
    let cls = tw!(
        "inline-flex items-center px-2 py-1 rounded-sm",
        (wide, "w-full", "w-auto")
    );
    span!(label).class(cls).style(css! { line_height: 1.25; })
}

/// `tw_variants!`：展开出 `declare_variants!` 与每个选项的 `tw!`
pub fn button_class(size: &str) -> String {
    let styles = tw_variants! {
        base: "inline-flex items-center justify-center rounded-md",
        variants: {
            size: {
                sm: "h-8 px-3 text-sm",
                md: "h-9 px-4 text-sm",
                lg: "h-10 px-6 text-base",
            }
        },
        default_variants: { size: md }
    };
    styles.get(size)
}

// `styled!`：展开出 `#[component]`、`inject_style`、`TypedElement` 等一大片绝对路径
styled! {
    pub Panel <div> (
        children: AnyView,
    ) {
        display: flex;
        flex_direction: column;
        padding: 1rem;
    }
}
