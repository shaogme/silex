use silex_thread_local::ThreadLocal;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

#[test]
fn test_basic_get_and_set() {
    let tl = ThreadLocal::new();
    assert!(tl.get().is_none());

    let val = tl.get_or(|| 42);
    assert_eq!(*val, 42);

    assert_eq!(tl.get(), Some(&42));
}

#[test]
fn test_multi_threaded_isolation() {
    let tl = Arc::new(ThreadLocal::new());
    let mut handles = vec![];

    for i in 0..10 {
        let tl_clone = tl.clone();
        let handle = thread::spawn(move || {
            assert!(tl_clone.get().is_none());
            let val = tl_clone.get_or(move || i * 10);
            assert_eq!(*val, i * 10);
            assert_eq!(tl_clone.get(), Some(&(i * 10)));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_drop_behavior() {
    let drop_counter = Arc::new(AtomicUsize::new(0));

    struct TrackDrop {
        counter: Arc<AtomicUsize>,
    }

    impl Drop for TrackDrop {
        fn drop(&mut self) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    {
        let tl = Arc::new(ThreadLocal::new());
        let mut handles = vec![];

        for _ in 0..5 {
            let tl_clone = tl.clone();
            let counter_clone = drop_counter.clone();
            let handle = thread::spawn(move || {
                tl_clone.get_or(|| TrackDrop {
                    counter: counter_clone,
                });
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // ThreadLocal itself is still alive, so drops should not have occurred yet.
        assert_eq!(drop_counter.load(Ordering::SeqCst), 0);
    }

    // ThreadLocal is now dropped. All values should be dropped.
    assert_eq!(drop_counter.load(Ordering::SeqCst), 5);
}

#[test]
fn test_thread_id_recycling_and_many_threads() {
    let tl = Arc::new(ThreadLocal::new());

    // Create 300 threads sequentially.
    // Since threads are created sequentially, their IDs should be recycled,
    // and we should not exceed the ThreadLocal capacity limit.
    for i in 0..300 {
        let tl_clone = tl.clone();
        let handle = thread::spawn(move || {
            let val = tl_clone.get_or(move || i);
            assert_eq!(*val, i);
        });
        handle.join().unwrap();
    }
}

#[test]
fn test_heavy_concurrency() {
    let tl = Arc::new(ThreadLocal::new());
    let mut handles = vec![];

    // Concurrently initialize and read
    for i in 0..50 {
        let tl_clone = tl.clone();
        let handle = thread::spawn(move || {
            let val = tl_clone.get_or(move || i);
            assert_eq!(*val, i);
            for _ in 0..100 {
                assert_eq!(tl_clone.get(), Some(&i));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_reentrant_get_or_drop() {
    let drop_counter = Arc::new(AtomicUsize::new(0));

    struct TrackDrop {
        id: u32,
        counter: Arc<AtomicUsize>,
    }

    impl Drop for TrackDrop {
        fn drop(&mut self) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    {
        let tls = ThreadLocal::new();
        let val = tls.get_or(|| {
            let inner = tls.get_or(|| TrackDrop {
                id: 1,
                counter: drop_counter.clone(),
            });
            assert_eq!(inner.id, 1);
            TrackDrop {
                id: 2,
                counter: drop_counter.clone(),
            }
        });

        assert_eq!(val.id, 1);
        // 外层重入生成的多余值（id: 2）应当在执行完 get_or 时就被立即释放
        assert_eq!(drop_counter.load(Ordering::SeqCst), 1);
    }

    // ThreadLocal 离开作用域被销毁，留在槽位里的内层值（id: 1）也应当随之释放
    assert_eq!(drop_counter.load(Ordering::SeqCst), 2);
}
