use crate::prelude::*;

styled! {
    /// 弹性布局容器 (Flexbox)
    pub Stack <div> (
        children: AnyView,
        #[prop(into)] #[chain(default = FlexDirectionKeyword::Column)]
        direction: Signal<FlexDirectionKeyword>,
        #[prop(into)] #[chain(default = AlignItemsKeyword::Stretch)]
        align: Signal<AlignItemsKeyword>,
        #[prop(into)] #[chain(default = JustifyContentKeyword::FlexStart)]
        justify: Signal<JustifyContentKeyword>,
        #[prop(into)] #[chain(default)]
        gap: Signal<i32>,
        #[prop(into)] #[chain(default)]
        style: Signal<Style>,
    ) {
        display: flex;
        flex-direction: $(direction);
        align-items: $(align);
        justify-content: $(justify);
        gap: $(gap.map_fn(|g| px(*g)));
    }
}

styled! {
    /// 居中容器
    pub Center <div> (
        children: AnyView,
        #[prop(into)] #[chain(default)]
        style: Signal<Style>,
    ) {
        display: flex;
        align-items: center;
        justify-content: center;
    }
}

styled! {
    /// 网格布局容器 (Grid)
    pub Grid <div> (
        children: AnyView,
        #[prop(into)] #[chain(default = 1)]
        columns: Signal<i32>,
        #[prop(into)] #[chain(default)]
        gap: Signal<i32>,
        #[prop(into)] #[chain(default)]
        style: Signal<Style>,
    ) {
        display: grid;
        grid-template-columns: repeat($(columns), minmax(0, 1fr));
        gap: $(gap.map_fn(|g| px(*g)));
    }
}
