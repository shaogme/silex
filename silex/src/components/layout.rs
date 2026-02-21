use crate::prelude::*;

styled! {
    /// 弹性布局容器 (Flexbox)
    pub Stack <div> (
        #[prop(default = FlexDirectionKeyword::Column, into)]
        direction: Signal<FlexDirectionKeyword>,
        #[prop(default = AlignItemsKeyword::Stretch, into)]
        align: Signal<AlignItemsKeyword>,
        #[prop(default = JustifyContentKeyword::FlexStart, into)]
        justify: Signal<JustifyContentKeyword>,
        #[prop(default, into)]
        gap: Signal<i32>,
        #[prop(default, into)]
        style: Signal<Style>,
        children: AnyView
    ) {
        display: flex;
        flex-direction: $(direction);
        align-items: $(align);
        justify-content: $(justify);
        gap: $(gap.map(|g| px(*g)));
    }
}

styled! {
    /// 居中容器
    pub Center <div> (
        #[prop(default, into)]
        style: Signal<Style>,
        children: AnyView
    ) {
        display: flex;
        align-items: center;
        justify-content: center;
    }
}

styled! {
    /// 网格布局容器 (Grid)
    pub Grid <div> (
        #[prop(default = 1, into)]
        columns: Signal<i32>,
        #[prop(default, into)]
        gap: Signal<i32>,
        #[prop(default, into)]
        style: Signal<Style>,
        children: AnyView
    ) {
        display: grid;
        grid-template-columns: repeat($(columns), minmax(0, 1fr));
        gap: $(gap.map(|g| px(*g)));
    }
}
