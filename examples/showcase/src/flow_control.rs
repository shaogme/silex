use std::borrow::Cow;

use crate::css::AppTheme;
use silex::prelude::*;

#[component]
pub fn ListDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (list, set_list) = scope.signal(Ok(vec![
        Cow::Borrowed("Apple"),
        Cow::Borrowed("Banana"),
        Cow::Borrowed("Cherry"),
    ]))?;
    let (error_msg, set_error_msg) = scope.signal(None::<String>)?;
    let list_error_handler = scope.error_handler(move |err: SilexError| {
        let _ = set_error_msg.set(Some(format!("捕获到错误: {}", err)));
    })?;

    Ok(div![
        h3("List Rendering with Error Handling"),
        p("Demonstrates explicit error handling in For component to avoid crashes."),
        // Error display
        Show(ctx, error_msg.map(scope, |e| e.is_some(), error_handler)?,)
            .children(
                div(rx!(ctx; $error_msg.clone().unwrap_or_default())).style(
                    sty(ctx)
                        .color(hex("#d32f2f"))?
                        .background(hex("#ffebee"))?
                        .padding(px(10))?
                        .border_radius(px(4))?
                        .margin_bottom(px(10))?
                        .border(format!("1px solid {}", hex("#ef9a9a")))?,
                )
            )
            .build(),
        ul(For(ctx, list, |item| item.clone())
            .children(|item, _idx| li(item))
            .row_error_handler(list_error_handler)
            .build()),
        div![
            button("Add Item").on(event::click, move |_| {
                set_error_msg.set(None)?;
                set_list.update(|l| {
                    if let Ok(v) = l {
                        v.push(Cow::Owned(format!("New Item {}", v.len())));
                    } else {
                        *l = Ok(vec!["Apple".into(), "Banana".into(), "Cherry".into()]);
                    }
                })?;
                Ok(())
            }),
            button("Duplicate Key").on(event::click, move |_| {
                set_error_msg.set(None)?;
                set_list.update(|l| {
                    if let Ok(v) = l {
                        v.push("Duplicate".into());
                        v.push("Duplicate".into());
                    }
                })?;
                Ok(())
            }),
            button("Simulate Error").on(event::click, move |_| {
                set_list.set(Err(SilexError::fatal(SilexErrorKind::Javascript(
                    "模拟数据加载失败".to_string(),
                ))))?;
                Ok(())
            }),
        ]
        .style(sty(ctx).display("flex")?.gap(px(10))?.margin_top(px(10))?),
    ])
}

#[component]
pub fn ShowDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (visible, set_visible) = scope.signal(true)?;

    Ok(div![
        h3("Conditional Rendering with Show"),
        p("Demonstrates passing a Signal directly to Show as condition."),
        button("Toggle Visibility").on(event::click, set_visible.updater(|v| *v = !*v)),
        Show(ctx, visible)
            .children(
                div("✅ Content is visible!").style(
                    sty(ctx)
                        .color(ColorName::Green)?
                        .padding(px(10))?
                        .background(hex("#e8f5e9"))?
                )
            )
            .fallback(
                div("❌ Content is hidden").style(
                    sty(ctx)
                        .color(ColorName::Red)?
                        .padding(px(10))?
                        .background(hex("#ffebee"))?
                )
            )
            .build(),
    ])
}

#[component]
pub fn DynamicDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (mode, set_mode) = scope.signal("A")?;

    Ok(div![
        h3("Dynamic Component Switching"),
        p("Demonstrates Dynamic component with closure accessor."),
        div![
            button("Show A").on(event::click, set_mode.setter("A")),
            button("Show B").on(event::click, set_mode.setter("B")),
            button("Show C").on(event::click, set_mode.setter("C")),
        ]
        .style(
            sty(ctx)
                .display("flex")?
                .gap(px(10))?
                .margin_bottom(px(10))?
        ),
        // You can also use Dynamic(mode.map(|m| { view_match!(m, { ... }) })).
        Dynamic(
            ctx,
            mode.map(
                scope,
                move |m| {
                    Ok(view_match!(*m, {
                        "A" => div("🅰️ Component A")
                        .style(sty(ctx).padding(px(20))?.background(hex("#e3f2fd"))?),
                        "B" => div("🅱️ Component B")
                        .style(sty(ctx).padding(px(20))?.background(hex("#fff3e0"))?),
                        _ => div("©️ Component C")
                        .style(sty(ctx).padding(px(20))?.background(hex("#f3e5f5"))?),
                    }))
                },
                error_handler
            )?,
        )
        .build(),
    ])
}

#[component]
pub fn SwitchDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (tab, set_tab) = scope.signal(0)?;

    let switch = Switch(ctx, tab)
        .fallback(div("Fallback (Should not happen)"))
        .build()
        .case(
            0,
            div("Content for Tab 1").style(
                sty(ctx)
                    .padding(px(10))?
                    .background(AppTheme::SURFACE_ALT)?,
            ),
        )?
        .case(
            1,
            div("Content for Tab 2").style(sty(ctx).padding(px(10))?.background(AppTheme::BORDER)?),
        )?
        .case(
            2,
            div("Content for Tab 3").style(
                sty(ctx)
                    .padding(px(10))?
                    .background(AppTheme::BORDER)?
                    .opacity(0.8)?,
            ),
        )?;

    Ok(div![
        h3("Switch (Match) Demo"),
        div![
            button("Tab 1").on(event::click, set_tab.setter(0)),
            button("Tab 2").on(event::click, set_tab.setter(1)),
            button("Tab 3").on(event::click, set_tab.setter(2)),
        ]
        .style(
            sty(ctx)
                .display("flex")?
                .gap(px(10))?
                .margin_bottom(px(10))?
        ),
        switch
    ])
}

#[component]
pub fn IndexDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (items, set_items) = scope.signal(vec!["Item A", "Item B", "Item C"])?;

    Ok(div![
        h3("Index For Loop Demo"),
        p("Optimized for list updates by index."),
        Index(ctx, items)
            .children(|item, idx| div![strong(format!("{}: ", idx)), item])
            .build(),
        button("Append Item")
            .on(event::click, move |_| {
                set_items.update(|list| list.push("New Item"))?;
                Ok(())
            })
            .style(sty(ctx).margin_top(px(10))?)
    ])
}

#[component]
pub fn PortalDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (show_modal, set_show_modal) = scope.signal(false)?;

    Ok(div![
        h3("Portal Demo"),
        button("Toggle Modal").on(event::click, set_show_modal.updater(|v| *v = !*v)),
        Show(ctx, show_modal)
            .children(
                Portal(
                    ctx,
                    div![
                        div![
                            h4("I am a Modal!"),
                            p("I am rendered via Portal directly into the body, but I share ctx!"),
                            button("Close").on(event::click, set_show_modal.setter(false))
                        ]
                        .style(
                            sty(ctx)
                                .background(AppTheme::SURFACE)?
                                .padding(px(20))?
                                .border_radius(px(8))?
                                .box_shadow("0 4px 12px rgba(0,0,0,0.2)")?
                                .min_width(px(300))?
                        )
                    ]
                    .style(
                        sty(ctx)
                            .position(PositionKeyword::Fixed)?
                            .top(px(0))?
                            .left(px(0))?
                            .width(vw(100))?
                            .height(vh(100))?
                            .background(rgba(0, 0, 0, 0.5))?
                            .display(DisplayKeyword::Flex)?
                            .justify_content(JustifyContentKeyword::Center)?
                            .align_items(AlignItemsKeyword::Center)?
                            .z_index(9999)?
                    )
                )
                .build()
            )
            .build(),
    ])
}

#[component]
pub fn FlowPage<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    Ok(div![
        h2("Control Flow"),
        ListDemo(ctx).build(),
        ShowDemo(ctx).build(),
        DynamicDemo(ctx).build(),
        SwitchDemo(ctx).build(),
        IndexDemo(ctx).build(),
        PortalDemo(ctx).build(),
    ]
    .style(
        sty(ctx)
            .display("flex")?
            .flex_direction(FlexDirectionKeyword::Column)?
            .gap(px(20))?,
    ))
}
