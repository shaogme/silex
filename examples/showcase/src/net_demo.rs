use crate::css::AppTheme;
use serde::{Deserialize, Serialize};
use silex::prelude::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub user_id: i32,
}

#[component]
pub fn HttpClientDemo() -> impl Mount + MountRef {
    let (post_id, set_post_id) = Signal::pair(1);

    // 1. Using HttpClient::as_resource for declarative fetching
    let post_client = HttpClient::get("https://jsonplaceholder.typicode.com/posts/{id}")
        .path_param("id", post_id)
        .json::<Post>();

    let post_resource = post_client.as_resource(post_id);

    // 2. Using HttpClient::as_mutation for actions (POST)
    let create_post_mutation = HttpClient::post("https://jsonplaceholder.typicode.com/posts")
        .json_body(serde_json::json!({
            "title": "Silex Net Demo",
            "body": "This is a post created via Silex Net mutation.",
            "userId": 1
        }))
        .json::<Post>()
        .as_mutation();

    div![
        h3("HTTP Client Demo"),
        p("Declarative HTTP fetching with path parameters, resources, and mutations."),

        div![
            span("Fetch Post ID: "),
            input()
                .attr("type", "number")
                .prop("value", post_id)
                .on(event::input, move |e| {
                    if let Ok(id) = event_target_value(&e).parse::<i32>() {
                        set_post_id.set(id);
                    }
                })
                .style(sty().margin_right(px(10)).padding(padding::x_y(px(4), px(8))).border_radius(px(4)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).background(AppTheme::SURFACE).color(AppTheme::TEXT)),
            button("Refresh").on(event::click, move |_| post_resource.refetch()),
        ].style("margin-bottom: 20px;"),

        // Resource Display
        div![
            move || match post_resource.state.get() {
                ResourceState::Ready(post) | ResourceState::Reloading(post) => div![
                    h4(post.title).style(sty().color(AppTheme::PRIMARY).margin_top(px(0))),
                    p(post.body).style("opacity: 0.8;"),
                    small(format!("User ID: {}", post.user_id)).style("opacity: 0.6;")
                ].style(sty().padding(px(20)).background(AppTheme::SURFACE_ALT).border_radius(px(8)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))),
                ResourceState::Error(err) => div![
                    div("❌ Request Failed").style("color: red; font-weight: bold;"),
                    p(format!("{:?}", err)).style("font-size: 0.8em; opacity: 0.7;")
                ].style("padding: 20px; border: 1px solid red; border-radius: 8px; background: rgba(255,0,0,0.05);"),
                ResourceState::Loading if post_resource.get_data().is_none() => div("Loading post...").style(sty().padding(px(20)).color(AppTheme::PRIMARY)),
                _ => div("Select a post ID to fetch.").style("padding: 20px; opacity: 0.5;"),
            }
        ].style("min-height: 120px;"),

        hr().style(sty().margin_y(px(30)).border(px(0)).border_top(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))),

        h4("Mutations (POST Request)"),
        div![
            button("Create New Mock Post")
                .on(event::click, move |_| create_post_mutation.mutate(()))
                .attr("disabled", create_post_mutation.loading())
                .style(sty().padding(padding::x_y(px(10), px(20))).background(AppTheme::PRIMARY).color(hex("white")).border(NONE).border_radius(px(6)).cursor(CursorKeyword::Pointer)),

            move || if create_post_mutation.loading() {
                span(" Creating...").style(sty().margin_left(px(10)).color(AppTheme::PRIMARY)).into_any()
            } else {
                "".into_any()
            },
        ],

        move || if let Some(err) = create_post_mutation.error() {
            div(format!("❌ Error creating post: {:?}", err)).style("color: red; margin-top: 15px;").into_any()
        } else {
            create_post_mutation.value().map(|post| {
                div![
                    div("✅ Post Created Successfully (Mock)!").style("color: green; font-weight: bold; margin-bottom: 5px;"),
                    pre(format!("{:#?}", post)).style("background: #1e1e1e; color: #d4d4d4; padding: 15px; border-radius: 6px; font-size: 0.85em; overflow-x: auto;")
                ].style("margin-top: 15px;")
            }).into_any()
        }
    ]
}

#[component]
pub fn WebSocketDemo() -> impl Mount + MountRef {
    let url = RwSignal::new("wss://echo.websocket.org".to_string());
    let socket = StoredValue::new(None::<WebSocketConnection>);
    let (is_connected, set_is_connected) = Signal::pair(false);
    let (last_message, set_last_message) = Signal::pair(String::new());
    let input_text = RwSignal::new(String::new());

    div![
        h3("WebSocket Demo"),
        p("Real-time bidirectional communication with automatic connection state handling."),

        div![
            input()
                .bind_value(url)
                .style("flex-grow: 1; padding: 8px; border-radius: 4px; border: 1px solid var(--slx-theme-border); background: var(--slx-theme-surface); color: var(--slx-theme-text);"),
            button(move || if is_connected.get() { "Disconnect" } else { "Connect" })
                .on(event::click, move |_| {
                    if is_connected.get() {
                        socket.with_untracked(|conn| if let Some(conn) = conn {
                            let _ = conn.close();
                        });
                        set_is_connected.set(false);
                    } else {
                        let conn = WebSocket::connect(url.get())
                            .on_open(move || set_is_connected.set(true))
                            .on_close(move |_, _| set_is_connected.set(false))
                            .build();

                        // Register message handler
                        let msg_signal = conn.raw_message();
                        Effect::new(move |_| {
                            if let Some(msg) = msg_signal.get() {
                                set_last_message.set(msg);
                            }
                        });

                        socket.set_untracked(Some(conn));
                    }
                })
                .style("padding: 8px 16px; margin-left:10px; border-radius: 4px; cursor: pointer;"),
        ].style("display: flex; margin-bottom: 20px;"),

        div![
            span("Status: "),
            strong(move || if is_connected.get() { "Connected" } else { "Disconnected" })
                .style(rx!(@fn if is_connected.get() { sty().color(hex("green")) } else { sty().color(hex("red")) })),
        ].style("margin-bottom: 15px;"),

        Show(is_connected).children(
            div![
                div![
                    input()
                        .placeholder("Send something to echo server...")
                        .bind_value(input_text)
                        .style(sty().padding(px(8)).width(px(200)).border_radius(px(4)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).background(AppTheme::SURFACE).color(AppTheme::TEXT)),
                    button("Send").on(event::click, move |_| {
                        socket.with_untracked(|conn| if let Some(conn) = conn {
                            let _ = conn.send(input_text.get());
                            input_text.set(String::new());
                        });
                    })
                    .style("margin-left: 10px; padding: 8px 16px; cursor: pointer;"),
                ],
                div![
                    p("Last Echoed Message:"),
                    div(last_message).style(sty().padding(px(15)).background(AppTheme::SURFACE_ALT).border_radius(px(6)).font_family("monospace").border_left(border(px(4), BorderStyleKeyword::Solid, AppTheme::PRIMARY)))
                ].style(sty().margin_top(px(15))),
            ]
        )
    ]
}

#[component]
pub fn EventStreamDemo() -> impl Mount + MountRef {
    let (is_active, set_is_active) = Signal::pair(false);
    let url = RwSignal::new("https://stream.wikimedia.org/v2/stream/recentchange".to_string());
    let stream = StoredValue::new(None::<EventStreamConnection>);
    let (events, set_events) = Signal::pair(Vec::<String>::new());

    div![
        h3("EventSource (SSE) Demo"),
        p("One-way server-to-client streaming for real-time updates."),

        div![
            input()
                .bind_value(url)
                .style("flex-grow: 1; padding: 8px; border-radius: 4px; border: 1px solid var(--slx-theme-border); background: var(--slx-theme-surface); color: var(--slx-theme-text);"),
            button(move || if is_active.get() { "Stop Stream" } else { "Start Stream" })
                .on(event::click, move |_| {
                    if is_active.get() {
                        stream.with_untracked(|conn| if let Some(conn) = conn {
                            conn.close();
                        });
                        set_is_active.set(false);
                    } else {
                        let conn = EventStream::builder(url.get())
                            .on_open(move || set_events.update(|e| e.push("Connected to stream".into())))
                            .build();

                        let msgs = conn.raw_messages();
                        Effect::new(move |_| {
                            if let Some(items) = msgs.get().last() {
                                set_events.update(|e| {
                                    e.push(format!("Event: {}", items.data));
                                    if e.len() > 50 { e.remove(0); } // Keep log manageable
                                });
                            }
                        });

                        stream.set_untracked(Some(conn));
                        set_is_active.set(true);
                    }
                })
                .style("padding: 8px 16px; margin-left:10px; border-radius: 4px; cursor: pointer;"),
        ].style("display: flex; margin-bottom: 20px;"),

        div![
            h4("Stream Log (Latest 50 events):"),
            ul(For(events, |e| e.clone()).children(|e, _idx| {
                li(e).style(sty().font_family("monospace").font_size(em_unit(0.8)).opacity(0.8).margin_bottom(px(4)).word_break(WordBreakKeyword::BreakAll).border_bottom(border(px(1), BorderStyleKeyword::Solid, AppTheme::SURFACE_ALT)).padding_bottom(px(2)))
            }))
            .style(sty().max_height(px(300)).overflow_y(OverflowYKeyword::Auto).background(AppTheme::SURFACE).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).padding(px(15)).border_radius(px(8)))
        ]
    ]
}

#[component]
pub fn NetDemoPage() -> impl Mount + MountRef {
    let (active_tab, set_active_tab) = Signal::pair("http");

    inject_style("net-demo-css", "
        .tab-nav { display: flex; gap: 10px; margin-bottom: 30px; border-bottom: 1px solid var(--slx-theme-border); padding-bottom: 15px; }
        .tab-nav button { background: none; border: none; padding: 10px 20px; cursor: pointer; color: var(--slx-theme-text); opacity: 0.6; border-radius: 6px; transition: all 0.3s; }
        .tab-nav button:hover { background: rgba(0,0,0,0.05); opacity: 1; }
        .tab-nav button.active { background: var(--slx-theme-primary); color: white; opacity: 1; font-weight: bold; }
        .demo-container { min-height: 400px; }
    ");

    div![
        h2("🌐 Networking (silex_net)"),
        p("Comprehensive networking suite for Silex, supporting REST, WebSockets, and Server-Sent Events."),

        // Navigation Tabs
        div![
            button("HTTP Client")
                .on(event::click, set_active_tab.setter("http"))
                .classes(rx!(@fn if *$active_tab == "http" { "active" } else { "" })),
            button("WebSocket")
                .on(event::click, set_active_tab.setter("ws"))
                .classes(rx!(@fn if *$active_tab == "ws" { "active" } else { "" })),
            button("EventStream")
                .on(event::click, set_active_tab.setter("sse"))
                .classes(rx!(@fn if *$active_tab == "sse" { "active" } else { "" })),
        ].class("tab-nav"),

        // Content
        div![
            move || match active_tab.get() {
                "http" => HttpClientDemo().into_any(),
                "ws" => WebSocketDemo().into_any(),
                "sse" => EventStreamDemo().into_any(),
                _ => "".into_any(),
            }
        ].class("demo-container")
    ]
    .style(sty().padding(px(24)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).border_radius(px(12)).background(AppTheme::SURFACE).transition("all 0.3s"))
    .classes("net-demo-page")
}
