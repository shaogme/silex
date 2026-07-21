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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WikimediaChange {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub user: String,
    #[serde(default, rename = "type")]
    pub change_type: String,
    #[serde(default)]
    pub wiki: String,
}

#[component]
pub fn HttpClientDemo() -> impl View {
    let (post_id, set_post_id) = Signal::pair(1);
    let search_query = RwSignal::new(String::new());

    // 1. Declarative HTTP fetching with path parameters, retry policy, and optional query params
    let post_resource = HttpClient::get("https://jsonplaceholder.typicode.com/posts/{id}")
        .path_param("id", post_id)
        .query_opt("filter", {
            let q = search_query.get();
            if q.is_empty() { None } else { Some(q) }
        })
        .timeout_ms(5000)
        .retry_policy(2, std::time::Duration::from_millis(300))
        .json::<Post>()
        .as_resource(post_id);

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
        p("Declarative HTTP fetching with path parameters, optional query parameters, auto-retries, resources, and mutations."),

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

            span("Optional Filter Query: ").style("margin-left: 10px;"),
            input()
                .placeholder("Type query...")
                .bind_value(search_query)
                .style(sty().margin_right(px(10)).padding(padding::x_y(px(4), px(8))).border_radius(px(4)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).background(AppTheme::SURFACE).color(AppTheme::TEXT)),

            button("Refresh").on(event::click, move |_| post_resource.refetch()),
        ].style("margin-bottom: 20px; display: flex; align-items: center; flex-wrap: wrap; gap: 8px;"),

        // Resource Display
        div![
            move || match post_resource.state.get() {
                ResourceState::Ready(post) | ResourceState::Reloading(post) => div![
                    h4(post.title).style(sty().color(AppTheme::PRIMARY).margin_top(px(0))),
                    p(post.body).style("opacity: 0.8;"),
                    small(format!("User ID: {} | Post ID: {}", post.user_id, post.id)).style("opacity: 0.6;")
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
pub fn WebSocketDemo() -> impl View {
    let url = RwSignal::new("wss://echo.websocket.org".to_string());
    let socket = RwSignal::new(None::<WebSocketConnection>);
    let input_text = RwSignal::new(String::new());

    let state_text = socket.map_or("Disconnected", |c| c.state.get().as_str());
    let is_connected = socket.map_or(false, |c| c.is_connected().get());

    let last_message = move || {
        socket.with(|conn| {
            conn.as_ref()
                .and_then(|c| c.raw_message().get())
                .unwrap_or_default()
        })
    };

    let send_message = move || {
        let text = input_text.get();
        if !text.trim().is_empty() {
            socket.with_untracked(|conn| {
                if let Some(conn) = conn {
                    let _ = conn.send(text);
                    input_text.set(String::new());
                }
            });
        }
    };

    div![
        h3("WebSocket Demo"),
        p("Real-time bidirectional communication with automatic connection state handling & Enter key support."),

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
                        socket.set(None);
                    } else {
                        let conn = WebSocket::open(url.get());
                        socket.set(Some(conn));
                    }
                })
                .style("padding: 8px 16px; margin-left:10px; border-radius: 4px; cursor: pointer;"),
        ].style("display: flex; margin-bottom: 20px;"),

        div![
            span("Status: "),
            strong(state_text)
                .style(rx!(@fn if is_connected.get() { sty().color(hex("green")) } else { sty().color(hex("red")) })),
        ].style("margin-bottom: 15px;"),

        Show(is_connected).children(
            div![
                div![
                    input()
                        .placeholder("Send message (Press Enter)...")
                        .bind_value(input_text)
                        .on(event::keydown, move |e: silex::reexports::web_sys::KeyboardEvent| {
                            if e.key() == "Enter" {
                                send_message();
                            }
                        })
                        .style(sty().padding(px(8)).width(px(260)).border_radius(px(4)).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).background(AppTheme::SURFACE).color(AppTheme::TEXT)),
                    button("Send").on(event::click, move |_| {
                        send_message();
                    })
                    .style("margin-left: 10px; padding: 8px 16px; cursor: pointer;"),
                ],
                div![
                    p("Last Echoed Message:"),
                    div(move || last_message()).style(sty().padding(px(15)).background(AppTheme::SURFACE_ALT).border_radius(px(6)).font_family("monospace").border_left(border(px(4), BorderStyleKeyword::Solid, AppTheme::PRIMARY)))
                ].style(sty().margin_top(px(15))),
            ]
        )
    ]
}

#[component]
pub fn EventStreamDemo() -> impl View {
    let url = RwSignal::new("https://stream.wikimedia.org/v2/stream/recentchange".to_string());
    let stream = RwSignal::new(None::<EventStreamConnection>);
    let logs = RwSignal::new(Vec::<WikimediaChange>::new());

    let is_connected = stream.map_or(false, |c| c.is_connected().get());

    // Sync stream messages to independent logs signal while connection is active
    Effect::new(move |_| {
        stream.with(|conn| {
            if let Some(conn) = conn.as_ref() {
                let msgs = conn.messages::<WikimediaChange>().get();
                if !msgs.is_empty() {
                    logs.set(msgs.into_iter().rev().take(50).collect());
                }
            }
        });
    });

    div![
        h3("EventSource (SSE) Demo"),
        p("One-way server-to-client streaming parsed directly into strongly typed Rust structs (Wikimedia Recent Changes)."),

        div![
            input()
                .bind_value(url)
                .style("flex-grow: 1; padding: 8px; border-radius: 4px; border: 1px solid var(--slx-theme-border); background: var(--slx-theme-surface); color: var(--slx-theme-text);"),
            button(move || if is_connected.get() { "Stop Stream" } else { "Start Stream" })
                .on(event::click, move |_| {
                    if is_connected.get() {
                        stream.with_untracked(|conn| if let Some(conn) = conn {
                            conn.close();
                        });
                        stream.set(None);
                    } else {
                        let conn = EventStream::open(url.get());
                        stream.set(Some(conn));
                    }
                })
                .style("padding: 8px 16px; margin-left:10px; border-radius: 4px; cursor: pointer;"),
            button("Clear Log")
                .on(event::click, move |_| logs.set(Vec::new()))
                .style("padding: 8px 16px; margin-left:10px; border-radius: 4px; cursor: pointer; background: transparent; border: 1px solid var(--slx-theme-border); color: var(--slx-theme-text);"),
        ].style("display: flex; margin-bottom: 20px;"),

        div![
            h4("Stream Log (Wikipedia Edits):"),
            ul(For(logs, |item| item.id.unwrap_or(0).to_string() + &item.title).children(|change_sig, _idx| {
                let change = change_sig.get();
                li(div![
                    span(format!("[{}] ", change.wiki)).style("font-weight: bold; opacity: 0.6;"),
                    span(format!("{} ", change.title)).style(sty().color(AppTheme::PRIMARY).font_weight("bold")),
                    span(format!("by {}", change.user)).style("opacity: 0.8; font-style: italic;"),
                    span(format!(" ({})", change.change_type)).style("font-size: 0.85em; opacity: 0.5;")
                ]).style(sty().font_family("sans-serif").font_size(em_unit(0.9)).margin_bottom(px(6)).border_bottom(border(px(1), BorderStyleKeyword::Solid, AppTheme::SURFACE_ALT)).padding_bottom(px(4)))
            }))
            .style(sty().max_height(px(320)).overflow_y(OverflowYKeyword::Auto).background(AppTheme::SURFACE).border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER)).padding(px(15)).border_radius(px(8)))
        ]
    ]
}

#[component]
pub fn NetDemoPage() -> impl View {
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
