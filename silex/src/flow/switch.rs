use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_macros::component;
use web_sys::Node;

/// Switch/Match 组件：多路分支渲染
///
/// # Example
/// ```rust
/// use silex::prelude::*;
/// let (count, set_count) = Signal::pair(0);
///
/// Switch(count)
///     .fallback("Default View")
///     .case(0, "Zero")
///     .case(1, "One");
/// ```
#[component]
pub fn Switch<Source, T>(
    source: Source,
    #[prop(default)] cases: Vec<(T, SharedView)>,
    #[prop(default = SharedView::Empty, into)] fallback: SharedView,
) -> impl Mount + MountRef
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
{
    SwitchView {
        source: source.clone(),
        cases: cases.clone(),
        fallback: fallback.clone(),
    }
}

impl<Source, T> SwitchComponent<Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
{
    /// 添加一个匹配分支
    pub fn case<V>(mut self, value: T, view: V) -> Self
    where
        V: MountRefExt + 'static,
    {
        self.cases.push((value, view.into_shared()));
        self
    }
}

#[derive(Clone)]
struct SwitchView<Source, T> {
    source: Source,
    cases: Vec<(T, SharedView)>,
    fallback: SharedView,
}

impl<Source, T> ApplyAttributes for SwitchView<Source, T> {}

impl<Source, T> Mount for SwitchView<Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(self.source, self.cases, self.fallback, parent, attrs);
    }
}

impl<Source, T> AutoReactiveView for SwitchView<Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
{
}

impl<Source, T> MountRef for SwitchView<Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
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

fn mount_switch_internal<Source, T>(
    source: Source,
    cases: Vec<(T, SharedView)>,
    fallback: SharedView,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
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
