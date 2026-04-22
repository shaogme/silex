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
    #[prop(default)] cases: HashMap<T, AnyView>,
    #[prop(default = AnyView::Empty, render)] fallback: AnyView,
) -> impl View
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    SwitchView {
        source,
        cases,
        fallback,
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
            Prop::new_owned(self.source.clone()),
            Prop::new_owned(self.cases.clone()),
            Prop::new_owned(self.fallback.clone()),
            parent,
            attrs,
        );
    }
}

fn mount_switch_internal<'a, Source, T>(
    source: Prop<'a, Source>,
    cases: Prop<'a, HashMap<T, AnyView>>,
    fallback: Prop<'a, AnyView>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Source: RxGet<Value = T> + Clone + 'static,
    T: Eq + Hash + Clone + 'static,
{
    let source = source.into_owned();
    let cases = std::rc::Rc::new(cases.into_owned());
    let fallback = fallback.into_owned();

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
