use core::{
    alloc::{AllocError, Allocator, Layout},
    marker::{PhantomData, Unsize},
    mem::{forget, ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::{drop_in_place, from_raw_parts, from_raw_parts_mut, null, read, NonNull, Pointee},
};

union Data<S> {
    stack: ManuallyDrop<MaybeUninit<S>>,
    heap: NonNull<u8>,
}

impl<S> Data<S> {
    #[inline]
    fn try_new_uninit_in<T: ?Sized, A: Allocator>(
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, AllocError> {
        if Self::inlined::<T>(metadata) {
            Ok(Self {
                stack: ManuallyDrop::new(MaybeUninit::uninit()),
            })
        } else {
            Ok(Self {
                heap: alloc.allocate(layout_from_metadata::<T>(metadata))?.cast(),
            })
        }
    }

    #[inline]
    fn try_new_zeroed_in<T: ?Sized, A: Allocator>(
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, AllocError> {
        if Self::inlined::<T>(metadata) {
            Ok(Self {
                stack: ManuallyDrop::new(MaybeUninit::uninit()),
            })
        } else {
            Ok(Self {
                heap: alloc
                    .allocate_zeroed(layout_from_metadata::<T>(metadata))?
                    .cast(),
            })
        }
    }

    #[inline]
    fn as_ptr<T: ?Sized>(&self, metadata: <T as Pointee>::Metadata) -> (*const T, bool) {
        if Self::inlined::<T>(metadata) {
            (
                from_raw_parts(unsafe { &self.stack as *const _ as *const () }, metadata),
                false,
            )
        } else {
            (
                from_raw_parts(unsafe { self.heap.as_ptr() as *const () }, metadata),
                true,
            )
        }
    }

    #[inline]
    fn as_mut_ptr<T: ?Sized>(&mut self, metadata: <T as Pointee>::Metadata) -> (*mut T, bool) {
        if Self::inlined::<T>(metadata) {
            (
                from_raw_parts_mut(unsafe { &mut self.stack as *mut _ as *mut () }, metadata),
                false,
            )
        } else {
            (
                from_raw_parts_mut(unsafe { self.heap.as_ptr() as *mut () }, metadata),
                true,
            )
        }
    }

    #[inline]
    const fn inlined<T: ?Sized>(metadata: <T as Pointee>::Metadata) -> bool {
        let store = Layout::new::<S>();
        let layout = layout_from_metadata::<T>(metadata);

        layout.size() <= store.size() && layout.align() <= store.align()
    }
}

pub struct Inner<T: ?Sized, S, A: Allocator> {
    phantom: PhantomData<T>,
    metadata: <T as Pointee>::Metadata,
    data: Data<S>,
    alloc: A,
}

impl<T: Sized, S, A: Allocator> Inner<MaybeUninit<T>, S, A> {
    #[inline]
    pub fn try_new_uninit_in(alloc: A) -> Result<Self, AllocError> {
        Ok(Self {
            phantom: PhantomData,
            metadata: (),
            data: Data::try_new_uninit_in::<T, _>((), &alloc)?,
            alloc,
        })
    }

    #[inline]
    pub fn try_new_zeroed_in(alloc: A) -> Result<Self, AllocError> {
        Ok(Self {
            phantom: PhantomData,
            metadata: (),
            data: Data::try_new_zeroed_in::<T, _>((), &alloc)?,
            alloc,
        })
    }

    #[inline]
    pub unsafe fn assume_init(self) -> Inner<T, S, A> {
        let (data, metadata, alloc) = self.into_parts();

        Inner {
            phantom: PhantomData,
            metadata,
            data,
            alloc,
        }
    }
}

impl<T: Sized, S, A: Allocator> Inner<[MaybeUninit<T>], S, A> {
    #[inline]
    pub fn try_new_uninit_slice_in(len: usize, alloc: A) -> Result<Self, AllocError> {
        Ok(Self {
            phantom: PhantomData,
            metadata: len,
            data: Data::try_new_uninit_in::<[T], _>(len, &alloc)?,
            alloc,
        })
    }

    #[inline]
    pub fn try_new_zeroed_slice_in(len: usize, alloc: A) -> Result<Self, AllocError> {
        Ok(Self {
            phantom: PhantomData,
            metadata: len,
            data: Data::try_new_zeroed_in::<[T], _>(len, &alloc)?,
            alloc,
        })
    }

    #[inline]
    pub unsafe fn assume_init(self) -> Inner<[T], S, A> {
        let (data, metadata, alloc) = self.into_parts();

        Inner {
            phantom: PhantomData,
            metadata,
            data,
            alloc,
        }
    }
}

impl<T: ?Sized, S, A: Allocator> Inner<T, S, A> {
    #[inline]
    pub const fn inlined(metadata: <T as Pointee>::Metadata) -> bool {
        Data::<S>::inlined::<T>(metadata)
    }

    #[inline]
    pub const fn is_inlined(&self) -> bool {
        Self::inlined(self.metadata)
    }

    #[inline]
    fn into_parts(self) -> (Data<S>, <T as Pointee>::Metadata, A) {
        unsafe {
            let metadata = self.metadata;
            let data = read(&self.data as *const _);
            let alloc = read(&self.alloc as *const _);

            forget(self);
            (data, metadata, alloc)
        }
    }

    #[inline]
    pub fn allocator(&self) -> &A {
        &self.alloc
    }

    #[inline]
    pub fn coerce<U: ?Sized>(self) -> Inner<U, S, A>
    where
        T: Unsize<U>,
    {
        let (data, metadata, alloc) = self.into_parts();

        Inner {
            phantom: PhantomData,
            metadata: coerce_metadata::<U, T>(metadata),
            data,
            alloc,
        }
    }

    #[inline]
    pub unsafe fn reinterpret_unchecked<
        U: ?Sized + Pointee<Metadata = <T as Pointee>::Metadata>,
    >(
        self,
    ) -> Inner<U, S, A> {
        let (data, metadata, alloc) = self.into_parts();

        Inner {
            phantom: PhantomData,
            metadata,
            data,
            alloc,
        }
    }

    #[inline]
    pub unsafe fn downcast_unchecked<U: Sized>(self) -> Inner<U, S, A> {
        let (data, _, alloc) = self.into_parts();

        Inner {
            phantom: PhantomData,
            metadata: (),
            data,
            alloc,
        }
    }
}

impl<T: ?Sized, S, A: Allocator> Drop for Inner<T, S, A> {
    #[inline]
    fn drop(&mut self) {
        let (ptr, heap) = self.data.as_mut_ptr::<T>(self.metadata);

        unsafe {
            drop_in_place(ptr);

            if heap {
                self.alloc.deallocate(
                    NonNull::new_unchecked(ptr).cast(),
                    layout_from_metadata::<T>(self.metadata),
                )
            }
        }
    }
}

impl<T: ?Sized, S, A: Allocator> Deref for Inner<T, S, A> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data.as_ptr::<T>(self.metadata).0 }
    }
}

impl<T: ?Sized, S, A: Allocator> DerefMut for Inner<T, S, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data.as_mut_ptr::<T>(self.metadata).0 }
    }
}

unsafe impl<T: ?Sized + Send, S, A: Allocator> Send for Inner<T, S, A> {}
unsafe impl<T: ?Sized + Sync, S, A: Allocator> Sync for Inner<T, S, A> {}

#[inline(always)]
const fn layout_from_metadata<T: ?Sized>(metadata: <T as Pointee>::Metadata) -> Layout {
    unsafe { Layout::for_value_raw(from_raw_parts::<T>(null(), metadata)) }
}

#[inline(always)]
const fn coerce_metadata<U: ?Sized, T: ?Sized>(
    metadata: <T as Pointee>::Metadata,
) -> <U as Pointee>::Metadata
where
    T: Unsize<U>,
{
    let ptr = from_raw_parts::<T>(null(), metadata) as *const U;
    let (_, metadata) = ptr.to_raw_parts();
    metadata
}

#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
pub fn handle_alloc_error<T: ?Sized>(metadata: <T as Pointee>::Metadata) -> ! {
    alloc::alloc::handle_alloc_error(layout_from_metadata::<T>(metadata))
}
