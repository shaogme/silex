use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_macros::component;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::rc::Rc;

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
    let cases = Rc::new(cases);
    silex_core::rx! {
        let val = source.get();
        if let Some(view) = cases.get(&val) {
            view.clone()
        } else {
            fallback.clone()
        }
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
