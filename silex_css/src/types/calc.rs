use crate::types::units::{Deg, Em, Percent, Px, Rad, Rem, Turn, Vh, Vw};
use std::fmt::Display;
use std::marker::PhantomData;

// ==========================================
// 核心标记 Trait 与 量纲 (Marks & Marker Traits)
// ==========================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMark;

pub trait CssLength: Display {}
pub trait CssAngle: Display {}
pub trait CssColor: Display {}
pub trait CssNumber: Display {}
pub trait CssPercentage: Display {}

#[derive(Clone, Debug, PartialEq)]
pub struct CalcValue<Mark>(pub String, pub PhantomData<Mark>);

impl<Mark> Default for CalcValue<Mark> {
    fn default() -> Self {
        Self(String::new(), PhantomData)
    }
}

impl<Mark> CalcValue<Mark> {
    pub fn new(s: String) -> Self {
        Self(s, PhantomData)
    }
    pub fn binary<L: Display, R: Display>(l: L, op: &'static str, r: R) -> Self {
        Self(format!("({} {} {})", l, op, r), PhantomData)
    }
}

impl<Mark> Display for CalcValue<Mark> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CssLength for CalcValue<LengthMark> {}
impl CssAngle for CalcValue<AngleMark> {}

impl<Mark> From<CalcValue<Mark>> for String {
    fn from(v: CalcValue<Mark>) -> Self {
        v.0
    }
}

pub fn calc<Mark>(v: CalcValue<Mark>) -> CalcValue<Mark> {
    CalcValue::new(format!("calc({})", v.0))
}

pub fn min<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    let mut s = String::from("min(");
    let mut empty = true;
    for arg in args {
        let v: CalcValue<Mark> = arg.into();
        if !v.0.is_empty() {
            if !empty {
                s.push_str(", ");
            }
            s.push_str(&v.0);
            empty = false;
        }
    }
    if empty {
        CalcValue::default()
    } else {
        s.push(')');
        CalcValue::new(s)
    }
}

pub fn max<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    let mut s = String::from("max(");
    let mut empty = true;
    for arg in args {
        let v: CalcValue<Mark> = arg.into();
        if !v.0.is_empty() {
            if !empty {
                s.push_str(", ");
            }
            s.push_str(&v.0);
            empty = false;
        }
    }
    if empty {
        CalcValue::default()
    } else {
        s.push(')');
        CalcValue::new(s)
    }
}

pub fn clamp<Mark, T>(min_v: T, val: T, max_v: T) -> CalcValue<Mark>
where
    T: Into<CalcValue<Mark>> + Display,
{
    CalcValue::new(format!("clamp({}, {}, {})", min_v, val, max_v))
}

pub trait IntoCalc<Mark> {
    fn into_calc(self) -> CalcValue<Mark>;
}

impl<T: Display> IntoCalc<LengthMark> for T {
    fn into_calc(self) -> CalcValue<LengthMark> {
        CalcValue::new(self.to_string())
    }
}

impl IntoCalc<AngleMark> for Deg {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for Rad {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for Turn {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for CalcValue<AngleMark> {
    fn into_calc(self) -> CalcValue<AngleMark> {
        self
    }
}

impl From<Deg> for CalcValue<AngleMark> {
    fn from(v: Deg) -> Self {
        v.into_calc()
    }
}
impl From<Rad> for CalcValue<AngleMark> {
    fn from(v: Rad) -> Self {
        v.into_calc()
    }
}
impl From<Turn> for CalcValue<AngleMark> {
    fn from(v: Turn) -> Self {
        v.into_calc()
    }
}

impl From<Px> for CalcValue<LengthMark> {
    fn from(v: Px) -> Self {
        v.into_calc()
    }
}
impl From<Percent> for CalcValue<LengthMark> {
    fn from(v: Percent) -> Self {
        v.into_calc()
    }
}
impl From<Rem> for CalcValue<LengthMark> {
    fn from(v: Rem) -> Self {
        v.into_calc()
    }
}
impl From<Em> for CalcValue<LengthMark> {
    fn from(v: Em) -> Self {
        v.into_calc()
    }
}
impl From<Vw> for CalcValue<LengthMark> {
    fn from(v: Vw) -> Self {
        v.into_calc()
    }
}
impl From<Vh> for CalcValue<LengthMark> {
    fn from(v: Vh) -> Self {
        v.into_calc()
    }
}
