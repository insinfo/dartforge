//! The closed-world fixed point, in generic form.
//!
//! It is the same loop as `dart2js`'s (`resolution/enqueuer.dart:283-302`),
//! reduced to the essentials: sets that only grow, a unit lights up at most
//! once, and each symbol has the list of units waiting for it so that there
//! is no full scan on every change — which is the role of the inverted
//! indexes of `universe/resolution_world_builder.dart:232-240`.
//!
//! A unit lights up when
//!
//! * **any** of its triggers is live (or it has no trigger), **and**
//! * **all** of its requirements are live.
//!
//! The conjunction is what gives member granularity: a class method has the
//! requirements `[the class, the selector]`, which is the rule "member `m` of
//! `C` is live if `C` is instantiated **and** an applicable selector exists"
//! of §1.1 of the plan.

use std::collections::HashMap;

pub type Sym = u32;

/// Symbol interning table. The names are strings such as `core.Object`,
/// `dartx.length`, `C#42` or `sel:toString`.
#[derive(Default)]
pub struct Symbols {
    map: HashMap<String, Sym>,
    names: Vec<String>,
}

impl Symbols {
    pub fn intern(&mut self, name: &str) -> Sym {
        if let Some(&s) = self.map.get(name) {
            return s;
        }
        let s = self.names.len() as Sym;
        self.names.push(name.to_string());
        self.map.insert(name.to_string(), s);
        s
    }
    pub fn lookup(&self, name: &str) -> Option<Sym> {
        self.map.get(name).copied()
    }
    pub fn name(&self, s: Sym) -> &str {
        &self.names[s as usize]
    }
    pub fn total(&self) -> usize {
        self.names.len()
    }
    /// The names, in `Sym` order (what the index stores).
    pub fn names(self) -> Vec<String> {
        self.names
    }
    /// The table rebuilt from the names in `Sym` order.
    pub fn from_names(names: Vec<String>) -> Symbols {
        let map = names.iter().enumerate().map(|(i, n)| (n.clone(), i as Sym)).collect();
        Symbols { map, names }
    }
}

/// An indivisible unit of the output file.
#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Unit {
    /// Live from the start, unconditionally (the `export`, `trackLibraries`).
    pub always: bool,
    /// OR: any one of them live satisfies this part.
    pub triggers: Vec<Sym>,
    /// AND: all of them must be live.
    pub requirements: Vec<Sym>,
    /// Symbols that the text of this unit references.
    pub refs: Vec<Sym>,
    /// Selectors that the text of this unit uses (`sel:…`), already interned.
    pub selectors: Vec<Sym>,
}

/// Runs the fixed point and returns the "is the unit live?" vector.
pub fn solve(units: &[Unit], roots: &[Sym], total_symbols: usize) -> Vec<bool> {
    // Inverted index: symbol → units that mention it in their condition.
    let mut waiting: Vec<Vec<u32>> = vec![Vec::new(); total_symbols];
    for (i, u) in units.iter().enumerate() {
        for &s in u.triggers.iter().chain(u.requirements.iter()) {
            waiting[s as usize].push(i as u32);
        }
    }
    let mut live = vec![false; units.len()];
    let mut symbol_live = vec![false; total_symbols];
    let mut queue: Vec<u32> = Vec::new();
    let mut pending: Vec<Sym> = Vec::new();

    let satisfied = |u: &Unit, sv: &[bool]| {
        (u.triggers.is_empty() || u.triggers.iter().any(|&s| sv[s as usize]))
            && u.requirements.iter().all(|&s| sv[s as usize])
    };

    for (i, u) in units.iter().enumerate() {
        if u.always {
            live[i] = true;
            queue.push(i as u32);
        }
    }
    pending.extend_from_slice(roots);

    loop {
        // Lights up the pending symbols and re-examines whoever waited for them.
        while let Some(s) = pending.pop() {
            if symbol_live[s as usize] {
                continue;
            }
            symbol_live[s as usize] = true;
            for &i in &waiting[s as usize] {
                let i = i as usize;
                if !live[i] && satisfied(&units[i], &symbol_live) {
                    live[i] = true;
                    queue.push(i as u32);
                }
            }
        }
        // Propagates what the lit units reference.
        let Some(i) = queue.pop() else { break };
        let u = &units[i as usize];
        for &s in u.refs.iter().chain(u.selectors.iter()) {
            if !symbol_live[s as usize] {
                pending.push(s);
            }
        }
    }
    live
}

/// Shortest path from a root to `target`, in symbols — the diagnostic tool
/// that answers "**who** pulled this into the file?". Without it, a reach
/// that is too large can only be investigated by trial and error.
pub fn path(units: &[Unit], roots: &[Sym], total_symbols: usize, target: Sym) -> Option<Vec<Sym>> {
    let mut waiting: Vec<Vec<u32>> = vec![Vec::new(); total_symbols];
    for (i, u) in units.iter().enumerate() {
        for &s in u.triggers.iter().chain(u.requirements.iter()) {
            waiting[s as usize].push(i as u32);
        }
    }
    let mut live = vec![false; units.len()];
    let mut sym_live = vec![false; total_symbols];
    // For each symbol, which symbol it came from (breadth-first, so it is the
    // shortest).
    let mut came_from: Vec<Option<Sym>> = vec![None; total_symbols];
    let mut sym_queue: std::collections::VecDeque<Sym> = std::collections::VecDeque::new();
    let mut unit_queue: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
    for (i, u) in units.iter().enumerate() {
        if u.always {
            live[i] = true;
            unit_queue.push_back(i as u32);
        }
    }
    for &r in roots {
        if !sym_live[r as usize] {
            sym_live[r as usize] = true;
            sym_queue.push_back(r);
        }
    }
    loop {
        while let Some(s) = sym_queue.pop_front() {
            if s == target {
                let mut v = vec![s];
                let mut cur = s;
                while let Some(p) = came_from[cur as usize] {
                    v.push(p);
                    cur = p;
                }
                v.reverse();
                return Some(v);
            }
            for &i in &waiting[s as usize] {
                let i = i as usize;
                let u = &units[i];
                let ok = (u.triggers.is_empty() || u.triggers.iter().any(|&x| sym_live[x as usize]))
                    && u.requirements.iter().all(|&x| sym_live[x as usize]);
                if !live[i] && ok {
                    live[i] = true;
                    unit_queue.push_back(i as u32);
                }
            }
        }
        let i = unit_queue.pop_front()?;
        let u = &units[i as usize];
        let from = u.triggers.first().or(u.requirements.first()).copied();
        for &s in u.refs.iter().chain(u.selectors.iter()) {
            if !sym_live[s as usize] {
                sym_live[s as usize] = true;
                came_from[s as usize] = from;
                sym_queue.push_back(s);
            }
        }
    }
}

/// Redoes the discovery as a dry run and returns the units that should be
/// live and are not — the equivalent of `checkEnqueuerConsistency`
/// (`pkg/compiler/lib/src/enqueue.dart:142-156`). A non-empty result is a
/// defect of the fixed point, not of the compiled program.
pub fn check(units: &[Unit], live: &[bool], total_symbols: usize) -> Vec<usize> {
    // Rebuilds the set of live symbols **only** from the units marked live,
    // without reusing the fixed point's state.
    let mut symbol_live = vec![false; total_symbols];
    for (i, u) in units.iter().enumerate() {
        if !live[i] {
            continue;
        }
        for &s in u.refs.iter().chain(u.selectors.iter()) {
            symbol_live[s as usize] = true;
        }
        for &s in u.triggers.iter().chain(u.requirements.iter()) {
            symbol_live[s as usize] = true;
        }
    }
    let mut missing = Vec::new();
    for (i, u) in units.iter().enumerate() {
        if live[i] {
            continue;
        }
        let ok = (u.triggers.is_empty() || u.triggers.iter().any(|&s| symbol_live[s as usize]))
            && u.requirements.iter().all(|&s| symbol_live[s as usize]);
        if ok && !u.triggers.is_empty() {
            missing.push(i);
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_chain() {
        let mut s = Symbols::default();
        let (a, b, c) = (s.intern("a"), s.intern("b"), s.intern("c"));
        let us = vec![
            Unit { triggers: vec![a], refs: vec![b], ..Default::default() },
            Unit { triggers: vec![b], ..Default::default() },
            Unit { triggers: vec![c], ..Default::default() },
        ];
        let live = solve(&us, &[a], s.total());
        assert_eq!(live, vec![true, true, false], "c is not reached from a");
    }

    #[test]
    fn member_requires_class_and_selector() {
        let mut s = Symbols::default();
        let class = s.intern("core.C");
        let sel_f = s.intern("sel:f");
        let sel_g = s.intern("sel:g");
        let us = vec![
            Unit { triggers: vec![class], ..Default::default() },
            // member f: live because the class is live and `f` is called
            Unit { requirements: vec![class, sel_f], ..Default::default() },
            // member g: the class is live, but nobody calls `g`
            Unit { requirements: vec![class, sel_g], ..Default::default() },
        ];
        let live = solve(&us, &[class, sel_f], s.total());
        assert_eq!(live, vec![true, true, false]);
    }

    #[test]
    fn selector_used_by_live_member_lights_up_another() {
        let mut s = Symbols::default();
        let c = s.intern("core.C");
        let d = s.intern("core.D");
        let sel_f = s.intern("sel:f");
        let sel_h = s.intern("sel:h");
        let us = vec![
            Unit { triggers: vec![c], ..Default::default() },
            Unit { triggers: vec![d], ..Default::default() },
            // C.f uses `x.h()` and mentions D
            Unit { requirements: vec![c, sel_f], refs: vec![d], selectors: vec![sel_h], ..Default::default() },
            // D.h is only live because C.f uses the selector `h`
            Unit { requirements: vec![d, sel_h], ..Default::default() },
        ];
        let live = solve(&us, &[c, sel_f], s.total());
        assert_eq!(live, vec![true, true, true, true]);
    }
}
