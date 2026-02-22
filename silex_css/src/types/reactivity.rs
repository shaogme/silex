use crate::types::*;

macro_rules! impl_into_signal_for_css {
    ($($t:ty),*) => {
        $(
            impl silex_core::traits::IntoSignal for $t {
                type Value = $t;
                type Signal = silex_core::reactivity::Constant<$t>;
                fn into_signal(self) -> Self::Signal { silex_core::reactivity::Constant(self) }
                fn is_constant_value(&self) -> bool { true }
            }
        )*
    };
}

pub(crate) use impl_into_signal_for_css;

impl_into_signal_for_css!(
    Px,
    Percent,
    Rgba,
    Auto,
    Rem,
    Em,
    Vw,
    Vh,
    Hex,
    Hsl,
    Url,
    BorderValue,
    MarginValue,
    PaddingValue,
    FlexValue,
    TransitionValue,
    BackgroundValue,
    UnsafeCss,
    CalcValue<LengthMark>,
    CalcValue<AngleMark>,
    Deg,
    Rad,
    Turn,
    GradientValue
);
