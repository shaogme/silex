use core::fmt;

/// 选项名解析失败：写错的选项名不应静默回退到默认值
///
/// `tw_variants!` 生成的 `get()` 为了配合运行时字符串（`Signal<String>`）仍然宽容，
/// 但 `try_from_str` / `FromStr` / `get_checked` 这几条路径会把错误如实交回调用方。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVariantOption {
    /// 用户实际传入的字符串
    pub input: String,
    /// 该变体的全部合法选项名
    pub options: &'static [&'static str],
}

impl fmt::Display for UnknownVariantOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown variant option '{}'; expected one of {:?}",
            self.input, self.options
        )
    }
}

impl core::error::Error for UnknownVariantOption {}

/// 选项名匹配：忽略大小写与 `-` / `_` 分隔符
///
/// 生成的枚举变体名是 PascalCase（`icon-xs` → `IconXs`），但用户在运行时传进来的
/// 是源码里写的那个字符串 `"icon-xs"`。此前的比较只做了"忽略大小写"与
/// "把下划线换成连字符"两种尝试，`IconXs` 与 `icon-xs` 因此永远匹配不上——
/// 于是 `size="icon-xs"` 静默拿到了默认档位的样式。
pub fn variant_key_eq(variant_ident: &str, input: &str) -> bool {
    let norm = |s: &str| -> String {
        s.trim()
            .chars()
            .filter(|c| *c != '-' && *c != '_')
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    norm(variant_ident) == norm(input)
}

/// 定义组件 CSS 变体 Schema 的 Trait
pub trait VariantSchema {
    type Config;

    /// 获取变体基础 CSS 类名
    fn base(&self) -> &'static str;

    /// 渲染变体配置组合对应的 CSS 类名字符串
    fn render(&self, config: &Self::Config) -> String;
}

/// 声明组件变体的宏 `declare_variants!`
///
/// 自动生成类型安全的 Variant 枚举、Config 结构体、`VariantSchema` 与 `IntoClass` 实现。
#[macro_export]
macro_rules! declare_variants {
    // 1. 包含 compound_variants 的入口
    (
        $vis:vis struct $struct_name:ident {
            base: $base:expr,
            variants: {
                $(
                    $var_vis:vis $var_name:ident : $var_type:ident [default = $def_val:ident] = {
                        $($val_name:ident => $cls:expr),* $(,)?
                    }
                ),* $(,)?
            },
            compound_variants: [
                $(( $($cmp_var:ident == $cmp_type:ident :: $cmp_val:ident),+ $(,)? ) => $cmp_cls:expr),* $(,)?
            ] $(,)?
        }
    ) => {
        $crate::declare_variants! {
            @impl
            $vis struct $struct_name {
                base: $base,
                variants: {
                    $($var_vis $var_name : $var_type [default = $def_val] = { $($val_name => $cls),* }),*
                },
                compound_variants: [
                    $(( $($cmp_var == $cmp_type :: $cmp_val),+ ) => $cmp_cls),*
                ]
            }
        }
    };

    // 2. 不包含 compound_variants 的入口
    (
        $vis:vis struct $struct_name:ident {
            base: $base:expr,
            variants: {
                $(
                    $var_vis:vis $var_name:ident : $var_type:ident [default = $def_val:ident] = {
                        $($val_name:ident => $cls:expr),* $(,)?
                    }
                ),* $(,)?
            } $(,)?
        }
    ) => {
        $crate::declare_variants! {
            @impl
            $vis struct $struct_name {
                base: $base,
                variants: {
                    $($var_vis $var_name : $var_type [default = $def_val] = { $($val_name => $cls),* }),*
                },
                compound_variants: []
            }
        }
    };

    // 3. 核心展开模式
    (
        @impl
        $vis:vis struct $struct_name:ident {
            base: $base:expr,
            variants: {
                $(
                    $var_vis:vis $var_name:ident : $var_type:ident [default = $def_val:ident] = {
                        $($val_name:ident => $cls:expr),*
                    }
                ),*
            },
            compound_variants: [
                $(( $($cmp_var:ident == $cmp_type:ident :: $cmp_val:ident),+ ) => $cmp_cls:expr),*
            ]
        }
    ) => {
        // A. 为每个变体自动生成 Enum
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            $vis enum $var_type {
                $($val_name),*
            }

            impl Default for $var_type {
                fn default() -> Self {
                    Self::$def_val
                }
            }

            impl $var_type {
                /// 该变体的全部合法选项名（生成的 PascalCase 形式）
                ///
                /// 解析时忽略大小写与 `-` / `_`，所以源码里写的 `icon-xs` 与这里的
                /// `IconXs` 是同一个选项。
                pub const OPTIONS: &'static [&'static str] = &[$(stringify!($val_name)),*];

                /// 严格解析：未知选项名返回 `Err`，不静默回退默认值
                ///
                /// 空字符串视为"未指定"，返回默认值——运行时用 `Signal<String>` 驱动的
                /// 组件在未设置该 prop 时拿到的就是空串，那不是拼写错误。
                pub fn try_from_str(
                    s: &str,
                ) -> ::core::result::Result<Self, $crate::tw::variants::UnknownVariantOption> {
                    let clean = s.trim();
                    if clean.is_empty() {
                        return ::core::result::Result::Ok(<Self as ::core::default::Default>::default());
                    }
                    $(
                        if $crate::tw::variants::variant_key_eq(stringify!($val_name), clean) {
                            return ::core::result::Result::Ok($var_type::$val_name);
                        }
                    )*
                    ::core::result::Result::Err($crate::tw::variants::UnknownVariantOption {
                        input: ::std::string::String::from(clean),
                        options: Self::OPTIONS,
                    })
                }
            }

            impl ::core::str::FromStr for $var_type {
                type Err = $crate::tw::variants::UnknownVariantOption;
                fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                    Self::try_from_str(s)
                }
            }

            impl<S: AsRef<str>> From<S> for $var_type {
                fn from(s: S) -> Self {
                    Self::try_from_str(s.as_ref()).unwrap_or_default()
                }
            }
        )*

        // B. 生成 Config 结构体
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $struct_name {
            $(
                $var_vis $var_name: $var_type,
            )*
        }

        impl $struct_name {
            #[inline]
            #[allow(dead_code)]
            pub fn new() -> Self {
                Self::default()
            }
        }

        // C. 实现 IntoClass Trait
        impl $crate::IntoClass for $struct_name {
            fn write_class(&self, out: &mut ::std::string::String) {
                $crate::IntoClass::write_class(&($base), out);
                $(
                    match self.$var_name {
                        $(
                            $var_type::$val_name => {
                                $crate::IntoClass::write_class(&($cls), out);
                            }
                        )*
                    }
                )*

                $(
                    if true $(&& self.$cmp_var == $cmp_type::$cmp_val)+ {
                        $crate::IntoClass::write_class(&($cmp_cls), out);
                    }
                )*
            }
        }

        // D. 实现 VariantSchema Trait
        impl $crate::tw::variants::VariantSchema for $struct_name {
            type Config = Self;

            fn base(&self) -> &'static str {
                let mut out = ::std::string::String::new();
                $crate::IntoClass::write_class(&($base), &mut out);
                // 由于 base 可能为 &str 或 String，通过 Leak 变成 &'static str
                ::std::boxed::Box::leak(out.into_boxed_str())
            }

            fn render(&self, config: &Self::Config) -> String {
                let mut out = ::std::string::String::with_capacity(128);
                $crate::IntoClass::write_class(config, &mut out);
                out
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    declare_variants! {
        pub struct TestButtonVariants {
            base: "btn",
            variants: {
                pub variant: TestButtonVariant [default = Primary] = {
                    Primary => "btn-primary",
                    Secondary => "btn-secondary",
                },
                pub size: TestButtonSize [default = Sm] = {
                    Sm => "btn-sm",
                    Lg => "btn-lg",
                },
            },
            compound_variants: [
                (variant == TestButtonVariant::Secondary, size == TestButtonSize::Lg) => "btn-secondary-lg",
            ]
        }
    }

    #[test]
    fn test_declare_variants_basic() {
        let v = TestButtonVariants {
            variant: TestButtonVariant::Primary,
            size: TestButtonSize::Sm,
        };
        assert_eq!(v.render(&v), "btn btn-primary btn-sm");
    }

    #[test]
    fn test_declare_variants_compound() {
        let v = TestButtonVariants {
            variant: TestButtonVariant::Secondary,
            size: TestButtonSize::Lg,
        };
        assert_eq!(v.render(&v), "btn btn-secondary btn-lg btn-secondary-lg");
    }

    #[test]
    fn test_from_str() {
        let v1 = TestButtonVariant::from("secondary");
        assert_eq!(v1, TestButtonVariant::Secondary);

        let v2 = TestButtonVariant::from("unknown");
        assert_eq!(v2, TestButtonVariant::Primary); // default
    }

    declare_variants! {
        pub struct TestSizeVariants {
            base: "box",
            variants: {
                pub size: TestSizeOption [default = Md] = {
                    Md => "box-md",
                    IconXs => "box-icon-xs",
                },
            }
        }
    }

    /// 回归点：源码里写 `icon-xs`，生成的枚举变体是 `IconXs`。
    /// 旧的 `From<S>` 只试了"忽略大小写"与"下划线换连字符"，两者都匹配不上，
    /// 于是 `size="icon-xs"` 静默拿到了 `Md` 的样式。
    #[test]
    fn kebab_case_option_names_match_their_pascal_case_variant() {
        assert_eq!(
            TestSizeOption::try_from_str("icon-xs"),
            Ok(TestSizeOption::IconXs)
        );
        assert_eq!(TestSizeOption::from("icon-xs"), TestSizeOption::IconXs);
        assert_eq!(
            "icon_xs".parse::<TestSizeOption>(),
            Ok(TestSizeOption::IconXs)
        );
    }

    #[test]
    fn strict_parsing_reports_unknown_options() {
        let err = TestSizeOption::try_from_str("icon-xxl").unwrap_err();
        assert_eq!(err.input, "icon-xxl");
        assert_eq!(err.options, &["Md", "IconXs"]);
        assert!(err.to_string().contains("unknown variant option"));
        // 空串是"未指定"，不是拼写错误
        assert_eq!(TestSizeOption::try_from_str(""), Ok(TestSizeOption::Md));
        assert_eq!(TestSizeOption::try_from_str("  "), Ok(TestSizeOption::Md));
    }
}
