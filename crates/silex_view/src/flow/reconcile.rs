use crate::flow::keyed::KeyedRow;
use crate::flow::rows::RowBlock;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::model::DomNode;
use std::{
    any::Any,
    collections::HashMap,
    hash::Hash,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub(crate) fn read_values<IF, IS, T>(source: &IF) -> SilexResult<Vec<T>>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS>,
    IS: ForLoopSource<Item = T>,
    T: Clone,
{
    source
        .with(|items| items.as_slice().map(|values| values.to_vec()))
        .and_then(|result| result)
}

pub(crate) fn reconcile_rollback_error(
    primary: SilexError,
    restore_order: Option<SilexError>,
    restore_updates: Option<CloseError>,
    cleanup: Option<CloseError>,
) -> SilexError {
    if restore_order.is_none() && restore_updates.is_none() && cleanup.is_none() {
        return primary;
    }
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "keyed reconcile rollback failed after {primary}; order={restore_order:?}; updates={restore_updates:?}; cleanup={cleanup:?}"
    )))
}

pub(crate) fn dispose_rows<'scope, T>(rows: &mut Vec<RowBlock<'scope, T>>) -> Option<CloseError> {
    let mut errors = Vec::new();
    for mut row in rows.drain(..) {
        match catch_unwind(AssertUnwindSafe(|| row.dispose())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}

pub(crate) fn dispose_keyed<'scope, T, K>(
    rows: &mut Vec<KeyedRow<'scope, T, K>>,
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for KeyedRow { mut row, .. } in rows.drain(..) {
        match catch_unwind(AssertUnwindSafe(|| row.dispose())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}

pub(crate) fn restore_indexed<'scope, T: Clone + 'scope>(
    rows: &mut [RowBlock<'scope, T>],
    updates: &[(usize, (T, usize))],
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for (index, (item, row_index)) in updates.iter().rev() {
        match catch_unwind(AssertUnwindSafe(|| {
            rows[*index].update(item.clone(), *row_index)
        })) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(error.to_string()))),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}

pub(crate) fn restore_keyed<'scope, T: Clone + 'scope, K: Hash + Eq>(
    rows: &mut HashMap<K, KeyedRow<'scope, T, K>>,
    updates: &[(K, (T, usize))],
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for (key, (item, index)) in updates.iter().rev() {
        if let Some(row) = rows.get_mut(key) {
            match catch_unwind(AssertUnwindSafe(|| row.row.update(item.clone(), *index))) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(error.to_string()))),
                Err(panic) => errors.push(CloseError::from_panic(panic)),
            }
        }
    }
    CloseError::combine(errors)
}

pub(crate) fn restore_keyed_order<T, K>(
    rows: &mut HashMap<K, KeyedRow<'_, T, K>>,
    order: &[K],
    parent: &DomNode,
    end: &DomNode,
) -> SilexResult<()>
where
    K: Hash + Eq,
{
    for key in order {
        if let Some(row) = rows.get_mut(key) {
            row.row.move_before(parent, end)?;
        }
    }
    Ok(())
}

pub(crate) fn panic_message(prefix: &str, panic: Box<dyn Any + Send>) -> String {
    if let Some(value) = panic.downcast_ref::<&str>() {
        format!("{prefix}: {value}")
    } else if let Some(value) = panic.downcast_ref::<String>() {
        format!("{prefix}: {value}")
    } else {
        format!("{prefix}: unknown panic")
    }
}
