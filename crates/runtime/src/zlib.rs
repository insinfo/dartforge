// Runtime nativo: os filtros zlib do `dart:io` (`ZLibEncoder`,
// `ZLibDecoder`, `GZipCodec`, `RawZLibFilter`) — os natives `Filter_*` do
// `filter_patch.dart` da VM (`runtime/bin/filter.cc`), sobre o `flate2` com o
// zlib clássico (o da VM), compilado da fonte e ligado estaticamente no
// runtime: a mesma saída comprimida do `dart run`, sem zlib na máquina.
//
// O protocolo da VM: `process(dados, início, fim)` entrega entrada ao
// filtro; `processed(flush:, end:)` devolve a saída disponível, ou `null`
// quando não há mais — quem chama repete até o `null`. A compressão usa o
// descarte síncrono (`Z_SYNC_FLUSH`) no `flush` e fecha o fluxo no `end`.
// A descompressão detecta zlib ou gzip pelo cabeçalho (o `windowBits + 32`
// da VM); `raw` é o deflate cru dos dois lados.
//
// O filtro mora no campo nativo do objeto (`NativeFieldWrapperClass1`),
// como a `Box` de um `FiltroZlib`, solta pelo finalizador do objeto.

use std::io::Write as _;

/// A compressão, no formato pedido.
enum Compressor {
    Zlib(flate2::write::ZlibEncoder<Vec<u8>>),
    Gzip(flate2::write::GzEncoder<Vec<u8>>),
    Cru(flate2::write::DeflateEncoder<Vec<u8>>),
}

/// A descompressão; o formato de um fluxo não cru sai do cabeçalho.
enum Descompressor {
    /// Ainda sem os dois primeiros bytes (zlib ou gzip?).
    Detectando(Vec<u8>),
    Zlib(flate2::write::ZlibDecoder<Vec<u8>>),
    Gzip(flate2::write::MultiGzDecoder<Vec<u8>>),
    Cru(flate2::write::DeflateDecoder<Vec<u8>>),
}

enum FiltroZlib {
    Compressao(Option<Compressor>),
    Descompressao(Descompressor),
}

impl Compressor {
    fn saida(&mut self) -> &mut Vec<u8> {
        match self {
            Compressor::Zlib(c) => c.get_mut(),
            Compressor::Gzip(c) => c.get_mut(),
            Compressor::Cru(c) => c.get_mut(),
        }
    }
    fn escrever(&mut self, b: &[u8]) -> std::io::Result<()> {
        match self {
            Compressor::Zlib(c) => c.write_all(b),
            Compressor::Gzip(c) => c.write_all(b),
            Compressor::Cru(c) => c.write_all(b),
        }
    }
    fn descarregar(&mut self) -> std::io::Result<()> {
        match self {
            Compressor::Zlib(c) => c.flush(),
            Compressor::Gzip(c) => c.flush(),
            Compressor::Cru(c) => c.flush(),
        }
    }
    fn terminar(self) -> std::io::Result<Vec<u8>> {
        match self {
            Compressor::Zlib(c) => c.finish(),
            Compressor::Gzip(c) => c.finish(),
            Compressor::Cru(c) => c.finish(),
        }
    }
}

impl Descompressor {
    fn escrever(&mut self, b: &[u8]) -> std::io::Result<()> {
        if let Descompressor::Detectando(pendente) = self {
            pendente.extend_from_slice(b);
            if pendente.len() < 2 {
                return Ok(());
            }
            let pendente = std::mem::take(pendente);
            *self = if pendente[0] == 0x1f && pendente[1] == 0x8b {
                Descompressor::Gzip(flate2::write::MultiGzDecoder::new(Vec::new()))
            } else {
                Descompressor::Zlib(flate2::write::ZlibDecoder::new(Vec::new()))
            };
            return self.escrever(&pendente);
        }
        match self {
            Descompressor::Zlib(d) => d.write_all(b),
            Descompressor::Gzip(d) => d.write_all(b),
            Descompressor::Cru(d) => d.write_all(b),
            Descompressor::Detectando(_) => unreachable!(),
        }
    }
    fn descarregar(&mut self) -> std::io::Result<()> {
        match self {
            Descompressor::Zlib(d) => d.flush(),
            Descompressor::Gzip(d) => d.flush(),
            Descompressor::Cru(d) => d.flush(),
            Descompressor::Detectando(_) => Ok(()),
        }
    }
    fn tirar_saida(&mut self) -> Vec<u8> {
        match self {
            Descompressor::Zlib(d) => std::mem::take(d.get_mut()),
            Descompressor::Gzip(d) => std::mem::take(d.get_mut()),
            Descompressor::Cru(d) => std::mem::take(d.get_mut()),
            Descompressor::Detectando(_) => Vec::new(),
        }
    }
}

fn filtro_zlib(obj: i64) -> Option<&'static mut FiltroZlib> {
    let p = campo_nativo(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: o campo guarda a `Box` de `instalar_filtro_zlib`, usada só pela
    // thread do isolado dono e viva até a coleta do objeto.
    Some(unsafe { &mut *(p as *mut FiltroZlib) })
}

fn liberar_filtro_zlib(p: usize) {
    // SAFETY: a `Box` do filtro, solta uma vez pelo finalizador.
    unsafe { drop(Box::from_raw(p as *mut FiltroZlib)) };
}

fn instalar_filtro_zlib(this: i64, f: FiltroZlib) {
    let p = Box::into_raw(Box::new(f)) as i64;
    anexar_finalizador(this, liberar_filtro_zlib, p as usize);
    gravar_campo_nativo(this, p);
}

/// O nível do `ZLibOption` (-1 = o padrão do zlib, 6) para o `flate2`.
fn nivel_zlib(nivel: i64) -> flate2::Compression {
    if (0..=9).contains(&nivel) { flate2::Compression::new(nivel as u32) } else { flate2::Compression::default() }
}

/// O dicionário de `ZLibOption.dictionary` não é suportado pelo backend em
/// Rust: `UnsupportedError`, como um recurso que a plataforma não tem.
fn recusar_dicionario(dicionario: i64) -> bool {
    if dicionario == 0 {
        return false;
    }
    let msg = alocar_str("ZLibOption.dictionary não é suportado pelo dartforge");
    let e = com_raizes(&[msg], || dartforge_unsupported_error_new(msg));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
    true
}

/// `Filter_CreateZLibDeflate(gzip, level, windowBits, memLevel, strategy,
/// dictionary, raw)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Filter_CreateZLibDeflate(
    this: i64,
    gzip: u8,
    nivel: i64,
    _window_bits: i64,
    _mem_level: i64,
    _estrategia: i64,
    dicionario: i64,
    cru: u8,
) {
    if recusar_dicionario(dicionario) {
        return;
    }
    let n = nivel_zlib(nivel);
    let c = if cru != 0 {
        Compressor::Cru(flate2::write::DeflateEncoder::new(Vec::new(), n))
    } else if gzip != 0 {
        Compressor::Gzip(flate2::write::GzEncoder::new(Vec::new(), n))
    } else {
        Compressor::Zlib(flate2::write::ZlibEncoder::new(Vec::new(), n))
    };
    instalar_filtro_zlib(this, FiltroZlib::Compressao(Some(c)));
}

/// `Filter_CreateZLibInflate(gzip, windowBits, dictionary, raw)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Filter_CreateZLibInflate(this: i64, _gzip: u8, _window_bits: i64, dicionario: i64, cru: u8) {
    if recusar_dicionario(dicionario) {
        return;
    }
    let d = if cru != 0 {
        Descompressor::Cru(flate2::write::DeflateDecoder::new(Vec::new()))
    } else {
        Descompressor::Detectando(Vec::new())
    };
    instalar_filtro_zlib(this, FiltroZlib::Descompressao(d));
}

/// Os bytes de uma `List<int>` (lista tipada, ou lista de inteiros
/// truncados a 8 bits, como o `Dart_ListGetAsBytes` da VM).
fn bytes_de_lista_de_int(h: i64) -> Vec<u8> {
    if let Some(b) = bytes_da_lista_tipada(h) {
        return b;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.try_get(h) {
            Some(Value::List(itens)) => itens
                .iter()
                .map(|v| if v.is_ref { heap.int_de_ref(v.bits).unwrap_or(0) as u8 } else { v.bits as u8 })
                .collect(),
            _ => Vec::new(),
        }
    })
}

fn lancar_dados_invalidos() {
    let e = allocate_format_exception("Filter error, bad data");
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

/// `Filter_Process(data, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Filter_Process(this: i64, dados: i64, inicio: i64, fim: i64) {
    let Some(f) = filtro_zlib(this) else { return lancar_erro_interno("filtro zlib fechado") };
    let bytes = bytes_de_lista_de_int(dados);
    let fim = (fim.max(0) as usize).min(bytes.len());
    let inicio = (inicio.max(0) as usize).min(fim);
    let r = match f {
        FiltroZlib::Compressao(Some(c)) => c.escrever(&bytes[inicio..fim]),
        FiltroZlib::Compressao(None) => Err(std::io::Error::other("fechado")),
        FiltroZlib::Descompressao(d) => d.escrever(&bytes[inicio..fim]),
    };
    if r.is_err() {
        lancar_dados_invalidos();
    }
}

/// `Filter_Processed({flush, end})`: a saída disponível, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Filter_Processed(this: i64, descarregar: u8, terminar: u8) -> i64 {
    let Some(f) = filtro_zlib(this) else { return 0 };
    let saida = match f {
        FiltroZlib::Compressao(c) => {
            if terminar != 0 {
                match c.take().map(Compressor::terminar) {
                    Some(Ok(v)) => v,
                    Some(Err(_)) => return { lancar_dados_invalidos(); 0 },
                    None => Vec::new(),
                }
            } else {
                let Some(c) = c else { return 0 };
                if descarregar != 0 && c.descarregar().is_err() {
                    lancar_dados_invalidos();
                    return 0;
                }
                std::mem::take(c.saida())
            }
        }
        FiltroZlib::Descompressao(d) => {
            if (descarregar != 0 || terminar != 0) && d.descarregar().is_err() {
                lancar_dados_invalidos();
                return 0;
            }
            d.tirar_saida()
        }
    };
    if saida.is_empty() { 0 } else { dart_bytes(saida) }
}
