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

            impl<S: AsRef<str>> From<S> for $var_type {
                fn from(s: S) -> Self {
                    let str_ref = s.as_ref();
                    let clean = str_ref.trim();
                    $(
                        if clean.eq_ignore_ascii_case(stringify!($val_name))
                            || clean.eq_ignore_ascii_case(stringify!($val_name).replace('_', "-").as_str()) {
                            return $var_type::$val_name;
                        }
                    )*
                    Self::default()
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
}
