use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ptr::NonNull;

/// 一个包装了任意函数指针的结构体
///
/// - 内存布局：等同于 *mut ()，即一个机器字长
/// - 类型安全：通过 PhantomData<F> 携带函数签名
/// - 空指针优化：Option<FuncPtr<F>> 的大小仍然是一个机器字长
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FuncPtr<F> {
    // 内部擦除为 void 指针存储，NonNull 保证了 Option 优化
    ptr: NonNull<()>,
    // 幽灵数据，占据 0 字节，仅用于编译器类型检查
    _marker: PhantomData<F>,
}

unsafe impl<F> Sync for FuncPtr<F> {}
unsafe impl<F> Send for FuncPtr<F> {}

impl<F> FuncPtr<F> {
    /// 从一个具体的函数指针创建包装
    ///
    /// # Safety
    /// F 必须是一个函数指针类型 (fn type)，而不是闭包 trait。
    #[inline(always)]
    pub const fn new(f: F) -> Self {
        const {
            assert!(core::mem::size_of::<F>() == core::mem::size_of::<usize>());
        }

        #[repr(C)]
        union Transmute<F> {
            f: ManuallyDrop<F>,
            ptr: NonNull<()>,
        }

        unsafe {
            let t = Transmute {
                f: ManuallyDrop::new(f),
            };
            Self {
                ptr: t.ptr,
                _marker: PhantomData,
            }
        }
    }

    /// 获取原始函数指针
    #[inline(always)]
    pub fn as_fn(&self) -> F {
        unsafe { core::mem::transmute_copy(&self.ptr) }
    }
}

const _: () = assert!(core::mem::size_of::<FuncPtr<fn()>>() == core::mem::size_of::<usize>());
const _: () =
    assert!(core::mem::size_of::<Option<FuncPtr<fn()>>>() == core::mem::size_of::<usize>());
