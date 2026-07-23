use phf::phf_map;
use proc_macro2::Span;
use syn::Result;

/// 属性解析结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyResolveResult {
    /// 匹配到强类型的标准属性/短别名 (如 Padding, BorderRadius)
    Builtin(String),
    /// 自定义 CSS 变量 (--*)，统一映射为 props::Any
    CustomVar,
}

/// 短别名映射表 (如 p => Padding, bg => BackgroundColor)
pub static SHORT_ALIAS_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "any" => "Any",
    "p" => "Padding",
    "px" => "PaddingInline",
    "py" => "PaddingBlock",
    "pt" => "PaddingTop",
    "pr" => "PaddingRight",
    "pb" => "PaddingBottom",
    "pl" => "PaddingLeft",
    "m" => "Margin",
    "mx" => "MarginInline",
    "my" => "MarginBlock",
    "mt" => "MarginTop",
    "mr" => "MarginRight",
    "mb" => "MarginBottom",
    "ml" => "MarginLeft",
    "w" => "Width",
    "h" => "Height",
    "bg" => "BackgroundColor",
    "text" => "Color",
    "border" => "BorderColor",
    "rounded" => "BorderRadius",
};

/// 解析 CSS 属性名，返回与之匹配的强类型 Struct 名称或错误
pub fn resolve_property_type(prop: &str, _span: Span) -> Result<PropertyResolveResult> {
    // 1. 自定义 CSS 变量 (--*) 统一映射为 props::Any
    if prop.starts_with("--") {
        return Ok(PropertyResolveResult::CustomVar);
    }

    // 2. 短别名优先 PHF 查表
    if let Some(&type_name) = SHORT_ALIAS_MAP.get(prop) {
        return Ok(PropertyResolveResult::Builtin(type_name.to_string()));
    }

    // 3. 动态将 kebab-case 转换为 PascalCase Ident（如 margin-top => MarginTop）
    let mut pascal = String::with_capacity(prop.len());
    for part in prop.split('-') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            pascal.extend(first.to_uppercase());
            pascal.push_str(chars.as_str());
        }
    }

    Ok(PropertyResolveResult::Builtin(pascal))
}
