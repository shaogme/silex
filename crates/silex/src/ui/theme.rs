use silex_css::prelude::*;
use silex_macros::theme;

theme! {
    #[theme(main, prefix = "slx-ui")]
    pub struct ShadcnTheme {
        pub background: Hex,
        pub foreground: Hex,
        pub primary: Hex,
        pub primary_foreground: Hex,
        pub secondary: Hex,
        pub secondary_foreground: Hex,
        pub muted: Hex,
        pub muted_foreground: Hex,
        pub accent: Hex,
        pub accent_foreground: Hex,
        pub destructive: Hex,
        pub destructive_foreground: Hex,
        pub border: Hex,
        pub input: Hex,
        pub ring: Hex,
        pub radius: Px,
    }
}

pub fn shadcn_light_theme() -> ShadcnTheme {
    ShadcnTheme {
        background: hex("#ffffff"),
        foreground: hex("#020817"),
        primary: hex("#0f172a"),
        primary_foreground: hex("#f8fafc"),
        secondary: hex("#f1f5f9"),
        secondary_foreground: hex("#0f172a"),
        muted: hex("#f1f5f9"),
        muted_foreground: hex("#64748b"),
        accent: hex("#f1f5f9"),
        accent_foreground: hex("#0f172a"),
        destructive: hex("#ef4444"),
        destructive_foreground: hex("#f8fafc"),
        border: hex("#e2e8f0"),
        input: hex("#e2e8f0"),
        ring: hex("#94a3b8"),
        radius: px(8),
    }
}

pub fn shadcn_dark_theme() -> ShadcnTheme {
    ShadcnTheme {
        background: hex("#020817"),
        foreground: hex("#f8fafc"),
        primary: hex("#f8fafc"),
        primary_foreground: hex("#0f172a"),
        secondary: hex("#1e293b"),
        secondary_foreground: hex("#f8fafc"),
        muted: hex("#1e293b"),
        muted_foreground: hex("#94a3b8"),
        accent: hex("#1e293b"),
        accent_foreground: hex("#f8fafc"),
        destructive: hex("#7f1d1d"),
        destructive_foreground: hex("#f8fafc"),
        border: hex("#1e293b"),
        input: hex("#1e293b"),
        ring: hex("#cbd5e1"),
        radius: px(8),
    }
}

/// Injects standard Tailwind CSS Base Preflight Reset (`box-sizing: border-box`, etc.)
/// into the document style registry.
pub fn inject_shadcn_base_styles() {
    let base_css = r#"
*, ::before, ::after {
  box-sizing: border-box;
  border-width: 0;
  border-style: solid;
  border-color: #e2e8f0;
}
"#;
    silex_css::inject_style("slx-shadcn-base-reset", base_css);
}

