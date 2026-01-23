use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, parse_macro_input};

#[cfg(feature = "component")]
mod component;
#[cfg(feature = "css")]
mod css;
#[cfg(feature = "route")]
mod route;
#[cfg(feature = "store")]
mod store;

#[cfg(feature = "css")]
#[proc_macro]
pub fn css(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::LitStr);
    match css::css_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
mod style;

#[cfg(feature = "css")]
#[proc_macro]
pub fn style(input: TokenStream) -> TokenStream {
    match style::style_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "css")]
#[proc_macro]
pub fn classes(input: TokenStream) -> TokenStream {
    match style::classes_impl(input.into()) {
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
/// ```rust
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
/// - `#[prop(default)]`: 该属性将使用 `Default::default()` 作为默认值
/// - `#[prop(into)]`: 该属性将使用 `Into<T>` 转换输入
/// - `#[prop(default, into)]`: 可以组合使用
#[cfg(feature = "component")]
#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    match component::generate_component(input_fn) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "store")]
#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match store::derive_store_impl(input) {
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
