//! A emissão das strings (P1, docs/NATIVO-ESPACO-UNIFICADO.md §2.11, §3.5 e
//! §4.3): os literais (`Const(String/StringWtf8)`), a interpolação
//! (`JuntarTextos`) e os ajudantes `@df.texto_*` do prelúdio.
//!
//! Uma string é um bloco `BRUTO` de cid 6 (`_OneByteString`) ou 7
//! (`_TwoByteString`): com o handle `h = bloco + 2`, o cid em `h+2` (`i32`), o
//! hash em `h+6` (`i32`, 0 = não calculado), o comprimento em `h+14` (`i64`) e as
//! unidades a partir de `h+22` (`layout::desl`).
//!
//! **Literal no AOT** (`objetos_estaticos`): um objeto estático do módulo,
//! `@df.s.<chave>`, `linkonce_odr` num `comdat` pela chave do conteúdo (o ligador
//! mantém uma cópia só entre o programa e os módulos do SDK: `identical` de
//! literais iguais vale entre bibliotecas), estado `PERMANENTE`, na seção da
//! imagem, com o hash já calculado (`layout::hash_de_texto`). O valor é a
//! constante `ptrtoint (gep @df.s.<chave>, 2)`: sem chamada nem cache no ponto
//! de uso. **No JIT** a memória de uma geração é liberada (J02), então o literal
//! continua internado no heap pelo runtime (`dartforge_string_new`) com o cache
//! do ponto de uso na área do isolado.
//!
//! Os ajudantes são `define internal … alwaysinline`; os números deles (cids,
//! deslocamentos, palavra do cabeçalho) são os de `layout`, conferidos pelo teste
//! `ajudantes_de_texto_seguem_o_layout`.

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, desl, estado, flags};
use std::fmt::Write;

/// Os ajudantes `@df.texto_*` (§3.5), impressos no prelúdio de cada módulo por
/// `emit_runtime_decls`.
///
/// * `@df.texto_len(s)`: o comprimento (`s` é string);
/// * `@df.texto_unidade(s, i)`: a unidade `i`, sem conferir limites;
/// * `@df.texto_alocar(len, dois)`: string de `len` unidades zeradas, em linha
///   por `@df.alocar` até 16 palavras, senão `@dartforge_texto_novo`;
/// * `@df.texto_gravar(s, i, u)`: o `writeInto*String` (grava sem barreira: a
///   string é `BRUTO` e ainda não foi publicada);
/// * `@df.texto_hash(s)`: o hash do cabeçalho, ou `@dartforge_texto_hash` na
///   primeira consulta;
/// * `@df.texto_igual(a, b)`: `==` entre duas strings (identidade; comprimento e
///   hashes calculados diferentes dão falso sem chamada);
/// * `@df.texto_igual_a(a, b)`: `==` de uma string com um `Object?` (o
///   `_StringBase.==` da sobreposição): o que não é string é diferente.
pub const AJUDANTES: &str = "\
define internal i64 @df.texto_len(i64 %s) alwaysinline nounwind {
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  %n = load i64, ptr %q, align 8
  ret i64 %n
}
define internal i64 @df.texto_unidade(i64 %s, i64 %i) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %b1 = load i8, ptr %a1, align 1
  %r1 = zext i8 %b1 to i64
  ret i64 %r1
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %b2 = load i16, ptr %a2, align 2
  %r2 = zext i16 %b2 to i64
  ret i64 %r2
}
define internal i64 @df.texto_alocar(i64 %len, i1 %dois) alwaysinline {
entrada:
  %d = zext i1 %dois to i64
  %bytes = shl i64 %len, %d
  %b7 = add i64 %bytes, 7
  %pw = lshr i64 %b7, 3
  %w = add i64 %pw, 1
  %grande = icmp ugt i64 %w, 16
  br i1 %grande, label %lento, label %rapido
rapido:
  %c = add i64 %d, 6
  %cs = shl i64 %c, 32
  %ws = shl i64 %w, 16
  %c0 = or i64 %cs, %ws
  %cab = or i64 %c0, 513
  %h = call i64 @df.alocar(i64 %cab, i64 %w)
  %p = inttoptr i64 %h to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  store i64 %len, ptr %q, align 8
  ret i64 %h
lento:
  %hl = call i64 @dartforge_texto_novo(i64 %len, i64 %d)
  ret i64 %hl
}
define internal void @df.texto_gravar(i64 %s, i64 %i, i64 %x) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %t1 = trunc i64 %x to i8
  store i8 %t1, ptr %a1, align 1
  ret void
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %t2 = trunc i64 %x to i16
  store i16 %t2, ptr %a2, align 2
  ret void
}
define internal i64 @df.texto_hash(i64 %s) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 6
  %x = load i32, ptr %q, align 4
  %z = icmp eq i32 %x, 0
  %ze = call i1 @llvm.expect.i1(i1 %z, i1 false)
  br i1 %ze, label %lento, label %rapido
rapido:
  %r = zext i32 %x to i64
  ret i64 %r
lento:
  %l = call i64 @dartforge_texto_hash(i64 %s)
  ret i64 %l
}
define internal i1 @df.texto_igual(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %compr
compr:
  %pa = inttoptr i64 %a to ptr
  %pb = inttoptr i64 %b to ptr
  %qa = getelementptr inbounds i8, ptr %pa, i64 14
  %qb = getelementptr inbounds i8, ptr %pb, i64 14
  %la = load i64, ptr %qa, align 8
  %lb = load i64, ptr %qb, align 8
  %ml = icmp eq i64 %la, %lb
  br i1 %ml, label %hashes, label %nao
hashes:
  %ra = getelementptr inbounds i8, ptr %pa, i64 6
  %rb = getelementptr inbounds i8, ptr %pb, i64 6
  %ha = load i32, ptr %ra, align 4
  %hb = load i32, ptr %rb, align 4
  %az = icmp eq i32 %ha, 0
  %bz = icmp eq i32 %hb, 0
  %mh = icmp eq i32 %ha, %hb
  %algum = or i1 %az, %bz
  %talvez = or i1 %algum, %mh
  br i1 %talvez, label %lento, label %nao
lento:
  %r = call i8 @dartforge_texto_iguais(i64 %a, i64 %b)
  %rb1 = icmp ne i8 %r, 0
  ret i1 %rb1
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i1 @df.texto_igual_a(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %objeto
objeto:
  %t = and i64 %b, -9223372036854775801
  %e = icmp eq i64 %t, 2
  br i1 %e, label %classe, label %nao
classe:
  %pb = inttoptr i64 %b to ptr
  %pc = getelementptr inbounds i8, ptr %pb, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %c6 = sub i32 %c, 6
  %texto = icmp ult i32 %c6, 2
  br i1 %texto, label %comparar, label %nao
comparar:
  %r = call i1 @df.texto_igual(i64 %a, i64 %b)
  ret i1 %r
sim:
  ret i1 true
nao:
  ret i1 false
}
";

/// Os efeitos de cada ajudante `@df.*` deste arquivo chamado por
/// `CallRuntime`: `(nome, aloca, lança)`. `externs::efeitos_de` os consulta
/// antes da tabela de externs (um ajudante fora daqui é conservador).
pub(super) const EFEITOS_DOS_AJUDANTES: &[(&str, bool, bool)] = &[
    ("df.texto_len", false, false),
    ("df.texto_unidade", false, false),
    ("df.texto_alocar", true, false),
    ("df.texto_gravar", false, false),
    ("df.texto_hash", false, false),
    ("df.texto_igual", false, false),
    ("df.texto_igual_a", false, false),
];

/// O estado da emissão de strings por módulo.
#[derive(Default)]
pub(super) struct EstadoDeTextos {
    /// Os literais estáticos usados pelo módulo: a chave (`@df.s.<chave>`) e a
    /// definição do global, escritos no fim do módulo em ordem de chave.
    literais: std::collections::BTreeMap<String, String>,
}

/// A seção da imagem (§2.11) no formato de objeto do hospedeiro.
fn secao_da_imagem() -> &'static str {
    match crate::alvo::sistema() {
        crate::alvo::Sistema::Windows => ".dfimg$m",
        crate::alvo::Sistema::MacOs => "__DATA_CONST,__dfimg",
        crate::alvo::Sistema::Linux => "dfimg",
    }
}

/// Um literal estático: a chave e a definição do global.
fn literal_estatico(bytes: &[u8]) -> (String, String) {
    let texto = dartforge_runtime::heap::Texto::de_wtf8(bytes);
    let vista = texto.vista();
    let dois = !vista.e_um_byte();
    let c = if dois { cid::TWO_BYTE_STRING } else { cid::ONE_BYTE_STRING };
    let len = vista.len();
    let w = layout::palavras_de_texto(len, dois);
    // As unidades em bytes (little-endian), completadas com zeros até a palavra.
    let mut unidades: Vec<u8> = match vista {
        dartforge_runtime::textos::TextoRef::Um(b) => b.to_vec(),
        dartforge_runtime::textos::TextoRef::Dois(u) => u.iter().flat_map(|x| x.to_le_bytes()).collect(),
    };
    unidades.resize(8 * (w - 1), 0);
    let mut h = blake3::Hasher::new();
    h.update(&c.to_le_bytes());
    h.update(&unidades[..if dois { 2 * len } else { len }]);
    let chave = h.finalize().to_hex()[..32].to_string();
    let cabecalho = layout::palavra_do_cabecalho(estado::PERMANENTE, flags::BRUTO, w, c);
    let hash = layout::hash_de_texto(vista.unidades());
    let n = unidades.len();
    let corpo = if n == 0 {
        "zeroinitializer".to_string()
    } else {
        let mut s = String::from("c\"");
        for &b in &unidades {
            if b.is_ascii_graphic() && b != b'"' && b != b'\\' || b == b' ' {
                s.push(b as char);
            } else {
                write!(s, "\\{b:02X}").unwrap();
            }
        }
        s.push('"');
        s
    };
    let comdat = if crate::alvo::tem_comdat() { ", comdat" } else { "" };
    let mut def = String::new();
    if crate::alvo::tem_comdat() {
        writeln!(def, "$\"df.s.{chave}\" = comdat any").unwrap();
    }
    writeln!(
        def,
        "@\"df.s.{chave}\" = linkonce_odr constant {{ i64, i64, i64, [{n} x i8] }} {{ i64 {}, i64 {hash}, i64 {len}, [{n} x i8] {corpo} }}{comdat}, section \"{}\", align 8",
        cabecalho as i64,
        secao_da_imagem()
    )
    .unwrap();
    (chave, def)
}

impl LlvmEmitter<'_> {
    /// O literal de string `bytes` (WTF-8) em `%v<v>`.
    pub(super) fn emitir_const_string(&mut self, v: u32, bytes: &[u8]) {
        if self.objetos_estaticos {
            let (chave, def) = literal_estatico(bytes);
            self.textos.literais.entry(chave.clone()).or_insert(def);
            writeln!(
                self.out,
                "  %v{v} = ptrtoint ptr getelementptr inbounds (i8, ptr @\"df.s.{chave}\", i64 {}) to i64",
                layout::DESLOCAMENTO_DO_HANDLE
            )
            .unwrap();
            return;
        }
        let idx = self.string_const_index(bytes).unwrap_or(0);
        let len = bytes.len();
        // JIT: o handle do literal fica num cache do ponto de uso, na área do
        // isolado (o literal é canônico e permanente; a recarga do JIT
        // esvazia os caches): a busca no runtime só na primeira avaliação.
        let slot = self.slot_de_cache();
        let anterior = self.rotulo_atual.clone();
        let o = &mut self.out;
        writeln!(o, "  %lsp{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %lsv{v} = load i64, ptr %lsp{v}, align 8").unwrap();
        writeln!(o, "  %lsz{v} = icmp eq i64 %lsv{v}, 0").unwrap();
        writeln!(o, "  %lsx{v} = call i1 @llvm.expect.i1(i1 %lsz{v}, i1 false)").unwrap();
        writeln!(o, "  br i1 %lsx{v}, label %ls{v}.nova, label %ls{v}.fim").unwrap();
        writeln!(o, "ls{v}.nova:").unwrap();
        writeln!(o, "  %lsn{v} = call i64 @dartforge_string_new(ptr @.str.{idx}, i64 {len})").unwrap();
        writeln!(o, "  store i64 %lsn{v}, ptr %lsp{v}, align 8").unwrap();
        writeln!(o, "  br label %ls{v}.fim").unwrap();
        writeln!(o, "ls{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %lsv{v}, %{anterior} ], [ %lsn{v}, %ls{v}.nova ]").unwrap();
        self.rotulo_atual = format!("ls{v}.fim");
    }

    /// O `alloca` do bloco de entrada que `JuntarTextos` usa (se usar).
    pub(super) fn buffer_de_juntar_textos(&mut self, v: u32, partes: &[Operand]) {
        writeln!(self.out, "  %jbuf{v} = alloca [{} x i64]", 2 * partes.len().max(1)).unwrap();
    }

    /// A interpolação: as partes (texto `Ref` ou `int` sem caixa) numa string.
    /// O runtime (`dartforge_string_juntar_tipado`) mede as partes pelas vistas
    /// dos blocos, aloca uma vez e copia as unidades, sem cópia intermediária.
    pub(super) fn emitir_juntar_textos(&mut self, v: u32, partes: &[Operand]) {
        // Pares (espécie, bits): 0 e o `Ref` de um texto, 1 e um `int`.
        let n = partes.len();
        for (i, p) in partes.iter().enumerate() {
            let (especie, s) = if self.tipo_de(p) == Type::I64 {
                (1, self.coagir(p, Type::I64))
            } else {
                (0, self.coagir(p, Type::Ref))
            };
            writeln!(self.out, "  %jk{v}_{i} = getelementptr [{} x i64], ptr %jbuf{v}, i64 0, i64 {}", 2 * n, 2 * i).unwrap();
            writeln!(self.out, "  store i64 {especie}, ptr %jk{v}_{i}").unwrap();
            writeln!(self.out, "  %jp{v}_{i} = getelementptr [{} x i64], ptr %jbuf{v}, i64 0, i64 {}", 2 * n, 2 * i + 1).unwrap();
            writeln!(self.out, "  store i64 {s}, ptr %jp{v}_{i}").unwrap();
        }
        writeln!(self.out, "  %v{v} = call i64 @dartforge_string_juntar_tipado(ptr %jbuf{v}, i64 {n})").unwrap();
    }

    /// Os globais de nível de módulo das strings: os literais estáticos usados
    /// pelo módulo (com o `comdat` de cada um), escritos no fim do módulo.
    pub(super) fn emitir_globais_de_texto(&mut self) {
        let literais = std::mem::take(&mut self.textos.literais);
        if literais.is_empty() {
            return;
        }
        self.out.push_str("; Literais de string estáticos (docs/NATIVO-ESPACO-UNIFICADO.md §2.11)\n");
        for def in literais.values() {
            self.out.push_str(def);
        }
        self.out.push('\n');
    }
}

// Confere que os números escritos à mão em `AJUDANTES` são os de `layout`.
const _: () = {
    assert!(desl::CLASSE as i64 - layout::DESLOCAMENTO_DO_HANDLE == 2);
    assert!(desl::HASH_DO_TEXTO as i64 - layout::DESLOCAMENTO_DO_HANDLE == 6);
    assert!(desl::COMPRIMENTO as i64 - layout::DESLOCAMENTO_DO_HANDLE == 14);
    assert!(desl::UNIDADES as i64 - layout::DESLOCAMENTO_DO_HANDLE == 22);
    assert!(cid::ONE_BYTE_STRING == 6 && cid::TWO_BYTE_STRING == 7);
    assert!(layout::TLAB_N == 16);
    assert!(layout::palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, 0, 0) == 513);
};

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ajudantes_de_texto_seguem_o_layout() {
        // A máscara de `e_objeto` (`h & (7 | i64::MIN) == 2`).
        assert!(AJUDANTES.contains(&format!("and i64 %b, {}", 7 | i64::MIN)));
        // A palavra do cabeçalho montada em `@df.texto_alocar` é a de `layout`.
        for (len, dois) in [(0usize, false), (5, false), (9, true), (60, false)] {
            let w = layout::palavras_de_texto(len, dois);
            let c = if dois { cid::TWO_BYTE_STRING } else { cid::ONE_BYTE_STRING };
            let montada = (c as u64) << 32 | (w as u64) << 16 | 513;
            assert_eq!(montada, layout::palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, w, c));
            // A conta de `w` do ajudante: 1 + ((len << dois) + 7) >> 3.
            assert_eq!(1 + (((len << u32::from(dois)) + 7) >> 3), w);
        }
        for (nome, _, _) in EFEITOS_DOS_AJUDANTES {
            assert!(AJUDANTES.contains(&format!("@{nome}(")), "{nome} sem definição");
        }
    }

    #[test]
    fn literal_estatico_na_forma_do_bloco() {
        let (chave, def) = literal_estatico(b"ab");
        assert_eq!(chave.len(), 32);
        let w0 = layout::palavra_do_cabecalho(estado::PERMANENTE, flags::BRUTO, 2, cid::ONE_BYTE_STRING) as i64;
        let hash = layout::hash_de_texto("ab".encode_utf16());
        assert!(def.contains(&format!("{{ i64 {w0}, i64 {hash}, i64 2, [8 x i8] c\"ab\\00\\00\\00\\00\\00\\00\" }}")), "{def}");
        assert!(def.contains("linkonce_odr constant"));
        // A mesma string dá a mesma chave; a forma entra na chave.
        assert_eq!(literal_estatico(b"ab").0, chave);
        assert_ne!(literal_estatico(b"ac").0, chave);
        let (_, dois) = literal_estatico("€".as_bytes());
        let w0 = layout::palavra_do_cabecalho(estado::PERMANENTE, flags::BRUTO, 2, cid::TWO_BYTE_STRING) as i64;
        assert!(dois.contains(&format!("i64 {w0}")), "{dois}");
        assert!(dois.contains("c\"\\AC \\00\\00\\00\\00\\00\\00\""), "{dois}");
        let (_, vazia) = literal_estatico(b"");
        assert!(vazia.contains("[0 x i8] zeroinitializer"), "{vazia}");
    }
}
