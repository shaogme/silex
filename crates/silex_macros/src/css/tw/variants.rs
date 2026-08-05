use crate::css::tw::{
    merge::{WriteSet, cluster},
    parser::{TokenAnchor, parse_class_list},
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, Visibility, braced, bracketed};

use silex_tw_core::normalize_variant_key;

/// 一个合并簇最多允许预编译的组合数
///
/// 只有**真的互相覆盖**的槽位才会进同一个簇，所以这个上限在实践中够用得多：
/// 仓库里 6 个组件（Button 是 6 × 8 两槽）一个簇都不会形成。
const MAX_VARIANT_COMBINATIONS: usize = 256;

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

/// 跨槽位层叠顺序的消解方案（报告 §3.5 / §5.4）
///
/// `base` **不参与**：`declare_variants!` 生成的 `write_class` 恒定先写 base、
/// 再写各个变体槽位、最后写 compound（见 `silex_css` 的 `test_declare_variants_basic`
/// 与 `test_declare_variants_compound` 两个顺序断言）。类名是在求值 `tw!(…)` 时
/// 注入样式表的，于是 base 的样式**必然**先于任何选项落盘，选项覆盖 base 是确定的——
/// 这也正是 CVA 语义要的先后。真正不确定的是**槽位与槽位之间**、
/// **槽位与 compound 之间**：谁先落盘取决于用户先渲染了哪个组合，换一次渲染顺序就翻盘。
#[derive(Default)]
struct MergePlan {
    /// 类名改由合并项承载、自身置空的变体槽位
    silenced_variants: BTreeSet<usize>,
    /// 内容已折进合并项、需要从 `compound_variants` 移除的下标
    silenced_compounds: BTreeSet<usize>,
    /// 追加的合并项：`(每个驱动槽位的 (变体名, 选项名), 合并后的词条串)`
    merged: Vec<(Vec<(String, String)>, String)>,
}

/// 把一串词条解析成规则；解析失败时交回 `None`，让后续既有的 `tw!` 去报那份错误
fn rules_of(class_str: &str, span: Span) -> Option<Vec<crate::css::tw::ast::UtilityRule>> {
    let mut extra = Vec::new();
    parse_class_list(&TokenAnchor::whole(class_str, span), &mut extra).ok()
}

fn write_set_of(class_str: &str, span: Span) -> WriteSet {
    rules_of(class_str, span)
        .map(|r| WriteSet::of(&r))
        .unwrap_or_default()
}

fn option_index(opts: &[(String, String)], name: &str) -> Option<usize> {
    let key = normalize_variant_key(name);
    opts.iter()
        .position(|(option, _)| normalize_variant_key(option) == key)
}

fn plan_merges(input: &TwVariantsMacroInput, span: Span) -> Result<MergePlan> {
    let n = input.variants.len();
    let m = input.compound_variants.len();
    if n + m < 2 {
        return Ok(MergePlan::default());
    }

    let variant_index = |name: &str| input.variants.iter().position(|(v, _)| v == name);

    // compound 引用了不存在的变体时不在这里报错：让既有的校验去给出那条更贴切的信息
    for cv in &input.compound_variants {
        if cv.conditions.keys().any(|k| variant_index(k).is_none()) {
            return Ok(MergePlan::default());
        }
    }

    let mut sets: Vec<WriteSet> = Vec::with_capacity(n + m);
    for (_, opts) in &input.variants {
        let mut set = WriteSet::default();
        for (_, cls) in opts {
            set.merge_from(&write_set_of(cls, span));
        }
        sets.push(set);
    }
    for cv in &input.compound_variants {
        sets.push(write_set_of(&cv.class_str, span));
    }

    let mut plan = MergePlan::default();
    for group in cluster(&sets) {
        if group.len() < 2 {
            continue;
        }
        let clustered_variants: Vec<usize> = group.iter().copied().filter(|&i| i < n).collect();
        let clustered_compounds: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&i| i >= n)
            .map(|i| i - n)
            .collect();

        // 驱动槽位 = 簇里的变体槽位 + 被簇内 compound 的条件引用到的变体槽位
        let mut drivers: BTreeSet<usize> = clustered_variants.iter().copied().collect();
        for &c in &clustered_compounds {
            for key in input.compound_variants[c].conditions.keys() {
                drivers.insert(variant_index(key).expect("已在上面校验过"));
            }
        }
        let drivers: Vec<usize> = drivers.into_iter().collect();

        let combinations: usize = drivers
            .iter()
            .map(|&d| input.variants[d].1.len())
            .product::<usize>();
        if combinations > MAX_VARIANT_COMBINATIONS {
            let names: Vec<&str> = drivers
                .iter()
                .map(|&d| input.variants[d].0.as_str())
                .collect();
            return Err(syn::Error::new(
                span,
                format!(
                    "变体 {} 写到了同一组 CSS 属性，要让它们的覆盖关系在编译期确定下来\
                     需要预编译 {} 个组合，超过了 {} 的上限。\
                     请让这些变体各自负责互不相干的属性，或把冲突的部分收进 base。",
                    names.join(" / "),
                    combinations,
                    MAX_VARIANT_COMBINATIONS
                ),
            ));
        }

        // 枚举驱动槽位的取值组合（把组合序号按各槽位的选项数逐位拆开）
        for combo in 0..combinations {
            let mut rem = combo;
            let mut choice = vec![0usize; drivers.len()];
            for (at, &d) in drivers.iter().enumerate().rev() {
                let len = input.variants[d].1.len();
                choice[at] = rem % len;
                rem /= len;
            }

            let picked: Vec<(String, String)> = drivers
                .iter()
                .zip(&choice)
                .map(|(&d, &c)| {
                    (
                        input.variants[d].0.clone(),
                        input.variants[d].1[c].0.clone(),
                    )
                })
                .collect();

            let mut parts: Vec<&str> = Vec::new();
            // 先写簇内变体槽位（按声明顺序），再写命中的 compound——与 `write_class` 同序
            for &v in &clustered_variants {
                let at = drivers.iter().position(|&d| d == v).expect("必是驱动槽位");
                parts.push(input.variants[v].1[choice[at]].1.as_str());
            }
            for &c in &clustered_compounds {
                let cv = &input.compound_variants[c];
                let hit = cv.conditions.iter().try_fold(true, |acc, (var, opt)| {
                    let chosen = &picked
                        .iter()
                        .find(|(v, _)| v == var)
                        .expect("条件引用的变体必在驱动集里")
                        .1;
                    // 与运行时字符串解析使用同一个规范化键，避免 compound
                    // 判断和 `get_checked` 对同一选项得出不同结果。
                    Ok::<bool, syn::Error>(
                        acc && normalize_variant_key(chosen) == normalize_variant_key(opt),
                    )
                })?;
                if hit {
                    parts.push(cv.class_str.as_str());
                }
            }

            parts.retain(|p| !p.trim().is_empty());
            plan.merged.push((picked, parts.join(" ")));
        }

        plan.silenced_variants.extend(clustered_variants);
        plan.silenced_compounds.extend(clustered_compounds);
    }

    Ok(plan)
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

    let plan = plan_merges(&input, span)?;

    let mut var_decls = Vec::with_capacity(input.variants.len());
    for (var_idx, (var_name, opts)) in input.variants.iter().enumerate() {
        if opts.is_empty() {
            return Err(syn::Error::new(
                span,
                format!("Variant '{}' must contain at least one option", var_name),
            ));
        }
        let var_name_ident = field_ident(var_name, span)?;
        let var_type_ident = type_ident(var_name)?;

        // PascalCase 冲突会让生成的枚举出现重复变体；规范化键冲突则会让
        // 字符串入口无法确定选中哪个变体。两套冲突都在宏阶段拒绝。
        let mut seen_identifiers: BTreeMap<String, String> = BTreeMap::new();
        let mut seen_keys: BTreeMap<String, String> = BTreeMap::new();
        for (opt_name, _) in opts {
            let pascal = to_pascal_case(opt_name, span)?.to_string();
            if let Some(prev) = seen_identifiers.insert(pascal.clone(), opt_name.clone()) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "Options '{}' and '{}' of variant '{}' both map to the enum variant \
                         `{}`; rename one of them.",
                        prev, opt_name, var_name, pascal
                    ),
                ));
            }

            let key = normalize_variant_key(opt_name);
            if let Some(prev) = seen_keys.insert(key, opt_name.clone()) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "Options '{}' and '{}' of variant '{}' normalize to the same string key; \
                         rename one of them.",
                        prev, opt_name, var_name
                    ),
                ));
            }
        }

        let def_opt_str = input
            .default_variants
            .get(var_name)
            .cloned()
            .unwrap_or_else(|| opts.first().map(|(k, _)| k.clone()).unwrap_or_default());
        let Some(def_opt_index) = option_index(opts, &def_opt_str) else {
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
        };
        let def_opt_ident = to_pascal_case(&opts[def_opt_index].0, span)?;

        // 被并进合并项的槽位自身不再产出类名：它的样式已经写进那张组合表，
        // 留在这里就成了同一份声明的第二个类，覆盖关系又交还给注入顺序
        let is_silenced = plan.silenced_variants.contains(&var_idx);
        let opt_entries = opts
            .iter()
            .map(|(opt_name, opt_cls)| {
                let opt_ident = to_pascal_case(opt_name, span)?;
                let opt_key = LitStr::new(opt_name, span);
                if is_silenced {
                    return Ok(quote! { #opt_ident [key = #opt_key] => "" });
                }
                Ok(quote! {
                    #opt_ident [key = #opt_key] => #__silex::macros::tw!(#opt_cls)
                })
            })
            .collect::<Result<Vec<_>>>()?;

        var_decls.push(quote! {
            pub #var_name_ident : #var_type_ident [default = #def_opt_ident] = {
                #(#opt_entries),*
            }
        });
    }

    let mut compound_entries: Vec<_> = input
        .compound_variants
        .iter()
        .enumerate()
        .filter(|(i, _)| !plan.silenced_compounds.contains(i))
        .map(|(_, cv)| {
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
                    let opts = input
                        .variants
                        .iter()
                        .find(|(name, _)| name == var_name)
                        .map(|(_, opts)| opts)
                        .expect("variant existence checked above");
                    let Some(opt_index) = option_index(opts, opt_val) else {
                        return Err(syn::Error::new(
                            span,
                            format!(
                                "compound_variants.{} is '{}', which is not one of its options ({}).",
                                var_name,
                                opt_val,
                                opts.iter()
                                    .map(|(name, _)| name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        ));
                    };
                    let opt_ident = to_pascal_case(&opts[opt_index].0, span)?;
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

    // 合并项：把互相覆盖的槽位/compound 折成一张"组合 → 完整合并后的类名"表。
    // 借用 compound_variants 这条既有通道，`declare_variants!` 不必为此改一行——
    // compound 本来就是"条件全部命中时追加一个类"，合并项就是它的一般形式。
    for (picked, merged_cls) in &plan.merged {
        let cond_checks = picked
            .iter()
            .map(|(var_name, opt_name)| {
                let var_ident = field_ident(var_name, span)?;
                let var_type_ident = type_ident(var_name)?;
                let opt_ident = to_pascal_case(opt_name, span)?;
                Ok(quote! { #var_ident == #var_type_ident :: #opt_ident })
            })
            .collect::<Result<Vec<_>>>()?;
        compound_entries.push(quote! {
            ( #(#cond_checks),* ) => #__silex::macros::tw!(#merged_cls)
        });
    }

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
    fn case_insensitive_option_names_are_rejected() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { sm: "text-sm", SM: "text-uppercase" } },
        });
        assert!(err.contains("normalize to the same string key"), "{err}");
    }

    #[test]
    fn separator_insensitive_option_names_are_rejected() {
        let err = err_of(quote! {
            base: "p-4",
            variants: { size: { "icon-xs": "a", iconxs: "b" } },
        });
        assert!(err.contains("normalize to the same string key"), "{err}");
    }

    #[test]
    fn numeric_option_names_emit_their_original_match_key() {
        let out = expand(quote! {
            base: "p-4",
            variants: { size: { "1x": "text-1x" } },
        });
        assert!(out.contains("Val1x [key = \"1x\"]"), "{out}");
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

    // -----------------------------------------------------------------
    // §3.5 / §5.4 跨槽位层叠顺序
    // -----------------------------------------------------------------

    fn expand(ts: TokenStream) -> String {
        tw_variants_impl(ts).unwrap().to_string()
    }

    /// 互不覆盖的槽位**不合并**——仓库里 6 个 UI 组件都属于这一类，
    /// 产物必须与修复前一模一样，不能因为这项修复膨胀出组合表
    #[test]
    fn independent_variant_slots_are_not_combined() {
        let out = expand(quote! {
            base: "inline-flex rounded-md text-sm",
            variants: {
                variant: { default: "bg-primary text-primary-foreground", ghost: "hover:bg-accent" },
                size: { default: "h-9 px-4 py-2", sm: "h-8 px-3" },
            },
        });
        // 每个选项照旧各自 `tw!`，没有 compound_variants 段
        assert!(
            out.contains(r#"tw ! ("bg-primary text-primary-foreground")"#),
            "{out}"
        );
        assert!(out.contains(r#"tw ! ("h-9 px-4 py-2")"#), "{out}");
        assert!(!out.contains("compound_variants"), "{out}");
    }

    /// 两个槽位写到同一个属性时，谁覆盖谁此前取决于用户先渲染了哪个组合。
    /// 现在展开成"组合 → 完整合并后的类名"，覆盖关系回到编译期。
    #[test]
    fn conflicting_variant_slots_expand_into_a_combination_table() {
        let out = expand(quote! {
            base: "rounded",
            variants: {
                tone: { light: "bg-white", dark: "bg-black" },
                state: { on: "bg-blue-500", off: "" },
            },
        });

        // 2 × 2 四个组合，每个都是合并后的完整词条串
        assert!(out.contains(r#"tw ! ("bg-white bg-blue-500")"#), "{out}");
        assert!(out.contains(r#"tw ! ("bg-black bg-blue-500")"#), "{out}");
        assert!(out.contains(r#"tw ! ("bg-white")"#), "{out}");
        assert!(out.contains(r#"tw ! ("bg-black")"#), "{out}");

        // 槽位自身不再产出类名，否则同一份声明会有第二个类
        assert!(!out.contains(r#"tw ! ("bg-blue-500")"#), "{out}");
        assert!(out.contains(r#"Light [key = "light"] => """#), "{out}");
        assert!(out.contains(r#"On [key = "on"] => """#), "{out}");

        // base 不参与：它恒定先于任何选项写入，覆盖关系本来就是确定的
        assert!(out.contains(r#"tw ! ("rounded")"#), "{out}");
    }

    /// compound 与它所覆盖的槽位同理：折进组合表，而不是留成第三个类
    #[test]
    fn a_compound_that_overrides_a_slot_is_folded_in() {
        let out = expand(quote! {
            base: "border",
            variants: {
                size: { sm: "p-2", lg: "p-6" },
            },
            compound_variants: [ { size: "lg", class: "p-8" } ],
        });
        assert!(out.contains(r#"tw ! ("p-6 p-8")"#), "{out}");
        assert!(out.contains(r#"tw ! ("p-2")"#), "{out}");
        assert!(!out.contains(r#"tw ! ("p-8")"#), "{out}");
        assert!(out.contains(r#"Lg [key = "lg"] => """#), "{out}");
    }

    /// 组合数按各槽位选项数相乘，超过上限时报错而不是悄悄退回不确定的老行为
    #[test]
    fn too_many_conflicting_combinations_are_rejected() {
        let mut opts_a = TokenStream::new();
        let mut opts_b = TokenStream::new();
        for i in 0..20u32 {
            let name = format_ident!("o{}", i);
            let cls = format!("p-{}", i);
            opts_a.extend(quote! { #name: #cls, });
            let cls2 = format!("px-{}", i);
            opts_b.extend(quote! { #name: #cls2, });
        }
        let err = err_of(quote! {
            base: "border",
            variants: {
                a: { #opts_a },
                b: { #opts_b },
            },
        });
        assert!(err.contains("上限"), "{err}");
        assert!(err.contains("a / b"), "{err}");
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
