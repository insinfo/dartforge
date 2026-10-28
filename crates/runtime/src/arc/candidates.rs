//! Candidate queue lifetime and generation regression tests.

use super::*;

#[derive(Clone, Default)]
struct Graph(HashMap<Ref, Vec<Ref>>);

impl ArcGraph for Graph {
    fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        for &target in self.0.get(&h).into_iter().flatten() {
            f(target);
        }
    }

    fn break_off(&mut self, h: Ref) {
        self.0.remove(&h);
    }
}

fn live(a: &mut ArcHeapState, h: Ref) {
    a.register(h).unwrap();
    a.finish_construction(h);
    a.add_candidate(h);
}

fn backlinks(a: &ArcHeapState) {
    for (index, id) in a.candidates.iter().enumerate() {
        let m = a.meta(id.handle).unwrap();
        assert_eq!(m.generation, id.generation);
        assert!(m.candidate);
        assert_eq!(m.candidate_index as usize, index);
    }
    for (h, m) in a.objects.iter() {
        if m.candidate {
            assert_eq!(
                a.candidates.get(m.candidate_index as usize),
                Some(&ArcId {
                    handle: h,
                    generation: m.generation
                })
            );
        } else {
            assert_eq!(m.candidate_index, u32::MAX);
        }
    }
}

#[test]
fn dead_candidates_swap_remove_and_repair_moved_identity() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    for h in [2, 4, 6, 8] {
        live(&mut a, h);
    }
    a.release(4).unwrap();
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
    assert_eq!(
        a.candidates.iter().map(|id| id.handle).collect::<Vec<_>>(),
        [2, 8, 6]
    );
    backlinks(&a);
    a.release(8).unwrap();
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
    backlinks(&a);
    a.release(6).unwrap();
    a.release(2).unwrap();
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(2));
    assert_eq!(a.candidates(), 0);
    backlinks(&a);
}

#[test]
fn candidate_indices_do_not_truncate_large_live_queues() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    for index in 0..65_540 {
        live(&mut a, 2 * (index + 1));
    }
    backlinks(&a);
    for h in [2, 131_074, 131_080, 131_072] {
        a.release(h).unwrap();
    }
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(4));
    assert_eq!(a.candidates(), 65_536);
    backlinks(&a);
}

#[test]
fn stale_generation_and_wrong_backlink_cannot_remove_current_identity() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    live(&mut a, 2);
    let old = a.candidates[0];
    a.release(2).unwrap();
    a.drain_zeros(&mut g, 1, &|_| false).unwrap();
    a.take_dead();
    live(&mut a, 2);
    live(&mut a, 4);
    a.candidates.insert(0, old);
    // A corrupt backlink points to a different generation: refusal preserves it.
    a.objects.get_mut(&2).unwrap().candidate_index = 0;
    let before = a.candidates.clone();
    a.remove_candidate(
        ArcId {
            handle: 2,
            generation: a.meta(2).unwrap().generation,
        },
        0,
    );
    assert_eq!(a.candidates, before);
    assert!(a.alive(2) && a.alive(4));
    // The cycle snapshot repairs old links before requeueing protected identities.
    assert_eq!(a.collect_cycles(&mut g, &|_| true), Ok(0));
    assert_eq!(a.candidates(), 2);
    backlinks(&a);
}

#[test]
fn snapshot_detaches_backlinks_before_protected_requeue_and_death() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    for h in [2, 4, 6] {
        live(&mut a, h);
    }
    assert_eq!(a.collect_cycles(&mut g, &|h| h != 4), Ok(0));
    assert_eq!(
        a.candidates.iter().map(|id| id.handle).collect::<Vec<_>>(),
        [2, 6]
    );
    backlinks(&a);
    a.release(6).unwrap();
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
    backlinks(&a);
    assert_eq!(a.candidates[0].handle, 2);
    a.add_candidate(4);
    backlinks(&a);
}

#[test]
fn building_and_conditional_protection_keep_existing_lifetime_rules() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    a.register(2).unwrap();
    a.release(2).unwrap();
    a.add_candidate(2);
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(0));
    assert_eq!(a.meta(2).unwrap().state, ArcState::Building);
    live(&mut a, 4);
    a.protect(&g, 4);
    a.release(4).unwrap();
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(0));
    assert_eq!(a.candidates(), 1);
    backlinks(&a);
    a.clear_protections();
    a.review(4);
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(1));
    backlinks(&a);
}

#[test]
fn negative_trial_still_precedes_rc_or_graph_commit() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    live(&mut a, 2);
    g.0.insert(2, vec![2, 2]); // Deliberately invalid multiplicity: RC is only one.
    assert_eq!(
        a.collect_cycles(&mut g, &|_| false),
        Err(ArcError::NegativeTrial(2))
    );
    assert_eq!(a.meta(2).unwrap().rc, 1);
    assert_eq!(a.meta(2).unwrap().state, ArcState::Live);
    assert_eq!(g.0[&2], [2, 2]);
    backlinks(&a);
}

#[test]
fn randomized_duplicate_edge_graphs_match_retained_candidate_reference() {
    let mut random = 0x82f1_6517_u64;
    for _ in 0..64 {
        let (mut a, mut b, mut ga) = (ArcHeapState::new(), ArcHeapState::new(), Graph::default());
        b.retain_dead_candidates = true;
        for h in (2..=64).step_by(2) {
            live(&mut a, h);
            live(&mut b, h);
        }
        for _ in 0..96 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let from = 2 * (1 + ((random >> 32) % 32) as Ref);
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let to = 2 * (1 + ((random >> 32) % 32) as Ref);
            ga.0.entry(from).or_default().push(to);
            a.retain_ref(to).unwrap();
            b.retain_ref(to).unwrap();
        }
        let mut gb = ga.clone();
        for h in (2..=64).step_by(2) {
            a.release(h).unwrap();
            b.release(h).unwrap();
        }
        for protected in [true, false] {
            let root = |h| protected && h % 10 == 0;
            a.resume_deferred();
            b.resume_deferred();
            assert_eq!(
                a.drain_zeros(&mut ga, usize::MAX, &root),
                b.drain_zeros(&mut gb, usize::MAX, &root)
            );
            assert_eq!(
                a.collect_cycles(&mut ga, &root),
                b.collect_cycles(&mut gb, &root)
            );
            assert_eq!(
                a.drain_zeros(&mut ga, usize::MAX, &root),
                b.drain_zeros(&mut gb, usize::MAX, &root)
            );
            a.audit(&ga, &[]).unwrap();
            b.audit(&gb, &[]).unwrap();
            for h in (2..=64).step_by(2) {
                let (ma, mb) = (a.meta(h).unwrap(), b.meta(h).unwrap());
                assert_eq!((ma.rc, ma.state), (mb.rc, mb.state));
            }
            assert_eq!(ga.0, gb.0);
            backlinks(&a);
        }
    }
}

#[test]
fn paged_and_loose_backlinks_survive_reclaimed_page_geometry_changes() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    let first = (1u64 << 32) + 1024;
    let h = first as Ref + crate::layout::HANDLE_OFFSET;
    let area = Geometry {
        first,
        size: 24,
        blocks: 4,
    };
    let old = a.register_live_at(h, Some(area));
    a.retain_ref(h).unwrap();
    a.add_candidate(h);
    live(&mut a, 2);
    a.release(h).unwrap();
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
    a.take_dead();
    backlinks(&a);
    let current = a.register_live_at(
        h,
        Some(Geometry {
            first,
            size: 48,
            blocks: 2,
        }),
    );
    a.retain_ref(h).unwrap();
    a.add_candidate(h);
    a.zeros.push(old);
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(0));
    assert_eq!(a.meta(h).unwrap().generation, current.generation);
    backlinks(&a);
    a.release(2).unwrap();
    assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
    backlinks(&a);
    assert_eq!(a.candidates[0], current);
}

#[test]
fn randomized_invalid_multiplicities_abort_before_commit() {
    let mut random = 17u64;
    for _ in 0..64 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let multiplicity = 2 + (random % 9) as usize;
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        live(&mut a, 2);
        g.0.insert(2, vec![2; multiplicity]);
        assert_eq!(
            a.collect_cycles(&mut g, &|_| false),
            Err(ArcError::NegativeTrial(2))
        );
        assert_eq!(
            (a.meta(2).unwrap().rc, a.meta(2).unwrap().state),
            (1, ArcState::Live)
        );
        assert_eq!(g.0[&2].len(), multiplicity);
        backlinks(&a);
    }
}

#[test]
fn unqueued_deaths_leave_live_candidate_backlinks_untouched() {
    let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
    live(&mut a, 2);
    live(&mut a, 4);
    let queued = a.candidates.clone();
    // Construction owners with no positive-RC decrement never enter the queue.
    for h in (6..=128).step_by(2) {
        a.register(h).unwrap();
        a.finish_construction(h);
        a.release(h).unwrap();
    }
    assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(62));
    assert_eq!(a.candidates, queued);
    backlinks(&a);
    for h in (6..=128).step_by(2) {
        assert_eq!(a.meta(h).unwrap().state, ArcState::Dead);
        assert_eq!(a.meta(h).unwrap().candidate_index, u32::MAX);
    }
    a.audit(&g, &[2, 4]).unwrap();
}
