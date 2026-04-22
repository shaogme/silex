use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::{View, ViewCons, ViewNil};
use std::rc::Rc;
use web_sys::Node;

pub type RenderThunk = silex_vtable::thunk::ThunkBox<(Node, Vec<PendingAttribute>), ()>;

/// 优化的 AnyView，作为所有视图类型擦除的终点。
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(Element),
    List(Vec<AnyView>),
    Boxed(Rc<dyn View>, Vec<PendingAttribute>),
}

impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        AnyView::Boxed(Rc::new(view), Vec::new())
    }

    pub fn into_any(self) -> Self {
        self
    }
}

fn merge_attrs(
    mut inner_attrs: Vec<PendingAttribute>,
    attrs: Vec<PendingAttribute>,
) -> Vec<PendingAttribute> {
    inner_attrs.extend(attrs);
    crate::attribute::consolidate_attributes(inner_attrs)
}

fn mount_list(list: &[AnyView], parent: &Node, attrs: Vec<PendingAttribute>) {
    for (i, child) in list.iter().enumerate() {
        child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
    }
}

fn mount_list_owned(list: Vec<AnyView>, parent: &Node, attrs: Vec<PendingAttribute>) {
    for (i, child) in list.into_iter().enumerate() {
        child.mount_owned(parent, if i == 0 { attrs.clone() } else { Vec::new() });
    }
}

impl View for AnyView {
    fn into_any(self) -> Self {
        self
    }

    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount(parent, attrs),
            AnyView::Element(el) => el.mount(parent, attrs),
            AnyView::List(list) => mount_list(list, parent, attrs),
            AnyView::Boxed(b, inner_attrs) => {
                b.mount(parent, merge_attrs(inner_attrs.clone(), attrs));
            }
        }
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount_owned(parent, attrs),
            AnyView::Element(el) => el.mount_owned(parent, attrs),
            AnyView::List(list) => mount_list_owned(list, parent, attrs),
            AnyView::Boxed(b, inner_attrs) => {
                b.mount(parent, merge_attrs(inner_attrs, attrs));
            }
        }
    }
}

impl crate::view::ApplyAttributes for AnyView {
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
            AnyView::Boxed(_, inner_attrs) => {
                let temp = std::mem::take(inner_attrs);
                *inner_attrs = merge_attrs(temp, attrs);
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

impl View for Fragment {
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_list(&self.0, parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        mount_list_owned(self.0, parent, attrs);
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

impl<V: View + 'static> From<Vec<V>> for AnyView {
    fn from(v: Vec<V>) -> Self {
        AnyView::List(v.into_iter().map(|item| item.into_any()).collect())
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

impl From<ViewNil> for AnyView {
    fn from(_: ViewNil) -> Self {
        AnyView::Empty
    }
}

impl<H, T> From<ViewCons<H, T>> for AnyView
where
    H: View + 'static,
    T: View + 'static,
{
    fn from(v: ViewCons<H, T>) -> Self {
        AnyView::new(v)
    }
}

/// 一个辅助宏，用于简化从 `match` 表达式返回 `AnyView` 的操作。
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
