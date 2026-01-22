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
/// # Returns
/// 如果匹配成功，返回包含参数的 MatchResult，否则返回 None
pub fn match_path(pattern: &str, path: &str) -> Option<MatchResult> {
    let pattern_segments: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let path_segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    let mut params = HashMap::new();

    // 如果 pattern 是空的或者 "/"，且 path 也是空的或者 "/"
    if pattern_segments.len() == 1 && pattern_segments[0].is_empty() {
        if path_segments.len() == 1 && path_segments[0].is_empty() {
            return Some(MatchResult {
                params,
                remaining_path: String::new(),
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
            let remaining = path_segments[i..].join("/");
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

    // 检查路径是否比模式长（且模式最后不是 *）
    if path_segments.len() > pattern_segments.len() {
        // 除非我们支持嵌套路由的前缀匹配，否则这里应该返回 None
        // 目前实现完全匹配
        return None;
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
        assert!(match_path("/", "/").is_some());
        assert!(match_path("/users", "/users").is_some());
        assert!(match_path("/users", "/posts").is_none());
    }

    #[test]
    fn test_param_match() {
        let res = match_path("/users/:id", "/users/123").unwrap();
        assert_eq!(res.params.get("id").unwrap(), "123");

        let res = match_path("/users/:id/posts/:pid", "/users/1/posts/99").unwrap();
        assert_eq!(res.params.get("id").unwrap(), "1");
        assert_eq!(res.params.get("pid").unwrap(), "99");
    }

    #[test]
    fn test_wildcard() {
        assert!(match_path("/docs/*", "/docs/api/v1").is_some());
        assert!(match_path("/*", "/any/thing").is_some());
    }
}
