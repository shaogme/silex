// 自动生成的 Tailwind 测试用例规则表（用于验证 test-cases 的生成与 CSS 规则解析正确性）
// 对应 tailwind-classes.json 中的 test_cases
// 避免手写硬编码，与 silex_codegen/resolver 保持 100% 规则对齐

#[allow(unused_imports)]
use crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};
#[allow(unused_imports)]
use crate::css::tw::resolver::make_rule;
#[allow(unused_imports)]
use proc_macro2::Span;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum StaticVal {
    Kw(&'static str),
    Num(f64, &'static str),
    Hex(&'static str),
    Literal(&'static str),
    RingShadow,
}

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

#[rustfmt::skip]
pub static TEST_CASE_RULES: &[(&str, &[(&str, StaticVal)])] = &[
    ("animate-bounce", &[
        ("animation", StaticVal::Literal("bounce 1s infinite")),
        ("will-change", StaticVal::Kw("transform")),
    ]),
    ("animate-none", &[
        ("animation", StaticVal::Kw("none")),
    ]),
    ("animate-out", &[
        ("animation-name", StaticVal::Kw("exit")),
        ("animation-duration", StaticVal::Num(150.0, "ms")),
    ]),
    ("animate-spin", &[
        ("animation", StaticVal::Literal("spin 1s linear infinite")),
        ("will-change", StaticVal::Kw("transform")),
    ]),
    ("backdrop-blur-2xl", &[
        ("backdrop-filter", StaticVal::Kw("blur(40px)")),
    ]),
    ("backdrop-blur-lg", &[
        ("backdrop-filter", StaticVal::Kw("blur(16px)")),
    ]),
    ("backdrop-blur-md", &[
        ("backdrop-filter", StaticVal::Kw("blur(8px)")),
    ]),
    ("backdrop-blur-none", &[
        ("backdrop-filter", StaticVal::Kw("blur(0px)")),
    ]),
    ("backdrop-blur-sm", &[
        ("backdrop-filter", StaticVal::Kw("blur(4px)")),
    ]),
    ("backdrop-blur-xl", &[
        ("backdrop-filter", StaticVal::Kw("blur(24px)")),
    ]),
    ("backdrop-blur-xs", &[
        ("backdrop-filter", StaticVal::Kw("blur(2px)")),
    ]),
    ("bg-conic", &[
        ("background-image", StaticVal::Kw("conic-gradient(var(--tw-gradient-stops))")),
    ]),
    ("bg-linear-to-b", &[
        ("background-image", StaticVal::Kw("linear-gradient(to bottom, var(--tw-gradient-stops))")),
    ]),
    ("bg-none", &[
        ("background-image", StaticVal::Kw("none")),
    ]),
    ("bg-radial", &[
        ("background-image", StaticVal::Kw("radial-gradient(var(--tw-gradient-stops))")),
    ]),
    ("blur-2xl", &[
        ("filter", StaticVal::Kw("blur(40px)")),
    ]),
    ("blur-lg", &[
        ("filter", StaticVal::Kw("blur(16px)")),
    ]),
    ("blur-md", &[
        ("filter", StaticVal::Kw("blur(8px)")),
    ]),
    ("blur-none", &[
        ("filter", StaticVal::Kw("none")),
    ]),
    ("blur-sm", &[
        ("filter", StaticVal::Kw("blur(4px)")),
    ]),
    ("blur-xl", &[
        ("filter", StaticVal::Kw("blur(24px)")),
    ]),
    ("blur-xs", &[
        ("filter", StaticVal::Kw("blur(2px)")),
    ]),
    ("border-0", &[
        ("border-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-1", &[
        ("border-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-4", &[
        ("border-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-b-0", &[
        ("border-bottom-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-b-1", &[
        ("border-bottom-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-b-2", &[
        ("border-bottom-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-b-4", &[
        ("border-bottom-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-b-8", &[
        ("border-bottom-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-be-0", &[
        ("border-block-end-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-be-1", &[
        ("border-block-end-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-be-2", &[
        ("border-block-end-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-be-4", &[
        ("border-block-end-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-be-8", &[
        ("border-block-end-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-bs-0", &[
        ("border-block-start-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-bs-1", &[
        ("border-block-start-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-bs-2", &[
        ("border-block-start-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-bs-4", &[
        ("border-block-start-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-bs-8", &[
        ("border-block-start-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-dashed", &[
        ("border-style", StaticVal::Kw("dashed")),
    ]),
    ("border-e-0", &[
        ("border-inline-end-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-e-1", &[
        ("border-inline-end-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-e-2", &[
        ("border-inline-end-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-e-4", &[
        ("border-inline-end-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-e-8", &[
        ("border-inline-end-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-l-0", &[
        ("border-left-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-l-1", &[
        ("border-left-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-l-2", &[
        ("border-left-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-l-4", &[
        ("border-left-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-l-8", &[
        ("border-left-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-none", &[
        ("border-style", StaticVal::Kw("none")),
    ]),
    ("border-r-0", &[
        ("border-right-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-r-1", &[
        ("border-right-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-r-2", &[
        ("border-right-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-r-4", &[
        ("border-right-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-r-8", &[
        ("border-right-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-s-0", &[
        ("border-inline-start-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-s-1", &[
        ("border-inline-start-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-s-2", &[
        ("border-inline-start-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-s-4", &[
        ("border-inline-start-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-s-8", &[
        ("border-inline-start-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-solid", &[
        ("border-style", StaticVal::Kw("solid")),
    ]),
    ("border-t-0", &[
        ("border-top-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-t-1", &[
        ("border-top-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-t-2", &[
        ("border-top-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-t-4", &[
        ("border-top-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-t-8", &[
        ("border-top-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-x-0", &[
        ("border-left-width", StaticVal::Num(0.0, "px")),
        ("border-right-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-x-1", &[
        ("border-left-width", StaticVal::Num(1.0, "px")),
        ("border-right-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-x-2", &[
        ("border-left-width", StaticVal::Num(2.0, "px")),
        ("border-right-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-x-4", &[
        ("border-left-width", StaticVal::Num(4.0, "px")),
        ("border-right-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-x-8", &[
        ("border-left-width", StaticVal::Num(8.0, "px")),
        ("border-right-width", StaticVal::Num(8.0, "px")),
    ]),
    ("border-y-0", &[
        ("border-top-width", StaticVal::Num(0.0, "px")),
        ("border-bottom-width", StaticVal::Num(0.0, "px")),
    ]),
    ("border-y-1", &[
        ("border-top-width", StaticVal::Num(1.0, "px")),
        ("border-bottom-width", StaticVal::Num(1.0, "px")),
    ]),
    ("border-y-2", &[
        ("border-top-width", StaticVal::Num(2.0, "px")),
        ("border-bottom-width", StaticVal::Num(2.0, "px")),
    ]),
    ("border-y-4", &[
        ("border-top-width", StaticVal::Num(4.0, "px")),
        ("border-bottom-width", StaticVal::Num(4.0, "px")),
    ]),
    ("border-y-8", &[
        ("border-top-width", StaticVal::Num(8.0, "px")),
        ("border-bottom-width", StaticVal::Num(8.0, "px")),
    ]),
    ("bottom-0", &[
        ("bottom", StaticVal::Num(0.0, "px")),
    ]),
    ("bottom-1", &[
        ("bottom", StaticVal::Num(0.25, "rem")),
    ]),
    ("bottom-32", &[
        ("bottom", StaticVal::Num(8.0, "rem")),
    ]),
    ("bottom-4", &[
        ("bottom", StaticVal::Num(1.0, "rem")),
    ]),
    ("bottom-auto", &[
        ("bottom", StaticVal::Kw("auto")),
    ]),
    ("bottom-full", &[
        ("bottom", StaticVal::Num(100.0, "%")),
    ]),
    ("bottom-px", &[
        ("bottom", StaticVal::Num(1.0, "px")),
    ]),
    ("break-after-auto", &[
        ("break-after", StaticVal::Kw("auto")),
    ]),
    ("break-after-avoid-flex", &[
        ("break-after", StaticVal::Kw("avoid-flex")),
    ]),
    ("break-after-avoid-page", &[
        ("break-after", StaticVal::Kw("avoid-page")),
    ]),
    ("break-before-auto", &[
        ("break-before", StaticVal::Kw("auto")),
    ]),
    ("break-before-avoid-flex", &[
        ("break-before", StaticVal::Kw("avoid-flex")),
    ]),
    ("break-before-avoid-page", &[
        ("break-before", StaticVal::Kw("avoid-page")),
    ]),
    ("break-inside-auto", &[
        ("break-inside", StaticVal::Kw("auto")),
    ]),
    ("break-inside-avoid-column", &[
        ("break-inside", StaticVal::Kw("avoid-column")),
    ]),
    ("break-inside-avoid-page", &[
        ("break-inside", StaticVal::Kw("avoid-page")),
    ]),
    ("columns-0", &[
        ("column-count", StaticVal::Num(0.0, "")),
    ]),
    ("columns-1", &[
        ("column-count", StaticVal::Num(1.0, "")),
    ]),
    ("columns-4", &[
        ("column-count", StaticVal::Num(4.0, "")),
    ]),
    ("columns-4xl", &[
        ("column-width", StaticVal::Num(56.0, "rem")),
    ]),
    ("columns-auto", &[
        ("column-count", StaticVal::Kw("auto")),
    ]),
    ("columns-lg", &[
        ("column-width", StaticVal::Num(32.0, "rem")),
    ]),
    ("columns-md", &[
        ("column-width", StaticVal::Num(28.0, "rem")),
    ]),
    ("columns-sm", &[
        ("column-width", StaticVal::Num(24.0, "rem")),
    ]),
    ("columns-xl", &[
        ("column-width", StaticVal::Num(36.0, "rem")),
    ]),
    ("columns-xs", &[
        ("column-width", StaticVal::Num(20.0, "rem")),
    ]),
    ("delay-100", &[
        ("transition-delay", StaticVal::Num(100.0, "ms")),
    ]),
    ("delay-300", &[
        ("transition-delay", StaticVal::Num(300.0, "ms")),
    ]),
    ("delay-75", &[
        ("transition-delay", StaticVal::Num(75.0, "ms")),
    ]),
    ("duration-100", &[
        ("transition-duration", StaticVal::Num(100.0, "ms")),
    ]),
    ("duration-300", &[
        ("transition-duration", StaticVal::Num(300.0, "ms")),
    ]),
    ("duration-75", &[
        ("transition-duration", StaticVal::Num(75.0, "ms")),
    ]),
    ("font-black", &[
        ("font-weight", StaticVal::Num(900.0, "")),
    ]),
    ("font-mono", &[
        ("font-family", StaticVal::Literal("ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace")),
    ]),
    ("font-thin", &[
        ("font-weight", StaticVal::Num(100.0, "")),
    ]),
    ("gap-0", &[
        ("gap", StaticVal::Num(0.0, "px")),
    ]),
    ("gap-1", &[
        ("gap", StaticVal::Num(0.25, "rem")),
    ]),
    ("gap-36", &[
        ("gap", StaticVal::Num(9.0, "rem")),
    ]),
    ("gap-4", &[
        ("gap", StaticVal::Num(1.0, "rem")),
    ]),
    ("gap-px", &[
        ("gap", StaticVal::Num(1.0, "px")),
    ]),
    ("h-0", &[
        ("height", StaticVal::Num(0.0, "px")),
    ]),
    ("h-1", &[
        ("height", StaticVal::Num(0.25, "rem")),
    ]),
    ("h-4", &[
        ("height", StaticVal::Num(1.0, "rem")),
    ]),
    ("h-auto", &[
        ("height", StaticVal::Kw("auto")),
    ]),
    ("h-full", &[
        ("height", StaticVal::Num(100.0, "%")),
    ]),
    ("h-px", &[
        ("height", StaticVal::Num(1.0, "px")),
    ]),
    ("h-screen", &[
        ("height", StaticVal::Num(100.0, "vh")),
    ]),
    ("inset-0", &[
        ("top", StaticVal::Num(0.0, "px")),
        ("right", StaticVal::Num(0.0, "px")),
        ("bottom", StaticVal::Num(0.0, "px")),
        ("left", StaticVal::Num(0.0, "px")),
    ]),
    ("inset-1", &[
        ("top", StaticVal::Num(0.25, "rem")),
        ("right", StaticVal::Num(0.25, "rem")),
        ("bottom", StaticVal::Num(0.25, "rem")),
        ("left", StaticVal::Num(0.25, "rem")),
    ]),
    ("inset-32", &[
        ("top", StaticVal::Num(8.0, "rem")),
        ("right", StaticVal::Num(8.0, "rem")),
        ("bottom", StaticVal::Num(8.0, "rem")),
        ("left", StaticVal::Num(8.0, "rem")),
    ]),
    ("inset-4", &[
        ("top", StaticVal::Num(1.0, "rem")),
        ("right", StaticVal::Num(1.0, "rem")),
        ("bottom", StaticVal::Num(1.0, "rem")),
        ("left", StaticVal::Num(1.0, "rem")),
    ]),
    ("inset-auto", &[
        ("top", StaticVal::Kw("auto")),
        ("right", StaticVal::Kw("auto")),
        ("bottom", StaticVal::Kw("auto")),
        ("left", StaticVal::Kw("auto")),
    ]),
    ("inset-full", &[
        ("top", StaticVal::Num(100.0, "%")),
        ("right", StaticVal::Num(100.0, "%")),
        ("bottom", StaticVal::Num(100.0, "%")),
        ("left", StaticVal::Num(100.0, "%")),
    ]),
    ("inset-px", &[
        ("top", StaticVal::Num(1.0, "px")),
        ("right", StaticVal::Num(1.0, "px")),
        ("bottom", StaticVal::Num(1.0, "px")),
        ("left", StaticVal::Num(1.0, "px")),
    ]),
    ("inset-shadow-2xs", &[
        ("box-shadow", StaticVal::Literal("inset 0 1px 1px 0 rgba(0, 0, 0, 0.05)")),
    ]),
    ("inset-shadow-none", &[
        ("box-shadow", StaticVal::Literal("inset 0 0 #0000")),
    ]),
    ("inset-shadow-sm", &[
        ("box-shadow", StaticVal::Literal("inset 0 1px 3px 0 rgba(0, 0, 0, 0.1)")),
    ]),
    ("inset-shadow-xs", &[
        ("box-shadow", StaticVal::Literal("inset 0 1px 2px 0 rgba(0, 0, 0, 0.05)")),
    ]),
    ("leading-0", &[
        ("line-height", StaticVal::Num(0.0, "px")),
    ]),
    ("leading-1", &[
        ("line-height", StaticVal::Num(0.25, "rem")),
    ]),
    ("leading-4", &[
        ("line-height", StaticVal::Num(1.0, "rem")),
    ]),
    ("leading-44", &[
        ("line-height", StaticVal::Num(11.0, "rem")),
    ]),
    ("leading-none", &[
        ("line-height", StaticVal::Num(1.0, "")),
    ]),
    ("leading-px", &[
        ("line-height", StaticVal::Num(1.0, "px")),
    ]),
    ("leading-tight", &[
        ("line-height", StaticVal::Num(1.25, "")),
    ]),
    ("left-0", &[
        ("left", StaticVal::Num(0.0, "px")),
    ]),
    ("left-1", &[
        ("left", StaticVal::Num(0.25, "rem")),
    ]),
    ("left-32", &[
        ("left", StaticVal::Num(8.0, "rem")),
    ]),
    ("left-4", &[
        ("left", StaticVal::Num(1.0, "rem")),
    ]),
    ("left-auto", &[
        ("left", StaticVal::Kw("auto")),
    ]),
    ("left-full", &[
        ("left", StaticVal::Num(100.0, "%")),
    ]),
    ("left-px", &[
        ("left", StaticVal::Num(1.0, "px")),
    ]),
    ("m-0", &[
        ("margin", StaticVal::Num(0.0, "px")),
    ]),
    ("m-1", &[
        ("margin", StaticVal::Num(0.25, "rem")),
    ]),
    ("m-4", &[
        ("margin", StaticVal::Num(1.0, "rem")),
    ]),
    ("m-auto", &[
        ("margin", StaticVal::Kw("auto")),
    ]),
    ("m-px", &[
        ("margin", StaticVal::Num(1.0, "px")),
    ]),
    ("max-h-0", &[
        ("max-height", StaticVal::Num(0.0, "px")),
    ]),
    ("max-h-1", &[
        ("max-height", StaticVal::Num(0.25, "rem")),
    ]),
    ("max-h-4", &[
        ("max-height", StaticVal::Num(1.0, "rem")),
    ]),
    ("max-h-full", &[
        ("max-height", StaticVal::Num(100.0, "%")),
    ]),
    ("max-h-none", &[
        ("max-height", StaticVal::Kw("none")),
    ]),
    ("max-h-px", &[
        ("max-height", StaticVal::Num(1.0, "px")),
    ]),
    ("max-h-screen", &[
        ("max-height", StaticVal::Num(100.0, "vh")),
    ]),
    ("max-w-0", &[
        ("max-width", StaticVal::Num(0.0, "rem")),
    ]),
    ("max-w-1", &[
        ("max-width", StaticVal::Num(0.25, "rem")),
    ]),
    ("max-w-4", &[
        ("max-width", StaticVal::Num(1.0, "rem")),
    ]),
    ("max-w-44", &[
        ("max-width", StaticVal::Num(11.0, "rem")),
    ]),
    ("max-w-full", &[
        ("max-width", StaticVal::Num(100.0, "%")),
    ]),
    ("max-w-lg", &[
        ("max-width", StaticVal::Num(32.0, "rem")),
    ]),
    ("max-w-md", &[
        ("max-width", StaticVal::Num(28.0, "rem")),
    ]),
    ("max-w-none", &[
        ("max-width", StaticVal::Kw("none")),
    ]),
    ("max-w-px", &[
        ("max-width", StaticVal::Num(1.0, "px")),
    ]),
    ("max-w-sm", &[
        ("max-width", StaticVal::Num(24.0, "rem")),
    ]),
    ("max-w-xl", &[
        ("max-width", StaticVal::Num(36.0, "rem")),
    ]),
    ("max-w-xs", &[
        ("max-width", StaticVal::Num(20.0, "rem")),
    ]),
    ("mb-0", &[
        ("margin-bottom", StaticVal::Num(0.0, "px")),
    ]),
    ("mb-1", &[
        ("margin-bottom", StaticVal::Num(0.25, "rem")),
    ]),
    ("mb-4", &[
        ("margin-bottom", StaticVal::Num(1.0, "rem")),
    ]),
    ("mb-auto", &[
        ("margin-bottom", StaticVal::Kw("auto")),
    ]),
    ("mb-px", &[
        ("margin-bottom", StaticVal::Num(1.0, "px")),
    ]),
    ("min-h-0", &[
        ("min-height", StaticVal::Num(0.0, "px")),
    ]),
    ("min-h-1", &[
        ("min-height", StaticVal::Num(0.25, "rem")),
    ]),
    ("min-h-4", &[
        ("min-height", StaticVal::Num(1.0, "rem")),
    ]),
    ("min-h-auto", &[
        ("min-height", StaticVal::Kw("auto")),
    ]),
    ("min-h-full", &[
        ("min-height", StaticVal::Num(100.0, "%")),
    ]),
    ("min-h-px", &[
        ("min-height", StaticVal::Num(1.0, "px")),
    ]),
    ("min-h-screen", &[
        ("min-height", StaticVal::Num(100.0, "vh")),
    ]),
    ("min-w-0", &[
        ("min-width", StaticVal::Num(0.0, "px")),
    ]),
    ("min-w-1", &[
        ("min-width", StaticVal::Num(0.25, "rem")),
    ]),
    ("min-w-4", &[
        ("min-width", StaticVal::Num(1.0, "rem")),
    ]),
    ("min-w-44", &[
        ("min-width", StaticVal::Num(11.0, "rem")),
    ]),
    ("min-w-auto", &[
        ("min-width", StaticVal::Kw("auto")),
    ]),
    ("min-w-full", &[
        ("min-width", StaticVal::Num(100.0, "%")),
    ]),
    ("min-w-lg", &[
        ("min-width", StaticVal::Num(32.0, "rem")),
    ]),
    ("min-w-md", &[
        ("min-width", StaticVal::Num(28.0, "rem")),
    ]),
    ("min-w-px", &[
        ("min-width", StaticVal::Num(1.0, "px")),
    ]),
    ("min-w-sm", &[
        ("min-width", StaticVal::Num(24.0, "rem")),
    ]),
    ("min-w-xl", &[
        ("min-width", StaticVal::Num(36.0, "rem")),
    ]),
    ("min-w-xs", &[
        ("min-width", StaticVal::Num(20.0, "rem")),
    ]),
    ("ml-0", &[
        ("margin-left", StaticVal::Num(0.0, "px")),
    ]),
    ("ml-1", &[
        ("margin-left", StaticVal::Num(0.25, "rem")),
    ]),
    ("ml-4", &[
        ("margin-left", StaticVal::Num(1.0, "rem")),
    ]),
    ("ml-auto", &[
        ("margin-left", StaticVal::Kw("auto")),
    ]),
    ("ml-px", &[
        ("margin-left", StaticVal::Num(1.0, "px")),
    ]),
    ("mr-0", &[
        ("margin-right", StaticVal::Num(0.0, "px")),
    ]),
    ("mr-1", &[
        ("margin-right", StaticVal::Num(0.25, "rem")),
    ]),
    ("mr-4", &[
        ("margin-right", StaticVal::Num(1.0, "rem")),
    ]),
    ("mr-auto", &[
        ("margin-right", StaticVal::Kw("auto")),
    ]),
    ("mr-px", &[
        ("margin-right", StaticVal::Num(1.0, "px")),
    ]),
    ("mt-0", &[
        ("margin-top", StaticVal::Num(0.0, "px")),
    ]),
    ("mt-1", &[
        ("margin-top", StaticVal::Num(0.25, "rem")),
    ]),
    ("mt-4", &[
        ("margin-top", StaticVal::Num(1.0, "rem")),
    ]),
    ("mt-auto", &[
        ("margin-top", StaticVal::Kw("auto")),
    ]),
    ("mt-px", &[
        ("margin-top", StaticVal::Num(1.0, "px")),
    ]),
    ("mx-0", &[
        ("margin-left", StaticVal::Num(0.0, "px")),
        ("margin-right", StaticVal::Num(0.0, "px")),
    ]),
    ("mx-1", &[
        ("margin-left", StaticVal::Num(0.25, "rem")),
        ("margin-right", StaticVal::Num(0.25, "rem")),
    ]),
    ("mx-4", &[
        ("margin-left", StaticVal::Num(1.0, "rem")),
        ("margin-right", StaticVal::Num(1.0, "rem")),
    ]),
    ("mx-auto", &[
        ("margin-left", StaticVal::Kw("auto")),
        ("margin-right", StaticVal::Kw("auto")),
    ]),
    ("mx-px", &[
        ("margin-left", StaticVal::Num(1.0, "px")),
        ("margin-right", StaticVal::Num(1.0, "px")),
    ]),
    ("my-0", &[
        ("margin-top", StaticVal::Num(0.0, "px")),
        ("margin-bottom", StaticVal::Num(0.0, "px")),
    ]),
    ("my-1", &[
        ("margin-top", StaticVal::Num(0.25, "rem")),
        ("margin-bottom", StaticVal::Num(0.25, "rem")),
    ]),
    ("my-4", &[
        ("margin-top", StaticVal::Num(1.0, "rem")),
        ("margin-bottom", StaticVal::Num(1.0, "rem")),
    ]),
    ("my-auto", &[
        ("margin-top", StaticVal::Kw("auto")),
        ("margin-bottom", StaticVal::Kw("auto")),
    ]),
    ("my-px", &[
        ("margin-top", StaticVal::Num(1.0, "px")),
        ("margin-bottom", StaticVal::Num(1.0, "px")),
    ]),
    ("opacity-0", &[
        ("opacity", StaticVal::Num(0.0, "")),
    ]),
    ("opacity-100", &[
        ("opacity", StaticVal::Num(1.0, "")),
    ]),
    ("opacity-5", &[
        ("opacity", StaticVal::Num(0.05, "")),
    ]),
    ("opacity-50", &[
        ("opacity", StaticVal::Num(0.5, "")),
    ]),
    ("opacity-95", &[
        ("opacity", StaticVal::Num(0.95, "")),
    ]),
    ("p-0", &[
        ("padding", StaticVal::Num(0.0, "px")),
    ]),
    ("p-1", &[
        ("padding", StaticVal::Num(0.25, "rem")),
    ]),
    ("p-36", &[
        ("padding", StaticVal::Num(9.0, "rem")),
    ]),
    ("p-4", &[
        ("padding", StaticVal::Num(1.0, "rem")),
    ]),
    ("p-px", &[
        ("padding", StaticVal::Num(1.0, "px")),
    ]),
    ("pb-0", &[
        ("padding-bottom", StaticVal::Num(0.0, "px")),
    ]),
    ("pb-1", &[
        ("padding-bottom", StaticVal::Num(0.25, "rem")),
    ]),
    ("pb-36", &[
        ("padding-bottom", StaticVal::Num(9.0, "rem")),
    ]),
    ("pb-4", &[
        ("padding-bottom", StaticVal::Num(1.0, "rem")),
    ]),
    ("pb-px", &[
        ("padding-bottom", StaticVal::Num(1.0, "px")),
    ]),
    ("pl-0", &[
        ("padding-left", StaticVal::Num(0.0, "px")),
    ]),
    ("pl-1", &[
        ("padding-left", StaticVal::Num(0.25, "rem")),
    ]),
    ("pl-36", &[
        ("padding-left", StaticVal::Num(9.0, "rem")),
    ]),
    ("pl-4", &[
        ("padding-left", StaticVal::Num(1.0, "rem")),
    ]),
    ("pl-px", &[
        ("padding-left", StaticVal::Num(1.0, "px")),
    ]),
    ("pr-0", &[
        ("padding-right", StaticVal::Num(0.0, "px")),
    ]),
    ("pr-1", &[
        ("padding-right", StaticVal::Num(0.25, "rem")),
    ]),
    ("pr-36", &[
        ("padding-right", StaticVal::Num(9.0, "rem")),
    ]),
    ("pr-4", &[
        ("padding-right", StaticVal::Num(1.0, "rem")),
    ]),
    ("pr-px", &[
        ("padding-right", StaticVal::Num(1.0, "px")),
    ]),
    ("pt-0", &[
        ("padding-top", StaticVal::Num(0.0, "px")),
    ]),
    ("pt-1", &[
        ("padding-top", StaticVal::Num(0.25, "rem")),
    ]),
    ("pt-36", &[
        ("padding-top", StaticVal::Num(9.0, "rem")),
    ]),
    ("pt-4", &[
        ("padding-top", StaticVal::Num(1.0, "rem")),
    ]),
    ("pt-px", &[
        ("padding-top", StaticVal::Num(1.0, "px")),
    ]),
    ("px-0", &[
        ("padding-left", StaticVal::Num(0.0, "px")),
        ("padding-right", StaticVal::Num(0.0, "px")),
    ]),
    ("px-1", &[
        ("padding-left", StaticVal::Num(0.25, "rem")),
        ("padding-right", StaticVal::Num(0.25, "rem")),
    ]),
    ("px-36", &[
        ("padding-left", StaticVal::Num(9.0, "rem")),
        ("padding-right", StaticVal::Num(9.0, "rem")),
    ]),
    ("px-4", &[
        ("padding-left", StaticVal::Num(1.0, "rem")),
        ("padding-right", StaticVal::Num(1.0, "rem")),
    ]),
    ("px-px", &[
        ("padding-left", StaticVal::Num(1.0, "px")),
        ("padding-right", StaticVal::Num(1.0, "px")),
    ]),
    ("py-0", &[
        ("padding-top", StaticVal::Num(0.0, "px")),
        ("padding-bottom", StaticVal::Num(0.0, "px")),
    ]),
    ("py-1", &[
        ("padding-top", StaticVal::Num(0.25, "rem")),
        ("padding-bottom", StaticVal::Num(0.25, "rem")),
    ]),
    ("py-36", &[
        ("padding-top", StaticVal::Num(9.0, "rem")),
        ("padding-bottom", StaticVal::Num(9.0, "rem")),
    ]),
    ("py-4", &[
        ("padding-top", StaticVal::Num(1.0, "rem")),
        ("padding-bottom", StaticVal::Num(1.0, "rem")),
    ]),
    ("py-px", &[
        ("padding-top", StaticVal::Num(1.0, "px")),
        ("padding-bottom", StaticVal::Num(1.0, "px")),
    ]),
    ("right-0", &[
        ("right", StaticVal::Num(0.0, "px")),
    ]),
    ("right-1", &[
        ("right", StaticVal::Num(0.25, "rem")),
    ]),
    ("right-32", &[
        ("right", StaticVal::Num(8.0, "rem")),
    ]),
    ("right-4", &[
        ("right", StaticVal::Num(1.0, "rem")),
    ]),
    ("right-auto", &[
        ("right", StaticVal::Kw("auto")),
    ]),
    ("right-full", &[
        ("right", StaticVal::Num(100.0, "%")),
    ]),
    ("right-px", &[
        ("right", StaticVal::Num(1.0, "px")),
    ]),
    ("rotate-0", &[
        ("transform", StaticVal::Kw("rotate(0deg)")),
    ]),
    ("rotate-45", &[
        ("transform", StaticVal::Kw("rotate(45deg)")),
    ]),
    ("rotate-90", &[
        ("transform", StaticVal::Kw("rotate(90deg)")),
    ]),
    ("rounded-2xl", &[
        ("border-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-b-2xl", &[
        ("border-bottom-left-radius", StaticVal::Num(1.0, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-b-full", &[
        ("border-bottom-left-radius", StaticVal::Num(9999.0, "px")),
        ("border-bottom-right-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-b-lg", &[
        ("border-bottom-left-radius", StaticVal::Num(0.5, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-b-md", &[
        ("border-bottom-left-radius", StaticVal::Num(0.375, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-b-none", &[
        ("border-bottom-left-radius", StaticVal::Num(0.0, "px")),
        ("border-bottom-right-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-b-sm", &[
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-b-xl", &[
        ("border-bottom-left-radius", StaticVal::Num(0.75, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-b-xs", &[
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-bl-2xl", &[
        ("border-bottom-left-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-bl-full", &[
        ("border-bottom-left-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-bl-lg", &[
        ("border-bottom-left-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-bl-md", &[
        ("border-bottom-left-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-bl-none", &[
        ("border-bottom-left-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-bl-sm", &[
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-bl-xl", &[
        ("border-bottom-left-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-bl-xs", &[
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-br-2xl", &[
        ("border-bottom-right-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-br-full", &[
        ("border-bottom-right-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-br-lg", &[
        ("border-bottom-right-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-br-md", &[
        ("border-bottom-right-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-br-none", &[
        ("border-bottom-right-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-br-sm", &[
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-br-xl", &[
        ("border-bottom-right-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-br-xs", &[
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-e-2xl", &[
        ("border-start-end-radius", StaticVal::Num(1.0, "rem")),
        ("border-end-end-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-e-full", &[
        ("border-start-end-radius", StaticVal::Num(9999.0, "px")),
        ("border-end-end-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-e-lg", &[
        ("border-start-end-radius", StaticVal::Num(0.5, "rem")),
        ("border-end-end-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-e-md", &[
        ("border-start-end-radius", StaticVal::Num(0.375, "rem")),
        ("border-end-end-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-e-none", &[
        ("border-start-end-radius", StaticVal::Num(0.0, "px")),
        ("border-end-end-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-e-sm", &[
        ("border-start-end-radius", StaticVal::Num(0.125, "rem")),
        ("border-end-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-e-xl", &[
        ("border-start-end-radius", StaticVal::Num(0.75, "rem")),
        ("border-end-end-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-e-xs", &[
        ("border-start-end-radius", StaticVal::Num(0.125, "rem")),
        ("border-end-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-ee-2xl", &[
        ("border-end-end-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-ee-full", &[
        ("border-end-end-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-ee-lg", &[
        ("border-end-end-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-ee-md", &[
        ("border-end-end-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-ee-none", &[
        ("border-end-end-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-ee-sm", &[
        ("border-end-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-ee-xl", &[
        ("border-end-end-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-ee-xs", &[
        ("border-end-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-es-2xl", &[
        ("border-end-start-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-es-full", &[
        ("border-end-start-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-es-lg", &[
        ("border-end-start-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-es-md", &[
        ("border-end-start-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-es-none", &[
        ("border-end-start-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-es-sm", &[
        ("border-end-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-es-xl", &[
        ("border-end-start-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-es-xs", &[
        ("border-end-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-full", &[
        ("border-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-l-2xl", &[
        ("border-top-left-radius", StaticVal::Num(1.0, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-l-full", &[
        ("border-top-left-radius", StaticVal::Num(9999.0, "px")),
        ("border-bottom-left-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-l-lg", &[
        ("border-top-left-radius", StaticVal::Num(0.5, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-l-md", &[
        ("border-top-left-radius", StaticVal::Num(0.375, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-l-none", &[
        ("border-top-left-radius", StaticVal::Num(0.0, "px")),
        ("border-bottom-left-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-l-sm", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-l-xl", &[
        ("border-top-left-radius", StaticVal::Num(0.75, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-l-xs", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-lg", &[
        ("border-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-md", &[
        ("border-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-none", &[
        ("border-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-r-2xl", &[
        ("border-top-right-radius", StaticVal::Num(1.0, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-r-full", &[
        ("border-top-right-radius", StaticVal::Num(9999.0, "px")),
        ("border-bottom-right-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-r-lg", &[
        ("border-top-right-radius", StaticVal::Num(0.5, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-r-md", &[
        ("border-top-right-radius", StaticVal::Num(0.375, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-r-none", &[
        ("border-top-right-radius", StaticVal::Num(0.0, "px")),
        ("border-bottom-right-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-r-sm", &[
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-r-xl", &[
        ("border-top-right-radius", StaticVal::Num(0.75, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-r-xs", &[
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
        ("border-bottom-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-s-2xl", &[
        ("border-start-start-radius", StaticVal::Num(1.0, "rem")),
        ("border-end-start-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-s-full", &[
        ("border-start-start-radius", StaticVal::Num(9999.0, "px")),
        ("border-end-start-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-s-lg", &[
        ("border-start-start-radius", StaticVal::Num(0.5, "rem")),
        ("border-end-start-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-s-md", &[
        ("border-start-start-radius", StaticVal::Num(0.375, "rem")),
        ("border-end-start-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-s-none", &[
        ("border-start-start-radius", StaticVal::Num(0.0, "px")),
        ("border-end-start-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-s-sm", &[
        ("border-start-start-radius", StaticVal::Num(0.125, "rem")),
        ("border-end-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-s-xl", &[
        ("border-start-start-radius", StaticVal::Num(0.75, "rem")),
        ("border-end-start-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-s-xs", &[
        ("border-start-start-radius", StaticVal::Num(0.125, "rem")),
        ("border-end-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-se-2xl", &[
        ("border-start-end-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-se-full", &[
        ("border-start-end-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-se-lg", &[
        ("border-start-end-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-se-md", &[
        ("border-start-end-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-se-none", &[
        ("border-start-end-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-se-sm", &[
        ("border-start-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-se-xl", &[
        ("border-start-end-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-se-xs", &[
        ("border-start-end-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-sm", &[
        ("border-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-ss-2xl", &[
        ("border-start-start-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-ss-full", &[
        ("border-start-start-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-ss-lg", &[
        ("border-start-start-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-ss-md", &[
        ("border-start-start-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-ss-none", &[
        ("border-start-start-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-ss-sm", &[
        ("border-start-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-ss-xl", &[
        ("border-start-start-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-ss-xs", &[
        ("border-start-start-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-t-2xl", &[
        ("border-top-left-radius", StaticVal::Num(1.0, "rem")),
        ("border-top-right-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-t-full", &[
        ("border-top-left-radius", StaticVal::Num(9999.0, "px")),
        ("border-top-right-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-t-lg", &[
        ("border-top-left-radius", StaticVal::Num(0.5, "rem")),
        ("border-top-right-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-t-md", &[
        ("border-top-left-radius", StaticVal::Num(0.375, "rem")),
        ("border-top-right-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-t-none", &[
        ("border-top-left-radius", StaticVal::Num(0.0, "px")),
        ("border-top-right-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-t-sm", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-t-xl", &[
        ("border-top-left-radius", StaticVal::Num(0.75, "rem")),
        ("border-top-right-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-t-xs", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tl-2xl", &[
        ("border-top-left-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-tl-full", &[
        ("border-top-left-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-tl-lg", &[
        ("border-top-left-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-tl-md", &[
        ("border-top-left-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-tl-none", &[
        ("border-top-left-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-tl-sm", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tl-xl", &[
        ("border-top-left-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-tl-xs", &[
        ("border-top-left-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tr-2xl", &[
        ("border-top-right-radius", StaticVal::Num(1.0, "rem")),
    ]),
    ("rounded-tr-full", &[
        ("border-top-right-radius", StaticVal::Num(9999.0, "px")),
    ]),
    ("rounded-tr-lg", &[
        ("border-top-right-radius", StaticVal::Num(0.5, "rem")),
    ]),
    ("rounded-tr-md", &[
        ("border-top-right-radius", StaticVal::Num(0.375, "rem")),
    ]),
    ("rounded-tr-none", &[
        ("border-top-right-radius", StaticVal::Num(0.0, "px")),
    ]),
    ("rounded-tr-sm", &[
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-tr-xl", &[
        ("border-top-right-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-tr-xs", &[
        ("border-top-right-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("rounded-xl", &[
        ("border-radius", StaticVal::Num(0.75, "rem")),
    ]),
    ("rounded-xs", &[
        ("border-radius", StaticVal::Num(0.125, "rem")),
    ]),
    ("scale-0", &[
        ("scale", StaticVal::Num(0.0, "")),
    ]),
    ("scale-100", &[
        ("scale", StaticVal::Num(1.0, "")),
    ]),
    ("scale-150", &[
        ("scale", StaticVal::Num(1.5, "")),
    ]),
    ("scale-50", &[
        ("scale", StaticVal::Num(0.5, "")),
    ]),
    ("scale-95", &[
        ("scale", StaticVal::Num(0.95, "")),
    ]),
    ("shadow-2xl", &[
        ("box-shadow", StaticVal::Literal("0 25px 50px -12px rgba(0, 0, 0, 0.25)")),
    ]),
    ("shadow-lg", &[
        ("box-shadow", StaticVal::Literal("0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-md", &[
        ("box-shadow", StaticVal::Literal("0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-none", &[
        ("box-shadow", StaticVal::Kw("none")),
    ]),
    ("shadow-sm", &[
        ("box-shadow", StaticVal::Literal("0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-xl", &[
        ("box-shadow", StaticVal::Literal("0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)")),
    ]),
    ("shadow-xs", &[
        ("--tw-shadow", StaticVal::Literal("0 1px 2px 0 rgba(0, 0, 0, 0.05)")),
        ("box-shadow", StaticVal::Literal("var(--tw-ring-offset-shadow, 0 0 #0000), var(--tw-ring-shadow, 0 0 #0000), var(--tw-shadow, 0 1px 2px 0 rgba(0, 0, 0, 0.05))")),
    ]),
    ("text-2xl", &[
        ("font-size", StaticVal::Num(1.5, "rem")),
        ("line-height", StaticVal::Num(2.0, "rem")),
    ]),
    ("text-8xl", &[
        ("font-size", StaticVal::Num(6.0, "rem")),
        ("line-height", StaticVal::Num(1.0, "")),
    ]),
    ("text-lg", &[
        ("font-size", StaticVal::Num(1.125, "rem")),
        ("line-height", StaticVal::Num(1.75, "rem")),
    ]),
    ("text-sm", &[
        ("font-size", StaticVal::Num(0.875, "rem")),
        ("line-height", StaticVal::Num(1.25, "rem")),
    ]),
    ("text-xl", &[
        ("font-size", StaticVal::Num(1.25, "rem")),
        ("line-height", StaticVal::Num(1.75, "rem")),
    ]),
    ("text-xs", &[
        ("font-size", StaticVal::Num(0.75, "rem")),
        ("line-height", StaticVal::Num(1.0, "rem")),
    ]),
    ("top-0", &[
        ("top", StaticVal::Num(0.0, "px")),
    ]),
    ("top-1", &[
        ("top", StaticVal::Num(0.25, "rem")),
    ]),
    ("top-32", &[
        ("top", StaticVal::Num(8.0, "rem")),
    ]),
    ("top-4", &[
        ("top", StaticVal::Num(1.0, "rem")),
    ]),
    ("top-auto", &[
        ("top", StaticVal::Kw("auto")),
    ]),
    ("top-full", &[
        ("top", StaticVal::Num(100.0, "%")),
    ]),
    ("top-px", &[
        ("top", StaticVal::Num(1.0, "px")),
    ]),
    ("tracking-normal", &[
        ("letter-spacing", StaticVal::Num(0.0, "em")),
    ]),
    ("tracking-wide", &[
        ("letter-spacing", StaticVal::Num(0.025, "em")),
    ]),
    ("tracking-widest", &[
        ("letter-spacing", StaticVal::Num(0.1, "em")),
    ]),
    ("translate-x-0", &[
        ("transform", StaticVal::Kw("translateX(0px)")),
    ]),
    ("translate-x-1/2", &[
        ("transform", StaticVal::Kw("translateX(50%)")),
    ]),
    ("translate-x-full", &[
        ("transform", StaticVal::Kw("translateX(100%)")),
    ]),
    ("translate-y-0", &[
        ("transform", StaticVal::Kw("translateY(0px)")),
    ]),
    ("translate-y-1/2", &[
        ("transform", StaticVal::Kw("translateY(50%)")),
    ]),
    ("translate-y-full", &[
        ("transform", StaticVal::Kw("translateY(100%)")),
    ]),
    ("w-0", &[
        ("width", StaticVal::Num(0.0, "px")),
    ]),
    ("w-1", &[
        ("width", StaticVal::Num(0.25, "rem")),
    ]),
    ("w-4", &[
        ("width", StaticVal::Num(1.0, "rem")),
    ]),
    ("w-44", &[
        ("width", StaticVal::Num(11.0, "rem")),
    ]),
    ("w-auto", &[
        ("width", StaticVal::Kw("auto")),
    ]),
    ("w-full", &[
        ("width", StaticVal::Num(100.0, "%")),
    ]),
    ("w-lg", &[
        ("width", StaticVal::Num(32.0, "rem")),
    ]),
    ("w-md", &[
        ("width", StaticVal::Num(28.0, "rem")),
    ]),
    ("w-px", &[
        ("width", StaticVal::Num(1.0, "px")),
    ]),
    ("w-sm", &[
        ("width", StaticVal::Num(24.0, "rem")),
    ]),
    ("w-xl", &[
        ("width", StaticVal::Num(36.0, "rem")),
    ]),
    ("w-xs", &[
        ("width", StaticVal::Num(20.0, "rem")),
    ]),
    ("z-0", &[
        ("z-index", StaticVal::Num(0.0, "")),
    ]),
    ("z-30", &[
        ("z-index", StaticVal::Num(30.0, "")),
    ]),
    ("z-50", &[
        ("z-index", StaticVal::Num(50.0, "")),
    ]),
    ("z-auto", &[
        ("z-index", StaticVal::Kw("auto")),
    ]),
];

