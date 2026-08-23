//! Lifetime-aware reactive traits.

use crate::{
    Callback, ErrorHandlerInput, NodeRef, OwnerAccess, Rx, SilexError, SilexResult,
    reactivity::{
        Computed, MappedOptionReadGuard, ReactiveSource, ReadSignal, Signal, StoredValue,
    },
};
use std::fmt::Debug;
use std::ops::DerefMut;

/// Values accepted by the scoped runtime.
pub trait RxData {}
impl<T: ?Sized> RxData for T {}

pub trait RxCloneData: Clone {}
impl<T: Clone> RxCloneData for T {}

pub trait RxError: Clone + Debug {}
impl<T: Clone + Debug> RxError for T {}

/// Construct a owner-owned reactive wrapper from an explicit value.
///
/// Unlike [`From`], this trait receives the [`OwnerAccess`] that owns the node.
/// Implementations must create every node, callback, and owner resource from
/// that owner. They must not create a [`crate::Runtime`], a detached owner, or use
/// thread-local runtime state.
pub trait RxFrom<'owner>: Sized {
    type Owned: 'owner;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>;
}

/// Convert an existing scoped reactive source or an explicitly supported
/// value into a target reactive wrapper.
///
/// Unlike [`Into`], this trait receives the [`OwnerAccess`] that owns a node created
/// for a constant value. Existing sources are converted without materializing
/// another node, so their runtime provenance remains attached to the original
/// handle.
#[diagnostic::on_unimplemented(
    message = "reactive input must be an existing scoped source or an explicitly supported value",
    note = "constant reactive inputs require a OwnerAccess<'owner> and only the framework-supported value types are accepted"
)]
pub trait ReactiveInput<'owner, Target>: Sized {
    fn into_reactive_input(self, owner: OwnerAccess<'owner>) -> SilexResult<Target>;
}

/// Construct a owner-owned reactive wrapper from its value's default.
///
/// Every [`RxFrom`] implementation automatically implements this trait. The
/// default operation only delegates to [`RxFrom::rx_from`], so it cannot
/// create a [`crate::Runtime`], a detached owner, or thread-local runtime state.
pub trait RxDefault<'owner>: RxFrom<'owner> {
    fn rx_default(owner: OwnerAccess<'owner>) -> SilexResult<Self>
    where
        Self::Owned: Default,
    {
        Self::rx_from(owner, Self::Owned::default())
    }
}

impl<'owner, T> RxDefault<'owner> for T where T: RxFrom<'owner> {}

pub trait RxValue {
    type Owned: ?Sized;
}

/// Establish a dependency without borrowing or cloning the source payload.
pub trait RxBase: RxValue {
    fn track(&self) -> SilexResult<()>;
}

/// A live lease held by a borrowed reactive view.
pub trait RxReadLease {
    fn finish(self) -> SilexResult<()>;
}

/// A guard shape that exposes one borrowed payload.
pub trait RxRefGuard<T: ?Sized>: RxReadLease {
    fn with_ref<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> U;
}

/// A guard shape that exposes an optional borrowed payload.
pub trait RxOptionGuard<T>: RxReadLease {
    fn with_option<U>(&self, f: impl for<'view> FnOnce(Option<&'view T>) -> U) -> U;
}

/// The source-side adapter needed to avoid stable Rust's GAT higher-ranked
/// bound limitation for non-`'static` sources.
pub trait RxReadRefSource: RxRead {
    type ViewGuard<'a>: RxRefGuard<Self::Owned>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
}

/// Closure-based access for sources whose callback view is `&T`.
pub trait RxRead: RxBase {
    type ReadGuard<'a>: RxReadLease
    where
        Self: 'a;

    fn read<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>>;

    fn read_untracked<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>>;
}

pub trait RxReadRef<T: ?Sized>: RxRead<Owned = T> + RxReadRefSource {
    fn with<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> SilexResult<U> {
        let guard = self.read_ref()?;
        let value = guard.with_ref(f);
        guard.finish()?;
        Ok(value)
    }

    fn with_untracked<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> SilexResult<U> {
        let guard = self.read_ref_untracked()?;
        let value = guard.with_ref(f);
        guard.finish()?;
        Ok(value)
    }
}

impl<S, T: ?Sized> RxReadRef<T> for S where S: RxRead<Owned = T> + RxReadRefSource {}

/// The source-side adapter needed by the optional borrowed view.
pub trait RxReadOptionSource<T>: RxRead<Owned = Option<T>> {
    type ViewGuard<'a>: RxOptionGuard<T>
    where
        Self: 'a;

    fn read_option<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
    fn read_option_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
}

fn option_view<T>(value: &Option<T>) -> Option<&T> {
    value.as_ref()
}

impl<S, T> RxReadOptionSource<T> for S
where
    S: RxRead<Owned = Option<T>> + RxReadRefSource,
{
    type ViewGuard<'a>
        = MappedOptionReadGuard<S::ViewGuard<'a>, fn(&Option<T>) -> Option<&T>, Option<T>, T>
    where
        Self: 'a;

    fn read_option<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        Ok(MappedOptionReadGuard::new(
            self.read_ref()?,
            option_view::<T>,
        ))
    }

    fn read_option_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        Ok(MappedOptionReadGuard::new(
            self.read_ref_untracked()?,
            option_view::<T>,
        ))
    }
}

/// Closure-based access for sources whose callback view is `Option<&T>`.
pub trait RxReadOption<T>: RxRead<Owned = Option<T>> + RxReadOptionSource<T> {
    fn with<U>(&self, f: impl for<'view> FnOnce(Option<&'view T>) -> U) -> SilexResult<U> {
        let guard = self.read_option()?;
        let value = guard.with_option(f);
        guard.finish()?;
        Ok(value)
    }

    fn with_untracked<U>(
        &self,
        f: impl for<'view> FnOnce(Option<&'view T>) -> U,
    ) -> SilexResult<U> {
        let guard = self.read_option_untracked()?;
        let value = guard.with_option(f);
        guard.finish()?;
        Ok(value)
    }
}

impl<S, T> RxReadOption<T> for S where S: RxRead<Owned = Option<T>> + RxReadOptionSource<T> {}

macro_rules! define_tuple_read_traits {
    ($source:ident, $read:ident, $guard:ident, $($name:ident),+) => {
        pub trait $guard<$($name),+>: RxReadLease {
            fn with_tuple<U>(
                &self,
                f: impl for<'view> FnOnce(($(&'view $name,)+)) -> U,
            ) -> U;
        }

        pub trait $source<$($name),+>: RxRead<Owned = ($($name,)+)> {
            type ViewGuard<'a>: $guard<$($name),+>
            where
                Self: 'a;

            fn read_tuple<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
            fn read_tuple_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>>;
        }

        pub trait $read<$($name),+>:
            RxRead<Owned = ($($name,)+)> + $source<$($name),+>
        {
            fn with<U>(
                &self,
                f: impl for<'view> FnOnce(($(&'view $name,)+)) -> U,
            ) -> SilexResult<U> {
                let guard = self.read_tuple()?;
                let value = guard.with_tuple(f);
                guard.finish()?;
                Ok(value)
            }

            fn with_untracked<U>(
                &self,
                f: impl for<'view> FnOnce(($(&'view $name,)+)) -> U,
            ) -> SilexResult<U> {
                let guard = self.read_tuple_untracked()?;
                let value = guard.with_tuple(f);
                guard.finish()?;
                Ok(value)
            }
        }

        impl<S, $($name),+> $read<$($name),+> for S
        where
            S: RxRead<Owned = ($($name,)+)> + $source<$($name),+>,
        {}
    };
}

define_tuple_read_traits!(RxReadTupleSource1, RxReadTuple1, RxTupleGuard1, A);
define_tuple_read_traits!(RxReadTupleSource2, RxReadTuple2, RxTupleGuard2, A, B);
define_tuple_read_traits!(RxReadTupleSource3, RxReadTuple3, RxTupleGuard3, A, B, C);
define_tuple_read_traits!(RxReadTupleSource4, RxReadTuple4, RxTupleGuard4, A, B, C, D);
define_tuple_read_traits!(
    RxReadTupleSource5,
    RxReadTuple5,
    RxTupleGuard5,
    A,
    B,
    C,
    D,
    E
);
define_tuple_read_traits!(
    RxReadTupleSource6,
    RxReadTuple6,
    RxTupleGuard6,
    A,
    B,
    C,
    D,
    E,
    F
);

/// Exposes the runtime provenance retained by a scoped reactive source.
pub trait RuntimeScoped {
    fn owner_access(&self) -> OwnerAccess<'_>;
}

impl<'owner, T: 'owner> RxFrom<'owner> for ReadSignal<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.signal(value.into()).map(Into::into)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Signal<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.signal(value.into())
    }
}

impl<'owner, T: Clone + PartialEq + 'owner> RxFrom<'owner> for Computed<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        let value = value.into();
        let handler = owner.error_handler(|_: SilexError| {
            unreachable!("constant computed cannot report a user error")
        })?;
        owner.computed(move || Ok::<T, SilexError>(value.clone()), handler)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for StoredValue<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.stored(value.into())
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Rx<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.constant(value.into())
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Callback<'owner, T> {
    type Owned = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.callback(|_: T| Ok::<(), SilexError>(()))
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for NodeRef<'owner, T> {
    type Owned = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.node_ref()
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Computed<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Computed<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, StoredValue<'owner, T>> for StoredValue<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<StoredValue<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for StoredValue<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Rx<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self)
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'owner> ReactiveInput<'owner, ReadSignal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $value>> {
                    <ReadSignal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Signal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $value>> {
                    <Signal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Computed<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $value>> {
                    <Computed<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, StoredValue<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $value>> {
                    <StoredValue<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Rx<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $value>> {
                    <Rx<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_values!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
);

macro_rules! impl_reactive_input_str_values {
    ($($target:ty),* $(,)?) => {
        $(
            impl<'owner, 'value> ReactiveInput<'owner, ReadSignal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $target>> {
                    <ReadSignal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Signal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $target>> {
                    <Signal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Computed<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $target>> {
                    <Computed<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, StoredValue<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $target>> {
                    <StoredValue<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Rx<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $target>> {
                    <Rx<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_str_values!(String);

/// Explicit owned-value reads.
pub trait RxGet: RxRead
where
    Self::Owned: Sized,
{
    fn get_untracked(&self) -> SilexResult<Self::Owned>;

    fn get(&self) -> SilexResult<Self::Owned>;
}

/// Unified scoped writes.
pub trait RxWrite: RxValue {
    type WriteGuard<'a>: DerefMut<Target = Self::Owned>
    where
        Self: 'a;

    fn write(&self) -> SilexResult<Self::WriteGuard<'_>>;

    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Owned) -> U) -> SilexResult<U>;

    fn rx_notify(&self) -> SilexResult<()>;

    fn update<U>(&self, f: impl FnOnce(&mut Self::Owned) -> U) -> SilexResult<U> {
        self.rx_update_untracked(f)
    }

    fn set(&self, value: Self::Owned) -> SilexResult<()>
    where
        Self::Owned: Sized,
    {
        self.update(|current| *current = value)
    }

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Self::Owned) -> U) -> SilexResult<U> {
        self.rx_update_untracked(f)
    }

    fn set_untracked(&self, value: Self::Owned) -> SilexResult<()>
    where
        Self::Owned: Sized,
    {
        self.update_untracked(|current| *current = value)
    }

    fn notify(&self) -> SilexResult<()> {
        self.rx_notify()
    }

    fn setter(self, value: Self::Owned) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Owned: Sized + Clone,
    {
        move || self.set(value.clone())
    }

    fn updater<F>(self, f: F) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Owned: Sized,
        F: Fn(&mut Self::Owned) + Clone,
    {
        move || self.update(f.clone())
    }
}

/// Reactive helpers for `Option<T>` values.
pub trait RxOptionExt<T>: RxReadOption<T> + Clone {
    fn map_or<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: U,
        f: impl Fn(&T) -> U + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, U>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: Clone + 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || {
                    RxReadOption::with(&source, |value| {
                        value.map(&f).unwrap_or_else(|| default.clone())
                    })
                },
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn unwrap_or<'owner, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: T,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, T>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        T: PartialEq + Clone + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed(
                move || {
                    RxReadOption::with(&source, |value| {
                        value.cloned().unwrap_or_else(|| default.clone())
                    })
                },
                error_handler,
            )
            .map(|computed| computed.into_rx())
    }

    fn map_or_else<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: impl Fn() -> U + 'owner,
        f: impl Fn(&T) -> U + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, U>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || RxReadOption::with(&source, |value| value.map(&f).unwrap_or_else(&default)),
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn and_then<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        f: impl Fn(&T) -> Option<U> + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, Option<U>>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || RxReadOption::with(&source, |value| value.and_then(&f)),
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn is_some_and<'owner, H>(
        &self,
        owner: OwnerAccess<'owner>,
        f: impl Fn(&T) -> bool + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed(
                move || RxReadOption::with(&source, |value| value.is_some_and(&f)),
                error_handler,
            )
            .map(|computed| computed.into_rx())
    }

    fn if_some_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<Option<U>> {
        RxReadOption::with_untracked(self, |value| value.map(f))
    }
}

impl<S, T> RxOptionExt<T> for S where S: RxReadOption<T> + Clone {}

/// Sources used by list rendering helpers.
pub trait ForLoopSource {
    type Item: Clone;
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

impl<T: Clone> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self)
    }
}

impl<T: Clone> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_deref().unwrap_or_default())
    }
}

impl<T: Clone> ForLoopSource for SilexResult<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Ok(items) => Ok(items.as_slice()),
            Err(error) => Err(error.clone()),
        }
    }
}
