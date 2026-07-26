use super::codegen::table::{DYNAMIC_UTILITY_PREFIXES, STATIC_CANDIDATE_UTILITIES};

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

/// 在变体前缀候选集中寻找与给定前缀最相近的建议
///
/// 候选集为 `MODIFIER_TABLE` 全表 + 配置中的自定义断点 + 内建的函数式变体前缀。
pub fn find_best_modifier_suggestion(prefix: &str) -> Option<String> {
    let mut best_match = None;
    let mut min_dist = usize::MAX;

    let mut check_candidate = |cand: &str| {
        let dist = levenshtein_distance(prefix, cand);
        let max_allowed = if cand.len() <= 4 { 2 } else { 3 };
        if dist <= max_allowed && dist < min_dist {
            min_dist = dist;
            best_match = Some(cand.to_string());
        }
    };

    for meta in super::codegen::modifiers::MODIFIER_TABLE {
        check_candidate(meta.key);
    }
    if let Some(cfg) = crate::css::config::get_config() {
        for bp_key in cfg.theme.breakpoints.keys() {
            check_candidate(bp_key);
        }
    }

    best_match
}

/// 候选词中出现过的 ASCII 小写字母集合，压成 32 位掩码。
///
/// 用途是给编辑距离做**无损**粗筛：token 里有、候选里没有的每一个字符，
/// 至少要一次删除或替换才能消掉，因此
/// `编辑距离 ≥ popcount(token 掩码 & !候选掩码)`。
/// 这个下界成立与否与字符出现次数无关，所以按位取集合就够了。
#[inline]
fn letter_mask(s: &str) -> u32 {
    let mut mask = 0u32;
    for &b in s.as_bytes() {
        if b.is_ascii_lowercase() {
            mask |= 1 << (b - b'a');
        }
    }
    mask
}

/// 遍历全部 Utility 候选词
///
/// 静态表 22 879 条 + 动态前缀笛卡尔积 + 配置颜色的 4 倍展开。
/// 抽成一个函数是为了让"粗筛版"与"朴素版"跑的是同一个候选集合，
/// 测试才能拿两者对拍（报告 §4.2）。
fn for_each_utility_candidate(mut visit: impl FnMut(&str)) {
    for &cand in STATIC_CANDIDATE_UTILITIES {
        visit(cand);
    }

    let mut buf = String::with_capacity(32);
    for (prefix, suffixes) in DYNAMIC_UTILITY_PREFIXES {
        for suffix in *suffixes {
            buf.clear();
            buf.push_str(prefix);
            buf.push_str(suffix);
            visit(&buf);
        }
    }

    if let Some(cfg) = crate::css::config::get_config() {
        for color_key in cfg.theme.colors.keys().chain(cfg.theme.dark_colors.keys()) {
            visit(color_key);
            for prefix in ["bg-", "text-", "border-"] {
                buf.clear();
                buf.push_str(prefix);
                buf.push_str(color_key);
                visit(&buf);
            }
        }
        for bp_key in cfg.theme.breakpoints.keys() {
            visit(bp_key);
        }
    }
}

/// 在候选词列表中寻找与给定 token 最相近的 Utility 建议
///
/// 全量算编辑距离是 `O(候选数 × len_token × len_cand)`，在 22 879 条候选上单次错误
/// 就要跑上千万次内层循环；一个文件里几十个笔误的编译期开销肉眼可感（报告 §4.2）。
///
/// 这里先用两个**下界**把绝大多数候选挡在 Levenshtein 之外：
///
/// 1. 长度差——`编辑距离 ≥ |len(a) - len(b)|`；
/// 2. 字符集差——见 [`letter_mask`]。
///
/// 两者都是严格下界，所以粗筛不会改变结果，只会更快；
/// 并且阈值随已找到的最优解收紧（后来的候选必须**更好**才有意义）。
pub fn find_best_suggestion(token: &str) -> Option<String> {
    let mut best_match: Option<String> = None;
    let mut min_dist = usize::MAX;

    let token_len = token.len();
    let token_mask = letter_mask(token);

    for_each_utility_candidate(|cand| {
        // 候选越短容忍度越低，同时后来者必须严格优于当前最优
        let cap = if cand.len() <= 4 { 2 } else { 3 };
        let bound = cap.min(min_dist.saturating_sub(1));

        if token_len.abs_diff(cand.len()) > bound {
            return;
        }
        if (token_mask & !letter_mask(cand)).count_ones() as usize > bound {
            return;
        }

        let dist = levenshtein_distance(token, cand);
        if dist <= cap && dist < min_dist {
            min_dist = dist;
            best_match = Some(cand.to_string());
        }
    });

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

    /// 不带任何粗筛的朴素实现，只用于给粗筛版对拍
    fn find_best_suggestion_naive(token: &str) -> Option<String> {
        let mut best_match: Option<String> = None;
        let mut min_dist = usize::MAX;
        for_each_utility_candidate(|cand| {
            let dist = levenshtein_distance(token, cand);
            let max_allowed = if cand.len() <= 4 { 2 } else { 3 };
            if dist <= max_allowed && dist < min_dist {
                min_dist = dist;
                best_match = Some(cand.to_string());
            }
        });
        best_match
    }

    /// 粗筛必须是**无损**的：它只允许更快，不允许换答案。
    ///
    /// 长度差与字符集差都是编辑距离的严格下界，所以这条性质应当对任意输入成立；
    /// 这里拿一批形状各异的笔误逐个对拍，防止将来有人往粗筛里塞一个"看起来挺合理"
    /// 但其实不是下界的启发式（比如按首字母分桶）。
    #[test]
    fn prefilter_never_changes_the_suggestion() {
        let cases = [
            "flexx",
            "items-centerr",
            "shadow-mdd",
            "px-444",
            "p-44x",
            "bg-red-5000",
            "rounde-lg",
            "grid-clos-3",
            "justify-betwen",
            "",
            "z",
            "zz",
            "text",
            "completely_unknown_token",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "UPPERCASE-Flex",
            "p-4",
            "hover",
        ];
        for token in cases {
            assert_eq!(
                find_best_suggestion(token),
                find_best_suggestion_naive(token),
                "粗筛改变了 `{token}` 的建议结果"
            );
        }
    }

    #[test]
    fn letter_mask_bound_is_a_real_lower_bound() {
        // token 有、候选没有的字符数量是编辑距离的下界
        for (a, b) in [
            ("flex", "grid"),
            ("p-4", "m-8"),
            ("items-center", "justify-between"),
            ("rounded", "rounded-lg"),
        ] {
            let missing = (letter_mask(a) & !letter_mask(b)).count_ones() as usize;
            assert!(
                missing <= levenshtein_distance(a, b),
                "`{a}` vs `{b}`: 掩码下界 {missing} 超过了真实距离"
            );
        }
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
