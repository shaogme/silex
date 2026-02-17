use std::alloc::Layout;
use std::any::TypeId;
use std::mem::{self, MaybeUninit};
use std::ptr;

/// The size of the inline buffer in `usize` units.
/// 3 * 8 = 24 bytes on 64-bit systems.
/// This matches the size of `String`, `Vec<T>`, and acts as a good balance.
const INLINE_WORDS: usize = 3;

/// A type-erased value with Small Object Optimization (SOO).
///
/// Instead of using an enum with variants for every primitive type,
/// this struct uses a manual vtable strategy:
/// - If `T` fits in the buffer and has suitable alignment, it is stored inline.
/// - Otherwise, it is boxed and the `Box<T>` is stored inline (which fits easily).
///
/// Total size: 1 word (vtable) + 3 words (data) = 32 bytes on 64-bit.
pub(crate) struct AnyValue {
    vtable: &'static AnyValueVTable,
    data: MaybeUninit<[usize; INLINE_WORDS]>,
}

struct AnyValueVTable {
    type_id: TypeId,
    /// Get an immutable reference to the data.
    /// The argument is a pointer to the start of the data buffer.
    as_ptr: unsafe fn(*const usize) -> *const (),
    /// Get a mutable reference to the data.
    /// The argument is a pointer to the start of the data buffer.
    as_mut_ptr: unsafe fn(*mut usize) -> *mut (),
    /// Drop the value stored in the buffer.
    drop: unsafe fn(*mut usize),
}

impl AnyValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();

        // Check if we can store T inline.
        // Conditions:
        // 1. Size fits in the buffer.
        // 2. Alignment requirement is satisfied by [usize; N].
        //    [usize] has alignment of `mem::align_of::<usize>()`.
        let fits_inline = layout.size() <= (INLINE_WORDS * mem::size_of::<usize>())
            && layout.align() <= mem::align_of::<usize>();

        if fits_inline {
            unsafe {
                let mut data = MaybeUninit::<[usize; INLINE_WORDS]>::uninit();
                // Write value into data buffer.
                // We cast *mut usize -> *mut T. This is valid because we checked size and align.
                ptr::write(data.as_mut_ptr() as *mut T, value);

                AnyValue {
                    vtable: &InlineVTable::<T>::VTABLE,
                    data,
                }
            }
        } else {
            // Box it
            let boxed = Box::new(value);
            unsafe {
                let mut data = MaybeUninit::<[usize; INLINE_WORDS]>::uninit();
                // Write Box<T> into data buffer.
                // Box<T> is a pointer, so it fits in [usize; 3] and aligns to usize.
                ptr::write(data.as_mut_ptr() as *mut Box<T>, boxed);

                AnyValue {
                    vtable: &BoxedVTable::<T>::VTABLE,
                    data,
                }
            }
        }
    }

    pub(crate) fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = (self.vtable.as_ptr)(self.data.as_ptr() as *const usize);
                Some(&*(val_ptr as *const T))
            }
        } else {
            None
        }
    }

    pub(crate) fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.vtable.type_id == TypeId::of::<T>() {
            unsafe {
                let val_ptr = (self.vtable.as_mut_ptr)(self.data.as_mut_ptr() as *mut usize);
                Some(&mut *(val_ptr as *mut T))
            }
        } else {
            None
        }
    }
}

impl Drop for AnyValue {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(self.data.as_mut_ptr() as *mut usize);
        }
    }
}

// --- VTable Generators ---

trait VTableGen<T> {
    const VTABLE: AnyValueVTable;
}

/// VTable for values stored inline.
struct InlineVTable<T>(std::marker::PhantomData<T>);

impl<T: 'static> VTableGen<T> for InlineVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: |ptr| {
            // The buffer IS the value.
            ptr as *const T as *const ()
        },
        as_mut_ptr: |ptr| ptr as *mut T as *mut (),
        drop: |ptr| unsafe { ptr::drop_in_place(ptr as *mut T) },
    };
}

/// VTable for values stored in a Box (heap).
struct BoxedVTable<T>(std::marker::PhantomData<T>);

impl<T: 'static> VTableGen<T> for BoxedVTable<T> {
    const VTABLE: AnyValueVTable = AnyValueVTable {
        type_id: TypeId::of::<T>(),
        as_ptr: |ptr| unsafe {
            // The buffer contains a Box<T>.
            // 1. Cast buffer ptr to Box<T> ptr
            let box_ptr = ptr as *const Box<T>;
            // 2. Dereference Box<T> to get T
            (&**box_ptr) as *const T as *const ()
        },
        as_mut_ptr: |ptr| unsafe {
            let box_ptr = ptr as *mut Box<T>;
            (&mut **box_ptr) as *mut T as *mut ()
        },
        drop: |ptr| unsafe {
            // Drop the Box<T> residing in the buffer.
            ptr::drop_in_place(ptr as *mut Box<T>)
        },
    };
}
