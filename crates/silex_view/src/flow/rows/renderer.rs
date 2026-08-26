use crate::flow::context::BranchRenderContext;
use crate::flow::rows::updater::RowUpdater;
use crate::kernel::MountContext;
use silex_core::SilexResult;
use std::rc::Rc;

pub(crate) struct RowRenderContext<'scope, T> {
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) context: MountContext<'scope>,
    pub(crate) branch_context: Option<BranchRenderContext<'scope>>,
    pub(crate) updater: RowUpdater<'scope, T>,
}

pub(crate) struct RowRenderer<'scope, T> {
    inner: Rc<dyn Fn(RowRenderContext<'scope, T>) -> SilexResult<()> + 'scope>,
}

impl<'scope, T> Clone for RowRenderer<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'scope, T> RowRenderer<'scope, T> {
    pub(crate) fn new<F>(render: F) -> Self
    where
        F: Fn(RowRenderContext<'scope, T>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(render),
        }
    }

    pub(crate) fn call(&self, args: RowRenderContext<'scope, T>) -> SilexResult<()> {
        (self.inner)(args)
    }
}
