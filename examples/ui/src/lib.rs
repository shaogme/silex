use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;
use silex::ui::{Button, Dialog, Input, Progress, Switch, Textarea, *};

#[component]
fn ButtonShowcase<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    Ok(Card(
        context,
        chain!(
            CardHeader(
                context,
                chain!(
                    CardTitle(context, "Button & Badge System").build(),
                    CardDescription(
                        context,
                        "Standard variants and sizes ported from shadcn/ui."
                    )
                    .build()
                )
            )
            .build(),
            CardContent(
                context,
                chain!(
                    // Variants
                    div(chain!(
                        span("Variants").class(tw!(
                            "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                        )),
                        Button(context, "Default").variant("default")?.build(),
                        Button(context, "Destructive")
                            .variant("destructive")?
                            .build(),
                        Button(context, "Outline").variant("outline")?.build(),
                        Button(context, "Secondary").variant("secondary")?.build(),
                        Button(context, "Ghost").variant("ghost")?.build(),
                        Button(context, "Link").variant("link")?.build()
                    ))
                    .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
                    // Sizes
                    div(chain!(
                        span("Sizes").class(tw!(
                            "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                        )),
                        Button(context, "XS")
                            .variant("outline")?
                            .size("xs")?
                            .build(),
                        Button(context, "Small")
                            .variant("outline")?
                            .size("sm")?
                            .build(),
                        Button(context, "Default")
                            .variant("outline")?
                            .size("default")?
                            .build(),
                        Button(context, "Large")
                            .variant("outline")?
                            .size("lg")?
                            .build(),
                        Button(context, "★")
                            .variant("outline")?
                            .size("icon")?
                            .build(),
                        Button(context, "⚡")
                            .variant("default")?
                            .size("icon-sm")?
                            .build()
                    ))
                    .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
                    // Badges
                    div(chain!(
                        span("Badges").class(tw!(
                            "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                        )),
                        Badge(context, "Default").variant("default")?.build(),
                        Badge(context, "Secondary").variant("secondary")?.build(),
                        Badge(context, "Destructive")
                            .variant("destructive")?
                            .build(),
                        Badge(context, "Outline").variant("outline")?.build(),
                        Badge(context, "Ghost").variant("ghost")?.build(),
                        Badge(context, "Link").variant("link")?.build()
                    ))
                    .class(tw!("flex flex-wrap items-center gap-2"))
                )
            )
            .build()
        ),
    )
    .build())
}

#[component]
fn FormControlsShowcase<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let (text_val, set_text_val) = scope.signal("Hello Silex UI!".to_string())?;
    let (checked_val, set_checked_val) = scope.signal(true)?;
    let (switch_val, set_switch_val) = scope.signal(true)?;

    Ok(Card(context, chain!(
        CardHeader(context, chain!(
            CardTitle(context, "Form & Interactive Controls").build(),
            CardDescription(context, "Reactive Input, Textarea, Checkbox and Switch components.").build()
        )).build(),
        CardContent(context, chain!(
            // Input
            div(chain!(
                span("Text Input").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Input(context)
                    .value(text_val)?
                    .placeholder("Type something...")?
                    .on_input(scope.callback(move |v| {
                        set_text_val.set(v).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))
                    })?).build(),
                p(rx!(context; format!("Live Bound Value: '{}'", $text_val)))
                    .class(tw!("text-xs text-slate-500 mt-1.5 font-mono"))
            )).class(tw!("flex flex-col mb-6")),

            // Textarea
            div(chain!(
                span("Textarea").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Textarea(context)
                    .value("Multi-line textarea component styling ported straight from shadcn/ui v4.")
                    .placeholder("Write a description...")
                    .build()
            )).class(tw!("flex flex-col mb-6")),

            // Checkbox & Switch
            div(chain!(
                div(chain!(
                Checkbox(context)
                    .checked(checked_val)?
                    .on_change(scope.callback(move |v| {
                        set_checked_val.set(v).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))
                    })?).build(),
                    span("Enable Notifications").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2")),

                div(chain!(
                Switch(context)
                    .checked(switch_val)?
                    .on_change(scope.callback(move |v| {
                        set_switch_val.set(v).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))
                    })?).build(),
                    span("Airplane Mode").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2"))
            )).class(tw!("flex flex-wrap items-center justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        )).build()
    )).build())
}

#[component]
fn TabsAndDialogShowcase<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let (active_tab, set_active_tab) = scope.signal("account".to_string())?;
    let (dialog_open, set_dialog_open) = scope.signal(false)?;

    Ok(Card(context, chain!(
        CardHeader(context, chain!(
            CardTitle(context, "Tabs & Modal Dialog").build(),
            CardDescription(context, "Seamless tab switching and portal-rendered modal dialogs.").build()
        )).build(),
        CardContent(context, chain!(
            // Tabs
            div(chain!(
                Tabs(context, chain!(
                    TabsList(context, chain!(
                        TabsTrigger(context, "Account", "account")
                            .active_tab(active_tab)?
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string()).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            })?)
                            .build(),
                        TabsTrigger(context, "Password", "password")
                            .active_tab(active_tab)?
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string()).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            })?)
                            .build(),
                        TabsTrigger(context, "Settings", "settings")
                            .active_tab(active_tab)?
                            .on_select(scope.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string()).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            })?)
                            .build()
                    ))
                    .class(tw!("grid w-full grid-cols-3"))?
                    .build(),
                    rx!(context; {
                        match $active_tab.as_str() {
                            "account" => {
                                TabsContent(context, p("Manage your account details and profile preferences.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("account")
                                    .active_tab(active_tab)?
                                    .build()
                                    .into_any()
                            }
                            "password" => {
                                TabsContent(context, p("Change your password and configure 2FA security.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("password")
                                    .active_tab(active_tab)?
                                    .build()
                                    .into_any()
                            }
                            _ => TabsContent(context, p("Customize system settings and notification channels.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                .value("settings")
                                .active_tab(active_tab)?
                                .build()
                                .into_any(),
                        }
                    })
                )).build()
            ))
            .class(tw!("mb-6")),
            Separator(context).class(tw!("my-4"))?.build(),
            div(chain!(
                span("Silex UI"),
                Separator(context)
                    .orientation("vertical")?
                    .class(tw!("h-4 mx-2"))?
                    .build(),
                span("Docs"),
                Separator(context)
                    .orientation("vertical")?
                    .class(tw!("h-4 mx-2"))?
                    .build(),
                span("GitHub")
            ))
            .class(tw!("flex items-center text-xs font-medium text-slate-500 dark:text-slate-400 mb-4")),
            // Dialog Trigger
            div(chain!(
                Button(context, "Open Modal Dialog")
                    .variant("default")?
                    .on_click(move |_| {
                        set_dialog_open.set(true).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                        Ok(())
                    })
                    .build(),
                Dialog(context, chain!(
                    DialogHeader(context, chain!(
                        DialogTitle(context, "Edit Profile").build(),
                        DialogDescription(
                            context,
                            "Make changes to your profile here. Click save when you're done."
                        )
                            .build()
                    )).build(),
                    div(chain!(
                        Input(context).value("Shao G.")?.placeholder("Name")?.build(),
                        Input(context)
                            .value("shaog.me@gmail.com")?
                            .placeholder("Email")?
                            .build()
                    ))
                    .class(tw!("grid gap-3 py-4")),
                    DialogFooter(context, chain!(
                        Button(context, "Cancel")
                            .variant("outline")?
                            .on_click(move |_| {
                                set_dialog_open.set(false).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            })
                            .build(),
                        Button(context, "Save Changes")
                            .variant("default")?
                            .on_click(move |_| {
                                set_dialog_open.set(false).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                Ok(())
                            })
                            .build()
                    ))
                    .build()
                ))
                .open(dialog_open)?
                .on_close(scope.callback(move |_| {
                    set_dialog_open.set(false).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    Ok(())
                })?)
                .build()
            ))
            .class(tw!("flex items-center justify-between"))
        ))
        .build()
    ))
    .build())
}

#[component]
fn FeedbackAndDataShowcase<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let (progress_val, set_progress_val) = scope.signal(45u32)?;

    Ok(Card(context, chain!(
        CardHeader(context, chain!(
            CardTitle(context, "Avatars, Progress & Feedback").build(),
            CardDescription(context, "Progress indicators, Avatar fallback, Alert banners and Skeletons.").build()
        )).build(),
        CardContent(context, chain!(
            // Alert
            Alert(context, chain!(
                AlertTitle(context, "System Update Complete").build(),
                AlertDescription(context, "All shadcn/ui components have been successfully compiled into Silex zero-runtime AST.").build()
            )).variant("default")?.class(tw!("mb-6"))?.build(),

            // Progress Bar
            div(chain!(
                div(chain!(
                    span("Progress").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                    span(rx!(context; format!("{}%", $progress_val))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                )).class(tw!("flex justify-between items-center mb-1.5")),
                Progress(context).value(progress_val)?.class(tw!("mb-3"))?.build(),
                div(chain!(
                    Button(context, "-10%").variant("outline")?.size("xs")?.on_click(move |_| {
                        set_progress_val.update(|v| *v = v.saturating_sub(10)).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                        Ok(())
                    }).build(),
                    Button(context, "+10%").variant("outline")?.size("xs")?.on_click(move |_| {
                        set_progress_val.update(|v| *v = (*v + 10).min(100)).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                        Ok(())
                    }).build()
                )).class(tw!("flex justify-end gap-2"))
            )).class(tw!("mb-6")),

            // Avatar & Skeleton
            div(chain!(
                // Avatars
                div(chain!(
                    Avatar(context, chain!(
                        AvatarFallback(context, "SG").build()
                    )).build(),
                    Avatar(context, chain!(
                        AvatarFallback(context, "UI").variant("indigo")?.build()
                    )).class(tw!("bg-indigo-600 text-white"))?.build(),
                    Avatar(context, chain!(
                        AvatarFallback(context, "SX").variant("emerald")?.build()
                    )).class(tw!("bg-emerald-600 text-white"))?.build()
                )).class(tw!("flex items-center gap-3")),

                // Skeletons
                div(chain!(
                    Skeleton(context)
                        .class(tw!("h-4 w-32"))
                        .build(),
                    Skeleton(context)
                        .class(tw!("h-4 w-20"))
                        .build()
                )).class(tw!("flex flex-col gap-2"))
            )).class(tw!("flex items-center justify-between p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
        .build()
    ))
    .build())
}

fn build_tooltip_content<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    ctx: TooltipContext<'scope>,
) -> SilexResult<AnyView<'scope>> {
    let context = SilexContext::new(scope, error_handler);
    Ok(chain!(
        TooltipTrigger(
            context,
            Button(context, "Hover for Tooltip")
                .variant("outline")?
                .size("sm")?
                .on_click(move |_| {
                    web_sys::console::log_1(&"Tooltip button clicked!".into());
                    Ok(())
                })
                .build()?
        )
        .ctx(ctx)
        .build()?,
        TooltipContent(context, span("This tooltip was ported from shadcn/ui!"))
            .ctx(ctx)
            .side("top")?
            .build()?
    )
    .into_any())
}

fn build_popover_content<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    ctx: PopoverContext<'scope>,
) -> SilexResult<AnyView<'scope>> {
    let context = SilexContext::new(scope, error_handler);
    Ok(chain!(
        PopoverTrigger(
            context,
            Button(context, "Open Popover")
                .variant("default")?
                .size("sm")?
                .build()?
        )
        .ctx(ctx)
        .build()?,
        PopoverContent(
            context,
            chain!(
                PopoverHeader(
                    context,
                    chain!(
                        PopoverTitle(context, "Dimensions").build()?,
                        PopoverDescription(context, "Set the height and width for the layer.")
                            .build()?
                    )
                    .into_any()
                )
                .build()?,
                div(chain!(
                    Input(context)
                        .value("100%")?
                        .placeholder("Width")?
                        .build()?,
                    Input(context)
                        .value("300px")?
                        .placeholder("Height")?
                        .build()?
                ))
                .class(tw!("grid gap-2 py-2")),
                div(chain!(
                    PopoverClose(
                        context,
                        Button(context, "Close")
                            .variant("outline")?
                            .size("sm")?
                            .build()?
                    )
                    .ctx(ctx)
                    .build()?
                ))
                .class(tw!("flex justify-end pt-2"))
            )
            .into_any()
        )
        .ctx(ctx)
        .build()?
    )
    .into_any())
}

#[component]
fn NewComponentsShowcase<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let (slider_val, set_slider_val) = scope.signal(65.0f64)?;
    let (toggle_pressed, set_toggle_pressed) = scope.signal(true)?;
    let (radio_val, set_radio_val) = scope.signal("option-1".to_string())?;
    let (popover_open, set_popover_open) = scope.signal(false)?;
    let (accordion_open, set_accordion_open) = scope.signal(true)?;

    Ok(Card(context, chain!(
        CardHeader(context, chain!(
            CardTitle(context, "Extended shadcn/ui Components").build(),
            CardDescription(context, "1:1 ported Slider, Tooltip, Toggle, RadioGroup, Accordion & Popover.").build()
        )).build(),
        CardContent(context, chain!(
            // Slider & Toggle
            div(chain!(
                div(chain!(
                    div(chain!(
                        span("Volume Slider").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                        span(rx!(context; format!("{:.0}%", $slider_val))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                    )).class(tw!("flex justify-between items-center mb-2")),
                    Slider(context)
                        .value(slider_val)?
                        .min(0.0)?
                        .max(100.0)?
                        .on_change(scope.callback(move |v| {
                            set_slider_val.set(v).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })?)
                        .build()
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Bold Toggle").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1 block")),
                    Toggle(context, span("B").class(tw!("font-bold")))
                        .variant("outline")?
                        .pressed(toggle_pressed)?
                        .on_change(scope.callback(move |p| {
                            set_toggle_pressed.set(p).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                            Ok(())
                        })?)
                        .build()
                )).class(tw!("flex flex-col items-start"))
            )).class(tw!("flex items-center gap-6 mb-6")),

            // Tooltip & Popover
            div(chain!(
                Tooltip(context, move |ctx| build_tooltip_content(scope, error_handler, ctx)).build()?,
                Popover(context, move |ctx| build_popover_content(scope, error_handler, ctx)).build()?
            )).class(tw!("flex items-center gap-4 mb-6")),

            // RadioGroup & Accordion
            div(chain!(
                div(chain!(
                    span("Theme Style").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    RadioGroup(context, chain!(
                        div(chain!(
                            RadioGroupItem(context, "option-1")
                                .selected_value(radio_val)?
                                .on_select(scope.callback(move |v: &'static str| {
                                    set_radio_val.set(v.to_string()).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                    Ok(())
                                })?)
                                .build(),
                            span("Default").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2")),
                        div(chain!(
                            RadioGroupItem(context, "option-2")
                                .selected_value(radio_val)?
                                .on_select(scope.callback(move |v: &'static str| {
                                    set_radio_val.set(v.to_string()).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                    Ok(())
                                })?)
                                .build(),
                            span("Comfortable").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2"))
                    )).build()
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Accordion").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    Accordion(context, chain!(
                        AccordionItem(context, chain!(
                            AccordionTrigger(context, "Is Silex 1:1 compatible?")
                                .open(accordion_open)?
                                .on_click(scope.callback(move |_| {
                                    set_accordion_open.update(|v| *v = !*v).map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                                    Ok(())
                                })?)
                                .build(),
                            AccordionContent(context, "Yes! Every layout, utility class and reactivity behavior matches shadcn/ui React components.")
                                .open(accordion_open)?
                                .build()
                        ), "item-1").build()
                    )).build()
                )).class(tw!("flex-1"))
            )).class(tw!("flex flex-col sm:flex-row gap-6 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
        .build()
    ))
    .build())
}

#[component]
fn App<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let is_dark = Persistent::builder(scope, "silex-ui-dark", error_handler)
        .local()
        .parse::<bool>()
        .default(true)
        .build()?;

    let _effect = scope.effect(
        move || -> SilexResult<()> {
            let dark = is_dark.get()?;
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

            button(rx!(context; if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                .class(tw!(
                    "flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                ))
                .on_click(move |_| -> SilexResult<()> {
                    is_dark.update(|d| *d = !*d)?;
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
                ButtonShowcase(context).build(),
                FormControlsShowcase(context).build()
            )).class(tw!("flex flex-col gap-6 w-full")),

            div(chain!(
                TabsAndDialogShowcase(context).build(),
                FeedbackAndDataShowcase(context).build(),
                NewComponentsShowcase(context).build()
            )).class(tw!("flex flex-col gap-6 w-full"))
        )).class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
    ))
    .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    .class(rx!(context; if *$is_dark { "dark" } else { "" }))
    .into_any()
    )
}

/// Mount the UI showcase into the conventional `#app` target.
pub fn mount_ui() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    silex::ui::inject_shadcn_base_styles();
    bootstrap.mount(Runtime::new(), mount_ui_view)?;
    bootstrap.into_js_host()
}

/// Mount the UI showcase into a caller-provided target node.
pub fn mount_ui_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    silex::ui::inject_shadcn_base_styles();
    bootstrap.mount(Runtime::new(), mount_ui_view)?;
    bootstrap.into_js_host()
}

fn mount_ui_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let silex_context = SilexContext::new(scope, error_handler);
    context.mount(App(silex_context).build(), error_handler)
}
