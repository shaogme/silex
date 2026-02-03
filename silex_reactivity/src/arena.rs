use std::alloc::{Layout, alloc};
use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::ptr;

const CHUNK_SIZE: usize = 128;

/// Strong typed index with generation counter to detect ABA problems.
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
        self.generation % 2 > 0
    }
}

impl<T> Drop for Slot<T> {
    fn drop(&mut self) {
        if core::mem::needs_drop::<T>() && self.occupied() {
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
            std::alloc::handle_alloc_error(layout);
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
    pub fn insert(&self, value: T) -> Index {
        // SAFETY:
        // We acquire pointers to internal state.
        // This is safe provided we follow single-threaded (thread_local) rules or
        // ensure no other concurrent mutable access exists (which RefCell/logic should ensure).

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

    /// Access mutable element by Index.
    /// Warning: This takes &self to allow interior mutability patterns (e.g. inside Reacitivity Runtime).
    /// CALLER MUST ENSURE EXCLUSIVE ACCESS to the specific 'T' being mutated.
    /// Creating multiple &mut T to the same Index is Undefined Behavior.
    #[allow(clippy::mut_from_ref)]
    pub fn get_mut(&self, id: Index) -> Option<&mut T> {
        let (chunk_idx, offset) = self.get_chunk_offset(id.index);
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }

            if id.index as usize >= *self.len.get() {
                return None;
            }

            let slot = &mut *chunks[chunk_idx].slots[offset].get();
            if slot.generation != id.generation {
                return None;
            }

            if slot.occupied() {
                Some(&mut slot.u.value)
            } else {
                None
            }
        }
    }

    /// Remove element.
    /// Returns true if removed, false if not found/already removed.
    pub fn remove(&self, id: Index) -> bool {
        let (chunk_idx, offset) = self.get_chunk_offset(id.index);

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

    #[inline]
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

// For SecondaryMap equivalent, we can use a simpler structure since keys are stable.
// We don't need generation checks here if we assume the caller (Reactivity Runtime)
// manages validity via the main Arena. If main Arena says Id is valid, this map
// is valid.
pub struct SparseSecondaryMap<T> {
    chunks: UnsafeCell<Vec<Option<Box<[UnsafeCell<Option<T>>]>>>>,
}

impl<T> SparseSecondaryMap<T> {
    pub fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
        }
    }

    pub fn insert(&self, key: Index, value: T) {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);

        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                chunks.resize_with(chunk_idx + 1, || None);
            }

            if chunks[chunk_idx].is_none() {
                // Initialize chunk entries to None
                // We construct a specific layout matching Box<[UnsafeCell<Option<T>>]>
                let vec_chunk: Vec<UnsafeCell<Option<T>>> =
                    (0..CHUNK_SIZE).map(|_| UnsafeCell::new(None)).collect();
                chunks[chunk_idx] = Some(vec_chunk.into_boxed_slice());
            }

            if let Some(ref mut chunk) = chunks[chunk_idx] {
                *chunk[offset].get() = Some(value);
            }
        }
    }

    pub fn get(&self, key: Index) -> Option<&T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        unsafe {
            let chunks = &*self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref chunk) = chunks[chunk_idx] {
                let slot = &*chunk[offset].get();
                slot.as_ref()
            } else {
                None
            }
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get_mut(&self, key: Index) -> Option<&mut T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref mut chunk) = chunks[chunk_idx] {
                let slot = &mut *chunk[offset].get();
                slot.as_mut()
            } else {
                None
            }
        }
    }

    pub fn remove(&self, key: Index) -> Option<T> {
        let (chunk_idx, offset) = self.get_chunk_offset(key.index);
        unsafe {
            let chunks = &mut *self.chunks.get();
            if chunk_idx >= chunks.len() {
                return None;
            }
            if let Some(ref mut chunk) = chunks[chunk_idx] {
                let slot = &mut *chunk[offset].get();
                slot.take()
            } else {
                None
            }
        }
    }

    /// Remove logic if ID is just u32 (for direct internal usage if needed)
    fn get_chunk_offset(&self, index: u32) -> (usize, usize) {
        let idx = index as usize;
        (idx / CHUNK_SIZE, idx % CHUNK_SIZE)
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
}
