use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::View;
use web_sys::Node;

/// 辅助特征（不要求 Clone，移动语义挂载）
pub trait RenderOnce {
    fn mount_boxed(self: Box<Self>, parent: &Node, attrs: Vec<PendingAttribute>);
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>);
}

impl<V: View + 'static> RenderOnce for V {
    fn mount_boxed(self: Box<Self>, parent: &Node, attrs: Vec<PendingAttribute>) {
        (*self).mount(parent, attrs)
    }
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>) {
        self.apply_attributes(attrs);
    }
}

/// 辅助特征（支持克隆）
pub trait RenderShared: RenderOnce {
    fn clone_boxed(&self) -> Box<dyn RenderShared>;
    fn into_once_boxed(self: Box<Self>) -> Box<dyn RenderOnce>;
}

impl<V: View + Clone + 'static> RenderShared for V {
    fn clone_boxed(&self) -> Box<dyn RenderShared> {
        Box::new(self.clone())
    }
    fn into_once_boxed(self: Box<Self>) -> Box<dyn RenderOnce> {
        self
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
    SharedBoxed(Box<dyn RenderShared>, Vec<PendingAttribute>),
}

/// 优化的 AnyView，作为所有视图类型擦除的终点（不要求 Clone）
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Unique(Box<dyn RenderOnce>, Vec<PendingAttribute>),
    FromShared(SharedView),
}

impl SharedView {
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        view.into_shared()
    }
}

impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        view.into_any()
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
            SharedView::SharedBoxed(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount_boxed(
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
            SharedView::SharedBoxed(_, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
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
            AnyView::Unique(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount_boxed(
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
            AnyView::Unique(_, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
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
            SharedView::SharedBoxed(b, attrs) => {
                SharedView::SharedBoxed(b.clone_boxed(), attrs.clone())
            }
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
            Self::Unique(_, _) => write!(f, "AnyView(Unique)"),
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
            Self::SharedBoxed(_, _) => write!(f, "SharedView(SharedBoxed)"),
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

macro_rules! impl_from_tuple {
    ($($name:ident),*) => {
        impl<$($name: View + 'static),*> From<($($name,)*)> for AnyView {
            fn from(v: ($($name,)*)) -> Self {
                AnyView::new(v)
            }
        }

        impl<$($name: View + Clone + 'static),*> From<($($name,)*)> for SharedView {
            fn from(v: ($($name,)*)) -> Self {
                SharedView::new(v)
            }
        }
    }
}

impl_from_tuple!(A);
impl_from_tuple!(A, B);
impl_from_tuple!(A, B, C);
impl_from_tuple!(A, B, C, D);
impl_from_tuple!(A, B, C, D, E);
impl_from_tuple!(A, B, C, D, E, F);
impl_from_tuple!(A, B, C, D, E, F, G);
impl_from_tuple!(A, B, C, D, E, F, G, H);
impl_from_tuple!(A, B, C, D, E, F, G, H, I);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

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
