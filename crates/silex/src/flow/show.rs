use silex_core::{Scope, reactivity::ReactiveSource};
use silex_dom::prelude::*;
use silex_macros::component;

/// Show 组件：根据条件渲染不同的视图
///
/// 使用方式：
/// ```rust,ignore
/// Show(scope, condition)
///     .children(view)
///     .fallback(fallback_view)
///     .build()
/// ```
#[component]
pub fn Show<'scope, C>(
    scope: Scope<'scope>,
    when: C,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[prop(render)]
    #[chain(default = AnyView::Empty)]
    fallback: AnyView<'scope>,
) -> impl View<'scope>
where
    C: ReactiveSource<'scope, Value = bool> + Clone + 'scope,
{
    let condition = scope.promote(when);
    silex_core::rx!(scope; if *$condition {
        children.clone()
    } else {
        fallback.clone()
    })
}

// --- Signal 扩展 ---

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt<'scope>: ReactiveSource<'scope, Value = bool> + Clone + Sized {
    fn when<V>(self, scope: Scope<'scope>, view: V) -> ShowComponent<'scope, Self>
    where
        V: View<'scope> + 'scope;
}

impl<'scope, S> SignalShowExt<'scope> for S
where
    S: ReactiveSource<'scope, Value = bool> + Clone,
{
    fn when<V>(self, scope: Scope<'scope>, view: V) -> ShowComponent<'scope, Self>
    where
        V: View<'scope> + 'scope,
    {
        Show(scope, self).children(view).build()
    }
}
