#![feature(test, allocator_api)]

extern crate smolbox;
extern crate test;

use smolbox::SmallBox;
use std::alloc::Global;
use test::{black_box, Bencher};

#[bench]
fn smallbox_small_item_small_space(b: &mut Bencher) {
    b.iter(|| {
        let small: SmallBox<_, [usize; 1], Global> =
            black_box(SmallBox::try_new(black_box(true)).unwrap());
        small
    })
}

#[bench]
fn smallbox_small_item_large_space(b: &mut Bencher) {
    b.iter(|| {
        let small: SmallBox<_, [usize; 64], Global> =
            black_box(SmallBox::try_new(black_box(true)).unwrap());
        small
    })
}

#[bench]
fn smallbox_large_item_small_space(b: &mut Bencher) {
    b.iter(|| {
        let large: SmallBox<_, [usize; 1], Global> =
            black_box(SmallBox::try_new(black_box([0usize; 64])).unwrap());
        large
    })
}

#[bench]
fn smallbox_large_item_large_space(b: &mut Bencher) {
    b.iter(|| {
        let large: SmallBox<_, [usize; 64], Global> =
            black_box(SmallBox::try_new(black_box([0usize; 64])).unwrap());
        large
    })
}

#[bench]
fn box_small_item(b: &mut Bencher) {
    b.iter(|| {
        let large: Box<_> = black_box(Box::new(black_box(true)));
        large
    })
}

#[bench]
fn box_large_item(b: &mut Bencher) {
    b.iter(|| {
        let large: Box<_> = black_box(Box::new(black_box([0usize; 64])));
        large
    })
}
