//! Canonical JSON comparison without retaining an object/array tree.
use serde::Deserialize;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::borrow::Cow;
use std::fmt;
use std::io::{BufWriter, Write};

struct Seed<'a, W>(&'a mut W);
impl<'de, W: Write> DeserializeSeed<'de> for Seed<'_, W> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de, W: Write> Visitor<'de> for Seed<'_, W> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("canonical JSON")
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        self.0.write_all(b"null").map_err(E::custom)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<(), E> {
        serde_json::to_writer(self.0, &value).map_err(E::custom)
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<(), E> {
        serde_json::to_writer(self.0, &value).map_err(E::custom)
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<(), E> {
        serde_json::to_writer(self.0, &value).map_err(E::custom)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<(), E> {
        serde_json::to_writer(self.0, &value).map_err(E::custom)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        serde_json::to_writer(self.0, value).map_err(E::custom)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        self.0.write_all(b"[").map_err(de::Error::custom)?;
        let mut first = true;
        while sequence
            .next_element_seed(Element {
                writer: self.0,
                first: &mut first,
            })?
            .is_some()
        {}
        self.0.write_all(b"]").map_err(de::Error::custom)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        self.0.write_all(b"{").map_err(de::Error::custom)?;
        let mut previous: Option<Cow<'de, str>> = None;
        while let Some(Key(key)) = map.next_key::<Key<'de>>()? {
            if previous.as_ref().is_some_and(|previous| previous >= &key) {
                return Err(de::Error::custom("unordered or duplicate JSON key"));
            }
            if previous.is_some() {
                self.0.write_all(b",").map_err(de::Error::custom)?;
            }
            serde_json::to_writer(&mut *self.0, &key).map_err(de::Error::custom)?;
            self.0.write_all(b":").map_err(de::Error::custom)?;
            map.next_value_seed(Seed(&mut *self.0))?;
            previous = Some(key);
        }
        self.0.write_all(b"}").map_err(de::Error::custom)
    }
}

/// Keep ordinary keys in the immutable input; decoded escapes require ownership.
/// The previous decoded key must survive parsing the following map value/key.
struct Key<'de>(Cow<'de, str>);
impl<'de> Deserialize<'de> for Key<'de> {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;
        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = Key<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object key")
            }
            fn visit_borrowed_str<E: de::Error>(self, key: &'de str) -> Result<Self::Value, E> {
                Ok(Key(Cow::Borrowed(key)))
            }
            fn visit_str<E: de::Error>(self, key: &str) -> Result<Self::Value, E> {
                Ok(Key(Cow::Owned(key.to_owned())))
            }
        }
        deserializer.deserialize_str(KeyVisitor)
    }
}

struct Element<'a, W> {
    writer: &'a mut W,
    first: &'a mut bool,
}
impl<'de, W: Write> DeserializeSeed<'de> for Element<'_, W> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if !*self.first {
            self.writer.write_all(b",").map_err(de::Error::custom)?;
        }
        *self.first = false;
        Seed(self.writer).deserialize(deserializer)
    }
}

pub(super) fn matches(bytes: &[u8]) -> bool {
    let mut writer =
        BufWriter::with_capacity(super::BUFFER_BYTES, super::Comparison { remaining: bytes });
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    Seed(&mut writer).deserialize(&mut parser).is_ok()
        && parser.end().is_ok()
        && writer.write_all(b"\n").is_ok()
        && writer.flush().is_ok()
        && writer.get_ref().remaining.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_verify_canonical_predicate_matches_value_oracle() {
        for text in [
            "null\n",
            "true\n",
            "0\n",
            "-0\n",
            "-0.0\n",
            "1.0\n",
            "1e0\n",
            "1e308\n",
            "1e999\n",
            "18446744073709551615\n",
            "18446744073709551616\n",
            "-9223372036854775809\n",
            "[1,false,null,\"é\"]\n",
            "{\"a\":0,\"z\":1}\n",
            "{\"z\":1,\"a\":0}\n",
            "{\"a\":0,\"a\":0}\n",
            "{\"a\":{\"x\":0,\"x\":1}}\n",
            "{\"a\":0,\"\\u0061\":1}\n",
            "\"\\u00e9\"\n",
            "\"é\"\n",
            "\"\\ud83d\\ude00\"\n",
            "\"😀\"\n",
            "\"\\ud800\"\n",
            "[1,]\n",
            "true\n\n",
            " true\n",
            "false",
            "{\"a\":[]}\r\n",
        ] {
            let expected = serde_json::from_str::<serde_json::Value>(text)
                .is_ok_and(|value| super::super::canonical_matches(text.as_bytes(), &value));
            assert_eq!(matches(text.as_bytes()), expected, "{text}");
        }
    }
    #[test]
    fn stream_verify_mixed_decoded_keys_preserve_order_and_duplicates() {
        for value in [
            serde_json::json!({"\n": {"plain": 0}, "plain": {"é": 1}, "😀": 2}),
            serde_json::json!({"\"": [0, {"\t": 1}], "\\": {"a": 2}, "a": 3}),
        ] {
            let text = format!("{value}\n");
            assert!(matches(text.as_bytes()), "{text}");
        }
        // Nested escaped keys reuse parser scratch. Retain the previous decoded
        // key across that value before checking the next key for order/duplicates.
        for text in [
            "{\"a\\n\":{\"scratch\\t\":0},\"a\\n\":1}\n",
            "{\"z\\n\":{\"scratch\\t\":0},\"a\":1}\n",
            "{\"a\":0,\"\\u0061\":1}\n",
            "{\"a\\n\":{},\"a\\t\":0}\n",
        ] {
            assert!(!matches(text.as_bytes()), "{text}");
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(!super::super::canonical_matches(text.as_bytes(), &value));
        }
    }
    #[test]
    fn stream_verify_large_strings_arrays_and_boundaries() {
        let value =
            serde_json::json!({"a":"é\"\\\n".repeat(65537),"b":(0..10000).collect::<Vec<_>>()});
        let text = format!("{value}\n");
        assert!(matches(text.as_bytes()));
        for offset in [0, 65535, 65536, text.len() - 1] {
            let mut bytes = text.as_bytes().to_vec();
            bytes[offset] = b' ';
            assert!(!matches(&bytes));
        }
        assert!(!matches(&text.as_bytes()[..text.len() - 1]));
    }
}
