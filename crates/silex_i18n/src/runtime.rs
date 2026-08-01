use crate::{Catalog, I18nError, Locale, Message, PluralCategory, Segment, plural_category};
use silex_core::{
    reactivity::{ReadSignal, RwSignal},
    traits::{RxGet, RxRead, RxWrite},
};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug)]
pub struct I18nBuilder {
    locale: Option<Locale>,
    fallback_locale: Option<Locale>,
    catalogs: Vec<Catalog>,
    missing_key: MissingKeyPolicy,
    missing_argument: MissingArgumentPolicy,
}

impl I18nBuilder {
    pub fn new() -> Self {
        Self {
            locale: None,
            fallback_locale: None,
            catalogs: Vec::new(),
            missing_key: MissingKeyPolicy::default(),
            missing_argument: MissingArgumentPolicy::default(),
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

    pub fn build(self) -> Result<I18nStore, I18nError> {
        let catalog_locale = self
            .catalogs
            .first()
            .map(|catalog| catalog.locale().clone());
        let locale = self
            .locale
            .or(catalog_locale)
            .unwrap_or_else(|| Locale::new("en"));
        let fallback_locale = self.fallback_locale.unwrap_or_else(|| locale.clone());

        let mut registry = CatalogRegistry::default();
        for catalog in self.catalogs {
            registry.catalogs.insert(catalog.locale().clone(), catalog);
        }

        Ok(I18nStore {
            locale: RwSignal::new(locale),
            fallback_locale: RwSignal::new(fallback_locale),
            catalogs: RwSignal::new(registry),
            catalog_revision: RwSignal::new(0),
            missing_key: self.missing_key,
            missing_argument: self.missing_argument,
        })
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
        self.catalogs.update_untracked(|registry| {
            registry.catalogs.insert(catalog.locale().clone(), catalog);
        });
        self.catalog_revision.update(|revision| {
            *revision = revision.wrapping_add(1);
        });
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

    pub fn translate_now(&self, key: &str, arguments: &[Argument]) -> String {
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
) -> String {
    match message {
        Message::Text(segments) => render_segments(segments, arguments, missing_argument),
        Message::Plural { forms, count_name } => {
            let number = arguments
                .iter()
                .find(|argument| argument.name() == count_name)
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
