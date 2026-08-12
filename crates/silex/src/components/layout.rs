use crate::prelude::*;

styled! {
    /// 弹性布局容器 (Flexbox)
    pub Stack<'scope> <div> (
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
        children: AnyView<'scope>,
        #[prop(into)] #[chain(default = FlexDirectionKeyword::Column)]
        direction: Signal<'scope, FlexDirectionKeyword>,
        #[prop(into)] #[chain(default = AlignItemsKeyword::Stretch)]
        align: Signal<'scope, AlignItemsKeyword>,
        #[prop(into)] #[chain(default = JustifyContentKeyword::FlexStart)]
        justify: Signal<'scope, JustifyContentKeyword>,
        #[prop(into)] #[chain(default)]
        gap: Signal<'scope, i32>,
        #[prop(into)] #[chain(default)]
        style: Signal<'scope, Style<'scope>>,
    ) {
        display: flex;
        flex-direction: $(direction);
        align-items: $(align);
        justify-content: $(justify);
        gap: $(gap.map_fn(scope, |g| px(*g), error_handler)?);
    }
}

styled! {
    /// 居中容器
    pub Center<'scope> <div> (
        scope: Scope<'scope>,
        children: AnyView<'scope>,
        #[prop(into)] #[chain(default)]
        style: Signal<'scope, Style<'scope>>,
    ) {
        display: flex;
        align-items: center;
        justify-content: center;
    }
}

styled! {
    /// 网格布局容器 (Grid)
    pub Grid<'scope> <div> (
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
        children: AnyView<'scope>,
        #[prop(into)] #[chain(default = 1)]
        columns: Signal<'scope, i32>,
        #[prop(into)] #[chain(default)]
        gap: Signal<'scope, i32>,
        #[prop(into)] #[chain(default)]
        style: Signal<'scope, Style<'scope>>,
    ) {
        display: grid;
        grid-template-columns: repeat($(columns), minmax(0, 1fr));
        gap: $(gap.map_fn(scope, |g| px(*g), error_handler)?);
    }
}
