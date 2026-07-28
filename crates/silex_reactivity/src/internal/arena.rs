//! 基础容器：[`Arena`]（主节点存储竞技场）与 [`SparseSecondaryMap`]（稀疏旁路块状映射表）。
//!
//! 容器通过句柄的代数 (Generation) 防范 ABA 槽位复用问题，配合 Rust 的借用规则保证安全并发与无别名引用。
#![forbid(unsafe_code)]

#[cfg(test)]
const CHUNK_SIZE: usize = 128;
const NO_FREE_SLOT: u32 = u32::MAX;

/// 带代数计数器 (Generation Counter) 的原始句柄，用于解决 ABA 问题。
///
/// `generation` 是 `u32` 且用 `wrapping_add` 递增（插入 +1、移除 +1）。同一个槽位
/// 被复用 2³¹ 次之后，旧句柄可能再次有效；这里保留 8 字节句柄，不为这个极端边界
/// 把所有订阅者与句柄扩大到 16 字节。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawId {
    index: u32,
    generation: u32,
}

impl RawId {
    /// 一个永远不指向任何节点的悬空句柄。
    pub const DANGLING: Self = Self {
        index: u32::MAX,
        generation: 0,
    };

    #[inline(always)]
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    #[inline(always)]
    pub(crate) const fn slot(self) -> u32 {
        self.index
    }
}

impl std::fmt::Debug for RawId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}v{}", self.index, self.generation)
    }
}

struct Slot<T> {
    value: Option<T>,
    generation: u32,
    next_free: u32,
}

pub(crate) struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_head: u32,
    len: usize,
}

impl<T> Arena<T> {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: NO_FREE_SLOT,
            len: 0,
        }
    }

    /// 向 Arena 插入一个值，返回其分配的原始句柄 [`RawId`]。
    pub(crate) fn insert(&mut self, value: T) -> RawId {
        if self.free_head != NO_FREE_SLOT {
            let index = self.free_head;
            let slot = self
                .slots
                .get_mut(index as usize)
                .expect("空闲链表指向不存在的槽位");
            debug_assert!(slot.value.is_none(), "Corrupted free list: slot {index}");
            self.free_head = slot.next_free;
            slot.next_free = NO_FREE_SLOT;
            slot.generation = slot.generation.wrapping_add(1);
            debug_assert!(slot.generation % 2 == 1);
            slot.value = Some(value);
            self.len += 1;
            return RawId::new(index, slot.generation);
        }

        let index = u32::try_from(self.slots.len()).expect("Arena: 槽位数超出 u32");
        self.slots.push(Slot {
            value: Some(value),
            generation: 1,
            next_free: NO_FREE_SLOT,
        });
        self.len += 1;
        RawId::new(index, 1)
    }

    /// 通过原始句柄读取元素的共享引用。若句柄已过期或槽位为空则返回 `None`。
    #[inline]
    pub(crate) fn get(&self, id: RawId) -> Option<&T> {
        let slot = self.slots.get(id.index as usize)?;
        (slot.generation == id.generation && slot.value.is_some())
            .then(|| slot.value.as_ref().expect("占用槽位必须有值"))
    }

    /// 通过独占借用访问元素。若句柄已过期或槽位为空则返回 `None`。
    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, id: RawId) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// 移除句柄对应的元素。移除成功返回 `true`，若不存在或已被移除则返回 `false`。
    pub(crate) fn remove(&mut self, id: RawId) -> bool {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return false;
        };
        if slot.generation != id.generation || slot.value.is_none() {
            return false;
        }
        slot.value.take();
        slot.generation = slot.generation.wrapping_add(1);
        debug_assert!(slot.generation % 2 == 0);
        slot.next_free = self.free_head;
        self.free_head = id.index;
        self.len -= 1;
        true
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

type Entry<T> = Option<(u32, T)>;

/// 以 [`RawId`] 为键的稀疏旁路块状存储表。
pub(crate) struct SparseSecondaryMap<T, const N: usize = 16> {
    chunks: Vec<Option<Vec<Entry<T>>>>,
}

impl<T, const N: usize> Default for SparseSecondaryMap<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> SparseSecondaryMap<T, N> {
    pub(crate) fn new() -> Self {
        assert!(N != 0, "SparseSecondaryMap 的 chunk 大小不能为零");
        Self { chunks: Vec::new() }
    }

    #[inline(always)]
    fn split(index: u32) -> (usize, usize) {
        let index = index as usize;
        (index / N, index % N)
    }

    fn entry_mut_growing(&mut self, index: u32) -> Option<&mut Entry<T>> {
        if index == u32::MAX {
            return None;
        }
        let (chunk_index, offset) = Self::split(index);
        if chunk_index >= self.chunks.len() {
            self.chunks.resize_with(chunk_index + 1, || None);
        }
        let chunk = self.chunks[chunk_index]
            .get_or_insert_with(|| (0..N).map(|_| None).collect::<Vec<Entry<T>>>());
        chunk.get_mut(offset)
    }

    #[inline]
    fn entry(&self, index: u32) -> Option<&Entry<T>> {
        if index == u32::MAX {
            return None;
        }
        let (chunk_index, offset) = Self::split(index);
        self.chunks.get(chunk_index)?.as_ref()?.get(offset)
    }

    /// 写入一个条目。如果键的代数小于已存储条目的代数（陈旧句柄），则拒绝写入并返回 `false`。
    pub(crate) fn insert(&mut self, key: RawId, value: T) -> bool {
        let Some(entry) = self.entry_mut_growing(key.index) else {
            return false;
        };
        let can_write = match entry.as_ref() {
            Some((stored_generation, _)) => key.generation >= *stored_generation,
            None => true,
        };
        if can_write {
            *entry = Some((key.generation, value));
        }
        can_write
    }

    /// 读取一个条目。仅在代数完全匹配时返回引用。
    #[inline]
    pub(crate) fn get(&self, key: RawId) -> Option<&T> {
        match self.entry(key.index)? {
            Some((stored_generation, value)) if *stored_generation == key.generation => Some(value),
            _ => None,
        }
    }

    /// 可变借用读取一个条目。仅在代数完全匹配时返回可变引用。
    #[inline]
    pub(crate) fn get_mut(&mut self, key: RawId) -> Option<&mut T> {
        let (chunk_index, offset) = Self::split(key.index);
        let entry = self
            .chunks
            .get_mut(chunk_index)?
            .as_mut()?
            .get_mut(offset)?;
        match entry {
            Some((stored_generation, value)) if *stored_generation == key.generation => Some(value),
            _ => None,
        }
    }

    /// 检查指定键及其代数对应条目是否存在。
    #[inline]
    pub(crate) fn contains_key(&self, key: RawId) -> bool {
        self.get(key).is_some()
    }

    /// 移除一个条目并返回其值。若句柄已过期或不存在则返回 `None`。
    pub(crate) fn remove(&mut self, key: RawId) -> Option<T> {
        let (chunk_index, offset) = Self::split(key.index);
        let entry = self
            .chunks
            .get_mut(chunk_index)?
            .as_mut()?
            .get_mut(offset)?;
        match entry {
            Some((stored_generation, _)) if *stored_generation == key.generation => {
                entry.take().map(|(_, value)| value)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn test_arena_basic_ops() {
        let mut arena = Arena::<String>::new();
        let id1 = arena.insert("Hello".to_string());
        let id2 = arena.insert("World".to_string());

        assert_ne!(id1, id2);
        assert_eq!(arena.get(id1).map(String::as_str), Some("Hello"));
        assert_eq!(arena.get(id2).map(String::as_str), Some("World"));
        assert!(arena.remove(id1));
        assert_eq!(arena.get(id1), None);
        assert_eq!(arena.get(id2).map(String::as_str), Some("World"));
    }

    #[test]
    fn test_arena_reuse() {
        let mut arena = Arena::<u32>::new();
        let id1 = arena.insert(100);
        let index = id1.index;
        assert!(arena.remove(id1));
        let id2 = arena.insert(200);
        assert_eq!(id2.index, index);
        assert_ne!(id2.generation, id1.generation);
        assert_eq!(arena.get(id2), Some(&200));
        assert_eq!(arena.get(id1), None);
    }

    #[test]
    fn test_chunk_overflow() {
        let mut arena = Arena::<usize>::new();
        let count = CHUNK_SIZE * 3 + 10;
        let ids: Vec<_> = (0..count).map(|i| arena.insert(i)).collect();
        for (i, id) in ids.into_iter().enumerate() {
            assert_eq!(arena.get(id), Some(&i));
        }
    }

    #[test]
    fn test_sparse_secondary_map() {
        let mut arena = Arena::<()>::new();
        let mut map = SparseSecondaryMap::<String>::new();
        let id1 = arena.insert(());
        let id2 = arena.insert(());
        assert!(map.insert(id1, "Data1".to_string()));
        assert_eq!(map.get(id1).map(String::as_str), Some("Data1"));
        assert_eq!(map.get(id2), None);
        assert!(map.remove(id1).is_some());
        assert_eq!(map.get(id1), None);
    }

    #[test]
    fn test_secondary_map_aba_protection() {
        let mut arena = Arena::<()>::new();
        let mut map = SparseSecondaryMap::<String>::new();
        let id1 = arena.insert(());
        assert!(map.insert(id1, "Data1".to_string()));
        assert!(arena.remove(id1));
        let id2 = arena.insert(());
        assert_eq!(id1.index, id2.index);
        assert_ne!(id1.generation, id2.generation);
        assert_eq!(map.get(id2), None);
        assert!(map.insert(id2, "Data2".to_string()));
        assert_eq!(map.get(id2).map(String::as_str), Some("Data2"));
        assert_eq!(map.get(id1), None);
        assert!(map.remove(id1).is_none());
        assert_eq!(map.get(id2).map(String::as_str), Some("Data2"));
    }

    #[test]
    fn insert_reports_whether_it_actually_wrote() {
        let mut arena = Arena::<()>::new();
        let mut map = SparseSecondaryMap::<String>::new();
        let old = arena.insert(());
        assert!(arena.remove(old));
        let new = arena.insert(());
        assert!(map.insert(new, "new".to_string()));
        assert!(!map.insert(old, "stale".to_string()));
        assert_eq!(map.get(new).map(String::as_str), Some("new"));
    }

    #[test]
    fn dropping_the_arena_drops_every_live_value() {
        struct DropSpy(Rc<Cell<usize>>);
        impl Drop for DropSpy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let hits = Rc::new(Cell::new(0));
        {
            let mut arena = Arena::<DropSpy>::new();
            let a = arena.insert(DropSpy(hits.clone()));
            arena.insert(DropSpy(hits.clone()));
            assert!(arena.remove(a));
            assert_eq!(hits.get(), 1);
        }
        assert_eq!(hits.get(), 2);
    }

    #[test]
    fn dropping_the_secondary_map_drops_every_live_value() {
        struct DropSpy(Rc<Cell<usize>>);
        impl Drop for DropSpy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let hits = Rc::new(Cell::new(0));
        let mut arena = Arena::<()>::new();
        {
            let mut map = SparseSecondaryMap::<DropSpy, 4>::new();
            for _ in 0..10 {
                let id = arena.insert(());
                assert!(map.insert(id, DropSpy(hits.clone())));
            }
            assert_eq!(hits.get(), 0);
        }
        assert_eq!(hits.get(), 10);
    }

    #[test]
    fn removing_twice_is_a_noop() {
        let mut arena = Arena::<String>::new();
        let id = arena.insert("x".to_string());
        assert!(arena.remove(id));
        assert!(!arena.remove(id));
        assert_eq!(arena.get(id), None);
    }

    #[test]
    fn mutable_access_requires_exclusive_borrow() {
        let mut arena = Arena::<u32>::new();
        let mut map = SparseSecondaryMap::<u32>::new();
        let id = arena.insert(1);
        assert!(map.insert(id, 2));
        *arena.get_mut(id).expect("arena 条目存在") = 3;
        *map.get_mut(id).expect("旁路条目存在") = 4;
        assert_eq!(arena.get(id), Some(&3));
        assert_eq!(map.get(id), Some(&4));
    }
}
