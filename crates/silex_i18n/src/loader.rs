use crate::{Catalog, I18nError, Locale};
use silex_core::reactivity::{ReadSignal, Resource, ResourceState};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

/// A small locale-keyed cache for successfully loaded catalogs.
#[derive(Clone, Default)]
pub struct CatalogCache {
    catalogs: Rc<RefCell<HashMap<Locale, Catalog>>>,
}

impl CatalogCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, locale: &Locale) -> Option<Catalog> {
        self.catalogs.borrow().get(locale).cloned()
    }

    pub fn contains(&self, locale: &Locale) -> bool {
        self.catalogs.borrow().contains_key(locale)
    }

    pub fn len(&self) -> usize {
        self.catalogs.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.catalogs.borrow().is_empty()
    }

    /// Inserts a catalog and reports whether its value changed.
    pub fn insert(&self, catalog: Catalog) -> bool {
        let locale = catalog.locale().clone();
        let mut catalogs = self.catalogs.borrow_mut();
        if catalogs.get(&locale) == Some(&catalog) {
            return false;
        }
        catalogs.insert(locale, catalog);
        true
    }

    pub fn remove(&self, locale: &Locale) -> Option<Catalog> {
        self.catalogs.borrow_mut().remove(locale)
    }

    pub fn clear(&self) {
        self.catalogs.borrow_mut().clear();
    }
}

/// Errors produced by a catalog loader, including a response for the wrong locale.
#[derive(Clone, Debug, PartialEq)]
pub enum CatalogLoadError<E> {
    Loader(E),
    LocaleMismatch { requested: Locale, loaded: Locale },
}

impl<E> From<E> for CatalogLoadError<E> {
    fn from(error: E) -> Self {
        Self::Loader(error)
    }
}

impl<E: std::fmt::Display> std::fmt::Display for CatalogLoadError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loader(error) => write!(f, "catalog loader failed: {error}"),
            Self::LocaleMismatch { requested, loaded } => write!(
                f,
                "catalog loader returned {loaded} while requesting {requested}"
            ),
        }
    }
}

/// A `Resource` that loads catalogs for the store's current locale.
#[derive(Clone)]
pub struct CatalogResource<E = I18nError> {
    resource: Resource<Catalog, CatalogLoadError<E>>,
    cache: CatalogCache,
}

impl<E: Clone + Debug + 'static> CatalogResource<E> {
    pub(crate) fn new(
        resource: Resource<Catalog, CatalogLoadError<E>>,
        cache: CatalogCache,
    ) -> Self {
        Self { resource, cache }
    }

    pub fn state(&self) -> ReadSignal<ResourceState<Catalog, CatalogLoadError<E>>> {
        self.resource.state
    }

    pub fn resource(&self) -> Resource<Catalog, CatalogLoadError<E>> {
        self.resource
    }

    pub fn refetch(&self) {
        self.resource.refetch();
    }

    pub fn loading(&self) -> bool {
        self.resource.loading()
    }

    pub fn value(&self) -> Option<Catalog> {
        self.resource.value()
    }

    pub fn get_data(&self) -> Option<Catalog> {
        self.resource.get_data()
    }

    pub fn cache(&self) -> CatalogCache {
        self.cache.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_deduplicates_equal_catalogs() {
        let cache = CatalogCache::new();
        let catalog =
            Catalog::from_entries(Locale::new("en"), [("title", "Silex")]).expect("valid catalog");

        assert!(cache.insert(catalog.clone()));
        assert!(!cache.insert(catalog.clone()));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&Locale::new("en")), Some(catalog));
    }

    #[test]
    fn reports_locale_mismatch() {
        let error = CatalogLoadError::<I18nError>::LocaleMismatch {
            requested: Locale::new("en-US"),
            loaded: Locale::new("en-GB"),
        };
        assert!(error.to_string().contains("en-GB"));
    }
}
