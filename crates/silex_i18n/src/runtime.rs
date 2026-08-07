use crate::{
    Catalog, CatalogLoadError, CatalogResource, I18nError, Locale, Message, PluralCategory,
    Segment, plural_category,
};
#[cfg(feature = "browser")]
use silex_core::Effect;
use silex_core::{
    ErrorReporter, Memo, Scope, SilexError, SilexResult,
    reactivity::{
        ReadSignal, Resource, ResourceState, RwSignal, StoredValue, SuspenseContext,
        runtime_inputs_of,
    },
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Debug,
    future::Future,
    rc::Rc,
};

#[cfg(feature = "persist")]
use silex_persist::Persistent;

use silex_core::RuntimeInputs;
#[cfg(feature = "persist")]
use silex_core::traits::RxGet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MissingKeyPolicy {
    #[default]
    ReturnKey,
    Empty,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MissingArgumentPolicy {
    #[default]
    KeepPlaceholder,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Argument {
    name: String,
    value: String,
}

pub trait I18nVariant {
    fn key(&self) -> &'static str;
    fn arguments(&self) -> Vec<Argument>;

    fn count_name(&self) -> Option<&'static str> {
        None
    }
}

impl Argument {
    pub fn new(name: impl Into<String>, value: impl ToString) -> Self {
        Self {
            name: name.into(),
            value: value.to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Default)]
struct CatalogRegistry {
    catalogs: HashMap<Locale, Catalog>,
}

impl CatalogRegistry {
    fn get(&self, locale: &Locale, key: &str) -> Option<&Message> {
        self.catalogs
            .get(locale)
            .and_then(|catalog| catalog.get(key))
    }

    fn has_catalog(&self, locale: &Locale) -> bool {
        self.catalogs.contains_key(locale)
    }

    fn catalog(&self, locale: &Locale) -> Option<Catalog> {
        self.catalogs.get(locale).cloned()
    }
}

#[derive(Clone, Copy)]
pub struct I18nStore<'scope> {
    scope: Scope<'scope>,
    error_handler: StoredValue<'scope, ErrorReporter<'scope>>,
    locale: RwSignal<'scope, Locale>,
    fallback_locale: RwSignal<'scope, Locale>,
    catalog_cache: StoredValue<'scope, Rc<RefCell<CatalogRegistry>>>,
    catalog_revision: RwSignal<'scope, u64>,
    missing_key: MissingKeyPolicy,
    missing_argument: MissingArgumentPolicy,
}

impl Debug for I18nStore<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("I18nStore")
            .field("missing_key", &self.missing_key)
            .field("missing_argument", &self.missing_argument)
            .finish_non_exhaustive()
    }
}

pub struct I18nBuilder<'scope> {
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    locale: Option<Locale>,
    fallback_locale: Option<Locale>,
    catalogs: Vec<Catalog>,
    missing_key: MissingKeyPolicy,
    missing_argument: MissingArgumentPolicy,
    #[cfg(feature = "persist")]
    locale_binding: Option<Persistent<'scope, Locale>>,
}

impl<'scope> I18nBuilder<'scope> {
    pub fn new(scope: Scope<'scope>, error_handler: ErrorReporter<'scope>) -> Self {
        Self {
            scope,
            error_handler,
            locale: None,
            fallback_locale: None,
            catalogs: Vec::new(),
            missing_key: MissingKeyPolicy::default(),
            missing_argument: MissingArgumentPolicy::default(),
            #[cfg(feature = "persist")]
            locale_binding: None,
        }
    }

    pub fn locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    pub fn fallback_locale(mut self, locale: Locale) -> Self {
        self.fallback_locale = Some(locale);
        self
    }

    pub fn catalog(mut self, catalog: Catalog) -> Self {
        self.catalogs.push(catalog);
        self
    }

    pub fn catalogs<I>(mut self, catalogs: I) -> Self
    where
        I: IntoIterator<Item = Catalog>,
    {
        self.catalogs.extend(catalogs);
        self
    }

    pub fn missing_key(mut self, policy: MissingKeyPolicy) -> Self {
        self.missing_key = policy;
        self
    }

    pub fn missing_argument(mut self, policy: MissingArgumentPolicy) -> Self {
        self.missing_argument = policy;
        self
    }

    #[cfg(feature = "persist")]
    pub fn locale_binding(mut self, binding: Persistent<'scope, Locale>) -> Self {
        self.locale_binding = Some(binding);
        self
    }

    pub fn build(self) -> Result<I18nStore<'scope>, I18nError> {
        let Self {
            scope,
            error_handler,
            locale,
            fallback_locale,
            catalogs,
            missing_key,
            missing_argument,
            #[cfg(feature = "persist")]
            locale_binding,
        } = self;

        let catalog_locale = catalogs.first().map(|catalog| catalog.locale().clone());

        let mut registry = CatalogRegistry::default();
        for catalog in catalogs {
            registry.catalogs.insert(catalog.locale().clone(), catalog);
        }

        #[cfg(feature = "persist")]
        let binding_inputs = locale_binding.map(|binding| {
            let inputs = runtime_inputs_of(binding);
            (binding, inputs)
        });

        #[cfg(feature = "persist")]
        if let Some((_, inputs)) = &binding_inputs {
            validate_inputs(scope, inputs)?;
        }

        #[cfg(feature = "persist")]
        let binding_locale = binding_inputs
            .as_ref()
            .map(|(binding, _)| binding.get_untracked());
        #[cfg(not(feature = "persist"))]
        let binding_locale: Option<Locale> = None;

        let locale = binding_locale
            .or(locale)
            .or(catalog_locale)
            .unwrap_or_else(|| Locale::new("en"));
        let fallback_locale = fallback_locale.unwrap_or_else(|| locale.clone());
        let catalog_cache = scope.stored(Rc::new(RefCell::new(registry)));
        let error_handler = scope.stored(error_handler);

        let store = I18nStore {
            scope,
            error_handler,
            locale: scope.rw_signal(locale),
            fallback_locale: scope.rw_signal(fallback_locale),
            catalog_cache,
            catalog_revision: scope.rw_signal(0),
            missing_key,
            missing_argument,
        };

        #[cfg(feature = "persist")]
        if let Some((binding, binding_inputs)) = binding_inputs {
            let locale_inputs = runtime_inputs_of(store.locale());

            let store_for_binding = store;
            scope
                .effect_from(
                    binding_inputs,
                    move || -> SilexResult<()> {
                        let locale = binding.signal().try_get()?;
                        if store_for_binding.locale.get_untracked() != locale {
                            store_for_binding.locale.set(locale);
                        }
                        Ok(())
                    },
                    store_for_binding.error_handler(),
                )
                .map_err(map_silex_error)?;

            let store_for_locale = store;
            scope
                .effect_from(
                    locale_inputs,
                    move || -> SilexResult<()> {
                        let locale = store_for_locale.locale.try_get()?;
                        if binding.get_untracked() != locale {
                            binding.set(locale);
                        }
                        Ok(())
                    },
                    store_for_locale.error_handler(),
                )
                .map_err(map_silex_error)?;
        }

        Ok(store)
    }
}

impl<'scope> I18nStore<'scope> {
    pub(crate) fn error_handler(&self) -> ErrorReporter<'scope> {
        self.error_handler.with(Clone::clone)
    }

    pub fn set_locale(&self, locale: Locale) {
        self.locale.set(locale);
    }

    #[cfg(feature = "browser")]
    pub(crate) fn scope(&self) -> Scope<'scope> {
        self.scope
    }

    pub fn locale(&self) -> ReadSignal<'scope, Locale> {
        self.locale.read_signal()
    }

    pub fn fallback_locale(&self) -> ReadSignal<'scope, Locale> {
        self.fallback_locale.read_signal()
    }

    pub fn set_fallback_locale(&self, locale: Locale) {
        self.fallback_locale.set(locale);
    }

    pub fn has_catalog(&self, locale: &Locale) -> bool {
        self.catalog_cache
            .with(|cache| cache.borrow().has_catalog(locale))
    }

    pub fn insert_catalog(&self, catalog: Catalog) {
        let changed = self.catalog_cache.with(|cache| {
            let mut registry = cache.borrow_mut();
            let locale = catalog.locale().clone();
            if registry.catalogs.get(&locale) == Some(&catalog) {
                return false;
            }
            registry.catalogs.insert(locale, catalog);
            true
        });
        if changed {
            self.catalog_revision.update(|revision| {
                *revision = revision.wrapping_add(1);
            });
        }
    }

    pub fn remove_catalog(&self, locale: &Locale) {
        let removed = self
            .catalog_cache
            .with(|cache| cache.borrow_mut().catalogs.remove(locale).is_some());
        if removed {
            self.catalog_revision.update(|revision| {
                *revision = revision.wrapping_add(1);
            });
        }
    }

    pub fn catalog_resource<F, Fut, E>(
        &self,
        loader: F,
        suspense_ctx: impl Into<Option<SuspenseContext<'scope>>>,
    ) -> CatalogResource<'scope, E>
    where
        F: Fn(Locale) -> Fut + 'static,
        Fut: Future<Output = Result<Catalog, E>> + 'static,
        E: Clone + Debug + 'static,
    {
        self.try_catalog_resource(loader, suspense_ctx)
            .unwrap_or_else(|error| panic!("创建 catalog resource 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_catalog_resource<F, Fut, E>(
        &self,
        loader: F,
        suspense_ctx: impl Into<Option<SuspenseContext<'scope>>>,
    ) -> Result<CatalogResource<'scope, E>, I18nError>
    where
        F: Fn(Locale) -> Fut + 'static,
        Fut: Future<Output = Result<Catalog, E>> + 'static,
        E: Clone + Debug + 'static,
    {
        let suspense = suspense_ctx.into();
        let mut inputs = runtime_inputs_of(self.locale());
        if let Some(context) = suspense.as_ref() {
            inputs.extend(&runtime_inputs_of(context.count));
        }
        validate_inputs(self.scope, &inputs)?;

        let cache = self.catalog_cache.with(Rc::clone);
        let loader = Rc::new(loader);
        let resource = Resource::new(
            self.scope,
            self.locale(),
            move |locale: Locale| {
                let cache = cache.clone();
                let loader = loader.clone();
                async move {
                    let cached = { cache.borrow().catalog(&locale) };
                    if let Some(catalog) = cached {
                        return Ok(catalog);
                    }

                    let catalog = loader(locale.clone())
                        .await
                        .map_err(CatalogLoadError::Loader)?;
                    if catalog.locale() != &locale {
                        return Err(CatalogLoadError::LocaleMismatch {
                            requested: locale,
                            loaded: catalog.locale().clone(),
                        });
                    }
                    Ok(catalog)
                }
            },
            suspense,
            self.error_handler(),
        );

        let state = resource.state;
        let state_inputs = runtime_inputs_of(state);
        let store = *self;
        self.scope
            .effect_from(
                state_inputs,
                move || -> SilexResult<()> {
                    if let ResourceState::Ready(catalog) = state.try_get()? {
                        store.insert_catalog(catalog);
                    }
                    Ok(())
                },
                store.error_handler(),
            )
            .map_err(map_silex_error)?;

        Ok(CatalogResource::new(resource))
    }

    #[cfg(feature = "browser")]
    pub fn sync_document_metadata(&self) -> SilexResult<Effect<'scope>> {
        crate::browser::sync_document_metadata(*self)
    }

    #[doc(hidden)]
    pub fn __memo<F>(self, mut f: F) -> Memo<'scope, String>
    where
        F: FnMut() -> String + 'scope,
    {
        self.scope.memo(move |_| f())
    }

    pub fn translate_now(&self, key: &str, arguments: &[Argument]) -> String {
        self.translate_now_with_count_name(key, arguments, None)
    }

    pub fn translate_variant_now<V>(&self, variant: &V) -> String
    where
        V: I18nVariant,
    {
        let arguments = variant.arguments();
        self.translate_now_with_count_name(variant.key(), &arguments, variant.count_name())
    }

    fn translate_now_with_count_name(
        &self,
        key: &str,
        arguments: &[Argument],
        count_name: Option<&str>,
    ) -> String {
        let locale = self.locale.get();
        let fallback_locale = self.fallback_locale.get();
        let _revision = self.catalog_revision.get();

        let translation = self.catalog_cache.with(|cache| {
            let registry = cache.borrow();
            let mut visited = HashSet::new();
            for candidate in locale
                .fallback_chain()
                .chain(fallback_locale.fallback_chain())
            {
                if visited.insert(candidate.clone())
                    && let Some(message) = registry.get(&candidate, key)
                {
                    return Some(render_message(
                        message,
                        &candidate,
                        arguments,
                        self.missing_argument,
                        count_name,
                    ));
                }
            }
            None
        });

        translation.unwrap_or_else(|| match self.missing_key {
            MissingKeyPolicy::ReturnKey => key.to_string(),
            MissingKeyPolicy::Empty => String::new(),
        })
    }
}

fn validate_inputs(scope: Scope<'_>, inputs: &RuntimeInputs) -> Result<(), I18nError> {
    scope
        .try_validate_inputs(inputs)
        .map_err(|error| match error {
            SilexError::Reactivity(error) => I18nError::from(error),
            error => {
                unreachable!("scope input validation returned a non-reactivity error: {error}")
            }
        })
}

fn map_silex_error(error: SilexError) -> I18nError {
    match error {
        SilexError::Reactivity(error) => I18nError::from(error),
        error => I18nError::InvalidCatalog(error.to_string()),
    }
}

fn render_message(
    message: &Message,
    locale: &Locale,
    arguments: &[Argument],
    missing_argument: MissingArgumentPolicy,
    count_name: Option<&str>,
) -> String {
    match message {
        Message::Text(segments) => render_segments(segments, arguments, missing_argument),
        Message::Plural {
            forms,
            count_name: message_count_name,
        } => {
            let number = arguments
                .iter()
                .find(|argument| argument.name() == count_name.unwrap_or(message_count_name))
                .and_then(|argument| argument.value().parse::<f64>().ok());
            let category = number
                .map(|number| plural_category(locale, number))
                .unwrap_or(PluralCategory::Other);
            let segments = forms
                .get(category)
                .or_else(|| forms.get(PluralCategory::Other))
                .expect("PluralForms always contains the other form");
            render_segments(segments, arguments, missing_argument)
        }
    }
}

fn render_segments(
    segments: &[Segment],
    arguments: &[Argument],
    missing_argument: MissingArgumentPolicy,
) -> String {
    let mut output = String::new();
    for segment in segments {
        match segment {
            Segment::Literal(value) => output.push_str(value),
            Segment::Argument(name) => {
                if let Some(argument) = arguments.iter().find(|argument| argument.name() == name) {
                    output.push_str(argument.value());
                } else if matches!(missing_argument, MissingArgumentPolicy::KeepPlaceholder) {
                    output.push('{');
                    output.push_str(name);
                    output.push('}');
                }
            }
        }
    }
    output
}
