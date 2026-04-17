use silex::prelude::*;

// --- Theme Definition ---
define_theme! {
    #[theme(prefix = "slx-theme")]
    pub struct AppTheme {
        pub primary: Hex,
        #[theme(var = "--slx-theme-secondary")] // Explicit override
        pub secondary: Hex,
        pub surface: Hex,
        pub text: Hex,
        pub border: Hex,
        pub radius: Px,
    }
}

pub type Theme = AppTheme;

pub fn default_light_theme() -> AppTheme {
    AppTheme {
        primary: hex("#6366f1"),
        secondary: hex("#a855f7"),
        surface: hex("#ffffff"),
        text: hex("#1f2937"),
        border: hex("#e5e7eb"),
        radius: px(12),
    }
}

pub fn default_dark_theme() -> AppTheme {
    AppTheme {
        primary: hex("#818cf8"),
        secondary: hex("#c084fc"),
        surface: hex("#111827"),
        text: hex("#f9fafb"),
        border: hex("#374151"),
        radius: px(12),
    }
}

pub fn get_theme(name: &str) -> AppTheme {
    match name {
        "Dark" => default_dark_theme(),
        _ => default_light_theme(),
    }
}

// --- Styled Components ---

styled! {
    pub DemoCard<div>(children: Children) {
        background: var(--slx-theme-surface);
        color: var(--slx-theme-text);
        border: 1px solid var(--slx-theme-border);
        border-radius: 16px;
        padding: 32px;
        margin: 24px 0;
        box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
        backdrop-filter: blur(12px);
        transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);

        &:hover {
            transform: translateY(-4px);
            border-color: rgba(255, 255, 255, 0.15);
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
        }
    }
}

styled! {
    pub StyledButton<button>(
        children: Children,
        #[prop(into)] color: Signal<Hex>,
        #[prop(into)] size: Signal<String>,
        #[prop(into)] hover_color: Signal<Hex>,
        #[prop(into)] pseudo_state: Signal<String>,
        #[prop(into)] border_style: Signal<BorderValue>,
        #[prop(into)] padding_val: Signal<PaddingValue>,
    ) {
        background: linear-gradient(135deg, #6366f1 0%, #a855f7 100%);
        color: $(color);
        border: $(border_style);
        margin: $(margin::x_y(px(8), px(0)));
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
    pub ThemePreviewCard<div>(children: Children) {
        background-color: $theme.surface;
        color: $theme.text;
        border-radius: $theme.radius;
        padding: 32px;
        border: 2px solid $theme.primary;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
        transition: all 0.3s ease;
        margin-top: 24px;
    }
}

styled! {
    pub ThemeButton<button>(
        children: Children,
        #[prop(into)] active: Signal<bool>
    ) {
        background-color: $theme.secondary;
        color: white;
        border: none;
        padding: 12px 24px;
        border-radius: 8px;
        cursor: pointer;
        font-weight: 600;
        transition: all 0.2s;
        opacity: $(rx!(if active.get() { 1.0 } else { 0.8 }));

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
    pub DynamicVariantBtn<button>(
        children: Children,
        #[prop(into)] kind: Signal<String>,
        #[prop(into)] dynamic_width: Signal<Px>,
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
                    width: $(rx!(dynamic_width.get() + px(60)));
                }
            }
        }
    }
}

// --- Demo Components ---

#[component]
pub fn StylingBasics() -> impl View {
    let (color, set_color) = Signal::new(hex("#ffffff"));
    let (size, set_size) = Signal::new("medium".to_string());
    let (hover_color, set_hover_color) = Signal::new(hex("#4f46e5"));
    let (pseudo_state, set_pseudo_state) = Signal::new("hover".to_string());
    let (border_state, set_border_state) =
        Signal::new(border(px(2), BorderStyleKeyword::Solid, hex("transparent")));
    let (padding_state, set_padding_state) = Signal::new(padding::x_y(px(12), px(24)));

    div![
        div![
            h2("✨ Styling Basics"),
            p("Silex offers powerful ways to style components: from scoped CSS-in-Rust to type-safe builders.")
                .style("opacity: 0.7; font-size: 1.1em;"),
        ].style("margin-bottom: 40px;"),

        DemoCard().children(view_chain!(
            h3("1. Atomic & Scoped Styles (styled!)"),
            p(
                "The `styled!` macro creates scoped, reusable components with dynamic interpolation and variants."
            ).style("margin-bottom: 24px; color: #9ca3af;"),
            StyledButton()
                .children("Interactive Scoped Button")
                .color(color)
                .size(size)
                .hover_color(hover_color)
                .pseudo_state(pseudo_state)
                .border_style(border_state)
                .padding_val(padding_state)
                .on(event::click, move |_| {
                    set_color.update(|c| {
                        *c = if c.0 == "#ffffff" { hex("#fbbf24") } else { hex("#ffffff") }
                    });
                    set_size.update(|s| {
                        *s = if *s == "medium" { "large".to_string() } else { "medium".to_string() }
                    });
                    set_border_state.update(|b| {
                        *b = border(px(2), BorderStyleKeyword::Dashed, hex("#f472b6"));
                    });
                    set_padding_state.update(|p| {
                        *p = padding::x_y(px(16), px(32));
                    });
                    set_hover_color.update(|c| {
                        *c = if c.0 == "#4f46e5" { hex("#ec4899") } else { hex("#4f46e5") }
                    });
                    set_pseudo_state.update(|s| {
                        *s = if *s == "hover" { "active".to_string() } else { "hover".to_string() }
                    });
                }),
        )),

        DemoCard().children(view_chain!(
            h3("1.5 Dynamic Variants & Attribute Passthrough"),
            p(
                "The `styled!` macro now supports dynamic interpolation directly inside variants, and fully preserves the chainable typed attributes of native HTML tags."
            ).style("margin-bottom: 24px; color: #9ca3af;"),
            {
                let (btn_kind, set_btn_kind) = Signal::new("primary".to_string());
                let (btn_width, _set_btn_width) = Signal::new(px(160));

                Stack().gap(16).children(view_chain!(
                    DynamicVariantBtn()
                        .kind(btn_kind)
                        .dynamic_width(btn_width)
                        .children("Toggle Variant")
                        // Below are native HTML <button> attributes seamlessly passed through!
                        .id("passthrough-button") 
                        .type_("button") 
                        .title("Hover me! I'm a native button")
                        .on_click(move |_| {
                            set_btn_kind.update(|k| *k = if k.as_str() == "primary" { "secondary".to_string() } else { "primary".to_string() });
                        }),
                    div(rx!(format!("Current Variant: {}, Base Width Signal: {}", btn_kind.get(), btn_width.get())))
                        .style("font-size: 0.9em; opacity: 0.8;")
                ))
            }
        )),

        DemoCard().children(view_chain!(
            h3("2. Type-Safe Style Builder (sty)"),
            p(
                "A chainable API for defining styles with full reactivity, ideal for dynamic inline styles."
            ).style("margin-bottom: 24px; color: #9ca3af;"),
            div![
                span("Hover to Reveal Effects").style(
                    sty()
                        .display(DisplayKeyword::InlineBlock)
                        .padding(padding::x_y(px(24), px(40)))
                        .background_color(hex("#1e1e24"))
                        .border(border(px(1), BorderStyleKeyword::Solid, hex("#374151")))
                        .border_radius(px(16))
                        .color(hex("#e5e7eb"))
                        .font_size(px(16))
                        .font_weight(600)
                        .cursor(CursorKeyword::Pointer)
                        .transition("all 0.4s ease")
                        .on_hover(|s| {
                            s.background_color(hex("#312e81"))
                                .border_color(hex("#6366f1"))
                                .color(hex("#ffffff"))
                                .transform(transform().scale(1.05).rotate(deg(1)))
                        })
                )
            ],
            p("Signals are natively supported:").style("margin: 20px 0 10px; font-size: 0.9em; opacity: 0.6;"),
            {
                let (count, set_count) = Signal::new(0);
                div![
                    button("Grow").on(event::click, move |_| set_count.update(|n| *n += 1))
                        .style("padding: 8px 16px; border-radius: 6px; border: 1px solid #374151; background: #111827; color: white; cursor: pointer;"),
                    div(rx!(format!("Reactive Width: {}px", 180 + count.get() * 30))).style(
                        sty()
                            .width(rx!(px(180 + count.get() * 30)))
                            .height(px(48))
                            .background("linear-gradient(90deg, #4f46e5, #9333ea)")
                            .color(hex("#fff"))
                            .display(DisplayKeyword::Flex)
                            .align_items(AlignItemsKeyword::Center)
                            .justify_content(JustifyContentKeyword::Center)
                            .margin(margin::left(px(16)))
                            .border_radius(px(12))
                            .box_shadow("0 4px 12px rgba(79, 70, 229, 0.3)")
                            .transition("width 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275)")
                    )
                ]
                .style("display: flex; align-items: center;")
            }
        )),

        DemoCard().children(view_chain!(
            h3("3. Layout Primitives"),
            p("Structural layout components like Stack, Grid, and Center for effortless alignment.")
                .style("margin-bottom: 24px; color: #9ca3af;"),

                Stack()
                .gap(16)
                .children(view_chain!(
                    span("Vertical Stack with Gap"),
                    Grid()
                        .columns(3)
                        .gap(12)
                        .children(view_chain!(
                            div("Grid Item 1").style("background: #312e81; padding: 10px; border-radius: 8px;"),
                            div("Grid Item 2").style("background: #312e81; padding: 10px; border-radius: 8px;"),
                            div("Grid Item 3").style("background: #312e81; padding: 10px; border-radius: 8px;"),
                        )),
                    Center()
                        .style(sty().background_color(hex("#4f46e5")).padding(px(12)).border_radius(px(8)))
                        .children("I am perfectly centered"),
                ))
        )),
    ]
}

#[component]
pub fn Theming() -> impl View {
    let global_settings = crate::advanced::use_user_settings();
    let initial_theme = if global_settings.theme.get_untracked() == "Dark" {
        default_dark_theme()
    } else {
        default_light_theme()
    };
    let (theme, set_theme) = Signal::new(initial_theme);
    let is_dark = theme.map(|t| t.surface.0 == "#111827");

    div![
        h2("🎨 Theme Engine"),
        p("Define design tokens in Rust and propagate them via CSS variables with full layout transparency.")
            .style("color: #6b7280; margin-bottom: 32px; font-size: 1.1em;"),

        div![
            button("🌞 Light Mode")
                .on(event::click, move |_| set_theme.set(default_light_theme()))
                .style(
                    sty()
                        .padding(padding::x_y(px(8), px(16)))
                        .border_radius(px(6))
                        .cursor(CursorKeyword::Pointer)
                        .transition("all 0.2s")
                        .margin(margin::right(px(12)))
                        .background_color(rx!(if !is_dark.get() { hex("#6366f1") } else { hex("#f3f4f6") }))
                        .color(rx!(if !is_dark.get() { hex("#ffffff") } else { hex("#374151") }))
                        .border(rx!(if !is_dark.get() { border(px(1), BorderStyleKeyword::Solid, hex("#6366f1")) } else { border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db")) }))
                ),
            button("🌙 Dark Mode")
                .on(event::click, move |_| set_theme.set(default_dark_theme()))
                .style(
                    sty()
                        .padding(padding::x_y(px(8), px(16)))
                        .border_radius(px(6))
                        .cursor(CursorKeyword::Pointer)
                        .transition("all 0.2s")
                        .background_color(rx!(if is_dark.get() { hex("#4f46e5") } else { hex("#f3f4f6") }))
                        .color(rx!(if is_dark.get() { hex("#ffffff") } else { hex("#374151") }))
                        .border(rx!(if is_dark.get() { border(px(1), BorderStyleKeyword::Solid, hex("#4f46e5")) } else { border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db")) }))
                ),
        ].style("margin-bottom: 24px;"),

        ThemePreviewCard().apply(theme_variables(theme)).children(view_chain!(
            h4("Real-time Propagation"),
            p("These styles react to the Rust theme object via CSS variables."),
            ThemeButton().children("Themed Scoped Button").active(false)
        )),

        h3("Layout Continuity").style("margin: 40px 0 16px;"),
        p("Theme variables are injected via 'apply', ensuring no extra DOM wrappers break CSS layouts like Flex or Grid.")
            .style("color: #9ca3af; margin-bottom: 24px;"),

        DemoCard().children(view_chain!(
            h4("1. Theme variables in Flex (Stack)"),
            p("The red border is a Stack. Variable injection doesn't break the flow.").style("margin-bottom: 12px; font-size: 0.9em; opacity: 0.7;"),
            Stack().style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#ef4444"))).padding(px(8))).children(view_chain!(
                div("Themed Row 1").style("background: #1e1e24; padding: 10px; margin: 4px; border-radius: 4px; border: 1px solid $theme.primary;")
                    .apply(theme_variables(theme)),
                div("Themed Row 2").style("background: #1e1e24; padding: 10px; margin: 4px; border-radius: 4px; border: 1px solid $theme.secondary;")
                    .apply(theme_variables(theme)),
            ))
        )),

        DemoCard().children(view_chain!(
            h4("2. Nested Layout Stability"),
            p("Even deeply nested layouts remain stable with variable injection.").style("margin-bottom: 12px; font-size: 0.9em; opacity: 0.7;"),
            Stack().style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#3b82f6"))).padding(px(8))).children(view_chain!(
                Stack().gap(4).apply(theme_variables(theme)).children(view_chain!(
                    div("Nested 1").style("background: $theme.surface; color: $theme.text; padding: 10px; border-radius: 4px; border: 1px solid $theme.primary;"),
                    div("Nested 2").style("background: $theme.surface; color: $theme.text; padding: 10px; border-radius: 4px; border: 1px solid $theme.secondary;"),
                )),
                div("Sibling of Nested Stack").style("background: #1e1e24; color: #fff; padding: 10px; margin-top: 4px; border-radius: 4px;"),
            ))
        ))
    ]
    .style("padding: 24px; border: 1px solid var(--slx-theme-border); border-radius: 12px; background: var(--slx-theme-surface); transition: all 0.3s;")
}

#[component]
pub fn AdvancedStyling() -> impl View {
    div![
        h2("🧮 Advanced Styling"),
        p("Type-safe CSS math functions and declarative gradients for complex visuals.")
            .style("margin-bottom: 32px; color: #9ca3af; font-size: 1.1em;"),

        Stack().gap(24).children(view_chain!(
            DemoCard().children(view_chain!(
                h4("1. Math Functions (calc, clamp, min, max)"),
                p("Perform type-safe math operations across units at compile time.").style("margin-bottom: 16px; font-size: 0.9em; opacity: 0.7;"),
                Stack().gap(12).children(view_chain!(
                    div("Calc: 100% - 60px").style(
                        sty()
                            .width(calc(pct(100) - px(60)))
                            .height(px(40))
                            .background("#312e81")
                            .border_radius(px(8))
                            .display(DisplayKeyword::Flex)
                            .align_items(AlignItemsKeyword::Center)
                            .padding(padding::left(px(12)))
                    ),
                    div("Clamp (15% | 50% | 85%)").style(
                        sty()
                            .width(clamp(pct(15), pct(50), pct(85)))
                            .height(px(40))
                            .background("#4338ca")
                            .border_radius(px(8))
                            .display(DisplayKeyword::Flex)
                            .align_items(AlignItemsKeyword::Center)
                            .padding(padding::left(px(12)))
                    ),
                ))
            )),
            DemoCard().children(view_chain!(
                h4("2. Gradients DSL"),
                p("Declarative API for complex linear and radial gradients.").style("margin-bottom: 16px; font-size: 0.9em; opacity: 0.7;"),
                Grid().columns(2).gap(16).children(view_chain!(
                    div![
                        p("Linear").style("margin-bottom: 8px; font-size: 0.8em;"),
                        div(()).style(sty().height(px(100)).border_radius(px(12)).background_image(linear_gradient().to(Direction::ToRight).stop(hex("#6366f1"), pct(0)).stop(hex("#a855f7"), pct(100)).build()))
                    ],
                    div![
                        p("Radial").style("margin-bottom: 8px; font-size: 0.8em;"),
                        div(()).style(sty().height(px(100)).border_radius(px(12)).background_image(radial_gradient().circle().at(ObjectPositionKeyword::Center).stop(hex("#818cf8"), pct(0)).stop(hex("#1e1e24"), pct(100)).build()))
                    ],
                    div![
                        p("Angled (45deg)").style("margin-bottom: 8px; font-size: 0.8em;"),
                        div(()).style(sty().height(px(100)).border_radius(px(12)).background_image(linear_gradient().to(deg(45).into()).stop(hex("#f43f5e"), pct(0)).stop(hex("#fb923c"), pct(100)).build()))
                    ],
                    div![
                        p("Repeating").style("margin-bottom: 8px; font-size: 0.8em;"),
                        div(()).style(sty().height(px(100)).border_radius(px(12)).background_image(linear_gradient().repeating().to(Direction::ToBottomRight).stop(hex("#1e1e24"), pct(0)).stop(hex("#1e1e24"), px(10)).stop(hex("#312e81"), px(10)).stop(hex("#312e81"), px(20)).build()))
                    ],
                ))
            )),
            DemoCard().children(view_chain!(
                h4("3. Responsive & Nested (Style Builder)"),
                p("The enhanced `sty()` API now supports `@media` and complex nesting, just like the `styled!` macro.").style("margin-bottom: 16px; font-size: 0.9em; opacity: 0.7;"),
                div![
                    span("Resize window and hover child!").style(
                        sty()
                            .display(DisplayKeyword::Block)
                            .padding(px(32))
                            .background(hex("#1e1e24"))
                            .border(border(px(1), BorderStyleKeyword::Solid, hex("#374151")))
                            .border_radius(px(16))
                            .transition("all 0.3s")
                            .nest("& > .child-box", |s| s
                                .width(px(60))
                                .height(px(60))
                                .background(hex("#6366f1"))
                                .border_radius(px(8))
                                .transition("all 0.4s cubic-bezier(0.68, -0.55, 0.265, 1.55)")
                            )
                            .on_hover(|s| s
                                .border_color(hex("#6366f1"))
                                .nest("& > .child-box", |s| s
                                    .transform(transform().translate_x(px(100)).rotate(deg(180)))
                                    .background(hex("#a855f7"))
                                )
                            )
                            .media("@media (max-width: 768px)", |s| s
                                .background(hex("#312e81"))
                                .nest("& > .child-box", |s| s
                                    .background(hex("#f43f5e"))
                                )
                            )
                    ),
                    div("I am the child box").class("child-box").style("margin-top: 16px; color: #fff; font-size: 12px; text-align: center; line-height: 60px;")
                ].style("position: relative;")
            )),
            DemoCard().children(view_chain!(
                h4("4. Complex DSLs (Grid Areas & Font Variations)"),
                p("Specialized support for complex grid layouts and variable fonts.").style("margin-bottom: 24px; color: #9ca3af;"),
                Stack().gap(24).children(view_chain!(
                    div![
                        span("Grid Template Areas").style("margin-bottom: 8px; display: block; font-size: 0.9em; opacity: 0.7;"),
                        div![
                            div("Header").style(sty().grid_area("header").background(hex("#4f46e5")).padding(px(8))),
                            div("Main").style(sty().grid_area("main").background(hex("#312e81")).padding(px(24))),
                            div("Sidebar").style(sty().grid_area("sidebar").background(hex("#1e1e24")).padding(px(8))),
                        ].style(
                            sty()
                                .display(DisplayKeyword::Grid)
                                .gap(px(8))
                                .grid_template_areas(grid_template_areas(["header header", "main sidebar"]))
                                .grid_template_columns("2fr 1fr")
                        )
                    ],
                    div![
                        span("Font Variation Settings").style("margin-bottom: 8px; display: block; font-size: 0.9em; opacity: 0.7;"),
                        div("Variable Font Styling (Weight: 700, Ital: 0.5)")
                            .style(
                                sty()
                                    .font_size(px(24))
                                    .font_variation_settings(font_variation_settings([("wght", 700.0), ("ital", 0.5)]))
                            )
                    ]
                ))
            ))
        ))
    ]
}
