use std::fmt::{Display, Formatter, Result};

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

/// 数值单位的入口净化。
///
/// `NaN` / `±inf` 在 CSS 里没有对应写法，`px(f64::NAN)` 以前会直接产出 `NaNpx`
/// 这种无效声明。调试构建下直接 panic 把问题暴露在写代码的地方，发布构建下退化
/// 为 `0`，至少产出的还是合法 CSS。
#[inline]
#[track_caller]
pub(crate) fn finite(v: f64) -> f64 {
    debug_assert!(
        v.is_finite(),
        "CSS 长度/角度必须是有限数，收到 {v}（NaN 与无穷在 CSS 里无法表示）"
    );
    if v.is_finite() { v } else { 0.0 }
}

macro_rules! impl_string_value_wrapper {
    ($t:ident) => {
        impl_string_value_wrapper!(@methods $t);
        impl ::std::fmt::Display for $t {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
    // 需要自定义 `Display` 的包裹类型（如 `Url` 要补 `url()` 外壳）
    (@no_display $t:ident) => {
        impl_string_value_wrapper!(@methods $t);
    };
    (@methods $t:ident) => {
        impl $t {
            #[inline]
            pub fn as_str(&self) -> &str {
                &self.0
            }
            #[inline]
            pub fn into_inner(self) -> String {
                self.0
            }
            #[inline]
            pub fn set_value(&mut self, val: impl Into<String>) {
                self.0 = val.into();
            }
            #[inline]
            pub fn with_value(mut self, val: impl Into<String>) -> Self {
                self.0 = val.into();
                self
            }
        }
    };
}
pub(crate) use impl_string_value_wrapper;

macro_rules! impl_unit_value_methods {
    ($t:ty) => {
        impl $t {
            #[inline]
            pub const fn value(&self) -> f64 {
                self.0
            }
            #[inline]
            pub fn set_value(&mut self, val: impl Into<f64>) {
                self.0 = $crate::types::units::finite(val.into());
            }
            #[inline]
            pub fn with_value(mut self, val: impl Into<f64>) -> Self {
                self.0 = $crate::types::units::finite(val.into());
                self
            }
            #[inline]
            pub fn map(mut self, f: impl FnOnce(f64) -> f64) -> Self {
                self.0 = $crate::types::units::finite(f(self.0));
                self
            }
            #[inline]
            pub fn update(&mut self, f: impl FnOnce(f64) -> f64) {
                self.0 = $crate::types::units::finite(f(self.0));
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Px(f64);
impl_unit_value_methods!(Px);
impl Display for Px {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}px", self.0)
    }
}

impl From<i32> for Px {
    fn from(v: i32) -> Self {
        Px(v as f64)
    }
}

impl From<f64> for Px {
    fn from(v: f64) -> Self {
        Px(v)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percent(f64);
impl_unit_value_methods!(Percent);
impl Display for Percent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}%", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba(u8, u8, u8, f64);
impl Rgba {
    #[inline]
    pub const fn red(&self) -> u8 {
        self.0
    }
    #[inline]
    pub const fn green(&self) -> u8 {
        self.1
    }
    #[inline]
    pub const fn blue(&self) -> u8 {
        self.2
    }
    #[inline]
    pub const fn alpha_val(&self) -> f64 {
        self.3
    }
    #[inline]
    pub const fn channels(&self) -> (u8, u8, u8, f64) {
        (self.0, self.1, self.2, self.3)
    }
    #[inline]
    pub fn set_red(&mut self, r: u8) {
        self.0 = r;
    }
    #[inline]
    pub fn set_green(&mut self, g: u8) {
        self.1 = g;
    }
    #[inline]
    pub fn set_blue(&mut self, b: u8) {
        self.2 = b;
    }
    #[inline]
    pub fn set_alpha(&mut self, a: impl Into<f64>) {
        self.3 = clamp_alpha(a.into());
    }
    #[inline]
    pub fn with_red(mut self, r: u8) -> Self {
        self.0 = r;
        self
    }
    #[inline]
    pub fn with_green(mut self, g: u8) -> Self {
        self.1 = g;
        self
    }
    #[inline]
    pub fn with_blue(mut self, b: u8) -> Self {
        self.2 = b;
        self
    }
    #[inline]
    pub fn with_alpha(mut self, a: impl Into<f64>) -> Self {
        self.3 = clamp_alpha(a.into());
        self
    }
    #[inline]
    pub fn alpha(mut self, alpha: impl Into<f64>) -> Self {
        self.3 = clamp_alpha(alpha.into());
        self
    }
}
impl Display for Rgba {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let Rgba(r, g, b, a) = *self;
        write!(f, "rgba({}, {}, {}, {})", r, g, b, a)
    }
}

/// CSS 关键字 `auto`。
///
/// 曾经是 `Auto(Option<()>)`，于是 `Auto::default()` 渲染成空串、产出
/// `width: ;` 这种无效声明。`auto` 就是 `auto`，没有第二种状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Auto;
impl Display for Auto {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "auto")
    }
}

pub const AUTO: Auto = Auto;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoneValue;
impl Display for NoneValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "none")
    }
}

pub const NONE: NoneValue = NoneValue;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rem(f64);
impl_unit_value_methods!(Rem);
impl Display for Rem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}rem", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Em(f64);
impl_unit_value_methods!(Em);
impl Display for Em {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}em", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vw(f64);
impl_unit_value_methods!(Vw);
impl Display for Vw {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}vw", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vh(f64);
impl_unit_value_methods!(Vh);
impl Display for Vh {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}vh", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deg(f64);
impl_unit_value_methods!(Deg);
impl Display for Deg {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}deg", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rad(f64);
impl_unit_value_methods!(Rad);
impl Display for Rad {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}rad", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn(f64);
impl_unit_value_methods!(Turn);
impl Display for Turn {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}turn", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hex(String);
impl_string_value_wrapper!(Hex);

impl Default for Hex {
    fn default() -> Self {
        Hex("#000000".to_string())
    }
}

/// 把 `#RGB` / `#RGBA` 展开成 `#RRGGBB` / `#RRGGBBAA`，其余原样返回。
fn expand_short_hex(s: &str) -> String {
    let digits = &s[1..];
    if digits.len() == 3 || digits.len() == 4 {
        let mut out = String::with_capacity(1 + digits.len() * 2);
        out.push('#');
        for c in digits.chars() {
            out.push(c);
            out.push(c);
        }
        out
    } else {
        s.to_string()
    }
}

impl Hex {
    /// 校验并归一化一个十六进制颜色。
    ///
    /// 接受 `#RGB` / `#RGBA` / `#RRGGBB` / `#RRGGBBAA`，缺失的 `#` 会补上。
    pub fn try_new(value: impl AsRef<str>) -> ::std::result::Result<Self, String> {
        let raw = value.as_ref().trim();
        let normalized = if raw.starts_with('#') {
            raw.to_ascii_lowercase()
        } else {
            format!("#{}", raw.to_ascii_lowercase())
        };
        let digits = &normalized[1..];
        let len_ok = matches!(digits.len(), 3 | 4 | 6 | 8);
        if !len_ok || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "`{raw}` 不是合法的十六进制颜色，期望 #RGB / #RGBA / #RRGGBB / #RRGGBBAA"
            ));
        }
        Ok(Hex(normalized))
    }

    /// 追加 alpha 通道，产出 `#RRGGBBAA`。
    ///
    /// 此前是直接往字符串尾巴上接两位十六进制，`hex("#fff").alpha(0.5)` 得到
    /// 五位的 `#fff7f`——不是任何合法颜色。现在先把短写法展开再接。
    pub fn alpha(self, alpha: f64) -> Hex {
        let a = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let alpha_hex = (a * 255.0).round() as u8;
        // 已经带 alpha 的先去掉原有的两位
        let base = expand_short_hex(&self.0);
        let base = if base.len() == 9 {
            &base[..7]
        } else {
            &base[..]
        };
        Hex(format!("{}{:02x}", base, alpha_hex))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl(u16, u8, u8, f64);
impl Hsl {
    #[inline]
    pub const fn hue(&self) -> u16 {
        self.0
    }
    #[inline]
    pub const fn saturation(&self) -> u8 {
        self.1
    }
    #[inline]
    pub const fn lightness(&self) -> u8 {
        self.2
    }
    #[inline]
    pub const fn alpha_val(&self) -> f64 {
        self.3
    }
    #[inline]
    pub const fn channels(&self) -> (u16, u8, u8, f64) {
        (self.0, self.1, self.2, self.3)
    }
    #[inline]
    pub fn set_hue(&mut self, h: u16) {
        self.0 = h % 360;
    }
    #[inline]
    pub fn set_saturation(&mut self, s: u8) {
        self.1 = s.min(100);
    }
    #[inline]
    pub fn set_lightness(&mut self, l: u8) {
        self.2 = l.min(100);
    }
    #[inline]
    pub fn set_alpha(&mut self, a: impl Into<f64>) {
        self.3 = clamp_alpha(a.into());
    }
    #[inline]
    pub fn with_hue(mut self, h: u16) -> Self {
        self.0 = h % 360;
        self
    }
    #[inline]
    pub fn with_saturation(mut self, s: u8) -> Self {
        self.1 = s.min(100);
        self
    }
    #[inline]
    pub fn with_lightness(mut self, l: u8) -> Self {
        self.2 = l.min(100);
        self
    }
    #[inline]
    pub fn with_alpha(mut self, a: impl Into<f64>) -> Self {
        self.3 = clamp_alpha(a.into());
        self
    }
    #[inline]
    pub fn alpha(mut self, alpha: impl Into<f64>) -> Self {
        self.3 = clamp_alpha(alpha.into());
        self
    }
}
impl Display for Hsl {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (h, s, l, a) = self.channels();
        if a >= 1.0 {
            write!(f, "hsl({}, {}%, {}%)", h, s, l)
        } else {
            write!(f, "hsla({}, {}%, {}%, {})", h, s, l, a)
        }
    }
}

/// CSS 的 `<url>`。
///
/// `Display` 会补上 `url("…")` 外壳并转义引号——此前它复用通用字符串包装器，
/// `url("a.png")` 渲染出来是裸的 `a.png`，而 `Url` 恰恰被声明为
/// `ValidFor<props::BackgroundImage>`，于是产出 `background-image: a.png`。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Url(String);
impl_string_value_wrapper!(@no_display Url);

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "url(\"")?;
        for ch in self.0.chars() {
            match ch {
                '"' => write!(f, "\\\"")?,
                '\\' => write!(f, "\\\\")?,
                '\n' => write!(f, "\\A ")?,
                '\r' => write!(f, "\\D ")?,
                c => write!(f, "{c}")?,
            }
        }
        write!(f, "\")")
    }
}

#[inline(always)]
#[track_caller]
pub fn px<T: Into<f64>>(v: T) -> Px {
    Px(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn pct<T: Into<f64>>(v: T) -> Percent {
    Percent(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn rem<T: Into<f64>>(v: T) -> Rem {
    Rem(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn em_unit<T: Into<f64>>(v: T) -> Em {
    Em(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn vw<T: Into<f64>>(v: T) -> Vw {
    Vw(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn vh<T: Into<f64>>(v: T) -> Vh {
    Vh(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn deg<T: Into<f64>>(v: T) -> Deg {
    Deg(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn rad<T: Into<f64>>(v: T) -> Rad {
    Rad(finite(v.into()))
}
#[inline(always)]
#[track_caller]
pub fn turn<T: Into<f64>>(v: T) -> Turn {
    Turn(finite(v.into()))
}
#[inline(always)]
pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba(r, g, b, 1.0)
}
#[inline(always)]
pub fn rgba<T: Into<f64>>(r: u8, g: u8, b: u8, a: T) -> Rgba {
    Rgba(r, g, b, clamp_alpha(a.into()))
}
/// 构造一个十六进制颜色。
///
/// 输入必须是 `#RGB` / `#RGBA` / `#RRGGBB` / `#RRGGBBAA`（`#` 可省略）。
/// 拼错的颜色以前会一路静默流到 CSS 里（`hex("not a color")` → `not a color`），
/// 这里直接 panic 把它挡在写代码的地方；需要处理不可信输入时用
/// [`Hex::try_new`]。
#[inline]
#[track_caller]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    let raw = v.into();
    match Hex::try_new(&raw) {
        Ok(h) => h,
        Err(e) => panic!("{e}"),
    }
}
#[inline(always)]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(h % 360, s.min(100), l.min(100), 1.0)
}
#[inline(always)]
pub fn hsla<A: Into<f64>>(h: u16, s: u8, l: u8, a: A) -> Hsl {
    Hsl(h % 360, s.min(100), l.min(100), clamp_alpha(a.into()))
}
#[inline(always)]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
}

/// alpha 通道必须落在 `[0, 1]`，非有限值按不透明处理。
#[inline]
pub(crate) fn clamp_alpha(a: f64) -> f64 {
    if a.is_finite() {
        a.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_renders_a_url_function() {
        assert_eq!(url("a.png").to_string(), "url(\"a.png\")");
        // 引号必须转义，否则可以从 url() 里逃出来
        assert_eq!(url("a\").x(\"b").to_string(), "url(\"a\\\").x(\\\"b\")");
    }

    #[test]
    fn auto_always_renders_the_keyword() {
        assert_eq!(Auto.to_string(), "auto");
        assert_eq!(AUTO.to_string(), "auto");
    }

    #[test]
    fn hex_alpha_expands_short_form() {
        // 曾经产出五位的 `#fff7f`
        assert_eq!(hex("#fff").alpha(0.5).to_string(), "#ffffff80");
        assert_eq!(hex("#ff0000").alpha(1.0).to_string(), "#ff0000ff");
        // 重复设置 alpha 不会越接越长
        assert_eq!(
            hex("#ff0000").alpha(0.5).alpha(1.0).to_string(),
            "#ff0000ff"
        );
    }

    #[test]
    fn hex_normalizes_and_rejects_garbage() {
        assert_eq!(hex("FFF").to_string(), "#fff");
        assert!(Hex::try_new("not a color").is_err());
        assert!(Hex::try_new("#12345").is_err());
    }

    #[test]
    #[should_panic(expected = "不是合法的十六进制颜色")]
    fn hex_panics_on_invalid_input() {
        let _ = hex("not a color");
    }

    #[test]
    fn hsl_components_are_clamped() {
        assert_eq!(hsl(900, 200, 200).to_string(), "hsl(180, 100%, 100%)");
        assert_eq!(hsla(0, 0, 0, 5.0).to_string(), "hsl(0, 0%, 0%)");
        assert_eq!(hsla(0, 0, 0, 0.5).to_string(), "hsla(0, 0%, 0%, 0.5)");
    }

    #[test]
    #[should_panic(expected = "必须是有限数")]
    fn non_finite_lengths_are_rejected() {
        let _ = px(f64::NAN);
    }
}
