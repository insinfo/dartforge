//! Bounded scratch space for byte-exact inventory integrity checks.
use std::io::{self, BufWriter, Write};
use std::path::Path;

const BUFFER_BYTES: usize = 64 * 1024;
#[path = "/tmp/forge-borrowed-keys/canonical-after.rs"]
mod canonical_stream;

/// Check canonical JSON without constructing a Value for the document.
pub(super) fn canonical_document_matches(bytes: &[u8]) -> bool {
    canonical_stream::matches(bytes)
}

pub(super) fn digest_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(&mut file).ok()?;
    Some(hasher.finalize().to_hex().to_string())
}

struct DigestWriter<W> {
    target: W,
    hasher: blake3::Hasher,
}
impl<W: Write> Write for DigestWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.target.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.target.flush()
    }
}

/// Hash exactly the bytes successfully flushed to the target, including one LF.
/// A partial write, serialization error or failed flush cannot publish a digest.
pub(super) fn write_json<W: Write>(
    target: W,
    value: &serde_json::Value,
) -> io::Result<blake3::Hash> {
    let target = DigestWriter {
        target,
        hasher: blake3::Hasher::new(),
    };
    let mut writer = BufWriter::with_capacity(BUFFER_BYTES, target);
    serde_json::to_writer(&mut writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(writer.get_ref().hasher.finalize())
}

struct Comparison<'a> {
    remaining: &'a [u8],
}
impl Write for Comparison<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.remaining.starts_with(bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "noncanonical inventory JSON",
            ));
        }
        self.remaining = &self.remaining[bytes.len()..];
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Compare canonical serialization against the verified read without an output
/// string proportional to the inventory. Whitespace and duplicate keys still fail.
pub(super) fn canonical_matches(bytes: &[u8], value: &serde_json::Value) -> bool {
    let mut writer = BufWriter::with_capacity(BUFFER_BYTES, Comparison { remaining: bytes });
    serde_json::to_writer(&mut writer, value).is_ok()
        && writer.write_all(b"\n").is_ok()
        && writer.flush().is_ok()
        && writer.get_ref().remaining.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inventory_io_canonical_bytes_and_hashes_match_existing_format() {
        for value in [
            json!(null),
            json!({"z":[true,1,-2,1.25],"a":"é\n\"\\\0"}),
            json!({"large":"abcé\"".repeat(BUFFER_BYTES)}),
        ] {
            let expected = format!("{}\n", serde_json::to_string(&value).unwrap()).into_bytes();
            let mut actual = Vec::new();
            let digest = write_json(&mut actual, &value).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(digest, blake3::hash(&expected));
            assert!(canonical_matches(&expected, &value));
            assert!(!canonical_matches(&expected[..expected.len() - 1], &value));
            let mut extra = expected.clone();
            extra.push(b'\n');
            assert!(!canonical_matches(&extra, &value));
            let mut changed = expected;
            changed[0] = b' ';
            assert!(!canonical_matches(&changed, &value));
        }
    }

    #[test]
    fn inventory_io_rejects_noncanonical_and_duplicate_input() {
        for text in [
            "{\"a\":1,\"a\":1}\n",
            "{\"a\":{\"x\":0,\"x\":1}}\n",
            "{ \"a\":1}\n",
            "{\"z\":0,\"a\":1}\n",
            "{\"a\":1}\r\n",
            "{\"a\":1e0}\n",
        ] {
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(!canonical_matches(text.as_bytes(), &value), "{text}");
        }
    }

    #[test]
    fn inventory_io_hashes_empty_large_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("artifact");
        assert_eq!(digest_file(&file), None);
        for bytes in [vec![], vec![b'x'; BUFFER_BYTES * 3 + 19]] {
            std::fs::write(&file, &bytes).unwrap();
            assert_eq!(
                digest_file(&file).unwrap(),
                blake3::hash(&bytes).to_hex().as_str()
            );
        }
    }

    struct ShortWriter {
        bytes: Vec<u8>,
        fail_write: bool,
        fail_flush: bool,
    }
    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                return Err(io::Error::other("injected write failure"));
            }
            let count = bytes.len().min(3);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn inventory_io_short_writes_and_errors_do_not_produce_incomplete_digests() {
        let value = json!({"text":"a".repeat(BUFFER_BYTES*2)});
        let expected = format!("{value}\n");
        let mut writer = ShortWriter {
            bytes: vec![],
            fail_write: false,
            fail_flush: false,
        };
        assert_eq!(
            write_json(&mut writer, &value).unwrap(),
            blake3::hash(expected.as_bytes())
        );
        assert_eq!(writer.bytes, expected.as_bytes());
        writer.fail_write = true;
        assert!(write_json(&mut writer, &value).is_err());
        writer.fail_write = false;
        writer.fail_flush = true;
        assert!(write_json(&mut writer, &value).is_err());
    }
}
