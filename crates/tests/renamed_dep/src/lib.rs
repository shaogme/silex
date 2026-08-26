//! 见 `Cargo.toml`：本 crate 以 `my_silex` 的名字依赖 `silex`，
//! 编译通过即证明宏展开出的路径是按调用方实际依赖名解析的，而不是写死的 `::silex::`。
//!
//! 覆盖面按"展开里出现过绝对路径"来选，每个宏至少踩一次。

use my_silex::prelude::*;

my_silex_view::define_tag!(
    RenamedDirectTag,
    "div",
    html,
    renamed_direct_tag,
    non_void,
    []
);

pub fn direct_view_abi<'scope>() -> my_silex_view::elements::AnyView<'scope> {
    let _attributes: my_silex_view::attributes::AttributeGroup<'scope> =
        my_silex_view::group![("data-origin", "renamed-view")];
    let _children = my_silex_view::chain!("first", "second");
    my_silex_view::view_match!(true, {
        true => renamed_direct_tag("content"),
        false => "fallback",
    })
}

router! {
    enum RenamedRoute {
        User { id: u32 } => "/users/:id",
    }
}

/// `tw!` 条件分支消费调用方已经创建的 scoped source，`css!` 保持静态路径。
pub fn badge_class<'scope>(wide: Rx<'scope, bool>) -> AttrOp<'scope> {
    tw!(
        "inline-flex items-center px-2 py-1 rounded-sm",
        (wide, "w-full", "w-auto")
    )
}

pub fn badge_style() -> &'static str {
    css! { line-height: 1.25; }
}

pub fn static_classes() -> AttributeGroup<'static> {
    classes!["inline-flex", "items-center"]
}

/// `tw_variants!` 表达式形式：展开出 `declare_variants!` 与每个选项的 `tw!`
pub fn button_class(size: &str) -> String {
    let styles = tw_variants! {
        base: "inline-flex items-center justify-center rounded-md",
        variants: {
            size: {
                sm: "h-8 px-3 text-sm",
                md: "h-9 px-4 text-sm",
                lg: "h-10 px-6 text-base",
            }
        },
        default_variants: { size: md }
    };
    styles.get(size)
}

// `tw_variants!` item 形式：类型定义落在本模块，可命名、可放进签名与结构体字段
tw_variants! {
    pub struct CardStyle {
        base: "rounded-lg border",
        variants: {
            tone: {
                muted: "bg-muted text-muted-foreground",
                accent: "bg-accent text-accent-foreground",
            },
            size: {
                sm: "p-2 text-sm",
                "icon-lg": "p-6 text-base",
            }
        },
        default_variants: { tone: "muted", size: "sm" },
        compound_variants: [
            { tone: "accent", size: "icon-lg", class: "shadow-lg" }
        ]
    }
}

tw_variants! {
    pub struct NumericStyle {
        base: "box-border",
        variants: {
            size: {
                "1x": "p-1",
                sm: "p-2",
            }
        },
        default_variants: { size: "1x" }
    }
}

/// 类型可命名才写得出这样的签名——这正是 item 形式存在的理由
pub fn card_class(style: &CardStyle) -> String {
    style.class()
}

/// 也能放进结构体字段
pub struct CardProps {
    pub style: CardStyle,
}

pub fn typed_card() -> String {
    let props = CardProps {
        style: CardStyle::new()
            .with_tone(CardStyleTone::Accent)
            .with_size(CardStyleSize::IconLg),
    };
    card_class(&props.style)
}

/// 严格解析：写错的选项名不会静默套用默认样式
pub fn card_class_from_str(size: &str) -> Result<String, String> {
    CardStyle::new()
        .get_checked("accent", size)
        .map_err(|e| e.to_string())
}

pub fn renamed_route_path() -> Result<RoutePath, String> {
    RenamedRoute::User { id: 42 }
        .path()
        .map_err(|error| error.to_string())
}

// `styled!`：展开出 `#[component]`、`inject_style`、`TypedElement` 等一大片绝对路径
styled! {
    pub Panel<'scope> <div> (
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
    ) {
        display: flex;
        flex-direction: column;
        padding: 1rem;
    }
}

styled! {
    pub ScopedPanel<'scope><div> (
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        color: Rx<'scope, Hex>,
    ) {
        color: $(color);
    }
}

#[component]
fn RenamedReactiveInput<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain(default)] value: Rx<'scope, String>,
) -> impl View<'scope> {
    let _ = (owner, value);
    children
}

pub fn renamed_reactive_input<'scope>(owner: OwnerAccess<'scope>) -> impl View<'scope> {
    let error_handler = owner
        .error_handler(|_| {})
        .expect("handler should register");
    let ctx = SilexContext::new(owner, error_handler.view());
    match RenamedReactiveInput(ctx, AnyView::Empty).value("renamed") {
        Ok(builder) => builder.build().into_any(),
        Err(error) => AnyView::from(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：这里只测**不渲染类名**的部分；类名渲染由 `silex_css` 的
    // `declare_variants!` 单元测试覆盖。（`.class()` 会触发 `inject_style`，
    // 那条路现在在非 wasm 目标上有个空转后端，不再 panic，但也断言不了什么。）

    /// item 形式生成的枚举可以在调用方作用域里直接命名与解析
    #[test]
    fn generated_enums_are_nameable_and_parse_strictly() {
        use std::str::FromStr;

        assert_eq!(
            CardStyleSize::from_str("icon-lg"),
            Ok(CardStyleSize::IconLg),
            "源码里写的 `icon-lg` 必须能对上枚举变体 `IconLg`"
        );
        assert_eq!(CardStyleSize::OPTIONS, &["Sm", "IconLg"]);
        assert_eq!(CardStyleTone::default(), CardStyleTone::Muted);

        // 写错的选项名如实报错，不静默套用默认样式
        let err = CardStyleSize::try_from_str("icon-xxl").unwrap_err();
        assert!(err.to_string().contains("unknown variant option"), "{err}");
    }

    #[test]
    fn generated_numeric_options_keep_their_source_names() {
        assert_eq!(
            NumericStyleSize::try_from_str("1x"),
            Ok(NumericStyleSize::Val1x)
        );
        assert_eq!(NumericStyleSize::from("1x"), NumericStyleSize::Val1x);
        assert!(NumericStyle::new().get_checked("1x").is_ok());
    }

    /// 类型可命名 ⇒ 能进结构体字段（编译通过即证明）
    #[test]
    fn generated_types_fit_in_struct_fields() {
        let props = CardProps {
            style: CardStyle::new().with_tone(CardStyleTone::Accent),
        };
        assert_eq!(props.style.tone, CardStyleTone::Accent);
    }
}
