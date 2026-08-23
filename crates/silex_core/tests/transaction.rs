use silex_core::{ReactiveError, Runtime, SilexError, SilexErrorKind, traits::RxGet};
use std::{cell::Cell, rc::Rc};

#[test]
fn transaction_publishes_multiple_signal_updates_once() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let first = owner.signal(1_i32).expect("first signal should initialize");
            let second = owner
                .signal(String::from("before"))
                .expect("second signal should initialize");
            let runs_in_effect = runs.clone();
            let first_for_effect = first;
            let second_for_effect = second;
            owner
                .effect(
                    silex_core::EffectPhase::Normal,
                    move || {
                        let _ = first_for_effect.get()?;
                        let _ = second_for_effect.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    owner
                        .error_handler(|_| {})
                        .expect("error handler should register"),
                )
                .expect("effect should initialize");

            owner
                .transaction(|transaction| {
                    assert_eq!(transaction.snapshot(first)?, 1);
                    transaction.update(first, |value| {
                        *value += 1;
                        Ok(())
                    })?;
                    transaction.set(second, String::from("after"))
                })
                .expect("transaction should commit");

            assert_eq!(first.get().expect("first signal should be readable"), 2);
            assert_eq!(
                second.get().expect("second signal should be readable"),
                "after"
            );
            assert_eq!(runs.get(), 2, "the effect should run once after commit");
            Ok::<(), SilexError>(())
        })
        .expect("owner should complete")
        .expect("test closure should complete");
}

#[test]
fn transaction_snapshot_clones_values_without_tracking() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let signal = owner.signal(vec![1_i32]).expect("signal should initialize");
            let runs_in_effect = runs.clone();
            let owner_in_effect = owner;
            owner
                .effect(
                    silex_core::EffectPhase::Normal,
                    move || {
                        owner_in_effect.transaction(move |transaction| {
                            let snapshot = transaction.snapshot(signal)?;
                            assert_eq!(snapshot, vec![1]);
                            Ok::<(), SilexError>(())
                        })?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    owner
                        .error_handler(|_| {})
                        .expect("error handler should register"),
                )
                .expect("effect should initialize");

            signal
                .set(vec![2])
                .expect("signal should update without retriggering snapshot effect");
            assert_eq!(runs.get(), 1);
            assert_eq!(signal.get().expect("signal should be readable"), vec![2]);
            Ok::<(), SilexError>(())
        })
        .expect("owner should complete")
        .expect("test closure should complete");
}

#[test]
fn transaction_rejects_a_foreign_runtime_signal() {
    let mut foreign_runtime = Runtime::new();
    let foreign_owner = foreign_runtime.owner().expect("foreign owner");
    let mut target_runtime = Runtime::new();
    let target_owner = target_runtime.owner().expect("target owner");
    let foreign_access = foreign_owner.access();
    let target_access = target_owner.access();
    let foreign_signal = foreign_access
        .signal(1_i32)
        .expect("foreign signal should initialize");

    let result = target_access.transaction(|transaction| transaction.set(foreign_signal, 2));
    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Transaction(error)))
            if error.primary() == ReactiveError::RuntimeMismatch
    ));

    target_owner.close().expect("target owner close");
    foreign_owner.close().expect("foreign owner close");
}

#[test]
fn transaction_user_error_discards_staged_values() {
    let mut runtime = Runtime::new();
    let result = runtime
        .with_transient(|owner| {
            let signal = owner.signal(7_i32).expect("signal should initialize");
            let user_error = SilexError::recoverable(SilexErrorKind::Framework(String::from(
                "user operation failed",
            )));
            let result: Result<(), SilexError> = owner.transaction(|transaction| {
                transaction.update(signal, |value| {
                    *value = 9;
                    Ok(())
                })?;
                Err(user_error)
            });

            assert!(matches!(
                result,
                Err(SilexError::Recoverable(SilexErrorKind::Framework(message)))
                    if message == "user operation failed"
            ));
            assert_eq!(signal.get().expect("signal should remain readable"), 7);
            Ok::<(), SilexError>(())
        })
        .expect("owner should complete");

    result.expect("the test closure should complete");
}

#[test]
fn transaction_rejects_duplicate_targets() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let signal = owner.signal(3_i32).expect("signal should initialize");
            let result = owner.transaction(|transaction| {
                transaction.set(signal, 4)?;
                transaction.set(signal, 5)
            });

            assert!(matches!(
                result,
                Err(SilexError::Fatal(SilexErrorKind::Transaction(error)))
                    if error.primary() == ReactiveError::DuplicateTarget
            ));
            assert_eq!(signal.get().expect("signal should remain readable"), 3);
            Ok::<(), SilexError>(())
        })
        .expect("owner should complete")
        .expect("test closure should complete");
}

#[test]
fn transaction_keeps_inventory_balance_and_orders_atomic() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let stock = owner.signal(5_i32).expect("stock should initialize");
            let balance = owner.signal(100_i32).expect("balance should initialize");
            let orders = owner.signal(0_u32).expect("orders should initialize");

            owner
                .transaction(|transaction| {
                    transaction.update(stock, |value| {
                        *value -= 3;
                        Ok(())
                    })?;
                    transaction.update(balance, |value| {
                        *value -= 30;
                        Ok(())
                    })?;
                    transaction.update(orders, |value| {
                        *value += 1;
                        Ok(())
                    })
                })
                .expect("successful transaction should commit");

            assert_eq!(stock.get().expect("stock should be readable"), 2);
            assert_eq!(balance.get().expect("balance should be readable"), 70);
            assert_eq!(orders.get().expect("orders should be readable"), 1);

            let failed: Result<(), SilexError> = owner.transaction(|transaction| {
                transaction.update(stock, |value| {
                    *value -= 2;
                    Ok(())
                })?;
                transaction.update(balance, |value| {
                    if *value < 80 {
                        return Err(SilexError::recoverable(SilexErrorKind::Framework(
                            String::from("balance is too low"),
                        )));
                    }
                    *value -= 80;
                    Ok(())
                })?;
                transaction.update(orders, |value| {
                    *value += 1;
                    Ok(())
                })
            });

            assert!(matches!(
                failed,
                Err(SilexError::Recoverable(SilexErrorKind::Framework(message)))
                    if message == "balance is too low"
            ));
            assert_eq!(stock.get().expect("stock should remain readable"), 2);
            assert_eq!(balance.get().expect("balance should remain readable"), 70);
            assert_eq!(orders.get().expect("orders should remain readable"), 1);
            Ok::<(), SilexError>(())
        })
        .expect("owner should complete")
        .expect("test closure should complete");
}
