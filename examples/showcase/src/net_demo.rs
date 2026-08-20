use serde::{Deserialize, Serialize};
use silex::prelude::*;

use crate::css::AppTheme;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatePostInput {
    pub title: String,
    pub body: String,
}

#[component]
pub fn HttpClientDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (post_id, set_post_id) = owner.signal(1)?;
    let search_query = owner.rw_signal(String::new())?;

    let new_title = owner.rw_signal("Silex Net Post".to_string())?;
    let new_body = owner.rw_signal("Created via Silex Net mutation_with.".to_string())?;

    // 1. Declarative HTTP fetching with path parameters, retry policy, and reactive closure query params
    let post_resource = HttpClient::get(
        owner,
        "https://jsonplaceholder.typicode.com/posts/{id}",
        error_handler,
    )
    .path_param("id", post_id)
    .query("filter", search_query)
    .timeout_ms(5000)
    .retry_policy(2, std::time::Duration::from_millis(300))
    .json::<Post>()
    .as_resource(post_id, None)
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    // 2. Using HttpClient::as_mutation_with for parameterized actions (POST)
    let create_post_builder = HttpClient::post(
        owner,
        "https://jsonplaceholder.typicode.com/posts",
        error_handler,
    )
    .json::<Post>();
    let create_post_mutation = create_post_builder
        .as_mutation_with(move |input: CreatePostInput| {
            let builder = HttpClient::post(
                owner,
                "https://jsonplaceholder.typicode.com/posts",
                error_handler,
            )
            .json_body(serde_json::json!({
                "title": input.title,
                "body": input.body,
                "userId": 1
            }))?;
            Ok(builder.json::<Post>())
        })
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    Ok(div![
        h3("HTTP Client Demo"),
        p(
            "Declarative HTTP fetching with path parameters, reactive closure query parameters, auto-retries, resources, and parameterized mutations."
        ),
        div![
            span("Fetch Post ID: "),
            input()
                .attr("type", "number")
                .prop("value", post_id)
                .on(event::input, move |e| {
                    if let Ok(id) = event_target_value(&e).parse::<i32>() {
                        set_post_id.set(id)?;
                    }
                    Ok(())
                })
                .style(
                    sty(ctx)
                        .margin_right(px(10))?
                        .padding("4px 8px")?
                        .border_radius(px(4))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .background(AppTheme::SURFACE)?
                        .color(AppTheme::TEXT)?
                ),
            span("Optional Filter Query: ").style(sty(ctx).margin_left(px(10))?),
            input()
                .placeholder("Type query...")
                .bind_value(search_query)
                .style(
                    sty(ctx)
                        .margin_right(px(10))?
                        .padding("4px 8px")?
                        .border_radius(px(4))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .background(AppTheme::SURFACE)?
                        .color(AppTheme::TEXT)?
                ),
            button("Refresh").on(event::click, move |_| {
                post_resource.refetch()?;
                Ok(())
            }),
        ]
        .style(
            sty(ctx)
                .margin_bottom(px(20))?
                .display("flex")?
                .align_items("center")?
                .flex_wrap(FlexWrapKeyword::Wrap)?
                .gap(px(8))?
        ),
        // Resource Display
        div![move || {
            Ok(match post_resource.state.get()? {
                ResourceState::Ready(post) | ResourceState::Reloading(post) => div![
                    h4(post.title).style(sty(ctx).color(AppTheme::PRIMARY)?.margin_top(px(0))?),
                    p(post.body).style(sty(ctx).opacity(0.8)?),
                    small(format!("User ID: {} | Post ID: {}", post.user_id, post.id))
                        .style(sty(ctx).opacity(0.6)?)
                ]
                .style(
                    sty(ctx)
                        .padding("20px")?
                        .background(AppTheme::SURFACE_ALT)?
                        .border_radius(px(8))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?,
                )
                .into_any(),
                ResourceState::Error(err) => div![
                    div("❌ Request Failed").style(
                        sty(ctx)
                            .color(ColorName::Red)?
                            .font_weight(FontWeightKeyword::Bold)?
                    ),
                    p(format!("{:?}", err)).style(sty(ctx).font_size(em_unit(0.8))?.opacity(0.7)?)
                ]
                .style(
                    sty(ctx)
                        .padding("20px")?
                        .border("1px solid red")?
                        .border_radius(px(8))?
                        .background("rgba(255,0,0,0.05)")?,
                )
                .into_any(),
                ResourceState::Loading if post_resource.get_data()?.is_none() => {
                    div("Loading post...")
                        .style(sty(ctx).padding("20px")?.color(AppTheme::PRIMARY)?)
                        .into_any()
                }
                _ => div("Select a post ID to fetch.")
                    .style(sty(ctx).padding("20px")?.opacity(0.5)?)
                    .into_any(),
            })
        }]
        .style(sty(ctx).min_height(px(120))?),
        hr().style(sty(ctx).margin("30px 0")?.border("0")?.border_top(border(
            px(1),
            BorderStyleKeyword::Solid,
            AppTheme::BORDER
        ))?),
        h4("Mutations (POST Request)"),
        div![
            div![
                input()
                    .placeholder("Post Title")
                    .bind_value(new_title)
                    .style(
                        sty(ctx)
                            .margin_right(px(10))?
                            .padding("6px 10px")?
                            .border_radius(px(4))?
                            .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                            .background(AppTheme::SURFACE)?
                            .color(AppTheme::TEXT)?
                            .width(px(200))?
                    ),
                input().placeholder("Post Body").bind_value(new_body).style(
                    sty(ctx)
                        .margin_right(px(10))?
                        .padding("6px 10px")?
                        .border_radius(px(4))?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .background(AppTheme::SURFACE)?
                        .color(AppTheme::TEXT)?
                        .width(px(300))?
                ),
                button("Submit Custom Post")
                    .on(event::click, move |_| {
                        create_post_mutation.mutate(CreatePostInput {
                            title: new_title.get()?,
                            body: new_body.get()?,
                        })?;
                        Ok(())
                    })
                    .attr("disabled", rx!(ctx; create_post_mutation.loading()?)?)
                    .style(
                        sty(ctx)
                            .padding("10px 20px")?
                            .background(AppTheme::PRIMARY)?
                            .color(ColorName::White)?
                            .border("0")?
                            .border_radius(px(6))?
                            .cursor("pointer")?
                    ),
            ]
            .style(
                sty(ctx)
                    .display("flex")?
                    .flex_wrap(FlexWrapKeyword::Wrap)?
                    .gap(px(8))?
                    .align_items("center")?
                    .margin_bottom(px(12))?
            ),
            move || {
                if create_post_mutation.loading()? {
                    Ok(span(" Creating...")
                        .style(sty(ctx).margin_left(px(10))?.color(AppTheme::PRIMARY)?)
                        .into_any())
                } else {
                    Ok("".into_any())
                }
            },
        ],
        move || {
            let view = if let Some(err) = create_post_mutation.error()? {
                div(format!("❌ Error creating post: {:?}", err))
                    .style(sty(ctx).color(ColorName::Red)?.margin_top(px(15))?)
                    .into_any()
            } else if let Some(post) = create_post_mutation.value()? {
                div![
                    div("✅ Post Created Successfully (Mock)!").style(
                        sty(ctx)
                            .color(ColorName::Green)?
                            .font_weight(FontWeightKeyword::Bold)?
                            .margin_bottom(px(5))?
                    ),
                    pre(format!("{:#?}", post)).style(
                        sty(ctx)
                            .background("#1e1e1e")?
                            .color(hex("#d4d4d4"))?
                            .padding("15px")?
                            .border_radius(px(6))?
                            .font_size(em_unit(0.85))?
                            .overflow_x(OverflowKeyword::Auto)?
                    )
                ]
                .style(sty(ctx).margin_top(px(15))?)
                .into_any()
            } else {
                "".into_any()
            };
            Ok(view)
        }
    ])
}

#[component]
pub fn WebSocketDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> + 'scope {
    let url = owner.rw_signal("wss://echo.websocket.org".to_string())?;
    let socket = WebSocket::lazy(owner, url.get_untracked()?, error_handler)
        .build()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let input_text = owner.rw_signal(String::new())?;

    let state_text = socket.state_str()?;
    let is_connected = socket.is_connected()?;

    let last_message = socket
        .raw_message()
        .into_rx()
        .map(|msg| msg.clone().unwrap_or_default(), error_handler)?;

    let send_message = move || -> SilexResult<()> {
        let text = input_text.get()?;
        if !text.trim().is_empty() {
            socket.send_text(&text).map_err(|error| {
                SilexError::recoverable(SilexErrorKind::Framework(error.to_string()))
            })?;
            input_text.set(String::new())?;
        }
        Ok(())
    };

    Ok(div![
        h3("WebSocket Demo"),
        p(
            "Real-time bidirectional communication with automatic connection state handling & Enter key support."
        ),
        div![
            input().bind_value(url).style(
                sty(ctx)
                    .flex_grow(1)?
                    .padding("8px")?
                    .border_radius(px(4))?
                    .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                    .background(AppTheme::SURFACE)?
                    .color(AppTheme::TEXT)?
            ),
            button(rx!(ctx; if *$is_connected { "Disconnect" } else { "Connect" })?)
                .on(event::click, move |_| {
                    socket.toggle().map_err(|error| {
                        SilexError::recoverable(SilexErrorKind::Framework(error.to_string()))
                    })?;
                    Ok(())
                })
                .style(
                    sty(ctx)
                        .padding("8px 16px")?
                        .margin_left(px(10))?
                        .border_radius(px(4))?
                        .cursor("pointer")?
                ),
        ]
        .style(sty(ctx).display("flex")?.margin_bottom(px(20))?),
        div![
            span("Status: "),
            strong(state_text).style(rx!(ctx; @fn if *$is_connected {
                sty(ctx).color(ColorName::Green)?
            } else {
                sty(ctx).color(ColorName::Red)?
            })?),
        ]
        .style(sty(ctx).margin_bottom(px(15))?),
        Show(ctx, is_connected)
            .children(div![
                div![
                    input()
                        .placeholder("Send message (Press Enter)...")
                        .bind_value(input_text)
                        .on(
                            event::keydown,
                            move |e: silex::reexports::web_sys::KeyboardEvent| {
                                if e.key() == "Enter" {
                                    send_message()?;
                                }
                                Ok(())
                            }
                        )
                        .style(
                            sty(ctx)
                                .padding("8px")?
                                .width(px(260))?
                                .border_radius(px(4))?
                                .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                                .background(AppTheme::SURFACE)?
                                .color(AppTheme::TEXT)?
                        ),
                    button("Send")
                        .on(event::click, move |_| {
                            send_message()?;
                            Ok(())
                        })
                        .style(
                            sty(ctx)
                                .margin_left(px(10))?
                                .padding("8px 16px")?
                                .cursor("pointer")?
                        ),
                ],
                div![
                    p("Last Echoed Message:"),
                    div(last_message).style(
                        sty(ctx)
                            .padding("15px")?
                            .background(AppTheme::SURFACE_ALT)?
                            .border_radius(px(6))?
                            .font_family("monospace")?
                            .border_left(border(
                                px(4),
                                BorderStyleKeyword::Solid,
                                AppTheme::PRIMARY
                            ))?
                    )
                ]
                .style(sty(ctx).margin_top(px(15))?),
            ])
            .build()
    ])
}

#[component]
pub fn EventStreamDemo<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let url = owner.rw_signal("https://stream.wikimedia.org/v2/stream/recentchange".to_string())?;
    let stream = EventStream::lazy(owner, url.get_untracked()?, error_handler)
        .build()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    let is_connected = stream.is_connected()?;
    let logs = stream.latest_messages::<WikimediaChange>(50)?;
    let stream_wiki_style = sty(ctx)
        .font_weight(FontWeightKeyword::Bold)?
        .opacity(0.6)?;
    let stream_title_style = sty(ctx)
        .color(AppTheme::PRIMARY)?
        .font_weight(FontWeightKeyword::Bold)?;
    let stream_user_style = sty(ctx).opacity(0.8)?.font_style("italic")?;
    let stream_type_style = sty(ctx).font_size(em_unit(0.85))?.opacity(0.5)?;
    let stream_row_style = sty(ctx)
        .font_family("sans-serif")?
        .font_size(em_unit(0.9))?
        .margin_bottom(px(6))?
        .border_bottom(border(
            px(1),
            BorderStyleKeyword::Solid,
            AppTheme::SURFACE_ALT,
        ))?
        .padding_bottom(px(4))?;

    Ok(div![
        h3("EventSource (SSE) Demo"),
        p(
            "One-way server-to-client streaming parsed directly into strongly typed Rust structs (Wikimedia Recent Changes)."
        ),
        div![
            input().bind_value(url).style(
                sty(ctx)
                    .flex_grow(1)?
                    .padding("8px")?
                    .border_radius(px(4))?
                    .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                    .background(AppTheme::SURFACE)?
                    .color(AppTheme::TEXT)?
            ),
            button(rx!(ctx; if *$is_connected { "Stop Stream" } else { "Start Stream" })?)
                .on(event::click, move |_| {
                    stream.toggle().map_err(|error| {
                        SilexError::recoverable(SilexErrorKind::Framework(error.to_string()))
                    })?;
                    Ok(())
                })
                .style(
                    sty(ctx)
                        .padding("8px 16px")?
                        .margin_left(px(10))?
                        .border_radius(px(4))?
                        .cursor("pointer")?
                ),
            button("Clear Log")
                .on(event::click, move |_| {
                    stream.clear_messages()?;
                    Ok(())
                })
                .style(
                    sty(ctx)
                        .padding("8px 16px")?
                        .margin_left(px(10))?
                        .border_radius(px(4))?
                        .cursor("pointer")?
                        .background("transparent")?
                        .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                        .color(AppTheme::TEXT)?
                ),
        ]
        .style(sty(ctx).display("flex")?.margin_bottom(px(20))?),
        div![
            h4("Stream Log (Wikipedia Edits):"),
            ul(For(ctx, logs, |item| item.id.unwrap_or(0).to_string()
                + &item.title)
            .children(move |change, _idx| {
                li(div![
                    span(format!("[{}] ", change.wiki)).style(stream_wiki_style.clone()),
                    span(format!("{} ", change.title)).style(stream_title_style.clone()),
                    span(format!("by {}", change.user)).style(stream_user_style.clone()),
                    span(format!(" ({})", change.change_type)).style(stream_type_style.clone())
                ])
                .style(stream_row_style.clone())
            })
            .build())
            .style(
                sty(ctx)
                    .max_height(px(320))?
                    .overflow_y(OverflowKeyword::Auto)?
                    .background(AppTheme::SURFACE)?
                    .border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?
                    .padding("15px")?
                    .border_radius(px(8))?
            )
        ]
    ])
}

#[component]
pub fn NetDemoPage<'scope, Ctx>(#[ctx] ctx: Ctx) -> impl View<'scope> {
    let (active_tab, set_active_tab) = owner.signal("http")?;

    inject_css! {
        .tab-nav { display: flex; gap: 10px; margin-bottom: 30px; border-bottom: 1px solid var(--slx-theme-border); padding-bottom: 15px; }
        .tab-nav button { background: none; border: none; padding: 10px 20px; cursor: pointer; color: var(--slx-theme-text); opacity: 0.6; border-radius: 6px; transition: all 0.3s; }
        .tab-nav button:hover { background: rgba(0,0,0,0.05); opacity: 1; }
        .tab-nav button.active { background: var(--slx-theme-primary); color: white; opacity: 1; font-weight: bold; }
        .demo-container { min-height: 400px; }
    };

    Ok(div![
        h2("🌐 Networking (silex_net)"),
        p("Comprehensive networking suite for Silex, supporting REST, WebSockets, and Server-Sent Events."),

        // Navigation Tabs
        div![
            button("HTTP Client")
                .on(event::click, set_active_tab.setter("http"))
                .classes(rx!(ctx; if *$active_tab == "http" { "active" } else { "" })?),
            button("WebSocket")
                .on(event::click, set_active_tab.setter("ws"))
                .classes(rx!(ctx; if *$active_tab == "ws" { "active" } else { "" })?),
            button("EventStream")
                .on(event::click, set_active_tab.setter("sse"))
                .classes(rx!(ctx; if *$active_tab == "sse" { "active" } else { "" })?),
        ].class("tab-nav"),

        // Content
        div![
            move || { Ok(match active_tab.get()? {
                "http" => HttpClientDemo(ctx).build().into_any(),
                "ws" => WebSocketDemo(ctx).build().into_any(),
                "sse" => EventStreamDemo(ctx).build().into_any(),
                _ => "".into_any(),
            })
            }
        ].class("demo-container")
    ]
    .style(sty(ctx).padding("24px")?.border(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?.border_radius(px(12))?.background(AppTheme::SURFACE)?.transition("all 0.3s")?)
    .classes("net-demo-page"))
}
