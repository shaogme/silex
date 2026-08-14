use silex_core::{SilexContextProvider, SilexError, SilexErrorKind, reactivity::ReactiveSource};
use silex_dom::prelude::*;
use silex_macros::component;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::rc::Rc;

/// Switch/Match 组件：多路分支渲染
///
/// # Example
/// ```rust, ignore
/// use silex::prelude::*;
/// let (count, set_count) = scope.signal(0);
///
/// Switch(context, count)
///     .fallback("Default View")
///     .build()
///     .case(0, "Zero")?
///     .case(1, "One")?;
/// ```
#[component]
pub fn Switch<'scope, Ctx, Source, T>(
    #[context] context: Ctx,
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
    let source = scope.promote(source, error_handler)?;
    let cases = Rc::new(cases);
    Ok(silex_core::rx!(context; {
        let val = (*$source).clone();
        if let Some(view) = cases.get(&val) {
            view.clone()
        } else {
            fallback.clone()
        }
    }))
}

impl<'scope, Ctx, Source, T> SwitchComponent<'scope, Ctx, Source, T>
where
    Ctx: SilexContextProvider<'scope>,
    Source: ReactiveSource<'scope, Value = T> + Clone + 'scope,
    T: Eq + Hash + Clone + 'scope,
{
    /// 添加一个匹配分支，并在重复 key 时返回配置错误。
    pub fn case<V>(mut self, value: T, view: V) -> Result<Self, SilexError>
    where
        V: View<'scope> + 'scope,
    {
        match self.props.cases.entry(value) {
            Entry::Vacant(entry) => {
                entry.insert(view.into_any());
                Ok(self)
            }
            Entry::Occupied(_) => Err(SilexError::fatal(SilexErrorKind::Javascript(
                "Duplicate case detected in Switch; each case value must be unique.".to_string(),
            ))),
        }
    }
}
