use silex::prelude::*;
use silex::reexports::web_sys;

// --- Store Definition ---
#[derive(Clone, Default, Store)]
#[store(name = "use_user_settings")]
pub struct UserSettings {
    pub theme: String,
    pub notifications: bool,
    pub username: String,
}

#[component]
pub fn StoreDemo() -> impl View {
    // Access global store provided in main
    // Note: `use_context::<T>() -> Option<T>` and `expect_context::<T>() -> T` are also available.
    // Access global store using the generated helper
    let settings = use_user_settings();

    div![
        h3("Global Store Demo"),
        div![
            p![strong("Username: "), settings.username],
            p![strong("Theme: "), settings.theme],
            p![
                strong("Notifications: "),
                text(
                    settings
                        .notifications
                        .map(|n| if *n { "On" } else { "Off" })
                ),
            ],
        ]
        .style("border: 1px solid #ccc; padding: 10px; margin-bottom: 10px;"),
        h4("Update Settings"),
        div![
            button("Toggle Theme").on(
                event::click,
                rx! {
                    settings.theme.update(|t| {
                        *t = if t == "Light" { "Dark".to_string() } else { "Light".to_string() }
                    })
                }
            ),
            button("Toggle Notifications")
                .on(event::click, settings.notifications.updater(|n| *n = !*n)),
            input()
                .bind_value(settings.username)
                .placeholder("Change username..."),
        ]
        .style("display: flex; gap: 10px;"),
    ]
}

#[component]
pub fn QueryDemo() -> impl View {
    let val = use_query_signal("demo_val");

    div![
        h3("Query Signal Demo"),
        p("This input is synced with the URL query parameter 'demo_val' using `use_query_signal`."),
        div![
            input()
                .bind_value(val) // Automatic two-way binding
                .placeholder("Type here...")
                .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px;"),
            button("Reset")
                .on(event::click, val.setter(String::new()))
                .style("padding: 8px 16px; cursor: pointer;")
        ]
        .style("display: flex; gap: 10px; margin: 10px 0; align-items: center;"),
        p![
            strong("Current Value: "),
            val // Signals implement Display
        ]
        .style("background: #f5f5f5; padding: 10px; border-radius: 4px;")
    ]
}

#[component]
pub fn AuthGuard(children: Children) -> impl View {
    let settings = use_user_settings();

    move || {
        if settings.username.get() != "Guest" {
            children.clone()
        } else {
            div![
                    h3("🔒 Restricted Access"),
                    p("This content is protected. Please go to 'Store Demo' and change your username to something other than 'Guest'."),
                ].style("padding: 20px; background: #fff0f0; border: 1px solid #ffcccc; color: #cc0000;")
                .into_shared()
        }
    }
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
pub fn ResourceDemo() -> impl View {
    let (user_id, set_user_id) = signal(1);

    // Create Resource: triggers when user_id changes
    let user_resource = Resource::new(user_id, mock_fetch_user);

    div![
        h3("Resource & Optimistic UI"),
        p("Fetches user data with a 1s delay. You can optimistically update the name before the server responds."),

        div![
            button("User 1").on(event::click, set_user_id.setter(1)),
            button("User 2").on(event::click, set_user_id.setter(2)),
            button("Invalid User").on(event::click, set_user_id.setter(-1)),
            button("Refetch").on(event::click, move |_| user_resource.refetch()),
        ].style("display: flex; gap: 10px; margin-bottom: 15px;"),

        div![
            "Status: ",
            // Show loading state using the new state enum helper
            move || {
                let state = user_resource.state.get();
                if state.is_loading() {
                    span(if let ResourceState::Reloading(_) = state { "Reloading..." } else { "Loading..." }).style("color: orange;")
                } else {
                    span("Idle").style("color: green;")
                }
            }
        ].style("margin-bottom: 10px; font-weight: bold;"),

        // Display Data using get_data() which covers both Ready and Reloading
        move || {
            match user_resource.get_data() {
                Some(user) => div![
                    div(format!("ID: {}", user.id)),
                    div(format!("Name: {}", user.name)),
                    div(format!("Role: {}", user.role)),

                    // Optimistic Update Controls
                    div![
                        h4("Optimistic Updates (Local Cache)"),
                        button("Rename to 'Modified' (Optimistic)")
                            .on(event::click, move |_| {
                                // Manually update the local resource data
                                user_resource.update(|u| {
                                    u.name = "Modified Name".to_string();
                                });
                            }),
                    ].style("margin-top: 15px; border-top: 1px solid #eee; padding-top: 10px;")
                ].into_any(),
                None => div("No Data (or Loading...)").into_any(),
            }
        },

        // Error Handling via state matching
        move || {
            if let ResourceState::Error(err) = user_resource.state.get() {
                div(format!("Error: {}", err)).style("color: red; margin-top: 10px;").into_any()
            } else {
                "".into_any()
            }
        }
    ]
    .style("padding: 20px; border: 1px solid #ccc; border-radius: 8px;")
}

#[component]
pub fn MutationDemo() -> impl View {
    // Simulate a login mutation
    // Takes (username, password) and returns a Result<String, String> token
    let login_mutation = Mutation::new(|(user, pass): (String, String)| async move {
        console_log(format!("Logging in as {}...", user));
        gloo_timers::future::TimeoutFuture::new(1500).await;

        if user == "admin" && pass == "password" {
            Ok("fake_jwt_token_12345".to_string())
        } else {
            Err("Invalid credentials".to_string())
        }
    });

    let username = RwSignal::new("".to_string());
    let password = RwSignal::new("".to_string());

    div![
        h3("Mutation Demo (Async Write)"),
        p("Enter 'admin' / 'password' to succeed, others to fail."),
        div![
            input()
                .bind_value(username)
                .placeholder("Username")
                .style("margin-right: 10px; padding: 5px;"),
            input()
                .bind_value(password)
                .attr("type", "password")
                .placeholder("Password")
                .style("margin-right: 10px; padding: 5px;"),
            button("Login")
                .attr("type", "button") // Prevent accidental form submission
                .on(event::click, move |e: web_sys::MouseEvent| {
                    e.prevent_default();

                    // Note: "login_mutation.mutate((username.get(), password.get()));" is the same as "login_mutation.mutate_with((username, password));"
                    login_mutation.mutate_with((username, password).into_rx());
                })
                .attr("disabled", rx!(login_mutation.loading())) // Make reactive
                .style("padding: 5px 10px;"),
        ]
        .style("margin-bottom: 10px;"),
        // Loading State
        move || if login_mutation.loading() {
            div("Logging in...").style("color: blue;").into_any()
        } else {
            "".into_any()
        },
        // Error State
        move || login_mutation
            .error()
            .map(|err| { div(format!("Error: {}", err)).style("color: red;") }),
        // Success State
        move || login_mutation.value().map(|token| {
            div![
                div("Login Successful!").style("color: green; font-weight: bold;"),
                div(format!("Token: {}", token))
                    .style("font-family: monospace; background: #eee; padding: 5px;")
            ]
        })
    ]
    .style("padding: 20px; border: 1px solid #ccc; border-radius: 8px;")
}

#[component]
pub fn SuspenseDemo() -> impl View {
    use silex::components::{SuspenseBoundary, SuspenseMode};

    let (show_content, set_show_content) = signal(false);
    let (mode, set_mode) = signal(SuspenseMode::KeepAlive);

    // Trigger for reloading the resource
    let (trigger, set_trigger) = signal(0);

    // Mock heavy resource
    async fn heavy_work(id: i32) -> Result<String, String> {
        gloo_timers::future::TimeoutFuture::new(2000).await;
        Ok(format!("Content Loaded! (Req ID: {})", id))
    }

    div![
        h3("Suspense Modes Demo"),
        p("Compare KeepAlive (Data persists) vs Unmount mode (Data resets)."),
        // Mode Selection
        div![
            label![
                input()
                    .attr("type", "radio")
                    .attr("name", "suspense_mode")
                    .attr("checked", rx!(*$mode == SuspenseMode::KeepAlive))
                    .on(event::change, set_mode.setter(SuspenseMode::KeepAlive)),
                " KeepAlive (CSS Hide)"
            ]
            .style("margin-right: 15px;"),
            label![
                input()
                    .attr("type", "radio")
                    .attr("name", "suspense_mode")
                    .attr("checked", rx!(*$mode == SuspenseMode::Unmount))
                    .on(event::change, set_mode.setter(SuspenseMode::Unmount)),
                " Unmount (DOM Remove)"
            ]
        ]
        .style("margin-bottom: 15px;"),
        div![
            button(show_content.map(|s| if *s {
                "Destroy Component"
            } else {
                "Create Component"
            }))
            .on(event::click, set_show_content.updater(|s| *s = !*s))
            .style("margin-right: 10px;"),
            button("Reload Resource").on(event::click, set_trigger.updater(|n| *n += 1))
        ]
        .style("margin-bottom: 15px;"),
        div![Show::new(show_content, rx! {
            suspense()
                .resource(move || Resource::new(trigger, heavy_work))
                .children(move |resource| {
                    SuspenseBoundary::new()
                        .mode(mode.get())
                        .fallback(rx!(div("Loading... (2s)").style("color: blue; font-weight: bold;")))
                        .children(rx! {
                            // Crucial: We do NOT read resource.get() here.
                            div![
                                div![
                                    "Resource Data: ",
                                    // Fine-grained reading: Only this text node updates
                                    rx!(resource.get().unwrap_or_else(|| "Waiting...".to_string()))
                                ],
                                div("1. Type something below."),
                                div("2. Click 'Reload Resource'."),
                                div("3. KeepAlive: Text stays. Unmount: Text gone."),
                                input()
                                    .placeholder("Type here test persistence...")
                                    .style("margin-top: 5px; padding: 5px; width: 250px;")
                            ]
                            .style("border: 1px solid green; padding: 10px; background: #e8f5e9;")
                        })
                })
        })]
        .style("min-height: 150px; border: 1px dashed #ccc; padding: 10px;")
    ]
    .style("padding: 20px; border: 1px solid #ccc; border-radius: 8px; margin-top: 20px;")
}

// --- Generics Demo ---

#[component]
pub fn GenericMessage<'a, T: std::fmt::Display + Clone + 'static>(
    value: T,
    title: &'a str,
) -> impl View {
    div![h4(title.to_string()), p(format!("Value: {}", value)),]
        .style("padding: 10px; border: 1px solid #eee; margin-bottom: 10px;")
}

#[component]
pub fn GenericsDemo() -> impl View {
    div![
        h3("Generics & Lifetimes Demo"),
        p("This demonstrates how #[component] macro supports generics and lifetimes natively."),
        GenericMessage().value(42).title("Integer Message"),
        GenericMessage()
            .value("Hello Silex!")
            .title("String Message"),
    ]
    .style("padding: 20px; border: 1px solid #ccc; border-radius: 8px; margin-top: 20px;")
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
pub fn AdaptiveReadDemo() -> impl View {
    let system_name = RwSignal::new("Nebula-1".to_string());
    let (stability, set_stability) = signal(0.85); // 0.0 to 1.0

    // Create a non-cloneable resource
    let (identity, _) = signal(QuantumIdentity::new(0xDEADBEEF));

    // 1. REACTIVE TUPLE: Used for organizational grouping and tracking.
    // Note: (RwSignal<String>, ReadSignal<f64>, ReadSignal<QuantumIdentity>)
    // implements RxInternal, allowing tracking even with non-cloneable items.
    let core_vitals = (system_name, stability, identity);

    Effect::new(move |_| {
        core_vitals.track(); // Track the whole group at once
        console_log("Quantum Core Vitals updated.");
    });

    // 2. SEGMENTED ACCESS (Recommended):
    // Using $ syntax on individual signals is ALWAYS zero-copy and
    // works even if the types are NOT Clone.
    let status_bar = rx!(format!(
        "System: {} | Stability: {:.0}% | {}",
        $system_name,
        $stability * 100.0,
        $identity
    ));

    // 3. FINE-GRAINED REACTIVITY:
    // Only the specific parts of the UI update when their respective signals change.
    let detail_metrics = rx! {
        div![
            div![
                strong("CORE NAME: "),
                span($system_name.to_uppercase()).style("letter-spacing: 2px;")
            ],
            div![
                strong("QUANTUM SIGNATURE: "),
                i($identity.signature.clone())
            ].style("margin-top: 5px; color: #7f8c8d;"),
        ]
    };

    div![
        h3("Adaptive Read & Segmented Access")
            .style("color: #2c3e50; border-left: 5px solid #e74c3c; padding-left: 15px; margin-bottom: 20px;"),

        p("Silex 0.1.0-beta.1 optimizes reactive access for performance. While tuples can group resources, segmented access using individual signals ensures zero-copy performance without Clone requirements."),

        div![
            // Live Status Bar
            div(status_bar)
                .style("background: #2c3e50; color: #ecf0f1; padding: 12px 20px; border-radius: 8px 8px 0 0; font-family: 'Courier New', monospace; font-size: 0.9em;"),

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
                                set_stability.set(val);
                            }
                        })
                        .style("flex-grow: 1; accent-color: #e74c3c;"),
                    span(rx!(format!("{:.0}%", *$stability * 100.0)))
                        .style("width: 50px; text-align: right; font-weight: bold; color: #e74c3c;"),
                ].style("margin-top: 20px; display: flex; align-items: center; gap: 15px;"),

                div![
                    label("Rename Core: "),
                    input()
                        .bind_value(system_name)
                        .style("padding: 8px; border: 1px solid #ddd; border-radius: 4px; width: 100%; box-sizing: border-box;"),
                ].style("margin-top: 15px;"),
            ]
            .style("background: white; padding: 25px; border: 1px solid #2c3e50; border-top: none; border-radius: 0 0 8px 8px; box-shadow: 0 10px 30px rgba(0,0,0,0.1);"),
        ]
        .style("margin: 20px 0;"),

        div![
            p("Architecture Insights:")
                .style("font-weight: bold; margin-bottom: 5px;"),
            ul![
                li("Zero-Copy: The $ syntax expands to .with() calls, providing direct references."),
                li("No Clone Needed: QuantumIdentity is non-cloneable, yet accessible via references."),
                li("Tuple Limitation: Tuples grouping non-cloneable items are valid for tracking, but 'overall' access via .with() on the tuple itself is restricted to avoid accidental deep clones."),
            ]
            .style("font-size: 0.85em; color: #34495e;"),
        ]
        .style("padding: 15px; background: #fdf2f2; border-radius: 6px; border: 1px solid #fab1a0;")
    ]
    .style("margin-top: 30px;")
}
