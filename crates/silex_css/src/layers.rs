//! 级联层（`@layer`）的层序——整个 CSS 系统里唯一一处优先级约定。
//!
//! 此前 `@layer base, components, utilities;` 这行层序声明是写了的，但真正落进
//! 层里的只有 `styled!`（components）与 `css!` / `tw!`（utilities）：`base` 一条
//! 规则都没有，而 `sty()` 与 `global!` 的产出**完全不带 layer**。CSS 规范里
//! 无层规则的优先级高于所有具名层，于是 `sty()` 与 `global!` 无条件压过所有组件
//! 样式，两者之间又只能靠注入先后决定胜负。这套链条既没写进文档，也没有任何
//! 断言测试保护。
//!
//! 现在四层都有明确归属，优先级从低到高：
//!
//! | 层 | 谁写进来 | 用途 |
//! | --- | --- | --- |
//! | `base` | `global!` | 全局重置、元素默认样式 |
//! | `components` | `styled!` | 组件自身的样式 |
//! | `utilities` | `css!` / `tw!` | 工具类，按设计就该压过组件默认值 |
//! | `overrides` | `sty()` | 针对单个元素实例的就地覆盖，优先级最高 |
//!
//! 注意 `global!` 从「无层（压过一切）」变成了 `base`（垫在最底下）——这正是
//! `base` 这一层存在的意义，也是它此前一直空着的原因。

/// 层序声明，必须是静态样式表的第一条规则。
pub const ORDER_STATEMENT: &str = "@layer base, components, utilities, overrides;";

/// 层序，从低优先级到高优先级。
pub const ORDER: [&str; 4] = [BASE, COMPONENTS, UTILITIES, OVERRIDES];

/// `global!` —— 全局重置
pub const BASE: &str = "base";
/// `styled!` —— 组件样式
pub const COMPONENTS: &str = "components";
/// `css!` / `tw!` —— 工具类
pub const UTILITIES: &str = "utilities";
/// `sty()` —— 元素实例上的就地覆盖
pub const OVERRIDES: &str = "overrides";

/// 把一段 CSS 包进指定的层。
pub fn wrap(layer: &str, css: &str) -> String {
    let mut out = String::with_capacity(css.len() + layer.len() + 12);
    out.push_str("@layer ");
    out.push_str(layer);
    out.push_str(" {\n");
    out.push_str(css);
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 层序声明与 `ORDER` 必须是同一份事实：声明里的顺序决定优先级，
    /// 常量则是各产出点写 layer 名时的依据，两者错开就等于优先级链断了。
    #[test]
    fn the_order_statement_matches_the_constants() {
        assert_eq!(ORDER_STATEMENT, format!("@layer {};", ORDER.join(", ")));
    }

    #[test]
    fn wrap_nests_the_css_in_the_named_layer() {
        assert_eq!(
            wrap(OVERRIDES, ".a{color:red}\n"),
            "@layer overrides {\n.a{color:red}\n}\n"
        );
    }
}
