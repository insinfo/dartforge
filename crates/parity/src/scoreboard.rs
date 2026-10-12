//! Comparison with the oracle and per-code scoreboard.
//!
//! Key: `(file, code, UTF-16 offset, length)`. A pair with the same key is a
//! **hit**; among hits, a different severity, message or correction counts as
//! a **wrong message**. What is left on each side, in the same file and with
//! the same code, is paired in offset order as a **wrong position**; the rest
//! is a **false positive** (ours only) or a **false negative** (oracle only).
//! Everything is in `BTreeMap`s: the scoreboard text does not depend on the
//! order in which the batches finished.

use crate::oraculo::Record;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Divergence samples per (code, category): `DARTFORGE_PARIDADE_AMOSTRAS`, 3 by default.
fn sample_limit() -> usize {
    std::env::var("DARTFORGE_PARIDADE_AMOSTRAS").ok().and_then(|v| v.parse().ok()).unwrap_or(3)
}

/// The counters of one code.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub oracle: usize,
    pub ours: usize,
    pub hits: usize,
    pub wrong_message: usize,
    pub wrong_position: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
}

impl Count {
    pub fn add(&mut self, o: &Count) {
        self.oracle += o.oracle;
        self.ours += o.ours;
        self.hits += o.hits;
        self.wrong_message += o.wrong_message;
        self.wrong_position += o.wrong_position;
        self.false_positives += o.false_positives;
        self.false_negatives += o.false_negatives;
    }

    /// Exact hit (position and message) in every case, with at least one.
    pub fn perfect(&self) -> bool {
        self.oracle > 0 && self.hits == self.oracle && self.ours == self.oracle && self.wrong_message == 0
    }

    /// Everything we emit matches the oracle (position, severity and
    /// message), with at least one hit: no false positive, no wrong position,
    /// no wrong message. False negatives are allowed — this is the publication
    /// criterion (`verificados.txt`): what the user sees is right, even if
    /// some diagnostic is missing.
    pub fn no_emitted_error(&self) -> bool {
        self.hits > 0 && self.false_positives == 0 && self.wrong_position == 0 && self.wrong_message == 0
    }
}

/// Per-code scoreboard, and divergence samples per code.
#[derive(Debug, Default, Clone)]
pub struct Scoreboard {
    pub by_code: BTreeMap<String, Count>,
    /// Up to 3 examples per (code, category): `file:line:column text`.
    pub samples: BTreeMap<(String, &'static str), Vec<String>>,
}

impl Scoreboard {
    pub fn total(&self) -> Count {
        let mut t = Count::default();
        for c in self.by_code.values() {
            t.add(c);
        }
        t
    }

    pub fn add(&mut self, other: &Scoreboard) {
        for (k, c) in &other.by_code {
            self.by_code.entry(k.clone()).or_default().add(c);
        }
        for (k, v) in &other.samples {
            let d = self.samples.entry(k.clone()).or_default();
            for s in v {
                if d.len() < sample_limit() {
                    d.push(s.clone());
                }
            }
        }
    }

    fn sample(&mut self, code: &str, cat: &'static str, r: &Record, extra: &str) {
        let v = self.samples.entry((code.to_string(), cat)).or_default();
        if v.len() < sample_limit() {
            v.push(format!("{}:{}:{} {}{extra}", r.arquivo, r.line, r.column, r.problem_message));
        }
    }

    /// Compares the diagnostics of a group (both sides relative to the same root).
    pub fn compare(&mut self, oracle: &[Record], ours: &[Record]) {
        // (file, code) → each side's records.
        type Sides<'r> = (Vec<&'r Record>, Vec<&'r Record>);
        let mut sides: BTreeMap<(&str, &str), Sides<'_>> = BTreeMap::new();
        for r in oracle {
            sides.entry((&r.arquivo, &r.code)).or_default().0.push(r);
        }
        for r in ours {
            sides.entry((&r.arquivo, &r.code)).or_default().1.push(r);
        }
        for ((_, code), (mut o, mut n)) in sides {
            o.sort();
            n.sort();
            let mut count = Count { oracle: o.len(), ours: n.len(), ..Count::default() };
            let mut rest_o = Vec::new();
            let mut used = vec![false; n.len()];
            for ro in &o {
                let found = (0..n.len()).find(|&i| !used[i] && n[i].offset == ro.offset && n[i].length == ro.length);
                match found {
                    Some(i) => {
                        used[i] = true;
                        count.hits += 1;
                        let rn = n[i];
                        if rn.severity != ro.severity
                            || rn.problem_message != ro.problem_message
                            || rn.correction_message != ro.correction_message
                        {
                            count.wrong_message += 1;
                            let extra = format!("  <> ours: {}", rn.problem_message);
                            self.sample(code, "message", ro, &extra);
                        }
                    }
                    None => rest_o.push(*ro),
                }
            }
            let rest_n: Vec<&Record> = (0..n.len()).filter(|&i| !used[i]).map(|i| n[i]).collect();
            let pairs = rest_o.len().min(rest_n.len());
            count.wrong_position = pairs;
            for k in 0..pairs {
                let extra = format!("  <> ours {}:{} (+{})", rest_n[k].line, rest_n[k].column, rest_n[k].length);
                self.sample(code, "position", rest_o[k], &extra);
            }
            count.false_negatives = rest_o.len() - pairs;
            count.false_positives = rest_n.len() - pairs;
            for r in &rest_o[pairs..] {
                self.sample(code, "FN", r, "");
            }
            for r in &rest_n[pairs..] {
                self.sample(code, "FP", r, "");
            }
            self.by_code.entry(code.to_string()).or_default().add(&count);
        }
    }

    /// Per-code table (alphabetical order), with the total line.
    ///
    /// The `código` column header is matched by `scripts/comparar-placares.py`.
    pub fn table(&self, title: &str) -> String {
        let mut s = String::new();
        let t = self.total();
        let _ = writeln!(
            s,
            "{title}: oracle {} | ours {} | hits {} (wrong message {}) | wrong position {} | FP {} | FN {}",
            t.oracle, t.ours, t.hits, t.wrong_message, t.wrong_position, t.false_positives, t.false_negatives
        );
        let perfect = self.by_code.values().filter(|c| c.perfect()).count();
        let with_cases = self.by_code.values().filter(|c| c.oracle > 0).count();
        let _ = writeln!(s, "codes at 100% (position and message): {perfect} of {with_cases} with cases in the oracle");
        let _ = writeln!(s, "{:<58} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6}", "código", "oracle", "ours", "hit", "msg≠", "pos≠", "FP", "FN");
        for (k, c) in &self.by_code {
            let _ = writeln!(
                s,
                "{:<58} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6}{}",
                k,
                c.oracle,
                c.ours,
                c.hits,
                c.wrong_message,
                c.wrong_position,
                c.false_positives,
                c.false_negatives,
                if c.perfect() { "  100%" } else { "" }
            );
        }
        s
    }

    /// The divergence samples, per code and category.
    pub fn samples_text(&self) -> String {
        let mut s = String::new();
        for ((c, cat), v) in &self.samples {
            for a in v {
                let _ = writeln!(s, "  [{c}] {cat}: {a}");
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(file: &str, code: &str, off: usize, len: usize, msg: &str) -> Record {
        Record {
            arquivo: file.into(),
            offset: off,
            length: len,
            code: code.into(),
            severity: "ERROR".into(),
            tipo: "COMPILE_TIME_ERROR".into(),
            line: 1,
            column: off + 1,
            problem_message: msg.into(),
            correction_message: None,
        }
    }

    #[test]
    fn categories() {
        let o = vec![r("a", "x", 1, 1, "m"), r("a", "x", 5, 1, "m"), r("a", "y", 2, 2, "m"), r("a", "z", 3, 1, "m")];
        let n = vec![r("a", "x", 1, 1, "other"), r("a", "x", 9, 1, "m"), r("a", "w", 0, 1, "m"), r("a", "z", 3, 1, "m")];
        let mut p = Scoreboard::default();
        p.compare(&o, &n);
        let x = p.by_code["x"];
        assert_eq!((x.hits, x.wrong_message, x.wrong_position), (1, 1, 1));
        assert_eq!(p.by_code["y"].false_negatives, 1);
        assert_eq!(p.by_code["w"].false_positives, 1);
        assert!(p.by_code["z"].perfect());
        assert!(!x.perfect());
    }
}
