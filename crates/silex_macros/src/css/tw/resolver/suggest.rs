use super::table::{DYNAMIC_UTILITY_PREFIXES, STATIC_CANDIDATE_UTILITIES};

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

    let mut check_candidate = |cand: &str| {
        let dist = levenshtein_distance(token, cand);
        let max_allowed = if cand.len() <= 4 { 2 } else { 3 };
        if dist <= max_allowed && dist < min_dist {
            min_dist = dist;
            best_match = Some(cand.to_string());
        }
    };

    for &cand in STATIC_CANDIDATE_UTILITIES {
        check_candidate(cand);
    }

    let mut dyn_buf = String::with_capacity(32);
    for (prefix, suffixes) in DYNAMIC_UTILITY_PREFIXES {
        for suffix in *suffixes {
            dyn_buf.clear();
            dyn_buf.push_str(prefix);
            dyn_buf.push_str(suffix);
            check_candidate(&dyn_buf);
        }
    }

    if let Some(cfg) = crate::css::config::get_config() {
        for color_key in cfg.theme.colors.keys().chain(cfg.theme.dark_colors.keys()) {
            check_candidate(color_key);

            dyn_buf.clear();
            dyn_buf.push_str("bg-");
            dyn_buf.push_str(color_key);
            check_candidate(&dyn_buf);

            dyn_buf.clear();
            dyn_buf.push_str("text-");
            dyn_buf.push_str(color_key);
            check_candidate(&dyn_buf);

            dyn_buf.clear();
            dyn_buf.push_str("border-");
            dyn_buf.push_str(color_key);
            check_candidate(&dyn_buf);
        }
        for bp_key in cfg.theme.breakpoints.keys() {
            check_candidate(bp_key);
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
        assert_eq!(
            find_best_suggestion("shadow-mdd"),
            Some("shadow-md".to_string())
        );
        assert_eq!(find_best_suggestion("px-444"), Some("px-44".to_string()));
        assert_eq!(find_best_suggestion("completely_unknown_token"), None);
    }
}
