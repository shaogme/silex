use silex::prelude::*;
mod advanced;
mod basics;
mod css;
mod flow_control;
mod net_demo;
mod persistence;
mod routes;

use advanced::{UserSettings, UserSettingsStore};
use routes::{AppRoute, NavBar};

fn main() {
    setup_global_error_handlers();

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
        username: "Guest".to_string(),
    });

    mount_to_body(move || {
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
            Router::new().match_route::<AppRoute>(),
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
