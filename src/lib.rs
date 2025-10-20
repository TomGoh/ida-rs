#![no_std]

extern crate alloc;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use spin::Mutex;

pub struct Ida {
    root: Mutex<IdaNode>,
}

struct IdaNode {
    bitmap: u64,
    children: BTreeMap<usize, Box<IdaNode>>,
}

impl IdaNode {
    fn new() -> Self {
        Self {
            bitmap: 0,
            children: BTreeMap::new(),
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

        if root.bitmap != u64::MAX {
            let id = root.bitmap.trailing_ones() as usize;
            root.bitmap |= 1 << id;
            return Some(id);
        }

        for i in 0.. {
            if let Some(child) = root.children.get_mut(&i) {
                if child.bitmap != u64::MAX {
                    let bit_index = child.bitmap.trailing_ones() as usize;
                    child.bitmap |= 1 << bit_index;
                    return Some((i + 1) * 64 + bit_index);
                }
                continue;
            }

            let mut new_child = Box::new(IdaNode::new());
            let bit_index = 0;
            new_child.bitmap |= 1 << bit_index;
            root.children.insert(i, new_child);
            return Some((i + 1) * 64 + bit_index);
        }

        unreachable!()
    }

    pub fn free(&self, id: usize) {
        let mut root = self.root.lock();

        let bit_index = id % 64;
        let block_index = id / 64;

        if block_index == 0 {
            root.bitmap &= !(1 << bit_index);
        } else {
            let child_index = block_index - 1;
            if let Some(child) = root.children.get_mut(&child_index) {
                child.bitmap &= !(1 << bit_index);
                if child.bitmap == 0 {
                    root.children.remove(&child_index);
                }
            }
        }
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

    #[test]
    fn test_alloc_simple() {
        let ida = Ida::new();
        assert_eq!(ida.alloc(), Some(0));
        assert_eq!(ida.alloc(), Some(1));
        assert_eq!(ida.alloc(), Some(2));
    }

    #[test]
    fn test_acoss_boundary_alloc() {
        let ida = Ida::new();
        for i in 0..64 {
            assert_eq!(ida.alloc(), Some(i));
        }
        assert_eq!(ida.alloc(), Some(64));
        assert_eq!(ida.alloc(), Some(65));
    }

    #[test]
    fn test_free_simple() {
        let ida = Ida::new();
        let id1 = ida.alloc().unwrap();
        let id2 = ida.alloc().unwrap();
        ida.free(id1);
        let id3 = ida.alloc().unwrap();
        assert_eq!(id1, id3);
        ida.free(id2);
        let id4 = ida.alloc().unwrap();
        assert_eq!(id2, id4);
    }

    #[test]
    fn test_free_from_child() {
        let ida = Ida::new();
        for _ in 0..66 {
            ida.alloc();
        }

        ida.free(65);
        let id = ida.alloc().unwrap();
        assert_eq!(id, 65);

        ida.free(10);
        let id = ida.alloc().unwrap();
        assert_eq!(id, 10);
    }
}
