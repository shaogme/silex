#![no_std]

extern crate alloc;

pub mod any_box;
pub mod func_ptr;
pub mod thunk;

pub use any_box::{AnyBox, SOO_CAPACITY};
pub use func_ptr::FuncPtr;
pub use thunk::{FactoryBox, FnBox, OnceBox};
