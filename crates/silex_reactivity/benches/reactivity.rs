use criterion::{Criterion, criterion_group, criterion_main};
use silex_reactivity::Runtime;
use std::hint::black_box;

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

criterion_group!(benches, scoped_signal_round_trip);
criterion_main!(benches);
