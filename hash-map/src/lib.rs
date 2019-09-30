use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::mem;

use fnv::FnvHasher;

#[derive(Clone)]
pub(crate) struct Map<K, V> {
    buckets: Vec<Vec<(K, V)>>,
    items: usize,
    SIZE: Option<usize>,
}

impl<K, V> Map<K, V> {
    pub fn new(bucket_size: Option<usize>) -> Self {
        Map {
            buckets: Vec::new(),
            items: 0,
            SIZE: bucket_size,
        }
    }
}

impl<K, V> Map<K, V>
where
    K: Hash + Eq,
{
    fn bucket<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut hasher = FnvHasher::default();
        key.hash(&mut hasher);
        let h_key = hasher.finish();
        let bucket_idx = (h_key % self.buckets.len() as u64) as usize;
        println!(
            "bucket: {} % {} = {}",
            h_key,
            self.buckets.len(),
            bucket_idx
        );
        bucket_idx
    }

    fn resize(&mut self) {
        let target_size = match self.buckets.len() {
            0 => {
                if let Some(size) = self.SIZE {
                    size
                } else {
                    // TODO a sensible default??
                    1
                }
            }
            // bucket size doubles
            n => 2 * n,
        };
        let mut new_buckets = Vec::with_capacity(target_size);
        new_buckets.extend((0..target_size).map(|_| Vec::new()));

        for (k, v) in self.buckets.iter_mut().flat_map(|bucket| bucket.drain(..)) {
            let mut hasher = FnvHasher::default();
            k.hash(&mut hasher);
            let bucket_idx = (hasher.finish() % new_buckets.len() as u64) as usize;
            new_buckets[bucket_idx].push((k, v));
        }
        mem::replace(&mut self.buckets, new_buckets);
    }

    /// Number of items in the hashmap.
    pub fn len(&self) -> usize {
        self.items
    }

    /// Returns true if hashmap has no elements.
    pub fn is_empty(&self) -> bool {
        self.items == 0
    }

    /// Returns true if map contains given key.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// Insert key value pair into hashmap.
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        //                           total > 3 * 
        if self.buckets.is_empty() || self.items > 3 * self.buckets.len() / 4 {
            self.resize();
        }

        let bucket_idx = self.bucket(&key);
        let bucket = &mut self.buckets[bucket_idx];

        self.items += 1;
        for (ekey, eval) in bucket.iter_mut() {
            if ekey == &key {
                return Some(mem::replace(eval, val));
            }
        }
        bucket.push((key, val));
        None
    }

    /// Insert key value pair into hashmap.
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        let bucket_idx = self.bucket(&key);
        // let bucket = &mut self.buckets[bucket_idx];

        if let Some(entry) = self.buckets[bucket_idx]
            .iter_mut()
            .find(|(k, _)| k == &key)
            {
                return Entry::Occupied( OccEntry {
                    entry: unsafe { &mut *(entry as *mut _) }
                } );
            };

        Entry::Vacant( VacEntry { key, bucket: &mut self.buckets[bucket_idx], } )
    }

    /// Iterator over keys and values.
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            bucket_idx: 0,
            item_idx: 0,
            map: self,
        }
    }

    /// Get value from key.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let bucket_idx = self.bucket(key.borrow());
        self.buckets[bucket_idx]
            .iter()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v)
    }

    /// Get value from key.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let bucket_idx = self.bucket(key.borrow());
        self.buckets[bucket_idx]
            .iter_mut()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v)
    }

    /// Removes key value pair based on key.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let bucket_idx = self.bucket(&key);
        let bucket = &mut self.buckets[bucket_idx];
        let idx = bucket.iter().position(|(k, _)| k.borrow() == key)?;
        self.items -= 1;
        Some(bucket.swap_remove(idx).1)
    }

    pub fn clear(&mut self) {
        self.items = 0;
        self.buckets.clear();
        self.resize();
    }
}

impl<'a, K, Q, V> std::ops::Index<&Q> for Map<K, V> 
where
    K: Hash + Eq + Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = V;
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("uhoh no entry found for key")
    }
}

impl<K, V> std::fmt::Debug for Map<K, V> 
where
    K: std::fmt::Display + Eq + Hash,
    V: std::fmt::Display + Eq + Hash,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Map {{")?;
        writeln!(f, "    buckets:")?;
        let mut count = 0;
        for b in self.buckets.iter() {
            for (k, v) in b.iter() {
                writeln!(f, "       #{} [ ({}, {}) ],", count, k, v)?;
                count += 1;
            }
            count += 1;
        }
        writeln!(f, "items: {}, SIZE: {:?}\n}}", self.items, self.SIZE)
    }
}

pub struct Iter<'a, K, V> {
    map: &'a Map<K, V>,
    bucket_idx: usize,
    item_idx: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.map.buckets.get(self.bucket_idx) {
                Some(bucket) => match bucket.get(self.item_idx) {
                    Some((k, v)) => {
                        self.item_idx += 1;
                        break Some((k, v));
                    }
                    None => {
                        self.bucket_idx += 1;
                        self.item_idx = 0;
                        continue;
                    }
                },
                None => break None,
            }
        }
    }
}

pub struct IterMut<'a, K, V> {
    map: Option<&'a mut Map<K, V>>,
    bucket_idx: usize,
    item_idx: usize,
}

impl<'a, K, V> IterMut<'a, K, V> {
    fn iter_mut(&'a mut self) -> Option<(&'a K, &'a mut V)> {
        loop {
            match self.map.take()?.buckets.get_mut(self.bucket_idx) {
                Some(bucket) => match bucket.get_mut(self.item_idx) {
                    Some((ref mut k, v)) => {
                        self.item_idx += 1;
                        break Some((k, v));
                    }
                    None => {
                        self.bucket_idx += 1;
                        self.item_idx = 0;
                        continue;
                    }
                },
                None => break None,
            }
        }
    }
}

impl<'a, K, V> IntoIterator for &'a Map<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            map: self,
            bucket_idx: 0,
            item_idx: 0,
        }
    }
}

pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, _)| k)
    }
}

pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, v)| v)
    }
}

pub struct OccEntry<'a, K, V> {
    entry: &'a mut (K, V),
}

pub struct VacEntry<'a, K, V> {
    key: K,
    bucket: &'a mut Vec<(K, V)>,
}

impl<'a, K, V> VacEntry<'a, K, V> {
    pub fn insert(self, val: V) -> &'a mut V {
        self.bucket.push((self.key, val));
        &mut self.bucket.last_mut().unwrap().1
    }
}

pub enum Entry<'a, K, V> {
    Occupied(OccEntry<'a, K, V>),
    Vacant(VacEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn or_insert(self, val: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => &mut entry.entry.1,
            Entry::Vacant(entry) => entry.insert(val),
        }
    }

    pub fn or_insert_with<F>(self, f: F) -> &'a mut V
    where
        F: FnOnce() -> V,
    {
        match self {
            Entry::Occupied(entry) => &mut entry.entry.1,
            Entry::Vacant(entry) => entry.insert(f()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_insert() {
        let mut map = Map::new(None);
        map.insert('a', 1);
        map.insert('b', 2);
        map.insert('f', 2);

        let val = map.get(&'a');
        println!("found {:?}", val);
        assert_eq!(val, Some(&1));
    }

    #[test]
    fn test_map_q_as_borrow() {
        let mut map = Map::new(None);
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.insert("c".to_string(), 3);

        let val = map.get("b");
        println!("found {:?}", val);
        println!("{:#?}", map);
        assert_eq!(val, Some(&2));
    }

    #[test]
    fn test_map_entry() {
        let mut map: Map<&str, u32> = Map::new(None);
        map.insert("foo", 11);
        map.entry("poneyland").or_insert(3);
        assert_eq!(map["poneyland"], 3);
        *map.entry("poneyland").or_insert(10) *= 2;
        assert_eq!(map["poneyland"], 6);

        map.clear();

        map.entry("poneyland").or_insert_with(|| 3 * 11);
        assert_eq!(map["poneyland"], 33);
    }

    #[test]
    fn test_map_big() {
        let mut map = Map::new(None);
        let pairs = (b'A'..=b'L').map(|c| c as char)
            .filter(|c| c.is_alphabetic())
            .collect::<Vec<_>>();
        
        for (k, v) in pairs.iter().enumerate() {
            map.insert(k, v);
        }
        println!("{:#?}", map);
    }
}
