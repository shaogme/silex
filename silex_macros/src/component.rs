use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::parse::Parser;
use syn::{Attribute, Block, FnArg, Generics, Ident, ItemFn, Pat, Type, Visibility};

// --- Data Structures ---

/// 组件装饰器参数 (Component macro attributes)
pub struct ComponentAttrs {
    pub standalone: usize,
}

/// 属性解析结果
struct PropAttrs {
    default: bool,
    default_value: Option<TokenStream2>,
    into_trait: bool,
    clone: bool,
}

/// 记录独立参数的信息，用于生成构造函数
struct StandaloneArgInfo {
    name: Ident,
    ty: Type,
    into_trait: bool,
    type_ident: String,
}

/// 组件生成上下文
struct ComponentGenerator {
    fn_name: Ident,
    fn_vis: Visibility,
    fn_generics: Generics,
    fn_body: Box<Block>,
    struct_name: Ident,

    struct_fields: Vec<TokenStream2>,
    builder_methods: Vec<TokenStream2>,
    new_initializers: Vec<TokenStream2>,
    mount_checks: Vec<TokenStream2>,
    mount_ref_checks: Vec<TokenStream2>,

    phantom_decl: TokenStream2,
    phantom_init: TokenStream2,
    standalone_count: usize,
    standalone_args: Vec<StandaloneArgInfo>,
    used_prop_names: HashSet<String>,
}

// --- Implementation ---

impl ComponentGenerator {
    fn new(input_fn: ItemFn) -> Self {
        let fn_name = input_fn.sig.ident.clone();
        let struct_name = quote::format_ident!("{}Component", fn_name);

        Self {
            fn_name,
            fn_vis: input_fn.vis,
            fn_generics: input_fn.sig.generics,
            fn_body: input_fn.block,
            struct_name,
            struct_fields: Vec::new(),
            builder_methods: Vec::new(),
            new_initializers: Vec::new(),
            mount_checks: Vec::new(),
            mount_ref_checks: Vec::new(),
            phantom_decl: quote!(),
            phantom_init: quote!(),
            standalone_count: 1, // 默认值为 1
            standalone_args: Vec::new(),
            used_prop_names: HashSet::new(),
        }
    }

    fn with_standalone_count(mut self, count: usize) -> Self {
        self.standalone_count = count;
        self
    }

    /// 准备 PhantomData 以处理泛型
    fn prepare_phantom_data(&mut self) {
        let phantom_types: Vec<_> = self
            .fn_generics
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

        if !phantom_types.is_empty() {
            self.phantom_decl =
                quote! { _phantom: ::std::marker::PhantomData<fn() -> (#(#phantom_types,)*)>, };
            self.phantom_init = quote! { _phantom: ::std::marker::PhantomData, };
        }
    }

    /// 处理函数参数
    fn process_args(
        &mut self,
        inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    ) -> syn::Result<()> {
        for (index, arg) in inputs.iter().enumerate() {
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
            let type_ident = get_base_type_name(ty);

            // 自动推断是否需要 into_trait
            if !prop_attrs.into_trait && is_auto_into_type(&type_ident) {
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
            if !self.used_prop_names.insert(param_name_str.clone()) {
                return Err(syn::Error::new_spanned(
                    param_name,
                    format!("Duplicate component parameter: `{}`", param_name_str),
                ));
            }

            let is_standalone = index < self.standalone_count;
            if is_standalone {
                self.standalone_args.push(StandaloneArgInfo {
                    name: param_name.clone(),
                    ty: (**ty).clone(),
                    into_trait: prop_attrs.into_trait,
                    type_ident: type_ident.clone(),
                });
            }

            self.generate_prop_logic(param_name, ty, &prop_attrs, &type_ident, is_standalone);
        }
        Ok(())
    }

    /// 为单个属性生成字段、初始化、挂载检查和 Builder 方法
    fn generate_prop_logic(
        &mut self,
        name: &Ident,
        ty: &Type,
        attrs: &PropAttrs,
        type_ident: &str,
        is_standalone: bool,
    ) {
        let is_required = is_standalone || (!attrs.default && attrs.default_value.is_none());
        let struct_name = &self.struct_name;
        let name_str = name.to_string();

        // 1. 生成字段和初始化逻辑
        if is_required {
            if is_standalone {
                self.struct_fields.push(quote! { pub #name: #ty });
                self.new_initializers.push(quote! { #name });
            } else {
                self.struct_fields.push(quote! { pub #name: Option<#ty> });
                self.new_initializers.push(quote! { #name: None });
            }
        } else {
            self.struct_fields.push(quote! { pub #name: #ty });

            let init_val = if let Some(ref default_expr) = attrs.default_value {
                if attrs.into_trait {
                    match type_ident {
                        "SharedView" => {
                            quote! { ::silex::dom::view::MountRefExt::into_shared(#default_expr) }
                        }
                        "AnyView" => {
                            quote! { ::silex::dom::view::MountExt::into_any(#default_expr) }
                        }
                        _ => quote! { (#default_expr).into() },
                    }
                } else {
                    quote! { #default_expr }
                }
            } else {
                quote! { std::default::Default::default() }
            };
            self.new_initializers.push(quote! { #name: #init_val });
        }

        // 2. 生成 Mount 时的 Prop 处理逻辑
        let (mount_owned, mount_borrowed) = if is_required {
            if is_standalone {
                (
                    quote! { ::silex::dom::view::Prop::new_owned(self.#name) },
                    if attrs.clone {
                        quote! { ::silex::dom::view::Prop::new_owned(self.#name.clone()) }
                    } else {
                        quote! { ::silex::dom::view::Prop::new_borrowed(&self.#name) }
                    },
                )
            } else {
                (
                    quote! { ::silex::dom::view::Prop::new_owned(self.#name.expect(concat!("Component '", stringify!(#struct_name), "' missing required prop: '", #name_str, "'"))) },
                    if attrs.clone {
                        quote! { ::silex::dom::view::Prop::new_owned(self.#name.as_ref().expect(concat!("Component '", stringify!(#struct_name), "' missing required prop: '", #name_str, "'")).clone()) }
                    } else {
                        quote! { ::silex::dom::view::Prop::new_borrowed(self.#name.as_ref().expect(concat!("Component '", stringify!(#struct_name), "' missing required prop: '", #name_str, "'"))) }
                    },
                )
            }
        } else {
            (
                quote! { ::silex::dom::view::Prop::new_owned(self.#name) },
                if attrs.clone {
                    quote! { ::silex::dom::view::Prop::new_owned(self.#name.clone()) }
                } else {
                    quote! { ::silex::dom::view::Prop::new_borrowed(&self.#name) }
                },
            )
        };

        self.mount_checks.push(quote! { let #name = #mount_owned; });
        self.mount_ref_checks
            .push(quote! { let #name = #mount_borrowed; });

        // 3. 生成 Builder 方法
        self.builder_methods.push(self.generate_builder_method(
            name,
            ty,
            attrs,
            type_ident,
            is_standalone,
            is_required,
        ));
    }

    fn generate_builder_method(
        &self,
        name: &Ident,
        ty: &Type,
        attrs: &PropAttrs,
        type_ident: &str,
        is_standalone: bool,
        is_required: bool,
    ) -> TokenStream2 {
        if attrs.into_trait {
            let target_val = if is_standalone || !is_required {
                quote! { val.into_shared() }
            } else {
                quote! { Some(val.into_shared()) }
            };
            let any_target_val = if is_standalone || !is_required {
                quote! { val.into_any() }
            } else {
                quote! { Some(val.into_any()) }
            };
            let into_target_val = if is_standalone || !is_required {
                quote! { val.into() }
            } else {
                quote! { Some(val.into()) }
            };

            match type_ident {
                "SharedView" => quote! {
                    pub fn #name<__SilexValue: ::silex::dom::view::ApplyAttributes + ::silex::dom::view::MountRefExt>(mut self, val: __SilexValue) -> Self {
                        use ::silex::dom::view::MountRefExt;
                        self.#name = #target_val;
                        self
                    }
                },
                "AnyView" => quote! {
                    pub fn #name<__SilexValue: ::silex::dom::view::MountExt>(mut self, val: __SilexValue) -> Self {
                        use ::silex::dom::view::MountExt;
                        self.#name = #any_target_val;
                        self
                    }
                },
                _ => quote! {
                    pub fn #name(mut self, val: impl Into<#ty>) -> Self {
                        self.#name = #into_target_val;
                        self
                    }
                },
            }
        } else {
            let direct_val = if is_standalone || !is_required {
                quote! { ::silex::dom::view::PropInto::prop_into(val) }
            } else {
                quote! { Some(::silex::dom::view::PropInto::prop_into(val)) }
            };
            quote! {
                pub fn #name(mut self, val: impl ::silex::dom::view::PropInto<#ty>) -> Self {
                    self.#name = #direct_val;
                    self
                }
            }
        }
    }

    /// 生成组件构造函数（用于简化宏调用的入口函数）
    fn generate_constructor(&self) -> TokenStream2 {
        let fn_name = &self.fn_name;
        let fn_vis = &self.fn_vis;
        let struct_name = &self.struct_name;
        let (impl_generics, ty_generics, where_clause) = self.fn_generics.split_for_impl();

        let mut params = Vec::new();
        let mut call_args = Vec::new();

        for info in &self.standalone_args {
            let p_name = &info.name;
            let ty = &info.ty;
            let type_ident = &info.type_ident;

            if info.into_trait {
                match type_ident.as_str() {
                    "SharedView" => {
                        params.push(quote! { #p_name: impl ::silex::dom::view::ApplyAttributes + ::silex::dom::view::MountRefExt });
                    }
                    "AnyView" => {
                        params.push(quote! { #p_name: impl ::silex::dom::view::MountExt });
                    }
                    _ => {
                        params.push(quote! { #p_name: impl Into<#ty> });
                    }
                }
            } else {
                params.push(quote! { #p_name: impl ::silex::dom::view::PropInto<#ty> });
            }
            call_args.push(quote! { #p_name });
        }

        quote! {
            #[allow(non_snake_case)]
            #fn_vis fn #fn_name #impl_generics(#(#params),*) -> #struct_name #ty_generics #where_clause {
                #struct_name::new(#(#call_args),*)
            }
        }
    }

    /// 展开生成所有相关的 Rust 代码
    fn expand(self) -> TokenStream2 {
        let struct_decl = self.gen_struct_decl();
        let impl_block = self.gen_impl_block();
        let attr_builder_impl = self.gen_attribute_builder_impl();
        let mount_impls = self.gen_mount_impls();
        let constructor = self.generate_constructor();

        quote! {
            #struct_decl
            #impl_block
            #attr_builder_impl
            #mount_impls
            #constructor
        }
    }

    fn gen_struct_decl(&self) -> TokenStream2 {
        let struct_name = &self.struct_name;
        let fn_vis = &self.fn_vis;
        let fields = &self.struct_fields;
        let phantom_decl = &self.phantom_decl;
        let (impl_generics, _, where_clause) = self.fn_generics.split_for_impl();

        quote! {
            #fn_vis struct #struct_name #impl_generics #where_clause {
                #(#fields,)*
                _pending_attrs: Vec<::silex::dom::attribute::PendingAttribute>,
                #phantom_decl
            }
        }
    }

    fn gen_impl_block(&self) -> TokenStream2 {
        let struct_name = &self.struct_name;
        let initializers = &self.new_initializers;
        let builders = &self.builder_methods;
        let phantom_init = &self.phantom_init;
        let (impl_generics, ty_generics, where_clause) = self.fn_generics.split_for_impl();

        let mut new_params = Vec::new();
        let mut new_prelude = Vec::new();

        for info in &self.standalone_args {
            let p_name = &info.name;
            let ty = &info.ty;
            let type_ident = &info.type_ident;

            if info.into_trait {
                match type_ident.as_str() {
                    "SharedView" => {
                        new_params.push(quote! { #p_name: impl ::silex::dom::view::ApplyAttributes + ::silex::dom::view::MountRefExt });
                        new_prelude.push(quote! {
                            use ::silex::dom::view::MountRefExt;
                            let #p_name = #p_name.into_shared();
                        });
                    }
                    "AnyView" => {
                        new_params.push(quote! { #p_name: impl ::silex::dom::view::MountExt });
                        new_prelude.push(quote! {
                            use ::silex::dom::view::MountExt;
                            let #p_name = #p_name.into_any();
                        });
                    }
                    _ => {
                        new_params.push(quote! { #p_name: impl Into<#ty> });
                        new_prelude.push(quote! { let #p_name = #p_name.into(); });
                    }
                }
            } else {
                new_params.push(quote! { #p_name: impl ::silex::dom::view::PropInto<#ty> });
                new_prelude.push(
                    quote! { let #p_name = ::silex::dom::view::PropInto::prop_into(#p_name); },
                );
            }
        }

        quote! {
            impl #impl_generics #struct_name #ty_generics #where_clause {
                pub fn new(#(#new_params),*) -> Self {
                    #(#new_prelude)*
                    Self {
                        #(#initializers,)*
                        _pending_attrs: Vec::new(),
                        #phantom_init
                    }
                }

                #(#builders)*
            }
        }
    }

    fn gen_attribute_builder_impl(&self) -> TokenStream2 {
        let struct_name = &self.struct_name;
        let (impl_generics, ty_generics, where_clause) = self.fn_generics.split_for_impl();

        quote! {
            impl #impl_generics ::silex::dom::attribute::AttributeBuilder for #struct_name #ty_generics #where_clause {
                fn build_attribute<__SilexValue>(mut self, target: ::silex::dom::attribute::ApplyTarget, value: __SilexValue) -> Self
                where __SilexValue: ::silex::dom::attribute::IntoStorable
                {
                    let owned_target = ::silex::dom::attribute::OwnedApplyTarget::from(target);
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
                     let event = event.clone();
                     self._pending_attrs.push(
                        ::silex::dom::attribute::PendingAttribute::new_listener(move |el| {
                            ::silex::dom::element::bind_event(el, event, callback.clone());
                        })
                    );
                    self
                }
            }

            impl #impl_generics ::silex::dom::view::ApplyAttributes for #struct_name #ty_generics #where_clause {}
        }
    }

    fn gen_mount_impls(&self) -> TokenStream2 {
        let struct_name = &self.struct_name;
        let fn_body = &self.fn_body;
        let mount_checks = &self.mount_checks;
        let mount_ref_checks = &self.mount_ref_checks;
        let (impl_generics, ty_generics, where_clause) = self.fn_generics.split_for_impl();

        quote! {
            impl #impl_generics ::silex::dom::view::Mount for #struct_name #ty_generics #where_clause {
                fn mount(self, parent: &::silex::reexports::web_sys::Node, attrs: Vec<::silex::dom::attribute::PendingAttribute>) {
                    #(#mount_checks)*
                    let view_instance = #fn_body;
                    let mut all_attrs = self._pending_attrs;
                    all_attrs.extend(attrs);
                    ::silex::dom::view::Mount::mount(view_instance, parent, all_attrs);
                }
            }

            impl #impl_generics ::silex::dom::view::MountRef for #struct_name #ty_generics #where_clause {
                fn mount_ref(&self, parent: &::silex::reexports::web_sys::Node, attrs: Vec<::silex::dom::attribute::PendingAttribute>) {
                    #(#mount_ref_checks)*
                    let view_instance = #fn_body;
                    let mut all_attrs = self._pending_attrs.clone();
                    all_attrs.extend(attrs);
                    ::silex::dom::view::MountRef::mount_ref(&view_instance, parent, all_attrs);
                }
            }
        }
    }
}

// --- Entry Point & Helpers ---

/// 生成组件的核心入口
pub fn generate_component(input_fn: ItemFn, attrs: ComponentAttrs) -> syn::Result<TokenStream2> {
    let mut generator = ComponentGenerator::new(input_fn.clone())
        .with_standalone_count(attrs.standalone);
    generator.prepare_phantom_data();
    generator.process_args(&input_fn.sig.inputs)?;
    Ok(generator.expand())
}

/// 解析属性上的标记，如 `#[prop(default = ...)]`
fn parse_prop_attrs(attrs: &[Attribute]) -> syn::Result<PropAttrs> {
    let mut result = PropAttrs {
        default: false,
        default_value: None,
        into_trait: false,
        clone: false,
    };

    for attr in attrs {
        if attr.path().is_ident("prop") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    result.default = true;
                    if meta.input.peek(syn::Token![=]) {
                        meta.input.parse::<syn::Token![=]>()?;
                        let expr: syn::Expr = meta.input.parse()?;
                        result.default_value = Some(quote! { #expr });
                    }
                    Ok(())
                } else if meta.path.is_ident("into") {
                    result.into_trait = true;
                    Ok(())
                } else if meta.path.is_ident("clone") {
                    result.clone = true;
                    Ok(())
                } else {
                    Err(meta.error("expected `default`, `into` or `clone`"))
                }
            })?;
        }
    }

    Ok(result)
}

/// 解析组件宏本身的属性
pub fn parse_component_attrs(args: TokenStream2) -> syn::Result<ComponentAttrs> {
    let mut standalone = 1;

    if args.is_empty() {
        return Ok(ComponentAttrs { standalone });
    }

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("standalone") {
            let value: syn::LitInt = meta.value()?.parse()?;
            standalone = value.base10_parse()?;
            Ok(())
        } else {
            Err(meta.error("unsupported component attribute"))
        }
    });

    parser.parse2(args)?;
    Ok(ComponentAttrs { standalone })
}

/// 判断类型是否应该默认开启 `into` 转换
fn is_auto_into_type(ident: &str) -> bool {
    matches!(
        ident,
        "AnyView"
            | "SharedView"
            | "String"
            | "PathBuf"
            | "Callback"
            | "Signal"
            | "ReadSignal"
            | "RwSignal"
            | "Memo"
    )
}

/// 获取类型的基本名称，用于特殊处理某些类型
fn get_base_type_name(ty: &Type) -> String {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident.to_string();
    }
    "".to_string()
}
