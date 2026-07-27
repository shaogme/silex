// 自动生成的 Tailwind 测试用例规则表（用于验证 test-cases 的生成与 CSS 规则解析正确性）
// 对应 tailwind-classes.json 中的 test_cases
// 避免手写硬编码，与 silex_tw_core 的 resolver 保持 100% 规则对齐

#[allow(unused_imports)]
use crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};
#[allow(unused_imports)]
use crate::css::tw::resolver::codegen::property_id::CssPropertyId;
#[allow(unused_imports)]
use crate::css::tw::resolver::make_rule;
#[allow(unused_imports)]
use proc_macro2::Span;

#[rustfmt::skip]
pub const TEST_CASE_CANDIDATE_UTILITIES: &[&str] = &[
    "animate-bounce",
    "animate-none",
    "animate-out",
    "animate-spin",
    "backdrop-blur-2xl",
    "backdrop-blur-lg",
    "backdrop-blur-md",
    "backdrop-blur-none",
    "backdrop-blur-sm",
    "backdrop-blur-xl",
    "backdrop-blur-xs",
    "bg-conic",
    "bg-linear-to-b",
    "bg-none",
    "bg-radial",
    "bg-slate-900",
    "blur-2xl",
    "blur-lg",
    "blur-md",
    "blur-none",
    "blur-sm",
    "blur-xl",
    "blur-xs",
    "border-0",
    "border-1",
    "border-4",
    "border-b-0",
    "border-b-1",
    "border-b-2",
    "border-b-4",
    "border-b-8",
    "border-be-0",
    "border-be-1",
    "border-be-2",
    "border-be-4",
    "border-be-8",
    "border-bs-0",
    "border-bs-1",
    "border-bs-2",
    "border-bs-4",
    "border-bs-8",
    "border-dashed",
    "border-e-0",
    "border-e-1",
    "border-e-2",
    "border-e-4",
    "border-e-8",
    "border-l-0",
    "border-l-1",
    "border-l-2",
    "border-l-4",
    "border-l-8",
    "border-none",
    "border-r-0",
    "border-r-1",
    "border-r-2",
    "border-r-4",
    "border-r-8",
    "border-s-0",
    "border-s-1",
    "border-s-2",
    "border-s-4",
    "border-s-8",
    "border-solid",
    "border-t-0",
    "border-t-1",
    "border-t-2",
    "border-t-4",
    "border-t-8",
    "border-x-0",
    "border-x-1",
    "border-x-2",
    "border-x-4",
    "border-x-8",
    "border-y-0",
    "border-y-1",
    "border-y-2",
    "border-y-4",
    "border-y-8",
    "bottom-0",
    "bottom-1",
    "bottom-32",
    "bottom-4",
    "bottom-auto",
    "bottom-full",
    "bottom-px",
    "break-after-auto",
    "break-after-avoid-flex",
    "break-after-avoid-page",
    "break-before-auto",
    "break-before-avoid-flex",
    "break-before-avoid-page",
    "break-inside-auto",
    "break-inside-avoid-column",
    "break-inside-avoid-page",
    "columns-0",
    "columns-1",
    "columns-4",
    "columns-4xl",
    "columns-auto",
    "columns-lg",
    "columns-md",
    "columns-sm",
    "columns-xl",
    "columns-xs",
    "delay-100",
    "delay-300",
    "delay-75",
    "duration-100",
    "duration-300",
    "duration-75",
    "font-black",
    "font-mono",
    "font-thin",
    "gap-0",
    "gap-1",
    "gap-36",
    "gap-4",
    "gap-px",
    "h-0",
    "h-1",
    "h-4",
    "h-auto",
    "h-full",
    "h-px",
    "h-screen",
    "inset-0",
    "inset-1",
    "inset-32",
    "inset-4",
    "inset-auto",
    "inset-full",
    "inset-px",
    "inset-shadow-2xs",
    "inset-shadow-none",
    "inset-shadow-sm",
    "inset-shadow-xs",
    "leading-0",
    "leading-1",
    "leading-4",
    "leading-44",
    "leading-none",
    "leading-px",
    "leading-tight",
    "left-0",
    "left-1",
    "left-32",
    "left-4",
    "left-auto",
    "left-full",
    "left-px",
    "m-0",
    "m-1",
    "m-4",
    "m-auto",
    "m-px",
    "max-h-0",
    "max-h-1",
    "max-h-4",
    "max-h-full",
    "max-h-none",
    "max-h-px",
    "max-h-screen",
    "max-w-0",
    "max-w-1",
    "max-w-4",
    "max-w-44",
    "max-w-full",
    "max-w-lg",
    "max-w-md",
    "max-w-none",
    "max-w-px",
    "max-w-sm",
    "max-w-xl",
    "max-w-xs",
    "mb-0",
    "mb-1",
    "mb-4",
    "mb-auto",
    "mb-px",
    "min-h-0",
    "min-h-1",
    "min-h-4",
    "min-h-auto",
    "min-h-full",
    "min-h-px",
    "min-h-screen",
    "min-w-0",
    "min-w-1",
    "min-w-4",
    "min-w-44",
    "min-w-auto",
    "min-w-full",
    "min-w-lg",
    "min-w-md",
    "min-w-px",
    "min-w-sm",
    "min-w-xl",
    "min-w-xs",
    "ml-0",
    "ml-1",
    "ml-4",
    "ml-auto",
    "ml-px",
    "mr-0",
    "mr-1",
    "mr-4",
    "mr-auto",
    "mr-px",
    "mt-0",
    "mt-1",
    "mt-4",
    "mt-auto",
    "mt-px",
    "mx-0",
    "mx-1",
    "mx-4",
    "mx-auto",
    "mx-px",
    "my-0",
    "my-1",
    "my-4",
    "my-auto",
    "my-px",
    "opacity-0",
    "opacity-100",
    "opacity-5",
    "opacity-50",
    "opacity-95",
    "p-0",
    "p-1",
    "p-36",
    "p-4",
    "p-px",
    "pb-0",
    "pb-1",
    "pb-36",
    "pb-4",
    "pb-px",
    "pl-0",
    "pl-1",
    "pl-36",
    "pl-4",
    "pl-px",
    "pr-0",
    "pr-1",
    "pr-36",
    "pr-4",
    "pr-px",
    "pt-0",
    "pt-1",
    "pt-36",
    "pt-4",
    "pt-px",
    "px-0",
    "px-1",
    "px-36",
    "px-4",
    "px-px",
    "py-0",
    "py-1",
    "py-36",
    "py-4",
    "py-px",
    "right-0",
    "right-1",
    "right-32",
    "right-4",
    "right-auto",
    "right-full",
    "right-px",
    "ring",
    "rotate-0",
    "rotate-45",
    "rotate-90",
    "rounded-2xl",
    "rounded-b-2xl",
    "rounded-b-full",
    "rounded-b-lg",
    "rounded-b-md",
    "rounded-b-none",
    "rounded-b-sm",
    "rounded-b-xl",
    "rounded-b-xs",
    "rounded-bl-2xl",
    "rounded-bl-full",
    "rounded-bl-lg",
    "rounded-bl-md",
    "rounded-bl-none",
    "rounded-bl-sm",
    "rounded-bl-xl",
    "rounded-bl-xs",
    "rounded-br-2xl",
    "rounded-br-full",
    "rounded-br-lg",
    "rounded-br-md",
    "rounded-br-none",
    "rounded-br-sm",
    "rounded-br-xl",
    "rounded-br-xs",
    "rounded-e-2xl",
    "rounded-e-full",
    "rounded-e-lg",
    "rounded-e-md",
    "rounded-e-none",
    "rounded-e-sm",
    "rounded-e-xl",
    "rounded-e-xs",
    "rounded-ee-2xl",
    "rounded-ee-full",
    "rounded-ee-lg",
    "rounded-ee-md",
    "rounded-ee-none",
    "rounded-ee-sm",
    "rounded-ee-xl",
    "rounded-ee-xs",
    "rounded-es-2xl",
    "rounded-es-full",
    "rounded-es-lg",
    "rounded-es-md",
    "rounded-es-none",
    "rounded-es-sm",
    "rounded-es-xl",
    "rounded-es-xs",
    "rounded-full",
    "rounded-l-2xl",
    "rounded-l-full",
    "rounded-l-lg",
    "rounded-l-md",
    "rounded-l-none",
    "rounded-l-sm",
    "rounded-l-xl",
    "rounded-l-xs",
    "rounded-lg",
    "rounded-md",
    "rounded-none",
    "rounded-r-2xl",
    "rounded-r-full",
    "rounded-r-lg",
    "rounded-r-md",
    "rounded-r-none",
    "rounded-r-sm",
    "rounded-r-xl",
    "rounded-r-xs",
    "rounded-s-2xl",
    "rounded-s-full",
    "rounded-s-lg",
    "rounded-s-md",
    "rounded-s-none",
    "rounded-s-sm",
    "rounded-s-xl",
    "rounded-s-xs",
    "rounded-se-2xl",
    "rounded-se-full",
    "rounded-se-lg",
    "rounded-se-md",
    "rounded-se-none",
    "rounded-se-sm",
    "rounded-se-xl",
    "rounded-se-xs",
    "rounded-sm",
    "rounded-ss-2xl",
    "rounded-ss-full",
    "rounded-ss-lg",
    "rounded-ss-md",
    "rounded-ss-none",
    "rounded-ss-sm",
    "rounded-ss-xl",
    "rounded-ss-xs",
    "rounded-t-2xl",
    "rounded-t-full",
    "rounded-t-lg",
    "rounded-t-md",
    "rounded-t-none",
    "rounded-t-sm",
    "rounded-t-xl",
    "rounded-t-xs",
    "rounded-tl-2xl",
    "rounded-tl-full",
    "rounded-tl-lg",
    "rounded-tl-md",
    "rounded-tl-none",
    "rounded-tl-sm",
    "rounded-tl-xl",
    "rounded-tl-xs",
    "rounded-tr-2xl",
    "rounded-tr-full",
    "rounded-tr-lg",
    "rounded-tr-md",
    "rounded-tr-none",
    "rounded-tr-sm",
    "rounded-tr-xl",
    "rounded-tr-xs",
    "rounded-xl",
    "rounded-xs",
    "scale-0",
    "scale-100",
    "scale-150",
    "scale-50",
    "scale-95",
    "shadow-2xl",
    "shadow-lg",
    "shadow-md",
    "shadow-none",
    "shadow-sm",
    "shadow-xl",
    "shadow-xs",
    "text-2xl",
    "text-8xl",
    "text-lg",
    "text-sm",
    "text-xl",
    "text-xs",
    "top-0",
    "top-1",
    "top-32",
    "top-4",
    "top-auto",
    "top-full",
    "top-px",
    "tracking-normal",
    "tracking-wide",
    "tracking-widest",
    "translate-x-0",
    "translate-x-1/2",
    "translate-x-full",
    "translate-y-0",
    "translate-y-1/2",
    "translate-y-full",
    "w-0",
    "w-1",
    "w-4",
    "w-44",
    "w-auto",
    "w-full",
    "w-lg",
    "w-md",
    "w-px",
    "w-sm",
    "w-xl",
    "w-xs",
    "z-0",
    "z-30",
    "z-50",
    "z-auto",
];

/// 静态表的一行：类名 + 伴生选择器（`divide-*` / `placeholder-*` 等）+ 声明列表
pub type StaticRuleRow = (
    &'static str,
    Option<&'static str>,
    &'static [(CssPropertyId, StaticVal)],
);

#[rustfmt::skip]
pub static TEST_CASE_RULES: &[StaticRuleRow] = &[
    ("animate-bounce", None, &[
        (CssPropertyId::Animation, StaticVal::Literal("bounce 1s infinite")),
        (CssPropertyId::WillChange, StaticVal::Kw("transform")),
    ]),
    ("animate-none", None, &[
        (CssPropertyId::Animation, StaticVal::Kw("none")),
    ]),
    ("animate-out", None, &[
        (CssPropertyId::AnimationName, StaticVal::Kw("exit")),
        (CssPropertyId::AnimationDuration, StaticVal::Num(150.0, "ms")),
    ]),
    ("animate-spin", None, &[
        (CssPropertyId::Animation, StaticVal::Literal("spin 1s linear infinite")),
        (CssPropertyId::WillChange, StaticVal::Kw("transform")),
    ]),
    ("backdrop-blur-2xl", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(40px)")),
    ]),
    ("backdrop-blur-lg", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(16px)")),
    ]),
    ("backdrop-blur-md", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(8px)")),
    ]),
    ("backdrop-blur-none", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(0px)")),
    ]),
    ("backdrop-blur-sm", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(4px)")),
    ]),
    ("backdrop-blur-xl", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(24px)")),
    ]),
    ("backdrop-blur-xs", None, &[
        (CssPropertyId::BackdropFilter, StaticVal::Kw("blur(2px)")),
    ]),
    ("bg-conic", None, &[
        (CssPropertyId::BackgroundImage, StaticVal::Kw("conic-gradient(var(--tw-gradient-stops))")),
    ]),
    ("bg-linear-to-b", None, &[
        (CssPropertyId::BackgroundImage, StaticVal::Kw("linear-gradient(to bottom, var(--tw-gradient-stops))")),
    ]),
    ("bg-none", None, &[
        (CssPropertyId::BackgroundImage, StaticVal::Kw("none")),
    ]),
    ("bg-radial", None, &[
        (CssPropertyId::BackgroundImage, StaticVal::Kw("radial-gradient(var(--tw-gradient-stops))")),
    ]),
    ("bg-slate-900", None, &[
        (CssPropertyId::BackgroundColor, StaticVal::Hex("#0f172b")),
    ]),
    ("blur-2xl", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(40px)")),
    ]),
    ("blur-lg", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(16px)")),
    ]),
    ("blur-md", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(8px)")),
    ]),
    ("blur-none", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("none")),
    ]),
    ("blur-sm", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(4px)")),
    ]),
    ("blur-xl", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(24px)")),
    ]),
    ("blur-xs", None, &[
        (CssPropertyId::Filter, StaticVal::Kw("blur(2px)")),
    ]),
    ("border-0", None, &[
        (CssPropertyId::BorderWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-1", None, &[
        (CssPropertyId::BorderWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-4", None, &[
        (CssPropertyId::BorderWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-b-0", None, &[
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-b-1", None, &[
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-b-2", None, &[
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-b-4", None, &[
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-b-8", None, &[
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-be-0", None, &[
        (CssPropertyId::BorderBlockEndWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-be-1", None, &[
        (CssPropertyId::BorderBlockEndWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-be-2", None, &[
        (CssPropertyId::BorderBlockEndWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-be-4", None, &[
        (CssPropertyId::BorderBlockEndWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-be-8", None, &[
        (CssPropertyId::BorderBlockEndWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-bs-0", None, &[
        (CssPropertyId::BorderBlockStartWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-bs-1", None, &[
        (CssPropertyId::BorderBlockStartWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-bs-2", None, &[
        (CssPropertyId::BorderBlockStartWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-bs-4", None, &[
        (CssPropertyId::BorderBlockStartWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-bs-8", None, &[
        (CssPropertyId::BorderBlockStartWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-dashed", None, &[
        (CssPropertyId::BorderStyle, StaticVal::Kw("dashed")),
    ]),
    ("border-e-0", None, &[
        (CssPropertyId::BorderInlineEndWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-e-1", None, &[
        (CssPropertyId::BorderInlineEndWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-e-2", None, &[
        (CssPropertyId::BorderInlineEndWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-e-4", None, &[
        (CssPropertyId::BorderInlineEndWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-e-8", None, &[
        (CssPropertyId::BorderInlineEndWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-l-0", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-l-1", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-l-2", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-l-4", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-l-8", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-none", None, &[
        (CssPropertyId::BorderStyle, StaticVal::Kw("none")),
    ]),
    ("border-r-0", None, &[
        (CssPropertyId::BorderRightWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-r-1", None, &[
        (CssPropertyId::BorderRightWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-r-2", None, &[
        (CssPropertyId::BorderRightWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-r-4", None, &[
        (CssPropertyId::BorderRightWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-r-8", None, &[
        (CssPropertyId::BorderRightWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-s-0", None, &[
        (CssPropertyId::BorderInlineStartWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-s-1", None, &[
        (CssPropertyId::BorderInlineStartWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-s-2", None, &[
        (CssPropertyId::BorderInlineStartWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-s-4", None, &[
        (CssPropertyId::BorderInlineStartWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-s-8", None, &[
        (CssPropertyId::BorderInlineStartWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-solid", None, &[
        (CssPropertyId::BorderStyle, StaticVal::Kw("solid")),
    ]),
    ("border-t-0", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-t-1", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-t-2", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-t-4", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-t-8", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-x-0", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderRightWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-x-1", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(1.0, "px")),
        (CssPropertyId::BorderRightWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-x-2", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(2.0, "px")),
        (CssPropertyId::BorderRightWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-x-4", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(4.0, "px")),
        (CssPropertyId::BorderRightWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-x-8", None, &[
        (CssPropertyId::BorderLeftWidth, StaticVal::Num(8.0, "px")),
        (CssPropertyId::BorderRightWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("border-y-0", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("border-y-1", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(1.0, "px")),
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("border-y-2", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(2.0, "px")),
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(2.0, "px")),
    ]),
    ("border-y-4", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(4.0, "px")),
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(4.0, "px")),
    ]),
    ("border-y-8", None, &[
        (CssPropertyId::BorderTopWidth, StaticVal::Num(8.0, "px")),
        (CssPropertyId::BorderBottomWidth, StaticVal::Num(8.0, "px")),
    ]),
    ("bottom-0", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(0.0, "px")),
    ]),
    ("bottom-1", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(0.25, "rem")),
    ]),
    ("bottom-32", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(8.0, "rem")),
    ]),
    ("bottom-4", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(1.0, "rem")),
    ]),
    ("bottom-auto", None, &[
        (CssPropertyId::Bottom, StaticVal::Kw("auto")),
    ]),
    ("bottom-full", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(100.0, "%")),
    ]),
    ("bottom-px", None, &[
        (CssPropertyId::Bottom, StaticVal::Num(1.0, "px")),
    ]),
    ("break-after-auto", None, &[
        (CssPropertyId::BreakAfter, StaticVal::Kw("auto")),
    ]),
    ("break-after-avoid-flex", None, &[
        (CssPropertyId::BreakAfter, StaticVal::Kw("avoid-flex")),
    ]),
    ("break-after-avoid-page", None, &[
        (CssPropertyId::BreakAfter, StaticVal::Kw("avoid-page")),
    ]),
    ("break-before-auto", None, &[
        (CssPropertyId::BreakBefore, StaticVal::Kw("auto")),
    ]),
    ("break-before-avoid-flex", None, &[
        (CssPropertyId::BreakBefore, StaticVal::Kw("avoid-flex")),
    ]),
    ("break-before-avoid-page", None, &[
        (CssPropertyId::BreakBefore, StaticVal::Kw("avoid-page")),
    ]),
    ("break-inside-auto", None, &[
        (CssPropertyId::BreakInside, StaticVal::Kw("auto")),
    ]),
    ("break-inside-avoid-column", None, &[
        (CssPropertyId::BreakInside, StaticVal::Kw("avoid-column")),
    ]),
    ("break-inside-avoid-page", None, &[
        (CssPropertyId::BreakInside, StaticVal::Kw("avoid-page")),
    ]),
    ("columns-0", None, &[
        (CssPropertyId::Columns, StaticVal::Num(0.0, "")),
    ]),
    ("columns-1", None, &[
        (CssPropertyId::Columns, StaticVal::Num(1.0, "")),
    ]),
    ("columns-4", None, &[
        (CssPropertyId::Columns, StaticVal::Num(4.0, "")),
    ]),
    ("columns-4xl", None, &[
        (CssPropertyId::Columns, StaticVal::Num(56.0, "rem")),
    ]),
    ("columns-auto", None, &[
        (CssPropertyId::Columns, StaticVal::Kw("auto")),
    ]),
    ("columns-lg", None, &[
        (CssPropertyId::Columns, StaticVal::Num(32.0, "rem")),
    ]),
    ("columns-md", None, &[
        (CssPropertyId::Columns, StaticVal::Num(28.0, "rem")),
    ]),
    ("columns-sm", None, &[
        (CssPropertyId::Columns, StaticVal::Num(24.0, "rem")),
    ]),
    ("columns-xl", None, &[
        (CssPropertyId::Columns, StaticVal::Num(36.0, "rem")),
    ]),
    ("columns-xs", None, &[
        (CssPropertyId::Columns, StaticVal::Num(20.0, "rem")),
    ]),
    ("delay-100", None, &[
        (CssPropertyId::TransitionDelay, StaticVal::Num(100.0, "ms")),
    ]),
    ("delay-300", None, &[
        (CssPropertyId::TransitionDelay, StaticVal::Num(300.0, "ms")),
    ]),
    ("delay-75", None, &[
        (CssPropertyId::TransitionDelay, StaticVal::Num(75.0, "ms")),
    ]),
    ("duration-100", None, &[
        (CssPropertyId::TransitionDuration, StaticVal::Num(100.0, "ms")),
    ]),
    ("duration-300", None, &[
        (CssPropertyId::TransitionDuration, StaticVal::Num(300.0, "ms")),
    ]),
    ("duration-75", None, &[
        (CssPropertyId::TransitionDuration, StaticVal::Num(75.0, "ms")),
    ]),
    ("font-black", None, &[
        (CssPropertyId::FontWeight, StaticVal::Num(900.0, "")),
    ]),
    ("font-mono", None, &[
        (CssPropertyId::FontFamily, StaticVal::Literal("ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace")),
    ]),
    ("font-thin", None, &[
        (CssPropertyId::FontWeight, StaticVal::Num(100.0, "")),
    ]),
    ("gap-0", None, &[
        (CssPropertyId::Gap, StaticVal::Num(0.0, "px")),
    ]),
    ("gap-1", None, &[
        (CssPropertyId::Gap, StaticVal::Num(0.25, "rem")),
    ]),
    ("gap-36", None, &[
        (CssPropertyId::Gap, StaticVal::Num(9.0, "rem")),
    ]),
    ("gap-4", None, &[
        (CssPropertyId::Gap, StaticVal::Num(1.0, "rem")),
    ]),
    ("gap-px", None, &[
        (CssPropertyId::Gap, StaticVal::Num(1.0, "px")),
    ]),
    ("h-0", None, &[
        (CssPropertyId::Height, StaticVal::Num(0.0, "px")),
    ]),
    ("h-1", None, &[
        (CssPropertyId::Height, StaticVal::Num(0.25, "rem")),
    ]),
    ("h-4", None, &[
        (CssPropertyId::Height, StaticVal::Num(1.0, "rem")),
    ]),
    ("h-auto", None, &[
        (CssPropertyId::Height, StaticVal::Kw("auto")),
    ]),
    ("h-full", None, &[
        (CssPropertyId::Height, StaticVal::Num(100.0, "%")),
    ]),
    ("h-px", None, &[
        (CssPropertyId::Height, StaticVal::Num(1.0, "px")),
    ]),
    ("h-screen", None, &[
        (CssPropertyId::Height, StaticVal::Num(100.0, "vh")),
    ]),
    ("inset-0", None, &[
        (CssPropertyId::Top, StaticVal::Num(0.0, "px")),
        (CssPropertyId::Right, StaticVal::Num(0.0, "px")),
        (CssPropertyId::Bottom, StaticVal::Num(0.0, "px")),
        (CssPropertyId::Left, StaticVal::Num(0.0, "px")),
    ]),
    ("inset-1", None, &[
        (CssPropertyId::Top, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::Right, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::Bottom, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::Left, StaticVal::Num(0.25, "rem")),
    ]),
    ("inset-32", None, &[
        (CssPropertyId::Top, StaticVal::Num(8.0, "rem")),
        (CssPropertyId::Right, StaticVal::Num(8.0, "rem")),
        (CssPropertyId::Bottom, StaticVal::Num(8.0, "rem")),
        (CssPropertyId::Left, StaticVal::Num(8.0, "rem")),
    ]),
    ("inset-4", None, &[
        (CssPropertyId::Top, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::Right, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::Bottom, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::Left, StaticVal::Num(1.0, "rem")),
    ]),
    ("inset-auto", None, &[
        (CssPropertyId::Top, StaticVal::Kw("auto")),
        (CssPropertyId::Right, StaticVal::Kw("auto")),
        (CssPropertyId::Bottom, StaticVal::Kw("auto")),
        (CssPropertyId::Left, StaticVal::Kw("auto")),
    ]),
    ("inset-full", None, &[
        (CssPropertyId::Top, StaticVal::Num(100.0, "%")),
        (CssPropertyId::Right, StaticVal::Num(100.0, "%")),
        (CssPropertyId::Bottom, StaticVal::Num(100.0, "%")),
        (CssPropertyId::Left, StaticVal::Num(100.0, "%")),
    ]),
    ("inset-px", None, &[
        (CssPropertyId::Top, StaticVal::Num(1.0, "px")),
        (CssPropertyId::Right, StaticVal::Num(1.0, "px")),
        (CssPropertyId::Bottom, StaticVal::Num(1.0, "px")),
        (CssPropertyId::Left, StaticVal::Num(1.0, "px")),
    ]),
    ("inset-shadow-2xs", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("inset 0 1px 1px 0 rgba(0, 0, 0, 0.05)")),
    ]),
    ("inset-shadow-none", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("inset 0 0 #0000")),
    ]),
    ("inset-shadow-sm", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("inset 0 1px 3px 0 rgba(0, 0, 0, 0.1)")),
    ]),
    ("inset-shadow-xs", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("inset 0 1px 2px 0 rgba(0, 0, 0, 0.05)")),
    ]),
    ("leading-0", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(0.0, "px")),
    ]),
    ("leading-1", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(0.25, "rem")),
    ]),
    ("leading-4", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(1.0, "rem")),
    ]),
    ("leading-44", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(11.0, "rem")),
    ]),
    ("leading-none", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(1.0, "")),
    ]),
    ("leading-px", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(1.0, "px")),
    ]),
    ("leading-tight", None, &[
        (CssPropertyId::LineHeight, StaticVal::Num(1.25, "")),
    ]),
    ("left-0", None, &[
        (CssPropertyId::Left, StaticVal::Num(0.0, "px")),
    ]),
    ("left-1", None, &[
        (CssPropertyId::Left, StaticVal::Num(0.25, "rem")),
    ]),
    ("left-32", None, &[
        (CssPropertyId::Left, StaticVal::Num(8.0, "rem")),
    ]),
    ("left-4", None, &[
        (CssPropertyId::Left, StaticVal::Num(1.0, "rem")),
    ]),
    ("left-auto", None, &[
        (CssPropertyId::Left, StaticVal::Kw("auto")),
    ]),
    ("left-full", None, &[
        (CssPropertyId::Left, StaticVal::Num(100.0, "%")),
    ]),
    ("left-px", None, &[
        (CssPropertyId::Left, StaticVal::Num(1.0, "px")),
    ]),
    ("m-0", None, &[
        (CssPropertyId::Margin, StaticVal::Num(0.0, "px")),
    ]),
    ("m-1", None, &[
        (CssPropertyId::Margin, StaticVal::Num(0.25, "rem")),
    ]),
    ("m-4", None, &[
        (CssPropertyId::Margin, StaticVal::Num(1.0, "rem")),
    ]),
    ("m-auto", None, &[
        (CssPropertyId::Margin, StaticVal::Kw("auto")),
    ]),
    ("m-px", None, &[
        (CssPropertyId::Margin, StaticVal::Num(1.0, "px")),
    ]),
    ("max-h-0", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(0.0, "px")),
    ]),
    ("max-h-1", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(0.25, "rem")),
    ]),
    ("max-h-4", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(1.0, "rem")),
    ]),
    ("max-h-full", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(100.0, "%")),
    ]),
    ("max-h-none", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Kw("none")),
    ]),
    ("max-h-px", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(1.0, "px")),
    ]),
    ("max-h-screen", None, &[
        (CssPropertyId::MaxHeight, StaticVal::Num(100.0, "vh")),
    ]),
    ("max-w-0", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(0.0, "rem")),
    ]),
    ("max-w-1", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(0.25, "rem")),
    ]),
    ("max-w-4", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(1.0, "rem")),
    ]),
    ("max-w-44", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(11.0, "rem")),
    ]),
    ("max-w-full", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(100.0, "%")),
    ]),
    ("max-w-lg", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(32.0, "rem")),
    ]),
    ("max-w-md", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(28.0, "rem")),
    ]),
    ("max-w-none", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Kw("none")),
    ]),
    ("max-w-px", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("max-w-sm", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(24.0, "rem")),
    ]),
    ("max-w-xl", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(36.0, "rem")),
    ]),
    ("max-w-xs", None, &[
        (CssPropertyId::MaxWidth, StaticVal::Num(20.0, "rem")),
    ]),
    ("mb-0", None, &[
        (CssPropertyId::MarginBottom, StaticVal::Num(0.0, "px")),
    ]),
    ("mb-1", None, &[
        (CssPropertyId::MarginBottom, StaticVal::Num(0.25, "rem")),
    ]),
    ("mb-4", None, &[
        (CssPropertyId::MarginBottom, StaticVal::Num(1.0, "rem")),
    ]),
    ("mb-auto", None, &[
        (CssPropertyId::MarginBottom, StaticVal::Kw("auto")),
    ]),
    ("mb-px", None, &[
        (CssPropertyId::MarginBottom, StaticVal::Num(1.0, "px")),
    ]),
    ("min-h-0", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(0.0, "px")),
    ]),
    ("min-h-1", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(0.25, "rem")),
    ]),
    ("min-h-4", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(1.0, "rem")),
    ]),
    ("min-h-auto", None, &[
        (CssPropertyId::MinHeight, StaticVal::Kw("auto")),
    ]),
    ("min-h-full", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(100.0, "%")),
    ]),
    ("min-h-px", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(1.0, "px")),
    ]),
    ("min-h-screen", None, &[
        (CssPropertyId::MinHeight, StaticVal::Num(100.0, "vh")),
    ]),
    ("min-w-0", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(0.0, "px")),
    ]),
    ("min-w-1", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(0.25, "rem")),
    ]),
    ("min-w-4", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(1.0, "rem")),
    ]),
    ("min-w-44", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(11.0, "rem")),
    ]),
    ("min-w-auto", None, &[
        (CssPropertyId::MinWidth, StaticVal::Kw("auto")),
    ]),
    ("min-w-full", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(100.0, "%")),
    ]),
    ("min-w-lg", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(32.0, "rem")),
    ]),
    ("min-w-md", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(28.0, "rem")),
    ]),
    ("min-w-px", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(1.0, "px")),
    ]),
    ("min-w-sm", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(24.0, "rem")),
    ]),
    ("min-w-xl", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(36.0, "rem")),
    ]),
    ("min-w-xs", None, &[
        (CssPropertyId::MinWidth, StaticVal::Num(20.0, "rem")),
    ]),
    ("ml-0", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(0.0, "px")),
    ]),
    ("ml-1", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(0.25, "rem")),
    ]),
    ("ml-4", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(1.0, "rem")),
    ]),
    ("ml-auto", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Kw("auto")),
    ]),
    ("ml-px", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(1.0, "px")),
    ]),
    ("mr-0", None, &[
        (CssPropertyId::MarginRight, StaticVal::Num(0.0, "px")),
    ]),
    ("mr-1", None, &[
        (CssPropertyId::MarginRight, StaticVal::Num(0.25, "rem")),
    ]),
    ("mr-4", None, &[
        (CssPropertyId::MarginRight, StaticVal::Num(1.0, "rem")),
    ]),
    ("mr-auto", None, &[
        (CssPropertyId::MarginRight, StaticVal::Kw("auto")),
    ]),
    ("mr-px", None, &[
        (CssPropertyId::MarginRight, StaticVal::Num(1.0, "px")),
    ]),
    ("mt-0", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(0.0, "px")),
    ]),
    ("mt-1", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(0.25, "rem")),
    ]),
    ("mt-4", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(1.0, "rem")),
    ]),
    ("mt-auto", None, &[
        (CssPropertyId::MarginTop, StaticVal::Kw("auto")),
    ]),
    ("mt-px", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(1.0, "px")),
    ]),
    ("mx-0", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(0.0, "px")),
        (CssPropertyId::MarginRight, StaticVal::Num(0.0, "px")),
    ]),
    ("mx-1", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::MarginRight, StaticVal::Num(0.25, "rem")),
    ]),
    ("mx-4", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::MarginRight, StaticVal::Num(1.0, "rem")),
    ]),
    ("mx-auto", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Kw("auto")),
        (CssPropertyId::MarginRight, StaticVal::Kw("auto")),
    ]),
    ("mx-px", None, &[
        (CssPropertyId::MarginLeft, StaticVal::Num(1.0, "px")),
        (CssPropertyId::MarginRight, StaticVal::Num(1.0, "px")),
    ]),
    ("my-0", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(0.0, "px")),
        (CssPropertyId::MarginBottom, StaticVal::Num(0.0, "px")),
    ]),
    ("my-1", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::MarginBottom, StaticVal::Num(0.25, "rem")),
    ]),
    ("my-4", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::MarginBottom, StaticVal::Num(1.0, "rem")),
    ]),
    ("my-auto", None, &[
        (CssPropertyId::MarginTop, StaticVal::Kw("auto")),
        (CssPropertyId::MarginBottom, StaticVal::Kw("auto")),
    ]),
    ("my-px", None, &[
        (CssPropertyId::MarginTop, StaticVal::Num(1.0, "px")),
        (CssPropertyId::MarginBottom, StaticVal::Num(1.0, "px")),
    ]),
    ("opacity-0", None, &[
        (CssPropertyId::Opacity, StaticVal::Num(0.0, "")),
    ]),
    ("opacity-100", None, &[
        (CssPropertyId::Opacity, StaticVal::Num(1.0, "")),
    ]),
    ("opacity-5", None, &[
        (CssPropertyId::Opacity, StaticVal::Num(0.05, "")),
    ]),
    ("opacity-50", None, &[
        (CssPropertyId::Opacity, StaticVal::Num(0.5, "")),
    ]),
    ("opacity-95", None, &[
        (CssPropertyId::Opacity, StaticVal::Num(0.95, "")),
    ]),
    ("p-0", None, &[
        (CssPropertyId::Padding, StaticVal::Num(0.0, "px")),
    ]),
    ("p-1", None, &[
        (CssPropertyId::Padding, StaticVal::Num(0.25, "rem")),
    ]),
    ("p-36", None, &[
        (CssPropertyId::Padding, StaticVal::Num(9.0, "rem")),
    ]),
    ("p-4", None, &[
        (CssPropertyId::Padding, StaticVal::Num(1.0, "rem")),
    ]),
    ("p-px", None, &[
        (CssPropertyId::Padding, StaticVal::Num(1.0, "px")),
    ]),
    ("pb-0", None, &[
        (CssPropertyId::PaddingBottom, StaticVal::Num(0.0, "px")),
    ]),
    ("pb-1", None, &[
        (CssPropertyId::PaddingBottom, StaticVal::Num(0.25, "rem")),
    ]),
    ("pb-36", None, &[
        (CssPropertyId::PaddingBottom, StaticVal::Num(9.0, "rem")),
    ]),
    ("pb-4", None, &[
        (CssPropertyId::PaddingBottom, StaticVal::Num(1.0, "rem")),
    ]),
    ("pb-px", None, &[
        (CssPropertyId::PaddingBottom, StaticVal::Num(1.0, "px")),
    ]),
    ("pl-0", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(0.0, "px")),
    ]),
    ("pl-1", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(0.25, "rem")),
    ]),
    ("pl-36", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(9.0, "rem")),
    ]),
    ("pl-4", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(1.0, "rem")),
    ]),
    ("pl-px", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(1.0, "px")),
    ]),
    ("pr-0", None, &[
        (CssPropertyId::PaddingRight, StaticVal::Num(0.0, "px")),
    ]),
    ("pr-1", None, &[
        (CssPropertyId::PaddingRight, StaticVal::Num(0.25, "rem")),
    ]),
    ("pr-36", None, &[
        (CssPropertyId::PaddingRight, StaticVal::Num(9.0, "rem")),
    ]),
    ("pr-4", None, &[
        (CssPropertyId::PaddingRight, StaticVal::Num(1.0, "rem")),
    ]),
    ("pr-px", None, &[
        (CssPropertyId::PaddingRight, StaticVal::Num(1.0, "px")),
    ]),
    ("pt-0", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(0.0, "px")),
    ]),
    ("pt-1", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(0.25, "rem")),
    ]),
    ("pt-36", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(9.0, "rem")),
    ]),
    ("pt-4", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(1.0, "rem")),
    ]),
    ("pt-px", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(1.0, "px")),
    ]),
    ("px-0", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(0.0, "px")),
        (CssPropertyId::PaddingRight, StaticVal::Num(0.0, "px")),
    ]),
    ("px-1", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::PaddingRight, StaticVal::Num(0.25, "rem")),
    ]),
    ("px-36", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(9.0, "rem")),
        (CssPropertyId::PaddingRight, StaticVal::Num(9.0, "rem")),
    ]),
    ("px-4", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::PaddingRight, StaticVal::Num(1.0, "rem")),
    ]),
    ("px-px", None, &[
        (CssPropertyId::PaddingLeft, StaticVal::Num(1.0, "px")),
        (CssPropertyId::PaddingRight, StaticVal::Num(1.0, "px")),
    ]),
    ("py-0", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(0.0, "px")),
        (CssPropertyId::PaddingBottom, StaticVal::Num(0.0, "px")),
    ]),
    ("py-1", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::PaddingBottom, StaticVal::Num(0.25, "rem")),
    ]),
    ("py-36", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(9.0, "rem")),
        (CssPropertyId::PaddingBottom, StaticVal::Num(9.0, "rem")),
    ]),
    ("py-4", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::PaddingBottom, StaticVal::Num(1.0, "rem")),
    ]),
    ("py-px", None, &[
        (CssPropertyId::PaddingTop, StaticVal::Num(1.0, "px")),
        (CssPropertyId::PaddingBottom, StaticVal::Num(1.0, "px")),
    ]),
    ("right-0", None, &[
        (CssPropertyId::Right, StaticVal::Num(0.0, "px")),
    ]),
    ("right-1", None, &[
        (CssPropertyId::Right, StaticVal::Num(0.25, "rem")),
    ]),
    ("right-32", None, &[
        (CssPropertyId::Right, StaticVal::Num(8.0, "rem")),
    ]),
    ("right-4", None, &[
        (CssPropertyId::Right, StaticVal::Num(1.0, "rem")),
    ]),
    ("right-auto", None, &[
        (CssPropertyId::Right, StaticVal::Kw("auto")),
    ]),
    ("right-full", None, &[
        (CssPropertyId::Right, StaticVal::Num(100.0, "%")),
    ]),
    ("right-px", None, &[
        (CssPropertyId::Right, StaticVal::Num(1.0, "px")),
    ]),
    ("ring", None, &[
        (CssPropertyId::VarTwRingWidth, StaticVal::Num(0.1875, "rem")),
        (CssPropertyId::BoxShadow, StaticVal::RingShadow),
    ]),
    ("rotate-0", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("rotate(0deg)")),
    ]),
    ("rotate-45", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("rotate(45deg)")),
    ]),
    ("rotate-90", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("rotate(90deg)")),
    ]),
    ("rounded-2xl", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-b-2xl", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-b-full", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-b-lg", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-b-md", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-b-none", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-b-sm", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-b-xl", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-b-xs", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-bl-2xl", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-bl-full", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-bl-lg", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-bl-md", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-bl-none", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-bl-sm", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-bl-xl", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-bl-xs", None, &[
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-br-2xl", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-br-full", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-br-lg", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-br-md", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-br-none", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-br-sm", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-br-xl", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-br-xs", None, &[
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-e-2xl", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-e-full", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-e-lg", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-e-md", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-e-none", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-e-sm", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-e-xl", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-e-xs", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-ee-2xl", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-ee-full", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-ee-lg", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-ee-md", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-ee-none", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-ee-sm", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-ee-xl", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-ee-xs", None, &[
        (CssPropertyId::BorderEndEndRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-es-2xl", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-es-full", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-es-lg", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-es-md", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-es-none", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-es-sm", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-es-xl", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-es-xs", None, &[
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-full", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-l-2xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-l-full", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-l-lg", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-l-md", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-l-none", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-l-sm", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-l-xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-l-xs", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderBottomLeftRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-lg", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-md", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-none", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-r-2xl", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-r-full", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-r-lg", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-r-md", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-r-none", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-r-sm", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-r-xl", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-r-xs", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderBottomRightRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-s-2xl", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-s-full", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-s-lg", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-s-md", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-s-none", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-s-sm", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-s-xl", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-s-xs", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderEndStartRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-se-2xl", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-se-full", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-se-lg", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-se-md", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-se-none", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-se-sm", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-se-xl", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-se-xs", None, &[
        (CssPropertyId::BorderStartEndRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-sm", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-ss-2xl", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-ss-full", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-ss-lg", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-ss-md", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-ss-none", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-ss-sm", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-ss-xl", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-ss-xs", None, &[
        (CssPropertyId::BorderStartStartRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-t-2xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(1.0, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-t-full", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-t-lg", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.5, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-t-md", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.375, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-t-none", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.0, "px")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-t-sm", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.25, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-t-xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-t-xs", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.125, "rem")),
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tl-2xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-tl-full", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-tl-lg", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-tl-md", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-tl-none", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-tl-sm", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-tl-xl", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-tl-xs", None, &[
        (CssPropertyId::BorderTopLeftRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tr-2xl", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-tr-full", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Kw("calc(infinity * 1px)")),
    ]),
    ("rounded-tr-lg", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-tr-md", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-tr-none", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-tr-sm", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.25, "rem")),
    ]),
    ("rounded-tr-xl", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-tr-xs", None, &[
        (CssPropertyId::BorderTopRightRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-xl", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-xs", None, &[
        (CssPropertyId::BorderRadius, StaticVal::Num(0.125, "rem")),
    ]),
    ("scale-0", None, &[
        (CssPropertyId::Scale, StaticVal::Num(0.0, "")),
    ]),
    ("scale-100", None, &[
        (CssPropertyId::Scale, StaticVal::Num(1.0, "")),
    ]),
    ("scale-150", None, &[
        (CssPropertyId::Scale, StaticVal::Num(1.5, "")),
    ]),
    ("scale-50", None, &[
        (CssPropertyId::Scale, StaticVal::Num(0.5, "")),
    ]),
    ("scale-95", None, &[
        (CssPropertyId::Scale, StaticVal::Num(0.95, "")),
    ]),
    ("shadow-2xl", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("0 25px 50px -12px rgba(0, 0, 0, 0.25)")),
    ]),
    ("shadow-lg", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-md", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-none", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Kw("none")),
    ]),
    ("shadow-sm", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-xl", None, &[
        (CssPropertyId::BoxShadow, StaticVal::Literal("0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-xs", None, &[
        (CssPropertyId::VarTwShadow, StaticVal::Literal("0 1px 2px 0 rgba(0, 0, 0, 0.05)")),
        (CssPropertyId::BoxShadow, StaticVal::Literal("var(--tw-ring-offset-shadow, 0 0 #0000), var(--tw-ring-shadow, 0 0 #0000), var(--tw-shadow, 0 1px 2px 0 rgba(0, 0, 0, 0.05))")),
    ]),
    ("text-2xl", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(1.5, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(2.0, "rem")),
    ]),
    ("text-8xl", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(6.0, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(1.0, "")),
    ]),
    ("text-lg", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(1.125, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(1.75, "rem")),
    ]),
    ("text-sm", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(0.875, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(1.25, "rem")),
    ]),
    ("text-xl", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(1.25, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(1.75, "rem")),
    ]),
    ("text-xs", None, &[
        (CssPropertyId::FontSize, StaticVal::Num(0.75, "rem")),
        (CssPropertyId::LineHeight, StaticVal::Num(1.0, "rem")),
    ]),
    ("top-0", None, &[
        (CssPropertyId::Top, StaticVal::Num(0.0, "px")),
    ]),
    ("top-1", None, &[
        (CssPropertyId::Top, StaticVal::Num(0.25, "rem")),
    ]),
    ("top-32", None, &[
        (CssPropertyId::Top, StaticVal::Num(8.0, "rem")),
    ]),
    ("top-4", None, &[
        (CssPropertyId::Top, StaticVal::Num(1.0, "rem")),
    ]),
    ("top-auto", None, &[
        (CssPropertyId::Top, StaticVal::Kw("auto")),
    ]),
    ("top-full", None, &[
        (CssPropertyId::Top, StaticVal::Num(100.0, "%")),
    ]),
    ("top-px", None, &[
        (CssPropertyId::Top, StaticVal::Num(1.0, "px")),
    ]),
    ("tracking-normal", None, &[
        (CssPropertyId::LetterSpacing, StaticVal::Num(0.0, "em")),
    ]),
    ("tracking-wide", None, &[
        (CssPropertyId::LetterSpacing, StaticVal::Num(0.025, "em")),
    ]),
    ("tracking-widest", None, &[
        (CssPropertyId::LetterSpacing, StaticVal::Num(0.1, "em")),
    ]),
    ("translate-x-0", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateX(0px)")),
    ]),
    ("translate-x-1/2", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateX(50%)")),
    ]),
    ("translate-x-full", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateX(100%)")),
    ]),
    ("translate-y-0", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateY(0px)")),
    ]),
    ("translate-y-1/2", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateY(50%)")),
    ]),
    ("translate-y-full", None, &[
        (CssPropertyId::Transform, StaticVal::Kw("translateY(100%)")),
    ]),
    ("w-0", None, &[
        (CssPropertyId::Width, StaticVal::Num(0.0, "px")),
    ]),
    ("w-1", None, &[
        (CssPropertyId::Width, StaticVal::Num(0.25, "rem")),
    ]),
    ("w-4", None, &[
        (CssPropertyId::Width, StaticVal::Num(1.0, "rem")),
    ]),
    ("w-44", None, &[
        (CssPropertyId::Width, StaticVal::Num(11.0, "rem")),
    ]),
    ("w-auto", None, &[
        (CssPropertyId::Width, StaticVal::Kw("auto")),
    ]),
    ("w-full", None, &[
        (CssPropertyId::Width, StaticVal::Num(100.0, "%")),
    ]),
    ("w-lg", None, &[
        (CssPropertyId::Width, StaticVal::Num(32.0, "rem")),
    ]),
    ("w-md", None, &[
        (CssPropertyId::Width, StaticVal::Num(28.0, "rem")),
    ]),
    ("w-px", None, &[
        (CssPropertyId::Width, StaticVal::Num(1.0, "px")),
    ]),
    ("w-sm", None, &[
        (CssPropertyId::Width, StaticVal::Num(24.0, "rem")),
    ]),
    ("w-xl", None, &[
        (CssPropertyId::Width, StaticVal::Num(36.0, "rem")),
    ]),
    ("w-xs", None, &[
        (CssPropertyId::Width, StaticVal::Num(20.0, "rem")),
    ]),
    ("z-0", None, &[
        (CssPropertyId::ZIndex, StaticVal::Num(0.0, "")),
    ]),
    ("z-30", None, &[
        (CssPropertyId::ZIndex, StaticVal::Num(30.0, "")),
    ]),
    ("z-50", None, &[
        (CssPropertyId::ZIndex, StaticVal::Num(50.0, "")),
    ]),
    ("z-auto", None, &[
        (CssPropertyId::ZIndex, StaticVal::Kw("auto")),
    ]),
];

#[derive(Clone, Copy)]
pub enum StaticVal {
    Kw(&'static str),
    Num(f64, &'static str),
    Hex(&'static str),
    Literal(&'static str),
    RingShadow,
}
