//! An bag of objects accessed with a unique index.
//!
//! [`IndexBag`] provides a mechanism for storing items and retrieving them again later. The
//!
//! The [`Index`] can be copied safetly, providing multiple references to the same object without
//! requiring ownership. The [`Index`] is also generational, meaning that if an [`Index`] is held
//! to an object in the bag, but that object is removed and another added in its place, the
//! [`Index`] referring to the original item will fail to resolve to the replacement item (rather
//! than erroneously referring to the new item placed at the same location in the [`IndexBag`]).
//!
//! ```rust
//! use index_bag::IndexBag;
//!
//! let mut bag = IndexBag::new();
//! let index = bag.insert(12);
//! assert_eq!(bag.get(index).unwrap(), &12);
//! assert_eq!(bag.remove(index), Some(12));
//! assert_eq!(bag.remove(index), None);
//! ```
#![no_std]
#![feature(alloc)]
#![feature(test)]

extern crate alloc;
#[cfg(test)]
extern crate test;

use alloc::prelude::*;
use core::num::NonZeroUsize;
use core::mem::{size_of, swap};
use core::iter::Iterator;

const USIZE_BITS: u8 = (size_of::<usize>() * 8) as u8;

/// The bag of values.
///
/// ```rust
/// use index_bag::IndexBag;
///
/// let mut bag = IndexBag::new();
/// let index = bag.insert(12);
/// assert_eq!(bag.get(index).unwrap(), &12);
/// assert_eq!(bag.remove(index), Some(12));
/// assert_eq!(bag.remove(index), None);
/// ```
#[derive(Debug, Clone)]
pub struct IndexBag<T> {
    data: Node<T>,
    pool_size: usize,
}

impl<T: ::core::fmt::Debug> IndexBag<T> {
    /// Create an empty bag.
    pub fn new() -> IndexBag<T> {
        IndexBag {
            data: Node::Leaf,
            pool_size: 0,
        }
    }

    /// The current size of the bag.
    ///
    /// The bag expands only when it has no available unused indexes.
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// The number of allocated but unused indexes in the bag.
    pub fn unused_indexes(&self) -> usize {
        self.data.vacant_children()
    }

    /// Insert an item into the bag.
    pub fn insert(&mut self, value: T) -> Index {
        let append_path = Path::new(self.pool_size + 1);
        let index = self.data.insert(value, append_path);

        let inserted_index = index.index.get();
        if inserted_index > self.pool_size {
            debug_assert!(inserted_index == self.pool_size + 1);
            self.pool_size = inserted_index;
        }

        index
    }

    /// Remove an item from the bag.
    pub fn remove(&mut self, index: Index) -> Option<T> {
        self.data.remove(index.path())
    }

    /// Get a reference to an item in the bag.
    pub fn get(&self, index: Index) -> Option<&T> {
        self.data.get(index.path())
            .and_then(|node| node.matches(index))
            .and_then(|node| node.value())
    }

    /// Get a mutable reference to an item in the bag.
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.data.get_mut(index.path())
            .and_then(|node| node.matches_mut(index))
            .and_then(|node| node.value_mut())
    }

    /// Translate a [`usize`] index to an [`Index`].
    ///
    /// The generated [`Index`] will refer to the item in the bag that currently resides at a given
    /// numeric index.
    ///
    /// ```rust
    /// use index_bag::IndexBag;
    /// let mut bag = IndexBag::new();
    ///
    /// let index = bag.insert(12);
    /// let i_index: usize = index.into();
    /// let current_index = bag.get_index(i_index).unwrap();
    /// assert_eq!(index, current_index);
    /// assert_eq!(bag.remove(current_index), Some(12));
    ///
    /// // Note that usize indexes do not prevent access to incorrect data.
    /// let new_index = bag.insert(13);
    /// let current_index = bag.get_index(i_index).unwrap();
    /// assert_eq!(new_index, current_index);
    /// assert_eq!(bag.remove(current_index), Some(13));
    /// ```
    pub fn get_index(&self, index: usize) -> Option<Index> {
        let non_zero_index = NonZeroUsize::new(index)?;
        self.data.get(Path::new(index))
            .and_then(Node::generation)
            .map(|generation| Index { index: non_zero_index, generation })
    }
}

/// An index into an IndexBag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index {
    index: NonZeroUsize,
    generation: usize,
}

impl Index {
    fn new(generation: usize) -> Index {
        Index {
            index: NonZeroUsize::new(1).unwrap(),
            generation,
        }
    }

    fn path(&self) -> Path {
        Path::new(self.index.get())
    }

    fn push_left(mut self) -> Index {
        let index = self.index.get() << 1;
        self.index = NonZeroUsize::new(index).unwrap();
        self
    }

    fn push_right(mut self) -> Index {
        let index = (self.index.get() << 1) | 1;
        self.index = NonZeroUsize::new(index).unwrap();
        self
    }
}

impl Into<usize> for Index {
    fn into(self) -> usize {
        self.index.get()
    }
}

/// The path to take to get to an index.
#[derive(Debug)]
struct Path {
    depth: u8,
    path: usize,
}

impl Path {
    fn new(index: usize) -> Path {
        for depth in (0u8..USIZE_BITS).rev() {
            if index & (1 << depth) != 0 {
                let path = index & ((1 << depth) - 1);
                return Path { depth, path };
            }
        }

        unreachable!("Got zero index")
    }
}

impl Iterator for Path {
    type Item = Step;

    fn next(&mut self) -> Option<Step> {
        if self.depth == 0 {
            None
        } else {
            let step = if self.path & 1 == 0 {
                Some(Step::Left)
            } else {
                Some(Step::Right)
            };

            self.depth -= 1;
            self.path >>= 1;

            step
        }
    }
}

/// The steps to take on the path.
#[derive(Debug)]
enum Step {
    Left,
    Right,
}

/// The tree which tracks the used indexes.
#[derive(Debug, Clone)]
enum Node<T> {
    Index {
        value: Option<T>,
        vacant_children: usize,
        generation: usize,
        left: Box<Node<T>>,
        right: Box<Node<T>>,
    },
    Leaf,
}

#[bench]
fn node_create(b: &mut test::Bencher) {
    b.iter(|| Node::new(10u16));
}

#[bench]
fn node_insert(b: &mut test::Bencher) {
    b.iter(|| {
        let mut node = Node::new(10u16);
        node.remove(Path::new(1));
        node.insert(11u16, Path::new(1))
    });
}

#[bench]
fn node_insert_3(b: &mut test::Bencher) {
    b.iter(|| {
        let mut node = Node::new(10u16);
        node.remove(Path::new(1));
        node.insert(11u16, Path::new(1));
        node.insert(12u16, Path::new(2));
        node.insert(13u16, Path::new(3));
    });
}

#[bench]
fn node_insert_100(b: &mut test::Bencher) {
    b.iter(|| {
        let mut node = Node::Leaf;
        (0..100)
            .map(move |i| node.insert(i, Path::new(i + 1)))
            .count()
    })
}

#[bench]
fn node_create_100(b: &mut test::Bencher) {
    b.iter(|| {
        (0..100)
            .map(move |i| Node::new(i))
            .count()
    })
}
#[bench]
fn struct_100(b: &mut test::Bencher) {
    b.iter(|| {
        (0..100)
            .map(move |i| Node::Index {
                value: Some(i),
                generation: 0,
                vacant_children: 0,
                left: Box::new(Node::Leaf),
                right: Box::new(Node::Leaf),
            })
            .count()
    })
}

#[bench]
fn node_recreate_100(b: &mut test::Bencher) {
    let mut node = Node::Leaf;
    for i in 0..100 {
        node.insert(i + 1, Path::new(i + 1));
    }
    for i in 0..100 {
        assert_eq!(node.remove(Path::new(i + 1)).unwrap(), i + 1);
    }
    b.iter(|| {
        let mut node = node.clone();
        for i in 0..100 {
            node.insert(i + 1, Path::new(i + 1));
        }
    })
}

#[bench]
fn node_swap_100(b: &mut test::Bencher) {
    let mut node = Node::Leaf;
    for i in 0..100 {
        node.insert(i + 1, Path::new(i + 1));
    }
    for i in 0..100 {
        let mut node = node.get_mut(Path::new(i + 1)).unwrap();
        assert_eq!(node.value().cloned().unwrap(), i + 1);
        if let Node::Index { value, .. } = node {
            *value = None;
        }
    }
    b.iter(|| {
        for i in 0..100 {
            let mut node = node.get_mut(Path::new(i + 1)).unwrap();
            if let Node::Index { value, .. } = node {
                *value = Some(i + 1);
            }
        }
    })
}

#[bench]
fn node_lookup_3(b: &mut test::Bencher) {
    let mut node = Node::new(10u16);
    node.insert(12u16, Path::new(2));
    node.insert(13u16, Path::new(3));
    b.iter(move || {
        assert_eq!(node.get(Path::new(1)).unwrap().value().unwrap(), &10);
        assert_eq!(node.get(Path::new(2)).unwrap().value().unwrap(), &12);
        assert_eq!(node.get(Path::new(3)).unwrap().value().unwrap(), &13);
    });
}

impl<T: ::core::fmt::Debug> Node<T> {
    fn new(value: T) -> Node<T> {
        use Node::*;
        Index {
            value: Some(value),
            vacant_children: 0,
            generation: 0,
            left: Box::new(Leaf),
            right: Box::new(Leaf),
        }
    }

    fn vacant_children(&self) -> usize {
        use Node::*;
        match self {
            Index { vacant_children, .. } => *vacant_children,
            Leaf => 0,
        }
    }

    fn generation(&self) -> Option<usize> {
        use Node::*;
        match self {
            Index { generation, .. } => Some(*generation),
            Leaf => None,
        }
    }

    fn get(&self, mut index: Path) -> Option<&Node<T>> {
        use Node::*;
        use Step::*;
        match (self, index.next()) {
            (Leaf,                Some(_)    ) => None,
            (node,                None       ) => Some(node),
            (Index { left,  .. }, Some(Left) ) => left.get(index),
            (Index { right, .. }, Some(Right)) => right.get(index),
        }
    }

    fn get_mut(&mut self, mut index: Path) -> Option<&mut Node<T>> {
        use Node::*;
        use Step::*;
        match (self, index.next()) {
            (Leaf,                Some(_)    ) => None,
            (node,                None       ) => Some(node),
            (Index { left,  .. }, Some(Left) ) => left.get_mut(index),
            (Index { right, .. }, Some(Right)) => right.get_mut(index),
        }
    }

    fn matches(&self, index: Index) -> Option<&Self> {
        use Node::*;
        match self {
            Index { generation, .. } if index.generation == *generation => Some(self),
            _ => None
        }
    }

    fn matches_mut(&mut self, index: Index) -> Option<&mut Self> {
        use Node::*;
        match self {
            Leaf => None,
            Index { generation, .. } if index.generation != *generation => None,
            node => Some(node)
        }
    }

    fn value(&self) -> Option<&T> {
        use Node::*;
        match self {
            Leaf => None,
            Index { value, .. } => value.as_ref()
        }
    }

    fn value_mut(&mut self) -> Option<&mut T> {
        use Node::*;
        match self {
            Leaf => None,
            Index { value, .. } => value.as_mut()
        }
    }

    fn remove(&mut self, mut path: Path) -> Option<T> {
        use Node::*;
        use Step::*;
        match (self, path.next()) {
            (node, None) => node.remove_value(),
            (Index { left, vacant_children, .. }, Some(Left)) => {
                left.remove(path).map(|removed| {
                    *vacant_children += 1;
                    removed
                })
            }
            (Index { right, vacant_children, .. }, Some(Right)) => {
                right.remove(path).map(|removed| {
                    *vacant_children += 1;
                    removed
                })
            }
            _ => None,
        }
    }

    fn remove_value(&mut self) -> Option<T> {
        let mut inner = None;
        if let Node::Index { value, vacant_children, .. } = self {
            *vacant_children += 1;
            swap(value, &mut inner);
        }
        inner
    }

    fn insert(&mut self, inner: T, mut path: Path) -> Index {
        use Step::*;
        match (self, path.next()) {
            (leaf @ Node::Leaf, None) => {
                *leaf = Node::new(inner);
                Index::new(0)
            },
            (Node::Index { value: value @ None, generation, vacant_children, .. }, _) => {
                *value = Some(inner);
                *vacant_children -= 1;
                *generation += 1;
                Index::new(*generation)
            }
            (Node::Index { left, vacant_children, .. }, _) if left.vacant_children() > 0 => {
                debug_assert!(*vacant_children >= left.vacant_children());

                *vacant_children -= 1;
                left.insert(inner, path).push_left()
            }
            (Node::Index { right, vacant_children, .. }, _) if right.vacant_children() > 0 => {
                debug_assert!(*vacant_children >= right.vacant_children());

                *vacant_children -= 1;
                right.insert(inner, path).push_right()
            }
            (Node::Index { left, .. }, Some(Left)) => {
                left.insert(inner, path).push_left()
            }
            (Node::Index { right, .. }, Some(Right)) => {
                right.insert(inner, path).push_right()
            }
            (node, step) => unreachable!("Invalid append path: {:?} on {:#?}", step, node),
        }
    }
}
