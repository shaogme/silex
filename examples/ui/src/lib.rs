use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;
use silex::ui::{Button, Dialog, Input, Progress, Switch, Textarea, *};

#[component]
fn ButtonShowcase<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    Ok(Card(
        ctx,
        chain!(
            CardHeader(
                ctx,
                chain!(
                    CardTitle(ctx, "Button & Badge System").build(),
                    CardDescription(ctx, "Standard variants and sizes ported from shadcn/ui.")
                        .build()
                )
            )
            .build(),
            CardContent(
                ctx,
                chain!(
                    // Variants
                    div(chain!(
                        span("Variants").class(tw!(
                            "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                        )),
                        Button(ctx, "Default").variant("default")?.build(),
                        Button(ctx, "Destructive").variant("destructive")?.build(),
                        Button(ctx, "Outline").variant("outline")?.build(),
                        Button(ctx, "Secondary").variant("secondary")?.build(),
                        Button(ctx, "Ghost").variant("ghost")?.build(),
                        Button(ctx, "Link").variant("link")?.build()
                    ))
                    .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
                    // Sizes
                    div(chain!(
                        span("Sizes").class(tw!(
                            "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                        )),
                        Button(ctx, "XS").variant("outline")?.size("xs")?.build(),
                        Button(ctx, "Small").variant("outline")?.size("sm")?.build(),
                        Button(ctx, "Default")
                            .variant("outline")?
                            .size("default")?
                            .build(),
                        Button(ctx, "Large").variant("outline")?.size("lg")?.build(),
                        Button(ctx, "★").variant("outline")?.size("icon")?.build(),
                        Button(ctx, "⚡")
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
                        Badge(ctx, "Default").variant("default")?.build(),
                        Badge(ctx, "Secondary").variant("secondary")?.build(),
                        Badge(ctx, "Destructive").variant("destructive")?.build(),
                        Badge(ctx, "Outline").variant("outline")?.build(),
                        Badge(ctx, "Ghost").variant("ghost")?.build(),
                        Badge(ctx, "Link").variant("link")?.build()
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
fn FormControlsShowcase<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (text_val, set_text_val) = owner.signal("Hello Silex UI!".to_string())?;
    let (checked_val, set_checked_val) = owner.signal(true)?;
    let (switch_val, set_switch_val) = owner.signal(true)?;

    Ok(Card(ctx, chain!(
        CardHeader(ctx, chain!(
            CardTitle(ctx, "Form & Interactive Controls").build(),
            CardDescription(ctx, "Reactive Input, Textarea, Checkbox and Switch components.").build()
        )).build(),
        CardContent(ctx, chain!(
            // Input
            div(chain!(
                span("Text Input").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Input(ctx)
                    .value(text_val)?
                    .placeholder("Type something...")?
                    .on_input(owner.callback(move |v| {
                        set_text_val.set(v)
                    })?).build(),
                p(rx!(ctx; format!("Live Bound Value: '{}'", $text_val))?)
                    .class(tw!("text-xs text-slate-500 mt-1.5 font-mono"))
            )).class(tw!("flex flex-col mb-6")),

            // Textarea
            div(chain!(
                span("Textarea").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Textarea(ctx)
                    .value("Multi-line textarea component styling ported straight from shadcn/ui v4.")
                    .placeholder("Write a description...")
                    .build()
            )).class(tw!("flex flex-col mb-6")),

            // Checkbox & Switch
            div(chain!(
                div(chain!(
                Checkbox(ctx)
                    .checked(checked_val)?
                    .on_change(owner.callback(move |v| {
                        set_checked_val.set(v)
                    })?).build(),
                    span("Enable Notifications").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2")),

                div(chain!(
                Switch(ctx)
                    .checked(switch_val)?
                    .on_change(owner.callback(move |v| {
                        set_switch_val.set(v)
                    })?).build(),
                    span("Airplane Mode").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2"))
            )).class(tw!("flex flex-wrap items-center justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        )).build()
    )).build())
}

#[component]
fn TabsAndDialogShowcase<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (active_tab, set_active_tab) = owner.signal("account".to_string())?;
    let (dialog_open, set_dialog_open) = owner.signal(false)?;

    Ok(Card(ctx, chain!(
        CardHeader(ctx, chain!(
            CardTitle(ctx, "Tabs & Modal Dialog").build(),
            CardDescription(ctx, "Seamless tab switching and portal-rendered modal dialogs.").build()
        )).build(),
        CardContent(ctx, chain!(
            // Tabs
            div(chain!(
                Tabs(ctx, chain!(
                    TabsList(ctx, chain!(
                        TabsTrigger(ctx, "Account", "account")
                            .active_tab(active_tab)?
                            .on_select(owner.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())?;
                                Ok(())
                            })?)
                            .build(),
                        TabsTrigger(ctx, "Password", "password")
                            .active_tab(active_tab)?
                            .on_select(owner.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())?;
                                Ok(())
                            })?)
                            .build(),
                        TabsTrigger(ctx, "Settings", "settings")
                            .active_tab(active_tab)?
                            .on_select(owner.callback(move |tab: &'static str| {
                                set_active_tab.set(tab.to_string())?;
                                Ok(())
                            })?)
                            .build()
                    ))
                    .class(tw!("grid w-full grid-cols-3"))?
                    .build(),
                    rx!(ctx; {
                        match $active_tab.as_str() {
                            "account" => {
                                TabsContent(ctx, p("Manage your account details and profile preferences.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("account")
                                    .active_tab(active_tab)?
                                    .build()
                                    .into_any()
                            }
                            "password" => {
                                TabsContent(ctx, p("Change your password and configure 2FA security.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("password")
                                    .active_tab(active_tab)?
                                    .build()
                                    .into_any()
                            }
                            _ => TabsContent(ctx, p("Customize system settings and notification channels.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                .value("settings")
                                .active_tab(active_tab)?
                                .build()
                                .into_any(),
                        }
                    })?
                )).build()
            ))
            .class(tw!("mb-6")),
            Separator(ctx).class(tw!("my-4"))?.build(),
            div(chain!(
                span("Silex UI"),
                Separator(ctx)
                    .orientation("vertical")?
                    .class(tw!("h-4 mx-2"))?
                    .build(),
                span("Docs"),
                Separator(ctx)
                    .orientation("vertical")?
                    .class(tw!("h-4 mx-2"))?
                    .build(),
                span("GitHub")
            ))
            .class(tw!("flex items-center text-xs font-medium text-slate-500 dark:text-slate-400 mb-4")),
            // Dialog Trigger
            div(chain!(
                Button(ctx, "Open Modal Dialog")
                    .variant("default")?
                    .on_click(move |_| {
                        set_dialog_open.set(true)?;
                        Ok(())
                    })
                    .build(),
                Dialog(ctx, chain!(
                    DialogHeader(ctx, chain!(
                        DialogTitle(ctx, "Edit Profile").build(),
                        DialogDescription(
                            ctx,
                            "Make changes to your profile here. Click save when you're done."
                        )
                            .build()
                    )).build(),
                    div(chain!(
                        Input(ctx).value("Shao G.")?.placeholder("Name")?.build(),
                        Input(ctx)
                            .value("shaog.me@gmail.com")?
                            .placeholder("Email")?
                            .build()
                    ))
                    .class(tw!("grid gap-3 py-4")),
                    DialogFooter(ctx, chain!(
                        Button(ctx, "Cancel")
                            .variant("outline")?
                            .on_click(move |_| {
                                set_dialog_open.set(false)?;
                                Ok(())
                            })
                            .build(),
                        Button(ctx, "Save Changes")
                            .variant("default")?
                            .on_click(move |_| {
                                set_dialog_open.set(false)?;
                                Ok(())
                            })
                            .build()
                    ))
                    .build()
                ))
                .open(dialog_open)?
                .on_close(owner.callback(move |_| {
                    set_dialog_open.set(false)?;
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
fn FeedbackAndDataShowcase<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (progress_val, set_progress_val) = owner.signal(45u32)?;

    Ok(Card(ctx, chain!(
        CardHeader(ctx, chain!(
            CardTitle(ctx, "Avatars, Progress & Feedback").build(),
            CardDescription(ctx, "Progress indicators, Avatar fallback, Alert banners and Skeletons.").build()
        )).build(),
        CardContent(ctx, chain!(
            // Alert
            Alert(ctx, chain!(
                AlertTitle(ctx, "System Update Complete").build(),
                AlertDescription(ctx, "All shadcn/ui components have been successfully compiled into Silex zero-runtime AST.").build()
            )).variant("default")?.class(tw!("mb-6"))?.build(),

            // Progress Bar
            div(chain!(
                div(chain!(
                    span("Progress").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                    span(rx!(ctx; format!("{}%", $progress_val))?).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                )).class(tw!("flex justify-between items-center mb-1.5")),
                Progress(ctx).value(progress_val)?.class(tw!("mb-3"))?.build(),
                div(chain!(
                    Button(ctx, "-10%").variant("outline")?.size("xs")?.on_click(move |_| {
                        set_progress_val.update(|v| *v = v.saturating_sub(10))?;
                        Ok(())
                    }).build(),
                    Button(ctx, "+10%").variant("outline")?.size("xs")?.on_click(move |_| {
                        set_progress_val.update(|v| *v = (*v + 10).min(100))?;
                        Ok(())
                    }).build()
                )).class(tw!("flex justify-end gap-2"))
            )).class(tw!("mb-6")),

            // Avatar & Skeleton
            div(chain!(
                // Avatars
                div(chain!(
                    Avatar(ctx, chain!(
                        AvatarFallback(ctx, "SG").build()
                    )).build(),
                    Avatar(ctx, chain!(
                        AvatarFallback(ctx, "UI").variant("indigo")?.build()
                    )).class(tw!("bg-indigo-600 text-white"))?.build(),
                    Avatar(ctx, chain!(
                        AvatarFallback(ctx, "SX").variant("emerald")?.build()
                    )).class(tw!("bg-emerald-600 text-white"))?.build()
                )).class(tw!("flex items-center gap-3")),

                // Skeletons
                div(chain!(
                    Skeleton(ctx)
                        .class(tw!("h-4 w-32"))
                        .build(),
                    Skeleton(ctx)
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
    owner: OwnerAccess<'scope>,
    error_handler: ErrorReporter<'scope>,
    context: TooltipContext<'scope>,
) -> SilexResult<AnyView<'scope>> {
    let ctx = SilexContext::new(owner, error_handler);
    Ok(chain!(
        TooltipTrigger(
            ctx,
            Button(ctx, "Hover for Tooltip")
                .variant("outline")?
                .size("sm")?
                .on_click(move |_| {
                    web_sys::console::log_1(&"Tooltip button clicked!".into());
                    Ok(())
                })
                .build()?
        )
        .context(context)
        .build()?,
        TooltipContent(ctx, span("This tooltip was ported from shadcn/ui!"))
            .context(context)
            .side("top")?
            .build()?
    )
    .into_any())
}

fn build_popover_content<'scope>(
    owner: OwnerAccess<'scope>,
    error_handler: ErrorReporter<'scope>,
    context: PopoverContext<'scope>,
) -> SilexResult<AnyView<'scope>> {
    let ctx = SilexContext::new(owner, error_handler);
    Ok(chain!(
        PopoverTrigger(
            ctx,
            Button(ctx, "Open Popover")
                .variant("default")?
                .size("sm")?
                .build()?
        )
        .context(context)
        .build()?,
        PopoverContent(
            ctx,
            chain!(
                PopoverHeader(
                    ctx,
                    chain!(
                        PopoverTitle(ctx, "Dimensions").build()?,
                        PopoverDescription(ctx, "Set the height and width for the layer.")
                            .build()?
                    )
                    .into_any()
                )
                .build()?,
                div(chain!(
                    Input(ctx).value("100%")?.placeholder("Width")?.build()?,
                    Input(ctx).value("300px")?.placeholder("Height")?.build()?
                ))
                .class(tw!("grid gap-2 py-2")),
                div(chain!(
                    PopoverClose(
                        ctx,
                        Button(ctx, "Close")
                            .variant("outline")?
                            .size("sm")?
                            .build()?
                    )
                    .context(context)
                    .build()?
                ))
                .class(tw!("flex justify-end pt-2"))
            )
            .into_any()
        )
        .context(context)
        .build()?
    )
    .into_any())
}

#[component]
fn NewComponentsShowcase<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (slider_val, set_slider_val) = owner.signal(65.0f64)?;
    let (toggle_pressed, set_toggle_pressed) = owner.signal(true)?;
    let (radio_val, set_radio_val) = owner.signal("option-1".to_string())?;
    let (popover_open, set_popover_open) = owner.signal(false)?;
    let (accordion_open, set_accordion_open) = owner.signal(true)?;

    Ok(Card(ctx, chain!(
        CardHeader(ctx, chain!(
            CardTitle(ctx, "Extended shadcn/ui Components").build(),
            CardDescription(ctx, "1:1 ported Slider, Tooltip, Toggle, RadioGroup, Accordion & Popover.").build()
        )).build(),
        CardContent(ctx, chain!(
            // Slider & Toggle
            div(chain!(
                div(chain!(
                    div(chain!(
                        span("Volume Slider").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                        span(rx!(ctx; format!("{:.0}%", $slider_val))?).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                    )).class(tw!("flex justify-between items-center mb-2")),
                    Slider(ctx)
                        .value(slider_val)?
                        .min(0.0)?
                        .max(100.0)?
                        .on_change(owner.callback(move |v| {
                            set_slider_val.set(v)?;
                            Ok(())
                        })?)
                        .build()
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Bold Toggle").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1 block")),
                    Toggle(ctx, span("B").class(tw!("font-bold")))
                        .variant("outline")?
                        .pressed(toggle_pressed)?
                        .on_change(owner.callback(move |p| {
                            set_toggle_pressed.set(p)?;
                            Ok(())
                        })?)
                        .build()
                )).class(tw!("flex flex-col items-start"))
            )).class(tw!("flex items-center gap-6 mb-6")),

            // Tooltip & Popover
            div(chain!(
                Tooltip(ctx, move |ctx| build_tooltip_content(owner, error_handler, ctx)).build()?,
                Popover(ctx, move |ctx| build_popover_content(owner, error_handler, ctx)).build()?
            )).class(tw!("flex items-center gap-4 mb-6")),

            // RadioGroup & Accordion
            div(chain!(
                div(chain!(
                    span("Theme Style").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    RadioGroup(ctx, chain!(
                        div(chain!(
                            RadioGroupItem(ctx, "option-1")
                                .selected_value(radio_val)?
                                .on_select(owner.callback(move |v: &'static str| {
                                    set_radio_val.set(v.to_string())?;
                                    Ok(())
                                })?)
                                .build(),
                            span("Default").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2")),
                        div(chain!(
                            RadioGroupItem(ctx, "option-2")
                                .selected_value(radio_val)?
                                .on_select(owner.callback(move |v: &'static str| {
                                    set_radio_val.set(v.to_string())?;
                                    Ok(())
                                })?)
                                .build(),
                            span("Comfortable").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2"))
                    )).build()
                )).class(tw!("flex-1")),

                div(chain!(
                    span("Accordion").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    Accordion(ctx, chain!(
                        AccordionItem(ctx, chain!(
                            AccordionTrigger(ctx, "Is Silex 1:1 compatible?")
                                .open(accordion_open)?
                                .on_click(owner.callback(move |_| {
                                    set_accordion_open.update(|v| *v = !*v)?;
                                    Ok(())
                                })?)
                                .build(),
                            AccordionContent(ctx, "Yes! Every layout, utility class and reactivity behavior matches shadcn/ui React components.")
                                .open(accordion_open)?
                                .mode(AccordionContentMode::KeepAlive)
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
fn App<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let is_dark = Persistent::builder(owner, "silex-ui-dark", error_handler)
        .local()
        .parse::<bool>()
        .default(true)
        .build()?;

    let _effect = owner.effect(
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

            button(rx!(ctx; if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" })?)
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
                ButtonShowcase(ctx).build(),
                FormControlsShowcase(ctx).build()
            )).class(tw!("flex flex-col gap-6 w-full")),

            div(chain!(
                TabsAndDialogShowcase(ctx).build(),
                FeedbackAndDataShowcase(ctx).build(),
                NewComponentsShowcase(ctx).build()
            )).class(tw!("flex flex-col gap-6 w-full"))
        )).class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
    ))
    .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    .class(rx!(ctx; if *$is_dark { "dark" } else { "" })?)
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

fn mount_ui_view<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let owner = ctx.access();
    let error_handler = owner.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let silex_ctx = SilexContext::new(owner, error_handler.view());
    ctx.mount(App(silex_ctx).build(), error_handler)
}
