use silex::prelude::*;

use crate::advanced::UserSettingsStore;

// --- Theme Definition ---
theme! {
    #[theme(main, prefix = "slx-theme")]
    pub struct AppTheme {
        pub primary: Hex,
        #[theme(var = "--slx-theme-secondary")] // Explicit override
        pub secondary: Hex,
        pub surface: Hex,
        pub surface_alt: Hex,
        pub text: Hex,
        pub border: Hex,
        pub error: Hex,
        pub radius: Px,
    }
}

pub fn default_light_theme() -> AppTheme {
    AppTheme {
        primary: hex("#6366f1"),
        secondary: hex("#a855f7"),
        surface: hex("#ffffff"),
        surface_alt: hex("#f3f4f6"),
        text: hex("#1f2937"),
        border: hex("#e5e7eb"),
        error: hex("#f44336"),
        radius: px(12),
    }
}

pub fn default_dark_theme() -> AppTheme {
    AppTheme {
        primary: hex("#818cf8"),
        secondary: hex("#c084fc"),
        surface: hex("#111827"),
        surface_alt: hex("#1f2937"),
        text: hex("#f9fafb"),
        border: hex("#374151"),
        error: hex("#f44336"),
        radius: px(12),
    }
}

pub fn get_theme(name: &str) -> AppTheme {
    match name {
        "Dark" => default_dark_theme(),
        _ => default_light_theme(),
    }
}

// --- Global Styles ---
// Using the new global! macro to define app-wide styles
global! {
    pub(super) GlobalStyles<'scope>(
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) {
        html, body {
            margin: 0;
            padding: 0;
            min-height: 100vh;
            background-color: $(static AppTheme::SURFACE);
            color: $(static AppTheme::TEXT);
            font-family: "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            transition: background-color 0.3s, color 0.3s;
            // 用 `rem` 而不是 `em`：`-0.05em` 写不成 token——Rust 的词法会把
            // `0.05e` 当成浮点指数，报「expected at least one digit in exponent」。
            // 这条声明此前写作 `-0.05 em`，中间那个空格让浏览器整条丢弃了它。
            letter-spacing: -0.05rem;
        }

        @keyframes fade_in {
            from { opacity: 0; transform: translateY(10px); }
            to { opacity: 1; transform: translateY(0); }
        }

        @media (max-width: 600px) {
            .global-card { padding: 12px; }
        }

        * {
            box-sizing: border-box;
        }

        // Global class for cards with a native hover effect
        .global-card {
            padding: 24px;
            border-radius: 12px;
            border: 1px solid $(static AppTheme::BORDER);
            background: $(static AppTheme::SURFACE);
            transition: transform 0.2s;

            &:hover {
                transform: scale(1.02);
                box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1);
            }
        }

        // Using string literal for complex selectors
        "div > .active-item" {
            border-left: 4px solid $(static AppTheme::PRIMARY);
            padding-left: 12px;
            font-weight: bold;
        }

        unsafe {
            // Global raw styles
            "::-webkit-scrollbar" {
                width: 8px;
            }
            "::-webkit-scrollbar-thumb" {
                background: $(static AppTheme::PRIMARY);
                border-radius: 4px;
            }
        }
    }
}

// --- Styled Components ---

styled! {
    pub DemoCard<'scope><div>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
    ) {
        background: $(static AppTheme::SURFACE);
        color: $(static AppTheme::TEXT);
        border: 1px solid $(static AppTheme::BORDER);
        border-radius: 16px;
        padding: 32px;
        margin: 24px 0;
        box-shadow: 0 10px 40px $(rx!{ scope; error_handler; AppTheme::TEXT.alpha(0.15) });
        backdrop-filter: blur(12px);
        transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
        animation: fade_in 0.8s ease-out;

        &:hover {
            transform: translateY(-4px);
            border-color: rgba(255, 255, 255, 0.15);
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
        }

        @media (max-width: 768px) {
            padding: 16px;
            margin: 12px 0;
        }
    }
}

styled! {
    pub ApplyDemoButton<'scope><button>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
        #[chain] #[prop(into)] variant: Signal<'scope, String>,
    ) {
        @apply flex items-center justify-center px-5 py-2.5 rounded-xl font-semibold transition-all duration-300 shadow-md cursor-pointer;

        &:hover {
            @apply scale-105 shadow-lg;
        }

        variants: {
            variant: {
                primary: "bg-indigo-600 text-white hover:bg-indigo-500",
                secondary: "bg-slate-700 text-slate-100 hover:bg-slate-600",
                outline: "border border-indigo-500 text-indigo-400 hover:bg-indigo-500/10",
            }
        }
    }
}

styled! {
    pub StyledButton<'scope><button>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
        #[chain] #[prop(into)] color: Signal<'scope, CssVar<Hex>>,
        #[chain] #[prop(into)] size: Signal<'scope, String>,
        #[chain] #[prop(into)] hover_color: Signal<'scope, CssVar<Hex>>,
        #[chain] #[prop(into)] pseudo_state: Signal<'scope, String>,
        #[chain] #[prop(into)] border_style: Signal<'scope, BorderValue>,
        #[chain] #[prop(into)] padding_val: Signal<'scope, PaddingValue>,
    ) {
        background: linear-gradient(135deg, #6366f1 0%, #a855f7 100%);
        color: $(color);
        border: $(border_style);
        margin: margin::block_inline(px(8), px(0));
        padding: $(padding_val);
        border-radius: 10px;
        font-weight: 600;
        letter-spacing: 0.3px;
        cursor: pointer;
        transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
        box-shadow: 0 4px 14px 0 rgba(99, 102, 241, 0.3);
        outline: none;
        display: inline-flex;
        align-items: center;
        justify-content: center;

        &:$(pseudo_state) {
            background: $(hover_color);
            transform: translateY(-2px) scale(1.03);
            box-shadow: 0 8px 25px rgba(168, 85, 247, 0.4);
        }

        &:active {
            transform: translateY(0) scale(0.98);
        }

        variants: {
            size: {
                small: { font-size: 13px; }
                medium: { font-size: 15px; }
                large: { font-size: 18px; }
            }
        }
    }
}

styled! {
    #[theme(prefix = "slx-theme")]
    pub ThemePreviewCard<'scope><div>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
    ) {
        background-color: $(static AppTheme::SURFACE);
        color: $(static AppTheme::TEXT);
        border-radius: $(static AppTheme::RADIUS);
        padding: 32px;
        border: 2px solid $(static AppTheme::PRIMARY);
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
        transition: all 0.3s ease;
        margin-top: 24px;
    }
}

styled! {
    pub ThemeButton<'scope><button>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        error_handler: ErrorReporter<'scope>,
        #[chain] #[prop(into)] active: Signal<'scope, bool>
    ) {
        background-color: $(rx!{ scope; error_handler; AppTheme::SECONDARY.alpha(0.9) });
        color: white;
        border: none;
        padding: 12px 24px;
        border-radius: 8px;
        cursor: pointer;
        font-weight: 600;
        transition: all 0.2s;
        opacity: $(rx!(scope; error_handler; if *$active { 1.0 } else { 0.8 }));

        &:hover {
            filter: brightness(1.1);
            transform: translateY(-1px);
        }

        &:active {
            transform: translateY(0);
        }
    }
}

styled! {
    pub DynamicVariantBtn<'scope><button>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        error_handler: ErrorReporter<'scope>,
        #[chain] #[prop(into)] kind: Signal<'scope, String>,
        #[chain] #[prop(into)] dynamic_width: Signal<'scope, Px>,
    ) {
        border-radius: 8px;
        padding: 12px 24px;
        cursor: pointer;
        transition: all 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275);
        border: none;
        color: white;
        display: inline-flex;
        align-items: center;
        justify-content: center;

        variants: {
            kind: {
                primary: {
                    background: linear-gradient(135deg, #6366f1 0%, #a855f7 100%);
                    width: $(dynamic_width);
                }
                secondary: {
                    background: linear-gradient(135deg, #10b981 0%, #3b82f6 100%);
                    width: $(rx!(scope; error_handler; *$dynamic_width + px(60)));
                }
            }
        }
    }
}

// --- Demo Components ---

#[component]
pub fn StylingBasics<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let (color, set_color) = scope.signal(AppTheme::TEXT)?;
    let (size, set_size) = scope.signal("medium".to_string())?;
    let (hover_color, set_hover_color) = scope.signal(AppTheme::PRIMARY)?;
    let (pseudo_state, set_pseudo_state) = scope.signal("hover".to_string())?;
    let (border_state, set_border_state) = scope.signal(border(
        px(2),
        BorderStyleKeyword::Solid,
        ColorKeyword::Transparent,
    ))?;
    let (padding_state, set_padding_state) = scope.signal(padding::block_inline(px(12), px(24)))?;

    Ok(div![
        div![
            h2("✨ Styling Basics"),
            p("Silex offers powerful ways to style components: from scoped CSS-in-Rust to type-safe builders.")
                .style(sty().opacity(0.7)?.font_size(em_unit(1.1))?),
        ].style(sty().margin_bottom(px(40))?),

        DemoCard(scope, chain!(
            h3("🌈 Color Transformations"),
            p("Use the new $(...) syntax to perform Rust-side transformations like alpha blending, which are then compiled to efficient CSS color-mix functions."),
            div![
                span("Primary with 20% alpha").style(
                    sty()
                        .background_color(AppTheme::PRIMARY.alpha(0.2))?
                        .color(AppTheme::PRIMARY)?
                        .padding("8px 16px")?
                        .border_radius(px(8))?
                        .font_weight(600)?
                        .margin_right(px(12))?
                ),
                span("Secondary with 20% alpha").style(
                    sty()
                        .background_color(AppTheme::SECONDARY.alpha(0.2))?
                        .color(AppTheme::SECONDARY)?
                        .padding("8px 16px")?
                        .border_radius(px(8))?
                        .font_weight(600)?
                )
            ].style(sty().margin_top(px(16))?)
        )).error_handler(error_handler).build(),

        DemoCard(scope, chain!(
            h3("1. Atomic & Scoped Styles (styled!)"),
            p(
                "The `styled!` macro creates scoped, reusable components with dynamic interpolation and variants."
            ).style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),
                StyledButton(scope, chain!(
                    "Interactive Scoped Button"
                ))
                .error_handler(error_handler)
                .color(color)?
                .size(size)?
                .hover_color(hover_color)?
                .pseudo_state(pseudo_state)?
                .border_style(border_state)?
                .padding_val(padding_state)?
                .on(event::click, move |_| {
                    set_color.update(|c| {
                        // Toggle between theme text color and warning yellow
                        *c = if *c == AppTheme::TEXT { hex("#fbbf24").into() } else { AppTheme::TEXT };
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    set_size.update(|s| {
                        *s = if *s == "medium" { "large".to_string() } else { "medium".to_string() }
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    set_border_state.update(|b| {
                        *b = border(px(2), BorderStyleKeyword::Dashed, hex("#f472b6"));
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    set_padding_state.update(|p| {
                        *p = padding::block_inline(px(16), px(32));
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    set_hover_color.update(|c| {
                        *c = if c.0 == "var(--slx-theme-primary)" { hex("#ec4899").into() } else { AppTheme::PRIMARY };
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    set_pseudo_state.update(|s| {
                        *s = if *s == "hover" { "active".to_string() } else { "hover".to_string() }
                    })
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    Ok(())
                })
                .build(),
        )).error_handler(error_handler).build(),

        DemoCard(scope, chain!(
            h3("1.5 Dynamic Variants & Attribute Passthrough"),
            p(
                "The `styled!` macro now supports dynamic interpolation directly inside variants, and fully preserves the chainable typed attributes of native HTML tags."
            ).style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),
            {
                let (btn_kind, set_btn_kind) = scope.signal("primary".to_string())?;
                let (btn_width, _set_btn_width) = scope.signal(px(160))?;

                Stack(scope, error_handler, chain!(
                    DynamicVariantBtn(scope, chain!(
                        "Toggle Variant"
                    ), error_handler)
                        .kind(btn_kind)?
                        .dynamic_width(btn_width)?
                        // Below are native HTML <button> attributes seamlessly passed through!
                        .id("passthrough-button") 
                        .type_("button") 
                        .title("Hover me! I'm a native button")
                        .on_click(move |_| {
                            set_btn_kind.update(|k| *k = if k.as_str() == "primary" { "secondary".to_string() } else { "primary".to_string() })
                                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })
                        .build(),
                    div(rx!(scope; error_handler; format!("Current Variant: {}, Base Width Signal: {}", $btn_kind, $btn_width)))
                        .style(sty().font_size(em_unit(0.9))?.opacity(0.8)?)
                )).gap(16)?.build()
            }
        )).error_handler(error_handler).build(),

        DemoCard(scope, chain!(
            h3("2. Type-Safe Style Builder (sty)"),
            p(
                "A chainable API for defining styles with full reactivity, ideal for dynamic inline styles."
            ).style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),
            div![
                span("Hover to Reveal Effects").style(
                    sty()
                        .display(DisplayKeyword::InlineBlock)?
                        .padding(padding::block_inline(px(24), px(40)))?
                        .background_color(AppTheme::SURFACE)?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .border_radius(AppTheme::RADIUS)?
                        .color(AppTheme::TEXT)?
                        .font_size(px(16))?
                        .font_weight(600)?
                        .cursor(CursorKeyword::Pointer)?
                        .transition("all 0.4s ease")?
                        .on_hover(|s| {
                            s.background_color(AppTheme::PRIMARY)?
                                .border_color(AppTheme::SECONDARY)?
                                .color(hex("#ffffff"))?
                                .transform(transform().scale(1.05).rotate(deg(1)))
                        })?
                )
            ],
            p("Signals are natively supported:").style(sty().margin("20px 0 10px")?.font_size(em_unit(0.9))?.opacity(0.6)?),
            {
                let (count, set_count) = scope.signal(0)?;
                let (show_shadow, set_show_shadow) = scope.signal(true)?;
                let (active_border, set_active_border) = scope.signal(true)?;

                Stack(scope, error_handler, chain!(
                    div![
                        button("Grow").on(event::click, move |_| {
                            set_count.update(|n| *n += 1)
                                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })
                            .style(sty().padding("8px 16px")?.border_radius(px(6))?.border("1px solid #374151")?.background("#111827")?.color(ColorName::White)?.cursor("pointer")?.margin_right(px(8))?),
                        button("Toggle Box Shadow").on(event::click, move |_| {
                            set_show_shadow.update(|s| *s = !*s)
                                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })
                            .style(sty().padding("8px 16px")?.border_radius(px(6))?.border("1px solid #374151")?.background("#111827")?.color(ColorName::White)?.cursor("pointer")?.margin_right(px(8))?),
                        button("Toggle Border").on(event::click, move |_| {
                            set_active_border.update(|b| *b = !*b)
                                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })
                            .style(sty().padding("8px 16px")?.border_radius(px(6))?.border("1px solid #374151")?.background("#111827")?.color(ColorName::White)?.cursor("pointer")?),
                    ].style(sty().display("flex")?.align_items("center")?.margin_bottom(px(12))?),

                    div(rx!(scope; error_handler; format!("Reactive Width: {}px", 180 + *$count * 30))).style(
                        sty()
                            .width(rx!(scope; error_handler; px(180 + *$count * 30)))?
                            .height(px(48))?
                            .background("linear-gradient(90deg, #4f46e5, #9333ea)")?
                            .color(hex("#fff"))?
                            .display(DisplayKeyword::Flex)?
                            .align_items(AlignItemsKeyword::Center)?
                            .justify_content(JustifyContentKeyword::Center)?
                            .border_radius(px(12))?
                            .transition("all 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275)")?
                            // 动态开启/删除 box-shadow 属性 (css_none() 即删除属性)
                            .box_shadow(rx!(scope; error_handler; if *$show_shadow { css_some("0 8px 20px rgba(79, 70, 229, 0.5)") } else { css_none() }))?
                            // 动态开启/删除 border 属性 (使用 CssOption::some/none 独立方法)
                            .border(rx!(scope; error_handler; if *$active_border { CssOption::some(border(px(2), BorderStyleKeyword::Solid, hex("#f472b6"))) } else { CssOption::none() }))?
                    )
                )).build()
            }
        )).error_handler(error_handler).build(),

        DemoCard(scope, chain!(
            h3("3. Layout Primitives"),
            p("Structural layout components like Stack, Grid, and Center for effortless alignment.")
                .style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),

            Stack(scope, error_handler, chain!(
                span("Vertical Stack with Gap"),
                Grid(scope, error_handler, chain!(
                    div("Grid Item 1").style(sty().background("#312e81")?.padding("10px")?.border_radius(px(8))?),
                    div("Grid Item 2").style(sty().background("#312e81")?.padding("10px")?.border_radius(px(8))?),
                    div("Grid Item 3").style(sty().background("#312e81")?.padding("10px")?.border_radius(px(8))?),
                )).columns(3)?.gap(12)?.build(),
                Center(scope, chain!(
                    "I am perfectly centered"
                ))
                    .error_handler(error_handler)
                    .style(sty().background_color(hex("#4f46e5"))?.padding(px(12))?.border_radius(px(8))?)?
                    .build(),
            )).gap(16)?.build()
        )).error_handler(error_handler).build(),
    ])
}

#[component]
pub fn Theming<'scope>(
    scope: Scope<'scope>,
    global_settings: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let theme = global_settings
        .theme
        .map(scope, |t| get_theme(t), error_handler)?;
    let is_dark = global_settings
        .theme
        .map(scope, |t| t == "Dark", error_handler)?;

    Ok(div![
        h2("🎨 Theme Engine"),
        p("Define design tokens in Rust and propagate them via CSS variables with full layout transparency.")
            .style(sty().color(hex("#6b7280"))?.margin_bottom(px(32))?.font_size(em_unit(1.1))?),

        div![
            button("🌞 Light Mode")
                .on(event::click, move |_| {
                    global_settings
                        .theme
                        .set("Light".to_string())
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))
                })
                .style(
                    sty()
                        .padding(padding::block_inline(px(8), px(16)))?
                        .border_radius(px(6))?
                        .cursor(CursorKeyword::Pointer)?
                        .transition("all 0.2s")?
                        .margin_right(px(12))?
                        .background_color(rx!(scope; error_handler; if !*$is_dark { AppTheme::PRIMARY } else { hex("#f3f4f6").into() }))?
                        .color(rx!(scope; error_handler; if !*$is_dark { hex("#ffffff") } else { hex("#374151") }))?
                        .border(rx!(scope; error_handler; if !*$is_dark { border(px(1), BorderStyleKeyword::Solid, AppTheme::PRIMARY) } else { border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db")) }))?
                ),
            button("🌙 Dark Mode")
                .on(event::click, move |_| {
                    global_settings
                        .theme
                        .set("Dark".to_string())
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))
                })
                .style(
                    sty()
                        .padding(padding::block_inline(px(8), px(16)))?
                        .border_radius(px(6))?
                        .cursor(CursorKeyword::Pointer)?
                        .transition("all 0.2s")?
                        .background_color(rx!(scope; error_handler; if *$is_dark { AppTheme::PRIMARY } else { hex("#f3f4f6").into() }))?
                        .color(rx!(scope; error_handler; if *$is_dark { hex("#ffffff") } else { hex("#374151") }))?
                        .border(rx!(scope; error_handler; if *$is_dark { border(px(1), BorderStyleKeyword::Solid, AppTheme::PRIMARY) } else { border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db")) }))?
                ),
        ].style(sty().margin_bottom(px(24))?),

        ThemePreviewCard(scope, chain!(
            h4("Real-time Propagation"),
            p("These styles react to the Rust theme object via CSS variables."),
            ThemeButton(scope, "Themed Scoped Button", error_handler)
                .active(false)?
                .build()
        )).error_handler(error_handler).build().apply(theme_variables(theme)),

        h3("Incremental Patching (New)").style(sty().margin("40px 0 16px")?),
        p("Only override specific variables (like 'primary') while inheriting the rest from the environment via CSS inheritance.")
            .style(sty().color(hex("#9ca3af"))?.margin_bottom(px(24))?),

        div![
            ThemePreviewCard(scope, chain!(
                h4("Primary Patch"),
                p("This card ONLY patches 'primary' to Hot Pink."),
                div![
                    ThemeButton(scope, "Still Secondary Color", error_handler)
                        .active(false)?
                        .build(),
                    span(" (Variable inheritance in action!) ").style(sty().font_size(em_unit(0.8))?.opacity(0.6)?)
                ]
            )).error_handler(error_handler).build()
            .apply(theme_patch(rx!(scope; error_handler; AppThemePatch::default().primary(hex("#ff69b4"))))),
        ].apply(theme_variables(theme)),

        h3("Layout Continuity").style(sty().margin("40px 0 16px")?),
        p("Theme variables are injected via 'apply', ensuring no extra DOM wrappers break CSS layouts like Flex or Grid.")
            .style(sty().color(hex("#9ca3af"))?.margin_bottom(px(24))?),

        DemoCard(scope, chain!(
            h4("1. Theme variables in Flex (Stack)"),
            p("The red border is a Stack. Variable injection doesn't break the flow.").style(sty().margin_bottom(px(12))?.font_size(em_unit(0.9))?.opacity(0.7)?),
            Stack(scope, error_handler, chain!(
                div("Themed Row 1").style(sty().background(AppTheme::SURFACE_ALT)?.padding("10px")?.margin("4px")?.border_radius(px(4))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::PRIMARY))?)
                    .apply(theme_variables(theme)),
                div("Themed Row 2").style(sty().background(AppTheme::SURFACE_ALT)?.padding("10px")?.margin("4px")?.border_radius(px(4))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::SECONDARY))?)
                    .apply(theme_variables(theme)),
            )).style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#ef4444")))?.padding(px(8))?)?.build()
        )).error_handler(error_handler).build(),

        DemoCard(scope, chain!(
            h4("2. Nested Layout Stability"),
            p("Even deeply nested layouts remain stable with variable injection.").style(sty().margin_bottom(px(12))?.font_size(em_unit(0.9))?.opacity(0.7)?),
            Stack(scope, error_handler, chain!(
                Stack(scope, error_handler, chain!(
                    div("Nested 1").style(sty().background(AppTheme::SURFACE)?.color(AppTheme::TEXT)?.padding("10px")?.border_radius(px(4))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::PRIMARY))?),
                    div("Nested 2").style(sty().background(AppTheme::SURFACE)?.color(AppTheme::TEXT)?.padding("10px")?.border_radius(px(4))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::SECONDARY))?),
                )).gap(4)?.build()?.apply(theme_variables(theme)),
                div("Sibling of Nested Stack").style(sty().background(AppTheme::SURFACE_ALT)?.color(AppTheme::TEXT)?.padding("10px")?.margin_top(px(4))?.border_radius(px(4))?),
            )).style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#3b82f6")))?.padding(px(8))?)?.build()
        )).error_handler(error_handler).build(),
    ]
    .style(sty().padding(px(24))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(12))?.background(AppTheme::SURFACE)?.transition("all 0.3s")?))
}

#[component]
pub fn AdvancedStyling<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div![
        h2("🧮 Advanced Styling"),
        p("Type-safe CSS math functions and declarative gradients for complex visuals.")
            .style(sty().margin_bottom(px(32))?.color(hex("#9ca3af"))?.font_size(em_unit(1.1))?),

        Stack(scope, error_handler, chain!(
            DemoCard(scope, chain!(
                h4("1. Math Functions (calc, clamp, min, max)"),
                p("Perform type-safe math operations across units at compile time.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
                Stack(scope, error_handler, chain!(
                    div("Calc: 100% - 60px").style(
                        sty()
                            .width(calc(pct(100) - px(60)))?
                            .height(px(40))?
                            .background("#312e81")?
                            .border_radius(px(8))?
                            .display(DisplayKeyword::Flex)?
                            .align_items(AlignItemsKeyword::Center)?
                            .padding_left(px(12))?
                    ),
                    div("Clamp (15% | 50% | 85%)").style(
                        sty()
                            .width(clamp(pct(15), pct(50), pct(85)))?
                            .height(px(40))?
                            .background("#4338ca")?
                            .border_radius(px(8))?
                            .display(DisplayKeyword::Flex)?
                            .align_items(AlignItemsKeyword::Center)?
                            .padding_left(px(12))?
                    ),
                )).gap(12)?.build()
        )).error_handler(error_handler).build(),
            DemoCard(scope, chain!(
                h4("2. Gradients DSL"),
                p("Declarative API for complex linear and radial gradients.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
                Grid(scope, error_handler, chain!(
                    div![
                        p("Linear").style(sty().margin_bottom(px(8))?.font_size(em_unit(0.8))?),
                        div(()).style(sty().height(px(100))?.border_radius(px(12))?.background_image(linear_gradient().to(Direction::ToRight).stop_at(hex("#6366f1"), pct(0)).stop_at(hex("#a855f7"), pct(100)).build())?)
                    ],
                    div![
                        p("Radial").style(sty().margin_bottom(px(8))?.font_size(em_unit(0.8))?),
                        div(()).style(sty().height(px(100))?.border_radius(px(12))?.background_image(radial_gradient().circle().at(ObjectPositionKeyword::Center).stop_at(hex("#818cf8"), pct(0)).stop_at(hex("#1e1e24"), pct(100)).build())?)
                    ],
                    div![
                        p("Angled (45deg)").style(sty().margin_bottom(px(8))?.font_size(em_unit(0.8))?),
                        div(()).style(sty().height(px(100))?.border_radius(px(12))?.background_image(linear_gradient().to(deg(45).into()).stop_at(hex("#f43f5e"), pct(0)).stop_at(hex("#fb923c"), pct(100)).build())?)
                    ],
                    div![
                        p("Repeating").style(sty().margin_bottom(px(8))?.font_size(em_unit(0.8))?),
                        div(()).style(sty().height(px(100))?.border_radius(px(12))?.background_image(linear_gradient().repeating().to(Direction::ToBottomRight).stop_at(hex("#1e1e24"), pct(0)).stop_at(hex("#1e1e24"), px(10)).stop_at(hex("#312e81"), px(10)).stop_at(hex("#312e81"), px(20)).build())?)
                    ],
                )).columns(2)?.gap(16)?.build()
        )).error_handler(error_handler).build(),
            DemoCard(scope, chain!(
                h4("3. Responsive & Nested (Style Builder)"),
                p("The enhanced `sty()` API now supports `@media` and complex nesting, just like the `styled!` macro.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
                div![
                    span![
                        "Resize window and hover child!",
                        div("I am the child box").class("child-box")
                            .style(sty().margin_top(px(16))?.color(hex("#fff"))?.font_size(px(12))?.text_align(TextAlignKeyword::Center)?.width(BlockSizeKeyword::FitContent)?.white_space("nowrap")?)
                    ].style(
                        sty()
                            .display(DisplayKeyword::Block)?
                            .padding(px(32))?
                            .background(AppTheme::SURFACE)?
                            .color(AppTheme::TEXT)?
                            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                            .border_radius(px(16))?
                            .transition("all 0.3s")?
                            .nest("& > .child-box", |s| s
                                .padding(padding::block_inline(px(12), px(20)))?
                                .background(AppTheme::PRIMARY)?
                                .border_radius(px(8))?
                                .transition("all 0.4s cubic-bezier(0.68, -0.55, 0.265, 1.55)"))?
                            .on_hover(|s| {
                                let s = s.border_color(AppTheme::PRIMARY)?;
                                s.nest("& > .child-box", |s| {
                                    s.transform(transform().translate_x(px(100)).rotate(deg(180)))?
                                        .background(AppTheme::SECONDARY)
                                })
                            })?
                            .media("@media (max-width: 768px)", |s| {
                                let s = s.background(AppTheme::BORDER)?;
                                s.nest("& > .child-box", |s| s.background(hex("#f43f5e")))
                            })?
                    ),
                ].style(sty().position(PositionKeyword::Relative)?)
        )).error_handler(error_handler).build(),
            DemoCard(scope, chain!(
                h4("4. Complex DSLs (Grid Areas & Font Variations)"),
                p("Specialized support for complex grid layouts and variable fonts.").style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),
                Stack(scope, error_handler, chain!(
                    div![
                        span("Grid Template Areas").style(sty().margin_bottom(px(8))?.display("block")?.font_size(em_unit(0.9))?.opacity(0.7)?),
                        div![
                            div("Header").style(sty().grid_area("header")?.background(hex("#4f46e5"))?.padding(px(8))?),
                            div("Main").style(sty().grid_area("main")?.background(hex("#312e81"))?.padding(px(24))?),
                            div("Sidebar").style(sty().grid_area("sidebar")?.background(hex("#1e1e24"))?.padding(px(8))?),
                        ].style(
                            sty()
                                .display(DisplayKeyword::Grid)?
                                .gap(px(8))?
                                .grid_template_areas(grid_template_areas(["header header", "main sidebar"]))?
                                .grid_template_columns("2fr 1fr")?
                        )
                    ],
                    div![
                        span("Font Variation Settings").style(sty().margin_bottom(px(8))?.display("block")?.font_size(em_unit(0.9))?.opacity(0.7)?),
                        div("Variable Font Styling (Weight: 700, Ital: 0.5)")
                            .style(
                                sty()
                                    .font_size(px(24))?
                                    .font_variation_settings(font_variation_settings([("wght", 700.0), ("ital", 0.5)]))?
                            )
                    ]
                )).gap(24)?.build()
            )).error_handler(error_handler).build()
        )).gap(24)?.build(),

        h2("🎨 Deep Integration with `@apply` & Inline Tailwind Mixins").style(sty().margin_top(px(48))?),
        p("Use `@apply` directives inside `css!` & `styled!` blocks and inline Tailwind utility strings in variant definitions.")
            .style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),

        DemoCard(scope, chain!(
            h4("1. `@apply` Directives & Inline Tailwind String Variants"),
            p("Compose complex styles seamlessly using Tailwind utilities within standard styled! components.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
            div![
                ApplyDemoButton(scope, "Primary Variant")
                    .error_handler(error_handler)
                    .variant("primary")?
                    .build(),
                ApplyDemoButton(scope, "Secondary Variant")
                    .error_handler(error_handler)
                    .variant("secondary")?
                    .build(),
                ApplyDemoButton(scope, "Outline Variant")
                    .error_handler(error_handler)
                    .variant("outline")?
                    .build(),
            ].style(sty().display(DisplayKeyword::Flex)?.gap(px(16))?)
        )).error_handler(error_handler).build(),

        h2("🔒 Unsafe Styles & Escape Hatches").style(sty().margin_top(px(48))?),
        p("Bypass compile-time property and type validation for non-standard CSS or raw value injection.")
            .style(sty().margin_bottom(px(24))?.color(hex("#9ca3af"))?),

        Stack(scope, error_handler, chain!(
            DemoCard(scope, chain!(
                h4("1. Local Unsafe Blocks"),
                p("Use `unsafe { ... }` blocks to inject raw properties or bypass type checks locally.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
                UnsafeBlockDemo(scope, "I have a raw orange glow")
                    .error_handler(error_handler)
                    .build()
                    .style(sty().margin_bottom(px(16))?)
        )).error_handler(error_handler).build(),
            DemoCard(scope, chain!(
                h4("2. Global Unsafe Component"),
                p("Marking a styled component as `unsafe` disables all validation for its entire CSS block.").style(sty().margin_bottom(px(16))?.font_size(em_unit(0.9))?.opacity(0.7)?),
                UnsafeCompDemo(scope, "Everything here is raw")
                    .error_handler(error_handler)
                    .build()
                    .style(sty().width(pct(100))?)
            )).error_handler(error_handler).build()
        )).gap(24)?.build()
    ])
}

// --- Unsafe Demos ---

styled! {
    pub UnsafeBlockDemo<'scope><div>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
    ) {
        padding: 24px;
        border-radius: 12px;
        background: #1e1e24;
        transition: all 0.3s;

        // standard styles
        color: white;

        unsafe {
            // Non-standard or vendor properties
            -webkit-backdrop-filter: blur(10px);
            backdrop-filter: blur(10px);
            box-shadow: 0 0 20px rgba(251, 146, 60, 0.4);
            // Using a semi-transparent background to make the blur visible
            background-color: rgba(45, 45, 53, 0.7);
            // A truly custom property to verify raw injection
            --silex-unsafe-check: "passed";
        }
    }
}

styled! {
    pub unsafe UnsafeCompDemo<'scope><div>(
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
    ) {
        // Enire component is unsafe
        padding: 32px;
        border: 2px dashed #f43f5e;
        border-radius: 16px;
        background: rgba(17, 24, 39, 0.8);

        // No type checking here - passing a raw string for color
        color: rgb(244, 63, 94);

        font-family: $(rx!(scope; error_handler; "'Courier New', monospace".to_string()));
        cursor: help;

        &:hover {
            border-color: cyan;
            box-shadow: 0 0 30px rgba(0, 255, 255, 0.3);
        }
    }
}
