use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;
use silex::ui::{Button, Dialog, Input, Progress, Switch, Textarea, *};

#[component]
fn ButtonShowcase<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    Card(chain!(
        CardHeader(chain!(
            CardTitle("Button & Badge System"),
            CardDescription("Standard variants and sizes ported from shadcn/ui.")
        )),
        CardContent(chain!(
            // Variants
            div(chain!(
                span("Variants").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Button(scope, "Default").variant("default"),
                Button(scope, "Destructive").variant("destructive"),
                Button(scope, "Outline").variant("outline"),
                Button(scope, "Secondary").variant("secondary"),
                Button(scope, "Ghost").variant("ghost"),
                Button(scope, "Link").variant("link")
            ))
            .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
            // Sizes
            div(chain!(
                span("Sizes").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Button(scope, "XS").variant("outline").size("xs"),
                Button(scope, "Small").variant("outline").size("sm"),
                Button(scope, "Default").variant("outline").size("default"),
                Button(scope, "Large").variant("outline").size("lg"),
                Button(scope, "★").variant("outline").size("icon"),
                Button(scope, "⚡").variant("default").size("icon-sm")
            ))
            .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
            // Badges
            div(chain!(
                span("Badges").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Badge(scope, "Default").variant("default"),
                Badge(scope, "Secondary").variant("secondary"),
                Badge(scope, "Destructive").variant("destructive"),
                Badge(scope, "Outline").variant("outline"),
                Badge(scope, "Ghost").variant("ghost"),
                Badge(scope, "Link").variant("link")
            ))
            .class(tw!("flex flex-wrap items-center gap-2"))
        ))
    ))
}

#[component]
fn FormControlsShowcase<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (text_val, set_text_val) = scope.signal("Hello Silex UI!".to_string());
    let (checked_val, set_checked_val) = scope.signal(true);
    let (switch_val, set_switch_val) = scope.signal(true);

    Card(chain!(
        CardHeader(chain!(
            CardTitle("Form & Interactive Controls"),
            CardDescription("Reactive Input, Textarea, Checkbox and Switch components.")
        )),
        CardContent(chain!(
            // Input
            div(chain!(
                span("Text Input").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Input(scope)
                    .value(text_val)
                    .placeholder("Type something...")
                    .on_input(scope.callback(move |v| set_text_val.set(v))),
                p(rx!(scope; format!("Live Bound Value: '{}'", $text_val)))
                    .class(tw!("text-xs text-slate-500 mt-1.5 font-mono"))
            )).class(tw!("flex flex-col mb-6")),

            // Textarea
            div(chain!(
                span("Textarea").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Textarea(scope)
                    .value("Multi-line textarea component styling ported straight from shadcn/ui v4.")
                    .placeholder("Write a description...")
            )).class(tw!("flex flex-col mb-6")),

            // Checkbox & Switch
            div(chain!(
                div(chain!(
                Checkbox(scope)
                    .checked(checked_val)
                    .on_change(scope.callback(move |v| set_checked_val.set(v))),
                    span("Enable Notifications").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2")),

                div(chain!(
                Switch(scope)
                    .checked(switch_val)
                    .on_change(scope.callback(move |v| set_switch_val.set(v))),
                    span("Airplane Mode").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2"))
            )).class(tw!("flex flex-wrap items-center justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn TabsAndDialogShowcase<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (active_tab, set_active_tab) = scope.signal("account".to_string());
    let (dialog_open, set_dialog_open) = scope.signal(false);

    Card(chain!(
        CardHeader(chain!(
            CardTitle("Tabs & Modal Dialog"),
            CardDescription("Seamless tab switching and portal-rendered modal dialogs.")
        )),
        CardContent(chain!(
            // Tabs
            div(chain!(
                Tabs(scope, chain!(
                    TabsList(scope, chain!(
                        TabsTrigger(scope, "Account", "account")
                            .active_tab(active_tab)
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())
                            })),
                        TabsTrigger(scope, "Password", "password")
                            .active_tab(active_tab)
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())
                            })),
                        TabsTrigger(scope, "Settings", "settings")
                            .active_tab(active_tab)
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())
                            }))
                    ))
                    .class(tw!("grid w-full grid-cols-3")),
                    rx!(scope; {
                        match $active_tab.as_str() {
                            "account" => {
                                TabsContent(scope, p("Manage your account details and profile preferences.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("account")
                                    .active_tab(active_tab)
                                    .into_any()
                            }
                            "password" => {
                                TabsContent(scope, p("Change your password and configure 2FA security.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("password")
                                    .active_tab(active_tab)
                                    .into_any()
                            }
                            _ => TabsContent(scope, p("Customize system settings and notification channels.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                .value("settings")
                                .active_tab(active_tab)
                                .into_any(),
                        }
                    })
                ))
            ))
            .class(tw!("mb-6")),
            Separator(scope).class(tw!("my-4")),
            div(chain!(
                span("Silex UI"),
                Separator(scope).orientation("vertical").class(tw!("h-4 mx-2")),
                span("Docs"),
                Separator(scope).orientation("vertical").class(tw!("h-4 mx-2")),
                span("GitHub")
            ))
            .class(tw!("flex items-center text-xs font-medium text-slate-500 dark:text-slate-400 mb-4")),
            // Dialog Trigger
            div(chain!(
                Button(scope, "Open Modal Dialog")
                    .variant("default")
                    .on_click(move |_| {
                        set_dialog_open.set(true);
                        Ok(())
                    }),
                Dialog(scope, chain!(
                    DialogHeader(chain!(
                        DialogTitle("Edit Profile"),
                        DialogDescription(
                            "Make changes to your profile here. Click save when you're done."
                        )
                    )),
                    div(chain!(
                        Input(scope).value("Shao G.").placeholder("Name"),
                        Input(scope).value("shaog.me@gmail.com").placeholder("Email")
                    ))
                    .class(tw!("grid gap-3 py-4")),
                    DialogFooter(chain!(
                        Button(scope, "Cancel")
                            .variant("outline")
                            .on_click(move |_| {
                                set_dialog_open.set(false);
                                Ok(())
                            }),
                        Button(scope, "Save Changes")
                            .variant("default")
                            .on_click(move |_| {
                                set_dialog_open.set(false);
                                Ok(())
                            })
                    ))
                ))
                .open(dialog_open)
                .on_close(scope.callback(move |_| set_dialog_open.set(false)))
            ))
            .class(tw!("flex items-center justify-between"))
        ))
    ))
}

#[component]
fn FeedbackAndDataShowcase<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (progress_val, set_progress_val) = scope.signal(45u32);

    Card(chain!(
        CardHeader(chain!(
            CardTitle("Avatars, Progress & Feedback"),
            CardDescription("Progress indicators, Avatar fallback, Alert banners and Skeletons.")
        )),
        CardContent(chain!(
            // Alert
            Alert(scope, chain!(
                AlertTitle("System Update Complete"),
                AlertDescription("All shadcn/ui components have been successfully compiled into Silex zero-runtime AST.")
            )).variant("default").class(tw!("mb-6")),

            // Progress Bar
            div(chain!(
                div(chain!(
                    span("Progress").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                    span(rx!(scope; format!("{}%", $progress_val))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                )).class(tw!("flex justify-between items-center mb-1.5")),
                Progress(scope).value(progress_val).class(tw!("mb-3")),
                div(chain!(
                    Button(scope, "-10%").variant("outline").size("xs").on_click(move |_| {
                        set_progress_val.update(|v| *v = v.saturating_sub(10));
                        Ok(())
                    }),
                    Button(scope, "+10%").variant("outline").size("xs").on_click(move |_| {
                        set_progress_val.update(|v| *v = (*v + 10).min(100));
                        Ok(())
                    })
                )).class(tw!("flex justify-end gap-2"))
            )).class(tw!("mb-6")),

            // Avatar & Skeleton
            div(chain!(
                // Avatars
                div(chain!(
                    Avatar(scope, chain!(
                        AvatarFallback(scope, "SG")
                    )),
                    Avatar(scope, chain!(
                        AvatarFallback(scope, "UI").variant("indigo")
                    )).class(tw!("bg-indigo-600 text-white")),
                    Avatar(scope, chain!(
                        AvatarFallback(scope, "SX").variant("emerald")
                    )).class(tw!("bg-emerald-600 text-white"))
                )).class(tw!("flex items-center gap-3")),

                // Skeletons
                div(chain!(
                    Skeleton(scope).class(tw!("h-4 w-32")),
                    Skeleton(scope).class(tw!("h-4 w-20"))
                )).class(tw!("flex flex-col gap-2"))
            )).class(tw!("flex items-center justify-between p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn NewComponentsShowcase<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (slider_val, set_slider_val) = scope.signal(65.0f64);
    let (toggle_pressed, set_toggle_pressed) = scope.signal(true);
    let (radio_val, set_radio_val) = scope.signal("option-1".to_string());
    let (popover_open, set_popover_open) = scope.signal(false);
    let (accordion_open, set_accordion_open) = scope.signal(true);

    Card(chain!(
        CardHeader(chain!(
            CardTitle("Extended shadcn/ui Components"),
            CardDescription("1:1 ported Slider, Tooltip, Toggle, RadioGroup, Accordion & Popover.")
        )),
        CardContent(chain!(
            // Slider & Toggle
            div(chain!(
                div(chain!(
                    div(chain!(
                        span("Volume Slider").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                        span(rx!(scope; format!("{:.0}%", $slider_val))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                    )).class(tw!("flex justify-between items-center mb-2")),
                    Slider(scope)
                        .value(slider_val)
                        .min(0.0)
                        .max(100.0)
                        .on_change(scope.callback(move |v| set_slider_val.set(v)))
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Bold Toggle").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1 block")),
                    Toggle(scope, span("B").class(tw!("font-bold")))
                        .variant("outline")
                        .pressed(toggle_pressed)
                        .on_change(scope.callback(move |p| set_toggle_pressed.set(p)))
                )).class(tw!("flex flex-col items-start"))
            )).class(tw!("flex items-center gap-6 mb-6")),

            // Tooltip & Popover
            div(chain!(
                Tooltip(scope, move |ctx| chain!(
                    TooltipTrigger(scope,
                        Button(scope, "Hover for Tooltip")
                            .variant("outline")
                            .size("sm")
                            .on_click(move |_| {
                                web_sys::console::log_1(&"Tooltip button clicked!".into());
                                Ok(())
                            })
                    )
                    .ctx(ctx),
                    TooltipContent(scope, span("This tooltip was ported from shadcn/ui!"))
                        .ctx(ctx)
                        .side("top")
                )),

                Popover(scope, move |ctx| chain!(
                    PopoverTrigger(scope,
                        Button(scope, "Open Popover")
                            .variant("default")
                            .size("sm")
                    )
                    .ctx(ctx),
                    PopoverContent(scope, chain!(
                        PopoverHeader(scope, chain!(
                            PopoverTitle(scope, "Dimensions"),
                            PopoverDescription(scope, "Set the height and width for the layer.")
                        )),
                        div(chain!(
                            Input(scope).value("100%").placeholder("Width"),
                            Input(scope).value("300px").placeholder("Height")
                        )).class(tw!("grid gap-2 py-2")),
                        div(chain!(
                            PopoverClose(scope,
                                Button(scope, "Close")
                                    .variant("outline")
                                    .size("sm")
                            )
                            .ctx(ctx)
                        )).class(tw!("flex justify-end pt-2"))
                    ))
                    .ctx(ctx)
                ))
            )).class(tw!("flex items-center gap-4 mb-6")),

            // RadioGroup & Accordion
            div(chain!(
                div(chain!(
                    span("Theme Style").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    RadioGroup(scope, chain!(
                        div(chain!(
                            RadioGroupItem(scope, "option-1").selected_value(radio_val).on_select(scope.callback(move |v: &'static str| {
                                set_radio_val.set(v.to_string())
                            })),
                            span("Default").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2")),
                        div(chain!(
                            RadioGroupItem(scope, "option-2").selected_value(radio_val).on_select(scope.callback(move |v: &'static str| {
                                set_radio_val.set(v.to_string())
                            })),
                            span("Comfortable").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2"))
                    ))
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Accordion").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    Accordion(scope, chain!(
                        AccordionItem(scope, chain!(
                            AccordionTrigger(scope, "Is Silex 1:1 compatible?")
                                .open(accordion_open)
                                .on_click(scope.callback(move |_| {
                                    set_accordion_open.update(|v| *v = !*v)
                                })),
                            AccordionContent(scope, "Yes! Every layout, utility class and reactivity behavior matches shadcn/ui React components.")
                                .open(accordion_open)
                        ), "item-1")
                    ))
                )).class(tw!("flex-1"))
            )).class(tw!("flex flex-col sm:flex-row gap-6 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn App<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<impl View<'scope>> {
    let is_dark = Persistent::builder(scope, "silex-ui-dark", error_handler)
        .local()
        .parse::<bool>()
        .default(true)
        .build();

    let _effect = scope.effect(
        move || -> SilexResult<()> {
            let dark = is_dark.get();
            if let Some(doc) = window().document()
                && let Some(el) = doc.document_element()
            {
                if dark {
                    let _ = el.class_list().add_1("dark");
                } else {
                    let _ = el.class_list().remove_1("dark");
                }
            }
            Ok(())
        },
        error_handler,
    )?;

    Ok(div(chain!(
        // Header
        div(chain!(
            div(chain!(
                span("🎨 Silex UI Kit").class(tw!(
                    "text-xs font-black uppercase tracking-widest px-3.5 py-1.5 bg-indigo-50 dark:bg-indigo-950/80 text-indigo-600 dark:text-indigo-400 rounded-full border border-solid border-indigo-200 dark:border-indigo-800/60 shadow-sm"
                )),
                span("shadcn/ui v4 Ported Component Suite").class(tw!(
                    "hidden sm:inline-block text-xs font-semibold text-slate-500 dark:text-slate-400"
                ))
            )).class(tw!("flex items-center gap-3")),

            button(rx!(scope; if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                .class(tw!(
                    "flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                ))
                .on_click(move |_| -> SilexResult<()> {
                    is_dark.update(|d| *d = !*d);
                    Ok(())
                })
        )).class(tw!("w-full flex items-center justify-between mb-8 max-w-6xl mx-auto")),

        // Hero
        div(chain!(
            h1("Pure Rust shadcn/ui Component Library")
                .class(tw!("text-3xl sm:text-5xl font-black text-slate-900 dark:text-white tracking-tight mb-4 text-center")),
            p("Zero-runtime overhead Tailwind CSS styling with fine-grained signal reactivity and type-safe Rust components.")
                .class(tw!("text-sm sm:text-base text-slate-600 dark:text-slate-300 max-w-2xl text-center leading-relaxed mb-8"))
        )).class(tw!("flex flex-col items-center max-w-6xl mx-auto mb-10")),

        // Masonry Component Grid
        div(chain!(
            div(chain!(
                ButtonShowcase(scope),
                FormControlsShowcase(scope)
            )).class(tw!("flex flex-col gap-6 w-full")),

            div(chain!(
                TabsAndDialogShowcase(scope),
                FeedbackAndDataShowcase(scope),
                NewComponentsShowcase(scope)
            )).class(tw!("flex flex-col gap-6 w-full"))
        )).class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
    ))
    .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    .class(rx!(scope; if *$is_dark { "dark" } else { "" }))
    .into_any()
    )
}

/// Mount the UI showcase into the conventional `#app` target.
pub fn mount_ui() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    silex::ui::inject_shadcn_base_styles();
    bootstrap.mount(Runtime::new(), mount_ui_view)?;
    bootstrap.into_js_host()
}

/// Mount the UI showcase into a caller-provided target node.
pub fn mount_ui_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
    silex::ui::inject_shadcn_base_styles();
    bootstrap.mount(Runtime::new(), mount_ui_view)?;
    bootstrap.into_js_host()
}

fn mount_ui_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    });
    context.mount(App(scope, error_handler), error_handler)
}
