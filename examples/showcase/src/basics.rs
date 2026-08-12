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

    Ok(div![
        span("Hello, "),
        strong(name).style(sty().color(AppTheme::PRIMARY)?),
        span(full_punctuation),
    ]
    .class("greeting-card")
    .style(
        sty()
            .padding(px(10))?
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
            .border_radius(px(4))?
            .margin_bottom(px(10))?
            .background(AppTheme::SURFACE)?,
    ))
}

#[component]
pub fn Counter<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    #[inject(owner)] owner: ViewOwnerToken<'scope>,
) -> impl View<'scope> {
    let (count, set_count) = scope.signal(0)?;
    let double_count = rx!(scope; error_handler; $count * 2);

    // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
    let timer = scope.stored(None::<HostResourceHandle<'scope>>)?;
    let owner_for_timer = owner.clone();
    // UI State for the timer
    let (is_running, set_is_running) = scope.signal(false)?;

    Ok(div![
        h3("Interactive Counter"),
        div![
            button("-")
                .attr(
                    "disabled",
                    count.less_than_or_equals(scope, 0, error_handler)?
                )
                .on(event::click, set_count.updater(|n| *n -= 1)),
            strong(count).classes(classes![
                "counter-val",
                "positive" => count.greater_than(scope, 0, error_handler)?,
                "negative" => count.less_than(scope, 0, error_handler)?
            ]),
            button("+").on(event::click, set_count.updater(|n| *n += 1)),
        ]
        .style(sty().display("flex")?.gap(px(10))?.align_items("center")?),
        // Auto Increment Demo using set_interval and StoredValue
        div![
            button(rx!(scope; error_handler; if *$is_running {
                "Stop Auto Inc"
            } else {
                "Start Auto Inc"
            }))
            .on(event::click, move |_| {
                if is_running.get()? {
                    if let Some(handle) = timer.get_untracked()? {
                        handle.cancel();
                    }
                    timer.set_untracked(None)?;
                    set_is_running.set(false)?;
                } else {
                    let handle = set_interval(
                        &owner_for_timer,
                        move || -> SilexResult<()> {
                            set_count.update(|n| *n += 1)?;
                            Ok(())
                        },
                        Duration::from_millis(1000),
                    )?;
                    timer.set_untracked(Some(handle))?;
                    set_is_running.set(true)?;
                }
                Ok(())
            })
        ]
        .style(sty().margin("10px 0")?),
        // Manual Input Demo using event_target_value
        div![
            span("Set Value: "),
            input()
                .prop("value", count) // One-way binding from signal to DOM
                .on(event::input, move |e| {
                    let val_str = event_target_value(&e);
                    if let Ok(n) = val_str.parse::<i32>() {
                        set_count.set(n)?;
                    }
                    Ok(())
                })
        ]
        .style(sty().margin_bottom(px(10))?),
        div!["Double: ", double_count]
            .classes(rx!(scope; error_handler; if *$count % 2 == 0 { "even" } else { "odd" }))
            .style(
                sty()
                    .margin_top(px(5))?
                    .color(hex("#666"))?
                    .font_size(em_unit(0.9))?
            ),
    ])
}

#[component]
pub fn NodeRefDemo<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    use silex::reexports::web_sys::HtmlInputElement;
    let input_ref = scope.node_ref::<HtmlInputElement>()?;

    Ok(div![
        h3("NodeRef Demo"),
        p("Click the button to focus the input field using direct DOM access."),
        input()
            .placeholder("I will be focused...")
            .node_ref(input_ref) // NodeRef 是 Copy 的，无需 clone
            .style(sty().margin_right(px(10))?.padding("5px")?),
        button("Focus Input").on(event::click, move |_| {
            if let Some(el) = input_ref.get()? {
                let _ = el.focus();
            }
            Ok(())
        })
    ]
    .style(
        sty()
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
            .margin_top(px(20))?,
    ))
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

    Ok(div![
        h3("SVG Icon forwarding"),
        p("SVG icons with attribute forwarding."),
        div![
            ShieldCheck().build().style(
                sty()
                    .width(px(32))?
                    .height(px(32))?
                    .color(ColorName::Green)?
            ),
            ShieldCheck()
                .build()
                .style(
                    sty()
                        .width(px(48))?
                        .height(px(48))?
                        .color(ColorName::Blue)?
                        .margin_left(px(10))?
                        .cursor("pointer")?
                )
                .on(event::click, |_| {
                    console_log("Icon Clicked!");
                    Ok(())
                }),
            ShieldCheck()
                .build()
                .attr("width", "50")
                .attr("height", "50")
                .style(sty().color(ColorName::Red)?.margin_left(px(10))?),
        ]
        .style(
            sty()
                .display(DisplayKeyword::Flex)?
                .align_items(AlignItemsKeyword::Center)?
                .padding(px(10))?
                .background(AppTheme::SURFACE)?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
        )
    ]
    .style(sty().margin_top(px(20))?))
}

#[component]
pub fn EventDemo<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let (name, set_name) = scope.signal("Silex".to_string())?;
    let (count, set_count) = scope.signal(0)?;

    let (logs, set_logs) = scope.signal(Vec::<String>::new())?;
    let log_item_style = sty().font_size(em_unit(0.8))?;
    let payload = "DataPayload".to_string();

    // Since Signal is Copy, we can just move it directly into closures without cloning!
    let on_click = move |_| {
        console_log(format!(
            "Clicked! Name: {}, Count: {}",
            name.get()?,
            count.get()?
        ));
        set_count.update(|n| *n += 1)?;
        let next_count = count.get()? + 1;
        set_name.update(|n| *n = format!("Silex {}", next_count))?;
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
        })?;
        Ok(())
    };

    Ok(div![
        h3("Event & Closure Demo"),
        p("1. Signals are Copy: You can directly move them into closures without cloning."),
        div![
            p(name.map_fn(scope, |n| format!("Current Name: {}", n), error_handler)?),
            p(count.map(scope, |c| format!("Current Count: {}", c), error_handler)?),
        ]
        .style(sty().margin_bottom(px(10))?.font_family("monospace")?),
        button("Log & Update (Standard)")
            .on(event::click, on_click)
            .style(sty().margin_right(px(10))?),
        div![].style(sty().height(px(1))?.background("#ccc")?.margin("15px 0")?),
        p("2. Non-Copy types: Clone manually inside the closure."),
        button("Consume Payload").on(event::click, on_click_inner),
        ul(For(logs, |l| l.clone())
            .children(move |l, _idx, _updater| li(l).style(log_item_style.clone()))
            .build())
        .style(
            sty()
                .margin_top(px(10))?
                .background(AppTheme::BORDER)?
                .opacity(0.5)?
                .padding(px(10))?
                .border_radius(px(4))?
        )
    ]
    .style(
        sty()
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
            .margin_top(px(20))?,
    ))
}

#[component]
pub fn BasicsPage<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let name_signal = scope.rw_signal("Developer".to_string())?;

    Ok(div![
        h2("Basics"),
        div![
            "Reactive Greeting Name: ",
            "Reactive Greeting Name: ",
            input().bind_value(name_signal),
            button("Submit")
                .attr(
                    "disabled",
                    name_signal.read_signal().equals(scope, "", error_handler)?,
                )
                .style(sty().margin_left(px(10))?)
        ]
        .style(
            sty()
                .margin_bottom(px(15))?
                .padding(px(10))?
                .background(AppTheme::SURFACE)?
                .border_radius(px(4))?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
        ),
        Greeting(name_signal).build(),
        Counter(scope, error_handler).build(),
        EventDemo(scope, error_handler).build(),
        NodeRefDemo(scope).build(),
        SvgIconDemo::<'scope>().build(),
        // AttributeDemo omitted for brevity, logic is same as previous
    ])
}
