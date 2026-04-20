use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::Parser;
use syn::{Attribute, FnArg, ItemFn, Pat};

pub struct ComponentAttrs {
    pub no_clone: bool,
}

pub fn generate_component(input_fn: ItemFn, attrs: ComponentAttrs) -> syn::Result<TokenStream2> {
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_generics = &input_fn.sig.generics;
    let fn_body = &input_fn.block;

    let struct_name = quote::format_ident!("{}Component", fn_name); // Struct renamed to avoid collision

    let mut struct_fields = Vec::new();
    let mut builder_methods = Vec::new();
    let mut new_initializers = Vec::new();
    let mut mount_checks = Vec::new(); // Runtime checks for required props
    let mut used_prop_names = std::collections::HashSet::new();

    let mut first_arg_is_children = false;
    let mut first_arg_ty = None;
    let mut first_arg_into_trait = false;

    // 处理结构体定义的泛型
    let (impl_generics, ty_generics, where_clause) = fn_generics.split_for_impl();

    let phantom_types: Vec<_> = fn_generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => {
                let id = &t.ident;
                Some(quote! { #id })
            }
            syn::GenericParam::Lifetime(l) => {
                let id = &l.lifetime;
                Some(quote! { &#id () })
            }
            syn::GenericParam::Const(_) => None,
        })
        .collect();

    let phantom_decl = if !phantom_types.is_empty() {
        quote! { _phantom: ::std::marker::PhantomData<fn() -> (#(#phantom_types,)*)>, }
    } else {
        quote! {}
    };

    let phantom_init = if !phantom_types.is_empty() {
        quote! { _phantom: ::std::marker::PhantomData, }
    } else {
        quote! {}
    };

    for (index, arg) in input_fn.sig.inputs.iter().enumerate() {
        let fn_arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r.self_token,
                    "Component functions cannot have `self` parameter",
                ));
            }
        };

        let pat = &fn_arg.pat;
        let ty = &fn_arg.ty;
        let attrs = &fn_arg.attrs;

        let mut prop_attrs = parse_prop_attrs(attrs)?;

        // Auto-enable `into` for specific types to improve DX
        let type_ident = get_base_type_name(ty);
        if !prop_attrs.into_trait
            && (type_ident == "Children"
                || type_ident == "AnyView"
                || type_ident == "SharedView"
                || type_ident == "String"
                || type_ident == "PathBuf"
                || type_ident == "Callback"
                || type_ident == "Signal")
        {
            prop_attrs.into_trait = true;
        }

        let param_name = match pat.as_ref() {
            Pat::Ident(ident) => &ident.ident,
            _ => {
                return Err(syn::Error::new_spanned(
                    pat,
                    "Component parameters must be simple identifiers",
                ));
            }
        };

        let param_name_str = param_name.to_string();
        used_prop_names.insert(param_name_str.clone());

        if index == 0 && param_name_str == "children" {
            first_arg_is_children = true;
            first_arg_ty = Some(ty.clone());
            first_arg_into_trait = prop_attrs.into_trait;
        }

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
                    let type_ident = get_base_type_name(ty);

                    if type_ident == "SharedView" || type_ident == "Children" {
                        new_initializers.push(quote! { #param_name: ::silex::dom::view::MountRefExt::into_shared(#default_expr) });
                    } else if type_ident == "AnyView" {
                        new_initializers.push(quote! { #param_name: ::silex::dom::view::MountExt::into_any(#default_expr) });
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
            let type_ident = get_base_type_name(ty);

            if type_ident == "Children" || type_ident == "SharedView" {
                if is_required {
                    builder_methods.push(quote! {
                        pub fn #param_name<__SilexValue: ::silex::dom::view::MountRef + ::silex::dom::view::Mount + Clone + 'static>(mut self, val: __SilexValue) -> Self {
                            use ::silex::dom::view::MountRefExt;
                            self.#param_name = Some(val.into_shared());
                            self
                        }
                    });
                } else {
                    builder_methods.push(quote! {
                        pub fn #param_name<__SilexValue: ::silex::dom::view::MountRef + ::silex::dom::view::Mount + Clone + 'static>(mut self, val: __SilexValue) -> Self {
                            use ::silex::dom::view::MountRefExt;
                            self.#param_name = val.into_shared();
                            self
                        }
                    });
                }
            } else if type_ident == "AnyView" {
                if is_required {
                    builder_methods.push(quote! {
                        pub fn #param_name<__SilexValue: ::silex::dom::view::Mount + 'static>(mut self, val: __SilexValue) -> Self {
                            use ::silex::dom::view::MountExt;
                            self.#param_name = Some(val.into_any());
                            self
                        }
                    });
                } else {
                    builder_methods.push(quote! {
                        pub fn #param_name<__SilexValue: ::silex::dom::view::Mount + 'static>(mut self, val: __SilexValue) -> Self {
                            use ::silex::dom::view::MountExt;
                            self.#param_name = val.into_any();
                            self
                        }
                    });
                }
            } else if is_required {
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
        } else if is_required {
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

    // Forwarding methods are now handled by AttributeBuilder trait implementation

    let component_constructor = if first_arg_is_children {
        let ty = first_arg_ty.unwrap();
        let type_ident = get_base_type_name(&ty);

        if first_arg_into_trait {
            if type_ident == "Children" || type_ident == "SharedView" {
                quote! {
                    // 生成同名带 children 参数的构建函数
                    #[allow(non_snake_case)]
                    #fn_vis fn #fn_name #impl_generics(children: impl ::silex::dom::view::MountRef + ::silex::dom::view::Mount + Clone + 'static) -> #struct_name #ty_generics #where_clause {
                        #struct_name::new().children(children)
                    }
                }
            } else if type_ident == "AnyView" {
                quote! {
                    #[allow(non_snake_case)]
                    #fn_vis fn #fn_name #impl_generics(children: impl ::silex::dom::view::Mount + 'static) -> #struct_name #ty_generics #where_clause {
                        #struct_name::new().children(children)
                    }
                }
            } else {
                quote! {
                    #[allow(non_snake_case)]
                    #fn_vis fn #fn_name #impl_generics(children: impl Into<#ty>) -> #struct_name #ty_generics #where_clause {
                        #struct_name::new().children(children)
                    }
                }
            }
        } else {
            quote! {
                #[allow(non_snake_case)]
                #fn_vis fn #fn_name #impl_generics(children: #ty) -> #struct_name #ty_generics #where_clause {
                    #struct_name::new().children(children)
                }
            }
        }
    } else {
        quote! {
            // 生成同名无参构建函数
            #[allow(non_snake_case)]
            #fn_vis fn #fn_name #impl_generics() -> #struct_name #ty_generics #where_clause {
                #struct_name::new()
            }
        }
    };

    let derive_clone = if !attrs.no_clone {
        quote! { #[derive(Clone)] }
    } else {
        quote! {}
    };

    let mount_ref_impl = if !attrs.no_clone {
        quote! {
            impl #impl_generics ::silex::dom::view::MountRef for #struct_name #ty_generics #where_clause {
                fn mount_ref(&self, parent: &::silex::reexports::web_sys::Node, attrs: Vec<::silex::dom::attribute::PendingAttribute>) {
                    self.clone().mount(parent, attrs);
                }
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        // 生成结构体
        #derive_clone
        #fn_vis struct #struct_name #impl_generics #where_clause {
            #(#struct_fields,)*
            // Internal storage for forwarded attributes
            _pending_attrs: Vec<::silex::dom::attribute::PendingAttribute>,
            #phantom_decl
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            // New is always parameter-less
            pub fn new() -> Self {
                Self {
                    #(#new_initializers,)*
                    _pending_attrs: Vec::new(),
                    #phantom_init
                }
            }

            #(#builder_methods)*
        }

        impl #impl_generics ::silex::dom::attribute::AttributeBuilder for #struct_name #ty_generics #where_clause {
            fn build_attribute<__SilexValue>(mut self, target: ::silex::dom::attribute::ApplyTarget, value: __SilexValue) -> Self
            where __SilexValue: ::silex::dom::attribute::IntoStorable
            {
                let owned_target = ::silex::dom::attribute::OwnedApplyTarget::from(target);
                // Convert to storable type before storing
                self._pending_attrs.push(
                    ::silex::dom::attribute::PendingAttribute::build(value.into_storable(), owned_target)
                );
                self
            }

            fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
            where
                E: ::silex::dom::event::EventDescriptor + 'static,
                F: ::silex::dom::event::EventHandler<E::EventType, M> + Clone + 'static,
            {
                 // We specifically need Clone events to capture them in the pending closure
                 // EventDescriptor is Copy/Clone, so it's fine.
                 // Handler F accepts Clone bound.
                 let event = event.clone(); // In case E is not Copy? EventDescriptor implies Copy+Clone.

                 self._pending_attrs.push(
                    ::silex::dom::attribute::PendingAttribute::new_listener(move |el| {
                        ::silex::dom::element::bind_event(el, event, callback.clone());
                    })
                );
                self
            }
        }


        impl #impl_generics ::silex::dom::view::ApplyAttributes for #struct_name #ty_generics #where_clause {}

        impl #impl_generics ::silex::dom::view::Mount for #struct_name #ty_generics #where_clause {
            fn mount(self, parent: &::silex::reexports::web_sys::Node, attrs: Vec<::silex::dom::attribute::PendingAttribute>) {
                // Runtime checks and bindings
                #(#mount_checks)*

                let view_instance = #fn_body;

                // Merge component's own pending attributes with forwarded attributes
                let mut all_attrs = self._pending_attrs;
                all_attrs.extend(attrs);

                // Forward attributes using the new robust propagation method
                ::silex::dom::view::Mount::mount(view_instance, parent, all_attrs);
            }
        }

        #mount_ref_impl

        #component_constructor
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

pub fn parse_component_attrs(args: TokenStream2) -> syn::Result<ComponentAttrs> {
    let mut result = ComponentAttrs {
        no_clone: false,
    };

    if args.is_empty() {
        return Ok(result);
    }

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("no_clone") {
            result.no_clone = true;
            Ok(())
        } else {
            Err(meta.error("unsupported component attribute"))
        }
    });

    parser.parse2(args)?;
    Ok(result)
}

fn get_base_type_name(ty: &syn::Type) -> String {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident.to_string();
    }
    "".to_string()
}
