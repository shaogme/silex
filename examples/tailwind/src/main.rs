use silex::prelude::*;

#[component]
fn Header() -> impl View {
    div(view_chain!(
        div(view_chain!(
            span("⚡ Silex Tailwind Proc-Macro").class(tw!("text-xs font-bold text-[#0f172a] uppercase tracking-wider px-3 py-1 bg-white rounded-full border border-solid")),
            h1("Compile-Time Utility-First CSS")
                .class(tw!("text-4xl font-black text-white tracking-tight mt-2 mb-3")),
            p("Zero-runtime overhead Tailwind-style utilities parsed, merged, and compiled at compile time via LightningCSS.")
                .class(tw!("text-base text-white opacity-80 max-w-2xl text-center leading-relaxed"))
        ))
        .class(tw!("flex flex-col items-center text-center p-8 bg-[#1e293b] rounded-3xl border border-solid shadow-lg backdrop-blur-md"))
    ))
    .class(tw!("w-full max-w-5xl mx-auto mb-10"))
}

#[component]
fn TailwindMergeDemo() -> impl View {
    // 示范编译期智能消解: p-2 被 p-6 覆盖, bg-[#ef4444] 被 bg-[#1e293b] 覆盖
    let card_cls = tw!(
        "p-2 p-6 bg-[#ef4444] bg-[#1e293b] border border-solid rounded-3xl shadow-xl transition-all duration-300"
    );

    div(view_chain!(
        div(view_chain!(
            span("1. Compile-Time Tailwind Merge").class(tw!("text-sm font-bold text-[#818cf8] uppercase tracking-wider")),
            h2("Smart Conflict Resolution (Last-Wins)").class(tw!("text-2xl font-bold text-white mt-1 mb-2")),
            p("Multiple conflicting utility classes like p-2 vs p-6 and red vs dark background are resolved in proc-macro AST phase.")
                .class(tw!("text-sm text-white opacity-70 mb-6"))
        )),
        div(view_chain!(
            span("Input: tw!(\"p-2 p-6 bg-[#ef4444] bg-[#1e293b] ...\")").class(tw!("text-xs font-mono text-white bg-[#0f172a] p-4 rounded-xl border border-solid block mb-4")),
            div(view_chain!(
                span("✓ Computed Padding: 1.5rem (24px)").class(tw!("text-xs font-semibold text-white bg-[#10b981] px-3 py-1.5 rounded-lg border border-solid")),
                span("✓ Computed Background: #1e293b").class(tw!("text-xs font-semibold text-white bg-[#10b981] px-3 py-1.5 rounded-lg border border-solid"))
            )).class(tw!("flex flex-wrap gap-3"))
        ))
    ))
    .class(card_cls)
}

#[component]
fn KeyframesDemo() -> impl View {
    div(view_chain!(
        span("2. Preset Keyframe Animation Engine").class(tw!("text-sm font-bold text-[#c084fc] uppercase tracking-wider")),
        h2("Zero-Config Built-in Keyframes").class(tw!("text-2xl font-bold text-white mt-1 mb-6")),
        div(view_chain!(
            // Spin
            div(view_chain!(
                div(()).class(tw!("size-8 border-2 border-solid border-white border-t-transparent rounded-full animate-spin mb-3")),
                span("animate-spin").class(tw!("text-xs font-mono text-white font-bold")),
                span("Smooth 360° Loop").class(tw!("text-xs text-white opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-[#0f172a] rounded-2xl border border-solid")),
            // Pulse
            div(view_chain!(
                div(()).class(tw!("size-8 bg-[#a855f7] rounded-xl animate-pulse mb-3")),
                span("animate-pulse").class(tw!("text-xs font-mono text-white font-bold")),
                span("Glow Fade Effect").class(tw!("text-xs text-white opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-[#0f172a] rounded-2xl border border-solid")),
            // Bounce
            div(view_chain!(
                div("↓").class(tw!("size-8 bg-[#ec4899] text-white font-bold flex items-center justify-center rounded-full animate-bounce mb-3")),
                span("animate-bounce").class(tw!("text-xs font-mono text-white font-bold")),
                span("Cubic Bouncing").class(tw!("text-xs text-white opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-[#0f172a] rounded-2xl border border-solid")),
            // Ping
            div(view_chain!(
                div(()).class(tw!("size-8 bg-[#22d3ee] rounded-full animate-ping mb-3")),
                span("animate-ping").class(tw!("text-xs font-mono text-white font-bold")),
                span("Radar Beacon").class(tw!("text-xs text-white opacity-60 mt-1"))
            )).class(tw!("flex flex-col items-center p-5 bg-[#0f172a] rounded-2xl border border-solid"))
        )).class(tw!("grid grid-cols-4 gap-4"))
    ))
    .class(tw!("p-6 bg-[#1e293b] border border-solid rounded-3xl shadow-xl"))
}

#[component]
fn GroupAndPeerDemo() -> impl View {
    div(view_chain!(
        span("3. Compound State Selectors").class(tw!("text-sm font-bold text-[#f472b6] uppercase tracking-wider")),
        h2("Group & Peer Parent/Sibling Lifted Interactions").class(tw!("text-2xl font-bold text-white mt-1 mb-6")),
        div(view_chain!(
            // Group Hover Container
            div(view_chain!(
                span("Group Hover Container (Hover Entire Card)").class(tw!("text-xs font-bold text-white opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    span("★").class(tw!("text-lg text-[#fbbf24] transition-all duration-300 group-hover:rotate-180 group-hover:scale-125")),
                    span("Group Hover Reaction Button").class(tw!("text-sm font-bold text-white font-mono"))
                )).class(tw!("flex items-center gap-3 px-5 py-3 bg-[#6366f1] rounded-xl cursor-pointer transition-all duration-300 group-hover:scale-105 shadow-lg"))
            )).class(tw!("p-6 bg-[#0f172a] rounded-2xl border border-solid group transition-all duration-300")),

            // Peer Focus Form Input
            div(view_chain!(
                span("Peer Focus Sibling Input").class(tw!("text-xs font-bold text-white opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    input().class(tw!("w-full box-border px-4 py-2 bg-[#1e293b] border border-solid rounded-xl text-white peer outline-none")),
                    span("Focus sibling input to reveal this peer badge!").class(tw!("hidden peer-focus:block text-xs font-semibold text-[#38bdf8] mt-2 px-3 py-1 bg-[#0f172a] rounded-lg border border-solid"))
                ))
            )).class(tw!("p-6 bg-[#0f172a] rounded-2xl border border-solid"))
        )).class(tw!("grid grid-cols-2 gap-6"))
    ))
    .class(tw!("p-6 bg-[#1e293b] border border-solid rounded-3xl shadow-xl"))
}

#[component]
fn FiltersAndReactivityDemo() -> impl View {
    let (count, set_count) = Signal::pair(16);
    let pad_val = rx!(format!("{}px", count.get()));

    div(view_chain!(
        span("4. Filters, Transforms & Rust Dynamic Signals").class(tw!("text-sm font-bold text-[#38bdf8] uppercase tracking-wider")),
        h2("Glassmorphic Filters & Dynamic Expression Interpolation").class(tw!("text-2xl font-bold text-white mt-1 mb-6")),
        div(view_chain!(
            // Glassmorphism Filter Card
            div(view_chain!(
                span("Hover to Remove Blur Filter").class(tw!("text-xs font-bold text-white opacity-70 uppercase tracking-wider mb-2 block")),
                div("Backdrop Blur & Hover Filter Effect")
                    .class(tw!("p-6 bg-[#0f172a] border border-solid rounded-2xl blur-sm hover:blur-none transition-all duration-300 cursor-pointer text-center font-bold text-[#818cf8]"))
            )).class(tw!("p-6 bg-[#0f172a] rounded-2xl border border-solid")),

            // Dynamic Signal Interpolation
            div(view_chain!(
                span("Rust Reactive Signal Embedding $(expr)").class(tw!("text-xs font-bold text-white opacity-70 uppercase tracking-wider mb-2 block")),
                div(view_chain!(
                    button("Click to Increase Padding").class(tw!("px-4 py-2 bg-[#6366f1] text-white text-xs font-bold rounded-lg cursor-pointer transition-all mb-3"))
                        .on_click(move |_| set_count.update(|n| *n = (*n + 4).min(40))),
                    div(rx!(format!("Dynamic Computed Class Padding: {}px", count.get())))
                        .class(tw!("p-[$(pad_val)] bg-[#0f172a] border border-solid rounded-xl font-mono text-xs text-[#818cf8] transition-all duration-200 text-center"))
                ))
            )).class(tw!("p-6 bg-[#0f172a] rounded-2xl border border-solid"))
        )).class(tw!("grid grid-cols-2 gap-6"))
    ))
    .class(tw!("p-6 bg-[#1e293b] border border-solid rounded-3xl shadow-xl"))
}

#[component]
fn App() -> impl View {
    div(view_chain!(
        Header(),
        div(view_chain!(
            TailwindMergeDemo(),
            KeyframesDemo(),
            GroupAndPeerDemo(),
            FiltersAndReactivityDemo()
        ))
        .class(tw!("flex flex-col gap-8 max-w-5xl mx-auto"))
    ))
    .class(tw!("min-h-screen p-6 bg-[#0f172a]"))
}

fn main() {
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    let app = App();
    app.mount(&app_container, Vec::new());
}
