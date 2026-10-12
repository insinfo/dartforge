//! Configuration values (`options:` of `build.yaml`) in the shape Dart sees
//! them: `Map<String, dynamic>` with `String`, `int`, `double`, `bool`,
//! `null`, lists and maps, **in insertion order** — the order that
//! `build_script_generate.dart` writes into `build.dart` and that
//! `BuilderOptions.overrideWith` preserves (`{}..addAll(a)..addAll(b)`).
use yaml_rust2::Yaml;

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Bool(bool),
    Int(i64),
    /// Text of Dart's `double.toString()` (`1.0`, `0.5`, `1e+21`).
    Real(String),
    Texto(String),
    Lista(Vec<Valor>),
    Mapa(Mapa),
}

/// Map with insertion order. Duplicate keys do not exist: `inserir` replaces
/// in place, like Dart's `Map.operator[]=`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mapa(pub Vec<(Valor, Valor)>);

impl Mapa {
    pub fn vazio(&self) -> bool {
        self.0.is_empty()
    }

    pub fn obter(&self, key: &str) -> Option<&Valor> {
        self.0
            .iter()
            .find(|(k, _)| matches!(k, Valor::Texto(t) if t == key))
            .map(|(_, v)| v)
    }

    pub fn inserir(&mut self, key: Valor, value: Valor) {
        if let Some(pair) = self.0.iter_mut().find(|(k, _)| *k == key) {
            pair.1 = value;
        } else {
            self.0.push((key, value));
        }
    }

    /// `BuilderOptions.overrideWith`: the keys of `other` win, key by key,
    /// without merging values; the order is that of `self` followed by the
    /// new keys.
    pub fn sobrepor(&self, other: &Mapa) -> Mapa {
        let mut m = self.clone();
        for (k, v) in &other.0 {
            m.inserir(k.clone(), v.clone());
        }
        m
    }

    pub fn texto_canonico(&self) -> String {
        Valor::Mapa(self.clone()).texto_canonico()
    }
}

/// Dart's `double.toString()` for the values a YAML document produces.
pub fn real_dart(v: f64) -> String {
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

impl Valor {
    /// Converts what `yaml-rust2` read. An `Alias`/invalid value is an error:
    /// Dart's `package:yaml` does not deliver them as values either.
    pub fn de_yaml(y: &Yaml) -> Result<Valor, String> {
        Ok(match y {
            Yaml::Null => Valor::Nulo,
            Yaml::Boolean(b) => Valor::Bool(*b),
            Yaml::Integer(i) => Valor::Int(*i),
            Yaml::Real(r) => match y.as_f64() {
                Some(v) => Valor::Real(real_dart(v)),
                None => Valor::Texto(r.clone()),
            },
            Yaml::String(s) => Valor::Texto(s.clone()),
            Yaml::Array(a) => Valor::Lista(a.iter().map(Valor::de_yaml).collect::<Result<_, _>>()?),
            Yaml::Hash(h) => {
                let mut m = Mapa::default();
                for (k, v) in h {
                    m.inserir(Valor::de_yaml(k)?, Valor::de_yaml(v)?);
                }
                Valor::Mapa(m)
            }
            Yaml::Alias(_) => return Err("YAML alias not supported".into()),
            Yaml::BadValue => return Err("invalid YAML value".into()),
        })
    }

    pub fn como_texto(&self) -> Option<&str> {
        match self {
            Valor::Texto(t) => Some(t),
            _ => None,
        }
    }

    /// Canonical form, stable and unambiguous (strings as JSON). It is the
    /// form compared with the oracle's `build.dart` and the one that goes into
    /// the digest of the options.
    pub fn texto_canonico(&self) -> String {
        let mut s = String::new();
        self.write_to(&mut s);
        s
    }

    fn write_to(&self, s: &mut String) {
        match self {
            Valor::Nulo => s.push_str("null"),
            Valor::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Valor::Int(i) => s.push_str(&i.to_string()),
            Valor::Real(r) => s.push_str(r),
            Valor::Texto(t) => s.push_str(&serde_json::Value::String(t.clone()).to_string()),
            Valor::Lista(l) => {
                s.push('[');
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    v.write_to(s);
                }
                s.push(']');
            }
            Valor::Mapa(m) => {
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
        assert_eq!(real_dart(1.0), "1.0");
        assert_eq!(real_dart(0.5), "0.5");
        assert_eq!(real_dart(1000.0), "1000.0");
        assert_eq!(real_dart(1e21), "1e+21");
        assert_eq!(real_dart(1.5e-7), "1.5e-7");
    }

    #[test]
    fn override_by_key_in_order() {
        let t = |s: &str| Valor::Texto(s.into());
        let a = Mapa(vec![(t("a"), Valor::Int(1)), (t("b"), Valor::Int(2))]);
        let b = Mapa(vec![(t("b"), Valor::Int(3)), (t("c"), Valor::Int(4))]);
        assert_eq!(
            a.sobrepor(&b).texto_canonico(),
            r#"{"a": 1, "b": 3, "c": 4}"#
        );
    }
}
