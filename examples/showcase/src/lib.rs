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
    #[context] context: SilexContext<'scope>,
    i18n: I18nStore<'scope>,
    store: UserSettingsStore<'scope, 'scope>,
) -> impl View<'scope> + 'scope {
    let theme = store
        .theme
        .map(scope, |name| css::get_theme(name.as_str()), error_handler)?;

    let routes = routes!(AppRoutes {
        home "/" => move |ctx| routes::HomePage(ctx).build(),
        basics "/basics" => move |ctx| basics::BasicsPage(ctx).build(),
        flow "/flow" => move |ctx| flow_control::FlowPage(ctx).build(),
        i18n "/i18n" => move |ctx| i18n_demo::I18nPage(ctx, i18n).build(),
        net "/net" => move |ctx| net_demo::NetDemoPage(ctx).build(),
        persistence "/persistence" => move |ctx| {
            persistence::PersistencePage(ctx).build()
        },
        nest css "/css" => move |ctx, outlet| {
            routes::CssLayout(ctx, outlet)
                .build()
        } {
            basics "/" => move |ctx| css::StylingBasics(ctx).build(),
            theming "/theming" => move |ctx| css::Theming(ctx, store).build(),
            advanced "/advanced" => move |ctx| css::AdvancedStyling(ctx).build(),
        },
        nest advanced "/advanced" => move |ctx, outlet| {
            routes::AdvancedLayout(ctx, outlet)
                .build()
        } {
            index "/" => move |ctx| routes::SelectDemo(ctx).build(),
            store "/store" => move |ctx| advanced::StoreDemo(ctx, store).build(),
            query "/query" => move |ctx| advanced::QueryDemo(ctx, store).build(),
            storage "/storage" => move |ctx| advanced::StorageDemo(ctx).build(),
            resource "/resource" => move |ctx| advanced::ResourceDemo(ctx).build(),
            mutation "/mutation" => move |ctx| advanced::MutationDemo(ctx).build(),
            suspense "/suspense" => move |ctx| advanced::SuspenseDemo(ctx).build(),
            generics "/generics" => move |ctx| advanced::GenericsDemo(ctx).build(),
            adaptive "/adaptive" => move |ctx| advanced::AdaptiveReadDemo(ctx).build(),
        },
        not_found "/*" => move |ctx| routes::NotFoundPage(ctx).build(),
    })
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    Ok(div!(
        css::GlobalStyles(scope, error_handler),
        Router(context)
            .routes(routes.table())
            .layout(move |ctx, outlet| routes::AppLayout(ctx, outlet, store).build())
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
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_showcase_view)?;
    bootstrap.into_js_host()
}

/// Mount the showcase into a caller-provided target node.
pub fn mount_showcase_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    bootstrap.mount(Runtime::new(), mount_showcase_view)?;
    bootstrap.into_js_host()
}

fn mount_showcase_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;

    let parse_locale = |value: &str| {
        Locale::new(value)
            .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))
    };

    let en_catalog = Catalog::from_json(
        parse_locale("en-US")?,
        include_str!("../locales/en-US.json"),
    )
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let zh_catalog = Catalog::from_json(
        parse_locale("zh-CN")?,
        include_str!("../locales/zh-CN.json"),
    )
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let ar_catalog = Catalog::from_json(
        parse_locale("ar-EG")?,
        include_str!("../locales/ar-EG.json"),
    )
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let fr_catalog = Catalog::from_json(parse_locale("fr")?, include_str!("../locales/fr.json"))
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
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
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
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
        App(SilexContext::new(scope, error_handler), i18n, store).build(),
        error_handler,
    )
}
