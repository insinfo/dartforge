//! `LeakDetectionValidator`: the secret patterns of Pub 3.6.2
//! (`validator/leak_detection.dart`), their allow lists and entropy
//! thresholds, and the errors it reports.
//!
//! Pub decodes each file as ASCII with invalid bytes allowed (each byte
//! above 127 becomes U+FFFD, one UTF-16 unit), so the patterns run on the
//! raw bytes with Unicode disabled: a byte above 127 then matches exactly
//! the classes U+FFFD matches (only negated ones), and offsets are the
//! same as Dart's.
use super::files::IGNORE_CASE;
use super::ignore::Ignore;
use super::paths as p;
use super::span::SourceFile;
use super::validators::{Context, Found, false_secrets};
use crate::pub_get::Failure;
use regex::bytes::{Regex, RegexBuilder};
use std::sync::OnceLock;

/// An allowed match: a substring or a pattern.
enum Allowed {
    Text(&'static str),
    Pattern(Regex),
}

struct LeakPattern {
    kind: &'static str,
    pattern: Regex,
    allowed: Vec<Allowed>,
    /// `(group, threshold)`.
    entropy: Option<(usize, f64)>,
}

fn compile(pattern: &str) -> Regex {
    RegexBuilder::new(&format!("(?-u){pattern}"))
        .size_limit(256 << 20)
        .dfa_size_limit(64 << 20)
        .build()
        .expect("leak pattern")
}

const PEM_WSP: &str = r"(?:\\r|\\n|\\t|\s)*";
const PEM_LINE_BREAK: &str = r"\s*(?:\\r|\\n|\r|\n)\s*";

fn pem_begin(label: &str) -> String {
    format!("-----BEGIN {label}-----{PEM_LINE_BREAK}{PEM_WSP}")
}

fn pem_base64_block() -> String {
    format!(
        "(?:(?:[a-zA-Z0-9+/]{PEM_WSP}){{4}})+(?:((?:[a-zA-Z0-9+/]{PEM_WSP}){{3}}={PEM_WSP})|(?:(?:[a-zA-Z0-9+/]{PEM_WSP}){{2}}(?:={PEM_WSP}){{2}}))?"
    )
}

fn pem_end(label: &str) -> String {
    format!("{PEM_LINE_BREAK}{PEM_WSP}-----END {label}-----")
}

fn pem_key(label: &str) -> String {
    format!(
        "{}{}{}",
        pem_begin(label),
        pem_base64_block(),
        pem_end(label)
    )
}

fn patterns() -> &'static [LeakPattern] {
    static PATTERNS: OnceLock<Vec<LeakPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let simple = |kind, pattern: &str, threshold: f64| LeakPattern {
            kind,
            pattern: compile(pattern),
            allowed: Vec::new(),
            entropy: Some((1, threshold)),
        };
        let pem = |kind, label: &str| LeakPattern {
            kind,
            pattern: compile(&pem_key(label)),
            allowed: Vec::new(),
            entropy: None,
        };
        vec![
            LeakPattern {
                kind: "AWS Access Key",
                pattern: compile(
                    r"[^A-Z0-9]((?:A3T[A-Z0-9]|AKIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[A-Z0-9]{12,128})[^A-Z0-9]",
                ),
                allowed: vec![
                    Allowed::Text("AKIAIOSFODNN7EXAMPLE"),
                    Allowed::Text("Q3AM3UQ867SPQQA43P2F"),
                ],
                entropy: Some((1, 3.32)),
            },
            LeakPattern {
                kind: "AWS Secret Key",
                pattern: compile(
                    r#"(?:"|')?(?:AWS|aws|Aws)?_?(?:SECRET|secret|Secret)?_?(?:ACCESS|access|Access)?_?(?:KEY|key|Key)(?:"|')?\s*(?::|=>|=)\s*(?:"|')?([A-Za-z0-9/\+=]{40})(?:"|')"#,
                ),
                allowed: vec![
                    Allowed::Text(concat!("wJalrXUtnFEMI/K7MDENG/", "bPxRfiCYEXAMPLEKEY")),
                    Allowed::Text(concat!("zuf+tfteSlswRu7BJ86w", "ekitnifILbZam1KYY3TG")),
                    Allowed::Pattern(compile(
                        r#"(:|=>|=)\s*("|')?[A-Fa-f0-9][A-Fa-f0-9x][A-Fa-f0-9]{38}("|')?"#,
                    )),
                ],
                entropy: Some((1, 4.54)),
            },
            simple(
                "Google API Key",
                r"[^0-9A-Za-z\-_](AIza[0-9A-Za-z\-_]{35})[^0-9A-Za-z\-_]",
                4.25,
            ),
            simple(
                "Google OAuth ID",
                r"[0-9]+-([0-9A-Za-z_]{32})\.apps\.googleusercontent\.com",
                3.54,
            ),
            simple(
                "Google OAuth Refresh Token",
                r"[^0-9A-Za-z_\-/+&](1//?[0-9A-Za-z_-]{28,255})[^0-9A-Za-z_\-/+=]",
                5.0,
            ),
            simple(
                "Google OAuth Access Token",
                r"[^0-9A-Za-z._-]ya29\.([0-9A-Za-z_-]{30,255})[^0-9A-Za-z._-]",
                5.0,
            ),
            pem("Private Key", "PRIVATE KEY"),
            pem("RSA Private Key", "RSA PRIVATE KEY"),
            pem("EC Private Key", "EC PRIVATE KEY"),
            LeakPattern {
                kind: "PGP Private Key",
                pattern: compile(&format!(
                    "{}(?:\\w+: [^\\n]{{1,1024}}{PEM_LINE_BREAK}{PEM_WSP})*{}{PEM_LINE_BREAK}{PEM_WSP}=(?:(?:[a-zA-Z0-9+/]{PEM_WSP}){{4}}){}",
                    pem_begin("PGP PRIVATE KEY BLOCK"),
                    pem_base64_block(),
                    pem_end("PGP PRIVATE KEY BLOCK")
                )),
                allowed: Vec::new(),
                entropy: None,
            },
        ]
    })
}

/// The bytes as Dart's lenient ASCII decoding gives them.
fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { '\u{fffd}' })
        .collect()
}

/// `_entropy`: the Shannon entropy of the characters.
fn entropy(bytes: &[u8]) -> f64 {
    let text = ascii(bytes);
    let length = text.chars().count() as f64;
    let mut frequencies: Vec<(char, usize)> = Vec::new();
    for c in text.chars() {
        match frequencies.iter_mut().find(|(x, _)| *x == c) {
            Some((_, n)) => *n += 1,
            None => frequencies.push((c, 1)),
        }
    }
    let mut sum = 0.0;
    for (_, frequency) in frequencies {
        let f = frequency as f64;
        sum -= f / length * ((f / length).ln() / 2f64.ln());
    }
    sum
}

struct Leak {
    kind: &'static str,
    url: String,
    content: std::rc::Rc<Vec<u8>>,
    start: usize,
    end: usize,
}

impl Leak {
    fn describe(&self) -> String {
        if self.content.len() > 10000 {
            return format!(
                "Potential leak of {} in `{}` at offset {}:{}.\n\n```\n{}\n```\n",
                self.kind,
                self.url,
                self.start,
                self.end,
                ascii(&self.content[self.start..self.end])
            );
        }
        SourceFile::new(&ascii(&self.content), &self.url).message(
            self.start,
            self.end,
            &format!("Potential leak of {} detected.", self.kind),
        )
    }
}

fn find(pattern: &LeakPattern, url: &str, content: &std::rc::Rc<Vec<u8>>, leaks: &mut Vec<Leak>) {
    for captures in pattern.pattern.captures_iter(content) {
        let whole = captures.get(0).expect("group 0");
        let text = whole.as_bytes();
        let allowed = pattern.allowed.iter().any(|a| match a {
            Allowed::Text(s) => text.windows(s.len()).any(|w| w == s.as_bytes()),
            Allowed::Pattern(re) => re.is_match(text),
        });
        if allowed {
            continue;
        }
        if let Some((group, threshold)) = pattern.entropy {
            let value = captures.get(group).map_or(&[][..], |m| m.as_bytes());
            if entropy(value) < threshold {
                continue;
            }
        }
        leaks.push(Leak {
            kind: pattern.kind,
            url: url.to_owned(),
            content: content.clone(),
            start: whole.start(),
            end: whole.end(),
        });
    }
}

const DOCS: &str = "https://dart.dev/go/false-secrets";

pub(super) fn validate(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    // Pub reads `false_secrets` again here, and a bad value stops the whole
    // validation with the pubspec's YAML error.
    let secrets = false_secrets(c.fields()).map_err(|failure| {
        let pubspec = p::normalize(&p::join(&c.package.dir, "pubspec.yaml"));
        let text = super::files::read_text(&pubspec);
        let item = match c.fields().get("false_secrets") {
            crate::pub_get::tree::Node::List(items) => items
                .iter()
                .position(|n| !matches!(n, crate::pub_get::tree::Node::String(_))),
            _ => None,
        };
        match super::span::yaml_scalar_span(&text, "false_secrets", item) {
            Some((start, end)) => Failure {
                code: failure.code,
                message: format!(
                    "Error on {}",
                    SourceFile::new(&text, &pubspec).message(start, end, &failure.message)
                ),
            },
            None => failure,
        }
    })?;
    let false_secrets = Ignore::new(&secrets, IGNORE_CASE, |_, _| {});
    let mut leaks = Vec::new();
    for file in c.files {
        let relative = p::relative(file, &c.package.dir);
        let nix = p::split(&relative).join("/");
        if false_secrets.ignores(&nix) {
            continue;
        }
        let Ok(bytes) = std::fs::read(p::normalize(&p::absolute(file))) else {
            continue;
        };
        let content = std::rc::Rc::new(bytes);
        for pattern in patterns() {
            find(pattern, &relative, &content, &mut leaks);
        }
    }
    if leaks.len() > 3 {
        for leak in leaks.iter().take(2) {
            found.error(leak.describe());
        }
        let mut files: Vec<&str> = leaks.iter().map(|l| l.url.as_str()).collect();
        files.sort_by(|a, b| super::files::compare_utf16(a, b));
        files.dedup();
        let mut lines = vec![format!(
            "{} potential leaks detected in {} file{}:",
            leaks.len(),
            files.len(),
            if files.len() > 1 { "s" } else { "" }
        )];
        lines.extend(files.iter().take(10).map(|f| format!("- /{f}")));
        if files.len() > 10 {
            lines.push("...".into());
        }
        lines.push(String::new());
        lines.push("Add git-ignore style patterns to `false_secrets` in `pubspec.yaml`".into());
        lines.push(format!("to ignore this. See {DOCS}"));
        found.error(lines.join("\n"));
    } else if let Some(last) = leaks.pop() {
        for leak in leaks.iter().take(2) {
            found.error(leak.describe());
        }
        found.error(format!(
            "{}\nAdd a git-ignore style pattern to `false_secrets` in `pubspec.yaml`\nto ignore this. See {DOCS}",
            last.describe()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaks_in(text: &str) -> Vec<&'static str> {
        let content = std::rc::Rc::new(text.as_bytes().to_vec());
        let mut leaks = Vec::new();
        for pattern in patterns() {
            find(pattern, "x", &content, &mut leaks);
        }
        leaks.iter().map(|l| l.kind).collect()
    }

    /// Test vectors of `leak_detection.dart`.
    #[test]
    fn detects_the_vectors_of_pub() {
        assert_eq!(
            leaks_in(concat!("final accessKey = \"AKIA", "VBOGPFGGW6HQOSMY\";")),
            ["AWS Access Key"]
        );
        assert!(leaks_in("final accessKey = \"AKIAIOSFODNN7EXAMPLE\";").is_empty());
        assert_eq!(
            leaks_in("final apiKey = \"AIzaSyDG0yD6347wy0i1U4ThqQoEZ0y37ZvFKPM\";"),
            ["Google API Key"]
        );
        assert!(leaks_in("final apiKey = \"AIzaXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\";").is_empty());
        assert_eq!(
            leaks_in(
                "final superSecret = '204799038523-t6juuc8cvsvn7bdq0chhihkejuru0bkj.apps.googleusercontent.com';"
            ),
            ["Google OAuth ID"]
        );
        assert!(
            leaks_in(
                "final id = \"191919191919-onesonesonesonesonesonesonesones.apps.googleusercontent.com\";"
            )
            .is_empty()
        );
        assert_eq!(
            leaks_in(
                "-----BEGIN PRIVATE KEY-----\nH0M6xpM2q+53wmsN/eYLdgtjgBd3DBmHtPilCkiFICXyaA8z9LkJ\n-----END PRIVATE KEY-----\n"
            ),
            ["Private Key"]
        );
        assert!(
            leaks_in(
                "-----BEGIN PRIVATE KEY-----\nH0M6xpM2q+53wmsN/eYLdgtjgBd3DBmHtPilCkiFICXyaA8z9===\n-----END PRIVATE KEY-----\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn entropy_is_shannons() {
        assert_eq!(entropy(b"aaaa"), 0.0);
        assert!((entropy(b"ab") - 1.0).abs() < 1e-12);
    }
}
