use silex_core::SilexError;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    ops::Deref,
};

pub use silex_core::{PathError, PathErrorKind, PathParamError, PathParamErrorKind};

/// Converts a typed value to and from one URL path segment.
pub trait PathParam: Sized + 'static {
    type Error: std::error::Error + Into<SilexError> + 'static;

    fn decode_segment(value: &str) -> Result<Self, Self::Error>;
    fn encode_segment(&self) -> Result<String, Self::Error>;
}

/// A decoded multi-segment value captured by a `*wildcard` route.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathTail {
    value: String,
    segments: Vec<String>,
}

impl PathTail {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let segments = if value.is_empty() {
            Vec::new()
        } else {
            value.split('/').map(str::to_string).collect()
        };
        Self { value, segments }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn into_inner(self) -> String {
        self.value
    }

    fn from_segments(segments: Vec<String>) -> Self {
        let value = segments.join("/");
        Self { value, segments }
    }
}

impl Default for PathTail {
    fn default() -> Self {
        Self::new("")
    }
}

impl AsRef<str> for PathTail {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for PathTail {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Display for PathTail {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

impl From<String> for PathTail {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for PathTail {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// A validated, local navigation path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoutePath(String);

impl RoutePath {
    pub fn new(path: impl AsRef<str>) -> Result<Self, PathError> {
        let normalized = normalize_path(path.as_ref())?;
        for segment in raw_path_segments(&normalized)? {
            percent_decode_segment(segment.raw)?;
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for RoutePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for RoutePath {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Display for RoutePath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RoutePath {
    type Error = PathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for RoutePath {
    type Error = PathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A checked builder used by generated route enums to encode path segments.
#[derive(Default)]
pub struct RoutePathBuilder {
    path: String,
}

impl RoutePathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_static(&mut self, segment: &str) -> Result<(), PathParamError> {
        if segment.is_empty() || segment.contains(['/', '?', '#']) {
            return Err(PathParamError::recoverable(
                PathParamErrorKind::InvalidValue(
                    "static route segments must not be empty or contain separators".to_string(),
                ),
            ));
        }
        percent_decode_segment(segment).map_err(PathParamError::from)?;
        self.path.push('/');
        self.path.push_str(segment);
        Ok(())
    }

    pub fn push_param<T>(&mut self, value: &T) -> Result<(), PathParamError>
    where
        T: PathParam,
    {
        let encoded = value.encode_segment().map_err(|error| {
            PathParamError::recoverable(PathParamErrorKind::InvalidValue(error.to_string()))
        })?;
        self.path.push('/');
        self.path.push_str(&encoded);
        Ok(())
    }

    pub fn finish(self) -> Result<RoutePath, PathParamError> {
        let path = if self.path.is_empty() {
            "/"
        } else {
            self.path.as_str()
        };
        RoutePath::new(path).map_err(PathParamError::from)
    }
}

/// Joins a static route prefix and a validated relative route path.
pub fn join_route_paths(prefix: &str, suffix: &str) -> Result<RoutePath, PathError> {
    let prefix = normalize_path(prefix)?;
    let suffix = normalize_path(suffix)?;
    let path = if prefix == "/" {
        suffix
    } else if suffix == "/" {
        prefix
    } else {
        format!("{prefix}{suffix}")
    };
    RoutePath::new(path)
}

/// Normalizes a pathname without decoding its segments.
///
/// The final slash is optional, while empty intermediate segments are rejected
/// instead of being silently removed. Query strings and fragments are not part
/// of a route path.
pub fn normalize_path(path: &str) -> Result<String, PathError> {
    if path.is_empty() {
        return Ok(String::from("/"));
    }

    if path
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'?' | b'#'))
    {
        return Err(PathError::recoverable(PathErrorKind::InvalidPath(
            "query strings and fragments are not route paths".to_string(),
        )));
    }

    if !path.starts_with('/') {
        return Err(PathError::recoverable(PathErrorKind::InvalidPath(
            "route paths must start with '/'".to_string(),
        )));
    }

    if path == "/" {
        return Ok(String::from("/"));
    }

    if path.contains("//") {
        return Err(PathError::recoverable(PathErrorKind::InvalidPath(
            "empty path segments are not allowed".to_string(),
        )));
    }

    let normalized = path.strip_suffix('/').unwrap_or(path);
    if normalized.is_empty() {
        Ok(String::from("/"))
    } else {
        Ok(normalized.to_string())
    }
}

/// Decodes one raw URL path segment using percent encoding.
pub fn percent_decode_segment(value: &str) -> Result<String, PathError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(PathError::recoverable(
                    PathErrorKind::InvalidPercentEncoding,
                ));
            }
            let high = decode_hex(bytes[index + 1])?;
            let low = decode_hex(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).map_err(|_| PathError::recoverable(PathErrorKind::InvalidUtf8))
}

/// Encodes one decoded value for use as a URL path segment.
pub fn percent_encode_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());

    for byte in value.as_bytes() {
        if is_unreserved(*byte) {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    encoded
}

fn decode_hex(byte: u8) -> Result<u8, PathError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(PathError::recoverable(
            PathErrorKind::InvalidPercentEncoding,
        )),
    }
}

fn is_unreserved(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
}

fn encode_path_tail(value: &PathTail) -> String {
    value
        .segments
        .iter()
        .map(|segment| percent_encode_segment(segment))
        .collect::<Vec<_>>()
        .join("/")
}

fn decode_path_tail_value(value: &str) -> Result<PathTail, PathParamError> {
    if value.is_empty() {
        return Ok(PathTail::default());
    }

    value
        .split('/')
        .map(|segment| percent_decode_segment(segment).map_err(PathParamError::from))
        .collect::<Result<Vec<_>, _>>()
        .map(PathTail::from_segments)
}

impl PathParam for String {
    type Error = PathParamError;

    fn decode_segment(value: &str) -> Result<Self, Self::Error> {
        percent_decode_segment(value).map_err(PathParamError::from)
    }

    fn encode_segment(&self) -> Result<String, Self::Error> {
        Ok(percent_encode_segment(self))
    }
}

impl PathParam for PathTail {
    type Error = PathParamError;

    fn decode_segment(value: &str) -> Result<Self, Self::Error> {
        decode_path_tail_value(value)
    }

    fn encode_segment(&self) -> Result<String, Self::Error> {
        Ok(encode_path_tail(self))
    }
}

macro_rules! impl_scalar_path_param {
    ($($type:ty),* $(,)?) => {
        $(
            impl PathParam for $type {
                type Error = PathParamError;

                fn decode_segment(value: &str) -> Result<Self, Self::Error> {
                    let decoded = percent_decode_segment(value).map_err(PathParamError::from)?;
                    decoded.parse::<$type>().map_err(|error| {
                        PathParamError::recoverable(PathParamErrorKind::InvalidValue(
                            error.to_string(),
                        ))
                    })
                }

                fn encode_segment(&self) -> Result<String, Self::Error> {
                    Ok(percent_encode_segment(&self.to_string()))
                }
            }
        )*
    };
}

impl_scalar_path_param!(
    bool, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,
);

impl PathParam for char {
    type Error = PathParamError;

    fn decode_segment(value: &str) -> Result<Self, Self::Error> {
        let decoded = percent_decode_segment(value).map_err(PathParamError::from)?;
        let mut chars = decoded.chars();
        let character = chars.next().ok_or_else(|| {
            PathParamError::recoverable(PathParamErrorKind::InvalidValue(
                "expected one character".to_string(),
            ))
        })?;
        if chars.next().is_some() {
            return Err(PathParamError::recoverable(
                PathParamErrorKind::InvalidValue("expected one character".to_string()),
            ));
        }
        Ok(character)
    }

    fn encode_segment(&self) -> Result<String, Self::Error> {
        Ok(percent_encode_segment(&self.to_string()))
    }
}

pub(crate) struct RawPathSegment<'path> {
    pub(crate) raw: &'path str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn raw_path_segments(path: &str) -> Result<Vec<RawPathSegment<'_>>, PathError> {
    normalize_path(path)?;
    if path.is_empty() || path == "/" {
        return Ok(Vec::new());
    }

    let end = if path.ends_with('/') {
        path.len() - 1
    } else {
        path.len()
    };
    if end <= 1 {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let mut start = 1;
    for raw in path[1..end].split('/') {
        if raw.is_empty() {
            return Err(PathError::recoverable(PathErrorKind::InvalidPath(
                "empty path segments are not allowed".to_string(),
            )));
        }
        let segment_end = start + raw.len();
        segments.push(RawPathSegment {
            raw,
            start,
            end: segment_end,
        });
        start = segment_end + 1;
    }
    Ok(segments)
}

/// Removes a static route prefix after comparing decoded URL segments.
pub fn strip_route_prefix(prefix: &str, path: &str) -> Option<String> {
    let prefix_segments = raw_path_segments(prefix).ok()?;
    let path_segments = raw_path_segments(path).ok()?;

    if prefix_segments.len() > path_segments.len() {
        return None;
    }

    for (prefix_segment, path_segment) in prefix_segments.iter().zip(&path_segments) {
        let prefix_value = percent_decode_segment(prefix_segment.raw).ok()?;
        let path_value = percent_decode_segment(path_segment.raw).ok()?;
        if prefix_value != path_value {
            return None;
        }
    }

    let remaining = &path_segments[prefix_segments.len()..];
    if remaining.is_empty() {
        return Some(String::from("/"));
    }

    Some(format!(
        "/{}",
        &path[remaining[0].start..remaining.last()?.end]
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        PathParam, PathParamError, PathParamErrorKind, PathTail, RoutePath, RoutePathBuilder,
        join_route_paths, normalize_path, percent_decode_segment, percent_encode_segment,
        strip_route_prefix,
    };

    #[test]
    fn percent_codec_round_trips_reserved_values_and_unicode() {
        let value = "a/b?c#d% e\u{4e2d}";
        let encoded = percent_encode_segment(value);
        assert_eq!(encoded, "a%2Fb%3Fc%23d%25%20e%E4%B8%AD");
        assert_eq!(percent_decode_segment(&encoded).unwrap(), value);
    }

    #[test]
    fn percent_decode_rejects_malformed_sequences() {
        assert!(percent_decode_segment("%").is_err());
        assert!(percent_decode_segment("%0").is_err());
        assert!(percent_decode_segment("%GG").is_err());
        assert!(percent_decode_segment("%FF").is_err());
    }

    #[test]
    fn path_param_decodes_before_scalar_conversion() {
        assert_eq!(u32::decode_segment("42").unwrap(), 42);
        assert!(bool::decode_segment("true").unwrap());
        assert_eq!(String::decode_segment("a%2Fb").unwrap(), "a/b");
        assert!(matches!(
            u32::decode_segment("not-a-number"),
            Err(PathParamError::Recoverable(
                PathParamErrorKind::InvalidValue(_)
            ))
        ));
    }

    #[test]
    fn path_tail_preserves_segment_boundaries() {
        let tail = PathTail::decode_segment("a%2Fb/c").unwrap();
        assert_eq!(tail.as_str(), "a/b/c");
        assert_eq!(tail.encode_segment().unwrap(), "a%2Fb/c");
        assert_eq!(PathTail::decode_segment("").unwrap().as_str(), "");
    }

    #[test]
    fn normalize_path_only_removes_one_optional_trailing_slash() {
        assert_eq!(normalize_path("").unwrap(), "/");
        assert_eq!(normalize_path("/").unwrap(), "/");
        assert_eq!(normalize_path("/users/").unwrap(), "/users");
        assert!(normalize_path("/users//").is_err());
        assert!(normalize_path("/users//42").is_err());
        assert!(normalize_path("/users?tab=all").is_err());
        assert!(normalize_path("users").is_err());
    }

    #[test]
    fn route_path_validates_local_encoded_paths() {
        assert_eq!(RoutePath::new("/users/").unwrap().as_str(), "/users");
        assert_eq!(RoutePath::new("").unwrap().as_str(), "/");
        assert!(RoutePath::new("https://example.test").is_err());
        assert!(RoutePath::new("/users/%GG").is_err());
        assert!(RoutePath::new("/users?tab=all").is_err());
    }

    #[test]
    fn nested_prefix_stripping_respects_segment_boundaries_and_encoding() {
        assert_eq!(
            strip_route_prefix("/users", "/users/42"),
            Some(String::from("/42"))
        );
        assert_eq!(strip_route_prefix("/users", "/username/42"), None);
        assert_eq!(
            strip_route_prefix("/a%2Fb", "/a%2Fb/c"),
            Some(String::from("/c"))
        );
        assert_eq!(
            strip_route_prefix("/users", "/users/"),
            Some(String::from("/"))
        );
        assert_eq!(
            strip_route_prefix("/users", "/users"),
            Some(String::from("/"))
        );
        assert_eq!(
            strip_route_prefix("/", "/users/42"),
            Some(String::from("/users/42"))
        );
        assert_eq!(strip_route_prefix("/users", "/users//42"), None);
        assert_eq!(strip_route_prefix("/users", "/users?tab=all"), None);
        assert_eq!(strip_route_prefix("/a%2Fb", "/a/b/c"), None);
    }

    #[test]
    fn generated_path_builder_checks_static_and_typed_segments() {
        let mut builder = RoutePathBuilder::new();
        builder.push_static("users").unwrap();
        builder.push_param(&42_u32).unwrap();
        assert_eq!(builder.finish().unwrap().as_str(), "/users/42");

        let mut builder = RoutePathBuilder::new();
        assert!(builder.push_static("bad/segment").is_err());
    }

    #[test]
    fn route_path_join_preserves_root_and_segment_boundaries() {
        assert_eq!(join_route_paths("/app", "/").unwrap().as_str(), "/app");
        assert_eq!(
            join_route_paths("/app", "/users").unwrap().as_str(),
            "/app/users"
        );
        assert!(join_route_paths("/app", "users").is_err());
        assert!(join_route_paths("/application", "/users").is_ok());
    }
}
