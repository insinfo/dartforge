//! Per-resume-body ABI of compiler-generated asynchronous frames.
//!
//! The shared frame CID is not a nominal Dart class layout. Each suspended frame
//! keeps its own fingerprinted resume entry; that entry must retain its field ABI.
use std::collections::{BTreeMap, HashMap};

pub(crate) const ASYNC_FRAME_ID: i64 = 0x3fff_ff00;
pub(crate) type BodyLayouts = BTreeMap<String, i64>;

/// Separates verified generated-frame allocation sites from nominal allocations.
/// Missing metadata always leaves an allocation under the nominal contract.
pub(crate) fn classify(
    ir: &str,
    allocations: &[(String, String, i64, i64)],
    definitions: &[String],
    previous: &BodyLayouts,
) -> Result<(Vec<(i64, i64)>, BodyLayouts), String> {
    let mut sites = HashMap::new();
    for line in ir.lines() {
        let Some(rest) = line.strip_prefix("; df.async-frame ") else { continue };
        let parts: Vec<_> = rest.split_whitespace().collect();
        let malformed = || "invalid compiler-generated async-frame ABI metadata".to_string();
        if parts.len() != 4 { return Err(malformed()) }
        let size = parts[3].parse::<i64>().map_err(|_| malformed())?;
        let body = parts[2];
        let fingerprint = body.strip_suffix("$ent").and_then(|s| s.rsplit_once("$q")).map(|(_, s)| s);
        if size < 6 || !fingerprint.is_some_and(|s| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(malformed());
        }
        if sites.insert((parts[0], parts[1]), (body, size)).is_some() { return Err(malformed()) }
    }
    let mut nominal = Vec::new();
    let mut bodies = BodyLayouts::new();
    for (owner, value, cid, size) in allocations {
        let proof = sites.get(&(owner.as_str(), value.as_str())).filter(|(body, expected)| {
            *cid == ASYNC_FRAME_ID && size == expected && (definitions.iter().any(|symbol| symbol == body) || previous.get(*body) == Some(expected))
        });
        if let Some(&(body, size)) = proof {
            if bodies.insert(body.to_string(), size).is_some_and(|old| old != size) {
                return Err(format!("the async resume body {body} has conflicting frame layouts"));
            }
        } else {
            nominal.push((*cid, *size));
        }
    }
    nominal.sort_unstable();
    nominal.dedup();
    Ok((nominal, bodies))
}

/// A reserved CID without the compiler's per-body evidence must not acquire or
/// lose that evidence across generations while old objects may remain alive.
pub(crate) fn check_domains(previous: &[(i64, i64)], previous_frames: &BodyLayouts, new: &[(i64, i64)], new_frames: &BodyLayouts) -> Result<(), String> {
    if (previous.iter().any(|(cid, _)| *cid == ASYNC_FRAME_ID) && !new_frames.is_empty())
        || (!previous_frames.is_empty() && new.iter().any(|(cid, _)| *cid == ASYNC_FRAME_ID)) {
        return Err("async-frame allocations require consistent per-body ABI metadata across reloads; restart the session".to_string());
    }
    Ok(())
}

/// Old suspended frames retain their old entry. Reusing that same entry with a
/// different frame size would corrupt them, even if other bodies changed safely.
pub(crate) fn check(previous: &BodyLayouts, new: &BodyLayouts) -> Result<(), String> {
    for (body, size) in previous {
        if let Some(next) = new.get(body).filter(|next| *next != size) {
            return Err(format!("the async resume body {body} changed its frame from {size} to {next} fields; restart the session"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn allocations() -> Vec<(String, String, i64, i64)> {
        vec![("start_a".into(), "v0".into(), ASYNC_FRAME_ID, 6), ("start_b".into(), "v1".into(), ASYNC_FRAME_ID, 7)]
    }
    #[test]
    fn heterogeneous_generated_frames_are_not_nominal_class_layouts() {
        let ir = "; df.async-frame start_a v0 a$q01234567$ent 6\n; df.async-frame start_b v1 b$qabcdef01$ent 7\n";
        let symbols = vec!["a$q01234567$ent".into(), "b$qabcdef01$ent".into()];
        let (nominal, old) = classify(ir, &allocations(), &symbols, &BodyLayouts::new()).unwrap();
        assert!(nominal.is_empty());
        assert_eq!(old.len(), 2);
        assert!(check(&old, &old).is_ok());
        let new = BTreeMap::from([("a$q76543210$ent".into(), 12)]);
        assert!(check(&old, &new).is_ok());
        let changed = BTreeMap::from([("a$q01234567$ent".into(), 7)]);
        assert!(check(&old, &changed).unwrap_err().contains("6 to 7"));
    }
    #[test]
    fn unmarked_or_mismatched_reserved_cid_stays_nominal() {
        let symbols = vec!["a$q01234567$ent".into()];
        for ir in ["", "; df.async-frame start_a v0 a$q01234567$ent 9\n", "; df.async-frame another v0 a$q01234567$ent 6\n"] {
            assert_eq!(classify(ir, &allocations(), &symbols, &BodyLayouts::new()).unwrap().0, vec![(ASYNC_FRAME_ID, 6), (ASYNC_FRAME_ID, 7)]);
        }
        assert_eq!(classify("; df.async-frame start_a v0 a$q01234567$ent 6\n", &allocations(), &[], &BodyLayouts::new()).unwrap().0.len(), 2);
    }
    #[test]
    fn same_owner_and_size_do_not_certify_a_different_allocation_site() {
        let ir = "; df.async-frame start_a v0 a$q01234567$ent 6\n";
        let allocations = vec![("start_a".into(), "v99".into(), ASYNC_FRAME_ID, 6)];
        let definitions = vec!["a$q01234567$ent".into()];
        let (nominal, frames) = classify(ir, &allocations, &definitions, &BodyLayouts::new()).unwrap();
        assert_eq!(nominal, vec![(ASYNC_FRAME_ID, 6)]);
        assert!(frames.is_empty());
    }

    #[test]
    fn declared_resume_requires_the_exact_previously_certified_abi() {
        let ir = "; df.async-frame start_a v0 a$q01234567$ent 6\n";
        let allocations = vec![("start_a".into(), "v0".into(), ASYNC_FRAME_ID, 6)];
        let empty = BodyLayouts::new();
        let (nominal, frames) = classify(ir, &allocations, &[], &empty).unwrap();
        assert_eq!(nominal, vec![(ASYNC_FRAME_ID, 6)]);
        assert!(frames.is_empty(), "an unproven imported declaration is not a body ABI");
        let prior = BTreeMap::from([("a$q01234567$ent".into(), 6)]);
        let (nominal, frames) = classify(ir, &allocations, &[], &prior).unwrap();
        assert!(nominal.is_empty());
        assert_eq!(frames, prior, "delta-kept bodies retain their exact ABI");
        let wrong = BTreeMap::from([("a$q01234567$ent".into(), 7)]);
        let (nominal, frames) = classify(ir, &allocations, &[], &wrong).unwrap();
        assert_eq!(nominal, vec![(ASYNC_FRAME_ID, 6)]);
        assert!(check_domains(&[], &wrong, &nominal, &frames).is_err());
    }

    #[test]
    fn nominal_and_generated_reserved_cid_cannot_change_domains() {
        let frames = BTreeMap::from([("a$q01234567$ent".into(), 6)]);
        let empty = BodyLayouts::new();
        assert!(check_domains(&[(ASYNC_FRAME_ID, 6)], &empty, &[], &frames).is_err());
        assert!(check_domains(&[], &frames, &[(ASYNC_FRAME_ID, 7)], &empty).is_err());
        assert!(check_domains(&[(123, 6)], &empty, &[], &frames).is_ok());
    }

    #[test]
    fn duplicate_and_malformed_markers_fail_closed() {
        for ir in ["; df.async-frame owner v0 body 6", "; df.async-frame owner v0 a$q01234567$ent -1", "; df.async-frame owner v0 a$q01234567$ent 6 extra", "; df.async-frame owner v0 a$q01234567$ent 6\n; df.async-frame owner v0 a$q01234567$ent 6"] {
            assert!(classify(ir, &[], &[], &BodyLayouts::new()).is_err(), "{ir}");
        }
    }
}
