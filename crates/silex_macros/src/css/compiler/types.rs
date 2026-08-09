use proc_macro2::{Span, TokenStream};
use std::rc::Rc;

/// 级联层的名字。
///
/// 这四个常量必须与 `silex_css::layers` 里的同名常量保持一致——proc-macro
/// crate 不能依赖运行时 crate，只能各写一份。层序的完整说明在
/// `silex_css/src/layers.rs`。
pub(crate) const LAYER_BASE: &str = "base";
pub(crate) const LAYER_COMPONENTS: &str = "components";
pub(crate) const LAYER_UTILITIES: &str = "utilities";

#[derive(Debug, Clone)]
pub struct DynamicRule {
    /// 结构化模板：`\u{1}` 是组件类名占位，`\u{2}` 是第 n 个运行时取值占位
    /// （按出现顺序）。见 [`template_parts`]。
    pub template: String,
    pub expressions: Vec<(String, TokenStream)>,
}

/// 动态模板里的占位符。
///
/// 用控制字符而不是 `{}` / 类名文本，是为了让「哪里要填东西」这件事在编译期
/// 就确定下来：运行时只做拼接，不再做模式匹配，也就不会误伤 `.foo-bar` 这种
/// 以基类名开头的选择器，或值内容里恰好出现的 `{}`。
/// [`escape_css_string`] 会把用户字符串里的控制字符转义掉，所以这两个字符
/// 不可能来自源码。
pub(crate) const PLACEHOLDER_CLASS: char = '\u{1}';
pub(crate) const PLACEHOLDER_VALUE: char = '\u{2}';
/// 选择器片段专用占位符。它不能与声明值共用，否则运行时无法选择正确的
/// CSS 转义上下文。
pub(crate) const PLACEHOLDER_SELECTOR_VALUE: char = '\u{4}';
/// 构造阶段静态声明值专用占位符。它与响应式值分开，运行时只进行一次声明值
/// 转义，不会把静态值误当成 CSS 变量或 reactive getter。
pub(crate) const PLACEHOLDER_STATIC_VALUE: char = '\u{5}';
pub(crate) const PLACEHOLDER_STATIC_END: char = '\u{6}';

/// 类名在编译期的占位。
///
/// 类名要写进产物（`.slx-xxx { … }`、`var(--slx-xxx-0)`），而产物又要用来算类名，
/// 这是个环。解法是先用这个占位符跑完整个生成过程，拿产物取哈希得到真正的类名，
/// 再把占位符逐字换回去。见 [`CssCompiler::compile_block_internal`]。
///
/// 和上面两个占位符同理，用控制字符是为了让它不可能来自源码——[`escape_css_string`]
/// 会把用户字符串里的控制字符转义掉。
pub(crate) const PLACEHOLDER_PENDING_CLASS: &str = "\u{3}";

/// 模板的一个片段，与 `silex_css::runtime::template::CssPart` 一一对应。
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    Lit(String),
    Class,
    Val(usize),
    SelectorVal(usize),
    StaticVal(usize),
}

/// 把带占位符的模板切成片段。
pub fn template_parts(template: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut next_val = 0;
    let mut next_static = 0;
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            PLACEHOLDER_CLASS => {
                if !lit.is_empty() {
                    parts.push(TemplatePart::Lit(std::mem::take(&mut lit)));
                }
                parts.push(TemplatePart::Class);
            }
            PLACEHOLDER_VALUE | PLACEHOLDER_SELECTOR_VALUE => {
                if !lit.is_empty() {
                    parts.push(TemplatePart::Lit(std::mem::take(&mut lit)));
                }
                if ch == PLACEHOLDER_SELECTOR_VALUE {
                    parts.push(TemplatePart::SelectorVal(next_val));
                } else {
                    parts.push(TemplatePart::Val(next_val));
                }
                next_val += 1;
            }
            PLACEHOLDER_STATIC_VALUE => {
                if !lit.is_empty() {
                    parts.push(TemplatePart::Lit(std::mem::take(&mut lit)));
                }
                let mut digits = String::new();
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    if let Some(digit) = chars.next() {
                        digits.push(digit);
                    }
                }
                let index = if chars.peek() == Some(&PLACEHOLDER_STATIC_END) {
                    chars.next();
                    digits.parse().unwrap_or(next_static)
                } else {
                    next_static
                };
                parts.push(TemplatePart::StaticVal(index));
                next_static += 1;
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        parts.push(TemplatePart::Lit(lit));
    }
    parts
}

/// 把片段展开成 `&'static [silex::css::CssPart]`。
pub fn template_parts_tokens(template: &str) -> TokenStream {
    let __silex = crate::crate_path::silex();
    let items = template_parts(template).into_iter().map(|p| match p {
        TemplatePart::Lit(s) => quote::quote! { #__silex::css::CssPart::Lit(#s) },
        TemplatePart::Class => quote::quote! { #__silex::css::CssPart::Class },
        TemplatePart::Val(i) => quote::quote! { #__silex::css::CssPart::Val(#i) },
        TemplatePart::SelectorVal(i) => {
            quote::quote! { #__silex::css::CssPart::SelectorVal(#i) }
        }
        TemplatePart::StaticVal(i) => quote::quote! { #__silex::css::CssPart::StaticVal(#i) },
    });
    quote::quote! { &[ #(#items),* ] }
}

#[derive(Debug, Clone)]
pub struct CssWarning {
    pub message: String,
    pub span: Span,
}

/// 一条静态声明的编译期类型断言。
///
/// `css!{ color: 10px }` 这样的写法此前完全不经过类型系统——`ValidFor` 只在
/// `$expr` 分支起作用，静态声明是纯字符串拼接。这里把「取值一眼能定型」的
/// 静态声明也接回类型系统。
#[derive(Debug, Clone)]
pub struct StaticAssertion {
    /// CSS 属性名（kebab-case 或短别名）
    pub property: String,
    /// 取值对应的 CSS 值类型名（`silex::css::types::` 下的类型）
    pub value_type: &'static str,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub static_id: String,
    /// This is also the layer used by dynamic rules from this compilation.
    pub layer: &'static str,
    pub static_css: String,    // Fully static CSS (font-face, etc.)
    pub component_css: String, // CSS scoped to this component (with dynamic vars)
    pub expressions: Vec<(String, TokenStream)>,
    /// 构造阶段渲染的静态表达式。它们不能与 reactive expressions 共用哈希规则：
    /// 同一模板使用不同静态路径时，最终 CSS 的身份也不同。
    pub static_expressions: Vec<(String, TokenStream)>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub warnings: Vec<CssWarning>,
    pub assertions: Vec<StaticAssertion>,
}

impl CssCompileResult {
    /// Generates TokenStream for injecting static and component CSS styles
    pub fn generate_inits(&self) -> TokenStream {
        let __silex = crate::crate_path::silex();
        let static_id = &self.static_id;
        let static_css = &self.static_css;
        let style_id = &self.style_id;
        let component_css = &self.component_css;

        quote::quote! {
            if !#static_css.is_empty() {
                #__silex::css::inject_style(#static_id, #static_css);
            }
            if !#component_css.is_empty() {
                #__silex::css::inject_style(#style_id, #component_css);
            }
        }
    }
}

pub(crate) struct ParserState {
    pub static_css: String,
    pub lifted_css: String,
    pub expressions: Vec<(String, TokenStream)>,
    pub static_expressions: Vec<(String, TokenStream)>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub warnings: Vec<CssWarning>,
    pub assertions: Vec<StaticAssertion>,
    pub class_name: String,
    pub is_unsafe: bool,
    /// 是否校验属性名与静态取值。`@apply` 展开出来的声明是机器生成的
    /// （含 `--tw-*` 与厂商前缀），不走这套判据。
    pub validate: bool,
    /// 整个宏调用的源码，用于恢复 token 之间的空白（见 [`crate::css::spacing`]）
    pub region: Option<Rc<str>>,
}

#[derive(Clone)]
pub(crate) struct DynamicContext<'a> {
    pub class_name: &'a str,
    pub is_unsafe: bool,
    /// 是否校验动态规则体内用户书写的声明。`@apply` 会在递归时关闭它。
    pub validate: bool,
    pub region: Option<Rc<str>>,
}

/// `compile_block_internal` 的入参。
///
/// 这些开关组合起来决定「谁在编译、编译给谁用」，散成一长串位置参数极易接错。
pub(crate) struct CompileOptions<'a> {
    pub span: Span,
    /// 是否把产物包进 `.class { }`（`global!` 不包）
    pub wrap_in_class: bool,
    pub is_unsafe: bool,
    pub prefix: &'a str,
    /// 宏调用点的源码，用于恢复 token 之间的空白
    pub region: Option<Rc<str>>,
    /// 是否校验属性名与静态取值（机器生成的 CSS 不校验）
    pub validate: bool,
}
