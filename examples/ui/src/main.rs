use silex::persist::Persistent;
use silex::prelude::*;
use silex::reexports::web_sys;
use silex::ui::{Button, Dialog, Input, Progress, Switch, Textarea, *};

#[component]
fn ButtonShowcase() -> impl View {
    Card(view_chain!(
        CardHeader(view_chain!(
            CardTitle("Button & Badge System"),
            CardDescription("Standard variants and sizes ported from shadcn/ui.")
        )),
        CardContent(view_chain!(
            // Variants
            div(view_chain!(
                span("Variants").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Button("Default").variant("default"),
                Button("Destructive").variant("destructive"),
                Button("Outline").variant("outline"),
                Button("Secondary").variant("secondary"),
                Button("Ghost").variant("ghost"),
                Button("Link").variant("link")
            ))
            .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
            // Sizes
            div(view_chain!(
                span("Sizes").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Button("XS").variant("outline").size("xs"),
                Button("Small").variant("outline").size("sm"),
                Button("Default").variant("outline").size("default"),
                Button("Large").variant("outline").size("lg"),
                Button("★").variant("outline").size("icon"),
                Button("⚡").variant("default").size("icon-sm")
            ))
            .class(tw!("flex flex-wrap items-center gap-2 mb-6")),
            // Badges
            div(view_chain!(
                span("Badges").class(tw!(
                    "text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1"
                )),
                Badge("Default").variant("default"),
                Badge("Secondary").variant("secondary"),
                Badge("Destructive").variant("destructive"),
                Badge("Outline").variant("outline"),
                Badge("Ghost").variant("ghost"),
                Badge("Link").variant("link")
            ))
            .class(tw!("flex flex-wrap items-center gap-2"))
        ))
    ))
}

#[component]
fn FormControlsShowcase() -> impl View {
    let (text_val, set_text_val) = Signal::pair("Hello Silex UI!".to_string());
    let (checked_val, set_checked_val) = Signal::pair(true);
    let (switch_val, set_switch_val) = Signal::pair(true);

    Card(view_chain!(
        CardHeader(view_chain!(
            CardTitle("Form & Interactive Controls"),
            CardDescription("Reactive Input, Textarea, Checkbox and Switch components.")
        )),
        CardContent(view_chain!(
            // Input
            div(view_chain!(
                span("Text Input").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Input()
                    .value(text_val)
                    .placeholder("Type something...")
                    .on_input(move |v| set_text_val.set(v)),
                p(rx!(move || format!("Live Bound Value: '{}'", text_val.get())))
                    .class(tw!("text-xs text-slate-500 mt-1.5 font-mono"))
            )).class(tw!("flex flex-col mb-6")),

            // Textarea
            div(view_chain!(
                span("Textarea").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1")),
                Textarea()
                    .value("Multi-line textarea component styling ported straight from shadcn/ui v4.")
                    .placeholder("Write a description...")
            )).class(tw!("flex flex-col mb-6")),

            // Checkbox & Switch
            div(view_chain!(
                div(view_chain!(
                    Checkbox()
                        .checked(checked_val)
                        .on_change(move |v| set_checked_val.set(v)),
                    span("Enable Notifications").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2")),

                div(view_chain!(
                    Switch()
                        .checked(switch_val)
                        .on_change(move |v| set_switch_val.set(v)),
                    span("Airplane Mode").class(tw!("text-sm font-medium text-slate-900 dark:text-slate-100"))
                )).class(tw!("flex items-center gap-2"))
            )).class(tw!("flex flex-wrap items-center justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn TabsAndDialogShowcase() -> impl View {
    let (active_tab, set_active_tab) = Signal::pair("account".to_string());
    let (dialog_open, set_dialog_open) = Signal::pair(false);

    Card(view_chain!(
        CardHeader(view_chain!(
            CardTitle("Tabs & Modal Dialog"),
            CardDescription("Seamless tab switching and portal-rendered modal dialogs.")
        )),
        CardContent(view_chain!(
            // Tabs
            div(view_chain!(
                Tabs(view_chain!(
                    TabsList(view_chain!(
                        TabsTrigger("Account", "account")
                            .active_tab(active_tab)
                            .on_select(move |tab: &'static str| set_active_tab.set(tab.to_string())),
                        TabsTrigger("Password", "password")
                            .active_tab(active_tab)
                            .on_select(move |tab: &'static str| set_active_tab.set(tab.to_string())),
                        TabsTrigger("Settings", "settings")
                            .active_tab(active_tab)
                            .on_select(move |tab: &'static str| set_active_tab.set(tab.to_string()))
                    ))
                    .class(tw!("grid w-full grid-cols-3")),
                    rx!(move || {
                        match active_tab.get().as_str() {
                            "account" => {
                                TabsContent(p("Manage your account details and profile preferences.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("account")
                                    .active_tab(active_tab)
                                    .into_any()
                            }
                            "password" => {
                                TabsContent(p("Change your password and configure 2FA security.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                    .value("password")
                                    .active_tab(active_tab)
                                    .into_any()
                            }
                            _ => TabsContent(p("Customize system settings and notification channels.").class(tw!("p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300")))
                                .value("settings")
                                .active_tab(active_tab)
                                .into_any(),
                        }
                    })
                ))
            ))
            .class(tw!("mb-6")),
            Separator().class(tw!("my-4")),
            div(view_chain!(
                span("Silex UI"),
                Separator().orientation("vertical").class(tw!("h-4 mx-2")),
                span("Docs"),
                Separator().orientation("vertical").class(tw!("h-4 mx-2")),
                span("GitHub")
            ))
            .class(tw!("flex items-center text-xs font-medium text-slate-500 dark:text-slate-400 mb-4")),
            // Dialog Trigger
            div(view_chain!(
                Button("Open Modal Dialog")
                    .variant("default")
                    .on_click(move |_| set_dialog_open.set(true)),
                Dialog(view_chain!(
                    DialogHeader(view_chain!(
                        DialogTitle("Edit Profile"),
                        DialogDescription(
                            "Make changes to your profile here. Click save when you're done."
                        )
                    )),
                    div(view_chain!(
                        Input().value("Shao G.").placeholder("Name"),
                        Input().value("shaog.me@gmail.com").placeholder("Email")
                    ))
                    .class(tw!("grid gap-3 py-4")),
                    DialogFooter(view_chain!(
                        Button("Cancel")
                            .variant("outline")
                            .on_click(move |_| set_dialog_open.set(false)),
                        Button("Save Changes")
                            .variant("default")
                            .on_click(move |_| set_dialog_open.set(false))
                    ))
                ))
                .open(dialog_open)
                .on_close(move |_| set_dialog_open.set(false))
            ))
            .class(tw!("flex items-center justify-between"))
        ))
    ))
}

#[component]
fn FeedbackAndDataShowcase() -> impl View {
    let (progress_val, set_progress_val) = Signal::pair(45u32);

    Card(view_chain!(
        CardHeader(view_chain!(
            CardTitle("Avatars, Progress & Feedback"),
            CardDescription("Progress indicators, Avatar fallback, Alert banners and Skeletons.")
        )),
        CardContent(view_chain!(
            // Alert
            Alert(view_chain!(
                AlertTitle("System Update Complete"),
                AlertDescription("All shadcn/ui components have been successfully compiled into Silex zero-runtime AST.")
            )).variant("default").class(tw!("mb-6")),

            // Progress Bar
            div(view_chain!(
                div(view_chain!(
                    span("Progress").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                    span(rx!(move || format!("{}%", progress_val.get()))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                )).class(tw!("flex justify-between items-center mb-1.5")),
                Progress().value(progress_val).class(tw!("mb-3")),
                div(view_chain!(
                    Button("-10%").variant("outline").size("xs").on_click(move |_| set_progress_val.update(|v| *v = v.saturating_sub(10))),
                    Button("+10%").variant("outline").size("xs").on_click(move |_| set_progress_val.update(|v| *v = (*v + 10).min(100)))
                )).class(tw!("flex justify-end gap-2"))
            )).class(tw!("mb-6")),

            // Avatar & Skeleton
            div(view_chain!(
                // Avatars
                div(view_chain!(
                    Avatar(view_chain!(
                        AvatarFallback("SG")
                    )),
                    Avatar(view_chain!(
                        AvatarFallback("UI").variant("indigo")
                    )).class(tw!("bg-indigo-600 text-white")),
                    Avatar(view_chain!(
                        AvatarFallback("SX").variant("emerald")
                    )).class(tw!("bg-emerald-600 text-white"))
                )).class(tw!("flex items-center gap-3")),

                // Skeletons
                div(view_chain!(
                    Skeleton().class(tw!("h-4 w-32")),
                    Skeleton().class(tw!("h-4 w-20"))
                )).class(tw!("flex flex-col gap-2"))
            )).class(tw!("flex items-center justify-between p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn NewComponentsShowcase() -> impl View {
    let (slider_val, set_slider_val) = Signal::pair(65.0f64);
    let (toggle_pressed, set_toggle_pressed) = Signal::pair(true);
    let (radio_val, set_radio_val) = Signal::pair("option-1".to_string());
    let (popover_open, set_popover_open) = Signal::pair(false);
    let (accordion_open, set_accordion_open) = Signal::pair(true);

    Card(view_chain!(
        CardHeader(view_chain!(
            CardTitle("Extended shadcn/ui Components"),
            CardDescription("1:1 ported Slider, Tooltip, Toggle, RadioGroup, Accordion & Popover.")
        )),
        CardContent(view_chain!(
            // Slider & Toggle
            div(view_chain!(
                div(view_chain!(
                    div(view_chain!(
                        span("Volume Slider").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400")),
                        span(rx!(move || format!("{:.0}%", slider_val.get()))).class(tw!("text-xs font-bold text-indigo-600 dark:text-indigo-400"))
                    )).class(tw!("flex justify-between items-center mb-2")),
                    Slider()
                        .value(slider_val)
                        .min(0.0)
                        .max(100.0)
                        .on_change(move |v| set_slider_val.set(v))
                )).class(tw!("flex-1")),

                div(view_chain!(
                    span("Bold Toggle").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1 block")),
                    Toggle(span("B").class(tw!("font-bold")))
                        .variant("outline")
                        .pressed(toggle_pressed)
                        .on_change(move |p| set_toggle_pressed.set(p))
                )).class(tw!("flex flex-col items-start"))
            )).class(tw!("flex items-center gap-6 mb-6")),

            // Tooltip & Popover
            div(view_chain!(
                Tooltip(move |ctx| view_chain!(
                    TooltipTrigger(
                        Button("Hover for Tooltip")
                            .variant("outline")
                            .size("sm")
                            .on_click(move |_| {
                                web_sys::console::log_1(&"Tooltip button clicked!".into());
                            })
                    )
                    .ctx(ctx),
                    TooltipContent(span("This tooltip was ported from shadcn/ui!"))
                        .ctx(ctx)
                        .side("top")
                )),

                Popover(move |ctx| view_chain!(
                    PopoverTrigger(
                        Button("Open Popover")
                            .variant("default")
                            .size("sm")
                    )
                    .ctx(ctx),
                    PopoverContent(view_chain!(
                        PopoverHeader(view_chain!(
                            PopoverTitle("Dimensions"),
                            PopoverDescription("Set the height and width for the layer.")
                        )),
                        div(view_chain!(
                            Input().value("100%").placeholder("Width"),
                            Input().value("300px").placeholder("Height")
                        )).class(tw!("grid gap-2 py-2")),
                        div(view_chain!(
                            PopoverClose(
                                Button("Close")
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
            div(view_chain!(
                div(view_chain!(
                    span("Theme Style").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    RadioGroup(view_chain!(
                        div(view_chain!(
                            RadioGroupItem("option-1").selected_value(radio_val).on_select(move |v: &'static str| set_radio_val.set(v.to_string())),
                            span("Default").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2")),
                        div(view_chain!(
                            RadioGroupItem("option-2").selected_value(radio_val).on_select(move |v: &'static str| set_radio_val.set(v.to_string())),
                            span("Comfortable").class(tw!("text-xs font-medium text-slate-900 dark:text-slate-100"))
                        )).class(tw!("flex items-center gap-2"))
                    ))
                )).class(tw!("flex-1")),

                div(view_chain!(
                    span("Accordion").class(tw!("text-xs font-semibold text-slate-500 dark:text-slate-400 mb-2 block")),
                    Accordion(view_chain!(
                        AccordionItem(view_chain!(
                            AccordionTrigger("Is Silex 1:1 compatible?")
                                .open(accordion_open)
                                .on_click(move |_| set_accordion_open.update(|v| *v = !*v)),
                            AccordionContent("Yes! Every layout, utility class and reactivity behavior matches shadcn/ui React components.")
                                .open(accordion_open)
                        ), "item-1")
                    ))
                )).class(tw!("flex-1"))
            )).class(tw!("flex flex-col sm:flex-row gap-6 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800"))
        ))
    ))
}

#[component]
fn App() -> impl View {
    let is_dark = Persistent::builder("silex-ui-dark")
        .local()
        .parse::<bool>()
        .default(true)
        .build();

    Effect::new(move |_| {
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
    });

    div(view_chain!(
        // Header
        div(view_chain!(
            div(view_chain!(
                span("🎨 Silex UI Kit").class(tw!(
                    "text-xs font-black uppercase tracking-widest px-3.5 py-1.5 bg-indigo-50 dark:bg-indigo-950/80 text-indigo-600 dark:text-indigo-400 rounded-full border border-solid border-indigo-200 dark:border-indigo-800/60 shadow-sm"
                )),
                span("shadcn/ui v4 Ported Component Suite").class(tw!(
                    "hidden sm:inline-block text-xs font-semibold text-slate-500 dark:text-slate-400"
                ))
            )).class(tw!("flex items-center gap-3")),

            button(rx!(if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                .class(tw!(
                    "flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                ))
                .on_click(move |_| is_dark.update(|d| *d = !*d))
        )).class(tw!("w-full flex items-center justify-between mb-8 max-w-6xl mx-auto")),

        // Hero
        div(view_chain!(
            h1("Pure Rust shadcn/ui Component Library")
                .class(tw!("text-3xl sm:text-5xl font-black text-slate-900 dark:text-white tracking-tight mb-4 text-center")),
            p("Zero-runtime overhead Tailwind CSS styling with fine-grained signal reactivity and type-safe Rust components.")
                .class(tw!("text-sm sm:text-base text-slate-600 dark:text-slate-300 max-w-2xl text-center leading-relaxed mb-8"))
        )).class(tw!("flex flex-col items-center max-w-6xl mx-auto mb-10")),

        // Masonry Component Grid
        div(view_chain!(
            div(view_chain!(
                ButtonShowcase(),
                FormControlsShowcase()
            )).class(tw!("flex flex-col gap-6 w-full")),

            div(view_chain!(
                TabsAndDialogShowcase(),
                FeedbackAndDataShowcase(),
                NewComponentsShowcase()
            )).class(tw!("flex flex-col gap-6 w-full"))
        )).class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
    ))
    .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    .class(rx!(if *$is_dark { "dark" } else { "" }))
}

fn main() {
    setup_global_error_handlers();
    silex::ui::inject_shadcn_base_styles();
    mount_to_body(App);
}
