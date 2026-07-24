use std::borrow::Cow;

/// 颜色规则表解析
pub fn resolve_color_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let color_prefixes = &[
        ("scrollbar-thumb-", "scrollbar-color"),
        ("scrollbar-track-", "scrollbar-color"),
        ("inset-shadow-", "--tw-inset-shadow-color"),
        ("drop-shadow-", "--tw-drop-shadow-color"),
        ("text-shadow-", "--tw-text-shadow-color"),
        ("ring-offset-", "--tw-ring-offset-color"),
        ("placeholder-", "color"),
        ("decoration-", "text-decoration-color"),
        ("inset-ring-", "--tw-inset-ring-color"),
        ("border-bs-", "border-block-start-color"),
        ("border-be-", "border-block-end-color"),
        ("border-b-", "border-bottom-color"),
        ("border-t-", "border-top-color"),
        ("border-l-", "border-left-color"),
        ("border-r-", "border-right-color"),
        ("border-s-", "border-inline-start-color"),
        ("border-e-", "border-inline-end-color"),
        ("border-x-", "border-inline-color"),
        ("border-y-", "border-block-color"),
        ("outline-", "outline-color"),
        ("border-", "border-color"),
        ("accent-", "accent-color"),
        ("divide-", "border-color"),
        ("stroke-", "stroke"),
        ("shadow-", "--tw-shadow-color"),
        ("caret-", "caret-color"),
        ("text-", "color"),
        ("fill-", "fill"),
        ("from-", "--tw-gradient-from"),
        ("ring-", "outline-color"),
        ("via-", "--tw-gradient-via"),
        ("bg-", "background-color"),
        ("to-", "--tw-gradient-to"),
    ];

    for &(prefix, prop) in color_prefixes {
        if let Some(color_name) = class_name.strip_prefix(prefix) {
            if let Some(hex) = resolve_color_hex(color_name) {
                return Some(cow!(vec[(prop, hex)]));
            }
        }
    }

    if let Some(rest) = class_name.strip_prefix("mask-") {
        if let Some(idx) = rest.rfind("-from-") {
            let val_name = &rest[idx + 6..];
            if let Some(hex) = resolve_color_hex(val_name) {
                return Some(cow!(vec![("--tw-mask-from", hex)]));
            }
            if let Some(len) = super::dynamic::resolve_length_val(val_name) {
                return Some(cow!(vec![("--tw-mask-from-position", len)]));
            }
        }
        if let Some(idx) = rest.rfind("-to-") {
            let val_name = &rest[idx + 4..];
            if let Some(hex) = resolve_color_hex(val_name) {
                return Some(cow!(vec![("--tw-mask-to", hex)]));
            }
            if let Some(len) = super::dynamic::resolve_length_val(val_name) {
                return Some(cow!(vec![("--tw-mask-to-position", len)]));
            }
        }
    }

    None
}

/// Tailwind CSS 标准颜色面板 (50..950 + 基础色彩)
pub fn resolve_color_hex(color_name: &str) -> Option<&'static str> {
    let hex = match color_name {
        "transparent" => "transparent",
        "current" => "currentColor",
        "black" => "#000000",
        "white" => "#ffffff",
        "inherit" => "inherit",

        // Slate
        "slate-50" => "#f8fafc",
        "slate-100" => "#f1f5f9",
        "slate-200" => "#e2e8f0",
        "slate-300" => "#cbd5e1",
        "slate-400" => "#94a3b8",
        "slate-500" => "#64748b",
        "slate-600" => "#475569",
        "slate-700" => "#334155",
        "slate-800" => "#1e293b",
        "slate-900" => "#0f172a",
        "slate-950" => "#020617",

        // Gray
        "gray-50" => "#f9fafb",
        "gray-100" => "#f3f4f6",
        "gray-200" => "#e5e7eb",
        "gray-300" => "#d1d5db",
        "gray-400" => "#9ca3af",
        "gray-500" => "#6b7280",
        "gray-600" => "#4b5563",
        "gray-700" => "#374151",
        "gray-800" => "#1f2937",
        "gray-900" => "#111827",
        "gray-950" => "#030712",

        // Zinc
        "zinc-50" => "#fafafa",
        "zinc-100" => "#f4f4f5",
        "zinc-200" => "#e4e4e7",
        "zinc-300" => "#d4d4d8",
        "zinc-400" => "#a1a1aa",
        "zinc-500" => "#71717a",
        "zinc-600" => "#52525b",
        "zinc-700" => "#3f3f46",
        "zinc-800" => "#27272a",
        "zinc-900" => "#18181b",
        "zinc-950" => "#09090b",

        // Neutral
        "neutral-50" => "#fafafa",
        "neutral-100" => "#f5f5f5",
        "neutral-200" => "#e5e5e5",
        "neutral-300" => "#d4d4d4",
        "neutral-400" => "#a3a3a3",
        "neutral-500" => "#737373",
        "neutral-600" => "#525252",
        "neutral-700" => "#404040",
        "neutral-800" => "#262626",
        "neutral-900" => "#171717",
        "neutral-950" => "#0a0a0a",

        // Stone
        "stone-50" => "#fafaf9",
        "stone-100" => "#f5f5f4",
        "stone-200" => "#e7e5e4",
        "stone-300" => "#d6d3d1",
        "stone-400" => "#a8a29e",
        "stone-500" => "#78716c",
        "stone-600" => "#57534e",
        "stone-700" => "#44403c",
        "stone-800" => "#292524",
        "stone-900" => "#1c1917",
        "stone-950" => "#0c0a09",

        // Red
        "red-50" => "#fef2f2",
        "red-100" => "#fee2e2",
        "red-200" => "#fecaca",
        "red-300" => "#fca5a5",
        "red-400" => "#f87171",
        "red-500" => "#ef4444",
        "red-600" => "#dc2626",
        "red-700" => "#b91c1c",
        "red-800" => "#991b1b",
        "red-900" => "#7f1d1d",
        "red-950" => "#450a0a",

        // Orange
        "orange-50" => "#fff7ed",
        "orange-100" => "#ffedd5",
        "orange-200" => "#fed7aa",
        "orange-300" => "#fdba74",
        "orange-400" => "#fb923c",
        "orange-500" => "#f97316",
        "orange-600" => "#ea580c",
        "orange-700" => "#c2410c",
        "orange-800" => "#9a3412",
        "orange-900" => "#7c2d12",
        "orange-950" => "#431407",

        // Amber
        "amber-50" => "#fffbeb",
        "amber-100" => "#fef3c7",
        "amber-200" => "#fde68a",
        "amber-300" => "#fcd34d",
        "amber-400" => "#fbbf24",
        "amber-500" => "#f59e0b",
        "amber-600" => "#d97706",
        "amber-700" => "#b45309",
        "amber-800" => "#92400e",
        "amber-900" => "#78350f",
        "amber-950" => "#451a03",

        // Yellow
        "yellow-50" => "#fefce8",
        "yellow-100" => "#fef9c3",
        "yellow-200" => "#fef08a",
        "yellow-300" => "#fde047",
        "yellow-400" => "#facc15",
        "yellow-500" => "#eab308",
        "yellow-600" => "#ca8a04",
        "yellow-700" => "#a16207",
        "yellow-800" => "#854d0e",
        "yellow-900" => "#713f12",
        "yellow-950" => "#422006",

        // Lime
        "lime-50" => "#f7fee7",
        "lime-100" => "#ecfccb",
        "lime-200" => "#d9f99d",
        "lime-300" => "#bef264",
        "lime-400" => "#a3e635",
        "lime-500" => "#84cc16",
        "lime-600" => "#65a30d",
        "lime-700" => "#4d7c0f",
        "lime-800" => "#3f6212",
        "lime-900" => "#365314",
        "lime-950" => "#1a2e05",

        // Green
        "green-50" => "#f0fdf4",
        "green-100" => "#dcfce7",
        "green-200" => "#bbf7d0",
        "green-300" => "#86efac",
        "green-400" => "#4ade80",
        "green-500" => "#22c55e",
        "green-600" => "#16a34a",
        "green-700" => "#15803d",
        "green-800" => "#166534",
        "green-900" => "#14532d",
        "green-950" => "#052e16",

        // Emerald
        "emerald-50" => "#ecfdf5",
        "emerald-100" => "#d1fae5",
        "emerald-200" => "#a7f3d0",
        "emerald-300" => "#6ee7b7",
        "emerald-400" => "#34d399",
        "emerald-500" => "#10b981",
        "emerald-600" => "#059669",
        "emerald-700" => "#047857",
        "emerald-800" => "#065f46",
        "emerald-900" => "#064e3b",
        "emerald-950" => "#022c22",

        // Teal
        "teal-50" => "#f0fdfa",
        "teal-100" => "#ccfbf1",
        "teal-200" => "#99f6e4",
        "teal-300" => "#5eead4",
        "teal-400" => "#2dd4bf",
        "teal-500" => "#14b8a6",
        "teal-600" => "#0d9488",
        "teal-700" => "#0f766e",
        "teal-800" => "#115e59",
        "teal-900" => "#134e4a",
        "teal-950" => "#042f2e",

        // Cyan
        "cyan-50" => "#ecfeff",
        "cyan-100" => "#cffafe",
        "cyan-200" => "#a5f3fc",
        "cyan-300" => "#67e8f9",
        "cyan-400" => "#22d3ee",
        "cyan-500" => "#06b6d4",
        "cyan-600" => "#0891b2",
        "cyan-700" => "#0e7490",
        "cyan-800" => "#155e75",
        "cyan-900" => "#164e63",
        "cyan-950" => "#083344",

        // Sky
        "sky-50" => "#f0f9ff",
        "sky-100" => "#e0f2fe",
        "sky-200" => "#bae6fd",
        "sky-300" => "#7dd3fc",
        "sky-400" => "#38bdf8",
        "sky-500" => "#0ea5e9",
        "sky-600" => "#0284c7",
        "sky-700" => "#0369a1",
        "sky-800" => "#075985",
        "sky-900" => "#0c4a6e",
        "sky-950" => "#082f49",

        // Blue
        "blue-50" => "#eff6ff",
        "blue-100" => "#dbeafe",
        "blue-200" => "#bfdbfe",
        "blue-300" => "#93c5fd",
        "blue-400" => "#60a5fa",
        "blue-500" => "#3b82f6",
        "blue-600" => "#2563eb",
        "blue-700" => "#1d4ed8",
        "blue-800" => "#1e40af",
        "blue-900" => "#1e3a8a",
        "blue-950" => "#172554",

        // Indigo
        "indigo-50" => "#eef2ff",
        "indigo-100" => "#e0e7ff",
        "indigo-200" => "#c7d2fe",
        "indigo-300" => "#a5b4fc",
        "indigo-400" => "#818cf8",
        "indigo-500" => "#6366f1",
        "indigo-600" => "#4f46e5",
        "indigo-700" => "#4338ca",
        "indigo-800" => "#3730a3",
        "indigo-900" => "#312e81",
        "indigo-950" => "#1e1b4b",

        // Violet
        "violet-50" => "#f5f3ff",
        "violet-100" => "#ede9fe",
        "violet-200" => "#ddd6fe",
        "violet-300" => "#c4b5fd",
        "violet-400" => "#a78bfa",
        "violet-500" => "#8b5cf6",
        "violet-600" => "#7c3aed",
        "violet-700" => "#6d28d9",
        "violet-800" => "#5b21b6",
        "violet-900" => "#4c1d95",
        "violet-950" => "#2e1065",

        // Purple
        "purple-50" => "#faf5ff",
        "purple-100" => "#f3e8ff",
        "purple-200" => "#e9d5ff",
        "purple-300" => "#d8b4fe",
        "purple-400" => "#c084fc",
        "purple-500" => "#a855f7",
        "purple-600" => "#9333ea",
        "purple-700" => "#7e22ce",
        "purple-800" => "#6b21a8",
        "purple-900" => "#581c87",
        "purple-950" => "#3b0764",

        // Fuchsia
        "fuchsia-50" => "#fdf4ff",
        "fuchsia-100" => "#fae8ff",
        "fuchsia-200" => "#f5d0fe",
        "fuchsia-300" => "#f0abfc",
        "fuchsia-400" => "#e879f9",
        "fuchsia-500" => "#d946ef",
        "fuchsia-600" => "#c026d3",
        "fuchsia-700" => "#a21caf",
        "fuchsia-800" => "#86198f",
        "fuchsia-900" => "#701a75",
        "fuchsia-950" => "#4a044e",

        // Pink
        "pink-50" => "#fdf2f8",
        "pink-100" => "#fce7f3",
        "pink-200" => "#fbcfe8",
        "pink-300" => "#f9a8d4",
        "pink-400" => "#f472b6",
        "pink-500" => "#ec4899",
        "pink-600" => "#db2777",
        "pink-700" => "#be185d",
        "pink-800" => "#9d174d",
        "pink-900" => "#831843",
        "pink-950" => "#500724",

        // Rose
        "rose-50" => "#fff1f2",
        "rose-100" => "#ffe4e6",
        "rose-200" => "#fecdd3",
        "rose-300" => "#fda4af",
        "rose-400" => "#fb7185",
        "rose-500" => "#f43f5e",
        "rose-600" => "#e11d48",
        "rose-700" => "#be123c",
        "rose-800" => "#9f1239",
        "rose-900" => "#881337",
        "rose-950" => "#4c0519",

        // Mauve
        "mauve-50" => "var(--color-mauve-50)",
        "mauve-100" => "var(--color-mauve-100)",
        "mauve-200" => "var(--color-mauve-200)",
        "mauve-300" => "var(--color-mauve-300)",
        "mauve-400" => "var(--color-mauve-400)",
        "mauve-500" => "var(--color-mauve-500)",
        "mauve-600" => "var(--color-mauve-600)",
        "mauve-700" => "var(--color-mauve-700)",
        "mauve-800" => "var(--color-mauve-800)",
        "mauve-900" => "var(--color-mauve-900)",
        "mauve-950" => "var(--color-mauve-950)",

        // Mist
        "mist-50" => "var(--color-mist-50)",
        "mist-100" => "var(--color-mist-100)",
        "mist-200" => "var(--color-mist-200)",
        "mist-300" => "var(--color-mist-300)",
        "mist-400" => "var(--color-mist-400)",
        "mist-500" => "var(--color-mist-500)",
        "mist-600" => "var(--color-mist-600)",
        "mist-700" => "var(--color-mist-700)",
        "mist-800" => "var(--color-mist-800)",
        "mist-900" => "var(--color-mist-900)",
        "mist-950" => "var(--color-mist-950)",

        // Olive
        "olive-50" => "var(--color-olive-50)",
        "olive-100" => "var(--color-olive-100)",
        "olive-200" => "var(--color-olive-200)",
        "olive-300" => "var(--color-olive-300)",
        "olive-400" => "var(--color-olive-400)",
        "olive-500" => "var(--color-olive-500)",
        "olive-600" => "var(--color-olive-600)",
        "olive-700" => "var(--color-olive-700)",
        "olive-800" => "var(--color-olive-800)",
        "olive-900" => "var(--color-olive-900)",
        "olive-950" => "var(--color-olive-950)",

        // Taupe
        "taupe-50" => "var(--color-taupe-50)",
        "taupe-100" => "var(--color-taupe-100)",
        "taupe-200" => "var(--color-taupe-200)",
        "taupe-300" => "var(--color-taupe-300)",
        "taupe-400" => "var(--color-taupe-400)",
        "taupe-500" => "var(--color-taupe-500)",
        "taupe-600" => "var(--color-taupe-600)",
        "taupe-700" => "var(--color-taupe-700)",
        "taupe-800" => "var(--color-taupe-800)",
        "taupe-900" => "var(--color-taupe-900)",
        "taupe-950" => "var(--color-taupe-950)",

        _ => return None,
    };
    Some(hex)
}
