const CANDIDATE_UTILITIES: &[&str] = &[
    "box-border",
    "box-content",
    "block",
    "inline-block",
    "inline",
    "flex",
    "inline-flex",
    "grid",
    "inline-grid",
    "hidden",
    "group",
    "peer",
    "isolate",
    "isolation-auto",
    "static",
    "fixed",
    "absolute",
    "relative",
    "sticky",
    "flex-row",
    "flex-row-reverse",
    "flex-col",
    "flex-col-reverse",
    "flex-wrap",
    "flex-nowrap",
    "flex-1",
    "flex-auto",
    "flex-initial",
    "flex-none",
    "grow",
    "grow-0",
    "shrink",
    "shrink-0",
    "items-start",
    "items-center",
    "items-end",
    "items-stretch",
    "items-baseline",
    "justify-start",
    "justify-center",
    "justify-end",
    "justify-between",
    "justify-around",
    "justify-evenly",
    "justify-stretch",
    "self-auto",
    "self-start",
    "self-end",
    "self-center",
    "self-stretch",
    "place-items-center",
    "place-content-center",
    "w-full",
    "h-full",
    "w-screen",
    "h-screen",
    "w-auto",
    "h-auto",
    "w-min",
    "w-max",
    "w-fit",
    "h-min",
    "h-max",
    "h-fit",
    "w-1/2",
    "w-1/3",
    "w-2/3",
    "w-1/4",
    "w-3/4",
    "h-1/2",
    "min-w-0",
    "min-w-full",
    "min-h-0",
    "min-h-full",
    "min-h-screen",
    "max-w-xs",
    "max-w-sm",
    "max-w-md",
    "max-w-lg",
    "max-w-xl",
    "max-w-2xl",
    "max-w-3xl",
    "max-w-4xl",
    "max-w-5xl",
    "max-w-6xl",
    "max-w-7xl",
    "max-w-full",
    "max-w-none",
    "inset-0",
    "top-0",
    "bottom-0",
    "left-0",
    "right-0",
    "bg-transparent",
    "text-transparent",
    "border-transparent",
    "bg-current",
    "text-current",
    "border-current",
    "bg-white",
    "text-white",
    "border-white",
    "bg-black",
    "text-black",
    "border-black",
    "text-left",
    "text-center",
    "text-right",
    "text-justify",
    "uppercase",
    "lowercase",
    "capitalize",
    "italic",
    "not-italic",
    "underline",
    "line-through",
    "no-underline",
    "font-mono",
    "font-sans",
    "font-serif",
    "tracking-tighter",
    "tracking-tight",
    "tracking-normal",
    "tracking-wide",
    "tracking-wider",
    "tracking-widest",
    "leading-none",
    "leading-tight",
    "leading-normal",
    "leading-relaxed",
    "leading-loose",
    "mx-auto",
    "my-auto",
    "font-thin",
    "font-light",
    "font-normal",
    "font-medium",
    "font-semibold",
    "font-bold",
    "font-black",
    "text-xs",
    "text-sm",
    "text-base",
    "text-lg",
    "text-xl",
    "text-2xl",
    "text-3xl",
    "text-4xl",
    "text-5xl",
    "text-6xl",
    "whitespace-nowrap",
    "whitespace-pre",
    "break-words",
    "rounded-none",
    "rounded-sm",
    "rounded",
    "rounded-md",
    "rounded-lg",
    "rounded-xl",
    "rounded-2xl",
    "rounded-3xl",
    "rounded-full",
    "border",
    "border-0",
    "border-2",
    "border-4",
    "border-8",
    "border-t-2",
    "border-b-2",
    "border-solid",
    "border-dashed",
    "border-dotted",
    "border-none",
    "outline-none",
    "shadow-sm",
    "shadow",
    "shadow-md",
    "shadow-lg",
    "shadow-xl",
    "shadow-none",
    "transition-all",
    "transition",
    "cursor-pointer",
    "cursor-default",
    "cursor-not-allowed",
    "animate-spin",
    "animate-ping",
    "animate-pulse",
    "animate-bounce",
    "animate-none",
    "will-change-transform",
    "will-change-scroll",
    "will-change-auto",
    "blur-none",
    "blur-sm",
    "blur",
    "blur-md",
    "blur-lg",
    "blur-xl",
    "blur-2xl",
    "blur-3xl",
    "backdrop-blur-none",
    "backdrop-blur-sm",
    "backdrop-blur",
    "backdrop-blur-md",
    "backdrop-blur-lg",
    "backdrop-blur-xl",
    "backdrop-blur-2xl",
    "backdrop-blur-3xl",
    "scale-0",
    "scale-50",
    "scale-75",
    "scale-90",
    "scale-95",
    "scale-100",
    "scale-105",
    "scale-110",
    "scale-125",
    "scale-150",
    "rotate-0",
    "rotate-45",
    "rotate-90",
    "rotate-180",
    "-rotate-45",
    "-rotate-90",
    "-rotate-180",
    "translate-x-full",
    "translate-y-full",
    "-translate-x-full",
    "-translate-y-full",
    "translate-x-0",
    "translate-y-0",
    "p-4",
    "px-4",
    "py-4",
    "m-4",
    "mx-4",
    "my-4",
    "gap-4",
    "w-4",
    "h-4",
    "opacity-50",
    "duration-200",
];

fn levenshtein_distance_slice<T: PartialEq>(s1: &[T], s2: &[T]) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    if len2 == 0 {
        return len1;
    }

    if len2 <= 64 {
        let mut row = [0usize; 65];
        for (i, val) in row.iter_mut().take(len2 + 1).enumerate() {
            *val = i;
        }

        for i in 1..=len1 {
            let mut prev_diag = row[0];
            row[0] = i;
            for j in 1..=len2 {
                let old_row_j = row[j];
                let cost = if s1[i - 1] == s2[j - 1] { 0 } else { 1 };
                row[j] = (row[j] + 1).min(row[j - 1] + 1).min(prev_diag + cost);
                prev_diag = old_row_j;
            }
        }
        row[len2]
    } else {
        let mut row: Vec<usize> = (0..=len2).collect();
        for i in 1..=len1 {
            let mut prev_diag = row[0];
            row[0] = i;
            for j in 1..=len2 {
                let old_row_j = row[j];
                let cost = if s1[i - 1] == s2[j - 1] { 0 } else { 1 };
                row[j] = (row[j] + 1).min(row[j - 1] + 1).min(prev_diag + cost);
                prev_diag = old_row_j;
            }
        }
        row[len2]
    }
}

/// 计算两个字符串之间的 Levenshtein 编辑距离
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    if s1.is_ascii() && s2.is_ascii() {
        levenshtein_distance_slice(s1.as_bytes(), s2.as_bytes())
    } else {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        levenshtein_distance_slice(&s1_chars, &s2_chars)
    }
}

/// 在候选词列表中寻找与给定 token 最相近的 Utility 建议
pub fn find_best_suggestion(token: &str) -> Option<String> {
    let mut best_match = None;
    let mut min_dist = usize::MAX;

    let mut candidates: Vec<String> = CANDIDATE_UTILITIES.iter().map(|s| s.to_string()).collect();

    if let Some(cfg) = crate::css::config::get_config() {
        for color_key in cfg.theme.colors.keys().chain(cfg.theme.dark_colors.keys()) {
            candidates.push(color_key.clone());
            candidates.push(format!("bg-{}", color_key));
            candidates.push(format!("text-{}", color_key));
            candidates.push(format!("border-{}", color_key));
        }
        for bp_key in cfg.theme.breakpoints.keys() {
            candidates.push(bp_key.clone());
        }
    }

    for candidate in &candidates {
        let dist = levenshtein_distance(token, candidate);
        let max_allowed = if candidate.len() <= 4 { 2 } else { 3 };
        if dist <= max_allowed && dist < min_dist {
            min_dist = dist;
            best_match = Some(candidate.clone());
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("flex", "flexx"), 1);
        assert_eq!(levenshtein_distance("items-center", "items-centerr"), 1);
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }

    #[test]
    fn test_find_best_suggestion() {
        assert_eq!(find_best_suggestion("flexx"), Some("flex".to_string()));
        assert_eq!(
            find_best_suggestion("items-centerr"),
            Some("items-center".to_string())
        );
        assert_eq!(find_best_suggestion("completely_unknown_token"), None);
    }
}
