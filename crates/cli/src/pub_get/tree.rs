//! An order-preserving document tree for pubspecs, lockfiles and registry
//! responses.
//!
//! Pub reads YAML and JSON into insertion-ordered maps, and the order of
//! dependencies decides the order in which the solver learns facts; a
//! sorted map would change which of several valid solutions it finds.
use yaml_rust2::{Yaml, YamlLoader};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    /// An integer or real number, as written.
    Number(String),
    String(String),
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

static NULL: Node = Node::Null;

impl Node {
    /// The value of `key` in a map, or `Null`.
    pub fn get(&self, key: &str) -> &Node {
        match self {
            Node::Map(entries) => entries
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        matches!(self, Node::Map(entries) if entries.iter().any(|(k, _)| k == key))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Node::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, Node)]> {
        match self {
            Node::Map(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Node]> {
        match self {
            Node::List(items) => Some(items),
            _ => None,
        }
    }

    /// The compact JSON text of this node (Dart's `jsonEncode`), used to
    /// compare unknown descriptions.
    pub fn to_json(&self) -> String {
        match self {
            Node::Null => "null".into(),
            Node::Bool(b) => b.to_string(),
            Node::Number(n) => n.clone(),
            Node::String(s) => serde_json::to_string(s).expect("string serialization"),
            Node::List(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(Node::to_json)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Node::Map(entries) => format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(k, v)| format!(
                        "{}:{}",
                        serde_json::to_string(k).expect("string serialization"),
                        v.to_json()
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

/// Parses a single YAML document.
pub(crate) fn parse_yaml(text: &str) -> Result<Node, String> {
    fn convert(value: &Yaml) -> Result<Node, String> {
        Ok(match value {
            Yaml::Null | Yaml::BadValue => Node::Null,
            Yaml::String(s) => Node::String(s.clone()),
            Yaml::Integer(i) => Node::Number(i.to_string()),
            Yaml::Real(r) => Node::Number(r.clone()),
            Yaml::Boolean(b) => Node::Bool(*b),
            Yaml::Array(items) => Node::List(items.iter().map(convert).collect::<Result<_, _>>()?),
            Yaml::Hash(hash) => {
                let mut entries: Vec<(String, Node)> = Vec::with_capacity(hash.len());
                for (key, value) in hash {
                    let key = match key {
                        Yaml::String(s) => s.clone(),
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Real(r) => r.clone(),
                        Yaml::Boolean(b) => b.to_string(),
                        Yaml::Null => "null".into(),
                        _ => return Err("YAML keys must be scalars".into()),
                    };
                    if entries.iter().any(|(k, _)| *k == key) {
                        return Err(format!("Duplicate mapping key \"{key}\"."));
                    }
                    entries.push((key, convert(value)?));
                }
                Node::Map(entries)
            }
            Yaml::Alias(_) => return Err("YAML aliases are not supported".into()),
        })
    }
    let docs = YamlLoader::load_from_str(text).map_err(|e| {
        format!(
            "Error on line {}, column {}: {}",
            e.marker().line(),
            e.marker().col() + 1,
            e.info()
        )
    })?;
    match docs.as_slice() {
        [] => Ok(Node::Null),
        [doc] => convert(doc),
        _ => Err("Only one YAML document is allowed.".into()),
    }
}

/// Parses JSON, keeping the order of object keys (Dart's `jsonDecode`).
pub(crate) fn parse_json(bytes: &[u8]) -> Result<Node, String> {
    use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
    struct Seed;
    impl<'de> DeserializeSeed<'de> for Seed {
        type Value = Node;
        fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
            deserializer.deserialize_any(NodeVisitor)
        }
    }
    struct NodeVisitor;
    impl<'de> Visitor<'de> for NodeVisitor {
        type Value = Node;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a JSON value")
        }
        fn visit_unit<E>(self) -> Result<Node, E> {
            Ok(Node::Null)
        }
        fn visit_none<E>(self) -> Result<Node, E> {
            Ok(Node::Null)
        }
        fn visit_bool<E>(self, v: bool) -> Result<Node, E> {
            Ok(Node::Bool(v))
        }
        fn visit_i64<E>(self, v: i64) -> Result<Node, E> {
            Ok(Node::Number(v.to_string()))
        }
        fn visit_u64<E>(self, v: u64) -> Result<Node, E> {
            Ok(Node::Number(v.to_string()))
        }
        fn visit_f64<E>(self, v: f64) -> Result<Node, E> {
            Ok(Node::Number(v.to_string()))
        }
        fn visit_str<E>(self, v: &str) -> Result<Node, E> {
            Ok(Node::String(v.to_owned()))
        }
        fn visit_string<E>(self, v: String) -> Result<Node, E> {
            Ok(Node::String(v))
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
            let mut items = Vec::new();
            while let Some(item) = seq.next_element_seed(Seed)? {
                items.push(item);
            }
            Ok(Node::List(items))
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
            let mut entries: Vec<(String, Node)> = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                let value = map.next_value_seed(Seed)?;
                // A repeated key keeps its first position and its last value.
                match entries.iter_mut().find(|(k, _)| *k == key) {
                    Some(entry) => entry.1 = value,
                    None => entries.push((key, value)),
                }
            }
            Ok(Node::Map(entries))
        }
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let node = Seed
        .deserialize(&mut deserializer)
        .map_err(|e| e.to_string())?;
    deserializer.end().map_err(|e| e.to_string())?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_keep_their_written_order() {
        let yaml = parse_yaml("b: 1\na:\n  z: x\n  y: [1, two]\n").unwrap();
        let keys: Vec<_> = yaml
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["b", "a"]);
        assert_eq!(yaml.get("a").get("z").as_str(), Some("x"));
        assert!(matches!(yaml.get("missing"), Node::Null));
        let json = parse_json(br#"{"z":1,"a":{"q":null,"b":[true,"s"]}}"#).unwrap();
        let keys: Vec<_> = json
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["z", "a"]);
        assert_eq!(json.get("a").to_json(), r#"{"q":null,"b":[true,"s"]}"#);
        assert!(parse_yaml("a: 1\na: 2\n").is_err());
    }
}
