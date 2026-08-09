use crate::{
    RouteOutlet, RouterContext,
    path::{
        PathError, PathParam, RawPathSegment, RoutePath, normalize_path, percent_decode_segment,
        raw_path_segments,
    },
};
use silex_dom::view::{AnyView, View};
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    rc::Rc,
};

/// The position of a route in a route table.
pub type RouteId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RouteBranchKey {
    Empty,
    Matched {
        route_id: RouteId,
        params: Vec<String>,
    },
}

/// A raw route parameter captured from a pathname.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteParam<'path> {
    name: String,
    raw: &'path str,
}

impl<'path> RouteParam<'path> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn raw(&self) -> &str {
        self.raw
    }

    pub fn parse<T>(&self) -> Result<T, RouteMatchError<T::Error>>
    where
        T: PathParam,
    {
        T::decode_segment(self.raw).map_err(RouteMatchError::Decode)
    }
}

/// A route match that borrows the pathname supplied to the matcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteMatch<'path> {
    path: &'path str,
    route_id: RouteId,
    params: Vec<RouteParam<'path>>,
}

impl<'path> RouteMatch<'path> {
    pub fn path(&self) -> &str {
        self.path
    }

    pub fn route_id(&self) -> RouteId {
        self.route_id
    }

    pub fn params(&self) -> &[RouteParam<'path>] {
        &self.params
    }

    pub fn param(&self, name: &str) -> Option<&RouteParam<'path>> {
        self.params.iter().find(|param| param.name() == name)
    }

    pub fn raw(&self, name: &str) -> Option<&str> {
        self.param(name).map(RouteParam::raw)
    }

    pub fn parse<T>(&self, name: &str) -> Result<T, RouteMatchError<T::Error>>
    where
        T: PathParam,
    {
        self.param(name)
            .ok_or_else(|| RouteMatchError::Missing(name.to_string()))?
            .parse()
    }
}

/// Errors raised when a handler reads a route parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteMatchError<E> {
    Missing(String),
    Decode(E),
}

impl<E: Display> Display for RouteMatchError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Missing(name) => write!(formatter, "route parameter '{name}' is missing"),
            Self::Decode(error) => Display::fmt(error, formatter),
        }
    }
}

impl<E: Display + std::fmt::Debug + 'static> Error for RouteMatchError<E> {}

/// Errors raised while compiling a route pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutePatternError {
    Path(PathError),
    InvalidPattern { pattern: String, reason: String },
    DuplicateParameter { pattern: String, name: String },
    DuplicatePattern { pattern: String },
}

impl Display for RoutePatternError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Path(error) => Display::fmt(error, formatter),
            Self::InvalidPattern { pattern, reason } => {
                write!(formatter, "invalid route pattern '{pattern}': {reason}")
            }
            Self::DuplicateParameter { pattern, name } => {
                write!(
                    formatter,
                    "route pattern '{pattern}' repeats parameter '{name}'"
                )
            }
            Self::DuplicatePattern { pattern } => {
                write!(
                    formatter,
                    "route pattern '{pattern}' is declared more than once"
                )
            }
        }
    }
}

impl Error for RoutePatternError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            Self::InvalidPattern { .. }
            | Self::DuplicateParameter { .. }
            | Self::DuplicatePattern { .. } => None,
        }
    }
}

impl From<PathError> for RoutePatternError {
    fn from(error: PathError) -> Self {
        Self::Path(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PatternKeySegment {
    Static(String),
    Param,
    Wildcard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PatternSegment {
    Static(String),
    Param(String),
    Wildcard(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompiledRoute {
    pattern: String,
    segments: Vec<PatternSegment>,
    key: Vec<PatternKeySegment>,
}

impl CompiledRoute {
    fn parse(pattern: impl AsRef<str>) -> Result<Self, RoutePatternError> {
        let pattern = pattern.as_ref();
        let normalized = crate::path::normalize_path(pattern)?;
        let raw_segments = raw_path_segments(&normalized)?;
        let mut names = HashSet::new();
        let mut segments = Vec::new();

        for (index, raw_segment) in raw_segments.iter().enumerate() {
            let raw = raw_segment.raw;
            if let Some(name) = raw.strip_prefix(':') {
                validate_parameter_name(&normalized, name)?;
                if !names.insert(name.to_string()) {
                    return Err(RoutePatternError::DuplicateParameter {
                        pattern: normalized.clone(),
                        name: name.to_string(),
                    });
                }
                segments.push(PatternSegment::Param(name.to_string()));
            } else if let Some(name) = raw.strip_prefix('*') {
                if index + 1 != raw_segments.len() {
                    return Err(RoutePatternError::InvalidPattern {
                        pattern: normalized.clone(),
                        reason: "wildcard must be the final segment".to_string(),
                    });
                }
                if !name.is_empty() {
                    validate_parameter_name(&normalized, name)?;
                    if !names.insert(name.to_string()) {
                        return Err(RoutePatternError::DuplicateParameter {
                            pattern: normalized.clone(),
                            name: name.to_string(),
                        });
                    }
                }
                segments.push(PatternSegment::Wildcard(if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }));
            } else {
                segments.push(PatternSegment::Static(percent_decode_segment(raw)?));
            }
        }

        let key = segments
            .iter()
            .map(|segment| match segment {
                PatternSegment::Static(value) => PatternKeySegment::Static(value.clone()),
                PatternSegment::Param(_) => PatternKeySegment::Param,
                PatternSegment::Wildcard(_) => PatternKeySegment::Wildcard,
            })
            .collect();

        Ok(Self {
            pattern: normalized,
            segments,
            key,
        })
    }
}

fn validate_parameter_name(pattern: &str, name: &str) -> Result<(), RoutePatternError> {
    let mut chars = name.chars();
    let valid_first = chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    let valid_rest = chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !valid_first || !valid_rest {
        return Err(RoutePatternError::InvalidPattern {
            pattern: pattern.to_string(),
            reason: format!("invalid parameter name '{name}'"),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct MatcherNode {
    static_children: BTreeMap<String, MatcherNode>,
    param_child: Option<Box<MatcherNode>>,
    exact_routes: Vec<RouteId>,
    wildcard_routes: Vec<RouteId>,
}

impl MatcherNode {
    fn insert(&mut self, segments: &[PatternSegment], route_id: RouteId) {
        let Some(segment) = segments.first() else {
            self.exact_routes.push(route_id);
            return;
        };

        match segment {
            PatternSegment::Static(value) => self
                .static_children
                .entry(value.clone())
                .or_default()
                .insert(&segments[1..], route_id),
            PatternSegment::Param(_) => self
                .param_child
                .get_or_insert_with(|| Box::new(Self::default()))
                .insert(&segments[1..], route_id),
            PatternSegment::Wildcard(_) => self.wildcard_routes.push(route_id),
        }
    }

    fn collect<'path>(
        &self,
        routes: &[CompiledRoute],
        path: &'path str,
        segments: &[RawPathSegment<'path>],
        depth: usize,
        captures: &[&'path str],
        matches: &mut Vec<RouteMatch<'path>>,
    ) {
        if depth == segments.len() {
            for route_id in &self.exact_routes {
                push_route_match(routes, path, *route_id, captures, None, matches);
            }
            for route_id in &self.wildcard_routes {
                push_route_match(routes, path, *route_id, captures, Some(""), matches);
            }
            return;
        }

        let segment = &segments[depth];
        if let Ok(decoded) = percent_decode_segment(segment.raw)
            && let Some(child) = self.static_children.get(&decoded)
        {
            child.collect(routes, path, segments, depth + 1, captures, matches);
        }

        if let Some(child) = self.param_child.as_deref() {
            let mut next_captures = captures.to_vec();
            next_captures.push(segment.raw);
            child.collect(routes, path, segments, depth + 1, &next_captures, matches);
        }

        for route_id in &self.wildcard_routes {
            let tail = &path[segments[depth].start..segments.last().unwrap().end];
            push_route_match(routes, path, *route_id, captures, Some(tail), matches);
        }
    }
}

fn push_route_match<'path>(
    routes: &[CompiledRoute],
    path: &'path str,
    route_id: RouteId,
    captures: &[&'path str],
    wildcard_tail: Option<&'path str>,
    matches: &mut Vec<RouteMatch<'path>>,
) {
    let route = &routes[route_id];
    let mut capture_index = 0;
    let mut params = Vec::new();

    for segment in &route.segments {
        match segment {
            PatternSegment::Param(name) => {
                let Some(raw) = captures.get(capture_index) else {
                    return;
                };
                params.push(RouteParam {
                    name: name.clone(),
                    raw,
                });
                capture_index += 1;
            }
            PatternSegment::Wildcard(Some(name)) => {
                let Some(raw) = wildcard_tail else {
                    return;
                };
                params.push(RouteParam {
                    name: name.clone(),
                    raw,
                });
            }
            PatternSegment::Static(_) | PatternSegment::Wildcard(None) => {}
        }
    }

    matches.push(RouteMatch {
        path,
        route_id,
        params,
    });
}

/// A compiled, immutable-priority matcher independent from rendering.
#[derive(Clone, Debug, Default)]
pub struct RouteMatcher {
    root: MatcherNode,
    routes: Vec<CompiledRoute>,
}

impl RouteMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_patterns<I, S>(patterns: I) -> Result<Self, RoutePatternError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut matcher = Self::new();
        for pattern in patterns {
            matcher.add_pattern(pattern)?;
        }
        Ok(matcher)
    }

    pub fn add_pattern(&mut self, pattern: impl AsRef<str>) -> Result<RouteId, RoutePatternError> {
        self.add_compiled(CompiledRoute::parse(pattern)?)
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn pattern(&self, route_id: RouteId) -> Option<&str> {
        self.routes
            .get(route_id)
            .map(|route| route.pattern.as_str())
    }

    pub fn try_matches<'path>(
        &self,
        path: &'path str,
    ) -> Result<Vec<RouteMatch<'path>>, PathError> {
        let segments = raw_path_segments(path)?;
        let mut matches = Vec::new();
        self.root
            .collect(&self.routes, path, &segments, 0, &[], &mut matches);
        Ok(matches)
    }

    pub fn matches<'path>(&self, path: &'path str) -> Vec<RouteMatch<'path>> {
        self.try_matches(path).unwrap_or_default()
    }

    pub fn match_path<'path>(&self, path: &'path str) -> Option<RouteMatch<'path>> {
        self.try_matches(path).ok()?.into_iter().next()
    }

    pub(crate) fn branch_key(&self, path: &str) -> RouteBranchKey {
        self.match_path(path)
            .map(|matched| route_branch_key(&matched))
            .unwrap_or(RouteBranchKey::Empty)
    }

    pub fn resolve<'path, T, F>(&self, path: &'path str, handler: F) -> Option<T>
    where
        F: FnMut(RouteMatch<'path>) -> Option<T>,
    {
        self.try_matches(path).ok()?.into_iter().find_map(handler)
    }

    fn add_compiled(&mut self, route: CompiledRoute) -> Result<RouteId, RoutePatternError> {
        if self.routes.iter().any(|existing| existing.key == route.key) {
            return Err(RoutePatternError::DuplicatePattern {
                pattern: route.pattern,
            });
        }

        let route_id = self.routes.len();
        self.root.insert(&route.segments, route_id);
        self.routes.push(route);
        Ok(route_id)
    }
}

fn route_branch_key(matched: &RouteMatch<'_>) -> RouteBranchKey {
    RouteBranchKey::Matched {
        route_id: matched.route_id(),
        params: matched
            .params()
            .iter()
            .map(|param| param.raw().to_string())
            .collect(),
    }
}

/// A rendering callback attached to a compiled route.
pub type RouteHandler<'scope> = Rc<
    dyn for<'path> Fn(RouteMatch<'path>, RouterContext<'scope>) -> Option<AnyView<'scope>> + 'scope,
>;

/// A route pattern and its scope-bound rendering callback.
pub struct RouteEntry<'scope> {
    route: CompiledRoute,
    handler: RouteHandler<'scope>,
}

impl<'scope> Clone for RouteEntry<'scope> {
    fn clone(&self) -> Self {
        Self {
            route: self.route.clone(),
            handler: self.handler.clone(),
        }
    }
}

impl<'scope> RouteEntry<'scope> {
    pub fn new<F>(pattern: impl AsRef<str>, handler: F) -> Result<Self, RoutePatternError>
    where
        F: for<'path> Fn(RouteMatch<'path>, RouterContext<'scope>) -> Option<AnyView<'scope>>
            + 'scope,
    {
        Ok(Self {
            route: CompiledRoute::parse(pattern)?,
            handler: Rc::new(handler),
        })
    }

    pub fn from_handler(
        pattern: impl AsRef<str>,
        handler: RouteHandler<'scope>,
    ) -> Result<Self, RoutePatternError> {
        Ok(Self {
            route: CompiledRoute::parse(pattern)?,
            handler,
        })
    }

    pub fn pattern(&self) -> &str {
        &self.route.pattern
    }
}

/// A compiled route matcher with scope-bound rendering handlers.
#[derive(Clone)]
pub struct RouteTable<'scope> {
    matcher: RouteMatcher,
    entries: Vec<RouteEntry<'scope>>,
}

impl<'scope> RouteTable<'scope> {
    pub fn from_entries<I>(entries: I) -> Result<Self, RoutePatternError>
    where
        I: IntoIterator<Item = RouteEntry<'scope>>,
    {
        let entries: Vec<_> = entries.into_iter().collect();
        let mut matcher = RouteMatcher::new();
        for entry in &entries {
            matcher.add_compiled(entry.route.clone())?;
        }
        Ok(Self { matcher, entries })
    }

    pub fn matcher(&self) -> &RouteMatcher {
        &self.matcher
    }

    pub fn matches<'path>(&self, path: &'path str) -> Result<Vec<RouteMatch<'path>>, PathError> {
        self.matcher.try_matches(path)
    }

    pub fn match_path<'path>(&self, path: &'path str) -> Option<RouteMatch<'path>> {
        self.matcher.match_path(path)
    }

    pub(crate) fn branch_key(&self, path: &str) -> RouteBranchKey {
        self.matcher.branch_key(path)
    }

    /// Compose a child route table below a static path prefix.
    pub fn nest<F, V>(self, prefix: impl AsRef<str>, child: RouteTable<'scope>, layout: F) -> Self
    where
        F: Fn(RouterContext<'scope>, AnyView<'scope>) -> V + 'scope,
        V: View<'scope> + 'scope,
    {
        self.try_nest(prefix, child, layout)
            .unwrap_or_else(|error| panic!("failed to nest route table: {error}"))
    }

    /// Fallible form of [`RouteTable::nest`] for callers that validate route
    /// composition during setup instead of panicking.
    pub fn try_nest<F, V>(
        mut self,
        prefix: impl AsRef<str>,
        child: RouteTable<'scope>,
        layout: F,
    ) -> Result<Self, RoutePatternError>
    where
        F: Fn(RouterContext<'scope>, AnyView<'scope>) -> V + 'scope,
        V: View<'scope> + 'scope,
    {
        let prefix = normalize_nest_prefix(prefix.as_ref())?;
        let pattern = if prefix == "/" {
            String::from("/*")
        } else {
            format!("{prefix}/*")
        };
        let entry = RouteEntry::new(pattern, move |_, context| {
            let outlet = RouteOutlet::nested(context, child.clone(), prefix.clone()).into_any();
            Some(layout(context, outlet).into_any())
        })?;
        self.add_entry(entry)?;
        Ok(self)
    }

    pub fn resolve(&self, path: &str, context: RouterContext<'scope>) -> Option<AnyView<'scope>> {
        self.resolve_branch(path, context).map(|(_, view)| view)
    }

    pub(crate) fn resolve_branch(
        &self,
        path: &str,
        context: RouterContext<'scope>,
    ) -> Option<(RouteBranchKey, AnyView<'scope>)> {
        self.matcher
            .try_matches(path)
            .ok()?
            .into_iter()
            .find_map(|matched| {
                let entry = self.entries.get(matched.route_id())?;
                let key = route_branch_key(&matched);
                (entry.handler)(matched, context).map(|view| (key, view))
            })
    }

    fn add_entry(&mut self, entry: RouteEntry<'scope>) -> Result<RouteId, RoutePatternError> {
        let route_id = self.matcher.add_compiled(entry.route.clone())?;
        debug_assert_eq!(route_id, self.entries.len());
        self.entries.push(entry);
        Ok(route_id)
    }
}

/// A route catalog mounted below a static prefix.
///
/// The mounted table keeps relative child patterns for matching while its
/// generated catalog methods produce destinations with the mounted prefix.
#[derive(Clone)]
pub struct MountedCatalog<'scope> {
    prefix: RoutePath,
    table: RouteTable<'scope>,
}

impl<'scope> MountedCatalog<'scope> {
    pub fn from_parts(prefix: RoutePath, table: RouteTable<'scope>) -> Self {
        Self { prefix, table }
    }

    pub fn prefix(&self) -> &RoutePath {
        &self.prefix
    }

    pub fn table(&self) -> RouteTable<'scope> {
        self.table.clone()
    }

    pub fn child_prefix(&self, suffix: impl AsRef<str>) -> RoutePath {
        let suffix = normalize_path(suffix.as_ref())
            .unwrap_or_else(|error| panic!("invalid mounted route prefix: {error}"));
        let path = if self.prefix.as_str() == "/" {
            suffix
        } else if suffix == "/" {
            self.prefix.as_str().to_string()
        } else {
            format!("{}{}", self.prefix.as_str(), suffix)
        };
        RoutePath::new(path).unwrap_or_else(|error| panic!("invalid mounted route prefix: {error}"))
    }
}

fn normalize_nest_prefix(prefix: &str) -> Result<String, RoutePatternError> {
    let normalized = normalize_path(prefix)?;
    for segment in raw_path_segments(&normalized)? {
        if segment.raw.starts_with([':', '*']) {
            return Err(RoutePatternError::InvalidPattern {
                pattern: normalized,
                reason: "nested route prefixes must contain only static segments".to_string(),
            });
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{RouteEntry, RouteMatcher, RoutePatternError, RouteTable};
    use crate::path::PathTail;
    use silex_dom::view::AnyView;

    #[test]
    fn matcher_uses_static_then_param_then_wildcard_priority() {
        let matcher =
            RouteMatcher::from_patterns(["/files/:id", "/*", "/files/*rest", "/files/new"])
                .unwrap();

        let patterns = matcher
            .try_matches("/files/new")
            .unwrap()
            .into_iter()
            .map(|matched| matcher.pattern(matched.route_id()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            patterns,
            vec!["/files/new", "/files/:id", "/files/*rest", "/*"]
        );
    }

    #[test]
    fn typed_parameter_failure_can_fall_back_to_wildcard() {
        let matcher = RouteMatcher::from_patterns(["/users/:id", "/*"]).unwrap();
        let resolved = matcher.resolve("/users/not-a-number", |matched| {
            if matched.route_id() == 0 {
                matched.parse::<u32>("id").ok().map(|_| "user")
            } else {
                Some("fallback")
            }
        });
        assert_eq!(resolved, Some("fallback"));
    }

    #[test]
    fn encoded_slash_stays_inside_one_segment() {
        let matcher = RouteMatcher::from_patterns(["/files/:name", "/*"]).unwrap();
        let matched = matcher.match_path("/files/a%2Fb").unwrap();
        assert_eq!(matched.params().len(), 1);
        assert_eq!(matched.parse::<String>("name").unwrap(), "a/b");
        assert_eq!(matched.path(), "/files/a%2Fb");
    }

    #[test]
    fn wildcard_captures_empty_and_multiple_segments() {
        let matcher = RouteMatcher::from_patterns(["/files/*rest"]).unwrap();
        let empty = matcher.match_path("/files/").unwrap();
        assert_eq!(empty.parse::<PathTail>("rest").unwrap().as_str(), "");

        let many = matcher.match_path("/files/a%2Fb/c").unwrap();
        assert_eq!(many.parse::<PathTail>("rest").unwrap().as_str(), "a/b/c");
    }

    #[test]
    fn invalid_patterns_are_rejected_before_matching() {
        assert!(matches!(
            RouteMatcher::from_patterns(["/:id/:id"]),
            Err(RoutePatternError::DuplicateParameter { .. })
        ));
        assert!(matches!(
            RouteMatcher::from_patterns(["/files/*rest/more"]),
            Err(RoutePatternError::InvalidPattern { .. })
        ));
        assert!(matches!(
            RouteMatcher::from_patterns(["/:id", "/:name"]),
            Err(RoutePatternError::DuplicatePattern { .. })
        ));
        assert!(RouteMatcher::from_patterns(["/a//b"]).is_err());
    }

    #[test]
    fn normalized_trailing_slashes_match_without_merging_empty_segments() {
        let matcher = RouteMatcher::from_patterns(["/users", "/*"]).unwrap();
        assert_eq!(matcher.match_path("/users/").unwrap().route_id(), 0);
        assert!(matcher.match_path("/users//").is_none());
        assert!(matcher.try_matches("/users//").is_err());
    }

    #[test]
    fn nested_tables_match_the_prefix_and_keep_the_nested_branch_key_stable() {
        let child = RouteTable::from_entries(vec![
            RouteEntry::new("/", |_, _| Some(AnyView::from("child"))).unwrap(),
        ])
        .unwrap();
        let parent = RouteTable::from_entries(vec![
            RouteEntry::new("/", |_, _| Some(AnyView::from("home"))).unwrap(),
        ])
        .unwrap();

        let nested = parent.nest("/users", child, |_, outlet| outlet);
        assert_eq!(nested.matcher().pattern(1), Some("/users/*"));
        assert_eq!(nested.match_path("/users/42").unwrap().route_id(), 1);
        assert_eq!(nested.branch_key("/users/1"), nested.branch_key("/users/2"));
    }

    #[test]
    fn nested_prefixes_reject_dynamic_segments() {
        let child = RouteTable::from_entries(Vec::<RouteEntry<'static>>::new()).unwrap();
        let parent = RouteTable::from_entries(Vec::<RouteEntry<'static>>::new()).unwrap();
        assert!(
            parent
                .try_nest("/:tenant", child, |_, outlet| outlet)
                .is_err()
        );
    }
}
