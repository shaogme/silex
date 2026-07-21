use criterion::{Criterion, criterion_group, criterion_main};
use silex_thread_local::ThreadLocal;
use std::{hint::black_box, sync::Arc, thread};

fn bench_single_thread_get(c: &mut Criterion) {
    let tl = ThreadLocal::new();
    let _ = tl.get_or(|| 42);

    c.bench_function("single_thread_get", |b| {
        b.iter(|| {
            let val = tl.get().unwrap();
            black_box(val);
        })
    });
}

fn bench_single_thread_get_or(c: &mut Criterion) {
    let tl = ThreadLocal::new();

    c.bench_function("single_thread_get_or", |b| {
        b.iter(|| {
            let val = tl.get_or(|| 42);
            black_box(val);
        })
    });
}

fn bench_multi_thread_get(c: &mut Criterion) {
    let tl = Arc::new(ThreadLocal::new());
    let _ = tl.get_or(|| 42);

    c.bench_function("multi_thread_get", |b| {
        b.iter(|| {
            let tl_clone = tl.clone();
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let val = tl_clone.get_or(|| 42);
                    black_box(val);
                }
            });
            handle.join().unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_single_thread_get,
    bench_single_thread_get_or,
    bench_multi_thread_get
);
criterion_main!(benches);
