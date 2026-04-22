use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::{Mount, MountExt, MountRef};
use silex_vtable::any_box::AnyBox;
use silex_vtable::func_ptr::FuncPtr;
use silex_vtable::thunk::FactoryBox;
use std::marker::PhantomData;
use std::mem;
use std::rc::Rc;
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

impl AnyViewBox {
    pub fn new<V: MountExt>(view: V) -> Self {
        struct VGen<V>(PhantomData<V>);
        impl<V: MountExt> VGen<V> {
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

// --- VTable Glue Functions ---

unsafe fn mount_stack<V: Mount>(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        let view_ptr = data as *mut mem::MaybeUninit<V>;
        let view = mem::replace(&mut *view_ptr, mem::MaybeUninit::uninit()).assume_init();
        view.mount(parent, attrs);
    }
}

unsafe fn mount_ref_stack<V: MountRef>(
    data: *const u8,
    parent: &Node,
    attrs: Vec<PendingAttribute>,
) {
    unsafe {
        let view = &*(data as *const V);
        view.mount_ref(parent, attrs);
    }
}

unsafe fn apply_stack<V: crate::view::ApplyAttributes>(
    data: *mut u8,
    attrs: Vec<PendingAttribute>,
) {
    unsafe {
        let view = &mut *(data as *mut V);
        view.apply_attributes(attrs);
    }
}

unsafe fn drop_stack<V>(data: *mut u8) {
    unsafe {
        std::ptr::drop_in_place(data as *mut V);
    }
}

unsafe fn mount_heap<V: Mount>(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        let ptr_ref = &mut *(data as *mut *mut V);
        let ptr = mem::replace(ptr_ref, std::ptr::null_mut());
        if !ptr.is_null() {
            let view = *Box::from_raw(ptr);
            view.mount(parent, attrs);
        }
    }
}

unsafe fn mount_ref_heap<V: MountRef>(
    data: *const u8,
    parent: &Node,
    attrs: Vec<PendingAttribute>,
) {
    unsafe {
        let ptr = *(data as *const *mut V);
        let view = &*ptr;
        view.mount_ref(parent, attrs);
    }
}

unsafe fn apply_heap<V: crate::view::ApplyAttributes>(data: *mut u8, attrs: Vec<PendingAttribute>) {
    unsafe {
        let ptr = *(data as *mut *mut V);
        let view = &mut *ptr;
        view.apply_attributes(attrs);
    }
}

unsafe fn drop_heap<V>(data: *mut u8) {
    unsafe {
        let ptr_ref = &mut *(data as *mut *mut V);
        let ptr = mem::replace(ptr_ref, std::ptr::null_mut());
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr);
        }
    }
}

// --- Thunks ---

pub type ViewThunk = FactoryBox<AnyView>;
pub type RenderThunk = silex_vtable::thunk::ThunkBox<(Node, Vec<PendingAttribute>), ()>;

/// 优化的 AnyView，作为所有视图类型擦除的终点
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Boxed(Rc<AnyViewBox>, Vec<PendingAttribute>),
}

impl AnyView {
    pub fn new<V: MountExt>(view: V) -> Self {
        AnyView::Boxed(Rc::new(AnyViewBox::new(view)), Vec::new())
    }

    pub fn into_any(self) -> Self {
        self
    }
}

fn apply_list_attributes<V: crate::view::ApplyAttributes>(
    list: &mut [V],
    attrs: Vec<PendingAttribute>,
) {
    for child in list {
        child.apply_attributes(attrs.clone());
    }
}

fn mount_list_owned<V: Mount>(list: Vec<V>, parent: &Node, attrs: Vec<PendingAttribute>) {
    for (i, child) in list.into_iter().enumerate() {
        child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
    }
}

fn mount_list_ref<V: MountRef>(list: &[V], parent: &Node, attrs: Vec<PendingAttribute>) {
    for (i, child) in list.iter().enumerate() {
        child.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
    }
}

fn merge_attrs(
    mut inner_attrs: Vec<PendingAttribute>,
    attrs: Vec<PendingAttribute>,
) -> Vec<PendingAttribute> {
    inner_attrs.extend(attrs);
    crate::attribute::consolidate_attributes(inner_attrs)
}

impl crate::view::ApplyAttributes for AnyView {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(_) => {}
            AnyView::Element(el) => el.apply_attributes(attrs),
            AnyView::List(list) => apply_list_attributes(list, attrs),
            AnyView::Boxed(_, inner_attrs) => {
                let temp = std::mem::take(inner_attrs);
                *inner_attrs = merge_attrs(temp, attrs);
            }
        }
    }
}

impl MountRef for AnyView {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount_ref(parent, attrs),
            AnyView::Element(el) => el.mount_ref(parent, attrs),
            AnyView::List(list) => mount_list_ref(list, parent, attrs),
            AnyView::Boxed(b, inner_attrs) => {
                b.mount_ref(parent, merge_attrs(inner_attrs.clone(), attrs));
            }
        }
    }
}

impl Mount for AnyView {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount(parent, attrs),
            AnyView::Element(el) => el.mount(parent, attrs),
            AnyView::List(list) => mount_list_owned(list, parent, attrs),
            AnyView::Boxed(b, inner_attrs) => {
                b.mount_ref(parent, merge_attrs(inner_attrs, attrs));
            }
        }
    }
}

impl Clone for AnyView {
    fn clone(&self) -> Self {
        match self {
            AnyView::Empty => AnyView::Empty,
            AnyView::Text(s) => AnyView::Text(s.clone()),
            AnyView::Element(el) => AnyView::Element(el.clone()),
            AnyView::List(list) => AnyView::List(list.clone()),
            AnyView::Boxed(b, attrs) => AnyView::Boxed(b.clone(), attrs.clone()),
        }
    }
}

impl PartialEq for AnyView {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnyView::Empty, AnyView::Empty) => true,
            (AnyView::Text(a), AnyView::Text(b)) => a == b,
            (AnyView::Element(a), AnyView::Element(b)) => a == b,
            (AnyView::List(a), AnyView::List(b)) => a == b,
            (AnyView::Boxed(a, _), AnyView::Boxed(b, _)) => Rc::ptr_eq(a, b),
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
        }
    }
}

/// 片段，用于容纳多个不同类型的子组件
#[derive(Default, Clone)]
pub struct Fragment(pub Vec<AnyView>);

impl Fragment {
    pub fn new(children: Vec<AnyView>) -> Self {
        Self(children)
    }
}

impl crate::view::ApplyAttributes for Fragment {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for child in &mut self.0 {
            child.apply_attributes(attrs.clone());
        }
    }
}

impl Mount for Fragment {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, child) in self.0.into_iter().enumerate() {
            child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }
}

impl MountRef for Fragment {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, child) in self.0.iter().enumerate() {
            child.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
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

macro_rules! impl_from_primitive {
    ($($t:ty),*) => {
        $(
            impl From<$t> for AnyView {
                fn from(v: $t) -> Self {
                    AnyView::Text(v.to_string())
                }
            }
        )*
    };
}
impl_from_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<V: MountExt> From<Vec<V>> for AnyView {
    fn from(v: Vec<V>) -> Self {
        AnyView::List(v.into_iter().map(|item| item.into_any()).collect())
    }
}

impl<V: MountExt> From<Option<V>> for AnyView {
    fn from(v: Option<V>) -> Self {
        match v {
            Some(val) => AnyView::new(val),
            None => AnyView::Empty,
        }
    }
}

// --- Recursive View Chain Erasure ---

impl From<crate::view::ViewNil> for AnyView {
    fn from(_: crate::view::ViewNil) -> Self {
        AnyView::Empty
    }
}

impl<H, T> From<crate::view::ViewCons<H, T>> for AnyView
where
    H: MountExt,
    T: MountExt,
{
    fn from(v: crate::view::ViewCons<H, T>) -> Self {
        AnyView::new(v)
    }
}

/// 一个辅助宏，用于简化从 `match` 表达式返回 `AnyView` 的操作。
///
/// 它会自动对每个分支的结果调用 `.into_any()`，从而允许不同类型的 View 在同一个 `match` 块中返回。
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
                $pat $(if $guard)? => $val.into_any(),
            )*
        }
    };
}

#[macro_export]
macro_rules! any_view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $val.into_any(),
            )*
        }
    };
}
