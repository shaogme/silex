//! 两个基础容器：[`Arena`]（节点主表）与 [`SparseSecondaryMap`]（旁路表）。
//!
//! # 交出去的引用为什么不会被“动一下同一张表”作废
//!
//! 两张表都是分块的，而**一块 chunk 就是一次独立的堆分配**，承载 chunk 的那个
//! `Vec` 里只存裸指针。因此从槽位派生出来的引用，它的 provenance 来自 chunk
//! 自己的分配 —— `insert` / `remove` / `Vec` 扩容动的都是那个 `Vec`，
//! 动不到已经交出去的引用。
//!
//! 从前不是这样。引用的推导链是
//! `&mut *self.chunks.get()`（`&mut Vec`）→ `&mut chunks[i]` → `Box` 解引用 →
//! 槽位：在 Stacked Borrows 下，**第二次**取 `&mut Vec`（也就是任何一次后续的
//! `get` / `insert` / `remove`）都会把第一个 `&mut Vec` 的标记弹出栈，连带作废它
//! 的全部派生指针。不需要 key 冲突，动同一张 map 就够了（审计报告 §2.1）。
//! 阶段二把“交出 `&mut T`”换成了闭包形态的 `with_mut` 缓解这一点，代价是每一次
//! 状态写入（`propagate` 里最密集的操作）都要重新取一次 `&mut Vec`，于是
//! “持有一个节点的引用遍历它的订阅者表”这种最自然的写法始终是不合法的 ——
//! 这正是 `fill_subscribers(&self, dest: &mut Vec<NodeId>)` 那套 `Vec` 物化与
//! 配套池化存在的根本原因（审计报告 §3.3）。
//!
//! 现在剩下的规则窄到可以逐点审读：
//!
//! > `get` 返回的引用不得跨越对**同一个 key** 的 `insert` / `remove`。
//!
//! 而“同一个 key 的 remove”就是“这个节点被销毁了”。运行时里每一条会跨越用户
//! 代码的路径本来就用守卫把值整个移出了节点（见 [`crate::runtime::guard`]），
//! 因此这条规则在 crate 内部是被结构性满足的。
//!
//! 值本身的可变性不再由这两张表负责：`ReactiveNode` 自带 `Cell` / `RefCell`，
//! 从 `get` 拿到的共享引用就足以改它（阶段三方案 A）。

use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error},
    cell::{Cell, UnsafeCell},
    mem::{ManuallyDrop, needs_drop},
    ptr::{self, NonNull},
};

const CHUNK_SIZE: usize = 128;

/// 空闲链表的终止标记（`u32::MAX` 不可能是合法槽位号）。
const NO_FREE_SLOT: u32 = u32::MAX;

/// Strong typed index with generation counter to detect ABA problems.
///
/// # 代数回绕
///
/// `generation` 是 `u32` 且用 `wrapping_add` 递增（插入 +1、移除 +1），因此同一个
/// 槽位被复用 2³¹ 次之后，一个早已失效的 `Index` 会重新变得“有效”，读到的是
/// 另一个节点的数据（AUDIT P19.4）。按每秒创建并销毁 10 万个节点算，需要连续
/// 运行约 6 小时才会绕回同一个槽位一次 —— 对 Web 前端的实际负载有足够余量，
/// 而把它升到 `u64` 会让句柄从 8 字节变成 16 字节，订阅者表、依赖表、
/// 各类句柄全都要跟着变大。这里选择记下这个上限，而不是为它加倍内存开销。
///
/// # 字段为什么是私有的
///
/// 曾经这两个字段是 `pub` 的，安全代码因此可以凭空捏造任意句柄。所有读取路径
/// 都做了代数校验所以不会读到别人的数据，但 [`SparseSecondaryMap::insert`] 对一个
/// 伪造的巨大 index 会 `resize_with` 出巨量内存（审计报告 §3.4）。现在唯一能拿到
/// 的常量句柄是 [`Index::DANGLING`]，而它对每一张表都恒为“查无此项”。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Index {
    index: u32,
    generation: u32,
}

impl Index {
    /// 一个永远不指向任何节点的句柄。
    ///
    /// 用于需要一个“空句柄”占位的场合（下游框架的 `Default` 实现等）。
    /// 它的 index 大于任何真实节点，因此在 `Arena` 与 `SparseSecondaryMap` 里
    /// 一律查无此项，也不会触发任何分配。
    pub const DANGLING: Self = Self {
        index: u32::MAX,
        generation: 0,
    };

    #[inline(always)]
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// 槽位编号，只用于诊断信息。
    #[inline(always)]
    pub(crate) const fn slot(self) -> u32 {
        self.index
    }
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}v{}", self.index, self.generation)
    }
}

#[inline(always)]
const fn split(index: u32, chunk_len: usize) -> (usize, usize) {
    let idx = index as usize;
    (idx / chunk_len, idx % chunk_len)
}

// --- Arena ---

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
            // SAFETY: `occupied()` 为真即说明联合体里当前存的是 `value`。
            unsafe {
                ManuallyDrop::drop(&mut self.u.value);
            }
        }
    }
}

#[inline]
fn slot_chunk_layout<T>() -> Layout {
    Layout::array::<Slot<T>>(CHUNK_SIZE).expect("Arena: chunk layout overflow")
}

/// 单独分配一块 chunk，并把每个槽位初始化成“空闲”。
///
/// 返回的指针的 provenance 覆盖整块 chunk，且**与承载它的 `Vec` 无关** ——
/// 这是本模块头部那条规则的全部依据。
fn alloc_slot_chunk<T>() -> NonNull<Slot<T>> {
    let layout = slot_chunk_layout::<T>();
    // SAFETY: layout 的 size 非零（CHUNK_SIZE > 0 且 Slot<T> 至少含一个 u32）。
    let raw = unsafe { alloc(layout) } as *mut Slot<T>;
    let Some(base) = NonNull::new(raw) else {
        handle_alloc_error(layout)
    };
    for i in 0..CHUNK_SIZE {
        // SAFETY: i < CHUNK_SIZE，写入位置在刚分配的块之内且尚未初始化。
        unsafe {
            ptr::write(
                base.as_ptr().add(i),
                Slot {
                    u: SlotUnion {
                        next_free: NO_FREE_SLOT,
                    },
                    generation: 0,
                },
            );
        }
    }
    base
}

pub(crate) struct Arena<T> {
    /// 只存 chunk 的裸指针：扩容重分配的是这个 `Vec` 自己，动不到任何 chunk。
    chunks: UnsafeCell<Vec<NonNull<Slot<T>>>>,
    /// 空闲链表头，[`NO_FREE_SLOT`] 表示空。是 `Copy` 的标量，用 `Cell` 就够了。
    free_head: Cell<u32>,
    len: Cell<usize>,
}

impl<T> Arena<T> {
    pub(crate) fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            free_head: Cell::new(NO_FREE_SLOT),
            len: Cell::new(0),
        }
    }

    /// 取一个槽位的裸指针。
    ///
    /// `&Vec` 的存活期严格限制在这一行：从它身上只复制走一个裸指针，
    /// 而裸指针带的是 chunk 自己的 provenance。
    #[inline]
    fn slot_ptr(&self, index: u32) -> Option<*mut Slot<T>> {
        let (chunk_idx, offset) = split(index, CHUNK_SIZE);
        // SAFETY: 单线程；这个共享借用不跨越任何会取 `&mut Vec` 的调用。
        let base = *unsafe { &*self.chunks.get() }.get(chunk_idx)?;
        // SAFETY: offset < CHUNK_SIZE，仍在 chunk 之内。
        Some(unsafe { base.as_ptr().add(offset) })
    }

    /// 同上，但必要时把 chunk 建出来。
    fn slot_ptr_growing(&self, index: u32) -> *mut Slot<T> {
        let (chunk_idx, _) = split(index, CHUNK_SIZE);
        {
            // SAFETY: 单线程，本块内不执行任何用户代码，也不派生任何指向 chunk
            // 内容的引用 —— 因此这个 `&mut Vec` 只可能作废“指向 Vec 缓冲区自身”
            // 的引用，而那种引用不存在。
            let chunks = unsafe { &mut *self.chunks.get() };
            while chunk_idx >= chunks.len() {
                chunks.push(alloc_slot_chunk::<T>());
            }
        }
        self.slot_ptr(index).expect("chunk 刚刚被建出来")
    }

    /// Insert a value into the arena, returning its Index.
    pub(crate) fn insert(&self, value: T) -> Index {
        let free = self.free_head.get();
        if free != NO_FREE_SLOT {
            let slot = self.slot_ptr(free).expect("空闲链表指向不存在的槽位");
            // SAFETY: 空闲链表里的槽位一定已初始化且当前空闲，联合体里存的是
            // `next_free`；写入 `value` 不会覆盖任何需要析构的东西。
            unsafe {
                assert!(!(*slot).occupied(), "Corrupted free list: slot {free}");
                self.free_head.set((*slot).u.next_free);
                (*slot).u.value = ManuallyDrop::new(value);
                (*slot).generation = (*slot).generation.wrapping_add(1);
                return Index::new(free, (*slot).generation);
            }
        }

        let index = u32::try_from(self.len.get()).expect("Arena: 槽位数超出 u32");
        let slot = self.slot_ptr_growing(index);
        // SAFETY: 新槽位由 `alloc_slot_chunk` 初始化成空闲（generation 为偶数）。
        unsafe {
            (*slot).u.value = ManuallyDrop::new(value);
            (*slot).generation = (*slot).generation.wrapping_add(1);
            self.len.set(self.len.get() + 1);
            Index::new(index, (*slot).generation)
        }
    }

    /// Access element by Index.
    ///
    /// 返回的引用的规则见模块文档：它不得跨越对**同一个 key** 的 `remove`。
    #[inline]
    pub(crate) fn get(&self, id: Index) -> Option<&T> {
        if id.index as usize >= self.len.get() {
            return None;
        }
        let slot = self.slot_ptr(id.index)?;
        // SAFETY: 槽位已初始化；代数相符即说明里面存的就是这个 `Index` 的值。
        unsafe {
            if (*slot).generation != id.generation || !(*slot).occupied() {
                return None;
            }
            Some(&(*slot).u.value)
        }
    }

    /// Remove element.
    /// Returns true if removed, false if not found/already removed.
    pub(crate) fn remove(&self, id: Index) -> bool {
        if id.index as usize >= self.len.get() {
            return false;
        }
        let Some(slot) = self.slot_ptr(id.index) else {
            return false;
        };
        // SAFETY: 槽位已初始化；`ManuallyDrop::drop` 只在确实占用时调用一次，
        // 随后代数 +1 让所有旧 `Index` 失效。
        unsafe {
            if (*slot).generation != id.generation || !(*slot).occupied() {
                return false;
            }
            ManuallyDrop::drop(&mut (*slot).u.value);
            (*slot).u.next_free = self.free_head.get();
            (*slot).generation = (*slot).generation.wrapping_add(1);
            self.free_head.set(id.index);
            true
        }
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        let layout = slot_chunk_layout::<T>();
        for base in self.chunks.get_mut().iter() {
            // SAFETY: 每块 chunk 都由 `alloc_slot_chunk` 用同一个 layout 分配，
            // 其中每个槽位都已初始化（`Slot::drop` 自己会跳过空闲槽位）。
            unsafe {
                for offset in 0..CHUNK_SIZE {
                    ptr::drop_in_place(base.as_ptr().add(offset));
                }
                dealloc(base.as_ptr().cast::<u8>(), layout);
            }
        }
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

type Entry<T> = Option<(u32, T)>;

/// 单独分配一块条目 chunk，全部初始化为空。
fn alloc_entry_chunk<T>(len: usize) -> NonNull<Entry<T>> {
    let boxed: Box<[Entry<T>]> = (0..len).map(|_| None).collect();
    // SAFETY: `Box::into_raw` 交出的指针非空，provenance 覆盖整块分配。
    unsafe { NonNull::new_unchecked(Box::into_raw(boxed).cast::<Entry<T>>()) }
}

pub(crate) struct SparseSecondaryMap<T, const N: usize = 16> {
    chunks: UnsafeCell<Vec<Option<NonNull<Entry<T>>>>>,
}

impl<T, const N: usize> Default for SparseSecondaryMap<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> SparseSecondaryMap<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
        }
    }

    /// 取一个条目的裸指针；条目所在的 chunk 还没建出来时返回 `None`。
    ///
    /// `&Vec` 的存活期严格限制在这一行（理由同 [`Arena::slot_ptr`]）。
    #[inline]
    fn entry_ptr(&self, index: u32) -> Option<*mut Entry<T>> {
        let (chunk_idx, offset) = split(index, N);
        // SAFETY: 单线程；共享借用不跨越任何会取 `&mut Vec` 的调用。
        let base = (*unsafe { &*self.chunks.get() }.get(chunk_idx)?)?;
        // SAFETY: offset < N，仍在 chunk 之内。
        Some(unsafe { base.as_ptr().add(offset) })
    }

    /// 同上，但必要时把 chunk 建出来。
    fn entry_ptr_growing(&self, index: u32) -> *mut Entry<T> {
        let (chunk_idx, _) = split(index, N);
        {
            // SAFETY: 理由同 `Arena::slot_ptr_growing` —— 块内不派生任何指向
            // chunk 内容的引用。
            let chunks = unsafe { &mut *self.chunks.get() };
            if chunk_idx >= chunks.len() {
                chunks.resize_with(chunk_idx + 1, || None);
            }
            if chunks[chunk_idx].is_none() {
                chunks[chunk_idx] = Some(alloc_entry_chunk::<T>(N));
            }
        }
        self.entry_ptr(index).expect("chunk 刚刚被建出来")
    }

    /// 写入一个条目。
    ///
    /// 返回是否真的写进去了：用一个**比槽位里存着的还旧**的代数写入会被拒绝
    /// （ABA 防护，见 `test_secondary_map_aba_protection`）。之前这个拒绝是完全
    /// 静默的，调用方连失败都不知道（AUDIT P19.5）。
    pub(crate) fn insert(&self, key: Index, value: T) -> bool {
        let entry = self.entry_ptr_growing(key.index);
        // SAFETY: 条目已初始化。调用方须保证此刻没有指向**同一个 key** 的
        // 活引用（模块文档里的那条规则）—— 覆盖写会析构旧值。
        unsafe {
            // WRITE PROTECTION:
            // 1. 空槽位可以写；
            // 2. 新代数必须不小于已存代数（防止 ABA 降级）。
            let can_write = match &*entry {
                Some((stored_gen, _)) => key.generation >= *stored_gen,
                None => true,
            };
            if can_write {
                *entry = Some((key.generation, value));
            }
            can_write
        }
    }

    /// 读一个条目。
    ///
    /// 返回的引用的规则见模块文档：它不得跨越对**同一个 key** 的
    /// `insert` / `remove`。值本身的可变性由 `T` 内部的 `Cell` / `RefCell` 提供
    /// —— 这里曾经有一个 `with_mut`（更早还是 `get_mut(&self) -> &mut T`），
    /// 现在整张表对外只有共享引用（阶段三方案 A）。
    #[inline]
    pub(crate) fn get(&self, key: Index) -> Option<&T> {
        let entry = self.entry_ptr(key.index)?;
        // SAFETY: 条目已初始化；代数相符才把里面的值借出去。
        match unsafe { &*entry } {
            Some((stored_gen, value)) if *stored_gen == key.generation => Some(value),
            _ => None,
        }
    }

    /// 这个 key 是否有条目（不借出任何引用）。
    #[inline]
    pub(crate) fn contains_key(&self, key: Index) -> bool {
        self.get(key).is_some()
    }

    pub(crate) fn remove(&self, key: Index) -> Option<T> {
        let entry = self.entry_ptr(key.index)?;
        // SAFETY: 条目已初始化；代数不符时不动任何东西。调用方须保证此刻没有
        // 指向同一个 key 的活引用。
        unsafe {
            match &*entry {
                Some((stored_gen, _)) if *stored_gen == key.generation => {}
                _ => return None,
            }
            (*entry).take().map(|(_, value)| value)
        }
    }
}

impl<T, const N: usize> Drop for SparseSecondaryMap<T, N> {
    fn drop(&mut self) {
        for base in self.chunks.get_mut().iter().flatten() {
            // SAFETY: 每块 chunk 都是一个长度恒为 N 的 `Box<[Entry<T>]>` 转成的
            // 裸指针，这里原样还回去由 `Box` 析构。
            drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(base.as_ptr(), N)) });
        }
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
        use std::rc::Rc;

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

    /// 旁路表里剩下的值也要随表一起析构（chunk 现在是手工管理的裸指针）。
    #[test]
    fn dropping_the_secondary_map_drops_every_live_value() {
        use std::rc::Rc;

        struct DropSpy(Rc<Cell<usize>>);
        impl Drop for DropSpy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let hits = Rc::new(Cell::new(0));
        let arena = Arena::<()>::new();
        {
            let map = SparseSecondaryMap::<DropSpy, 4>::new();
            // 跨若干块 chunk，确保每一块都被回收。
            for _ in 0..10 {
                let id = arena.insert(());
                map.insert(id, DropSpy(hits.clone()));
            }
            assert_eq!(hits.get(), 0);
        }
        assert_eq!(hits.get(), 10);
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

    /// 阶段三的核心不变量：从 `get` 拿到的引用**不会**被对别的 key 的
    /// `insert` / `remove` 作废（见模块文档）。这条断言的价值全在 Miri 下 ——
    /// 从前引用是从 `&mut Vec` 一路派生的，下面这段在 Stacked Borrows 下是 UB。
    #[test]
    fn a_borrowed_entry_survives_unrelated_table_traffic() {
        let arena = Arena::<u64>::new();
        let map = SparseSecondaryMap::<Cell<u64>, 4>::new();

        let watched = arena.insert(1);
        map.insert(watched, Cell::new(7));
        let borrowed = map.get(watched).expect("刚写进去");

        // 大量无关流量：跨 chunk 插入、删除，以及 arena 自己的增删。
        let mut others = Vec::new();
        for i in 0..64u64 {
            let id = arena.insert(i);
            map.insert(id, Cell::new(i));
            others.push(id);
        }
        for id in others {
            map.remove(id);
            arena.remove(id);
        }

        // 借用仍然有效，而且仍然指向同一个条目。
        borrowed.set(borrowed.get() + 1);
        assert_eq!(map.get(watched).map(Cell::get), Some(8));
    }
}
