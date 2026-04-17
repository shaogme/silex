use silex::prelude::*;
use std::time::Duration;

#[component]
pub fn Greeting(
    // Explicitly `into` is NOT needed for common types like String, PathBuf, Children, AnyView, and Callback.
    // The macro enables it by default, allowing you to pass string literals directly (e.g., .name("...")) without .into().
    // `default = "..."` specifies a fallback value if the prop is omitted.
    #[prop(default = "World")] name: Signal<String>,
    #[prop(default)] punctuation: String,
) -> impl View {
    let full_punctuation = if punctuation.is_empty() {
        "!".to_string()
    } else {
        punctuation
    };

    div![
        span("Hello, "),
        strong(name).style("color: #007bff"),
        span(full_punctuation),
    ]
    .class("greeting-card")
    .style(style! {
        padding: "10px",
        border: "1px solid var(--slx-theme-border)",
        "border-radius": "4px",
        "margin-bottom": "10px",
        background: "var(--slx-theme-surface)"
    })
}

#[component]
pub fn Counter() -> impl View {
    let (count, set_count) = Signal::new(0);
    let double_count = count * 2; // Operator overloading creates a Memo automatically

    // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
    let timer = StoredValue::new(None::<IntervalHandle>);
    // UI State for the timer
    let (is_running, set_is_running) = Signal::new(false);

    div![
        h3("Interactive Counter"),
        div![
            button("-")
                .attr("disabled", count.less_than_or_equals(0)) // New: Directly pass Signal<bool> to attribute
                .on(event::click, set_count.updater(|n| *n -= 1)),
            strong(count).classes(classes![
                "counter-val",
                "positive" => count.greater_than(0),
                "negative" => count.less_than(0)
            ]),
            button("+").on(event::click, set_count.updater(|n| *n += 1)),
        ]
        .style("display: flex; gap: 10px; align-items: center;"),
        // Auto Increment Demo using set_interval and StoredValue
        div![
            button(rx!(if *$is_running {
                "Stop Auto Inc"
            } else {
                "Start Auto Inc"
            }))
            .on(event::click, move |_| {
                if is_running.get() {
                    if let Some(handle) = timer.get_untracked() {
                        handle.clear();
                    }
                    timer.set_untracked(None);
                    set_is_running.set(false);
                } else if let Ok(handle) = set_interval_with_handle(
                    move || {
                        set_count.update(|n| *n += 1);
                    },
                    Duration::from_millis(1000),
                ) {
                    timer.set_untracked(Some(handle));
                    set_is_running.set(true);
                }
            })
        ]
        .style("margin: 10px 0;"),
        // Manual Input Demo using event_target_value
        div![
            span("Set Value: "),
            input()
                .prop("value", count) // One-way binding from signal to DOM
                .on(event::input, move |e| {
                    let val_str = event_target_value(&e);
                    if let Ok(n) = val_str.parse::<i32>() {
                        set_count.set(n);
                    }
                })
        ]
        .style("margin-bottom: 10px;"),
        div!["Double: ", double_count]
            .classes(rx!(@fn if *$count % 2 == 0 { "even" } else { "odd" }))
            .style("margin-top: 5px; color: #666; font-size: 0.9em;"),
    ]
}

#[component]
pub fn NodeRefDemo() -> impl View {
    use silex::reexports::web_sys::HtmlInputElement;
    let input_ref = NodeRef::<HtmlInputElement>::new();

    div![
        h3("NodeRef Demo"),
        p("Click the button to focus the input field using direct DOM access."),
        input()
            .placeholder("I will be focused...")
            .node_ref(input_ref) // NodeRef 是 Copy 的，无需 clone
            .style("margin-right: 10px; padding: 5px;"),
        button("Focus Input").on(event::click, move |_| {
            if let Some(el) = input_ref.get() {
                let _ = el.focus();
            }
        })
    ]
    .style("padding: 20px; border: 1px dashed var(--slx-theme-border); margin-top: 20px;")
}
#[component]
pub fn SvgIconDemo() -> impl View {
    #[component]
    fn ShieldCheck() -> Element {
        svg(path()
            .attr("stroke-linecap", "round")
            .attr("stroke-linejoin", "round")
            .attr("stroke-width", "2")
            .attr("d", "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"))
        .attr("viewBox", "0 0 24 24")
        .attr("fill", "none")
        .attr("stroke", "currentColor")
        .attr("width", "24")
        .attr("height", "24")
    }

    div![
        h3("SVG Icon forwarding"),
        p("SVG icons with attribute forwarding."),
        div![
            ShieldCheck().style("width: 32px; height: 32px; color: green;"),
            ShieldCheck()
                .style("width: 48px; height: 48px; color: blue; margin-left: 10px; cursor: pointer;")
                .on(event::click, |_| console_log("Icon Clicked!")),
            ShieldCheck()
                .attr("width", "50")
                .attr("height", "50")
                .style("color: red; margin-left: 10px;"),
        ]
        .style("display: flex; align-items: center; padding: 10px; background: var(--slx-theme-surface); border: 1px solid var(--slx-theme-border);")
    ]
    .style("margin-top: 20px;")
}

#[component]
pub fn EventDemo() -> impl View {
    let (name, set_name) = Signal::new("Silex".to_string());
    let (count, set_count) = Signal::new(0);

    let (logs, set_logs) = Signal::new(Vec::<String>::new());
    let payload = "DataPayload".to_string();

    // Since Signal is Copy, we can just move it directly into closures without cloning!
    let on_click = move |_| {
        console_log(format!(
            "Clicked! Name: {}, Count: {}",
            name.get(),
            count.get()
        ));
        set_count.update(|n| *n += 1);
        set_name.update(|n| *n = format!("Silex {}", count.get() + 1));
    };

    let on_click_inner = move |_| {
        // For non-Copy types like String, we clone them manually if needed multiple times
        let owned_data = payload.clone();

        set_logs.update(|l| {
            if l.len() >= 5 {
                l.remove(0);
            }
            l.push(format!("Consumed: {}", owned_data));
        });
    };

    div![
        h3("Event & Closure Demo"),
        p("1. Signals are Copy: You can directly move them into closures without cloning."),
        div![
            p(name.map_fn(|n| format!("Current Name: {}", n))), // map_fn: zero-allocation, no monomorphization bloat
            p(count.map(|c| format!("Current Count: {}", c))), // map: traditional closure, still works for all cases
        ]
        .style("margin-bottom: 10px; font-family: monospace;"),
        button("Log & Update (Standard)")
            .on(event::click, on_click)
            .style("margin-right: 10px;"),
        div![].style("height: 1px; background: #ccc; margin: 15px 0;"),
        p("2. Non-Copy types: Clone manually inside the closure."),
        button("Consume Payload").on(event::click, on_click_inner),
        ul(For::new(
            logs,
            |l| l.clone(),
            |l| li(l).style("font-size: 0.8em;")
        ))
        .style("margin-top: 10px; background: var(--slx-theme-border); opacity: 0.5; padding: 10px; border-radius: 4px;")
    ]
    .style("padding: 20px; border: 1px dashed var(--slx-theme-border); margin-top: 20px;")
}

#[component]
pub fn BasicsPage() -> impl View {
    let name_signal = RwSignal::new("Developer".to_string());

    div![
        h2("Basics"),
        div![
            "Reactive Greeting Name: ",
            "Reactive Greeting Name: ",
            input().bind_value(name_signal),
            button("Submit")
                .attr("disabled", name_signal.read_signal().equals(""))
                .style("margin-left: 10px;")
        ]
        .style("margin-bottom: 15px; padding: 10px; background: var(--slx-theme-surface); border-radius: 4px; border: 1px solid var(--slx-theme-border);"),
        Greeting().name(name_signal),
        Counter(),
        EventDemo(),
        NodeRefDemo(),
        SvgIconDemo(),
        // AttributeDemo omitted for brevity, logic is same as previous
    ]
}
