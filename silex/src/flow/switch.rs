use silex_core::traits::RxGet;
use silex_dom::prelude::{ApplyAttributes, AutoReactiveView, Mount, MountRef};
use web_sys::Node;

/// Switch/Match 组件：多路分支渲染
///
/// # Example
/// ```rust
/// use silex::prelude::*;
/// let (count, set_count) = Signal::pair(0);
///
/// Switch::new(count, "Default View")
///     .case(0, "Zero")
///     .case(1, "One");
/// ```
#[derive(Clone)]
pub struct Switch<Source, T, V> {
    source: Source,
    cases: Vec<(T, V)>,
    fallback: V,
}

impl<Source, T, V> Switch<Source, T, V>
where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + 'static,
{
    pub fn new(source: Source, fallback: V) -> Self {
        Self {
            source,
            cases: Vec::new(),
            fallback,
        }
    }

    pub fn case(mut self, value: T, view: V) -> Self {
        self.cases.push((value, view));
        self
    }
}

impl<Source, T, V> ApplyAttributes for Switch<Source, T, V>
where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + 'static,
{
}

impl<Source, T, V> Mount for Switch<Source, T, V>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(self.source, self.cases, self.fallback, parent, attrs);
    }
}

impl<Source, T, V> AutoReactiveView for Switch<Source, T, V>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + Clone + 'static,
{
}

impl<Source, T, V> MountRef for Switch<Source, T, V>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(
            self.source.clone(),
            self.cases.clone(),
            self.fallback.clone(),
            parent,
            attrs,
        );
    }
}

fn mount_switch_internal<Source, T, V>(
    source: Source,
    cases: Vec<(T, V)>,
    fallback: V,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
    V: MountRef + 'static,
{
    use silex_dom::view::any::RenderThunk;
    silex_dom::view::mount_dynamic_view_universal(
        parent,
        attrs,
        RenderThunk::new(move |args| {
            let (p, a) = args;
            let val = source.get();
            let mut view = &fallback;

            for (case_val, case_view) in &cases {
                if *case_val == val {
                    view = case_view;
                    break;
                }
            }

            view.mount_ref(&p, a);
        }),
    );
}
