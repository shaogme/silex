use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, parse_macro_input};

#[cfg(feature = "component")]
mod component;
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
pub fn theme(input: TokenStream) -> TokenStream {
    match css::theme::bridge_theme_impl(input.into()) {
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
/// fn MyComponent(
///     name: String,
///     #[prop(default)] age: u32,
///     #[prop(into)] message: String,
/// ) -> impl View {
///     div(format!("{} ({}): {}", name, age, message))
/// }
///
/// // 生成的代码等效于:
/// // pub struct MyComponentProps<M> { ... }
/// // pub fn MyComponent(props: MyComponentProps<...>) -> impl View { ... }
/// ```
///
/// # 属性
///
/// - `#[chain(default)]`: 该属性将使用 `Default::default()` 作为默认值，并启用链式调用
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

    let input_fn = parse_macro_input!(item as ItemFn);
    match component::generate_component(input_fn) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "store")]
#[proc_macro_derive(Store, attributes(store, persist))]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match store::derive_store_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// `#[derive(PropsBuilder)]` 结构体派生宏
///
/// 为组件 Props 结构体生成链式构造器与 `View` 桥接层。
#[cfg(feature = "component")]
#[proc_macro_derive(PropsBuilder, attributes(prop, chain))]
pub fn derive_props_builder(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match props_builder::derive_props_builder_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "route")]
#[proc_macro_derive(Route, attributes(route, nested))]
pub fn derive_route(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match route::derive_route_impl(input) {
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
