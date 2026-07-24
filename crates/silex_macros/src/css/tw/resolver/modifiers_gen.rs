// 自动生成的 Tailwind 修饰符与断点规则表（供 silex_macros 使用）
// 由 silex_codegen 自动生成，切勿手写修改！

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    PseudoClass,
    PseudoElement,
    MediaBreakpoint,
    Child,
    Descendant,
    Dark,
}

pub struct ModifierMeta {
    pub key: &'static str,
    pub kind: ModifierKind,
    pub priority: u32,
    pub css_selector: &'static str,
}

#[rustfmt::skip]
pub static MODIFIER_TABLE: &[ModifierMeta] = &[
    ModifierMeta { key: "*", kind: ModifierKind::Child, priority: 10, css_selector: "& > *" },
    ModifierMeta { key: "**", kind: ModifierKind::Descendant, priority: 10, css_selector: "& *" },
    ModifierMeta { key: "2xl", kind: ModifierKind::MediaBreakpoint, priority: 2536, css_selector: "(min-width: 1536px)" },
    ModifierMeta { key: "active", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:active" },
    ModifierMeta { key: "after", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::after" },
    ModifierMeta { key: "autofill", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:autofill" },
    ModifierMeta { key: "backdrop", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::backdrop" },
    ModifierMeta { key: "before", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::before" },
    ModifierMeta { key: "checked", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:checked" },
    ModifierMeta { key: "dark", kind: ModifierKind::Dark, priority: 60, css_selector: ".dark &, &.dark" },
    ModifierMeta { key: "default", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:default" },
    ModifierMeta { key: "details-content", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::details-content" },
    ModifierMeta { key: "disabled", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:disabled" },
    ModifierMeta { key: "empty", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:empty" },
    ModifierMeta { key: "enabled", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:enabled" },
    ModifierMeta { key: "even", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:nth-child(even)" },
    ModifierMeta { key: "file", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::file-selector-button" },
    ModifierMeta { key: "first", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:first-child" },
    ModifierMeta { key: "first-letter", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::first-letter" },
    ModifierMeta { key: "first-line", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::first-line" },
    ModifierMeta { key: "first-of-type", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:first-of-type" },
    ModifierMeta { key: "focus", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:focus" },
    ModifierMeta { key: "focus-visible", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:focus-visible" },
    ModifierMeta { key: "focus-within", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:focus-within" },
    ModifierMeta { key: "hover", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:hover" },
    ModifierMeta { key: "in-range", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:in-range" },
    ModifierMeta { key: "indeterminate", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:indeterminate" },
    ModifierMeta { key: "invalid", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:invalid" },
    ModifierMeta { key: "last", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:last-child" },
    ModifierMeta { key: "last-of-type", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:last-of-type" },
    ModifierMeta { key: "lg", kind: ModifierKind::MediaBreakpoint, priority: 2024, css_selector: "(min-width: 1024px)" },
    ModifierMeta { key: "marker", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::marker" },
    ModifierMeta { key: "md", kind: ModifierKind::MediaBreakpoint, priority: 1768, css_selector: "(min-width: 768px)" },
    ModifierMeta { key: "odd", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:nth-child(odd)" },
    ModifierMeta { key: "only", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:only-child" },
    ModifierMeta { key: "only-of-type", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:only-of-type" },
    ModifierMeta { key: "optional", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:optional" },
    ModifierMeta { key: "out-of-range", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:out-of-range" },
    ModifierMeta { key: "placeholder", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::placeholder" },
    ModifierMeta { key: "placeholder-shown", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:placeholder-shown" },
    ModifierMeta { key: "read-only", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:read-only" },
    ModifierMeta { key: "required", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:required" },
    ModifierMeta { key: "selection", kind: ModifierKind::PseudoElement, priority: 20, css_selector: "&::selection" },
    ModifierMeta { key: "sm", kind: ModifierKind::MediaBreakpoint, priority: 1640, css_selector: "(min-width: 640px)" },
    ModifierMeta { key: "target", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:target" },
    ModifierMeta { key: "user-invalid", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:user-invalid" },
    ModifierMeta { key: "user-valid", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:user-valid" },
    ModifierMeta { key: "valid", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:valid" },
    ModifierMeta { key: "visited", kind: ModifierKind::PseudoClass, priority: 20, css_selector: "&:visited" },
    ModifierMeta { key: "xl", kind: ModifierKind::MediaBreakpoint, priority: 2280, css_selector: "(min-width: 1280px)" },
];

/// 根据修饰符 key 二分查找对应的元数据配置
pub fn lookup_modifier_meta(key: &str) -> Option<&'static ModifierMeta> {
    let idx = MODIFIER_TABLE.binary_search_by_key(&key, |m| m.key).ok()?;
    Some(&MODIFIER_TABLE[idx])
}
