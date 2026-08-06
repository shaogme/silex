use silex_core::{ErrorReporter, Scope, SilexError, reactivity::ReactiveSource};
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
/// let (count, set_count) = scope.signal(0);
///
/// Switch(scope, count)
///     .fallback("Default View")
///     .case(0, "Zero")
///     .case(1, "One");
/// ```
#[component]
pub fn Switch<'scope, Source, T>(
    scope: Scope<'scope>,
    source: Source,
    #[chain(default)] cases: HashMap<T, AnyView<'scope>>,
    #[prop(render)]
    #[chain(default = AnyView::Empty)]
    fallback: AnyView<'scope>,
) -> impl View<'scope>
where
    Source: ReactiveSource<'scope, Value = T> + Clone + 'scope,
    T: Eq + Hash + Clone + 'scope,
{
    let source = scope.promote(source);
    let cases = Rc::new(cases);
    silex_core::rx!(scope; {
        let val = (*$source).clone();
        if let Some(view) = cases.get(&val) {
            view.clone()
        } else {
            fallback.clone()
        }
    })
}

impl<'scope, Source, T> SwitchComponent<'scope, Source, T>
where
    Source: ReactiveSource<'scope, Value = T> + Clone + 'scope,
    T: Eq + Hash + Clone + 'scope,
{
    /// 添加一个匹配分支
    pub fn case<V>(mut self, value: T, view: V) -> Self
    where
        V: View<'scope> + 'scope,
    {
        match self.cases.entry(value) {
            Entry::Vacant(entry) => {
                entry.insert(view.into_any());
            }
            Entry::Occupied(_) => {
                ErrorReporter::unhandled().report(SilexError::Javascript(
                    "Duplicate case detected in Switch; each case value must be unique."
                        .to_string(),
                ));
            }
        }
        self
    }
}
