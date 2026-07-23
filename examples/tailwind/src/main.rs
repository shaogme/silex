use silex::persist::Persistent;
use silex::prelude::*;

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
fn CategoryTab(
    label: &'static str,
    target: DemoCategory,
    category: Persistent<DemoCategory>,
) -> impl View {
    let is_active = rx!(move || category.get() == target);
    button(label)
        .class(tw!(
            "px-4 py-2 text-xs rounded-xl transition-all duration-200 cursor-pointer border-0 outline-none",
            (
                is_active.get(),
                "bg-indigo-600 text-white font-bold shadow-md",
                "bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700 font-semibold"
            )
        ))
        .on_click(move |_| category.set(target))
}

#[component]
fn FeatureBadge(label: &'static str, theme: &'static str) -> impl View {
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
fn Header(is_dark: Persistent<bool>, category: Persistent<DemoCategory>) -> impl View {
    let categories = Constant::new(vec![
        ("All Highlights", DemoCategory::All),
        ("Core Engine", DemoCategory::Core),
        ("Interactive & Reactivity", DemoCategory::Interactive),
        ("Advanced (Phases 4-7)", DemoCategory::Advanced),
    ]);

    div(view_chain!(
        // Top Toolbar Row: Badge, Statuses & Theme Toggle
        div(view_chain!(
            div(view_chain!(
                span("⚡ Silex Tailwind Proc-Macro").class(tw!(
                    "text-xs font-black uppercase tracking-widest px-3.5 py-1.5 bg-indigo-50 dark:bg-indigo-950/80 text-indigo-600 dark:text-indigo-400 rounded-full border border-solid border-indigo-200 dark:border-indigo-800/60 shadow-sm"
                )),
                span("v0.1.0-beta.8 • Full Utility Coverage").class(tw!(
                    "hidden sm:inline-block text-xs font-semibold text-slate-500 dark:text-slate-400"
                ))
            )).class(tw!("flex items-center gap-3")),

            button(rx!(if *$is_dark { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                .class(tw!(
                    "flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                ))
                .on_click(move |_| is_dark.update(|d| *d = !*d))
        )).class(tw!("w-full flex items-center justify-between mb-8")),

        // Hero Title & Description
        h1("Compile-Time Utility-First CSS Engine")
            .class(tw!("text-3xl sm:text-5xl font-black text-slate-900 dark:text-white tracking-tight mb-4 transition-colors duration-300")),
        p("Zero-runtime overhead Tailwind CSS parsed, merged, and optimized into compact AST classes at compile time via LightningCSS. Fully responsive with dynamic signal reactivity.")
            .class(tw!("text-sm sm:text-base text-slate-600 dark:text-slate-300 max-w-3xl text-center leading-relaxed mb-8 transition-colors duration-300")),

        // Dashboard Category Tabs rendered via Index component
        div(Index(categories).children(move |item_sig, _| {
            let (label, target) = item_sig.get();
            CategoryTab(label, target, category)
        }))
        .class(tw!("flex flex-wrap items-center justify-center gap-2 p-1.5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800"))
    ))
    .class(tw!("w-full max-w-6xl mx-auto mb-10 p-8 sm:p-10 bg-white dark:bg-slate-850 rounded-3xl border border-solid border-slate-200 dark:border-slate-800 shadow-xl transition-colors duration-300 flex flex-col items-center text-center"))
}

// Card Wrapper for Consistency
fn card_container_cls() -> &'static str {
    tw!(
        "p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700/80 rounded-3xl shadow-lg hover:shadow-xl transition-all duration-300 flex flex-col h-fit"
    )
}

#[component]
fn TailwindMergeDemo() -> impl View {
    // 示范编译期智能消解: p-2 被 p-6 覆盖, bg-red-500 被 bg-white/dark:bg-slate-800 覆盖
    div(view_chain!(
        div(view_chain!(
            span("1. Compile-Time AST Merge").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("Smart Property Deduplication (Last-Wins)").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Multiple conflicting utilities (p-2 vs p-6, red vs background) are resolved in AST macro parsing phase.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            span("Input: tw!(\"p-2 p-6 bg-red-500 dark:bg-slate-800 ...\")")
                .class(tw!("text-xs font-mono text-slate-800 dark:text-slate-200 bg-slate-100 dark:bg-slate-900 p-3.5 rounded-xl border border-solid border-slate-200 dark:border-slate-800 block mb-4 transition-colors duration-300 overflow-x-auto")),
            div(view_chain!(
                span("✓ Computed Padding: 1.5rem (24px)").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ AST Override: bg-red-500 Removed").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn KeyframesDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("2. Preset Keyframes Engine").class(tw!("text-xs font-black text-purple-600 dark:text-purple-400 uppercase tracking-widest")),
            h2("Zero-Config Built-in Keyframes").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(view_chain!(
            // Spin
            div(view_chain!(
                div(()).class(tw!("size-7 border-2 border-solid border-indigo-600 dark:border-indigo-400 border-t-transparent dark:border-t-transparent rounded-full animate-spin mb-3")),
                span("animate-spin").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("360° Loop").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Pulse
            div(view_chain!(
                div(()).class(tw!("size-7 bg-purple-500 rounded-xl animate-pulse mb-3")),
                span("animate-pulse").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Glow Fade").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Bounce
            div(view_chain!(
                div("↓").class(tw!("size-7 bg-pink-500 text-white font-bold flex items-center justify-center rounded-full animate-bounce mb-3 text-xs")),
                span("animate-bounce").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Bouncing").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Ping
            div(view_chain!(
                div(()).class(tw!("size-7 bg-cyan-400 rounded-full animate-ping mb-3")),
                span("animate-ping").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Beacon").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-0.5"))
            )).class(tw!("flex flex-col items-center p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-2 sm:grid-cols-4 gap-3"))
    ))
    .class(card_container_cls())
}

#[component]
fn GroupAndPeerDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("3. Compound State Selectors").class(tw!("text-xs font-black text-pink-600 dark:text-pink-400 uppercase tracking-widest")),
            h2("Group & Peer Lifted Hover/Focus").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(view_chain!(
            // Group Hover Container
            div(view_chain!(
                span("Group Hover Card (Hover Card Below)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    span("★").class(tw!("text-base text-amber-300 transition-all duration-300 group-hover:rotate-180 group-hover:scale-125")),
                    span("Group Hover Reaction").class(tw!("text-xs font-bold text-white font-mono"))
                )).class(tw!("flex items-center gap-2.5 px-4 py-2.5 bg-indigo-600 rounded-xl cursor-pointer transition-all duration-300 group-hover:scale-105 shadow-md"))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 group transition-all duration-300")),

            // Peer Focus Form Input
            div(view_chain!(
                span("Peer Focus Input").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    input().class(tw!("w-full box-border px-3.5 py-2 bg-white dark:bg-slate-800 border border-solid border-slate-300 dark:border-slate-700 rounded-xl text-xs text-slate-900 dark:text-white peer outline-none transition-colors duration-300")),
                    span("✓ Peer Input Focused!").class(tw!("hidden peer-focus:block text-xs font-semibold text-sky-600 dark:text-sky-400 mt-2 px-3 py-1 bg-slate-100 dark:bg-slate-900 rounded-lg border border-solid border-slate-300 dark:border-slate-800"))
                ))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-4"))
    ))
    .class(card_container_cls())
}

#[component]
fn FiltersAndReactivityDemo() -> impl View {
    let (count, set_count) = Signal::pair(16);
    let pad_val = rx!(format!("{}px", count.get()));

    div(view_chain!(
        div(view_chain!(
            span("4. Filters & Dynamic Signals").class(tw!("text-xs font-black text-sky-600 dark:text-sky-400 uppercase tracking-widest")),
            h2("Glassmorphism & Rust Signals").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-5 transition-colors duration-300"))
        )),
        div(view_chain!(
            // Glassmorphism Filter Card
            div(view_chain!(
                span("Backdrop Blur Filter").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div("Hover to Remove Blur")
                    .class(tw!("p-4 bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-xl blur-sm hover:blur-none transition-all duration-300 cursor-pointer text-center font-bold text-xs text-indigo-600 dark:text-indigo-400"))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // Dynamic Signal Interpolation
            div(view_chain!(
                span("Signal Class $(pad_val)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    button("Increase Padding").class(tw!("px-3 py-1.5 bg-indigo-600 text-white text-xs font-bold rounded-lg cursor-pointer transition-all mb-2.5"))
                        .on_click(move |_| set_count.update(|n| *n = (*n + 4).min(36))),
                    div(rx!(format!("Padding: {}px", count.get())))
                        .class(tw!("p-[$(pad_val)] bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-xl font-mono text-xs text-indigo-600 dark:text-indigo-400 transition-all duration-200 text-center"))
                ))
            )).class(tw!("p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-4"))
    ))
    .class(card_container_cls())
}

#[component]
fn ThemeSystemAndDiagnosticsDemo() -> impl View {
    let theme_box_cls = tw_verbose!(
        "p-4 bg-theme(primary/50) text-theme(border/80) border border-solid border-slate-300 dark:border-slate-700 rounded-2xl shadow-sm transition-colors duration-300"
    );

    div(view_chain!(
        div(view_chain!(
            span("5. Theme System & Diagnostics").class(tw!("text-xs font-black text-sky-600 dark:text-sky-400 uppercase tracking-widest")),
            h2("Opacity Suffix & AST Inspection").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Supports bg-theme(primary/50) opacity suffix via CSS color-mix() and tw_verbose! compile-time CSS AST inspection.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            div(view_chain!(
                span("tw_verbose!(\"p-4 bg-theme(primary/50) ...\")").class(tw!("text-xs font-mono text-slate-800 dark:text-slate-200 block mb-2")),
                div(view_chain!(
                    span("✓ Theme Opacity: color-mix(...)").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700")),
                    span("✓ Levenshtein Typo Diagnostic").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700"))
                )).class(tw!("flex flex-wrap gap-2"))
            )).class(theme_box_cls)
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn ContainerQueriesAndDceDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("6. Container Queries & DCE").class(tw!("text-xs font-black text-emerald-600 dark:text-emerald-500 uppercase tracking-widest")),
            h2("Component Container Responsive @sm").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Element-level responsive queries (@sm:, @[400px]:) paired with compile-time dead @keyframes elimination.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            div(view_chain!(
                span("Container Box (@container)").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    span("Responds to Container Width rather than Viewport").class(tw!("text-xs font-mono text-slate-900 dark:text-white block font-bold")),
                    span("@sm:p-6 @[400px]:bg-slate-100 dark:@[400px]:bg-slate-800").class(tw!("text-xs text-slate-500 dark:text-slate-400 mt-1 block"))
                )).class(tw!("p-4 bg-white dark:bg-slate-900 rounded-xl border border-solid border-slate-200 dark:border-slate-800 @sm:p-6 @[400px]:bg-slate-100 @[400px]:dark:bg-slate-800 transition-colors duration-300"))
            )).class(tw!("@container p-3 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),
            div(view_chain!(
                span("✓ @container: inline-size").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ Zero Unused @keyframes").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn StandardColorPaletteDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("7. Standard Color Palette System").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("22 Color Families & /alpha Opacity").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("22 standard Tailwind color families with 50~950 shade steps & /alpha opacity suffixes.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            // Palette Swatches Showcase
            div(view_chain!(
                span("Standard Swatches Showcase").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2.5 block")),
                div(view_chain!(
                    div("slate-900").class(tw!("p-2.5 bg-slate-900 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("indigo-600").class(tw!("p-2.5 bg-indigo-600 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("emerald-500").class(tw!("p-2.5 bg-emerald-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("rose-500").class(tw!("p-2.5 bg-rose-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("amber-500").class(tw!("p-2.5 bg-amber-500 text-white text-xs font-mono font-bold rounded-lg text-center shadow-sm")),
                    div("sky-400").class(tw!("p-2.5 bg-sky-400 text-slate-900 text-xs font-mono font-bold rounded-lg text-center shadow-sm"))
                )).class(tw!("grid grid-cols-3 sm:grid-cols-6 gap-2 mb-4"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Multi-Property & Opacity Suffix Badges
            div(view_chain!(
                span("✓ bg-indigo-600/50: rgba(...)").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700")),
                span("✓ border-rose-500/25").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn ReactiveConditionalTwDemo() -> impl View {
    let (is_active, set_is_active) = Signal::pair(false);

    let card_cls = tw!(
        "p-6 bg-white dark:bg-slate-800 border rounded-3xl shadow-lg hover:shadow-xl transition-all duration-300 flex flex-col h-fit",
        (
            is_active.get(),
            "border-indigo-500 bg-indigo-50/50 dark:bg-indigo-950/40",
            "border-slate-200 dark:border-slate-700/80"
        )
    );

    let btn_cls = tw!(
        "px-4 py-2.5 rounded-xl font-bold text-xs cursor-pointer transition-all duration-300 shadow-sm",
        (
            is_active.get(),
            "bg-indigo-600 text-white hover:bg-indigo-700 scale-105"
        ),
        (
            !is_active.get(),
            "bg-slate-100 dark:bg-slate-900 text-slate-900 dark:text-slate-100 hover:bg-slate-200 dark:hover:bg-slate-800"
        )
    );

    div(view_chain!(
        div(view_chain!(
            span("8. Zero-Cost Reactive Conditional Utility").class(tw!("text-xs font-black text-violet-600 dark:text-violet-400 uppercase tracking-widest")),
            h2("Reactive Tuple Syntax tw!(..., (cond, then, else))").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("All branch classes are pre-compiled and CSS-hashed at compile time. Reactive closures switch static classes at runtime with zero string allocation.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            button(rx!(if is_active.get() { "✓ Active State Enabled" } else { "Click to Toggle State" }))
                .class(btn_cls)
                .on_click(move |_| set_is_active.update(|a| *a = !*a)),
            div(view_chain!(
                span("✓ Pre-compiled CSS Hash Branches").class(tw!("text-xs font-semibold text-white bg-violet-600 px-3 py-1.5 rounded-lg border border-solid border-violet-700")),
                span("✓ Zero String Allocations").class(tw!("text-xs font-semibold text-white bg-violet-600 px-3 py-1.5 rounded-lg border border-solid border-violet-700"))
            )).class(tw!("flex flex-wrap gap-2 mt-4"))
        ))
    ))
    .class(card_cls)
}

#[component]
fn NewSyntaxExpansionDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("9. Extended Utilities System").class(tw!("text-xs font-black text-amber-600 dark:text-amber-400 uppercase tracking-widest")),
            h2("Rings, Gradients, Divide & Space").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Support for Ring halos, multi-color gradients, child divide borders, and element spacing.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            // 1. Ring & Offset
            div(view_chain!(
                span("Ring & Focus Halos").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    button("Ring-2 Halo").class(tw!("px-3.5 py-1.5 bg-indigo-600 text-white text-xs font-bold rounded-xl cursor-pointer ring-2 ring-indigo-500/50 ring-offset-2 ring-offset-white dark:ring-offset-slate-900 transition-all")),
                    button("Ring-4 Rose Halo").class(tw!("px-3.5 py-1.5 bg-rose-600 text-white text-xs font-bold rounded-xl cursor-pointer ring-4 ring-rose-500/40 ring-offset-2 ring-offset-white dark:ring-offset-slate-900 transition-all"))
                )).class(tw!("flex flex-wrap gap-3"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // 2. Linear Gradients
            div(view_chain!(
                span("Linear Gradient System").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    div("Indigo -> Purple -> Pink Gradient")
                        .class(tw!("p-3.5 bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 text-white font-bold text-xs rounded-xl shadow-md text-center")),
                    div("Emerald -> Cyan Gradient")
                        .class(tw!("p-3.5 bg-gradient-to-br from-emerald-500 to-cyan-500 text-white font-bold text-xs rounded-xl shadow-md text-center"))
                )).class(tw!("grid grid-cols-1 sm:grid-cols-2 gap-3"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // 3. Divide & Space System
            div(view_chain!(
                span("Child Divide & Space System").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    div("Item 1: divide-y & space-y-1").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200")),
                    div("Item 2: Automatic child border insertion").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200")),
                    div("Item 3: Zero custom selector code").class(tw!("py-1.5 font-mono text-xs text-slate-700 dark:text-slate-200"))
                )).class(tw!("p-3 bg-white dark:bg-slate-900 rounded-xl border border-solid border-slate-200 dark:border-slate-800 divide-y divide-slate-200 dark:divide-slate-800 space-y-1"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // Badges
            div(view_chain!(
                span("✓ ring-2 ring-indigo-500/50").class(tw!("text-xs font-semibold text-white bg-amber-600 px-3 py-1.5 rounded-lg border border-solid border-amber-700")),
                span("✓ divide-y & space-y-1").class(tw!("text-xs font-semibold text-white bg-amber-600 px-3 py-1.5 rounded-lg border border-solid border-amber-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        )).class(tw!("flex flex-col gap-3.5"))
    ))
    .class(card_container_cls())
}

#[component]
fn FractionalAndDirectionalDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("10. Fractional Sizing & Inset Positioning").class(tw!("text-xs font-black text-teal-600 dark:text-teal-400 uppercase tracking-widest")),
            h2("Fractions (w-1/2, w-1/3) & Floating Insets").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Supports fractional sizing, floating inset position (-top-3 left-1/2 -translate-x-1/2), and directional borders.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            // Card with top accent border and floating centered badge
            div(view_chain!(
                // Floating Badge positioned with -top-3 left-1/2 -translate-x-1/2
                span("★ Floating Badge (-top-3 left-1/2 -translate-x-1/2)")
                    .class(tw!("absolute -top-3 left-1/2 -translate-x-1/2 px-3.5 py-1 bg-gradient-to-r from-teal-500 to-indigo-600 text-white text-xs font-bold rounded-full shadow-md whitespace-nowrap")),

                h3("Diagonal Rounded & Accent Top Border")
                    .class(tw!("text-sm font-bold text-slate-900 dark:text-white mb-2 pt-2")),

                // Fractional Width Progress Bars
                div(view_chain!(
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
            div(view_chain!(
                span("✓ w-1/2 & w-1/3 Fractions").class(tw!("text-xs font-semibold text-white bg-teal-600 px-3 py-1.5 rounded-lg border border-solid border-teal-700")),
                span("✓ Floating Inset Positioning").class(tw!("text-xs font-semibold text-white bg-teal-600 px-3 py-1.5 rounded-lg border border-solid border-teal-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn TailwindVariantsCvaDemo() -> impl View {
    let (intent, set_intent) = Signal::pair("primary");
    let (size, set_size) = Signal::pair("md");

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

    let btn_cls = rx!(move || button_variants.get(intent.get(), size.get()));

    div(view_chain!(
        div(view_chain!(
            span("11. Tailwind CVA Paradigm (tw_variants!)").class(tw!("text-xs font-black text-rose-600 dark:text-rose-400 uppercase tracking-widest")),
            h2("Class Variance Authority & Compile-Time Merge").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Compose complex component variants (Intent, Size) with zero runtime overlap, automatic tw! wrapping, and declare_variants! delegation.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            // Interactive Variant Selectors
            div(view_chain!(
                div(view_chain!(
                    span("Intent:").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 mr-2")),
                    button("Primary").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (intent.get() == "primary", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_intent.set("primary")),
                    button("Secondary").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (intent.get() == "secondary", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_intent.set("secondary")),
                    button("Danger").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (intent.get() == "danger", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_intent.set("danger"))
                )).class(tw!("flex flex-wrap items-center gap-1.5 mb-3")),
                div(view_chain!(
                    span("Size:").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 mr-2")),
                    button("Small (sm)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (size.get() == "sm", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_size.set("sm")),
                    button("Medium (md)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (size.get() == "md", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_size.set("md")),
                    button("Large (lg)").class(tw!("px-2.5 py-1 text-xs rounded-lg font-bold transition-all cursor-pointer", (size.get() == "lg", "bg-indigo-600 text-white", "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300"))).on_click(move |_| set_size.set("lg"))
                )).class(tw!("flex flex-wrap items-center gap-1.5"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Live Rendered Variant Button Showcase
            div(view_chain!(
                button(rx!(format!("⚡ Variant Button ({}, {})", intent.get(), size.get())))
                    .class(btn_cls)
            )).class(tw!("flex justify-center p-6 bg-slate-100 dark:bg-slate-950 rounded-2xl border border-solid border-slate-200 dark:border-slate-800/60 mb-4")),

            // Badges
            div(view_chain!(
                span("✓ Cartesian Combination AST Pre-compilation").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700")),
                span("✓ Compound Variant Override (Danger + Lg)").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"))
            )).class(tw!("flex flex-wrap gap-2"))
        ))
    ))
    .class(card_container_cls())
}

#[component]
fn SilexTomlDesignTokensDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("12. silex.toml & Design Tokens").class(tw!("text-xs font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-widest")),
            h2("silex.toml Static Token Linkage").class(tw!("text-xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Reads custom colors (bg-brand-primary, text-brand-accent) & custom breakpoints (3xl:) directly from silex.toml with compile-time Levenshtein typo suggestions.")
                .class(tw!("text-xs text-slate-600 dark:text-slate-300 mb-5 leading-relaxed transition-colors duration-300"))
        )),
        div(view_chain!(
            // Custom Color Token Swatches
            div(view_chain!(
                span("silex.toml Defined Tokens").class(tw!("text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-2.5 block")),
                div(view_chain!(
                    div("bg-brand-primary").class(tw!("p-2.5 bg-brand-primary text-white text-xs font-mono font-bold rounded-xl text-center shadow-md")),
                    div("text-brand-accent").class(tw!("p-2.5 bg-slate-100 dark:bg-slate-900 text-brand-accent text-xs font-mono font-bold rounded-xl text-center border border-solid border-slate-200 dark:border-slate-800 shadow-sm")),
                    div("bg-brand-success/20").class(tw!("p-2.5 bg-brand-success/20 text-emerald-600 dark:text-emerald-400 text-xs font-mono font-bold rounded-xl text-center border border-solid border-emerald-500/30")),
                    div("border-brand-warning").class(tw!("p-2.5 bg-amber-500/10 text-amber-600 dark:text-amber-400 text-xs font-mono font-bold rounded-xl text-center border-2 border-solid border-brand-warning"))
                )).class(tw!("grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4"))
            )).class(tw!("p-3.5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),

            // Badges
            div(view_chain!(
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

fn render_card(id: usize) -> AnyView {
    match id {
        1 => TailwindMergeDemo().into_any(),
        2 => KeyframesDemo().into_any(),
        3 => GroupAndPeerDemo().into_any(),
        4 => FiltersAndReactivityDemo().into_any(),
        5 => ThemeSystemAndDiagnosticsDemo().into_any(),
        6 => ContainerQueriesAndDceDemo().into_any(),
        7 => StandardColorPaletteDemo().into_any(),
        8 => ReactiveConditionalTwDemo().into_any(),
        9 => NewSyntaxExpansionDemo().into_any(),
        10 => FractionalAndDirectionalDemo().into_any(),
        11 => TailwindVariantsCvaDemo().into_any(),
        12 => SilexTomlDesignTokensDemo().into_any(),
        _ => ().into_any(),
    }
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

fn render_column(is_left: bool, category: Persistent<DemoCategory>) -> impl View {
    let visible_card_ids = category.map(move |cat| {
        CARDS_REGISTRY
            .iter()
            .copied()
            .filter(move |card| {
                current_cat_matches(card.category, *cat)
                    && (is_in_left_column(card.id, *cat) == is_left)
            })
            .map(|card| card.id)
            .collect::<Vec<usize>>()
    });

    Index(visible_card_ids).children(|id_sig, _| rx!(move || render_card(id_sig.get())))
}

fn current_cat_matches(card_cat: DemoCategory, current_cat: DemoCategory) -> bool {
    current_cat == DemoCategory::All || card_cat == current_cat
}

#[component]
fn App() -> impl View {
    let is_dark = Persistent::builder("silex-tailwind-dark")
        .local()
        .parse::<bool>()
        .default(true)
        .build();

    let category = Persistent::builder("silex-tailwind-category")
        .local()
        .parse::<DemoCategory>()
        .default(DemoCategory::All)
        .build();

    div(view_chain!(
        div(view_chain!(
            Header(is_dark, category),

            // Dashboard Content Grid: Dynamic Height-Sensing Greedy Masonry Allocation
            div(view_chain!(
                div(render_column(true, category)).class(tw!("flex flex-col gap-6 w-full")),
                div(render_column(false, category)).class(tw!("flex flex-col gap-6 w-full"))
            ))
            .class(tw!("grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto"))
        ))
        .class(tw!("min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    ))
    .class(rx!(if *$is_dark { "dark" } else { "" }))
}

fn main() {
    setup_global_error_handlers();
    mount_to_body(App);
}
