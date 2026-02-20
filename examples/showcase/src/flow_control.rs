use silex::prelude::*;

#[component]
pub fn ListDemo() -> impl View {
    let (list, set_list) = signal(vec!["Apple", "Banana", "Cherry"]);

    div![
        h3("List Rendering with Signal Ergonomics"),
        p("Demonstrates passing a Signal directly to For::new without closure wrapper."),
        ul(For::new(list, |item| *item, li)),
        button("Add Item").on(event::click, set_list.updater(|l| l.push("New Item"))),
    ]
}

#[component]
pub fn ShowDemo() -> impl View {
    let (visible, set_visible) = signal(true);

    div![
        h3("Conditional Rendering with Show"),
        p("Demonstrates passing a Signal directly to Show::new as condition."),
        button("Toggle Visibility").on(event::click, set_visible.updater(|v| *v = !*v)),
        Show::new(visible, || div("✅ Content is visible!")
            .style("color: green; padding: 10px; background: #e8f5e9;"),)
        .fallback(
            || div("❌ Content is hidden").style("color: red; padding: 10px; background: #ffebee;")
        ),
    ]
}

#[component]
pub fn DynamicDemo() -> impl View {
    let (mode, set_mode) = signal("A");

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
        Dynamic::bind(mode, |m| {
            view_match!(m, {
                "A" => div("🅰️ Component A")
                    .style("padding: 20px; background: #e3f2fd;"),
                "B" => div("🅱️ Component B")
                    .style("padding: 20px; background: #fff3e0;"),
                _ => div("©️ Component C")
                    .style("padding: 20px; background: #f3e5f5;"),
            })
        }),
    ]
}

#[component]
pub fn SwitchDemo() -> impl View {
    let (tab, set_tab) = signal(0);

    div![
        h3("Switch (Match) Demo"),
        div![
            button("Tab 1").on(event::click, set_tab.setter(0)),
            button("Tab 2").on(event::click, set_tab.setter(1)),
            button("Tab 3").on(event::click, set_tab.setter(2)),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 10px;"),
        Switch::new(tab, || div("Fallback (Should not happen)"))
            .case(0, || div("Content for Tab 1")
                .style("padding: 10px; background: #eee;"))
            .case(1, || div("Content for Tab 2")
                .style("padding: 10px; background: #ddd;"))
            .case(2, || div("Content for Tab 3")
                .style("padding: 10px; background: #ccc;"))
    ]
}

#[component]
pub fn IndexDemo() -> impl View {
    let (items, set_items) = signal(vec!["Item A", "Item B", "Item C"]);

    div![
        h3("Index For Loop Demo"),
        p("Optimized for list updates by index."),
        Index::new(items, |item, idx| {
            div![
                strong(format!("{}: ", idx)),
                // item is a ReadSignal<String> here
                item
            ]
        }),
        button("Append Item")
            .on(event::click, move |_| {
                set_items.update(|list| list.push("New Item"));
            })
            .style("margin-top: 10px;")
    ]
}

#[component]
pub fn PortalDemo() -> impl View {
    let (show_modal, set_show_modal) = signal(false);

    div![
        h3("Portal Demo"),
        button("Toggle Modal").on(event::click, set_show_modal.updater(|v| *v = !*v)),
        Show::new(show_modal, move || {
            Portal::new(
                div![
                    div![
                        h4("I am a Modal!"),
                        p("I am rendered via Portal directly into the body, but I share context!"),
                        button("Close").on(event::click, set_show_modal.setter(false))
                    ]
                    .style("background: white; padding: 20px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.2); min-width: 300px;")
                ]
                .style("position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0,0,0,0.5); display: flex; justify-content: center; align-items: center; z-index: 9999;")
            )
        })
    ]
}

#[component]
pub fn FlowPage() -> impl View {
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
