use std::collections::HashMap;
use std::hash::{Hash, RandomState};

#[derive(Debug, Clone)]
pub struct ChainMap<K, V, S = RandomState> {
    values: Vec<V>,
    index: HashMap<K, Vec<usize>, S>,
    pop_index: Vec<usize>,
}

impl<K, V, S> Default for ChainMap<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            values: Vec::new(),
            index: HashMap::default(),
            pop_index: Vec::new(),
        }
    }
}

#[allow(unused)]
impl<K, V, S> ChainMap<K, V, S>
where
    S: Default,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            index: HashMap::with_capacity_and_hasher(capacity, S::default()),
            pop_index: Vec::new(),
        }
    }
}

#[allow(unused)]
impl<K, V, S> ChainMap<K, V, S> {
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            values: Vec::new(),
            index: HashMap::with_hasher(hasher),
            pop_index: Vec::new(),
        }
    }

    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            index: HashMap::with_capacity_and_hasher(capacity, hasher),
            pop_index: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.index
            .values()
            .filter(|indices| !indices.is_empty())
            .count()
    }

    pub fn chain_len(&self) -> usize {
        self.pop_index.len()
    }

    pub fn value_len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[allow(unused)]
impl<K, V, S> ChainMap<K, V, S>
where
    K: Eq + Hash,
    S: std::hash::BuildHasher,
{
    pub fn push_chain<M>(&mut self, map: M)
    where
        M: IntoIterator<Item = (K, V)>,
    {
        let start_index = self.values.len();
        self.pop_index.push(start_index);
        for (offset, (k, v)) in map.into_iter().enumerate() {
            self.values.push(v);
            self.index.entry(k).or_default().push(start_index + offset);
        }
    }

    pub fn pop_chain(&mut self) {
        let value_start = self.pop_index.pop().expect("chain empty");
        self.values.truncate(value_start);
        self.index.values_mut().for_each(|v| {
            v.retain(|&i| i < value_start);
        });
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.index.contains_key(key) && !self.index[key].is_empty()
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.index.get(key).and_then(|indices| {
            if let Some(&index) = indices.last() {
                self.values.get(index)
            } else {
                None
            }
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.index
            .iter()
            .filter_map(|(k, indices)| indices.last().map(|_| k))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.index
            .iter()
            .filter_map(|(_, indices)| indices.last())
            .filter_map(move |&index| self.values.get(index))
    }
}

impl<K, V, S> ChainMap<K, V, S>
where
    K: Eq + Hash,
    S: std::hash::BuildHasher + Default,
{
    fn as_hashmap(&self) -> HashMap<&K, &V, S> {
        self.index
            .iter()
            .filter_map(|(k, indices)| {
                indices
                    .last()
                    .and_then(|&index| self.values.get(index).map(|v| (k, v)))
            })
            .collect()
    }
}

impl<K, V, S> PartialEq for ChainMap<K, V, S>
where
    K: Eq + Hash,
    V: PartialEq,
    S: std::hash::BuildHasher + Default,
{
    fn eq(&self, other: &ChainMap<K, V, S>) -> bool {
        self.as_hashmap() == other.as_hashmap()
    }
}

impl<K, V, S> Eq for ChainMap<K, V, S>
where
    K: Eq + Hash,
    V: Eq,
    S: std::hash::BuildHasher + Default,
{
}

#[cfg(test)]
mod test {
    #[test]
    fn test_chainmap() {
        use super::ChainMap;
        use std::collections::HashMap;

        let mut chain_map: ChainMap<&'static str, u8> = ChainMap::new();

        chain_map.push_chain(HashMap::from([("a", 1), ("b", 2)]));

        assert_eq!(chain_map.get(&"a"), Some(&1));
        assert_eq!(chain_map.get(&"b"), Some(&2));

        chain_map.push_chain(HashMap::from([("a", 2), ("c", 5)]));

        assert_eq!(chain_map.get(&"a"), Some(&2));
        assert_eq!(chain_map.get(&"b"), Some(&2));
        assert_eq!(chain_map.get(&"c"), Some(&5));

        chain_map.pop_chain();

        assert_eq!(chain_map.get(&"a"), Some(&1));
        assert_eq!(chain_map.get(&"b"), Some(&2));

        chain_map.pop_chain();

        assert_eq!(chain_map.get(&"a"), None);
        assert_eq!(chain_map.get(&"b"), None);
    }
}
