// 自动生成的 CSS 关键字 Enums
//
// 关键字集合完全相同的属性共用同一个枚举，其余属性以类型别名指向它。
// 此前是每个属性一个独立枚举（361 个 enum / 7 520 个 variant，实际只有
// 194 种不同的关键字集合），且 `AlignItemsKeyword::Center` 与
// `JustifyContentKeyword::Center` 是两个互不相干的类型。

use crate::define_css_enum;
use crate::types::{Auto, NoneValue, ValidFor, props};
use std::fmt::{Display, Formatter, Result};

// 全局具名颜色关键字。
// 所有接受 `<color>` 的属性共用这一份，不再每个属性复制一遍
//（此前有 18 个枚举是同一份 31 项系统颜色表的逐字拷贝）。
define_css_enum!(ColorKeyword (props::Color) {
    AccentColor => "AccentColor",
    AccentColorText => "AccentColorText",
    ActiveBorder => "ActiveBorder",
    ActiveCaption => "ActiveCaption",
    ActiveText => "ActiveText",
    AppWorkspace => "AppWorkspace",
    Background => "Background",
    ButtonBorder => "ButtonBorder",
    ButtonFace => "ButtonFace",
    ButtonHighlight => "ButtonHighlight",
    ButtonShadow => "ButtonShadow",
    ButtonText => "ButtonText",
    Canvas => "Canvas",
    CanvasText => "CanvasText",
    CaptionText => "CaptionText",
    Field => "Field",
    FieldText => "FieldText",
    GrayText => "GrayText",
    Highlight => "Highlight",
    HighlightText => "HighlightText",
    InactiveBorder => "InactiveBorder",
    InactiveCaption => "InactiveCaption",
    InactiveCaptionText => "InactiveCaptionText",
    InfoBackground => "InfoBackground",
    InfoText => "InfoText",
    LinkText => "LinkText",
    Mark => "Mark",
    MarkText => "MarkText",
    Menu => "Menu",
    MenuText => "MenuText",
    Scrollbar => "Scrollbar",
    SelectedItem => "SelectedItem",
    SelectedItemText => "SelectedItemText",
    ThreeDDarkShadow => "ThreeDDarkShadow",
    ThreeDFace => "ThreeDFace",
    ThreeDHighlight => "ThreeDHighlight",
    ThreeDLightShadow => "ThreeDLightShadow",
    ThreeDShadow => "ThreeDShadow",
    VisitedText => "VisitedText",
    Window => "Window",
    WindowFrame => "WindowFrame",
    WindowText => "WindowText",
    Aliceblue => "aliceblue",
    Antiquewhite => "antiquewhite",
    Aqua => "aqua",
    Aquamarine => "aquamarine",
    Azure => "azure",
    Beige => "beige",
    Bisque => "bisque",
    Black => "black",
    Blanchedalmond => "blanchedalmond",
    Blue => "blue",
    Blueviolet => "blueviolet",
    Brown => "brown",
    Burlywood => "burlywood",
    Cadetblue => "cadetblue",
    Chartreuse => "chartreuse",
    Chocolate => "chocolate",
    Coral => "coral",
    Cornflowerblue => "cornflowerblue",
    Cornsilk => "cornsilk",
    Crimson => "crimson",
    CurrentColor => "currentColor",
    Cyan => "cyan",
    Darkblue => "darkblue",
    Darkcyan => "darkcyan",
    Darkgoldenrod => "darkgoldenrod",
    Darkgray => "darkgray",
    Darkgreen => "darkgreen",
    Darkgrey => "darkgrey",
    Darkkhaki => "darkkhaki",
    Darkmagenta => "darkmagenta",
    Darkolivegreen => "darkolivegreen",
    Darkorange => "darkorange",
    Darkorchid => "darkorchid",
    Darkred => "darkred",
    Darksalmon => "darksalmon",
    Darkseagreen => "darkseagreen",
    Darkslateblue => "darkslateblue",
    Darkslategray => "darkslategray",
    Darkslategrey => "darkslategrey",
    Darkturquoise => "darkturquoise",
    Darkviolet => "darkviolet",
    Deeppink => "deeppink",
    Deepskyblue => "deepskyblue",
    Dimgray => "dimgray",
    Dimgrey => "dimgrey",
    Dodgerblue => "dodgerblue",
    Firebrick => "firebrick",
    Floralwhite => "floralwhite",
    Forestgreen => "forestgreen",
    Fuchsia => "fuchsia",
    Gainsboro => "gainsboro",
    Ghostwhite => "ghostwhite",
    Gold => "gold",
    Goldenrod => "goldenrod",
    Gray => "gray",
    Green => "green",
    Greenyellow => "greenyellow",
    Grey => "grey",
    Honeydew => "honeydew",
    Hotpink => "hotpink",
    Indianred => "indianred",
    Indigo => "indigo",
    Ivory => "ivory",
    Khaki => "khaki",
    Lavender => "lavender",
    Lavenderblush => "lavenderblush",
    Lawngreen => "lawngreen",
    Lemonchiffon => "lemonchiffon",
    Lightblue => "lightblue",
    Lightcoral => "lightcoral",
    Lightcyan => "lightcyan",
    Lightgoldenrodyellow => "lightgoldenrodyellow",
    Lightgray => "lightgray",
    Lightgreen => "lightgreen",
    Lightgrey => "lightgrey",
    Lightpink => "lightpink",
    Lightsalmon => "lightsalmon",
    Lightseagreen => "lightseagreen",
    Lightskyblue => "lightskyblue",
    Lightslategray => "lightslategray",
    Lightslategrey => "lightslategrey",
    Lightsteelblue => "lightsteelblue",
    Lightyellow => "lightyellow",
    Lime => "lime",
    Limegreen => "limegreen",
    Linen => "linen",
    Magenta => "magenta",
    Maroon => "maroon",
    Mediumaquamarine => "mediumaquamarine",
    Mediumblue => "mediumblue",
    Mediumorchid => "mediumorchid",
    Mediumpurple => "mediumpurple",
    Mediumseagreen => "mediumseagreen",
    Mediumslateblue => "mediumslateblue",
    Mediumspringgreen => "mediumspringgreen",
    Mediumturquoise => "mediumturquoise",
    Mediumvioletred => "mediumvioletred",
    Midnightblue => "midnightblue",
    Mintcream => "mintcream",
    Mistyrose => "mistyrose",
    Moccasin => "moccasin",
    Navajowhite => "navajowhite",
    Navy => "navy",
    Oldlace => "oldlace",
    Olive => "olive",
    Olivedrab => "olivedrab",
    Orange => "orange",
    Orangered => "orangered",
    Orchid => "orchid",
    Palegoldenrod => "palegoldenrod",
    Palegreen => "palegreen",
    Paleturquoise => "paleturquoise",
    Palevioletred => "palevioletred",
    Papayawhip => "papayawhip",
    Peachpuff => "peachpuff",
    Peru => "peru",
    Pink => "pink",
    Plum => "plum",
    Powderblue => "powderblue",
    Purple => "purple",
    Rebeccapurple => "rebeccapurple",
    Red => "red",
    Rosybrown => "rosybrown",
    Royalblue => "royalblue",
    Saddlebrown => "saddlebrown",
    Salmon => "salmon",
    Sandybrown => "sandybrown",
    Seagreen => "seagreen",
    Seashell => "seashell",
    Sienna => "sienna",
    Silver => "silver",
    Skyblue => "skyblue",
    Slateblue => "slateblue",
    Slategray => "slategray",
    Slategrey => "slategrey",
    Snow => "snow",
    Springgreen => "springgreen",
    Steelblue => "steelblue",
    Tan => "tan",
    Teal => "teal",
    Thistle => "thistle",
    Tomato => "tomato",
    Transparent => "transparent",
    Turquoise => "turquoise",
    Violet => "violet",
    Wheat => "wheat",
    White => "white",
    Whitesmoke => "whitesmoke",
    Yellow => "yellow",
    Yellowgreen => "yellowgreen",
});

define_css_enum!(WebkitBoxReflectKeyword (props::WebkitBoxReflect) {
    Above => "above",
    Below => "below",
    Left => "left",
    Right => "right",
});

define_css_enum!(PositionKeyword (props::Position) {
    Absolute => "absolute",
    Fixed => "fixed",
    Relative => "relative",
    Static_ => "static",
    Sticky => "sticky",
});

define_css_enum!(AnimationCompositionKeyword (props::AnimationComposition) {
    Accumulate => "accumulate",
    Add => "add",
    Replace => "replace",
});

define_css_enum!(MaskKeyword (props::Mask) {
    Add => "add",
    Alpha => "alpha",
    BorderBox => "border-box",
    Bottom => "bottom",
    Center => "center",
    ContentBox => "content-box",
    Exclude => "exclude",
    FillBox => "fill-box",
    Intersect => "intersect",
    Left => "left",
    Luminance => "luminance",
    MarginBox => "margin-box",
    MatchSource => "match-source",
    NoClip => "no-clip",
    NoRepeat => "no-repeat",
    None => "none",
    PaddingBox => "padding-box",
    Repeat => "repeat",
    RepeatX => "repeat-x",
    RepeatY => "repeat-y",
    Right => "right",
    Round => "round",
    Space => "space",
    StrokeBox => "stroke-box",
    Subtract => "subtract",
    Top => "top",
    ViewBox => "view-box",
});

define_css_enum!(MaskCompositeKeyword (props::MaskComposite) {
    Add => "add",
    Exclude => "exclude",
    Intersect => "intersect",
    Subtract => "subtract",
});

define_css_enum!(ScrollMarkerGroupKeyword (props::ScrollMarkerGroup) {
    After => "after",
    Before => "before",
    None => "none",
});

define_css_enum!(CursorKeyword (props::Cursor) {
    Alias => "alias",
    AllScroll => "all-scroll",
    Auto => "auto",
    Cell => "cell",
    ColResize => "col-resize",
    ContextMenu => "ctx-menu",
    Copy => "copy",
    Crosshair => "crosshair",
    Default_ => "default",
    EResize => "e-resize",
    EwResize => "ew-resize",
    Grab => "grab",
    Grabbing => "grabbing",
    Help => "help",
    Move_ => "move",
    NResize => "n-resize",
    NeResize => "ne-resize",
    NeswResize => "nesw-resize",
    NoDrop => "no-drop",
    None => "none",
    NotAllowed => "not-allowed",
    NsResize => "ns-resize",
    NwResize => "nw-resize",
    NwseResize => "nwse-resize",
    Pointer => "pointer",
    Progress => "progress",
    RowResize => "row-resize",
    SResize => "s-resize",
    SeResize => "se-resize",
    SwResize => "sw-resize",
    Text => "text",
    VerticalText => "vertical-text",
    WResize => "w-resize",
    Wait => "wait",
    ZoomIn => "zoom-in",
    ZoomOut => "zoom-out",
});

define_css_enum!(TransitionKeyword (props::Transition) {
    All => "all",
    AllowDiscrete => "allow-discrete",
    Ease => "ease",
    EaseIn => "ease-in",
    EaseInOut => "ease-in-out",
    EaseOut => "ease-out",
    Linear => "linear",
    None => "none",
    Normal => "normal",
    StepEnd => "step-end",
    StepStart => "step-start",
});

define_css_enum!(BreakAfterKeyword (props::BreakAfter, props::BreakBefore) {
    All => "all",
    Always => "always",
    Auto => "auto",
    Avoid => "avoid",
    AvoidColumn => "avoid-column",
    AvoidPage => "avoid-page",
    AvoidRegion => "avoid-region",
    Column => "column",
    Left => "left",
    Page => "page",
    Recto => "recto",
    Region => "region",
    Right => "right",
    Verso => "verso",
});

define_css_enum!(PointerEventsKeyword (props::PointerEvents) {
    All => "all",
    Auto => "auto",
    Fill => "fill",
    Inherit => "inherit",
    None => "none",
    Painted => "painted",
    Stroke => "stroke",
    Visible => "visible",
    VisibleFill => "visibleFill",
    VisiblePainted => "visiblePainted",
    VisibleStroke => "visibleStroke",
});

define_css_enum!(TextDecorationSkipInkKeyword (props::TextDecorationSkipInk) {
    All => "all",
    Auto => "auto",
    None => "none",
});

define_css_enum!(WebkitUserSelectKeyword (props::WebkitUserSelect, props::UserSelect) {
    All => "all",
    Auto => "auto",
    None => "none",
    Text => "text",
});

define_css_enum!(TextCombineUprightKeyword (props::TextCombineUpright) {
    All => "all",
    Digits => "digits",
    None => "none",
});

define_css_enum!(ColumnSpanKeyword (props::ColumnSpan, props::TransitionProperty, props::TriggerScope) {
    All => "all",
    None => "none",
});

define_css_enum!(FontVariantKeyword (props::FontVariant) {
    AllPetiteCaps => "all-petite-caps",
    AllSmallCaps => "all-small-caps",
    CommonLigatures => "common-ligatures",
    Contextual => "ctxual",
    DiagonalFractions => "diagonal-fractions",
    DiscretionaryLigatures => "discretionary-ligatures",
    FullWidth => "full-width",
    HistoricalForms => "historical-forms",
    HistoricalLigatures => "historical-ligatures",
    Jis04 => "jis04",
    Jis78 => "jis78",
    Jis83 => "jis83",
    Jis90 => "jis90",
    LiningNums => "lining-nums",
    NoCommonLigatures => "no-common-ligatures",
    NoContextual => "no-ctxual",
    NoDiscretionaryLigatures => "no-discretionary-ligatures",
    NoHistoricalLigatures => "no-historical-ligatures",
    None => "none",
    Normal => "normal",
    OldstyleNums => "oldstyle-nums",
    Ordinal => "ordinal",
    PetiteCaps => "petite-caps",
    ProportionalNums => "proportional-nums",
    ProportionalWidth => "proportional-width",
    Ruby => "ruby",
    Simplified => "simplified",
    SlashedZero => "slashed-zero",
    SmallCaps => "small-caps",
    StackedFractions => "stacked-fractions",
    TabularNums => "tabular-nums",
    TitlingCaps => "titling-caps",
    Traditional => "traditional",
    Unicase => "unicase",
});

define_css_enum!(FontVariantCapsKeyword (props::FontVariantCaps) {
    AllPetiteCaps => "all-petite-caps",
    AllSmallCaps => "all-small-caps",
    Normal => "normal",
    PetiteCaps => "petite-caps",
    SmallCaps => "small-caps",
    TitlingCaps => "titling-caps",
    Unicase => "unicase",
});

define_css_enum!(TransitionBehaviorKeyword (props::TransitionBehavior) {
    AllowDiscrete => "allow-discrete",
    Normal => "normal",
});

define_css_enum!(HangingPunctuationKeyword (props::HangingPunctuation) {
    AllowEnd => "allow-end",
    First => "first",
    ForceEnd => "force-end",
    Last => "last",
    None => "none",
});

define_css_enum!(MaskBorderModeKeyword (props::MaskBorderMode, props::MaskType) {
    Alpha => "alpha",
    Luminance => "luminance",
});

define_css_enum!(MaskModeKeyword (props::MaskMode) {
    Alpha => "alpha",
    Luminance => "luminance",
    MatchSource => "match-source",
});

define_css_enum!(MaskBorderKeyword (props::MaskBorder) {
    Alpha => "alpha",
    Luminance => "luminance",
    None => "none",
    Repeat => "repeat",
    Round => "round",
    Space => "space",
    Stretch => "stretch",
});

define_css_enum!(DominantBaselineKeyword (props::DominantBaseline) {
    Alphabetic => "alphabetic",
    Auto => "auto",
    Central => "central",
    Hanging => "hanging",
    Ideographic => "ideographic",
    Mathematical => "mathematical",
    Middle => "middle",
    TextBottom => "text-bottom",
    TextTop => "text-top",
});

define_css_enum!(AlignmentBaselineKeyword (props::AlignmentBaseline) {
    Alphabetic => "alphabetic",
    Baseline => "baseline",
    Central => "central",
    Ideographic => "ideographic",
    Mathematical => "mathematical",
    Middle => "middle",
    TextAfterEdge => "text-after-edge",
    TextBeforeEdge => "text-before-edge",
});

define_css_enum!(AnimationKeyword (props::Animation) {
    Alternate => "alternate",
    AlternateReverse => "alternate-reverse",
    Auto => "auto",
    Backwards => "backwards",
    Both => "both",
    Ease => "ease",
    EaseIn => "ease-in",
    EaseInOut => "ease-in-out",
    EaseOut => "ease-out",
    Forwards => "forwards",
    Infinite => "infinite",
    Linear => "linear",
    None => "none",
    Normal => "normal",
    Paused => "paused",
    Reverse => "reverse",
    Running => "running",
    StepEnd => "step-end",
    StepStart => "step-start",
});

define_css_enum!(AnimationDirectionKeyword (props::AnimationDirection) {
    Alternate => "alternate",
    AlternateReverse => "alternate-reverse",
    Normal => "normal",
    Reverse => "reverse",
});

define_css_enum!(RubyPositionKeyword (props::RubyPosition) {
    Alternate => "alternate",
    InterCharacter => "inter-character",
    Over => "over",
    Under => "under",
});

define_css_enum!(ScrollSnapStopKeyword (props::ScrollSnapStop) {
    Always => "always",
    Normal => "normal",
});

define_css_enum!(JustifySelfKeyword (props::JustifySelf) {
    AnchorCenter => "anchor-center",
    Auto => "auto",
    Baseline => "baseline",
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Left => "left",
    Normal => "normal",
    Right => "right",
    SelfEnd => "self-end",
    SelfStart => "self-start",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(AlignSelfKeyword (props::AlignSelf, props::PlaceSelf) {
    AnchorCenter => "anchor-center",
    Auto => "auto",
    Baseline => "baseline",
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Normal => "normal",
    SelfEnd => "self-end",
    SelfStart => "self-start",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(JustifyItemsKeyword (props::JustifyItems) {
    AnchorCenter => "anchor-center",
    Baseline => "baseline",
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Left => "left",
    Legacy => "legacy",
    Normal => "normal",
    Right => "right",
    SelfEnd => "self-end",
    SelfStart => "self-start",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(AlignItemsKeyword (props::AlignItems, props::PlaceItems) {
    AnchorCenter => "anchor-center",
    Baseline => "baseline",
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Normal => "normal",
    SelfEnd => "self-end",
    SelfStart => "self-start",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(LineBreakKeyword (props::LineBreak) {
    Anywhere => "anywhere",
    Auto => "auto",
    Loose => "loose",
    Normal => "normal",
    Strict => "strict",
});

define_css_enum!(OverflowWrapKeyword (props::OverflowWrap) {
    Anywhere => "anywhere",
    BreakWord => "break-word",
    Normal => "normal",
});

define_css_enum!(StrokeLinejoinKeyword (props::StrokeLinejoin) {
    Arcs => "arcs",
    Bevel => "bevel",
    Miter => "miter",
    MiterClip => "miter-clip",
    Round => "round",
});

define_css_enum!(BreakInsideKeyword (props::BreakInside) {
    Auto => "auto",
    Avoid => "avoid",
    AvoidColumn => "avoid-column",
    AvoidPage => "avoid-page",
    AvoidRegion => "avoid-region",
});

define_css_enum!(ColumnFillKeyword (props::ColumnFill) {
    Auto => "auto",
    Balance => "balance",
});

define_css_enum!(TextWrapKeyword (props::TextWrap) {
    Auto => "auto",
    Balance => "balance",
    Nowrap => "nowrap",
    Pretty => "pretty",
    Stable => "stable",
    Wrap => "wrap",
});

define_css_enum!(TextWrapStyleKeyword (props::TextWrapStyle) {
    Auto => "auto",
    Balance => "balance",
    Pretty => "pretty",
    Stable => "stable",
});

define_css_enum!(CaretKeyword (props::Caret) {
    Auto => "auto",
    Bar => "bar",
    Block => "block",
    Manual => "manual",
    Underscore => "underscore",
});

define_css_enum!(CaretShapeKeyword (props::CaretShape) {
    Auto => "auto",
    Bar => "bar",
    Block => "block",
    Underscore => "underscore",
});

define_css_enum!(TextDecorationKeyword (props::TextDecoration) {
    Auto => "auto",
    Blink => "blink",
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    FromFont => "from-font",
    GrammarError => "grammar-error",
    LineThrough => "line-through",
    None => "none",
    Overline => "overline",
    Solid => "solid",
    SpellingError => "spelling-error",
    Underline => "underline",
    Wavy => "wavy",
});

define_css_enum!(OffsetKeyword (props::Offset) {
    Auto => "auto",
    BorderBox => "border-box",
    Bottom => "bottom",
    Center => "center",
    ContentBox => "content-box",
    FillBox => "fill-box",
    Left => "left",
    None => "none",
    Normal => "normal",
    PaddingBox => "padding-box",
    Right => "right",
    StrokeBox => "stroke-box",
    Top => "top",
    ViewBox => "view-box",
});

define_css_enum!(OffsetPositionKeyword (props::OffsetPosition) {
    Auto => "auto",
    Bottom => "bottom",
    Center => "center",
    Left => "left",
    Normal => "normal",
    Right => "right",
    Top => "top",
});

define_css_enum!(OffsetAnchorKeyword (props::OffsetAnchor) {
    Auto => "auto",
    Bottom => "bottom",
    Center => "center",
    Left => "left",
    Right => "right",
    Top => "top",
});

define_css_enum!(AppearanceKeyword (props::Appearance) {
    Auto => "auto",
    Button => "button",
    Checkbox => "checkbox",
    Listbox => "listbox",
    Menulist => "menulist",
    MenulistButton => "menulist-button",
    Meter => "meter",
    None => "none",
    ProgressBar => "progress-bar",
    Radio => "radio",
    Searchfield => "searchfield",
    Textarea => "textarea",
    Textfield => "textfield",
});

define_css_enum!(TextBoxKeyword (props::TextBox) {
    Auto => "auto",
    Cap => "cap",
    Ex => "ex",
    Ideographic => "ideographic",
    IdeographicInk => "ideographic-ink",
    None => "none",
    Normal => "normal",
    Text => "text",
    TrimBoth => "trim-both",
    TrimEnd => "trim-end",
    TrimStart => "trim-start",
});

define_css_enum!(TextBoxEdgeKeyword (props::TextBoxEdge) {
    Auto => "auto",
    Cap => "cap",
    Ex => "ex",
    Ideographic => "ideographic",
    IdeographicInk => "ideographic-ink",
    Text => "text",
});

define_css_enum!(TextAlignLastKeyword (props::TextAlignLast) {
    Auto => "auto",
    Center => "center",
    End => "end",
    Justify => "justify",
    Left => "left",
    Right => "right",
    Start => "start",
});

define_css_enum!(OverflowKeyword (props::Overflow, props::OverflowBlock, props::OverflowInline, props::OverflowX, props::OverflowY) {
    Auto => "auto",
    Clip => "clip",
    Hidden => "hidden",
    Scroll => "scroll",
    Visible => "visible",
});

define_css_enum!(WebkitMaskSizeKeyword (props::WebkitMaskSize, props::BackgroundSize, props::MaskSize) {
    Auto => "auto",
    Contain => "contain",
    Cover => "cover",
});

define_css_enum!(TimelineTriggerExitRangeKeyword (props::TimelineTriggerExitRange, props::TimelineTriggerExitRangeEnd, props::TimelineTriggerExitRangeStart) {
    Auto => "auto",
    Contain => "contain",
    Cover => "cover",
    Entry => "entry",
    EntryCrossing => "entry-crossing",
    Exit => "exit",
    ExitCrossing => "exit-crossing",
    Normal => "normal",
});

define_css_enum!(OverscrollBehaviorKeyword (props::OverscrollBehavior, props::OverscrollBehaviorBlock, props::OverscrollBehaviorInline, props::OverscrollBehaviorX, props::OverscrollBehaviorY) {
    Auto => "auto",
    Contain => "contain",
    None => "none",
});

define_css_enum!(FlexBasisKeyword (props::FlexBasis) {
    Auto => "auto",
    Content => "content",
    FitContent => "fit-content",
    MaxContent => "max-content",
    MinContent => "min-content",
});

define_css_enum!(FlexKeyword (props::Flex) {
    Auto => "auto",
    Content => "content",
    FitContent => "fit-content",
    MaxContent => "max-content",
    MinContent => "min-content",
    None => "none",
});

define_css_enum!(WillChangeKeyword (props::WillChange) {
    Auto => "auto",
    Contents => "contents",
    ScrollPosition => "scroll-position",
});

define_css_enum!(ImageRenderingKeyword (props::ImageRendering) {
    Auto => "auto",
    CrispEdges => "crisp-edges",
    Pixelated => "pixelated",
    Smooth => "smooth",
});

define_css_enum!(ShapeRenderingKeyword (props::ShapeRendering) {
    Auto => "auto",
    CrispEdges => "crispEdges",
    GeometricPrecision => "geometricPrecision",
    OptimizeSpeed => "optimizeSpeed",
});

define_css_enum!(OutlineKeyword (props::Outline) {
    Auto => "auto",
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    Groove => "groove",
    Inset => "inset",
    Medium => "medium",
    None => "none",
    Outset => "outset",
    Ridge => "ridge",
    Solid => "solid",
    Thick => "thick",
    Thin => "thin",
});

define_css_enum!(OutlineStyleKeyword (props::OutlineStyle) {
    Auto => "auto",
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    Groove => "groove",
    Inset => "inset",
    None => "none",
    Outset => "outset",
    Ridge => "ridge",
    Solid => "solid",
});

define_css_enum!(BaselineSourceKeyword (props::BaselineSource) {
    Auto => "auto",
    First => "first",
    Last => "last",
});

define_css_enum!(BlockSizeKeyword (props::BlockSize, props::Height, props::InlineSize, props::MinBlockSize, props::MinHeight, props::MinInlineSize, props::MinWidth, props::Width) {
    Auto => "auto",
    FitContent => "fit-content",
    MaxContent => "max-content",
    MinContent => "min-content",
});

define_css_enum!(TableLayoutKeyword (props::TableLayout) {
    Auto => "auto",
    Fixed => "fixed",
});

define_css_enum!(TextDecorationThicknessKeyword (props::TextDecorationThickness) {
    Auto => "auto",
    FromFont => "from-font",
});

define_css_enum!(TextUnderlinePositionKeyword (props::TextUnderlinePosition) {
    Auto => "auto",
    FromFont => "from-font",
    Left => "left",
    Right => "right",
    Under => "under",
});

define_css_enum!(TextRenderingKeyword (props::TextRendering) {
    Auto => "auto",
    GeometricPrecision => "geometricPrecision",
    OptimizeLegibility => "optimizeLegibility",
    OptimizeSpeed => "optimizeSpeed",
});

define_css_enum!(ContentVisibilityKeyword (props::ContentVisibility) {
    Auto => "auto",
    Hidden => "hidden",
    Visible => "visible",
});

define_css_enum!(InteractivityKeyword (props::Interactivity) {
    Auto => "auto",
    Inert => "inert",
});

define_css_enum!(TextJustifyKeyword (props::TextJustify) {
    Auto => "auto",
    InterCharacter => "inter-character",
    InterWord => "inter-word",
    None => "none",
});

define_css_enum!(IsolationKeyword (props::Isolation) {
    Auto => "auto",
    Isolate => "isolate",
});

define_css_enum!(ColorInterpolationFiltersKeyword (props::ColorInterpolationFilters) {
    Auto => "auto",
    LinearRgb => "linearRGB",
    SRgb => "sRGB",
});

define_css_enum!(TouchActionKeyword (props::TouchAction) {
    Auto => "auto",
    Manipulation => "manipulation",
    None => "none",
    PanDown => "pan-down",
    PanLeft => "pan-left",
    PanRight => "pan-right",
    PanUp => "pan-up",
    PanX => "pan-x",
    PanY => "pan-y",
    PinchZoom => "pinch-zoom",
});

define_css_enum!(CaretAnimationKeyword (props::CaretAnimation) {
    Auto => "auto",
    Manual => "manual",
});

define_css_enum!(HyphensKeyword (props::Hyphens) {
    Auto => "auto",
    Manual => "manual",
    None => "none",
});

define_css_enum!(GridAutoColumnsKeyword (props::GridAutoColumns, props::GridAutoRows) {
    Auto => "auto",
    MaxContent => "max-content",
    MinContent => "min-content",
});

define_css_enum!(GridTemplateColumnsKeyword (props::GridTemplateColumns, props::GridTemplateRows) {
    Auto => "auto",
    MaxContent => "max-content",
    MinContent => "min-content",
    None => "none",
    Subgrid => "subgrid",
});

define_css_enum!(FontOpticalSizingKeyword (props::FontOpticalSizing, props::FontSynthesisSmallCaps, props::FontSynthesisStyle, props::FontSynthesisWeight, props::OverflowAnchor, props::Quotes, props::RubyOverhang, props::ScrollTargetGroup, props::TimelineTriggerSource) {
    Auto => "auto",
    None => "none",
});

define_css_enum!(FontKerningKeyword (props::FontKerning) {
    Auto => "auto",
    None => "none",
    Normal => "normal",
});

define_css_enum!(ForcedColorAdjustKeyword (props::ForcedColorAdjust) {
    Auto => "auto",
    None => "none",
    PreserveParentColor => "preserve-parent-color",
});

define_css_enum!(ScrollbarWidthKeyword (props::ScrollbarWidth) {
    Auto => "auto",
    None => "none",
    Thin => "thin",
});

define_css_enum!(TextAutospaceKeyword (props::TextAutospace) {
    Auto => "auto",
    Normal => "normal",
});

define_css_enum!(ColumnWrapKeyword (props::ColumnWrap) {
    Auto => "auto",
    Nowrap => "nowrap",
    Wrap => "wrap",
});

define_css_enum!(TextEmphasisPositionKeyword (props::TextEmphasisPosition) {
    Auto => "auto",
    Over => "over",
    Under => "under",
});

define_css_enum!(OffsetRotateKeyword (props::OffsetRotate) {
    Auto => "auto",
    Reverse => "reverse",
});

define_css_enum!(ScrollBehaviorKeyword (props::ScrollBehavior) {
    Auto => "auto",
    Smooth => "smooth",
});

define_css_enum!(ScrollbarGutterKeyword (props::ScrollbarGutter) {
    Auto => "auto",
    Stable => "stable",
});

define_css_enum!(WebkitOverflowScrollingKeyword (props::WebkitOverflowScrolling) {
    Auto => "auto",
    Touch => "touch",
});

define_css_enum!(MathDepthKeyword (props::MathDepth) {
    AutoAdd => "auto-add",
});

define_css_enum!(WordBreakKeyword (props::WordBreak) {
    AutoPhrase => "auto-phrase",
    BreakAll => "break-all",
    BreakWord => "break-word",
    KeepAll => "keep-all",
    Normal => "normal",
});

define_css_enum!(AnimationFillModeKeyword (props::AnimationFillMode) {
    Backwards => "backwards",
    Both => "both",
    Forwards => "forwards",
    None => "none",
});

define_css_enum!(VerticalAlignKeyword (props::VerticalAlign) {
    Baseline => "baseline",
    Bottom => "bottom",
    Middle => "middle",
    Sub => "sub",
    Super_ => "super",
    TextBottom => "text-bottom",
    TextTop => "text-top",
    Top => "top",
});

define_css_enum!(AlignContentKeyword (props::AlignContent, props::PlaceContent) {
    Baseline => "baseline",
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Normal => "normal",
    SpaceAround => "space-around",
    SpaceBetween => "space-between",
    SpaceEvenly => "space-evenly",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(BaselineShiftKeyword (props::BaselineShift) {
    Baseline => "baseline",
    Sub => "sub",
    Super_ => "super",
});

define_css_enum!(CornerBlockEndShapeKeyword (props::CornerBlockEndShape, props::CornerBlockStartShape, props::CornerBottomLeftShape, props::CornerBottomRightShape, props::CornerBottomShape, props::CornerEndEndShape, props::CornerEndStartShape, props::CornerInlineEndShape, props::CornerInlineStartShape, props::CornerLeftShape, props::CornerRightShape, props::CornerShape, props::CornerStartEndShape, props::CornerStartStartShape, props::CornerTopLeftShape, props::CornerTopRightShape, props::CornerTopShape) {
    Bevel => "bevel",
    Notch => "notch",
    Round => "round",
    Scoop => "scoop",
    Square => "square",
    Squircle => "squircle",
});

define_css_enum!(UnicodeBidiKeyword (props::UnicodeBidi) {
    BidiOverride => "bidi-override",
    Embed => "embed",
    Isolate => "isolate",
    IsolateOverride => "isolate-override",
    Normal => "normal",
    Plaintext => "plaintext",
});

define_css_enum!(TextDecorationLineKeyword (props::TextDecorationLine) {
    Blink => "blink",
    GrammarError => "grammar-error",
    LineThrough => "line-through",
    None => "none",
    Overline => "overline",
    SpellingError => "spelling-error",
    Underline => "underline",
});

define_css_enum!(ResizeKeyword (props::Resize) {
    Block => "block",
    Both => "both",
    Horizontal => "horizontal",
    Inline => "inline",
    None => "none",
    Vertical => "vertical",
});

define_css_enum!(ScrollSnapTypeKeyword (props::ScrollSnapType) {
    Block => "block",
    Both => "both",
    Inline => "inline",
    None => "none",
    X => "x",
    Y => "y",
});

define_css_enum!(DisplayKeyword (props::Display) {
    Block => "block",
    Contents => "contents",
    Flex => "flex",
    Flow => "flow",
    FlowRoot => "flow-root",
    Grid => "grid",
    Inline => "inline",
    InlineBlock => "inline-block",
    InlineFlex => "inline-flex",
    InlineGrid => "inline-grid",
    InlineListItem => "inline-list-item",
    InlineTable => "inline-table",
    ListItem => "list-item",
    None => "none",
    Ruby => "ruby",
    RubyBase => "ruby-base",
    RubyBaseContainer => "ruby-base-container",
    RubyText => "ruby-text",
    RubyTextContainer => "ruby-text-container",
    RunIn => "run-in",
    Table => "table",
    TableCaption => "table-caption",
    TableCell => "table-cell",
    TableColumn => "table-column",
    TableColumnGroup => "table-column-group",
    TableFooterGroup => "table-footer-group",
    TableHeaderGroup => "table-header-group",
    TableRow => "table-row",
    TableRowGroup => "table-row-group",
});

define_css_enum!(MozOrientKeyword (props::MozOrient) {
    Block => "block",
    Horizontal => "horizontal",
    Inline => "inline",
    Vertical => "vertical",
});

define_css_enum!(FontWeightKeyword (props::FontWeight) {
    Bold => "bold",
    Bolder => "bolder",
    Lighter => "lighter",
    Normal => "normal",
});

define_css_enum!(WebkitMaskKeyword (props::WebkitMask) {
    Border => "border",
    BorderBox => "border-box",
    Bottom => "bottom",
    Center => "center",
    Content => "content",
    ContentBox => "content-box",
    Left => "left",
    NoRepeat => "no-repeat",
    None => "none",
    Padding => "padding",
    PaddingBox => "padding-box",
    Repeat => "repeat",
    RepeatX => "repeat-x",
    RepeatY => "repeat-y",
    Right => "right",
    Round => "round",
    Space => "space",
    Text => "text",
    Top => "top",
});

define_css_enum!(WebkitMaskClipKeyword (props::WebkitMaskClip) {
    Border => "border",
    BorderBox => "border-box",
    Content => "content",
    ContentBox => "content-box",
    FillBox => "fill-box",
    NoClip => "no-clip",
    Padding => "padding",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    Text => "text",
    ViewBox => "view-box",
});

define_css_enum!(WebkitMaskOriginKeyword (props::WebkitMaskOrigin) {
    Border => "border",
    BorderBox => "border-box",
    Content => "content",
    ContentBox => "content-box",
    FillBox => "fill-box",
    Padding => "padding",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(BackgroundClipKeyword (props::BackgroundClip) {
    BorderArea => "border-area",
    BorderBox => "border-box",
    ContentBox => "content-box",
    PaddingBox => "padding-box",
    Text => "text",
});

define_css_enum!(BackgroundKeyword (props::Background) {
    BorderBox => "border-box",
    Bottom => "bottom",
    Center => "center",
    ContentBox => "content-box",
    Fixed => "fixed",
    Left => "left",
    Local => "local",
    NoRepeat => "no-repeat",
    None => "none",
    PaddingBox => "padding-box",
    Repeat => "repeat",
    RepeatX => "repeat-x",
    RepeatY => "repeat-y",
    Right => "right",
    Round => "round",
    Scroll => "scroll",
    Space => "space",
    Top => "top",
});

define_css_enum!(BoxSizingKeyword (props::BoxSizing) {
    BorderBox => "border-box",
    ContentBox => "content-box",
});

define_css_enum!(ClipPathKeyword (props::ClipPath) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    FillBox => "fill-box",
    MarginBox => "margin-box",
    None => "none",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(MaskClipKeyword (props::MaskClip) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    FillBox => "fill-box",
    NoClip => "no-clip",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(OffsetPathKeyword (props::OffsetPath) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    FillBox => "fill-box",
    None => "none",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(MaskOriginKeyword (props::MaskOrigin) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    FillBox => "fill-box",
    PaddingBox => "padding-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(TransformBoxKeyword (props::TransformBox) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    FillBox => "fill-box",
    StrokeBox => "stroke-box",
    ViewBox => "view-box",
});

define_css_enum!(ShapeOutsideKeyword (props::ShapeOutside) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    MarginBox => "margin-box",
    None => "none",
    PaddingBox => "padding-box",
});

define_css_enum!(BackgroundOriginKeyword (props::BackgroundOrigin, props::OverflowClipMargin) {
    BorderBox => "border-box",
    ContentBox => "content-box",
    PaddingBox => "padding-box",
});

define_css_enum!(ClearKeyword (props::Clear) {
    Both => "both",
    InlineEnd => "inline-end",
    InlineStart => "inline-start",
    Left => "left",
    None => "none",
    Right => "right",
});

define_css_enum!(WebkitMaskPositionKeyword (props::WebkitMaskPosition, props::BackgroundPosition, props::MaskPosition, props::ObjectPosition, props::PerspectiveOrigin, props::TransformOrigin) {
    Bottom => "bottom",
    Center => "center",
    Left => "left",
    Right => "right",
    Top => "top",
});

define_css_enum!(BackgroundPositionYKeyword (props::BackgroundPositionY) {
    Bottom => "bottom",
    Center => "center",
    Top => "top",
    YEnd => "y-end",
    YStart => "y-start",
});

define_css_enum!(CaptionSideKeyword (props::CaptionSide) {
    Bottom => "bottom",
    Top => "top",
});

define_css_enum!(WhiteSpaceKeyword (props::WhiteSpace) {
    BreakSpaces => "break-spaces",
    Collapse => "collapse",
    Normal => "normal",
    Nowrap => "nowrap",
    Pre => "pre",
    PreLine => "pre-line",
    PreWrap => "pre-wrap",
    Preserve => "preserve",
    PreserveBreaks => "preserve-breaks",
    PreserveSpaces => "preserve-spaces",
    Wrap => "wrap",
});

define_css_enum!(WhiteSpaceCollapseKeyword (props::WhiteSpaceCollapse) {
    BreakSpaces => "break-spaces",
    Collapse => "collapse",
    Preserve => "preserve",
    PreserveBreaks => "preserve-breaks",
    PreserveSpaces => "preserve-spaces",
});

define_css_enum!(WordWrapKeyword (props::WordWrap) {
    BreakWord => "break-word",
    Normal => "normal",
});

define_css_enum!(StrokeLinecapKeyword (props::StrokeLinecap) {
    Butt => "butt",
    Round => "round",
    Square => "square",
});

define_css_enum!(MozAppearanceKeyword (props::MozAppearance) {
    Button => "button",
    ButtonArrowDown => "button-arrow-down",
    ButtonArrowNext => "button-arrow-next",
    ButtonArrowPrevious => "button-arrow-previous",
    ButtonArrowUp => "button-arrow-up",
    ButtonBevel => "button-bevel",
    ButtonFocus => "button-focus",
    Caret => "caret",
    Checkbox => "checkbox",
    CheckboxContainer => "checkbox-container",
    CheckboxLabel => "checkbox-label",
    Checkmenuitem => "checkmenuitem",
    Dualbutton => "dualbutton",
    Groupbox => "groupbox",
    Listbox => "listbox",
    Listitem => "listitem",
    Menuarrow => "menuarrow",
    Menubar => "menubar",
    Menucheckbox => "menucheckbox",
    Menuimage => "menuimage",
    Menuitem => "menuitem",
    Menuitemtext => "menuitemtext",
    Menulist => "menulist",
    MenulistButton => "menulist-button",
    MenulistText => "menulist-text",
    MenulistTextfield => "menulist-textfield",
    Menupopup => "menupopup",
    Menuradio => "menuradio",
    Menuseparator => "menuseparator",
    Meterbar => "meterbar",
    Meterchunk => "meterchunk",
    None => "none",
    Progressbar => "progressbar",
    ProgressbarVertical => "progressbar-vertical",
    Progresschunk => "progresschunk",
    ProgresschunkVertical => "progresschunk-vertical",
    Radio => "radio",
    RadioContainer => "radio-container",
    RadioLabel => "radio-label",
    Radiomenuitem => "radiomenuitem",
    Range => "range",
    RangeThumb => "range-thumb",
    Resizer => "resizer",
    Resizerpanel => "resizerpanel",
    ScaleHorizontal => "scale-horizontal",
    ScaleVertical => "scale-vertical",
    ScalethumbHorizontal => "scalethumb-horizontal",
    ScalethumbVertical => "scalethumb-vertical",
    Scalethumbend => "scalethumbend",
    Scalethumbstart => "scalethumbstart",
    Scalethumbtick => "scalethumbtick",
    ScrollbarbuttonDown => "scrollbarbutton-down",
    ScrollbarbuttonLeft => "scrollbarbutton-left",
    ScrollbarbuttonRight => "scrollbarbutton-right",
    ScrollbarbuttonUp => "scrollbarbutton-up",
    ScrollbarthumbHorizontal => "scrollbarthumb-horizontal",
    ScrollbarthumbVertical => "scrollbarthumb-vertical",
    ScrollbartrackHorizontal => "scrollbartrack-horizontal",
    ScrollbartrackVertical => "scrollbartrack-vertical",
    Searchfield => "searchfield",
    Separator => "separator",
    Sheet => "sheet",
    Spinner => "spinner",
    SpinnerDownbutton => "spinner-downbutton",
    SpinnerTextfield => "spinner-textfield",
    SpinnerUpbutton => "spinner-upbutton",
    Splitter => "splitter",
    Statusbar => "statusbar",
    Statusbarpanel => "statusbarpanel",
    Tab => "tab",
    TabScrollArrowBack => "tab-scroll-arrow-back",
    TabScrollArrowForward => "tab-scroll-arrow-forward",
    Tabpanel => "tabpanel",
    Tabpanels => "tabpanels",
    Textfield => "textfield",
    TextfieldMultiline => "textfield-multiline",
    Toolbar => "toolbar",
    Toolbarbutton => "toolbarbutton",
    ToolbarbuttonDropdown => "toolbarbutton-dropdown",
    Toolbargripper => "toolbargripper",
    Toolbox => "toolbox",
    Tooltip => "tooltip",
    Treeheader => "treeheader",
    Treeheadercell => "treeheadercell",
    Treeheadersortarrow => "treeheadersortarrow",
    Treeitem => "treeitem",
    Treeline => "treeline",
    Treetwisty => "treetwisty",
    Treetwistyopen => "treetwistyopen",
    Treeview => "treeview",
});

define_css_enum!(WebkitAppearanceKeyword (props::WebkitAppearance) {
    Button => "button",
    ButtonBevel => "button-bevel",
    Caret => "caret",
    Checkbox => "checkbox",
    DefaultButton => "default-button",
    InnerSpinButton => "inner-spin-button",
    Listbox => "listbox",
    Listitem => "listitem",
    MediaControlsBackground => "media-controls-background",
    MediaControlsFullscreenBackground => "media-controls-fullscreen-background",
    MediaCurrentTimeDisplay => "media-current-time-display",
    MediaEnterFullscreenButton => "media-enter-fullscreen-button",
    MediaExitFullscreenButton => "media-exit-fullscreen-button",
    MediaFullscreenButton => "media-fullscreen-button",
    MediaMuteButton => "media-mute-button",
    MediaOverlayPlayButton => "media-overlay-play-button",
    MediaPlayButton => "media-play-button",
    MediaSeekBackButton => "media-seek-back-button",
    MediaSeekForwardButton => "media-seek-forward-button",
    MediaSlider => "media-slider",
    MediaSliderthumb => "media-sliderthumb",
    MediaTimeRemainingDisplay => "media-time-remaining-display",
    MediaToggleClosedCaptionsButton => "media-toggle-closed-captions-button",
    MediaVolumeSlider => "media-volume-slider",
    MediaVolumeSliderContainer => "media-volume-slider-container",
    MediaVolumeSliderthumb => "media-volume-sliderthumb",
    Menulist => "menulist",
    MenulistButton => "menulist-button",
    MenulistText => "menulist-text",
    MenulistTextfield => "menulist-textfield",
    Meter => "meter",
    None => "none",
    ProgressBar => "progress-bar",
    ProgressBarValue => "progress-bar-value",
    PushButton => "push-button",
    Radio => "radio",
    Searchfield => "searchfield",
    SearchfieldCancelButton => "searchfield-cancel-button",
    SearchfieldDecoration => "searchfield-decoration",
    SearchfieldResultsButton => "searchfield-results-button",
    SearchfieldResultsDecoration => "searchfield-results-decoration",
    SliderHorizontal => "slider-horizontal",
    SliderVertical => "slider-vertical",
    SliderthumbHorizontal => "sliderthumb-horizontal",
    SliderthumbVertical => "sliderthumb-vertical",
    SquareButton => "square-button",
    Textarea => "textarea",
    Textfield => "textfield",
});

define_css_enum!(TextTransformKeyword (props::TextTransform) {
    Capitalize => "capitalize",
    FullSizeKana => "full-size-kana",
    FullWidth => "full-width",
    Lowercase => "lowercase",
    MathAuto => "math-auto",
    None => "none",
    Uppercase => "uppercase",
});

define_css_enum!(FontKeyword (props::Font) {
    Caption => "caption",
    Icon => "icon",
    Menu => "menu",
    MessageBox => "message-box",
    SmallCaption => "small-caption",
    StatusBar => "status-bar",
});

define_css_enum!(JustifyContentKeyword (props::JustifyContent) {
    Center => "center",
    End => "end",
    FlexEnd => "flex-end",
    FlexStart => "flex-start",
    Left => "left",
    Normal => "normal",
    Right => "right",
    SpaceAround => "space-around",
    SpaceBetween => "space-between",
    SpaceEvenly => "space-evenly",
    Start => "start",
    Stretch => "stretch",
});

define_css_enum!(TextAlignKeyword (props::TextAlign) {
    Center => "center",
    End => "end",
    Justify => "justify",
    Left => "left",
    MatchParent => "match-parent",
    Right => "right",
    Start => "start",
});

define_css_enum!(ScrollSnapAlignKeyword (props::ScrollSnapAlign) {
    Center => "center",
    End => "end",
    None => "none",
    Start => "start",
});

define_css_enum!(BackgroundPositionXKeyword (props::BackgroundPositionX) {
    Center => "center",
    Left => "left",
    Right => "right",
    XEnd => "x-end",
    XStart => "x-start",
});

define_css_enum!(RubyAlignKeyword (props::RubyAlign) {
    Center => "center",
    SpaceAround => "space-around",
    SpaceBetween => "space-between",
    Start => "start",
});

define_css_enum!(TextEmphasisKeyword (props::TextEmphasis, props::TextEmphasisStyle) {
    Circle => "circle",
    Dot => "dot",
    DoubleCircle => "double-circle",
    Filled => "filled",
    None => "none",
    Open => "open",
    Sesame => "sesame",
    Triangle => "triangle",
});

define_css_enum!(WebkitMaskCompositeKeyword (props::WebkitMaskComposite) {
    Clear => "clear",
    Copy => "copy",
    DestinationAtop => "destination-atop",
    DestinationIn => "destination-in",
    DestinationOut => "destination-out",
    DestinationOver => "destination-over",
    SourceAtop => "source-atop",
    SourceIn => "source-in",
    SourceOut => "source-out",
    SourceOver => "source-over",
    Xor => "xor",
});

define_css_enum!(TextOverflowKeyword (props::TextOverflow) {
    Clip => "clip",
    Ellipsis => "ellipsis",
});

define_css_enum!(BoxDecorationBreakKeyword (props::BoxDecorationBreak) {
    Clone => "clone",
    Slice => "slice",
});

define_css_enum!(ContentKeyword (props::Content) {
    CloseQuote => "close-quote",
    NoCloseQuote => "no-close-quote",
    NoOpenQuote => "no-open-quote",
    None => "none",
    Normal => "normal",
    OpenQuote => "open-quote",
});

define_css_enum!(VisibilityKeyword (props::Visibility) {
    Collapse => "collapse",
    Hidden => "hidden",
    Visible => "visible",
});

define_css_enum!(BorderCollapseKeyword (props::BorderCollapse) {
    Collapse => "collapse",
    Separate => "separate",
});

define_css_enum!(MixBlendModeKeyword (props::MixBlendMode) {
    Color => "color",
    ColorBurn => "color-burn",
    ColorDodge => "color-dodge",
    Darken => "darken",
    Difference => "difference",
    Exclusion => "exclusion",
    HardLight => "hard-light",
    Hue => "hue",
    Lighten => "lighten",
    Luminosity => "luminosity",
    Multiply => "multiply",
    Normal => "normal",
    Overlay => "overlay",
    PlusDarker => "plus-darker",
    PlusLighter => "plus-lighter",
    Saturation => "saturation",
    Screen => "screen",
    SoftLight => "soft-light",
});

define_css_enum!(BackgroundBlendModeKeyword (props::BackgroundBlendMode) {
    Color => "color",
    ColorBurn => "color-burn",
    ColorDodge => "color-dodge",
    Darken => "darken",
    Difference => "difference",
    Exclusion => "exclusion",
    HardLight => "hard-light",
    Hue => "hue",
    Lighten => "lighten",
    Luminosity => "luminosity",
    Multiply => "multiply",
    Normal => "normal",
    Overlay => "overlay",
    Saturation => "saturation",
    Screen => "screen",
    SoftLight => "soft-light",
});

define_css_enum!(FlexFlowKeyword (props::FlexFlow) {
    Column => "column",
    ColumnReverse => "column-reverse",
    Nowrap => "nowrap",
    Row => "row",
    RowReverse => "row-reverse",
    Wrap => "wrap",
    WrapReverse => "wrap-reverse",
});

define_css_enum!(FlexDirectionKeyword (props::FlexDirection) {
    Column => "column",
    ColumnReverse => "column-reverse",
    Row => "row",
    RowReverse => "row-reverse",
});

define_css_enum!(GridAutoFlowKeyword (props::GridAutoFlow) {
    Column => "column",
    Dense => "dense",
    Row => "row",
});

define_css_enum!(FontVariantLigaturesKeyword (props::FontVariantLigatures) {
    CommonLigatures => "common-ligatures",
    Contextual => "ctxual",
    DiscretionaryLigatures => "discretionary-ligatures",
    HistoricalLigatures => "historical-ligatures",
    NoCommonLigatures => "no-common-ligatures",
    NoContextual => "no-ctxual",
    NoDiscretionaryLigatures => "no-discretionary-ligatures",
    NoHistoricalLigatures => "no-historical-ligatures",
    None => "none",
    Normal => "normal",
});

define_css_enum!(MathStyleKeyword (props::MathStyle) {
    Compact => "compact",
    Normal => "normal",
});

define_css_enum!(DynamicRangeLimitKeyword (props::DynamicRangeLimit) {
    Constrained => "constrained",
    NoLimit => "no-limit",
    Standard => "standard",
});

define_css_enum!(TimelineTriggerRangeKeyword (props::TimelineTriggerRange, props::TimelineTriggerRangeEnd, props::TimelineTriggerRangeStart) {
    Contain => "contain",
    Cover => "cover",
    Entry => "entry",
    EntryCrossing => "entry-crossing",
    Exit => "exit",
    ExitCrossing => "exit-crossing",
    Normal => "normal",
});

define_css_enum!(ObjectFitKeyword (props::ObjectFit) {
    Contain => "contain",
    Cover => "cover",
    Fill => "fill",
    None => "none",
    ScaleDown => "scale-down",
});

define_css_enum!(ContainKeyword (props::Contain) {
    Content => "content",
    InlineSize => "inline-size",
    Layout => "layout",
    None => "none",
    Paint => "paint",
    Size => "size",
    Strict => "strict",
    Style => "style",
});

define_css_enum!(FillKeyword (props::Fill, props::Stroke) {
    ContextFill => "ctx-fill",
    ContextStroke => "ctx-stroke",
    None => "none",
});

define_css_enum!(FontFamilyKeyword (props::FontFamily) {
    Cursive => "cursive",
    Emoji => "emoji",
    Fangsong => "fangsong",
    Fantasy => "fantasy",
    Math => "math",
    Monospace => "monospace",
    SansSerif => "sans-serif",
    Serif => "serif",
    SystemUi => "system-ui",
    UiMonospace => "ui-monospace",
    UiRounded => "ui-rounded",
    UiSansSerif => "ui-sans-serif",
    UiSerif => "ui-serif",
});

define_css_enum!(ColorSchemeKeyword (props::ColorScheme, props::FontPalette) {
    Dark => "dark",
    Light => "light",
    Normal => "normal",
});

define_css_enum!(BorderKeyword (props::Border, props::BorderBlock, props::BorderBlockEnd, props::BorderBlockStart, props::BorderBottom, props::BorderInline, props::BorderInlineEnd, props::BorderInlineStart, props::BorderLeft, props::BorderRight, props::BorderTop, props::ColumnRule) {
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    Groove => "groove",
    Hidden => "hidden",
    Inset => "inset",
    Medium => "medium",
    None => "none",
    Outset => "outset",
    Ridge => "ridge",
    Solid => "solid",
    Thick => "thick",
    Thin => "thin",
});

define_css_enum!(BorderBlockEndStyleKeyword (props::BorderBlockEndStyle, props::BorderBlockStartStyle, props::BorderBlockStyle, props::BorderBottomStyle, props::BorderInlineEndStyle, props::BorderInlineStartStyle, props::BorderInlineStyle, props::BorderLeftStyle, props::BorderRightStyle, props::BorderStyle, props::BorderTopStyle, props::ColumnRuleStyle) {
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    Groove => "groove",
    Hidden => "hidden",
    Inset => "inset",
    None => "none",
    Outset => "outset",
    Ridge => "ridge",
    Solid => "solid",
});

define_css_enum!(TextDecorationStyleKeyword (props::TextDecorationStyle) {
    Dashed => "dashed",
    Dotted => "dotted",
    Double => "double",
    Solid => "solid",
    Wavy => "wavy",
});

define_css_enum!(WebkitTouchCalloutKeyword (props::WebkitTouchCallout) {
    Default_ => "default",
    None => "none",
});

define_css_enum!(FontVariantNumericKeyword (props::FontVariantNumeric) {
    DiagonalFractions => "diagonal-fractions",
    LiningNums => "lining-nums",
    Normal => "normal",
    OldstyleNums => "oldstyle-nums",
    Ordinal => "ordinal",
    ProportionalNums => "proportional-nums",
    SlashedZero => "slashed-zero",
    StackedFractions => "stacked-fractions",
    TabularNums => "tabular-nums",
});

define_css_enum!(AnimationTimingFunctionKeyword (props::AnimationTimingFunction, props::TransitionTimingFunction) {
    Ease => "ease",
    EaseIn => "ease-in",
    EaseInOut => "ease-in-out",
    EaseOut => "ease-out",
    Linear => "linear",
    StepEnd => "step-end",
    StepStart => "step-start",
});

define_css_enum!(PrintColorAdjustKeyword (props::PrintColorAdjust) {
    Economy => "economy",
    Exact => "exact",
});

define_css_enum!(FontVariantEmojiKeyword (props::FontVariantEmoji) {
    Emoji => "emoji",
    Normal => "normal",
    Text => "text",
    Unicode => "unicode",
});

define_css_enum!(TextAnchorKeyword (props::TextAnchor) {
    End => "end",
    Middle => "middle",
    Start => "start",
});

define_css_enum!(ClipRuleKeyword (props::ClipRule, props::FillRule) {
    Evenodd => "evenodd",
    Nonzero => "nonzero",
});

define_css_enum!(MozContextPropertiesKeyword (props::MozContextProperties) {
    Fill => "fill",
    FillOpacity => "fill-opacity",
    None => "none",
    Stroke => "stroke",
    StrokeOpacity => "stroke-opacity",
});

define_css_enum!(PaintOrderKeyword (props::PaintOrder) {
    Fill => "fill",
    Markers => "markers",
    Normal => "normal",
    Stroke => "stroke",
});

define_css_enum!(MaxBlockSizeKeyword (props::MaxBlockSize, props::MaxHeight, props::MaxInlineSize, props::MaxWidth) {
    FitContent => "fit-content",
    MaxContent => "max-content",
    MinContent => "min-content",
    None => "none",
});

define_css_enum!(BackgroundAttachmentKeyword (props::BackgroundAttachment) {
    Fixed => "fixed",
    Local => "local",
    Scroll => "scroll",
});

define_css_enum!(VectorEffectKeyword (props::VectorEffect) {
    FixedPosition => "fixed-position",
    NonRotation => "non-rotation",
    NonScalingSize => "non-scaling-size",
    NonScalingStroke => "non-scaling-stroke",
    None => "none",
});

define_css_enum!(TransformStyleKeyword (props::TransformStyle) {
    Flat => "flat",
    Preserve3d => "preserve-3d",
});

define_css_enum!(ReadingFlowKeyword (props::ReadingFlow) {
    FlexFlow => "flex-flow",
    FlexVisual => "flex-visual",
    GridColumns => "grid-columns",
    GridOrder => "grid-order",
    GridRows => "grid-rows",
    Normal => "normal",
    SourceOrder => "source-order",
});

define_css_enum!(ImageOrientationKeyword (props::ImageOrientation) {
    Flip => "flip",
    FromImage => "from-image",
});

define_css_enum!(FontSizeAdjustKeyword (props::FontSizeAdjust) {
    FromFont => "from-font",
    None => "none",
});

define_css_enum!(FontVariantEastAsianKeyword (props::FontVariantEastAsian) {
    FullWidth => "full-width",
    Jis04 => "jis04",
    Jis78 => "jis78",
    Jis83 => "jis83",
    Jis90 => "jis90",
    Normal => "normal",
    ProportionalWidth => "proportional-width",
    Ruby => "ruby",
    Simplified => "simplified",
    Traditional => "traditional",
});

define_css_enum!(BackfaceVisibilityKeyword (props::BackfaceVisibility) {
    Hidden => "hidden",
    Visible => "visible",
});

define_css_enum!(EmptyCellsKeyword (props::EmptyCells) {
    Hide => "hide",
    Show => "show",
});

define_css_enum!(FontVariantAlternatesKeyword (props::FontVariantAlternates) {
    HistoricalForms => "historical-forms",
    Normal => "normal",
});

define_css_enum!(WritingModeKeyword (props::WritingMode) {
    HorizontalTb => "horizontal-tb",
    SidewaysLr => "sideways-lr",
    SidewaysRl => "sideways-rl",
    VerticalLr => "vertical-lr",
    VerticalRl => "vertical-rl",
});

define_css_enum!(AnimationIterationCountKeyword (props::AnimationIterationCount) {
    Infinite => "infinite",
});

define_css_enum!(AllKeyword (props::All) {
    Inherit => "inherit",
    Initial => "initial",
    Revert => "revert",
    RevertLayer => "revert-layer",
    Unset => "unset",
});

define_css_enum!(FloatKeyword (props::Float) {
    InlineEnd => "inline-end",
    InlineStart => "inline-start",
    Left => "left",
    None => "none",
    Right => "right",
});

define_css_enum!(ContainerTypeKeyword (props::ContainerType) {
    InlineSize => "inline-size",
    Normal => "normal",
    ScrollState => "scroll-state",
    Size => "size",
});

define_css_enum!(ListStyleKeyword (props::ListStyle) {
    Inside => "inside",
    None => "none",
    Outside => "outside",
});

define_css_enum!(ListStylePositionKeyword (props::ListStylePosition) {
    Inside => "inside",
    Outside => "outside",
});

define_css_enum!(FontStyleKeyword (props::FontStyle) {
    Italic => "italic",
    Normal => "normal",
    Oblique => "oblique",
});

define_css_enum!(FontSizeKeyword (props::FontSize) {
    Large => "large",
    Larger => "larger",
    Math => "math",
    Medium => "medium",
    Small => "small",
    Smaller => "smaller",
    XLarge => "x-large",
    XSmall => "x-small",
    XxLarge => "xx-large",
    XxSmall => "xx-small",
    XxxLarge => "xxx-large",
});

define_css_enum!(DirectionKeyword (props::Direction) {
    Ltr => "ltr",
    Rtl => "rtl",
});

define_css_enum!(ViewTransitionNameKeyword (props::ViewTransitionName) {
    MatchElement => "match-element",
    None => "none",
});

define_css_enum!(BorderBlockEndWidthKeyword (props::BorderBlockEndWidth, props::BorderBlockStartWidth, props::BorderBlockWidth, props::BorderBottomWidth, props::BorderInlineEndWidth, props::BorderInlineStartWidth, props::BorderInlineWidth, props::BorderLeftWidth, props::BorderRightWidth, props::BorderTopWidth, props::BorderWidth, props::ColumnRuleWidth, props::OutlineWidth) {
    Medium => "medium",
    Thick => "thick",
    Thin => "thin",
});

define_css_enum!(TextOrientationKeyword (props::TextOrientation) {
    Mixed => "mixed",
    Sideways => "sideways",
    Upright => "upright",
});

define_css_enum!(WebkitMaskRepeatKeyword (props::WebkitMaskRepeat, props::BackgroundRepeat, props::MaskRepeat) {
    NoRepeat => "no-repeat",
    Repeat => "repeat",
    RepeatX => "repeat-x",
    RepeatY => "repeat-y",
    Round => "round",
    Space => "space",
});

define_css_enum!(FontSynthesisKeyword (props::FontSynthesis) {
    None => "none",
    Position => "position",
    SmallCaps => "small-caps",
    Style => "style",
    Weight => "weight",
});

define_css_enum!(BorderImageKeyword (props::BorderImage) {
    None => "none",
    Repeat => "repeat",
    Round => "round",
    Space => "space",
    Stretch => "stretch",
});

define_css_enum!(TextBoxTrimKeyword (props::TextBoxTrim) {
    None => "none",
    TrimBoth => "trim-both",
    TrimEnd => "trim-end",
    TrimStart => "trim-start",
});

define_css_enum!(ColumnGapKeyword (props::ColumnGap, props::FontFeatureSettings, props::FontLanguageOverride, props::FontVariationSettings, props::Gap, props::InitialLetter, props::InterestDelay, props::InterestDelayEnd, props::InterestDelayStart, props::LetterSpacing, props::LineHeight, props::RowGap, props::WordSpacing) {
    Normal => "normal",
});

define_css_enum!(ZoomKeyword (props::Zoom) {
    Normal => "normal",
    Reset => "reset",
});

define_css_enum!(FontVariantPositionKeyword (props::FontVariantPosition) {
    Normal => "normal",
    Sub => "sub",
    Super_ => "super",
});

define_css_enum!(TextWrapModeKeyword (props::TextWrapMode) {
    Nowrap => "nowrap",
    Wrap => "wrap",
});

define_css_enum!(FlexWrapKeyword (props::FlexWrap) {
    Nowrap => "nowrap",
    Wrap => "wrap",
    WrapReverse => "wrap-reverse",
});

define_css_enum!(AnimationPlayStateKeyword (props::AnimationPlayState) {
    Paused => "paused",
    Running => "running",
});

define_css_enum!(WebkitUserModifyKeyword (props::WebkitUserModify) {
    ReadOnly => "read-only",
    ReadWrite => "read-write",
    ReadWritePlaintextOnly => "read-write-plaintext-only",
});

define_css_enum!(BorderImageRepeatKeyword (props::BorderImageRepeat, props::MaskBorderRepeat) {
    Repeat => "repeat",
    Round => "round",
    Space => "space",
    Stretch => "stretch",
});

// --- 关键字集合相同的属性共用同一个枚举 ---
pub type BreakBeforeKeyword = BreakAfterKeyword;
pub type UserSelectKeyword = WebkitUserSelectKeyword;
pub type TransitionPropertyKeyword = ColumnSpanKeyword;
pub type TriggerScopeKeyword = ColumnSpanKeyword;
pub type MaskTypeKeyword = MaskBorderModeKeyword;
pub type PlaceSelfKeyword = AlignSelfKeyword;
pub type PlaceItemsKeyword = AlignItemsKeyword;
pub type OverflowBlockKeyword = OverflowKeyword;
pub type OverflowInlineKeyword = OverflowKeyword;
pub type OverflowXKeyword = OverflowKeyword;
pub type OverflowYKeyword = OverflowKeyword;
pub type BackgroundSizeKeyword = WebkitMaskSizeKeyword;
pub type MaskSizeKeyword = WebkitMaskSizeKeyword;
pub type TimelineTriggerExitRangeEndKeyword = TimelineTriggerExitRangeKeyword;
pub type TimelineTriggerExitRangeStartKeyword = TimelineTriggerExitRangeKeyword;
pub type OverscrollBehaviorBlockKeyword = OverscrollBehaviorKeyword;
pub type OverscrollBehaviorInlineKeyword = OverscrollBehaviorKeyword;
pub type OverscrollBehaviorXKeyword = OverscrollBehaviorKeyword;
pub type OverscrollBehaviorYKeyword = OverscrollBehaviorKeyword;
pub type HeightKeyword = BlockSizeKeyword;
pub type InlineSizeKeyword = BlockSizeKeyword;
pub type MinBlockSizeKeyword = BlockSizeKeyword;
pub type MinHeightKeyword = BlockSizeKeyword;
pub type MinInlineSizeKeyword = BlockSizeKeyword;
pub type MinWidthKeyword = BlockSizeKeyword;
pub type WidthKeyword = BlockSizeKeyword;
pub type GridAutoRowsKeyword = GridAutoColumnsKeyword;
pub type GridTemplateRowsKeyword = GridTemplateColumnsKeyword;
pub type FontSynthesisSmallCapsKeyword = FontOpticalSizingKeyword;
pub type FontSynthesisStyleKeyword = FontOpticalSizingKeyword;
pub type FontSynthesisWeightKeyword = FontOpticalSizingKeyword;
pub type OverflowAnchorKeyword = FontOpticalSizingKeyword;
pub type QuotesKeyword = FontOpticalSizingKeyword;
pub type RubyOverhangKeyword = FontOpticalSizingKeyword;
pub type ScrollTargetGroupKeyword = FontOpticalSizingKeyword;
pub type TimelineTriggerSourceKeyword = FontOpticalSizingKeyword;
pub type PlaceContentKeyword = AlignContentKeyword;
pub type CornerBlockStartShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerBottomLeftShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerBottomRightShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerBottomShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerEndEndShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerEndStartShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerInlineEndShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerInlineStartShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerLeftShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerRightShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerStartEndShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerStartStartShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerTopLeftShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerTopRightShapeKeyword = CornerBlockEndShapeKeyword;
pub type CornerTopShapeKeyword = CornerBlockEndShapeKeyword;
pub type OverflowClipMarginKeyword = BackgroundOriginKeyword;
pub type BackgroundPositionKeyword = WebkitMaskPositionKeyword;
pub type MaskPositionKeyword = WebkitMaskPositionKeyword;
pub type ObjectPositionKeyword = WebkitMaskPositionKeyword;
pub type PerspectiveOriginKeyword = WebkitMaskPositionKeyword;
pub type TransformOriginKeyword = WebkitMaskPositionKeyword;
pub type TextEmphasisStyleKeyword = TextEmphasisKeyword;
pub type TimelineTriggerRangeEndKeyword = TimelineTriggerRangeKeyword;
pub type TimelineTriggerRangeStartKeyword = TimelineTriggerRangeKeyword;
pub type StrokeKeyword = FillKeyword;
pub type FontPaletteKeyword = ColorSchemeKeyword;
pub type BorderBlockKeyword = BorderKeyword;
pub type BorderBlockEndKeyword = BorderKeyword;
pub type BorderBlockStartKeyword = BorderKeyword;
pub type BorderBottomKeyword = BorderKeyword;
pub type BorderInlineKeyword = BorderKeyword;
pub type BorderInlineEndKeyword = BorderKeyword;
pub type BorderInlineStartKeyword = BorderKeyword;
pub type BorderLeftKeyword = BorderKeyword;
pub type BorderRightKeyword = BorderKeyword;
pub type BorderTopKeyword = BorderKeyword;
pub type ColumnRuleKeyword = BorderKeyword;
pub type BorderBlockStartStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderBlockStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderBottomStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderInlineEndStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderInlineStartStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderInlineStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderLeftStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderRightStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderStyleKeyword = BorderBlockEndStyleKeyword;
pub type BorderTopStyleKeyword = BorderBlockEndStyleKeyword;
pub type ColumnRuleStyleKeyword = BorderBlockEndStyleKeyword;
pub type TransitionTimingFunctionKeyword = AnimationTimingFunctionKeyword;
pub type FillRuleKeyword = ClipRuleKeyword;
pub type MaxHeightKeyword = MaxBlockSizeKeyword;
pub type MaxInlineSizeKeyword = MaxBlockSizeKeyword;
pub type MaxWidthKeyword = MaxBlockSizeKeyword;
pub type BorderBlockStartWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderBlockWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderBottomWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderInlineEndWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderInlineStartWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderInlineWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderLeftWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderRightWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderTopWidthKeyword = BorderBlockEndWidthKeyword;
pub type BorderWidthKeyword = BorderBlockEndWidthKeyword;
pub type ColumnRuleWidthKeyword = BorderBlockEndWidthKeyword;
pub type OutlineWidthKeyword = BorderBlockEndWidthKeyword;
pub type BackgroundRepeatKeyword = WebkitMaskRepeatKeyword;
pub type MaskRepeatKeyword = WebkitMaskRepeatKeyword;
pub type FontFeatureSettingsKeyword = ColumnGapKeyword;
pub type FontLanguageOverrideKeyword = ColumnGapKeyword;
pub type FontVariationSettingsKeyword = ColumnGapKeyword;
pub type GapKeyword = ColumnGapKeyword;
pub type InitialLetterKeyword = ColumnGapKeyword;
pub type InterestDelayKeyword = ColumnGapKeyword;
pub type InterestDelayEndKeyword = ColumnGapKeyword;
pub type InterestDelayStartKeyword = ColumnGapKeyword;
pub type LetterSpacingKeyword = ColumnGapKeyword;
pub type LineHeightKeyword = ColumnGapKeyword;
pub type RowGapKeyword = ColumnGapKeyword;
pub type WordSpacingKeyword = ColumnGapKeyword;
pub type MaskBorderRepeatKeyword = BorderImageRepeatKeyword;

// --- 全局 `auto` / `none` ---
impl ValidFor<props::Mask> for NoneValue {}
impl ValidFor<props::ScrollMarkerGroup> for NoneValue {}
impl ValidFor<props::Cursor> for Auto {}
impl ValidFor<props::Cursor> for NoneValue {}
impl ValidFor<props::Transition> for NoneValue {}
impl ValidFor<props::BreakAfter> for Auto {}
impl ValidFor<props::BreakBefore> for Auto {}
impl ValidFor<props::PointerEvents> for Auto {}
impl ValidFor<props::PointerEvents> for NoneValue {}
impl ValidFor<props::TextDecorationSkipInk> for Auto {}
impl ValidFor<props::TextDecorationSkipInk> for NoneValue {}
impl ValidFor<props::WebkitUserSelect> for Auto {}
impl ValidFor<props::WebkitUserSelect> for NoneValue {}
impl ValidFor<props::UserSelect> for Auto {}
impl ValidFor<props::UserSelect> for NoneValue {}
impl ValidFor<props::TextCombineUpright> for NoneValue {}
impl ValidFor<props::ColumnSpan> for NoneValue {}
impl ValidFor<props::TransitionProperty> for NoneValue {}
impl ValidFor<props::TriggerScope> for NoneValue {}
impl ValidFor<props::FontVariant> for NoneValue {}
impl ValidFor<props::HangingPunctuation> for NoneValue {}
impl ValidFor<props::MaskBorder> for NoneValue {}
impl ValidFor<props::DominantBaseline> for Auto {}
impl ValidFor<props::Animation> for Auto {}
impl ValidFor<props::Animation> for NoneValue {}
impl ValidFor<props::JustifySelf> for Auto {}
impl ValidFor<props::AlignSelf> for Auto {}
impl ValidFor<props::PlaceSelf> for Auto {}
impl ValidFor<props::LineBreak> for Auto {}
impl ValidFor<props::AccentColor> for Auto {}
impl ValidFor<props::AnimationDuration> for Auto {}
impl ValidFor<props::AspectRatio> for Auto {}
impl ValidFor<props::BorderImageWidth> for Auto {}
impl ValidFor<props::Bottom> for Auto {}
impl ValidFor<props::CaretColor> for Auto {}
impl ValidFor<props::ColumnCount> for Auto {}
impl ValidFor<props::ColumnHeight> for Auto {}
impl ValidFor<props::ColumnWidth> for Auto {}
impl ValidFor<props::Columns> for Auto {}
impl ValidFor<props::GridArea> for Auto {}
impl ValidFor<props::GridColumn> for Auto {}
impl ValidFor<props::GridColumnEnd> for Auto {}
impl ValidFor<props::GridColumnStart> for Auto {}
impl ValidFor<props::GridRow> for Auto {}
impl ValidFor<props::GridRowEnd> for Auto {}
impl ValidFor<props::GridRowStart> for Auto {}
impl ValidFor<props::HyphenateCharacter> for Auto {}
impl ValidFor<props::HyphenateLimitChars> for Auto {}
impl ValidFor<props::Inset> for Auto {}
impl ValidFor<props::InsetBlock> for Auto {}
impl ValidFor<props::InsetBlockEnd> for Auto {}
impl ValidFor<props::InsetBlockStart> for Auto {}
impl ValidFor<props::InsetInline> for Auto {}
impl ValidFor<props::InsetInlineEnd> for Auto {}
impl ValidFor<props::InsetInlineStart> for Auto {}
impl ValidFor<props::Left> for Auto {}
impl ValidFor<props::Margin> for Auto {}
impl ValidFor<props::MarginBlock> for Auto {}
impl ValidFor<props::MarginBlockEnd> for Auto {}
impl ValidFor<props::MarginBlockStart> for Auto {}
impl ValidFor<props::MarginBottom> for Auto {}
impl ValidFor<props::MarginInline> for Auto {}
impl ValidFor<props::MarginInlineEnd> for Auto {}
impl ValidFor<props::MarginInlineStart> for Auto {}
impl ValidFor<props::MarginLeft> for Auto {}
impl ValidFor<props::MarginRight> for Auto {}
impl ValidFor<props::MarginTop> for Auto {}
impl ValidFor<props::MaskBorderWidth> for Auto {}
impl ValidFor<props::OutlineColor> for Auto {}
impl ValidFor<props::Page> for Auto {}
impl ValidFor<props::Right> for Auto {}
impl ValidFor<props::ScrollPadding> for Auto {}
impl ValidFor<props::ScrollPaddingBlock> for Auto {}
impl ValidFor<props::ScrollPaddingBlockEnd> for Auto {}
impl ValidFor<props::ScrollPaddingBlockStart> for Auto {}
impl ValidFor<props::ScrollPaddingBottom> for Auto {}
impl ValidFor<props::ScrollPaddingInline> for Auto {}
impl ValidFor<props::ScrollPaddingInlineEnd> for Auto {}
impl ValidFor<props::ScrollPaddingInlineStart> for Auto {}
impl ValidFor<props::ScrollPaddingLeft> for Auto {}
impl ValidFor<props::ScrollPaddingRight> for Auto {}
impl ValidFor<props::ScrollPaddingTop> for Auto {}
impl ValidFor<props::ScrollbarColor> for Auto {}
impl ValidFor<props::TextDecorationInset> for Auto {}
impl ValidFor<props::TextUnderlineOffset> for Auto {}
impl ValidFor<props::Top> for Auto {}
impl ValidFor<props::ZIndex> for Auto {}
impl ValidFor<props::BreakInside> for Auto {}
impl ValidFor<props::ColumnFill> for Auto {}
impl ValidFor<props::TextWrap> for Auto {}
impl ValidFor<props::TextWrapStyle> for Auto {}
impl ValidFor<props::Caret> for Auto {}
impl ValidFor<props::CaretShape> for Auto {}
impl ValidFor<props::TextDecoration> for Auto {}
impl ValidFor<props::TextDecoration> for NoneValue {}
impl ValidFor<props::Offset> for Auto {}
impl ValidFor<props::Offset> for NoneValue {}
impl ValidFor<props::OffsetPosition> for Auto {}
impl ValidFor<props::OffsetAnchor> for Auto {}
impl ValidFor<props::Appearance> for Auto {}
impl ValidFor<props::Appearance> for NoneValue {}
impl ValidFor<props::TextBox> for Auto {}
impl ValidFor<props::TextBox> for NoneValue {}
impl ValidFor<props::TextBoxEdge> for Auto {}
impl ValidFor<props::TextAlignLast> for Auto {}
impl ValidFor<props::Overflow> for Auto {}
impl ValidFor<props::OverflowBlock> for Auto {}
impl ValidFor<props::OverflowInline> for Auto {}
impl ValidFor<props::OverflowX> for Auto {}
impl ValidFor<props::OverflowY> for Auto {}
impl ValidFor<props::WebkitMaskSize> for Auto {}
impl ValidFor<props::BackgroundSize> for Auto {}
impl ValidFor<props::MaskSize> for Auto {}
impl ValidFor<props::TimelineTriggerExitRange> for Auto {}
impl ValidFor<props::TimelineTriggerExitRangeEnd> for Auto {}
impl ValidFor<props::TimelineTriggerExitRangeStart> for Auto {}
impl ValidFor<props::OverscrollBehavior> for Auto {}
impl ValidFor<props::OverscrollBehavior> for NoneValue {}
impl ValidFor<props::OverscrollBehaviorBlock> for Auto {}
impl ValidFor<props::OverscrollBehaviorBlock> for NoneValue {}
impl ValidFor<props::OverscrollBehaviorInline> for Auto {}
impl ValidFor<props::OverscrollBehaviorInline> for NoneValue {}
impl ValidFor<props::OverscrollBehaviorX> for Auto {}
impl ValidFor<props::OverscrollBehaviorX> for NoneValue {}
impl ValidFor<props::OverscrollBehaviorY> for Auto {}
impl ValidFor<props::OverscrollBehaviorY> for NoneValue {}
impl ValidFor<props::FlexBasis> for Auto {}
impl ValidFor<props::Flex> for Auto {}
impl ValidFor<props::Flex> for NoneValue {}
impl ValidFor<props::WillChange> for Auto {}
impl ValidFor<props::ImageRendering> for Auto {}
impl ValidFor<props::ShapeRendering> for Auto {}
impl ValidFor<props::Outline> for Auto {}
impl ValidFor<props::Outline> for NoneValue {}
impl ValidFor<props::OutlineStyle> for Auto {}
impl ValidFor<props::OutlineStyle> for NoneValue {}
impl ValidFor<props::BaselineSource> for Auto {}
impl ValidFor<props::BlockSize> for Auto {}
impl ValidFor<props::Height> for Auto {}
impl ValidFor<props::InlineSize> for Auto {}
impl ValidFor<props::MinBlockSize> for Auto {}
impl ValidFor<props::MinHeight> for Auto {}
impl ValidFor<props::MinInlineSize> for Auto {}
impl ValidFor<props::MinWidth> for Auto {}
impl ValidFor<props::Width> for Auto {}
impl ValidFor<props::TableLayout> for Auto {}
impl ValidFor<props::TextDecorationThickness> for Auto {}
impl ValidFor<props::TextUnderlinePosition> for Auto {}
impl ValidFor<props::TextRendering> for Auto {}
impl ValidFor<props::ContentVisibility> for Auto {}
impl ValidFor<props::Interactivity> for Auto {}
impl ValidFor<props::TextJustify> for Auto {}
impl ValidFor<props::TextJustify> for NoneValue {}
impl ValidFor<props::Isolation> for Auto {}
impl ValidFor<props::ColorInterpolationFilters> for Auto {}
impl ValidFor<props::TouchAction> for Auto {}
impl ValidFor<props::TouchAction> for NoneValue {}
impl ValidFor<props::CaretAnimation> for Auto {}
impl ValidFor<props::Hyphens> for Auto {}
impl ValidFor<props::Hyphens> for NoneValue {}
impl ValidFor<props::GridAutoColumns> for Auto {}
impl ValidFor<props::GridAutoRows> for Auto {}
impl ValidFor<props::GridTemplateColumns> for Auto {}
impl ValidFor<props::GridTemplateColumns> for NoneValue {}
impl ValidFor<props::GridTemplateRows> for Auto {}
impl ValidFor<props::GridTemplateRows> for NoneValue {}
impl ValidFor<props::FontOpticalSizing> for Auto {}
impl ValidFor<props::FontOpticalSizing> for NoneValue {}
impl ValidFor<props::FontSynthesisSmallCaps> for Auto {}
impl ValidFor<props::FontSynthesisSmallCaps> for NoneValue {}
impl ValidFor<props::FontSynthesisStyle> for Auto {}
impl ValidFor<props::FontSynthesisStyle> for NoneValue {}
impl ValidFor<props::FontSynthesisWeight> for Auto {}
impl ValidFor<props::FontSynthesisWeight> for NoneValue {}
impl ValidFor<props::OverflowAnchor> for Auto {}
impl ValidFor<props::OverflowAnchor> for NoneValue {}
impl ValidFor<props::Quotes> for Auto {}
impl ValidFor<props::Quotes> for NoneValue {}
impl ValidFor<props::RubyOverhang> for Auto {}
impl ValidFor<props::RubyOverhang> for NoneValue {}
impl ValidFor<props::ScrollTargetGroup> for Auto {}
impl ValidFor<props::ScrollTargetGroup> for NoneValue {}
impl ValidFor<props::TimelineTriggerSource> for Auto {}
impl ValidFor<props::TimelineTriggerSource> for NoneValue {}
impl ValidFor<props::FontKerning> for Auto {}
impl ValidFor<props::FontKerning> for NoneValue {}
impl ValidFor<props::ForcedColorAdjust> for Auto {}
impl ValidFor<props::ForcedColorAdjust> for NoneValue {}
impl ValidFor<props::ScrollbarWidth> for Auto {}
impl ValidFor<props::ScrollbarWidth> for NoneValue {}
impl ValidFor<props::TextAutospace> for Auto {}
impl ValidFor<props::ColumnWrap> for Auto {}
impl ValidFor<props::TextEmphasisPosition> for Auto {}
impl ValidFor<props::OffsetRotate> for Auto {}
impl ValidFor<props::ScrollBehavior> for Auto {}
impl ValidFor<props::ScrollbarGutter> for Auto {}
impl ValidFor<props::WebkitOverflowScrolling> for Auto {}
impl ValidFor<props::AnimationFillMode> for NoneValue {}
impl ValidFor<props::TextDecorationLine> for NoneValue {}
impl ValidFor<props::Resize> for NoneValue {}
impl ValidFor<props::ScrollSnapType> for NoneValue {}
impl ValidFor<props::Display> for NoneValue {}
impl ValidFor<props::WebkitMask> for NoneValue {}
impl ValidFor<props::Background> for NoneValue {}
impl ValidFor<props::ClipPath> for NoneValue {}
impl ValidFor<props::OffsetPath> for NoneValue {}
impl ValidFor<props::ShapeOutside> for NoneValue {}
impl ValidFor<props::Clear> for NoneValue {}
impl ValidFor<props::MozAppearance> for NoneValue {}
impl ValidFor<props::WebkitAppearance> for NoneValue {}
impl ValidFor<props::TextTransform> for NoneValue {}
impl ValidFor<props::ScrollSnapAlign> for NoneValue {}
impl ValidFor<props::TextEmphasis> for NoneValue {}
impl ValidFor<props::TextEmphasisStyle> for NoneValue {}
impl ValidFor<props::Content> for NoneValue {}
impl ValidFor<props::FontVariantLigatures> for NoneValue {}
impl ValidFor<props::ObjectFit> for NoneValue {}
impl ValidFor<props::Contain> for NoneValue {}
impl ValidFor<props::Fill> for NoneValue {}
impl ValidFor<props::Stroke> for NoneValue {}
impl ValidFor<props::Border> for NoneValue {}
impl ValidFor<props::BorderBlock> for NoneValue {}
impl ValidFor<props::BorderBlockEnd> for NoneValue {}
impl ValidFor<props::BorderBlockStart> for NoneValue {}
impl ValidFor<props::BorderBottom> for NoneValue {}
impl ValidFor<props::BorderInline> for NoneValue {}
impl ValidFor<props::BorderInlineEnd> for NoneValue {}
impl ValidFor<props::BorderInlineStart> for NoneValue {}
impl ValidFor<props::BorderLeft> for NoneValue {}
impl ValidFor<props::BorderRight> for NoneValue {}
impl ValidFor<props::BorderTop> for NoneValue {}
impl ValidFor<props::ColumnRule> for NoneValue {}
impl ValidFor<props::BorderBlockEndStyle> for NoneValue {}
impl ValidFor<props::BorderBlockStartStyle> for NoneValue {}
impl ValidFor<props::BorderBlockStyle> for NoneValue {}
impl ValidFor<props::BorderBottomStyle> for NoneValue {}
impl ValidFor<props::BorderInlineEndStyle> for NoneValue {}
impl ValidFor<props::BorderInlineStartStyle> for NoneValue {}
impl ValidFor<props::BorderInlineStyle> for NoneValue {}
impl ValidFor<props::BorderLeftStyle> for NoneValue {}
impl ValidFor<props::BorderRightStyle> for NoneValue {}
impl ValidFor<props::BorderStyle> for NoneValue {}
impl ValidFor<props::BorderTopStyle> for NoneValue {}
impl ValidFor<props::ColumnRuleStyle> for NoneValue {}
impl ValidFor<props::WebkitTouchCallout> for NoneValue {}
impl ValidFor<props::MozContextProperties> for NoneValue {}
impl ValidFor<props::MaxBlockSize> for NoneValue {}
impl ValidFor<props::MaxHeight> for NoneValue {}
impl ValidFor<props::MaxInlineSize> for NoneValue {}
impl ValidFor<props::MaxWidth> for NoneValue {}
impl ValidFor<props::VectorEffect> for NoneValue {}
impl ValidFor<props::FontSizeAdjust> for NoneValue {}
impl ValidFor<props::Float> for NoneValue {}
impl ValidFor<props::ListStyle> for NoneValue {}
impl ValidFor<props::ViewTransitionName> for NoneValue {}
impl ValidFor<props::WebkitLineClamp> for NoneValue {}
impl ValidFor<props::WebkitMaskImage> for NoneValue {}
impl ValidFor<props::AnimationName> for NoneValue {}
impl ValidFor<props::AnimationTrigger> for NoneValue {}
impl ValidFor<props::BackdropFilter> for NoneValue {}
impl ValidFor<props::BackgroundImage> for NoneValue {}
impl ValidFor<props::BorderImageSource> for NoneValue {}
impl ValidFor<props::BoxShadow> for NoneValue {}
impl ValidFor<props::ContainIntrinsicBlockSize> for NoneValue {}
impl ValidFor<props::ContainIntrinsicHeight> for NoneValue {}
impl ValidFor<props::ContainIntrinsicInlineSize> for NoneValue {}
impl ValidFor<props::ContainIntrinsicSize> for NoneValue {}
impl ValidFor<props::ContainIntrinsicWidth> for NoneValue {}
impl ValidFor<props::Container> for NoneValue {}
impl ValidFor<props::ContainerName> for NoneValue {}
impl ValidFor<props::CounterIncrement> for NoneValue {}
impl ValidFor<props::CounterReset> for NoneValue {}
impl ValidFor<props::CounterSet> for NoneValue {}
impl ValidFor<props::D> for NoneValue {}
impl ValidFor<props::Filter> for NoneValue {}
impl ValidFor<props::Grid> for NoneValue {}
impl ValidFor<props::GridTemplate> for NoneValue {}
impl ValidFor<props::GridTemplateAreas> for NoneValue {}
impl ValidFor<props::LineClamp> for NoneValue {}
impl ValidFor<props::ListStyleImage> for NoneValue {}
impl ValidFor<props::ListStyleType> for NoneValue {}
impl ValidFor<props::Marker> for NoneValue {}
impl ValidFor<props::MarkerEnd> for NoneValue {}
impl ValidFor<props::MarkerMid> for NoneValue {}
impl ValidFor<props::MarkerStart> for NoneValue {}
impl ValidFor<props::MaskBorderSource> for NoneValue {}
impl ValidFor<props::MaskImage> for NoneValue {}
impl ValidFor<props::Perspective> for NoneValue {}
impl ValidFor<props::Rotate> for NoneValue {}
impl ValidFor<props::Scale> for NoneValue {}
impl ValidFor<props::StrokeDasharray> for NoneValue {}
impl ValidFor<props::TextShadow> for NoneValue {}
impl ValidFor<props::TimelineTrigger> for NoneValue {}
impl ValidFor<props::TimelineTriggerName> for NoneValue {}
impl ValidFor<props::Transform> for NoneValue {}
impl ValidFor<props::Translate> for NoneValue {}
impl ValidFor<props::ViewTransitionClass> for NoneValue {}
impl ValidFor<props::FontSynthesis> for NoneValue {}
impl ValidFor<props::BorderImage> for NoneValue {}
impl ValidFor<props::TextBoxTrim> for NoneValue {}

#[macro_export]
macro_rules! register_generated_keywords {
    ($callback:ident) => {
        $callback! {
            AlignContentKeyword,
            AlignItemsKeyword,
            AlignSelfKeyword,
            AlignmentBaselineKeyword,
            AllKeyword,
            AnimationCompositionKeyword,
            AnimationDirectionKeyword,
            AnimationFillModeKeyword,
            AnimationIterationCountKeyword,
            AnimationKeyword,
            AnimationPlayStateKeyword,
            AnimationTimingFunctionKeyword,
            AppearanceKeyword,
            BackfaceVisibilityKeyword,
            BackgroundAttachmentKeyword,
            BackgroundBlendModeKeyword,
            BackgroundClipKeyword,
            BackgroundKeyword,
            BackgroundOriginKeyword,
            BackgroundPositionXKeyword,
            BackgroundPositionYKeyword,
            BaselineShiftKeyword,
            BaselineSourceKeyword,
            BlockSizeKeyword,
            BorderBlockEndStyleKeyword,
            BorderBlockEndWidthKeyword,
            BorderCollapseKeyword,
            BorderImageKeyword,
            BorderImageRepeatKeyword,
            BorderKeyword,
            BoxDecorationBreakKeyword,
            BoxSizingKeyword,
            BreakAfterKeyword,
            BreakInsideKeyword,
            CaptionSideKeyword,
            CaretAnimationKeyword,
            CaretKeyword,
            CaretShapeKeyword,
            ClearKeyword,
            ClipPathKeyword,
            ClipRuleKeyword,
            ColorInterpolationFiltersKeyword,
            ColorKeyword,
            ColorSchemeKeyword,
            ColumnFillKeyword,
            ColumnGapKeyword,
            ColumnSpanKeyword,
            ColumnWrapKeyword,
            ContainKeyword,
            ContainerTypeKeyword,
            ContentKeyword,
            ContentVisibilityKeyword,
            CornerBlockEndShapeKeyword,
            CursorKeyword,
            DirectionKeyword,
            DisplayKeyword,
            DominantBaselineKeyword,
            DynamicRangeLimitKeyword,
            EmptyCellsKeyword,
            FillKeyword,
            FlexBasisKeyword,
            FlexDirectionKeyword,
            FlexFlowKeyword,
            FlexKeyword,
            FlexWrapKeyword,
            FloatKeyword,
            FontFamilyKeyword,
            FontKerningKeyword,
            FontKeyword,
            FontOpticalSizingKeyword,
            FontSizeAdjustKeyword,
            FontSizeKeyword,
            FontStyleKeyword,
            FontSynthesisKeyword,
            FontVariantAlternatesKeyword,
            FontVariantCapsKeyword,
            FontVariantEastAsianKeyword,
            FontVariantEmojiKeyword,
            FontVariantKeyword,
            FontVariantLigaturesKeyword,
            FontVariantNumericKeyword,
            FontVariantPositionKeyword,
            FontWeightKeyword,
            ForcedColorAdjustKeyword,
            GridAutoColumnsKeyword,
            GridAutoFlowKeyword,
            GridTemplateColumnsKeyword,
            HangingPunctuationKeyword,
            HyphensKeyword,
            ImageOrientationKeyword,
            ImageRenderingKeyword,
            InteractivityKeyword,
            IsolationKeyword,
            JustifyContentKeyword,
            JustifyItemsKeyword,
            JustifySelfKeyword,
            LineBreakKeyword,
            ListStyleKeyword,
            ListStylePositionKeyword,
            MaskBorderKeyword,
            MaskBorderModeKeyword,
            MaskClipKeyword,
            MaskCompositeKeyword,
            MaskKeyword,
            MaskModeKeyword,
            MaskOriginKeyword,
            MathDepthKeyword,
            MathStyleKeyword,
            MaxBlockSizeKeyword,
            MixBlendModeKeyword,
            MozAppearanceKeyword,
            MozContextPropertiesKeyword,
            MozOrientKeyword,
            ObjectFitKeyword,
            OffsetAnchorKeyword,
            OffsetKeyword,
            OffsetPathKeyword,
            OffsetPositionKeyword,
            OffsetRotateKeyword,
            OutlineKeyword,
            OutlineStyleKeyword,
            OverflowKeyword,
            OverflowWrapKeyword,
            OverscrollBehaviorKeyword,
            PaintOrderKeyword,
            PointerEventsKeyword,
            PositionKeyword,
            PrintColorAdjustKeyword,
            ReadingFlowKeyword,
            ResizeKeyword,
            RubyAlignKeyword,
            RubyPositionKeyword,
            ScrollBehaviorKeyword,
            ScrollMarkerGroupKeyword,
            ScrollSnapAlignKeyword,
            ScrollSnapStopKeyword,
            ScrollSnapTypeKeyword,
            ScrollbarGutterKeyword,
            ScrollbarWidthKeyword,
            ShapeOutsideKeyword,
            ShapeRenderingKeyword,
            StrokeLinecapKeyword,
            StrokeLinejoinKeyword,
            TableLayoutKeyword,
            TextAlignKeyword,
            TextAlignLastKeyword,
            TextAnchorKeyword,
            TextAutospaceKeyword,
            TextBoxEdgeKeyword,
            TextBoxKeyword,
            TextBoxTrimKeyword,
            TextCombineUprightKeyword,
            TextDecorationKeyword,
            TextDecorationLineKeyword,
            TextDecorationSkipInkKeyword,
            TextDecorationStyleKeyword,
            TextDecorationThicknessKeyword,
            TextEmphasisKeyword,
            TextEmphasisPositionKeyword,
            TextJustifyKeyword,
            TextOrientationKeyword,
            TextOverflowKeyword,
            TextRenderingKeyword,
            TextTransformKeyword,
            TextUnderlinePositionKeyword,
            TextWrapKeyword,
            TextWrapModeKeyword,
            TextWrapStyleKeyword,
            TimelineTriggerExitRangeKeyword,
            TimelineTriggerRangeKeyword,
            TouchActionKeyword,
            TransformBoxKeyword,
            TransformStyleKeyword,
            TransitionBehaviorKeyword,
            TransitionKeyword,
            UnicodeBidiKeyword,
            VectorEffectKeyword,
            VerticalAlignKeyword,
            ViewTransitionNameKeyword,
            VisibilityKeyword,
            WebkitAppearanceKeyword,
            WebkitBoxReflectKeyword,
            WebkitMaskClipKeyword,
            WebkitMaskCompositeKeyword,
            WebkitMaskKeyword,
            WebkitMaskOriginKeyword,
            WebkitMaskPositionKeyword,
            WebkitMaskRepeatKeyword,
            WebkitMaskSizeKeyword,
            WebkitOverflowScrollingKeyword,
            WebkitTouchCalloutKeyword,
            WebkitUserModifyKeyword,
            WebkitUserSelectKeyword,
            WhiteSpaceCollapseKeyword,
            WhiteSpaceKeyword,
            WillChangeKeyword,
            WordBreakKeyword,
            WordWrapKeyword,
            WritingModeKeyword,
            ZoomKeyword
        }
    };
}
