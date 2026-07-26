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
        "CSS 长度/角度/时间必须是有限数，收到 {v}（NaN 与无穷在 CSS 里无法表示）"
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
    // 只读访问。这里曾经还有 `set_value` / `with_value`：
    // `hex("#fff").with_value("javascript:alert(1)")` 能把工厂函数刚建立起来
    // 的不变量一行抹掉——`Hex` 的十六进制校验、`grid_template_areas` 的引号
    // 转义、`border()` 的三段式结构，全都形同虚设。要改值就重新构造一个。
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
        }
    };
}
pub(crate) use impl_string_value_wrapper;

/// 定义一个「数值 + 单位后缀」的量纲类型。
///
/// 每个单位都是独立的 newtype——这样 `Px` 与 `Deg` 不会互相赋值，也不会在
/// `calc()` 里被混着运算。展开出来的东西对每个单位都一样：读写访问器、
/// `Display`、`From<i32>` / `From<f64>`，以及一个工厂函数。
macro_rules! define_dimension {
    ($(#[$meta:meta])* $t:ident, $suffix:literal, $ctor:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
        pub struct $t(f64);

        impl $t {
            #[inline]
            pub const fn value(&self) -> f64 {
                self.0
            }
            /// 该单位在 CSS 里的后缀（`Px` → `"px"`）。
            pub const SUFFIX: &'static str = $suffix;
            #[inline]
            #[track_caller]
            pub fn set_value(&mut self, val: impl Into<f64>) {
                self.0 = finite(val.into());
            }
            #[inline]
            #[track_caller]
            pub fn with_value(mut self, val: impl Into<f64>) -> Self {
                self.0 = finite(val.into());
                self
            }
            #[inline]
            #[track_caller]
            pub fn map(mut self, f: impl FnOnce(f64) -> f64) -> Self {
                self.0 = finite(f(self.0));
                self
            }
            #[inline]
            #[track_caller]
            pub fn update(&mut self, f: impl FnOnce(f64) -> f64) {
                self.0 = finite(f(self.0));
            }
        }

        impl Display for $t {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                write!(f, "{}{}", self.0, $suffix)
            }
        }

        impl From<i32> for $t {
            #[track_caller]
            fn from(v: i32) -> Self {
                $t(v as f64)
            }
        }

        impl From<f64> for $t {
            #[track_caller]
            fn from(v: f64) -> Self {
                $t(finite(v))
            }
        }

        #[inline]
        #[track_caller]
        pub fn $ctor<T: Into<f64>>(v: T) -> $t {
            $t(finite(v.into()))
        }
    };
}

// ------------------------------------------
// 长度
// ------------------------------------------

define_dimension!(
    /// 绝对长度：像素。
    Px, "px", px
);
define_dimension!(
    /// 相对于根元素字号。
    Rem, "rem", rem
);
define_dimension!(
    /// 相对于当前元素字号。
    Em, "em", em_unit
);
define_dimension!(
    /// 当前字体 `0` 字形的宽度。
    Ch, "ch", ch
);
define_dimension!(
    /// 当前字体的 x-height。
    Ex, "ex", ex
);
define_dimension!(
    /// 视口宽度的 1%。
    Vw, "vw", vw
);
define_dimension!(
    /// 视口高度的 1%。
    Vh, "vh", vh
);
define_dimension!(
    /// 视口较短边的 1%。
    Vmin, "vmin", vmin
);
define_dimension!(
    /// 视口较长边的 1%。
    Vmax, "vmax", vmax
);
define_dimension!(
    /// 动态视口宽度的 1%（移动端浏览器地址栏收起/展开时会变）。
    Dvw, "dvw", dvw
);
define_dimension!(
    /// 动态视口高度的 1%。
    Dvh, "dvh", dvh
);
define_dimension!(
    /// 最小视口宽度的 1%（地址栏完全展开时）。
    Svw, "svw", svw
);
define_dimension!(
    /// 最小视口高度的 1%。
    Svh, "svh", svh
);
define_dimension!(
    /// 最大视口宽度的 1%（地址栏完全收起时）。
    Lvw, "lvw", lvw
);
define_dimension!(
    /// 最大视口高度的 1%。
    Lvh, "lvh", lvh
);
define_dimension!(
    /// 点，1pt = 1/72in。
    Pt, "pt", pt
);
define_dimension!(
    /// 派卡，1pc = 12pt。
    Pc, "pc", pc
);
define_dimension!(
    /// 厘米。
    Cm, "cm", cm
);
define_dimension!(
    /// 毫米。
    Mm, "mm", mm
);
define_dimension!(
    /// 英寸。工厂函数叫 `inch`，因为 `in` 是 Rust 关键字。
    In, "in", inch
);
define_dimension!(
    /// 四分之一毫米（CSS 里写作 `Q`）。
    ///
    /// Rust 侧叫 `Qmm` / `qmm` 而不是 `Q` / `q`：`silex` 的 prelude 会同时导出
    /// CSS 单位和 HTML 标签，而 `<q>` 是个标签。
    Qmm, "Q", qmm
);

define_dimension!(
    /// 百分比。
    ///
    /// 它有独立的能力位：接受 `<length>` 的属性未必接受 `<percentage>`
    /// （`border-width` 就不接受），反过来也一样。
    Percent, "%", pct
);

define_dimension!(
    /// 网格轨道的弹性系数（`<flex>`）。
    ///
    /// `fr` 只在网格轨道尺寸里合法，不能与长度混着算，所以它不属于长度量纲。
    Fr, "fr", fr
);

// ------------------------------------------
// 角度
// ------------------------------------------

define_dimension!(
    /// 角度（度）。
    Deg, "deg", deg
);
define_dimension!(
    /// 角度（弧度）。
    Rad, "rad", rad
);
define_dimension!(
    /// 角度（圈），1turn = 360deg。
    Turn, "turn", turn
);

// ------------------------------------------
// 时间
// ------------------------------------------

define_dimension!(
    /// 时间（秒）。
    Sec, "s", sec
);
define_dimension!(
    /// 时间（毫秒）。
    Ms, "ms", ms
);

/// 所有长度单位（**不含** `Percent`——它有独立的能力位）。
///
/// 这份清单是长度量纲的唯一事实来源：`CssLength`、算术运算符、`calc()` 操作数、
/// `ValidFor<接受 <length> 的属性>`、响应式登记全都由它展开。加一个新单位只要
/// 在这里补一行。
macro_rules! for_all_length_units {
    ($cb:ident $(, $($pre:tt)*)?) => {
        $cb!(
            $($($pre)*,)?
            Px, Rem, Em, Ch, Ex, Vw, Vh, Vmin, Vmax, Dvw, Dvh, Svw, Svh, Lvw, Lvh,
            Pt, Pc, Cm, Mm, In, Qmm
        );
    };
}

/// 所有角度单位。
macro_rules! for_all_angle_units {
    ($cb:ident $(, $($pre:tt)*)?) => {
        $cb!($($($pre)*,)? Deg, Rad, Turn);
    };
}

/// 所有时间单位。
macro_rules! for_all_time_units {
    ($cb:ident $(, $($pre:tt)*)?) => {
        $cb!($($($pre)*,)? Sec, Ms);
    };
}

pub(crate) use {for_all_angle_units, for_all_length_units, for_all_time_units};

// ==========================================
// 关键字
// ==========================================

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

/// CSS 宽关键字（CSS-wide keywords）。
///
/// 规范规定这五个词对**任何**属性都合法，但此前 361 个关键字枚举里 `inherit`
/// 只出现过 2 次，也没有任何一个类型对所有属性有效。结果是 12 个真正做了颜色
/// 约束的属性反而连 `color: inherit` 都写不出来——`sty().color("inherit")`
/// 编译失败，只能退到 `css_unsafe("inherit")`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CssWide {
    #[default]
    Inherit,
    Initial,
    Unset,
    Revert,
    RevertLayer,
}

impl Display for CssWide {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(match self {
            Self::Inherit => "inherit",
            Self::Initial => "initial",
            Self::Unset => "unset",
            Self::Revert => "revert",
            Self::RevertLayer => "revert-layer",
        })
    }
}

pub const INHERIT: CssWide = CssWide::Inherit;
pub const INITIAL: CssWide = CssWide::Initial;
pub const UNSET: CssWide = CssWide::Unset;
pub const REVERT: CssWide = CssWide::Revert;
pub const REVERT_LAYER: CssWide = CssWide::RevertLayer;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoneValue;
impl Display for NoneValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "none")
    }
}

pub const NONE: NoneValue = NoneValue;

// ==========================================
// 颜色
// ==========================================

/// alpha 通道必须落在 `[0, 1]`，非有限值按不透明处理。
#[inline]
pub(crate) fn clamp_alpha(a: f64) -> f64 {
    if a.is_finite() {
        a.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// alpha 是否等于「完全不透明」。
///
/// `Rgba` 与 `Hsl` 共用这一条判据，两者的 `Display` 才不会一个恒定输出
/// `rgba(…)`、另一个按 alpha 切换。
#[inline]
pub(crate) fn is_opaque(a: f64) -> bool {
    a >= 1.0
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

/// 不透明时输出 `rgb(…)`，带透明度时输出 `rgba(…)`。
///
/// 此前它恒定输出 `rgba(…)`（`rgb(1,2,3)` → `rgba(1, 2, 3, 1)`），而同一个
/// crate 里的 `Hsl` 是按 alpha 切换的。同一份语义在两个颜色类型上写出两种
/// 文本，静态哈希的稳定性判断也跟着分叉。
impl Display for Rgba {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let Rgba(r, g, b, a) = *self;
        if is_opaque(a) {
            write!(f, "rgb({}, {}, {})", r, g, b)
        } else {
            write!(f, "rgba({}, {}, {}, {})", r, g, b, a)
        }
    }
}

/// HSL 颜色。
///
/// 分量是 `f64`：此前是 `Hsl(u16, u8, u8, f64)`，`hsl(210.5, 12.5, 40)` 这种
/// 小数分量根本表达不了。色相按 360 取模（保留小数），饱和度与亮度 clamp 到
/// `[0, 100]`。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl(f64, f64, f64, f64);

#[inline]
fn wrap_hue(h: f64) -> f64 {
    if !h.is_finite() {
        return 0.0;
    }
    let h = h % 360.0;
    if h < 0.0 { h + 360.0 } else { h }
}

#[inline]
fn clamp_pct(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

impl Hsl {
    #[inline]
    pub const fn hue(&self) -> f64 {
        self.0
    }
    #[inline]
    pub const fn saturation(&self) -> f64 {
        self.1
    }
    #[inline]
    pub const fn lightness(&self) -> f64 {
        self.2
    }
    #[inline]
    pub const fn alpha_val(&self) -> f64 {
        self.3
    }
    #[inline]
    pub const fn channels(&self) -> (f64, f64, f64, f64) {
        (self.0, self.1, self.2, self.3)
    }
    #[inline]
    pub fn set_hue(&mut self, h: impl Into<f64>) {
        self.0 = wrap_hue(h.into());
    }
    #[inline]
    pub fn set_saturation(&mut self, s: impl Into<f64>) {
        self.1 = clamp_pct(s.into());
    }
    #[inline]
    pub fn set_lightness(&mut self, l: impl Into<f64>) {
        self.2 = clamp_pct(l.into());
    }
    #[inline]
    pub fn set_alpha(&mut self, a: impl Into<f64>) {
        self.3 = clamp_alpha(a.into());
    }
    #[inline]
    pub fn with_hue(mut self, h: impl Into<f64>) -> Self {
        self.0 = wrap_hue(h.into());
        self
    }
    #[inline]
    pub fn with_saturation(mut self, s: impl Into<f64>) -> Self {
        self.1 = clamp_pct(s.into());
        self
    }
    #[inline]
    pub fn with_lightness(mut self, l: impl Into<f64>) -> Self {
        self.2 = clamp_pct(l.into());
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
        if is_opaque(a) {
            write!(f, "hsl({}, {}%, {}%)", h, s, l)
        } else {
            write!(f, "hsla({}, {}%, {}%, {})", h, s, l, a)
        }
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
        let a = clamp_alpha(alpha);
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

/// 现代颜色函数的一个分量。
///
/// `oklch()` / `lab()` / `hwb()` 的分量既可以是数值也可以是百分比，还可以是
/// `none`（表示「该通道缺省」，在插值与相对颜色语法里有意义）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorComponent {
    Number(f64),
    Percentage(f64),
    /// CSS Color 4 的 `none`
    Missing,
}

impl Display for ColorComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Number(v) => write!(f, "{}", v),
            Self::Percentage(v) => write!(f, "{}%", v),
            Self::Missing => f.write_str("none"),
        }
    }
}

impl From<f64> for ColorComponent {
    fn from(v: f64) -> Self {
        Self::Number(finite(v))
    }
}
impl From<f32> for ColorComponent {
    fn from(v: f32) -> Self {
        Self::Number(finite(v as f64))
    }
}
impl From<i32> for ColorComponent {
    fn from(v: i32) -> Self {
        Self::Number(v as f64)
    }
}
impl From<Percent> for ColorComponent {
    fn from(v: Percent) -> Self {
        Self::Percentage(v.value())
    }
}
impl From<Deg> for ColorComponent {
    fn from(v: Deg) -> Self {
        Self::Number(v.value())
    }
}
impl From<NoneValue> for ColorComponent {
    fn from(_: NoneValue) -> Self {
        Self::Missing
    }
}

/// 一个已经拼好的现代颜色函数（`oklch(…)`、`color-mix(…)` 等）。
///
/// 这些语法在 Rust 侧没必要各建一个类型：它们的共同点是「渲染出来就是一个
/// 完整的 `<color>`」，能用在任何接受颜色的地方，也能作为 `color-mix()` 的
/// 操作数继续嵌套。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorFn(String);
impl_string_value_wrapper!(ColorFn);

impl Default for ColorFn {
    fn default() -> Self {
        // 空串会渲染成 `color: ;`——无效声明。`transparent` 合法且语义中性。
        ColorFn("transparent".to_string())
    }
}

fn color_fn(name: &str, args: &[ColorComponent], alpha: Option<f64>) -> ColorFn {
    let mut s = String::with_capacity(name.len() + args.len() * 6 + 8);
    s.push_str(name);
    s.push('(');
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&a.to_string());
    }
    if let Some(a) = alpha {
        s.push_str(" / ");
        s.push_str(&clamp_alpha(a).to_string());
    }
    s.push(')');
    ColorFn(s)
}

macro_rules! define_color_fn {
    ($(#[$meta:meta])* $name:ident, $with_alpha:ident, $css:literal) => {
        $(#[$meta])*
        pub fn $name(
            a: impl Into<ColorComponent>,
            b: impl Into<ColorComponent>,
            c: impl Into<ColorComponent>,
        ) -> ColorFn {
            color_fn($css, &[a.into(), b.into(), c.into()], None)
        }

        $(#[$meta])*
        ///
        /// 带 alpha 的形式，渲染成 `f(a b c / alpha)`。
        pub fn $with_alpha(
            a: impl Into<ColorComponent>,
            b: impl Into<ColorComponent>,
            c: impl Into<ColorComponent>,
            alpha: impl Into<f64>,
        ) -> ColorFn {
            color_fn($css, &[a.into(), b.into(), c.into()], Some(alpha.into()))
        }
    };
}

define_color_fn!(
    /// `oklch(L C H)`——感知均匀的柱坐标色空间，做主题色阶最好用的一个。
    oklch, oklcha, "oklch"
);
define_color_fn!(
    /// `oklab(L a b)`
    oklab, oklaba, "oklab"
);
define_color_fn!(
    /// `lch(L C H)`
    lch, lcha, "lch"
);
define_color_fn!(
    /// `lab(L a b)`
    lab, laba, "lab"
);
define_color_fn!(
    /// `hwb(H W B)`——色相 + 白度 + 黑度。
    hwb, hwba, "hwb"
);

/// `color-mix()` 的插值色空间。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorSpace {
    #[default]
    Srgb,
    SrgbLinear,
    Lab,
    Oklab,
    Lch,
    Oklch,
    Hsl,
    Hwb,
    DisplayP3,
    XyzD65,
}

impl Display for ColorSpace {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(match self {
            Self::Srgb => "srgb",
            Self::SrgbLinear => "srgb-linear",
            Self::Lab => "lab",
            Self::Oklab => "oklab",
            Self::Lch => "lch",
            Self::Oklch => "oklch",
            Self::Hsl => "hsl",
            Self::Hwb => "hwb",
            Self::DisplayP3 => "display-p3",
            Self::XyzD65 => "xyz-d65",
        })
    }
}

/// `color-mix(in <space>, <a>, <b>)`——两色等量混合。
pub fn color_mix<A: Display, B: Display>(space: ColorSpace, a: A, b: B) -> ColorFn {
    ColorFn(format!("color-mix(in {}, {}, {})", space, a, b))
}

/// `color-mix(in <space>, <a> <pa>, <b> <pb>)`——按权重混合。
pub fn color_mix_weighted<A: Display, B: Display>(
    space: ColorSpace,
    a: A,
    a_weight: Percent,
    b: B,
    b_weight: Percent,
) -> ColorFn {
    ColorFn(format!(
        "color-mix(in {}, {} {}, {} {})",
        space, a, a_weight, b, b_weight
    ))
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

// ==========================================
// 工厂函数（单位的工厂由 define_dimension! 生成）
// ==========================================

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
pub fn hsl<H: Into<f64>, S: Into<f64>, L: Into<f64>>(h: H, s: S, l: L) -> Hsl {
    Hsl(
        wrap_hue(h.into()),
        clamp_pct(s.into()),
        clamp_pct(l.into()),
        1.0,
    )
}
#[inline(always)]
pub fn hsla<H: Into<f64>, S: Into<f64>, L: Into<f64>, A: Into<f64>>(h: H, s: S, l: L, a: A) -> Hsl {
    Hsl(
        wrap_hue(h.into()),
        clamp_pct(s.into()),
        clamp_pct(l.into()),
        clamp_alpha(a.into()),
    )
}
#[inline(always)]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
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
        // 负色相绕回正区间，而不是留下 `hsl(-30, …)`
        assert_eq!(hsl(-30, 50, 50).to_string(), "hsl(330, 50%, 50%)");
    }

    /// 报告 P3-8：`Hsl(u16, u8, u8, f64)` 表达不了小数分量
    #[test]
    fn hsl_keeps_fractional_components() {
        assert_eq!(
            hsl(210.5, 12.5, 40.25).to_string(),
            "hsl(210.5, 12.5%, 40.25%)"
        );
    }

    /// 报告 P3-3：`Rgba` 恒定输出 `rgba(…)`，`Hsl` 却按 alpha 切换。
    /// 同一条规则必须适用于两者，否则静态哈希的稳定性判断也跟着分叉。
    #[test]
    fn rgba_and_hsl_switch_on_alpha_the_same_way() {
        assert_eq!(rgb(1, 2, 3).to_string(), "rgb(1, 2, 3)");
        assert_eq!(rgba(1, 2, 3, 1.0).to_string(), "rgb(1, 2, 3)");
        assert_eq!(rgba(1, 2, 3, 0.5).to_string(), "rgba(1, 2, 3, 0.5)");

        assert_eq!(hsl(200, 50, 50).to_string(), "hsl(200, 50%, 50%)");
        assert_eq!(
            hsla(200, 50, 50, 0.5).to_string(),
            "hsla(200, 50%, 50%, 0.5)"
        );
    }

    #[test]
    #[should_panic(expected = "必须是有限数")]
    fn non_finite_lengths_are_rejected() {
        let _ = px(f64::NAN);
    }

    /// 报告 P3-8 的单位缺口：现代视口单位、排版单位与绝对单位
    #[test]
    fn every_unit_renders_its_own_suffix() {
        assert_eq!(px(1).to_string(), "1px");
        assert_eq!(pct(50).to_string(), "50%");
        assert_eq!(rem(1.5).to_string(), "1.5rem");
        assert_eq!(em_unit(2).to_string(), "2em");
        assert_eq!(ch(3).to_string(), "3ch");
        assert_eq!(ex(4).to_string(), "4ex");
        assert_eq!(vw(10).to_string(), "10vw");
        assert_eq!(vh(10).to_string(), "10vh");
        assert_eq!(vmin(10).to_string(), "10vmin");
        assert_eq!(vmax(10).to_string(), "10vmax");
        assert_eq!(dvw(10).to_string(), "10dvw");
        assert_eq!(dvh(10).to_string(), "10dvh");
        assert_eq!(svw(10).to_string(), "10svw");
        assert_eq!(svh(10).to_string(), "10svh");
        assert_eq!(lvw(10).to_string(), "10lvw");
        assert_eq!(lvh(10).to_string(), "10lvh");
        assert_eq!(pt(12).to_string(), "12pt");
        assert_eq!(pc(1).to_string(), "1pc");
        assert_eq!(cm(2).to_string(), "2cm");
        assert_eq!(mm(5).to_string(), "5mm");
        assert_eq!(inch(1).to_string(), "1in");
        assert_eq!(qmm(4).to_string(), "4Q");
        assert_eq!(fr(1).to_string(), "1fr");
    }

    /// 报告 P3-8：没有时间单位，`transition_duration` 只能吃字符串
    #[test]
    fn time_units_render_themselves() {
        assert_eq!(sec(0.3).to_string(), "0.3s");
        assert_eq!(ms(300).to_string(), "300ms");
    }

    #[test]
    fn angle_units_render_themselves() {
        assert_eq!(deg(90).to_string(), "90deg");
        assert_eq!(rad(1.5).to_string(), "1.5rad");
        assert_eq!(turn(0.25).to_string(), "0.25turn");
    }

    /// 报告 P3-8：无 oklch / oklab / lab / lch / hwb / color-mix
    #[test]
    fn modern_color_functions_render_css_color_4_syntax() {
        assert_eq!(oklch(0.7, 0.15, 250).to_string(), "oklch(0.7 0.15 250)");
        assert_eq!(
            oklcha(0.7, 0.15, 250, 0.5).to_string(),
            "oklch(0.7 0.15 250 / 0.5)"
        );
        assert_eq!(oklab(0.5, -0.1, 0.1).to_string(), "oklab(0.5 -0.1 0.1)");
        assert_eq!(lch(50, 40, 120).to_string(), "lch(50 40 120)");
        assert_eq!(lab(50, 40, -30).to_string(), "lab(50 40 -30)");
        assert_eq!(hwb(200, pct(10), pct(20)).to_string(), "hwb(200 10% 20%)");
    }

    /// `none` 是 CSS Color 4 的合法分量，代表「该通道缺省」
    #[test]
    fn a_color_component_can_be_missing() {
        assert_eq!(oklch(0.7, 0.15, NONE).to_string(), "oklch(0.7 0.15 none)");
    }

    #[test]
    fn color_mix_nests_any_color_type() {
        assert_eq!(
            color_mix(ColorSpace::Oklch, hex("#fff"), rgb(0, 0, 0)).to_string(),
            "color-mix(in oklch, #fff, rgb(0, 0, 0))"
        );
        assert_eq!(
            color_mix_weighted(ColorSpace::Srgb, hex("#fff"), pct(30), hex("#000"), pct(70))
                .to_string(),
            "color-mix(in srgb, #fff 30%, #000 70%)"
        );
        // 混合结果本身还是颜色，可以继续作为操作数
        let inner = color_mix(ColorSpace::Srgb, hex("#fff"), hex("#000"));
        assert!(
            color_mix(ColorSpace::Srgb, inner, hex("#f00"))
                .to_string()
                .starts_with("color-mix(in srgb, color-mix(")
        );
    }

    /// 空的 `ColorFn` 会渲染成 `color: ;`——无效声明
    #[test]
    fn a_default_color_fn_is_still_a_valid_color() {
        assert_eq!(ColorFn::default().to_string(), "transparent");
    }
}
