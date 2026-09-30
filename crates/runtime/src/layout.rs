//! O contrato de layout do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2 e §3.1).
//!
//! Toda constante numérica que o código gerado usa para ler ou gravar um bloco do
//! heap sai daqui: o runtime a usa como `crate::layout` e o emissor como
//! `dartforge_runtime::layout`. O módulo não depende de nenhum outro (é compilado
//! também dentro do `RUNTIME_MAIN`, o texto que o AOT passa ao `rustc` avulso).
//!
//! Um bloco é um cabeçalho de 16 bytes ([`Cabecalho`]) seguido do corpo, em palavras
//! de 8 bytes. O handle de um bloco é `bloco + 2` ([`DESLOCAMENTO_DO_HANDLE`]); o
//! `Smi` é ímpar e o null é `0` ([`smi`]). O coletor percorre o corpo pelo formato
//! que está em `flags` ([`flags::FORMA`]), sem olhar a classe.

/// Um valor em posição de referência: `0` (null), `Smi` (ímpar) ou handle de bloco
/// (`bloco + 2`).
pub type Ref = i64;

/// O cabeçalho de todo bloco do espaço de objetos (16 bytes, layout C fixo: contrato
/// com o emissor).
///
/// * `estado`: [`estado::LIVRE`], [`estado::JOVEM`], [`estado::VELHO`],
///   [`estado::LEMBRADO`] ou [`estado::PERMANENTE`];
/// * `flags`: os bits de [`flags`] (corpo de fora, formato do corpo, cartões, anexo,
///   forma compacta, memória externa);
/// * `n`: em `INSTANCIA`, o número de campos; em `BRUTO`/`REFS`, as palavras do corpo
///   (saturado em `u16::MAX` no objeto grande: a região sabe o real);
/// * `class_id`: o cid;
/// * `mapa`: em `INSTANCIA`, os bits de referência dos 32 primeiros campos; nas
///   strings, o hash (30 bits, 0 = ainda não calculado); nos demais, 0;
/// * `metadado`: a RTI, `id + 1` (0 = nenhuma).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cabecalho {
    pub estado: u8,
    pub flags: u8,
    pub n: u16,
    pub class_id: i32,
    pub mapa: u32,
    pub metadado: u32,
}

const _: () = {
    assert!(std::mem::size_of::<Cabecalho>() == TAMANHO_DO_CABECALHO);
    assert!(std::mem::align_of::<Cabecalho>() == 4);
    assert!(std::mem::offset_of!(Cabecalho, estado) == desl::ESTADO);
    assert!(std::mem::offset_of!(Cabecalho, flags) == desl::FLAGS);
    assert!(std::mem::offset_of!(Cabecalho, n) == desl::N);
    assert!(std::mem::offset_of!(Cabecalho, class_id) == desl::CLASSE);
    assert!(std::mem::offset_of!(Cabecalho, mapa) == desl::MAPA);
    assert!(std::mem::offset_of!(Cabecalho, metadado) == desl::METADADO);
};

/// Bytes do cabeçalho.
pub const TAMANHO_DO_CABECALHO: usize = 16;
/// O handle de um bloco é `bloco + DESLOCAMENTO_DO_HANDLE`.
pub const DESLOCAMENTO_DO_HANDLE: i64 = 2;
/// Tamanho (e alinhamento) de uma página do espaço de objetos.
pub const PAGINA: usize = 64 * 1024;
/// O começo de cada página com o mapa de marcas (um bit por palavra da página).
pub const CABECA_DA_PAGINA: usize = PAGINA / 64;
/// Maior classe de tamanho exata, em palavras do corpo.
pub const MAIOR_CLASSE: usize = 64;
/// As classes de tamanho médias, em palavras do corpo: cada uma enche a página
/// (`k` blocos de `16 + 8w` bytes nos `PAGINA - CABECA_DA_PAGINA` úteis; ver §2.6).
pub const CLASSES_MEDIAS: [usize; 24] = [
    70, 82, 98, 110, 124, 142, 166, 199, 222, 250, 286, 334, 401, 446, 502, 574, 670, 804, 894, 1006, 1150, 1342,
    1610, 2014,
];
/// Maior corpo (em palavras) fora de uma região grande.
pub const MAIOR_MEDIA: usize = 2014;
/// Quantas classes de tamanho: as exatas `1..=64` e as médias; o índice 0 não é usado.
pub const N_CLASSES: usize = 1 + MAIOR_CLASSE + CLASSES_MEDIAS.len();
/// Maior corpo (em palavras) com alocação em linha pela TLAB (`tlab[w]`, `w` em
/// `1..=TLAB_N`).
pub const TLAB_N: usize = 16;
/// Maior índice de campo que o emissor lê em linha (acima dele, a chamada
/// `dartforge_object_get`/`_set`).
pub const CAMPOS_EM_LINHA: usize = 4096;
/// Elementos por cartão de uma lista grande (o `kSlotsPerCardLog2 = 5` da VM).
pub const ELEMENTOS_POR_CARTAO: usize = 32;
/// Elementos cobertos por uma palavra de cartões (64 cartões).
pub const ELEMENTOS_POR_PALAVRA_DE_CARTAO: usize = 2048;
/// O primeiro cid das classes que não estão em [`cid::DO_SDK`].
pub const PRIMEIRO_CID_LIVRE: i64 = 128;

const _: () = {
    assert!(N_CLASSES == 89);
    assert!(CLASSES_MEDIAS[CLASSES_MEDIAS.len() - 1] == MAIOR_MEDIA);
    assert!(ELEMENTOS_POR_PALAVRA_DE_CARTAO == 64 * ELEMENTOS_POR_CARTAO);
    assert!(TLAB_N <= MAIOR_CLASSE);
};

/// Estados de um bloco ([`Cabecalho::estado`]). O 2 é reservado (o `MARCADO` que o
/// coletor de hoje ainda usa na coleta em curso).
pub mod estado {
    /// Bloco livre (na lista livre ou nunca usado).
    pub const LIVRE: u8 = 0;
    /// Alocado desde a última coleta.
    pub const JOVEM: u8 = 1;
    /// Sobreviveu a uma coleta.
    pub const VELHO: u8 = 3;
    /// Velho que recebeu referência para um jovem depois da última coleta (a
    /// barreira o pôs na lista dos lembrados).
    pub const LEMBRADO: u8 = 4;
    /// Objeto estático, fora do heap (seção da imagem): nunca marcado, varrido,
    /// lembrado nem gravado.
    pub const PERMANENTE: u8 = 5;
}

/// Bits de [`Cabecalho::flags`].
pub mod flags {
    /// Os campos (`INSTANCIA`) moram num corpo de fora.
    pub const FORA: u8 = 0x01;
    /// A máscara do formato do corpo.
    pub const FORMA: u8 = 0x06;
    /// Campos com o mapa de referências (o coletor segue os bits acesos).
    pub const INSTANCIA: u8 = 0x00;
    /// Bytes sem referência (o coletor não segue nada).
    pub const BRUTO: u8 = 0x02;
    /// Palavra 0 bruta; palavras 1.. são `Ref` (o coletor segue as pares não nulas).
    pub const REFS: u8 = 0x04;
    /// A lista grande tem cartões depois do último elemento.
    pub const CARTOES: u8 = 0x08;
    /// O corpo guarda um ponteiro nativo (`b+16`) solto quando o dono morre.
    pub const ANEXO: u8 = 0x10;
    /// A máscara da forma compacta de uma `_List`/`_ImmutableList` `BRUTO`.
    pub const ELEMENTO: u8 = 0x60;
    /// Elementos `i64`.
    pub const ELEMENTO_INT: u8 = 0x20;
    /// Elementos com os bits de `f64`.
    pub const ELEMENTO_DOUBLE: u8 = 0x40;
    /// Elementos 0/1.
    pub const ELEMENTO_BOOL: u8 = 0x60;
    /// Lista tipada sobre memória de fora (`asTypedList`).
    pub const EXTERNO: u8 = 0x80;
}

/// Os cids fixos (§2.4): iguais no runtime, no emissor, no JIT e em toda geração.
pub mod cid {
    pub const NULL: i32 = 1;
    pub const SMI: i32 = 2;
    pub const MINT: i32 = 3;
    pub const DOUBLE: i32 = 4;
    pub const BOOL: i32 = 5;
    pub const ONE_BYTE_STRING: i32 = 6;
    pub const TWO_BYTE_STRING: i32 = 7;
    pub const LIST: i32 = 8;
    pub const IMMUTABLE_LIST: i32 = 9;
    pub const GROWABLE_LIST: i32 = 10;
    pub const CLOSURE: i32 = 11;
    pub const RECORD: i32 = 12;
    pub const CONTEXTO: i32 = 13;
    pub const CELULA: i32 = 14;
    pub const ACUMULADOR_DE_TEXTO: i32 = 15;
    pub const PROGRAMA_DE_REGEXP: i32 = 16;
    pub const SEND_PORT: i32 = 17;
    pub const CAPABILITY: i32 = 18;
    pub const FLOAT32X4: i32 = 19;
    pub const INT32X4: i32 = 20;
    pub const FLOAT64X2: i32 = 21;
    /// `_Int8List` + t, para t = `TIPO_*` 0–13 de `typed_data.rs`.
    pub const PRIMEIRA_TIPADA: i32 = 22;
    /// `_Int8ArrayView` + t.
    pub const PRIMEIRA_VISAO: i32 = 36;
    /// `_UnmodifiableInt8ArrayView` + t.
    pub const PRIMEIRA_VISAO_IMUTAVEL: i32 = 50;
    pub const BYTE_DATA_VIEW: i32 = 64;
    pub const UNMODIFIABLE_BYTE_DATA_VIEW: i32 = 65;
    /// Quantos tipos de elemento de lista tipada (`TIPO_INT8` … `TIPO_FLOAT64X2`).
    pub const TIPOS_DE_ELEMENTO: i32 = 14;

    /// A biblioteca das classes internas do runtime (sem declaração em Dart): o
    /// cid fica reservado e a numeração do emissor não acha a classe no SDK.
    pub const INTERNA: &str = "";

    /// `(cid, biblioteca, classe)` de toda classe com cid fixo, em ordem de cid: a
    /// fonte da numeração do emissor (`context.rs`) e da conferência de ABI
    /// (`dartforge_registrar_cids`). A biblioteca é a URI (`dart:core`), a chave da
    /// numeração estável; as internas do runtime têm [`INTERNA`].
    pub const DO_SDK: &[(i32, &str, &str)] = &[
        (NULL, "dart:core", "Null"),
        (SMI, "dart:core", "_Smi"),
        (MINT, "dart:core", "_Mint"),
        (DOUBLE, "dart:core", "_Double"),
        (BOOL, "dart:core", "bool"),
        (ONE_BYTE_STRING, "dart:core", "_OneByteString"),
        (TWO_BYTE_STRING, "dart:core", "_TwoByteString"),
        (LIST, "dart:core", "_List"),
        (IMMUTABLE_LIST, "dart:core", "_ImmutableList"),
        (GROWABLE_LIST, "dart:core", "_GrowableList"),
        (CLOSURE, "dart:core", "_Closure"),
        (RECORD, "dart:core", "_Record"),
        (CONTEXTO, INTERNA, "_Contexto"),
        (CELULA, INTERNA, "_Celula"),
        (ACUMULADOR_DE_TEXTO, INTERNA, "_AcumuladorDeTexto"),
        (PROGRAMA_DE_REGEXP, INTERNA, "_ProgramaDeRegExp"),
        (SEND_PORT, "dart:isolate", "_SendPort"),
        (CAPABILITY, "dart:isolate", "_Capability"),
        (FLOAT32X4, "dart:typed_data", "_Float32x4"),
        (INT32X4, "dart:typed_data", "_Int32x4"),
        (FLOAT64X2, "dart:typed_data", "_Float64x2"),
        (22, "dart:typed_data", "_Int8List"),
        (23, "dart:typed_data", "_Uint8List"),
        (24, "dart:typed_data", "_Uint8ClampedList"),
        (25, "dart:typed_data", "_Int16List"),
        (26, "dart:typed_data", "_Uint16List"),
        (27, "dart:typed_data", "_Int32List"),
        (28, "dart:typed_data", "_Uint32List"),
        (29, "dart:typed_data", "_Int64List"),
        (30, "dart:typed_data", "_Uint64List"),
        (31, "dart:typed_data", "_Float32List"),
        (32, "dart:typed_data", "_Float64List"),
        (33, "dart:typed_data", "_Float32x4List"),
        (34, "dart:typed_data", "_Int32x4List"),
        (35, "dart:typed_data", "_Float64x2List"),
        (36, "dart:typed_data", "_Int8ArrayView"),
        (37, "dart:typed_data", "_Uint8ArrayView"),
        (38, "dart:typed_data", "_Uint8ClampedArrayView"),
        (39, "dart:typed_data", "_Int16ArrayView"),
        (40, "dart:typed_data", "_Uint16ArrayView"),
        (41, "dart:typed_data", "_Int32ArrayView"),
        (42, "dart:typed_data", "_Uint32ArrayView"),
        (43, "dart:typed_data", "_Int64ArrayView"),
        (44, "dart:typed_data", "_Uint64ArrayView"),
        (45, "dart:typed_data", "_Float32ArrayView"),
        (46, "dart:typed_data", "_Float64ArrayView"),
        (47, "dart:typed_data", "_Float32x4ArrayView"),
        (48, "dart:typed_data", "_Int32x4ArrayView"),
        (49, "dart:typed_data", "_Float64x2ArrayView"),
        (50, "dart:typed_data", "_UnmodifiableInt8ArrayView"),
        (51, "dart:typed_data", "_UnmodifiableUint8ArrayView"),
        (52, "dart:typed_data", "_UnmodifiableUint8ClampedArrayView"),
        (53, "dart:typed_data", "_UnmodifiableInt16ArrayView"),
        (54, "dart:typed_data", "_UnmodifiableUint16ArrayView"),
        (55, "dart:typed_data", "_UnmodifiableInt32ArrayView"),
        (56, "dart:typed_data", "_UnmodifiableUint32ArrayView"),
        (57, "dart:typed_data", "_UnmodifiableInt64ArrayView"),
        (58, "dart:typed_data", "_UnmodifiableUint64ArrayView"),
        (59, "dart:typed_data", "_UnmodifiableFloat32ArrayView"),
        (60, "dart:typed_data", "_UnmodifiableFloat64ArrayView"),
        (61, "dart:typed_data", "_UnmodifiableFloat32x4ArrayView"),
        (62, "dart:typed_data", "_UnmodifiableInt32x4ArrayView"),
        (63, "dart:typed_data", "_UnmodifiableFloat64x2ArrayView"),
        (BYTE_DATA_VIEW, "dart:typed_data", "_ByteDataView"),
        (UNMODIFIABLE_BYTE_DATA_VIEW, "dart:typed_data", "_UnmodifiableByteDataView"),
    ];

    /// O cid da lista tipada interna do tipo `tipo` (`TIPO_*`).
    pub const fn tipada(tipo: u8) -> i32 {
        PRIMEIRA_TIPADA + tipo as i32
    }

    /// O cid da visão do tipo `tipo`, modificável ou não.
    pub const fn visao(tipo: u8, imutavel: bool) -> i32 {
        (if imutavel { PRIMEIRA_VISAO_IMUTAVEL } else { PRIMEIRA_VISAO }) + tipo as i32
    }

    /// `_OneByteString` ou `_TwoByteString`.
    pub const fn e_texto(c: i32) -> bool {
        (c.wrapping_sub(ONE_BYTE_STRING) as u32) < 2
    }

    /// `_List` ou `_ImmutableList`.
    pub const fn e_lista_fixa(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 2
    }

    /// Lista do núcleo: `_List`, `_ImmutableList` ou `_GrowableList`.
    pub const fn e_lista(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 3
    }

    /// Lista tipada interna (`_Int8List` … `_Float64x2List`).
    pub const fn e_tipada_interna(c: i32) -> bool {
        (c.wrapping_sub(PRIMEIRA_TIPADA) as u32) < TIPOS_DE_ELEMENTO as u32
    }

    /// Qualquer lista tipada ou visão (22..=65).
    pub const fn e_tipada(c: i32) -> bool {
        (c.wrapping_sub(PRIMEIRA_TIPADA) as u32) < 44
    }

    /// Visão não modificável (`_UnmodifiableXArrayView`, `_UnmodifiableByteDataView`).
    pub const fn e_visao_imutavel(c: i32) -> bool {
        (c.wrapping_sub(PRIMEIRA_VISAO_IMUTAVEL) as u32) < TIPOS_DE_ELEMENTO as u32 || c == UNMODIFIABLE_BYTE_DATA_VIEW
    }

    /// Valor SIMD (`_Float32x4`, `_Int32x4`, `_Float64x2`).
    pub const fn e_simd(c: i32) -> bool {
        (c.wrapping_sub(FLOAT32X4) as u32) < 3
    }
}

/// Deslocamentos a partir do bloco (o handle é bloco + 2: a palavra em `b + k` está
/// em `h + k - 2`).
pub mod desl {
    pub const ESTADO: usize = 0;
    pub const FLAGS: usize = 1;
    pub const N: usize = 2;
    pub const CLASSE: usize = 4;
    pub const MAPA: usize = 8;
    /// O hash das strings mora no lugar do mapa.
    pub const HASH_DO_TEXTO: usize = 8;
    pub const METADADO: usize = 12;
    /// A primeira palavra do corpo.
    pub const CORPO: usize = 16;
    /// Strings, `_List`, `_ImmutableList`, `_GrowableList`, listas tipadas e visões.
    pub const COMPRIMENTO: usize = 16;
    /// As unidades de uma string.
    pub const UNIDADES: usize = 24;
    /// O primeiro elemento de uma `_List`/`_ImmutableList`.
    pub const ELEMENTOS: usize = 24;
    /// O valor de `_Mint`, `_Double`, `bool`, e as pistas SIMD.
    pub const VALOR: usize = 16;
    /// O endereço dos bytes de uma lista tipada (interna, externa ou visão).
    pub const DADOS: usize = 24;
    /// Os bytes de uma lista tipada interna.
    pub const BYTES_INTERNOS: usize = 32;
    pub const BASE_DA_VISAO: usize = 32;
    pub const DESLOCAMENTO_DA_VISAO: usize = 40;
    /// O armazenamento (`_List`) de uma `_GrowableList`.
    pub const EXPANSIVEL_DADOS: usize = 24;
    pub const CLOSURE_CODIGO: usize = 16;
    pub const CLOSURE_CONTEXTO: usize = 24;
    pub const CLOSURE_TIPADO: usize = 32;
    pub const CLOSURE_ABI: usize = 40;
    pub const RECORD_FORMA: usize = 16;
    pub const RECORD_CAMPOS: usize = 24;
    /// O ponteiro nativo de um bloco com `ANEXO`.
    pub const ANEXO: usize = 16;
}

/// Deslocamentos do `Contexto` da thread (§3.4).
pub mod contexto {
    pub const PENDENTE: usize = 0;
    pub const TOPO: usize = 8;
    pub const AREAS: usize = 16;
    pub const N_AREAS: usize = 24;
    pub const INTERRUPCAO: usize = 32;
    pub const VAZIOS: usize = 40;
    pub const REGISTRADAS: usize = 48;
    pub const N_REGISTRADAS: usize = 56;
    /// O cursor da TLAB de `w` palavras em `TLAB + 16·w`, o fim em `TLAB + 16·w + 8`,
    /// `w` em `1..=TLAB_N`.
    pub const TLAB: usize = 64;
    pub const SUBTIPOS: usize = 336;
    pub const N_SUBTIPOS: usize = 344;
    pub const LARGURA_SUBTIPOS: usize = 352;
    /// Os handles das caixas estáticas de `true` e `false` do runtime da thread
    /// (o código gerado os lê daqui, e não dos símbolos de dados: entre o
    /// executável e a DLL do SDK só o runtime ativo vale, §4.10 item 9).
    pub const VERDADEIRO: usize = 360;
    pub const FALSO: usize = 368;

    /// O cursor da TLAB de `w` palavras.
    pub const fn tlab_cursor(w: usize) -> usize {
        TLAB + 16 * w
    }

    /// O fim da TLAB de `w` palavras.
    pub const fn tlab_fim(w: usize) -> usize {
        TLAB + 16 * w + 8
    }

    const _: () = assert!(tlab_fim(super::TLAB_N) + 8 == SUBTIPOS);
}

/// `Ref` com `Smi` etiquetado (R10, docs/NATIVO-PLANO.md §6.2 e §7.1).
///
/// Um `Ref` do código gerado é um `i64` com a etiqueta no bit baixo, como o `Smi`
/// da VM (`runtime/vm/object.h`; lá `kSmiTag = 0` e o ponteiro do heap tem o bit 1 —
/// aqui o bit está invertido para o `0` continuar sendo null):
///
/// * `0` — null;
/// * **par** e positivo — um handle do heap (`bloco + 2`, [`e_objeto`]);
/// * **ímpar** — um `int` pequeno, `(v << 1) | 1`, para `v` em `[-2^62, 2^62)`: o
///   `Smi`. Não aloca, não é raiz, e o coletor nunca o segue.
///
/// Um `int` fora dessa faixa numa posição `Ref` vai para o heap como `_Mint`. A
/// forma é canônica: um valor que cabe no `Smi` nunca é encaixotado.
pub mod smi {
    /// Menor e maior `int` que cabem num `Smi` (63 bits com sinal).
    pub const MIN: i64 = -(1 << 62);
    pub const MAX: i64 = (1 << 62) - 1;

    /// O `Ref` de um `int` que cabe num `Smi`.
    #[inline]
    pub fn de(v: i64) -> Option<i64> {
        (MIN..=MAX).contains(&v).then_some((v << 1) | 1)
    }

    /// O `Ref` é um `Smi`?
    #[inline]
    pub fn e_smi(r: i64) -> bool {
        r & 1 == 1
    }

    /// O valor de um `Smi` (deslocamento aritmético).
    #[inline]
    pub fn valor(r: i64) -> i64 {
        r >> 1
    }

    /// O `Ref` aponta para o heap (não é null nem `Smi`)?
    #[inline]
    pub fn e_handle(r: i64) -> bool {
        r != 0 && r & 1 == 0
    }
}

/// O `Ref` é o handle de um bloco (`bloco + 2`, blocos alinhados a 8)? Negativo
/// nunca é objeto.
#[inline]
pub fn e_objeto(h: Ref) -> bool {
    h & (7 | i64::MIN) == 2
}

/// Palavras de extensão do mapa de referências para `n` campos (os 32 primeiros
/// bits estão no cabeçalho).
#[inline]
pub const fn palavras_do_mapa(n: usize) -> usize {
    if n > 32 { (n - 32).div_ceil(64) } else { 0 }
}

/// Campos que o bloco guarda: pelo menos um (o lugar do encadeamento da lista livre
/// e do endereço do corpo de fora).
#[inline]
pub const fn capacidade(n: usize) -> usize {
    if n == 0 { 1 } else { n }
}

/// Palavras do corpo de uma `INSTANCIA` de `n` campos.
#[inline]
pub const fn palavras_de_instancia(n: usize) -> usize {
    capacidade(n) + palavras_do_mapa(n)
}

/// Palavras do corpo de uma string de `len` unidades: o comprimento e as unidades
/// (1 ou 2 bytes cada), completadas com zeros até a palavra.
#[inline]
pub const fn palavras_de_texto(len: usize, dois: bool) -> usize {
    1 + (len * if dois { 2 } else { 1 }).div_ceil(8)
}

/// A lista de `len` elementos tem cartões (é objeto grande)?
#[inline]
pub const fn tem_cartoes(len: usize) -> bool {
    1 + len > MAIOR_MEDIA
}

/// Palavras de cartões depois do último elemento de uma lista grande.
#[inline]
pub const fn palavras_de_cartoes(len: usize) -> usize {
    if tem_cartoes(len) { len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) } else { 0 }
}

/// Palavras do corpo de uma `_List`/`_ImmutableList` de `len` elementos: o
/// comprimento, os elementos e, se grande, os cartões.
#[inline]
pub const fn palavras_de_lista(len: usize) -> usize {
    1 + len + palavras_de_cartoes(len)
}

/// Palavras do corpo de uma lista tipada interna de `len` elementos de
/// `tamanho_do_elemento` bytes: o comprimento, o endereço dos dados e os bytes.
#[inline]
pub const fn palavras_de_tipada(len: usize, tamanho_do_elemento: usize) -> usize {
    2 + (len * tamanho_do_elemento).div_ceil(8)
}

/// A classe de tamanho de um corpo de `palavras` palavras: `1..=64` exatas (0
/// palavras usa a 1), `65..=88` médias; `None` acima de [`MAIOR_MEDIA`] (região
/// grande).
#[inline]
pub const fn classe_de_tamanho(palavras: usize) -> Option<usize> {
    if palavras <= MAIOR_CLASSE {
        return Some(if palavras == 0 { 1 } else { palavras });
    }
    let mut i = 0;
    while i < CLASSES_MEDIAS.len() {
        if palavras <= CLASSES_MEDIAS[i] {
            return Some(MAIOR_CLASSE + 1 + i);
        }
        i += 1;
    }
    None
}

/// As palavras do corpo de um bloco da classe `classe` (`1..N_CLASSES`).
#[inline]
pub const fn palavras_da_classe(classe: usize) -> usize {
    if classe <= MAIOR_CLASSE { classe } else { CLASSES_MEDIAS[classe - MAIOR_CLASSE - 1] }
}

/// Bytes de um bloco com corpo de `palavras` palavras.
#[inline]
pub const fn bytes_do_bloco(palavras: usize) -> usize {
    TAMANHO_DO_CABECALHO + 8 * palavras
}

/// A palavra 0 do cabeçalho, como a alocação em linha a grava:
/// `estado | flags << 8 | n << 16 | cid << 32`, com `n` saturado em `u16::MAX`
/// (em `INSTANCIA`, `n` é o número de campos; nos demais, as palavras do corpo).
#[inline]
pub const fn palavra_do_cabecalho(estado: u8, flags: u8, n: usize, cid: i32) -> u64 {
    let n = if n > u16::MAX as usize { u16::MAX as u64 } else { n as u64 };
    estado as u64 | (flags as u64) << 8 | n << 16 | (cid as u32 as u64) << 32
}

/// O `hashCode` de uma string: o `StringHasher` da VM (`runtime/vm/hash.h`,
/// `CombineHashes` por unidade e `FinalizeHash` em 30 bits), 0 trocado por 1. É o
/// que o runtime grava em `mapa` e o que o emissor grava nos literais estáticos.
pub fn hash_de_texto(unidades: impl IntoIterator<Item = u16>) -> u32 {
    let mut h: u32 = 0;
    for u in unidades {
        h = h.wrapping_add(u32::from(u));
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h = h.wrapping_add(h << 15);
    h &= (1u32 << 30) - 1;
    if h == 0 { 1 } else { h }
}

#[cfg(test)]
mod testes_layout {
    use super::*;

    #[test]
    fn layout_classe_de_tamanho_monotona_e_cobre() {
        let mut anterior = 0;
        for w in 0..=MAIOR_MEDIA {
            let c = classe_de_tamanho(w).expect("classe de corpo até a maior média");
            assert!(c >= 1 && c < N_CLASSES, "classe {c} de {w} palavras fora da faixa");
            assert!(c >= anterior, "classe_de_tamanho não é monótona em {w}");
            assert!(palavras_da_classe(c) >= w, "a classe {c} não cabe {w} palavras");
            // A menor classe que cabe: a anterior não cabe.
            if c > 1 {
                assert!(palavras_da_classe(c - 1) < w.max(1), "classe {c} não é a menor para {w}");
            }
            anterior = c;
        }
        assert_eq!(classe_de_tamanho(MAIOR_MEDIA + 1), None);
        assert_eq!(classe_de_tamanho(0), Some(1));
        assert_eq!(classe_de_tamanho(64), Some(64));
        assert_eq!(classe_de_tamanho(65), Some(65));
        assert_eq!(palavras_da_classe(N_CLASSES - 1), MAIOR_MEDIA);
    }

    #[test]
    fn layout_classes_medias_enchem_a_pagina() {
        let uteis = PAGINA - CABECA_DA_PAGINA;
        let blocos = [112, 96, 80, 72, 64, 56, 48, 40, 36, 32, 28, 24, 20, 18, 16, 14, 12, 10, 9, 8, 7, 6, 5, 4];
        for (i, &w) in CLASSES_MEDIAS.iter().enumerate() {
            let k = uteis / bytes_do_bloco(w);
            assert_eq!(k, blocos[i], "classe média de {w} palavras");
            // A maior que cabe o mesmo número de blocos: uma palavra a mais perde um.
            assert!(uteis / bytes_do_bloco(w + 1) < k, "a classe de {w} palavras não enche a página");
        }
        // Perda interna máxima de 25% (entre 1610 e 2014).
        let mut anterior = MAIOR_CLASSE;
        for &w in &CLASSES_MEDIAS {
            assert!((w - anterior - 1) * 4 <= w, "perda interna acima de 25% antes de {w}");
            anterior = w;
        }
    }

    #[test]
    fn layout_hash_de_texto_igual_ao_da_vm() {
        for s in ["", "a", "abc", "hello world", "ç€😀", "0123456789abcdef0123456789"] {
            let t = crate::heap::Texto::de_str(s);
            let u: Vec<u16> = s.encode_utf16().collect();
            assert_eq!(i64::from(hash_de_texto(u.iter().copied())), t.hash_vm(), "hash de {s:?}");
        }
        // Surrogate solto e unidade acima de 0xFF.
        let u = vec![0xD800, 0x61, 0x1234, 0xFF];
        let t = crate::heap::Texto::de_unidades(u.clone());
        assert_eq!(i64::from(hash_de_texto(u)), t.hash_vm());
        assert_ne!(hash_de_texto([]), 0);
        assert!(hash_de_texto("xyz".encode_utf16()) < 1 << 30);
    }

    #[test]
    fn layout_palavra_do_cabecalho_e_handles() {
        let p = palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, 3, cid::ONE_BYTE_STRING);
        assert_eq!(p, 1 | 0x02 << 8 | 3 << 16 | 6 << 32);
        assert_eq!(palavra_do_cabecalho(estado::PERMANENTE, 0, 100_000, 200) >> 16 & 0xFFFF, 0xFFFF);
        assert_eq!(palavra_do_cabecalho(0, 0, 0, -1) >> 32, 0xFFFF_FFFF);
        assert!(e_objeto(0x1000 + DESLOCAMENTO_DO_HANDLE));
        assert!(!e_objeto(0));
        assert!(!e_objeto(0x1000));
        assert!(!e_objeto(0x1004));
        assert!(!e_objeto(0x1006));
        assert!(!e_objeto(smi::de(7).unwrap()));
        assert!(!e_objeto(i64::MIN | 2));
        assert_eq!(smi::valor(smi::de(-5).unwrap()), -5);
        assert_eq!(smi::de(smi::MAX + 1), None);
    }

    #[test]
    fn layout_tamanhos_dos_corpos() {
        assert_eq!(palavras_de_texto(0, false), 1);
        assert_eq!(palavras_de_texto(8, false), 2);
        assert_eq!(palavras_de_texto(9, false), 3);
        assert_eq!(palavras_de_texto(4, true), 2);
        assert_eq!(palavras_de_texto(5, true), 3);
        assert_eq!(palavras_de_lista(0), 1);
        assert_eq!(palavras_de_lista(MAIOR_MEDIA - 1), MAIOR_MEDIA);
        assert!(!tem_cartoes(MAIOR_MEDIA - 1));
        assert!(tem_cartoes(MAIOR_MEDIA));
        assert_eq!(palavras_de_lista(MAIOR_MEDIA), 1 + MAIOR_MEDIA + 1);
        assert_eq!(palavras_de_lista(4096), 1 + 4096 + 2);
        assert_eq!(palavras_de_tipada(3, 1), 3);
        assert_eq!(palavras_de_tipada(2, 8), 4);
        assert_eq!(palavras_de_instancia(0), 1);
        assert_eq!(palavras_de_instancia(33), 34);
    }

    #[test]
    fn layout_cids_do_sdk_em_ordem_e_predicados() {
        for (i, &(c, _, _)) in cid::DO_SDK.iter().enumerate() {
            assert_eq!(c, i as i32 + 1, "DO_SDK fora de ordem na posição {i}");
            assert!(i64::from(c) < PRIMEIRO_CID_LIVRE);
        }
        assert_eq!(cid::DO_SDK.len(), cid::UNMODIFIABLE_BYTE_DATA_VIEW as usize);
        assert_eq!(cid::tipada(1), 23);
        assert_eq!(cid::DO_SDK[cid::tipada(13) as usize - 1].2, "_Float64x2List");
        assert_eq!(cid::DO_SDK[cid::visao(1, false) as usize - 1].2, "_Uint8ArrayView");
        assert_eq!(cid::DO_SDK[cid::visao(13, true) as usize - 1].2, "_UnmodifiableFloat64x2ArrayView");
        for c in 0..200 {
            assert_eq!(cid::e_texto(c), c == 6 || c == 7);
            assert_eq!(cid::e_lista_fixa(c), c == 8 || c == 9);
            assert_eq!(cid::e_lista(c), (8..=10).contains(&c));
            assert_eq!(cid::e_tipada_interna(c), (22..=35).contains(&c));
            assert_eq!(cid::e_tipada(c), (22..=65).contains(&c));
            assert_eq!(cid::e_visao_imutavel(c), (50..=63).contains(&c) || c == 65);
            assert_eq!(cid::e_simd(c), (19..=21).contains(&c));
        }
    }
}
