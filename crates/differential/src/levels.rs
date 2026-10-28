//! The production profile's optimization level (`--level N`: `dartforge-jsprod
//! -O<N>`, with `dart2js`'s meanings, `docs/JS-PRODUCAO-TAMANHO.md` §21) and
//! the reviewed list of the corpus programs `-O3`/`-O4` exclude.
//!
//! `-O3` trusts implicit downcasts and parameters and `-O4` also index bounds,
//! as `dart2js`: a program whose output depends on one of those checks failing
//! (it catches the `TypeError` or `RangeError`, or prints it) is no longer
//! comparable with the VM there. `production_levels.tsv` names each one with
//! the level from which it is excluded, the check and what the program does
//! with it; every other program must still match the VM. An excluded program
//! that matches is reported (the list must stay exact).

use std::sync::OnceLock;

static LEVEL: OnceLock<u8> = OnceLock::new();

/// Sets the level (once, before running the corpus).
pub fn set_level(level: u8) {
    let _ = LEVEL.set(level);
}

/// The level the production profile runs at (`-O2` unless `--level` said
/// otherwise).
pub fn level() -> u8 {
    LEVEL.get().copied().unwrap_or(2)
}

/// One reviewed exclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exclusion {
    /// The lowest level that drops the check.
    pub level: u8,
    pub program: &'static str,
    /// The check the program relies on.
    pub check: &'static str,
    /// What the program does with it.
    pub reason: &'static str,
}

const LIST: &str = include_str!("../production_levels.tsv");

/// Every entry of `production_levels.tsv`.
///
/// ```
/// let all = dartforge_differential::levels::exclusions();
/// assert!(all.iter().all(|e| e.level == 3 || e.level == 4));
/// ```
pub fn exclusions() -> Vec<Exclusion> {
    LIST.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut f = l.splitn(4, '\t');
            Some(Exclusion {
                level: f.next()?.trim().parse().ok()?,
                program: f.next()?.trim(),
                check: f.next()?.trim(),
                reason: f.next().unwrap_or("").trim(),
            })
        })
        .collect()
}

/// The exclusion of `program` at the current level, if any.
pub fn excluded(program: &str) -> Option<Exclusion> {
    let level = level();
    exclusions()
        .into_iter()
        .find(|e| e.program == program && e.level <= level)
}
