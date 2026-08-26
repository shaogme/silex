use crate::flow::rows::{RangeHandle, RowBlock, RowBlockConfig, RowRenderContext, RowRenderer};
use crate::kernel::{MountContext, MountInstance, MountTarget, View};
use silex_core::{EffectPhase, SilexError, SilexErrorKind, SilexResult};
use std::rc::Rc;

#[derive(Clone)]
pub struct DynamicRenderer<'scope> {
    inner: Rc<dyn Fn(MountContext<'scope>) -> SilexResult<MountInstance<'scope>> + 'scope>,
}
impl<'scope> DynamicRenderer<'scope> {
    pub fn new<F>(render: F) -> Self
    where
        F: Fn(MountContext<'scope>) -> SilexResult<MountInstance<'scope>> + 'scope,
    {
        Self {
            inner: Rc::new(render),
        }
    }
    pub(crate) fn call(&self, context: MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        (self.inner)(context)
    }
}

impl<'scope> View<'scope> for DynamicRenderer<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_dynamic_view(context, self.clone())
    }
}

impl<'scope, F, V> View<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let factory = self.clone();
        let renderer = DynamicRenderer::new(move |context| {
            let view = factory();
            context.mount(&view)
        });
        context.mount(&renderer)
    }
}

/// 含 comment anchors 的通用动态 View mount。
fn mount_dynamic_view<'scope>(
    context: &MountContext<'scope>,
    renderer: DynamicRenderer<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    let range = RangeHandle::at_target(context.target(), "dyn")?;
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, ()>| {
        renderer.call(args.context).map(|_| ())
    });
    let token = context.owner();
    let row_context = context.with_target(MountTarget::before(
        context.dom().clone(),
        range.end.clone(),
    ));
    let range_instance = range.clone();
    let row = match RowBlock::empty(
        &token,
        RowBlockConfig {
            range,
            render,
            item: (),
            index: 0,
            stateful: false,
            branch_runtime: false,
            error_handler: context.error_handler(),
            context: row_context,
        },
    ) {
        Ok(row) => row,
        Err(error) => {
            let _ = range_instance.remove();
            return Err(error);
        }
    };
    let row_state = token.owner_state(Some(row))?;
    let cleanup_state = row_state.clone();
    if let Err(error) = token.on_cleanup(
        Box::new(move || {
            if let Some(mut row) = cleanup_state.take_for_cleanup().flatten() {
                row.dispose()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
            }
            Ok(())
        }),
        context.error_handler(),
    ) {
        if let Some(mut row) = row_state.take_for_cleanup().flatten()
            && let Err(close_error) = row.dispose()
        {
            token.report_close_error(close_error);
        }
        let _ = range_instance.remove();
        return Err(error);
    }
    let effect_state = row_state.clone();
    if let Err(error) = token.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let Some(mut row) = effect_state.take()? else {
                return Ok(());
            };
            let result = row.update((), 0);
            effect_state.replace(Some(row))?;
            result
        }),
        context.error_handler(),
    ) {
        if let Some(mut row) = row_state.take_for_cleanup().flatten()
            && let Err(close_error) = row.dispose()
        {
            token.report_close_error(close_error);
        }
        let _ = range_instance.remove();
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![
        range_instance.start,
        range_instance.end,
    ]))
}
