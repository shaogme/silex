use crate::{builder::Style, types::*};
use silex_core::{
    Memo, ReactiveInput, ReadSignal, RwSignal, Rx, RxFrom, RxValueKind, Scope, Signal, StoredValue,
};
use std::{borrow::Cow, fmt::Display};

/// A CSS value that is either owned static data or an existing scoped source.
#[derive(Clone)]
pub enum CssSource<'scope, T> {
    Static(T),
    Reactive(Rx<'scope, T>),
}

/// Converts a value into the CSS-local source representation.
pub trait IntoCssSource<'scope> {
    type Value: Display + Clone + 'scope;

    fn into_css_source(self) -> CssSource<'scope, Self::Value>;
}

/// Converts an existing scoped reactive node into a CSS source.
#[diagnostic::on_unimplemented(
    message = "CSS 动态插值只能接受带当前生命周期的响应式 source (`IntoCssReactive<'scope>`)；普通静态值请直接写入 CSS 或使用 var(--name)",
    label = "此值不是 scoped reactive source",
    note = "动态 `$(...)` 不会为静态值创建隐式 Runtime"
)]
pub trait IntoCssReactive<'scope> {
    type Value: Display + Clone + 'scope;

    fn into_css_reactive(self) -> Rx<'scope, Self::Value>;
}

macro_rules! impl_static_source {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<'scope> IntoCssSource<'scope> for $ty
            where
                $ty: Display + Clone + 'scope,
            {
                type Value = $ty;

                fn into_css_source(self) -> CssSource<'scope, Self::Value> {
                    CssSource::Static(self)
                }
            }
        )*
    };
}

impl_static_source!(
    String,
    &'static str,
    Cow<'static, str>,
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    Percent,
    Fr,
    Rgba,
    Auto,
    Hex,
    Hsl,
    ColorFn,
    ColorName,
    NoneValue,
    CssWide,
    Url,
    BorderValue,
    MarginValue,
    PaddingValue,
    FlexValue,
    TransitionValue,
    BackgroundValue,
    CssUnsafe,
    TransformValue,
    TransformBuilder,
    GridTemplateAreasValue,
    FontVariationSettingsValue,
    CalcValue<LengthMark>,
    CalcValue<AngleMark>,
    CalcValue<TimeMark>,
    GradientValue,
    Px,
    Rem,
    Em,
    Ch,
    Ex,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Dvw,
    Dvh,
    Svw,
    Svh,
    Lvw,
    Lvh,
    Pt,
    Pc,
    Cm,
    Mm,
    In,
    Qmm,
    Deg,
    Rad,
    Turn,
    Sec,
    Ms,
);

impl<'scope, T> IntoCssSource<'scope> for CssVar<T>
where
    T: 'scope,
    CssVar<T>: Display + Clone,
{
    type Value = Self;

    fn into_css_source(self) -> CssSource<'scope, Self::Value> {
        CssSource::Static(self)
    }
}

impl<'scope, T> IntoCssSource<'scope> for CssOption<T>
where
    T: Display + Clone + 'scope,
{
    type Value = Self;

    fn into_css_source(self) -> CssSource<'scope, Self::Value> {
        CssSource::Static(self)
    }
}

crate::register_generated_keywords!(impl_static_source);

impl<'scope, T> IntoCssSource<'scope> for Rx<'scope, T, RxValueKind>
where
    T: Display + Clone + 'scope,
{
    type Value = T;

    fn into_css_source(self) -> CssSource<'scope, Self::Value> {
        CssSource::Reactive(self)
    }
}

impl<'scope, T> IntoCssReactive<'scope> for Rx<'scope, T, RxValueKind>
where
    T: Display + Clone + 'scope,
{
    type Value = T;

    fn into_css_reactive(self) -> Rx<'scope, Self::Value> {
        self
    }
}

macro_rules! impl_css_source_for_node {
    ($($ty:ident),* $(,)?) => {
        $(
            impl<'scope, T> IntoCssSource<'scope> for $ty<'scope, T>
            where
                T: Display + Clone + 'scope,
            {
                type Value = T;

                fn into_css_source(self) -> CssSource<'scope, Self::Value> {
                    CssSource::Reactive(self.into_rx())
                }
            }

            impl<'scope, T> IntoCssReactive<'scope> for $ty<'scope, T>
            where
                T: Display + Clone + 'scope,
            {
                type Value = T;

                fn into_css_reactive(self) -> Rx<'scope, Self::Value> {
                    self.into_rx()
                }
            }
        )*
    };
}

impl_css_source_for_node!(ReadSignal, RwSignal, Signal, Memo, StoredValue);

macro_rules! impl_reactive_input_for_keyword {
    ($($ty:ident),* $(,)?) => {
        $(
            impl<'scope> ReactiveInput<'scope, Signal<'scope, $ty>> for $ty {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Signal<'scope, $ty> {
                    <Signal<'scope, $ty> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }
        )*
    };
}

crate::register_generated_keywords!(impl_reactive_input_for_keyword);

impl<'scope> ReactiveInput<'scope, Signal<'scope, Style<'scope>>> for Style<'scope> {
    fn into_reactive_input(self, scope: Scope<'scope>) -> Signal<'scope, Style<'scope>> {
        <Signal<'scope, Style<'scope>> as RxFrom<'scope>>::rx_from(scope, self)
    }
}
