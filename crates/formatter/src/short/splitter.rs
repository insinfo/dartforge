//! Port of `short/line_splitting/*`: the line splitter (`LineSplitter`), the
//! solution states (`SolveState`), the priority queue that merges overlapping
//! states (`SolveStateQueue`), the `RuleSet` and the `SplitSet`.
//!
//! The search is the same as the original's, including the limit of 5000
//! attempts and the exploration order, because the choice between solutions
//! of equal cost depends on it.

use std::collections::HashSet;

use super::arena::{Arenas, ChunkId, MUST_SPLIT, RuleId, UNSPLIT, indentation};
use super::writer::{BlockCache, format_block};

const MAX_ATTEMPTS: usize = 5000;

/// `SplitSet`.
#[derive(Debug, Clone)]
pub struct Splits {
    columns: Vec<i32>,
    pub cost: i32,
}

impl Splits {
    /// `shouldSplitAt(index)`.
    pub fn should_split_at(&self, i: usize) -> bool {
        i < self.columns.len() && self.columns[i] != -1
    }

    /// `getColumn(index)`.
    pub fn column(&self, i: usize) -> i32 {
        self.columns[i]
    }
}

/// `RuleSet`: values by rule index.
#[derive(Debug, Clone)]
struct RuleValues(Vec<Option<i32>>);

/// The previous values of the entries a [`RuleValues::try_bind`] changed, in
/// order, so that [`RuleValues::undo`] can restore them.
type UndoLog = Vec<(usize, Option<i32>)>;

impl RuleValues {
    fn index(a: &Arenas, r: RuleId) -> Option<usize> {
        a.rules[r as usize].index
    }

    fn contains_rule(&self, a: &Arenas, r: RuleId) -> bool {
        if a.is_hardened(r) {
            return true;
        }
        Self::index(a, r)
            .and_then(|i| self.0.get(i).copied().flatten())
            .is_some()
    }

    fn value(&self, a: &Arenas, r: RuleId) -> i32 {
        if a.is_hardened(r) {
            return a.full_value(r);
        }
        Self::index(a, r)
            .and_then(|i| self.0.get(i).copied().flatten())
            .unwrap_or(UNSPLIT)
    }

    fn raw(&self, a: &Arenas, r: RuleId) -> Option<i32> {
        Self::index(a, r).and_then(|i| self.0.get(i).copied().flatten())
    }

    fn set_value(&mut self, a: &Arenas, r: RuleId, v: i32, log: &mut UndoLog) {
        if let Some(i) = Self::index(a, r)
            && i < self.0.len()
        {
            log.push((i, self.0[i]));
            self.0[i] = Some(v);
        }
    }

    /// Restores the entries recorded in `log`, newest first, and empties it.
    fn undo(&mut self, log: &mut UndoLog) {
        while let Some((i, old)) = log.pop() {
            self.0[i] = old;
        }
    }

    /// `tryBind(rules, rule, value, onSplitRule)`.
    ///
    /// Binds in place and records every change in `log`. The original binds
    /// on a fresh clone of the set; here the caller undoes the bindings
    /// instead, so that a binding that fails (most of them, on a line with
    /// thousands of live rules) costs only the rules it touched rather than a
    /// copy of the whole set.
    fn try_bind(
        &mut self,
        a: &Arenas,
        r: RuleId,
        value: i32,
        on_split: &mut dyn FnMut(RuleId),
        log: &mut UndoLog,
    ) -> bool {
        debug_assert!(!a.is_hardened(r));
        self.set_value(a, r, value, log);
        let mut i = 0;
        while let Some(other) = a.constrained(r, i) {
            i += 1;
            let other_value = if a.is_hardened(other) {
                Some(a.full_value(other))
            } else {
                self.raw(a, other)
            };
            let constraint = a.constrains(r, value, other);
            match other_value {
                None => {
                    if constraint == Some(MUST_SPLIT) {
                        if a.num_values(other) == 2 {
                            if !self.try_bind(a, other, 1, on_split, log) {
                                return false;
                            }
                        } else {
                            on_split(other);
                        }
                    } else if let Some(c) = constraint
                        && !self.try_bind(a, other, c, on_split, log)
                    {
                        return false;
                    }
                }
                Some(ov) => {
                    if constraint == Some(MUST_SPLIT) {
                        if ov == UNSPLIT {
                            return false;
                        }
                    } else if let Some(c) = constraint
                        && ov != c
                    {
                        return false;
                    }
                    let inverse = a.constrains(other, ov, r);
                    if inverse == Some(MUST_SPLIT) {
                        if value == UNSPLIT {
                            return false;
                        }
                    } else if let Some(c) = inverse
                        && value != c
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// `LineSplitter`.
pub struct LineSplitter {
    chunks: Vec<ChunkId>,
    rules: Vec<RuleId>,
    block_indent: i32,
}

/// `SolveState`.
struct State {
    values: RuleValues,
    splits: Splits,
    overflow: i32,
    full: bool,
    live: Vec<RuleId>,
    live_set: HashSet<RuleId>,
    constraints: Option<Vec<(RuleId, i32)>>,
    bound: Option<HashSet<RuleId>>,
    unbound: Option<HashSet<RuleId>>,
    unbound_constraints: Option<Vec<(RuleId, Vec<i32>)>>,
    bound_in_unbound_lines: Option<Vec<RuleId>>,
}

impl LineSplitter {
    /// `LineSplitter(writer, chunks, blockIndentation)`.
    pub fn new(a: &mut Arenas, chunks: Vec<ChunkId>, block_indent: i32) -> LineSplitter {
        let mut rules: Vec<RuleId> = Vec::new();
        let mut seen = HashSet::new();
        for &c in &chunks {
            let r = a.chunks[c as usize].rule;
            if seen.insert(r) {
                rules.push(r);
            }
        }
        for (i, &r) in rules.iter().enumerate() {
            a.rules[r as usize].index = Some(i);
        }
        for &r in &rules {
            a.forget_unused(r);
        }
        LineSplitter {
            chunks,
            rules,
            block_indent,
        }
    }

    /// `apply()`.
    pub fn apply(&self, a: &mut Arenas, cache: &mut BlockCache) -> Splits {
        let mut queue = Queue { items: Vec::new() };
        let initial = self.new_state(a, cache, RuleValues(vec![None; self.rules.len()]));
        let mut best_idx;
        let mut states: Vec<State> = vec![initial];
        best_idx = 0usize;
        queue.add(self, a, &mut states, 0);

        let mut attempts = 0;
        while !queue.items.is_empty() {
            let current = queue.remove_first(self, a, &mut states);
            if self.better_than(&states[current], &states[best_idx]) {
                best_idx = current;
                if states[best_idx].overflow == 0 {
                    break;
                }
            }
            let exceeded = attempts > MAX_ATTEMPTS;
            attempts += 1;
            if exceeded {
                break;
            }
            self.expand(a, cache, &mut states, &mut queue, current);
        }
        states.swap_remove(best_idx).splits
    }

    fn better_than(&self, e: &State, other_: &State) -> bool {
        if !e.full {
            return false;
        }
        if e.overflow != other_.overflow {
            return e.overflow < other_.overflow;
        }
        e.splits.cost < other_.splits.cost
    }

    fn new_state(&self, a: &mut Arenas, cache: &mut BlockCache, values: RuleValues) -> State {
        let mut e = State {
            values,
            splits: Splits {
                columns: Vec::new(),
                cost: 0,
            },
            overflow: 0,
            full: true,
            live: Vec::new(),
            live_set: HashSet::new(),
            constraints: None,
            bound: None,
            unbound: None,
            unbound_constraints: None,
            bound_in_unbound_lines: None,
        };
        self.compute_splits(a, &mut e);
        self.compute_cost(a, cache, &mut e);
        e
    }

    fn compute_splits(&self, a: &mut Arenas, e: &mut State) {
        let mut used = Vec::new();
        for &c in &self.chunks {
            let r = a.chunks[c as usize].rule;
            if a.split(r, e.values.value(a, r), c) {
                let n = a.chunks[c as usize].nesting_;
                if a.mark_nesting(n) {
                    used.push(n);
                    a.clear_total(n);
                }
            }
        }
        for &n in &used {
            a.update_total(n);
        }
        for &n in &used {
            a.unmark_nesting(n);
        }
        let mut columns = vec![-1; self.chunks.len()];
        for (i, &c) in self.chunks.iter().enumerate() {
            let r = a.chunks[c as usize].rule;
            if a.split(r, e.values.value(a, r), c) {
                let mut col = 0;
                let ch = &a.chunks[c as usize];
                if !ch.flush_left {
                    col = self.block_indent + ch.indentation;
                    col += a.nesting_total(ch.nesting_);
                    let values = &e.values;
                    if a.indents_block(c, &|r| values.value(a, r)) {
                        col += indentation::EXPRESSION;
                    }
                }
                columns[i] = col;
            }
        }
        e.splits = Splits { columns, cost: 0 };
    }

    fn compute_cost(&self, a: &mut Arenas, cache: &mut BlockCache, e: &mut State) {
        let mut cost = 0;
        let mut length: i64 = 0;
        let mut found_rules = false;
        let mut start = 0usize;
        let page_width = a.page_width as i64;

        let mut split_spans = Vec::new();
        let mut previous_nesting = None;

        let line_ending = |a: &mut Arenas,
                           e: &mut State,
                           end: usize,
                           length: i64,
                           start: &mut usize,
                           found_it: &mut bool| {
            if length > page_width {
                e.overflow += (length - page_width) as i32;
                if !*found_it {
                    for i in *start..end {
                        let r = a.chunks[self.chunks[i] as usize].rule;
                        if add_live_rules(a, e, r) {
                            *found_it = true;
                        }
                    }
                }
            }
            *start = end;
        };

        for i in 0..self.chunks.len() {
            let c = self.chunks[i];
            if e.splits.should_split_at(i) {
                line_ending(a, e, i, length, &mut start, &mut found_rules);
                let spans = a.chunks[c as usize].spans.clone();
                for s in spans {
                    if a.mark_span(s) {
                        split_spans.push(s);
                        cost += a.spans[s as usize].cost;
                    }
                }
                let nesting_ = a.chunks[c as usize].nesting_;
                let total = a.nesting_total(nesting_);
                if let Some(ant) = previous_nesting
                    && total != 0
                    && total == a.nesting_total(ant)
                    && nesting_ != ant
                {
                    e.overflow += 10000;
                }
                previous_nesting = Some(nesting_);
                length = e.splits.column(i) as i64;
            } else if a.chunks[c as usize].space {
                length += 1;
            }
            if a.chunks[c as usize].block.is_some() {
                if e.splits.should_split_at(i) {
                    cost += format_block(a, cache, c, e.splits.column(i)).cost;
                } else {
                    length += a.block_length(c) as i64;
                }
            }
            length += a.chunks[c as usize].text_len as i64;
        }

        for (i, &r) in self.rules.iter().enumerate() {
            if let Some(v) = e.values.0[i]
                && v != UNSPLIT
            {
                cost += a.cost(r);
            }
        }

        for s in split_spans {
            a.unmark_span(s);
        }

        line_ending(
            a,
            e,
            self.chunks.len(),
            length,
            &mut start,
            &mut found_rules,
        );
        e.splits.cost = cost;
    }

    /// `expand()`.
    fn expand(
        &self,
        a: &mut Arenas,
        cache: &mut BlockCache,
        states: &mut Vec<State>,
        queue: &mut Queue,
        current: usize,
    ) {
        let mut no_split = states[current].values.clone();
        let mut log = UndoLog::new();
        let mut attempted = 0;
        for &r in &self.rules {
            if states[current].live_set.contains(&r) {
                for value in 1..a.num_values(r) {
                    let mut must: Option<Vec<RuleId>> = None;
                    let valid = no_split.try_bind(
                        a,
                        r,
                        value,
                        &mut |x| must.get_or_insert_with(Vec::new).push(x),
                        &mut log,
                    );
                    let bound = valid.then(|| no_split.clone());
                    no_split.undo(&mut log);
                    let Some(bound) = bound else {
                        continue;
                    };
                    let mut new = self.new_state(a, cache, bound);
                    if let Some(must) = must {
                        new.full = false;
                        for x in must {
                            if new.live_set.insert(x) {
                                new.live.push(x);
                            }
                        }
                    }
                    states.push(new);
                    let id = states.len() - 1;
                    queue.add(self, a, states, id);
                }
                attempted += 1;
                if attempted == states[current].live.len() {
                    break;
                }
            }
            // The original binds a clone and keeps it only on success; on failure
            // the loop stops, so binding in place is equivalent.
            if !states[current].values.contains_rule(a, r)
                && !no_split.try_bind(a, r, 0, &mut |_| {}, &mut log)
            {
                break;
            }
            log.clear();
        }
    }

    fn value(&self, a: &Arenas, e: &State, r: RuleId) -> i32 {
        e.values.value(a, r)
    }

    /// `compareOverlap(other)`: `<0` if `e` wins, `>0` if `other_` wins, `0` if
    /// they do not overlap.
    fn compare_overlap(
        &self,
        a: &mut Arenas,
        states: &mut [State],
        e: usize,
        other_: usize,
    ) -> i32 {
        if !self.overlaps(a, states, e, other_) {
            return 0;
        }
        for &r in &self.rules {
            let v = self.value(a, &states[e], r);
            let ov = self.value(a, &states[other_], r);
            if v != ov {
                return if v < ov { -1 } else { 1 };
            }
        }
        panic!("repeated states in the line splitter");
    }

    fn overlaps(&self, a: &mut Arenas, states: &mut [State], e: usize, other_: usize) -> bool {
        self.prepare_bound_in_lines(a, &mut states[e]);
        self.prepare_bound_in_lines(a, &mut states[other_]);
        let le = states[e].bound_in_unbound_lines.as_ref().expect("prepared");
        let lo = states[other_]
            .bound_in_unbound_lines
            .as_ref()
            .expect("prepared");
        if le.len() != lo.len() {
            return false;
        }
        for &r in le {
            if !lo.contains(&r) {
                return false;
            }
            if self.value(a, &states[e], r) != self.value(a, &states[other_], r) {
                return false;
            }
        }

        self.prepare_constraints(a, &mut states[e]);
        self.prepare_constraints(a, &mut states[other_]);
        let re = states[e].constraints.as_ref().expect("prepared");
        let ro = states[other_].constraints.as_ref().expect("prepared");
        if re.len() != ro.len() {
            return false;
        }
        for (r, v) in re {
            if ro.iter().find(|(o, _)| o == r).map(|(_, x)| *x) != Some(*v) {
                return false;
            }
        }

        self.prepare_unbound_constraints(a, &mut states[e]);
        self.prepare_unbound_constraints(a, &mut states[other_]);
        let if_ = states[e].unbound_constraints.as_ref().expect("prepared");
        let only = states[other_]
            .unbound_constraints
            .as_ref()
            .expect("prepared");
        if if_.len() != only.len() {
            return false;
        }
        for (r, forbidden) in if_ {
            let Some((_, others)) = only.iter().find(|(o, _)| o == r) else {
                return false;
            };
            if forbidden.len() != others.len() {
                return false;
            }
            for v in forbidden {
                if !others.contains(v) {
                    return false;
                }
            }
        }
        true
    }

    fn prepare_bound_in_lines(&self, a: &Arenas, e: &mut State) {
        if e.bound_in_unbound_lines.is_some() {
            return;
        }
        let mut rules: Vec<RuleId> = Vec::new();
        let mut on_line: Vec<RuleId> = Vec::new();
        let mut has_unbound = false;
        let join = |rules: &mut Vec<RuleId>, on_line: &Vec<RuleId>| {
            for &r in on_line {
                if !rules.contains(&r) {
                    rules.push(r);
                }
            }
        };
        for (i, &c) in self.chunks.iter().enumerate() {
            if e.splits.should_split_at(i) {
                if has_unbound {
                    join(&mut rules, &on_line);
                }
                on_line.clear();
                has_unbound = false;
            }
            let r = a.chunks[c as usize].rule;
            if e.values.contains_rule(a, r) {
                if !on_line.contains(&r) {
                    on_line.push(r);
                }
            } else {
                has_unbound = true;
            }
        }
        if has_unbound {
            join(&mut rules, &on_line);
        }
        e.bound_in_unbound_lines = Some(rules);
    }

    fn prepare_constraints(&self, a: &Arenas, e: &mut State) {
        if e.constraints.is_some() {
            return;
        }
        let mut bound = HashSet::new();
        let mut unbound = HashSet::new();
        let mut bound_order = Vec::new();
        for &r in &self.rules {
            if e.values.contains_rule(a, r) {
                bound.insert(r);
                bound_order.push(r);
            } else {
                unbound.insert(r);
            }
        }
        let mut constraints: Vec<(RuleId, i32)> = Vec::new();
        for &l in &bound_order {
            let mut i = 0;
            while let Some(s) = a.constrained(l, i) {
                i += 1;
                if !unbound.contains(&s) {
                    continue;
                }
                let v = e.values.value(a, l);
                if let Some(c) = a.constrains(l, v, s) {
                    if let Some(x) = constraints.iter_mut().find(|(o, _)| *o == s) {
                        x.1 = c;
                    } else {
                        constraints.push((s, c));
                    }
                }
            }
        }
        e.constraints = Some(constraints);
        e.bound = Some(bound);
        e.unbound = Some(unbound);
    }

    fn prepare_unbound_constraints(&self, a: &Arenas, e: &mut State) {
        if e.unbound_constraints.is_some() {
            return;
        }
        let bound = e.bound.as_ref().expect("prepared constraints");
        let unbound = e.unbound.as_ref().expect("prepared constraints");
        let mut result: Vec<(RuleId, Vec<i32>)> = Vec::new();
        // `_unboundRules` is an insertion-ordered set: the order of the splitter's
        // rules.
        for &s in self.rules.iter().filter(|r| unbound.contains(r)) {
            let mut forbidden: Option<Vec<i32>> = None;
            let mut i = 0;
            while let Some(l) = a.constrained(s, i) {
                i += 1;
                if !bound.contains(&l) {
                    continue;
                }
                let bound_value = e.values.value(a, l);
                for v in 0..a.num_values(s) {
                    let Some(c) = a.constrains(s, v, l) else {
                        continue;
                    };
                    if c == bound_value {
                        continue;
                    }
                    if c == MUST_SPLIT && bound_value != UNSPLIT {
                        continue;
                    }
                    let p = forbidden.get_or_insert_with(Vec::new);
                    if !p.contains(&v) {
                        p.push(v);
                    }
                }
            }
            if let Some(p) = forbidden {
                result.push((s, p));
            }
        }
        e.unbound_constraints = Some(result);
    }
}

/// `_addLiveRules(rule)`.
fn add_live_rules(a: &mut Arenas, e: &mut State, r: RuleId) -> bool {
    let mut added = false;
    for x in a.all_constrained(r) {
        if e.values.contains_rule(a, x) {
            continue;
        }
        if e.live_set.insert(x) {
            e.live.push(x);
        }
        added = true;
    }
    added
}

/// `SolveStateQueue`: a binary heap of indices into `states`.
struct Queue {
    items: Vec<usize>,
}

impl Queue {
    fn compare_scores(states: &[State], x: usize, y: usize) -> i32 {
        let (cx, cy) = (states[x].splits.cost, states[y].splits.cost);
        if cx != cy {
            return if cx < cy { -1 } else { 1 };
        }
        let (ox, oy) = (states[x].overflow, states[y].overflow);
        ox.cmp(&oy) as i32
    }

    fn compare_rules(d: &LineSplitter, a: &Arenas, states: &[State], x: usize, y: usize) -> i32 {
        for &r in &d.rules {
            let vx = d.value(a, &states[x], r);
            let vy = d.value(a, &states[y], r);
            if vx != vy {
                return if vx < vy { -1 } else { 1 };
            }
        }
        panic!("repeated states in the splitter queue");
    }

    fn compare(d: &LineSplitter, a: &Arenas, states: &[State], x: usize, y: usize) -> i32 {
        let c = Self::compare_scores(states, x, y);
        if c != 0 {
            return c;
        }
        Self::compare_rules(d, a, states, x, y)
    }

    fn add(&mut self, d: &LineSplitter, a: &mut Arenas, states: &mut [State], e: usize) {
        if self.try_overlap(d, a, states, e) {
            return;
        }
        self.items.push(e);
        let n = self.items.len() - 1;
        self.sift_up(d, a, states, e, n);
    }

    fn remove_first(&mut self, d: &LineSplitter, a: &Arenas, states: &mut [State]) -> usize {
        let result = self.items[0];
        let last = self.items.pop().expect("non-empty queue");
        if !self.items.is_empty() {
            self.sift_down(d, a, states, last, 0);
        }
        result
    }

    fn try_overlap(
        &mut self,
        d: &LineSplitter,
        a: &mut Arenas,
        states: &mut [State],
        e: usize,
    ) -> bool {
        let n = self.items.len();
        if n == 0 {
            return false;
        }
        let mut position = 1usize;
        loop {
            let index = position - 1;
            let enqueued = self.items[index];
            let mut comparison = Self::compare_scores(states, enqueued, e);
            if comparison == 0 {
                let over = d.compare_overlap(a, states, enqueued, e);
                if over < 0 {
                    return true;
                } else if over > 0 {
                    self.items[index] = e;
                    return true;
                } else {
                    comparison = Self::compare_rules(d, a, states, enqueued, e);
                }
            }
            if comparison < 0 {
                let left_ = position * 2;
                if left_ <= n {
                    position = left_;
                    continue;
                }
            }
            loop {
                while position % 2 == 1 {
                    position >>= 1;
                }
                position += 1;
                if position <= n {
                    break;
                }
            }
            if position == 1 {
                break;
            }
        }
        false
    }

    fn sift_up(
        &mut self,
        d: &LineSplitter,
        a: &Arenas,
        states: &[State],
        e: usize,
        mut index: usize,
    ) {
        while index > 0 {
            let parent_i = (index - 1) / 2;
            let parent = self.items[parent_i];
            if Self::compare(d, a, states, e, parent) > 0 {
                break;
            }
            self.items[index] = parent;
            index = parent_i;
        }
        self.items[index] = e;
    }

    fn sift_down(
        &mut self,
        d: &LineSplitter,
        a: &Arenas,
        states: &[State],
        e: usize,
        mut index: usize,
    ) {
        let n = self.items.len();
        let mut right = index * 2 + 2;
        while right < n {
            let left_ = right - 1;
            let fe = self.items[left_];
            let fd = self.items[right];
            let (min_i, smaller) = if Self::compare(d, a, states, fe, fd) < 0 {
                (left_, fe)
            } else {
                (right, fd)
            };
            if Self::compare(d, a, states, e, smaller) <= 0 {
                self.items[index] = e;
                return;
            }
            self.items[index] = smaller;
            index = min_i;
            right = index * 2 + 2;
        }
        let left_ = right - 1;
        if left_ < n {
            let child = self.items[left_];
            if Self::compare(d, a, states, e, child) > 0 {
                self.items[index] = child;
                index = left_;
            }
        }
        self.items[index] = e;
    }
}
