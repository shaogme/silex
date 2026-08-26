use crate::flow::indexed::mount_indexed_list;
use crate::flow::keyed::{KeyedListConfig, mount_keyed_list};
use crate::flow::rows::RowUpdater;
use crate::kernel::elements::AnyView;
use crate::kernel::{MountContext, MountInstance, View};
use silex_core::ErrorHandlerToken;
use silex_core::SilexResult;
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use std::{hash::Hash, marker::PhantomData, rc::Rc};

pub struct RenderOnlyKeyedListView<'scope, IF, IS, T, K> {
    each: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    error_handler: Option<ErrorHandlerToken<'scope>>,
    marker: PhantomData<(IS, T)>,
}

pub struct StatefulKeyedListView<'scope, IF, IS, T, K> {
    each: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    view_fn: Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>,
    error_handler: Option<ErrorHandlerToken<'scope>>,
    marker: PhantomData<(IS, T)>,
}

pub struct IndexedListView<'scope, IF, T, IS> {
    each: IF,
    view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    marker: PhantomData<(T, IS)>,
}

#[derive(Clone)]
pub(crate) enum RowFactory<'scope, T> {
    RenderOnly(Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>),
    Stateful(Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>),
}

impl<'scope, T> RowFactory<'scope, T> {
    pub(crate) fn render(
        &self,
        item: T,
        index: usize,
        updater: RowUpdater<'scope, T>,
    ) -> AnyView<'scope> {
        match self {
            Self::RenderOnly(value) => value(item, index),
            Self::Stateful(value) => value(item, index, updater),
        }
    }

    pub(crate) fn stateful(&self) -> bool {
        matches!(self, Self::Stateful(_))
    }
}

impl<'scope, IF, T, IS> IndexedListView<'scope, IF, T, IS> {
    pub fn new(each: IF, view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>) -> Self {
        Self {
            each,
            view_fn,
            marker: PhantomData,
        }
    }
}

impl<'scope, IF, IS, T, K> RenderOnlyKeyedListView<'scope, IF, IS, T, K> {
    pub fn new(
        each: IF,
        key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
        view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
        error_handler: Option<ErrorHandlerToken<'scope>>,
    ) -> Self {
        Self {
            each,
            key_fn,
            view_fn,
            error_handler,
            marker: PhantomData,
        }
    }
}

impl<'scope, IF, IS, T, K> StatefulKeyedListView<'scope, IF, IS, T, K> {
    pub fn new(
        each: IF,
        key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
        view_fn: Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>,
        error_handler: Option<ErrorHandlerToken<'scope>>,
    ) -> Self {
        Self {
            each,
            key_fn,
            view_fn,
            error_handler,
            marker: PhantomData,
        }
    }
}

impl<'scope, IF, IS, T, K> View<'scope> for RenderOnlyKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    T: Clone + 'scope,
    K: Hash + Eq + Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListConfig {
            context: context.clone(),
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::RenderOnly(self.view_fn.clone()),
            custom_handler: self.error_handler.clone(),
            parent_handler: context.error_handler(),
        })
    }
}

impl<'scope, IF, IS, T, K> View<'scope> for StatefulKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    T: Clone + 'scope,
    K: Hash + Eq + Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListConfig {
            context: context.clone(),
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::Stateful(self.view_fn.clone()),
            custom_handler: self.error_handler.clone(),
            parent_handler: context.error_handler(),
        })
    }
}

impl<'scope, IF, T, IS> View<'scope> for IndexedListView<'scope, IF, T, IS>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_indexed_list(
            context.clone(),
            self.each.clone(),
            RowFactory::RenderOnly(self.view_fn.clone()),
        )
    }
}
