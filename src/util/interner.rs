use std::collections::HashMap;
use std::hash::Hash;

// TODO - write a better Interner
#[derive(Debug, Default)]
pub struct Interner<T> {
    map: HashMap<T, usize>,
    items: Vec<T>,
}

impl<T: Eq + Hash + Clone> Interner<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            items: Vec::new(),
        }
    }

    pub fn intern(&mut self, value: T) -> usize {
        if let Some(&index) = self.map.get(&value) {
            index
        } else {
            let index = self.items.len();
            self.items.push(value.clone());
            self.map.insert(value, index);
            index
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn get_index<U>(&self, key: &U) -> Option<usize>
    where
        T: std::borrow::Borrow<U>,
        U: Hash + Eq + ?Sized,
    {
        self.map.get(&key).copied()
    }

    pub fn intern_ref<U>(&mut self, value: &U) -> usize
    where
        T: std::borrow::Borrow<U>,
        U: Hash + Eq + ?Sized + std::borrow::ToOwned<Owned = T>,
    {
        if let Some(&index) = self.map.get(value) {
            index
        } else {
            self.intern(value.to_owned())
        }
    }
}
