pub mod any;
pub mod list;
pub mod logic;
pub(crate) mod owner;
pub mod reactive;

pub use any::*;
pub use list::*;
pub use logic::*;
pub use reactive::*;

use crate::attribute::PendingAttribute;
use silex_core::{
    OwnedScope, RootScope, Scope,
    error::handle_error,
    traits::{IntoRx, IntoSignal, RxData, RxValue},
};
use silex_core::{Rx, SilexError, SilexResult};
use std::{
    borrow::Cow,
    cell::RefCell,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    ops::{Add, Deref, Div, Mul, Sub},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use web_sys::Node;

use owner::{DomRange, RowController, RowRender, RowRenderArgs};

/// Owner capabilities captured by a mounted view or attribute operation.
///
/// The token owns only registration functions. It never stores a borrowed
/// `ViewOwner`, so an effect cannot outlive the adapter stack frame used by
/// the original mount call.
#[derive(Clone)]
struct EffectRegistrar<'scope> {
    inner: Rc<dyn EffectRegister<'scope> + 'scope>,
}

trait EffectRegister<'scope> {
    fn register(&self, callback: Box<dyn FnMut() + 'scope>);
}

impl<'scope, F> EffectRegister<'scope> for F
where
    F: Fn(Box<dyn FnMut() + 'scope>) + 'scope,
{
    fn register(&self, callback: Box<dyn FnMut() + 'scope>) {
        self(callback);
    }
}

impl<'scope> EffectRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(Box<dyn FnMut() + 'scope>) + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(&self, callback: Box<dyn FnMut() + 'scope>) {
        self.inner.register(callback);
    }
}

#[derive(Clone)]
struct CleanupRegistrar<'scope> {
    inner: Rc<dyn CleanupRegister<'scope> + 'scope>,
}

trait CleanupRegister<'scope> {
    fn register(&self, cleanup: Box<dyn FnOnce() + 'scope>);
}

impl<'scope, F> CleanupRegister<'scope> for F
where
    F: Fn(Box<dyn FnOnce() + 'scope>) + 'scope,
{
    fn register(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self(cleanup);
    }
}

impl<'scope> CleanupRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(Box<dyn FnOnce() + 'scope>) + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.inner.register(cleanup);
    }
}

#[derive(Clone)]
struct OwnedScopeRegistrar<'scope, 'run> {
    inner: Rc<dyn OwnedScopeRegister<'scope, 'run> + 'scope>,
}

trait OwnedScopeRegister<'scope, 'run> {
    fn create(&self) -> OwnedScope<'scope, 'run>;
}

impl<'scope, 'run, F> OwnedScopeRegister<'scope, 'run> for F
where
    F: Fn() -> OwnedScope<'scope, 'run> + 'scope,
{
    fn create(&self) -> OwnedScope<'scope, 'run> {
        self()
    }
}

impl<'scope, 'run> OwnedScopeRegistrar<'scope, 'run> {
    fn new<F>(create: F) -> Self
    where
        F: Fn() -> OwnedScope<'scope, 'run> + 'scope,
    {
        Self {
            inner: Rc::new(create),
        }
    }

    fn call(&self) -> OwnedScope<'scope, 'run> {
        self.inner.create()
    }
}

#[derive(Clone)]
pub struct ViewOwnerToken<'scope, 'run> {
    effect: EffectRegistrar<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope, 'run>,
    marker: PhantomData<fn(&'run ())>,
}

impl<'scope, 'run> ViewOwnerToken<'scope, 'run> {
    fn new(
        effect: EffectRegistrar<'scope>,
        cleanup: CleanupRegistrar<'scope>,
        owned_scope: OwnedScopeRegistrar<'scope, 'run>,
    ) -> Self {
        Self {
            effect,
            cleanup,
            owned_scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn effect(&self, callback: Box<dyn FnMut() + 'scope>) {
        self.effect.call(callback);
    }

    pub(crate) fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.cleanup.call(cleanup);
    }
}

/// Mount-time capability shared by all view implementations.
pub trait ViewOwner<'scope, 'run> {
    fn effect(&self, callback: Box<dyn FnMut() + 'scope>);
    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>);
    fn token(&self) -> ViewOwnerToken<'scope, 'run>;
    fn owned_scope(&self) -> OwnedScope<'scope, 'run>;
}

impl<'scope, 'run> ViewOwner<'scope, 'run> for ViewOwnerToken<'scope, 'run> {
    fn effect(&self, callback: Box<dyn FnMut() + 'scope>) {
        self.effect(callback);
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope, 'run> {
        self.clone()
    }

    fn owned_scope(&self) -> OwnedScope<'scope, 'run> {
        self.owned_scope.call()
    }
}

/// Adapter for a long-lived root owner.
#[derive(Clone)]
pub struct RootViewOwner {
    scope: RootScope,
}

impl RootViewOwner {
    pub fn new(scope: RootScope) -> Self {
        Self { scope }
    }
}

impl ViewOwner<'static, 'static> for RootViewOwner {
    fn effect(&self, callback: Box<dyn FnMut() + 'static>) {
        let _ = self.scope.effect(callback);
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'static>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'static, 'static> {
        let scope_for_effect = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        ViewOwnerToken::new(
            EffectRegistrar::new(move |callback| {
                let _ = scope_for_effect.effect(callback);
            }),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'static, 'static> {
        self.scope.owned_scope()
    }
}

/// Adapter for a lexical child scope.
#[derive(Clone, Copy)]
pub struct ScopedViewOwner<'scope, 'run> {
    scope: Scope<'scope, 'run>,
}

impl<'scope, 'run> ScopedViewOwner<'scope, 'run> {
    pub fn new(scope: Scope<'scope, 'run>) -> Self {
        Self { scope }
    }
}

impl<'scope, 'run> ViewOwner<'scope, 'run> for ScopedViewOwner<'scope, 'run> {
    fn effect(&self, callback: Box<dyn FnMut() + 'scope>) {
        let mut callback = callback;
        let _ = self.scope.effect(move |_: Option<()>| {
            callback();
        });
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope, 'run> {
        let scope_for_effect = self.scope;
        let scope_for_cleanup = self.scope;
        let scope_for_owned = self.scope;
        ViewOwnerToken::new(
            EffectRegistrar::new(move |callback| {
                let mut callback = callback;
                let _ = scope_for_effect.effect(move |_: Option<()>| {
                    callback();
                });
            }),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'scope, 'run> {
        self.scope.owned_scope()
    }
}

pub(crate) struct OwnedViewOwner<'scope, 'run> {
    scope: Rc<OwnedScope<'scope, 'run>>,
}

impl<'scope, 'run> OwnedViewOwner<'scope, 'run> {
    pub(crate) fn new(scope: Rc<OwnedScope<'scope, 'run>>) -> Self {
        Self { scope }
    }
}

impl<'scope, 'run> ViewOwner<'scope, 'run> for OwnedViewOwner<'scope, 'run>
where
    'run: 'scope,
{
    fn effect(&self, callback: Box<dyn FnMut() + 'scope>) {
        self.scope.effect(callback);
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope, 'run> {
        let scope_for_effect = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        ViewOwnerToken::new(
            EffectRegistrar::new(move |callback| scope_for_effect.effect(callback)),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.child()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'scope, 'run> {
        self.scope.child()
    }
}

/// Apply attributes to a view while preserving their scope boundary.
pub trait ApplyAttributes<'scope, 'run> {
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute<'scope, 'run>>) {}
}

/// Component prop wrapper used by generated builders.
pub enum Prop<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T> Prop<'a, T> {
    pub fn new_borrowed(value: &'a T) -> Self {
        Self::Borrowed(value)
    }

    pub fn new_owned(value: T) -> Self {
        Self::Owned(value)
    }

    pub fn new(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, T: Clone> Prop<'a, T> {
    pub fn into_owned(self) -> T {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value.clone(),
        }
    }
}

impl<'a, T: Clone> Clone for Prop<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(value) => Self::Owned(value.clone()),
            Self::Borrowed(value) => Self::Owned((*value).clone()),
        }
    }
}

impl<'a, T: Copy> Copy for Prop<'a, T> {}

pub trait PropInto<T> {
    fn prop_into(self) -> T;
}

impl<T> PropInto<T> for T {
    fn prop_into(self) -> T {
        self
    }
}

impl<'a, T: Clone> PropInto<T> for Prop<'a, T> {
    fn prop_into(self) -> T {
        self.into_owned()
    }
}

impl<'scope, 'run, 'a, T> ApplyAttributes<'scope, 'run> for Prop<'a, T>
where
    'a: 'scope,
    T: ApplyAttributes<'scope, 'run>,
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        match self {
            Self::Owned(value) => value.apply_attributes(attrs),
            Self::Borrowed(value) => {
                let _ = (value, attrs);
            }
        }
    }
}

impl<'scope, 'run, 'a, T> View<'scope, 'run> for Prop<'a, T>
where
    'a: 'scope,
    T: View<'scope, 'run>,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        match self {
            Self::Owned(value) => value.mount(owner, parent, attrs),
            Self::Borrowed(value) => value.mount(owner, parent, attrs),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        match self {
            Self::Owned(value) => value.mount_owned(owner, parent, attrs),
            Self::Borrowed(value) => value.mount(owner, parent, attrs),
        }
    }
}

impl<'a, T: RxValue> RxValue for Prop<'a, T> {
    type Value = T::Value;
}

impl<'scope, 'run, 'a, T> IntoRx<'scope, 'run> for Prop<'a, T>
where
    'a: 'scope,
    T: IntoRx<'scope, 'run> + Clone,
    T::Value: Sized + RxData,
{
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Self::Value> {
        self.into_owned().into_rx(scope)
    }

    fn is_constant(&self) -> bool {
        match self {
            Self::Owned(value) => value.is_constant(),
            Self::Borrowed(value) => value.is_constant(),
        }
    }
}

impl<'scope, 'run, 'a, T> IntoSignal<'scope, 'run> for Prop<'a, T>
where
    'a: 'scope,
    T: IntoSignal<'scope, 'run> + Clone,
    T::Value: Sized + RxData,
{
    fn into_signal(
        self,
        scope: &Scope<'scope, 'run>,
    ) -> silex_core::Signal<'scope, 'run, Self::Value> {
        self.into_owned().into_signal(scope)
    }
}

impl<'a, T> Deref for Prop<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value,
        }
    }
}

impl<'a, T: Debug> Debug for Prop<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

impl<'a, T: Display> Display for Prop<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

macro_rules! impl_forward_binop_copy {
    ($trait:ident, $method:ident) => {
        impl<'a, T, Rhs> $trait<Rhs> for Prop<'a, T>
        where
            T: Copy + $trait<Rhs>,
        {
            type Output = <T as $trait<Rhs>>::Output;

            fn $method(self, rhs: Rhs) -> Self::Output {
                self.deref().$method(rhs)
            }
        }
    };
}

impl_forward_binop_copy!(Add, add);
impl_forward_binop_copy!(Sub, sub);
impl_forward_binop_copy!(Mul, mul);
impl_forward_binop_copy!(Div, div);

/// View conversion and mounting contract.
pub trait View<'scope, 'run> {
    fn into_any(self) -> AnyView<'scope, 'run>
    where
        Self: Sized + 'scope,
    {
        AnyView::new(self)
    }

    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    );

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized;
}

pub fn mount_text_node(parent: &Node, text: &str) {
    let document = crate::document();
    let node = document.create_text_node(text);
    if let Err(error) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(error);
    }
}

macro_rules! impl_text_view {
    ($ty:ty) => {
        impl<'scope, 'run> ApplyAttributes<'scope, 'run> for $ty {}

        impl<'scope, 'run> View<'scope, 'run> for $ty {
            fn mount(
                &self,
                _owner: &dyn ViewOwner<'scope, 'run>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope, 'run>>,
            ) {
                mount_text_node(parent, self);
            }

            fn mount_owned(
                self,
                _owner: &dyn ViewOwner<'scope, 'run>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope, 'run>>,
            ) where
                Self: Sized,
            {
                mount_text_node(parent, &self);
            }
        }
    };
}

impl_text_view!(String);

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for &'scope str {}

impl<'scope, 'run> View<'scope, 'run> for &'scope str {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_text_node(parent, self);
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        mount_text_node(parent, self);
    }
}

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for Cow<'scope, str> {}

impl<'scope, 'run> View<'scope, 'run> for Cow<'scope, str> {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_text_node(parent, self.as_ref());
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        mount_text_node(parent, self.as_ref());
    }
}

macro_rules! impl_primitive_view {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'run> ApplyAttributes<'scope, 'run> for $ty {}

            impl<'scope, 'run> View<'scope, 'run> for $ty {
                fn mount(
                    &self,
                    _owner: &dyn ViewOwner<'scope, 'run>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope, 'run>>,
                ) {
                    mount_text_node(parent, &self.to_string());
                }

                fn mount_owned(
                    self,
                    _owner: &dyn ViewOwner<'scope, 'run>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope, 'run>>,
                ) where
                    Self: Sized,
                {
                    mount_text_node(parent, &self.to_string());
                }
            }
        )*
    };
}

impl_primitive_view!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64, bool, char
);

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for () {}

impl<'scope, 'run> View<'scope, 'run> for () {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
    }
}

impl<'scope, 'run, F, V> ApplyAttributes<'scope, 'run> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope, 'run> + 'scope,
{
}

impl<'scope, 'run, F, V> View<'scope, 'run> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope, 'run> + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        self.clone().mount_owned(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        mount_dynamic_view_universal(
            owner,
            parent,
            attrs,
            RenderThunk::new(move |args| {
                let RenderArgs {
                    parent,
                    attrs,
                    owner: token,
                } = args;
                let view = self();
                view.mount_owned(&token, &parent, attrs);
            }),
        );
    }
}

/// Shared dynamic-view mount kernel.
pub fn mount_dynamic_view_universal<'scope, 'run>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
    renderer: RenderThunk<'scope, 'run>,
) {
    let range = match DomRange::append(parent, "dyn") {
        Ok(range) => range,
        Err(error) => {
            handle_error(error);
            return;
        }
    };
    let render = RowRender::new(move |args: RowRenderArgs<'scope, 'run, ()>| {
        let RowRenderArgs {
            parent,
            attrs,
            owner: token,
            ..
        } = args;
        renderer.call(RenderArgs::new(parent, attrs, token));
    });
    let token = owner.token();
    let row = RowController::new(&token, range, render, attrs, (), 0);
    let row_state = Rc::new(RefCell::new(Some(row)));
    let cleanup_state = row_state.clone();
    owner.on_cleanup(Box::new(move || {
        if let Some(mut row) = cleanup_state.borrow_mut().take() {
            row.dispose();
        }
    }));
}

/// Dynamic view mount with a persistent row owner keyed by the current key.
pub fn mount_dynamic_view_cached<'scope, 'run, K, KeyFn, RenderFn>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
    key_fn: KeyFn,
    renderer: RenderFn,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    RenderFn: Fn(K, (Node, Vec<PendingAttribute<'scope, 'run>>)) + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, 'run, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            ..
        } = args;
        renderer(key, (parent, attrs));
    });
    mount_keyed_dynamic_view(owner, parent, attrs, key_fn, render);
}

pub fn mount_branch_cached<'scope, 'run, K, KeyFn, BranchFn>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    BranchFn: Fn(K) -> AnyView<'scope, 'run> + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, 'run, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            owner: token,
            ..
        } = args;
        branch_fn(key).mount_owned(&token, &parent, attrs);
    });
    mount_keyed_dynamic_view(owner, parent, attrs, key_fn, render);
}

struct BranchState<'scope, 'run, K> {
    range: DomRange,
    row: Option<RowController<'scope, 'run, K>>,
    key: Option<K>,
    render: RowRender<'scope, 'run, K>,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
}

fn mount_keyed_dynamic_view<'scope, 'run, K, KeyFn>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
    key_fn: KeyFn,
    render: RowRender<'scope, 'run, K>,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
{
    let range = match DomRange::append(parent, "branch") {
        Ok(range) => range,
        Err(error) => {
            handle_error(error);
            return;
        }
    };
    let state = Rc::new(RefCell::new(BranchState {
        range,
        row: None,
        key: None,
        render,
        attrs,
    }));
    let cleanup_state = state.clone();
    owner.on_cleanup(Box::new(move || {
        let mut state = cleanup_state.borrow_mut();
        if let Some(mut row) = state.row.take() {
            row.dispose();
        } else {
            state.range.remove();
        }
        state.key = None;
    }));

    let token = owner.token();
    owner.effect(Box::new(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let key = key_fn();
            let mut state = state.borrow_mut();
            let same_key = state.key.as_ref().is_some_and(|current| current == &key);
            if same_key {
                if let Some(row) = state.row.as_mut() {
                    row.update(key, 0);
                }
                return;
            }

            state.key = None;
            if let Some(mut row) = state.row.take() {
                row.dispose_keep_range();
            }
            let row = RowController::new(
                &token,
                state.range.clone(),
                state.render.clone(),
                state.attrs.clone(),
                key.clone(),
                0,
            );
            state.key = Some(key);
            state.row = Some(row);
        }));
        if let Err(panic) = result {
            let message = if let Some(value) = panic.downcast_ref::<&str>() {
                format!("Panic in Dynamic Branch: {value}")
            } else if let Some(value) = panic.downcast_ref::<String>() {
                format!("Panic in Dynamic Branch: {value}")
            } else {
                "Panic in Dynamic Branch: unknown panic".to_string()
            };
            handle_error(SilexError::Javascript(message));
        }
    }));
}

impl<'scope, 'run, V: View<'scope, 'run> + ApplyAttributes<'scope, 'run>>
    ApplyAttributes<'scope, 'run> for Option<V>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        if let Some(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run>> View<'scope, 'run> for Option<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        if let Some(value) = self {
            value.mount(owner, parent, attrs);
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        if let Some(value) = self {
            value.mount_owned(owner, parent, attrs);
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run> + ApplyAttributes<'scope, 'run>>
    ApplyAttributes<'scope, 'run> for Vec<V>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run>> View<'scope, 'run> for Vec<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        for (index, value) in self.iter().enumerate() {
            value.mount(
                owner,
                parent,
                if index == 0 {
                    attrs.clone()
                } else {
                    Vec::new()
                },
            );
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        for (index, value) in self.into_iter().enumerate() {
            value.mount_owned(
                owner,
                parent,
                if index == 0 {
                    attrs.clone()
                } else {
                    Vec::new()
                },
            );
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run> + ApplyAttributes<'scope, 'run>, const N: usize>
    ApplyAttributes<'scope, 'run> for [V; N]
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run>, const N: usize> View<'scope, 'run> for [V; N] {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        for (index, value) in self.iter().enumerate() {
            value.mount(
                owner,
                parent,
                if index == 0 {
                    attrs.clone()
                } else {
                    Vec::new()
                },
            );
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        for (index, value) in self.into_iter().enumerate() {
            value.mount_owned(
                owner,
                parent,
                if index == 0 {
                    attrs.clone()
                } else {
                    Vec::new()
                },
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropMissing;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropFixed;

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for ViewNil {}

impl<'scope, 'run> View<'scope, 'run> for ViewNil {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope, 'run>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
    }
}

impl<'scope, 'run, H: ApplyAttributes<'scope, 'run>, T: ApplyAttributes<'scope, 'run>>
    ApplyAttributes<'scope, 'run> for ViewCons<H, T>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<'scope, 'run, H: View<'scope, 'run>, T: View<'scope, 'run>> View<'scope, 'run>
    for ViewCons<H, T>
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        self.0.mount(owner, parent, attrs);
        self.1.mount(owner, parent, Vec::new());
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        let ViewCons(head, tail) = self;
        head.mount_owned(owner, parent, attrs);
        tail.mount_owned(owner, parent, Vec::new());
    }
}

#[macro_export]
macro_rules! view_chain {
    () => {
        $crate::view::ViewNil
    };
    ($head:expr $(,)?) => {
        $crate::view::ViewCons($head, $crate::view::ViewNil)
    };
    ($head:expr, $($tail:expr),+ $(,)?) => {
        $crate::view::ViewCons($head, $crate::view_chain!($($tail),+))
    };
}

impl<'scope, 'run, V: View<'scope, 'run> + ApplyAttributes<'scope, 'run>>
    ApplyAttributes<'scope, 'run> for SilexResult<V>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        if let Ok(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, 'run, V: View<'scope, 'run>> View<'scope, 'run> for SilexResult<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        match self {
            Ok(value) => value.mount(owner, parent, attrs),
            Err(error) => handle_error(error.clone()),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        match self {
            Ok(value) => value.mount_owned(owner, parent, attrs),
            Err(error) => handle_error(error),
        }
    }
}
