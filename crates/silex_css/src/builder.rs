use crate::{
    for_all_properties, inject_style,
    source::{CssSource, IntoCssSource},
    types::{
        ValidFor, props,
        props::{
            MarginBottom, MarginLeft, MarginRight, MarginTop, PaddingBottom, PaddingLeft,
            PaddingRight, PaddingTop,
        },
    },
};
use silex_core::{
    EffectPhase, ErrorReporter, Rx, RxGet, SilexContextProvider, SilexError, SilexErrorKind,
    SilexResult,
};
use silex_dom::DomElement;
use silex_hash::{
    css::{CssHasher, Normalized, encode_base36},
    css_hasher,
};
use silex_view::MountContext as ViewMountContext;
use silex_view::attribute::{
    ApplyTarget as ViewApplyTarget, ApplyToDom as ViewApplyToDom, IntoStorable as ViewIntoStorable,
    ReactiveBinding as ViewReactiveBinding, ReactiveBindingContext as ViewReactiveBindingContext,
    ReactiveBindingPlan as ViewReactiveBindingPlan,
};
use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::{Hash, Hasher},
};

/// 属性名是 `Cow`：注册表里的是 `&'static str` 常量，`var()` / `raw()` 写进来的
/// 则来自调用方，写进 CSS 前要先过 `escape::property_name`。
pub(crate) type PropName = Cow<'static, str>;
pub(crate) type StaticRule = (PropName, Cow<'static, str>);
pub(crate) type DynamicRule<'scope> = (PropName, Rx<'scope, String>);

#[derive(Clone)]
enum StyleValue<'scope> {
    Static(Cow<'static, str>),
    Dynamic(Rx<'scope, String>),
}

impl<'scope> StyleValue<'scope> {
    fn from_source<S>(value: S, error_handler: ErrorReporter<'scope>) -> SilexResult<Self>
    where
        S: IntoCssSource<'scope>,
        S::Value: Display + Clone + 'scope,
    {
        match value.into_css_source() {
            CssSource::Static(value) => Ok(Self::Static(Cow::Owned(value.to_string()))),
            CssSource::Reactive(source) => Ok(Self::Dynamic(
                source.map(|value| value.to_string(), error_handler)?,
            )),
        }
    }
}

#[derive(Clone)]
pub(crate) enum NestedRule<'scope> {
    Media(&'static str, Style<'scope>),
    /// CSS Nesting 语义：含 `&` 则替换，不含则补后代关系
    Selector(&'static str, Style<'scope>),
    /// 直接附着在基类后面（`:hover` → `.cls:hover`），由 `pseudo()` 一族产生
    Attached(&'static str, Style<'scope>),
}

#[derive(Clone)]
pub struct Style<'scope> {
    pub(crate) static_rules: Vec<StaticRule>,
    pub(crate) dynamic_rules: Vec<DynamicRule<'scope>>,
    pub(crate) nested_rules: Vec<NestedRule<'scope>>,
    error_handler: ErrorReporter<'scope>,
}

impl<'scope> Style<'scope> {
    pub fn new<C>(ctx: C) -> Self
    where
        C: SilexContextProvider<'scope>,
    {
        Self {
            static_rules: Vec::new(),
            dynamic_rules: Vec::new(),
            nested_rules: Vec::new(),
            error_handler: ctx.error_reporter(),
        }
    }

    fn nested_style(&self) -> Self {
        Self {
            static_rules: Vec::new(),
            dynamic_rules: Vec::new(),
            nested_rules: Vec::new(),
            error_handler: self.error_handler,
        }
    }

    /// 定义媒体查询，例如 `.media("@media (max-width: 600px)", |s| s.width(PX(100)))`
    pub fn media<F>(mut self, query: &'static str, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.nested_rules
            .push(NestedRule::Media(query, f(self.nested_style())?));
        Ok(self)
    }

    /// 定义嵌套选择器，语义与 CSS Nesting 一致。
    ///
    /// - 含 `&`：`&` 被替换成本样式的类选择器。`.nest("& > div", …)` → `.cls > div`。
    /// - 不含 `&`：按 CSS Nesting 的规定补一个**后代**关系。
    ///   `.nest(":hover", …)` → `.cls :hover`。
    ///
    /// 这一条此前是反的：无 `&` 时直接拼接，`.nest(":hover")` 得到 `.cls:hover`
    /// （元素自身的 hover），而 `css!` 里裸写 `:hover { … }` 走的是 CSS Nesting
    /// 语义，得到 `.cls :hover`（后代的 hover）。**同一个字符串，builder 当伪类、
    /// 宏当后代选择器**，两边匹配的是完全不同的元素集。
    ///
    /// 想要「贴在自身上」的伪类请用 [`Style::pseudo`] 或 `on_hover()` 一族。
    pub fn nest<F>(mut self, selector: &'static str, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.nested_rules
            .push(NestedRule::Selector(selector, f(self.nested_style())?));
        Ok(self)
    }

    /// 把一个伪类/伪元素**贴在本样式自身**上：`.pseudo(":hover", …)` → `.cls:hover`。
    ///
    /// 等价于 `nest("&:hover", …)`。与 [`Style::nest`] 的区别就是那个隐含的 `&`
    /// ——`nest` 按 CSS Nesting 补的是后代关系。
    pub fn pseudo<F>(self, selector: &'static str, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(selector, f)
    }

    /// `pseudo` 的内部实现：把 `sel` 当作直接附着在基类后面的片段。
    fn attached<F>(mut self, selector: &'static str, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.nested_rules
            .push(NestedRule::Attached(selector, f(self.nested_style())?));
        Ok(self)
    }

    pub fn on_hover<F>(self, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(":hover", f)
    }

    pub fn on_active<F>(self, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(":active", f)
    }

    pub fn on_focus<F>(self, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(":focus", f)
    }

    pub fn on_focus_visible<F>(self, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(":focus-visible", f)
    }

    pub fn on_disabled<F>(self, f: F) -> SilexResult<Self>
    where
        F: FnOnce(Style<'scope>) -> SilexResult<Style<'scope>>,
    {
        self.attached(":disabled", f)
    }

    pub fn margin_x<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope> + Clone,
        V::Value: ValidFor<MarginLeft> + ValidFor<MarginRight> + Display + Clone + 'scope,
    {
        self.margin_left(value.clone())?.margin_right(value)
    }

    pub fn margin_y<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope> + Clone,
        V::Value: ValidFor<MarginTop> + ValidFor<MarginBottom> + Display + Clone + 'scope,
    {
        self.margin_top(value.clone())?.margin_bottom(value)
    }

    pub fn padding_x<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope> + Clone,
        V::Value: ValidFor<PaddingLeft> + ValidFor<PaddingRight> + Display + Clone + 'scope,
    {
        self.padding_left(value.clone())?.padding_right(value)
    }

    pub fn padding_y<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope> + Clone,
        V::Value: ValidFor<PaddingTop> + ValidFor<PaddingBottom> + Display + Clone + 'scope,
    {
        self.padding_top(value.clone())?.padding_bottom(value)
    }

    /// 写一个自定义属性（CSS 变量）：`.var("--brand", hex("#09f"))`。
    ///
    /// 名字不带 `--` 时会自动补上。整个主题系统都建立在 CSS 变量之上，而
    /// `sty()` 此前**根本写不出** `--my-var: red`——`generate_builder_methods`
    /// 只覆盖 `for_all_properties!` 注册表，自定义属性不在里面。想设一个变量
    /// 就只能退回 `styled!` 宏。
    ///
    /// 自定义属性按规范接受任意 token 序列，所以这里不做值类型校验；值仍然会
    /// 过一遍声明边界净化。
    pub fn var<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope>,
        V::Value: Display + Clone + 'scope,
    {
        let name = name.into();
        debug_assert!(!name.is_empty(), "自定义属性名不能为空");
        let name = if name.starts_with("--") {
            name
        } else {
            Cow::Owned(format!("--{}", name.trim_start_matches('-')))
        };
        let style_value = StyleValue::from_source(value, self.error_handler)?;
        self.add_rule(name, style_value)
    }

    /// 逃生舱：写一条**不经类型系统**的声明。
    ///
    /// 留给注册表覆盖不到的属性——MDN 数据里根本没有的厂商前缀属性
    /// （`-webkit-font-smoothing`、`-moz-osx-font-smoothing`、
    /// `-webkit-backdrop-filter` 都属于这一类），以及规范刚落地、数据还没跟上的
    /// 新属性。此前完全没有这条路，只能退回 `styled!` 宏。
    ///
    /// 属性名与值都会过净化，写不出越界的声明；但**语义正确与否由调用方负责**。
    pub fn raw<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> SilexResult<Self>
    where
        V: IntoCssSource<'scope>,
        V::Value: Display + Clone + 'scope,
    {
        let name = name.into();
        debug_assert!(!name.is_empty(), "属性名不能为空");
        let style_value = StyleValue::from_source(value, self.error_handler)?;
        self.add_rule(name, style_value)
    }

    fn add_rule(mut self, prop: PropName, value: StyleValue<'scope>) -> SilexResult<Self> {
        match value {
            StyleValue::Static(val_str) => {
                self.static_rules.push((prop, val_str));
            }
            StyleValue::Dynamic(getter) => {
                self.dynamic_rules.push((prop, getter));
            }
        }
        Ok(self)
    }
}

pub fn sty<'scope, C>(ctx: C) -> Style<'scope>
where
    C: SilexContextProvider<'scope>,
{
    Style::new(ctx)
}

macro_rules! generate_builder_methods {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, [$($cap:ident)*]) ),*) => {
        impl<'scope> Style<'scope> {
            $(
                pub fn $snake<V>(self, value: V) -> SilexResult<Self>
                where
                    V: IntoCssSource<'scope>,
                    V::Value: ValidFor<props::$pascal> + Display + Clone + 'scope,
                {
                    let style_value = StyleValue::from_source(value, self.error_handler)?;
                    self.add_rule(
                        ::std::borrow::Cow::Borrowed($kebab),
                        style_value,
                    )
                }
            )*
        }
    };
}

for_all_properties!(generate_builder_methods);

/// `Style` 编译出的产物：类名、要注入的 CSS、以及待建立的动态绑定。
///
/// 单独拆出来是为了让「生成什么 CSS」这件事能脱离 DOM 被断言——`@layer` 的
/// 归属、嵌套选择器的展开此前都只能靠读代码确认。
pub(crate) struct RenderedStyle<'scope> {
    pub class_base: String,
    pub css: String,
    pub dyn_bindings: Vec<(String, Rx<'scope, String>)>,
}

impl<'scope> Style<'scope> {
    /// 只生成，不碰 DOM。
    pub(crate) fn render(&self) -> RenderedStyle<'scope> {
        // 1. 生成稳定哈希（忽略动态值，递归所有嵌套规则）
        let mut hasher = css_hasher!();
        hash_recursive(self, &mut hasher);
        let hash_val = hasher.finish();
        let mut hash_buf = [0u8; 13];
        let hash_str = encode_base36(hash_val, &mut hash_buf);
        let class_base = format!("slx-{}", hash_str);

        // 2. 递归构造 CSS，收集所有动态绑定
        let mut css_str = String::new();
        let mut dyn_bindings = Vec::new();
        let base_sel = format!(".{}", class_base);

        generate_css_recursive(self, &base_sel, hash_str, &mut css_str, &mut dyn_bindings);

        // 3. 归入 `overrides` 层。`sty()` 是针对单个元素实例的就地覆盖，
        //    优先级理应最高；此前它**不带任何 layer**，靠「无层规则压过所有
        //    具名层」这条规范侧效达到同样效果——顺带也压过了 `global!`，
        //    而两者之间只能靠注入先后决定胜负。
        let css = if css_str.trim().is_empty() {
            String::new()
        } else {
            crate::layers::wrap(crate::layers::OVERRIDES, &css_str)
        };

        RenderedStyle {
            class_base,
            css,
            dyn_bindings,
        }
    }
}

/// 递归计算样式的稳定哈希
fn hash_recursive(style: &Style<'_>, hasher: &mut CssHasher) {
    for (k, v) in &style.static_rules {
        Normalized(k).hash(hasher);
        Normalized(v).hash(hasher);
    }
    for (prop, _) in &style.dynamic_rules {
        Normalized(prop).hash(hasher);
        "dyn-val".hash(hasher); // 动态值占位
    }
    for rule in &style.nested_rules {
        match rule {
            NestedRule::Media(query, sub) => {
                "media".hash(hasher);
                Normalized(query).hash(hasher);
                hash_recursive(sub, hasher);
            }
            NestedRule::Selector(selector, sub) => {
                "selector".hash(hasher);
                Normalized(selector).hash(hasher);
                hash_recursive(sub, hasher);
            }
            NestedRule::Attached(selector, sub) => {
                // 与 `Selector` 用不同的标签：同一个 `":hover"` 在两条路下
                // 展开成不同的选择器，类名必须跟着分叉
                "attached".hash(hasher);
                Normalized(selector).hash(hasher);
                hash_recursive(sub, hasher);
            }
        }
    }
}

/// 递归生成 CSS 字符串并收集动态绑定
fn generate_css_recursive<'scope>(
    style: &Style<'scope>,
    base_selector: &str,
    hash_str: &str,
    css_out: &mut String,
    dyn_bindings: &mut Vec<(String, Rx<'scope, String>)>,
) {
    // 写入当前层级的规则
    if !style.static_rules.is_empty() || !style.dynamic_rules.is_empty() {
        css_out.push_str(base_selector);
        css_out.push_str(" {\n");
        for (k, v) in &style.static_rules {
            // 属性名与值都可能来自调用方（`var()` / `raw()`）：都要挡在声明
            // 边界内，否则一个 `;` 就能注入新规则
            let _ = writeln!(
                css_out,
                "  {}: {};",
                crate::escape::property_name(k),
                crate::escape::declaration_value(v)
            );
        }
        for (prop, getter) in &style.dynamic_rules {
            let var_name = format!("--sb-{}-{}", hash_str, dyn_bindings.len());
            let _ = writeln!(
                css_out,
                "  {}: var({});",
                crate::escape::property_name(prop),
                var_name
            );
            dyn_bindings.push((var_name, *getter));
        }
        css_out.push_str("}\n");
    }

    // 处理嵌套规则
    for rule in &style.nested_rules {
        match rule {
            NestedRule::Media(query, sub) => {
                css_out.push_str(query);
                css_out.push_str(" {\n");
                generate_css_recursive(sub, base_selector, hash_str, css_out, dyn_bindings);
                css_out.push_str("}\n");
            }
            // CSS Nesting 语义：含 `&` 则替换，不含则补一个后代关系。
            // 这与 `css!` 里裸写 `:hover { … }` 的展开结果一致。
            NestedRule::Selector(selector, sub) => {
                let full_selector = if selector.contains('&') {
                    selector.replace('&', base_selector)
                } else {
                    format!("{} {}", base_selector, selector)
                };
                generate_css_recursive(sub, &full_selector, hash_str, css_out, dyn_bindings);
            }
            // `pseudo()` / `on_hover()` 一族：直接贴在基类后面
            NestedRule::Attached(selector, sub) => {
                let full_selector = format!("{}{}", base_selector, selector);
                generate_css_recursive(sub, &full_selector, hash_str, css_out, dyn_bindings);
            }
        }
    }
}

impl<'scope> ViewApplyToDom<'scope> for Style<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        _target: ViewApplyTarget,
        context: &ViewMountContext<'scope>,
    ) -> SilexResult<()> {
        let RenderedStyle {
            class_base,
            css,
            dyn_bindings,
        } = self.render();
        if !css.is_empty() {
            inject_style(&class_base, &css);
        }
        context
            .dom()
            .set_attribute(silex_dom::attribute::AttributeRequest::new(
                element,
                silex_dom::attribute::AttributeTarget::Class,
                silex_dom::attribute::AttributeValue::ClassTokens {
                    add: vec![class_base.clone()],
                    remove: Vec::new(),
                },
            ))
            .map_err(|error| SilexError::fatal(SilexErrorKind::Dom(error.to_string())))?;

        let property_names: Vec<String> =
            dyn_bindings.iter().map(|(name, _)| name.clone()).collect();
        if !dyn_bindings.is_empty() {
            let dom = context.dom().clone();
            let element_for_effect = element.clone();
            context.owner().effect_with_previous(
                EffectPhase::Normal,
                Box::new(move |previous: Option<&Vec<Option<String>>>| {
                    let values: Vec<String> = dyn_bindings
                        .iter()
                        .map(|(_, source)| source.get())
                        .collect::<SilexResult<_>>()?;
                    for (index, ((name, _), value)) in
                        dyn_bindings.iter().zip(values.iter()).enumerate()
                    {
                        let old_value = previous
                            .and_then(|values| values.get(index))
                            .and_then(Option::as_deref);
                        if old_value != Some(value.as_str()) {
                            dom.set_style_property(&element_for_effect, name, Some(value))
                                .map_err(|error| {
                                    SilexError::fatal(SilexErrorKind::Dom(error.to_string()))
                                })?;
                        }
                    }
                    Ok(values.into_iter().map(Some).collect())
                }),
                context.error_handler(),
            )?;
        }

        let dom = context.dom().clone();
        let element_for_cleanup = element.clone();
        let class_name = class_base;
        context.owner().on_cleanup(
            Box::new(move || {
                for name in property_names {
                    dom.set_style_property(&element_for_cleanup, &name, None)
                        .map_err(|error| {
                            SilexError::fatal(SilexErrorKind::Dom(error.to_string()))
                        })?;
                }
                dom.set_attribute(silex_dom::attribute::AttributeRequest::new(
                    &element_for_cleanup,
                    silex_dom::attribute::AttributeTarget::Class,
                    silex_dom::attribute::AttributeValue::ClassTokens {
                        add: Vec::new(),
                        remove: vec![class_name],
                    },
                ))
                .map_err(|error| SilexError::fatal(SilexErrorKind::Dom(error.to_string())))
            }),
            context.error_handler(),
        )
    }
}

impl<'scope> ViewReactiveBinding<'scope> for Style<'scope> {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ViewReactiveBindingContext,
    ) -> Option<ViewReactiveBindingPlan<'scope>> {
        if !matches!(context, ViewReactiveBindingContext::Value(_)) {
            return None;
        }
        let handler = rx.owner().error_handler(|_| {}).ok()?;
        let classes = rx
            .map(
                |style| {
                    let RenderedStyle {
                        class_base, css, ..
                    } = style.render();
                    if !css.is_empty() {
                        inject_style(&class_base, &css);
                    }
                    class_base
                },
                handler.view(),
            )
            .ok()?;
        Some(ViewReactiveBindingPlan::dynamic_classes(classes))
    }
}

impl<'scope> ViewIntoStorable<'scope> for Style<'scope> {
    type Stored = Self;

    fn into_storable(self) -> Self::Stored {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers;
    use crate::types::{hex, px};
    use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, RxGet, SilexContext};

    fn discard_test_errors<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
        owner
            .error_handler(|_| {})
            .expect("test error handler should register")
    }

    fn css_of(style: SilexResult<Style<'_>>) -> String {
        style.expect("static style should build").render().css
    }

    fn with_test_ctx<F>(f: F)
    where
        F: for<'scope> FnOnce(SilexContext<'scope>),
    {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let error_handler = discard_test_errors(owner);
                f(SilexContext::new(owner, error_handler.view()));
            })
            .expect("test ctx should initialize");
    }

    fn css_of_with_ctx<F>(build: F) -> String
    where
        F: for<'scope> FnOnce(SilexContext<'scope>) -> SilexResult<Style<'scope>>,
    {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let error_handler = discard_test_errors(owner);
                css_of(build(SilexContext::new(owner, error_handler.view())))
            })
            .expect("test ctx should initialize")
    }

    /// 报告 P2-3：`sty()` 的产出此前**不带任何 layer**，靠「无层规则压过所有
    /// 具名层」的规范侧效获得最高优先级——顺带压过了 `global!`，而两者之间
    /// 只能靠注入先后决定胜负。
    #[test]
    fn sty_lands_in_the_overrides_layer() {
        let css = css_of_with_ctx(|ctx| Style::new(ctx).color(hex("#fff")));
        assert!(
            css.starts_with(&format!("@layer {} {{", layers::OVERRIDES)),
            "{css}"
        );
        assert!(css.contains("color: #fff;"), "{css}");
    }

    /// 空样式不该注入一个空的 layer 块
    #[test]
    fn an_empty_style_produces_no_css_at_all() {
        assert_eq!(css_of_with_ctx(|ctx| Ok(Style::new(ctx))), "");
    }

    /// 类名只由静态结构决定：同样的声明必须给出同一个类名，
    /// 否则每次渲染都会往静态表里塞一份新副本
    #[test]
    fn the_class_name_is_stable_across_renders() {
        with_test_ctx(|ctx| {
            let a = Style::new(ctx)
                .color(hex("#fff"))
                .expect("color should build")
                .width(px(10))
                .expect("width should build")
                .render();
            let b = Style::new(ctx)
                .color(hex("#fff"))
                .expect("color should build")
                .width(px(10))
                .expect("width should build")
                .render();
            assert_eq!(a.class_base, b.class_base);
            assert!(a.class_base.starts_with("slx-"));
        });
    }

    /// 值来自用户输入时必须挡在声明边界内（报告 P0-8 的回归）。
    ///
    /// 产物里只该有两层花括号：`@layer` 块与那条规则本身。多出来的就是被
    /// 用户字符串撑开的新规则。
    #[test]
    fn a_static_value_cannot_break_out_of_its_declaration() {
        let css = css_of_with_ctx(|ctx| {
            Style::new(ctx).grid_template_areas("red; } body { display: none")
        });
        assert_eq!(css.matches('{').count(), 2, "{css}");
        assert_eq!(css.matches('}').count(), 2, "{css}");
        assert_eq!(css.matches(';').count(), 1, "{css}");
    }

    /// 嵌套选择器：带 `&` 的替换到位
    #[test]
    fn nested_selectors_expand_against_the_base_class() {
        with_test_ctx(|ctx| {
            let rendered = Style::new(ctx)
                .nest("& > div", |s| s.color(hex("#000")))
                .expect("nested style should build")
                .render();
            let base = format!(".{}", rendered.class_base);
            assert!(
                rendered.css.contains(&format!("{base} > div {{")),
                "{}",
                rendered.css
            );
        });
    }

    /// 报告 P3-7：同一个 `":hover"`，builder 当伪类拼成 `.cls:hover`，
    /// 而 `css!` 里裸写走 CSS Nesting 语义得到 `.cls :hover`（后代）。
    /// 两边匹配的是完全不同的元素集。
    ///
    /// 现在 `nest` 统一到 CSS Nesting，「贴在自身上」交给 `pseudo` 一族。
    #[test]
    fn nest_follows_css_nesting_and_pseudo_attaches() {
        with_test_ctx(|ctx| {
            let nested = Style::new(ctx)
                .nest(":hover", |s| s.color(hex("#000")))
                .expect("nested style should build")
                .render();
            let base = format!(".{}", nested.class_base);
            assert!(
                nested.css.contains(&format!("{base} :hover {{")),
                "无 `&` 的 nest 该是后代关系：{}",
                nested.css
            );

            let attached = Style::new(ctx)
                .pseudo(":hover", |s| s.color(hex("#000")))
                .expect("pseudo style should build")
                .render();
            let base = format!(".{}", attached.class_base);
            assert!(
                attached.css.contains(&format!("{base}:hover {{")),
                "pseudo 该贴在基类上：{}",
                attached.css
            );
        });
    }

    /// 两条路展开出的选择器不同，类名必须跟着分叉——否则先注入的那份会被
    /// 另一份的类名直接命中
    #[test]
    fn nest_and_pseudo_do_not_collide_on_the_same_class_name() {
        with_test_ctx(|ctx| {
            let a = Style::new(ctx)
                .nest(":hover", |s| s.color(hex("#000")))
                .expect("nested style should build")
                .render();
            let b = Style::new(ctx)
                .pseudo(":hover", |s| s.color(hex("#000")))
                .expect("pseudo style should build")
                .render();
            assert_ne!(a.class_base, b.class_base);
        });
    }

    #[test]
    fn the_on_x_helpers_attach_to_the_base_class() {
        with_test_ctx(|ctx| {
            let rendered = Style::new(ctx)
                .on_hover(|s| s.color(hex("#000")))
                .expect("hover style should build")
                .render();
            let base = format!(".{}", rendered.class_base);
            assert!(
                rendered.css.contains(&format!("{base}:hover {{")),
                "{}",
                rendered.css
            );
        });
    }

    /// 报告 P3-4：`sty()` 写不出 `--my-var: red`，而整个主题系统都建立在
    /// CSS 变量之上
    #[test]
    fn a_custom_property_can_be_written_from_the_builder() {
        let css = css_of_with_ctx(|ctx| Style::new(ctx).var("--brand", hex("#09f")));
        assert!(css.contains("--brand: #09f;"), "{css}");
        // 不带 `--` 时自动补上
        let css = css_of_with_ctx(|ctx| Style::new(ctx).var("brand", hex("#09f")));
        assert!(css.contains("--brand: #09f;"), "{css}");
    }

    /// 报告 P3-4：没有任何 `raw(name, value)` 逃生舱，注册表覆盖不到的属性
    /// （`-webkit-font-smoothing` 根本不在 MDN 数据里）只能退回 `styled!`
    #[test]
    fn raw_reaches_properties_the_registry_does_not_cover() {
        let css =
            css_of_with_ctx(|ctx| Style::new(ctx).raw("-webkit-font-smoothing", "antialiased"));
        assert!(
            css.contains("-webkit-font-smoothing: antialiased;"),
            "{css}"
        );
    }

    /// 属性名也可能来自调用方：一个 `:` 就能把一条声明劈成两条
    #[test]
    fn a_raw_property_name_cannot_open_a_second_declaration() {
        let css = css_of_with_ctx(|ctx| Style::new(ctx).raw("color: red; background", "blue"));
        // 只该有一条声明——`@layer` 块 + 规则块 = 2 个花括号，1 个分号
        assert_eq!(css.matches(';').count(), 1, "{css}");
        assert!(!css.contains("color: red"), "{css}");
    }

    /// 自定义属性同样走动态路径
    #[test]
    fn a_custom_property_can_be_reactive() {
        let mut runtime = silex_core::Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(px(1)).expect("signal should initialize");
                let error_handler = discard_test_errors(owner);
                let rendered = Style::new(SilexContext::new(owner, error_handler.view()))
                    .var("--gap", signal)
                    .expect("reactive style should build")
                    .render();
                assert_eq!(rendered.dyn_bindings.len(), 1);
                let var_name = &rendered.dyn_bindings[0].0;
                assert!(
                    rendered.css.contains(&format!("--gap: var({var_name});")),
                    "{}",
                    rendered.css
                );
            })
            .expect("child scope should initialize");
    }

    /// View attribute pipeline 每次应用都为每个动态绑定新建一个 `Effect`，而
    /// 响应式绑定计划会在一个外层 `Effect` 里反复调用它。
    /// 这里成立的前提是：内层 `Effect` 是外层的子节点，外层重跑时随之回收。
    ///
    /// 这条不变量在 `silex_reactivity` 里（`run_effect` → `run_cleanups` →
    /// `dispose_node_internal(child)`），但 `builder.rs` 依赖它，所以在这里
    /// 钉一根桩：哪天所有权模型变了，先坏在这儿而不是坏成线上的 Effect 泄漏。
    #[test]
    fn inner_effects_are_reclaimed_when_the_outer_effect_reruns() {
        use silex_core::Runtime;
        use std::{cell::Cell, rc::Rc};

        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let outer = owner.signal(0).expect("outer signal should initialize");
                let inner_dep = owner.signal(0).expect("inner signal should initialize");
                let inner_runs = Rc::new(Cell::new(0));

                let counter = inner_runs.clone();
                owner
                    .effect(
                        EffectPhase::Normal,
                        move || -> SilexResult<()> {
                            outer.get()?;
                            let counter = counter.clone();
                            owner.effect(
                                EffectPhase::Normal,
                                move || -> SilexResult<()> {
                                    inner_dep.get()?;
                                    counter.set(counter.get() + 1);
                                    Ok(())
                                },
                                discard_test_errors(owner),
                            )?;
                            Ok(())
                        },
                        discard_test_errors(owner),
                    )
                    .expect("nested effects can be registered");

                assert_eq!(inner_runs.get(), 1, "首轮内层 Effect 跑一次");
                outer.set(1).expect("outer signal should update");
                assert_eq!(inner_runs.get(), 2, "外层重跑，新内层 Effect 跑一次");
                inner_dep.set(1).expect("inner signal should update");
                assert_eq!(
                    inner_runs.get(),
                    3,
                    "只有存活的那个内层 Effect 响应；上一轮的若没回收会多跑一次"
                );
            })
            .expect("child scope should initialize");
    }

    /// 动态值走行内 CSS 变量，规则里只留一个 `var()` 引用
    #[test]
    fn dynamic_values_become_a_css_variable_reference() {
        let mut runtime = silex_core::Runtime::new();
        runtime
            .with_transient(|owner| {
                let signal = owner.signal(px(1)).expect("signal should initialize");
                let error_handler = discard_test_errors(owner);
                let rendered = Style::new(SilexContext::new(owner, error_handler.view()))
                    .width(signal)
                    .expect("reactive style should build")
                    .render();
                assert_eq!(rendered.dyn_bindings.len(), 1);
                let var_name = &rendered.dyn_bindings[0].0;
                assert!(var_name.starts_with("--sb-"), "{var_name}");
                assert!(rendered.css.contains(&format!("width: var({var_name});")));
            })
            .expect("child scope should initialize");
    }
}
