//! 类型擦除的回调：注册一次，句柄到处传。

use crate::{CallbackId, ReactiveResult, store};
use std::{any::Any, rc::Rc};

type Erased = Rc<dyn Fn(Box<dyn Any>)>;

/// 注册一个类型擦除的回调，返回可以到处传递的句柄。
///
/// 回调的生命周期与所属 scope 绑定：scope 销毁后 [`invoke`] 变成一个
/// [`NoSuchNode`](ReactiveError::NoSuchNode)。
#[track_caller]
pub fn create<F>(f: F) -> CallbackId
where
    F: Fn(Box<dyn Any>) + 'static,
{
    CallbackId::from_raw(store::create_raw::<Erased>(Rc::new(f)))
}

/// 调用一个已注册的回调。
///
/// 回调本身是用户代码，因此它在运行时**放开对该节点的借用之后**才被执行 ——
/// 这里先把 `Rc` 克隆出来（一次引用计数递增），归还载荷，然后才调用。
/// 从前是直接在借用作用域内 `(data.f)(arg)`，回调里注册/销毁任何节点都会
/// 作废运行时手里那个引用（审计报告 §2.1）。
///
/// 这条路径也因此**允许重入**：回调里再 `invoke` 同一个回调是可以的
/// （由用户自己保证不会无限递归）。
pub fn invoke(id: CallbackId, arg: Box<dyn Any>) -> ReactiveResult<()> {
    let f: Erased = store::with_raw::<Erased, _>(id.raw(), Rc::clone)?;
    f(arg);
    Ok(())
}
