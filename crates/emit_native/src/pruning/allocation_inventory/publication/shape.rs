//! Integrity-only envelope projection. Function facts are deliberately ignored.
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub artifacts: Value,
    pub cache_key: String,
    pub compiler: String,
    pub inventory: InventoryShape,
    pub production: bool,
    pub runtime: Value,
    pub schema: u64,
}

pub(super) struct InventoryShape;
struct ArrayShape;
impl<'de> Deserialize<'de> for ArrayShape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Array;
        impl<'de> Visitor<'de> for Array {
            type Value = ArrayShape;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an inventory array")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                while sequence.next_element::<IgnoredAny>()?.is_some() {}
                Ok(ArrayShape)
            }
        }
        deserializer.deserialize_seq(Array)
    }
}
impl<'de> Deserialize<'de> for InventoryShape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Shape;
        impl<'de> Visitor<'de> for Shape {
            type Value = InventoryShape;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("four inventory fields including three arrays")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut count = 0;
                let mut arrays = 0;
                while let Some(key) = map.next_key::<String>()? {
                    count += 1;
                    let bit = match key.as_str() {
                        "functions" => 1,
                        "globals" => 2,
                        "modules" => 4,
                        _ => 0,
                    };
                    if bit == 0 {
                        map.next_value::<IgnoredAny>()?;
                    } else {
                        map.next_value::<ArrayShape>()?;
                        arrays |= bit;
                    }
                }
                // This is the existing integrity envelope contract. The fourth
                // field's name/value and every record require consumer checks.
                if count != 4 || arrays != 7 {
                    return Err(de::Error::custom("invalid inventory shape"));
                }
                Ok(InventoryShape)
            }
        }
        deserializer.deserialize_map(Shape)
    }
}
