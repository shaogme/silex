use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use silex_reactivity::Runtime;
use std::{
    cell::Cell,
    hint::black_box,
    rc::Rc,
    time::{Duration, Instant},
};

const SIGNAL_SIZES: &[usize] = &[1, 64, 1024, 4096];
const GRAPH_SIZES: &[usize] = &[1, 8, 32, 128];
const OWNER_SIZES: &[usize] = &[1, 16, 128, 512];

fn scoped_signal_round_trip(c: &mut Criterion) {
    c.bench_function("scoped signal round trip", |bench| {
        bench.iter(|| {
            let mut runtime = Runtime::new();
            let root = runtime.run();
            {
                let scope = root.scope();
                let (read, write) = scope.signal(0i32);
                let _effect = scope.effect(move || {
                    black_box(read.get());
                });
                write.set(black_box(1));
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
                    let root = runtime.run();
                    let start = Instant::now();

                    root.with_scope(|scope| {
                        for index in 0..size {
                            let signal = scope.signal((iteration as i32) ^ (index as i32));
                            black_box(signal);
                        }
                    });

                    total += start.elapsed();
                    root.dispose().expect("benchmark root disposal");
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
                    let root = runtime.run();
                    let start = Instant::now();

                    root.with_scope(|scope| {
                        for index in 0..size {
                            let value = [(iteration as u8) ^ (index as u8); 32];
                            black_box(scope.signal(value));
                        }
                    });

                    total += start.elapsed();
                    root.dispose().expect("benchmark root disposal");
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
            runtime.child(|scope| {
                let mut signals = Vec::with_capacity(size);
                for index in 0..size {
                    signals.push(scope.signal(index as i32).0);
                }

                bench.iter(|| {
                    let mut observed = 0i32;
                    for signal in &signals {
                        observed = observed.wrapping_add(black_box(signal.get_untracked()));
                    }
                    black_box(observed);
                });
            });
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
            runtime.child(|scope| {
                let mut signals = Vec::with_capacity(size);
                for index in 0..size {
                    signals.push(scope.signal(index as i32));
                }

                let reads: Vec<_> = signals.iter().map(|(read, _)| *read).collect();
                let trigger = signals[0].1;
                let runs = Rc::new(Cell::new(0usize));
                let reads_in_effect = reads.clone();
                let runs_in_effect = runs.clone();
                let _effect = scope.effect(move || {
                    let mut observed = 0i32;
                    for signal in &reads_in_effect {
                        observed = observed.wrapping_add(black_box(signal.get()));
                    }
                    black_box(observed);
                    runs_in_effect.set(runs_in_effect.get().wrapping_add(1));
                });

                let mut value = 0i32;
                bench.iter(|| {
                    value = value.wrapping_add(1);
                    trigger.set(black_box(value));
                    black_box(runs.get());
                });
            });
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
            runtime.child(|scope| {
                let mut signals = Vec::with_capacity(size);
                for index in 0..size {
                    signals.push(scope.signal(index as i32).1);
                }

                bench.iter(|| {
                    for signal in &signals {
                        let result = signal.try_update(|value| {
                            *value = black_box(value.wrapping_add(1));
                        });
                        result.expect("benchmark signal update");
                    }
                });
            });
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
                    let root = runtime.run();
                    let start = Instant::now();

                    root.with_scope(|scope| {
                        for index in 0..size {
                            let stored = scope.stored((iteration as u32) ^ (index as u32));
                            black_box(stored);
                        }
                    });

                    total += start.elapsed();
                    root.dispose().expect("benchmark root disposal");
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
                    let root = runtime.run();
                    let stored = root.with_scope(|scope| {
                        let mut stored = Vec::with_capacity(size);
                        for index in 0..size {
                            stored.push(scope.stored((iteration as u32) ^ (index as u32)));
                        }
                        stored
                    });
                    black_box(stored.len());
                    drop(stored);

                    let start = Instant::now();
                    root.dispose().expect("benchmark root disposal");
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
            runtime.child(|scope| {
                let mut stored_values = Vec::with_capacity(size);
                for index in 0..size {
                    stored_values.push(scope.stored(index as i32));
                }

                bench.iter(|| {
                    for stored in &stored_values {
                        let result = stored.try_update(|value| {
                            *value = black_box(value.wrapping_add(1));
                        });
                        result.expect("benchmark stored update");
                    }
                });
            });
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
                    let root = runtime.run();
                    let start = Instant::now();

                    root.with_scope(|scope| {
                        for _ in 0..size {
                            black_box(scope.node_ref::<u32>());
                        }
                    });

                    total += start.elapsed();
                    root.dispose().expect("benchmark root disposal");
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
                    let root = runtime.run();
                    let node_refs = root.with_scope(|scope| {
                        let mut node_refs = Vec::with_capacity(size);
                        for _ in 0..size {
                            node_refs.push(scope.node_ref::<u32>());
                        }
                        node_refs
                    });
                    black_box(node_refs.len());
                    drop(node_refs);

                    let start = Instant::now();
                    root.dispose().expect("benchmark root disposal");
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
            runtime.child(|scope| {
                let mut node_refs = Vec::with_capacity(size);
                for _ in 0..size {
                    node_refs.push(scope.node_ref::<u32>());
                }

                bench.iter(|| {
                    for (index, node_ref) in node_refs.iter().enumerate() {
                        node_ref.set(index as u32).expect("benchmark node ref set");
                        node_ref.clear().expect("benchmark node ref clear");
                    }
                });
            });
        });
    }

    group.finish();
}

fn bench_graph_effect_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/effect-fanout");

    for &fanout in GRAPH_SIZES {
        group.throughput(Throughput::Elements(fanout as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(fanout),
            &fanout,
            |bench, &fanout| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    let notifications = Rc::new(Cell::new(0usize));

                    for _ in 0..fanout {
                        let notifications = notifications.clone();
                        scope.effect(move || {
                            black_box(source.get());
                            notifications.set(notifications.get().wrapping_add(1));
                        });
                    }

                    let mut value = 0i32;
                    bench.iter(|| {
                        value = value.wrapping_add(1);
                        set_source.set(black_box(value));
                        black_box(notifications.get());
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_memo_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/memo-chain");

    for &depth in GRAPH_SIZES {
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(depth),
            &depth,
            |bench, &depth| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    let first_source = source;
                    let mut tail = scope.memo(move |_| first_source.get().wrapping_add(1));

                    for _ in 1..depth {
                        let upstream = tail;
                        tail = scope.memo(move |_| upstream.get().wrapping_add(1));
                    }

                    let observed = Rc::new(Cell::new(0i32));
                    let observed_in_effect = observed.clone();
                    let tail_in_effect = tail;
                    scope.effect(move || {
                        observed_in_effect.set(tail_in_effect.get());
                    });

                    let mut value = 0i32;
                    bench.iter(|| {
                        value = value.wrapping_add(1);
                        set_source.set(black_box(value));
                        black_box(observed.get());
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_memo_diamond(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/memo-diamond");

    for &width in GRAPH_SIZES {
        group.throughput(Throughput::Elements(width as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(width),
            &width,
            |bench, &width| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    let mut memos = Vec::with_capacity(width);
                    for index in 0..width {
                        memos.push(scope.memo(move |_| source.get().wrapping_add(index as i32)));
                    }

                    let observed = Rc::new(Cell::new(0i32));
                    let memos_in_effect = memos.clone();
                    let observed_in_effect = observed.clone();
                    scope.effect(move || {
                        let mut value = 0i32;
                        for memo in &memos_in_effect {
                            value = value.wrapping_add(memo.get());
                        }
                        observed_in_effect.set(value);
                    });

                    let mut value = 0i32;
                    bench.iter(|| {
                        value = value.wrapping_add(1);
                        set_source.set(black_box(value));
                        black_box(observed.get());
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_effect_fanout_cross_scope(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/effect-fanout-cross-scope");

    for &fanout in GRAPH_SIZES {
        group.throughput(Throughput::Elements(fanout as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(fanout),
            &fanout,
            |bench, &fanout| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    scope.child(|child| {
                        let notifications = Rc::new(Cell::new(0usize));
                        for _ in 0..fanout {
                            let notifications = notifications.clone();
                            child.effect(move || {
                                black_box(source.get());
                                notifications.set(notifications.get().wrapping_add(1));
                            });
                        }

                        let mut value = 0i32;
                        bench.iter(|| {
                            value = value.wrapping_add(1);
                            set_source.set(black_box(value));
                            black_box(notifications.get());
                        });
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_memo_chain_cross_scope(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/memo-chain-cross-scope");

    for &depth in GRAPH_SIZES {
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(depth),
            &depth,
            |bench, &depth| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    scope.child(|child| {
                        let mut tail = child.memo(move |_| source.get().wrapping_add(1));
                        for _ in 1..depth {
                            let upstream = tail;
                            tail = child.memo(move |_| upstream.get().wrapping_add(1));
                        }

                        let observed = Rc::new(Cell::new(0i32));
                        let observed_in_effect = observed.clone();
                        let tail_in_effect = tail;
                        child.effect(move || {
                            observed_in_effect.set(tail_in_effect.get());
                        });

                        let mut value = 0i32;
                        bench.iter(|| {
                            value = value.wrapping_add(1);
                            set_source.set(black_box(value));
                            black_box(observed.get());
                        });
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_memo_diamond_cross_scope(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/memo-diamond-cross-scope");

    for &width in GRAPH_SIZES {
        group.throughput(Throughput::Elements(width as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(width),
            &width,
            |bench, &width| {
                let mut runtime = Runtime::new();
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    scope.child(|child| {
                        let mut memos = Vec::with_capacity(width);
                        for index in 0..width {
                            memos
                                .push(child.memo(move |_| source.get().wrapping_add(index as i32)));
                        }

                        let observed = Rc::new(Cell::new(0i32));
                        let memos_in_effect = memos.clone();
                        let observed_in_effect = observed.clone();
                        child.effect(move || {
                            let mut value = 0i32;
                            for memo in &memos_in_effect {
                                value = value.wrapping_add(memo.get());
                            }
                            observed_in_effect.set(value);
                        });

                        let mut value = 0i32;
                        bench.iter(|| {
                            value = value.wrapping_add(1);
                            set_source.set(black_box(value));
                            black_box(observed.get());
                        });
                    });
                });
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
                runtime.child(|scope| {
                    let (source, set_source) = scope.signal(0i32);
                    let memo_runs = Rc::new(Cell::new(0usize));
                    let mut memos = Vec::with_capacity(width);
                    for _ in 0..width {
                        let memo_runs = memo_runs.clone();
                        memos.push(scope.memo(move |_| {
                            memo_runs.set(memo_runs.get().wrapping_add(1));
                            source.get()
                        }));
                    }

                    let effect_runs = Rc::new(Cell::new(0usize));
                    let memos_in_effect = memos.clone();
                    let effect_runs_in_effect = effect_runs.clone();
                    scope.effect(move || {
                        for memo in &memos_in_effect {
                            black_box(memo.get());
                        }
                        effect_runs_in_effect.set(effect_runs_in_effect.get().wrapping_add(1));
                    });

                    bench.iter_custom(|iterations| {
                        let start = Instant::now();
                        for _ in 0..iterations {
                            set_source.set(0);
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
                });
            },
        );
    }

    group.finish();
}

fn bench_owner_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("owner/churn");

    for &rows in OWNER_SIZES {
        group.throughput(Throughput::Elements(rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |bench, &rows| {
            let mut runtime = Runtime::new();
            let root = runtime.run();
            root.with_scope(|scope| {
                let (source, _) = scope.signal(0i32);
                let cleanup_count = Rc::new(Cell::new(0usize));
                let mut owners = Vec::with_capacity(rows);

                for _ in 0..rows {
                    let row_scope = scope.owned_scope();
                    let render_scope = row_scope.child();
                    let source_in_effect = source;
                    render_scope.effect(move || {
                        black_box(source_in_effect.get());
                    });
                    let cleanup_count_in_row = cleanup_count.clone();
                    render_scope.on_cleanup(move || {
                        cleanup_count_in_row.set(cleanup_count_in_row.get().wrapping_add(1));
                    });
                    owners.push((row_scope, render_scope));
                }

                bench.iter(|| {
                    for (row_scope, render_scope) in owners.drain(..) {
                        render_scope.dispose();
                        row_scope.dispose();
                    }

                    for _ in 0..rows {
                        let row_scope = scope.owned_scope();
                        let render_scope = row_scope.child();
                        let source_in_effect = source;
                        render_scope.effect(move || {
                            black_box(source_in_effect.get());
                        });
                        let cleanup_count_in_row = cleanup_count.clone();
                        render_scope.on_cleanup(move || {
                            cleanup_count_in_row.set(cleanup_count_in_row.get().wrapping_add(1));
                        });
                        owners.push((row_scope, render_scope));
                    }

                    black_box(cleanup_count.get());
                });

                for (row_scope, render_scope) in owners.drain(..) {
                    render_scope.dispose();
                    row_scope.dispose();
                }
                black_box(cleanup_count.get());
            });
            root.dispose().expect("benchmark root disposal");
        });
    }

    group.finish();
}

fn bench_completion_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("proxy/completion-message");
    group.throughput(Throughput::Elements(1));

    group.bench_function("u32", |bench| {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let value = scope.rw_signal(0u32);
            let setter = value.write();
            let token = scope.completion_sender(move |message: u32| setter.set(message));
            let mut message = 0u32;

            bench.iter(|| {
                message = message.wrapping_add(1);
                black_box(token.submit(message));
            });
        });
    });

    group.bench_function("String", |bench| {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let messages = scope.rw_signal(Vec::<String>::with_capacity(64));
            let setter = messages.write();
            let token = scope.completion_sender(move |message: String| {
                setter.update(|buffer| {
                    buffer.push(message);
                    if buffer.len() > 64 {
                        buffer.drain(..buffer.len() - 64);
                    }
                });
            });

            bench.iter(|| {
                black_box(token.submit(String::from("message")));
            });
        });
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
        bench_owner_churn,
        bench_signal_create_heap,
        bench_completion_message,
        scoped_signal_round_trip,
}
criterion_main!(benches);
