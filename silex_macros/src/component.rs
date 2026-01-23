use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, FnArg, ItemFn, Pat, spanned::Spanned};

pub fn generate_component(input_fn: ItemFn) -> syn::Result<TokenStream2> {
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_generics = &input_fn.sig.generics;
    let fn_body = &input_fn.block;

    let struct_name = quote::format_ident!("{}Component", fn_name); // Struct renamed to avoid collision

    let mut struct_fields = Vec::new();
    let mut builder_methods = Vec::new();
    let mut new_initializers = Vec::new();
    let mut mount_checks = Vec::new(); // Runtime checks for required props

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

        let mut prop_attrs = parse_prop_attrs(attrs)?;

        // Auto-enable `into` for specific types to improve DX
        if !prop_attrs.into_trait {
            let type_str = quote::quote!(#ty).to_string();
            // Clean up whitespace for comparison
            let type_clean: String = type_str.chars().filter(|c| !c.is_whitespace()).collect();

            if type_clean.ends_with("Children")
                || type_clean.ends_with("AnyView")
                || type_clean.ends_with("String")
                || type_clean.ends_with("PathBuf")
                || type_clean.starts_with("Callback")
            {
                prop_attrs.into_trait = true;
            }
        }

        let param_name = match pat.as_ref() {
            Pat::Ident(ident) => &ident.ident,
            _ => {
                return Err(syn::Error::new(
                    pat.span(),
                    "Component parameters must be simple identifiers",
                ));
            }
        };

        let param_name_str = param_name.to_string();

        // 策略:
        // 1. 如果有 default 值，字段类型为 T，初始化为 default。
        // 2. 如果无 default 值 (必填)，字段类型为 Option<T>，初始化为 None。
        //    在 mount 时 check unwrap。

        let is_required = !prop_attrs.default && prop_attrs.default_value.is_none();

        if is_required {
            // 必填字段：存为 Option<T>
            struct_fields.push(quote! {
                pub #param_name: Option<#ty>
            });
            new_initializers.push(quote! {
                #param_name: None
            });

            // Mount 时检查
            mount_checks.push(quote! {
                let #param_name = self.#param_name.expect(concat!("Component '", stringify!(#struct_name), "' missing required prop: '", #param_name_str, "'"));
            });
        } else {
            // 可选字段：直接存 T
            struct_fields.push(quote! {
                pub #param_name: #ty
            });

            // 初始化逻辑
            if let Some(default_expr) = prop_attrs.default_value {
                if prop_attrs.into_trait {
                    let type_str = quote::quote!(#ty).to_string();
                    let type_clean: String =
                        type_str.chars().filter(|c| !c.is_whitespace()).collect();

                    if type_clean.ends_with("Children") || type_clean.ends_with("AnyView") {
                        new_initializers.push(quote! { #param_name: ::silex::core::dom::view::IntoAnyView::into_any(#default_expr) });
                    } else {
                        new_initializers.push(quote! { #param_name: (#default_expr).into() });
                    }
                } else {
                    new_initializers.push(quote! { #param_name: #default_expr });
                }
            } else {
                // #[prop(default)]
                new_initializers.push(quote! { #param_name: std::default::Default::default() });
            }

            // Mount 时直接解构（只是为了统一变量名绑定）
            mount_checks.push(quote! {
                let #param_name = self.#param_name;
            });
        }

        // 构建器方法 (Builder Methods)
        // 始终生成 .prop(val) 方法
        if prop_attrs.into_trait {
            let type_str = quote::quote!(#ty).to_string();
            let type_clean: String = type_str.chars().filter(|c| !c.is_whitespace()).collect();

            if type_clean.ends_with("Children") || type_clean.ends_with("AnyView") {
                if is_required {
                    builder_methods.push(quote! {
                        pub fn #param_name<V: ::silex::core::dom::view::IntoAnyView>(mut self, val: V) -> Self {
                            self.#param_name = Some(val.into_any());
                            self
                        }
                    });
                } else {
                    builder_methods.push(quote! {
                        pub fn #param_name<V: ::silex::core::dom::view::IntoAnyView>(mut self, val: V) -> Self {
                            self.#param_name = val.into_any();
                            self
                        }
                    });
                }
            } else {
                if is_required {
                    builder_methods.push(quote! {
                        pub fn #param_name(mut self, val: impl Into<#ty>) -> Self {
                            self.#param_name = Some(val.into());
                            self
                        }
                    });
                } else {
                    builder_methods.push(quote! {
                        pub fn #param_name(mut self, val: impl Into<#ty>) -> Self {
                            self.#param_name = val.into();
                            self
                        }
                    });
                }
            }
        } else {
            if is_required {
                builder_methods.push(quote! {
                    pub fn #param_name(mut self, val: #ty) -> Self {
                        self.#param_name = Some(val);
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
        }
    }

    let expanded = quote! {
        // 生成结构体
        #[derive(Clone)]
        #fn_vis struct #struct_name #impl_generics #where_clause {
            #(#struct_fields),*
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            // New is always parameter-less
            pub fn new() -> Self {
                Self {
                    #(#new_initializers),*
                }
            }

            #(#builder_methods)*
        }

        impl #impl_generics ::silex::core::dom::view::View for #struct_name #ty_generics #where_clause {
            fn mount(self, parent: &::silex::reexports::web_sys::Node) {
                // Runtime checks and bindings
                #(#mount_checks)*

                let view_instance = #fn_body;
                ::silex::core::dom::view::View::mount(view_instance, parent);
            }
        }

        // 生成同名构建函数
        #[allow(non_snake_case)]
        #fn_vis fn #fn_name #impl_generics() -> #struct_name #ty_generics #where_clause {
            #struct_name::new()
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
                    // Support standard expression parsing for default values
                    // This allows `default = "foo"` (Literal) and `default = 1 + 2` (Expression)
                    if meta.input.peek(syn::Token![=]) {
                        meta.input.parse::<syn::Token![=]>()?;
                        let expr: syn::Expr = meta.input.parse()?;
                        result.default_value = Some(quote! { #expr });
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
