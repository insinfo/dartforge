//! The hasher of the runtime's internal maps: Fx (the one from `rustc`), instead of
//! the SipHash of the standard `HashMap`.
//!
//! The keys here are the runtime's own ids, handles and types (the type table,
//! the subtype caches, the sets of special lists) — never data chosen by the
//! Dart program: Dart's `Map` and `Set` are implemented in Dart by the SDK. So
//! there are no provoked collisions to avoid, and SipHash cost about a third of
//! the time of a `Map.[]=` loop (measured with callgrind). Iteration order
//! becomes deterministic; nothing in the runtime depended on it.

use std::hash::Hasher;

/// The `FxHasher`: rotate, mix in the word and multiply.
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

const K: u64 = 0x517c_c1b7_2722_0a95;

impl Fx {
    #[inline]
    fn word(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(K);
    }
}

impl Hasher for Fx {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for p in chunks {
            self.word(u64::from_le_bytes(*p));
        }
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.word(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.word(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.word(i as u64);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.word(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `HashMap` with the Fx hasher.
#[cfg(not(target_arch = "wasm32"))]
pub type HashMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<Fx>>;
/// `HashSet` with the Fx hasher.
#[cfg(not(target_arch = "wasm32"))]
pub type HashSet<K> = std::collections::HashSet<K, std::hash::BuildHasherDefault<Fx>>;
/// The entry of a [`HashMap`] key.
#[cfg(not(target_arch = "wasm32"))]
pub use std::collections::hash_map::Entry;

/// The runtime's map in a WebAssembly module: [`compact::Map`], whose code
/// is a fraction of the standard map's for the same types.
#[cfg(target_arch = "wasm32")]
pub type HashMap<K, V> = compact::Map<K, V>;
/// The runtime's set in a WebAssembly module ([`compact::Set`]).
#[cfg(target_arch = "wasm32")]
pub type HashSet<K> = compact::Set<K>;
/// The entry of a [`HashMap`] key.
#[cfg(target_arch = "wasm32")]
pub use compact::Entry;

/// A hash map and set with a small code footprint (WebAssembly modules).
///
/// The entries live in insertion order in a vector, with their hashes in
/// another; an open-addressing index (linear probing, backward-shift deletion)
/// of 64-bit slots, each with an entry's hash and position, maps a hash to its
/// entry: a search reads the slots and only the entry whose hash matches.
/// Everything that moves the index only reads the hashes, so it is compiled
/// once for every key and value type;
/// each type only instantiates the key comparison and the vector moves. The
/// standard `HashMap` (hashbrown) compiled its rehash and insertion again for
/// each of the runtime's ~25 map types (~40 KB of a module). Removal moves the
/// last entry into the hole (`swap_remove`): the iteration order is
/// deterministic but not the insertion order, as nothing in the runtime
/// depends on the order.
pub mod compact {
    use std::borrow::Borrow;
    use std::hash::{BuildHasher, Hash};

    type Hasher = std::hash::BuildHasherDefault<super::Fx>;

    /// The 32-bit hash of `k`: the Fx word through a multiplicative mix, whose
    /// high bits pick the slot (the Fx word of an aligned handle has zero low
    /// bits).
    #[inline]
    fn hash_of<Q: Hash + ?Sized>(k: &Q) -> u32 {
        let h = Hasher::default().hash_one(k);
        (h.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as u32
    }

    /// A slot of the index: the entry's hash in the high half and its position
    /// plus one in the low half (0 is an empty slot). The hash in the slot lets
    /// a search skip the other keys without reading the entries.
    #[inline]
    fn slot(hash: u32, i: usize) -> u64 {
        (u64::from(hash) << 32) | (i as u64 + 1)
    }

    /// The entry of a non-empty slot.
    #[inline]
    fn entry_of(s: u64) -> usize {
        (s as u32) as usize - 1
    }

    /// The first slot of `hash` in an index of `slots.len()` (a power of two).
    #[inline]
    fn home(hash: u32, slots: &[u64]) -> usize {
        // `slots.len()` ≥ 8: the shift is at most 29.
        (hash >> (32 - slots.len().trailing_zeros())) as usize
    }

    /// The index for `hashes` with room for `capacity` entries: every entry
    /// placed again (non-generic).
    #[inline(never)]
    fn rebuild(slots: &mut Vec<u64>, hashes: &[u32], capacity: usize) {
        let mut n = 8;
        while n * 3 < capacity * 4 {
            n *= 2;
        }
        slots.clear();
        slots.resize(n, 0);
        for (i, &h) in hashes.iter().enumerate() {
            place(slots, h, i);
        }
    }

    /// Room in the index for one more entry than `hashes` has.
    #[inline]
    fn reserve_one(slots: &mut Vec<u64>, hashes: &[u32]) {
        if (hashes.len() + 1) * 4 > slots.len() * 3 {
            rebuild(slots, hashes, (hashes.len() + 1) * 2);
        }
    }

    /// Places entry `i` (of hash `hash`) in the index (non-generic).
    #[inline(never)]
    fn place(slots: &mut [u64], hash: u32, i: usize) {
        let mask = slots.len() - 1;
        let mut p = home(hash, slots);
        while slots[p] != 0 {
            p = (p + 1) & mask;
        }
        slots[p] = slot(hash, i);
    }

    /// The slot of entry `i`, of hash `hash` (non-generic).
    fn slot_of(slots: &[u64], hash: u32, i: usize) -> usize {
        let mask = slots.len() - 1;
        let mut p = home(hash, slots);
        while slots[p] != slot(hash, i) {
            p = (p + 1) & mask;
        }
        p
    }

    /// Takes entry `i` out of the index, and points the slot of the last entry
    /// to `i` (the entry vectors then `swap_remove(i)`); non-generic.
    #[inline(never)]
    fn unindex(slots: &mut [u64], hashes: &[u32], i: usize) {
        let mask = slots.len() - 1;
        let mut hole = slot_of(slots, hashes[i], i);
        // Backward shift: each following entry that may move closer to its home
        // fills the hole.
        let mut p = (hole + 1) & mask;
        while slots[p] != 0 {
            let h = home((slots[p] >> 32) as u32, slots);
            // `p` may move into `hole` when its home is not in (hole, p].
            if (p.wrapping_sub(h) & mask) >= (p.wrapping_sub(hole) & mask) {
                slots[hole] = slots[p];
                hole = p;
            }
            p = (p + 1) & mask;
        }
        slots[hole] = 0;
        let last = hashes.len() - 1;
        if i != last {
            let s = slot_of(slots, hashes[last], last);
            slots[s] = slot(hashes[last], i);
        }
    }

    /// The map.
    pub struct Map<K, V> {
        entries: Vec<(K, V)>,
        hashes: Vec<u32>,
        slots: Vec<u64>,
    }

    impl<K, V> Default for Map<K, V> {
        fn default() -> Self {
            Map {
                entries: Vec::new(),
                hashes: Vec::new(),
                slots: Vec::new(),
            }
        }
    }

    impl<K: Clone, V: Clone> Clone for Map<K, V> {
        fn clone(&self) -> Self {
            Map {
                entries: self.entries.clone(),
                hashes: self.hashes.clone(),
                slots: self.slots.clone(),
            }
        }
    }

    impl<K: std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for Map<K, V> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_map()
                .entries(self.entries.iter().map(|(k, v)| (k, v)))
                .finish()
        }
    }

    impl<K, V> Map<K, V> {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn with_capacity(n: usize) -> Self {
            Map {
                entries: Vec::with_capacity(n),
                hashes: Vec::with_capacity(n),
                slots: Vec::new(),
            }
        }
        pub fn len(&self) -> usize {
            self.entries.len()
        }
        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
        pub fn capacity(&self) -> usize {
            self.entries.capacity()
        }
        pub fn clear(&mut self) {
            self.entries.clear();
            self.hashes.clear();
            self.slots.clear();
        }
        pub fn iter(&self) -> Iter<'_, K, V> {
            Iter(self.entries.iter())
        }
        pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
            IterMut(self.entries.iter_mut())
        }
        pub fn keys(&self) -> impl Iterator<Item = &K> + '_ {
            self.entries.iter().map(|(k, _)| k)
        }
        pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
            self.entries.iter().map(|(_, v)| v)
        }
        pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> + '_ {
            self.entries.iter_mut().map(|(_, v)| v)
        }
        pub fn into_values(self) -> impl Iterator<Item = V> {
            self.entries.into_iter().map(|(_, v)| v)
        }
        pub fn into_keys(self) -> impl Iterator<Item = K> {
            self.entries.into_iter().map(|(k, _)| k)
        }
        pub fn drain(&mut self) -> std::vec::IntoIter<(K, V)> {
            self.hashes.clear();
            self.slots.clear();
            std::mem::take(&mut self.entries).into_iter()
        }
        /// Keeps the entries `f` accepts.
        pub fn retain(&mut self, mut f: impl FnMut(&K, &mut V) -> bool) {
            let n = self.entries.len();
            let mut i = 0;
            while i < self.entries.len() {
                let (k, v) = &mut self.entries[i];
                if f(k, v) {
                    i += 1;
                } else {
                    self.entries.swap_remove(i);
                    self.hashes.swap_remove(i);
                }
            }
            if self.entries.len() != n {
                rebuild(&mut self.slots, &self.hashes, self.hashes.len());
            }
        }
        /// Removes entry `i` (the last one takes its place).
        fn take_at(&mut self, i: usize) -> (K, V) {
            unindex(&mut self.slots, &self.hashes, i);
            self.hashes.swap_remove(i);
            self.entries.swap_remove(i)
        }
        /// Appends an entry whose key is not in the map: `vacancy` is the empty
        /// slot where the search for it stopped ([`Map::find`]), which takes it
        /// unless the index must grow first.
        fn push(&mut self, hash: u32, vacancy: usize, k: K, v: V) -> usize {
            let i = self.entries.len();
            if vacancy == usize::MAX || (i + 1) * 4 > self.slots.len() * 3 {
                reserve_one(&mut self.slots, &self.hashes);
                place(&mut self.slots, hash, i);
            } else {
                self.slots[vacancy] = slot(hash, i);
            }
            self.hashes.push(hash);
            self.entries.push((k, v));
            i
        }
    }

    impl<K: Hash + Eq, V> Map<K, V> {
        /// The entry of `k`, or the empty slot where its search stopped
        /// (`usize::MAX` without an index), and its hash.
        #[inline]
        fn find<Q>(&self, k: &Q) -> (Result<usize, usize>, u32)
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            let hash = hash_of(k);
            if self.slots.is_empty() {
                return (Err(usize::MAX), hash);
            }
            let mask = self.slots.len() - 1;
            let mut p = home(hash, &self.slots);
            loop {
                let s = self.slots[p];
                if s == 0 {
                    return (Err(p), hash);
                }
                if (s >> 32) as u32 == hash {
                    let i = entry_of(s);
                    if self.entries[i].0.borrow() == k {
                        return (Ok(i), hash);
                    }
                }
                p = (p + 1) & mask;
            }
        }
        pub fn get<Q>(&self, k: &Q) -> Option<&V>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.find(k).0.ok().map(|i| &self.entries[i].1)
        }
        pub fn get_key_value<Q>(&self, k: &Q) -> Option<(&K, &V)>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.find(k).0.ok().map(|i| {
                let (k, v) = &self.entries[i];
                (k, v)
            })
        }
        pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.find(k).0.ok().map(|i| &mut self.entries[i].1)
        }
        pub fn contains_key<Q>(&self, k: &Q) -> bool
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.find(k).0.is_ok()
        }
        pub fn insert(&mut self, k: K, v: V) -> Option<V> {
            match self.find(&k) {
                (Ok(i), _) => Some(std::mem::replace(&mut self.entries[i].1, v)),
                (Err(vacancy), hash) => {
                    self.push(hash, vacancy, k, v);
                    None
                }
            }
        }
        pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.remove_entry(k).map(|(_, v)| v)
        }
        pub fn remove_entry<Q>(&mut self, k: &Q) -> Option<(K, V)>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            let i = self.find(k).0.ok()?;
            Some(self.take_at(i))
        }
        pub fn entry(&mut self, k: K) -> Entry<'_, K, V> {
            match self.find(&k) {
                (Ok(i), _) => Entry::Occupied(OccupiedEntry { map: self, i }),
                (Err(vacancy), hash) => Entry::Vacant(VacantEntry {
                    map: self,
                    hash,
                    vacancy,
                    key: k,
                }),
            }
        }
        pub fn reserve(&mut self, n: usize) {
            self.entries.reserve(n);
            self.hashes.reserve(n);
        }
    }

    impl<K: Hash + Eq, V: PartialEq> PartialEq for Map<K, V> {
        fn eq(&self, other: &Self) -> bool {
            self.len() == other.len()
                && self
                    .entries
                    .iter()
                    .all(|(k, v)| other.get(k).is_some_and(|w| v == w))
        }
    }

    impl<K: Hash + Eq, V: Eq> Eq for Map<K, V> {}

    impl<K: Hash + Eq, V> FromIterator<(K, V)> for Map<K, V> {
        fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
            let mut m = Map::new();
            m.extend(iter);
            m
        }
    }

    impl<K: Hash + Eq, V> Extend<(K, V)> for Map<K, V> {
        fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
            for (k, v) in iter {
                self.insert(k, v);
            }
        }
    }

    impl<K: Hash + Eq + Borrow<Q>, Q: Hash + Eq + ?Sized, V> std::ops::Index<&Q> for Map<K, V> {
        type Output = V;
        fn index(&self, k: &Q) -> &V {
            self.get(k).expect("key in the map")
        }
    }

    impl<K, V> IntoIterator for Map<K, V> {
        type Item = (K, V);
        type IntoIter = std::vec::IntoIter<(K, V)>;
        fn into_iter(self) -> Self::IntoIter {
            self.entries.into_iter()
        }
    }

    impl<'a, K, V> IntoIterator for &'a Map<K, V> {
        type Item = (&'a K, &'a V);
        type IntoIter = Iter<'a, K, V>;
        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    impl<'a, K, V> IntoIterator for &'a mut Map<K, V> {
        type Item = (&'a K, &'a mut V);
        type IntoIter = IterMut<'a, K, V>;
        fn into_iter(self) -> Self::IntoIter {
            self.iter_mut()
        }
    }

    /// The entries of a [`Map`].
    pub struct Iter<'a, K, V>(std::slice::Iter<'a, (K, V)>);

    impl<'a, K, V> Iterator for Iter<'a, K, V> {
        type Item = (&'a K, &'a V);
        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|(k, v)| (k, v))
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
    }

    impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

    impl<K, V> Clone for Iter<'_, K, V> {
        fn clone(&self) -> Self {
            Iter(self.0.clone())
        }
    }

    /// The entries of a [`Map`], with mutable values.
    pub struct IterMut<'a, K, V>(std::slice::IterMut<'a, (K, V)>);

    impl<'a, K, V> Iterator for IterMut<'a, K, V> {
        type Item = (&'a K, &'a mut V);
        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|(k, v)| (&*k, v))
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
    }

    /// The entry of a key ([`Map::entry`]).
    pub enum Entry<'a, K, V> {
        Occupied(OccupiedEntry<'a, K, V>),
        Vacant(VacantEntry<'a, K, V>),
    }

    pub struct OccupiedEntry<'a, K, V> {
        map: &'a mut Map<K, V>,
        i: usize,
    }

    pub struct VacantEntry<'a, K, V> {
        map: &'a mut Map<K, V>,
        hash: u32,
        vacancy: usize,
        key: K,
    }

    impl<'a, K, V> Entry<'a, K, V> {
        pub fn or_insert(self, v: V) -> &'a mut V {
            match self {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(e) => e.insert(v),
            }
        }
        pub fn or_insert_with(self, f: impl FnOnce() -> V) -> &'a mut V {
            match self {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(e) => e.insert(f()),
            }
        }
        pub fn or_default(self) -> &'a mut V
        where
            V: Default,
        {
            self.or_insert_with(V::default)
        }
        pub fn and_modify(mut self, f: impl FnOnce(&mut V)) -> Self {
            if let Entry::Occupied(e) = &mut self {
                f(e.get_mut());
            }
            self
        }
        pub fn key(&self) -> &K {
            match self {
                Entry::Occupied(e) => e.key(),
                Entry::Vacant(e) => &e.key,
            }
        }
    }

    impl<'a, K, V> OccupiedEntry<'a, K, V> {
        pub fn key(&self) -> &K {
            &self.map.entries[self.i].0
        }
        pub fn get(&self) -> &V {
            &self.map.entries[self.i].1
        }
        pub fn get_mut(&mut self) -> &mut V {
            &mut self.map.entries[self.i].1
        }
        pub fn into_mut(self) -> &'a mut V {
            &mut self.map.entries[self.i].1
        }
        pub fn insert(&mut self, v: V) -> V {
            std::mem::replace(self.get_mut(), v)
        }
        pub fn remove(self) -> V {
            self.map.take_at(self.i).1
        }
        pub fn remove_entry(self) -> (K, V) {
            self.map.take_at(self.i)
        }
    }

    impl<'a, K, V> VacantEntry<'a, K, V> {
        pub fn key(&self) -> &K {
            &self.key
        }
        pub fn into_key(self) -> K {
            self.key
        }
        pub fn insert(self, v: V) -> &'a mut V {
            let i = self.map.push(self.hash, self.vacancy, self.key, v);
            &mut self.map.entries[i].1
        }
    }

    /// The set: a [`Map`] without values.
    pub struct Set<K>(Map<K, ()>);

    impl<K> Default for Set<K> {
        fn default() -> Self {
            Set(Map::default())
        }
    }

    impl<K: Clone> Clone for Set<K> {
        fn clone(&self) -> Self {
            Set(self.0.clone())
        }
    }

    impl<K: std::fmt::Debug> std::fmt::Debug for Set<K> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_set().entries(self.0.keys()).finish()
        }
    }

    impl<K> Set<K> {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn with_capacity(n: usize) -> Self {
            Set(Map::with_capacity(n))
        }
        pub fn len(&self) -> usize {
            self.0.len()
        }
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
        pub fn capacity(&self) -> usize {
            self.0.capacity()
        }
        pub fn clear(&mut self) {
            self.0.clear()
        }
        pub fn iter(&self) -> SetIter<'_, K> {
            SetIter(self.0.entries.iter())
        }
        pub fn drain(&mut self) -> impl Iterator<Item = K> + '_ {
            self.0.drain().map(|(k, ())| k)
        }
        pub fn retain(&mut self, mut f: impl FnMut(&K) -> bool) {
            self.0.retain(|k, ()| f(k))
        }
    }

    impl<K: Hash + Eq> Set<K> {
        pub fn insert(&mut self, k: K) -> bool {
            match self.0.find(&k) {
                (Ok(_), _) => false,
                (Err(vacancy), hash) => {
                    self.0.push(hash, vacancy, k, ());
                    true
                }
            }
        }
        pub fn contains<Q>(&self, k: &Q) -> bool
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.contains_key(k)
        }
        pub fn get<Q>(&self, k: &Q) -> Option<&K>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.get_key_value(k).map(|(k, ())| k)
        }
        pub fn remove<Q>(&mut self, k: &Q) -> bool
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.remove(k).is_some()
        }
        pub fn take<Q>(&mut self, k: &Q) -> Option<K>
        where
            K: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.remove_entry(k).map(|(k, ())| k)
        }
        pub fn reserve(&mut self, n: usize) {
            self.0.reserve(n)
        }
    }

    impl<K: Hash + Eq> PartialEq for Set<K> {
        fn eq(&self, other: &Self) -> bool {
            self.len() == other.len() && self.iter().all(|k| other.contains(k))
        }
    }

    impl<K: Hash + Eq> Eq for Set<K> {}

    impl<K: Hash + Eq> FromIterator<K> for Set<K> {
        fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
            let mut s = Set::new();
            s.extend(iter);
            s
        }
    }

    impl<K: Hash + Eq> Extend<K> for Set<K> {
        fn extend<I: IntoIterator<Item = K>>(&mut self, iter: I) {
            for k in iter {
                self.insert(k);
            }
        }
    }

    impl<'a, K: Hash + Eq + Copy + 'a> Extend<&'a K> for Set<K> {
        fn extend<I: IntoIterator<Item = &'a K>>(&mut self, iter: I) {
            for k in iter {
                self.insert(*k);
            }
        }
    }

    impl<K> IntoIterator for Set<K> {
        type Item = K;
        type IntoIter = std::iter::Map<std::vec::IntoIter<(K, ())>, fn((K, ())) -> K>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.entries.into_iter().map(|(k, ())| k)
        }
    }

    impl<'a, K> IntoIterator for &'a Set<K> {
        type Item = &'a K;
        type IntoIter = SetIter<'a, K>;
        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    /// The keys of a [`Set`].
    pub struct SetIter<'a, K>(std::slice::Iter<'a, (K, ())>);

    impl<'a, K> Iterator for SetIter<'a, K> {
        type Item = &'a K;
        fn next(&mut self) -> Option<&'a K> {
            self.0.next().map(|(k, ())| k)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
    }

    impl<K> Clone for SetIter<'_, K> {
        fn clone(&self) -> Self {
            SetIter(self.0.clone())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The map against the standard one, over a mixed sequence of
        /// insertions, removals, entries and retains.
        #[test]
        fn behaves_like_the_standard_map() {
            let mut ours: Map<i64, i64> = Map::new();
            let mut std_map: std::collections::HashMap<i64, i64> = Default::default();
            let mut x: u64 = 0x1234_5678;
            for step in 0..20_000 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                // Aligned keys, as handles are.
                let k = ((x % 512) as i64) * 16;
                match step % 7 {
                    0..=2 => assert_eq!(ours.insert(k, step), std_map.insert(k, step)),
                    3 => assert_eq!(ours.remove(&k), std_map.remove(&k)),
                    4 => {
                        *ours.entry(k).or_insert(0) += 1;
                        *std_map.entry(k).or_insert(0) += 1;
                    }
                    5 => assert_eq!(ours.get(&k), std_map.get(&k)),
                    _ if step % 1000 == 6 => {
                        ours.retain(|k, _| k % 3 != 0);
                        std_map.retain(|k, _| k % 3 != 0);
                    }
                    _ => assert_eq!(ours.contains_key(&k), std_map.contains_key(&k)),
                }
                assert_eq!(ours.len(), std_map.len());
            }
            let mut a: Vec<(i64, i64)> = ours.iter().map(|(k, v)| (*k, *v)).collect();
            let mut b: Vec<(i64, i64)> = std_map.into_iter().collect();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b);
        }

        #[test]
        fn borrowed_keys_and_sets() {
            let mut m: Map<String, usize> = Map::new();
            m.insert("a".to_string(), 1);
            m.insert("b".to_string(), 2);
            assert_eq!(m.get("a"), Some(&1));
            assert_eq!(m["b"], 2);
            assert_eq!(m.remove("a"), Some(1));
            assert!(!m.contains_key("a"));
            let mut s: Set<Vec<u16>> = Set::new();
            assert!(s.insert(vec![1, 2]));
            assert!(!s.insert(vec![1, 2]));
            assert!(s.contains(&[1u16, 2][..]));
            assert!(s.remove(&[1u16, 2][..]));
            assert!(s.is_empty());
        }
    }
}
