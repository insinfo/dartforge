//! A forma comprimida da tabela da RTI (`emit_native/src/lower/rti.rs`,
//! `dartforge_rti_iniciar_tabela`): o texto das classes e das regras de
//! supertipo de um programa grande passa de 1 MB (1,37 MB no
//! `new_sali/backend`, 10 mil classes) e é muito repetitivo; o zlib o
//! reduz a ~1/5. O compilador comprime, o runtime descomprime uma vez por
//! processo. Os dois lados estão aqui, para o formato ficar num lugar só.

/// O cabeçalho da forma comprimida: estes 4 bytes, o tamanho do texto (u32
/// little-endian) e o fluxo zlib. O texto da tabela nunca começa com NUL.
pub const HEADER: &[u8; 4] = b"\0DFZ";

/// A partir deste tamanho o compilador comprime a tabela.
pub const LIMIAR: usize = 16 * 1024;

/// A forma comprimida de `text` (o compilador).
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
    out.extend_from_slice(&u32::try_from(text.len()).expect("tabela da RTI acima de 4 GB").to_le_bytes());
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

/// O texto de uma tabela comprimida por [`comprimir`].
///
/// # Panics
///
/// O cabeçalho ou o fluxo zlib não confere.
pub fn decompress(data: &[u8]) -> Vec<u8> {
    assert!(data.starts_with(HEADER), "tabela da RTI sem o cabeçalho da forma comprimida");
    let size = u32::from_le_bytes(data[4..8].try_into().expect("cabeçalho da tabela da RTI")) as usize;
    let mut text = Vec::with_capacity(size);
    let mut z = flate2::Decompress::new(true);
    let estado = z.decompress_vec(&data[8..], &mut text, flate2::FlushDecompress::Finish).expect("tabela da RTI comprimida");
    assert!(estado == flate2::Status::StreamEnd && text.len() == size, "tabela da RTI comprimida incompleta");
    text
}

/// O texto de uma tabela comprimida que vive o processo inteiro (a constante
/// do módulo), descomprimido uma vez: cada isolado registra o universo de
/// novo. A chave é o endereço, o tamanho e o CRC dos dados: a recarga do JIT
/// traz outra tabela, que pode reusar o endereço da anterior.
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
