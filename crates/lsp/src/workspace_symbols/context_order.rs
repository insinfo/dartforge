//! Driver-map iteration in the Dart 3.6.2 VM. ContextManager uses HashMap,
//! whose bucket order differs from the contexts' creation order. Folder
//! hashes are path String.hashCode values, computed over UTF-16 code units.
//!
//! Adapted from Dart SDK 3.6.2 collection_patch.dart (Copyright 2013, the Dart
//! project authors) and hash.h (Copyright 2018, the Dart project authors).
//! BSD license and attribution: THIRD_PARTY_NOTICES.md.

use super::Context;
use std::path::PathBuf;

mod dart_sort;

/// ContextLocator._uniqueSortedPaths: a fresh HashSet followed by List.sort
/// on UTF-16 path length. This precedes driver creation and differs from the
/// persistent driver map's order. Return each distinct root only once.
pub(crate) fn root_order(roots: &[PathBuf]) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<_> = roots
        .iter()
        .enumerate()
        .filter(|(_, p)| seen.insert(*p))
        .map(|(i, _)| i)
        .collect();
    let hashes: Vec<_> = unique
        .iter()
        .map(|&i| path_hash(&roots[i].to_string_lossy()))
        .collect();
    let mut indices: Vec<_> = bucket_order(&hashes, 8)
        .0
        .into_iter()
        .map(|i| unique[i])
        .collect();
    let lengths: Vec<_> = roots
        .iter()
        .map(|p| p.to_string_lossy().encode_utf16().count())
        .collect();
    dart_sort::sort(&mut indices, &lengths);
    indices
}

/// ContextManager removes each driver during a rebuild, without clearing its
/// HashMap. Retain its bucket capacity, even with no active analysis roots.
/// Only live roots and their already computed order remain in this model.
#[derive(Debug)]
pub(crate) struct DriverOrder {
    capacity: usize,
    roots: Vec<PathBuf>,
    indices: Vec<usize>,
}

impl Default for DriverOrder {
    fn default() -> Self {
        Self {
            capacity: 8,
            roots: Vec::new(),
            indices: Vec::new(),
        }
    }
}

impl DriverOrder {
    /// Reinsert a new context collection, retaining the old bucket capacity.
    pub(crate) fn rebuild(&mut self, roots: Vec<PathBuf>) {
        let hashes: Vec<_> = roots
            .iter()
            .map(|p| path_hash(&p.to_string_lossy()))
            .collect();
        let (indices, capacity) = bucket_order(&hashes, self.capacity);
        self.capacity = capacity;
        self.indices = indices;
        self.roots = roots;
    }

    /// Detect new context boundaries during disk revalidation as well as
    /// explicit root changes. A query alone must not rebuild collision chains.
    pub(crate) fn update(&mut self, roots: Vec<PathBuf>) -> bool {
        if roots == self.roots {
            return false;
        }
        self.rebuild(roots);
        true
    }

    /// Attach the saved iteration rank without reordering context creation,
    /// which also determines bootstrap and fallback ownership.
    pub(crate) fn apply(&self, contexts: &mut [Context]) {
        for (rank, &index) in self.indices.iter().enumerate() {
            if let Some(context) = contexts.iter_mut().find(|c| c.root == self.roots[index]) {
                context.driver_rank = Some(rank);
            }
        }
    }
}

fn path_hash(path: &str) -> u32 {
    let mut hash = 0u32;
    for unit in path.encode_utf16() {
        hash = hash.wrapping_add(u32::from(unit));
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
    }
    hash = hash.wrapping_add(hash << 3);
    hash ^= hash >> 11;
    hash = hash.wrapping_add(hash << 15) & 0x3fff_ffff;
    hash.max(1)
}

/// Entry indices in the VM HashMap's iteration order, including collision
/// chains and the reversals introduced when a bucket table grows.
pub(super) fn order(contexts: &[Context]) -> Vec<usize> {
    if contexts.iter().all(|c| c.driver_rank.is_some()) {
        let mut indices: Vec<_> = (0..contexts.len()).collect();
        indices.sort_by_key(|&i| contexts[i].driver_rank);
        return indices;
    }
    let hashes: Vec<_> = contexts
        .iter()
        .map(|c| path_hash(&c.root.to_string_lossy()))
        .collect();
    bucket_order(&hashes, 8).0
}

fn bucket_order(hashes: &[u32], capacity: usize) -> (Vec<usize>, usize) {
    if hashes.is_empty() {
        return (Vec::new(), capacity);
    }
    let mut buckets = vec![Vec::new(); capacity];
    for (index, hash) in hashes.iter().enumerate() {
        let bucket = *hash as usize & (buckets.len() - 1);
        buckets[bucket].insert(0, index);
        if (index + 1) * 4 > buckets.len() * 3 {
            let mut grown = vec![Vec::new(); buckets.len() * 2];
            for chain in buckets {
                for entry in chain {
                    let bucket = hashes[entry] as usize & (grown.len() - 1);
                    grown[bucket].insert(0, entry);
                }
            }
            buckets = grown;
        }
    }
    let capacity = buckets.len();
    (buckets.into_iter().flatten().collect(), capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_root_order_matches_vm_hashset_and_unstable_length_sort() {
        // Captured on the actual Dart 3.6.2 VM. Include the 33/34-element
        // insertion/quicksort boundary, duplicate roots, UTF-16 pairs and
        // large groups with equal and unequal path lengths.
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/data/context_root_order.json")).unwrap();
        assert_eq!(reference["sdk"], "3.6.2");
        for case in reference["cases"].as_array().unwrap() {
            let pattern = case["pattern"].as_str().unwrap();
            let count = case["count"].as_u64().unwrap() as usize;
            let roots: Vec<PathBuf> = (0..count)
                .map(|mut i| {
                    if pattern == "duplicates" {
                        i %= 17;
                    }
                    let nested = if pattern == "same_length" {
                        String::new()
                    } else {
                        "nested/".repeat((i * 7) % 11)
                    };
                    let unicode = if pattern != "same_length" && i % 3 == 0 {
                        "_😀"
                    } else {
                        ""
                    };
                    format!("/roots/{nested}project_{i:03}{unicode}").into()
                })
                .collect();
            let expected: Vec<usize> = case["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(root_order(&roots), expected, "{pattern}, {count}");
        }
    }

    // Captured from Dart 3.6.2 VM String.hashCode and HashMap<String, int>.
    // Forty paths force three resizes, including collisions and UTF-16 pairs.
    fn reference_paths() -> Vec<String> {
        let mut paths = vec![
            "/workspace".into(),
            "/workspace/example".into(),
            "/unicode/😀".into(),
            "/unicode/𐐀".into(),
        ];
        paths.extend((0..36).map(|i| format!("/projects/catalog_{i}")));
        paths
    }

    #[test]
    fn path_hashes_match_the_floor_vm_including_utf16_pairs() {
        let hashes: Vec<_> = reference_paths().iter().map(|p| path_hash(p)).collect();
        assert_eq!(
            hashes,
            [
                871044642, 1067390715, 2272167, 442568899, 633669757, 891332404, 995537823,
                161140776, 540638564, 779786727, 3980650, 243653117, 347170387, 590578520,
                26385479, 64725209, 375408098, 665217134, 213234318, 468996363, 824671089,
                56631268, 468013511, 228767042, 194885899, 1034165528, 268485066, 41559741,
                348539737, 117354442, 892144679, 660271235, 437278190, 937169286, 80489031,
                675475765, 503176363, 196032526, 837223550, 561603483
            ]
        );
    }

    #[test]
    fn driver_order_matches_vm_collisions_and_bucket_growth() {
        let contexts: Vec<_> = reference_paths()
            .into_iter()
            .map(|root| Context {
                root: root.into(),
                ..Context::default()
            })
            .collect();
        assert_eq!(
            order(&contexts),
            [
                23, 31, 3, 33, 34, 14, 22, 29, 26, 19, 24, 37, 18, 12, 25, 13, 28, 15, 39, 6, 0,
                16, 8, 21, 30, 9, 2, 7, 10, 36, 32, 17, 20, 5, 35, 1, 27, 11, 4, 38
            ]
        );
    }

    #[test]
    fn removed_drivers_release_paths_but_preserve_vm_bucket_capacity() {
        // Dart 3.6.2: fresh pair [0, 6], after adding/removing all forty
        // reference paths [6, 0]. Removal never shrinks the VM HashMap.
        let roots: Vec<PathBuf> = reference_paths().into_iter().map(PathBuf::from).collect();
        let pair = vec![roots[0].clone(), roots[6].clone()];
        let mut drivers = DriverOrder::default();
        drivers.rebuild(pair.clone());
        assert_eq!(drivers.indices, [0, 1]);
        drivers.rebuild(roots);
        assert_eq!(drivers.capacity, 64);
        drivers.rebuild(Vec::new());
        assert_eq!(drivers.capacity, 64);
        assert!(drivers.roots.is_empty());
        assert!(drivers.indices.is_empty());
        drivers.rebuild(pair.clone());
        assert_eq!(drivers.indices, [1, 0]);
        assert!(!drivers.update(pair.clone()));
        assert_eq!(drivers.indices, [1, 0]);
        let mut contexts: Vec<_> = pair
            .into_iter()
            .map(|root| Context {
                root,
                ..Context::default()
            })
            .collect();
        drivers.apply(&mut contexts);
        assert_eq!(order(&contexts), [1, 0]);
    }

    #[test]
    fn rebuilding_a_grown_table_changes_collisions_only_once() {
        // These two UTF-16 hashes collide even in the grown table. A resize
        // reverses their chain, while reinsertion into that table prepends it.
        let roots: Vec<PathBuf> = reference_paths().into_iter().map(PathBuf::from).collect();
        let mut drivers = DriverOrder::default();
        drivers.rebuild(roots.clone());
        let initial = drivers.indices.clone();
        assert!(!drivers.update(roots.clone()));
        assert_eq!(drivers.indices, initial);
        drivers.rebuild(roots.clone());
        assert_ne!(drivers.indices, initial);
        let rebuilt = drivers.indices.clone();
        drivers.rebuild(roots);
        assert_eq!(drivers.indices, rebuilt);
    }
}
