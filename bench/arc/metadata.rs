//! Isolated measurement of the real ARC module's metadata, without a heap or
//! collection.
//!
//! Eight pages with 1,024 blocks each; ten million retain/release pairs and
//! queries per round, in permuted order. The RC stays at one: the zero queue
//! does not grow and each object becomes a candidate only once.
//! Compile with `rustc --edition=2024 -O bench/arc/metadata.rs -o <exe>`.
//! Pin the affinity of the measuring process and alternate before/after
//! versions; discard the first round. It does not represent the total cost of
//! ARC in Dart.
#![allow(dead_code)]

#[path = "../../crates/runtime/src/arc.rs"]
mod arc;
#[path = "../../crates/runtime/src/hash.rs"]
mod hash;
#[path = "../../crates/runtime/src/layout.rs"]
mod layout;

use arc::{ArcHeapState, Geometry};
use std::{hint::black_box, time::Instant};

#[inline(never)]
fn measure(arc: &mut ArcHeapState, handles: &[i64]) -> u64 {
    let mut sum = 0;
    for i in 0..10_000_000usize {
        let h = black_box(handles[i.wrapping_mul(4099) & (handles.len() - 1)]);
        arc.retain_ref(h).unwrap();
        arc.release(h).unwrap();
        sum += black_box(arc.meta(h).unwrap().rc);
    }
    sum
}

fn main() {
    let mut arc = ArcHeapState::new();
    let mut handles = Vec::new();
    for p in 0..8 {
        let g = Geometry {
            first: (1u64 << 32) + p * 65536 + 1024,
            size: 48,
            blocks: 1024,
        };
        for i in 0..g.blocks {
            let h = (g.first + u64::from(i) * g.size) as i64 + layout::HANDLE_OFFSET;
            arc.register_live_at(h, Some(g));
            arc.retain_ref(h).unwrap();
            handles.push(h);
        }
    }
    for _ in 0..6 {
        let t = Instant::now();
        let sum = measure(&mut arc, black_box(&handles));
        assert_eq!(sum, 10_000_000);
        println!("{} {sum}", t.elapsed().as_micros());
    }
}
