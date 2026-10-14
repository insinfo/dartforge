//! Content identities for immutable numeric vectors in a fixed AOT image.
//! These constants already lack address identity (`unnamed_addr`). The emitter
//! additionally checks the actual AOT lifecycle before using shared linkage;
//! reloadable code and its shared SDK keep module-private storage.

pub(crate) fn numeric_vector_pooling_enabled() -> bool {
    std::env::var("DARTFORGE_AOT_CONST_ARRAY_POOLING").as_deref() != Ok("0")
}

fn digest(element_type: &str, count: usize, values: &[i64]) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    hash.update(b"dartforge.numeric-vector.v1\0");
    hash.update(&(element_type.len() as u64).to_le_bytes());
    hash.update(element_type.as_bytes());
    hash.update(&(count as u64).to_le_bytes());
    for value in values {
        hash.update(&value.to_le_bytes());
    }
    hash.finalize()
}

pub(super) fn symbol(values: &[i64]) -> String {
    format!("df.const.i64.{}.{}", values.len(), digest("i64", values.len(), values).to_hex())
}

#[cfg(test)]
mod tests;
