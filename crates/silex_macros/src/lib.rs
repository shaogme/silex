#![allow(linker_messages)]

use proc_macro::TokenStream;
#[cfg(any(feature = "component", feature = "store"))]
use syn::parse_macro_input;

#[cfg(feature = "component")]
use syn::DeriveInput;
#[cfg(feature = "store")]
use syn::ItemStruct;

#[cfg(feature = "component")]
mod component;
mod crate_path;
#[cfg(feature = "css")]
mod css;
#[cfg(feature = "component")]
mod props_builder;
mod render;
#[cfg(feature = "route")]
mod route;
#[cfg(feature = "store")]
mod store;

#[cfg(feature = "css")]
#[proc_macro]
pub fn css(input: TokenStream) -> TokenStream {
    match css::css_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn styled(input: TokenStream) -> TokenStream {
    match css::styled::styled_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn global(input: TokenStream) -> TokenStream {
    match css::styled::global_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn classes(input: TokenStream) -> TokenStream {
    match css::classes::classes_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn inject_css(input: TokenStream) -> TokenStream {
    match css::inject_css_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn theme(input: TokenStream) -> TokenStream {
    match css::theme::bridge_theme_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "tw")]
#[proc_macro]
pub fn tw(input: TokenStream) -> TokenStream {
    match css::tw::tw_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "tw")]
#[proc_macro]
pub fn tw_variants(input: TokenStream) -> TokenStream {
    match css::tw::tw_variants_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn tw_verbose(input: TokenStream) -> TokenStream {
    match css::tw::tw_verbose_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// `#[component]` 属性宏
///
/// 将一个函数转换为 Silex 组件，自动生成 Props 结构体并简化组件定义。
///
/// # 用法
///
/// ```rust, ignore
/// use silex::prelude::*;
///
/// #[component]
/// fn MyComponent<'scope>(
///     scope: Scope<'scope>,
///     name: String,
///     #[chain(default)] age: u32,
///     #[prop(into)] message: String,
/// ) -> impl View<'scope> {
///     div(format!("{} ({}): {}", name, age, message))
/// }
///
/// // 生成 Props、builder 和 product；builder 只有在状态满足后才有 build 方法。
/// let view = MyComponent(scope, "name", "message").build();
/// ```
///
/// # 属性
///
/// - `#[chain(default)]`: 普通字段使用 `Default::default()`；scoped reactive wrapper
///   使用当前显式 `Scope<'scope>` 创建默认值，并启用链式调用
/// - `#[prop(into)]`: 该属性将使用 `Into<T>` 转换输入
/// - `#[chain(default), prop(into)]`: 可以组合使用
#[cfg(feature = "component")]
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[component] no longer accepts arguments; use field-level #[chain] instead",
        )
        .to_compile_error()
        .into();
    }

    let input_fn = parse_macro_input!(item as syn::ItemFn);
    match component::generate_component(input_fn) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "store")]
#[proc_macro_attribute]
pub fn store(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[store] does not accept arguments; configure fields with explicit handles",
        )
        .to_compile_error()
        .into();
    }

    let input = parse_macro_input!(item as ItemStruct);
    match store::store_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `#[derive(PropsBuilder)]` 结构体派生宏
///
/// 为组件 Props 结构体生成链式 builder 和独立的 `View` product。
/// `#[component]` 会通过隐藏的 `silex_component` metadata 传入生成名称；
/// standalone derive 未提供 metadata 时保留 `<PropsName>Builder` 的 fallback。
#[cfg(feature = "component")]
#[proc_macro_derive(PropsBuilder, attributes(prop, chain, silex_component))]
pub fn derive_props_builder(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match props_builder::derive_props_builder_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "route")]
#[proc_macro]
pub fn routes(input: TokenStream) -> TokenStream {
    match route::routes_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn render(input: TokenStream) -> TokenStream {
    match render::render_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
