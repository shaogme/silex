use std::collections::HashMap;

/// 路径匹配结果
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    /// 提取的路径参数 (e.g. "/users/:id" 匹配 "/users/123" -> {"id": "123"})
    pub params: HashMap<String, String>,
    /// 剩余未匹配的路径 (用于嵌套路由，暂不使用)
    pub remaining_path: String,
}

/// 检查路由模式是否匹配当前路径
///
/// 支持的模式:
/// - 静态匹配: "/users/profile"
/// - 参数匹配: "/users/:id"
/// - 通配符: "/docs/*" (匹配 /docs/a/b/c)
///
/// # Arguments
/// * `pattern` - 路由定义模式
/// * `path` - 当前 URL 路径
/// * `partial` - 是否允许部分匹配 (用于父级路由匹配)
///
/// # Returns
/// 如果匹配成功，返回包含参数的 MatchResult，否则返回 None
pub fn match_path(pattern: &str, path: &str, partial: bool) -> Option<MatchResult> {
    let pattern_segments: Vec<&str> = pattern
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let path_segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    let mut params = HashMap::new();

    // 根路径特殊处理: pattern "/" (segments empty) matches path "/" (segments empty)
    if pattern_segments.is_empty() {
        if path_segments.is_empty() {
            return Some(MatchResult {
                params,
                remaining_path: String::new(),
            });
        } else if partial {
            // 如果是 partial 匹配，pattern 是 "" 或 "/"，它匹配任何路径的前缀（实际上不消耗任何路径）
            // 但通常 "/" 路由作为父路由时，意为 wrapper。
            // 这里约定: 如果 pattern 是空的，它不消耗 path，param 为空，remaining 为 whole path
            let remaining = "/".to_string() + &path_segments.join("/");
            return Some(MatchResult {
                params,
                remaining_path: remaining,
            });
        } else {
            return None;
        }
    }

    // 遍历模式段
    for (i, segment) in pattern_segments.iter().enumerate() {
        // 处理通配符 *
        if *segment == "*" {
            // 通配符匹配剩余所有路径
            let remaining = if i < path_segments.len() {
                "/".to_string() + &path_segments[i..].join("/")
            } else {
                String::new()
            };
            return Some(MatchResult {
                params,
                remaining_path: remaining,
            });
        }

        // 检查 path 是否还有对应段
        if i >= path_segments.len() {
            return None; // 路径比模式短
        }

        let path_segment = path_segments[i];

        if segment.starts_with(':') {
            // 参数匹配
            let param_name = &segment[1..];
            params.insert(param_name.to_string(), path_segment.to_string());
        } else if segment != &path_segment {
            // 静态匹配失败
            return None;
        }
    }

    // 检查是否有剩余路径
    if path_segments.len() > pattern_segments.len() {
        if partial {
            // 允许部分匹配，返回剩余路径
            let remaining = "/".to_string() + &path_segments[pattern_segments.len()..].join("/");
            return Some(MatchResult {
                params,
                remaining_path: remaining,
            });
        } else {
            // 完全匹配模式下，路径不能比模式长
            return None;
        }
    }

    Some(MatchResult {
        params,
        remaining_path: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_match() {
        assert!(match_path("/", "/", false).is_some());
        assert!(match_path("/users", "/users", false).is_some());
        assert!(match_path("/users", "/posts", false).is_none());
    }

    #[test]
    fn test_param_match() {
        let res = match_path("/users/:id", "/users/123", false).unwrap();
        assert_eq!(res.params.get("id").unwrap(), "123");

        let res = match_path("/users/:id/posts/:pid", "/users/1/posts/99", false).unwrap();
        assert_eq!(res.params.get("id").unwrap(), "1");
        assert_eq!(res.params.get("pid").unwrap(), "99");
    }

    #[test]
    fn test_wildcard() {
        assert!(match_path("/docs/*", "/docs/api/v1", false).is_some());
        assert!(match_path("/*", "/any/thing", false).is_some());
    }

    #[test]
    fn test_partial_match() {
        let res = match_path("/users", "/users/123", true).unwrap();
        assert_eq!(res.remaining_path, "/123");

        // 根路径前缀
        let res = match_path("/", "/users", true).unwrap();
        // "/" pattern segments is empty.
        assert_eq!(res.remaining_path, "/users");
    }
}
