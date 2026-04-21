use crate::css::AppTheme;
use silex::prelude::*;

#[component]
pub fn ListDemo() -> impl Mount + MountRef {
    let (list, set_list) = Signal::pair(vec!["Apple", "Banana", "Cherry"]);

    div![
        h3("List Rendering with Signal Ergonomics"),
        p("Demonstrates passing a Signal directly to For without closure wrapper."),
        ul(For(list, |item| *item).children(|item, _idx| li(item))),
        button("Add Item").on(event::click, set_list.updater(|l| l.push("New Item"))),
    ]
}

#[component]
pub fn ShowDemo() -> impl Mount + MountRef {
    let (visible, set_visible) = Signal::pair(true);

    div![
        h3("Conditional Rendering with Show"),
        p("Demonstrates passing a Signal directly to Show::new as condition."),
        button("Toggle Visibility").on(event::click, set_visible.updater(|v| *v = !*v)),
        Show(visible)
            .children(
                div("✅ Content is visible!").style(
                    sty()
                        .color(hex("green"))
                        .padding(px(10))
                        .background(hex("#e8f5e9"))
                )
            )
            .fallback(
                div("❌ Content is hidden").style(
                    sty()
                        .color(hex("red"))
                        .padding(px(10))
                        .background(hex("#ffebee"))
                )
            ),
    ]
}

#[component]
pub fn DynamicDemo() -> impl Mount + MountRef {
    let (mode, set_mode) = Signal::pair("A");

    div![
        h3("Dynamic Component Switching"),
        p("Demonstrates Dynamic component with closure accessor."),
        div![
            button("Show A").on(event::click, set_mode.setter("A")),
            button("Show B").on(event::click, set_mode.setter("B")),
            button("Show C").on(event::click, set_mode.setter("C")),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 10px;"),
        // You can also use Dynamic::new(mode.map(|m| { view_match!(m, { ... }) })).
        Dynamic(mode.map(|m| {
            view_match!(*m, {
                "A" => div("🅰️ Component A")
                    .style(sty().padding(px(20)).background(hex("#e3f2fd"))),
                "B" => div("🅱️ Component B")
                    .style(sty().padding(px(20)).background(hex("#fff3e0"))),
                _ => div("©️ Component C")
                    .style(sty().padding(px(20)).background(hex("#f3e5f5"))),
            })
        })),
    ]
}

#[component]
pub fn SwitchDemo() -> impl Mount + MountRef {
    let (tab, set_tab) = Signal::pair(0);

    div![
        h3("Switch (Match) Demo"),
        div![
            button("Tab 1").on(event::click, set_tab.setter(0)),
            button("Tab 2").on(event::click, set_tab.setter(1)),
            button("Tab 3").on(event::click, set_tab.setter(2)),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 10px;"),
        Switch(tab)
            .fallback(div("Fallback (Should not happen)"))
            .case(
                0,
                div("Content for Tab 1")
                    .style(sty().padding(px(10)).background(AppTheme::SURFACE_ALT))
            )
            .case(
                1,
                div("Content for Tab 2").style(sty().padding(px(10)).background(AppTheme::BORDER))
            )
            .case(
                2,
                div("Content for Tab 3").style(
                    sty()
                        .padding(px(10))
                        .background(AppTheme::BORDER)
                        .opacity(0.8)
                )
            )
    ]
}

#[component]
pub fn IndexDemo() -> impl Mount + MountRef {
    let (items, set_items) = Signal::pair(vec!["Item A", "Item B", "Item C"]);

    div![
        h3("Index For Loop Demo"),
        p("Optimized for list updates by index."),
        Index(items).children(|item, idx| { div![strong(format!("{}: ", idx.get())), item] }),
        button("Append Item")
            .on(event::click, move |_| {
                set_items.update(|list| list.push("New Item"));
            })
            .style("margin-top: 10px;")
    ]
}

#[component]
pub fn PortalDemo() -> impl Mount + MountRef {
    let (show_modal, set_show_modal) = Signal::pair(false);

    div![
        h3("Portal Demo"),
        button("Toggle Modal").on(event::click, set_show_modal.updater(|v| *v = !*v)),
        Show(show_modal).children(Portal(
            div![
                div![
                    h4("I am a Modal!"),
                    p("I am rendered via Portal directly into the body, but I share context!"),
                    button("Close").on(event::click, set_show_modal.setter(false))
                ]
                .style(
                    sty()
                        .background(AppTheme::SURFACE)
                        .padding(px(20))
                        .border_radius(px(8))
                        .box_shadow("0 4px 12px rgba(0,0,0,0.2)")
                        .min_width(px(300))
                )
            ]
            .style(
                sty()
                    .position(PositionKeyword::Fixed)
                    .top(px(0))
                    .left(px(0))
                    .width(vw(100))
                    .height(vh(100))
                    .background(rgba(0, 0, 0, 0.5))
                    .display(DisplayKeyword::Flex)
                    .justify_content(JustifyContentKeyword::Center)
                    .align_items(AlignItemsKeyword::Center)
                    .z_index(9999)
            )
        ))
    ]
}

#[component]
pub fn FlowPage() -> impl Mount + MountRef {
    div![
        h2("Control Flow"),
        ListDemo(),
        ShowDemo(),
        DynamicDemo(),
        SwitchDemo(),
        IndexDemo(),
        PortalDemo(),
    ]
    .style("display: flex; flex-direction: column; gap: 20px;")
}
