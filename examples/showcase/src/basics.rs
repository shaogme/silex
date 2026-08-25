use crate::css::AppTheme;
use gloo_timers::future::TimeoutFuture;
use silex::core::TaskHandle;
use silex::dom::DomError;
use silex::{dom::log::console_log, prelude::*};
use std::rc::Rc;

#[component]
pub fn Greeting<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    name: Rx<'scope, String>,
    #[chain(default)] punctuation: String,
) -> impl View<'scope> {
    let full_punctuation = if punctuation.is_empty() {
        "!".to_string()
    } else {
        punctuation.clone()
    };

    Ok(div![
        span("Hello, "),
        strong(name).style(sty(ctx).color(AppTheme::PRIMARY)?),
        span(full_punctuation),
    ]
    .class("greeting-card")
    .style(
        sty(ctx)
            .padding(px(10))?
            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
            .border_radius(px(4))?
            .margin_bottom(px(10))?
            .background(AppTheme::SURFACE)?,
    ))
}

#[component]
pub fn Counter<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let count = owner.signal(0)?;
    let double_count = rx!(ctx; $count * 2)?;

    // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
    let timer = owner.stored(None::<TaskHandle<'scope>>)?;
    // UI State for the timer
    let is_running = owner.signal(false)?;

    Ok(div![
        h3("Interactive Counter"),
        div![
            button("-")
                .attr(
                    "disabled",
                    count.less_than_or_equals(owner, 0, error_handler)?
                )
                .on(event::click, count.updater(|n| *n -= 1)),
            strong(count).classes(classes![
                "counter-val",
                "positive" => count.greater_than(owner, 0, error_handler)?,
                "negative" => count.less_than(owner, 0, error_handler)?
            ]),
            button("+").on(event::click, count.updater(|n| *n += 1)),
        ]
        .style(
            sty(ctx)
                .display("flex")?
                .gap(px(10))?
                .align_items("center")?
        ),
        // Auto Increment Demo using set_interval and StoredValue
        div![
            button(rx!(ctx; if *$is_running {
                "Stop Auto Inc"
            } else {
                "Start Auto Inc"
            })?)
            .on(event::click, move |_| {
                if is_running.get()? {
                    if let Some(handle) = timer.update(Option::take)? {
                        handle.cancel();
                    }
                    is_running.set(false)?;
                } else {
                    let handle = owner.spawn_scoped(
                        async move {
                            loop {
                                TimeoutFuture::new(1000).await;
                                if count.update(|n| *n += 1).is_err() {
                                    break;
                                }
                            }
                        },
                        error_handler,
                    )?;
                    timer.set_untracked(Some(handle))?;
                    is_running.set(true)?;
                }
                Ok(())
            })
        ]
        .style(sty(ctx).margin("10px 0")?),
        // Manual Input Demo using event_target_value
        div![
            span("Set Value: "),
            input()
                .prop("value", count) // One-way binding from signal to DOM
                .on(event::input, move |e: DomEvent| {
                    let val_str = e.input_value().unwrap_or_default();
                    if let Ok(n) = val_str.parse::<i32>() {
                        count.set(n)?;
                    }
                    Ok(())
                })
        ]
        .style(sty(ctx).margin_bottom(px(10))?),
        div!["Double: ", double_count]
            .classes(rx!(ctx; if *$count % 2 == 0 { "even" } else { "odd" })?)
            .style(
                sty(ctx)
                    .margin_top(px(5))?
                    .color(hex("#666"))?
                    .font_size(em_unit(0.9))?
            ),
    ])
}

#[component]
pub fn SignalGuardDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let item = owner.signal("Silex T-Shirt".to_string())?;
    let quantity = owner.signal(1_i32)?;
    let stock = owner.signal(5_i32)?;
    let balance = owner.signal(100_i32)?;
    let order_count = owner.signal(0_u32)?;
    let status = owner.signal("请填写商品并检查库存".to_string())?;

    let quantity_summary = owner.computed(
        move || {
            let guard = quantity.read()?;
            let current = *guard;
            guard.finish()?;
            Ok(format!("当前购买数量：{current}"))
        },
        error_handler,
    )?;

    let on_increase = move |_| {
        let stock_guard = stock.read_untracked()?;
        let available = *stock_guard;
        stock_guard.finish()?;

        let mut guard = quantity.write()?;
        if *guard < available {
            *guard += 1;
        }
        guard.commit()?;
        Ok(())
    };

    let on_decrease = move |_| {
        let mut guard = quantity.write()?;
        if *guard > 1 {
            *guard -= 1;
        }
        guard.commit()?;
        Ok(())
    };

    let on_stock_check = move |_| {
        let item_guard = item.read_untracked()?;
        let item_name = item_guard.trim().to_string();
        item_guard.finish()?;

        let quantity_guard = quantity.read_untracked()?;
        let requested = *quantity_guard;
        quantity_guard.finish()?;

        let stock_guard = stock.read_untracked()?;
        let available = *stock_guard;
        stock_guard.finish()?;

        if item_name.is_empty() {
            status.set("请先填写商品名称".to_string())?;
        } else if requested <= available {
            status.set(format!(
                "库存充足：{item_name} × {requested}，提交后剩余 {} 件",
                available - requested
            ))?;
        } else {
            status.set(format!("库存不足：当前仅剩 {available} 件"))?;
        }
        Ok(())
    };

    let on_submit = move |_| {
        let result = owner.transaction(move |transaction| {
            let item_name = transaction.snapshot(item)?;
            let requested = transaction.snapshot(quantity)?;
            let total = requested.saturating_mul(10);

            let remaining = transaction.update(stock, |available| {
                if item_name.trim().is_empty() {
                    return Err(SilexError::recoverable(SilexErrorKind::Framework(
                        "商品名称不能为空".to_string(),
                    )));
                }
                if *available < requested {
                    return Err(SilexError::recoverable(SilexErrorKind::Framework(
                        "库存不足".to_string(),
                    )));
                }
                *available -= requested;
                Ok(*available)
            })?;

            let balance_remaining = transaction.update(balance, |available| {
                if *available < total {
                    return Err(SilexError::recoverable(SilexErrorKind::Framework(
                        "余额不足".to_string(),
                    )));
                }
                *available -= total;
                Ok(*available)
            })?;

            let orders = transaction.update(order_count, |count| {
                *count = count.saturating_add(1);
                Ok(*count)
            })?;

            Ok((item_name, requested, remaining, balance_remaining, orders))
        });

        match result {
            Ok((item_name, requested, remaining, balance_remaining, orders)) => {
                status.set(format!(
                    "订单已提交：{item_name} × {requested}，库存剩余 {remaining} 件，余额 {balance_remaining} 元，订单数 {orders}"
                ))?;
            }
            Err(error) => status.set(format!("提交失败：{error}"))?,
        }
        Ok(())
    };

    Ok(div![
        h3("购物车草稿"),
        p("用 guard 读取订单快照，用 transaction 原子提交库存、余额和订单数。"),
        label("商品："),
        input().bind_value(item),
        p(quantity_summary),
        p![strong("剩余库存："), strong(stock)],
        p![strong("账户余额："), strong(balance)],
        p![strong("已提交订单："), strong(order_count)],
        div![
            button("减少").on(event::click, on_decrease),
            strong(quantity),
            button("增加").on(event::click, on_increase),
        ]
        .style(sty(ctx).display("flex")?.gap(px(10))?),
        p(status),
        div![
            button("检查库存").on(event::click, on_stock_check),
            button("提交订单").on(event::click, on_submit),
        ]
        .style(sty(ctx).display("flex")?.gap(px(10))?),
    ]
    .style(
        sty(ctx)
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
            .margin_top(px(20))?,
    ))
}

fn focus_status(error: &SilexError) -> String {
    match error.kind() {
        SilexErrorKind::Dom(DomError::Unsupported {
            capability: "focus",
        }) => "NodeRef focus unsupported on this backend".to_string(),
        SilexErrorKind::Dom(DomError::NotBound) => {
            "NodeRef focus unavailable: ref is not bound".to_string()
        }
        SilexErrorKind::Dom(DomError::Detached { .. }) => {
            "NodeRef focus failed: target is detached".to_string()
        }
        _ => format!("NodeRef focus failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::focus_status;
    use silex::{SilexError, dom::DomError};

    #[test]
    fn focus_status_preserves_capability_failure_categories() {
        assert_eq!(
            focus_status(&SilexError::from(DomError::Unsupported {
                capability: "focus",
            })),
            "NodeRef focus unsupported on this backend"
        );
        assert_eq!(
            focus_status(&SilexError::from(DomError::NotBound)),
            "NodeRef focus unavailable: ref is not bound"
        );
        assert_eq!(
            focus_status(&SilexError::from(DomError::Detached { kind: "element" })),
            "NodeRef focus failed: target is detached"
        );
    }
}

#[component]
pub fn NodeRefDemo<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    dom_action: MountDomAction<'scope>,
    cleanup_hook: Rc<dyn Fn(bool) + 'scope>,
) -> impl View<'scope> {
    let input_ref = MountOwnerToken::new(owner).node_ref();
    let cleanup_ref = input_ref.clone();
    owner.on_cleanup(
        Box::new(move || {
            let is_cleared = cleanup_ref.get().map(|node| node.is_none())?;
            cleanup_hook(is_cleared);
            Ok(())
        }),
        error_handler,
    )?;
    let ref_status = owner.signal("NodeRef focus is ready".to_string())?;
    let focus_action = dom_action.clone();

    Ok(div![
        h3("NodeRef Demo"),
        p("Click the button to focus this input through a backend-neutral NodeRef."),
        input()
            .placeholder("I will be focused by NodeRef")
            .node_ref(input_ref.clone())
            .style(sty(ctx).margin_right(px(10))?.padding("5px")?),
        button("Focus Input via NodeRef").on(event::click, move |_| {
            let message = match focus_action.focus(&input_ref) {
                Ok(()) => "NodeRef focus succeeded".to_string(),
                Err(error) => focus_status(&error),
            };
            ref_status.set(message)?;
            Ok(())
        }),
        p(ref_status)
    ]
    .style(
        sty(ctx)
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
            .margin_top(px(20))?,
    ))
}
#[component]
pub fn SvgIconDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    #[component]
    fn ShieldCheck<'scope, Ctx>(
        #[ctx] ctx: Ctx,
        #[attrs] attrs: AttributeGroup<'scope>,
    ) -> impl View<'scope> {
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
        .apply(attrs)
    }

    Ok(div![
        h3("SVG Icon forwarding"),
        p("SVG icons with attribute forwarding."),
        div![
            ShieldCheck(ctx)
                .style(
                    sty(ctx)
                        .width(px(32))?
                        .height(px(32))?
                        .color(ColorName::Green)?
                )
                .build(),
            ShieldCheck(ctx)
                .style(
                    sty(ctx)
                        .width(px(48))?
                        .height(px(48))?
                        .color(ColorName::Blue)?
                        .margin_left(px(10))?
                        .cursor("pointer")?
                )
                .on(event::click, |_| {
                    console_log("Icon Clicked!");
                    Ok(())
                })
                .build(),
            ShieldCheck(ctx)
                .attr("width", "50")
                .attr("height", "50")
                .style(sty(ctx).color(ColorName::Red)?.margin_left(px(10))?)
                .build(),
        ]
        .style(
            sty(ctx)
                .display(DisplayKeyword::Flex)?
                .align_items(AlignItemsKeyword::Center)?
                .padding(px(10))?
                .background(AppTheme::SURFACE)?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
        )
    ]
    .style(sty(ctx).margin_top(px(20))?))
}

#[component]
pub fn EventDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let name = owner.signal("Silex".to_string())?;
    let count = owner.signal(0)?;

    let logs = owner.signal(Vec::<String>::new())?;
    let log_item_style = sty(ctx).font_size(em_unit(0.8))?;
    let payload = "DataPayload".to_string();

    // Since Signal is Copy, we can just move it directly into closures without cloning!
    let on_click = move |_| {
        console_log(format!(
            "Clicked! Name: {}, Count: {}",
            name.get()?,
            count.get()?
        ));
        count.update(|n| *n += 1)?;
        let next_count = count.get()? + 1;
        name.update(|n| *n = format!("Silex {}", next_count))?;
        Ok(())
    };

    let on_click_inner = move |_| {
        // For non-Copy types like String, we clone them manually if needed multiple times
        let owned_data = payload.clone();

        logs.update(|l| {
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
            p(name.map_fn(owner, |n| format!("Current Name: {}", n), error_handler)?),
            p(count.map(owner, |c| format!("Current Count: {}", c), error_handler)?),
        ]
        .style(sty(ctx).margin_bottom(px(10))?.font_family("monospace")?),
        button("Log & Update (Standard)")
            .on(event::click, on_click)
            .style(sty(ctx).margin_right(px(10))?),
        div![].style(
            sty(ctx)
                .height(px(1))?
                .background("#ccc")?
                .margin("15px 0")?
        ),
        p("2. Non-Copy types: Clone manually inside the closure."),
        button("Consume Payload").on(event::click, on_click_inner),
        ul(For(ctx, logs, |l| l.clone())
            .children(move |l, _idx| li(l).style(log_item_style.clone()))
            .build())
        .style(
            sty(ctx)
                .margin_top(px(10))?
                .background(AppTheme::BORDER)?
                .opacity(0.5)?
                .padding(px(10))?
                .border_radius(px(4))?
        )
    ]
    .style(
        sty(ctx)
            .padding(px(20))?
            .border(border(px(1), BorderStyleKeyword::Dashed, AppTheme::BORDER))?
            .margin_top(px(20))?,
    ))
}

#[component]
pub fn BasicsPage<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    dom_action: MountDomAction<'scope>,
    cleanup_hook: Rc<dyn Fn(bool) + 'scope>,
) -> impl View<'scope> {
    let name_signal = owner.signal("Developer".to_string())?;
    let name_draft = owner.signal("Developer".to_string())?;

    Ok(div![
        h2("Basics"),
        div![
            "Reactive Greeting Name: ",
            input().bind_value(name_draft),
            button("Submit")
                .attr("disabled", name_draft.equals(owner, "", error_handler)?,)
                .on(event::click, move |_| {
                    name_signal.set(name_draft.get()?)?;
                    Ok(())
                })
                .style(sty(ctx).margin_left(px(10))?)
        ]
        .style(
            sty(ctx)
                .margin_bottom(px(15))?
                .padding(px(10))?
                .background(AppTheme::SURFACE)?
                .border_radius(px(4))?
                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
        ),
        Greeting(ctx, name_signal).build(),
        Counter(ctx).build(),
        SignalGuardDemo(ctx).build(),
        EventDemo(ctx).build(),
        NodeRefDemo(ctx, dom_action, cleanup_hook).build(),
        SvgIconDemo(ctx).build(),
        // AttributeDemo omitted for brevity, logic is same as previous
    ])
}
