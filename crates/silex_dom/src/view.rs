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
    CompletionToken, OwnedScope, RootScope, RuntimeInputs, Scope,
    error::handle_error,
    reactivity::ReactiveSource,
    traits::{RxData, RxValue},
};
use silex_core::{Rx, SilexError, SilexResult};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    ops::{Add, Deref, Div, Mul, Sub},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::JsValue;
use web_sys::Node;

pub use owner::RowUpdater;
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
    fn register(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>);
}

impl<'scope, F> EffectRegister<'scope> for F
where
    F: Fn(RuntimeInputs, Box<dyn FnMut() + 'scope>) + 'scope,
{
    fn register(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        self(inputs, callback);
    }
}

impl<'scope> EffectRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(RuntimeInputs, Box<dyn FnMut() + 'scope>) + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        self.inner.register(inputs, callback);
    }
}

type ValidationCallback<'scope> = Rc<dyn Fn(&RuntimeInputs) -> SilexResult<()> + 'scope>;

#[derive(Clone)]
struct ValidationRegistrar<'scope> {
    inner: ValidationCallback<'scope>,
}

impl<'scope> ValidationRegistrar<'scope> {
    fn new<F>(validate: F) -> Self
    where
        F: Fn(&RuntimeInputs) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(validate),
        }
    }

    fn call(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        (self.inner)(inputs)
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
struct OwnedScopeRegistrar<'scope> {
    inner: Rc<dyn OwnedScopeRegister<'scope> + 'scope>,
}

trait OwnedScopeRegister<'scope> {
    fn create(&self) -> OwnedScope<'scope>;
}

impl<'scope, F> OwnedScopeRegister<'scope> for F
where
    F: Fn() -> OwnedScope<'scope> + 'scope,
{
    fn create(&self) -> OwnedScope<'scope> {
        self()
    }
}

impl<'scope> OwnedScopeRegistrar<'scope> {
    fn new<F>(create: F) -> Self
    where
        F: Fn() -> OwnedScope<'scope> + 'scope,
    {
        Self {
            inner: Rc::new(create),
        }
    }

    fn call(&self) -> OwnedScope<'scope> {
        self.inner.create()
    }
}

#[derive(Clone)]
struct ActiveRegistrar<'scope> {
    inner: Rc<dyn Fn() -> bool + 'scope>,
}

impl<'scope> ActiveRegistrar<'scope> {
    fn new<F>(is_active: F) -> Self
    where
        F: Fn() -> bool + 'scope,
    {
        Self {
            inner: Rc::new(is_active),
        }
    }

    fn get(&self) -> bool {
        (self.inner)()
    }
}

#[derive(Clone)]
struct CompletionRegistrar<'scope> {
    inner: Rc<dyn CompletionRegister<'scope> + 'scope>,
}

trait CompletionRegister<'scope> {
    fn register(&self, callback: Box<dyn FnMut(JsValue) + 'scope>) -> CompletionToken<JsValue>;
}

impl<'scope, F> CompletionRegister<'scope> for F
where
    F: Fn(Box<dyn FnMut(JsValue) + 'scope>) -> CompletionToken<JsValue> + 'scope,
{
    fn register(&self, callback: Box<dyn FnMut(JsValue) + 'scope>) -> CompletionToken<JsValue> {
        self(callback)
    }
}

impl<'scope> CompletionRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(Box<dyn FnMut(JsValue) + 'scope>) -> CompletionToken<JsValue> + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(&self, callback: Box<dyn FnMut(JsValue) + 'scope>) -> CompletionToken<JsValue> {
        self.inner.register(callback)
    }
}

/// A host resource cancellation handle owned by a view scope.
///
/// The handle deliberately exposes no reactive capability. Cancellation is
/// idempotent and the owner retains a clone so dropping this value early does
/// not transfer lifecycle ownership away from the view.
type CancelAction<'scope> = Rc<RefCell<Option<Box<dyn FnOnce() + 'scope>>>>;
type ResourceGate = Rc<Cell<bool>>;

pub struct HostResourceHandle<'scope> {
    cancel: CancelAction<'scope>,
    active: ResourceGate,
}

impl<'scope> Clone for HostResourceHandle<'scope> {
    fn clone(&self) -> Self {
        Self {
            cancel: self.cancel.clone(),
            active: self.active.clone(),
        }
    }
}

impl<'scope> HostResourceHandle<'scope> {
    pub(crate) fn inactive() -> Self {
        Self {
            cancel: Rc::new(RefCell::new(None)),
            active: Rc::new(Cell::new(false)),
        }
    }

    fn with_gate<F>(active: ResourceGate, cancel: F) -> Self
    where
        F: FnOnce() + 'scope,
    {
        Self {
            cancel: Rc::new(RefCell::new(Some(Box::new(cancel)))),
            active,
        }
    }

    /// Cancel the host resource. Repeated calls are harmless.
    pub fn cancel(&self) {
        if !self.active.replace(false) {
            let _ = self.cancel.borrow_mut().take();
            return;
        }
        if let Some(cancel) = self.cancel.borrow_mut().take() {
            cancel();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }
}

impl Drop for HostResourceHandle<'_> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.cancel) == 1 {
            self.cancel();
        }
    }
}

/// A `'static` browser closure's only path back into a scoped view.
#[derive(Clone)]
pub(crate) struct HostCallback {
    destination: CompletionToken<JsValue>,
    gate: ResourceGate,
}

impl HostCallback {
    pub(crate) fn dispatch(&self, payload: JsValue) -> bool {
        if !self.gate.get() {
            return false;
        }
        self.destination.submit(payload)
    }

    pub(crate) fn finish(&self) {
        self.gate.set(false);
    }

    pub(crate) fn invalidate(&self) {
        self.gate.set(false);
    }
}

#[derive(Clone)]
pub struct ViewOwnerToken<'scope> {
    effect: EffectRegistrar<'scope>,
    validate: ValidationRegistrar<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope>,
    completion: CompletionRegistrar<'scope>,
    active: ActiveRegistrar<'scope>,
}

impl<'scope> ViewOwnerToken<'scope> {
    fn new(
        effect: EffectRegistrar<'scope>,
        validate: ValidationRegistrar<'scope>,
        cleanup: CleanupRegistrar<'scope>,
        owned_scope: OwnedScopeRegistrar<'scope>,
        completion: CompletionRegistrar<'scope>,
        active: ActiveRegistrar<'scope>,
    ) -> Self {
        Self {
            effect,
            validate,
            cleanup,
            owned_scope,
            completion,
            active,
        }
    }

    pub(crate) fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        self.effect.call(inputs, callback);
    }

    pub(crate) fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.validate.call(inputs)
    }

    pub(crate) fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.cleanup.call(cleanup);
    }

    pub(crate) fn host_callback<F>(&self, callback: F) -> HostCallback
    where
        F: FnMut(JsValue) + 'scope,
    {
        HostCallback {
            destination: self.completion.call(Box::new(callback)),
            gate: Rc::new(Cell::new(true)),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn host_resource_for_callback<F>(
        &self,
        callback: &HostCallback,
        cancel: F,
    ) -> HostResourceHandle<'scope>
    where
        F: FnOnce() + 'scope,
    {
        self.register_host_resource(callback.gate.clone(), cancel)
    }

    fn register_host_resource<F>(&self, gate: ResourceGate, cancel: F) -> HostResourceHandle<'scope>
    where
        F: FnOnce() + 'scope,
    {
        let resource = HostResourceHandle::with_gate(gate, cancel);
        if !self.is_active() {
            resource.cancel();
            return resource;
        }
        let owner_resource = resource.clone();
        self.on_cleanup(Box::new(move || owner_resource.cancel()));
        resource
    }
}

/// Mount-time capability shared by all view implementations.
pub trait ViewOwner<'scope> {
    fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>);
    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()>;
    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>);
    fn token(&self) -> ViewOwnerToken<'scope>;
    fn owned_scope(&self) -> OwnedScope<'scope>;
}

impl<'scope> ViewOwner<'scope> for ViewOwnerToken<'scope> {
    fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        self.effect_from(inputs, callback);
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.validate_inputs(inputs)
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        self.clone()
    }

    fn owned_scope(&self) -> OwnedScope<'scope> {
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

impl ViewOwner<'static> for RootViewOwner {
    fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'static>) {
        if let Err(error) = self.scope.try_effect_from(inputs, callback) {
            handle_error(error);
        }
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.scope.try_validate_inputs(inputs)
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'static>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'static> {
        let scope_for_effect = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        let scope_for_completion = self.scope.clone();
        let scope_for_active = self.scope.clone();
        let scope_for_validate = self.scope.clone();
        ViewOwnerToken::new(
            EffectRegistrar::new(move |inputs, callback| {
                if let Err(error) = scope_for_effect.try_effect_from(inputs, callback) {
                    handle_error(error);
                }
            }),
            ValidationRegistrar::new(move |inputs| scope_for_validate.try_validate_inputs(inputs)),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
            CompletionRegistrar::new(move |callback| scope_for_completion.completion(callback)),
            ActiveRegistrar::new(move || scope_for_active.is_active()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'static> {
        self.scope.owned_scope()
    }
}

/// Adapter for a lexical child scope.
#[derive(Clone, Copy)]
pub struct ScopedViewOwner<'scope> {
    scope: Scope<'scope>,
}

impl<'scope> ScopedViewOwner<'scope> {
    pub fn new(scope: Scope<'scope>) -> Self {
        Self { scope }
    }
}

impl<'scope> ViewOwner<'scope> for ScopedViewOwner<'scope> {
    fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        if let Err(error) = self.scope.try_effect_from(inputs, callback) {
            handle_error(error);
        }
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.scope.try_validate_inputs(inputs)
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        let scope_for_effect = self.scope;
        let scope_for_cleanup = self.scope;
        let scope_for_owned = self.scope;
        let scope_for_completion = self.scope;
        let scope_for_active = self.scope;
        let scope_for_validate = self.scope;
        ViewOwnerToken::new(
            EffectRegistrar::new(move |inputs, callback| {
                if let Err(error) = scope_for_effect.try_effect_from(inputs, callback) {
                    handle_error(error);
                }
            }),
            ValidationRegistrar::new(move |inputs| scope_for_validate.try_validate_inputs(inputs)),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
            CompletionRegistrar::new(move |callback| scope_for_completion.completion(callback)),
            ActiveRegistrar::new(move || scope_for_active.is_active()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'scope> {
        self.scope.owned_scope()
    }
}

pub(crate) struct OwnedViewOwner<'scope> {
    scope: Rc<OwnedScope<'scope>>,
}

impl<'scope> OwnedViewOwner<'scope> {
    pub(crate) fn new(scope: Rc<OwnedScope<'scope>>) -> Self {
        Self { scope }
    }
}

impl<'scope> ViewOwner<'scope> for OwnedViewOwner<'scope> {
    fn effect_from(&self, inputs: RuntimeInputs, callback: Box<dyn FnMut() + 'scope>) {
        if let Err(error) = self.scope.try_effect_from(inputs, callback) {
            handle_error(error);
        }
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.scope.try_validate_inputs(inputs)
    }

    fn on_cleanup(&self, cleanup: Box<dyn FnOnce() + 'scope>) {
        self.scope.on_cleanup(cleanup);
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        let scope_for_effect = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        let scope_for_completion = self.scope.clone();
        let scope_for_active = self.scope.clone();
        let scope_for_validate = self.scope.clone();
        ViewOwnerToken::new(
            EffectRegistrar::new(move |inputs, callback| {
                if let Err(error) = scope_for_effect.try_effect_from(inputs, callback) {
                    handle_error(error);
                }
            }),
            ValidationRegistrar::new(move |inputs| scope_for_validate.try_validate_inputs(inputs)),
            CleanupRegistrar::new(move |cleanup| scope_for_cleanup.on_cleanup(cleanup)),
            OwnedScopeRegistrar::new(move || scope_for_owned.child()),
            CompletionRegistrar::new(move |callback| scope_for_completion.completion(callback)),
            ActiveRegistrar::new(move || scope_for_active.is_active()),
        )
    }

    fn owned_scope(&self) -> OwnedScope<'scope> {
        self.scope.child()
    }
}

/// Apply attributes to a view while preserving their scope boundary.
pub trait ApplyAttributes<'scope> {
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute<'scope>>) {}
}

/// Component prop wrapper used by generated builders.
pub enum Prop<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T: RxValue> Prop<'a, T> {
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

impl<'scope, 'a, T> ApplyAttributes<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: ApplyAttributes<'scope>,
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        match self {
            Self::Owned(value) => value.apply_attributes(attrs),
            Self::Borrowed(value) => {
                let _ = (value, attrs);
            }
        }
    }
}

impl<'scope, 'a, T> View<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: View<'scope>,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        match self {
            Self::Owned(value) => value.mount(owner, parent, attrs),
            Self::Borrowed(value) => value.mount(owner, parent, attrs),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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

impl<'a, T> Prop<'a, T> {
    pub fn promote<'scope>(self, scope: &Scope<'scope>) -> Rx<'scope, T::Value>
    where
        'a: 'scope,
        T: ReactiveSource<'scope> + Clone,
        T::Value: Sized + RxData + 'scope,
    {
        scope.promote(self.into_owned())
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
pub trait View<'scope> {
    fn into_any(self) -> AnyView<'scope>
    where
        Self: Sized + 'scope,
    {
        AnyView::new(self)
    }

    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    );

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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
        impl<'scope> ApplyAttributes<'scope> for $ty {}

        impl<'scope> View<'scope> for $ty {
            fn mount(
                &self,
                _owner: &dyn ViewOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
            ) {
                mount_text_node(parent, self);
            }

            fn mount_owned(
                self,
                _owner: &dyn ViewOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
            ) where
                Self: Sized,
            {
                mount_text_node(parent, &self);
            }
        }
    };
}

impl_text_view!(String);

impl<'scope> ApplyAttributes<'scope> for &'scope str {}

impl<'scope> View<'scope> for &'scope str {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) {
        mount_text_node(parent, self);
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        mount_text_node(parent, self);
    }
}

impl<'scope> ApplyAttributes<'scope> for Cow<'scope, str> {}

impl<'scope> View<'scope> for Cow<'scope, str> {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) {
        mount_text_node(parent, self.as_ref());
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        mount_text_node(parent, self.as_ref());
    }
}

macro_rules! impl_primitive_view {
    ($($ty:ty),*) => {
        $(
            impl<'scope> ApplyAttributes<'scope> for $ty {}

            impl<'scope> View<'scope> for $ty {
                fn mount(
                    &self,
                    _owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                ) {
                    mount_text_node(parent, &self.to_string());
                }

                fn mount_owned(
                    self,
                    _owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
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

impl<'scope> ApplyAttributes<'scope> for () {}

impl<'scope> View<'scope> for () {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) {
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
    }
}

impl<'scope, F, V> ApplyAttributes<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
}

impl<'scope, F, V> View<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        self.clone().mount_owned(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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
pub fn mount_dynamic_view_universal<'scope>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    renderer: RenderThunk<'scope>,
) {
    mount_dynamic_view_universal_from(owner, parent, attrs, RuntimeInputs::new(), renderer);
}

pub(crate) fn mount_dynamic_view_universal_from<'scope>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    renderer: RenderThunk<'scope>,
) {
    if let Err(error) = owner.validate_inputs(&inputs) {
        handle_error(error);
        return;
    }
    let range = match DomRange::append(parent, "dyn") {
        Ok(range) => range,
        Err(error) => {
            handle_error(error);
            return;
        }
    };
    let render = RowRender::new(move |args: RowRenderArgs<'scope, ()>| {
        let RowRenderArgs {
            parent,
            attrs,
            owner: token,
            ..
        } = args;
        renderer.call(RenderArgs::new(parent, attrs, token));
    });
    let token = owner.token();
    let Some(row) = RowController::try_new(&token, range, render, inputs, attrs, (), 0, false)
    else {
        return;
    };
    let row_state = Rc::new(RefCell::new(Some(row)));
    let cleanup_state = row_state.clone();
    owner.on_cleanup(Box::new(move || {
        if let Some(mut row) = cleanup_state.borrow_mut().take() {
            row.dispose();
        }
    }));
}

/// Dynamic view mount with a persistent row owner keyed by the current key.
pub fn mount_dynamic_view_cached<'scope, K, KeyFn, RenderFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    key_fn: KeyFn,
    renderer: RenderFn,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    RenderFn: Fn(K, (Node, Vec<PendingAttribute<'scope>>)) + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            ..
        } = args;
        renderer(key, (parent, attrs));
    });
    mount_keyed_dynamic_view(owner, parent, attrs, inputs, key_fn, render);
}

pub fn mount_branch_cached<'scope, K, KeyFn, BranchFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    BranchFn: Fn(K) -> AnyView<'scope> + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            owner: token,
            ..
        } = args;
        branch_fn(key).mount_owned(&token, &parent, attrs);
    });
    mount_keyed_dynamic_view(owner, parent, attrs, inputs, key_fn, render);
}

struct BranchState<'scope, K> {
    range: DomRange,
    row: Option<RowController<'scope, K>>,
    key: Option<K>,
    render: RowRender<'scope, K>,
    attrs: Vec<PendingAttribute<'scope>>,
}

fn mount_keyed_dynamic_view<'scope, K, KeyFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    key_fn: KeyFn,
    render: RowRender<'scope, K>,
) where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
{
    if let Err(error) = owner.validate_inputs(&inputs) {
        handle_error(error);
        return;
    }
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
        let (row, range) = {
            let mut state = cleanup_state.borrow_mut();
            state.key = None;
            (state.row.take(), state.range.clone())
        };
        let panic = row
            .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
            .and_then(Result::err);
        range.remove();
        if let Some(panic) = panic {
            resume_unwind(panic);
        }
    }));

    let token = owner.token();
    owner.effect_from(
        inputs,
        Box::new(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let key = key_fn();
                let same_key = state
                    .borrow()
                    .key
                    .as_ref()
                    .is_some_and(|current| current == &key);
                if same_key {
                    let updated = state
                        .borrow_mut()
                        .row
                        .as_mut()
                        .is_some_and(|row| row.update(key, 0));
                    if !updated {
                        handle_error(SilexError::Javascript(
                            "dynamic row update was rejected".to_string(),
                        ));
                    }
                    return;
                }

                let (outer_range, render, attrs, old_row, old_key) = {
                    let mut state = state.borrow_mut();
                    (
                        state.range.clone(),
                        state.render.clone(),
                        state.attrs.clone(),
                        state.row.take(),
                        state.key.take(),
                    )
                };
                let Ok(row_range) = DomRange::before(&outer_range.end, "branch-row") else {
                    let mut state = state.borrow_mut();
                    state.row = old_row;
                    state.key = old_key;
                    return;
                };
                let Some(row) = RowController::try_new(
                    &token,
                    row_range,
                    render,
                    RuntimeInputs::new(),
                    attrs,
                    key.clone(),
                    0,
                    false,
                ) else {
                    let mut state = state.borrow_mut();
                    state.row = old_row;
                    state.key = old_key;
                    return;
                };

                let old_panic = old_row
                    .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
                    .and_then(Result::err);
                let mut state = state.borrow_mut();
                state.key = Some(key);
                state.row = Some(row);
                drop(state);
                if let Some(panic) = old_panic {
                    resume_unwind(panic);
                }
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
        }),
    );
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Option<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        if let Some(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Option<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        if let Some(value) = self {
            value.mount(owner, parent, attrs);
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        if let Some(value) = self {
            value.mount_owned(owner, parent, attrs);
        }
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Vec<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Vec<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>, const N: usize> ApplyAttributes<'scope>
    for [V; N]
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>, const N: usize> View<'scope> for [V; N] {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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

impl<'scope> ApplyAttributes<'scope> for ViewNil {}

impl<'scope> View<'scope> for ViewNil {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) {
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
    }
}

impl<'scope, H: ApplyAttributes<'scope>, T: ApplyAttributes<'scope>> ApplyAttributes<'scope>
    for ViewCons<H, T>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<'scope, H: View<'scope>, T: View<'scope>> View<'scope> for ViewCons<H, T> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        self.0.mount(owner, parent, attrs);
        self.1.mount(owner, parent, Vec::new());
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
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

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for SilexResult<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        if let Ok(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for SilexResult<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        match self {
            Ok(value) => value.mount(owner, parent, attrs),
            Err(error) => handle_error(error.clone()),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        match self {
            Ok(value) => value.mount_owned(owner, parent, attrs),
            Err(error) => handle_error(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostResourceHandle, RootViewOwner, ScopedViewOwner, ViewOwner};
    use silex_core::Runtime;
    use std::{cell::Cell, rc::Rc};
    use wasm_bindgen::JsValue;

    #[test]
    fn host_callback_is_gated_after_root_dispose() {
        let seen = Rc::new(Cell::new(0));
        let seen_in_callback = seen.clone();
        let mut runtime = Runtime::new();
        let mut root = runtime.run(|_| {});
        let owner = RootViewOwner::new(root.scope());
        let token = owner.token();
        let bridge = token.host_callback(move |_| {
            seen_in_callback.set(seen_in_callback.get() + 1);
        });
        assert!(bridge.dispatch(JsValue::UNDEFINED));

        assert_eq!(seen.get(), 1);
        root.dispose().expect("root cleanup should succeed");
        assert!(!bridge.dispatch(JsValue::UNDEFINED));
        let late = token.host_callback(|_| panic!("inactive owner callback ran"));
        assert!(!late.dispatch(JsValue::UNDEFINED));
        let resource = token.host_resource_for_callback(&late, || {});
        assert!(!resource.is_active());
        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn host_resource_cancellation_is_idempotent() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let handle = HostResourceHandle::with_gate(Rc::new(Cell::new(true)), move || {
            cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
        });
        let clone = handle.clone();

        handle.cancel();
        clone.cancel();
        drop(clone);
        drop(handle);

        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_keeps_resource_alive_when_returned_handle_is_dropped() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let mut runtime = Runtime::new();
        let mut root = runtime.run(|_| {});
        let owner = RootViewOwner::new(root.scope());
        let token = owner.token();
        let callback = token.host_callback(|_| {});
        let handle = token.host_resource_for_callback(&callback, move || {
            cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
        });
        drop(handle);
        assert_eq!(cancelled.get(), 0);
        root.dispose().expect("root cleanup should succeed");
        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_rejects_foreign_inputs_before_effect_registration() {
        let mut first = Runtime::new();
        let mut second = Runtime::new();
        let inputs = first.child(|scope| {
            let (source, _) = scope.signal(1i32);
            scope.promote(source).runtime_inputs()
        });
        let runs = Rc::new(Cell::new(0));
        let runs_for_effect = runs.clone();

        second.child(|scope| {
            let owner = ScopedViewOwner::new(scope);
            assert!(owner.validate_inputs(&inputs).is_err());
            owner.effect_from(
                inputs,
                Box::new(move || runs_for_effect.set(runs_for_effect.get() + 1)),
            );
        });

        assert_eq!(runs.get(), 0);
    }
}
