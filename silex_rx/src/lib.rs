use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::visit_mut::{self, VisitMut};
use syn::{Expr, parse2};

/// 用于收集并处理 $ 变量的访问器
struct SignalVisitor {
    // 映射：原始 Ident -> 内部生成的引用 Ident
    signal_map: HashMap<syn::Ident, syn::Ident>,
}

impl VisitMut for SignalVisitor {
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        if let Expr::Path(expr_path) = i
            && let Some(segment) = expr_path.path.segments.last_mut()
        {
            let name = segment.ident.to_string();
            if let Some(original_name) = name.strip_prefix("__silex_rx_sig_") {
                let span = segment.ident.span();
                let original_ident = format_ident!("{}", original_name, span = span);

                // 为该信号生成一个唯一的内部变量名（用于 .with 闭包参数）
                // 这样就不会遮蔽外部同名的信号句柄
                let ref_ident = self
                    .signal_map
                    .entry(original_ident.clone())
                    .or_insert_with(|| format_ident!("__ref_{}", original_name, span = span));

                segment.ident = ref_ident.clone();
            }
        }
        visit_mut::visit_expr_mut(self, i);
    }

    fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
        fn process_tokens(
            tokens: proc_macro2::TokenStream,
            signal_map: &mut HashMap<syn::Ident, syn::Ident>,
        ) -> proc_macro2::TokenStream {
            tokens
                .into_iter()
                .map(|tt| match tt {
                    proc_macro2::TokenTree::Ident(id) => {
                        let name = id.to_string();
                        if let Some(original_name) = name.strip_prefix("__silex_rx_sig_") {
                            let span = id.span();
                            let original_ident = format_ident!("{}", original_name, span = span);
                            let ref_ident =
                                signal_map.entry(original_ident.clone()).or_insert_with(|| {
                                    format_ident!("__ref_{}", original_name, span = span)
                                });
                            proc_macro2::TokenTree::Ident(ref_ident.clone())
                        } else {
                            proc_macro2::TokenTree::Ident(id)
                        }
                    }
                    proc_macro2::TokenTree::Group(g) => {
                        let inner = process_tokens(g.stream(), signal_map);
                        let mut new_group = proc_macro2::Group::new(g.delimiter(), inner);
                        new_group.set_span(g.span());
                        proc_macro2::TokenTree::Group(new_group)
                    }
                    _ => tt,
                })
                .collect()
        }
        i.tokens = process_tokens(i.tokens.clone(), &mut self.signal_map);
        visit_mut::visit_macro_mut(self, i);
    }
}

fn preprocess_tokens(tokens: proc_macro2::TokenStream) -> (proc_macro2::TokenStream, bool) {
    let mut output = proc_macro2::TokenStream::new();
    let mut found_invalid_dollar = false;
    let mut iter = tokens.into_iter().peekable();

    while let Some(tt) = iter.next() {
        match tt {
            proc_macro2::TokenTree::Punct(ref p) if p.as_char() == '$' => {
                if let Some(proc_macro2::TokenTree::Ident(id)) = iter.peek() {
                    let id = id.clone();
                    let span = id.span();
                    iter.next(); // consume ident
                    let placeholder = format_ident!("__silex_rx_sig_{}", id, span = span);
                    output.extend(std::iter::once(proc_macro2::TokenTree::Ident(placeholder)));
                } else {
                    found_invalid_dollar = true;
                    output.extend(std::iter::once(proc_macro2::TokenTree::Punct(p.clone())));
                }
            }
            proc_macro2::TokenTree::Group(g) => {
                let (inner_tokens, inner_err) = preprocess_tokens(g.stream());
                if inner_err {
                    found_invalid_dollar = true;
                }
                let mut new_group = proc_macro2::Group::new(g.delimiter(), inner_tokens);
                new_group.set_span(g.span());
                output.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
            }
            _ => {
                output.extend(std::iter::once(tt));
            }
        }
    }
    (output, found_invalid_dollar)
}

/// `rx!` 过程宏：实现智能信号包装与零拷贝多路访问。
#[proc_macro]
pub fn rx(input: TokenStream) -> TokenStream {
    let mut iter = proc_macro2::TokenStream::from(input).into_iter();

    // 尝试解析前缀路径
    let mut prefix = quote! { ::silex_core };
    let mut raw_input = proc_macro2::TokenStream::new();
    let mut first_part = proc_macro2::TokenStream::new();
    let mut found_semi = false;

    for tt in iter.by_ref() {
        if let proc_macro2::TokenTree::Punct(ref p) = tt
            && p.as_char() == ';'
        {
            found_semi = true;
            break;
        }
        first_part.extend(std::iter::once(tt));
    }

    if found_semi {
        prefix = first_part;
        raw_input.extend(iter);
    } else {
        raw_input = first_part;
    }

    if raw_input.is_empty() {
        return quote! {
            #prefix::Rx(move || {}, ::core::marker::PhantomData::<#prefix::RxValue>)
        }
        .into();
    }

    let (processed_input, found_err) = preprocess_tokens(raw_input.clone());
    if found_err {
        return syn::Error::new_spanned(
            raw_input,
            "Invalid signal identifier: '$' alone is not allowed",
        )
        .to_compile_error()
        .into();
    }

    let mut expr = match parse2::<Expr>(processed_input.clone()) {
        Ok(e) => e,
        Err(_) => match parse2::<syn::Block>(processed_input.clone()) {
            Ok(block) => Expr::Block(syn::ExprBlock {
                attrs: vec![],
                label: None,
                block,
            }),
            Err(_) => {
                return quote! {
                        #prefix::Rx(move || { #raw_input }, ::core::marker::PhantomData::<#prefix::RxValue>)
                    }
                    .into();
            }
        },
    };

    let mut visitor = SignalVisitor {
        signal_map: HashMap::new(),
    };
    visitor.visit_expr_mut(&mut expr);

    let m_type = if let Expr::Closure(ref closure) = expr {
        if !closure.inputs.is_empty() {
            quote! { #prefix::RxEffect }
        } else {
            quote! { #prefix::RxValue }
        }
    } else {
        quote! { #prefix::RxValue }
    };

    // 准备信号列表
    let mut pairs: Vec<_> = visitor.signal_map.into_iter().collect();
    pairs.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));

    let original_idents: Vec<_> = pairs.iter().map(|p| &p.0).collect();

    // 构建嵌套逻辑
    let inner_logic = if let Expr::Closure(mut closure) = expr {
        closure.capture = Some(syn::token::Move::default());
        let body = &closure.body;
        let mut nested = quote! { #body };
        // 按相反顺序嵌套，以便外层先被处理
        for (orig, refer) in pairs.iter().rev() {
            nested = quote! {
                #prefix::traits::With::with(&#orig, |#refer| {
                    #nested
                })
            };
        }
        *closure.body = parse2::<Expr>(quote! { { #nested } }).unwrap();
        quote! { #closure }
    } else {
        let mut nested = quote! { #expr };
        for (orig, refer) in pairs.iter().rev() {
            nested = quote! {
                #prefix::traits::With::with(&#orig, |#refer| {
                    #nested
                })
            };
        }
        quote! { move || { #nested } }
    };

    // 最终克隆与构造
    let output = quote! {
        {
            #(let #original_idents = #original_idents.clone();)*
            #prefix::Rx(#inner_logic, ::core::marker::PhantomData::<#m_type>)
        }
    };

    output.into()
}
