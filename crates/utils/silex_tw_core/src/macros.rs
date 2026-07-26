/// 快捷宏：用于构造 `Cow<'static, str>` 静态规则切片、Vector 或单个 Cow/Tuple
///
/// 仅在 crate 内部通过 `#[macro_use]` 的文本作用域可见，不对外导出。
macro_rules! cow {
    () => {
        &[] as &[(&'static str, ::std::borrow::Cow<'static, str>)]
    };
    // 静态切片：cow![ ("k1", "v1"), ("k2", "v2") ]
    [ $( ( $k:expr, $v:expr $(,)? ) ),* $(,)? ] => {
        &[ $( ($k, ::std::borrow::Cow::Borrowed($v)) ),* ]
    };
    // 动态/混合 Vec：cow!(vec [ ("k1", "v1"), ("k2", string_val) ])
    ( vec [ $( ( $k:expr, $v:expr $(,)? ) ),* $(,)? ] ) => {
        vec![ $( ($k, ::std::borrow::Cow::from($v)) ),* ]
    };
    ( vec! [ $( ( $k:expr, $v:expr $(,)? ) ),* $(,)? ] ) => {
        vec![ $( ($k, ::std::borrow::Cow::from($v)) ),* ]
    };
    // 单个 key-value 元组：cow!("key", val)
    ( $k:expr, $v:expr ) => {
        ($k, ::std::borrow::Cow::from($v))
    };
    // 单个 Cow 转换：cow!(val)
    ( $v:expr ) => {
        ::std::borrow::Cow::from($v)
    };
}
