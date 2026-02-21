/// 中心化 CSS 属性注册表
///
/// 格式: (方法名, CSS 属性名, 标记结构体名, 分组)
#[macro_export]
macro_rules! for_all_properties {
    ($callback:ident) => {
        $callback! {
            // 基础布局
            (width, "width", Width, Dimension),
            (height, "height", Height, Dimension),
            (margin, "margin", Margin, Dimension),
            (padding, "padding", Padding, Dimension),
            (z_index, "z-index", ZIndex, Number),
            (display, "display", Display, Keyword),
            (position, "position", Position, Keyword),
            (top, "top", Top, Dimension),
            (left, "left", Left, Dimension),
            (right, "right", Right, Dimension),
            (bottom, "bottom", Bottom, Dimension),

            // Flexbox
            (flex_direction, "flex-direction", FlexDirection, Keyword),
            (flex_wrap, "flex-wrap", FlexWrap, Keyword),
            (flex_grow, "flex-grow", FlexGrow, Number),
            (flex_shrink, "flex-shrink", FlexShrink, Number),
            (flex_basis, "flex-basis", FlexBasis, Dimension),
            (align_items, "align-items", AlignItems, Keyword),
            (justify_content, "justify-content", JustifyContent, Keyword),
            (gap, "gap", Gap, Dimension),

            // 文本与字体
            (font_size, "font-size", FontSize, Dimension),
            (font_weight, "font-weight", FontWeight, Number),
            (line_height, "line-height", LineHeight, Dimension),
            (letter_spacing, "letter-spacing", LetterSpacing, Dimension),
            (text_align, "text-align", TextAlign, Keyword),
            (text_decoration, "text-decoration", TextDecoration, Custom),

            // 边框与装饰
            (border, "border", Border, Shorthand),
            (border_width, "border-width", BorderWidth, Dimension),
            (border_style, "border-style", BorderStyle, Keyword),
            (border_color, "border-color", BorderColor, Color),
            (border_radius, "border-radius", BorderRadius, Dimension),
            (outline, "outline", Outline, Color),
            (opacity, "opacity", Opacity, Number),
            (visibility, "visibility", Visibility, Keyword),
            (box_shadow, "box-shadow", BoxShadow, Custom),

            // 背景
            (background, "background", Background, Color), // Background 也支持颜色直接赋值
            (background_color, "background-color", BackgroundColor, Color),
            (background_image, "background-image", BackgroundImage, Custom),

            // 颜色定义 (注意：由于结构体名也是 Color，这里必须确保正确生成)
            (color, "color", Color, Color),

            // 交互
            (cursor, "cursor", Cursor, Keyword),
            (pointer_events, "pointer-events", PointerEvents, Keyword),
            (overflow, "overflow", Overflow, Keyword),
            (overflow_x, "overflow-x", OverflowX, Keyword),
            (overflow_y, "overflow-y", OverflowY, Keyword),

            // 动画与变换
            (transition, "transition", Transition, Custom),
            (transform, "transform", Transform, Custom),
            (filter, "filter", Filter, Custom),
            (backdrop_filter, "backdrop-filter", BackdropFilter, Custom)
        }
    };
}
