#![feature(allocator_api)]

extern crate smolbox;

use std::{alloc::Global, any::Any, mem::size_of};

use assert_no_alloc::*;
use smolbox::SmallBox;

#[cfg(not(miri))] // this fucks up miri for some reason, tests pass ok otherwise? seems like a miri issue
#[global_allocator]
static A: AllocDisabler = AllocDisabler;

#[test]
pub fn test_inlined_small() {
    assert_no_alloc(|| {
        let mut boxed = SmallBox::<_, [usize; 1], Global>::try_new(1usize).unwrap();

        assert!(SmallBox::is_inlined(&boxed));
        assert_eq!(*boxed, 1);

        *boxed = 2;

        assert!(SmallBox::is_inlined(&boxed));
        assert_eq!(*boxed, 2);
    });
}

#[test]
pub fn test_inlined_large() {
    assert_no_alloc(|| {
        let mut boxed = SmallBox::<_, [usize; 64], Global>::try_new([0usize; 64]).unwrap();

        assert!(SmallBox::is_inlined(&boxed));
        assert_eq!(*boxed, [0usize; 64]);

        boxed.fill(1usize);

        assert!(SmallBox::is_inlined(&boxed));
        assert_eq!(*boxed, [1usize; 64]);
    });
}

#[test]
pub fn test_heap_small() {
    let mut boxed = SmallBox::<_, [usize; 0], Global>::try_new(1usize).unwrap();

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 1);

    *boxed = 2;

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 2);
}

#[test]
pub fn test_heap_large() {
    let mut boxed = SmallBox::<_, [usize; 16], Global>::try_new([0usize; 64]).unwrap();

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, [0usize; 64]);

    boxed.fill(1usize);

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, [1usize; 64]);
}

#[test]
pub fn test_inlined_any() {
    let mut boxed: SmallBox<dyn Any, [usize; 1], Global> =
        SmallBox::coerce(SmallBox::try_new(1usize).unwrap());

    assert!(SmallBox::is_inlined(&boxed));
    assert_eq!(boxed.downcast_ref(), Some(&1usize));

    *boxed.downcast_mut().unwrap() = 2usize;

    let mut boxed = boxed.downcast::<usize>().unwrap();

    assert!(SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 2usize);
    *boxed = 3usize;

    let boxed: SmallBox<dyn Any, [usize; 1], Global> = SmallBox::coerce(boxed);

    assert!(SmallBox::is_inlined(&boxed));
    assert_eq!(boxed.downcast_ref(), Some(&3usize));

    assert!(boxed.downcast::<u8>().is_err());
}

#[test]
pub fn test_heap_any() {
    let mut boxed: SmallBox<dyn Any, [usize; 0], Global> =
        SmallBox::coerce(SmallBox::try_new(1usize).unwrap());

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(boxed.downcast_ref(), Some(&1usize));

    *boxed.downcast_mut().unwrap() = 2usize;

    let mut boxed = boxed.downcast::<usize>().unwrap();

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 2usize);
    *boxed = 3usize;

    let boxed: SmallBox<dyn Any, [usize; 0], Global> = SmallBox::coerce(boxed);

    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(boxed.downcast_ref(), Some(&3usize));

    assert!(boxed.downcast::<u8>().is_err());
}

#[test]
pub fn test_drop() {
    use core::cell::Cell;

    struct Struct<'a>(&'a Cell<bool>, u8);
    impl<'a> Drop for Struct<'a> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    let flag = Cell::new(false);
    let stacked: SmallBox<_, [usize; 2], Global> = SmallBox::try_new(Struct(&flag, 0)).unwrap();
    assert!(SmallBox::is_inlined(&stacked));
    assert!(!flag.get());
    drop(stacked);
    assert!(flag.get());

    let flag = Cell::new(false);
    let heaped: SmallBox<_, [usize; 0], Global> = SmallBox::try_new(Struct(&flag, 0)).unwrap();
    assert!(!SmallBox::is_inlined(&heaped));
    assert!(!flag.get());
    drop(heaped);
    assert!(flag.get());
}

#[test]
fn test_zst() {
    #[derive(Debug, Eq, PartialEq)]
    struct ZST;

    let zst: SmallBox<ZST, [usize; 0], Global> = SmallBox::try_new(ZST).unwrap();
    assert_eq!(*zst, ZST);
    assert!(SmallBox::is_inlined(&zst))
}

#[test]
fn test_sizes() {
    let ptr = size_of::<usize>();

    assert!(size_of::<SmallBox<u8, [usize; 0], Global>>() == 1 * ptr);
    assert!(size_of::<SmallBox<u8, [usize; 1], Global>>() == 1 * ptr);
    assert!(size_of::<SmallBox<u8, [usize; 2], Global>>() == 2 * ptr);
    assert!(size_of::<SmallBox<u8, [usize; 3], Global>>() == 3 * ptr);

    assert!(size_of::<SmallBox<[u8], [usize; 0], Global>>() == 2 * ptr);
    assert!(size_of::<SmallBox<[u8], [usize; 1], Global>>() == 2 * ptr);
    assert!(size_of::<SmallBox<[u8], [usize; 2], Global>>() == 3 * ptr);
    assert!(size_of::<SmallBox<[u8], [usize; 3], Global>>() == 4 * ptr);
}

#[test]
#[cfg(feature = "alloc")]
fn test_heap_box_conversions() {
    let boxed = SmallBox::<_, [usize; 0], Global>::try_new(1usize).unwrap();
    assert!(!SmallBox::is_inlined(&boxed));

    let mut boxed = SmallBox::try_into_box(boxed).unwrap();
    assert_eq!(*boxed, 1);
    *boxed = 2;

    let boxed: SmallBox<_, [usize; 0], Global> = SmallBox::from_box(boxed);
    assert!(!SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 2);
}

#[test]
#[cfg(feature = "alloc")]
fn test_inlined_box_conversions() {
    let boxed = SmallBox::<_, [usize; 1], Global>::try_new(1usize).unwrap();
    assert!(SmallBox::is_inlined(&boxed));

    let mut boxed = SmallBox::try_into_box(boxed).unwrap();
    assert_eq!(*boxed, 1);
    *boxed = 2;

    let boxed: SmallBox<_, [usize; 1], Global> = SmallBox::from_box(boxed);
    assert!(SmallBox::is_inlined(&boxed));
    assert_eq!(*boxed, 2);
}
