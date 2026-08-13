use std::borrow::Cow;

use crate::css::AppTheme;
use silex::core::log::console_log;
use silex::prelude::*;
use silex::reexports::web_sys;

// --- Store Definition ---
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[store]
pub struct UserSettings<'s> {
    pub theme: String,
    pub notifications: bool,
    pub username: Cow<'s, str>,
}

#[component]
pub fn StoreDemo<'scope>(
    scope: Scope<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div![
        h3("Global Store Demo"),
        div![
            p![strong("Username: "), settings.username],
            p![strong("Theme: "), settings.theme],
            p![
                strong("Notifications: "),
                text(settings.notifications.map_fn(
                    scope,
                    |n| if *n { "On" } else { "Off" },
                    error_handler
                )?),
            ],
        ]
        .style(
            sty()
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                .background(AppTheme::SURFACE)?
                .padding(px(10))?
                .margin_bottom(px(10))?
        ),
        h4("Update Settings"),
        div![
            button("Toggle Theme").on(
                event::click,
                settings.theme.updater(|t| {
                    *t = if t == "Light" {
                        "Dark".to_string()
                    } else {
                        "Light".to_string()
                    }
                })
            ),
            button("Toggle Notifications")
                .on(event::click, settings.notifications.updater(|n| *n = !*n)),
            input()
                .bind_value(settings.username)
                .placeholder("Change username..."),
        ]
        .style(sty().display("flex")?.gap(px(10))?),
    ])
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ComplexState {
    name: String,
    level: i32,
    inventory: Vec<String>,
}

impl Default for ComplexState {
    fn default() -> Self {
        Self {
            name: "New Hero".to_string(),
            level: 1,
            inventory: vec!["Wooden Sword".to_string()],
        }
    }
}

#[component]
pub fn JsonStorageDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let state = Persistent::builder(scope, "showcase-json-state", error_handler)
        .local()
        .json::<ComplexState>()
        .default(ComplexState::default())
        .build()?;

    Ok(div![
        h4("Native JSON Persistence Demo"),
        p(
            "This demo uses the JSON codec to persist a complex struct via browser-native `JSON.stringify/parse`."
        ),
        div![
            p![
                strong("Hero: "),
                rx!(scope; error_handler; $state.name.clone())
            ],
            p![
                strong("Level: "),
                rx!(scope; error_handler; $state.level.to_string())
            ],
            p![
                strong("Inventory: "),
                rx!(scope; error_handler; $state.inventory.join(", "))
            ],
        ]
        .style(
            sty()
                .background(AppTheme::SURFACE_ALT)?
                .padding(px(10))?
                .border_left(border(px(4), BorderStyleKeyword::Solid, AppTheme::PRIMARY))?
                .border_radius(px(4))?
                .margin_bottom(px(10))?
        ),
        div![
            button("Level Up").on(event::click, move |_| {
                state.update(|s| s.level += 1).map_err(Into::into)
            }),
            button("Add Shield").on(event::click, move |_| {
                state
                    .update(|s| {
                        if !s.inventory.contains(&"Shield".to_string()) {
                            s.inventory.push("Shield".to_string());
                        }
                    })
                    .map_err(Into::into)
            }),
            button("Reset").on(event::click, move |_| {
                state.set(ComplexState::default()).map_err(Into::into)
            }),
        ]
        .style(sty().display("flex")?.gap(px(10))?),
    ])
}

#[component]
pub fn StorageDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let count = Persistent::builder(scope, "showcase-counter", error_handler)
        .local()
        .parse::<i32>()
        .default(0)
        .build()?;

    Ok(div![
        h3("LocalStorage Persistence"),
        p("Silex provides a unified persistence abstraction. Basic types use string and parse codecs, while complex structures use the JSON codec."),

        // 1. 基本类型持久化
        div![
            h4("Basic Type Persistence (No Serde needed)"),
            div![
                button("-1").on(event::click, count.updater(|c| *c -= 1)),
                span(count).style(sty().font_size(em_unit(1.5))?.font_weight(FontWeightKeyword::Bold)?.min_width(px(50))?.text_align(TextAlignKeyword::Center)?),
                button("+1").on(event::click, count.updater(|c| *c += 1)),
            ]
            .style(sty().display("flex")?.gap(px(20))?.align_items("center")?.margin("15px 0")?),
        ].style(sty().padding(px(15))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(4))?.margin_bottom(px(20))?),

        // 2. 复杂类型持久化
        JsonStorageDemo(scope, error_handler).build(),

        p![
            "Try opening this page in ",
            strong("another tab"),
            " and watch them sync in real-time!"
        ]
    ]
    .style(sty().padding(px(20))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(8))?.background(AppTheme::SURFACE)?.transition("all 0.3s")?))
}

#[component]
pub fn QueryDemo<'scope>(
    ctx: RouterContext<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();
    let val = Persistent::builder(scope, "demo_val", error_handler)
        .query(ctx)
        .cow()
        .default("".into())
        .build()?;

    let page = div![
        h3("Query Signal Demo"),
        p(
            "This input is synced with the URL query parameter 'demo_val' using `Persistent::builder(...).query()`."
        ),
        div![
            input()
                .bind_value(val) // Automatic two-way binding
                .placeholder("Type here...")
                .style(
                    sty()
                        .padding(px(8))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .border_radius(px(4))?
                        .background(AppTheme::SURFACE)?
                        .color(AppTheme::TEXT)?
                ),
            button("Reset")
                .on(event::click, val.setter("".into()))
                .style(sty().padding("8px 16px")?.cursor("pointer")?)
        ]
        .style(
            sty()
                .display("flex")?
                .gap(px(10))?
                .margin("10px 0")?
                .align_items("center")?
        ),
        p![strong("Current Value: "), val].style(
            sty()
                .background(AppTheme::SURFACE)?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                .padding(px(10))?
                .border_radius(px(4))?
        )
    ];

    Ok(AuthGuard(scope, settings, page.into_any(), error_handler).build())
}

#[component]
pub fn AuthGuard<'scope>(
    scope: Scope<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
    children: AnyView<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let children = children.clone();

    Ok(rx!(scope; error_handler;
        if $(settings.username) != "Guest" {
            children.clone()
        } else {
            div![
                h3("🔒 Restricted Access"),
                p("This content is protected. Please go to 'Store Demo' and change your username to something other than 'Guest'."),
            ].style(sty().padding("20px")?.background("#fff0f0")?.border("1px solid #ffcccc")?.color(hex("#cc0000"))?)
            .into_any()
        }
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct UserProfile {
    id: i32,
    name: String,
    role: String,
}

async fn mock_fetch_user(id: i32) -> Result<UserProfile, String> {
    // Simulate network delay
    gloo_timers::future::TimeoutFuture::new(1000).await;

    if id < 0 {
        return Err("Invalid User ID".to_string());
    }

    Ok(UserProfile {
        id,
        name: format!("User {}", id),
        role: if id == 1 {
            "Admin".to_string()
        } else {
            "Member".to_string()
        },
    })
}

#[component]
pub fn ResourceDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let (user_id, set_user_id) = scope.signal(1)?;

    // Create Resource: triggers when user_id changes
    let user_resource = Resource::new(scope, user_id, mock_fetch_user, None, error_handler)?;

    Ok(div![
        h3("Resource & Optimistic UI"),
        p("Fetches user data with a 1s delay. You can optimistically update the name before the server responds."),

        div![
            button("User 1").on(event::click, set_user_id.setter(1)),
            button("User 2").on(event::click, set_user_id.setter(2)),
            button("Invalid User").on(event::click, set_user_id.setter(-1)),
            button("Refetch").on(event::click, move |_| {
                user_resource
                    .refetch()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                Ok(())
            }),
        ].style(sty().display("flex")?.gap(px(10))?.margin_bottom(px(15))?),

        div![
            "Status: ",
            // Show loading state using the new state enum helper
            move || {
                let state = user_resource.state.get()?;
                let view = if state.is_loading() {
                    span(if let ResourceState::Reloading(_) = state { "Reloading..." } else { "Loading..." }).style(sty().color(ColorName::Orange)?)
                } else {
                    span("Idle").style(sty().color(ColorName::Green)?)
                };
                Ok(view.into_any())
            }
        ].style(sty().margin_bottom(px(10))?.font_weight(FontWeightKeyword::Bold)?),

        // Display Data using get_data() which covers both Ready and Reloading
        move || {
            Ok(match user_resource.get_data()? {
                Some(user) => div![
                    div(format!("ID: {}", user.id)),
                    div(format!("Name: {}", user.name)),
                    div(format!("Role: {}", user.role)),

                    // Optimistic Update Controls
                    div![
                        h4("Optimistic Updates (Local Cache)"),
                        button("Rename to 'Modified'")
                            .on(event::click, move |_| {
                                // Manually update the local resource data
                                user_resource.update(|u| {
                                    u.name = "Modified Name".to_string();
                                })
                                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            }),
                    ].style(sty().margin_top(px(15))?.border_top("1px solid #eee")?.padding_top(px(10))?)
                ]
                .into_any(),
                None => div("No Data (or Loading...)").into_any(),
            })
        },

        // Error Handling via state matching
        move || {
            if let ResourceState::Error(err) = user_resource.state.get()? {
                Ok(div(format!("Error: {}", err))
                    .style(sty().color(ColorName::Red)?.margin_top(px(10))?)
                    .into_any())
            } else {
                Ok(div("").into_any())
            }
        }
    ]
    .style(sty().padding(px(20))?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(8))?.background(AppTheme::SURFACE)?.transition("all 0.3s")?))
}

#[component]
pub fn MutationDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    // Simulate a login mutation
    // Takes (username, password) and returns a Result<String, String> token
    let login_mutation = Mutation::new(
        scope,
        |(user, pass): (String, String)| async move {
            console_log(format!("Logging in as {}...", user));
            gloo_timers::future::TimeoutFuture::new(1500).await;

            if user == "admin" && pass == "password" {
                Ok("fake_jwt_token_12345".to_string())
            } else {
                Err("Invalid credentials".to_string())
            }
        },
        error_handler,
    )?;

    let username = scope.rw_signal("".to_string())?;
    let password = scope.rw_signal("".to_string())?;
    let login_error_style = sty().color(ColorName::Red)?;
    let login_success_style = sty()
        .color(ColorName::Green)?
        .font_weight(FontWeightKeyword::Bold)?;
    let login_token_style = sty()
        .font_family("monospace")?
        .background("#eee")?
        .padding("5px")?;

    Ok(div![
        h3("Mutation Demo (Async Write)"),
        p("Enter 'admin' / 'password' to succeed, others to fail."),
        div![
            input()
                .bind_value(username)
                .placeholder("Username")
                .style(sty().margin_right(px(10))?.padding("5px")?),
            input()
                .bind_value(password)
                .attr("type", "password")
                .placeholder("Password")
                .style(sty().margin_right(px(10))?.padding("5px")?),
            button("Login")
                .attr("type", "button") // Prevent accidental form submission
                .on(event::click, move |e: web_sys::MouseEvent| {
                    e.prevent_default();

                    // Note: "login_mutation.mutate((username.get(), password.get()));" is the same as "login_mutation.mutate_with((username, password));"
                    login_mutation.mutate((username.get()?, password.get()?))?;
                    Ok(())
                })
                .attr(
                    "disabled",
                    rx!(scope; error_handler; login_mutation.loading()?)
                )
                .style(sty().padding("5px 10px")?),
        ]
        .style(sty().margin_bottom(px(10))?),
        // Loading State
        move || {
            if login_mutation.loading()? {
                Ok(div("Logging in...")
                    .style(sty().color(ColorName::Blue)?)
                    .into_any())
            } else {
                Ok(div("").into_any())
            }
        },
        move || {
            Ok(login_mutation
                .error()?
                .map(|err| div(format!("Error: {}", err)).style(login_error_style.clone()))
                .map(|view| view.into_any())
                .unwrap_or_else(|| div("").into_any()))
        },
        move || {
            Ok(login_mutation
                .value()?
                .map(|token| {
                    div![
                        div("Login Successful!").style(login_success_style.clone()),
                        div(format!("Token: {}", token)).style(login_token_style.clone()),
                    ]
                })
                .map(|view| view.into_any())
                .unwrap_or_else(|| div("").into_any()))
        }
    ]
    .style(
        sty()
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
            .border_radius(px(8))?
            .background(AppTheme::SURFACE)?
            .transition("all 0.3s")?,
    ))
}

#[component]
pub fn SuspenseDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    use silex::components::SuspenseMode;

    let (show_content, set_show_content) = scope.signal(false)?;
    let (mode, set_mode) = scope.signal(SuspenseMode::KeepAlive)?;

    // Trigger for reloading the resource
    let (trigger, set_trigger) = scope.signal(0)?;

    // Mock heavy resource
    async fn heavy_work(id: i32) -> Result<String, String> {
        gloo_timers::future::TimeoutFuture::new(2000).await;
        Ok(format!("Content Loaded! (Req ID: {})", id))
    }

    Ok(div![
        h3("Suspense Modes Demo"),
        p("Compare KeepAlive (Data persists) vs Unmount mode (Data resets)."),
        // Mode Selection
        div![
            label![
                input()
                    .attr("type", "radio")
                    .attr("name", "suspense_mode")
                    .attr("checked", rx!(scope; error_handler; *$mode == SuspenseMode::KeepAlive))
                    .on(event::change, set_mode.setter(SuspenseMode::KeepAlive)),
                " KeepAlive (CSS Hide)"
            ]
            .style(sty().margin_right(px(15))?),
            label![
                input()
                    .attr("type", "radio")
                    .attr("name", "suspense_mode")
                    .attr("checked", rx!(scope; error_handler; *$mode == SuspenseMode::Unmount))
                    .on(event::change, set_mode.setter(SuspenseMode::Unmount)),
                " Unmount (DOM Remove)"
            ]
        ]
        .style(sty().margin_bottom(px(15))?),
        div![
            button(show_content.map_fn(scope, |s| if *s {
                "Destroy Component"
            } else {
                "Create Component"
            }, error_handler)?)
            .on(event::click, set_show_content.updater(|s| *s = !*s))
            .style(sty().margin_right(px(10))?),
            button("Reload Resource").on(event::click, set_trigger.updater(|n| *n += 1))
        ]
        .style(sty().margin_bottom(px(15))?),
        div![rx!(scope; error_handler;
            if *$show_content {
                Suspense(scope, error_handler, move |cx| {
                    let resource = Resource::new(
                        scope,
                        trigger,
                        heavy_work,
                        Some(cx),
                        error_handler,
                    )?;
                    Ok(div![
                        div![
                            "Resource Data: ",
                            // Fine-grained reading: Only this text node updates
                            rx!(scope; error_handler; resource.get_data()?.unwrap_or_else(|| "Waiting...".to_string()))
                        ],
                        div("1. Type something below."),
                        div("2. Click 'Reload Resource'."),
                        div("3. KeepAlive: Text stays. Unmount: Text gone."),
                        input()
                            .placeholder("Type here test persistence...")
                            .style(sty().margin_top(px(5))?.padding("5px")?.width(px(250))?)
                    ]
                    .style(sty().border("1px solid green")?.padding("10px")?.background("#e8f5e9")?)
                    .into_any())
                })
                .fallback(div("Loading... (2s)").style(sty().color(ColorName::Blue)?.font_weight(FontWeightKeyword::Bold)?))
                .mode(mode.get()?)
                .build()
                .into_any()
            } else {
                ().into_any()
            }
        )]
        .style(sty().min_height(px(150))?.border("1px dashed #ccc")?.padding("10px")?)
    ]
    .style(sty().padding("20px")?.border("1px solid #ccc")?.border_radius(px(8))?.margin_top(px(20))?))
}

// --- Generics Demo ---

#[component]
pub fn GenericMessage<'scope, T: std::fmt::Display + Clone + 'scope>(
    value: T,
    #[chain] title: &'scope str,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(
        div![h4(title.to_string()), p(format!("Value: {}", value)),].style(
            sty()
                .padding(px(10))?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                .background(AppTheme::SURFACE)?
                .transition("all 0.3s")?,
        ),
    )
}

#[component]
pub fn GenericsDemo<'scope>(
    _scope: Scope<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div![
        h3("Generics & Lifetimes Demo"),
        p("This demonstrates how #[component] macro supports generics and lifetimes natively."),
        GenericMessage(42)
            .title("Integer Message")
            .error_handler(error_handler)
            .build(),
        GenericMessage("Hello Silex!")
            .title("String Message")
            .error_handler(error_handler)
            .build(),
    ]
    .style(
        sty()
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
            .border_radius(px(8))?
            .margin_top(px(20))?
            .background(AppTheme::SURFACE)?
            .transition("all 0.3s")?,
    ))
}

// --- Adaptive Read & Reactive Tuple Demo ---

/// A futuristic non-cloneable structure representing a unique hardware identity.
/// This type represents a resource that should not be duplicated in memory.
struct QuantumIdentity {
    serial: u32,
    signature: String,
}

impl QuantumIdentity {
    fn new(serial: u32) -> Self {
        Self {
            serial,
            signature: format!("Q-SIG-{:08X}", serial),
        }
    }
}

impl std::fmt::Display for QuantumIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID:{} Σ:{}", self.serial, &self.signature[..8])
    }
}

#[component]
pub fn AdaptiveReadDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let system_name = scope.rw_signal(Cow::Borrowed("Nebula-1"))?;
    let (stability, set_stability) = scope.signal(0.85)?; // 0.0 to 1.0

    // Create a non-cloneable resource
    let (identity, _) = scope.signal(QuantumIdentity::new(0xDEADBEEF))?;
    let adaptive_state = (system_name, stability);

    scope.effect(
        move || -> SilexResult<()> {
            let (name, current_stability) = adaptive_state.get()?;
            identity.with(|_| ())?;
            console_log(format!(
                "Quantum Core Vitals updated: {name} at {current_stability:.0}%"
            ));
            Ok(())
        },
        error_handler,
    )?;

    // Cloneable reactive values can be read as one tracked tuple snapshot.
    let status_bar = scope.derived(
        move || {
            let (name, current_stability) = adaptive_state.get()?;
            let identity_label = identity.with(ToString::to_string)?;
            Ok(format!(
                "System: {name} | Stability: {current_stability:.0}% | {identity_label}"
            ))
        },
        error_handler,
    )?;

    // Non-cloneable values still use segmented `with` access for zero-copy reads.
    // Only the specific parts of the UI update when their respective signals change.
    let detail_metrics = rx!(scope; error_handler; {
        div![
            div![
                strong("CORE NAME: "),
                span($system_name.to_uppercase()).style(sty().letter_spacing(px(2))?)
            ],
            div![
                strong("QUANTUM SIGNATURE: "),
                i($identity.signature.clone())
            ].style(sty().margin_top(px(5))?.color(hex("#7f8c8d"))?),
        ]
    });

    Ok(div![
        h3("Adaptive Read & Segmented Access")
            .style(sty().color(hex("#2c3e50"))?.border_left("5px solid #e74c3c")?.padding_left(px(15))?.margin_bottom(px(20))?),

        p("Cloneable reactive values can be grouped into a tuple and read with get(), which tracks every member. Non-cloneable resources remain available through segmented with() access without copying."),

        div![
            // Live Status Bar
            div(status_bar)
                .style(sty().background("#2c3e50")?.color(hex("#ecf0f1"))?.padding("12px 20px")?.border_radius("8px 8px 0 0")?.font_family("'Courier New', monospace")?.font_size(em_unit(0.9))?),

            // Interaction Area
            div![
                detail_metrics,

                div![
                    label("Adjustment Stability: "),
                    input()
                        .attr("type", "range")
                        .attr("min", "0")
                        .attr("max", "1")
                        .attr("step", "0.01")
                        .prop("value", stability)
                        .on(event::input, move |e| {
                            if let Ok(val) = event_target_value(&e).parse::<f64>() {
                                set_stability
                                    .set(val)
                                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            }
                            Ok(())
                        })
                        .style(sty().flex_grow(1)?.accent_color(hex("#e74c3c"))?),
                    span(rx!(scope; error_handler; format!("{:.0}%", *$stability * 100.0)))
                        .style(sty().width(px(50))?.text_align(TextAlignKeyword::Right)?.font_weight(FontWeightKeyword::Bold)?.color(hex("#e74c3c"))?),
                ].style(sty().margin_top(px(20))?.display("flex")?.align_items("center")?.gap(px(15))?),

                div![
                    label("Rename Core: "),
                    input()
                        .bind_value(system_name)
                        .style(sty().padding("8px")?.border("1px solid #ddd")?.border_radius(px(4))?.width(pct(100))?.box_sizing(BoxSizingKeyword::BorderBox)?),
                ].style(sty().margin_top(px(15))?),
            ]
            .style(sty().background("white")?.padding("25px")?.border("1px solid #2c3e50")?.border_top("none")?.border_radius("0 0 8px 8px")?.box_shadow("0 10px 30px rgba(0,0,0,0.1)")?),
        ]
        .style(sty().margin("20px 0")?),

        div![
            p("Architecture Insights:")
                .style(sty().font_weight(FontWeightKeyword::Bold)?.margin_bottom(px(5))?),
            ul![
                li("Tuple Snapshot: adaptive_state.get() tracks and clones the cloneable system name and stability values together."),
                li("Zero-Copy: The $ syntax expands to .with() calls, providing direct references."),
                li("No Clone Needed: QuantumIdentity is non-cloneable, yet accessible via a direct with() read."),
            ]
            .style(sty().font_size(em_unit(0.85))?.color(hex("#34495e"))?),
        ]
        .style(sty().padding("15px")?.background("#fdf2f2")?.border_radius(px(6))?.border("1px solid #fab1a0")?)
    ]
    .style(sty().margin_top(px(30))?))
}
