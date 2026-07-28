use std::{
    alloc::{self, Layout},
    marker::PhantomData,
    mem::{align_of, needs_drop, replace, size_of},
    ptr::{self, NonNull},
    slice,
};

/// 为 `T` 定制的紧凑型堆动态数组。
///
/// 将长度与容量保存在堆内存的头部 (Header) 中，使得栈空间占用最小化（仅 1 个指针大小/单字）。
/// 类似于 `ThinVec`。
pub(crate) struct ThinVec<T> {
    /// 指向分配内存块的指针。
    /// 内存布局：`[Header][padding?][Data...]`
    /// 若为 `None`，表示尚未分配内存或为空。
    ptr: Option<NonNull<u8>>,
    _marker: PhantomData<T>,
}

#[repr(C)]
struct Header {
    len: usize,
    cap: usize,
}

/// 数据区相对于分配起始处的偏移。
///
/// 计算与 `Layout::new::<Header>().extend(Layout::array::<T>(..))` 导出的偏移一致，
/// 将 `size_of::<Header>()` 向上对齐到 `align_of::<T>()`。
/// 当 `align_of::<T>() > align_of::<Header>()` 时自动包含必要的 Padding 填充。
#[inline(always)]
const fn data_offset<T>() -> usize {
    let align = align_of::<T>();
    // align 一定是 2 的幂，因此可以用掩码向上取整。
    (size_of::<Header>() + align - 1) & !(align - 1)
}

/// `[Header][padding?][T; cap]` 的完整布局。
#[inline]
fn layout_of<T>(cap: usize) -> Layout {
    let (layout, offset) = Layout::new::<Header>()
        .extend(Layout::array::<T>(cap).expect("ThinVec: invalid array layout"))
        .expect("ThinVec: invalid allocation layout");
    debug_assert_eq!(offset, data_offset::<T>());
    layout
}

#[inline(always)]
fn header_ptr(base: NonNull<u8>) -> *mut Header {
    base.as_ptr().cast::<Header>()
}

/// # Safety
/// `base` 必须指向一个按 `layout_of::<T>` 分配、且 Header 已初始化的块。
#[inline(always)]
unsafe fn data_ptr<T>(base: NonNull<u8>) -> *mut T {
    unsafe { base.as_ptr().add(data_offset::<T>()).cast::<T>() }
}

/// # Safety
/// 同 [`data_ptr`]。
#[inline(always)]
unsafe fn len_of(base: NonNull<u8>) -> usize {
    unsafe { (*header_ptr(base)).len }
}

/// # Safety
/// 同 [`data_ptr`]。
#[inline(always)]
unsafe fn cap_of(base: NonNull<u8>) -> usize {
    unsafe { (*header_ptr(base)).cap }
}

/// # Safety
/// 同 [`data_ptr`]；调用者必须保证 `len` 个元素确实已初始化。
#[inline(always)]
unsafe fn set_len(base: NonNull<u8>, len: usize) {
    unsafe { (*header_ptr(base)).len = len };
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
        let base = match self.ptr {
            // SAFETY: `self.ptr` 为 Some 时分配与 Header 均已初始化。
            Some(base) if unsafe { len_of(base) } < unsafe { cap_of(base) } => base,
            Some(_) => {
                self.grow();
                self.ptr.expect("ThinVec::ptr should be Some after grow")
            }
            None => {
                self.grow_from_zero();
                self.ptr
                    .expect("ThinVec::ptr should be Some after grow_from_zero")
            }
        };

        // SAFETY: 上面保证了 base 有效且 len < cap；数据指针从覆盖整块分配的
        // base 派生，写入位置在容量之内。
        unsafe {
            let len = len_of(base);
            ptr::write(data_ptr::<T>(base).add(len), elem);
            set_len(base, len + 1);
        }
    }

    #[cold]
    fn grow_from_zero(&mut self) {
        let layout = layout_of::<T>(Self::MIN_CAP);

        // SAFETY: layout 的 size 非零（至少包含 Header）。
        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }

        // SAFETY: 分配成功，Header 位于偏移 0 且对齐正确。
        unsafe {
            let base = NonNull::new_unchecked(ptr);
            ptr::write(
                header_ptr(base),
                Header {
                    len: 0,
                    cap: Self::MIN_CAP,
                },
            );
            self.ptr = Some(base);
        }
    }

    #[cold]
    fn grow(&mut self) {
        let old_base = self.ptr.expect("ThinVec::grow called on an empty ThinVec");
        // SAFETY: `self.ptr` 为 Some 时 Header 已初始化。
        let old_cap = unsafe { cap_of(old_base) };
        let new_cap = old_cap * 2;

        let old_layout = layout_of::<T>(old_cap);
        let new_layout = layout_of::<T>(new_cap);

        // SAFETY: old_base 由同一个 old_layout 分配而来。
        let new_ptr = unsafe { alloc::realloc(old_base.as_ptr(), old_layout, new_layout.size()) };
        if new_ptr.is_null() {
            alloc::handle_alloc_error(new_layout);
        }

        // SAFETY: realloc 成功，数据区偏移只依赖 `align_of::<T>()`，扩容不会改变它。
        unsafe {
            let base = NonNull::new_unchecked(new_ptr);
            (*header_ptr(base)).cap = new_cap;
            self.ptr = Some(base);
        }
    }

    fn as_slice(&self) -> &[T] {
        match self.ptr {
            // SAFETY: 前 len 个元素已初始化，指针从整块分配的 base 派生。
            Some(base) => unsafe { slice::from_raw_parts(data_ptr::<T>(base), len_of(base)) },
            None => &[],
        }
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        match self.ptr {
            // SAFETY: 前 len 个元素已初始化，独占借用保证返回的切片不会别名。
            Some(base) => unsafe { slice::from_raw_parts_mut(data_ptr::<T>(base), len_of(base)) },
            None => &mut [],
        }
    }

    /// 取出唯一剩余的元素并把长度清零（保留分配）。
    /// 供 [`List`] 做 `Many -> Single` 降级使用，避免调用方越层操作内部布局。
    #[cfg(test)]
    fn take_only(&mut self) -> Option<T> {
        let base = self.ptr?;
        // SAFETY: len == 1 时 index 0 的元素已初始化；读出后把 len 置 0，
        // 所有权转移给调用方，后续 Drop 不会重复析构。
        unsafe {
            if len_of(base) != 1 {
                return None;
            }
            let only = data_ptr::<T>(base).read();
            set_len(base, 0);
            Some(only)
        }
    }
}

impl<T: PartialEq> ThinVec<T> {
    /// 移除第一个等于 `elem` 的元素。
    /// 若成功移除则返回 `true`。
    #[cfg(test)]
    fn remove(&mut self, elem: &T) -> bool {
        let Some(base) = self.ptr else { return false };

        // SAFETY: 切片覆盖已初始化的 len 个元素，指针从整块分配的 base 派生。
        unsafe {
            let len = len_of(base);
            let items = slice::from_raw_parts_mut(data_ptr::<T>(base), len);
            let Some(pos) = items.iter().position(|x| x == elem) else {
                return false;
            };

            // 注意：用 `slice::swap` 而不是从同一个切片取两个 `&mut` 再 `ptr::swap`
            // —— 后者是 Stacked Borrows 违规模式（AUDIT P4 附注）。
            items.swap(pos, len - 1);

            // 异常安全性：必须先递减长度，再执行 `drop_in_place`。
            // 避免 `T::drop()` 在 panic 时由于 len 尚未更新导致 `ThinVec::drop` 重复析构末尾元素（Double Drop UB）。
            set_len(base, len - 1);
            if needs_drop::<T>() {
                ptr::drop_in_place(&raw mut items[len - 1]);
            }
            true
        }
    }
}

impl<T> Drop for ThinVec<T> {
    fn drop(&mut self) {
        let Some(base) = self.ptr else { return };
        // SAFETY: base 由 layout_of::<T>(cap) 分配，前 len 个元素已初始化。
        unsafe {
            if needs_drop::<T>() {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                    data_ptr::<T>(base),
                    len_of(base),
                ));
            }
            alloc::dealloc(base.as_ptr(), layout_of::<T>(cap_of(base)));
        }
    }
}

impl<T: Clone> Clone for ThinVec<T> {
    fn clone(&self) -> Self {
        let Some(base) = self.ptr else {
            return Self::new();
        };

        // SAFETY: 新分配使用与源相同的 layout；先把 len 置 0 再逐个写入并递增，
        // 使得 `clone` panic 时已写入的元素能被 `out` 的 Drop 正确析构。
        unsafe {
            let len = len_of(base);
            let cap = cap_of(base);
            let layout = layout_of::<T>(cap);

            let raw = alloc::alloc(layout);
            if raw.is_null() {
                alloc::handle_alloc_error(layout);
            }
            let new_base = NonNull::new_unchecked(raw);
            ptr::write(header_ptr(new_base), Header { len: 0, cap });

            let out = Self {
                ptr: Some(new_base),
                _marker: PhantomData,
            };

            let src = data_ptr::<T>(base);
            let dst = data_ptr::<T>(new_base);
            for i in 0..len {
                ptr::write(dst.add(i), (*src.add(i)).clone());
                set_len(new_base, i + 1);
            }

            out
        }
    }
}

pub(crate) struct ThinVecIntoIter<T> {
    ptr: Option<NonNull<u8>>,
    idx: usize,
    len: usize,
    cap: usize,
    _marker: PhantomData<T>,
}

impl<T> Iterator for ThinVecIntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let base = self.ptr?;
        if self.idx >= self.len {
            return None;
        }
        // SAFETY: idx < len，该元素尚未被移出；读出后 idx 前进，不会重复读取。
        unsafe {
            let item = data_ptr::<T>(base).add(self.idx).read();
            self.idx += 1;
            Some(item)
        }
    }
}

impl<T> Drop for ThinVecIntoIter<T> {
    fn drop(&mut self) {
        let Some(base) = self.ptr else { return };
        // SAFETY: [idx, len) 是尚未被 `next` 移出的元素。
        unsafe {
            if needs_drop::<T>() {
                let data = data_ptr::<T>(base);
                for i in self.idx..self.len {
                    ptr::drop_in_place(data.add(i));
                }
            }
            alloc::dealloc(base.as_ptr(), layout_of::<T>(self.cap));
        }
    }
}

impl<T> IntoIterator for ThinVec<T> {
    type Item = T;
    type IntoIter = ThinVecIntoIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        match self.ptr.take() {
            // SAFETY: 所有权（含释放责任）随 `ptr` 一起转交给迭代器，
            // `self.ptr` 已被 take 为 None，`ThinVec::drop` 不会再碰这块分配。
            Some(base) => unsafe {
                ThinVecIntoIter {
                    ptr: Some(base),
                    idx: 0,
                    len: len_of(base),
                    cap: cap_of(base),
                    _marker: PhantomData,
                }
            },
            None => ThinVecIntoIter {
                ptr: None,
                idx: 0,
                len: 0,
                cap: 0,
                _marker: PhantomData,
            },
        }
    }
}

// --- List Wrapper ---

#[derive(Clone, Default)]
pub(crate) enum List<T> {
    #[default]
    Empty,
    Single(T),
    Many(ThinVec<T>),
}

impl<T> List<T> {
    pub(crate) fn push(&mut self, elem: T) {
        match replace(self, Self::Empty) {
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

    /// 就地借出全部元素切片。
    #[inline]
    pub(crate) fn as_slice(&self) -> &[T] {
        match self {
            Self::Empty => &[],
            Self::Single(val) => std::slice::from_ref(val),
            Self::Many(vec) => vec.as_slice(),
        }
    }

    #[inline]
    pub(crate) fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Self::Empty => &mut [],
            Self::Single(val) => std::slice::from_mut(val),
            Self::Many(vec) => vec.as_mut_slice(),
        }
    }
}

impl<T: PartialEq> List<T> {
    #[cfg(test)]
    pub(crate) fn remove(&mut self, elem: &T) {
        match self {
            Self::Empty => {}
            Self::Single(existing) => {
                if existing == elem {
                    *self = Self::Empty;
                }
            }
            Self::Many(vec) => {
                if vec.remove(elem)
                    && let Some(only) = vec.take_only()
                {
                    // `Many` 移除元素后剩 1 个元素时降级为 `Single`。
                    *self = Self::Single(only);
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
            List::Many(vec) => ListIntoIter::Many(vec.into_iter()),
        }
    }
}

pub(crate) enum ListIntoIter<T> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    fn collect<T: Clone>(list: &List<T>) -> Vec<T> {
        let mut out = Vec::new();
        out.extend_from_slice(list.as_slice());
        out
    }

    #[test]
    fn thin_vec_grows_past_initial_capacity() {
        let mut list = List::Empty;
        for i in 0..64u32 {
            list.push(i);
        }
        assert_eq!(collect(&list), (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn list_downgrades_many_to_single() {
        let mut list = List::Empty;
        list.push(1u32);
        list.push(2);
        list.push(3);
        assert!(matches!(list, List::Many(_)));

        list.remove(&2);
        // swap-remove：3 被换到 2 的位置
        assert_eq!(collect(&list), vec![1, 3]);

        list.remove(&3);
        assert!(matches!(list, List::Single(1)));

        list.remove(&1);
        assert!(matches!(list, List::Empty));
    }

    #[test]
    fn list_remove_missing_element_is_a_noop() {
        let mut list = List::Empty;
        list.push(1u32);
        list.push(2);
        list.remove(&99);
        assert_eq!(collect(&list), vec![1, 2]);
    }

    /// `align_of::<T>() > align_of::<Header>()` 时数据区前会有 padding，
    /// 指针推导必须用 `Layout::extend` 给出的偏移（AUDIT P19.3）。
    #[test]
    fn thin_vec_handles_over_aligned_elements() {
        #[derive(Clone, PartialEq, Debug)]
        #[repr(align(32))]
        struct OverAligned(u64);

        let mut list = List::Empty;
        for i in 0..10u64 {
            list.push(OverAligned(i));
        }
        assert_eq!(collect(&list), (0..10).map(OverAligned).collect::<Vec<_>>());

        list.remove(&OverAligned(5));
        assert_eq!(collect(&list).len(), 9);
    }

    #[derive(Clone)]
    struct DropCounter(Rc<Cell<usize>>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    impl PartialEq for DropCounter {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    #[test]
    fn thin_vec_drops_every_element_exactly_once() {
        let counter = Rc::new(Cell::new(0));
        {
            let mut list = List::Empty;
            for _ in 0..10 {
                list.push(DropCounter(counter.clone()));
            }
            assert_eq!(counter.get(), 0);
        }
        assert_eq!(counter.get(), 10);
    }

    #[test]
    fn many_to_single_downgrade_does_not_double_drop() {
        let counter = Rc::new(Cell::new(0));
        {
            let mut list = List::Empty;
            let a = DropCounter(Rc::new(Cell::new(0)));
            list.push(a.clone());
            list.push(DropCounter(counter.clone()));
            list.remove(&a);
            drop(a);
            assert_eq!(counter.get(), 0, "剩下的元素不应在降级时被析构");
        }
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn clone_is_a_deep_copy() {
        let counter = Rc::new(Cell::new(0));
        let mut list = List::Empty;
        for _ in 0..5 {
            list.push(DropCounter(counter.clone()));
        }
        let copy = list.clone();
        let mut seen = 0;
        seen += copy.as_slice().len();
        assert_eq!(seen, 5);
        drop(copy);
        assert_eq!(counter.get(), 5);
        drop(list);
        assert_eq!(counter.get(), 10);
    }

    #[test]
    fn into_iter_drops_the_unconsumed_tail() {
        let counter = Rc::new(Cell::new(0));
        let mut list = List::Empty;
        for _ in 0..6 {
            list.push(DropCounter(counter.clone()));
        }
        {
            let mut iter = list.into_iter();
            let first = iter.next().expect("first element");
            drop(first);
            assert_eq!(counter.get(), 1);
            // iter 在此处被 drop，剩余 5 个元素应被析构
        }
        assert_eq!(counter.get(), 6);
    }
}
