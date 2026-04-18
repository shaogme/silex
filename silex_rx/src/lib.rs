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
            #prefix::Rx::<(), #prefix::RxValueKind>::new_constant(())
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

    // 检测 @fn 标记
    let mut force_static = false;
    let mut final_raw_input = processed_input.clone();

    let mut tokens_iter = processed_input.clone().into_iter().peekable();
    if let Some(proc_macro2::TokenTree::Punct(p)) = tokens_iter.peek()
        && p.as_char() == '@'
    {
        tokens_iter.next(); // consume @
        if let Some(proc_macro2::TokenTree::Ident(id)) = tokens_iter.peek()
            && *id == "fn"
        {
            tokens_iter.next(); // consume fn
            force_static = true;
            final_raw_input = tokens_iter.collect();
        }
    }

    let mut expr = match parse2::<Expr>(final_raw_input.clone()) {
        Ok(e) => e,
        Err(_) => match parse2::<syn::Block>(final_raw_input.clone()) {
            Ok(block) => Expr::Block(syn::ExprBlock {
                attrs: vec![],
                label: None,
                block,
            }),
            Err(_) => {
                if force_static {
                    return syn::Error::new_spanned(
                        final_raw_input,
                        "@fn requires a valid expression or block",
                    )
                    .to_compile_error()
                    .into();
                }
                return quote! {
                    #prefix::Rx::derive(Box::new(move || { #raw_input }))
                }
                .into();
            }
        },
    };

    let mut visitor = SignalVisitor {
        signal_map: HashMap::new(),
    };
    visitor.visit_expr_mut(&mut expr);

    // 准备信号列表
    let mut pairs: Vec<_> = visitor.signal_map.into_iter().collect();
    pairs.sort_by_key(|a| a.0.to_string());

    // 构建扁平化的捕获和读取逻辑
    // 捕获阶段：在闭包外克隆信号句柄
    // 读取阶段：在闭包内通过 .read() 获取 Guard
    let mut capture_stmts = vec![];
    let mut read_init_stmts = vec![];
    for (orig, refer) in &pairs {
        let sig_name = format_ident!("__sig_{}", orig);
        let guard_name = format_ident!("__guard_{}", orig);
        capture_stmts.push(quote! { let #sig_name = #orig.clone(); });
        read_init_stmts.push(quote! {
            let #guard_name = #prefix::traits::RxRead::read(&#sig_name);
            let #refer = &*#guard_name;
        });
    }

    let (f_expr, is_stored) = if let Expr::Closure(mut closure) = expr.clone() {
        closure.capture = Some(syn::token::Move::default());
        let has_inputs = !closure.inputs.is_empty();
        let body = &closure.body;

        *closure.body = parse2::<Expr>(quote! {
            {
                #(#read_init_stmts)*
                #body
            }
        })
        .unwrap();

        (quote! { #closure }, has_inputs)
    } else {
        (
            quote! {
                move || {
                    #(#read_init_stmts)*
                    #expr
                }
            },
            false,
        )
    };

    // 最终构造：根据是否带有参数决定调用 derive (计算) 还是 effect (存储/副作用)
    let output = if force_static {
        if is_stored {
            // @fn 目前仅支持计算映射 (无参数闭包或带有 $ 变量的表达式)
            // 带有参数的闭包 (|el| ...) 目前无法直接静态化，因为参数由外部注入，尚未在 StaticPayload 中实现支持。
            return syn::Error::new_spanned(
                final_raw_input,
                "@fn optimization is not supported for effect closures with parameters (like |el: &Element|).",
            )
            .to_compile_error()
            .into();
        }

        if pairs.is_empty() {
            // 如果没有探测到信号变量，判断是否为字面量
            let is_literal = matches!(expr, syn::Expr::Lit(_));
            if is_literal {
                quote! { #prefix::Rx::<_, #prefix::RxValueKind>::new_constant(#expr) }
            } else {
                // 如果不是字面量（可能是方法调用如 login.loading()），回退到 derive 以确保响应性
                quote! { #prefix::Rx::<_, #prefix::RxValueKind>::derive(Box::new(#f_expr)) }
            }
        } else if pairs.len() == 1 {
            let (orig, refer) = &pairs[0];
            quote! {
                #prefix::macros_helper::map1_static(#orig.clone(), |#refer| { #expr })
            }
        } else if pairs.len() == 2 {
            let (orig1, refer1) = &pairs[0];
            let (orig2, refer2) = &pairs[1];
            quote! {
                #prefix::macros_helper::map2_static(#orig1.clone(), #orig2.clone(), |#refer1, #refer2| { #expr })
            }
        } else if pairs.len() == 3 {
            let (orig1, refer1) = &pairs[0];
            let (orig2, refer2) = &pairs[1];
            let (orig3, refer3) = &pairs[2];
            quote! {
                #prefix::macros_helper::map3_static(#orig1.clone(), #orig2.clone(), #orig3.clone(), |#refer1, #refer2, #refer3| { #expr })
            }
        } else {
            // 超过 3 个信号，报错提醒（或者你可以选择在这里也回退，但既然用户写了 @fn，报错更负责）
            return syn::Error::new_spanned(
                final_raw_input,
                "@fn optimization currently supports up to 3 signals.",
            )
            .to_compile_error()
            .into();
        }
    } else if is_stored {
        quote! {
            {
                #(#capture_stmts)*
                #prefix::Rx::effect(std::rc::Rc::new(#f_expr) as std::rc::Rc<dyn Fn(_)>)
            }
        }
    } else if pairs.is_empty() && matches!(expr, syn::Expr::Lit(_)) {
        // 只有纯字面量才视为常量。
        // 如果是代码块或方法调用，即便没有依赖也应视为派生，以支持延迟执行和副作用。
        quote! { #prefix::Rx::<_, #prefix::RxValueKind>::new_constant(#expr) }
    } else {
        quote! {
            {
                #(#capture_stmts)*
                #prefix::Rx::derive(Box::new(#f_expr))
            }
        }
    };

    output.into()
}
