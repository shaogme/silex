use alloc::boxed::Box;
use core::alloc::Layout;
use core::mem;
use core::ptr;

pub const SOO_CAPACITY: usize = 3 * mem::size_of::<usize>();

/// 通用的类型擦除容器，支持小对象优化 (SOO)。
/// V 类型通常是具体的 VTable 结构体。
pub struct AnyBox<V: 'static> {
    pub data: [usize; 3],
    pub vtable: &'static V,
}

impl<V: 'static> AnyBox<V> {
    /// 创建一个新的 AnyBox。
    /// 给定一个值、分配 VTable 的逻辑（栈/堆两种情况）。
    pub fn new<T: 'static>(value: T, vtable_stack: &'static V, vtable_heap: &'static V) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline =
            layout.size() <= SOO_CAPACITY && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            let mut data = [0usize; 3];
            unsafe {
                ptr::write(data.as_mut_ptr() as *mut T, value);
            }
            Self {
                data,
                vtable: vtable_stack,
            }
        } else {
            let mut data = [0usize; 3];
            let ptr = Box::into_raw(Box::new(value));
            unsafe {
                ptr::write(data.as_mut_ptr() as *mut *mut T, ptr);
            }
            Self {
                data,
                vtable: vtable_heap,
            }
        }
    }

    /// 获取底层数据的指针。
    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr() as *const u8
    }

    /// 获取底层数据的可变指针。
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr() as *mut u8
    }
}
