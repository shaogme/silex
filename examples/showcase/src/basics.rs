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
        border: "1px solid #ddd",
        "border-radius": "4px",
        "margin-bottom": "10px"
    })
}

#[component]
pub fn Counter() -> impl View {
    let (count, set_count) = signal(0);
    let double_count = count * 2; // Operator overloading creates a Memo automatically

    // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
    let timer = StoredValue::new(None::<IntervalHandle>);
    // UI State for the timer
    let (is_running, set_is_running) = signal(false);

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
            button(is_running.map(|r| if *r {
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
            .classes(
                (count % 2)
                    .equals(0)
                    .map(|is_even| if *is_even { "even" } else { "odd" })
            )
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
    .style("padding: 20px; border: 1px dashed #999; margin-top: 20px;")
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
        .style("display: flex; align-items: center; padding: 10px; background: white; border: 1px solid #ddd;")
    ]
    .style("margin-top: 20px;")
}

#[component]
pub fn CloneDemo() -> impl View {
    let (name, set_name) = signal("Silex".to_string());
    let (count, set_count) = signal(0);

    let (logs, set_logs) = signal(Vec::<String>::new());
    let payload = "DataPayload".to_string();

    // 1. Standard clone!
    let on_click = clone!(name, count => move |_| {
        console_log(format!("Clicked! Name: {}, Count: {}", name.get(), count.get()));
        set_count.update(|n| *n += 1);
        set_name.update(|n| *n = format!("Silex {}", count.get() + 1));
    });

    // 2. Inner clone! (@ syntax)
    // Useful when the closure is FnMut (multiple calls) but you need ownership of data inside (e.g., async move)
    let on_click_inner = clone!(set_logs, @payload => move |_| {
        // 'payload' is automatically cloned at the start of this block
        // so we can move it (consume it) without "use of moved value" errors
        let owned_data = payload;

        set_logs.update(|l| {
            if l.len() >= 5 { l.remove(0); }
            l.push(format!("Consumed: {}", owned_data));
        });
    });

    div![
        h3("Clone Macro Demo"),
        p("1. Standard Clone: Captures external variables for use in closure."),
        div![
            p(name.map(|n| format!("Current Name: {}", n))),
            p(count.map(|c| format!("Current Count: {}", c))),
        ]
        .style("margin-bottom: 10px; font-family: monospace;"),
        button("Log & Update (Standard)")
            .on(event::click, on_click)
            .style("margin-right: 10px;"),
        div![].style("height: 1px; background: #ccc; margin: 15px 0;"),
        p("2. Inner Clone (@): Clones variable INSIDE closure to allow moving/consuming it."),
        button("Consume Payload (Inner Clone)").on(event::click, on_click_inner),
        ul(For::new(
            logs,
            |l| l.clone(),
            |l| li(l).style("font-size: 0.8em;")
        ))
        .style("margin-top: 10px; background: #eee; padding: 10px; border-radius: 4px;")
    ]
    .style("padding: 20px; border: 1px dashed #4caf50; margin-top: 20px;")
}

#[component]
pub fn SelectStyleDemo() -> impl View {
    div("Select a style above to see the comparison.")
}

#[component]
pub fn BuilderDemo() -> impl View {
    // Nested structure using purely function calls
    div(
        div(
            // Use a tuple for multiple children if not using macros
            (
                h3("Builder Style"),
                p("This component is built using only function calls and method chaining."),
                button("Click Me (Builder)")
                    .class("btn-builder")
                    .style("background-color: #e0f7fa; color: #006064; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;")
                    .on(event::click, |_| console_log("Builder button clicked!")),
            )
        )
        .style("padding: 20px; border: 1px solid #b2ebf2; border-radius: 8px; margin-bottom: 20px;")
    )
}

#[component]
pub fn MacroDemo() -> impl View {
    let (count, set_count) = signal(0);

    // Note how clean the children list is: plain comma-separated values
    div![
        h3![ "Macro Style" ],
        p![ "This component uses macros for a more declarative, HTML-like feel." ],
        div![
            button![ "-" ]
                .class("btn-macro")
                .on(event::click, set_count.updater(|n| *n -= 1)),
            span![ " Count: ", count, " " ].style("margin: 0 10px; font-weight: bold;"),
            button![ "+" ]
                .class("btn-macro")
                .on(event::click, set_count.updater(|n| *n += 1)),
        ]
        .style("display: flex; align-items: center; margin-top: 10px;")
    ]
    .style("padding: 20px; border: 1px solid #ffccbc; background-color: #fffbe6; border-radius: 8px; margin-bottom: 20px;")
}

#[component]
pub fn HybridDemo() -> impl View {
    let (is_active, set_active) = signal(false);

    div![
        h3("Hybrid Style (Recommended)"), // Function call for single child is fine too!
        p("Mix macros for structure and builder methods for attributes."),

        // "Toggle" Logic
        div![
            span(is_active.map(|v| if *v { "State: Active" } else { "State: Inactive" }))
                .style("margin-right: 15px;"),

            button(is_active.map(|v| if *v { "Deactivate" } else { "Activate" }))
                .on(event::click, move |_| set_active.update(|v| *v = !*v))
                // Dynamic styling with builder pattern
                .style(is_active.map(|v| {
                    if *v {
                        "background-color: #ef5350; color: white;"
                    } else {
                        "background-color: #66bb6a; color: white;"
                    }
                }))
                .style("padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer; transition: background-color 0.2s;")
        ]
    ]
    .style("padding: 20px; border: 1px solid #d1c4e9; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
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
        .style("margin-bottom: 15px; padding: 10px; background: #f8f9fa; border-radius: 4px;"),
        Greeting().name(name_signal),
        Counter(),
        CloneDemo(),
        NodeRefDemo(),
        SvgIconDemo(),
        // AttributeDemo omitted for brevity, logic is same as previous
    ]
}
