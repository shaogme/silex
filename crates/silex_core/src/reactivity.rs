/// 类型擦除的原始句柄。
///
/// `silex_core` 的读取侧本来就是一个运行时打 tag 的联合体（[`Signal`] 枚举 +
/// [`RxNodeKind`](crate::RxNodeKind)），所以这一层保留擦除后的句柄；
/// 带种类的句柄（[`SignalId`] / [`MemoId`] / [`StoredId`] …）用在具体的原语上
/// —— 那里才有静态的种类可言，也正是在那里“把 stored value 当 signal 写”
/// 这类错误变成了编译错误。
pub use silex_reactivity::RawNodeId as NodeId;
pub use silex_reactivity::{
    CallbackId, EffectId, MemoId, NodeRefId, RawNodeId, ReactiveError, ReactiveResult, ScopeId,
    SignalId, StoredId,
    scope::{batch, create as create_scope, create_detached as create_detached_scope, dispose, on_cleanup},
    store::create as store_value,
};

mod effect;
mod memo;
mod mutation;
mod resource;
mod signal;
mod slice;
mod stored_value;

pub mod dispatch;

pub use effect::*;
pub use memo::*;
pub use mutation::*;
pub use resource::*;
pub use signal::*;
pub use slice::*;
pub use stored_value::*;
