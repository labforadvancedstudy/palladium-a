// Rust implementation of Splay Tree
use std::cmp::Ordering;
use std::mem;

#[derive(Debug)]
pub struct SplayTree<T: Ord> {
    root: Option<Box<Node<T>>>,
}

#[derive(Debug)]
struct Node<T> {
    value: T,
    left: Option<Box<Node<T>>>,
    right: Option<Box<Node<T>>>,
}

impl<T: Ord> Node<T> {
    fn new(value: T) -> Self {
        Node {
            value,
            left: None,
            right: None,
        }
    }
}

impl<T: Ord> SplayTree<T> {
    pub fn new() -> Self {
        SplayTree { root: None }
    }

    pub fn insert(&mut self, value: T) {
        if self.root.is_none() {
            self.root = Some(Box::new(Node::new(value)));
            return;
        }
        
        self.splay(|v| value.cmp(v));
        
        let mut root = self.root.take().unwrap();
        match value.cmp(&root.value) {
            Ordering::Less => {
                let mut new_root = Box::new(Node::new(value));
                new_root.left = root.left.take();
                new_root.right = Some(root);
                self.root = Some(new_root);
            }
            Ordering::Greater => {
                let mut new_root = Box::new(Node::new(value));
                new_root.right = root.right.take();
                new_root.left = Some(root);
                self.root = Some(new_root);
            }
            Ordering::Equal => {
                self.root = Some(root);
            }
        }
    }

    pub fn find(&mut self, value: &T) -> bool {
        if self.root.is_none() {
            return false;
        }
        
        self.splay(|v| value.cmp(v));
        
        self.root.as_ref().map(|node| &node.value == value).unwrap_or(false)
    }

    pub fn delete(&mut self, value: &T) -> bool {
        if self.root.is_none() {
            return false;
        }
        
        self.splay(|v| value.cmp(v));
        
        if let Some(root) = self.root.as_ref() {
            if &root.value != value {
                return false;
            }
        }
        
        let root = self.root.take().unwrap();
        match (root.left, root.right) {
            (None, None) => self.root = None,
            (Some(left), None) => self.root = Some(left),
            (None, Some(right)) => self.root = Some(right),
            (Some(left), Some(right)) => {
                self.root = Some(left);
                self.splay_max();
                let mut new_root = self.root.take().unwrap();
                new_root.right = Some(right);
                self.root = Some(new_root);
            }
        }
        
        true
    }

    fn splay<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T) -> Ordering,
    {
        let mut left_tree: Option<Box<Node<T>>> = None;
        let mut right_tree: Option<Box<Node<T>>> = None;
        let mut left_rightmost = &mut left_tree;
        let mut right_leftmost = &mut right_tree;
        
        let mut current = self.root.take();
        
        while let Some(mut node) = current {
            match cmp(&node.value) {
                Ordering::Less => {
                    if let Some(mut left) = node.left.take() {
                        match cmp(&left.value) {
                            Ordering::Less => {
                                // Zig-zig
                                node.left = left.right.take();
                                left.right = Some(node);
                                current = left.left.take();
                                *right_leftmost = Some(left);
                                right_leftmost = &mut right_leftmost.as_mut().unwrap().left;
                            }
                            _ => {
                                // Zig
                                current = left.left.take();
                                node.left = left.right.take();
                                left.right = Some(node);
                                *right_leftmost = Some(left);
                                right_leftmost = &mut right_leftmost.as_mut().unwrap().left;
                            }
                        }
                    } else {
                        current = None;
                        *right_leftmost = Some(node);
                        right_leftmost = &mut right_leftmost.as_mut().unwrap().left;
                    }
                }
                Ordering::Greater => {
                    if let Some(mut right) = node.right.take() {
                        match cmp(&right.value) {
                            Ordering::Greater => {
                                // Zag-zag
                                node.right = right.left.take();
                                right.left = Some(node);
                                current = right.right.take();
                                *left_rightmost = Some(right);
                                left_rightmost = &mut left_rightmost.as_mut().unwrap().right;
                            }
                            _ => {
                                // Zag
                                current = right.right.take();
                                node.right = right.left.take();
                                right.left = Some(node);
                                *left_rightmost = Some(right);
                                left_rightmost = &mut left_rightmost.as_mut().unwrap().right;
                            }
                        }
                    } else {
                        current = None;
                        *left_rightmost = Some(node);
                        left_rightmost = &mut left_rightmost.as_mut().unwrap().right;
                    }
                }
                Ordering::Equal => {
                    node.left = left_tree;
                    node.right = right_tree;
                    self.root = Some(node);
                    return;
                }
            }
        }
        
        // Reassemble
        *left_rightmost = right_tree;
        self.root = left_tree;
    }

    fn splay_max(&mut self) {
        self.splay(|_| Ordering::Less);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_find() {
        let mut tree = SplayTree::new();
        tree.insert(10);
        tree.insert(5);
        tree.insert(15);
        
        assert!(tree.find(&10));
        assert!(tree.find(&5));
        assert!(tree.find(&15));
        assert!(!tree.find(&20));
    }

    #[test]
    fn test_delete() {
        let mut tree = SplayTree::new();
        tree.insert(10);
        tree.insert(5);
        tree.insert(15);
        
        assert!(tree.delete(&5));
        assert!(!tree.find(&5));
        assert!(tree.find(&10));
        assert!(tree.find(&15));
    }
}