//! 宏展开里指代 `silex` 根 crate 的路径。
//!
//! 报告 §3.4：此前所有展开都硬编码 `::silex::core::rx!`、`::silex::css::cx!` 这类
//! **绝对路径**，代价有三：
//!
//! - `silex` 自身的 crate 内部无法使用这些宏（`::silex` 在它自己里面不存在）；
//! - 用户 `my_silex = { package = "silex" }` 重命名依赖后，展开出来的路径找不到；
//! - 没有自定义 re-export 根的余地。
//!
//! 现在统一从调用方的 `Cargo.toml` 解析真实名字（`proc-macro-crate`），
//! 宏模板一律写 `#silex::…` 而不是 `::silex::…`。

use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::sync::OnceLock;
use syn::Ident;

/// 解析结果按进程缓存。
///
/// `crate_name` 每次都要读一遍调用方的 `Cargo.toml`，而一个 rustc 进程只编译一个 crate，
/// `CARGO_MANIFEST_DIR` 在整个进程生命周期内不变，因此结果可以安全复用。
static RESOLVED: OnceLock<Option<String>> = OnceLock::new();
#[cfg(feature = "store")]
static RESOLVED_CORE: OnceLock<Option<String>> = OnceLock::new();

/// 展开中引用 `silex` 根 crate 的路径。
///
/// - 依赖里能找到（含 `package = "silex"` 的重命名）→ `::<真实名字>`
/// - 调用方就是 `silex` 自己 → `crate`
/// - 解析失败（例如 doctest 环境没有 `CARGO_MANIFEST_DIR`）→ 退回 `::silex`
pub fn silex() -> TokenStream {
    let resolved = RESOLVED.get_or_init(|| match proc_macro_crate::crate_name("silex") {
        Ok(proc_macro_crate::FoundCrate::Itself) => None,
        Ok(proc_macro_crate::FoundCrate::Name(name)) => Some(name),
        Err(_) => Some("silex".to_string()),
    });

    match resolved {
        Some(name) => {
            let ident = Ident::new(name, Span::call_site());
            quote!(::#ident)
        }
        None => quote!(crate),
    }
}

/// 展开中引用 `silex_core` crate 的路径。
#[cfg(feature = "store")]
pub fn silex_core() -> TokenStream {
    let resolved = RESOLVED_CORE.get_or_init(|| match proc_macro_crate::crate_name("silex_core") {
        Ok(proc_macro_crate::FoundCrate::Itself) => None,
        Ok(proc_macro_crate::FoundCrate::Name(name)) => Some(name),
        Err(_) => match proc_macro_crate::crate_name("silex") {
            Ok(proc_macro_crate::FoundCrate::Itself) => Some("silex::core".to_string()),
            Ok(proc_macro_crate::FoundCrate::Name(name)) => Some(format!("{}::core", name)),
            Err(_) => Some("silex_core".to_string()),
        },
    });

    match resolved {
        Some(name) => {
            let path: syn::Path = syn::parse_str(&format!("::{}", name))
                .unwrap_or_else(|_| syn::parse_str("::silex_core").unwrap());
            quote!(#path)
        }
        None => quote!(crate),
    }
}
