//! The compressed form of the RTI table (`emit_native/src/lower/rti.rs`,
//! `dartforge_rti_init_table`): the text of the classes and supertype rules of
//! a large program exceeds 1 MB (1.37 MB in `new_sali/backend`, 10 thousand
//! classes) and is very repetitive; zlib reduces it to ~1/5. The compiler
//! compresses, the runtime decompresses once per process. Both sides are here, so
//! the format lives in a single place.

/// The header of the compressed form: these 4 bytes, the text size (u32
/// little-endian) and the zlib stream. The table text never starts with NUL.
pub const HEADER: &[u8; 4] = b"\0DFZ";

/// From this size on the compiler compresses the table.
pub const LIMIAR: usize = 16 * 1024;

/// The compressed form of `text` (the compiler).
///
/// ```
/// use dartforge_runtime::tabela_rti::{comprimir, decompress, HEADER};
/// let text = b"C1 0 Null\nB1 dart:core Null\n".repeat(100);
/// let z = comprimir(&text);
/// assert!(z.starts_with(HEADER) && z.len() < text.len());
/// assert_eq!(decompress(&z), text);
/// ```
pub fn comprimir(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 4 + 64);
    out.extend_from_slice(HEADER);
    out.extend_from_slice(&u32::try_from(text.len()).expect("RTI table above 4 GB").to_le_bytes());
    let mut z = flate2::Compress::new(flate2::Compression::best(), true);
    loop {
        out.reserve(64 * 1024);
        let read_so_far = usize::try_from(z.total_in()).expect("tamanho");
        let estado = z.compress_vec(&text[read_so_far..], &mut out, flate2::FlushCompress::Finish).expect("zlib");
        if estado == flate2::Status::StreamEnd {
            return out;
        }
    }
}

/// The text of a table compressed by [`comprimir`].
///
/// # Panics
///
/// The header or the zlib stream does not check out.
pub fn decompress(data: &[u8]) -> Vec<u8> {
    assert!(data.starts_with(HEADER), "RTI table without the header of the compressed form");
    let size = u32::from_le_bytes(data[4..8].try_into().expect("RTI table header")) as usize;
    let mut text = Vec::with_capacity(size);
    let mut z = flate2::Decompress::new(true);
    let estado = z.decompress_vec(&data[8..], &mut text, flate2::FlushDecompress::Finish).expect("compressed RTI table");
    assert!(estado == flate2::Status::StreamEnd && text.len() == size, "incomplete compressed RTI table");
    text
}

/// The text of a compressed table that lives for the whole process (the module
/// constant), decompressed once: each isolate registers the universe again. The
/// key is the address, the size and the CRC of the data: a JIT reload brings
/// another table, which may reuse the address of the previous one.
pub fn decompressed(data: &'static [u8]) -> &'static [u8] {
    type Cache = std::sync::Mutex<Vec<((usize, usize, u32), &'static [u8])>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut crc = flate2::Crc::new();
    crc.update(data);
    let key = (data.as_ptr() as usize, data.len(), crc.sum());
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, t)) = cache.iter().find(|(k, _)| *k == key) {
        return t;
    }
    let t: &'static [u8] = Box::leak(decompress(data).into_boxed_slice());
    cache.push((key, t));
    t
}
