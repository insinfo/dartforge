//! Configuration values (`options:` of `build.yaml`) in the shape Dart sees
//! them: `Map<String, dynamic>` with `String`, `int`, `double`, `bool`,
//! `null`, lists and maps, **in insertion order** — the order that
//! `build_script_generate.dart` writes into `build.dart` and that
//! `BuilderOptions.overrideWith` preserves (`{}..addAll(a)..addAll(b)`).
use yaml_rust2::Yaml;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    /// Text of Dart's `double.toString()` (`1.0`, `0.5`, `1e+21`).
    Real(String),
    Text(String),
    List(Vec<Value>),
    Map(Map),
}

/// Map with insertion order. Duplicate keys do not exist: `insert` replaces
/// in place, like Dart's `Map.operator[]=`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map(pub Vec<(Value, Value)>);

impl Map {
    pub fn empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(k, _)| matches!(k, Value::Text(t) if t == key))
            .map(|(_, v)| v)
    }

    pub fn insert(&mut self, key: Value, value: Value) {
        if let Some(pair) = self.0.iter_mut().find(|(k, _)| *k == key) {
            pair.1 = value;
        } else {
            self.0.push((key, value));
        }
    }

    /// `BuilderOptions.overrideWith`: the keys of `other` win, key by key,
    /// without merging values; the order is that of `self` followed by the
    /// new keys.
    pub fn overlay(&self, other: &Map) -> Map {
        let mut m = self.clone();
        for (k, v) in &other.0 {
            m.insert(k.clone(), v.clone());
        }
        m
    }

    pub fn canonical_text(&self) -> String {
        Value::Map(self.clone()).canonical_text()
    }
}

/// Dart's `double.toString()` for the values a YAML document produces.
pub fn dart_real(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let a = v.abs();
    if v.fract() == 0.0 && a < 1e21 {
        return format!("{v:.1}");
    }
    if (1e-6..1e21).contains(&a) {
        return format!("{v}");
    }
    // Exponential notation: Dart writes `1e+21`, `1.5e-7`.
    let s = format!("{v:e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

impl Value {
    /// Converts what `yaml-rust2` read. An `Alias`/invalid value is an error:
    /// Dart's `package:yaml` does not deliver them as values either.
    pub fn from_yaml(y: &Yaml) -> Result<Value, String> {
        Ok(match y {
            Yaml::Null => Value::Null,
            Yaml::Boolean(b) => Value::Bool(*b),
            Yaml::Integer(i) => Value::Int(*i),
            Yaml::Real(r) => match y.as_f64() {
                Some(v) => Value::Real(dart_real(v)),
                None => Value::Text(r.clone()),
            },
            Yaml::String(s) => Value::Text(s.clone()),
            Yaml::Array(a) => Value::List(a.iter().map(Value::from_yaml).collect::<Result<_, _>>()?),
            Yaml::Hash(h) => {
                let mut m = Map::default();
                for (k, v) in h {
                    m.insert(Value::from_yaml(k)?, Value::from_yaml(v)?);
                }
                Value::Map(m)
            }
            Yaml::Alias(_) => return Err("YAML alias not supported".into()),
            Yaml::BadValue => return Err("invalid YAML value".into()),
        })
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Canonical form, stable and unambiguous (strings as JSON). It is the
    /// form compared with the oracle's `build.dart` and the one that goes into
    /// the digest of the options.
    pub fn canonical_text(&self) -> String {
        let mut s = String::new();
        self.write_to(&mut s);
        s
    }

    fn write_to(&self, s: &mut String) {
        match self {
            Value::Null => s.push_str("null"),
            Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => s.push_str(&i.to_string()),
            Value::Real(r) => s.push_str(r),
            Value::Text(t) => s.push_str(&serde_json::Value::String(t.clone()).to_string()),
            Value::List(l) => {
                s.push('[');
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    v.write_to(s);
                }
                s.push(']');
            }
            Value::Map(m) => {
                s.push('{');
                for (i, (k, v)) in m.0.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    k.write_to(s);
                    s.push_str(": ");
                    v.write_to(s);
                }
                s.push('}');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_like_dart() {
        assert_eq!(dart_real(1.0), "1.0");
        assert_eq!(dart_real(0.5), "0.5");
        assert_eq!(dart_real(1000.0), "1000.0");
        assert_eq!(dart_real(1e21), "1e+21");
        assert_eq!(dart_real(1.5e-7), "1.5e-7");
    }

    #[test]
    fn override_by_key_in_order() {
        let t = |s: &str| Value::Text(s.into());
        let a = Map(vec![(t("a"), Value::Int(1)), (t("b"), Value::Int(2))]);
        let b = Map(vec![(t("b"), Value::Int(3)), (t("c"), Value::Int(4))]);
        assert_eq!(
            a.overlay(&b).canonical_text(),
            r#"{"a": 1, "b": 3, "c": 4}"#
        );
    }
}
