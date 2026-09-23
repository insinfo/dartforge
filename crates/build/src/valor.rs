//! Valores de configuração (`options:` do `build.yaml`) na forma que o Dart
//! vê: `Map<String, dynamic>` com `String`, `int`, `double`, `bool`, `null`,
//! listas e mapas, **na ordem de inserção** — é a ordem que o
//! `build_script_generate.dart` escreve no `build.dart` e a que o
//! `BuilderOptions.overrideWith` preserva (`{}..addAll(a)..addAll(b)`).
use yaml_rust2::Yaml;

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Bool(bool),
    Int(i64),
    /// Texto do `double.toString()` do Dart (`1.0`, `0.5`, `1e+21`).
    Real(String),
    Texto(String),
    Lista(Vec<Valor>),
    Mapa(Mapa),
}

/// Mapa com ordem de inserção. Chaves repetidas não existem: `inserir`
/// substitui no lugar, como `Map.operator[]=` do Dart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mapa(pub Vec<(Valor, Valor)>);

impl Mapa {
    pub fn vazio(&self) -> bool {
        self.0.is_empty()
    }

    pub fn obter(&self, chave: &str) -> Option<&Valor> {
        self.0.iter().find(|(k, _)| matches!(k, Valor::Texto(t) if t == chave)).map(|(_, v)| v)
    }

    pub fn inserir(&mut self, chave: Valor, valor: Valor) {
        if let Some(par) = self.0.iter_mut().find(|(k, _)| *k == chave) {
            par.1 = valor;
        } else {
            self.0.push((chave, valor));
        }
    }

    /// `BuilderOptions.overrideWith`: as chaves de `outro` vencem, por chave,
    /// sem fusão de valores; a ordem é a de `self` seguida das novas.
    pub fn sobrepor(&self, outro: &Mapa) -> Mapa {
        let mut m = self.clone();
        for (k, v) in &outro.0 {
            m.inserir(k.clone(), v.clone());
        }
        m
    }

    pub fn texto_canonico(&self) -> String {
        Valor::Mapa(self.clone()).texto_canonico()
    }
}

/// `double.toString()` do Dart para os valores que um YAML produz.
pub fn real_dart(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let a = v.abs();
    if v.fract() == 0.0 && a < 1e21 {
        return format!("{v:.1}");
    }
    if (1e-6..1e21).contains(&a) {
        return format!("{v}");
    }
    // Notação exponencial: Dart escreve `1e+21`, `1.5e-7`.
    let s = format!("{v:e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

impl Valor {
    /// Converte o que o `yaml-rust2` leu. `Alias`/valor inválido é erro: o
    /// `package:yaml` do Dart também não os entrega como valor.
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
            Yaml::Alias(_) => return Err("alias YAML não suportado".into()),
            Yaml::BadValue => return Err("valor YAML inválido".into()),
        })
    }

    pub fn como_texto(&self) -> Option<&str> {
        match self {
            Valor::Texto(t) => Some(t),
            _ => None,
        }
    }

    /// Forma canônica, estável e sem ambiguidade (textos em JSON). É a forma
    /// comparada com o `build.dart` do oráculo e a que entra no digest das
    /// opções.
    pub fn texto_canonico(&self) -> String {
        let mut s = String::new();
        self.escrever(&mut s);
        s
    }

    fn escrever(&self, s: &mut String) {
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
                    v.escrever(s);
                }
                s.push(']');
            }
            Valor::Mapa(m) => {
                s.push('{');
                for (i, (k, v)) in m.0.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    k.escrever(s);
                    s.push_str(": ");
                    v.escrever(s);
                }
                s.push('}');
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn real_como_o_dart() {
        assert_eq!(real_dart(1.0), "1.0");
        assert_eq!(real_dart(0.5), "0.5");
        assert_eq!(real_dart(1000.0), "1000.0");
        assert_eq!(real_dart(1e21), "1e+21");
        assert_eq!(real_dart(1.5e-7), "1.5e-7");
    }

    #[test]
    fn sobrepor_por_chave_na_ordem() {
        let t = |s: &str| Valor::Texto(s.into());
        let a = Mapa(vec![(t("a"), Valor::Int(1)), (t("b"), Valor::Int(2))]);
        let b = Mapa(vec![(t("b"), Valor::Int(3)), (t("c"), Valor::Int(4))]);
        assert_eq!(a.sobrepor(&b).texto_canonico(), r#"{"a": 1, "b": 3, "c": 4}"#);
    }
}
