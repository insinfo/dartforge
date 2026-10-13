//! The `build_runner` command-line options that change the plan or the
//! outputs: `--define`, `--config` and `--build-filter`. Port of
//! `entrypoint/options.dart` (2.4.15) and `build_plan/build_options.dart`
//! (2.16.1) — `--define` and `--config` are the same in both; `--build-filter`
//! gained the `asset:` scheme in 2.14.0.
//!
//! Precedence of a builder's options (`createBuildPhases`): `defaults` <
//! the defaults' `dev_options`/`release_options` < the target's `options` <
//! the target's `dev_options`/`release_options` < `global_options` (with its
//! `dev`/`release`) < `--define`, the latter merged key by key over the
//! global ones.
use crate::config::builder_usage_key;
use crate::glob::Glob;
use crate::graph::AssetId;
use crate::value::{Map, Value, dart_real};

/// The `--define`s already grouped by builder (normalized key), in the order
/// in which they appeared.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Defines(pub Vec<(String, Map)>);

impl Defines {
    /// `_parseBuilderConfigOverrides`: each argument is
    /// `<builder>=<option>=<value>`; the rest after the second `=` is the
    /// value, read as JSON and, if it is not JSON, as a string. The builder
    /// key is normalized with the root package (`:x` → `root:x`, `x` →
    /// `x:x`).
    ///
    /// ```
    /// use dartforge_build::command_line::Defines;
    /// let d = Defines::parse(&[":b=n=1", "p:b=t=a=b", "p:b=l=[1, 2.0]"], "p").unwrap();
    /// assert_eq!(d.0[0].0, "p:b");
    /// assert_eq!(d.0[0].1.canonical_text(), r#"{"n": 1, "t": "a=b", "l": [1, 2.0]}"#);
    /// assert!(Defines::parse(&["b=n"], "p").is_err());
    /// assert!(Defines::parse(&["b=n=1", "b=n=2"], "p").is_err());
    /// ```
    ///
    /// # Errors
    /// Fewer than two `=`, or the same option of the same builder twice —
    /// `build_runner`'s messages.
    pub fn parse<S: AsRef<str>>(arguments: &[S], root: &str) -> Result<Defines, String> {
        let mut d = Defines::default();
        for a in arguments {
            let a = a.as_ref();
            let mut parts = a.splitn(3, '=');
            let (Some(builder), Some(option), Some(value)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return Err(format!(
                    "Invalid argument (define): Expected at least 2 `=` signs, should be of the format like --define \"<builder_key>=<option>=<value>\": \"{a}\""
                ));
            };
            let key = builder_usage_key(builder, root);
            let value = json(value).unwrap_or_else(|| Value::Text(value.to_string()));
            let i = match d.0.iter().position(|(k, _)| *k == key) {
                Some(i) => i,
                None => {
                    d.0.push((key.clone(), Map::default()));
                    d.0.len() - 1
                }
            };
            let m = &mut d.0[i].1;
            if m.get(option).is_some() {
                return Err(format!(
                    "Invalid argument(s): Got duplicate overrides for the same builder option: {key}={option}. Only one is allowed."
                ));
            }
            m.insert(Value::Text(option.to_string()), value);
        }
        Ok(d)
    }

    /// The options defined for the builder `key`.
    pub fn for_builder(&self, key: &str) -> Option<&Map> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, m)| m)
    }
}

/// Dart's `json.decode`: `None` if the text is not JSON. Numbers with `.` or
/// an exponent are `double`, the others `int` (or `double`, if they do not
/// fit in 64 bits); objects keep the key order (the last duplicate wins).
///
/// ```
/// use dartforge_build::command_line::json;
/// use dartforge_build::value::Value;
/// assert_eq!(json("1"), Some(Value::Int(1)));
/// assert_eq!(json("1e3"), Some(Value::Real("1000.0".into())));
/// assert_eq!(json(" \"a\\u00e9\" "), Some(Value::Text("aé".into())));
/// assert_eq!(json("{a: 1}"), None);
/// assert_eq!(json("~"), None);
/// ```
pub fn json(text: &str) -> Option<Value> {
    let b = text.as_bytes();
    let mut i = 0;
    let v = json_value(b, &mut i)?;
    skip_whitespace(b, &mut i);
    (i == b.len()).then_some(v)
}

fn skip_whitespace(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn json_value(b: &[u8], i: &mut usize) -> Option<Value> {
    skip_whitespace(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut m = Map::default();
            skip_whitespace(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(Value::Map(m));
            }
            loop {
                skip_whitespace(b, i);
                let Value::Text(k) = json_string(b, i)? else {
                    return None;
                };
                skip_whitespace(b, i);
                if b.get(*i) != Some(&b':') {
                    return None;
                }
                *i += 1;
                let v = json_value(b, i)?;
                m.insert(Value::Text(k), v);
                skip_whitespace(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(Value::Map(m));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut l = Vec::new();
            skip_whitespace(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(Value::List(l));
            }
            loop {
                l.push(json_value(b, i)?);
                skip_whitespace(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(Value::List(l));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => json_string(b, i),
        b't' => literal(b, i, "true", Value::Bool(true)),
        b'f' => literal(b, i, "false", Value::Bool(false)),
        b'n' => literal(b, i, "null", Value::Null),
        b'-' | b'0'..=b'9' => json_number(b, i),
        _ => None,
    }
}

fn literal(b: &[u8], i: &mut usize, t: &str, v: Value) -> Option<Value> {
    if b[*i..].starts_with(t.as_bytes()) {
        *i += t.len();
        Some(v)
    } else {
        None
    }
}

fn json_number(b: &[u8], i: &mut usize) -> Option<Value> {
    let start = *i;
    if b[*i] == b'-' {
        *i += 1;
    }
    let digits = |i: &mut usize| {
        let d = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > d
    };
    // Integer: `0` or a non-zero digit followed by digits.
    if b.get(*i) == Some(&b'0') {
        *i += 1;
    } else if !digits(i) {
        return None;
    }
    let mut real = false;
    if b.get(*i) == Some(&b'.') {
        *i += 1;
        real = true;
        if !digits(i) {
            return None;
        }
    }
    if matches!(b.get(*i), Some(b'e' | b'E')) {
        *i += 1;
        real = true;
        if matches!(b.get(*i), Some(b'+' | b'-')) {
            *i += 1;
        }
        if !digits(i) {
            return None;
        }
    }
    let t = std::str::from_utf8(&b[start..*i]).ok()?;
    if !real && let Ok(n) = t.parse::<i64>() {
        return Some(Value::Int(n));
    }
    t.parse::<f64>().ok().map(|v| Value::Real(dart_real(v)))
}

fn json_string(b: &[u8], i: &mut usize) -> Option<Value> {
    if b.get(*i) != Some(&b'"') {
        return None;
    }
    *i += 1;
    let mut units: Vec<u16> = Vec::new();
    loop {
        let c = *b.get(*i)?;
        match c {
            b'"' => {
                *i += 1;
                return Some(Value::Text(String::from_utf16_lossy(&units)));
            }
            b'\\' => {
                *i += 1;
                let e = *b.get(*i)?;
                *i += 1;
                let u = match e {
                    b'"' => '"' as u16,
                    b'\\' => '\\' as u16,
                    b'/' => '/' as u16,
                    b'b' => 8,
                    b'f' => 12,
                    b'n' => 10,
                    b'r' => 13,
                    b't' => 9,
                    b'u' => {
                        let h = std::str::from_utf8(b.get(*i..*i + 4)?).ok()?;
                        *i += 4;
                        u16::from_str_radix(h, 16).ok()?
                    }
                    _ => return None,
                };
                units.push(u);
            }
            0..=0x1f => return None,
            _ => {
                // A whole UTF-8 character.
                let rest = std::str::from_utf8(&b[*i..]).ok().or_else(|| {
                    let end = (*i + 4).min(b.len());
                    (*i + 1..=end)
                        .rev()
                        .find_map(|f| std::str::from_utf8(&b[*i..f]).ok())
                })?;
                let ch = rest.chars().next()?;
                let mut buf = [0u16; 2];
                units.extend_from_slice(ch.encode_utf16(&mut buf));
                *i += ch.len_utf8();
            }
        }
    }
}

/// A project's `--config`, `--define` and `--build-filter` options, read
/// against `pubspec.yaml` (the root package's name normalizes the keys and the
/// relative paths) and `pubspec.lock` (the profile decides whether `asset:` is
/// accepted).
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    pub config: Option<String>,
    pub definitions: Defines,
    pub filters: Vec<BuildFilter>,
}

impl BuildOptions {
    /// Reads the options for the project in `root`.
    ///
    /// # Errors
    /// Unreadable `pubspec.yaml`/`pubspec.lock`, a malformed `--define` or an
    /// invalid `--build-filter`.
    pub fn for_project<S: AsRef<str>>(
        root: &std::path::Path,
        config: Option<&str>,
        defines: &[S],
        filters: &[S],
    ) -> Result<BuildOptions, String> {
        let pubspec = crate::config::Pubspec::read(root)?;
        let name = pubspec
            .name
            .ok_or("The current package has no name, please add one to the pubspec.yaml.")?;
        let profile = match std::fs::read_to_string(root.join("pubspec.lock")) {
            Ok(t) => {
                let lock = crate::config::read_lock(&t, "pubspec.lock")?;
                let v = |p: &str| {
                    lock.iter()
                        .find(|(n, _)| n == p)
                        .map(|(_, t)| t.version.as_str())
                };
                crate::profile::Profile::from_versions(v("build_config"), v("build_runner"))
            }
            Err(_) => crate::profile::Profile::default(),
        };
        Ok(BuildOptions {
            config: config.map(str::to_string),
            definitions: Defines::parse(defines, &name)?,
            filters: filters
                .iter()
                .map(|f| BuildFilter::from_arg(f.as_ref(), &name, profile.asset_filter()))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Applies them to the engine options.
    pub fn apply(self, options: &mut crate::engine::EngineOptions) {
        options.config = self.config;
        options.defines = self.definitions;
        options.filters = self.filters;
    }
}

/// `BuildFilter`: a package glob and a path glob.
#[derive(Debug, Clone)]
pub struct BuildFilter {
    pub package: Glob,
    pub path: Glob,
}

impl BuildFilter {
    /// `BuildFilter.fromArg`: a relative path (from the root package),
    /// `package:` (under `lib/`) and, with `asset` (`build_runner` ≥ 2.14.0),
    /// `asset:` (the whole package).
    ///
    /// ```
    /// use dartforge_build::graph::AssetId;
    /// use dartforge_build::command_line::BuildFilter;
    /// let f = BuildFilter::from_arg("package:a/*.dart", "root", false).unwrap();
    /// assert!(f.matches(&AssetId::new("a", "lib/x.dart")));
    /// let r = BuildFilter::from_arg("web/**", "root", false).unwrap();
    /// assert!(r.matches(&AssetId::new("root", "web/a/b.js")));
    /// assert!(BuildFilter::from_arg("asset:a/web/x", "root", false).is_err());
    /// assert!(BuildFilter::from_arg("asset:a/web/x", "root", true).is_ok());
    /// ```
    ///
    /// # Errors
    /// Unsupported scheme or invalid glob, with `build_runner`'s message.
    pub fn from_arg(arg: &str, root: &str, asset: bool) -> Result<BuildFilter, String> {
        let error = |e: String| {
            format!(
                "Invalid argument (--build-filter): Not a valid build filter, must be either a relative path or `package:` uri.\n\n{e}: \"{arg}\""
            )
        };
        // `Uri.parse`: scheme = letters, digits, `+`, `-`, `.` before the
        // first `:`, starting with a letter.
        let scheme = arg.split_once(':').and_then(|(e, _)| {
            (!e.is_empty()
                && e.as_bytes()[0].is_ascii_alphabetic()
                && e.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c)))
            .then_some(e)
        });
        // The URI's path, without query or fragment.
        let uri_path = |rest: &str| -> String {
            let end = rest.find(['?', '#']).unwrap_or(rest.len());
            rest[..end].to_string()
        };
        let (package, path) = match scheme.map(|e| e.to_ascii_lowercase()).as_deref() {
            Some("package") => {
                let p = unescape(&uri_path(&arg["package:".len()..]));
                let mut seg = p.split('/');
                let package = seg.next().unwrap_or_default().to_string();
                let rest: Vec<&str> = seg.collect();
                let mut c = String::from("lib");
                for s in rest {
                    c.push('/');
                    c.push_str(s);
                }
                (package, c)
            }
            Some("asset") if asset => {
                let p = unescape(&uri_path(&arg["asset:".len()..]));
                let (package, rest) = p.split_once('/').unwrap_or((p.as_str(), ""));
                (package.to_string(), rest.to_string())
            }
            Some(e) => return Err(error(format!("Unsupported scheme {e}"))),
            None => (root.to_string(), escaped_path(&uri_path(arg))),
        };
        Ok(BuildFilter {
            package: Glob::new(&package).map_err(error)?,
            path: Glob::new(&path).map_err(error)?,
        })
    }

    /// `BuildFilter.matches`.
    pub fn matches(&self, id: &AssetId) -> bool {
        self.package.matches(&id.package) && self.path.matches(&id.path)
    }
}

/// The path segments of a URI with the `%XX` escapes undone
/// (`Uri.pathSegments`, joined by `/`).
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            v.push(h(b[i + 1]) * 16 + h(b[i + 2]));
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// `Uri.path` of a relative reference: Dart's `Uri.parse` escapes what cannot
/// stay in a path (space, `{`, `}`, `[`, `]`, non-ASCII…) and normalizes the
/// existing `%XX` (uppercase; that of an unreserved character becomes the
/// character itself). The filter's glob is made from this text, so `{a,b}` in
/// a relative path is not an alternation, as in the official implementation.
fn escaped_path(s: &str) -> String {
    let b = s.as_bytes();
    let allowed = |c: u8| c.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&c);
    let mut o = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            let x = h(b[i + 1]) * 16 + h(b[i + 2]);
            if x.is_ascii_alphanumeric() || b"-._~".contains(&x) {
                o.push(x as char);
            } else {
                o.push_str(&format!("%{:02X}", x));
            }
            i += 3;
        } else if allowed(c) {
            o.push(c as char);
            i += 1;
        } else {
            o.push_str(&format!("%{c:02X}"));
            i += 1;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_like_dart() {
        assert_eq!(json("true"), Some(Value::Bool(true)));
        assert_eq!(json("null"), Some(Value::Null));
        assert_eq!(json("-0.5"), Some(Value::Real("-0.5".into())));
        assert_eq!(json("2.0"), Some(Value::Real("2.0".into())));
        assert_eq!(json("01"), None);
        assert_eq!(json("[1,"), None);
        assert_eq!(json("'x'"), None);
        assert_eq!(
            json(r#"{"b": 1, "a": null, "b": 2}"#).map(|v| v.canonical_text()),
            Some(r#"{"b": 2, "a": null}"#.into())
        );
        assert_eq!(json(r#""😀""#), Some(Value::Text("😀".into())));
        assert_eq!(json("\"três\""), Some(Value::Text("três".into())));
    }

    #[test]
    fn define_normalizes_and_keeps_the_rest() {
        let d = Defines::parse(&["x=a=1", "q|y=b=text without json"], "root").unwrap();
        assert_eq!(d.0[0].0, "x:x");
        assert_eq!(d.0[1].0, "q:y");
        assert_eq!(
            d.for_builder("q:y").unwrap().canonical_text(),
            r#"{"b": "text without json"}"#
        );
    }

    #[test]
    fn filter_with_escape_and_glob() {
        // Relative: the path is the `Uri.path`, escaped.
        let f = BuildFilter::from_arg("lib/a b.*", "r", false).unwrap();
        assert!(f.matches(&AssetId::new("r", "lib/a%20b.txt")));
        assert!(!f.matches(&AssetId::new("r", "lib/a b.txt")));
        assert!(
            !BuildFilter::from_arg("lib/{a,b}.txt", "r", false)
                .unwrap()
                .matches(&AssetId::new("r", "lib/a.txt"))
        );
        // `package:`: the segments, unescaped.
        let p = BuildFilter::from_arg("package:r/a%20b.*", "r", false).unwrap();
        assert!(p.matches(&AssetId::new("r", "lib/a b.txt")));
        let g = BuildFilter::from_arg("package:{a,b}/**", "r", false).unwrap();
        assert!(g.matches(&AssetId::new("b", "lib/x/y.dart")));
        assert!(!g.matches(&AssetId::new("b", "web/y.dart")));
        assert!(BuildFilter::from_arg("http://x/y", "r", true).is_err());
    }
}
