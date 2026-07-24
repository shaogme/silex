use crate::css::tw::resolver::codegen::property_id::CssPropertyId;
use proc_macro2::Span;
use quote::quote;
use smallvec::SmallVec;
use std::{
    hash::{Hash, Hasher},
    mem::discriminant,
};
use syn::Expr;

/// 状态修饰符与响应式断点
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Modifier {
    /// 伪类状态: hover, focus, active, disabled, visited, first, last 等
    PseudoClass(String),
    /// 伪元素: before, after, placeholder 等
    PseudoElement(String),
    /// 响应式媒体查询断点: sm (640px), md (768px), lg (1024px), xl (1280px), 2xl (1536px)
    MediaBreakpoint(String),
    /// 暗黑模式: dark
    Dark,
    /// 自定义任意选择器修饰符: [&>svg]
    CustomSelector(String),
    /// 复合 Group 状态修饰符 (例: group-hover -> state="hover", name=None; group-data-[size=sm]/avatar -> state="data-[size=sm]", name=Some("avatar"))
    Group { state: String, name: Option<String> },
    /// 复合 Peer 状态修饰符 (例: peer-focus -> state="focus", name=None; peer-data-[state=open]/sidebar -> state="focus", name=Some("sidebar"))
    Peer { state: String, name: Option<String> },
    /// 容器查询修饰符 (例: @sm, @md, @[300px], @sidebar/md)
    ContainerQuery {
        name: Option<String>,
        min_width: String,
    },
    /// 子元素选择修饰符: *: (`& > *`)
    Child,
    /// 后代元素选择修饰符: **: (`& *`)
    Descendant,
    /// Data 属性修饰符: data-[slot=avatar] -> key="slot", value=Some("avatar")
    DataAttribute { key: String, value: Option<String> },
    /// Aria 属性修饰符: aria-[expanded=true] 或 aria-checked
    AriaAttribute { key: String, value: Option<String> },
    /// Has 条件选择修饰符: has-[.active] 或 has-data-[size=lg]
    Has(String),
}

/// Utility 规则值类型
#[derive(Debug, Clone)]
pub enum UtilityValue {
    /// 关键字值, 如 flex, block, auto, transparent, space-between
    Keyword(&'static str),
    /// 数值与标准单位: (val, unit)
    Numeric(f64, &'static str),
    /// 颜色 Hex 值: "#1e1e24"
    HexColor(String),
    /// Silex 主题变量: bg-theme(primary) / bg-theme(primary/50)
    ThemeVar(String, Option<f64>),
    /// 任意值字面量: p-[12px] -> "12px"
    ArbitraryLiteral(String),
    /// 动态 Rust 信号/表达式: p-[$(signal_val)]
    DynamicExpr(Expr, Span),
}

impl PartialEq for UtilityValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Keyword(a), Self::Keyword(b)) => a == b,
            (Self::Numeric(v1, u1), Self::Numeric(v2, u2)) => v1 == v2 && u1 == u2,
            (Self::HexColor(a), Self::HexColor(b)) => a == b,
            (Self::ThemeVar(v1, o1), Self::ThemeVar(v2, o2)) => v1 == v2 && o1 == o2,
            (Self::ArbitraryLiteral(a), Self::ArbitraryLiteral(b)) => a == b,
            (Self::DynamicExpr(e1, _), Self::DynamicExpr(e2, _)) => {
                quote!(#e1).to_string() == quote!(#e2).to_string()
            }
            _ => false,
        }
    }
}

impl Hash for UtilityValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        discriminant(self).hash(state);
        match self {
            Self::Keyword(k) => k.hash(state),
            Self::Numeric(v, u) => {
                v.to_bits().hash(state);
                u.hash(state);
            }
            Self::HexColor(c) => c.hash(state),
            Self::ThemeVar(v, o) => {
                v.hash(state);
                if let Some(alpha) = o {
                    alpha.to_bits().hash(state);
                }
            }
            Self::ArbitraryLiteral(a) => a.hash(state),
            Self::DynamicExpr(e, _) => {
                quote!(#e).to_string().hash(state);
            }
        }
    }
}

pub type ModifierList = SmallVec<[SpannedModifier; 2]>;

/// 归一化的 Utility 规则
#[derive(Debug, Clone)]
pub struct UtilityRule {
    pub modifiers: ModifierList,
    pub css_property: CssPropertyId,
    pub value: UtilityValue,
    pub span: Span,
}

/// 关联源码 Span 的修饰符包装，按 modifier 比较与 Hash
#[derive(Debug, Clone)]
pub struct SpannedModifier {
    pub modifier: Modifier,
    span: Span,
}

impl SpannedModifier {
    #[inline]
    pub fn new(modifier: Modifier, span: Span) -> Self {
        Self { modifier, span }
    }

    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }
}

impl PartialEq for SpannedModifier {
    fn eq(&self, other: &Self) -> bool {
        self.modifier == other.modifier
    }
}

impl PartialEq<Modifier> for SpannedModifier {
    fn eq(&self, other: &Modifier) -> bool {
        &self.modifier == other
    }
}

impl PartialEq<SpannedModifier> for Modifier {
    fn eq(&self, other: &SpannedModifier) -> bool {
        self == &other.modifier
    }
}

impl From<Modifier> for SpannedModifier {
    fn from(modifier: Modifier) -> Self {
        Self {
            modifier,
            span: Span::call_site(),
        }
    }
}

impl Eq for SpannedModifier {}

impl Hash for SpannedModifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.modifier.hash(state);
    }
}

impl Hash for UtilityRule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.modifiers.hash(state);
        self.css_property.hash(state);
        self.value.hash(state);
    }
}

/// `tw!` 宏的片段规则类型
#[derive(Debug, Clone)]
pub enum TwSegment {
    /// 静态 Utility 规则段 (例: "p-4 rounded-xl")
    Static(Vec<UtilityRule>),

    /// 动态条件响应式分支段: (condition, then_rules, else_rules)
    Conditional {
        condition: Expr,
        then_rules: Vec<UtilityRule>,
        else_rules: Vec<UtilityRule>,
    },
}

/// `tw!` 过程宏根输入结构
#[derive(Debug, Clone)]
pub struct TwInput {
    pub segments: Vec<TwSegment>,
    pub extra_classes: Vec<String>,
}
