use crate::ToRoute;
use silex_core::{
    ErrorReporter, Rx, Scope, SilexContext, SilexContextProvider, SilexError, SilexErrorKind,
    SilexResult,
    reactivity::{ReadSignal, StoredValue, WriteSignal},
    traits::RxGet,
};
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{
    AnyView, ApplyAttributes, MountErrorHandler, MountInstance, MountOwner, View,
};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::Node;

/// Router View 工厂包装器，必须实现 PartialEq 以便在 Signal/Memo 中使用
#[derive(Clone)]
pub struct RouterView<'scope>(pub Rc<dyn Fn() -> AnyView<'scope> + 'scope>);

impl PartialEq for RouterView<'_> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<'scope> ApplyAttributes<'scope> for RouterView<'scope> {}

impl<'scope> View<'scope> for RouterView<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let view = (self.0)();
        view.mount(owner, parent, attrs, error_handler)
    }
}

/// 路由上下文，存储当前的路由状态
#[derive(Clone, Copy)]
pub struct RouterContext<'scope> {
    silex: SilexContext<'scope>,
    /// 基础路径 (e.g. "/app")
    pub base_path: StoredValue<'scope, String>,
    /// 当前路径 (pathname, relative to base_path)
    pub path: ReadSignal<'scope, String>,
    /// 当前查询参数 (search string)
    pub search: ReadSignal<'scope, String>,
    /// 导航控制器
    pub navigator: Navigator<'scope>,
    query: Rx<'scope, HashMap<String, String>>,
}

impl<'scope> RouterContext<'scope> {
    /// Create a RouterContext after validating every read and write source.
    pub fn new(
        silex: SilexContext<'scope>,
        props: RouterContextProps<'scope>,
    ) -> SilexResult<Self> {
        let scope = silex.scope();
        let RouterContextProps {
            base_path: raw_base_path,
            path,
            search,
            set_path,
            set_search,
        } = props;

        scope.validate_runtime(&path.into_rx())?;
        scope.validate_runtime(&search.into_rx())?;
        scope.validate_runtime(&set_path)?;
        scope.validate_runtime(&set_search)?;
        let base_path = scope.stored(normalize_base_path(&raw_base_path))?;
        let navigator = Navigator {
            base_path,
            path,
            search,
            set_path,
            set_search,
        };
        let query = scope
            .memo(
                move |_| {
                    let search = search.get()?;
                    parse_query(&search)
                },
                silex.error_reporter(),
            )
            .map(|memo| memo.into_rx())?;
        Ok(Self {
            base_path,
            path,
            search,
            navigator,
            silex,
            query,
        })
    }

    pub fn scope(self) -> Scope<'scope> {
        self.silex.scope()
    }

    pub fn error_reporter(self) -> ErrorReporter<'scope> {
        self.silex.error_reporter()
    }

    /// 获取解析后的查询参数 Memo
    pub fn query_map(self) -> Rx<'scope, HashMap<String, String>> {
        self.query
    }
}

impl<'scope> SilexContextProvider<'scope> for RouterContext<'scope> {
    fn scope(&self) -> Scope<'scope> {
        self.silex.scope()
    }

    fn error_reporter(&self) -> ErrorReporter<'scope> {
        self.silex.error_reporter()
    }

    fn with_error_reporter(self, reporter: ErrorReporter<'scope>) -> Self {
        Self {
            silex: self.silex.with_error_reporter(reporter),
            ..self
        }
    }
}

fn parse_query(search: &str) -> SilexResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    let params = web_sys::UrlSearchParams::new_with_str(search).map_err(SilexError::fatal)?;
    if let Some(iter) = js_sys::try_iter(&params).map_err(SilexError::fatal)? {
        for val in iter {
            let val = val.map_err(SilexError::fatal)?;
            let pair: js_sys::Array = val.unchecked_into();
            let key = pair.get(0).as_string().ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Javascript(
                    "query key is not a string".to_string(),
                ))
            })?;
            let value = pair.get(1).as_string().ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Javascript(
                    "query value is not a string".to_string(),
                ))
            })?;
            map.insert(key, value);
        }
    }
    Ok(map)
}

pub(crate) fn normalize_base_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }

    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn strip_base_path(base: &str, raw_path: &str) -> String {
    let base = normalize_base_path(base);
    if base == "/" {
        return if raw_path.is_empty() {
            "/".to_string()
        } else {
            raw_path.to_string()
        };
    }

    if raw_path == base {
        "/".to_string()
    } else if let Some(rest) = raw_path.strip_prefix(&(base + "/")) {
        if rest.is_empty() {
            "/".to_string()
        } else {
            format!("/{rest}")
        }
    } else {
        raw_path.to_string()
    }
}

pub(crate) fn build_history_url(base: &str, logical_path: &str) -> String {
    if !logical_path.starts_with('/') {
        return logical_path.to_string();
    }

    let base = normalize_base_path(base);
    if base == "/" {
        logical_path.to_string()
    } else {
        format!("{}{}", base, logical_path)
    }
}

/// 导航控制器，用于执行路由跳转
#[derive(Clone, Copy)]
pub struct Navigator<'scope> {
    pub base_path: StoredValue<'scope, String>,
    pub path: ReadSignal<'scope, String>,
    pub search: ReadSignal<'scope, String>,
    pub set_path: WriteSignal<'scope, String>,
    pub set_search: WriteSignal<'scope, String>,
}

impl<'scope> Navigator<'scope> {
    fn handle_navigation(self, url: &str, replace: bool) -> SilexResult<()> {
        let window = web_sys::window().ok_or_else(|| {
            SilexError::fatal(SilexErrorKind::Javascript(
                "no global `window` exists".to_string(),
            ))
        })?;

        // 1. 构造用于浏览器历史记录的完整 URL
        let base_path = normalize_base_path(&self.base_path.get_untracked()?);
        let full_url = build_history_url(&base_path, url);

        // 2. 使用 History API
        let history = window.history().map_err(SilexError::fatal)?;
        if replace {
            history
                .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&full_url))
                .map_err(SilexError::fatal)?;
        } else {
            history
                .push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&full_url))
                .map_err(SilexError::fatal)?;
        }

        self.refresh_location()
    }

    pub(crate) fn refresh_location(self) -> SilexResult<()> {
        let window = web_sys::window().ok_or_else(|| {
            SilexError::fatal(SilexErrorKind::Javascript(
                "no global `window` exists".to_string(),
            ))
        })?;
        let base_path = normalize_base_path(&self.base_path.get_untracked()?);

        // 读取当前状态并更新信号 (需要剥离 base_path)
        let location = window.location();
        let raw_path = location.pathname().map_err(SilexError::fatal)?;

        let logical_path = strip_base_path(&base_path, &raw_path);

        let search = location.search().map_err(SilexError::fatal)?;

        // 更新信号 (带去重，避免不必要的副作用)
        if self.path.get_untracked()? != logical_path {
            self.set_path.set(logical_path).map_err(SilexError::fatal)?;
        }

        if self.search.get_untracked()? != search {
            self.set_search.set(search).map_err(SilexError::fatal)?;
        }
        Ok(())
    }

    /// 导航到指定路径
    pub fn push<T: ToRoute>(self, to: T) -> SilexResult<()> {
        self.handle_navigation(&to.to_route(), false)
    }

    /// 替换当前路径
    pub fn replace<T: ToRoute>(self, to: T) -> SilexResult<()> {
        self.handle_navigation(&to.to_route(), true)
    }

    /// 设置或更新查询参数
    ///
    /// * `key`: 参数名
    /// * `value`: 参数值。如果为 `None`，则删除该参数。
    pub fn set_query(self, key: &str, value: Option<&str>) -> SilexResult<()> {
        let current_search = self.search.get_untracked()?;

        let params =
            web_sys::UrlSearchParams::new_with_str(&current_search).map_err(SilexError::fatal)?;
        match value {
            Some(v) => params.set(key, v),
            None => params.delete(key),
        }

        let new_search = params.to_string().as_string().ok_or_else(|| {
            SilexError::fatal(SilexErrorKind::Javascript(
                "query serialization is not a string".to_string(),
            ))
        })?;
        let new_search = if new_search.is_empty() {
            String::new()
        } else {
            format!("?{new_search}")
        };

        if new_search == current_search {
            return Ok(());
        }

        let pathname = self.path.get_untracked()?;
        let new_url = if new_search.is_empty() {
            pathname
        } else {
            format!("{}{}", pathname, new_search)
        };

        self.push(&new_url)
    }
}

/// 路由上下文所需的属性集合
#[derive(Clone)]
pub struct RouterContextProps<'scope> {
    pub base_path: String,
    pub path: ReadSignal<'scope, String>,
    pub search: ReadSignal<'scope, String>,
    pub set_path: WriteSignal<'scope, String>,
    pub set_search: WriteSignal<'scope, String>,
}

#[cfg(test)]
mod tests {
    use super::{build_history_url, normalize_base_path, strip_base_path};

    #[test]
    fn base_path_helpers_normalize_and_respect_segment_boundaries() {
        assert_eq!(normalize_base_path(""), "/");
        assert_eq!(normalize_base_path("app/"), "/app");
        assert_eq!(normalize_base_path("/app/"), "/app");
        assert_eq!(strip_base_path("/app", "/app"), "/");
        assert_eq!(strip_base_path("/app", "/app/"), "/");
        assert_eq!(strip_base_path("/app", "/app/users"), "/users");
        assert_eq!(strip_base_path("/app", "/application"), "/application");
    }

    #[test]
    fn history_urls_use_logical_paths() {
        assert_eq!(build_history_url("/", "/users"), "/users");
        assert_eq!(build_history_url("/app", "/"), "/app/");
        assert_eq!(
            build_history_url("/app", "/users?tab=all"),
            "/app/users?tab=all"
        );
        assert_eq!(
            build_history_url("/app", "https://example.test/users"),
            "https://example.test/users"
        );
    }
}
