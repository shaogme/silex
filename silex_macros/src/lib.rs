use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Fields, FnArg, ItemFn, Pat, parse_macro_input, spanned::Spanned,
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
    // let fn_return_type = &input_fn.sig.output; // We don't need return type for struct impl

    let struct_name = fn_name; // 结构体使用函数名作为名称

    let mut struct_fields = Vec::new();
    let mut builder_methods = Vec::new();
    let mut new_args = Vec::new();
    let mut new_initializers = Vec::new();
    let mut body_let_bindings = Vec::new();

    // 处理结构体定义的泛型
    let (impl_generics, ty_generics, where_clause) = fn_generics.split_for_impl();

    for arg in input_fn.sig.inputs.iter() {
        let fn_arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(r) => {
                return Err(syn::Error::new(
                    r.span(),
                    "Component functions cannot have `self` parameter",
                ));
            }
        };

        let pat = &fn_arg.pat;
        let ty = &fn_arg.ty;
        let attrs = &fn_arg.attrs;

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

        // 1. 结构体字段
        struct_fields.push(quote! {
            pub #param_name: #ty
        });

        // 2. new 方法初始化器
        if let Some(default_expr) = prop_attrs.default_value {
            // 有显式默认值或 #[prop(default)]
            new_initializers.push(quote! {
                #param_name: #default_expr
            });
        } else if prop_attrs.default {
            // #[prop(default)] 无值 (使用 Default trait)
            new_initializers.push(quote! {
                #param_name: std::default::Default::default()
            });
        } else {
            // 无默认值：添加到 new() 参数
            new_args.push(quote! { #param_name: #ty });
            new_initializers.push(quote! { #param_name });
        }

        // 3. 构建器方法
        if prop_attrs.into_trait {
            builder_methods.push(quote! {
                pub fn #param_name(mut self, val: impl Into<#ty>) -> Self {
                    self.#param_name = val.into();
                    self
                }
            });
        } else {
            builder_methods.push(quote! {
                pub fn #param_name(mut self, val: #ty) -> Self {
                    self.#param_name = val;
                    self
                }
            });
        }

        // 4. 函数体绑定 (恢复函数参数变量)
        body_let_bindings.push(quote! {
            let #param_name = self.#param_name;
        });
    }

    // 检查函数体是否为空或只是 unit，通常组件都有函数体。

    let expanded = quote! {
        // 生成结构体
        // View 接收 self，所以严格来说不需要 Clone，且 AnyView 等字段可能无法 Clone。
        #fn_vis struct #struct_name #impl_generics #where_clause {
            #(#struct_fields),*
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn new(#(#new_args),*) -> Self {
                Self {
                    #(#new_initializers),*
                }
            }

            #(#builder_methods)*
        }

        impl #impl_generics ::silex::dom::view::View for #struct_name #ty_generics #where_clause {
            fn mount(self, parent: &::web_sys::Node) {
                #(#body_let_bindings)*
                let view_instance = #fn_body;
                ::silex::dom::view::View::mount(view_instance, parent);
            }
        }
    };

    Ok(expanded)
}

struct PropAttrs {
    default: bool,
    default_value: Option<TokenStream2>,
    into_trait: bool,
}

fn parse_prop_attrs(attrs: &[Attribute]) -> syn::Result<PropAttrs> {
    let mut result = PropAttrs {
        default: false,
        default_value: None,
        into_trait: false,
    };

    for attr in attrs {
        if attr.path().is_ident("prop") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    result.default = true;
                    // 如果指定了具体的值： #[prop(default = "some_expr")] 或 #[prop(default = 100)]
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?;
                        let expr: syn::Expr = value.parse()?;

                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }) = &expr
                        {
                            // 如果是字符串字面量，解析其中的代码
                            let valid_expr: syn::Expr = lit_str.parse()?;
                            result.default_value = Some(quote! { #valid_expr });
                        } else {
                            // 如果是其他表达式（如数字、布尔值），直接使用
                            result.default_value = Some(quote! { #expr });
                        }
                    }
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
