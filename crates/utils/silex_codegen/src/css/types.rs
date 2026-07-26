use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MdnCssProperty {
    pub syntax: String,
    pub status: String,
    pub inherited: bool,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MdnCssSyntax {
    pub syntax: String,
}

/// 一个属性允许的值类型能力。
///
/// 取代了此前那 8 个粗粒度分组（其中 `Shorthand` 一组就占了 78% 的属性，
/// 且「什么都收」）。每个能力对应 `define_props!` 里一组互不重叠的
/// `impl ValidFor<…>`——互不重叠是硬要求，重叠会直接编译失败（E0119）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueCap {
    /// `Px` / `Rem` / `Em` / `Vw` / `Vh`
    Length,
    /// `Percent`
    Percent,
    /// `CalcValue<LengthMark>`：有长度或百分比时才有意义
    LenCalc,
    /// 全部数值类型（含整数——`<number>` 也接受整数字面量）
    Num,
    /// 仅整数类型
    Int,
    /// `Deg` / `Rad` / `Turn` / `CalcValue<AngleMark>`
    Angle,
    /// `Sec` / `Ms` / `CalcValue<TimeMark>`
    Time,
    /// `Fr`——网格轨道的 `<flex>`，不与长度互通
    Flex,
    /// `Rgba` / `Hex` / `Hsl` / `ColorFn` / `ColorKeyword`
    Color,
    /// `Url`
    Url,
    /// `String` / `&'static str`
    Str,
}

impl ValueCap {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Percent => "Percent",
            Self::LenCalc => "LenCalc",
            Self::Num => "Num",
            Self::Int => "Int",
            Self::Angle => "Angle",
            Self::Time => "Time",
            Self::Flex => "Flex",
            Self::Color => "Color",
            Self::Url => "Url",
            Self::Str => "Str",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedProp {
    pub name: String,        // e.g. "background-color"
    pub method_name: String, // e.g. "background_color"
    pub struct_name: String, // e.g. "BackgroundColor"
    pub caps: Vec<ValueCap>,
    /// 该属性可以单独取的字面关键字
    pub keywords: Vec<String>,
    /// 取值可以由多个顶层分量拼成（`<length>{1,4}`、`a && b`、`<x>#`）。
    ///
    /// `caps` 里的 `Str` 有一部分正是从这里来的（复合属性得能写裸字符串），
    /// 所以宏侧不能拿 `Str` 反推「分量个数不限」——必须单独知道这一位。
    pub multi: bool,
    /// 语法里有 `<custom-ident>` / `<string>` / 解析不出来的引用：这个属性能取
    /// 什么在编译期无法穷举。
    ///
    /// `animation-name: fadeIn`、`font-family: Inter`、`grid-area: header` 都
    /// 落在这一类里——它们的关键字表只有 `none` / `auto`，照表判错会把完全
    /// 合法的写法拒掉。宏侧对这些属性一律放行。
    pub open: bool,
}

use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CssConfig {
    pub properties: Vec<ProcessedProp>,
    #[serde(default)]
    pub syntaxes: HashMap<String, MdnCssSyntax>,
    /// 全局具名颜色表，供 `ColorKeyword` 使用
    #[serde(default)]
    pub color_keywords: Vec<String>,
}
