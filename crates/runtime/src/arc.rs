//! The ARC core with cycle collection (docs/ARC-CICLOS-ESPECIFICACAO.md §19, §22):
//! the per-object side metadata, the `retain_ref`/`release` of a strong
//! occurrence, the zero queue with cascading discard and cycle collection by side
//! *trial deletion*, without changing the real RC (§22.1).
//!
//! The module does not know the block layout: each object's edges come from an
//! [`crate::arc::ArcGraph`] (in the heap, the enumeration by the body's shape; in
//! the tests, a fake graph). No operation here allocates a Dart object, throws a
//! Dart exception or calls Dart code (§19.3).
//!
//! Contracts:
//! * the counter is per strong **occurrence** (§18.1, decision 2): two positions of
//!   a list pointing to `x` count twice;
//! * `null` (0) and a `Smi` (odd) are not managed; a handle without metadata (an
//!   image object, permanent) is treated as immortal;
//! * the generation is monotonic per registration; the queues keep
//!   [`crate::arc::ArcId`] and discard an entry whose generation does not match
//!   (§19.1);
//! * cycle collection runs with the mutator stopped and without reentry (§18.1,
//!   decision 5): the caller guarantees that.

use crate::hash::{HashMap, HashSet};

/// A value in a reference position (the `Ref` of `layout`).
pub type Ref = i64;

/// The logical life cycle of an object (§19.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcState {
    /// Allocated, fields still being initialized: it does not die by a zero RC.
    Building,
    Live,
    /// Inside the discard transaction of a set.
    Collecting,
    /// Logically dead: waits for the physical reclaimer.
    Dead,
}

/// The side metadata of a managed object (§19.1).
#[derive(Clone, Copy, Debug)]
pub struct MetaArc {
    pub generation: u64,
    /// Owners plus strong occurrences; does not include weak ones.
    pub rc: u64,
    pub state: ArcState,
    /// Is (or was) in the queue of cycle candidates since the last round.
    pub candidate: bool,
    /// Index in the current candidate queue, or `usize::MAX` when absent.
    candidate_index: usize,
    /// An ephemeron value (or its closure) protected between rounds (§22.2): a zero
    /// RC does not start the discard.
    pub conditionally_protected: bool,
    pub immortal: bool,
    /// Is on the deferred list (zero RC with a root), only once.
    pub deferred: bool,
    /// The body is not enough to prove the absence of edges in this generation.
    pub side_edges: bool,
}

/// The identity of an object in the queues: the handle and the registration's generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArcId {
    pub handle: Ref,
    pub generation: u64,
}

/// The strong edges of the objects and breaking them at death (§19.2).
pub trait ArcGraph {
    /// Each strong occurrence of object `h`, with multiplicity.
    fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref));
    /// Clears the strong occurrences of `h` (the discard already debited the targets).
    fn break_off(&mut self, h: Ref);
    /// [`Self::edges`] and [`Self::break_off`] in a single walk: each strong
    /// occurrence of `h`, which leaves cleared (the discard of a single object, the
    /// cascade).
    fn take_edges(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        self.edges(h, f);
        self.break_off(h);
    }
}

/// An internal counter violation (not a Dart error): a fatal diagnostic (§19.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArcError {
    /// `release` with the RC already zero.
    RcZero(Ref),
    /// An increment beyond `u64::MAX`.
    Overflow(Ref),
    /// An operation on a `Collecting`/`Dead` object.
    InvalidState(Ref, ArcState),
    /// Registration of a handle that is still alive.
    AlreadyRegistered(Ref),
    /// Trial deletion found more internal edges than the RC (wrong inventory or
    /// count): the collection is aborted without changing anything (§22.1).
    NegativeTrial(Ref),
    /// The audit recounted a different RC.
    Audit { handle: Ref, rc: u64, recounted: u64 },
}

/// Work counters (§22.5, §26.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArcStats {
    pub registered_set: u64,
    pub retains: u64,
    pub releases: u64,
    /// Dead by a zero RC (acyclic cascade).
    pub dead_by_rc: u64,
    /// Dead by cycle collection.
    pub dead_by_cycle: u64,
    pub cycle_rounds: u64,
    /// Objects in region `R` over all rounds.
    pub examined: u64,
    /// Entries taken from the zero queue and, of those, the deferred ones (zero RC
    /// with a root): the diagnostic of the cascade's cost.
    pub seen_zeros: u64,
    pub deferred_zeros: u64,
}

/// `null` and `Smi` are not managed references.
#[inline]
pub fn e_handle(h: Ref) -> bool {
    h != 0 && h & 1 == 0
}

/// The page geometry of a space block, which the heap reports at registration: the
/// address of the first block, the bytes of each block and how many fit in the
/// page. The size is positive and `size * blocks` fits in u64: the extent describes
/// contiguous memory, without wrapping around the address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub first: u64,
    pub size: u64,
    pub blocks: u32,
}

/// The metadata of a page's blocks, by block index.
struct ArcPage {
    geometry: Geometry,
    /// The inverse of the odd part of the size, modulo 2⁶⁴, and the removed power of
    /// two: exact division becomes multiplication and rotation.
    inverse: u64,
    offset: u32,
    metas: Vec<Option<MetaArc>>,
    occupied: u32,
}

impl ArcPage {
    /// Prepares the exact division by the block size once. The extent must fit in
    /// u64: thanks to that, an index smaller than `blocks` cannot represent a product
    /// that wrapped around the address space.
    fn factors(g: Geometry) -> (u64, u32) {
        assert!(g.size != 0 && g.size.checked_mul(u64::from(g.blocks)).is_some(), "ARC bug: invalid geometry");
        let offset = g.size.trailing_zeros();
        let odd = g.size >> offset;
        let mut inverse = odd;
        // Newton: each step doubles the correct bits of the odd inverse.
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(odd.wrapping_mul(inverse)));
        }
        (inverse, offset)
    }

    fn new_area(g: Geometry) -> Self {
        let (inverse, offset) = Self::factors(g);
        Self { geometry: g, inverse, offset, metas: (0..g.blocks).map(|_| None).collect(), occupied: 0 }
    }

    /// The index of a block start; the rotation moves the bits that are not
    /// divisible by the power of two to the top, outside `blocks`. The product by
    /// the inverse likewise rejects the inexact multiples of the odd part. There is
    /// no division or remainder on the query path.
    #[inline]
    fn ix(&self, h: Ref) -> Option<usize> {
        let d = (h.wrapping_sub(crate::layout::HANDLE_OFFSET) as u64).checked_sub(self.geometry.first)?;
        let i = d.wrapping_mul(self.inverse).rotate_right(self.offset);
        (i < u64::from(self.geometry.blocks)).then_some(i as usize)
    }
}

/// The table of metadata by handle (§19.1: "measure later a table per page/block
/// index, avoiding a hash per retain"). A space block's handle finds the page by its
/// number (`h >> 16`), in a radix map of 2¹⁶ positions per 4 GiB region — like the
/// space's page map —, and the metadata by the block index: no hash. A handle
/// registered without a geometry (the core tests, the image objects) goes into a
/// fallback hash table.
#[derive(Default)]
struct Table {
    regions: Vec<(u64, Box<[u32]>)>,
    pages: Vec<ArcPage>,
    loose: HashMap<Ref, MetaArc>,
    n: usize,
}

/// `PAGE` = 64 KiB; a radix map region covers 2¹⁶ pages.
const PAGE_BITS: u32 = 16;
const REGION_BITS: u32 = 16;

// A private location, valid only while no objects are registered/reclaimed. Draining
// a zero changes counters/queues, not the page geometry or the identity of the
// slots. Do not keep this value across drains.
#[derive(Clone, Copy)]
enum MetadataLocation {
    Page { page: usize, ix: usize },
    Loose(Ref),
}

impl Table {
    #[inline]
    fn key(h: Ref) -> (u64, usize) {
        let k = (h as u64) >> PAGE_BITS;
        (k >> REGION_BITS, (k & ((1 << REGION_BITS) - 1)) as usize)
    }

    /// The registered page of `h`, if there is one.
    #[inline]
    fn page(&self, h: Ref) -> Option<usize> {
        let (r, i) = Self::key(h);
        let (_, t) = self.regions.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }

    /// (page, index) of the metadata of `h` in a registered page.
    #[inline]
    fn place(&self, h: Ref) -> Option<(usize, usize)> {
        let p = self.page(h)?;
        Some((p, self.pages[p].ix(h)?))
    }

    fn locate(&self, h: Ref) -> MetadataLocation {
        match self.place(h) {
            Some((page, ix)) => MetadataLocation::Page { page, ix },
            None => MetadataLocation::Loose(h),
        }
    }

    fn in_local(&self, local: MetadataLocation) -> Option<&MetaArc> {
        match local {
            MetadataLocation::Page { page, ix } => self.pages[page].metas[ix].as_ref(),
            MetadataLocation::Loose(h) => self.loose.get(&h),
        }
    }

    fn in_local_mut(&mut self, local: MetadataLocation) -> Option<&mut MetaArc> {
        match local {
            MetadataLocation::Page { page, ix } => self.pages[page].metas[ix].as_mut(),
            MetadataLocation::Loose(h) => self.loose.get_mut(&h),
        }
    }

    fn get(&self, h: &Ref) -> Option<&MetaArc> {
        match self.place(*h) {
            Some((p, i)) => self.pages[p].metas[i].as_ref(),
            None if self.loose.is_empty() => None,
            None => self.loose.get(h),
        }
    }

    fn get_mut(&mut self, h: &Ref) -> Option<&mut MetaArc> {
        match self.place(*h) {
            Some((p, i)) => self.pages[p].metas[i].as_mut(),
            None if self.loose.is_empty() => None,
            None => self.loose.get_mut(h),
        }
    }

    fn contains_key(&self, h: &Ref) -> bool {
        self.get(h).is_some()
    }

    fn len(&self) -> usize {
        self.n
    }

    /// Puts the metadata of `h`: through the page, with its geometry, or in the
    /// fallback, without one. A page the space reformatted for another class (a reused
    /// empty page) arrives with another geometry and cannot have occupied metadata:
    /// every dead one has already left the table.
    fn insert(&mut self, h: Ref, m: MetaArc, geometry: Option<Geometry>) {
        let Some(g) = geometry else {
            if self.loose.insert(h, m).is_none() {
                self.n += 1;
            }
            return;
        };
        let p = match self.page(h) {
            Some(p) => p,
            None => self.new_page(h, g),
        };
        let pg = &mut self.pages[p];
        if pg.geometry != g {
            assert!(pg.occupied == 0, "ARC bug: page reformatted with {} live metadata entries", pg.occupied);
            pg.geometry = g;
            (pg.inverse, pg.offset) = ArcPage::factors(g);
            pg.metas.clear();
            pg.metas.resize_with(g.blocks as usize, || None);
        }
        let i = pg.ix(h).expect("ARC bug: handle outside the page geometry");
        if pg.metas[i].replace(m).is_none() {
            pg.occupied += 1;
            self.n += 1;
        }
    }

    fn new_page(&mut self, h: Ref, g: Geometry) -> usize {
        let (r, i) = Self::key(h);
        let k = match self.regions.iter().position(|(x, _)| *x == r) {
            Some(k) => k,
            None => {
                self.regions.push((r, vec![0u32; 1 << REGION_BITS].into_boxed_slice()));
                self.regions.len() - 1
            }
        };
        self.pages.push(ArcPage::new_area(g));
        let p = self.pages.len() - 1;
        self.regions[k].1[i] = u32::try_from(p + 1).expect("ARC: too many pages");
        p
    }

    fn remove(&mut self, h: &Ref) -> Option<MetaArc> {
        let m = match self.place(*h) {
            Some((p, i)) => {
                let m = self.pages[p].metas[i].take();
                if m.is_some() {
                    self.pages[p].occupied -= 1;
                }
                m
            }
            None => self.loose.remove(h),
        };
        if m.is_some() {
            self.n -= 1;
        }
        m
    }

    /// Each registered handle with its metadata.
    fn iter(&self) -> impl Iterator<Item = (Ref, &MetaArc)> + '_ {
        self.pages
            .iter()
            .flat_map(|pg| {
                let g = pg.geometry;
                pg.metas.iter().enumerate().filter_map(move |(i, m)| {
                    m.as_ref().map(|m| ((g.first + i as u64 * g.size) as Ref + crate::layout::HANDLE_OFFSET, m))
                })
            })
            .chain(self.loose.iter().map(|(&h, m)| (h, m)))
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut MetaArc> + '_ {
        self.pages.iter_mut().flat_map(|pg| pg.metas.iter_mut().flatten()).chain(self.loose.values_mut())
    }
}

impl std::ops::Index<&Ref> for Table {
    type Output = MetaArc;
    fn index(&self, h: &Ref) -> &MetaArc {
        self.get(h).expect("ARC bug: handle without metadata")
    }
}

/// The ARC state of an isolate (§19.1).
#[derive(Default)]
pub struct ArcHeapState {
    objects: Table,
    candidates: Vec<ArcId>,
    /// Diagnostic comparison mode: retain dead queue entries until the cycle round.
    pub(crate) retain_dead_candidates: bool,
    zeros: Vec<ArcId>,
    /// The zero-RC objects that a root held at the last drain of the zeros, one entry
    /// per object ([`MetaArc::deferred`]); they go back to the queue once per
    /// collection ([`Self::resume_deferred`]), when the roots may have changed.
    deferred_set: Vec<ArcId>,
    dead_set: Vec<Ref>,
    next_generation: u64,
    /// A discard transaction is in progress (do not reenter).
    transaction: bool,
    /// The exits of the set being discarded, reused ([`Self::discard`]).
    exits: Vec<Ref>,
    pub statistics: ArcStats,
    /// One line on stderr per event (registration, retain, release, death): the
    /// diagnostic of `DARTFORGE_ARC_TRACE=1`.
    pub trace_log: bool,
    /// Does the object have no reference position at all (in the heap, the `RAW`
    /// body: string, numeric box, typed data)? Without an outgoing edge it is never
    /// in a cycle, and it does not enter the candidates (the minimal filter of §32).
    /// `None` (the core tests): any object may.
    pub without_edges: Option<fn(Ref) -> bool>,
}

impl ArcHeapState {
    pub fn new() -> Self {
        ArcHeapState { next_generation: 1, ..Default::default() }
    }

    /// The metadata of `h`, if managed.
    pub fn meta(&self, h: Ref) -> Option<&MetaArc> {
        self.objects.get(&h)
    }

    /// How many objects have metadata (alive, under construction or dead and not yet
    /// reclaimed).
    pub fn registered_set(&self) -> usize {
        self.objects.len()
    }

    /// Registers the freshly reserved block `h` (§19.3): `rc=1` (the construction
    /// owner), state `Building`, a new generation. Fails before the handle is
    /// published.
    pub fn register(&mut self, h: Ref) -> Result<ArcId, ArcError> {
        debug_assert!(e_handle(h));
        if let Some(m) = self.objects.get(&h)
            && m.state != ArcState::Dead
        {
            return Err(ArcError::AlreadyRegistered(h));
        }
        let generation = self.next_generation;
        // Monotonic generation; overflow is an internal failure, never reuse (§19.1).
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: generation exhausted");
        self.objects.insert(
            h,
            MetaArc { generation, rc: 1, state: ArcState::Building, candidate: false, candidate_index: usize::MAX, conditionally_protected: false, immortal: false, deferred: false, side_edges: false },
            None,
        );
        self.statistics.registered_set += 1;
        Ok(ArcId { handle: h, generation })
    }

    /// Registers an image object provably alive for the whole image (§19.1): it
    /// neither dies nor enters queues; its edges are still counted.
    ///
    /// # Panics
    /// The monotonic generation was exhausted; no registration is published then.
    ///
    /// ```
    /// use dartforge_runtime::arc::ArcHeapState;
    /// let mut arc = ArcHeapState::new();
    /// arc.register_immortal(2);
    /// assert!(arc.meta(2).unwrap().immortal);
    /// ```
    pub fn register_immortal(&mut self, h: Ref) {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: generation exhausted");
        self.objects.insert(h, MetaArc { generation, rc: 0, state: ArcState::Live, candidate: false, candidate_index: usize::MAX, conditionally_protected: false, immortal: true, deferred: false, side_edges: false }, None);
    }

    /// Registers an object that already exists and starts being counted (the young
    /// object promoted by the minor collection, the live ones at activation):
    /// `rc=0`, `Live`; the caller counts the occurrences that reach it and then
    /// calls [`ArcHeapState::review`].
    pub fn register_live(&mut self, h: Ref) -> ArcId {
        self.register_live_at(h, None)
    }

    /// [`Self::register_live`] of a space block, with its page's geometry: the
    /// metadata goes to the per-page table, without a hash.
    ///
    /// ```
    /// use dartforge_runtime::arc::{ArcHeapState, Geometry};
    /// let mut arc = ArcHeapState::new();
    /// let first = 0x1_0000_0400;
    /// let h = first as i64 + dartforge_runtime::layout::HANDLE_OFFSET;
    /// arc.register_live_at(h, Some(Geometry { first, size: 24, blocks: 2 }));
    /// assert_eq!(arc.meta(h).unwrap().rc, 0);
    /// ```
    ///
    /// # Panics
    /// The geometry has size zero or an extent that overflows u64; `h` does not start
    /// one of its blocks; the page changed geometry with live objects; or the
    /// monotonic generation was exhausted.
    pub fn register_live_at(&mut self, h: Ref, geometry: Option<Geometry>) -> ArcId {
        debug_assert!(e_handle(h));
        let generation = self.next_generation;
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: generation exhausted");
        if self.trace_log {
            eprintln!("[arc-trace] register {h} already={}", self.objects.contains_key(&h));
        }
        self.objects.insert(h, MetaArc { generation, rc: 0, state: ArcState::Live, candidate: false, candidate_index: usize::MAX, conditionally_protected: false, immortal: false, deferred: false, side_edges: false }, geometry);
        self.statistics.registered_set += 1;
        ArcId { handle: h, generation }
    }

    /// Puts `h` in the zero queue if its RC is zero (one registered without
    /// occurrences: only roots see it, or nobody).
    pub fn review(&mut self, h: Ref) {
        if let Some(m) = self.objects.get(&h)
            && m.rc == 0
            && m.state == ArcState::Live
            && !m.immortal
        {
            self.zeros.push(ArcId { handle: h, generation: m.generation });
        }
    }

    /// Object `h` is counted and alive (neither dead nor being discarded).
    pub fn alive(&self, h: Ref) -> bool {
        self.objects.get(&h).is_some_and(|m| matches!(m.state, ArcState::Live | ArcState::Building))
    }

    /// Object `h` has died (logically) and waits for the reclaimer.
    pub fn dead(&self, h: Ref) -> bool {
        self.objects.get(&h).is_some_and(|m| m.state == ArcState::Dead)
    }

    /// The counted handles still alive.
    pub fn live_set(&self) -> impl Iterator<Item = Ref> + '_ {
        self.objects.iter().filter(|(_, m)| matches!(m.state, ArcState::Live | ArcState::Building)).map(|(h, _)| h)
    }

    /// Puts the live `h` among the candidates of the next cycle round: an object that
    /// an observational root sees may lose that root without a decrement (roots do
    /// not count), and only the round examines it again.
    pub fn add_candidate(&mut self, h: Ref) {
        let without_edges = self.without_edges;
        if let Some(m) = self.objects.get_mut(&h)
            && !m.candidate
            && !m.immortal
            && m.state == ArcState::Live
            && (m.side_edges || !without_edges.is_some_and(|f| f(h)))
        {
            m.candidate = true;
            m.candidate_index = self.candidates.len();
            self.candidates.push(ArcId { handle: h, generation: m.generation });
        }
    }

    /// Removes only the queued identity belonging to this registration. A moved
    /// entry gets its new backlink before any further discard or requeue.
    fn remove_candidate(&mut self, h: Ref, local: Option<MetadataLocation>) {
        if self.retain_dead_candidates {
            return;
        }
        let m = match local {
            Some(local) => self.objects.in_local_mut(local),
            None => self.objects.get_mut(&h),
        };
        let Some(m) = m else { return };
        let index = std::mem::replace(&mut m.candidate_index, usize::MAX);
        let id = ArcId { handle: h, generation: m.generation };
        // Never let a stale generation or backlink remove another identity.
        if self.candidates.get(index) != Some(&id) {
            return;
        }
        self.candidates.swap_remove(index);
        if let Some(moved) = self.candidates.get(index).copied()
            && let Some(m) = self.objects.get_mut(&moved.handle)
            && m.generation == moved.generation
        {
            m.candidate_index = index;
        }
    }

    /// Prevents the body filter from excluding an object with side edges. The hint
    /// is conservative until the identity's next registration.
    pub(crate) fn mark_side_edges(&mut self, h: Ref) {
        if let Some(m) = self.objects.get_mut(&h) { m.side_edges = true; }
        self.add_candidate(h);
    }

    /// The young `h` survived the drain (it has metadata): the zero RC goes to the
    /// zero queue ([`Self::review`]) and, unless immortal, it becomes a cycle
    /// candidate ([`Self::add_candidate`]) — in a single lookup. Returns whether `h`
    /// had metadata.
    pub fn decide_young(&mut self, h: Ref) -> bool {
        let without_edges = self.without_edges;
        let Some(m) = self.objects.get_mut(&h) else { return false };
        if m.immortal || m.state != ArcState::Live {
            return true;
        }
        let id = ArcId { handle: h, generation: m.generation };
        if m.rc == 0 {
            self.zeros.push(id);
        }
        if !m.candidate && (m.side_edges || !without_edges.is_some_and(|f| f(h))) {
            m.candidate = true;
            m.candidate_index = self.candidates.len();
            self.candidates.push(id);
        }
        true
    }

    /// How many cycle candidates wait for the next round.
    pub fn candidates(&self) -> usize {
        self.candidates.len()
    }

    /// Construction finished: the construction owner becomes the result.
    pub fn finish_construction(&mut self, h: Ref) {
        if let Some(m) = self.objects.get_mut(&h)
            && m.state == ArcState::Building
        {
            m.state = ArcState::Live;
        }
    }

    /// One more strong occurrence of `h` (§19.3).
    pub fn retain_ref(&mut self, h: Ref) -> Result<(), ArcError> {
        self.retain_if_registered(h).map(|_| ())
    }

    // Retains in a single lookup, reporting whether the handle was already resolved.
    // false is only the absence of metadata, never an invalid/immortal state; the
    // heap may register the absent target before counting the occurrence.
    pub(crate) fn retain_if_registered(&mut self, h: Ref) -> Result<bool, ArcError> {
        if !e_handle(h) {
            return Ok(true);
        }
        let Some(m) = self.objects.get_mut(&h) else { return Ok(false) };
        if m.immortal {
            return Ok(true);
        }
        if matches!(m.state, ArcState::Collecting | ArcState::Dead) {
            return Err(ArcError::InvalidState(h, m.state));
        }
        m.rc = m.rc.checked_add(1).ok_or(ArcError::Overflow(h))?;
        self.statistics.retains += 1;
        if self.trace_log {
            eprintln!("[arc-trace] retain {h} rc={}", m.rc);
        }
        Ok(true)
    }

    /// One strong occurrence of `h` fewer (§19.3): zero goes to the zero queue;
    /// positive becomes a cycle candidate.
    pub fn release(&mut self, h: Ref) -> Result<(), ArcError> {
        if !e_handle(h) {
            return Ok(());
        }
        let Some(m) = self.objects.get_mut(&h) else { return Ok(()) };
        if m.immortal {
            return Ok(());
        }
        if matches!(m.state, ArcState::Collecting | ArcState::Dead) {
            return Err(ArcError::InvalidState(h, m.state));
        }
        if m.rc == 0 {
            return Err(ArcError::RcZero(h));
        }
        m.rc -= 1;
        self.statistics.releases += 1;
        if self.trace_log {
            eprintln!("[arc-trace] release {h} rc={}", m.rc);
        }
        let id = ArcId { handle: h, generation: m.generation };
        if m.rc == 0 {
            self.zeros.push(id);
        } else if !m.candidate && (m.side_edges || !self.without_edges.is_some_and(|f| f(h))) {
            m.candidate = true;
            m.candidate_index = self.candidates.len();
            self.candidates.push(id);
        }
        Ok(())
    }

    /// Marks the strong closure of `h` as conditionally protected (§22.2): the zero
    /// RC of a protected object does not start the discard.
    pub fn protect(&mut self, graph: &dyn ArcGraph, h: Ref) {
        let mut stack = vec![h];
        while let Some(x) = stack.pop() {
            let Some(m) = self.objects.get_mut(&x) else { continue };
            if m.conditionally_protected || m.immortal {
                continue;
            }
            m.conditionally_protected = true;
            graph.edges(x, &mut |v| {
                if e_handle(v) {
                    stack.push(v);
                }
            });
        }
    }

    /// Removes all conditional protection (the round's fixpoint redoes it).
    pub fn clear_protections(&mut self) {
        for m in self.objects.values_mut() {
            m.conditionally_protected = false;
        }
    }

    /// Processes the zero queue (§19.3): each live object, of the entry's
    /// generation, with an RC still zero and without protection dies; the targets of
    /// its edges lose one occurrence, which may chain more zeros (an iterative queue,
    /// without recursion). `limit` counts discarded objects.
    ///
    /// `protected` are the observational roots (§21.3): the occurrences the RC does
    /// not count (the stack, the runtime's frames, the globals). A zero that they see
    /// does not die and stays in the queue for the next drain (the zero-count table
    /// of deferred RC).
    pub fn drain_zeros(&mut self, graph: &mut dyn ArcGraph, limit: usize, protected: &dyn Fn(Ref) -> bool) -> Result<usize, ArcError> {
        let mut done = 0;
        while done < limit {
            let Some(id) = self.zeros.pop() else { break };
            self.statistics.seen_zeros += 1;
            let local = self.objects.locate(id.handle);
            let Some(m) = self.objects.in_local(local).filter(|m| m.generation == id.generation) else { continue };
            if m.state == ArcState::Live && m.rc > 0 {
                // It went down to zero and back up before the drain: the new
                // occurrence may close a rootless cycle (the self-cycle `x[i] = x`
                // after releasing the last outside one), so it is a candidate like
                // one that goes down to a positive RC.
                self.add_candidate(id.handle);
                continue;
            }
            if m.state != ArcState::Live || m.rc != 0 || m.conditionally_protected || m.immortal {
                continue;
            }
            if protected(id.handle) {
                // It stays out of the queue until the next collection: within this
                // one the roots do not change, and reexamining it on every pass (and
                // each copy of it, each time the RC went back to zero) was the
                // largest cost of the cascade.
                self.statistics.deferred_zeros += 1;
                if let Some(m) = self.objects.in_local_mut(local)
                    && !m.deferred
                {
                    m.deferred = true;
                    self.deferred_set.push(id);
                }
                continue;
            }
            self.discard_located(graph, &[id.handle], Some(local))?;
            self.statistics.dead_by_rc += 1;
            done += 1;
        }
        Ok(done)
    }

    /// The deferred ones go back to the zero queue, once per collection (at its
    /// start: the root that held them may have disappeared). Whatever is no longer
    /// valid, or no longer has a zero RC, leaves.
    pub fn resume_deferred(&mut self) {
        let deferred_set = std::mem::take(&mut self.deferred_set);
        for id in deferred_set {
            if let Some(m) = self.objects.get_mut(&id.handle)
                && m.generation == id.generation
            {
                m.deferred = false;
                self.zeros.push(id);
            }
        }
    }

    /// There are zeros waiting.
    pub fn ha_zeros(&self) -> bool {
        !self.zeros.is_empty()
    }

    /// The discard transaction of set `d` (§19.4): marks them all `Collecting`; for
    /// each strong occurrence `u→v` with `u` in `d`, releases `v` if `v` is not in
    /// `d`; breaks the edges; marks them `Dead` and publishes the dead ones to the
    /// reclaimer.
    pub fn discard(&mut self, graph: &mut dyn ArcGraph, d: &[Ref]) -> Result<(), ArcError> {
        self.discard_located(graph, d, None)
    }

    // The slot already validated by the generation stays valid during this
    // transaction: releasing exits neither registers nor removes metadata. The
    // collective discard still resolves each handle, without reusing another
    // object's location.
    fn discard_located(&mut self, graph: &mut dyn ArcGraph, d: &[Ref], local: Option<MetadataLocation>) -> Result<(), ArcError> {
        debug_assert!(local.is_none() || d.len() == 1);
        assert!(!self.transaction, "ARC: reentrant discard");
        self.transaction = true;
        // The zero cascade discards one object at a time: there the only internal
        // edge is the object's own, the set (one allocation per death) is not
        // built, and the edges are read and cleared in a single walk.
        let single = if d.len() == 1 { Some(d[0]) } else { None };
        let item_set: HashSet<Ref> = if single.is_some() { HashSet::default() } else { d.iter().copied().collect() };
        let internal = |v: Ref| match single {
            Some(u) => u == v,
            None => item_set.contains(&v),
        };
        for &u in d {
            self.remove_candidate(u, local);
            let m = match local {
                Some(local) => self.objects.in_local_mut(local),
                None => self.objects.get_mut(&u),
            };
            if let Some(m) = m {
                m.state = ArcState::Collecting;
            }
        }
        // The set's exits, once per occurrence (the vector is reused between
        // discards).
        let mut exits = std::mem::take(&mut self.exits);
        if let Some(u) = single {
            graph.take_edges(u, &mut |v| {
                if e_handle(v) && v != u {
                    exits.push(v);
                }
            });
        } else {
            for &u in d {
                graph.edges(u, &mut |v| {
                    if e_handle(v) && !internal(v) {
                        exits.push(v);
                    }
                });
            }
        }
        let mut error = None;
        for v in exits.drain(..) {
            if let Err(e) = self.release(v) {
                error.get_or_insert(e);
            }
        }
        self.exits = exits;
        for &u in d {
            if self.trace_log {
                eprintln!("[arc-trace] die {u}");
            }
            if single.is_none() {
                graph.break_off(u);
            }
            let m = match local {
                Some(local) => self.objects.in_local_mut(local),
                None => self.objects.get_mut(&u),
            };
            if let Some(m) = m {
                m.state = ArcState::Dead;
                m.rc = 0;
                m.candidate = false;
            }
            self.dead_set.push(u);
        }
        self.transaction = false;
        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Cycle collection by side *trial deletion* (§22.1): region `R` is the strong
    /// closure of the valid candidates; `trial[v] = rc[v]` minus the occurrences
    /// internal to `R`; those with a positive trial (and the immortal ones, those
    /// under construction or protected) survive and propagate survival; the rest is a
    /// dead set, discarded in a batch. No real RC changes before the commit; a
    /// negative trial aborts with a diagnostic. `protected`: the observational roots,
    /// which make objects survive like an external reference.
    pub fn collect_cycles(&mut self, graph: &mut dyn ArcGraph, protected: &dyn Fn(Ref) -> bool) -> Result<usize, ArcError> {
        let candidates = std::mem::take(&mut self.candidates);
        // Detach every old backlink before callbacks can requeue a protected
        // identity into the new vector. Old indices must never name that vector.
        for id in &candidates {
            if let Some(m) = self.objects.get_mut(&id.handle).filter(|m| m.generation == id.generation) {
                m.candidate = false;
                m.candidate_index = usize::MAX;
            }
        }
        let mut seeds: Vec<Ref> = Vec::new();
        for id in candidates {
            let Some(m) = self.objects.get(&id.handle).filter(|m| m.generation == id.generation) else { continue };
            if m.state == ArcState::Live && m.rc > 0 && !m.immortal {
                if protected(id.handle) {
                    self.add_candidate(id.handle);
                } else {
                    seeds.push(id.handle);
                }
            }
        }
        if seeds.is_empty() {
            return Ok(0);
        }
        self.statistics.cycle_rounds += 1;
        // The protected root stays out of R: its edges remain external inputs in the
        // trial. Requeuing it allows collecting it when the root disappears.
        let mut r: Vec<Ref> = Vec::new();
        // A dense index per handle; counters and marks live in vectors, without three
        // distinct hash tables for the same region.
        let mut in_r: HashMap<Ref, usize> = HashMap::default();
        let mut stack = seeds;
        while let Some(x) = stack.pop() {
            let std::collections::hash_map::Entry::Vacant(input) = in_r.entry(x) else { continue };
            let Some(m) = self.objects.get(&x) else { continue };
            if m.immortal || m.state == ArcState::Dead {
                continue;
            }
            if protected(x) {
                self.add_candidate(x);
                continue;
            }
            input.insert(r.len());
            r.push(x);
            graph.edges(x, &mut |v| {
                if e_handle(v) {
                    stack.push(v);
                }
            });
        }
        self.statistics.examined += r.len() as u64;
        // trial = rc − internal occurrences.
        let mut trial: Vec<i128> = r.iter().map(|x| self.objects[x].rc as i128).collect();
        for &u in &r {
            graph.edges(u, &mut |v| {
                if let Some(&ix) = in_r.get(&v) {
                    trial[ix] -= 1;
                }
            });
        }
        if let Some(ix) = trial.iter().position(|&t| t < 0) {
            return Err(ArcError::NegativeTrial(r[ix]));
        }
        // Survivors: positive trial, under construction or protected; they propagate.
        let mut live_set = vec![false; r.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (ix, &x) in r.iter().enumerate() {
            let m = &self.objects[&x];
            if trial[ix] > 0 || m.state == ArcState::Building || m.conditionally_protected || protected(x) {
                live_set[ix] = true;
                stack.push(ix);
            }
        }
        // Marks when enqueuing: converging paths and repeated positions do not
        // multiply stack entries. The trial above still counts each occurrence.
        // Propagation uses indices, without looking the handle up again when popping
        // from the stack; targets outside R are not walked.
        while let Some(ix) = stack.pop() {
            graph.edges(r[ix], &mut |v| {
                if e_handle(v)
                    && let Some(&dest) = in_r.get(&v)
                    && !live_set[dest]
                {
                    live_set[dest] = true;
                    stack.push(dest);
                }
            });
        }
        // A survivor that only the observational root holds becomes a candidate
        // again: the root may disappear without a decrement.
        for (ix, &x) in r.iter().enumerate() {
            if live_set[ix] && protected(x) {
                self.add_candidate(x);
            }
        }
        let dead_set: Vec<Ref> = r.into_iter().enumerate().filter_map(|(i, x)| (!live_set[i]).then_some(x)).collect();
        if dead_set.is_empty() {
            return Ok(0);
        }
        let n = dead_set.len();
        self.discard(graph, &dead_set)?;
        self.statistics.dead_by_cycle += n as u64;
        Ok(n)
    }

    /// The logical deaths published since the last call, for the physical reclaimer;
    /// their metadata leaves here (§19.4, step 2).
    pub fn take_dead(&mut self) -> Vec<Ref> {
        let mut dead_set = Vec::new();
        self.transfer_dead(&mut dead_set);
        dead_set
    }

    /// Removes metadata and transfers the dead to the physical reclaimer. An empty
    /// destination receives the buffer by swap, without copying each handle; later
    /// rounds append their dead, preserving the previous ones.
    pub(crate) fn transfer_dead(&mut self, dest: &mut Vec<Ref>) -> usize {
        let amount = self.dead_set.len();
        for h in &self.dead_set {
            if self.objects.get(h).is_some_and(|m| m.state == ArcState::Dead) {
                self.objects.remove(h);
            }
        }
        if dest.is_empty() {
            std::mem::swap(dest, &mut self.dead_set);
        } else {
            dest.append(&mut self.dead_set);
        }
        amount
    }

    /// Recounts, with the heap stabilized, `root owners + strong edges` of each live
    /// object and compares with the RC (§20.3, audit mode). `roots` are the owning
    /// occurrences (not the observational ones, §21.3).
    pub fn audit(&self, graph: &dyn ArcGraph, roots: &[Ref]) -> Result<(), ArcError> {
        let mut count: HashMap<Ref, u64> = HashMap::default();
        for &h in roots {
            if e_handle(h) {
                *count.entry(h).or_insert(0) += 1;
            }
        }
        for (h, m) in self.objects.iter() {
            if m.state == ArcState::Dead {
                continue;
            }
            graph.edges(h, &mut |v| {
                if e_handle(v) {
                    *count.entry(v).or_insert(0) += 1;
                }
            });
        }
        for (h, m) in self.objects.iter() {
            if m.immortal || m.state == ArcState::Dead {
                continue;
            }
            let recounted = count.get(&h).copied().unwrap_or(0);
            if recounted != m.rc {
                return Err(ArcError::Audit { handle: h, rc: m.rc, recounted });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    #[test]
    fn immortal_does_not_reuse_generation_after_exhaustion() {
        let mut arc = ArcHeapState::new();
        arc.next_generation = u64::MAX - 1;
        arc.register_immortal(2);
        assert_eq!(arc.meta(2).unwrap().generation, u64::MAX - 1);
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arc.register_immortal(4);
        }));
        assert!(error.is_err());
        assert_eq!(arc.next_generation, u64::MAX);
        assert!(arc.meta(4).is_none());
        assert_eq!(arc.registered_set(), 1);
    }

    #[test]
    fn index_by_inverse_checks_every_class_byte() {
        use crate::layout::{PAGE_HEAD, HANDLE_OFFSET, N_CLASSES, PAGE, block_bytes, class_words};
        let first = (1u64 << 32) + PAGE_HEAD as u64;
        for class in 1..N_CLASSES {
            let size = block_bytes(class_words(class)) as u64;
            let blocks = ((PAGE - PAGE_HEAD) as u64 / size) as u32;
            let page = ArcPage::new_area(Geometry { first, size, blocks });
            assert_eq!(page.ix(first as Ref + HANDLE_OFFSET - 1), None);
            for d in 0..=PAGE as u64 {
                // A deliberate oracle through the original division: every start,
                // interior and byte beyond the last block.
                let expected = (d % size == 0 && d / size < u64::from(blocks)).then_some((d / size) as usize);
                let h = (first + d) as Ref + HANDLE_OFFSET;
                assert_eq!(page.ix(h), expected, "class {class}, offset {d}");
            }
        }
    }

    #[test]
    fn index_by_inverse_checks_large_sizes_and_u64_bounds() {
        for size in [1, 2, 3, 24, (1u64 << 32) - 1, 1u64 << 32, (1u64 << 63) - 1, 1u64 << 63, u64::MAX] {
            let blocks = (u64::MAX / size).min(7) as u32;
            let page = ArcPage::new_area(Geometry { first: 0, size, blocks });
            let query = |d: u64| page.ix(d.wrapping_add(crate::layout::HANDLE_OFFSET as u64) as Ref);
            for i in 0..u64::from(blocks) {
                let d = i * size;
                assert_eq!(query(d), Some(i as usize), "size {size}, index {i}");
                if size > 1 {
                    assert_eq!(query(d + 1), None);
                }
            }
            let end = size * u64::from(blocks);
            for d in [end, end.saturating_add(1), u64::MAX] {
                assert_eq!(query(d), None, "size {size}, offset {d}");
            }
        }
    }

    #[test]
    fn reformatted_page_updates_inverse_and_preserves_metadata() {
        let first = (1u64 << 32) + 1024;
        let h = (first + 48) as Ref + crate::layout::HANDLE_OFFSET;
        let mut a = ArcHeapState::new();
        a.register_live_at(h, Some(Geometry { first, size: 24, blocks: 4 }));
        a.review(h);
        a.drain_zeros(&mut Graph::default(), usize::MAX, &|_| false).unwrap();
        a.take_dead();
        let new = a.register_live_at(h, Some(Geometry { first, size: 48, blocks: 2 }));
        a.retain_ref(h).unwrap();
        assert_eq!(a.meta(h).unwrap().generation, new.generation);
        assert_eq!(a.meta(h).unwrap().rc, 1);
        assert!(a.meta(h + 24).is_none());
    }

    /// A fake graph: handle → list of targets (with multiplicity).
    #[derive(Default)]
    struct Graph(HashMap<Ref, Vec<Ref>>);

    impl ArcGraph for Graph {
        fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
            for &v in self.0.get(&h).map(|v| v.as_slice()).unwrap_or(&[]) {
                f(v);
            }
        }
        fn break_off(&mut self, h: Ref) {
            self.0.remove(&h);
        }
    }

    /// Allocates `h` (rc=1 of the construction owner) and finishes it.
    fn new(a: &mut ArcHeapState, h: Ref) {
        a.register(h).unwrap();
        a.finish_construction(h);
    }

    /// `u.field = v`: the target gains the occurrence.
    fn enable(a: &mut ArcHeapState, g: &mut Graph, u: Ref, v: Ref) {
        a.retain_ref(v).unwrap();
        g.0.entry(u).or_default().push(v);
    }

    fn dead(a: &ArcHeapState, h: Ref) -> bool {
        a.meta(h).is_none_or(|m| m.state == ArcState::Dead)
    }

    #[test]
    fn null_and_smi_are_not_counted() {
        let mut a = ArcHeapState::new();
        a.retain_ref(0).unwrap();
        a.retain_ref(7).unwrap();
        a.release(7).unwrap();
        assert_eq!(a.statistics.retains, 0);
    }

    #[test]
    fn transferred_dead_preserve_cascade_and_rounds_without_copying_first_buffer() {
        let mut a = ArcHeapState::new();
        let mut g = Graph::default();
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        a.release(4).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 2);
        let buffer = a.dead_set.as_ptr();
        let mut dest = Vec::new();
        assert_eq!(a.transfer_dead(&mut dest), 2);
        assert_eq!(dest, [2, 4]);
        assert_eq!(dest.as_ptr(), buffer);
        assert!(a.meta(2).is_none() && a.meta(4).is_none());
        new(&mut a, 6);
        a.release(6).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert_eq!(a.transfer_dead(&mut dest), 1);
        assert_eq!(dest, [2, 4, 6]);
        assert!(a.meta(6).is_none());
        assert_eq!(a.transfer_dead(&mut dest), 0);
        assert_eq!(dest, [2, 4, 6]);
        new(&mut a, 8);
        a.release(8).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert_eq!(a.take_dead(), [8]);
        assert!(a.meta(8).is_none());
    }

    #[test]
    fn side_edge_prevents_refless_body_filter() {
        let mut a = ArcHeapState::new();
        a.without_edges = Some(|_| true);
        let mut g = Graph::default();
        a.register_live(2);
        a.register_live(4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        a.retain_ref(2).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.candidates(), 0);
        a.mark_side_edges(2);
        assert_eq!(a.candidates(), 1);
        a.collect_cycles(&mut g, &|_| false).unwrap();
        assert!(dead(&a, 2) && dead(&a, 4));
        a.register_live(2);
        assert!(!a.meta(2).unwrap().side_edges);
        assert_eq!(std::mem::size_of::<MetaArc>(), 32);
    }

    #[test]
    fn acyclic_chain_dies_in_cascade_when_root_is_released() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        for h in [2, 4, 6] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 6);
        // The construction owners of 4 and 6 leave: only the edges keep them.
        a.release(4).unwrap();
        a.release(6).unwrap();
        a.audit(&g, &[2]).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 3);
        assert!(dead(&a, 2) && dead(&a, 4) && dead(&a, 6));
        assert_eq!(a.take_dead().len(), 3);
        assert_eq!(a.registered_set(), 0);
    }

    #[test]
    fn occurrence_multiplicity() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        // Two positions of list 2 point to 4: they count twice.
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 2, 4);
        a.release(4).unwrap();
        assert_eq!(a.meta(4).unwrap().rc, 2);
        a.audit(&g, &[2]).unwrap();
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 4));
    }

    #[test]
    fn isolated_cycle_dies_only_by_cycle_collection() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        a.release(4).unwrap();
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2) && !dead(&a, 4), "the RC alone does not close the cycle");
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
        assert!(dead(&a, 2) && dead(&a, 4));
        assert_eq!(a.statistics.dead_by_cycle, 2);
    }

    #[test]
    fn retain_if_registered_tells_absence_without_hiding_errors() {
        let mut a = ArcHeapState::new();
        for h in [0, 1, 3] { assert_eq!(a.retain_if_registered(h), Ok(true)); }
        assert_eq!(a.retain_if_registered(2), Ok(false));
        assert_eq!(a.retain_ref(2), Ok(()));
        assert_eq!(a.registered_set(), 0);
        assert_eq!(a.statistics.retains, 0);
        let id = a.register(2).unwrap();
        assert_eq!(a.retain_if_registered(2), Ok(true));
        assert_eq!(a.meta(2).unwrap().rc, 2);
        assert_eq!(a.meta(2).unwrap().generation, id.generation);
        a.finish_construction(2);
        a.objects.get_mut(&2).unwrap().rc = u64::MAX;
        assert_eq!(a.retain_if_registered(2), Err(ArcError::Overflow(2)));
        assert_eq!(a.meta(2).unwrap().rc, u64::MAX);
        assert_eq!(a.statistics.retains, 1);
        for state in [ArcState::Collecting, ArcState::Dead] {
            a.objects.get_mut(&2).unwrap().state = state;
            assert_eq!(a.retain_if_registered(2), Err(ArcError::InvalidState(2, state)));
            assert_eq!(a.statistics.retains, 1);
        }
        a.register_immortal(4);
        assert_eq!(a.retain_if_registered(4), Ok(true));
        assert_eq!(a.meta(4).unwrap().rc, 0);
        assert_eq!(a.statistics.retains, 1);
    }

    #[test]
    fn dense_survival_preserves_converging_paths_and_repeated_edges() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        let (x, y, z, w, root) = (2, 0x1_0002, 0x2_0002, 0x3_0002, 0x4_0002);
        for h in [x, y, z, w, root] { new(&mut a, h); }
        enable(&mut a, &mut g, root, x);
        enable(&mut a, &mut g, w, x);
        for _ in 0..96 {
            enable(&mut a, &mut g, x, y);
            enable(&mut a, &mut g, x, z);
            enable(&mut a, &mut g, y, w);
            enable(&mut a, &mut g, z, w);
        }
        for h in [x, y, z, w] { a.release(h).unwrap(); }
        let before = g.0.clone();
        let counts: Vec<_> = [x, y, z, w, root].map(|h| a.meta(h).unwrap().rc).into();
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!(g.0, before);
        assert_eq!([x, y, z, w, root].map(|h| a.meta(h).unwrap().rc).as_slice(), counts);
        a.audit(&g, &[root]).unwrap();
        // The same region, now without an external input, dies at once; all the
        // internal occurrences are cut down, without releasing internal RC.
        a.release(root).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 1);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 4);
        for h in [x, y, z, w, root] { assert!(dead(&a, h)); }
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 5);
        assert_eq!(dead_set.into_iter().collect::<HashSet<_>>(), [x, y, z, w, root].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn dense_trial_preserves_multiplicity_and_external_entry_in_sparse_handles() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        let (x, y, root) = (2, 0x1_0002, 0x4_0002);
        for h in [x, y, root] { new(&mut a, h); }
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, y, x);
        enable(&mut a, &mut g, root, y);
        a.release(x).unwrap();
        a.release(y).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!((a.meta(x).unwrap().rc, a.meta(y).unwrap().rc), (1, 3));
        a.release(root).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 1);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 3);
        assert!([x, y, root].iter().all(|h| dead_set.contains(h)));
    }

    #[test]
    fn dense_negative_trial_identifies_handle_and_aborts_before_discard() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        let (x, y) = (2, 0x1_0002);
        new(&mut a, x);
        new(&mut a, y);
        // A tampered inventory: two internal edges, but the RC of y is one.
        g.0.insert(x, vec![y, y]);
        g.0.insert(y, vec![x]);
        a.add_candidate(x);
        let before = g.0.clone();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Err(ArcError::NegativeTrial(y)));
        assert_eq!(g.0, before);
        for h in [x, y] {
            assert_eq!(a.meta(h).unwrap().rc, 1);
            assert_eq!(a.meta(h).unwrap().state, ArcState::Live);
        }
        assert!(a.take_dead().is_empty());
        // Fixing the inventory and requeuing must neither inherit indices/trial from
        // the previous diagnostic nor apply a partial discard of it.
        g.0.insert(x, vec![y]);
        a.add_candidate(x);
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(2));
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 2);
        assert!([x, y].iter().all(|h| dead_set.contains(h)));
    }

    #[test]
    fn successive_cycles_inherit_neither_survivors_nor_reclaimed_generations() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        let (x, y) = (2, 0x1_0002);
        for h in [x, y] { new(&mut a, h); }
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, y, x);
        a.release(y).unwrap();
        a.add_candidate(x);
        let generation = a.meta(x).unwrap().generation;
        // The external owner of x preserves both nodes; in the next round neither
        // of them may inherit the survivor mark.
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(0));
        assert_eq!((a.meta(x).unwrap().rc, a.meta(y).unwrap().rc), (2, 1));
        a.release(x).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(2));
        assert_eq!(a.take_dead().len(), 2);
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(0));
        // Same address, a smaller region and a new generation: the old map and
        // vectors must not prevent the discovery or protect the self-cycle.
        new(&mut a, x);
        assert!(a.meta(x).unwrap().generation > generation);
        enable(&mut a, &mut g, x, x);
        a.release(x).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(1));
        assert_eq!(a.take_dead(), vec![x]);
        assert_eq!(a.registered_set(), 0);
    }

    #[test]
    fn cycle_with_external_reference_survives_and_collection_keeps_rc() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        for h in [2, 4, 6] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        // 6 (root) points to 4.
        enable(&mut a, &mut g, 6, 4);
        a.release(4).unwrap();
        a.release(2).unwrap();
        let before = (a.meta(2).unwrap().rc, a.meta(4).unwrap().rc);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!((a.meta(2).unwrap().rc, a.meta(4).unwrap().rc), before);
        a.audit(&g, &[6]).unwrap();
        // Without the root, the cycle dies in the next round; 6 dies by RC.
        a.release(6).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 6));
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
    }

    #[test]
    fn cycle_holding_an_acyclic_releases_both() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        for h in [2, 4, 8] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        enable(&mut a, &mut g, 4, 8);
        for h in [2, 4, 8] {
            a.release(h).unwrap();
        }
        a.collect_cycles(&mut g, &|_| false).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 2) && dead(&a, 4) && dead(&a, 8));
    }

    #[test]
    fn under_construction_and_protected_do_not_die() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        a.register(2).unwrap();
        // Under construction: a zero RC does not discard.
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2));
        new(&mut a, 4);
        a.protect(&g, 4);
        a.release(4).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 4), "a conditionally protected object waits for the fixpoint");
    }

    #[test]
    fn protected_root_does_not_expand_trial_and_keeps_candidate() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        for h in [2, 4, 6, 8] { new(&mut a, h); }
        for (of, to) in [(2, 4), (4, 2), (4, 6), (6, 8), (8, 6)] {
            enable(&mut a, &mut g, of, to);
        }
        for h in [2, 4, 6, 8] { a.release(h).unwrap(); }
        let root = |h| h == 2;
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        a.audit(&g, &[]).unwrap();
        let examined = a.statistics.examined;
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        assert_eq!(a.statistics.examined, examined);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 4);
        for h in [2, 4, 6, 8] { assert!(dead(&a, h)); }
    }

    #[test]
    fn observational_root_holds_zero_and_cycle() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        new(&mut a, 6);
        a.release(2).unwrap();
        a.release(4).unwrap();
        a.release(6).unwrap();
        // 6 is only seen by the stack; the 2-4 cycle too.
        let root = |h: Ref| h == 6 || h == 2;
        a.drain_zeros(&mut g, usize::MAX, &root).unwrap();
        assert!(!dead(&a, 6));
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        // The stack released: they die in the next drain (which starts by resuming
        // the deferred ones) and round.
        a.resume_deferred();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 6));
        let n = {
            a.candidates.push(ArcId { handle: 2, generation: a.meta(2).unwrap().generation });
            a.collect_cycles(&mut g, &|_| false).unwrap()
        };
        assert_eq!(n, 2);
    }

    #[test]
    fn release_with_zero_rc_is_an_error_and_overflow_too() {
        let mut a = ArcHeapState::new();
        new(&mut a, 2);
        a.release(2).unwrap();
        assert_eq!(a.release(2), Err(ArcError::RcZero(2)));
    }

    #[test]
    fn stale_queue_entry_is_ignored_by_generation() {
        let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
        new(&mut a, 2);
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        a.take_dead();
        // The address is reused: a new generation, the old entry is not valid.
        new(&mut a, 2);
        a.zeros.push(ArcId { handle: 2, generation: 1 });
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2));
    }

    #[test]
    fn audit_finds_wrong_rc() {
        let (mut a, g) = (ArcHeapState::new(), Graph::default());
        new(&mut a, 2);
        a.retain_ref(2).unwrap();
        assert!(matches!(a.audit(&g, &[2]), Err(ArcError::Audit { handle: 2, rc: 2, recounted: 1 })));
    }

    #[test]
    fn bounded_cascade_preserves_root_multiplicity_and_generation_in_reformatted_page() {
        use crate::layout::HANDLE_OFFSET;
        for paged in [false, true] {
            let first = (1u64 << 32) + 1024;
            let h1 = first as Ref + HANDLE_OFFSET;
            let h2 = h1 + 24;
            let h3 = 2; // Hash fallback, even when the others use a page.
            let geometry = paged.then_some(Geometry { first, size: 24, blocks: 4 });
            let (mut a, mut g) = (ArcHeapState::new(), Graph::default());
            let id1 = a.register_live_at(h1, geometry);
            let id2 = a.register_live_at(h2, geometry);
            a.register_live(h3);
            for h in [h1, h2, h3] { a.retain_ref(h).unwrap(); }
            for (of, to) in [(h1, h2), (h1, h2), (h1, h3), (h2, h3)] {
                enable(&mut a, &mut g, of, to);
            }
            for h in [h2, h3, h1] { a.release(h).unwrap(); }
            let root = |h| h == h2;
            assert_eq!(a.drain_zeros(&mut g, 1, &root), Ok(1));
            assert_eq!(a.take_dead(), vec![h1]);
            assert_eq!(a.meta(h2).unwrap().rc, 0);
            assert_eq!(a.meta(h3).unwrap().rc, 1);
            assert_eq!(a.drain_zeros(&mut g, usize::MAX, &root), Ok(0));
            assert!(a.alive(h2) && a.alive(h3));
            a.audit(&g, &[]).unwrap();
            a.resume_deferred();
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert!(a.alive(h3));
            assert_eq!(a.meta(h3).unwrap().rc, 0);
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert_eq!(a.take_dead(), vec![h2, h3]);
            assert_eq!(a.registered_set(), 0);
            // All the page's slots died; another size may occupy the same address.
            // No location crosses this boundary.
            let new_area = paged.then_some(Geometry { first, size: 48, blocks: 2 });
            let current = a.register_live_at(h1, new_area);
            a.retain_ref(h1).unwrap();
            a.zeros.extend([id1, id2]);
            assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(0));
            let m = a.meta(h1).unwrap();
            assert_eq!((m.generation, m.rc, m.candidate), (current.generation, 1, false));
            a.release(h1).unwrap();
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert_eq!(a.take_dead(), vec![h1]);
            a.audit(&g, &[]).unwrap();
        }
    }
}

#[cfg(test)]
mod candidates;
