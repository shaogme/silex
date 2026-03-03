use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::View;
use web_sys::Node;

// --- Manual VTable & SOO (Small Object Optimization) Infrastructure ---

const SOO_CAPACITY: usize = 24; // 3 * size_of::<usize>()

pub(crate) struct AnyViewVTable {
    pub mount: unsafe fn(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>),
    pub apply_attributes: unsafe fn(data: *mut u8, attrs: Vec<PendingAttribute>),
    pub drop: unsafe fn(data: *mut u8),
}

pub struct AnyViewBox {
    data: [usize; 3],
    vtable: &'static AnyViewVTable,
}

pub(crate) struct SharedViewVTable {
    pub any: AnyViewVTable,
    pub clone: unsafe fn(data: *const u8) -> SharedViewBox,
}

pub struct SharedViewBox {
    data: [usize; 3],
    vtable: &'static SharedViewVTable,
}

impl AnyViewBox {
    #[inline(always)]
    pub fn new<V: View + 'static>(view: V) -> Self {
        unsafe {
            if std::mem::size_of::<V>() <= SOO_CAPACITY
                && std::mem::align_of::<V>() <= std::mem::align_of::<usize>()
            {
                let mut data = [0usize; 3];
                std::ptr::write(data.as_mut_ptr() as *mut V, view);
                Self {
                    data,
                    vtable: &AnyViewVTable {
                        mount: mount_stack::<V>,
                        apply_attributes: apply_stack::<V>,
                        drop: drop_stack::<V>,
                    },
                }
            } else {
                let mut data = [0usize; 3];
                let ptr = Box::into_raw(Box::new(view));
                std::ptr::write(data.as_mut_ptr() as *mut *mut V, ptr);
                Self {
                    data,
                    vtable: &AnyViewVTable {
                        mount: mount_heap::<V>,
                        apply_attributes: apply_heap::<V>,
                        drop: drop_heap::<V>,
                    },
                }
            }
        }
    }

    pub fn mount(mut self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let vtable = self.vtable;
        unsafe {
            (vtable.mount)(self.data.as_mut_ptr() as *mut u8, parent, attrs);
            std::mem::forget(self);
        }
    }

    pub fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.vtable.apply_attributes)(self.data.as_mut_ptr() as *mut u8, attrs);
        }
    }
}

impl Drop for AnyViewBox {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(self.data.as_mut_ptr() as *mut u8);
        }
    }
}

impl SharedViewBox {
    #[inline(always)]
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        unsafe {
            if std::mem::size_of::<V>() <= SOO_CAPACITY
                && std::mem::align_of::<V>() <= std::mem::align_of::<usize>()
            {
                let mut data = [0usize; 3];
                std::ptr::write(data.as_mut_ptr() as *mut V, view);
                Self {
                    data,
                    vtable: &SharedViewVTable {
                        any: AnyViewVTable {
                            mount: mount_stack::<V>,
                            apply_attributes: apply_stack::<V>,
                            drop: drop_stack::<V>,
                        },
                        clone: clone_stack::<V>,
                    },
                }
            } else {
                let mut data = [0usize; 3];
                let ptr = Box::into_raw(Box::new(view));
                std::ptr::write(data.as_mut_ptr() as *mut *mut V, ptr);
                Self {
                    data,
                    vtable: &SharedViewVTable {
                        any: AnyViewVTable {
                            mount: mount_heap::<V>,
                            apply_attributes: apply_heap::<V>,
                            drop: drop_heap::<V>,
                        },
                        clone: clone_heap::<V>,
                    },
                }
            }
        }
    }

    pub fn mount(mut self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let vtable = self.vtable;
        unsafe {
            (vtable.any.mount)(self.data.as_mut_ptr() as *mut u8, parent, attrs);
            std::mem::forget(self);
        }
    }

    pub fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        unsafe {
            (self.vtable.any.apply_attributes)(self.data.as_mut_ptr() as *mut u8, attrs);
        }
    }

    pub fn into_any(self) -> AnyViewBox {
        let any_box = AnyViewBox {
            data: self.data,
            vtable: &self.vtable.any,
        };
        std::mem::forget(self);
        any_box
    }
}

impl Clone for SharedViewBox {
    fn clone(&self) -> Self {
        unsafe { (self.vtable.clone)(self.data.as_ptr() as *const u8) }
    }
}

impl Drop for SharedViewBox {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.any.drop)(self.data.as_mut_ptr() as *mut u8);
        }
    }
}

// --- VTable Glue Functions ---

unsafe fn mount_stack<V: View>(data: *mut u8, parent: &Node, attrs: Vec<PendingAttribute>) {
    unsafe {
        let view = std::ptr::read(data as *mut V);
        view.mount(parent, attrs);
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
        let ptr = std::ptr::read(data as *mut *mut V);
        let view = *Box::from_raw(ptr);
        view.mount(parent, attrs);
    }
}

unsafe fn apply_heap<V: View>(data: *mut u8, attrs: Vec<PendingAttribute>) {
    unsafe {
        let ptr = std::ptr::read(data as *mut *mut V);
        let view = &mut *ptr;
        view.apply_attributes(attrs);
    }
}

unsafe fn drop_heap<V: View>(data: *mut u8) {
    unsafe {
        let ptr = std::ptr::read(data as *mut *mut V);
        let _ = Box::from_raw(ptr);
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
        let ptr = std::ptr::read(data as *const *mut V);
        let view = &*ptr;
        SharedViewBox::new(view.clone())
    }
}

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
