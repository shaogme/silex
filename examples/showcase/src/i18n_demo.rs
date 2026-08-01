use silex::prelude::*;

#[component]
pub fn I18nPage(i18n: I18nStore) -> impl View {
    let count = RwSignal::new(1u32);

    div![
        h2(t!(i18n, "i18n.title")),
        p(t!(i18n, "i18n.description")),
        p(t!(i18n, "welcome.user", name = "Silex")),
        p(t!(i18n, "cart.items", count = count.get())),
        div![
            button("中文").on_click(move |_| i18n.set_locale(Locale::new("zh-CN"))),
            button("English").on_click(move |_| i18n.set_locale(Locale::new("en-US"))),
            button("-").on_click(move |_| count.update(|value| *value = value.saturating_sub(1))),
            button("+").on_click(move |_| count.update(|value| *value += 1)),
        ]
        .style("display: flex; gap: 8px; margin-top: 16px;"),
    ]
    .style("padding: 20px;")
}
