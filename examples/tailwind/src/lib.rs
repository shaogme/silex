use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoCategory {
    All,
    Core,
    Interactive,
    Advanced,
}

impl std::fmt::Display for DemoCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Core => write!(f, "Core"),
            Self::Interactive => write!(f, "Interactive"),
            Self::Advanced => write!(f, "Advanced"),
        }
    }
}

impl std::str::FromStr for DemoCategory {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Core" => Ok(Self::Core),
            "Interactive" => Ok(Self::Interactive),
            "Advanced" => Ok(Self::Advanced),
            _ => Ok(Self::All),
        }
    }
}

#[component]
fn CategoryTab<'scope>(
    #[ctx] ctx: SilexContext<'scope>,
    label: &'static str,
    target: DemoCategory,
    category: Persistent<'scope, DemoCategory>,
) -> impl View<'scope> {
    let is_active = rx!(ctx; *$category == target);
    Ok(button(label)
        .class(tw!(
            "px-4 py-2 text-xs rounded-xl transition-all duration-200 cursor-pointer border-0 outline-none",
            (
                is_active,
                "bg-indigo-600 text-white font-bold shadow-md",
                "bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700 font-semibold"
            )
        ))
        .on_click(move |_| {
            category.set(target).map_err(SilexError::from)?;
            Ok(())
        })
    )
}

#[component]
fn FeatureBadge<'scope>(
    #[ctx] ctx: SilexContext<'scope>,
    label: &'static str,
    theme: &'static str,
) -> impl View<'scope> {
    let cls = match theme {
        "emerald" => tw!(
            "text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"
        ),
        "rose" => tw!(
            "text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"
        ),
        _ => tw!(
            "text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700"
        ),
    };
    span(label).class(cls)
}

#[component]
fn Header<'scope>(
    #[ctx] ctx: SilexContext<'scope>,
    is_dark: Persistent<'scope, bool>,
    category: Persistent<'scope, DemoCategory>,
) -> impl View<'scope> {
    let categories = Constant::new(vec![
        ("All Highlights", DemoCategory::All),
        ("Core Engine", DemoCategory::Core),
        ("Interactive & Reactivity", DemoCategory::Interactive),
        ("Advanced (Phases 4-7)", DemoCategory::Advanced),
    ]);

    Ok(div(chain!(
        // Top Toolbar Row: Badge, Statuses & Theme Toggle
        div(chain!(
            div(chain!(
                span("⚡ Silex Tailwind Proc-Macro").class(tw!(
                    "text-xs font-black uppercase tracking-widest px-3.5 py-1.5 bg-indigo-50 dark:bg-indigo-950/80 text-indigo-600 dark:text-indigo-400 rounded-full border border-solid border-indigo-200 dark:border-indigo-800/60 shadow-sm"
                )),
                span("v0.1.0-beta.8 • Full Utility Coverage").class(tw!(
                    "hidden sm:inline-block text-xs font-semibold text-slate-500 dark:text-slate-400"
                ))
            )).class(tw!("flex items-center gap-3")),

            button(rx!(ctx; if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                .class(tw!(
                    "flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                ))
                .on_click(move |_| {
                    is_dark.update(|d| *d = !*d)?;
                    Ok(())
                })
        )).class(tw!("w-full flex items-center justify-between mb-8")),

        // Hero Title & Description
        h1("Compile-Time Utility-First CSS Engine")
            .class(tw!("text-3xl sm:text-5xl font-black text-slate-900 dark:text-white tracking-tight mb-4 transition-colors duration-300")),
        p("Zero-runtime overhead Tailwind CSS parsed, merged, and optimized into compact AST classes at compile time via LightningCSS. Fully responsive with dynamic signal reactivity.")
            .class(tw!("text-sm sm:text-base text-slate-600 dark:text-slate-300 max-w-3xl text-center leading-relaxed mb-8 transition-colors duration-300")),

        // Dashboard Category Tabs rendered via Index component
        div(Index(ctx, categories)
            .children(move |item, _| {
                let (label, target) = item;
                CategoryTab(ctx, label, target, category).build()
            })
            .build())
        .class(tw!("flex flex-wrap items-center justify-center gap-2 p-1.5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800"))
    ))
    .class(tw!("w-full max-w-6xl mx-auto mb-10 p-8 sm:p-10 bg-white dark:bg-slate-850 rounded-3xl border border-solid border-slate-200 dark:border-slate-800 shadow-xl transition-colors duration-300 flex flex-col items-center text-center")))
}

// Card Wrapper for Consistency
fn card_container_cls() -> &'static str {
    tw!(
        "p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700/80 rounded-3xl shadow-lg hover:shadow-xl transition-all duration-300 flex flex-col h-fit"
    )
}

#[component]
fn TailwindMergeDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    // 示范编译期智能消解: p-2 被 p-6 覆盖, bg-red-500 被 bg-white/dark:bg-slate-800 覆盖
    div(chain!(
        div(chain!(
            span("1. Compile-Time AST Merge").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("Smart Property Deduplication (Last-Wins)").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Multiple conflicting utilities (p-2 vs p-6, red vs background) are resolved in AST macro parsing phase.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            span("Input: tw!(\"p-2 p-6 bg-red-500 dark:bg-slate-800 ...\")")
                .class(tw!("text-xs font-mono text-slate-800 dark:text-slate-200 bg-slate-100 dark:bg-slate-900 p-3.5 rounded-xl border border-solid border-slate-200 dark:border-slate-800 block mb-4 transition-colors duration-300 overflow-x-auto")),
            div(chain!(
                span("✓ Computed Padding: 1.5rem (24px)").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ AST Override: bg-red-500 Removed").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn KeyframesDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("2. Preset Keyframes Engine").class(tw!("text-xs font-black text-purple-600 dark:text-purple-400 uppercase tracking-widest")),
            h2("Zero-Config Built-in Keyframes").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(chain!(
            // Spin
            div(chain!(
                div(()).class(tw!("size-7 border-2 border-solid border-indigo-600 dark:border-indigo-400 border-t-transparent dark:border-t-transparent rounded-full animate-spin mb-3")),
                span("animate-spin").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("360° Loop").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Pulse
            div(chain!(
                div(()).class(tw!("size-7 bg-purple-500 rounded-xl animate-pulse mb-3")),
                span("animate-pulse").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Glow Fade").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Bounce
            div(chain!(
                div("↓").class(tw!("size-7 bg-pink-500 text-white font-bold flex items-center justify-center rounded-full animate-bounce mb-3 text-xs")),
                span("animate-bounce").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Bouncing").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Ping
            div(chain!(
                div(()).class(tw!("size-7 bg-cyan-400 rounded-full animate-ping mb-3")),
                span("animate-ping").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Beacon").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-2 sm:grid-cols-4 gap-3"))
    ))
    .class(card_container_cls())
}

#[component]
fn GroupAndPeerDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("3. Compound State Selectors").class(tw!("text-xs font-black text-pink-600 dark:text-pink-400 uppercase tracking-widest")),
            h2("Group & Peer Lifted Hover/Focus").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(chain!(
            // Group Hover Container
            div(chain!(
                span("Group Hover Card (Hover Card Below)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    span("★").class(tw!("text-base text-amber-300 transition-all duration-300 group-hover:rotate-180 group-hover:scale-125")),
                    span("Group Hover Reaction").class(tw!("text-xs font-bold text-white font-mono"))
                )).class(tw!("flex items-center gap-2.5 px-4 py-2.5 bg-indigo-600 rounded-xl cursor-pointer transition-all duration-300 group-hover:scale-105 shadow-md"))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 group transition-all duration-300")),

            // Peer Focus Form Input
            div(chain!(
                span("Peer Focus Input").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    input().class(tw!("w-full box-border px-3.5 py-2 bg-white dark:bg-slate-800 border border-solid border-slate-300 dark:border-slate-700 rounded-xl text-xs text-slate-900 dark:text-white peer outline-none transition-colors duration-300")),
                    span("✓ Peer Input Focused!").class(tw!("hidden peer-focus:block text-xs font-semibold text-sky-600 dark:text-sky-400 mt-2 px-3 py-1 bg-slate-100 dark:bg-slate-900 rounded-lg border border-solid border-slate-300 dark:border-slate-800"))
                ))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-4"))
    ))
    .class(card_container_cls())
}

#[component]
fn FiltersAndReactivityDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (count, set_count) = scope.signal(16)?;
    let pad_val = rx!(ctx; format!("{}px", $count));

    Ok(div(chain!(
        div(chain!(
            span("4. Filters & Dynamic Signals").class(tw!("text-xs font-black text-sky-600 dark:text-sky-400 uppercase tracking-widest")),
            h2("Glassmorphism & Rust Signals").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(chain!(
            // Glassmorphism Filter Card
            div(chain!(
                span("Backdrop Blur Filter").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div("Hover to Remove Blur")
                    .class(tw!("p-4 bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-xl blur-sm hover:blur-none transition-all duration-300 cursor-pointer text-center font-bold text-xs text-indigo-600 dark:text-indigo-400"))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // Dynamic Signal Interpolation
            div(chain!(
                span("Signal Class $(pad_val)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    button("Increase Padding").class(tw!("px-3 py-1.5 bg-indigo-600 text-white text-xs font-bold rounded-lg cursor-pointer transition-all mb-2.5"))
                        .on_click(move |_| {
                            set_count.update(|n| *n = (*n + 4).min(36))?;
                            Ok(())
                        }),
                    div(rx!(ctx; format!("Padding: {}px", $count)))
                        .class(tw!(error_handler; "p-[$(pad_val)] bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-xl font-mono text-xs text-indigo-600 dark:text-indigo-400 transition-all duration-200 text-center")?)
                ))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-4"))
    ))
    .class(card_container_cls()))
}

#[component]
fn ThemeSystemAndDiagnosticsDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let theme_box_cls = tw_verbose!(
        "p-4 bg-theme(primary/50) text-theme(border/80) border border-solid border-slate-300 dark:border-slate-700 rounded-2xl shadow-sm transition-colors duration-300"
    );

    div(chain!(
        div(chain!(
            span("5. Theme System & Diagnostics").class(tw!("text-xs font-black text-sky-600 dark:text-sky-400 uppercase tracking-widest")),
            h2("Opacity Suffix & AST Inspection").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Supports bg-theme(primary/50) opacity suffix via CSS color-mix() and tw_verbose! compile-time CSS AST inspection.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            div(chain!(
                span("tw_verbose!(\"p-4 bg-theme(primary/50) ...\")").class(tw!("text-xs font-mono text-slate-800 dark:text-slate-200 block mb-2")),
                div(chain!(
                    span("✓ Theme Opacity: color-mix(...)").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700")),
                    span("✓ Levenshtein Typo Diagnostic").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700"))
                )).class(tw!("flex flex-wrap gap-2"))
            )).class(theme_box_cls)
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn ContainerQueriesAndDceDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("6. Container Queries & DCE").class(tw!("text-xs font-black text-emerald-600 dark:text-emerald-500 uppercase tracking-widest")),
            h2("Component Container Responsive @sm").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Element-level responsive queries (@sm:, @[400px]:) paired with compile-time dead @keyframes elimination.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            div(chain!(
                span("Container Box (@container)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    span("Responds to Container Width rather than Viewport").class(tw!("text-xs font-mono text-slate-900 dark:text-white block font-bold")),
                    span("@sm:p-6 @[400px]:bg-slate-100 dark:@[400px]:bg-slate-800").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-1 block"))
                )).class(tw!("p-4 bg-white dark:bg-slate-900 rounded-xl border border-solid border-slate-200 dark:border-slate-800 @sm:p-6 @[400px]:bg-slate-100 @[400px]:dark:bg-slate-800 transition-colors duration-300"))
            )).class(tw!("@container p-3 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),
            div(chain!(
                span("✓ @container: inline-size").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ Zero Unused @keyframes").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn StandardColorPaletteDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("7. Standard Color Palette System").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("22 Color Families & /alpha Opacity").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("22 standard Tailwind color families with 50~950 shade steps & /alpha opacity suffixes.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            // Palette Swatches Showcase
            div(chain!(
                span("Standard Swatches Showcase").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2.5 block")),
                div(chain!(
                    div("slate-900").class(tw!("p-2.5 bg-slate-900 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("indigo-600").class(tw!("p-2.5 bg-indigo-600 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("emerald-500").class(tw!("p-2.5 bg-emerald-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("rose-500").class(tw!("p-2.5 bg-rose-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("amber-500").class(tw!("p-2.5 bg-amber-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("sky-400").class(tw!("p-2.5 bg-sky-400 text-slate-900 text-xs font-mono font-bold rounded-lg text-center shadow-sm"))
                )).class(tw!("grid grid-cols-3 sm:grid-cols-6 gap-2 mb-4"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Multi-Property & Opacity Suffix Badges
            div(chain!(
                span("✓ bg-indigo-600/50: rgba(...)").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700")),
                span("✓ border-rose-500/25").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn ReactiveConditionalTwDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (is_active, set_is_active) = scope.signal(false)?;

    let card_cls = tw!(
        "p-6 bg-white dark:bg-slate-800 border rounded-3xl shadow-lg hover:shadow-xl transition-all duration-300 flex flex-col h-fit",
        (
            is_active,
            "border-indigo-500 bg-indigo-50/50 dark:bg-indigo-950/40",
            "border-slate-200 dark:border-slate-700/80"
        )
    );

    let btn_cls = tw!(
        "px-4 py-2.5 rounded-xl font-bold text-xs cursor-pointer transition-all duration-300 shadow-sm",
        (
            is_active,
            "bg-indigo-600 text-white hover:bg-indigo-700 scale-105"
        ),
        (
            rx!(ctx; !*$is_active),
            "bg-slate-100 dark:bg-slate-900 text-slate-900 dark:text-slate-100 hover:bg-slate-200 dark:hover:bg-slate-800"
        )
    );

    Ok(div(chain!(
        div(chain!(
            span("8. Zero-Cost Reactive Conditional Utility").class(tw!("text-xs font-black text-violet-600 dark:text-violet-400 uppercase tracking-widest")),
            h2("Reactive Tuple Syntax tw!(..., (cond, then, else))").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("All branch classes are pre-compiled and CSS-hashed at compile time. Reactive closures switch static classes at runtime with zero string allocation.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            button(rx!(ctx; if *$is_active { "✓ Active State Enabled" } else { "Click to Toggle State" }))
                .class(btn_cls)
                .on_click(move |_| {
                    set_is_active.update(|a| *a = !*a)?;
                    Ok(())
                }),
            div(chain!(
                span("✓ Pre-compiled CSS Hash Branches").class(tw!("text-xs font-semibold text-white bg-violet-600 px-3 py-1.5 rounded-lg border border-solid border-violet-700")),
                span("✓ Zero String Allocations").class(tw!("text-xs font-semibold text-white bg-violet-600 px-3 py-1.5 rounded-lg border border-solid border-violet-700"))
            )).class(tw!("flex flex-wrap gap-2 mt-4"))
        ))
    ))
    .class(card_cls))
}

#[component]
fn NewSyntaxExpansionDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("9. Extended Utilities System").class(tw!("text-xs font-black text-amber-600 dark:text-amber-400 uppercase tracking-widest")),
            h2("Rings, Gradients, Divide & Space").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Support for Ring halos, multi-color gradients, child divide borders, and element spacing.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            // 1. Ring & Offset
            div(chain!(
                span("Ring & Focus Halos").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    button("Ring-2 Halo").class(tw!("px-3.5 py-1.5 bg-indigo-600 text-white text-xs font-bold rounded-xl cursor-pointer ring-2 ring-indigo-500/50 ring-offset-2 ring-offset-white dark:ring-offset-slate-900 transition-all")),
                    button("Ring-4 Rose Halo").class(tw!("px-3.5 py-1.5 bg-rose-600 text-white text-xs font-bold rounded-xl cursor-pointer ring-4 ring-rose-500/40 ring-offset-2 ring-offset-white dark:ring-offset-slate-900 transition-all"))
                )).class(tw!("flex flex-wrap gap-3"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // 2. Linear Gradients
            div(chain!(
                span("Linear Gradient System").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    div("Indigo -> Purple -> Pink Gradient")
                        .class(tw!("p-3.5 bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 text-white font-bold text-xs rounded-xl shadow-md text-center")),
                    div("Emerald -> Cyan Gradient")
                        .class(tw!("p-3.5 bg-gradient-to-br from-emerald-500 to-cyan-500 text-white font-bold text-xs rounded-xl shadow-md text-center"))
                )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-3"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // 3. Divide & Space System
            div(chain!(
                span("Child Divide & Space System").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(chain!(
                    div("Item 1: divide-y & space-y-1").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200")),
                    div("Item 2: Automatic child border insertion").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200")),
                    div("Item 3: Zero custom selector code").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200"))
                )).class(tw!("p-3 bg-white dark:bg-slate-900 rounded-xl border border-solid border-slate-200 dark:border-slate-800 divide-y divide-slate-200 dark:divide-slate-800 space-y-1"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // Badges
            div(chain!(
                span("✓ ring-2 ring-indigo-500/50").class(tw!("text-xs font-semibold text-white bg-amber-600 px-3 py-1.5 rounded-lg border border-solid border-amber-700")),
                span("✓ divide-y & space-y-1").class(tw!("text-xs font-semibold text-white bg-amber-600 px-3 py-1.5 rounded-lg border border-solid border-amber-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        )).class(tw!("flex flex-col gap-3.5"))
    ))
    .class(card_container_cls())
}

#[component]
fn FractionalAndDirectionalDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("10. Fractional Sizing & Inset Positioning").class(tw!("text-xs font-black text-teal-600 dark:text-teal-400 uppercase tracking-widest")),
            h2("Fractions (w-1/2, w-1/3) & Floating Insets").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Supports fractional sizing, floating inset position (-top-3 left-1/2 -translate-x-1/2), and directional borders.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            // Card with top accent border and floating centered badge
            div(chain!(
                // Floating Badge positioned with -top-3 left-1/2 -translate-x-1/2
                span("★ Floating Badge (-top-3 left-1/2 -translate-x-1/2)")
                    .class(tw!("absolute -top-3 left-1/2 -translate-x-1/2 px-3.5 py-1 bg-gradient-to-r from-teal-500 to-indigo-600 text-white text-xs font-bold rounded-full shadow-md whitespace-nowrap")),

                h3("Diagonal Rounded & Accent Top Border")
                    .class(tw!("text-sm font-bold text-slate-900 dark:text-white mb-2 pt-2")),

                // Fractional Width Progress Bars
                div(chain!(
                    span("w-1/2 Progress Bar (50%)").class(tw!("text-xs font-mono text-slate-700 dark:text-slate-300 font-bold mb-1 block")),
                    div(
                        div("").class(tw!("w-1/2 h-full bg-teal-500 rounded-full"))
                    ).class(tw!("w-full h-2.5 bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden mb-3 p-0.5")),

                    span("w-1/3 Progress Bar (33.3%)").class(tw!("text-xs font-mono text-slate-700 dark:text-slate-300 font-bold mb-1 block")),
                    div(
                        div("").class(tw!("w-1/3 h-full bg-emerald-500 rounded-full"))
                    ).class(tw!("w-full h-2.5 bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden p-0.5"))
                ))
            ))
            .class(tw!("relative p-5 bg-slate-50 dark:bg-slate-900 border-t-4 border-teal-500 rounded-tl-3xl rounded-br-3xl rounded-tr-lg rounded-bl-lg border border-solid border-slate-200 dark:border-slate-800 shadow-sm transition-colors duration-300 mb-4")),

            // Badges
            div(chain!(
                span("✓ w-1/2 & w-1/3 Fractions").class(tw!("text-xs font-semibold text-white bg-teal-600 px-3 py-1.5 rounded-lg border border-solid border-teal-700")),
                span("✓ Floating Inset Positioning").class(tw!("text-xs font-semibold text-white bg-teal-600 px-3 py-1.5 rounded-lg border border-solid border-teal-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn TailwindVariantsCvaDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let (intent, set_intent) = scope.signal("primary".to_string())?;
    let (size, set_size) = scope.signal("md".to_string())?;

    let button_variants = tw_variants! {
        base: "font-semibold rounded-xl transition-all duration-300 flex items-center justify-center cursor-pointer border-0 outline-none shadow-md hover:scale-105",
        variants: {
            intent: {
                primary: "bg-indigo-600 text-white hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600 shadow-md",
                secondary: "bg-slate-200 text-slate-800 hover:bg-slate-300 dark:bg-slate-800 dark:text-white",
                danger: "bg-rose-600 text-white hover:bg-rose-700 dark:bg-rose-500 dark:hover:bg-rose-600 shadow-md"
            },
            size: {
                sm: "text-xs px-3 py-1.5 gap-1.5",
                md: "text-sm px-4 py-2 gap-2",
                lg: "text-base px-6 py-3 gap-3"
            }
        },
        default_variants: {
            intent: "primary",
            size: "md"
        },
        compound_variants: [
            {
                intent: "danger",
                size: "lg",
                class: "ring-4 ring-rose-500/30 dark:ring-rose-400/40"
            }
        ]
    };

    let btn_cls = rx!(ctx; button_variants.get($intent, $size));

    Ok(div(chain!(
        div(chain!(
            span("11. Tailwind CVA Paradigm (tw_variants!)").class(tw!("text-xs font-black text-rose-600 dark:text-rose-400 uppercase tracking-widest")),
            h2("Class Variance Authority & Compile-Time Merge").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Compose complex component variants (Intent, Size) with zero runtime overlap, automatic tw! wrapping, and declare_variants! delegation.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            // Interactive Variant Selectors
            div(chain!(
                div(chain!(
                    span("Intent:").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 mr-2")),
                    button("Primary").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$intent == "primary"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_intent.set("primary".to_string())?;
                        Ok(())
                    }),
                    button("Secondary").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$intent == "secondary"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_intent.set("secondary".to_string())?;
                        Ok(())
                    }),
                    button("Danger").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$intent == "danger"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_intent.set("danger".to_string())?;
                        Ok(())
                    })
                )).class(tw!("flex flex-wrap items-center gap-1.5 mb-3")),
                div(chain!(
                    span("Size:").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 mr-2")),
                    button("Small (sm)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$size == "sm"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_size.set("sm".to_string())?;
                        Ok(())
                    }),
                    button("Medium (md)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$size == "md"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_size.set("md".to_string())?;
                        Ok(())
                    }),
                    button("Large (lg)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (rx!(ctx; *$size == "lg"), "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| {
                        set_size.set("lg".to_string())?;
                        Ok(())
                    })
                )).class(tw!("flex flex-wrap items-center gap-1.5"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Live Rendered Variant Button Showcase
            div(chain!(
                button(rx!(ctx; format!("⚡ Variant Button ({}, {})", $intent, $size)))
                    .class(btn_cls)
            )).class(tw!("flex justify-center p-6 bg-slate-100 dark:bg-slate-950 rounded-2xl border border-solid border-slate-200 dark:border-slate-800/60 mb-4")),

            // Badges
            div(chain!(
                span("✓ Cartesian Combination AST Pre-compilation").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700")),
                span("✓ Compound Variant Override (Danger + Lg)").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls()))
}

#[component]
fn SilexTomlDesignTokensDemo<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    div(chain!(
        div(chain!(
            span("12. silex.toml & Design Tokens").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("silex.toml Static Token Linkage").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Reads custom colors (bg-brand-primary, text-brand-accent) & custom breakpoints (3xl:) directly from silex.toml with compile-time Levenshtein typo suggestions.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(chain!(
            // Custom Color Token Swatches
            div(chain!(
                span("silex.toml Defined Tokens").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2.5 block")),
                div(chain!(
                    div("bg-brand-primary").class(tw!("p-2.5 bg-brand-primary text-white text-xs font-mono font-bold rounded-xl text-center shadow-md")),
                    div("text-brand-accent").class(tw!("p-2.5 bg-slate-100 dark:bg-slate-900 text-brand-accent text-xs font-mono font-bold rounded-xl text-center border border-solid border-slate-200 dark:border-slate-800 shadow-sm")),
                    div("bg-brand-success/20").class(tw!("p-2.5 bg-brand-success/20 text-emerald-600 dark:text-emerald-400 text-xs font-mono font-bold rounded-xl text-center border border-solid border-emerald-500/30")),
                    div("border-brand-warning").class(tw!("p-2.5 bg-amber-500/10 text-amber-600 dark:text-amber-400 text-xs font-mono font-bold rounded-xl text-center border-2 border-solid border-brand-warning"))
                )).class(tw!("grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Badges
            div(chain!(
                span("✓ bg-brand-primary (#6366f1)").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700")),
                span("✓ 3xl: (min-width: 1920px)").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700")),
                span("✓ Levenshtein Typo Check").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[derive(Clone, Copy)]
struct CardMeta {
    id: usize,
    category: DemoCategory,
    height: u32,
}

const CARDS_REGISTRY: &[CardMeta] = &[
    CardMeta {
        id: 1,
        category: DemoCategory::Core,
        height: 180,
    },
    CardMeta {
        id: 2,
        category: DemoCategory::Core,
        height: 220,
    },
    CardMeta {
        id: 3,
        category: DemoCategory::Interactive,
        height: 210,
    },
    CardMeta {
        id: 4,
        category: DemoCategory::Interactive,
        height: 230,
    },
    CardMeta {
        id: 5,
        category: DemoCategory::Advanced,
        height: 180,
    },
    CardMeta {
        id: 6,
        category: DemoCategory::Advanced,
        height: 230,
    },
    CardMeta {
        id: 7,
        category: DemoCategory::Core,
        height: 260,
    },
    CardMeta {
        id: 8,
        category: DemoCategory::Interactive,
        height: 190,
    },
    CardMeta {
        id: 9,
        category: DemoCategory::Core,
        height: 350,
    },
    CardMeta {
        id: 10,
        category: DemoCategory::Advanced,
        height: 240,
    },
    CardMeta {
        id: 11,
        category: DemoCategory::Advanced,
        height: 380,
    },
    CardMeta {
        id: 12,
        category: DemoCategory::Advanced,
        height: 260,
    },
];

fn render_card<'scope>(
    scope: Scope<'scope>,
    id: usize,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let ctx = SilexContext::new(scope, error_handler);
    macro_rules! render_card {
        ($card_id:expr, { $($id:literal => $builder:expr),+ $(,)? }) => {
            match $card_id {
                $($id => $builder.build().into_any(),)+
                _ => ().into_any(),
            }
        };
    }

    render_card!(id, {
        1 => TailwindMergeDemo(ctx),
        2 => KeyframesDemo(ctx),
        3 => GroupAndPeerDemo(ctx),
        4 => FiltersAndReactivityDemo(ctx),
        5 => ThemeSystemAndDiagnosticsDemo(ctx),
        6 => ContainerQueriesAndDceDemo(ctx),
        7 => StandardColorPaletteDemo(ctx),
        8 => ReactiveConditionalTwDemo(ctx),
        9 => NewSyntaxExpansionDemo(ctx),
        10 => FractionalAndDirectionalDemo(ctx),
        11 => TailwindVariantsCvaDemo(ctx),
        12 => SilexTomlDesignTokensDemo(ctx),
    })
}

fn is_in_left_column(target_id: usize, current_cat: DemoCategory) -> bool {
    let mut left_h = 0u32;
    let mut right_h = 0u32;

    for card in CARDS_REGISTRY {
        if current_cat != DemoCategory::All && card.category != current_cat {
            continue;
        }
        let is_left = left_h <= right_h;
        if card.id == target_id {
            return is_left;
        }
        if is_left {
            left_h += card.height;
        } else {
            right_h += card.height;
        }
    }
    false
}

fn render_column<'scope>(
    scope: Scope<'scope>,
    is_left: bool,
    category: Persistent<'scope, DemoCategory>,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<impl View<'scope>> {
    let visible_card_ids = category.map(
        scope,
        move |cat| {
            CARDS_REGISTRY
                .iter()
                .copied()
                .filter(move |card| {
                    current_cat_matches(card.category, *cat)
                        && (is_in_left_column(card.id, *cat) == is_left)
                })
                .map(|card| card.id)
                .collect::<Vec<usize>>()
        },
        error_handler,
    )?;

    let ctx = SilexContext::new(scope, error_handler);
    Ok(Index(ctx, visible_card_ids)
        .children(move |id, _| render_card(scope, id, error_handler))
        .build())
}

fn current_cat_matches(card_cat: DemoCategory, current_cat: DemoCategory) -> bool {
    current_cat == DemoCategory::All || card_cat == current_cat
}

#[component]
fn App<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let is_dark = Persistent::builder(scope, "silex-tailwind-dark", error_handler)
        .local()
        .parse::<bool>()
        .default(true)
        .build()?;

    let category = Persistent::builder(scope, "silex-tailwind-category", error_handler)
        .local()
        .parse::<DemoCategory>()
        .default(DemoCategory::All)
        .build()?;

    Ok(div(chain!(
        div(chain!(
            Header(ctx, is_dark, category).build(),

            // Dashboard Content Grid: Dynamic Height-Sensing Greedy Masonry Allocation
            div(chain!(
                div(render_column(scope, true, category, error_handler)?).class(tw!("flex flex-col gap-6 w-full")),
                div(render_column(scope, false, category, error_handler)?).class(tw!("flex flex-col gap-6 w-full"))
            ))
            .class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
        ))
        .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    ))
    .class(rx!(ctx; if *$is_dark { "dark" } else { "" })))
}

/// Mount the Tailwind showcase into the conventional `#app` target.
pub fn mount_tailwind() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_tailwind_view)?;
    bootstrap.into_js_host()
}

/// Mount the Tailwind showcase into a caller-provided target node.
pub fn mount_tailwind_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    bootstrap.mount(Runtime::new(), mount_tailwind_view)?;
    bootstrap.into_js_host()
}

fn mount_tailwind_view<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let scope = ctx.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let silex_ctx = SilexContext::new(scope, error_handler);
    ctx.mount(App(silex_ctx).build(), error_handler)
}
