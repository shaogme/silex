use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_macros::component;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
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
    #[chain(default)] cases: HashMap<T, AnyView>,
    #[prop(render)]
    #[chain(default = AnyView::Empty)]
    fallback: AnyView,
) -> impl View
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    SwitchView {
        source: Prop::new_owned(source),
        cases: Prop::new_owned(cases),
        fallback: Prop::new_owned(fallback),
    }
}

impl<Source, T> SwitchComponent<Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    /// 添加一个匹配分支
    pub fn case<V>(mut self, value: T, view: V) -> Self
    where
        V: View + 'static,
    {
        match self.cases.entry(value) {
            Entry::Vacant(entry) => {
                entry.insert(view.into_any());
            }
            Entry::Occupied(_) => {
                silex_core::error::handle_error(silex_core::SilexError::Javascript(
                    "Duplicate case detected in Switch; each case value must be unique."
                        .to_string(),
                ));
            }
        }
        self
    }
}

#[derive(Clone)]
struct SwitchView<'a, Source, T> {
    source: Prop<'a, Source>,
    cases: Prop<'a, HashMap<T, AnyView>>,
    fallback: Prop<'a, AnyView>,
}

impl<'a, Source, T> ApplyAttributes for SwitchView<'a, Source, T> {}

impl<'a, Source, T> View for SwitchView<'a, Source, T>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    fn mount(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(
            self.source.clone().into_owned(),
            self.cases.clone().into_owned(),
            self.fallback.clone().into_owned(),
            parent,
            attrs,
        );
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>)
    where
        Self: Sized,
    {
        mount_switch_internal(
            self.source.into_owned(),
            self.cases.into_owned(),
            self.fallback.into_owned(),
            parent,
            attrs,
        );
    }
}

fn mount_switch_internal<Source, T>(
    source: Source,
    cases: HashMap<T, AnyView>,
    fallback: AnyView,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    let cases = std::rc::Rc::new(cases);

    let cases_for_key = cases.clone();
    let cases_for_render = cases.clone();

    silex_dom::view::mount_branch_cached(
        parent,
        attrs,
        move || {
            let val = source.get();
            if cases_for_key.contains_key(&val) {
                Some(val)
            } else {
                None
            }
        },
        move |selected| {
            if let Some(case_key) = selected
                && let Some(view) = cases_for_render.get(&case_key)
            {
                return view.clone();
            }

            fallback.clone()
        },
    );
}
