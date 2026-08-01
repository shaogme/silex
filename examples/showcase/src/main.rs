mod advanced;
mod basics;
mod css;
mod flow_control;
mod i18n_demo;
mod net_demo;
mod persistence;
mod routes;

use advanced::{UserSettings, UserSettingsStore};
use routes::{AppRoute, NavBar};
use silex::prelude::*;

fn main() {
    setup_global_error_handlers();

    let en_catalog =
        Catalog::from_json(Locale::new("en-US"), include_str!("../locales/en-US.json"))
            .expect("valid en-US catalog");
    let zh_catalog =
        Catalog::from_json(Locale::new("zh-CN"), include_str!("../locales/zh-CN.json"))
            .expect("valid zh-CN catalog");
    let ar_catalog =
        Catalog::from_json(Locale::new("ar-EG"), include_str!("../locales/ar-EG.json"))
            .expect("valid ar-EG catalog");
    let fr_catalog = Catalog::from_json(Locale::new("fr"), include_str!("../locales/fr.json"))
        .expect("valid fr catalog");
    let available_locales = [
        Locale::new("en-US"),
        Locale::new("zh-CN"),
        Locale::new("ar-EG"),
        Locale::new("fr"),
    ];
    let fallback_locale = Locale::new("en-US");
    let browser_locale =
        resolve_requested_locale(navigator_languages(), &available_locales, &fallback_locale);
    let locale_binding = Persistent::builder("silex-showcase-locale")
        .local()
        .parse::<Locale>()
        .default(browser_locale.clone())
        .build();
    let i18n = I18nBuilder::new()
        .locale(browser_locale)
        .fallback_locale(fallback_locale)
        .catalog(en_catalog)
        .catalog(zh_catalog)
        .catalog(ar_catalog)
        .catalog(fr_catalog)
        .build()
        .expect("valid i18n configuration");

    // 1. 使用持久化 Hook 代替手动的 localStorage 读取
    let theme_persistent = Persistent::builder("silex-showcase-theme")
        .local()
        .string()
        .default("Light".to_string())
        .build();

    // Global State Initialization
    let store = UserSettingsStore::new(UserSettings {
        theme: theme_persistent.get_untracked(),
        notifications: true,
        username: "Guest".into(),
    });

    mount_to_body(move || {
        i18n.bind_locale(locale_binding);
        i18n.sync_document_metadata();

        // Provide Global Store to the entire app tree
        store.provide();

        // Create the global theme signal and sync it inside the reactive scope
        let (theme_signal, set_theme_signal) =
            Signal::pair(crate::css::get_theme(&store.theme.get_untracked()));

        // 副作用：当 Store 中的主题变化时，同步给持久化信号、DOM 属性和 CSS 引擎
        Effect::new({
            let store = store;
            let theme_persistent = theme_persistent;
            move |_| {
                let theme_name = store.theme.get();

                // 同步至持久化信号（这会自动触发 localStorage 的写入）
                theme_persistent.set(theme_name.clone());

                // 同步至 <html> 的 data-theme 属性（用于 CSS 选择器）
                if let Some(win) = ::silex::reexports::web_sys::window()
                    && let Some(doc) = win.document()
                    && let Some(root) = doc.document_element()
                {
                    let _ = root.set_attribute("data-theme", &theme_name);
                }

                console_log(format!("Global Sync: switching theme to {}", theme_name));
                set_theme_signal.set(crate::css::get_theme(&theme_name));
            }
        });

        // 跨标签同步支持：
        // 如果用户在另一个标签页改了主题，持久化信号会变化，将其同步回 Store
        Effect::new({
            let store = store;
            let theme_persistent = theme_persistent;
            move |_| {
                let name = theme_persistent.get();
                if store.theme.get_untracked() != name {
                    store.theme.set(name);
                }
            }
        });

        // Apply theme to :root reactive updates
        set_global_theme(theme_signal);

        // Define and return the root view
        div![
            // Global Styles Component (Automatic injection)
            crate::css::GlobalStyles(),
            // Global Layout Shell
            NavBar(),
            // Root Router
            Router().match_enum(move |route, ctx| match route {
                AppRoute::I18n => i18n_demo::I18nPage(i18n, ctx).into_any(),
                route => route.render(ctx),
            }),
        ]
        .style(
            sty()
                .background_color(crate::css::AppTheme::SURFACE)
                .color(crate::css::AppTheme::TEXT)
                .min_height(vh(100))
                .transition("background-color 0.3s, color 0.3s"),
        )
    });
}
