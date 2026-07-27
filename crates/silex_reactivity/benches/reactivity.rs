#[cfg(not(target_arch = "wasm32"))]
mod native {
    use criterion::Criterion;
    use silex_reactivity::{RawId, SignalId, memo, scope, signal, store};
    use std::hint::black_box;

    fn create_memo_chain(source: SignalId, length: usize) -> RawId {
        let mut upstream = source.raw();
        for _ in 0..length {
            let input = upstream;
            upstream =
                memo::create::<u64, _>(move |_| signal::get::<u64>(input).unwrap_or_default() + 1)
                    .raw();
        }
        upstream
    }

    pub fn signal_read(c: &mut Criterion) {
        let (owner, signal_id) = scope::create_detached(|| signal::create(1u64));
        c.bench_function("signal_read", |b| {
            b.iter(|| black_box(signal::get::<u64>(signal_id).unwrap()))
        });
        scope::dispose(owner);
    }

    pub fn memo_chain(c: &mut Criterion) {
        let (source_owner, source) = scope::create_detached(|| signal::create(0u64));
        let (memo_owner, tail) = scope::create_detached(|| create_memo_chain(source, 10));

        c.bench_function("memo_chain_10", |b| {
            b.iter(|| {
                signal::update::<u64>(source, |value| *value = value.wrapping_add(1));
                black_box(signal::get::<u64>(tail).unwrap())
            })
        });

        scope::dispose(memo_owner);
        scope::dispose(source_owner);
    }

    pub fn memo_fanout(c: &mut Criterion) {
        let (source_owner, source) = scope::create_detached(|| signal::create(0u64));
        let (memo_owner, subscribers) = scope::create_detached(|| {
            (0..100)
                .map(|_| create_memo_chain(source, 1))
                .collect::<Vec<_>>()
        });

        c.bench_function("memo_fanout_100", |b| {
            b.iter(|| {
                signal::update::<u64>(source, |value| *value = value.wrapping_add(1));
                for &subscriber in &subscribers {
                    black_box(signal::get::<u64>(subscriber).unwrap());
                }
            })
        });

        scope::dispose(memo_owner);
        scope::dispose(source_owner);
    }

    pub fn scope_lifecycle(c: &mut Criterion) {
        c.bench_function("scope_dispose_1000_stored", |b| {
            b.iter(|| {
                let (owner, ()) = scope::create_detached(|| {
                    for _ in 0..1000 {
                        black_box(store::create(()));
                    }
                });
                scope::dispose(owner);
            })
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_group!(
    benches,
    native::signal_read,
    native::memo_chain,
    native::memo_fanout,
    native::scope_lifecycle,
);

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(benches);

#[cfg(target_arch = "wasm32")]
fn main() {}
