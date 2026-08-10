use crate::css::AppTheme;
use silex::{core::log::console_log, prelude::*};
use std::time::Duration;

#[component]
pub fn Greeting<'scope>(
    name: Signal<'scope, String>,
    #[chain(default)] punctuation: String,
) -> impl View<'scope> {
    let full_punctuation = if punctuation.is_empty() {
        "!".to_string()
    } else {
        punctuation.clone()
    };

    div![
        span("Hello, "),
        strong(name).style(sty().color(AppTheme::PRIMARY)),
        span(full_punctuation),
    ]
    .class("greeting-card")
    .style(
        sty()
            .padding(px(10))
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))
            .border_radius(px(4))
            .margin_bottom(px(10))
            .background(AppTheme::SURFACE),
    )
}

#[component]
pub fn Counter<'scope>(
    scope: Scope<'scope>,
    #[inject(owner)] owner: ViewOwnerToken<'scope>,
) -> impl View<'scope> {
    let (count, set_count) = scope.signal(0);
    let double_count = count.into_rx() * 2;

    // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
    let timer = scope.stored(None::<HostResourceHandle<'scope>>);
    let owner_for_timer = owner.clone();
    // UI State for the timer
    let (is_running, set_is_running) = scope.signal(false);

    div![
        h3("Interactive Counter"),
        div![
            button("-")
                .attr("disabled", count.less_than_or_equals(scope, 0))
                .on(event::click, set_count.updater(|n| *n -= 1)),
            strong(count).classes(classes![
                "counter-val",
                "positive" => count.greater_than(scope, 0),
                "negative" => count.less_than(scope, 0)
            ]),
            button("+").on(event::click, set_count.updater(|n| *n += 1)),
        ]
        .style("display: flex; gap: 10px; align-items: center;"),
        // Auto Increment Demo using set_interval and StoredValue
        div![
            button(rx!(scope; if *$is_running {
                "Stop Auto Inc"
            } else {
                "Start Auto Inc"
            }))
            .on(event::click, move |_| {
                if is_running.get() {
                    if let Some(handle) = timer.get_untracked() {
                        handle.cancel();
                    }
                    timer.set_untracked(None);
                    set_is_running.set(false);
                } else if let Ok(handle) = set_interval(
                    &owner_for_timer,
                    move || -> SilexResult<()> {
                        set_count.update(|n| *n += 1);
                        Ok(())
                    },
                    Duration::from_millis(1000),
                ) {
                    timer.set_untracked(Some(handle));
                    set_is_running.set(true);
                }
                Ok(())
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
                    Ok(())
                })
        ]
        .style("margin-bottom: 10px;"),
        div!["Double: ", double_count]
            .classes(rx!(scope; if *$count % 2 == 0 { "even" } else { "odd" }))
            .style("margin-top: 5px; color: #666; font-size: 0.9em;"),
    ]
}

#[component]
pub fn NodeRefDemo<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    use silex::reexports::web_sys::HtmlInputElement;
    let input_ref = scope.node_ref::<HtmlInputElement>();

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
            Ok(())
        })
    ]
    .style(
        sty()
            .padding(px(20))
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))
            .margin_top(px(20)),
    )
}
#[component]
pub fn SvgIconDemo<'scope>() -> impl View<'scope> {
    #[component]
    fn ShieldCheck<'scope>() -> impl View<'scope> {
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
            ShieldCheck()
                .build()
                .style("width: 32px; height: 32px; color: green;"),
            ShieldCheck()
                .build()
                .style(
                    "width: 48px; height: 48px; color: blue; margin-left: 10px; cursor: pointer;"
                )
                .on(event::click, |_| {
                    console_log("Icon Clicked!");
                    Ok(())
                }),
            ShieldCheck()
                .build()
                .attr("width", "50")
                .attr("height", "50")
                .style("color: red; margin-left: 10px;"),
        ]
        .style(
            sty()
                .display(DisplayKeyword::Flex)
                .align_items(AlignItemsKeyword::Center)
                .padding(px(10))
                .background(AppTheme::SURFACE)
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))
        )
    ]
    .style("margin-top: 20px;")
}

#[component]
pub fn EventDemo<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (name, set_name) = scope.signal("Silex".to_string());
    let (count, set_count) = scope.signal(0);

    let (logs, set_logs) = scope.signal(Vec::<String>::new());
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
        Ok(())
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
        Ok(())
    };

    div![
        h3("Event & Closure Demo"),
        p("1. Signals are Copy: You can directly move them into closures without cloning."),
        div![
            p(name.map_fn(scope, |n| format!("Current Name: {}", n))),
            p(count.map(scope, |c| format!("Current Count: {}", c))),
        ]
        .style("margin-bottom: 10px; font-family: monospace;"),
        button("Log & Update (Standard)")
            .on(event::click, on_click)
            .style("margin-right: 10px;"),
        div![].style("height: 1px; background: #ccc; margin: 15px 0;"),
        p("2. Non-Copy types: Clone manually inside the closure."),
        button("Consume Payload").on(event::click, on_click_inner),
        ul(For(logs, |l| l.clone())
            .children(|l, _idx, _updater| li(l).style("font-size: 0.8em;"))
            .build())
        .style(
            sty()
                .margin_top(px(10))
                .background(AppTheme::BORDER)
                .opacity(0.5)
                .padding(px(10))
                .border_radius(px(4))
        )
    ]
    .style(
        sty()
            .padding(px(20))
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))
            .margin_top(px(20)),
    )
}

#[component]
pub fn BasicsPage<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let name_signal = scope.rw_signal("Developer".to_string());

    div![
        h2("Basics"),
        div![
            "Reactive Greeting Name: ",
            "Reactive Greeting Name: ",
            input().bind_value(name_signal),
            button("Submit")
                .attr("disabled", name_signal.read_signal().equals(scope, ""))
                .style("margin-left: 10px;")
        ]
        .style(
            sty()
                .margin_bottom(px(15))
                .padding(px(10))
                .background(AppTheme::SURFACE)
                .border_radius(px(4))
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))
        ),
        Greeting(name_signal).build(),
        Counter(scope).build(),
        EventDemo(scope).build(),
        NodeRefDemo(scope).build(),
        SvgIconDemo::<'scope>().build(),
        // AttributeDemo omitted for brevity, logic is same as previous
    ]
}
