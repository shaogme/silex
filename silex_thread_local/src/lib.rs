use std::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
    ptr::{self, null_mut},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

const MAX_BLOCKS: usize = 256;

// Global Thread ID Manager to assign dense IDs lock-freely using a bitmap
struct ThreadIdManager {
    blocks: [AtomicPtr<AtomicU64>; MAX_BLOCKS],
}

impl ThreadIdManager {
    const fn new() -> Self {
        const NULL_PTR: AtomicPtr<AtomicU64> = AtomicPtr::new(null_mut());
        Self {
            blocks: [NULL_PTR; MAX_BLOCKS],
        }
    }

    fn alloc(&self) -> usize {
        for block_idx in 0..MAX_BLOCKS {
            let mut block_ptr = self.blocks[block_idx].load(Ordering::Acquire);
            if block_ptr.is_null() {
                // Try to allocate a new block
                let new_block = Box::into_raw(Box::new(AtomicU64::new(0)));
                match self.blocks[block_idx].compare_exchange(
                    null_mut(),
                    new_block,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        block_ptr = new_block;
                    }
                    Err(actual) => {
                        // Reclaim memory
                        unsafe {
                            let _ = Box::from_raw(new_block);
                        }
                        block_ptr = actual;
                    }
                }
            }

            // Now we have a valid block_ptr
            let block = unsafe { &*block_ptr };
            let mut mask = block.load(Ordering::Acquire);
            loop {
                if mask == u64::MAX {
                    break; // Block is full, try next block
                }
                let bit_idx = mask.trailing_ones() as usize;
                if bit_idx >= 64 {
                    break;
                }
                let new_mask = mask | (1 << bit_idx);
                match block.compare_exchange_weak(
                    mask,
                    new_mask,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        return block_idx * 64 + bit_idx;
                    }
                    Err(actual) => {
                        mask = actual;
                    }
                }
            }
        }
        panic!("Thread limit exceeded");
    }

    fn free(&self, id: usize) {
        let block_idx = id / 64;
        let bit_idx = id % 64;
        assert!(block_idx < MAX_BLOCKS, "Invalid thread ID");

        let block_ptr = self.blocks[block_idx].load(Ordering::Acquire);
        assert!(!block_ptr.is_null(), "Block not initialized for ID");

        let block = unsafe { &*block_ptr };
        let mask = 1 << bit_idx;
        block.fetch_and(!mask, Ordering::Release);
    }
}

impl Drop for ThreadIdManager {
    fn drop(&mut self) {
        for block_ptr_atomic in &mut self.blocks {
            let block_ptr = *block_ptr_atomic.get_mut();
            if !block_ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(block_ptr);
                }
            }
        }
    }
}

static THREAD_ID_MANAGER: ThreadIdManager = ThreadIdManager::new();

thread_local! {
    static THREAD_ID: ThreadKey = const { ThreadKey::new() };
    static UNIQUE_THREAD_ID: u64 = {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    };
}

fn get_unique_thread_id() -> u64 {
    UNIQUE_THREAD_ID.with(|id| *id)
}

struct ThreadKey {
    id: Cell<Option<usize>>,
}

impl ThreadKey {
    const fn new() -> Self {
        Self {
            id: Cell::new(None),
        }
    }

    fn get(&self) -> usize {
        if let Some(id) = self.id.get() {
            id
        } else {
            let id = THREAD_ID_MANAGER.alloc();
            self.id.set(Some(id));
            id
        }
    }
}

impl Drop for ThreadKey {
    fn drop(&mut self) {
        if let Some(id) = self.id.get() {
            THREAD_ID_MANAGER.free(id);
        }
    }
}

fn get_thread_index() -> usize {
    THREAD_ID.with(|key| key.get())
}

const BUCKETS: usize = 32;

fn bucket_capacity(i: usize) -> usize {
    if i == 0 { 1 } else { 1 << (i - 1) }
}

struct RetiredNode<T> {
    value: T,
    next: *mut RetiredNode<T>,
}

struct Entry<T> {
    present: AtomicBool,
    owner_unique_id: AtomicU64,
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Drop for Entry<T> {
    fn drop(&mut self) {
        if *self.present.get_mut() {
            unsafe {
                (*self.value.get()).assume_init_drop();
            }
        }
    }
}

pub struct ThreadLocal<T> {
    buckets: [AtomicPtr<Entry<T>>; BUCKETS],
    retired: AtomicPtr<RetiredNode<T>>,
}

unsafe impl<T: Send> Send for ThreadLocal<T> {}
unsafe impl<T> Sync for ThreadLocal<T> {}

impl<T> ThreadLocal<T> {
    const NULL_PTR: AtomicPtr<Entry<T>> = AtomicPtr::new(null_mut());

    pub const fn new() -> Self {
        Self {
            buckets: [Self::NULL_PTR; BUCKETS],
            retired: AtomicPtr::new(null_mut()),
        }
    }

    fn get_bucket_and_sub_index(idx: usize) -> (usize, usize) {
        if idx == 0 {
            (0, 0)
        } else {
            let bits = usize::BITS - 1 - idx.leading_zeros();
            ((bits + 1) as usize, idx ^ (1 << bits))
        }
    }

    /// Retrieves a reference to the value for the current thread, if initialized.
    pub fn get(&self) -> Option<&T> {
        let idx = get_thread_index();
        let (bucket_idx, sub_idx) = Self::get_bucket_and_sub_index(idx);

        if bucket_idx >= BUCKETS {
            return None;
        }

        let bucket_ptr = self.buckets[bucket_idx].load(Ordering::Acquire);
        if bucket_ptr.is_null() {
            return None;
        }

        // SAFETY: bucket_ptr is valid and points to an array of size bucket_capacity(bucket_idx)
        unsafe {
            let entry = &*bucket_ptr.add(sub_idx);
            if entry.present.load(Ordering::Acquire) {
                if entry.owner_unique_id.load(Ordering::Relaxed) == get_unique_thread_id() {
                    return Some(&*entry.value.get().cast::<T>());
                }
            }
        }
        None
    }

    /// Retrieves a reference to the value for the current thread, initializing it
    /// with the provided closure if it has not been set yet.
    pub fn get_or<F>(&self, default: F) -> &T
    where
        F: FnOnce() -> T,
    {
        let idx = get_thread_index();
        let (bucket_idx, sub_idx) = Self::get_bucket_and_sub_index(idx);

        assert!(bucket_idx < BUCKETS, "Thread limit exceeded");

        let mut bucket_ptr = self.buckets[bucket_idx].load(Ordering::Acquire);
        if bucket_ptr.is_null() {
            // Allocate bucket
            let size = bucket_capacity(bucket_idx);
            let new_bucket = std::iter::repeat_with(|| Entry {
                present: AtomicBool::new(false),
                owner_unique_id: AtomicU64::new(0),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            })
            .take(size)
            .collect::<Box<[Entry<T>]>>();
            let new_bucket = Box::into_raw(new_bucket) as *mut Entry<T>;

            match self.buckets[bucket_idx].compare_exchange(
                null_mut(),
                new_bucket,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    bucket_ptr = new_bucket;
                }
                Err(actual) => {
                    // Reclaim our new bucket
                    unsafe {
                        let _ = Box::from_raw(std::slice::from_raw_parts_mut(new_bucket, size));
                    }
                    bucket_ptr = actual;
                }
            }
        }

        let unique_id = get_unique_thread_id();
        // SAFETY: bucket_ptr is initialized and sub_idx is within bounds
        unsafe {
            let entry = &*bucket_ptr.add(sub_idx);
            if entry.present.load(Ordering::Acquire)
                && entry.owner_unique_id.load(Ordering::Relaxed) == unique_id
            {
                return &*entry.value.get().cast::<T>();
            }

            let was_present = entry.present.swap(false, Ordering::AcqRel);
            if was_present {
                let old_val = ptr::read(entry.value.get().cast::<T>());
                if std::mem::needs_drop::<T>() {
                    let node = Box::into_raw(Box::new(RetiredNode {
                        value: old_val,
                        next: ptr::null_mut(),
                    }));
                    let mut head = self.retired.load(Ordering::Acquire);
                    loop {
                        (*node).next = head;
                        match self.retired.compare_exchange_weak(
                            head,
                            node,
                            Ordering::Release,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => break,
                            Err(actual) => head = actual,
                        }
                    }
                } else {
                    drop(old_val);
                }
            }

            let new_val = default();

            if entry.present.load(Ordering::Acquire)
                && entry.owner_unique_id.load(Ordering::Relaxed) == unique_id
            {
                drop(new_val);
                return &*entry.value.get().cast::<T>();
            }

            ptr::write(entry.value.get().cast::<T>(), new_val);
            entry.owner_unique_id.store(unique_id, Ordering::Relaxed);
            entry.present.store(true, Ordering::Release);

            &*entry.value.get().cast::<T>()
        }
    }
}

impl<T> Drop for ThreadLocal<T> {
    fn drop(&mut self) {
        for (i, bucket_ptr_atomic) in self.buckets.iter_mut().enumerate() {
            let bucket_ptr = *bucket_ptr_atomic.get_mut();
            if !bucket_ptr.is_null() {
                let size = bucket_capacity(i);
                // SAFETY: We own ThreadLocal and are dropping it.
                unsafe {
                    let slice = std::slice::from_raw_parts_mut(bucket_ptr, size);
                    let _ = Box::from_raw(slice);
                }
            }
        }
        let mut head = *self.retired.get_mut();
        while !head.is_null() {
            unsafe {
                let node = *Box::from_raw(head);
                head = node.next;
                let _ = node.value;
            }
        }
    }
}
