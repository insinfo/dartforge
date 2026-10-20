//! Bounded symbolic subobjects of local stack allocations, not byte offsets.
//! <https://llvm.org/docs/LangRef.html#alloca-instruction>
//! <https://llvm.org/docs/GetElementPtr.html>
use super::read_only;
use std::collections::{HashMap, HashSet};

const MAX_UNITS: u64 = 1 << 20;

#[derive(Clone, PartialEq, Eq)]
enum Ty {
    Atom(&'static str),
    Array(u64, Box<Ty>),
    Struct(Vec<Ty>),
}
impl Ty {
    fn units(&self) -> Option<u64> {
        let units = match self {
            Self::Atom(_) => 1,
            Self::Array(n, ty) => n.checked_mul(ty.units()?)?,
            Self::Struct(fields) => fields
                .iter()
                .try_fold(0u64, |n, ty| n.checked_add(ty.units()?))?,
        };
        (units <= MAX_UNITS).then_some(units)
    }
}
fn ty(tokens: &[&str], at: &mut usize, depth: usize) -> Option<Ty> {
    if depth > 16 {
        return None;
    }
    let token = *tokens.get(*at)?;
    *at += 1;
    let atom = match token {
        "i1" => Some("i1"),
        "i8" => Some("i8"),
        "i16" => Some("i16"),
        "i32" => Some("i32"),
        "i64" => Some("i64"),
        "ptr" => Some("ptr"),
        "float" => Some("float"),
        "double" => Some("double"),
        _ => None,
    };
    if let Some(atom) = atom {
        return Some(Ty::Atom(atom));
    }
    let result = match token {
        "[" => {
            let n = tokens.get(*at)?.parse::<u64>().ok()?;
            if n > MAX_UNITS || tokens.get(*at + 1) != Some(&"x") {
                return None;
            }
            *at += 2;
            let child = ty(tokens, at, depth + 1)?;
            if tokens.get(*at) != Some(&"]") {
                return None;
            }
            *at += 1;
            Ty::Array(n, Box::new(child))
        }
        "{" => {
            let mut fields = Vec::new();
            while tokens.get(*at) != Some(&"}") {
                if fields.len() == 64 {
                    return None;
                }
                if !fields.is_empty() {
                    if tokens.get(*at) != Some(&",") {
                        return None;
                    }
                    *at += 1;
                }
                fields.push(ty(tokens, at, depth + 1)?);
            }
            *at += 1;
            Ty::Struct(fields)
        }
        _ => return None,
    };
    result.units()?;
    Some(result)
}
fn ssa(token: &str) -> Option<String> {
    if !read_only::assignment(token) {
        return None;
    }
    let raw = token.strip_prefix('%')?;
    let raw = if raw.starts_with('"') {
        raw.strip_prefix('"')?.strip_suffix('"')?
    } else {
        raw
    };
    super::table_getters::canonical_symbol(raw)
}
fn alignment(tokens: &[&str]) -> bool {
    tokens.is_empty()
        || matches!(tokens, [",", "align", n] if n.parse::<u32>().is_ok_and(u32::is_power_of_two))
}
fn index(ty: &str, value: &str) -> Option<u64> {
    let max = match ty {
        "i32" => i32::MAX as u64,
        "i64" => i64::MAX as u64,
        _ => return None,
    };
    value.parse::<u64>().ok().filter(|n| *n <= max)
}
#[derive(Clone)]
struct Region {
    ty: Ty,
    remaining: u64,
}
fn allocated(tokens: &[&str]) -> Option<Region> {
    if tokens.first() != Some(&"alloca") {
        return None;
    }
    let mut at = 1;
    let ty = ty(tokens, &mut at, 0)?;
    let mut count = 1;
    if tokens.get(at) == Some(&",") && matches!(tokens.get(at + 1), Some(&"i32" | &"i64")) {
        count = index(tokens[at + 1], tokens.get(at + 2)?)?;
        at += 3;
    }
    if count == 0 || count.checked_mul(ty.units()?)? > MAX_UNITS || !alignment(&tokens[at..]) {
        return None;
    }
    Some(Region {
        ty,
        remaining: count,
    })
}
fn derived(tokens: &[&str], regions: &HashMap<String, Region>) -> Option<Region> {
    if tokens.first() != Some(&"getelementptr") {
        return None;
    }
    let mut at = 1 + usize::from(tokens.get(1) == Some(&"inbounds"));
    let source = ty(tokens, &mut at, 0)?;
    if tokens.get(at..at + 2)? != [",", "ptr"] {
        return None;
    }
    let mut region = regions.get(&ssa(tokens.get(at + 2)?)?)?.clone();
    at += 3;
    // Opaque pointers also permit a direct array-to-element GEP. Keep the
    // capability within the first array, never reinterpret unrelated layouts.
    if source != region.ty {
        let Ty::Array(n, element) = &region.ty else {
            return None;
        };
        if source != **element {
            return None;
        }
        region = Region {
            ty: source,
            remaining: *n,
        };
    }
    let [",", first_ty, first] = tokens.get(at..at + 3)? else {
        return None;
    };
    let first = index(first_ty, first)?;
    if first >= region.remaining {
        return None;
    }
    region.remaining -= first;
    at += 3;
    while at < tokens.len() {
        let [",", index_ty, number] = tokens.get(at..at + 3)? else {
            return None;
        };
        let number = index(index_ty, number)?;
        region = match region.ty {
            Ty::Array(n, element) if number < n => Region {
                ty: *element,
                remaining: n - number,
            },
            Ty::Struct(fields) if *index_ty == "i32" => Region {
                ty: fields.get(usize::try_from(number).ok()?)?.clone(),
                remaining: 1,
            },
            _ => return None,
        };
        at += 3;
    }
    Some(region)
}
fn stored(tokens: &[&str], regions: &HashMap<String, Region>) -> bool {
    let start = 1 + usize::from(tokens.get(1) == Some(&"volatile"));
    let Some([scalar, value, ",", "ptr", destination]) = tokens.get(start..start + 5) else {
        return false;
    };
    let Some(Ty::Atom(atom)) = ty(&[*scalar], &mut 0, 0) else {
        return false;
    };
    read_only::operand(value, atom)
        && ssa(destination)
            .and_then(|name| regions.get(&name))
            .is_some_and(|region| region.ty == Ty::Atom(atom) && region.remaining > 0)
        && alignment(&tokens[start + 5..])
}

/// Only complete, bounded typed stack addresses and writes gain observations.
/// Unknown destinations, SSA identities and budget exhaustion grant nothing.
pub(super) fn observe(body: &[&str], parameters: &[String]) -> HashSet<usize> {
    if body.len() > 8192 || !body.iter().any(|line| line.contains("alloca")) {
        return HashSet::new();
    }
    let mut seen = HashSet::new();
    for parameter in parameters {
        let Some(tokens) = read_only::tokens(parameter) else {
            return HashSet::new();
        };
        let [_, name] = tokens.as_slice() else {
            return HashSet::new();
        };
        let Some(name) = ssa(name) else {
            return HashSet::new();
        };
        if !seen.insert(name) {
            return HashSet::new();
        }
    }
    let mut lines = Vec::new();
    let mut budget = 65536usize;
    for (index, raw) in body.iter().enumerate() {
        let Some(line) = super::uncomment(raw) else {
            return HashSet::new();
        };
        let line = line.trim();
        if line.is_empty() || line.ends_with(':') {
            continue;
        }
        let (name, operation) = if let Some((left, operation)) = line.split_once(" = ") {
            let Some(name) = ssa(left.trim()) else {
                return HashSet::new();
            };
            if !seen.insert(name.clone()) {
                return HashSet::new();
            }
            (Some(name), operation.trim())
        } else {
            (None, line)
        };
        let Some(tokens) = read_only::tokens(operation) else {
            return HashSet::new();
        };
        if tokens.len() > 1024 || tokens.len() > budget {
            return HashSet::new();
        }
        budget -= tokens.len();
        lines.push((index, name, tokens));
    }
    let mut regions = HashMap::new();
    let mut accepted = HashSet::new();
    for (index, name, tokens) in lines {
        if let Some(name) = name {
            if let Some(region) = allocated(&tokens).or_else(|| derived(&tokens, &regions)) {
                if tokens.first() == Some(&"getelementptr") {
                    accepted.insert(index);
                }
                regions.insert(name, region);
            }
        } else if tokens.first() == Some(&"store") && stored(&tokens, &regions) {
            accepted.insert(index);
        }
    }
    accepted
}
