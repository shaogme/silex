//! 应用级 mount 生命周期 facade。

mod boundary;
mod builder;
mod handle;

pub use builder::MountBuilderContext;
pub use handle::MountedApp;
