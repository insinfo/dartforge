//! A forma comprimida da tabela da RTI (`emit_native/src/lower/rti.rs`,
//! `dartforge_rti_iniciar_tabela`): o texto das classes e das regras de
//! supertipo de um programa grande passa de 1 MB (1,37 MB no
//! `new_sali/backend`, 10 mil classes) e é muito repetitivo; o zlib o
//! reduz a ~1/5. O compilador comprime, o runtime descomprime uma vez por
//! processo. Os dois lados estão aqui, para o formato ficar num lugar só.

/// O cabeçalho da forma comprimida: estes 4 bytes, o tamanho do texto (u32
/// little-endian) e o fluxo zlib. O texto da tabela nunca começa com NUL.
pub const CABECALHO: &[u8; 4] = b"\0DFZ";

/// A partir deste tamanho o compilador comprime a tabela.
pub const LIMIAR: usize = 16 * 1024;

/// A forma comprimida de `texto` (o compilador).
///
/// ```
/// use dartforge_runtime::tabela_rti::{comprimir, descomprimir, CABECALHO};
/// let texto = b"C1 0 Null\nB1 dart:core Null\n".repeat(100);
/// let z = comprimir(&texto);
/// assert!(z.starts_with(CABECALHO) && z.len() < texto.len());
/// assert_eq!(descomprimir(&z), texto);
/// ```
pub fn comprimir(texto: &[u8]) -> Vec<u8> {
    let mut saida = Vec::with_capacity(texto.len() / 4 + 64);
    saida.extend_from_slice(CABECALHO);
    saida.extend_from_slice(&u32::try_from(texto.len()).expect("tabela da RTI acima de 4 GB").to_le_bytes());
    let mut z = flate2::Compress::new(flate2::Compression::best(), true);
    loop {
        saida.reserve(64 * 1024);
        let lido = usize::try_from(z.total_in()).expect("tamanho");
        let estado = z.compress_vec(&texto[lido..], &mut saida, flate2::FlushCompress::Finish).expect("zlib");
        if estado == flate2::Status::StreamEnd {
            return saida;
        }
    }
}

/// O texto de uma tabela comprimida por [`comprimir`].
///
/// # Panics
///
/// O cabeçalho ou o fluxo zlib não confere.
pub fn descomprimir(dados: &[u8]) -> Vec<u8> {
    assert!(dados.starts_with(CABECALHO), "tabela da RTI sem o cabeçalho da forma comprimida");
    let tamanho = u32::from_le_bytes(dados[4..8].try_into().expect("cabeçalho da tabela da RTI")) as usize;
    let mut texto = Vec::with_capacity(tamanho);
    let mut z = flate2::Decompress::new(true);
    let estado = z.decompress_vec(&dados[8..], &mut texto, flate2::FlushDecompress::Finish).expect("tabela da RTI comprimida");
    assert!(estado == flate2::Status::StreamEnd && texto.len() == tamanho, "tabela da RTI comprimida incompleta");
    texto
}

/// O texto de uma tabela comprimida que vive o processo inteiro (a constante
/// do módulo), descomprimido uma vez: cada isolado registra o universo de
/// novo. A chave é o endereço, o tamanho e o CRC dos dados: a recarga do JIT
/// traz outra tabela, que pode reusar o endereço da anterior.
pub fn descomprimida(dados: &'static [u8]) -> &'static [u8] {
    type Cache = std::sync::Mutex<Vec<((usize, usize, u32), &'static [u8])>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut crc = flate2::Crc::new();
    crc.update(dados);
    let chave = (dados.as_ptr() as usize, dados.len(), crc.sum());
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, t)) = cache.iter().find(|(k, _)| *k == chave) {
        return t;
    }
    let t: &'static [u8] = Box::leak(descomprimir(dados).into_boxed_slice());
    cache.push((chave, t));
    t
}
