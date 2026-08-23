use crate::{
    MappedReadGuard, OwnerAccess, SilexResult,
    traits::{RuntimeScoped, RxBase, RxData, RxGet, RxRead, RxReadRef, RxReadRefSource, RxValue},
};
use std::marker::PhantomData;

/// A safe projection over a source reactive value.
pub struct SignalSlice<S, F, O: ?Sized> {
    pub(crate) source: S,
    pub(crate) getter: F,
    marker: PhantomData<fn() -> O>,
}

impl<S, F, O: ?Sized> SignalSlice<S, F, O> {
    pub(crate) fn new(source: S, getter: F) -> Self {
        Self {
            source,
            getter,
            marker: PhantomData,
        }
    }
}

impl<S, F, O> RxValue for SignalSlice<S, F, O>
where
    O: ?Sized + RxData,
{
    type Owned = O;
}

impl<S, F, O: ?Sized> RuntimeScoped for SignalSlice<S, F, O>
where
    S: RuntimeScoped,
{
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.source.owner_access()
    }
}

impl<S, F, O> RxBase for SignalSlice<S, F, O>
where
    S: RxBase,
    O: ?Sized + RxData,
{
    fn track(&self) -> SilexResult<()> {
        self.source.track()
    }
}

impl<S, F, O> RxRead for SignalSlice<S, F, O>
where
    S: RxReadRefSource,
    F: Fn(&S::Owned) -> &O,
    O: ?Sized + RxData,
{
    type ReadGuard<'a>
        = MappedReadGuard<S::ViewGuard<'a>, &'a F, S::Owned, O>
    where
        Self: 'a;

    fn read<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>> {
        Ok(MappedReadGuard::new(self.source.read_ref()?, &self.getter))
    }

    fn read_untracked<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>> {
        Ok(MappedReadGuard::new(
            self.source.read_ref_untracked()?,
            &self.getter,
        ))
    }
}

impl<S, F, O> RxReadRefSource for SignalSlice<S, F, O>
where
    S: RxReadRefSource,
    F: Fn(&S::Owned) -> &O,
    O: ?Sized + RxData,
{
    type ViewGuard<'a>
        = MappedReadGuard<S::ViewGuard<'a>, &'a F, S::Owned, O>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<S, F, O> RxGet for SignalSlice<S, F, O>
where
    S: RxReadRefSource,
    F: Fn(&S::Owned) -> &O,
    O: Clone + RxData,
{
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
    }
}
