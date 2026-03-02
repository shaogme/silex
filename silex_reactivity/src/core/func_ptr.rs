use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

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

impl<F> FuncPtr<F> {
    /// 从一个具体的函数指针创建包装
    ///
    /// # Safety
    /// F 必须是一个函数指针类型 (fn type)，而不是闭包 trait。
    #[inline(always)]
    pub const fn new(f: F) -> Self {
        // 在编译期断言 F 的大小必须等于指针大小
        const {
            assert!(std::mem::size_of::<F>() == std::mem::size_of::<usize>());
        }

        // 使用 Union 实现 const 环境下的类型擦除转换
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
        unsafe {
            // 通过 transmute_copy 将 NonNull<()> 转换回函数指针 F
            // 由于 FuncPtr::new 保证了大小一致，这里是安全的
            std::mem::transmute_copy(&self.ptr)
        }
    }
}

// 确保 FuncPtr 的大小等于指针大小
const _: () = assert!(std::mem::size_of::<FuncPtr<fn()>>() == std::mem::size_of::<usize>());
// 确保 Option 优化生效
const _: () = assert!(std::mem::size_of::<Option<FuncPtr<fn()>>>() == std::mem::size_of::<usize>());
