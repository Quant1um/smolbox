use core::{
    alloc::{AllocError, Allocator, Layout},
    marker::{PhantomData, Unsize},
    mem::{forget, ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::{
        copy_nonoverlapping, drop_in_place, from_raw_parts, from_raw_parts_mut, null, read,
        NonNull, Pointee,
    },
};

struct Stack<S>(MaybeUninit<S>);

impl<S> Stack<S> {
    #[inline]
    fn new_uninit() -> Self {
        Self(MaybeUninit::uninit())
    }

    #[inline]
    fn new_zeroed() -> Self {
        Self(MaybeUninit::zeroed())
    }

    #[inline]
    unsafe fn from_stack<T: ?Sized, Z>(src: Stack<Z>, metadata: <T as Pointee>::Metadata) -> Self {
        let layout = layout_from_metadata::<T>(metadata);
        let mut stack = Self::new_uninit();

        copy_nonoverlapping(
            src.as_ptr::<u8>(()),
            stack.as_mut_ptr::<u8>(()),
            layout.size(),
        );

        stack
    }

    #[inline]
    unsafe fn from_heap<T: ?Sized, A: Allocator>(
        heap: Heap,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Self {
        let layout = layout_from_metadata::<T>(metadata);
        let mut stack = Self::new_uninit();

        copy_nonoverlapping(
            heap.as_ptr::<u8>(()),
            stack.as_mut_ptr::<u8>(()),
            layout.size(),
        );

        heap.deallocate::<T, _>(metadata, alloc);

        stack
    }

    #[inline]
    fn as_ptr<T: ?Sized>(&self, metadata: <T as Pointee>::Metadata) -> *const T {
        from_raw_parts(self as *const _ as *const (), metadata)
    }

    #[inline]
    fn as_mut_ptr<T: ?Sized>(&mut self, metadata: <T as Pointee>::Metadata) -> *mut T {
        from_raw_parts_mut(self as *mut _ as *mut (), metadata)
    }

    #[inline]
    unsafe fn drop<T: ?Sized>(mut self, metadata: <T as Pointee>::Metadata) {
        drop_in_place(self.as_mut_ptr::<T>(metadata));
    }
}

struct Heap(NonNull<u8>);

impl Heap {
    #[inline]
    fn try_new_uninit_in<T: ?Sized, A: Allocator>(
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, AllocError> {
        Ok(Self(
            alloc.allocate(layout_from_metadata::<T>(metadata))?.cast(),
        ))
    }

    #[inline]
    fn try_new_zeroed_in<T: ?Sized, A: Allocator>(
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, AllocError> {
        Ok(Self(
            alloc
                .allocate_zeroed(layout_from_metadata::<T>(metadata))?
                .cast(),
        ))
    }

    #[inline]
    unsafe fn try_from_stack_in<T: ?Sized, S, A: Allocator>(
        stack: Stack<S>,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, Stack<S>> {
        match Self::try_new_uninit_in::<T, _>(metadata, alloc) {
            Ok(mut heap) => {
                let layout = layout_from_metadata::<T>(metadata);

                copy_nonoverlapping(
                    stack.as_ptr::<u8>(()),
                    heap.as_mut_ptr::<u8>(()),
                    layout.size(),
                );

                Ok(heap)
            }

            Err(_) => Err(stack),
        }
    }

    #[inline]
    unsafe fn from_raw(ptr: *mut u8) -> Self {
        Self(NonNull::new_unchecked(ptr))
    }

    #[inline]
    fn as_ptr<T: ?Sized>(&self, metadata: <T as Pointee>::Metadata) -> *const T {
        from_raw_parts(self.0.as_ptr() as *const (), metadata)
    }

    #[inline]
    fn as_mut_ptr<T: ?Sized>(&mut self, metadata: <T as Pointee>::Metadata) -> *mut T {
        from_raw_parts_mut(self.0.as_ptr() as *mut (), metadata)
    }

    #[inline]
    unsafe fn deallocate<T: ?Sized, A: Allocator>(
        self,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) {
        alloc.deallocate(self.0, layout_from_metadata::<T>(metadata));
    }

    #[inline]
    unsafe fn drop<T: ?Sized, A: Allocator>(
        mut self,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) {
        drop_in_place(self.as_mut_ptr::<T>(metadata));
        self.deallocate::<T, _>(metadata, alloc)
    }
}

union Data<S> {
    stack: ManuallyDrop<Stack<S>>,
    heap: ManuallyDrop<Heap>,
}

impl<S> Data<S> {
    #[inline]
    unsafe fn try_from_data_in<T: ?Sized, Z, A: Allocator>(
        data: Data<Z>,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, Data<Z>> {
        if Data::<Z>::inlined::<T>(metadata) {
            let stack = ManuallyDrop::into_inner(data.stack);

            if Self::inlined::<T>(metadata) {
                Ok(Self {
                    stack: ManuallyDrop::new(Stack::from_stack::<T, _>(stack, metadata)),
                })
            } else {
                match Heap::try_from_stack_in::<T, _, _>(stack, metadata, alloc) {
                    Ok(heap) => Ok(Self {
                        heap: ManuallyDrop::new(heap),
                    }),
                    Err(stack) => Err(Data {
                        stack: ManuallyDrop::new(stack),
                    }),
                }
            }
        } else {
            Ok(Self::from_heap::<T, _>(
                ManuallyDrop::into_inner(data.heap),
                metadata,
                alloc,
            ))
        }
    }

    // #[inline]
    // unsafe fn try_from_stack_in<T: ?Sized, A: Allocator>(stack: Stack<S>, metadata: <T as Pointee>::Metadata, alloc: &A) -> Result<Self, Stack<S>> {
    //
    // }

    #[inline]
    unsafe fn from_heap<T: ?Sized, A: Allocator>(
        heap: Heap,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Self {
        if Self::inlined::<T>(metadata) {
            Self {
                stack: ManuallyDrop::new(Stack::from_heap::<T, _>(heap, metadata, alloc)),
            }
        } else {
            Self {
                heap: ManuallyDrop::new(heap),
            }
        }
    }

    #[inline]
    unsafe fn try_into_heap_in<T: ?Sized, A: Allocator>(
        mut self,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Heap, Self> {
        if Self::inlined::<T>(metadata) {
            match Heap::try_from_stack_in::<T, _, _>(
                ManuallyDrop::take(&mut self.stack),
                metadata,
                alloc,
            ) {
                Ok(heap) => Ok(heap),
                Err(stack) => Err(Self {
                    stack: ManuallyDrop::new(stack),
                }),
            }
        } else {
            Ok(ManuallyDrop::take(&mut self.heap))
        }
    }

    #[inline]
    fn try_new_uninit_in<T: ?Sized, A: Allocator>(
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) -> Result<Self, AllocError> {
        if Self::inlined::<T>(metadata) {
            Ok(Self {
                stack: ManuallyDrop::new(Stack::new_uninit()),
            })
        } else {
            Ok(Self {
                heap: ManuallyDrop::new(Heap::try_new_uninit_in::<T, _>(metadata, alloc)?),
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
                stack: ManuallyDrop::new(Stack::new_zeroed()),
            })
        } else {
            Ok(Self {
                heap: ManuallyDrop::new(Heap::try_new_zeroed_in::<T, _>(metadata, alloc)?),
            })
        }
    }

    #[inline]
    fn as_ptr<T: ?Sized>(&self, metadata: <T as Pointee>::Metadata) -> *const T {
        unsafe {
            if Self::inlined::<T>(metadata) {
                self.stack.as_ptr(metadata)
            } else {
                self.heap.as_ptr(metadata)
            }
        }
    }

    #[inline]
    fn as_mut_ptr<T: ?Sized>(&mut self, metadata: <T as Pointee>::Metadata) -> *mut T {
        unsafe {
            if Self::inlined::<T>(metadata) {
                self.stack.as_mut_ptr(metadata)
            } else {
                self.heap.as_mut_ptr(metadata)
            }
        }
    }

    #[inline]
    unsafe fn drop<T: ?Sized, A: Allocator>(
        &mut self,
        metadata: <T as Pointee>::Metadata,
        alloc: &A,
    ) {
        unsafe {
            if Self::inlined::<T>(metadata) {
                ManuallyDrop::take(&mut self.stack).drop::<T>(metadata)
            } else {
                ManuallyDrop::take(&mut self.heap).drop::<T, _>(metadata, alloc)
            }
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
    pub fn allocator(&self) -> &A {
        &self.alloc
    }

    #[inline]
    pub fn metadata(&self) -> <T as Pointee>::Metadata {
        self.metadata
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
    #[cfg(feature = "alloc")]
    pub fn from_box(boxed: alloc::boxed::Box<T, A>) -> Self {
        let (src, alloc) = alloc::boxed::Box::into_raw_with_allocator(boxed);
        let (src, metadata) = src.to_raw_parts();
        let heap = unsafe { Heap::from_raw(src as *mut u8) };

        Self {
            phantom: PhantomData,
            data: unsafe { Data::from_heap::<T, _>(heap, metadata, &alloc) },
            metadata,
            alloc,
        }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_into_box(self) -> Result<alloc::boxed::Box<T, A>, Self> {
        let (data, metadata, alloc) = self.into_parts();

        unsafe {
            match data.try_into_heap_in::<T, _>(metadata, &alloc) {
                Ok(mut heap) => Ok(alloc::boxed::Box::from_raw_in(
                    heap.as_mut_ptr(metadata),
                    alloc,
                )),
                Err(data) => Err(Self {
                    phantom: PhantomData,
                    metadata,
                    data,
                    alloc,
                }),
            }
        }
    }

    #[inline]
    pub fn try_resize_stack<Z>(self) -> Result<Inner<T, Z, A>, Self> {
        let (data, metadata, alloc) = self.into_parts();

        unsafe {
            match Data::<Z>::try_from_data_in::<T, _, _>(data, metadata, &alloc) {
                Ok(data) => Ok(Inner {
                    phantom: PhantomData,
                    metadata,
                    data,
                    alloc,
                }),

                Err(data) => Err(Self {
                    phantom: PhantomData,
                    metadata,
                    data,
                    alloc,
                }),
            }
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
        unsafe { self.data.drop::<T, _>(self.metadata, &self.alloc) }
    }
}

impl<T: ?Sized, S, A: Allocator> Deref for Inner<T, S, A> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data.as_ptr::<T>(self.metadata) }
    }
}

impl<T: ?Sized, S, A: Allocator> DerefMut for Inner<T, S, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data.as_mut_ptr::<T>(self.metadata) }
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

#[cold]
#[inline(never)]
#[cfg(feature = "alloc")]
#[cfg(not(no_global_oom_handling))]
pub fn handle_alloc_error<T: ?Sized>(metadata: <T as Pointee>::Metadata) -> ! {
    alloc::alloc::handle_alloc_error(layout_from_metadata::<T>(metadata))
}
