//! Example demonstrating the behaviour of an indexbag.

#![feature(test)]

extern crate test;

extern crate rand;
extern crate index_bag;

use test::Bencher;
use rand::*;
use rand::prng::XorShiftRng;

use index_bag::{IndexBag, Index};

fn main() {
    let mut rng = create_rng();
    let mut bag = IndexBag::new();
    let mut values = Vec::new();

    for action in &[Insert, Insert, Insert, Insert, Insert, Insert, Insert, Remove, Insert] {
        action.enact(&mut rng, &mut bag, &mut values);
    }

    while let Some(action) = rng.choose(ALL_ACTIONS) {
        rng.shuffle(values.as_mut_slice());
        action.enact(&mut rng, &mut bag, &mut values);
    }
}

#[bench]
fn random_ops_on_0(b: &mut Bencher) {
    bench_random_ops(b, 0, 100)
}
#[bench]
fn random_ops_on_10(b: &mut Bencher) {
    bench_random_ops(b, 10, 100)
}
#[bench]
fn random_ops_on_100(b: &mut Bencher) {
    bench_random_ops(b, 100, 100)
}
#[bench]
fn random_ops_on_1000(b: &mut Bencher) {
    bench_random_ops(b, 1000, 100)
}
#[bench]
fn random_ops_on_10000(b: &mut Bencher) {
    bench_random_ops(b, 10000, 100)
}

#[bench]
fn insert_1(b: &mut Bencher) {
    bench_ops(b, 10, 1, Insert)
}
#[bench]
fn insert_2(b: &mut Bencher) {
    bench_ops(b, 10, 2, Insert)
}
#[bench]
fn insert_3(b: &mut Bencher) {
    bench_ops(b, 10, 3, Insert)
}
#[bench]
fn insert_4(b: &mut Bencher) {
    bench_ops(b, 10, 4, Insert)
}
#[bench]
fn insert_5(b: &mut Bencher) {
    bench_ops(b, 10, 5, Insert)
}
#[bench]
fn insert_10(b: &mut Bencher) {
    bench_ops(b, 10, 10, Insert)
}
#[bench]
fn insert_on_10(b: &mut Bencher) {
    bench_ops(b, 10, 100, Insert)
}
#[bench]
fn insert_on_100(b: &mut Bencher) {
    bench_ops(b, 100, 100, Insert)
}
#[bench]
fn insert_on_1000(b: &mut Bencher) {
    bench_ops(b, 1000, 100, Insert)
}
#[bench]
fn insert_on_10000(b: &mut Bencher) {
    bench_ops(b, 10000, 100, Insert)
}
#[bench]
#[ignore]
fn insert_on_100000000(b: &mut Bencher) {
    bench_ops(b, 1000000, 100, Insert)
}
#[bench]
fn vec_insert(b: &mut Bencher) {
    let mut values = vec![0; 10000];
    b.iter(move || {
        for i in 0..100 {
            values.push(i);
        }
    })
}

#[bench]
fn remove_on_1000(b: &mut Bencher) {
    bench_ops(b, 1000, 100, Remove)
}
#[bench]
fn remove_on_10000(b: &mut Bencher) {
    bench_ops(b, 10000, 100, Remove)
}
#[bench]
fn remove_on_100000(b: &mut Bencher) {
    bench_ops(b, 100000, 100, Remove)
}

#[bench]
fn lookup_on_10(b: &mut Bencher) {
    bench_ops(b, 10, 100, Lookup)
}
#[bench]
fn lookup_on_100(b: &mut Bencher) {
    bench_ops(b, 100, 100, Lookup)
}
#[bench]
fn lookup_on_1000(b: &mut Bencher) {
    bench_ops(b, 1000, 100, Lookup)
}
#[bench]
fn lookup_on_10000(b: &mut Bencher) {
    bench_ops(b, 10000, 100, Lookup)
}
#[bench]
fn vec_lookup(b: &mut Bencher) {
    let values = vec![1; 10000];
    b.iter(move || {
        let mut sum = 0;
        for i in 0..100 {
            sum += values[i]
        }
        sum
    })
}

#[cfg(test)]
fn bench_random_ops(b: &mut Bencher, base: usize, ops: usize) {
    let mut rng = create_rng();
    let mut bag = IndexBag::new();
    let mut values = Vec::with_capacity(base + ops);

    for _ in 0..base {
        // let action = action_rng.choose(&[Insert, Insert, Remove, Lookup]).unwrap();
        Insert.enact(&mut rng, &mut bag, &mut values);
    }

    b.iter(move || {
        for _ in 0..ops {
            let action = rng.choose(ALL_ACTIONS).unwrap();
            action.enact(&mut rng, &mut bag, &mut values);
        }
    })
}

#[cfg(test)]
fn bench_ops(b: &mut Bencher, base: usize, ops: usize, op: Action) {
    let mut bag = IndexBag::new();
    let mut values = Vec::with_capacity(base + ops);
    let mut rng = create_rng();

    for _ in 0..(base * 2) {
        Insert.enact(&mut rng, &mut bag, &mut values);
    }
    rng.shuffle(values.as_mut_slice());

    for _ in 0..(base) {
        let (value, index) = values.pop().unwrap();
        assert_eq!(value, bag.remove(index).unwrap());
    }
    rng.shuffle(values.as_mut_slice());

    b.iter(move || {
        let mut bag = IndexBag::new();
        let mut values = Vec::new();

        for _ in 0..ops {
            op.enact(&mut rng, &mut bag, &mut values);
            if let Lookup = op { values.pop(); }
        }
    })
}

#[derive(Clone, Copy)]
enum Action {
    Insert,
    Remove,
    Lookup,
}
use Action::*;
const ALL_ACTIONS: &[Action] = &[Insert, Remove, Lookup];

impl Action {
    fn enact(
        &self,
        rng: &mut impl Rng,
        bag: &mut IndexBag<u16>,
        values: &mut Vec<(u16, Index)>,
    ) {
        match self {
            Insert => {
                let value = rng.gen();
                let index = bag.insert(value);
                #[cfg(not(test))]
                eprintln!(
                    "\x1B[1;32mINSERT\x1B[0m ({:4}/{:4}) {:04X}         @ {:?}",
                    bag.unused_indexes(), bag.pool_size(), value, index
                );
                values.push((value, index));
            }
            Remove => {
                if values.len() > 0 {
                    let (value, index) = values.pop().unwrap();
                    let removed_value = bag.remove(index)
                        .expect(&format!("Couldn't remove index {:?}", index));
                    #[cfg(not(test))]
                    eprintln!(
                        "\x1B[1;31mREMOVE\x1b[0m ({:4}/{:4}) {:04X} == {:04X} @ {:?}",
                        bag.unused_indexes(), bag.pool_size(), removed_value, value, index
                    );
                    assert_eq!(value, removed_value);
                }
            }
            Lookup => {
                if values.len() > 0 {
                    let (value, index) = values[values.len() - 1];
                    let removed_value = bag.get(index)
                        .expect(&format!("Couldn't lookup index {:?}", index));
                    #[cfg(not(test))]
                    eprintln!(
                        "\x1B[1;33mLOOKUP\x1B[0m ({:4}/{:4}) {:04X} == {:04X} @ {:?}",
                        bag.unused_indexes(), bag.pool_size(), removed_value, value, index
                    );
                    assert_eq!(value, *removed_value);
                }
            }
        }
    }
}

/// Create a RNG with a set seed.
fn create_rng() -> impl Rng {
    let seed = [
        0x4a, 0x94, 0xef, 0x6a,
        0x5d, 0x23, 0x38, 0x90,
        0xec, 0x58, 0xdb, 0x09,
        0xe6, 0x6d, 0xea, 0xd3,
    ];

    XorShiftRng::from_seed(seed)
}
