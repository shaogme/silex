use std::alloc::{self, Layout};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::slice;

/// A specialized, memory-efficient vector for `T`.
/// Is stores length and capacity in a heap header to keep the stack size small (1 word).
/// This is similar to `ThinVec`.
pub struct ThinVec<T> {
    /// Pointer to the allocation.
    /// Layout: [Header][Data...]
    /// If None, it's empty/unallocated.
    ptr: Option<NonNull<u8>>,
    _marker: PhantomData<T>,
}

#[repr(C)]
struct Header {
    len: usize,
    cap: usize,
}

impl Header {
    fn data_ptr<T>(&self) -> *const T {
        unsafe { (self as *const Header).add(1) as *const T }
    }

    fn data_ptr_mut<T>(&mut self) -> *mut T {
        unsafe { (self as *mut Header).add(1) as *mut T }
    }
}

impl<T> ThinVec<T> {
    const MIN_CAP: usize = 4;

    fn new() -> Self {
        Self {
            ptr: None,
            _marker: PhantomData,
        }
    }

    fn push(&mut self, elem: T) {
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

    fn len(&self) -> usize {
        self.ptr
            .map_or(0, |p| unsafe { p.cast::<Header>().as_ref().len })
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    unsafe fn write_at(&mut self, header: &mut Header, idx: usize, elem: T) {
        let data_ptr = header.data_ptr_mut::<T>();
        unsafe {
            ptr::write(data_ptr.add(idx), elem);
        }
        header.len += 1;
    }

    #[cold]
    fn grow_from_zero(&mut self) {
        let (layout, _) = Layout::new::<Header>()
            .extend(Layout::array::<T>(Self::MIN_CAP).unwrap())
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

        let (old_layout, _) = Layout::new::<Header>()
            .extend(Layout::array::<T>(old_cap).unwrap())
            .unwrap();

        let (new_layout, _) = Layout::new::<Header>()
            .extend(Layout::array::<T>(new_cap).unwrap())
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

    fn as_slice(&self) -> &[T] {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_ref();
                slice::from_raw_parts(header.data_ptr(), header.len)
            }
        } else {
            &[]
        }
    }
}

impl<T: PartialEq> ThinVec<T> {
    /// Removes the first occurrence of `elem`.
    /// Returns true if removed.
    fn remove(&mut self, elem: &T) -> bool {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_mut();
                let data_ptr = header.data_ptr_mut::<T>();
                let slice = slice::from_raw_parts_mut(data_ptr, header.len);

                if let Some(pos) = slice.iter().position(|x| x == elem) {
                    let len = header.len;
                    // Move the last element to current position
                    ptr::swap(
                        slice.get_unchecked_mut(pos),
                        slice.get_unchecked_mut(len - 1),
                    );

                    // Drop the removed element (now at the end) if necessary
                    if std::mem::needs_drop::<T>() {
                        ptr::drop_in_place(slice.get_unchecked_mut(len - 1));
                    }

                    header.len -= 1;
                    return true;
                }
            }
        }
        false
    }
}

impl<T> Drop for ThinVec<T> {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_mut();

                if std::mem::needs_drop::<T>() {
                    let data_ptr = header.data_ptr_mut::<T>();
                    let slice = slice::from_raw_parts_mut(data_ptr, header.len);
                    for item in slice {
                        ptr::drop_in_place(item);
                    }
                }

                let (layout, _) = Layout::new::<Header>()
                    .extend(Layout::array::<T>(header.cap).unwrap())
                    .unwrap();
                alloc::dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

impl<T: Clone> Clone for ThinVec<T> {
    fn clone(&self) -> Self {
        if let Some(ptr) = self.ptr {
            unsafe {
                let header: &Header = ptr.cast::<Header>().as_ref();
                let (layout, _) = Layout::new::<Header>()
                    .extend(Layout::array::<T>(header.cap).unwrap())
                    .unwrap();

                let new_ptr = alloc::alloc(layout);
                if new_ptr.is_null() {
                    alloc::handle_alloc_error(layout);
                }

                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr, std::mem::size_of::<Header>());

                let new_header = &mut *new_ptr.cast::<Header>();
                new_header.len = 0; // for exception safety

                let src_data = header.data_ptr::<T>();
                let dst_data = new_header.data_ptr_mut::<T>();

                for i in 0..header.len {
                    let src = &*src_data.add(i);
                    let cloned = src.clone();
                    ptr::write(dst_data.add(i), cloned);
                    new_header.len += 1;
                }

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

pub struct ThinVecIntoIter<T> {
    vec: ThinVec<T>,
    idx: usize,
}

impl<T> Iterator for ThinVecIntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ptr) = self.vec.ptr {
            unsafe {
                let header = ptr.cast::<Header>().as_ref();
                if self.idx < header.len {
                    // Use data_ptr
                    let data = header.data_ptr::<T>().add(self.idx).read();
                    self.idx += 1;
                    return Some(data);
                }
            }
        }
        None
    }
}

// --- List Wrapper ---

#[derive(Clone)]
pub enum List<T> {
    Empty,
    Single(T),
    Many(ThinVec<T>),
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self::Empty
    }
}

impl<T: Clone> List<T> {
    pub fn push(&mut self, elem: T) {
        match std::mem::replace(self, Self::Empty) {
            Self::Empty => *self = Self::Single(elem),
            Self::Single(val) => {
                let mut vec = ThinVec::new();
                vec.push(val);
                vec.push(elem);
                *self = Self::Many(vec);
            }
            Self::Many(mut vec) => {
                vec.push(elem);
                *self = Self::Many(vec);
            }
        }
    }

    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&T),
    {
        match self {
            Self::Empty => {}
            Self::Single(val) => f(val),
            Self::Many(vec) => {
                for item in vec.as_slice() {
                    f(item);
                }
            }
        }
    }
}

impl<T: PartialEq + Clone> List<T> {
    pub fn remove(&mut self, elem: &T) {
        match self {
            Self::Empty => {}
            Self::Single(existing) => {
                if existing == elem {
                    *self = Self::Empty;
                }
            }
            Self::Many(vec) => {
                if vec.remove(elem) {
                    if vec.len() == 1 {
                        unsafe {
                            let header = vec.ptr.unwrap().cast::<Header>().as_mut();
                            // Read the remaining element (at index 0)
                            let first = header.data_ptr::<T>().read();

                            // Important: Set len to 0 before vec is dropped when `*self` is overwritten
                            // This prevents double-drop of the element we just read out.
                            header.len = 0;

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

impl<T> IntoIterator for List<T> {
    type Item = T;
    type IntoIter = ListIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            List::Empty => ListIntoIter::Empty,
            List::Single(item) => ListIntoIter::Single(Some(item)),
            List::Many(vec) => ListIntoIter::Many(ThinVecIntoIter { vec, idx: 0 }),
        }
    }
}

pub enum ListIntoIter<T> {
    Empty,
    Single(Option<T>),
    Many(ThinVecIntoIter<T>),
}

impl<T> Iterator for ListIntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Single(opt) => opt.take(),
            Self::Many(iter) => iter.next(),
        }
    }
}
