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

router! {
    pub enum CssRoute {
        Basics => "/",
        Theming => "/theming",
        Advanced => "/advanced",
    }
}

router! {
    pub enum AdvancedRoute {
        Index => "/",
        Store => "/store",
        Query => "/query",
        Storage => "/storage",
        Resource => "/resource",
        Mutation => "/mutation",
        Suspense => "/suspense",
        Generics => "/generics",
        Adaptive => "/adaptive",
    }
}

router! {
    pub enum AppRoute {
        Home => "/",
        Basics => "/basics",
        Flow => "/flow",
        I18n => "/i18n",
        Net => "/net",
        Persistence => "/persistence",
        Css(CssRoute) {
            prefix: "/css";
            layout: |ctx, outlet| routes::CssLayout(ctx, outlet).build();
        },
        Advanced(AdvancedRoute) {
            prefix: "/advanced";
            layout: |ctx, outlet| routes::AdvancedLayout(ctx, outlet).build();
        },
        NotFound => "/*",
    }
}

#[component]
fn App<'scope>(
    #[context] context: SilexContext<'scope>,
    i18n: I18nStore<'scope>,
    store: UserSettingsStore<'scope, 'scope>,
) -> impl View<'scope> + 'scope {
    let theme = store
        .theme
        .map(scope, |name| css::get_theme(name.as_str()), error_handler)?;

    let table = AppRoute::table(move |route, ctx| match route {
        AppRoute::Home => routes::HomePage(ctx).build().into_any(),
        AppRoute::Basics => basics::BasicsPage(ctx).build().into_any(),
        AppRoute::Flow => flow_control::FlowPage(ctx).build().into_any(),
        AppRoute::I18n => i18n_demo::I18nPage(ctx, i18n).build().into_any(),
        AppRoute::Net => net_demo::NetDemoPage(ctx).build().into_any(),
        AppRoute::Persistence => persistence::PersistencePage(ctx).build().into_any(),
        AppRoute::Css(CssRoute::Basics) => css::StylingBasics(ctx).build().into_any(),
        AppRoute::Css(CssRoute::Theming) => css::Theming(ctx, store).build().into_any(),
        AppRoute::Css(CssRoute::Advanced) => css::AdvancedStyling(ctx).build().into_any(),
        AppRoute::Advanced(AdvancedRoute::Index) => routes::SelectDemo(ctx).build().into_any(),
        AppRoute::Advanced(AdvancedRoute::Store) => {
            advanced::StoreDemo(ctx, store).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Query) => {
            advanced::QueryDemo(ctx, store).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Storage) => advanced::StorageDemo(ctx).build().into_any(),
        AppRoute::Advanced(AdvancedRoute::Resource) => {
            advanced::ResourceDemo(ctx).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Mutation) => {
            advanced::MutationDemo(ctx).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Suspense) => {
            advanced::SuspenseDemo(ctx).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Generics) => {
            advanced::GenericsDemo(ctx).build().into_any()
        }
        AppRoute::Advanced(AdvancedRoute::Adaptive) => {
            advanced::AdaptiveReadDemo(ctx).build().into_any()
        }
        AppRoute::NotFound => routes::NotFoundPage(ctx).build().into_any(),
    })
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    Ok(div!(
        css::GlobalStyles(scope, error_handler),
        Router(context)
            .routes(table)
            .layout(move |ctx, outlet| routes::AppLayout(ctx, outlet, store).build())
            .build()
    )
    .apply(theme_variables(theme))
    .style(
        sty(context)
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
