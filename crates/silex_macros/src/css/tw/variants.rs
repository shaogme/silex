use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, Visibility, braced, bracketed};

/// 复合变体结构输入
#[derive(Debug, Clone)]
pub struct CompoundVariantInput {
    pub conditions: BTreeMap<String, String>,
    pub class_str: String,
}

/// `tw_variants!` 宏 AST 输入结构
#[derive(Debug, Clone)]
pub struct TwVariantsMacroInput {
    /// item 形式的目标类型名与可见性：`tw_variants! { pub struct ButtonStyle { … } }`
    ///
    /// 为 `None` 时是旧的表达式形式，展开成一个块并返回 helper 实例——
    /// 那种形式生成的类型全在块内部，用户无法把它放进结构体字段或函数签名（报告 §5.1）。
    pub struct_name: Option<Ident>,
    pub vis: Visibility,
    pub base_str: String,
    /// variant_name -> Vec<(option_name, class_str)>
    pub variants: Vec<(String, Vec<(String, String)>)>,
    /// variant_name -> default_option_name
    pub default_variants: BTreeMap<String, String>,
    pub compound_variants: Vec<CompoundVariantInput>,
}

fn parse_key_str(input: ParseStream) -> Result<String> {
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        Ok(lit.value())
    } else if input.peek(Ident) {
        let ident: Ident = input.parse()?;
        Ok(ident.to_string())
    } else {
        Err(input.error("Expected identifier or string literal as key"))
    }
}

/// 解析 DSL 主体（`base` / `variants` / `default_variants` / `compound_variants`）
fn parse_body(input: ParseStream, out: &mut TwVariantsMacroInput) -> Result<()> {
    while !input.is_empty() {
        let key_span = input.span();
        let key = parse_key_str(input)?;
        input.parse::<Token![:]>()?;

        match key.as_str() {
            "base" => {
                let lit: LitStr = input.parse()?;
                out.base_str = lit.value();
            }
            "variants" => {
                let content;
                braced!(content in input);
                while !content.is_empty() {
                    let var_name = parse_key_str(&content)?;
                    content.parse::<Token![:]>()?;

                    let opts_content;
                    braced!(opts_content in content);
                    let mut opts_vec = Vec::new();

                    while !opts_content.is_empty() {
                        let opt_name = parse_key_str(&opts_content)?;
                        opts_content.parse::<Token![:]>()?;
                        let lit: LitStr = opts_content.parse()?;
                        opts_vec.push((opt_name, lit.value()));

                        if opts_content.peek(Token![,]) {
                            let _: Token![,] = opts_content.parse()?;
                        }
                    }

                    out.variants.push((var_name, opts_vec));

                    if content.peek(Token![,]) {
                        let _: Token![,] = content.parse()?;
                    }
                }
            }
            "default_variants" => {
                let content;
                braced!(content in input);
                while !content.is_empty() {
                    let var_name = parse_key_str(&content)?;
                    content.parse::<Token![:]>()?;
                    let def_opt = parse_key_str(&content)?;
                    out.default_variants.insert(var_name, def_opt);

                    if content.peek(Token![,]) {
                        let _: Token![,] = content.parse()?;
                    }
                }
            }
            "compound_variants" => {
                let content;
                bracketed!(content in input);
                while !content.is_empty() {
                    let item_content;
                    braced!(item_content in content);
                    let mut conds = BTreeMap::new();
                    let mut class_str = String::new();

                    while !item_content.is_empty() {
                        let item_key = parse_key_str(&item_content)?;
                        item_content.parse::<Token![:]>()?;

                        if item_key == "class" || item_key == "css" {
                            let lit: LitStr = item_content.parse()?;
                            class_str = lit.value();
                        } else {
                            let val = parse_key_str(&item_content)?;
                            conds.insert(item_key, val);
                        }

                        if item_content.peek(Token![,]) {
                            let _: Token![,] = item_content.parse()?;
                        }
                    }

                    out.compound_variants.push(CompoundVariantInput {
                        conditions: conds,
                        class_str,
                    });

                    if content.peek(Token![,]) {
                        let _: Token![,] = content.parse()?;
                    }
                }
            }
            _ => {
                return Err(syn::Error::new(
                    key_span,
                    format!(
                        "Unknown key '{}' in tw_variants! macro. Expected one of \
                         `base`, `variants`, `default_variants`, `compound_variants`.",
                        key
                    ),
                ));
            }
        }

        if input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
        }
    }
    Ok(())
}

impl Parse for TwVariantsMacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut out = TwVariantsMacroInput {
            struct_name: None,
            vis: Visibility::Inherited,
            base_str: String::new(),
            variants: Vec::new(),
            default_variants: BTreeMap::new(),
            compound_variants: Vec::new(),
        };

        // item 形式：`[pub] struct Name { … }`——类型定义落在调用方作用域，可命名
        let fork = input.fork();
        let is_item_form = {
            let _: Result<Visibility> = fork.parse();
            fork.peek(Token![struct])
        };

        if is_item_form {
            out.vis = input.parse()?;
            input.parse::<Token![struct]>()?;
            out.struct_name = Some(input.parse()?);
            let content;
            braced!(content in input);
            parse_body(&content, &mut out)?;
            if !input.is_empty() {
                return Err(input.error("Unexpected tokens after the tw_variants! struct body"));
            }
        } else {
            parse_body(input, &mut out)?;
        }

        Ok(out)
    }
}

/// 把选项名/变体名转成 PascalCase 标识符
///
/// 非法字符不再 `panic`：proc-macro panic 只会给出 `proc macro panicked` 这种
/// 不可读的信息，而 `syn::Error` 能指出是哪个名字的问题（报告 §5.3）。
fn to_pascal_case(s: &str, span: Span) -> Result<Ident> {
    let clean = s.trim();
    if clean.is_empty() {
        return Err(syn::Error::new(
            span,
            "Variant option name must not be empty",
        ));
    }

    let mut res = String::with_capacity(clean.len());
    for chunk in clean.split(['-', '_', ' ']) {
        let mut chars = chunk.chars();
        if let Some(first) = chars.next() {
            res.extend(first.to_uppercase());
            res.push_str(chars.as_str());
        }
    }
    if res.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        res = format!("Val{}", res);
    }

    if res.is_empty() || !res.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(syn::Error::new(
            span,
            format!(
                "Variant option name '{}' cannot be turned into a Rust identifier. \
                 Use letters, digits, `-`, `_` or spaces only.",
                s
            ),
        ));
    }

    Ok(Ident::new(&res, span))
}

/// 变体名（结构体字段名）必须本身就是合法标识符
fn field_ident(name: &str, span: Span) -> Result<Ident> {
    let clean = name.trim().replace('-', "_");
    if clean.is_empty()
        || clean.chars().next().is_some_and(|c| c.is_ascii_digit())
        || !clean.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(syn::Error::new(
            span,
            format!(
                "Variant name '{}' is not a valid Rust field name. \
                 Use letters, digits and `_` (or `-`, which becomes `_`).",
                name
            ),
        ));
    }
    Ok(format_ident!("{}", clean, span = span))
}

/// `tw_variants!` 过程宏核心入口实现
///
/// 两种形式：
///
/// ```ignore
/// // item 形式（推荐）：类型定义在调用方作用域，可命名、可放进结构体字段与函数签名
/// tw_variants! {
///     pub struct ButtonStyle {
///         base: "inline-flex",
///         variants: { size: { sm: "text-sm", lg: "text-lg" } },
///         default_variants: { size: "sm" },
///     }
/// }
/// let cls = ButtonStyle::new().with_size(ButtonStyleSize::Lg).class();
///
/// // 表达式形式（旧）：展开成一个块，类型不可命名
/// let styles = tw_variants! { base: "inline-flex", variants: { … } };
/// ```
pub fn tw_variants_impl(ts: TokenStream) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let span = Span::call_site();
    let input: TwVariantsMacroInput = syn::parse2(ts)?;

    let base_str = &input.base_str;
    let is_item_form = input.struct_name.is_some();
    let struct_ident = input
        .struct_name
        .clone()
        .unwrap_or_else(|| Ident::new("TwVariantsHelper", span));

    // item 形式下枚举名是 `<结构体名><变体名>`，用户能直接写出来；
    // 表达式形式沿用 `TwVariant<变体名>`（块内部私有，叫什么都无妨）
    let type_ident = |var_name: &str| -> Result<Ident> {
        let pascal = to_pascal_case(var_name, span)?;
        let name = if is_item_form {
            format!("{}{}", struct_ident, pascal)
        } else {
            format!("TwVariant{}", pascal)
        };
        Ok(Ident::new(&name, span))
    };

    let mut var_decls = Vec::with_capacity(input.variants.len());
    for (var_name, opts) in &input.variants {
        if opts.is_empty() {
            return Err(syn::Error::new(
                span,
                format!("Variant '{}' must contain at least one option", var_name),
            ));
        }
        let var_name_ident = field_ident(var_name, span)?;
        let var_type_ident = type_ident(var_name)?;

        // PascalCase 会把 `icon-xs` / `icon_xs` / `iconxs` 折成同一个 `IconXs`，
        // 撞名的话生成的枚举里会出现重复变体——直接报错，别让用户去猜
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for (opt_name, _) in opts {
            let pascal = to_pascal_case(opt_name, span)?.to_string();
            if let Some(prev) = seen.insert(pascal.clone(), opt_name.clone()) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "Options '{}' and '{}' of variant '{}' both map to the enum variant \
                         `{}`; rename one of them.",
                        prev, opt_name, var_name, pascal
                    ),
                ));
            }
        }

        let def_opt_str = input
            .default_variants
            .get(var_name)
            .cloned()
            .unwrap_or_else(|| opts.first().map(|(k, _)| k.clone()).unwrap_or_default());
        if !opts.iter().any(|(k, _)| k == &def_opt_str) {
            return Err(syn::Error::new(
                span,
                format!(
                    "default_variants.{} is '{}', which is not one of its options ({}).",
                    var_name,
                    def_opt_str,
                    opts.iter()
                        .map(|(k, _)| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        let def_opt_ident = to_pascal_case(&def_opt_str, span)?;

        let opt_entries = opts
            .iter()
            .map(|(opt_name, opt_cls)| {
                let opt_ident = to_pascal_case(opt_name, span)?;
                Ok(quote! {
                    #opt_ident => #__silex::macros::tw!(#opt_cls)
                })
            })
            .collect::<Result<Vec<_>>>()?;

        var_decls.push(quote! {
            pub #var_name_ident : #var_type_ident [default = #def_opt_ident] = {
                #(#opt_entries),*
            }
        });
    }

    let compound_entries: Vec<_> = input
        .compound_variants
        .iter()
        .map(|cv| {
            let cond_checks = cv
                .conditions
                .iter()
                .map(|(var_name, opt_val)| {
                    if !input.variants.iter().any(|(v, _)| v == var_name) {
                        return Err(syn::Error::new(
                            span,
                            format!(
                                "compound_variants references unknown variant '{}'",
                                var_name
                            ),
                        ));
                    }
                    let var_ident = field_ident(var_name, span)?;
                    let var_type_ident = type_ident(var_name)?;
                    let opt_ident = to_pascal_case(opt_val, span)?;
                    Ok(quote! {
                        #var_ident == #var_type_ident :: #opt_ident
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            let cmp_cls = &cv.class_str;
            Ok(quote! {
                ( #(#cond_checks),* ) => #__silex::macros::tw!(#cmp_cls)
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // --- 字符串驱动的兼容接口（运行时 `Signal<String>` 场景） ---
    let mut get_params = Vec::new();
    let mut get_inits = Vec::new();
    let mut get_checked_inits = Vec::new();
    let mut get_opt_params = Vec::new();
    let mut get_opt_inits = Vec::new();
    let mut setters = Vec::new();
    for (var_name, _) in &input.variants {
        let var_ident = field_ident(var_name, span)?;
        let var_type_ident = type_ident(var_name)?;
        let setter_ident = format_ident!("with_{}", var_ident);

        get_params.push(quote! { #var_ident: impl ::std::convert::AsRef<str> });
        get_inits.push(quote! { #var_ident: #var_type_ident::from(#var_ident) });
        get_checked_inits.push(quote! {
            #var_ident: #var_type_ident::try_from_str(#var_ident.as_ref())?
        });
        get_opt_params
            .push(quote! { #var_ident: ::std::option::Option<impl ::std::convert::AsRef<str>> });
        get_opt_inits.push(quote! {
            #var_ident: #var_ident.map(#var_type_ident::from).unwrap_or_default()
        });
        setters.push(quote! {
            /// 链式设置该变体（编译期类型检查，写错选项名根本编译不过）
            pub fn #setter_ident(mut self, value: #var_type_ident) -> Self {
                self.#var_ident = value;
                self
            }
        });
    }

    let compound_block = if compound_entries.is_empty() {
        quote! {}
    } else {
        quote! {
            compound_variants: [
                #(#compound_entries),*
            ]
        }
    };

    let vis = &input.vis;
    let schema = quote! {
        #__silex::css::declare_variants! {
            #vis struct #struct_ident {
                base: #__silex::macros::tw!(#base_str),
                variants: {
                    #(#var_decls),*
                },
                #compound_block
            }
        }

        #[allow(dead_code)]
        impl #struct_ident {
            #(#setters)*

            /// 渲染当前配置对应的完整类名
            pub fn class(&self) -> ::std::string::String {
                #__silex::css::cx!(self)
            }

            /// 由字符串渲染（运行时 `Signal<String>` 场景）
            ///
            /// 未知选项名回退到默认值；需要把拼写错误暴露出来时用 [`Self::get_checked`]。
            pub fn get(&self, #(#get_params),*) -> ::std::string::String {
                let config = Self {
                    #(#get_inits),*
                };
                #__silex::css::cx!(config)
            }

            /// [`Self::get`] 的严格版本：未知选项名返回 `Err`，不静默套用默认样式
            pub fn get_checked(
                &self,
                #(#get_params),*
            ) -> ::std::result::Result<
                ::std::string::String,
                #__silex::css::tw::variants::UnknownVariantOption,
            > {
                let config = Self {
                    #(#get_checked_inits),*
                };
                ::std::result::Result::Ok(#__silex::css::cx!(config))
            }

            pub fn get_opt(&self, #(#get_opt_params),*) -> ::std::string::String {
                let config = Self {
                    #(#get_opt_inits),*
                };
                #__silex::css::cx!(config)
            }
        }
    };

    Ok(if is_item_form {
        // item 形式：直接把定义放到调用方作用域，类型因此可命名
        schema
    } else {
        quote! {
            {
                #schema
                #struct_ident::new()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_of(ts: TokenStream) -> String {
        tw_variants_impl(ts)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| panic!("expected an error"))
    }

    #[test]
    fn test_empty_variant_options_error() {
        let input = quote! {
            base: "p-4",
            variants: {
                color: {}
            }
        };
        let err = err_of(input);
        assert!(
            err.contains("must contain at least one option"),
            "Unexpected error message: {}",
            err
        );
    }

    /// 报告 §5.1：item 形式把类型定义放在调用方作用域
    #[test]
    fn item_form_emits_nameable_types() {
        let out = tw_variants_impl(quote! {
            pub struct ButtonStyle {
                base: "inline-flex",
                variants: {
                    size: { sm: "text-sm", lg: "text-lg" },
                },
                default_variants: { size: "sm" },
            }
        })
        .unwrap()
        .to_string();

        // 没有外层块——定义直接落在模块作用域
        assert!(!out.trim_start().starts_with('{'), "{out}");
        assert!(out.contains("pub struct ButtonStyle"), "{out}");
        // 枚举名可预测：`<结构体名><变体名>`
        assert!(out.contains("ButtonStyleSize"), "{out}");
        assert!(out.contains("with_size"), "{out}");
        assert!(out.contains("get_checked"), "{out}");
    }

    #[test]
    fn expression_form_still_returns_a_value() {
        let out = tw_variants_impl(quote! {
            base: "p-4",
            variants: { size: { sm: "text-sm" } },
        })
        .unwrap()
        .to_string();
        assert!(out.trim_start().starts_with('{'), "{out}");
        assert!(out.contains("TwVariantsHelper :: new"), "{out}");
    }

    /// 报告 §5.3：非法标识符字符此前会让 proc-macro panic，
    /// 用户看到的是 `proc macro panicked` 而不是可读的错误
    #[test]
    fn illegal_names_produce_a_syn_error_not_a_panic() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { "size.lg": "text-lg" } },
        });
        assert!(
            err.contains("cannot be turned into a Rust identifier"),
            "{err}"
        );

        let err = err_of(quote! {
            base: "p-4",
            variants: { "size.x": { lg: "text-lg" } },
        });
        assert!(err.contains("not a valid Rust field name"), "{err}");
    }

    #[test]
    fn colliding_option_names_are_rejected() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { "icon-xs": "a", "icon_xs": "b" } },
        });
        assert!(err.contains("both map to the enum variant"), "{err}");
    }

    #[test]
    fn unknown_default_variant_is_rejected() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { sm: "text-sm" } },
            default_variants: { size: "lg" },
        });
        assert!(err.contains("not one of its options"), "{err}");
    }

    #[test]
    fn compound_variant_referencing_unknown_variant_is_rejected() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { sm: "text-sm" } },
            compound_variants: [ { tone: "loud", class: "x" } ],
        });
        assert!(err.contains("unknown variant 'tone'"), "{err}");
    }
}
