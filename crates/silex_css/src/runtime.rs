pub(crate) mod backend;
pub mod dynamic;
/// 非 wasm 目标下的样式表后端。生产构建里它是个空转的实现，测试里它是观察窗。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod fake;
pub(crate) mod platform;
pub mod registry;
#[cfg(target_arch = "wasm32")]
pub(crate) mod sheet;
pub mod template;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

pub use dynamic::*;
pub use registry::*;
pub use template::*;
