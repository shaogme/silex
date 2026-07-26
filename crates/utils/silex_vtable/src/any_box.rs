use alloc::boxed::Box;
use core::{alloc::Layout, mem, mem::MaybeUninit, ptr};

pub const SOO_CAPACITY: usize = 3 * mem::size_of::<usize>();

/// 内联存储缓冲区：`SOO_CAPACITY` 字节，对齐到 `align_of::<usize>()`。
///
/// 必须使用 `MaybeUninit<u8>` 字节数组，**不能**使用 `[usize; 3]`：
/// 整数类型的加载会丢弃指针 provenance，把 `Box`/`Rc`/vtable 指针写进整数数组
/// 再按值搬运，之后当指针解引用即为未定义行为（Miri 可直接复现）。
/// 字节数组的移动是逐字节的，provenance 得以保留。
///
/// # 不变量
///
/// 缓冲区在构造时**整体零初始化**。`AnyValue` 的按位比较 / 按位克隆路径
/// 会按完整的 `SOO_CAPACITY` 长度读取字节，依赖这一点来避免读到未初始化内存。
/// 该路径只对无填充字节的标量类型启用。
#[repr(C)]
pub struct InlineStorage {
    /// 零长度数组，仅用于把对齐提升到 `align_of::<usize>()`，不占用任何空间。
    _align: [usize; 0],
    bytes: [MaybeUninit<u8>; SOO_CAPACITY],
}

impl InlineStorage {
    /// 全零初始化的缓冲区。
    #[inline(always)]
    pub const fn zeroed() -> Self {
        Self {
            _align: [],
            bytes: [MaybeUninit::new(0); SOO_CAPACITY],
        }
    }

    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr().cast()
    }

    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr().cast()
    }

    /// 判断 `T` 是否能内联存放在偏移 `offset` 处。
    #[inline(always)]
    pub const fn fits<T>(offset: usize) -> bool {
        offset + mem::size_of::<T>() <= SOO_CAPACITY
            && mem::align_of::<T>() <= mem::align_of::<usize>()
    }

    /// 在偏移 `offset` 处写入一个值（不读取、也不析构原有内容）。
    ///
    /// # Safety
    ///
    /// - `Self::fits::<T>(offset)` 必须为真；
    /// - `offset` 必须满足 `T` 的对齐要求；
    /// - 调用者负责在合适的时机析构写入的值。
    #[inline(always)]
    pub unsafe fn write<T>(&mut self, offset: usize, value: T) {
        debug_assert!(Self::fits::<T>(offset));
        debug_assert!(offset.is_multiple_of(mem::align_of::<T>()));
        unsafe { ptr::write(self.as_mut_ptr().add(offset).cast::<T>(), value) };
    }
}

/// 通用的类型擦除容器，支持小对象优化 (SOO)。
/// V 类型通常是具体的 VTable 结构体。
pub struct AnyBox<V: 'static> {
    pub data: InlineStorage,
    pub vtable: &'static V,
}

impl<V: 'static> AnyBox<V> {
    /// 创建一个新的 AnyBox。
    /// 给定一个值、分配 VTable 的逻辑（栈/堆两种情况）。
    pub fn new<T: 'static>(value: T, vtable_stack: &'static V, vtable_heap: &'static V) -> Self {
        let layout = Layout::new::<T>();
        let fits_inline =
            layout.size() <= SOO_CAPACITY && layout.align() <= mem::align_of::<usize>();

        let mut data = InlineStorage::zeroed();
        if fits_inline {
            // SAFETY: 刚刚检查过 T 的大小与对齐都放得下。
            unsafe { data.write(0, value) };
            Self {
                data,
                vtable: vtable_stack,
            }
        } else {
            // SAFETY: 写入的是一个裸指针，必然放得下；
            // 所有权由 vtable_heap 的 drop 分支负责释放。
            unsafe { data.write(0, Box::into_raw(Box::new(value))) };
            Self {
                data,
                vtable: vtable_heap,
            }
        }
    }

    /// 获取底层数据的指针。
    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    /// 获取底层数据的可变指针。
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }
}
