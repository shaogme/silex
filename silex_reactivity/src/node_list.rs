use std::alloc::{self, Layout};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::slice;

use crate::arena::Index as NodeId;

/// A specialized, memory-efficient vector for `NodeId`s.
/// It stores length and capacity in a heap header to keep the stack size small (1 word).
/// This is similar to `ThinVec`.
pub struct NodeVec {
    /// Pointer to the allocation.
    /// Layout: [Header][Data...]
    /// If None, it's empty/unallocated.
    ptr: Option<NonNull<u8>>,
    _marker: PhantomData<NodeId>,
}

#[repr(C)]
struct Header {
    len: usize,
    cap: usize,
}

impl Header {
    fn data_ptr(&self) -> *const NodeId {
        unsafe { (self as *const Header).add(1) as *const NodeId }
    }

    fn data_ptr_mut(&mut self) -> *mut NodeId {
        unsafe { (self as *mut Header).add(1) as *mut NodeId }
    }
}

impl NodeVec {
    const MIN_CAP: usize = 4;

    fn new() -> Self {
        Self {
            ptr: None,
            _marker: PhantomData,
        }
    }

    fn push(&mut self, elem: NodeId) {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_mut();
                if header.len == header.cap {
                    self.grow();
                    // ptr might have changed
                    let header = self.ptr.unwrap().cast::<Header>().as_mut();
                    self.write_at(header, header.len, elem);
                } else {
                    self.write_at(header, header.len, elem);
                }
            }
        } else {
            self.grow_from_zero();
            let header = unsafe { self.ptr.unwrap().cast::<Header>().as_mut() };
            unsafe { self.write_at(header, 0, elem) };
        }
    }

    /// Removes the first occurrence of `elem`.
    /// Returns true if removed.
    fn remove(&mut self, elem: NodeId) -> bool {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_mut();
                let data_ptr = header.data_ptr_mut();
                let slice = slice::from_raw_parts_mut(data_ptr, header.len);

                if let Some(pos) = slice.iter().position(|&x| x == elem) {
                    // swap_remove
                    let len = header.len;
                    ptr::swap(
                        slice.get_unchecked_mut(pos),
                        slice.get_unchecked_mut(len - 1),
                    );
                    header.len -= 1;
                    return true;
                }
            }
        }
        false
    }

    fn len(&self) -> usize {
        self.ptr
            .map_or(0, |p| unsafe { p.cast::<Header>().as_ref().len })
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    unsafe fn write_at(&mut self, header: &mut Header, idx: usize, elem: NodeId) {
        let data_ptr = header.data_ptr_mut();
        unsafe {
            ptr::write(data_ptr.add(idx), elem);
        }
        header.len += 1;
    }

    #[cold]
    fn grow_from_zero(&mut self) {
        let (layout, _) = Layout::new::<Header>()
            .extend(Layout::array::<NodeId>(Self::MIN_CAP).unwrap())
            .unwrap();

        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }

        unsafe {
            let header_ptr = ptr as *mut Header;
            ptr::write(
                header_ptr,
                Header {
                    len: 0,
                    cap: Self::MIN_CAP,
                },
            );
            self.ptr = Some(NonNull::new_unchecked(ptr));
        }
    }

    #[cold]
    fn grow(&mut self) {
        let old_ptr = self.ptr.unwrap();
        let unsafe_header = unsafe { old_ptr.cast::<Header>().as_ref() };
        let old_cap = unsafe_header.cap;
        let new_cap = old_cap * 2;

        let (old_layout, _offset) = Layout::new::<Header>()
            .extend(Layout::array::<NodeId>(old_cap).unwrap())
            .unwrap();

        // This must match the calculation in grow_from_zero logic for alignment padding
        let (new_layout, _) = Layout::new::<Header>()
            .extend(Layout::array::<NodeId>(new_cap).unwrap())
            .unwrap();

        let new_ptr = unsafe { alloc::realloc(old_ptr.as_ptr(), old_layout, new_layout.size()) };

        if new_ptr.is_null() {
            alloc::handle_alloc_error(new_layout);
        }

        unsafe {
            let header_ptr = new_ptr as *mut Header;
            (*header_ptr).cap = new_cap;
            self.ptr = Some(NonNull::new_unchecked(new_ptr));
        }
    }
}

impl Drop for NodeVec {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_ref();
                let (layout, _) = Layout::new::<Header>()
                    .extend(Layout::array::<NodeId>(header.cap).unwrap())
                    .unwrap();
                // NodeId is Copy, so no need to drop elements manually.
                alloc::dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

impl Clone for NodeVec {
    fn clone(&self) -> Self {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header: &Header = ptr.cast::<Header>().as_ref();
                let (layout, _) = Layout::new::<Header>()
                    .extend(Layout::array::<NodeId>(header.cap).unwrap())
                    .unwrap();

                let new_ptr = alloc::alloc(layout);
                if new_ptr.is_null() {
                    alloc::handle_alloc_error(layout);
                }

                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr, layout.size());

                Self {
                    ptr: Some(NonNull::new_unchecked(new_ptr)),
                    _marker: PhantomData,
                }
            }
        } else {
            Self::new()
        }
    }
}

// Iterator implementation
pub struct NodeVecIntoIter {
    vec: NodeVec,
    idx: usize,
}

impl Iterator for NodeVecIntoIter {
    type Item = NodeId;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ptr) = self.vec.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_ref();
                if self.idx < header.len {
                    // Use data_ptr which takes &self
                    let data = header.data_ptr().add(self.idx).read();
                    self.idx += 1;
                    return Some(data);
                }
            }
        }
        None
    }
}

// --- NodeList Wrapper ---

#[derive(Clone)]
pub(crate) enum NodeList {
    Empty,
    Single(NodeId),
    Many(NodeVec),
}

impl Default for NodeList {
    fn default() -> Self {
        Self::Empty
    }
}

impl NodeList {
    pub(crate) fn push(&mut self, id: NodeId) {
        match self {
            Self::Empty => *self = Self::Single(id),
            Self::Single(existing) => {
                let mut vec = NodeVec::new();
                vec.push(*existing);
                vec.push(id);
                *self = Self::Many(vec);
            }
            Self::Many(vec) => vec.push(id),
        }
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        match self {
            Self::Empty => {}
            Self::Single(existing) => {
                if *existing == id {
                    *self = Self::Empty;
                }
            }
            Self::Many(vec) => {
                if vec.remove(id) {
                    if vec.len() == 1 {
                        // Downgrade to Single
                        unsafe {
                            let header = vec.ptr.unwrap().cast::<Header>().as_ref();
                            let first = *header.data_ptr(); // index 0, reads via const ptr
                            *self = Self::Single(first);
                        }
                    } else if vec.is_empty() {
                        *self = Self::Empty;
                    }
                }
            }
        }
    }
}

impl IntoIterator for NodeList {
    type Item = NodeId;
    type IntoIter = NodeListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            NodeList::Empty => NodeListIntoIter::Empty,
            NodeList::Single(id) => NodeListIntoIter::Single(Some(id)),
            NodeList::Many(vec) => NodeListIntoIter::Many(NodeVecIntoIter { vec, idx: 0 }),
        }
    }
}

pub(crate) enum NodeListIntoIter {
    Empty,
    Single(Option<NodeId>),
    Many(NodeVecIntoIter),
}

impl Iterator for NodeListIntoIter {
    type Item = NodeId;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Single(opt) => opt.take(),
            Self::Many(iter) => iter.next(),
        }
    }
}
