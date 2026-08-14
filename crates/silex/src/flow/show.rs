use silex_core::{SilexContextProvider, reactivity::ReactiveSource};
use silex_dom::prelude::*;
use silex_macros::component;

/// Show 组件：根据条件渲染不同的视图
///
/// 使用方式：
/// ```rust,ignore
/// Show(ctx, condition)
///     .children(view)
///     .fallback(fallback_view)
///     .build()
/// ```
#[component]
pub fn Show<'scope, Ctx, C>(
    #[ctx] ctx: Ctx,
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
    let condition = scope.promote(when, error_handler)?;
    Ok(silex_core::rx!(ctx; if *$condition {
        children.clone()
    } else {
        fallback.clone()
    }))
}

// --- Signal 扩展 ---

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt<'scope>: ReactiveSource<'scope, Value = bool> + Clone + Sized {
    fn when<Ctx, V>(self, ctx: Ctx, view: V) -> ShowBuilder<'scope, PropFixed, Ctx, Self>
    where
        Ctx: SilexContextProvider<'scope>,
        V: View<'scope> + 'scope;
}

impl<'scope, S> SignalShowExt<'scope> for S
where
    S: ReactiveSource<'scope, Value = bool> + Clone,
{
    fn when<Ctx, V>(self, ctx: Ctx, view: V) -> ShowBuilder<'scope, PropFixed, Ctx, Self>
    where
        Ctx: SilexContextProvider<'scope>,
        V: View<'scope> + 'scope,
    {
        Show(ctx, self).children(view)
    }
}
