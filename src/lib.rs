#![cfg_attr(not(test), no_std)]

//!
//! # An ID Allocator for Sparse ID Spaces
//!
//! `ida` provides a thread-safe, `no_std` compatible ID allocator suitable for
//! systems-level programming, such as in OS kernels or embedded environments.
//!
//! It is implemented as a radix tree, which makes it highly memory-efficient
//! when dealing with sparse ID allocations (e.g., allocating ID 5 and ID 5,000,000
//! without allocating the space in between).
//!
//! ## Features
//! - **`no_std` compatible:** Usable in bare-metal environments.
//! - **Thread-Safe:** All public methods are thread-safe, using a spinlock for synchronization.
//! - **Memory-Efficient for Sparse Sets:** Ideal when allocated IDs are far apart.
//!
//! ## Example
//! ```
//! use ida::Ida;
//!
//! let ida = Ida::new();
//!
//! // Allocate new IDs
//! let id1 = ida.alloc().unwrap();
//! let id2 = ida.alloc().unwrap();
//!
//! assert_eq!(id1, 0);
//! assert_eq!(id2, 1);
//!
//! // Free an ID
//! ida.free(id1);
//!
//! // The next allocation reuses the freed ID
//! let id3 = ida.alloc().unwrap();
//! assert_eq!(id3, 0);
//! ```

extern crate alloc;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use core::fmt::Debug;
use spin::Mutex;

const IDA_SHIFT: usize = 6;
const IDA_BITMAP_BITS: usize = 1 << IDA_SHIFT;
// This calculation is the integer division equivalent of `ceil(64 / IDA_SHIFT)`
// and ensures that we have enough levels to cover the entire 64-bit ID space.
const IDA_MAX_LEVELS: usize = (64 + IDA_SHIFT - 1) / IDA_SHIFT;

#[derive(Debug)]
pub struct Ida {
    root: Mutex<IdaNode>,
}

#[derive(Debug)]
struct IdaNode {
    bitmap: u64,
    children: BTreeMap<usize, Box<IdaNode>>,
}

impl IdaNode {
    pub fn new() -> Self {
        Self {
            bitmap: 0,
            children: BTreeMap::new(),
        }
    }

    pub fn alloc(&mut self, level: usize) -> Option<usize> {
        // CASE: We are at a leaf node
        // The bitmap here represents individual IDs
        if level == 0 {
            // All ones means no free IDs
            if self.bitmap == u64::MAX {
                return None;
            }
            // Using trailing_ones to find the first zero bit,
            // which is an unallocated ID
            let bit = self.bitmap.trailing_ones() as usize;
            self.bitmap |= 1 << bit;
            return Some(bit);
        }

        // CASE: We are at an internal node
        // The bitmap here represents child nodes. We iterate through the unset bits
        // (0s), which correspond to children that are not full.
        while self.bitmap != u64::MAX {
            let i = self.bitmap.trailing_ones() as usize; // Find index of first 0 bit.

            // The child node is either unallocated or not fully allocated, get it.
            let child = self
                .children
                .entry(i)
                .or_insert_with(|| Box::new(IdaNode::new()));

            // Recursively allocate in the child node.
            if let Some(id_in_child) = child.alloc(level - 1) {
                // After the allocation, check if the child is now fully allocated.
                // If so, set the corresponding bit in this node's bitmap.
                if child.bitmap == u64::MAX {
                    self.bitmap |= 1 << i;
                }
                // Compute the full ID by combining the index and the child's ID.
                let id = (i << (level * IDA_SHIFT)) | id_in_child;
                return Some(id);
            } else {
                // The child was marked as having space in our bitmap, but the recursive
                // alloc returned None, implying it's actually full. We fix this
                // inconsistency here and continue the search in the next available child.
                self.bitmap |= 1 << i;
            }
        }

        None
    }

    pub fn free(&mut self, id: usize, level: usize) {
        // Determine which bit index to clear at this level
        let bit_index = (id >> (level * IDA_SHIFT)) & (IDA_BITMAP_BITS - 1);

        // CASE: We are at a leaf node
        if level == 0 {
            // Simply clear the bit corresponding to the ID
            self.bitmap &= !(1 << bit_index);
            return;
        }

        // CASE: We are at an internal node
        // Clear the bit in this node's bitmap,
        // mark the child as not full or non-existent
        self.bitmap &= !(1 << bit_index);
        // Recurse into the appropriate child node
        // if it exists, clearing the ID there
        if let Some(child) = self.children.get_mut(&bit_index) {
            // Recurse into the child node
            child.free(id, level - 1);
            // If the child is now empty, remove it to save space
            if child.bitmap == 0 && child.children.is_empty() {
                self.children.remove(&bit_index);
            }
        }
    }

    pub fn is_allocated(&self, id: usize, level: usize) -> bool {
        let bit_index = (id >> (level * IDA_SHIFT)) & (IDA_BITMAP_BITS - 1);

        if level == 0 {
            return (self.bitmap >> bit_index) & 1 == 1;
        }

        if let Some(child) = self.children.get(&bit_index) {
            child.is_allocated(id, level - 1)
        } else {
            // If the child node doesn't exist, no IDs in that range can be allocated.
            false
        }
    }
}

impl Ida {
    pub fn new() -> Self {
        Self {
            root: Mutex::new(IdaNode::new()),
        }
    }

    pub fn alloc(&self) -> Option<usize> {
        let mut root = self.root.lock();
        root.alloc(IDA_MAX_LEVELS - 1)
    }

    pub fn free(&self, id: usize) {
        let mut root = self.root.lock();
        root.free(id, IDA_MAX_LEVELS - 1);
    }

    /// Checks if a given ID is currently allocated.
    pub fn is_allocated(&self, id: usize) -> bool {
        let root = self.root.lock();
        root.is_allocated(id, IDA_MAX_LEVELS - 1)
    }
}

impl Default for Ida {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec::Vec;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn test_alloc_and_free_simple() {
        let ida = Ida::default();
        assert_eq!(ida.alloc(), Some(0));
        assert_eq!(ida.alloc(), Some(1));
        assert_eq!(ida.alloc(), Some(2));

        ida.free(1); // Free an ID in the middle.
        assert_eq!(ida.alloc(), Some(1)); // Should be reused.

        ida.free(0);
        ida.free(2);
        assert_eq!(ida.alloc(), Some(0));
        assert_eq!(ida.alloc(), Some(2));
    }

    #[test]
    fn test_first_level_boundary() {
        let ida = Ida::default();
        // Allocate the first 64 IDs to fill the first leaf.
        for i in 0..64 {
            assert_eq!(ida.alloc(), Some(i));
        }

        // The next allocation should cross the boundary into a new leaf.
        assert_eq!(ida.alloc(), Some(64));
        assert_eq!(ida.alloc(), Some(65));

        // Free the boundary-crossing ID and ensure it's reused.
        ida.free(64);
        assert_eq!(ida.alloc(), Some(64));
    }

    #[test]
    fn test_second_level_boundary() {
        let ida = Ida::default();
        let count = IDA_BITMAP_BITS * IDA_BITMAP_BITS; // 64 * 64 = 4096

        // Allocate enough IDs to fill an entire first-level child node.
        for i in 0..count {
            assert_eq!(ida.alloc(), Some(i));
        }

        // The next allocation should cross a major boundary.
        assert_eq!(ida.alloc(), Some(count));

        // Free it and ensure it's reused.
        ida.free(count);
        assert_eq!(ida.alloc(), Some(count));
    }

    #[test]
    fn test_free_unallocated() {
        let ida = Ida::default();
        ida.free(100); // Free an ID that was never allocated.
        assert_eq!(ida.alloc(), Some(0)); // The first ID should still be 0.
    }

    #[test]
    fn test_stress_and_random_free() {
        let ida = Ida::default();
        let mut ids = alloc::vec::Vec::new();
        let count = 10_000;

        // 1. Allocate a large number of IDs.
        for _ in 0..count {
            if let Some(id) = ida.alloc() {
                ids.push(id);
            } else {
                panic!("Allocator ran out of space unexpectedly.");
            }
        }

        // 2. Free a random subset of those IDs.
        let mut freed_ids = alloc::vec::Vec::new();
        for (i, &id) in ids.iter().enumerate() {
            if i % 3 == 0 || i % 7 == 0 {
                // Arbitrary condition for freeing
                ida.free(id);
                freed_ids.push(id);
            }
        }

        // Sort the freed IDs to make reallocation predictable.
        freed_ids.sort();

        // 3. Allocate new IDs and check if they are the same as the freed ones.
        for &expected_id in &freed_ids {
            assert_eq!(ida.alloc(), Some(expected_id));
        }

        // 4. Free all remaining IDs.
        for (i, &id) in ids.iter().enumerate() {
            if !(i % 3 == 0 || i % 7 == 0) {
                ida.free(id);
            }
        }
        for id in freed_ids {
            ida.free(id);
        }

        // 5. Check that the allocator is empty and starts from 0 again.
        assert_eq!(ida.alloc(), Some(0));
    }

    #[test]
    fn test_is_allocated() {
        let ida = Ida::default();
        assert!(!ida.is_allocated(0));
        assert!(!ida.is_allocated(100));

        let id1 = ida.alloc().unwrap();
        let id2 = ida.alloc().unwrap();

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        assert!(ida.is_allocated(0));
        assert!(ida.is_allocated(1));
        assert!(!ida.is_allocated(2));
        assert!(!ida.is_allocated(100));

        ida.free(1);
        assert!(ida.is_allocated(0));
        assert!(!ida.is_allocated(1));
        assert!(!ida.is_allocated(2));
    }

    #[test]
    fn test_debug_output() {
        let ida = Ida::default();
        ida.alloc();
        ida.alloc();
        // The exact output is not asserted as it's complex and implementation-dependent,
        // but this test ensures the Debug trait is implemented and doesn't panic.
        let _ = format!("{ida:?}");
    }

    #[test]
    fn test_multi_threaded_alloc() {
        let ida = Arc::new(Ida::default());
        let mut handles = Vec::new();
        let results = Arc::new(Mutex::new(Vec::new()));
        let num_threads = 4;
        let ids_per_thread = 1000;

        for _ in 0..num_threads {
            let ida_clone = Arc::clone(&ida);
            let results_clone = Arc::clone(&results);
            let handle = thread::spawn(move || {
                for _ in 0..ids_per_thread {
                    if let Some(id) = ida_clone.alloc() {
                        results_clone.lock().unwrap().push(id);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let mut final_ids = results.lock().unwrap();
        assert_eq!(final_ids.len(), num_threads * ids_per_thread);

        final_ids.sort();
        let original_len = final_ids.len();
        final_ids.dedup();
        assert_eq!(
            original_len,
            final_ids.len(),
            "Duplicate IDs were allocated in a multi-threaded context!"
        );
    }
}
