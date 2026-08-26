use super::{MountContext, MountInstance, MountTarget, MountTransaction};
use crate::lifecycle::OwnerMount;
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::model::DomNode;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(crate) struct MountComposite;

impl MountComposite {
    pub(crate) fn mount<'scope, F>(
        context: &MountContext<'scope>,
        mount: F,
    ) -> SilexResult<MountInstance<'scope>>
    where
        F: FnOnce(&MountContext<'scope>) -> SilexResult<MountInstance<'scope>>,
    {
        let owner = context.owner();
        let transaction = context.transaction().child()?;
        let provisional_owner = OwnerMount::new(owner.child());
        let fragment = context.dom().create_fragment()?;
        let child_context = context.with_parts(
            MountTarget::append(context.dom().clone(), fragment.clone()),
            context.ancestry().clone(),
            provisional_owner.token(),
            transaction.clone(),
        );

        if let Err(error) = mount(&child_context) {
            return Self::rollback(
                context,
                &transaction,
                &provisional_owner,
                &fragment,
                &[],
                error,
            );
        }
        let nodes = match context.dom().children(&fragment) {
            Ok(nodes) => nodes,
            Err(error) => {
                return Self::rollback(
                    context,
                    &transaction,
                    &provisional_owner,
                    &fragment,
                    &[],
                    error.into(),
                );
            }
        };
        let owner_for_cleanup = provisional_owner.token();
        if let Err(error) = owner.on_cleanup(
            Box::new(move || {
                owner_for_cleanup
                    .close()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
            }),
            context.error_handler(),
        ) {
            return Self::rollback(
                context,
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        if let Err(error) = context.target().append_node(&fragment) {
            return Self::rollback(
                context,
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        if let Err(error) = transaction.commit() {
            return Self::rollback(
                context,
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        Ok(MountInstance::from_nodes(nodes))
    }

    fn rollback<'scope>(
        context: &MountContext<'scope>,
        transaction: &MountTransaction<'scope>,
        owner: &OwnerMount<'scope>,
        fragment: &DomNode,
        nodes: &[DomNode],
        primary: SilexError,
    ) -> SilexResult<MountInstance<'scope>> {
        let _ = transaction.rollback();
        if let Ok(fragment_nodes) = context.dom().children(fragment) {
            for node in fragment_nodes {
                let _ = context.dom().remove(&node);
            }
        }
        let remove_nodes = || {
            for node in nodes {
                if context.dom().parent(node).ok().flatten().is_some() {
                    let _ = context.dom().remove(node);
                }
            }
        };
        match catch_unwind(AssertUnwindSafe(|| owner.token().close())) {
            Ok(Ok(())) => {
                remove_nodes();
                Err(primary)
            }
            Ok(Err(error)) => {
                owner.token().report_close_error(error);
                remove_nodes();
                Err(primary.into_fatal())
            }
            Err(panic) => {
                owner
                    .token()
                    .report_close_error(CloseError::from_panic(panic));
                remove_nodes();
                Err(primary)
            }
        }
    }
}
