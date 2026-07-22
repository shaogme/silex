use silex::prelude::*;

#[component]
fn Header(is_dark: Signal<bool>, set_is_dark: WriteSignal<bool>) -> impl View {
    div(view_chain!(
        div(view_chain!(
            // 顶栏顶层 Bar: 左侧 Label，右侧 Theme Toggle 按钮
            div(view_chain!(
                span("⚡ Silex Tailwind Proc-Macro").class(tw!(
                    "text-xs font-bold text-slate-700 dark:text-slate-900 uppercase tracking-wider px-3 py-1 bg-slate-100 dark:bg-white rounded-full border border-solid border-slate-300 dark:border-white"
                )),
                button(rx!(if is_dark.get() { "🌙 Dark Mode" } else { "☀️ Light Mode" }))
                    .class(tw!(
                        "flex items-center gap-2 px-4 py-1.5 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-amber-400 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
                    ))
                    .on_click(move |_| set_is_dark.update(|d| *d = !*d))
            )).class(tw!("w-full flex items-center justify-between mb-6")),

            h1("Compile-Time Utility-First CSS")
                .class(tw!("text-4xl font-black text-slate-900 dark:text-white tracking-tight mt-2 mb-3 transition-colors duration-300")),
            p("Zero-runtime overhead Tailwind-style utilities parsed, merged, and compiled at compile time via LightningCSS. Supports responsive dark: mode toggle.")
                .class(tw!("text-base text-slate-600 dark:text-white dark:opacity-80 max-w-2xl text-center leading-relaxed transition-colors duration-300"))
        ))
        .class(tw!("flex flex-col items-center text-center p-8 bg-white dark:bg-slate-800 rounded-3xl border border-solid border-slate-200 dark:border-slate-700 shadow-xl transition-colors duration-300"))
    ))
    .class(tw!("w-full max-w-5xl mx-auto mb-10"))
}

#[component]
fn TailwindMergeDemo() -> impl View {
    // 示范编译期智能消解: p-2 被 p-6 覆盖, bg-red-500 被 bg-white/dark:bg-slate-800 覆盖
    let card_cls = tw!(
        "p-2 p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"
    );

    div(view_chain!(
        div(view_chain!(
            span("1. Compile-Time Tailwind Merge").class(tw!("text-sm font-bold text-indigo-600 dark:text-indigo-400 uppercase tracking-wider")),
            h2("Smart Conflict Resolution (Last-Wins)").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Multiple conflicting utility classes like p-2 vs p-6 and red vs background are resolved in proc-macro AST phase.")
                .class(tw!("text-sm text-slate-600 dark:text-white dark:opacity-70 mb-6 transition-colors duration-300"))
        )),
        div(view_chain!(
            span("Input: tw!(\"p-2 p-6 bg-red-500 dark:bg-slate-800 ...\")").class(tw!("text-xs font-mono text-slate-900 dark:text-white bg-slate-100 dark:bg-slate-900 p-4 rounded-xl border border-solid border-slate-300 dark:border-slate-800 block mb-4 transition-colors duration-300")),
            div(view_chain!(
                span("✓ Computed Padding: 1.5rem (24px)").class(tw!("text-xs font-semibold text-white bg-emerald-500 px-3 py-1.5 rounded-lg border border-solid border-emerald-600")),
                span("✓ Computed Responsive Dark Mode: dark:bg-slate-800").class(tw!("text-xs font-semibold text-white bg-emerald-500 px-3 py-1.5 rounded-lg border border-solid border-emerald-600"))
            )).class(tw!("flex flex-wrap gap-3"))
        ))
    ))
    .class(card_cls)
}

#[component]
fn KeyframesDemo() -> impl View {
    div(view_chain!(
        span("2. Preset Keyframe Animation Engine").class(tw!("text-sm font-bold text-purple-600 dark:text-purple-400 uppercase tracking-wider")),
        h2("Zero-Config Built-in Keyframes").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-6 transition-colors duration-300")),
        div(view_chain!(
            // Spin
            div(view_chain!(
                div(()).class(tw!("size-8 border-2 border-solid border-indigo-600 dark:border-white border-t-transparent dark:border-t-transparent rounded-full animate-spin mb-3")),
                span("animate-spin").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Smooth 360° Loop").class(tw!("text-xs text-slate-500 dark:text-white dark:opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Pulse
            div(view_chain!(
                div(()).class(tw!("size-8 bg-purple-500 rounded-xl animate-pulse mb-3")),
                span("animate-pulse").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Glow Fade Effect").class(tw!("text-xs text-slate-500 dark:text-white dark:opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Bounce
            div(view_chain!(
                div("↓").class(tw!("size-8 bg-pink-500 text-white font-bold flex items-center justify-center rounded-full animate-bounce mb-3")),
                span("animate-bounce").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Cubic Bouncing").class(tw!("text-xs text-slate-500 dark:text-white dark:opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),
            // Ping
            div(view_chain!(
                div(()).class(tw!("size-8 bg-cyan-400 rounded-full animate-ping mb-3")),
                span("animate-ping").class(tw!("text-xs font-mono text-slate-900 dark:text-white font-bold")),
                span("Radar Beacon").class(tw!("text-xs text-slate-500 dark:text-white dark:opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-4 gap-4"))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn GroupAndPeerDemo() -> impl View {
    div(view_chain!(
        span("3. Compound State Selectors").class(tw!("text-sm font-bold text-pink-600 dark:text-pink-400 uppercase tracking-wider")),
        h2("Group & Peer Parent/Sibling Lifted Interactions").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-6 transition-colors duration-300")),
        div(view_chain!(
            // Group Hover Container
            div(view_chain!(
                span("Group Hover Container (Hover Entire Card)").class(tw!("text-xs font-bold text-slate-600 dark:text-white dark:opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    span("★").class(tw!("text-lg text-amber-400 transition-all duration-300 group-hover:rotate-180 group-hover:scale-125")),
                    span("Group Hover Reaction Button").class(tw!("text-sm font-bold text-white font-mono"))
                )).class(tw!("flex items-center gap-3 px-5 py-3 bg-indigo-500 rounded-xl cursor-pointer transition-all duration-300 group-hover:scale-105 shadow-lg"))
            )).class(tw!("p-6 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 group transition-all duration-300")),

            // Peer Focus Form Input
            div(view_chain!(
                span("Peer Focus Sibling Input").class(tw!("text-xs font-bold text-slate-600 dark:text-white dark:opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    input().class(tw!("w-full box-border px-4 py-2 bg-white dark:bg-slate-800 border border-solid border-slate-300 dark:border-slate-700 rounded-xl text-slate-900 dark:text-white peer outline-none transition-colors duration-300")),
                    span("Focus sibling input to reveal this peer badge!").class(tw!("hidden peer-focus:block text-xs font-semibold text-sky-600 dark:text-sky-400 mt-2 px-3 py-1 bg-slate-100 dark:bg-slate-900 rounded-lg border border-solid border-slate-300 dark:border-slate-800"))
                ))
            )).class(tw!("p-6 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-2 gap-6"))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn FiltersAndReactivityDemo() -> impl View {
    let (count, set_count) = Signal::pair(16);
    let pad_val = rx!(format!("{}px", count.get()));

    div(view_chain!(
        span("4. Filters, Transforms & Rust Dynamic Signals").class(tw!("text-sm font-bold text-sky-600 dark:text-sky-400 uppercase tracking-wider")),
        h2("Glassmorphic Filters & Dynamic Expression Interpolation").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-6 transition-colors duration-300")),
        div(view_chain!(
            // Glassmorphism Filter Card
            div(view_chain!(
                span("Hover to Remove Blur Filter").class(tw!("text-xs font-bold text-slate-600 dark:text-white dark:opacity-70 uppercase tracking-wider mb-2 block")),
                div("Backdrop Blur & Hover Filter Effect")
                    .class(tw!("p-6 bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-2xl blur-sm hover:blur-none transition-all duration-300 cursor-pointer text-center font-bold text-indigo-600 dark:text-indigo-400"))
            )).class(tw!("p-6 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300")),

            // Dynamic Signal Interpolation
            div(view_chain!(
                span("Rust Reactive Signal Embedding $(expr)").class(tw!("text-xs font-bold text-slate-600 dark:text-white dark:opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    button("Click to Increase Padding").class(tw!("px-4 py-2 bg-indigo-500 text-white text-xs font-bold rounded-lg cursor-pointer transition-all mb-3"))
                        .on_click(move |_| set_count.update(|n| *n = (*n + 4).min(40))),
                    div(rx!(format!("Dynamic Computed Class Padding: {}px", count.get())))
                        .class(tw!("p-[$(pad_val)] bg-slate-100 dark:bg-slate-900 border border-solid border-slate-300 dark:border-slate-800 rounded-xl font-mono text-xs text-indigo-600 dark:text-indigo-400 transition-all duration-200 text-center"))
                ))
            )).class(tw!("p-6 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 transition-colors duration-300"))
        )).class(tw!("grid grid-cols-2 gap-6"))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn ThemeSystemAndDiagnosticsDemo() -> impl View {
    let theme_box_cls = tw_verbose!(
        "p-6 bg-theme(primary/50) text-theme(border/80) border border-solid border-slate-300 dark:border-slate-700 rounded-2xl shadow-lg backdrop-blur-md transition-colors duration-300"
    );

    div(view_chain!(
        div(view_chain!(
            span("5. Theme System & Proc Diagnostics").class(tw!("text-sm font-bold text-sky-600 dark:text-sky-400 uppercase tracking-wider")),
            h2("Opacity Suffix & Compile-Time Diagnostics").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Supports bg-theme(primary/50) opacity suffix via CSS color-mix() and tw_verbose! compile-time CSS AST inspection.")
                .class(tw!("text-sm text-slate-600 dark:text-white dark:opacity-70 mb-6 transition-colors duration-300"))
        )),
        div(view_chain!(
            div(view_chain!(
                span("tw_verbose!(\"p-6 bg-theme(primary/50) text-theme(border/80) ...\")").class(tw!("text-xs font-mono text-slate-900 dark:text-white block mb-3")),
                div(view_chain!(
                    span("✓ Theme Opacity: color-mix(in srgb, var(--slx-theme-primary) 50%, transparent)").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700")),
                    span("✓ Levenshtein Spell Check: Did you mean 'flex'?").class(tw!("text-xs font-semibold text-white bg-sky-600 px-3 py-1.5 rounded-lg border border-solid border-sky-700"))
                )).class(tw!("flex flex-wrap gap-3"))
            )).class(theme_box_cls)
        ))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn ContainerQueriesAndDceDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("6. Enterprise & Container Queries (Phase 4)").class(tw!("text-sm font-bold text-emerald-600 dark:text-emerald-500 uppercase tracking-wider")),
            h2("CSS Container Queries & DCE Keyframe Elimination").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Element-level responsive queries (@sm:, @lg:, @[400px]:) combined with compile-time dead keyframe pruning.")
                .class(tw!("text-sm text-slate-600 dark:text-white dark:opacity-70 mb-6 transition-colors duration-300"))
        )),
        div(view_chain!(
            div(view_chain!(
                span("Container Query Box (@container)").class(tw!("text-xs font-bold text-slate-600 dark:text-white dark:opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    span("Resize or View on Different Container Sizes").class(tw!("text-xs font-mono text-slate-900 dark:text-white")),
                    span("Card layout responds to component container width rather than global viewport!").class(tw!("text-xs text-slate-600 dark:text-white dark:opacity-80 mt-1 block"))
                )).class(tw!("p-5 bg-white dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 @sm:p-8 @[400px]:bg-slate-100 @[400px]:dark:bg-slate-800 transition-colors duration-300"))
            )).class(tw!("@container p-4 bg-slate-50 dark:bg-slate-900 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-4 transition-colors duration-300")),
            div(view_chain!(
                span("✓ @container: container-type: inline-size").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ @sm:p-8 @[400px]:bg-...: @container (min-width: ...)").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ Dead-Code-Elimination: Zero unused @keyframes in output").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700"))
            )).class(tw!("flex flex-wrap gap-3"))
        ))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn StandardColorPaletteDemo() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("7. Standard Color Palette System (Phase 5)").class(tw!("text-sm font-bold text-indigo-600 dark:text-indigo-400 uppercase tracking-wider")),
            h2("22 Color Families & Opacity Suffix Conversion").class(tw!("text-2xl font-bold text-slate-900 dark:text-white mt-1 mb-2 transition-colors duration-300")),
            p("Full Tailwind v3/v4 22 color families with 50~950 shade steps, multi-property prefixes (text-, bg-, border-, outline-), and /alpha opacity suffix.")
                .class(tw!("text-sm text-slate-600 dark:text-slate-300 mb-6 transition-colors duration-300"))
        )),
        div(view_chain!(
            // Palette Swatches Showcase
            div(view_chain!(
                span("22 Standard Palette Swatches").class(tw!("text-xs font-bold text-slate-600 dark:text-slate-300 uppercase tracking-wider mb-3 block")),
                div(view_chain!(
                    div("slate-900").class(tw!("p-3 bg-slate-900 text-white text-xs font-mono font-bold rounded-xl text-center shadow")),
                    div("indigo-600").class(tw!("p-3 bg-indigo-600 text-white text-xs font-mono font-bold rounded-xl text-center shadow")),
                    div("emerald-500").class(tw!("p-3 bg-emerald-500 text-white text-xs font-mono font-bold rounded-xl text-center shadow")),
                    div("rose-500").class(tw!("p-3 bg-rose-500 text-white text-xs font-mono font-bold rounded-xl text-center shadow")),
                    div("amber-500").class(tw!("p-3 bg-amber-500 text-white text-xs font-mono font-bold rounded-xl text-center shadow")),
                    div("sky-400").class(tw!("p-3 bg-sky-400 text-slate-900 text-xs font-mono font-bold rounded-xl text-center shadow"))
                )).class(tw!("grid grid-cols-3 md:grid-cols-6 gap-3 mb-6"))
            )).class(tw!("p-5 bg-slate-50 dark:bg-slate-900/60 rounded-2xl border border-solid border-slate-200 dark:border-slate-800 mb-6 transition-colors duration-300")),

            // Multi-Property & Opacity Suffix Badges
            div(view_chain!(
                span("✓ text-slate-900 / dark:text-indigo-400: Palette hex mapping").class(tw!("text-xs font-semibold text-white bg-indigo-600 px-3 py-1.5 rounded-lg border border-solid border-indigo-700")),
                span("✓ bg-indigo-600/50: rgba(79, 70, 229, 0.5)").class(tw!("text-xs font-semibold text-white bg-emerald-600 px-3 py-1.5 rounded-lg border border-solid border-emerald-700")),
                span("✓ border-rose-500/25: rgba(244, 63, 94, 0.25)").class(tw!("text-xs font-semibold text-white bg-rose-600 px-3 py-1.5 rounded-lg border border-solid border-rose-700"))
            )).class(tw!("flex flex-wrap gap-3"))
        ))
    ))
    .class(tw!("p-6 bg-white dark:bg-slate-800 border border-solid border-slate-200 dark:border-slate-700 rounded-3xl shadow-xl transition-colors duration-300"))
}

#[component]
fn App() -> impl View {
    let (is_dark, set_is_dark) = Signal::pair(true);

    div(view_chain!(
        div(view_chain!(
            Header(is_dark, set_is_dark),
            div(view_chain!(
                TailwindMergeDemo(),
                KeyframesDemo(),
                GroupAndPeerDemo(),
                FiltersAndReactivityDemo(),
                ThemeSystemAndDiagnosticsDemo(),
                ContainerQueriesAndDceDemo(),
                StandardColorPaletteDemo()
            ))
            .class(tw!("flex flex-col gap-8 max-w-5xl mx-auto"))
        ))
        .class(tw!("min-h-screen p-6 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50"))
    ))
    .class(rx!(if is_dark.get() { "dark" } else { "" }))
}

fn main() {
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    let app = App();
    app.mount(&app_container, Vec::new());
}
