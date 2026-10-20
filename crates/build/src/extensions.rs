//! A builder's `buildExtensions`: which outputs an input produces.
//!
//! Reproduces `build-2.4.2/lib/src/generate/expected_outputs.dart`:
//! * suffix: `.dart → [.g.dart]` replaces the end of the path;
//! * `^path`: exact match of the whole path, and the outputs are paths;
//! * `{{name}}`: each group becomes `(.+)` in a regex anchored at the end (and
//!   at the start, with `^`), with the **first** match and greedy groups; the
//!   output replaces the matched span with the output with the groups
//!   substituted.
//!
//! Validations (essentially the same messages): an output with a group when
//! the input has none, a repeated group, an output that does not use every
//! group exactly once, and an output equal to the input.

#[derive(Debug, Clone, PartialEq)]
enum Rule {
    Suffix {
        input: String,
        outputs: Vec<String>,
    },
    Exact {
        path: String,
        outputs: Vec<String>,
    },
    Capture {
        anchored: bool,
        literals: Vec<String>,
        names: Vec<String>,
        outputs: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extensions {
    rules: Vec<Rule>,
    /// The declared form, in order: it goes into the phase's identity.
    pub declared: Vec<(String, Vec<String>)>,
}

/// The `{{name}}` groups of a string: `(start, end, name)`.
fn groups(s: &str) -> Vec<(usize, usize, String)> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            let mut j = i + 2;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j + 1 < b.len() && b[j] == b'}' && b[j + 1] == b'}' {
                v.push((i, j + 2, s[i + 2..j].to_string()));
                i = j + 2;
                continue;
            }
        }
        i += 1;
    }
    v
}

impl Extensions {
    pub fn new_for(
        declared: &[(String, Vec<String>)],
        builder: &str,
    ) -> Result<Extensions, String> {
        let mut rules = Vec::new();
        for (input, outputs) in declared {
            let gs = groups(input);
            if !gs.is_empty() {
                let anchored = input.starts_with('^');
                let mut pos = usize::from(anchored);
                let mut literals = Vec::new();
                let mut names: Vec<String> = Vec::new();
                for (start, end, name) in &gs {
                    if names.contains(name) {
                        return Err(format!(
                            "builder `{builder}` declares the input \"{input}\" with repeated groups (`{{{{{name}}}}}`)"
                        ));
                    }
                    names.push(name.clone());
                    literals.push(input[pos..*start].to_string());
                    pos = *end;
                }
                literals.push(input[pos..].to_string());
                for s in outputs {
                    let mut remaining = names.clone();
                    for (_, _, n) in groups(s) {
                        let Some(i) = remaining.iter().position(|x| *x == n) else {
                            return Err(format!(
                                "builder `{builder}` declares the output \"{s}\" with the group \"{n}\", which does not exist or appears more than once"
                            ));
                        };
                        remaining.remove(i);
                    }
                    if !remaining.is_empty() {
                        return Err(format!(
                            "builder `{builder}` declares the input \"{input}\" with a group, and the output \"{s}\" does not use {}",
                            remaining.join(", ")
                        ));
                    }
                }
                rules.push(Rule::Capture {
                    anchored,
                    literals,
                    names,
                    outputs: outputs.clone(),
                });
                continue;
            }
            for s in outputs {
                if !groups(s).is_empty() {
                    return Err(format!(
                        "builder `{builder}` declares the output \"{s}\" with a group, and the input \"{input}\" has no group"
                    ));
                }
            }
            if let Some(c) = input.strip_prefix('^') {
                rules.push(Rule::Exact {
                    path: c.to_string(),
                    outputs: outputs.clone(),
                });
            } else {
                rules.push(Rule::Suffix {
                    input: input.clone(),
                    outputs: outputs.clone(),
                });
            }
        }
        Ok(Extensions {
            rules,
            declared: declared.to_vec(),
        })
    }

    pub fn has_output(&self, path: &str) -> bool {
        self.rules.iter().any(|r| match r {
            Rule::Suffix { input, .. } => path.ends_with(input.as_str()),
            Rule::Exact { path: c, .. } => c == path,
            Rule::Capture {
                anchored, literals, ..
            } => first_match(path, *anchored, literals).is_some(),
        })
    }

    /// `expectedOutputs(builder, input)`: paths (in the same package), in the
    /// order of the rules and of the outputs. An output equal to the input is
    /// an error.
    pub fn outputs(&self, path: &str) -> Result<Vec<String>, String> {
        let mut v = Vec::new();
        for r in &self.rules {
            match r {
                Rule::Suffix { input, outputs } => {
                    if let Some(base) = path.strip_suffix(input.as_str()) {
                        v.extend(outputs.iter().map(|s| format!("{base}{s}")));
                    }
                }
                Rule::Exact { path: c, outputs } => {
                    if c == path {
                        v.extend(outputs.iter().cloned());
                    }
                }
                Rule::Capture {
                    anchored,
                    literals,
                    names,
                    outputs,
                } => {
                    if let Some((start, caps)) = first_match(path, *anchored, literals) {
                        for s in outputs {
                            let mut r = String::new();
                            let mut pos = 0;
                            for (a, b, n) in groups(s) {
                                r.push_str(&s[pos..a]);
                                let i = names.iter().position(|x| *x == n).unwrap_or(0);
                                r.push_str(&caps[i]);
                                pos = b;
                            }
                            r.push_str(&s[pos..]);
                            v.push(format!("{}{r}", &path[..start]));
                        }
                    }
                }
            }
        }
        if v.iter().any(|s| s == path) {
            return Err(format!(
                "the output \"{path}\" is equal to the input, which is not allowed"
            ));
        }
        Ok(v)
    }

    /// All declared outputs (for `required_inputs`).
    pub fn all_outputs(&self) -> impl Iterator<Item = &String> {
        self.declared.iter().flat_map(|(_, s)| s)
    }
}

/// First match of `lit0(.+)lit1(.+)…litN$` (with `^` if anchored): the
/// leftmost start, greedy groups. Returns the start and the groups.
fn first_match(t: &str, anchored: bool, literals: &[String]) -> Option<(usize, Vec<String>)> {
    let boundaries: Vec<usize> = t
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(t.len()))
        .collect();
    let starts: Vec<usize> = if anchored {
        vec![0]
    } else {
        boundaries.clone()
    };
    for start in starts {
        let mut caps = Vec::new();
        if match_from(t, &boundaries, start, literals, 0, &mut caps) {
            return Some((start, caps));
        }
    }
    None
}

fn match_from(
    t: &str,
    fr: &[usize],
    pos: usize,
    lits: &[String],
    k: usize,
    caps: &mut Vec<String>,
) -> bool {
    let lit = &lits[k];
    if !t[pos..].starts_with(lit.as_str()) {
        return false;
    }
    let pos = pos + lit.len();
    if k + 1 == lits.len() {
        return pos == t.len();
    }
    // Greedy `(.+)`: the longest span first, at least one character.
    for &end in fr.iter().rev() {
        if end <= pos {
            break;
        }
        caps.push(t[pos..end].to_string());
        if match_from(t, fr, end, lits, k + 1, caps) {
            return true;
        }
        caps.pop();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(pairs: &[(&str, &[&str])]) -> Result<Extensions, String> {
        let v: Vec<(String, Vec<String>)> = pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.iter().map(|s| s.to_string()).collect()))
            .collect();
        Extensions::new_for(&v, "test")
    }

    // Cases from `build-2.4.2/test/generate/expected_outputs_test.dart`.
    #[test]
    fn suffix() {
        let e = ext(&[(".dart", &[".g.dart", ".json"])]).unwrap();
        assert_eq!(
            e.outputs("lib/a.dart").unwrap(),
            vec!["lib/a.g.dart", "lib/a.json"]
        );
        assert!(e.outputs("lib/a.txt").unwrap().is_empty());
        assert!(e.has_output("lib/a.dart"));
    }

    #[test]
    fn exact() {
        let e = ext(&[("^pubspec.yaml", &["lib/version.dart"])]).unwrap();
        assert_eq!(e.outputs("pubspec.yaml").unwrap(), vec!["lib/version.dart"]);
        assert!(e.outputs("x/pubspec.yaml").unwrap().is_empty());
    }

    #[test]
    fn capture() {
        let e = ext(&[("{{}}.dart", &["{{}}.g.dart"])]).unwrap();
        assert_eq!(e.outputs("lib/a.dart").unwrap(), vec!["lib/a.g.dart"]);
        let e = ext(&[("^lib/{{}}.dart", &["lib/generated/{{}}.dart"])]).unwrap();
        assert_eq!(
            e.outputs("lib/a/b.dart").unwrap(),
            vec!["lib/generated/a/b.dart"]
        );
        assert!(e.outputs("test/a.dart").unwrap().is_empty());
        let e = ext(&[(
            "{{dir}}/models/{{file}}.dart",
            &["{{dir}}/gen/{{file}}.g.dart"],
        )])
        .unwrap();
        assert_eq!(
            e.outputs("lib/src/models/p.dart").unwrap(),
            vec!["lib/src/gen/p.g.dart"]
        );
    }

    #[test]
    fn validations() {
        assert!(ext(&[(".dart", &["{{}}.g.dart"])]).is_err());
        assert!(ext(&[("{{a}}{{a}}.dart", &["{{a}}.g"])]).is_err());
        assert!(ext(&[("{{a}}.dart", &["x.g"])]).is_err());
        assert!(ext(&[("{{a}}.dart", &["{{a}}{{a}}.g"])]).is_err());
        let e = ext(&[("^a", &["a"])]).unwrap();
        assert!(e.outputs("a").is_err());
    }
}
