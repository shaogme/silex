use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Fields, FnArg, ItemFn, Pat, ReturnType, parse_macro_input,
    spanned::Spanned,
};

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
///     div().text(format!("{} ({}): {}", name, age, message))
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
#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    match generate_component(input_fn) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn generate_component(input_fn: ItemFn) -> syn::Result<TokenStream2> {
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_generics = &input_fn.sig.generics;
    let fn_body = &input_fn.block;
    let fn_return_type = &input_fn.sig.output;

    // 生成 Props 结构体名称
    let props_name = format_ident!("{}Props", fn_name);

    // 提取函数参数
    let mut prop_fields = Vec::new();
    let mut prop_bindings = Vec::new();
    let mut generic_params = Vec::new();
    let mut generic_where_clauses = Vec::new();

    for (idx, arg) in input_fn.sig.inputs.iter().enumerate() {
        let fn_arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(_) => {
                return Err(syn::Error::new(
                    arg.span(),
                    "Component functions cannot have `self` parameter",
                ));
            }
        };

        let pat = &fn_arg.pat;
        let ty = &fn_arg.ty;
        let attrs = &fn_arg.attrs;

        // 解析 #[prop(...)] 属性
        let prop_attrs = parse_prop_attrs(attrs)?;

        let param_name = match pat.as_ref() {
            Pat::Ident(ident) => &ident.ident,
            _ => {
                return Err(syn::Error::new(
                    pat.span(),
                    "Component parameters must be simple identifiers",
                ));
            }
        };

        if prop_attrs.into_trait {
            // 为 `into` 属性生成泛型参数
            let generic_name = format_ident!("__PropInto{}", idx);
            generic_params.push(quote! { #generic_name });
            generic_where_clauses.push(quote! { #generic_name: Into<#ty> });

            prop_fields.push(quote! {
                pub #param_name: #generic_name
            });

            prop_bindings.push(quote! {
                let #param_name: #ty = props.#param_name.into();
            });
        } else {
            prop_fields.push(quote! {
                pub #param_name: #ty
            });

            prop_bindings.push(quote! {
                let #param_name: #ty = props.#param_name;
            });
        }
    }

    // 合并原有泛型和新生成的泛型
    let existing_generic_params: Vec<_> = fn_generics.params.iter().collect();
    let all_generic_params = if generic_params.is_empty() && existing_generic_params.is_empty() {
        quote! {}
    } else {
        let existing = existing_generic_params.iter();
        let new = generic_params.iter();
        quote! { <#(#existing,)* #(#new),*> }
    };

    let where_clause = if generic_where_clauses.is_empty() {
        match &fn_generics.where_clause {
            Some(wc) => quote! { #wc },
            None => quote! {},
        }
    } else {
        match &fn_generics.where_clause {
            Some(wc) => {
                let existing_predicates = &wc.predicates;
                quote! { where #existing_predicates, #(#generic_where_clauses),* }
            }
            None => quote! { where #(#generic_where_clauses),* },
        }
    };

    let return_type = match fn_return_type {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    // 如果没有参数，不生成 Props 结构体，保留原始无参函数
    let has_props = !prop_fields.is_empty();

    let expanded = if has_props {
        quote! {
            #fn_vis struct #props_name #all_generic_params #where_clause {
                #(#prop_fields),*
            }

            #[allow(non_snake_case)]
            #fn_vis fn #fn_name #all_generic_params (props: #props_name #all_generic_params) -> #return_type
            #where_clause
            {
                #(#prop_bindings)*
                #fn_body
            }
        }
    } else {
        // 无参数组件：不生成 Props，保持原始函数签名
        quote! {
            #[allow(non_snake_case)]
            #fn_vis fn #fn_name #all_generic_params () -> #return_type
            #where_clause
            #fn_body
        }
    };

    Ok(expanded)
}

struct PropAttrs {
    default: bool,
    into_trait: bool,
}

fn parse_prop_attrs(attrs: &[Attribute]) -> syn::Result<PropAttrs> {
    let mut result = PropAttrs {
        default: false,
        into_trait: false,
    };

    for attr in attrs {
        if attr.path().is_ident("prop") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    result.default = true;
                    Ok(())
                } else if meta.path.is_ident("into") {
                    result.into_trait = true;
                    Ok(())
                } else {
                    Err(meta.error("expected `default` or `into`"))
                }
            })?;
        }
    }

    Ok(result)
}

#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let store_name = format_ident!("{}Store", name);

    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Store derive only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Store derive only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let struct_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {
            pub #name: ::silex::reactivity::RwSignal<#ty>
        }
    });

    let new_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: ::silex::reactivity::create_rw_signal(source.#name)
        }
    });

    let get_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: self.#name.get()
        }
    });

    let expanded = quote! {
        #[derive(Clone, Copy)]
        pub struct #store_name {
            #(#struct_fields),*
        }

        impl #store_name {
            pub fn new(source: #name) -> Self {
                Self {
                    #(#new_fields),*
                }
            }

            pub fn get(&self) -> #name {
                #name {
                    #(#get_fields),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
