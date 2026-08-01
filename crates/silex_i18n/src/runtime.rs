use crate::{
    Catalog, CatalogCache, CatalogLoadError, CatalogResource, I18nError, Locale, Message,
    PluralCategory, Segment, plural_category,
};
use silex_core::{
    reactivity::{Effect, ReadSignal, Resource, ResourceState, RwSignal, SuspenseContext},
    traits::{RxGet, RxRead, RxWrite},
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    future::Future,
    rc::Rc,
};

#[cfg(feature = "persist")]
use silex_persist::Persistent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingKeyPolicy {
    ReturnKey,
    Empty,
}

impl Default for MissingKeyPolicy {
    fn default() -> Self {
        Self::ReturnKey
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingArgumentPolicy {
    KeepPlaceholder,
    Empty,
}

impl Default for MissingArgumentPolicy {
    fn default() -> Self {
        Self::KeepPlaceholder
    }
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
}

#[derive(Clone, Copy, Debug)]
pub struct I18nStore {
    locale: RwSignal<Locale>,
    fallback_locale: RwSignal<Locale>,
    catalogs: RwSignal<CatalogRegistry>,
    catalog_revision: RwSignal<u64>,
    missing_key: MissingKeyPolicy,
    missing_argument: MissingArgumentPolicy,
}

pub struct I18nBuilder {
    locale: Option<Locale>,
    fallback_locale: Option<Locale>,
    catalogs: Vec<Catalog>,
    missing_key: MissingKeyPolicy,
    missing_argument: MissingArgumentPolicy,
    #[cfg(feature = "persist")]
    locale_binding: Option<Persistent<Locale>>,
}

impl I18nBuilder {
    pub fn new() -> Self {
        Self {
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
    pub fn locale_binding(mut self, binding: Persistent<Locale>) -> Self {
        self.locale_binding = Some(binding);
        self
    }

    pub fn build(self) -> Result<I18nStore, I18nError> {
        #[cfg(feature = "persist")]
        let locale_binding = self.locale_binding;
        let catalog_locale = self
            .catalogs
            .first()
            .map(|catalog| catalog.locale().clone());
        #[cfg(feature = "persist")]
        let binding_locale = locale_binding.as_ref().map(Persistent::get_untracked);
        #[cfg(not(feature = "persist"))]
        let binding_locale: Option<Locale> = None;
        let locale = binding_locale
            .or(self.locale)
            .or(catalog_locale)
            .unwrap_or_else(|| Locale::new("en"));
        let fallback_locale = self.fallback_locale.unwrap_or_else(|| locale.clone());

        let mut registry = CatalogRegistry::default();
        for catalog in self.catalogs {
            registry.catalogs.insert(catalog.locale().clone(), catalog);
        }

        let store = I18nStore {
            locale: RwSignal::new(locale),
            fallback_locale: RwSignal::new(fallback_locale),
            catalogs: RwSignal::new(registry),
            catalog_revision: RwSignal::new(0),
            missing_key: self.missing_key,
            missing_argument: self.missing_argument,
        };

        #[cfg(feature = "persist")]
        if let Some(binding) = locale_binding {
            store.bind_locale(binding);
        }

        Ok(store)
    }
}

impl Default for I18nBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl I18nStore {
    pub fn set_locale(&self, locale: Locale) {
        self.locale.set(locale);
    }

    #[cfg(feature = "persist")]
    pub fn bind_locale(&self, binding: Persistent<Locale>) {
        let store = *self;
        Effect::new(move |_| {
            let locale = binding.get();
            if store.locale.get_untracked() != locale {
                store.locale.set(locale);
            }
        });

        let store = *self;
        Effect::new(move |_| {
            let locale = store.locale.get();
            if binding.get_untracked() != locale {
                binding.set(locale);
            }
        });
    }

    pub fn locale(&self) -> ReadSignal<Locale> {
        self.locale.read_signal()
    }

    pub fn fallback_locale(&self) -> ReadSignal<Locale> {
        self.fallback_locale.read_signal()
    }

    pub fn set_fallback_locale(&self, locale: Locale) {
        self.fallback_locale.set(locale);
    }

    pub fn has_catalog(&self, locale: &Locale) -> bool {
        self.catalogs
            .with_untracked(|registry| registry.has_catalog(locale))
    }

    pub fn insert_catalog(&self, catalog: Catalog) {
        let changed = self.catalogs.update_untracked(|registry| {
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
            .catalogs
            .update_untracked(|registry| registry.catalogs.remove(locale).is_some());
        if removed {
            self.catalog_revision.update(|revision| {
                *revision = revision.wrapping_add(1);
            });
        }
    }

    pub(crate) fn catalog(&self, locale: &Locale) -> Option<Catalog> {
        self.catalogs
            .with_untracked(|registry| registry.catalogs.get(locale).cloned())
    }

    pub fn catalog_resource<F, Fut, E>(
        &self,
        loader: F,
        suspense_ctx: impl Into<Option<SuspenseContext>>,
    ) -> CatalogResource<E>
    where
        F: Fn(Locale) -> Fut + 'static,
        Fut: Future<Output = Result<Catalog, E>> + 'static,
        E: Clone + Debug + 'static,
    {
        self.catalog_resource_with_cache(loader, CatalogCache::new(), suspense_ctx)
    }

    pub fn catalog_resource_with_cache<F, Fut, E>(
        &self,
        loader: F,
        cache: CatalogCache,
        suspense_ctx: impl Into<Option<SuspenseContext>>,
    ) -> CatalogResource<E>
    where
        F: Fn(Locale) -> Fut + 'static,
        Fut: Future<Output = Result<Catalog, E>> + 'static,
        E: Clone + Debug + 'static,
    {
        let store = *self;
        let loader = Rc::new(loader);
        let fetch_cache = cache.clone();
        let resource = Resource::new(
            self.locale(),
            move |locale: Locale| {
                let cache = fetch_cache.clone();
                let loader = loader.clone();
                let store = store;
                async move {
                    if let Some(catalog) = store.catalog(&locale) {
                        return Ok(catalog);
                    }
                    if let Some(catalog) = cache.get(&locale) {
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
            suspense_ctx,
        );

        let state = resource.state;
        let effect_cache = cache.clone();
        Effect::new(move |_| {
            if let ResourceState::Ready(catalog) = state.get() {
                effect_cache.insert(catalog.clone());
                store.insert_catalog(catalog);
            }
        });

        CatalogResource::new(resource, cache)
    }

    #[cfg(feature = "browser")]
    pub fn sync_document_metadata(&self) {
        crate::browser::sync_document_metadata(*self);
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

        let translation = self.catalogs.with_untracked(|registry| {
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
