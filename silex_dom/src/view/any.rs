use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::View;
use silex_vtable::any_box::AnyBox;
use silex_vtable::func_ptr::FuncPtr;
use silex_vtable::thunk::FactoryBox;
use std::marker::PhantomData;
use std::mem;
use web_sys::Node;

// --- Manual VTable & SOO Infrastucture using silex_vtable ---

pub(crate) struct AnyViewVTable {
    pub mount: FuncPtr<unsafe fn(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>)>,
    pub mount_ref: FuncPtr<unsafe fn(data: *const u8, parent: &Node, attrs: Vec<PendingAttribute>)>,
    pub apply_attributes: FuncPtr<unsafe fn(data: *mut u8, attrs: Vec<PendingAttribute>)>,
    pub drop: FuncPtr<unsafe fn(data: *mut u8)>,
}

pub struct AnyViewBox {
    inner: AnyBox<AnyViewVTable>,
}

pub(crate) struct SharedViewVTable {
    pub any: AnyViewVTable,
    pub clone: FuncPtr<unsafe fn(data: *const u8) -> SharedViewBox>,
}

pub struct SharedViewBox {
    inner: AnyBox<SharedViewVTable>,
}

impl AnyViewBox {
    #[inline(always)]
    pub fn new<V: View + 'static>(view: V) -> Self {
        struct VGen<V>(PhantomData<V>);
        impl<V: View + 'static> VGen<V> {
            const STACK: AnyViewVTable = AnyViewVTable {
                mount: FuncPtr::new(mount_stack::<V>),
                mount_ref: FuncPtr::new(mount_ref_stack::<V>),
                apply_attributes: FuncPtr::new(apply_stack::<V>),
                drop: FuncPtr::new(drop_stack::<V>),
            };
            const HEAP: AnyViewVTable = AnyViewVTable {
                mount: FuncPtr::new(mount_heap::<V>),
                mount_ref: FuncPtr::new(mount_ref_heap::<V>),
                apply_attributes: FuncPtr::new(apply_heap::<V>),
                drop: FuncPtr::new(drop_heap::<V>),
            };
        }
        Self {
            inner: AnyBox::new(view, &VGen::<V>::STACK, &VGen::<V>::HEAP),
        }
    }

    pub fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let mut this = mem::ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe {
            (vtable.mount.as_fn())(data_ptr, parent, attrs);
        }
    }

    pub fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.inner.vtable.mount_ref.as_fn())(self.inner.as_ptr(), parent, attrs);
        }
    }

    pub fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.inner.vtable.apply_attributes.as_fn())(self.inner.as_mut_ptr(), attrs);
        }
    }
}

impl Drop for AnyViewBox {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

impl SharedViewBox {
    #[inline(always)]
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        struct VGen<V>(PhantomData<V>);
        impl<V: View + Clone + 'static> VGen<V> {
            const STACK: SharedViewVTable = SharedViewVTable {
                any: AnyViewVTable {
                    mount: FuncPtr::new(mount_stack::<V>),
                    mount_ref: FuncPtr::new(mount_ref_stack::<V>),
                    apply_attributes: FuncPtr::new(apply_stack::<V>),
                    drop: FuncPtr::new(drop_stack::<V>),
                },
                clone: FuncPtr::new(clone_stack::<V>),
            };
            const HEAP: SharedViewVTable = SharedViewVTable {
                any: AnyViewVTable {
                    mount: FuncPtr::new(mount_heap::<V>),
                    mount_ref: FuncPtr::new(mount_ref_heap::<V>),
                    apply_attributes: FuncPtr::new(apply_heap::<V>),
                    drop: FuncPtr::new(drop_heap::<V>),
                },
                clone: FuncPtr::new(clone_heap::<V>),
            };
        }
        Self {
            inner: AnyBox::new(view, &VGen::<V>::STACK, &VGen::<V>::HEAP),
        }
    }

    pub fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let mut this = mem::ManuallyDrop::new(self);
        let vtable = this.inner.vtable;
        let data_ptr = this.inner.as_mut_ptr();
        unsafe {
            (vtable.any.mount.as_fn())(data_ptr, parent, attrs);
        }
    }

    pub fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.inner.vtable.any.mount_ref.as_fn())(self.inner.as_ptr(), parent, attrs);
        }
    }

    pub fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.inner.vtable.any.apply_attributes.as_fn())(self.inner.as_mut_ptr(), attrs);
        }
    }

    pub fn into_any(self) -> AnyViewBox {
        let this = mem::ManuallyDrop::new(self);
        AnyViewBox {
            inner: AnyBox {
                data: this.inner.data,
                vtable: &this.inner.vtable.any,
            },
        }
    }
}

impl Clone for SharedViewBox {
    fn clone(&self) -> Self {
        unsafe { (self.inner.vtable.clone.as_fn())(self.inner.as_ptr()) }
    }
}

impl Drop for SharedViewBox {
    fn drop(&mut self) {
        unsafe {
            (self.inner.vtable.any.drop.as_fn())(self.inner.as_mut_ptr());
        }
    }
}

// --- VTable Glue Functions ---

unsafe fn mount_stack<V: View>(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        // We need to move the view out of the raw buffer.
        // For stack-allocated data, we can't easily use mem::replace without Default.
        // But we can use replace with MaybeUninit to safely take the value.
        let view_ptr = data as *mut mem::MaybeUninit<V>;
        let view = mem::replace(&mut *view_ptr, mem::MaybeUninit::uninit()).assume_init();
        view.mount(parent, attrs);
    }
}

unsafe fn mount_ref_stack<V: View>(data: *const u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        let view = &*(data as *const V);
        view.mount_ref(parent, attrs);
    }
}

unsafe fn apply_stack<V: View>(data: *mut u8, attrs: Vec<PendingAttribute>) {
    unsafe {
        let view = &mut *(data as *mut V);
        view.apply_attributes(attrs);
    }
}

unsafe fn drop_stack<V: View>(data: *mut u8) {
    unsafe {
        std::ptr::drop_in_place(data as *mut V);
    }
}

unsafe fn mount_heap<V: View>(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        // For heap data, data contains a *mut V.
        // We replace it with null to "take" the pointer safely.
        let ptr_ref = &mut *(data as *mut *mut V);
        let ptr = mem::replace(ptr_ref, std::ptr::null_mut());
        if !ptr.is_null() {
            let view = *Box::from_raw(ptr);
            view.mount(parent, attrs);
        }
    }
}

unsafe fn mount_ref_heap<V: View>(data: *const u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        let ptr = *(data as *const *mut V);
        let view = &*ptr;
        view.mount_ref(parent, attrs);
    }
}

unsafe fn apply_heap<V: View>(data: *mut u8, attrs: Vec<PendingAttribute>) {
    unsafe {
        let ptr = *(data as *mut *mut V);
        let view = &mut *ptr;
        view.apply_attributes(attrs);
    }
}

unsafe fn drop_heap<V: View>(data: *mut u8) {
    unsafe {
        let ptr_ref = &mut *(data as *mut *mut V);
        let ptr = mem::replace(ptr_ref, std::ptr::null_mut());
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr);
        }
    }
}

unsafe fn clone_stack<V: View + Clone + 'static>(data: *const u8) -> SharedViewBox {
    unsafe {
        let view = &*(data as *const V);
        SharedViewBox::new(view.clone())
    }
}

unsafe fn clone_heap<V: View + Clone + 'static>(data: *const u8) -> SharedViewBox {
    unsafe {
        let ptr = *(data as *const *mut V);
        let view = &*ptr;
        SharedViewBox::new(view.clone())
    }
}

// --- Thunks ---

pub type ViewThunk = FactoryBox<AnyView>;
pub type RenderThunk = silex_vtable::thunk::ThunkBox<(Node, Vec<PendingAttribute>), ()>;

/// 优化的 SharedView，专用于需要重复使用或需要 Children 的组件边界
#[derive(Default)]
pub enum SharedView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<SharedView>),
    Boxed(SharedViewBox, Vec<PendingAttribute>),
}

/// 优化的 AnyView，作为所有视图类型擦除的终点（不要求 Clone）
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Boxed(AnyViewBox, Vec<PendingAttribute>),
    FromShared(SharedView),
}

impl SharedView {
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        SharedView::Boxed(SharedViewBox::new(view), Vec::new())
    }
}

impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        AnyView::Boxed(AnyViewBox::new(view), Vec::new())
    }
}

impl View for SharedView {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(s) => s.mount(parent, attrs),
            SharedView::Element(el) => el.mount(parent, attrs),
            SharedView::List(list) => {
                for (i, child) in list.into_iter().enumerate() {
                    child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            SharedView::Boxed(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount(
                    parent,
                    crate::attribute::consolidate_attributes(inner_attrs),
                );
            }
        }
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(s) => s.mount_ref(parent, attrs),
            SharedView::Element(el) => el.mount_ref(parent, attrs),
            SharedView::List(list) => {
                for (i, child) in list.iter().enumerate() {
                    child.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            SharedView::Boxed(b, inner_attrs) => {
                let mut temp = inner_attrs.clone();
                temp.extend(attrs);
                b.mount_ref(parent, crate::attribute::consolidate_attributes(temp));
            }
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(_) => {}
            SharedView::Element(el) => el.apply_attributes(attrs),
            SharedView::List(list) => {
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            SharedView::Boxed(b, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
                b.apply_attributes(inner_attrs.clone());
            }
        }
    }

    fn into_any(self) -> AnyView {
        AnyView::FromShared(self)
    }

    fn into_shared(self) -> SharedView {
        self
    }
}

impl View for AnyView {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount(parent, attrs),
            AnyView::Element(el) => el.mount(parent, attrs),
            AnyView::List(list) => {
                for (i, child) in list.into_iter().enumerate() {
                    child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            AnyView::Boxed(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount(
                    parent,
                    crate::attribute::consolidate_attributes(inner_attrs),
                );
            }
            AnyView::FromShared(s) => s.mount(parent, attrs),
        }
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount_ref(parent, attrs),
            AnyView::Element(el) => el.mount_ref(parent, attrs),
            AnyView::List(list) => {
                for (i, child) in list.iter().enumerate() {
                    child.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            AnyView::Boxed(b, inner_attrs) => {
                let mut temp = inner_attrs.clone();
                temp.extend(attrs);
                b.mount_ref(parent, crate::attribute::consolidate_attributes(temp));
            }
            AnyView::FromShared(s) => s.mount_ref(parent, attrs),
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(_) => {}
            AnyView::Element(el) => el.apply_attributes(attrs),
            AnyView::List(list) => {
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            AnyView::Boxed(b, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
                b.apply_attributes(inner_attrs.clone());
            }
            AnyView::FromShared(s) => s.apply_attributes(attrs),
        }
    }

    fn into_any(self) -> AnyView {
        self
    }
}

impl Clone for SharedView {
    fn clone(&self) -> Self {
        match self {
            SharedView::Empty => SharedView::Empty,
            SharedView::Text(s) => SharedView::Text(s.clone()),
            SharedView::Element(el) => SharedView::Element(el.clone()),
            SharedView::List(list) => SharedView::List(list.clone()),
            SharedView::Boxed(b, attrs) => SharedView::Boxed(b.clone(), attrs.clone()),
        }
    }
}

impl PartialEq for SharedView {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SharedView::Empty, SharedView::Empty) => true,
            (SharedView::Text(a), SharedView::Text(b)) => a == b,
            (SharedView::Element(a), SharedView::Element(b)) => a == b,
            (SharedView::List(a), SharedView::List(b)) => a == b,
            _ => false,
        }
    }
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "AnyView(Empty)"),
            Self::Text(arg0) => f.debug_tuple("AnyView(Text)").field(arg0).finish(),
            Self::Element(_) => write!(f, "AnyView(Element)"),
            Self::List(l) => f.debug_tuple("AnyView(List)").field(&l.len()).finish(),
            Self::Boxed(_, _) => write!(f, "AnyView(Boxed)"),
            Self::FromShared(s) => f.debug_tuple("AnyView(FromShared)").field(s).finish(),
        }
    }
}

impl std::fmt::Debug for SharedView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "SharedView(Empty)"),
            Self::Text(arg0) => f.debug_tuple("SharedView(Text)").field(arg0).finish(),
            Self::Element(_) => write!(f, "SharedView(Element)"),
            Self::List(l) => f.debug_tuple("SharedView(List)").field(&l.len()).finish(),
            Self::Boxed(_, _) => write!(f, "SharedView(Boxed)"),
        }
    }
}

/// 标准子组件类型，即受 Clone 保护的擦除 SharedView
pub type Children = SharedView;

/// 片段，用于容纳多个不同类型的子组件
#[derive(Default, Clone)]
pub struct Fragment(pub Vec<SharedView>);

impl Fragment {
    pub fn new(children: Vec<SharedView>) -> Self {
        Self(children)
    }
}

impl View for Fragment {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, child) in self.0.into_iter().enumerate() {
            child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, child) in self.0.iter().enumerate() {
            child.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for child in &mut self.0 {
            child.apply_attributes(attrs.clone());
        }
    }

    fn into_any(self) -> AnyView {
        AnyView::FromShared(SharedView::List(self.0))
    }

    fn into_shared(self) -> SharedView {
        SharedView::List(self.0)
    }
}

// --- From Implementations for Type Erasure ---

impl From<Element> for AnyView {
    fn from(v: Element) -> Self {
        AnyView::Element(v)
    }
}
impl From<String> for AnyView {
    fn from(v: String) -> Self {
        AnyView::Text(v)
    }
}
impl From<&str> for AnyView {
    fn from(v: &str) -> Self {
        AnyView::Text(v.to_string())
    }
}
impl From<()> for AnyView {
    fn from(_: ()) -> Self {
        AnyView::Empty
    }
}

impl From<Element> for SharedView {
    fn from(v: Element) -> Self {
        SharedView::Element(v)
    }
}
impl From<String> for SharedView {
    fn from(v: String) -> Self {
        SharedView::Text(v)
    }
}
impl From<&str> for SharedView {
    fn from(v: &str) -> Self {
        SharedView::Text(v.to_string())
    }
}
impl From<()> for SharedView {
    fn from(_: ()) -> Self {
        SharedView::Empty
    }
}

macro_rules! impl_from_primitive {
    ($($t:ty),*) => {
        $(
            impl From<$t> for AnyView {
                fn from(v: $t) -> Self {
                    AnyView::Text(v.to_string())
                }
            }

            impl From<$t> for SharedView {
                fn from(v: $t) -> Self {
                    SharedView::Text(v.to_string())
                }
            }
        )*
    };
}
impl_from_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<V: View + 'static> From<Vec<V>> for AnyView {
    fn from(v: Vec<V>) -> Self {
        AnyView::List(v.into_iter().map(|item| item.into_any()).collect())
    }
}
impl<V: View + Clone + 'static> From<Vec<V>> for SharedView {
    fn from(v: Vec<V>) -> Self {
        SharedView::List(v.into_iter().map(|item| item.into_shared()).collect())
    }
}

impl<V: View + 'static> From<Option<V>> for AnyView {
    fn from(v: Option<V>) -> Self {
        match v {
            Some(val) => AnyView::new(val),
            None => AnyView::Empty,
        }
    }
}
impl<V: View + Clone + 'static> From<Option<V>> for SharedView {
    fn from(v: Option<V>) -> Self {
        match v {
            Some(val) => SharedView::new(val),
            None => SharedView::Empty,
        }
    }
}

// --- Recursive View Chain Erasure ---

impl From<crate::view::ViewNil> for AnyView {
    fn from(_: crate::view::ViewNil) -> Self {
        AnyView::Empty
    }
}

impl From<crate::view::ViewNil> for SharedView {
    fn from(_: crate::view::ViewNil) -> Self {
        SharedView::Empty
    }
}

impl<H, T> From<crate::view::ViewCons<H, T>> for AnyView
where
    H: View + 'static,
    T: View + 'static,
{
    fn from(v: crate::view::ViewCons<H, T>) -> Self {
        AnyView::new(v)
    }
}

impl<H, T> From<crate::view::ViewCons<H, T>> for SharedView
where
    H: View + Clone + 'static,
    T: View + Clone + 'static,
{
    fn from(v: crate::view::ViewCons<H, T>) -> Self {
        SharedView::new(v)
    }
}

/// 一个辅助宏，用于简化从 `match` 表达式返回 `SharedView` 的操作。
///
/// 它会自动对每个分支的结果调用 `.into_shared()`，从而允许不同类型的 View 在同一个 `match` 块中返回。
///
/// # 示例
///
/// ```rust, ignore
/// view_match!(route, {
///     AppRoute::Home => HomePage::new(),
///     AppRoute::Basics => "Basics Page",
///     AppRoute::NotFound => (),
/// })
/// ```
#[macro_export]
macro_rules! view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $crate::view::View::into_shared($val),
            )*
        }
    };
}

#[macro_export]
macro_rules! any_view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $crate::view::View::into_any($val),
            )*
        }
    };
}
