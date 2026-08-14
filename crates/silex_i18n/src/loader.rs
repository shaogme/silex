use crate::{Catalog, I18nError, Locale};
use silex_core::{
    SilexResult,
    reactivity::{ReadSignal, Resource, ResourceState},
};
use std::fmt::Debug;

/// Errors produced by a catalog loader, including a response for the wrong locale.
#[derive(Clone, Debug, PartialEq)]
pub enum CatalogLoadError<E> {
    Loader(E),
    LocaleMismatch { requested: Locale, loaded: Locale },
    Runtime(String),
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
            Self::Runtime(error) => write!(f, "catalog resource failed: {error}"),
        }
    }
}

/// A `Resource` that loads catalogs for the store's current locale.
#[derive(Clone)]
pub struct CatalogResource<'scope, E = I18nError> {
    resource: Resource<'scope, Catalog, CatalogLoadError<E>>,
}

impl<'scope, E: Clone + Debug + 'static> CatalogResource<'scope, E> {
    pub(crate) fn new(resource: Resource<'scope, Catalog, CatalogLoadError<E>>) -> Self {
        Self { resource }
    }

    pub fn state(&self) -> ReadSignal<'scope, ResourceState<Catalog, CatalogLoadError<E>>> {
        self.resource.state
    }

    pub fn resource(&self) -> Resource<'scope, Catalog, CatalogLoadError<E>> {
        self.resource
    }

    pub fn refetch(&self) -> SilexResult<()> {
        self.resource.refetch()
    }

    pub fn loading(&self) -> SilexResult<bool> {
        self.resource.loading()
    }

    pub fn value(&self) -> SilexResult<Option<Catalog>> {
        self.resource.value()
    }

    pub fn get_data(&self) -> SilexResult<Option<Catalog>> {
        self.resource.get_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_locale_mismatch() {
        let error = CatalogLoadError::<I18nError>::LocaleMismatch {
            requested: Locale::new("en-US").expect("valid test locale"),
            loaded: Locale::new("en-GB").expect("valid test locale"),
        };
        assert!(error.to_string().contains("en-GB"));
    }
}
