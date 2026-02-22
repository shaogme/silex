use silex::prelude::*;

// --- Theme Definition ---
define_theme! {
    pub struct AppTheme {
        pub primary: Hex,
        pub secondary: Hex,
        pub surface: Hex,
        pub text: Hex,
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
        radius: px(12),
    }
}

pub fn default_dark_theme() -> AppTheme {
    AppTheme {
        primary: hex("#818cf8"),
        secondary: hex("#c084fc"),
        surface: hex("#111827"),
        text: hex("#f9fafb"),
        radius: px(12),
    }
}

// --- Styled Components ---

styled! {
    pub DemoCard<div>(children: Children) {
        background: rgba(30, 30, 35, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.08);
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
        opacity: $(move || if active.get() { 1.0 } else { 0.8 });

        &:hover {
            filter: brightness(1.1);
            transform: translateY(-1px);
        }

        &:active {
            transform: translateY(0);
        }
    }
}

// --- Demo Components ---

#[component]
pub fn SelectStylesPage() -> impl View {
    div("Select a CSS demo above to see the power of Silex styling.")
}

#[component]
pub fn CssInRustDemo() -> impl View {
    let (color, set_color) = signal(hex("#ffffff"));
    let (size, set_size) = signal("medium".to_string());
    let (hover_color, set_hover_color) = signal(hex("#4f46e5"));
    let (pseudo_state, set_pseudo_state) = signal("hover".to_string());
    let (border_state, set_border_state) =
        signal(border(px(2), BorderStyleKeyword::Solid, hex("transparent")));
    let (padding_state, set_padding_state) = signal(padding::x_y(px(12), px(24)));

    div![
        div![
            h2("✨ CSS-in-Rust"),
            p("Experience the power of scoped, type-safe, and reactive styling in pure Rust.")
                .style("opacity: 0.7; font-size: 1.1em;"),
        ].style("margin-bottom: 40px;"),

        DemoCard().children((
            h3("Atomic & Scoped Styles"),
            p(
                "The button below demonstrates the `styled!` macro with dynamic interpolation, variants, and factory functions."
            ).style("margin-bottom: 24px; color: #9ca3af;"),
            StyledButton()
                .children("Interactive Button")
                .color(color)
                .size(size)
                .hover_color(hover_color)
                .pseudo_state(pseudo_state)
                .border_style(border_state)
                .padding_val(padding_state)
                .on(event::click, move |_| {
                    set_color.update(|c| {
                        *c = if c.0 == "#ffffff" {
                            hex("#fbbf24") // Amber 400
                        } else {
                            hex("#ffffff")
                        }
                    });
                    set_size.update(|s| {
                        *s = if *s == "medium" {
                            "large".to_string()
                        } else {
                            "medium".to_string()
                        }
                    });
                    set_border_state.update(|b| {
                        *b = border(px(2), BorderStyleKeyword::Dashed, hex("#f472b6"));
                    });
                    set_padding_state.update(|p| {
                        *p = padding::x_y(px(16), px(32));
                    });
                    set_hover_color.update(|c| {
                        *c = if c.0 == "#4f46e5" {
                            hex("#ec4899") // Pink 500
                        } else {
                            hex("#4f46e5")
                        }
                    });
                    set_pseudo_state.update(|s| {
                        *s = if *s == "hover" {
                            "active".to_string()
                        } else {
                            "hover".to_string()
                        }
                    });
                    console_log("Styles and dynamic rules updated!");
                }),
        )),

        DemoCard().children((
            h3("Type-Safe Style Builder"),
            p(
                "A chainable, type-safe API for defining styles without macros, supporting full reactivity."
            ).style("margin-bottom: 24px; color: #9ca3af;"),
            div![
                span("Hover to Reveal Effects").style(
                    Style::new()
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
                                .transform("scale(1.05) rotate(1deg)")
                        })
                )
            ],
            p("Signals are natively supported in the builder:").style("margin: 20px 0 10px; font-size: 0.9em; opacity: 0.6;"),
            {
                let (count, set_count) = signal(0);
                div![
                    button("Grow").on(event::click, move |_| set_count.update(|n| *n += 1))
                        .style("padding: 8px 16px; border-radius: 6px; border: 1px solid #374151; background: #111827; color: white; cursor: pointer;"),
                    div(move || format!("Reactive Width: {}px", 180 + count.get() * 30)).style(
                        sty()
                            .width(move || px(180 + count.get() * 30))
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

        DemoCard().children((
            h3("Layout Primitives"),
            p("Using the brand new Stack and Grid components for structural layout.")
                .style("margin-bottom: 24px; color: #9ca3af;"),

                Stack()
                .gap(16)
                .children((
                    span("Vertical Stack with Gap"),
                    Grid()
                        .columns(3)
                        .gap(12)
                        .children((
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
pub fn ThemeDemo() -> impl View {
    let (theme, set_theme) = signal(default_light_theme());
    let is_dark = theme.map(|t| t.surface.0 == "#111827");

    div![
        h3("🎨 Real-time Theme Engine"),
        p("Define CSS variables via Rust structs and propagate them through the component tree.")
            .style("color: #6b7280; margin-bottom: 24px;"),
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
                        .background_color(move || if !is_dark.get() {
                            hex("#6366f1")
                        } else {
                            hex("#f3f4f6")
                        })
                        .color(move || if !is_dark.get() {
                            hex("#ffffff")
                        } else {
                            hex("#374151")
                        })
                        .border(move || if !is_dark.get() {
                            border(px(1), BorderStyleKeyword::Solid, hex("#6366f1"))
                        } else {
                            border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db"))
                        })
                ),
            button("🌙 Dark Mode")
                .on(event::click, move |_| set_theme.set(default_dark_theme()))
                .style(
                    sty()
                        .padding(padding::x_y(px(8), px(16)))
                        .border_radius(px(6))
                        .cursor(CursorKeyword::Pointer)
                        .transition("all 0.2s")
                        .background_color(move || if is_dark.get() {
                            hex("#4f46e5")
                        } else {
                            hex("#f3f4f6")
                        })
                        .color(move || if is_dark.get() {
                            hex("#ffffff")
                        } else {
                            hex("#374151")
                        })
                        .border(move || if is_dark.get() {
                            border(px(1), BorderStyleKeyword::Solid, hex("#4f46e5"))
                        } else {
                            border(px(1), BorderStyleKeyword::Solid, hex("#d1d5db"))
                        })
                ),
        ],
        // Use apply to inject theme variables
        ThemePreviewCard().apply(theme_variables(theme)).children((
            h4("Dynamic Component Style"),
            p("These styles are reacting to the Rust-defined theme object via CSS variables."),
            ThemeButton().children("Themed Button").active(false)
        ))
    ]
    .style("padding: 24px; border: 1px solid #e5e7eb; border-radius: 12px; background: #f9fafb;")
}

#[component]
pub fn LayoutFriendlyThemeDemo() -> impl View {
    let (theme, _) = signal(default_dark_theme());

    div![
        h3("🏗️ Layout Friendly Theme Provider"),
        p("Testing ThemeProvider with 'display: contents' and manual variable injection."),

        DemoCard().children((
            h4("1. ThemeProvider inside Flex (Column)"),
            p("The red border is around the Stack. The ThemeProvider should NOT break the Flex layout flow."),
            Stack().style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#ef4444"))).padding(px(8))).children((
                div("Item 1 (Inside ThemeVariable context)").style("background: #1e1e24; padding: 10px; margin: 4px; border-radius: 4px; border: 1px solid $theme.primary;")
                    .apply(theme_variables(theme)),
                div("Item 2 (Inside ThemeVariable context)").style("background: #1e1e24; padding: 10px; margin: 4px; border-radius: 4px; border: 1px solid $theme.secondary;")
                    .apply(theme_variables(theme)),
                div("Item 3 (Outside ThemeProvider)").style("background: #1e1e24; padding: 10px; margin: 4px; border-radius: 4px;"),
            ))
        )),

        DemoCard().children((
            h4("2. Manual Variable Injection (theme_variables)"),
            p("Injecting theme variables directly into a div without an extra wrapper."),
            div![
                "I have theme colors applied directly!",
                div("Sub-element using $theme.primary").style("color: $theme.primary; font-weight: bold;")
            ]
            .apply(theme_variables(theme))
            .style("padding: 20px; border: 2px dashed $theme.secondary; border-radius: 12px;")
        )),

        DemoCard().children((
            h4("3. Stack Layout Continuity (Issue #1 Test)"),
            p("Stack as a child of another layout should maintain Flow correctly without wrapper div breaks."),
            Stack().style(sty().border(border(px(2), BorderStyleKeyword::Solid, hex("#3b82f6"))).padding(px(8))).children((
                Stack().gap(4).apply(theme_variables(theme)).children((
                    div("Nested Stack Item 1").style("background: $theme.surface; color: $theme.text; padding: 10px; border-radius: 4px; border: 1px solid $theme.primary;"),
                    div("Nested Stack Item 2").style("background: $theme.surface; color: $theme.text; padding: 10px; border-radius: 4px; border: 1px solid $theme.secondary;"),
                )),
                div("Direct Sibling of Nested Stack").style("background: #1e1e24; color: #fff; padding: 10px; margin-top: 4px; border-radius: 4px;"),
            ))
        ))
    ]
}

#[component]
pub fn BuilderDemo() -> impl View {
    div(
        div(
            (
                h3("Builder Style"),
                p("This component is built using only function calls and method chaining."),
                button("Click Me (Builder)")
                    .class("btn-builder")
                    .style("background-color: #e0f7fa; color: #006064; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;")
                    .on(event::click, |_| console_log("Builder button clicked!")),
            )
        )
        .style("padding: 20px; border: 1px solid #b2ebf2; border-radius: 8px; margin-bottom: 20px;")
    )
}

#[component]
pub fn MacroDemo() -> impl View {
    let (count, set_count) = signal(0);

    div![
        h3![ "Macro Style" ],
        p![ "This component uses macros for a more declarative, HTML-like feel." ],
        div![
            button![ "-" ]
                .class("btn-macro")
                .on(event::click, set_count.updater(|n| *n -= 1)),
            span![ " Count: ", count, " " ].style("margin: 0 10px; font-weight: bold;"),
            button![ "+" ]
                .class("btn-macro")
                .on(event::click, set_count.updater(|n| *n += 1)),
        ]
        .style("display: flex; align-items: center; margin-top: 10px;")
    ]
    .style("padding: 20px; border: 1px solid #ffccbc; background-color: #fffbe6; border-radius: 8px; margin-bottom: 20px;")
}

#[component]
pub fn HybridDemo() -> impl View {
    let (is_active, set_active) = signal(false);

    div![
        h3("Hybrid Style (Recommended)"),
        p("Mix macros for structure and builder methods for attributes."),

        div![
            span(is_active.map(|v| if *v { "State: Active" } else { "State: Inactive" }))
                .style("margin-right: 15px;"),

            button(is_active.map(|v| if *v { "Deactivate" } else { "Activate" }))
                .on(event::click, move |_| set_active.update(|v| *v = !*v))
                .style(sty()
                    .background_color(move || if is_active.get() { hex("#ef5350") } else { hex("#66bb6a") })
                    .color(hex("#ffffff"))
                    .padding(padding::x_y(px(8), px(16)))
                    .border(border(px(0), BorderStyleKeyword::None, hex("transparent")))
                    .border_radius(px(4))
                    .cursor(CursorKeyword::Pointer)
                    .transition("background-color 0.2s")
                )
        ]
    ]
    .style("padding: 20px; border: 1px solid #d1c4e9; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
}
