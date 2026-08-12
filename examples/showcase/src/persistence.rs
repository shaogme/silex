use crate::css::AppTheme;
use silex::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Settings {
    volume: u32,
    username: String,
    auto_save: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 80,
            username: "Default User".to_string(),
            auto_save: true,
        }
    }
}

#[component]
pub fn PersistencePage<'scope>(
    ctx: RouterContext<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();
    Ok(div![
        h2("Comprehensive Persistence Demo")
            .style(sty().color(AppTheme::PRIMARY)?.margin_bottom(px(10))?),
        p("This page demonstrates the full spectrum of Silex's persistence system, from basic LocalStorage to advanced debouncing and manual control."),

        div![
            // 1. Storage Backends Comparison
            BackendGrid(ctx, error_handler).build(),

            // 2. Manual Control & Flash
            ManualFlushDemo(scope, error_handler).build(),

            // 3. Debounced Persistence
            DebounceDemo(scope, error_handler).build(),

            // 4. Error Handling & JSON
            ErrorHandlingDemo(scope, error_handler).build(),
        ].style(sty().display("flex")?.flex_direction(FlexDirectionKeyword::Column)?.gap(px(30))?.margin_top(px(20))?)
    ]
    .style(sty().max_width(px(1000))?.margin("0 auto")?.padding("20px")?))
}

#[component]
fn Card<'scope>(
    children: AnyView<'scope>,
    #[chain] title: &'static str,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div![
        h3(title).style(
            sty()
                .margin_top(px(0))?
                .border_bottom(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                .padding_bottom(px(10))?
                .color(AppTheme::PRIMARY)?
        ),
        children
    ]
    .style(
        sty()
            .background(AppTheme::SURFACE)?
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
            .padding(px(24))?
            .border_radius(px(12))?
            .box_shadow("0 4px 12px rgba(0,0,0,0.08)")?
            .transition("transform 0.2s, box_shadow 0.2s")?,
    ))
}

#[component]
fn BackendGrid<'scope>(
    ctx: RouterContext<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();
    let local = Persistent::builder(scope, "demo-local", error_handler)
        .local()
        .string()
        .default("Stored in LocalStorage".to_string())
        .build()?;

    let session = Persistent::builder(scope, "demo-session", error_handler)
        .session()
        .string()
        .default("Stored in SessionStorage".to_string())
        .build()?;

    let query = Persistent::builder(scope, "demo-query", error_handler)
        .query(ctx)
        .string()
        .default("Stored in URL Query".to_string())
        .build()?;

    Ok(Card(chain!(
        p("Different storage areas serving different lifetimes and visibility needs."),
        div![
            div![
                label("LocalStorage").style(
                    sty()
                        .display("block")?
                        .font_weight(FontWeightKeyword::Bold)?
                        .margin_bottom(px(5))?
                ),
                input().bind_value(local).style(
                    sty()
                        .width(pct(100))?
                        .padding(px(8))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .border_radius(px(4))?
                        .background(AppTheme::SURFACE_ALT)?
                        .color(AppTheme::TEXT)?
                ),
                small("Persistent cross-sessions & tabs.")
                    .style(sty().display("block")?.margin_top(px(5))?.opacity(0.7)?)
            ],
            div![
                label("SessionStorage").style(
                    sty()
                        .display("block")?
                        .font_weight(FontWeightKeyword::Bold)?
                        .margin_bottom(px(5))?
                ),
                input().bind_value(session).style(
                    sty()
                        .width(pct(100))?
                        .padding(px(8))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .border_radius(px(4))?
                        .background(AppTheme::SURFACE_ALT)?
                        .color(AppTheme::TEXT)?
                ),
                small("Scoped to this tab/window.")
                    .style(sty().display("block")?.margin_top(px(5))?.opacity(0.7)?)
            ],
            div![
                label("URL Query").style(
                    sty()
                        .display("block")?
                        .font_weight(FontWeightKeyword::Bold)?
                        .margin_bottom(px(5))?
                ),
                input().bind_value(query).style(
                    sty()
                        .width(pct(100))?
                        .padding(px(8))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .border_radius(px(4))?
                        .background(AppTheme::SURFACE_ALT)?
                        .color(AppTheme::TEXT)?
                ),
                small("Synced to browser address bar.")
                    .style(sty().display("block")?.margin_top(px(5))?.opacity(0.7)?)
            ],
        ]
        .style(
            sty()
                .display("grid")?
                .grid_template_columns("repeat(auto-fit, minmax(280px, 1fr))")?
                .gap(px(20))?
        )
    ))
    .error_handler(error_handler)
    .title("1. Backends Comparison")
    .build())
}

#[component]
fn ManualFlushDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let draft = Persistent::builder(scope, "demo-draft", error_handler)
        .local()
        .string()
        .mode(PersistMode::Manual)
        .default(String::new())
        .build()?;

    Ok(Card(chain!(
        p("Sometimes you don't want every keystroke saved. Use Manual mode for 'Save' button behavior."),
        div![
            textarea("")
                .bind_value(draft)
                .placeholder("Type a long message here...")
                .style(sty().width(pct(100))?.height(px(120))?.padding(px(12))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(8))?.background(AppTheme::SURFACE_ALT)?.color(AppTheme::TEXT)?.resize(ResizeKeyword::Vertical)?),
            div![
                button("💾 Save to Storage")
                    .on(event::click, move |_| {
                        draft.flush()?;
                        Ok(())
                    })
                    .style(sty().background(AppTheme::PRIMARY)?.color(ColorName::White)?.border(NONE)?.padding(padding::block_inline(px(8), px(16)))?.border_radius(px(6))?.cursor(CursorKeyword::Pointer)?.transition("opacity 0.2s")?),
                button("🔄 Reload from Storage")
                    .on(event::click, move |_| {
                        draft.reload()?;
                        Ok(())
                    })
                    .style(sty().background(AppTheme::SURFACE)?.color(AppTheme::TEXT)?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.padding(padding::block_inline(px(8), px(16)))?.border_radius(px(6))?.cursor(CursorKeyword::Pointer)?),
                button("🗑️ Forget")
                    .on(event::click, move |_| {
                        draft.remove()?;
                        Ok(())
                    })
                    .style(sty().background("transparent")?.color(AppTheme::ERROR)?.border("1px solid currentColor")?.padding("8px 16px")?.border_radius(px(6))?.cursor("pointer")?.margin_left(AUTO)?),
            ].style(sty().display("flex")?.gap(px(10))?.margin_top(px(10))?),
            p![
                "Memory Status: ",
                move || -> SilexResult<AnyView<'scope>> {
                    Ok(match draft.state().get()? {
                        PersistenceState::Ready(_) => span("✓ Clean (Synced)").style(sty().color(hex("#4caf50"))?.font_weight(FontWeightKeyword::Bold)?).into_any(),
                        _ => span("✎ Dirty (Unsaved Changes)").style(sty().color(hex("#ff9800"))?.font_weight(FontWeightKeyword::Bold)?).into_any()
                    })
                }
            ].style(sty().margin_top(px(15))?.font_size(em_unit(0.9))?)
        ]
    )).error_handler(error_handler).title("2. Manual Persistence (Draft Mode)").build())
}

#[component]
fn DebounceDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let debounced = Persistent::builder(scope, "demo-debounced", error_handler)
        .local()
        .string()
        .sync(SyncStrategy::Debounce(std::time::Duration::from_millis(
            1500,
        )))
        .default(String::new())
        .build()?;

    Ok(Card(chain!(
        p("Optimizes performance by delaying the write operation until 1.5s after the last change."),
        div![
            input()
                .bind_value(debounced)
                .placeholder("Type quickly...")
                .style(sty().width(pct(100))?.padding(px(12))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(6))?.background(AppTheme::SURFACE_ALT)?.color(AppTheme::TEXT)?.font_size(em_unit(1.1))?),

            div![
                h4("Live Sync Tracking:").style(sty().margin_bottom(px(5))?),
                move || -> SilexResult<AnyView<'scope>> {
                    let state = debounced.state().get()?;
                    let (status, content) = match &state {
                        PersistenceState::Ready(raw) => ("Ready", raw),
                        PersistenceState::Dirty(raw) => ("Dirty (Modified)", raw),
                        PersistenceState::Syncing(raw) => ("Syncing...", raw),
                        PersistenceState::WriteError(err) => ("Write Error", err),
                        PersistenceState::ReadError(err) => ("Read Error", err),
                        PersistenceState::Unavailable => ("Unavailable", &"N/A".to_string()),
                        PersistenceState::DecodeError(_) => ("Decode Error", &"Invalid data".to_string()),
                    };

                    Ok(div![
                        span(format!("Status: {}", status)).style(sty().font_weight(FontWeightKeyword::Bold)?.margin_right(px(10))?),
                        span(format!("Raw Content: \"{}\"", content)).style(sty().opacity(0.7)?.font_size(em_unit(0.9))?)
                    ]
                    .style(match state {
                         PersistenceState::Ready(_) => "color: #4caf50; border-left: 3px solid #4caf50; padding-left: 10px;",
                         PersistenceState::Dirty(_) => "color: #ff9800; border-left: 3px solid #ff9800; padding-left: 10px;",
                         PersistenceState::Syncing(_) => "color: #2196f3; border-left: 3px solid #2196f3; padding-left: 10px;",
                         _ => "color: #f44336; border-left: 3px solid #f44336; padding-left: 10px;"
                    })
                    .into_any())
                }
            ].style(sty().margin_top(px(15))?.background("rgba(0,0,0,0.05)")?.padding("12px")?.border_radius(px(6))?.font_family("monospace")?)
        ]
    )).error_handler(error_handler).title("3. Debounced Syncing").build())
}

#[component]
fn ErrorHandlingDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let settings = Persistent::builder(scope, "demo-complex-settings", error_handler)
        .local()
        .json::<Settings>()
        .on_decode_error(DecodePolicy::UseDefault)
        .default(Settings::default())
        .build()?;

    Ok(Card(chain!(
        p("Using JSON codec for complex types with built-in error recovery policies."),
        div![
            div![
                label("Username").style(sty().display("block")?.margin_bottom(px(5))?),
                input()
                    .prop(
                        "value",
                        settings.map(scope, |s| s.username.clone(), error_handler)?
                    )
                    .on(event::input, move |e| {
                        settings.update(|s| s.username = event_target_value(&e))?;
                        Ok(())
                    })
                    .style(
                        sty()
                            .width(pct(100))?
                            .padding(px(8))?
                            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                            .border_radius(px(4))?
                            .background(AppTheme::SURFACE_ALT)?
                            .color(AppTheme::TEXT)?
                    )
            ],
            div![
                label(rx!(scope; error_handler; format!("Volume Level: {}%", $settings.volume)))
                    .style(
                        sty()
                            .display("block")?
                            .margin_top(px(15))?
                            .margin_bottom(px(5))?
                    ),
                input()
                    .attr("type", "range")
                    .attr("min", "0")
                    .attr("max", "100")
                    .prop("value", settings.map(scope, |s| s.volume, error_handler)?)
                    .on(event::input, move |e| {
                        if let Ok(v) = event_target_value(&e).parse::<u32>() {
                            settings.update(|s| s.volume = v)?;
                        }
                        Ok(())
                    })
                    .style(sty().width(pct(100))?.accent_color(AppTheme::PRIMARY)?)
            ],
        ],
        div![
            h4("Health Check").style(sty().margin_bottom(px(10))?),
            move || {
                Ok(match settings.state().get()? {
                    PersistenceState::DecodeError(err) => div![
                        p("⚠️ Decode Error detected!").style(
                            sty()
                                .color(hex("#f44336"))?
                                .font_weight(FontWeightKeyword::Bold)?
                        ),
                        pre(format!("Raw Content: {}\nReason: {}", err.raw, err.message)).style(
                            sty()
                                .background("#fff0f0")?
                                .color(hex("#b71c1c"))?
                                .padding("12px")?
                                .border_radius(px(4))?
                                .font_size(em_unit(0.85))?
                                .overflow("auto")?
                                .border("1px solid #ffcdd2")?
                        )
                    ]
                    .into_any(),
                    _ => p("✅ Ready: Backend content is valid JSON.")
                        .style(sty().color(hex("#4caf50"))?)
                        .into_any(),
                })
            },
            button("Reset to Factory Defaults")
                .on(event::click, move |_| {
                    settings.reset()?;
                    Ok(())
                })
                .style(
                    sty()
                        .margin_top(px(15))?
                        .background(ColorKeyword::Transparent)?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .padding(padding::block_inline(px(6), px(12)))?
                        .border_radius(px(4))?
                        .cursor(CursorKeyword::Pointer)?
                        .color(AppTheme::TEXT)?
                )
        ]
        .style(
            sty()
                .margin_top(px(25))?
                .padding(px(15))?
                .background(AppTheme::SURFACE_ALT)?
                .border_radius(px(8))?
                .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
        )
    ))
    .error_handler(error_handler)
    .title("4. Error Handling & JSON")
    .build())
}
