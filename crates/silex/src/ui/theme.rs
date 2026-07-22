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
:root {
  --radius: 0.5rem;
  --background: oklch(0.985 0 0);
  --foreground: oklch(0.145 0 0);
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.145 0 0);
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --secondary: oklch(0.94 0 0);
  --secondary-foreground: oklch(0.205 0 0);
  --muted: oklch(0.95 0 0);
  --muted-foreground: oklch(0.556 0 0);
  --accent: oklch(0.94 0 0);
  --accent-foreground: oklch(0.205 0 0);
  --destructive: oklch(0.577 0.245 27.325);
  --destructive-foreground: oklch(0.985 0 0);
  --border: oklch(0.88 0 0);
  --input: oklch(0.88 0 0);
  --ring: oklch(0.708 0 0);
}

.dark {
  --background: oklch(0.13 0.02 260);
  --foreground: oklch(0.985 0 0);
  --card: oklch(0.18 0.02 260);
  --card-foreground: oklch(0.985 0 0);
  --popover: oklch(0.18 0.02 260);
  --popover-foreground: oklch(0.985 0 0);
  --primary: oklch(0.985 0 0);
  --primary-foreground: oklch(0.145 0 0);
  --secondary: oklch(0.25 0.02 260);
  --secondary-foreground: oklch(0.985 0 0);
  --muted: oklch(0.25 0.02 260);
  --muted-foreground: oklch(0.708 0 0);
  --accent: oklch(0.28 0.02 260);
  --accent-foreground: oklch(0.985 0 0);
  --destructive: oklch(0.65 0.22 25);
  --destructive-foreground: oklch(0.985 0 0);
  --border: oklch(0.28 0.02 260);
  --input: oklch(0.28 0.02 260);
  --ring: oklch(0.556 0 0);
}

*, ::before, ::after {
  box-sizing: border-box;
  border-width: 0;
  border-style: solid;
  border-color: var(--border);
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", "Noto Sans", Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji";
}

button, input, optgroup, select, textarea {
  font-family: inherit;
  font-size: 100%;
  font-weight: inherit;
  line-height: inherit;
  color: inherit;
}

button, [type='button'], [type='reset'], [type='submit'] {
  padding: 0;
  margin: 0;
  background-color: transparent;
  background-image: none;
}
"#;
    silex_css::inject_style("slx-shadcn-base-reset", base_css);
}
