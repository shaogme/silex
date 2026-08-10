use gloo_timers::future::TimeoutFuture;
use silex::prelude::*;

#[derive(I18nKeys)]
#[i18n(path = "locales/en-US.json", crate = "silex::i18n")]
enum DemoText {
    #[i18n(key = "i18n.title")]
    Title,
    #[i18n(key = "i18n.description")]
    Description,
    #[i18n(key = "welcome.user")]
    Welcome { name: String },
    #[i18n(key = "cart.items", count = "count")]
    CartItems { count: u32 },
}

fn panel<'scope, T, U>(title: T, content: U) -> AnyView<'scope>
where
    T: View<'scope> + 'scope,
    U: View<'scope> + 'scope,
{
    div![
        h3(title).style(
            sty()
                .margin_top(px(0))
                .margin_bottom(px(8))
                .color(crate::css::AppTheme::PRIMARY),
        ),
        content,
    ]
    .style(
        "padding: 20px; border: 1px solid var(--slx-theme-border); border-radius: 8px; background: var(--slx-theme-surface);",
    )
    .into_any()
}

fn control_row<'scope, T: View<'scope> + 'scope>(content: T) -> impl View<'scope> {
    div(content).style("display: flex; flex-wrap: wrap; align-items: center; gap: 8px;")
}

fn locale_button<'scope>(
    i18n: I18nStore<'scope>,
    loader_store: I18nStore<'scope>,
    locale: &'static str,
    label: &'static str,
) -> impl View<'scope> {
    button(label)
        .on_click(move |_| {
            let locale = Locale::new(locale);
            i18n.set_locale(locale.clone());
            loader_store.set_locale(locale);
            Ok(())
        })
        .style(
            "padding: 7px 11px; border: 1px solid var(--slx-theme-border); border-radius: 5px; background: var(--slx-theme-surface-alt); color: var(--slx-theme-text); cursor: pointer;",
        )
}

#[component]
pub fn I18nPage<'scope>(
    i18n: I18nStore<'scope>,
    ctx: RouterContext<'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();
    let available_locales = [
        Locale::new("en-US"),
        Locale::new("zh-CN"),
        Locale::new("ar-EG"),
        Locale::new("fr"),
    ];
    let fallback_locale = Locale::new("en-US");

    let query_locale = Persistent::builder(scope, "silex-showcase-query-locale", error_handler)
        .query(ctx)
        .parse::<Locale>()
        .default(i18n.locale().get_untracked())
        .build();

    let browser_candidates = navigator_languages();
    let browser_candidates_label = if browser_candidates.is_empty() {
        "(none reported)".to_string()
    } else {
        browser_candidates
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let browser_match = resolve_requested_locale(
        browser_candidates.clone(),
        &available_locales,
        &fallback_locale,
    );
    let browser_match_label = browser_match.to_string();

    let loader_store = I18nBuilder::new(scope, error_handler)
        .locale(i18n.locale().get_untracked())
        .fallback_locale(fallback_locale.clone())
        .build()
        .expect("valid loader store");
    let catalog_resource = loader_store.catalog_resource(
        move |locale| async move {
            TimeoutFuture::new(650).await;
            let source = match locale.language() {
                "zh" => include_str!("../locales/zh-CN.json"),
                "ar" => include_str!("../locales/ar-EG.json"),
                "fr" => include_str!("../locales/fr.json"),
                _ => include_str!("../locales/en-US.json"),
            };
            Catalog::from_json(locale, source).map_err(|error| error.to_string())
        },
        None,
    );

    let name = scope.rw_signal("Ada".to_string());
    let count = scope.rw_signal(1u32);

    let resource_for_view = catalog_resource.clone();
    let resource_for_reload = catalog_resource.clone();
    let reload_store = loader_store;
    let resource_state = move || match resource_for_view.state().get() {
        ResourceState::Idle => div(i18n.translate_now("demo.loader.idle", &[])).into_any(),
        ResourceState::Loading => div(i18n.translate_now("demo.loader.loading", &[])).into_any(),
        ResourceState::Reloading(catalog) => {
            let message = i18n.translate_now(
                "demo.loader.reloading",
                &[Argument::new("locale", catalog.locale())],
            );
            div(message).into_any()
        }
        ResourceState::Ready(catalog) => {
            let message = i18n.translate_now(
                "demo.loader.ready",
                &[
                    Argument::new("locale", catalog.locale()),
                    Argument::new("count", catalog.len()),
                ],
            );
            div![
                p(message),
                small(format!("Catalog cache contains {} key(s).", catalog.len()))
                    .style("opacity: 0.65;"),
            ]
            .into_any()
        }
        ResourceState::Error(error) => div(i18n.translate_now(
            "demo.loader.error",
            &[Argument::new("value", format!("{error:?}"))],
        ))
        .into_any(),
    };

    let formatter_number = move || {
        let locale = i18n.locale().get();
        let value = Intl::number_format(&locale)
            .format(1_234_567.89)
            .unwrap_or_else(|error| error.to_string());
        i18n.translate_now("demo.formatter.number", &[Argument::new("value", value)])
    };
    let formatter_date = move || {
        let locale = i18n.locale().get();
        let value = DateTimeFormat::new(&locale)
            .format(1_705_320_000_000.0)
            .unwrap_or_else(|error| error.to_string());
        i18n.translate_now("demo.formatter.date", &[Argument::new("value", value)])
    };

    let current_locale = t!(
        i18n,
        "demo.locale.current",
        locale = i18n.locale().get().to_string()
    );
    let fallback_locale_text = t!(
        i18n,
        "demo.locale.fallback",
        locale = i18n.fallback_locale().get().to_string()
    );
    let query_locale_text = t!(
        i18n,
        "demo.locale.query",
        locale = query_locale.get().to_string()
    );
    let browser_candidates_text = t!(
        i18n,
        "demo.locale.browser",
        locales = browser_candidates_label.clone()
    );
    let browser_match_text = t!(
        i18n,
        "demo.locale.browser_match",
        locale = browser_match_label.clone()
    );
    let browser_match_for_click = browser_match.clone();
    let current_for_query = i18n;
    let query_for_apply = query_locale;
    let query_for_write = query_locale;
    let loader_for_browser = loader_store;
    let i18n_for_browser = i18n;

    div![
        h2(t!(i18n, DemoText::Title)),
        p(t!(i18n, DemoText::Description)).style("opacity: 0.75;"),
        panel(
            t!(i18n, "demo.locale.title"),
            div![
                p(t!(i18n, "demo.locale.description")).style("opacity: 0.75;"),
                p(current_locale),
                p(fallback_locale_text),
                p(t!(i18n, "demo.locale.persistence")).style("opacity: 0.7;"),
                control_row(div![
                    locale_button(i18n, loader_store, "en-US", "English"),
                    locale_button(i18n, loader_store, "zh-CN", "中文"),
                    locale_button(i18n, loader_store, "ar-EG", "العربية"),
                    locale_button(i18n, loader_store, "fr-CA", "Français (fallback)"),
                ]),
                p(browser_candidates_text).style("margin-bottom: 4px; opacity: 0.75;"),
                p(browser_match_text).style("margin-top: 0; opacity: 0.75;"),
                control_row(button(t!(i18n, "demo.locale.use_browser")).on_click(move |_| {
                    i18n_for_browser.set_locale(browser_match_for_click.clone());
                    loader_for_browser.set_locale(browser_match_for_click.clone());
                    Ok(())
                })),
                p(query_locale_text).style("margin-bottom: 8px; opacity: 0.75;"),
                control_row(div![
                    button(t!(i18n, "demo.locale.apply_query")).on_click(move |_| {
                        let locale = query_for_apply.get();
                        current_for_query.set_locale(locale.clone());
                        loader_store.set_locale(locale);
                        Ok(())
                    }),
                    button(t!(i18n, "demo.locale.write_query")).on_click(move |_| {
                        query_for_write.set(current_for_query.locale().get_untracked());
                        Ok(())
                    }),
                ]),
            ]
            .style("display: grid; gap: 10px;"),
        ),
        panel(
            t!(i18n, "demo.messages.title"),
            div![
                p(t!(i18n, "demo.messages.description")).style("opacity: 0.75;"),
                div![
                    label(t!(i18n, "demo.messages.name"))
                        .style("display: block; margin-bottom: 5px;"),
                    input()
                        .bind_value(name)
                        .style("width: 100%; max-width: 320px; padding: 8px;"),
                    p(t!(i18n, DemoText::Welcome { name: name.get() })),
                ],
                div![
                    label(t!(i18n, "demo.messages.count"))
                        .style("display: block; margin-bottom: 5px;"),
                    control_row(div![
                        button("-").on_click(move |_| {
                            count.update(|value| *value = value.saturating_sub(1));
                            Ok(())
                        }),
                        span(count.map_fn(scope, |value| value.to_string()))
                            .style("min-width: 30px; text-align: center;"),
                        button("+").on_click(move |_| {
                            count.update(|value| *value = value.saturating_add(1));
                            Ok(())
                        }),
                    ]),
                    p(t!(i18n, "demo.messages.literal", value = count.get())),
                    p(t!(i18n, "demo.messages.typed")).style("opacity: 0.7;"),
                    p(t!(i18n, DemoText::CartItems { count: count.get() })),
                ],
                p(t!(
                    i18n,
                    "demo.messages.missing_key",
                    value = i18n.translate_now("demo.missing.key", &[])
                )),
                p(move || {
                    let value = i18n.translate_now("welcome.user", &[]);
                    i18n.translate_now(
                        "demo.messages.missing_argument",
                        &[Argument::new("value", value)],
                    )
                }),
            ]
            .style("display: grid; gap: 14px;"),
        ),
        panel(
            t!(i18n, "demo.formatter.title"),
            div![
                p(t!(i18n, "demo.formatter.description")).style("opacity: 0.75;"),
                p(formatter_number),
                p(formatter_date),
            ]
            .style("display: grid; gap: 8px;"),
        ),
        panel(
            t!(i18n, "demo.loader.title"),
            div![
                p(t!(i18n, "demo.loader.description")).style("opacity: 0.75;"),
                control_row(button(t!(i18n, "demo.loader.reload")).on_click(move |_| {
                    let locale = reload_store.locale().get_untracked();
                    reload_store.remove_catalog(&locale);
                    resource_for_reload.refetch();
                    Ok(())
                })),
                div(resource_state).style(
                    "min-height: 42px; padding: 10px; border-left: 3px solid var(--slx-theme-primary); background: var(--slx-theme-surface-alt);",
                ),
            ]
            .style("display: grid; gap: 10px;"),
        ),
        panel(
            t!(i18n, "demo.metadata.title"),
            div![
                p(t!(i18n, "demo.metadata.description")).style("opacity: 0.75;"),
                p(move || {
                    let direction = locale_direction(&i18n.locale().get());
                    i18n.translate_now(
                        "demo.metadata.direction",
                        &[Argument::new("value", direction.as_str())],
                    )
                }),
            ]
            .style("display: grid; gap: 8px;"),
        ),
    ]
    .style("max-width: 1100px; margin: 0 auto; padding: 24px; display: grid; gap: 18px;")
}
