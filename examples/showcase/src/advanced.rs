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

styled! {
    pub StyledButton<button>(
        children: Children,
        #[prop(into)] color: Signal<silex::css::UnsafeCss>,
        #[prop(into)] size: Signal<String>,
        #[prop(into)] hover_color: Signal<silex::css::UnsafeCss>,
        #[prop(into)] pseudo_state: Signal<String>,
    ) {
        background-color: rgb(98, 0, 234);
        color: $(color);
        border: none;
        border-radius: 4px;
        cursor: pointer;
        transition: transform 0.1s, color 0.2s, padding 0.2s, font-size 0.2s, background-color 0.2s;

        &:$(pseudo_state) {
            background-color: $(hover_color);
            transform: scale(1.05);
        }

        variants: {
            size: {
                small: { padding: 4px 8px; font-size: 12px; }
                medium: { padding: 8px 16px; font-size: 14px; }
                large: { padding: 12px 24px; font-size: 18px; }
            }
        }
    }
}

#[component]
pub fn CssDemo() -> impl View {
    let (color, set_color) = signal(silex::css::UnsafeCss::new("white"));
    let (size, set_size) = signal("medium".to_string());
    let (hover_color, set_hover_color) = signal(silex::css::UnsafeCss::new("#3700b3"));
    let (pseudo_state, set_pseudo_state) = signal("hover".to_string());

    div![
        h3("CSS-in-Rust Demo"),
        p(
            "The button below is styled using the `styled!` macro with scoped styles, dynamic CSS variables (color), static variants (size), and NEW dynamic rules (selectors and nested values)!"
        ),
        StyledButton()
            .children("Scoped Style Button")
            .color(color)
            .size(size)
            .hover_color(hover_color)
            .pseudo_state(pseudo_state)
            .on(event::click, move |_| {
                set_color.update(|c| {
                    *c = if c.0 == "white" {
                        silex::css::UnsafeCss::new("#ffd700")
                    } else {
                        silex::css::UnsafeCss::new("white")
                    }
                });
                set_size.update(|s| {
                    *s = if *s == "medium" {
                        "large".to_string()
                    } else {
                        "medium".to_string()
                    }
                });
                set_hover_color.update(|c| {
                    *c = if c.0 == "#3700b3" {
                        silex::css::UnsafeCss::new("#ff4081") // Vibrant pinkish hue
                    } else {
                        silex::css::UnsafeCss::new("#3700b3")
                    }
                });
                set_pseudo_state.update(|s| {
                    *s = if *s == "hover" {
                        "active".to_string()
                    } else {
                        "hover".to_string()
                    }
                });
                console_log("Clicked! Toggled styles and dynamic rules.");
            }),
    ]
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
                        *t = if t == "Light" {
                            "Dark".to_string()
                        } else {
                            "Light".to_string()
                        }
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
                .into_any()
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
                    login_mutation.mutate_with((username, password).into_signal());
                })
                .attr("disabled", move || login_mutation.loading()) // Make reactive
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
                    .attr("checked", move || mode.get() == SuspenseMode::KeepAlive)
                    .on(event::change, set_mode.setter(SuspenseMode::KeepAlive)),
                " KeepAlive (CSS Hide)"
            ]
            .style("margin-right: 15px;"),
            label![
                input()
                    .attr("type", "radio")
                    .attr("name", "suspense_mode")
                    .attr("checked", move || mode.get() == SuspenseMode::Unmount)
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
        div![Show::new(show_content, move || {
            suspense()
                .resource(move || Resource::new(trigger, heavy_work))
                .children(move |resource| {
                    SuspenseBoundary::new()
                        .mode(mode.get())
                        .fallback(|| {
                            div("Loading... (2s)").style("color: blue; font-weight: bold;")
                        })
                        .children(move || {
                            // Crucial: We do NOT read resource.get() here.
                            div![
                                div![
                                    "Resource Data: ",
                                    // Fine-grained reading: Only this text node updates
                                    move || resource
                                        .get()
                                        .unwrap_or_else(|| "Waiting...".to_string())
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
