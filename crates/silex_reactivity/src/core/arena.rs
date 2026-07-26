use std::{
    alloc::{Layout, alloc, handle_alloc_error},
    cell::UnsafeCell,
    mem::{ManuallyDrop, needs_drop},
    ptr,
};

const CHUNK_SIZE: usize = 128;

/// Strong typed index with generation counter to detect ABA problems.
///
/// # 代数回绕
///
/// `generation` 是 `u32` 且用 `wrapping_add` 递增（插入 +1、移除 +1），因此同一个
/// 槽位被复用 2³¹ 次之后，一个早已失效的 `Index` 会重新变得“有效”，读到的是
/// 另一个节点的数据（AUDIT P19.4）。按每秒创建并销毁 10 万个节点算，需要连续
/// 运行约 6 小时才会绕回同一个槽位一次 —— 对 Web 前端的实际负载有足够余量，
/// 而把它升到 `u64` 会让 `NodeId` 从 8 字节变成 16 字节，订阅者表、依赖表、
/// 各类句柄全都要跟着变大。这里选择记下这个上限，而不是为它加倍内存开销。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Index {
    pub index: u32,
    pub generation: u32,
}

union SlotUnion<T> {
    value: ManuallyDrop<T>,
    next_free: u32,
}

struct Slot<T> {
    u: SlotUnion<T>,
    generation: u32, // Even = vacant, odd = occupied
}

impl<T> Slot<T> {
    #[inline(always)]
    fn occupied(&self) -> bool {
        !self.generation.is_multiple_of(2)
    }
}

impl<T> Drop for Slot<T> {
    fn drop(&mut self) {
        if needs_drop::<T>() && self.occupied() {
            unsafe {
                ManuallyDrop::drop(&mut self.u.value);
            }
        }
    }
}

/// Fixed size memory chunk.
/// Entries are wrapped in UnsafeCell to allow interior mutability.
struct Chunk<T> {
    slots: Box<[UnsafeCell<Slot<T>>]>,
}

impl<T> Chunk<T> {
    fn new() -> Self {
        let layout = Layout::array::<UnsafeCell<Slot<T>>>(CHUNK_SIZE).unwrap();
        let ptr = unsafe { alloc(layout) } as *mut UnsafeCell<Slot<T>>;

        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        // Initialize slots
        for i in 0..CHUNK_SIZE {
            unsafe {
                let slot_ptr = ptr.add(i);
                ptr::write(
                    slot_ptr,
                    UnsafeCell::new(Slot {
                        u: SlotUnion {
                            next_free: u32::MAX,
                        },
                        generation: 0,
                    }),
                );
            }
        }

        let slice_ptr = ptr::slice_from_raw_parts_mut(ptr, CHUNK_SIZE);
        let slots = unsafe { Box::from_raw(slice_ptr) };

        Self { slots }
    }
}

pub struct Arena<T> {
    chunks: UnsafeCell<Vec<Chunk<T>>>,
    free_head: UnsafeCell<Option<u32>>,
    len: UnsafeCell<usize>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            free_head: UnsafeCell::new(None),
            len: UnsafeCell::new(0),
        }
    }

    /// Insert a value into the arena, returning its Index.
    ///
    /// # 内部可变性的契约
    ///
    /// arena 用 `UnsafeCell` 实现内部可变性，`&self` 就能改内部状态。这要求
    /// **同一时刻只能有一个操作在动这些状态**：本 crate 是单线程的（运行时挂在
    /// thread-local 上），而 `insert` / `remove` / `get` 都不会调用任何用户代码，
    /// 因此不存在重入。往这里加任何会回调出去的逻辑都会破坏这条契约。
    pub fn insert(&self, value: T) -> Index {
        // SAFETY: 见上面的契约 —— 单线程 + 本函数内部不会重入，
        // 因此这几个由 `UnsafeCell` 派生的 `&mut` 在其存活期间是独占的。

        let chunks_ptr = self.chunks.get();
        let free_head_ptr = self.free_head.get();
        let len_ptr = self.len.get();

        unsafe {
            let chunks = &mut *chunks_ptr;

            // Priority 1: Reuse from Free List
            if let Some(free_idx) = *free_head_ptr {
                let (chunk_idx, offset) = self.get_chunk_offset(free_idx);

                // Must exist if it was in free list
                let chunk = &chunks[chunk_idx];
                let slot = &mut *chunk.slots[offset].get();

                if slot.occupied() {
                    panic!("Corrupted free list: slot at {} is occupied", free_idx);
                }

                // Retrieve next free index
                let next_free = slot.u.next_free;
                if next_free == u32::MAX {
                    *free_head_ptr = None;
                } else {
                    *free_head_ptr = Some(next_free);
                }

                // Store value
                slot.u.value = ManuallyDrop::new(value);
                // Increment generation (Even -> Odd)
                slot.generation = slot.generation.wrapping_add(1);

                return Index {
                    index: free_idx,
                    generation: slot.generation,
                };
            }

            // Priority 2: Append new slot
            let current_len = *len_ptr;
            let (chunk_idx, offset) = self.get_chunk_offset(current_len as u32);

            if chunk_idx >= chunks.len() {
                chunks.push(Chunk::new());
            }

            let chunk = &chunks[chunk_idx];
            let slot = &mut *chunk.slots[offset].get();

            // Store value
            slot.u.value = ManuallyDrop::new(value);
            // Increment generation to 1 (initially 0/Even)
            // Even (0) -> Odd (1)
            slot.generation = slot.generation.wrapping_add(1);

            *len_ptr += 1;

            Index {
                index: current_len as u32,
                generation: slot.generation,
            }
        }
    }

    /// Access element by Index.
    pub fn get(&self, id: Index) -> Option<&T> {
        let (chunk_idx, offset) = self.get_chunk_offset(id.index);

        // SAFETY: 单线程且本函数不重入（契约见 `insert`）。代数相符即说明槽位
        // 里存的就是这个 `Index` 对应的那个值，返回的引用绑定在 `&self` 上。
        unsafe {
            let chunks = &*self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }

            // Check if index is within valid range (allocated count)
            if id.index as usize >= *self.len.get() {
                return None;
            }

            let slot = &*chunks[chunk_idx].slots[offset].get();

            if slot.generation != id.generation {
                return None;
            }

            // Double check occupancy (redundant with generation but safe)
            if slot.occupied() {
                Some(&slot.u.value)
            } else {
                None
            }
        }
    }

    // 这里曾经有一个 `pub fn get_mut(&self, id) -> Option<&mut T>`：
    // 它取 `&self` 却交出 `&mut T`，安全代码两行就能造出两个同时存活的 `&mut`
    // （AUDIT P7）。实测 crate 内部一处都没用到 —— `graph` 只做 insert/get/remove，
    // 真正需要内部可变的是 `SparseSecondaryMap`。既然没有用户，直接删掉，
    // 而不是把一个无法由类型系统表达的契约继续留在这里。

    /// Remove element.
    /// Returns true if removed, false if not found/already removed.
    pub fn remove(&self, id: Index) -> bool {
        let (chunk_idx, offset) = self.get_chunk_offset(id.index);

        // SAFETY: 单线程且本函数不重入（契约见 `insert`）。`ManuallyDrop::drop`
        // 只在槽位确实被占用时调用一次，随后代数 +1 让所有旧 `Index` 失效。
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return false;
            }
            if id.index as usize >= *self.len.get() {
                return false;
            }

            let slot = &mut *chunks[chunk_idx].slots[offset].get();

            if slot.generation != id.generation {
                return false;
            }

            if slot.occupied() {
                // Remove value
                ManuallyDrop::drop(&mut slot.u.value);

                // Update freelist
                let old_head = (*self.free_head.get()).unwrap_or(u32::MAX);
                slot.u.next_free = old_head;

                // Update version: Odd -> Even
                slot.generation = slot.generation.wrapping_add(1);

                // Update free head
                *self.free_head.get() = Some(id.index);

                return true;
            }

            false
        }
    }

    fn get_chunk_offset(&self, index: u32) -> (usize, usize) {
        let idx = index as usize;
        (idx / CHUNK_SIZE, idx % CHUNK_SIZE)
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

// --- Chunked Sparse Map ---

// Use a small chunk size for secondary maps by default, but allow tuning via const generics.
// High density maps (Signals, Effects) benefit from larger chunks (e.g. 64) for cache locality.
// Sparse maps (NodeRefs) benefit from smaller chunks (e.g. 16) to save memory.

type ChunkArray<T> = Box<[UnsafeCell<Option<(u32, T)>>]>;

pub struct SparseSecondaryMap<T, const N: usize = 16> {
    chunks: UnsafeCell<Vec<Option<ChunkArray<T>>>>,
}

impl<T, const N: usize> Default for SparseSecondaryMap<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> SparseSecondaryMap<T, N> {
    pub fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
        }
    }

    /// 写入一个条目。
    ///
    /// 返回是否真的写进去了：用一个**比槽位里存着的还旧**的代数写入会被拒绝
    /// （ABA 防护，见 `test_secondary_map_aba_protection`）。之前这个拒绝是完全
    /// 静默的，调用方连失败都不知道（AUDIT P19.5）。
    pub fn insert(&self, key: Index, value: T) -> bool {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);

        // SAFETY: 与 `Arena` 相同的契约 —— 单线程、本函数内不执行用户代码。
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                chunks.resize_with(chunk_idx + 1, || None);
            }

            if chunks[chunk_idx].is_none() {
                // Initialize chunk entries to None
                let vec_chunk: Vec<UnsafeCell<Option<(u32, T)>>> =
                    (0..N).map(|_| UnsafeCell::new(None)).collect();
                chunks[chunk_idx] = Some(vec_chunk.into_boxed_slice());
            }

            if let Some(ref mut chunk) = chunks[chunk_idx] {
                let slot = &mut *chunk[offset].get();
                // WRITE PROTECTION:
                // Only allow insertion if:
                // 1. The slot is empty.
                // 2. The new generation is >= the stored generation (prevent ABA downgrade).
                let can_write = if let Some((stored_gen, _)) = slot {
                    key.generation >= *stored_gen
                } else {
                    true
                };

                if can_write {
                    *slot = Some((key.generation, value));
                }
                return can_write;
            }
            false
        }
    }

    pub fn get(&self, key: Index) -> Option<&T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        // SAFETY: 同上；代数相符才返回，引用绑定在 `&self` 上。
        unsafe {
            let chunks = &*self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref chunk) = chunks[chunk_idx] {
                let slot = &*chunk[offset].get();
                if let Some((stored_gen, val)) = slot
                    && *stored_gen == key.generation
                {
                    return Some(val);
                }
            }
            None
        }
    }

    /// 取 `&self` 却交出 `&mut T`。
    ///
    /// **调用方必须保证独占访问**：返回的引用存活期间，不能再对同一个 key 调用
    /// `get` / `get_mut` / `remove`，也不能执行任何可能这么做的用户代码。
    /// 运行时里的做法是把这类借用限制在不调用用户代码的短作用域内，需要跨越
    /// 用户代码时先把值移出去（见 `SignalValueGuard`、AUDIT P5）。
    ///
    /// 这个契约无法由类型系统表达，所以这个方法是 `pub(crate)` 的；同样签名的
    /// `Arena::get_mut` 因为一个用户都没有，已经直接删掉了（AUDIT P7）。
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn get_mut(&self, key: Index) -> Option<&mut T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        // SAFETY: 独占性由上面的契约转嫁给调用方；其余同 `get`。
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref mut chunk) = chunks[chunk_idx] {
                let slot = &mut *chunk[offset].get();
                if let Some((stored_gen, val)) = slot
                    && *stored_gen == key.generation
                {
                    return Some(val);
                }
            }
            None
        }
    }

    pub fn remove(&self, key: Index) -> Option<T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        // SAFETY: 同 `get`；代数不符时不动任何东西。
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref mut chunk) = chunks[chunk_idx] {
                let slot = &mut *chunk[offset].get();
                if let Some((stored_gen, _)) = slot
                    && *stored_gen == key.generation
                {
                    return slot.take().map(|(_, v)| v);
                }
            }
            None
        }
    }

    /// Remove logic if ID is just u32 (for direct internal usage if needed)
    fn get_chunk_offset(&self, index: u32) -> (usize, usize) {
        let idx = index as usize;
        (idx / N, idx % N)
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_basic_ops() {
        let arena = Arena::<String>::new();

        // Insert
        let id1 = arena.insert("Hello".to_string());
        let id2 = arena.insert("World".to_string());

        assert_ne!(id1, id2);

        // Get
        assert_eq!(arena.get(id1).map(|s| s.as_str()), Some("Hello"));
        assert_eq!(arena.get(id2).map(|s| s.as_str()), Some("World"));

        // Remove
        assert!(arena.remove(id1));
        assert_eq!(arena.get(id1), None);
        assert_eq!(arena.get(id2).map(|s| s.as_str()), Some("World")); // id2 still there

        // Stale usage
        assert_eq!(arena.get(id1), None);
    }

    #[test]
    fn test_arena_reuse() {
        let arena = Arena::<u32>::new();
        let id1 = arena.insert(100);
        let idx1_raw = id1.index;

        arena.remove(id1);

        // Re-insert, should reuse idx1_raw
        let id2 = arena.insert(200);
        assert_eq!(id2.index, idx1_raw);
        assert_ne!(id2.generation, id1.generation);

        assert_eq!(arena.get(id2), Some(&200));
        assert_eq!(arena.get(id1), None); // Old ID is invalid
    }

    #[test]
    fn test_chunk_overflow() {
        let arena = Arena::<usize>::new();
        let count = CHUNK_SIZE * 3 + 10; // More than 3 chunks
        let mut ids = Vec::new();

        for i in 0..count {
            ids.push(arena.insert(i));
        }

        for (i, id) in ids.iter().enumerate() {
            assert_eq!(arena.get(*id), Some(&i));
        }
    }

    #[test]
    fn test_sparse_secondary_map() {
        let arena = Arena::<()>::new();
        let map = SparseSecondaryMap::<String>::new();

        let id1 = arena.insert(());
        let id2 = arena.insert(());

        map.insert(id1, "Data1".to_string());

        assert_eq!(map.get(id1).map(|s| s.as_str()), Some("Data1"));
        assert_eq!(map.get(id2), None);
        map.remove(id1);
        assert_eq!(map.get(id1), None);
    }

    #[test]
    fn test_secondary_map_aba_protection() {
        let arena = Arena::<()>::new();
        let map = SparseSecondaryMap::<String>::new();

        let id1 = arena.insert(());
        map.insert(id1, "Data1".to_string());

        // 模拟回收并生成新节点（重用 Index 但 Generation 增加）
        arena.remove(id1);
        let id2 = arena.insert(());
        assert_eq!(id1.index, id2.index);
        assert_ne!(id1.generation, id2.generation);

        // 此时 map 中存的是 (id1.gen, "Data1")
        // 使用 id2 访问应该返回 None，因为代数不匹配
        assert_eq!(
            map.get(id2),
            None,
            "New ID should not be able to read leaked old data"
        );

        // 为 id2 存数据，这会覆盖之前的 (id1.gen, "Data1")
        map.insert(id2, "Data2".to_string());
        assert_eq!(map.get(id2).map(|s| s.as_str()), Some("Data2"));

        // 此时 id1 应该彻底失效，因为它尝试读取 index 时的 gen (id1.gen)
        // 与 map 中存储的 gen (id2.gen) 不匹配
        assert_eq!(
            map.get(id1),
            None,
            "Old ID should not be able to read new node's data (ABA)"
        );

        // 尝试使用旧 ID 移除数据，应该无效
        assert!(map.remove(id1).is_none());
        assert_eq!(
            map.get(id2).map(|s| s.as_str()),
            Some("Data2"),
            "Old ID removal should not affect new node"
        );
    }

    /// 用一个过时的代数写入会被拒绝，而且**说得出来**被拒绝了（AUDIT P19.5）。
    #[test]
    fn insert_reports_whether_it_actually_wrote() {
        let arena = Arena::<()>::new();
        let map = SparseSecondaryMap::<String>::new();

        let old = arena.insert(());
        arena.remove(old);
        let new = arena.insert(());
        assert_eq!(old.index, new.index);
        assert!(new.generation > old.generation);

        assert!(map.insert(new, "new".to_string()), "新代数写入应当成功");
        assert!(
            !map.insert(old, "stale".to_string()),
            "旧代数的写入必须被拒绝，并且返回 false"
        );
        assert_eq!(map.get(new).map(String::as_str), Some("new"));
    }

    #[test]
    fn dropping_the_arena_drops_every_live_value() {
        use std::{cell::Cell, rc::Rc};

        struct DropSpy(Rc<Cell<usize>>);
        impl Drop for DropSpy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let hits = Rc::new(Cell::new(0));
        {
            let arena = Arena::<DropSpy>::new();
            let a = arena.insert(DropSpy(hits.clone()));
            arena.insert(DropSpy(hits.clone()));
            arena.remove(a); // 显式移除的那个立刻析构
            assert_eq!(hits.get(), 1);
        }
        assert_eq!(hits.get(), 2, "剩下的值应随 arena 一起析构");
    }

    /// 空槽位不该被当成有值（`Slot::drop` 只析构占用中的槽位）。
    #[test]
    fn removing_twice_is_a_noop() {
        let arena = Arena::<String>::new();
        let id = arena.insert("x".to_string());
        assert!(arena.remove(id));
        assert!(!arena.remove(id), "重复移除必须返回 false，且不得重复析构");
        assert_eq!(arena.get(id), None);
    }
}
