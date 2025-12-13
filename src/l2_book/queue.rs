use std::{collections::HashMap, hash::Hash};

use slab::Slab;

struct Node<K, V> {
    key: K,
    value: V,
    next: Option<usize>,
    prev: Option<usize>,
}

impl<K, V> Node<K, V> {
    pub fn new(key: K, value: V) -> Node<K, V> {
        Node {
            key,
            value,
            next: None,
            prev: None,
        }
    }
}

pub struct Queue<K, V> {
    keys: HashMap<K, usize>,
    backing: slab::Slab<Node<K, V>>,
    head: Option<usize>,
    tail: Option<usize>,
}

impl<K: Eq + Hash + Clone, V> Queue<K, V> {
    pub fn new() -> Queue<K, V> {
        Queue {
            keys: HashMap::new(),
            backing: Slab::new(),
            head: None,
            tail: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn front(&mut self) -> Option<&mut V> {
        let id = self.head?;
        let node = self.backing.get_mut(id)?;
        Some(&mut node.value)
    }

    pub fn remove_key(&mut self, key: K) -> bool {
        match self.keys.remove(&key) {
            Some(key) => {
                let node = self.backing.remove(key);

                if let Some(nid) = node.next {
                    let next_node = self.backing.get_mut(nid).unwrap();
                    next_node.prev = node.prev;
                } else {
                    self.tail = None
                }

                if let Some(pid) = node.prev {
                    let prev_node = self.backing.get_mut(pid).unwrap();
                    prev_node.next = node.next;
                } else {
                    self.head = None
                }

                true
            }
            None => false,
        }
    }

    pub fn push_back(&mut self, key: K, value: V) -> bool {
        if self.keys.contains_key(&key) {
            return false;
        }

        let new_node = Node::new(key.clone(), value);
        let id = self.backing.insert(new_node);
        self.keys.insert(key, id);

        match self.tail {
            Some(pid) => {
                let prev_tail = self.backing.get_mut(pid).unwrap();
                prev_tail.next = Some(id);
                let new_tail = self.backing.get_mut(id).unwrap();
                new_tail.prev = Some(pid);
                self.tail = Some(id);
            }
            None => {
                self.head = Some(id);
                self.tail = Some(id);
            }
        };

        true
    }

    pub fn pop_front(&mut self) -> Option<V> {
        match self.head {
            Some(id) => {
                let head = self.backing.try_remove(id)?;
                let nid = {
                    self.keys.remove(&head.key);
                    head.next
                };

                match nid {
                    Some(nid) => {
                        let next_head = self.backing.get_mut(nid).unwrap();
                        next_head.prev = None;
                        self.head = Some(nid);
                    }
                    None => {
                        self.head = None;
                        self.tail = None;
                    }
                };

                Some(head.value)
            }
            None => None,
        }
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, K, V> {
        Iter {
            list: self,
            current: self.head,
        }
    }
}

pub struct Iter<'a, K, V> {
    list: &'a Queue<K, V>,
    current: Option<usize>,
}

impl<'a, K: Eq + Hash + Clone, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.list.backing[id];
        self.current = node.next;
        Some((&node.key, &node.value))
    }
}

impl<K: Eq + Hash + Clone, V> Default for Queue<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::Queue;

    #[test]
    fn basics() {
        let mut list = Queue::new();

        // Check empty list behaves right
        assert_eq!(list.pop_front(), None);

        // Populate list
        list.push_back("k", 1);
        list.push_back("k", 2);
        list.push_back("v", 2);

        // Check normal removal
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_front(), Some(2));
    }

    #[test]
    fn remove_key() {
        let mut list = Queue::new();

        list.push_back("a", "a");
        list.push_back("b", "b");
        list.push_back("c", "c");
        list.push_back("d", "d");

        assert!(!list.remove_key("e"));
        assert!(list.remove_key("b"));

        assert_eq!(list.front(), Some(&mut "a"));

        assert_eq!(list.pop_front(), Some("a"));
        assert_eq!(list.pop_front(), Some("c"));
        assert_eq!(list.pop_front(), Some("d"));
    }
}
