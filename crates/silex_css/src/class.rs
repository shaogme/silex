use std::borrow::Cow;

/// 将各种 Rust 类型转换为 CSS 类名字符串的 Trait
pub trait IntoClass {
    fn write_class(&self, out: &mut String);
}

// 1. 基础字符串切片
impl IntoClass for str {
    fn write_class(&self, out: &mut String) {
        if !self.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(self);
        }
    }
}

// 2. 引用类型通用转发
impl<T: IntoClass + ?Sized> IntoClass for &T {
    fn write_class(&self, out: &mut String) {
        (*self).write_class(out);
    }
}

// 3. Owned String
impl IntoClass for String {
    fn write_class(&self, out: &mut String) {
        self.as_str().write_class(out);
    }
}

// 4. Cow<'_, str>
impl IntoClass for Cow<'_, str> {
    fn write_class(&self, out: &mut String) {
        self.as_ref().write_class(out);
    }
}

// 5. Option<T> 可选类名
impl<T: IntoClass> IntoClass for Option<T> {
    fn write_class(&self, out: &mut String) {
        if let Some(val) = self {
            val.write_class(out);
        }
    }
}

// 6. 布尔元组 (bool, C)
impl<C: IntoClass> IntoClass for (bool, C) {
    fn write_class(&self, out: &mut String) {
        if self.0 {
            self.1.write_class(out);
        }
    }
}

// 7. 三元条件元组 (bool, C1, C2)
impl<C1: IntoClass, C2: IntoClass> IntoClass for (bool, C1, C2) {
    fn write_class(&self, out: &mut String) {
        if self.0 {
            self.1.write_class(out);
        } else {
            self.2.write_class(out);
        }
    }
}

// 8. 切片 [T]
impl<T: IntoClass> IntoClass for [T] {
    fn write_class(&self, out: &mut String) {
        for item in self {
            item.write_class(out);
        }
    }
}

// 9. 数组 [T; N]
impl<T: IntoClass, const N: usize> IntoClass for [T; N] {
    fn write_class(&self, out: &mut String) {
        for item in self {
            item.write_class(out);
        }
    }
}

// 10. Vec<T>
impl<T: IntoClass> IntoClass for Vec<T> {
    fn write_class(&self, out: &mut String) {
        self.as_slice().write_class(out);
    }
}

/// 声明式 CSS 类名组合宏 `cx!`
#[macro_export]
macro_rules! cx {
    ($($item:expr),* $(,)?) => {{
        let mut _out = ::std::string::String::with_capacity(64);
        $($crate::IntoClass::write_class(&($item), &mut _out);)*
        _out
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_into_class_basic() {
        let res = cx!("p-4", "m-2");
        assert_eq!(res, "p-4 m-2");
    }

    #[test]
    fn test_into_class_conditional_tuples() {
        let is_active = true;
        let is_dark = false;
        let res = cx!(
            "btn",
            (is_active, "bg-blue-500"),
            (is_dark, "text-white", "text-black"),
        );
        assert_eq!(res, "btn bg-blue-500 text-black");
    }

    #[test]
    fn test_into_class_option() {
        let extra: Option<&str> = Some("rounded-lg");
        let none_extra: Option<&str> = None;
        let res = cx!("card", extra, none_extra);
        assert_eq!(res, "card rounded-lg");
    }

    #[test]
    fn test_into_class_nested() {
        let is_active = true;
        let res = cx!(
            "base",
            (is_active, Some("active")),
            vec!["shadow", "border"]
        );
        assert_eq!(res, "base active shadow border");
    }

    #[test]
    fn test_into_class_empty_string_handling() {
        let res = cx!("", "first", "", (true, ""), (false, "hidden"), "last");
        assert_eq!(res, "first last");
    }
}
