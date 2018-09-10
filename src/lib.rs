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
use core::mem::swap;

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
    data: Vec<(Option<T>, usize)>,
    free_indexes: Vec<usize>,
}

impl<T: ::core::fmt::Debug> IndexBag<T> {
    /// Create an empty bag.
    pub fn new() -> IndexBag<T> {
        IndexBag {
            data: Vec::new(),
            free_indexes: Vec::new(),
        }
    }

    /// The current size of the bag.
    ///
    /// The bag expands only when it has no available unused indexes.
    pub fn pool_size(&self) -> usize {
        self.data.len()
    }

    /// The number of allocated but unused indexes in the bag.
    pub fn unused_indexes(&self) -> usize {
        self.free_indexes.len()
    }

    /// Insert an item into the bag.
    pub fn insert(&mut self, value: T) -> Index {
        if let Some(index) = self.free_indexes.pop() {
            self.data[index].0 = Some(value);
            self.data[index].1 += 1;
            Index::new(index, self.data[index].1)
        } else {
            let index = Index::new(self.data.len(), 0);
            self.data.push((Some(value), 0));
            index
        }
    }

    /// Remove an item from the bag.
    pub fn remove(&mut self, index: Index) -> Option<T> {
        if let Some((ref mut value @ Some(_), generation)) = self.data.get_mut(index.index) {
            if *generation == index.generation {
                let mut inner = None;
                swap(&mut inner, value);
                self.free_indexes.push(index.index);
                inner
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get a reference to an item in the bag.
    pub fn get(&self, index: Index) -> Option<&T> {
        self.data.get(index.index)
            .and_then(|(value, generation)| {
                if *generation == index.generation {
                    value.as_ref()
                } else {
                    None
                }
            })
    }

    /// Get a mutable reference to an item in the bag.
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.data.get_mut(index.index)
            .and_then(|(value, generation)| {
                if *generation == index.generation {
                    value.as_mut()
                } else {
                    None
                }
            })
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
        self.data.get(index)
            .map(|(_, generation)| Index::new(index, *generation))
    }
}

/// An index into an IndexBag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index {
    index: usize,
    generation: usize,
}

impl Index {
    fn new(index: usize, generation: usize) -> Index {
        Index {
            index,
            generation,
        }
    }
}

impl Into<usize> for Index {
    fn into(self) -> usize {
        self.index
    }
}
