#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use criterion::{BenchmarkId, Criterion, Throughput, criterion_group};
    use silex_reactivity::{ErrorHandlerToken, OwnerAccess, Runtime, unwind_safe};
    use std::{
        cell::Cell,
        hint::black_box,
        rc::Rc,
        time::{Duration, Instant},
    };

    const SIGNAL_SIZES: &[usize] = &[1, 64, 1024, 4096];
    const GRAPH_SIZES: &[usize] = &[1, 8, 32, 128];
    const DEPENDENCY_SIZES: &[usize] = &[10, 100, 1_000, 10_000];
    const OWNER_SIZES: &[usize] = &[1, 16, 128, 512];

    fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
        scope.error_handler(|_| {}).expect("handler registration")
    }

    fn scoped_signal_round_trip(c: &mut Criterion) {
        c.bench_function("scoped signal round trip", |bench| {
            bench.iter(|| {
                let mut runtime = Runtime::new();
                let root = runtime.owner().expect("runtime root creation");
                {
                    let scope = root.access();
                    let (read, write) = scope.signal(0i32).expect("fallible reactive creation");
                    let _effect = scope
                        .effect(
                            move || {
                                black_box(read.get().expect("benchmark read"));
                                Ok(())
                            },
                            handler(scope),
                        )
                        .expect("benchmark effect should initialize");
                    write.set(black_box(1)).expect("benchmark signal update");
                }
            });
        });
    }

    fn bench_signal_create(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/create");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for iteration in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let start = Instant::now();

                        root.with_access(|scope| {
                            for index in 0..size {
                                let signal = scope
                                    .signal((iteration as i32) ^ (index as i32))
                                    .expect("fallible reactive creation");
                                black_box(signal);
                            }
                        });

                        total += start.elapsed();
                        root.close().expect("benchmark root disposal");
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_signal_create_heap(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/create-heap");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for iteration in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let start = Instant::now();

                        root.with_access(|scope| {
                            for index in 0..size {
                                let value = [(iteration as u8) ^ (index as u8); 32];
                                black_box(scope.signal(value).expect("benchmark signal creation"));
                            }
                        });

                        total += start.elapsed();
                        root.close().expect("benchmark root disposal");
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_signal_read_untracked(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/read-untracked");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut signals = Vec::with_capacity(size);
                        for index in 0..size {
                            signals.push(
                                scope
                                    .signal(index as i32)
                                    .expect("benchmark signal creation")
                                    .0,
                            );
                        }

                        bench.iter(|| {
                            let mut observed = 0i32;
                            for signal in &signals {
                                observed = observed.wrapping_add(black_box(
                                    signal.get_untracked().expect("benchmark read"),
                                ));
                            }
                            black_box(observed);
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_signal_read_tracked(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/read-tracked");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut signals = Vec::with_capacity(size);
                        for index in 0..size {
                            signals.push(
                                scope
                                    .signal(index as i32)
                                    .expect("benchmark signal creation"),
                            );
                        }

                        let reads: Vec<_> = signals.iter().map(|(read, _)| *read).collect();
                        let trigger = signals[0].1;
                        let runs = Rc::new(Cell::new(0usize));
                        let reads_in_effect = reads.clone();
                        let runs_in_effect = runs.clone();
                        let _effect = scope
                            .effect(
                                move || {
                                    let mut observed = 0i32;
                                    for signal in &reads_in_effect {
                                        observed = observed.wrapping_add(black_box(
                                            signal.get().expect("benchmark read"),
                                        ));
                                    }
                                    black_box(observed);
                                    runs_in_effect.set(runs_in_effect.get().wrapping_add(1));
                                    Ok(())
                                },
                                handler(scope),
                            )
                            .expect("benchmark effect should initialize");

                        let mut value = 0i32;
                        bench.iter(|| {
                            value = value.wrapping_add(1);
                            trigger
                                .set(black_box(value))
                                .expect("benchmark signal update");
                            black_box(runs.get());
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_signal_write(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/write");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut signals = Vec::with_capacity(size);
                        for index in 0..size {
                            signals.push(
                                scope
                                    .signal(index as i32)
                                    .expect("benchmark signal creation")
                                    .1,
                            );
                        }

                        bench.iter(|| {
                            for signal in &signals {
                                let result = signal.update(|value| {
                                    *value = black_box(value.wrapping_add(1));
                                });
                                result.expect("benchmark signal update");
                            }
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_stored_create(c: &mut Criterion) {
        let mut group = c.benchmark_group("stored/create");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for iteration in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let start = Instant::now();

                        root.with_access(|scope| {
                            for index in 0..size {
                                let stored = scope
                                    .stored((iteration as u32) ^ (index as u32))
                                    .expect("fallible reactive creation");
                                black_box(stored);
                            }
                        });

                        total += start.elapsed();
                        root.close().expect("benchmark root disposal");
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_stored_dispose(c: &mut Criterion) {
        let mut group = c.benchmark_group("stored/dispose");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for iteration in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let scope = root.access();
                        let mut stored = Vec::with_capacity(size);
                        for index in 0..size {
                            stored.push(
                                scope
                                    .stored((iteration as u32) ^ (index as u32))
                                    .expect("benchmark stored creation"),
                            );
                        }
                        black_box(stored.len());
                        drop(stored);

                        let start = Instant::now();
                        root.close().expect("benchmark root disposal");
                        total += start.elapsed();
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_stored_update(c: &mut Criterion) {
        let mut group = c.benchmark_group("stored/update");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut stored_values = Vec::with_capacity(size);
                        for index in 0..size {
                            stored_values.push(
                                scope
                                    .stored(index as i32)
                                    .expect("benchmark stored creation"),
                            );
                        }

                        bench.iter(|| {
                            for stored in &stored_values {
                                let result = stored.update(|value| {
                                    *value = black_box(value.wrapping_add(1));
                                });
                                result.expect("benchmark stored update");
                            }
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_node_ref_create(c: &mut Criterion) {
        let mut group = c.benchmark_group("node-ref/create");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let start = Instant::now();

                        root.with_access(|scope| {
                            for _ in 0..size {
                                black_box(
                                    scope
                                        .node_ref::<u32>()
                                        .expect("benchmark node ref creation"),
                                );
                            }
                        });

                        total += start.elapsed();
                        root.close().expect("benchmark root disposal");
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_node_ref_dispose(c: &mut Criterion) {
        let mut group = c.benchmark_group("node-ref/dispose");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("runtime root creation");
                        let scope = root.access();
                        let mut node_refs = Vec::with_capacity(size);
                        for _ in 0..size {
                            node_refs.push(
                                scope
                                    .node_ref::<u32>()
                                    .expect("benchmark node ref creation"),
                            );
                        }
                        black_box(node_refs.len());
                        drop(node_refs);

                        let start = Instant::now();
                        root.close().expect("benchmark root disposal");
                        total += start.elapsed();
                    }

                    total
                });
            });
        }

        group.finish();
    }

    fn bench_node_ref_update(c: &mut Criterion) {
        let mut group = c.benchmark_group("node-ref/update");

        for &size in SIGNAL_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut node_refs = Vec::with_capacity(size);
                        for _ in 0..size {
                            node_refs.push(
                                scope
                                    .node_ref::<u32>()
                                    .expect("benchmark node ref creation"),
                            );
                        }

                        bench.iter(|| {
                            for (index, node_ref) in node_refs.iter().enumerate() {
                                node_ref.set(index as u32).expect("benchmark node ref set");
                                node_ref.clear().expect("benchmark node ref clear");
                            }
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_graph_effect_fanout(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/fanout");

        for &fanout in GRAPH_SIZES {
            group.throughput(Throughput::Elements(fanout as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(fanout),
                &fanout,
                |bench, &fanout| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            let notifications = Rc::new(Cell::new(0usize));

                            for _ in 0..fanout {
                                let notifications = notifications.clone();
                                scope
                                    .effect(
                                        move || {
                                            black_box(source.get().expect("benchmark read"));
                                            notifications.set(notifications.get().wrapping_add(1));
                                            Ok(())
                                        },
                                        handler(scope),
                                    )
                                    .expect("benchmark effect should initialize");
                            }

                            let mut value = 0i32;
                            bench.iter(|| {
                                value = value.wrapping_add(1);
                                set_source
                                    .set(black_box(value))
                                    .expect("benchmark signal update");
                                black_box(notifications.get());
                            });
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_memo_chain(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/chain");

        for &depth in GRAPH_SIZES {
            group.throughput(Throughput::Elements(depth as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(depth),
                &depth,
                |bench, &depth| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            let first_source = source;
                            let mut tail = scope
                                .computed(
                                    move || {
                                        Ok(first_source
                                            .get()
                                            .expect("benchmark read")
                                            .wrapping_add(1))
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark memo creation");

                            for _ in 1..depth {
                                let upstream = tail;
                                tail = scope
                                    .computed(
                                        move || {
                                            Ok(upstream
                                                .get()
                                                .expect("benchmark read")
                                                .wrapping_add(1))
                                        },
                                        handler(scope),
                                    )
                                    .expect("benchmark memo creation");
                            }

                            let observed = Rc::new(Cell::new(0i32));
                            let observed_in_effect = observed.clone();
                            let tail_in_effect = tail;
                            scope
                                .effect(
                                    move || {
                                        observed_in_effect
                                            .set(tail_in_effect.get().expect("benchmark read"));
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect should initialize");

                            let mut value = 0i32;
                            bench.iter(|| {
                                value = value.wrapping_add(1);
                                set_source
                                    .set(black_box(value))
                                    .expect("benchmark signal update");
                                black_box(observed.get());
                            });
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_memo_diamond(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/diamond");

        for &width in GRAPH_SIZES {
            group.throughput(Throughput::Elements(width as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(width),
                &width,
                |bench, &width| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            let mut memos = Vec::with_capacity(width);
                            for index in 0..width {
                                memos.push(
                                    scope
                                        .computed(
                                            move || {
                                                Ok(source
                                                    .get()
                                                    .expect("benchmark read")
                                                    .wrapping_add(index as i32))
                                            },
                                            handler(scope),
                                        )
                                        .expect("benchmark memo creation"),
                                );
                            }

                            let observed = Rc::new(Cell::new(0i32));
                            let memos_in_effect = memos.clone();
                            let observed_in_effect = observed.clone();
                            scope
                                .effect(
                                    move || {
                                        let mut value = 0i32;
                                        for memo in &memos_in_effect {
                                            value = value
                                                .wrapping_add(memo.get().expect("benchmark read"));
                                        }
                                        observed_in_effect.set(value);
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect should initialize");

                            let mut value = 0i32;
                            bench.iter(|| {
                                value = value.wrapping_add(1);
                                set_source
                                    .set(black_box(value))
                                    .expect("benchmark signal update");
                                black_box(observed.get());
                            });
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_effect_fanout_cross_scope(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/cross-scope-fanout");

        for &fanout in GRAPH_SIZES {
            group.throughput(Throughput::Elements(fanout as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(fanout),
                &fanout,
                |bench, &fanout| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            scope
                                .with_transient(|child| {
                                    let notifications = Rc::new(Cell::new(0usize));
                                    for _ in 0..fanout {
                                        let notifications = notifications.clone();
                                        child
                                            .effect(
                                                move || {
                                                    black_box(
                                                        source.get().expect("benchmark read"),
                                                    );
                                                    notifications
                                                        .set(notifications.get().wrapping_add(1));
                                                    Ok(())
                                                },
                                                handler(scope),
                                            )
                                            .expect("benchmark effect should initialize");
                                    }

                                    let mut value = 0i32;
                                    bench.iter(|| {
                                        value = value.wrapping_add(1);
                                        set_source
                                            .set(black_box(value))
                                            .expect("benchmark signal update");
                                        black_box(notifications.get());
                                    });
                                })
                                .expect("benchmark scope execution");
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_memo_chain_cross_scope(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/cross-scope-chain");

        for &depth in GRAPH_SIZES {
            group.throughput(Throughput::Elements(depth as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(depth),
                &depth,
                |bench, &depth| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            scope
                                .with_transient(|child| {
                                    let mut tail = child
                                        .computed(
                                            move || {
                                                Ok(source
                                                    .get()
                                                    .expect("benchmark read")
                                                    .wrapping_add(1))
                                            },
                                            handler(child),
                                        )
                                        .expect("benchmark memo creation");
                                    for _ in 1..depth {
                                        let upstream = tail;
                                        tail = child
                                            .computed(
                                                move || {
                                                    Ok(upstream
                                                        .get()
                                                        .expect("benchmark read")
                                                        .wrapping_add(1))
                                                },
                                                handler(child),
                                            )
                                            .expect("benchmark memo creation");
                                    }

                                    let observed = Rc::new(Cell::new(0i32));
                                    let observed_in_effect = observed.clone();
                                    let tail_in_effect = tail;
                                    child
                                        .effect(
                                            move || {
                                                observed_in_effect.set(
                                                    tail_in_effect.get().expect("benchmark read"),
                                                );
                                                Ok(())
                                            },
                                            handler(scope),
                                        )
                                        .expect("benchmark effect should initialize");

                                    let mut value = 0i32;
                                    bench.iter(|| {
                                        value = value.wrapping_add(1);
                                        set_source
                                            .set(black_box(value))
                                            .expect("benchmark signal update");
                                        black_box(observed.get());
                                    });
                                })
                                .expect("benchmark scope execution");
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_memo_diamond_cross_scope(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/propagation-frontier/cross-scope-diamond");

        for &width in GRAPH_SIZES {
            group.throughput(Throughput::Elements(width as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(width),
                &width,
                |bench, &width| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            scope
                                .with_transient(|child| {
                                    let mut memos = Vec::with_capacity(width);
                                    for index in 0..width {
                                        memos.push(
                                            child
                                                .computed(
                                                    move || {
                                                        Ok(source
                                                            .get()
                                                            .expect("benchmark read")
                                                            .wrapping_add(index as i32))
                                                    },
                                                    handler(child),
                                                )
                                                .expect("benchmark memo creation"),
                                        );
                                    }

                                    let observed = Rc::new(Cell::new(0i32));
                                    let memos_in_effect = memos.clone();
                                    let observed_in_effect = observed.clone();
                                    child
                                        .effect(
                                            move || {
                                                let mut value = 0i32;
                                                for memo in &memos_in_effect {
                                                    value = value.wrapping_add(
                                                        memo.get().expect("benchmark read"),
                                                    );
                                                }
                                                observed_in_effect.set(value);
                                                Ok(())
                                            },
                                            handler(scope),
                                        )
                                        .expect("benchmark effect should initialize");

                                    let mut value = 0i32;
                                    bench.iter(|| {
                                        value = value.wrapping_add(1);
                                        set_source
                                            .set(black_box(value))
                                            .expect("benchmark signal update");
                                        black_box(observed.get());
                                    });
                                })
                                .expect("benchmark scope execution");
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_graph_memo_equal_write(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/memo-equal-write");

        for &width in GRAPH_SIZES {
            group.throughput(Throughput::Elements(width as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(width),
                &width,
                |bench, &width| {
                    let mut runtime = Runtime::new();
                    runtime
                        .with_transient(|scope| {
                            let (source, set_source) =
                                scope.signal(0i32).expect("fallible reactive creation");
                            let memo_runs = Rc::new(Cell::new(0usize));
                            let mut memos = Vec::with_capacity(width);
                            for _ in 0..width {
                                let memo_runs = memo_runs.clone();
                                memos.push(
                                    scope
                                        .computed(
                                            move || {
                                                memo_runs.set(memo_runs.get().wrapping_add(1));
                                                Ok(source.get().expect("benchmark read"))
                                            },
                                            handler(scope),
                                        )
                                        .expect("benchmark memo creation"),
                                );
                            }

                            let effect_runs = Rc::new(Cell::new(0usize));
                            let memos_in_effect = memos.clone();
                            let effect_runs_in_effect = effect_runs.clone();
                            scope
                                .effect(
                                    move || {
                                        for memo in &memos_in_effect {
                                            black_box(memo.get().expect("benchmark read"));
                                        }
                                        effect_runs_in_effect
                                            .set(effect_runs_in_effect.get().wrapping_add(1));
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect should initialize");

                            bench.iter_custom(|iterations| {
                                let start = Instant::now();
                                for _ in 0..iterations {
                                    set_source.set(0).expect("benchmark signal update");
                                    black_box((memo_runs.get(), effect_runs.get()));
                                }
                                let elapsed = start.elapsed();
                                assert_eq!(
                                    effect_runs.get(),
                                    1,
                                    "equal memo writes must not rerun the effect",
                                );
                                assert_eq!(
                                    memo_runs.get(),
                                    width * (iterations as usize + 1),
                                    "equal memo writes must still reevaluate each memo",
                                );
                                elapsed
                            });
                        })
                        .expect("benchmark scope execution");
                },
            );
        }

        group.finish();
    }

    fn bench_dependency_create(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/dependency-create");

        for &size in DEPENDENCY_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for iteration in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("benchmark root creation");
                        let start = Instant::now();
                        root.with_access(|scope| {
                            let reads: Vec<_> = (0..size)
                                .map(|index| {
                                    scope
                                        .signal((iteration as i32) ^ (index as i32))
                                        .expect("benchmark signal creation")
                                        .0
                                })
                                .collect();
                            let reads_in_effect = reads.clone();
                            scope
                                .effect(
                                    move || {
                                        for read in &reads_in_effect {
                                            black_box(read.get().expect("benchmark read"));
                                        }
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect creation");
                            black_box(reads.len());
                        });
                        total += start.elapsed();
                        root.close().expect("benchmark root disposal");
                    }
                    total
                });
            });
        }

        group.finish();
    }

    fn bench_dependency_rerun(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/dependency-rerun");

        for &size in DEPENDENCY_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let mut signals = Vec::with_capacity(size);
                        for index in 0..size {
                            signals.push(
                                scope
                                    .signal(index as i32)
                                    .expect("benchmark signal creation"),
                            );
                        }
                        let reads: Vec<_> = signals.iter().map(|(read, _)| *read).collect();
                        let trigger = signals[0].1;
                        scope
                            .effect(
                                move || {
                                    for read in &reads {
                                        black_box(read.get().expect("benchmark read"));
                                    }
                                    Ok(())
                                },
                                handler(scope),
                            )
                            .expect("benchmark effect creation");

                        let mut value = 0i32;
                        bench.iter(|| {
                            value = value.wrapping_add(1);
                            trigger
                                .set(black_box(value))
                                .expect("benchmark signal update");
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_dependency_switch(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/dependency-rebind");

        for &size in DEPENDENCY_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                runtime
                    .with_transient(|scope| {
                        let (switch, set_switch) =
                            scope.signal(false).expect("benchmark switch creation");
                        let mut left = Vec::with_capacity(size);
                        let mut right = Vec::with_capacity(size);
                        for index in 0..size {
                            left.push(
                                scope
                                    .signal(index as i32)
                                    .expect("benchmark left signal creation")
                                    .0,
                            );
                            right.push(
                                scope
                                    .signal((index as i32).wrapping_neg())
                                    .expect("benchmark right signal creation")
                                    .0,
                            );
                        }
                        scope
                            .effect(
                                move || {
                                    let reads = if switch.get().expect("benchmark switch read") {
                                        &right
                                    } else {
                                        &left
                                    };
                                    for read in reads {
                                        black_box(read.get().expect("benchmark dependency read"));
                                    }
                                    Ok(())
                                },
                                handler(scope),
                            )
                            .expect("benchmark effect creation");

                        let mut selected = false;
                        bench.iter(|| {
                            selected = !selected;
                            set_switch
                                .set(black_box(selected))
                                .expect("benchmark switch update");
                        });
                    })
                    .expect("benchmark scope execution");
            });
        }

        group.finish();
    }

    fn bench_dependency_dispose(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/dependency-dispose");

        for &size in DEPENDENCY_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                bench.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        let mut runtime = Runtime::new();
                        let root = runtime.owner().expect("benchmark root creation");
                        root.with_access(|scope| {
                            let reads: Vec<_> = (0..size)
                                .map(|index| {
                                    scope
                                        .signal(index as i32)
                                        .expect("benchmark signal creation")
                                        .0
                                })
                                .collect();
                            let reads_in_effect = reads.clone();
                            scope
                                .effect(
                                    move || {
                                        for read in &reads_in_effect {
                                            black_box(read.get().expect("benchmark read"));
                                        }
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect creation");
                            black_box(reads.len());
                        });

                        let start = Instant::now();
                        root.close().expect("benchmark root disposal");
                        total += start.elapsed();
                    }
                    total
                });
            });
        }

        group.finish();
    }

    fn bench_dependency_dispose_cross_scope(c: &mut Criterion) {
        let mut group = c.benchmark_group("graph/dispose-cross-scope");

        for &size in DEPENDENCY_SIZES {
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
                let mut runtime = Runtime::new();
                let root = runtime.owner().expect("benchmark root creation");
                root.with_access(|scope| {
                    let mut signals = Vec::with_capacity(size);
                    for index in 0..size {
                        signals.push(
                            scope
                                .signal(index as i32)
                                .expect("benchmark signal creation")
                                .0,
                        );
                    }
                    bench.iter_custom(|iterations| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iterations {
                            let owner = scope.create_child().expect("benchmark owner creation");
                            let child = owner.create_child().expect("benchmark child creation");
                            let reads_in_effect = signals.clone();
                            child
                                .access()
                                .effect(
                                    move || {
                                        for read in &reads_in_effect {
                                            black_box(read.get().expect("benchmark read"));
                                        }
                                        Ok(())
                                    },
                                    handler(scope),
                                )
                                .expect("benchmark effect creation");

                            let start = Instant::now();
                            child.close().expect("benchmark child disposal");
                            owner.close().expect("benchmark owner disposal");
                            total += start.elapsed();
                        }
                        total
                    });
                });
                root.close().expect("benchmark root disposal");
            });
        }

        group.finish();
    }

    fn bench_owner_slot_churn(c: &mut Criterion) {
        let mut group = c.benchmark_group("owner/slot-churn");

        for &owners in OWNER_SIZES {
            group.throughput(Throughput::Elements(owners as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(owners),
                &owners,
                |bench, &owners| {
                    let mut runtime = Runtime::new();
                    let root = runtime.owner().expect("runtime root creation");
                    root.with_access(|scope| {
                        bench.iter(|| {
                            for _ in 0..owners {
                                let owner = scope.create_child().expect("benchmark owner creation");
                                owner.close().expect("benchmark owner disposal");
                            }
                        });
                    });
                    root.close().expect("runtime root disposal");
                },
            );
        }

        group.finish();
    }

    fn bench_owner_full_churn(c: &mut Criterion) {
        let mut group = c.benchmark_group("owner/full-churn");

        for &rows in OWNER_SIZES {
            group.throughput(Throughput::Elements(rows as u64));
            group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |bench, &rows| {
                let mut runtime = Runtime::new();
                let root = runtime.owner().expect("runtime root creation");
                root.with_access(|scope| {
                    let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
                    let cleanup_count = Rc::new(Cell::new(0usize));
                    let effect_token = handler(scope);
                    let effect_handler = effect_token.view();
                    let cleanup_token = handler(scope);
                    let cleanup_handler = cleanup_token.view();
                    let mut owners = Vec::with_capacity(rows);

                    for _ in 0..rows {
                        let row_scope = scope.create_child().expect("fallible reactive creation");
                        let render_scope = row_scope.create_child().expect("benchmark child scope");
                        let source_in_effect = source;
                        render_scope
                            .access()
                            .effect(
                                move || {
                                    black_box(source_in_effect.get().expect("benchmark read"));
                                    Ok(())
                                },
                                effect_handler,
                            )
                            .expect("benchmark effect should initialize");
                        let cleanup_count_in_row = cleanup_count.clone();
                        render_scope
                            .access()
                            .on_cleanup(
                                move || {
                                    cleanup_count_in_row
                                        .set(cleanup_count_in_row.get().wrapping_add(1));
                                    Ok(())
                                },
                                cleanup_handler,
                            )
                            .expect("benchmark cleanup should register");
                        owners.push((row_scope, render_scope));
                    }

                    bench.iter(|| {
                        for (row_scope, render_scope) in owners.drain(..) {
                            render_scope.close().expect("benchmark render disposal");
                            row_scope.close().expect("benchmark row disposal");
                        }

                        for _ in 0..rows {
                            let row_scope =
                                scope.create_child().expect("fallible reactive creation");
                            let render_scope =
                                row_scope.create_child().expect("benchmark child scope");
                            let source_in_effect = source;
                            render_scope
                                .access()
                                .effect(
                                    move || {
                                        black_box(source_in_effect.get().expect("benchmark read"));
                                        Ok(())
                                    },
                                    effect_handler,
                                )
                                .expect("benchmark effect should initialize");
                            let cleanup_count_in_row = cleanup_count.clone();
                            render_scope
                                .access()
                                .on_cleanup(
                                    move || {
                                        cleanup_count_in_row
                                            .set(cleanup_count_in_row.get().wrapping_add(1));
                                        Ok(())
                                    },
                                    cleanup_handler,
                                )
                                .expect("benchmark cleanup should register");
                            owners.push((row_scope, render_scope));
                        }

                        black_box(cleanup_count.get());
                    });

                    for (row_scope, render_scope) in owners.drain(..) {
                        render_scope.close().expect("benchmark render disposal");
                        row_scope.close().expect("benchmark row disposal");
                    }
                    black_box(cleanup_count.get());
                });
                root.close().expect("benchmark root disposal");
            });
        }

        group.finish();
    }

    fn bench_completion_message(c: &mut Criterion) {
        let mut group = c.benchmark_group("proxy/completion-message");
        group.throughput(Throughput::Elements(1));

        group.bench_function("u32", |bench| {
            let mut runtime = Runtime::new();
            runtime
                .with_transient(|scope| {
                    let value = scope.rw_signal(0u32).expect("fallible reactive creation");
                    let setter = value.write();
                    let token = scope
                        .completion_sender(unwind_safe(move |message: u32| {
                            setter.set(message).expect("benchmark signal update");
                            Ok::<(), ()>(())
                        }))
                        .expect("benchmark completion creation");
                    let mut message = 0u32;

                    bench.iter(|| {
                        message = message.wrapping_add(1);
                        black_box(token.submit(message).expect("completion submit"));
                    });
                })
                .expect("benchmark scope execution");
        });

        group.bench_function("String", |bench| {
            let mut runtime = Runtime::new();
            runtime
                .with_transient(|scope| {
                    let messages = scope
                        .rw_signal(Vec::<String>::with_capacity(64))
                        .expect("fallible reactive creation");
                    let setter = messages.write();
                    let token = scope
                        .completion_sender(unwind_safe(move |message: String| {
                            setter
                                .update(|buffer| {
                                    buffer.push(message);
                                    if buffer.len() > 64 {
                                        buffer.drain(..buffer.len() - 64);
                                    }
                                })
                                .expect("benchmark signal update");
                            Ok::<(), ()>(())
                        }))
                        .expect("benchmark completion creation");

                    bench.iter(|| {
                        black_box(
                            token
                                .submit(String::from("message"))
                                .expect("completion submit"),
                        );
                    });
                })
                .expect("benchmark scope execution");
        });

        group.finish();
    }

    fn criterion_config() -> Criterion {
        Criterion::default()
            .warm_up_time(Duration::from_secs(3))
            .measurement_time(Duration::from_secs(5))
            .sample_size(100)
            .configure_from_args()
    }

    criterion_group! {
        name = benches;
        config = criterion_config();
        targets =
            bench_signal_create,
            bench_signal_read_untracked,
            bench_signal_read_tracked,
            bench_signal_write,
            bench_stored_create,
            bench_stored_dispose,
            bench_stored_update,
            bench_node_ref_create,
            bench_node_ref_dispose,
            bench_node_ref_update,
            bench_graph_effect_fanout,
            bench_graph_memo_chain,
            bench_graph_memo_diamond,
            bench_graph_effect_fanout_cross_scope,
            bench_graph_memo_chain_cross_scope,
            bench_graph_memo_diamond_cross_scope,
            bench_graph_memo_equal_write,
            bench_dependency_create,
            bench_dependency_rerun,
            bench_dependency_switch,
            bench_dependency_dispose,
            bench_dependency_dispose_cross_scope,
            bench_owner_slot_churn,
            bench_owner_full_churn,
            bench_signal_create_heap,
            bench_completion_message,
            scoped_signal_round_trip,
    }
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(native::benches);
