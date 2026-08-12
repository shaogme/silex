pub mod advanced;
pub mod basics;
pub mod css;
pub mod flow_control;
pub mod i18n_demo;
pub mod net_demo;
pub mod persistence;
pub mod routes;

use advanced::UserSettingsStore;
use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[component]
fn App<'scope>(
    scope: Scope<'scope>,
    i18n: I18nStore<'scope>,
    store: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> + 'scope {
    let theme = store
        .theme
        .map(scope, |name| css::get_theme(name.as_str()), error_handler)?;

    let routes = routes!(AppRoutes {
        home "/" => move |ctx| routes::HomePage(ctx).build(),
        basics "/basics" => move |_ctx| basics::BasicsPage(scope, error_handler).build(),
        flow "/flow" => move |_ctx| flow_control::FlowPage(scope, error_handler).build(),
        i18n "/i18n" => move |ctx| i18n_demo::I18nPage(i18n, ctx, error_handler).build(),
        net "/net" => move |_ctx| net_demo::NetDemoPage(scope, error_handler).build(),
        persistence "/persistence" => move |ctx| {
            persistence::PersistencePage(ctx, error_handler).build()
        },
        nest css "/css" => move |ctx, outlet| routes::CssLayout(ctx, outlet).build() {
            basics "/" => move |_ctx| css::StylingBasics(scope, error_handler).build(),
            theming "/theming" => move |_ctx| css::Theming(scope, store, error_handler).build(),
            advanced "/advanced" => move |_ctx| css::AdvancedStyling(scope, error_handler).build(),
        },
        nest advanced "/advanced" => move |ctx, outlet| {
            routes::AdvancedLayout(ctx, outlet).build()
        } {
            index "/" => move |_ctx| routes::SelectDemo().build(),
            store "/store" => move |_ctx| advanced::StoreDemo(scope, store, error_handler).build(),
            query "/query" => move |ctx| advanced::QueryDemo(ctx, store, error_handler).build(),
            storage "/storage" => move |_ctx| advanced::StorageDemo(scope, error_handler).build(),
            resource "/resource" => move |_ctx| advanced::ResourceDemo(scope, error_handler).build(),
            mutation "/mutation" => move |_ctx| advanced::MutationDemo(scope, error_handler).build(),
            suspense "/suspense" => move |_ctx| advanced::SuspenseDemo(scope, error_handler).build(),
            generics "/generics" => move |_ctx| advanced::GenericsDemo(scope).build(),
            adaptive "/adaptive" => move |_ctx| advanced::AdaptiveReadDemo(scope, error_handler).build(),
        },
        not_found "/*" => move |ctx| routes::NotFoundPage(ctx).build(),
    })
    .map_err(|error| SilexError::Framework(error.to_string()))?;

    Ok(div!(
        css::GlobalStyles(scope, error_handler),
        Router(scope, error_handler)
            .routes(routes.table())
            .layout(move |ctx, outlet| routes::AppLayout(ctx, outlet, store, error_handler).build())
            .build()
    )
    .apply(theme_variables(theme))
    .style(
        sty()
            .min_height(vh(100))?
            .font_family("Segoe UI, Roboto, Helvetica Neue, Arial, sans-serif")?
            .background(css::AppTheme::SURFACE)?
            .color(css::AppTheme::TEXT)?
            .transition("background-color 0.3s, color 0.3s")?,
    ))
}

/// Mount the showcase into the conventional `#app` target.
pub fn mount_showcase() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    bootstrap.mount(Runtime::new(), mount_showcase_view)?;
    bootstrap.into_js_host()
}

/// Mount the showcase into a caller-provided target node.
pub fn mount_showcase_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
    bootstrap.mount(Runtime::new(), mount_showcase_view)?;
    bootstrap.into_js_host()
}

fn mount_showcase_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;

    let parse_locale =
        |value: &str| Locale::new(value).map_err(|error| SilexError::Framework(error.to_string()));

    let en_catalog = Catalog::from_json(
        parse_locale("en-US")?,
        include_str!("../locales/en-US.json"),
    )
    .map_err(|error| SilexError::Framework(error.to_string()))?;
    let zh_catalog = Catalog::from_json(
        parse_locale("zh-CN")?,
        include_str!("../locales/zh-CN.json"),
    )
    .map_err(|error| SilexError::Framework(error.to_string()))?;
    let ar_catalog = Catalog::from_json(
        parse_locale("ar-EG")?,
        include_str!("../locales/ar-EG.json"),
    )
    .map_err(|error| SilexError::Framework(error.to_string()))?;
    let fr_catalog = Catalog::from_json(parse_locale("fr")?, include_str!("../locales/fr.json"))
        .map_err(|error| SilexError::Framework(error.to_string()))?;
    let available_locales = [
        parse_locale("en-US")?,
        parse_locale("zh-CN")?,
        parse_locale("ar-EG")?,
        parse_locale("fr")?,
    ];
    let fallback_locale = parse_locale("en-US")?;
    let browser_locale =
        resolve_requested_locale(navigator_languages(), &available_locales, &fallback_locale);
    let locale_binding = Persistent::builder(scope, "silex-showcase-locale", error_handler)
        .local()
        .parse::<Locale>()
        .default(browser_locale.clone())
        .build()?;
    let i18n = I18nBuilder::new(scope, error_handler)
        .locale(browser_locale)
        .fallback_locale(fallback_locale)
        .locale_binding(locale_binding)
        .catalog(en_catalog)
        .catalog(zh_catalog)
        .catalog(ar_catalog)
        .catalog(fr_catalog)
        .build()
        .map_err(|error| SilexError::Framework(error.to_string()))?;
    let _metadata_effect = i18n.sync_document_metadata()?;

    let theme = Persistent::builder(scope, "silex-showcase-theme", error_handler)
        .local()
        .string()
        .default("Light".to_string())
        .build()?;
    let notifications =
        Persistent::builder(scope, "showcase-settings-notif_enabled", error_handler)
            .local()
            .parse::<bool>()
            .default(true)
            .build()?;
    let username = Persistent::builder(scope, "showcase-settings-username", error_handler)
        .local()
        .cow()
        .default(std::borrow::Cow::Borrowed("Guest"))
        .build()?;
    let store = UserSettingsStore::from_handles(scope, theme, notifications, username)?;

    context.mount(
        App(scope, i18n, store, error_handler).build(),
        error_handler,
    )
}
