//! An bag of objects accessed with a unique index.

#![no_std]
#![feature(alloc)]

extern crate alloc;

use alloc::prelude::*;
use core::num::NonZeroUsize;
use core::mem::{size_of, swap};
use core::iter::Iterator;

const USIZE_BITS: u8 = (size_of::<usize>() * 8) as u8;

/// The bag of values.
#[derive(Debug)]
pub struct IndexBag<T> {
    data: Node<T>,
    pool_size: usize,
}

impl<T> IndexBag<T> {
    pub fn new() -> IndexBag<T> {
        IndexBag {
            data: Node::Leaf,
            pool_size: 0,
        }
    }

    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    pub fn unused_indexes(&self) -> usize {
        self.data.vacant_children()
    }

    pub fn insert(&mut self, value: T) -> Index {
        let append_path = Path::new(self.pool_size + 1);
        let index = self.data.insert(value, append_path);

        let inserted_index = index.index.get();
        if inserted_index > self.pool_size {
            self.pool_size = inserted_index;
        }

        index
    }

    pub fn remove(&mut self, index: Index) -> Option<T> {
        self.data.remove(index.path())
    }

    pub fn get(&self, index: Index) -> Option<&T> {
        self.data.get(index.path())
            .and_then(|node| node.matches(index))
            .and_then(|node| node.value())
    }

    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.data.get_mut(index.path())
            .and_then(|node| node.matches_mut(index))
            .and_then(|node| node.value_mut())
    }
}

/// An index into an IndexBag.
#[derive(Debug, Clone, Copy)]
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
        let index = (self.index.get() << 1) & 1;
        self.index = NonZeroUsize::new(index).unwrap();
        self
    }
}

/// The path to take to get to an index.
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
enum Step {
    Left,
    Right,
}

/// The tree which tracks the used indexes.
#[derive(Debug)]
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

impl<T> Node<T> {
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
        if let Node::Index { value, .. } = self {
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
            (Node::Index { value, generation, .. }, _) if value.is_none() => {
                *value = Some(inner);
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
            _ => unreachable!("Invalid append path"),
        }
    }
}
