#![no_std]
#![feature(
    ptr_metadata,
    layout_for_ptr,
    allocator_api,
    const_alloc_layout,
    unsize,
    maybe_uninit_write_slice
)]
// #![warn(missing_docs)]
//#![deny(clippy::missing_safety_doc)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
extern crate alloc;

mod inner;

#[cfg(feature = "alloc")]
use inner::handle_alloc_error;

use core::{
    alloc::{AllocError, Allocator},
    any::{Any, TypeId},
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::Unsize,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::read,
};
use inner::Inner;

#[cfg(feature = "alloc")]
pub struct SmallBox<T: ?Sized, Space, A: Allocator = alloc::alloc::Global>(Inner<T, Space, A>);

#[cfg(not(feature = "alloc"))]
pub struct SmallBox<T: ?Sized, Space, A: Allocator>(Inner<T, Space, A>);

impl<T: Sized, S, A: Allocator + Default> SmallBox<T, S, A> {
    #[inline]
    pub fn try_new(value: T) -> Result<Self, AllocError> {
        Self::try_new_in(value, A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new(value: T) -> Self {
        Self::new_in(value, A::default())
    }
}

impl<T: Sized, S, A: Allocator + Default> SmallBox<MaybeUninit<T>, S, A> {
    #[inline]
    pub fn try_new_uninit() -> Result<Self, AllocError> {
        Self::try_new_uninit_in(A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_uninit() -> Self {
        Self::new_uninit_in(A::default())
    }

    #[inline]
    pub fn try_new_zeroed() -> Result<Self, AllocError> {
        Self::try_new_zeroed_in(A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_zeroed() -> Self {
        Self::new_zeroed_in(A::default())
    }
}

impl<T: Sized, S, A: Allocator> SmallBox<MaybeUninit<T>, S, A> {
    #[inline]
    pub fn try_new_uninit_in(alloc: A) -> Result<Self, AllocError> {
        Ok(Self(Inner::try_new_uninit_in(alloc)?))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_uninit_in(alloc: A) -> Self {
        match Inner::try_new_uninit_in(alloc) {
            Ok(inner) => Self(inner),
            Err(_) => handle_alloc_error::<T>(()),
        }
    }

    #[inline]
    pub fn try_new_zeroed_in(alloc: A) -> Result<Self, AllocError> {
        Ok(Self(Inner::try_new_zeroed_in(alloc)?))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_zeroed_in(alloc: A) -> Self {
        match Inner::try_new_zeroed_in(alloc) {
            Ok(inner) => Self(inner),
            Err(_) => handle_alloc_error::<T>(()),
        }
    }

    #[inline]
    pub unsafe fn assume_init(self) -> SmallBox<T, S, A> {
        SmallBox(self.0.assume_init())
    }

    #[inline]
    pub fn write(mut self, value: T) -> SmallBox<T, S, A> {
        (*self).write(value);
        unsafe { self.assume_init() }
    }
}

impl<T: Sized, S, A: Allocator + Default> SmallBox<[MaybeUninit<T>], S, A> {
    #[inline]
    pub fn try_new_uninit_slice(len: usize) -> Result<Self, AllocError> {
        Self::try_new_uninit_slice_in(len, A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_uninit_slice(len: usize) -> Self {
        Self::new_uninit_slice_in(len, A::default())
    }

    #[inline]
    pub fn try_new_zeroed_slice(len: usize) -> Result<Self, AllocError> {
        Self::try_new_zeroed_slice_in(len, A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_zeroed_slice(len: usize) -> Self {
        Self::new_zeroed_slice_in(len, A::default())
    }
}

impl<T: Sized, S, A: Allocator> SmallBox<[MaybeUninit<T>], S, A> {
    #[inline]
    pub fn try_new_uninit_slice_in(len: usize, alloc: A) -> Result<Self, AllocError> {
        Ok(Self(Inner::try_new_uninit_slice_in(len, alloc)?))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_uninit_slice_in(len: usize, alloc: A) -> Self {
        match Inner::try_new_uninit_slice_in(len, alloc) {
            Ok(inner) => Self(inner),
            Err(_) => handle_alloc_error::<[T]>(len),
        }
    }

    #[inline]
    pub fn try_new_zeroed_slice_in(len: usize, alloc: A) -> Result<Self, AllocError> {
        Ok(Self(Inner::try_new_zeroed_slice_in(len, alloc)?))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_zeroed_slice_in(len: usize, alloc: A) -> Self {
        match Inner::try_new_zeroed_slice_in(len, alloc) {
            Ok(inner) => Self(inner),
            Err(_) => handle_alloc_error::<[T]>(len),
        }
    }

    #[inline]
    pub unsafe fn assume_init(self) -> SmallBox<[T], S, A> {
        SmallBox(self.0.assume_init())
    }
}

impl<T: Sized, S, A: Allocator> SmallBox<T, S, A> {
    pub const INLINED: bool = Inner::<T, S, A>::inlined(());

    #[inline]
    pub fn try_new_in(value: T, alloc: A) -> Result<Self, AllocError> {
        Ok(SmallBox::try_new_uninit_in(alloc)?.write(value))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn new_in(value: T, alloc: A) -> Self {
        match SmallBox::try_new_uninit_in(alloc) {
            Ok(uninit) => uninit.write(value),
            Err(_) => handle_alloc_error::<T>(()),
        }
    }

    #[inline]
    pub fn into_inner(boxed: Self) -> T {
        let uninit = Self::uninit(boxed);
        unsafe { read(uninit.as_ptr()) }
    }

    #[inline]
    pub fn uninit(boxed: Self) -> SmallBox<MaybeUninit<T>, S, A> {
        unsafe { SmallBox(boxed.0.reinterpret_unchecked()) }
    }
}

impl<T: Sized, S, A: Allocator> SmallBox<[T], S, A> {
    #[inline]
    pub fn uninit(boxed: Self) -> SmallBox<[MaybeUninit<T>], S, A> {
        unsafe { SmallBox(boxed.0.reinterpret_unchecked()) }
    }
}

impl<T: ?Sized, S, A: Allocator> SmallBox<T, S, A> {
    #[inline]
    pub const fn is_inlined(boxed: &Self) -> bool {
        boxed.0.is_inlined()
    }

    #[inline]
    pub fn allocator(boxed: &Self) -> &A {
        boxed.0.allocator()
    }

    #[inline]
    pub fn coerce<U: ?Sized>(boxed: Self) -> SmallBox<U, S, A>
    where
        T: Unsize<U>,
    {
        SmallBox(boxed.0.coerce())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub fn from_box(boxed: alloc::boxed::Box<T, A>) -> Self {
        Self(Inner::from_box(boxed))
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_into_box(boxed: Self) -> Result<alloc::boxed::Box<T, A>, Self> {
        match boxed.0.try_into_box() {
            Ok(boxed) => Ok(boxed),
            Err(inner) => Err(Self(inner)),
        }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn into_box(boxed: Self) -> alloc::boxed::Box<T, A> {
        match boxed.0.try_into_box() {
            Ok(boxed) => boxed,
            Err(inner) => handle_alloc_error::<T>(inner.metadata()),
        }
    }
}

impl<T: Any + ?Sized, S, A: Allocator> SmallBox<T, S, A> {
    #[inline]
    pub unsafe fn downcast_unchecked<U: Any>(self) -> SmallBox<U, S, A> {
        SmallBox(self.0.downcast_unchecked::<U>())
    }

    #[inline]
    pub fn downcast<U: Any>(self) -> Result<SmallBox<U, S, A>, Self> {
        if T::type_id(&self) == TypeId::of::<U>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
}

impl<T: ?Sized, S, A: Allocator> Deref for SmallBox<T, S, A> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized, S, A: Allocator> DerefMut for SmallBox<T, S, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: ?Sized, S, A: Allocator> AsRef<T> for SmallBox<T, S, A> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized, S, A: Allocator> AsMut<T> for SmallBox<T, S, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: ?Sized, S, A: Allocator> Borrow<T> for SmallBox<T, S, A> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized, S, A: Allocator> BorrowMut<T> for SmallBox<T, S, A> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<T: Sized + Default, S, A: Allocator + Default> Default for SmallBox<T, S, A> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<T: Sized, S, A: Allocator + Default> Default for SmallBox<[T], S, A> {
    #[inline]
    fn default() -> Self {
        unsafe { SmallBox::new_uninit_slice(0).assume_init() }
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<S, A: Allocator + Default> Default for SmallBox<str, S, A> {
    #[inline]
    fn default() -> Self {
        Self::clone_from("")
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<T: Sized, S, A: Allocator + Default> From<T> for SmallBox<T, S, A> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<T: Sized + Clone, S, A: Allocator + Clone> Clone for SmallBox<T, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new_clone_from_in(self, Self::allocator(self).clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.deref_mut().clone_from(source.deref());
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<T: Sized + Clone, S, A: Allocator + Clone> Clone for SmallBox<[T], S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new_clone_from_in(self, Self::allocator(self).clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        if self.len() == source.len() {
            self.clone_from_slice(source);
        } else {
            *self = source.clone();
        }
    }
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
impl<S, A: Allocator + Clone> Clone for SmallBox<str, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new_clone_from_in("", Self::allocator(self).clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        if self.len() == source.len() {
            unsafe {
                self.as_bytes_mut().copy_from_slice(source.as_bytes());
            }
        } else {
            *self = source.clone();
        }
    }
}

impl<T: ?Sized + fmt::Display, S, A: Allocator> fmt::Display for SmallBox<T, S, A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug, S, A: Allocator> fmt::Debug for SmallBox<T, S, A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized, S, A: Allocator> fmt::Pointer for SmallBox<T, S, A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ptr: *const T = &**self;
        fmt::Pointer::fmt(&ptr, f)
    }
}

impl<T: ?Sized + PartialEq, S, A: Allocator> PartialEq for SmallBox<T, S, A> {
    #[inline]
    fn eq(&self, other: &SmallBox<T, S, A>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}

impl<T: ?Sized + PartialOrd, S, A: Allocator> PartialOrd for SmallBox<T, S, A> {
    #[inline]
    fn partial_cmp(&self, other: &SmallBox<T, S, A>) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }

    #[inline]
    fn lt(&self, other: &SmallBox<T, S, A>) -> bool {
        PartialOrd::lt(&**self, &**other)
    }

    #[inline]
    fn le(&self, other: &SmallBox<T, S, A>) -> bool {
        PartialOrd::le(&**self, &**other)
    }

    #[inline]
    fn ge(&self, other: &SmallBox<T, S, A>) -> bool {
        PartialOrd::ge(&**self, &**other)
    }

    #[inline]
    fn gt(&self, other: &SmallBox<T, S, A>) -> bool {
        PartialOrd::gt(&**self, &**other)
    }
}

impl<T: ?Sized + Ord, S, A: Allocator> Ord for SmallBox<T, S, A> {
    #[inline]
    fn cmp(&self, other: &SmallBox<T, S, A>) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T: ?Sized + Eq, S, A: Allocator> Eq for SmallBox<T, S, A> {}

impl<T: ?Sized + Hash, S, A: Allocator> Hash for SmallBox<T, S, A> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

pub trait CloneFrom<T: ?Sized, A: Allocator>
where
    Self: Sized,
{
    fn try_new_clone_from_in(data: &T, alloc: A) -> Result<Self, AllocError>;

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    fn new_clone_from_in(data: &T, alloc: A) -> Self {
        match Self::try_new_clone_from_in(data, alloc) {
            Ok(boxed) => boxed,
            Err(_) => handle_alloc_error::<T>((data as *const T).to_raw_parts().1),
        }
    }
}

impl<T: Sized + Clone, S, A: Allocator> CloneFrom<T, A> for SmallBox<T, S, A> {
    #[inline]
    fn try_new_clone_from_in(data: &T, alloc: A) -> Result<Self, AllocError> {
        Self::try_new_in(data.clone(), alloc)
    }
}

impl<T: Sized + Clone, S, A: Allocator> CloneFrom<[T], A> for SmallBox<[T], S, A> {
    #[inline]
    fn try_new_clone_from_in(data: &[T], alloc: A) -> Result<Self, AllocError> {
        let mut boxed = SmallBox::try_new_uninit_slice_in(data.len(), alloc)?;
        MaybeUninit::write_slice_cloned(&mut boxed, data);
        unsafe { Ok(boxed.assume_init()) }
    }
}

impl<S, A: Allocator> CloneFrom<str, A> for SmallBox<str, S, A> {
    #[inline]
    fn try_new_clone_from_in(data: &str, alloc: A) -> Result<Self, AllocError> {
        Self::try_new_copy_from_in(data, alloc)
    }
}

pub trait CopyFrom<T: ?Sized, A: Allocator>
where
    Self: Sized,
{
    fn try_new_copy_from_in(data: &T, alloc: A) -> Result<Self, AllocError>;

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    fn new_copy_from_in(data: &T, alloc: A) -> Self {
        match Self::try_new_copy_from_in(data, alloc) {
            Ok(boxed) => boxed,
            Err(_) => handle_alloc_error::<T>((data as *const T).to_raw_parts().1),
        }
    }
}

impl<T: Sized + Copy, S, A: Allocator> CopyFrom<T, A> for SmallBox<T, S, A> {
    #[inline]
    fn try_new_copy_from_in(data: &T, alloc: A) -> Result<Self, AllocError> {
        Self::try_new_in(*data, alloc)
    }
}

impl<T: Sized + Copy, S, A: Allocator> CopyFrom<[T], A> for SmallBox<[T], S, A> {
    #[inline]
    fn try_new_copy_from_in(data: &[T], alloc: A) -> Result<Self, AllocError> {
        let mut boxed = SmallBox::try_new_uninit_slice_in(data.len(), alloc)?;
        MaybeUninit::write_slice(&mut boxed, data);
        unsafe { Ok(boxed.assume_init()) }
    }
}

impl<S, A: Allocator> CopyFrom<str, A> for SmallBox<str, S, A> {
    #[inline]
    fn try_new_copy_from_in(data: &str, alloc: A) -> Result<Self, AllocError> {
        let mut boxed =
            SmallBox::<[MaybeUninit<u8>], _, _>::try_new_uninit_slice_in(data.len(), alloc)?;
        MaybeUninit::write_slice(&mut boxed, data.as_bytes());
        unsafe { Ok(Self::from_utf8_unchecked(boxed.assume_init())) }
    }
}

impl<T: ?Sized, S, A: Allocator + Default> SmallBox<T, S, A>
where
    Self: CloneFrom<T, A>,
{
    #[inline]
    pub fn try_clone_from(data: &T) -> Result<Self, AllocError> {
        Self::try_new_clone_from_in(data, A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn clone_from(data: &T) -> Self {
        Self::new_clone_from_in(data, A::default())
    }
}

impl<T: ?Sized, S, A: Allocator + Default> SmallBox<T, S, A>
where
    Self: CopyFrom<T, A>,
{
    #[inline]
    pub fn try_copy_from(data: &T) -> Result<Self, AllocError> {
        Self::try_new_copy_from_in(data, A::default())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    #[cfg(not(no_global_oom_handling))]
    pub fn copy_from(data: &T) -> Self {
        Self::new_copy_from_in(data, A::default())
    }
}

impl<S, A: Allocator> SmallBox<str, S, A> {
    #[inline]
    pub unsafe fn from_utf8_unchecked(boxed: SmallBox<[u8], S, A>) -> Self {
        unsafe { Self(boxed.0.reinterpret_unchecked()) }
    }

    #[inline]
    pub fn into_bytes(self) -> SmallBox<[u8], S, A> {
        unsafe { SmallBox(self.0.reinterpret_unchecked()) }
    }
}
