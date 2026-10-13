//! O mapa de pilha compacto (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.4):
//! o conversor do `.llvm_stackmaps` versão 3 de um objeto COFF para o
//! **DFGM v2**, reescrevendo o objeto no lugar.
//!
//! O formato do LLVM é feito para o patching de um JIT: ~98 bytes por
//! registro. O coletor só precisa, por chamada, do conjunto de slots do
//! quadro que guardam raízes; o DFGM guarda isso em ~8 bytes por registro
//! (deltas em varint, e um bit para "o mesmo conjunto do registro anterior").
//! É o porte do `gcmap.py` do experimento `x12`, que roda.
//!
//! ```text
//! cabeçalho (16 bytes)
//!   "DFGM"; u8 versão = 2; u8 alvo (1 = x86-64, 2 = aarch64); u16 bandeiras
//!   u32 tamanho total do blob (múltiplo de 4); u32 n_funcoes
//! índice: n_funcoes × { u32 endereço da função; u32 início no fluxo }
//!   bandeiras 0 (COFF): rva, relocação `ADDR32NB`
//!   bandeiras 1 (ELF): i32 do endereço da função menos o do próprio campo,
//!     relocação `R_X86_64_PC32` / `R_AARCH64_PREL32` — sem relocação
//!     dinâmica no executável PIE, e a seção pode ser só de leitura
//! fluxo (varints LEB128 sem sinal), por função:
//!   n_registros; tamanho do quadro / 8 (0 = dinâmico)
//!   n_registros × { delta do deslocamento de retorno; cabeçalho }
//!     cabeçalho ímpar: o conjunto de raízes do registro anterior
//!     cabeçalho par: (cabeçalho >> 1) raízes em slots, cada uma um varint `s`:
//!       bit 0 de s: base (0 = SP, 1 = FP)
//!       s >> 1: zigzag do delta do slot, em palavras de 8 bytes
//!     e um varint com a máscara dos registradores (o bit `r` é o
//!     registrador DWARF `r`) que guardam raízes na chamada (v2)
//! zeros até múltiplo de 4
//! ```
//!
//! A v1 (sem a máscara) é a de quando todo valor `gc-live` ia para um slot;
//! o runtime lê as duas. Com as raízes no operando `"deopt"` e
//! `use-registers-for-deopt-values` (`crates/llvm`), a raiz fica no
//! registrador preservado em que já está através da chamada — o local
//! `Register` do LLVM —, sem a gravação num slot a cada ponto de coleta
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12).
//!
//! O conversor troca o conteúdo da seção pelo blob (sempre menor), a renomeia
//! e troca cada relocação de 64 bits (uma por função, no `StkSizeRecord`) por
//! uma de 32 bits no campo do índice. No COFF a seção vira `.dfgcm$m` e o
//! runtime a acha pelo nome na imagem; no ELF vira `dfgcm` (um identificador
//! C: o ligador define `__start_dfgcm`/`__stop_dfgcm`, que o módulo passa ao
//! runtime na partida) com `SHF_GNU_RETAIN`, porque nada a referencia e o
//! `--gc-sections` a descartaria (o Perry mediu isso). O Mach-O não é
//! convertido: o `ld64.lld` não separa os índices do ThinLTO, então a
//! produção usa a LTO do ligador, que emite o mapa no formato do LLVM, e o
//! runtime lê esse formato (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Etapa 4).
//!
//! Regras (as do §14.4 e do §14.12): as raízes são os locais do `"deopt"` e,
//! de cada par (base, derivado), só a base, sem repetição; constantes são
//! descartadas; um local que não seja
//! `Indirect [SP|FP + d]` de 8 bytes com `d` múltiplo de 8, nem `Register`
//! de 8 bytes num registrador preservado pela convenção do alvo, é **erro**
//! (um registrador volátil não sobrevive à chamada); dois
//! registros no mesmo deslocamento de retorno são erro; depois de codificar,
//! o blob é decodificado e comparado com a origem.

/// O registrador DWARF do SP e o do FP no x86-64 e no aarch64.
const REG_SP: u16 = 7;
const REG_FP: u16 = 6;
const REG_SP_AARCH64: u16 = 31;
const REG_FP_AARCH64: u16 = 29;
/// Os registradores DWARF preservados através de uma chamada, por
/// convenção: no Windows x64, rbx, rsi, rdi, rbp e r12–r15; no SysV x86-64,
/// rbx, rbp e r12–r15; no aarch64, x19–x29.
const PRESERVED_WINDOWS: u32 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (0xF << 12);
const PRESERVED_SYSV: u32 = (1 << 3) | (1 << 6) | (0xF << 12);
const PRESERVED_AARCH64: u32 = ((1 << 11) - 1) << 19;
/// O valor que o LLVM grava no mapa para um operando `undef`/`poison`
/// (`StatepointLowering.cpp`): sem valor, não é raiz.
const UNDEFINED: i64 = 0xFEFE_FEFE;
/// O tipo de uma raiz: slot sobre o SP, slot sobre o FP, registrador.
const NO_SP: u8 = 0;
const NO_FP: u8 = 1;
const IN_REGISTER: u8 = 2;
/// A versão do blob que o conversor escreve.
const VERSION: u8 = 2;
/// As bandeiras do blob: o índice com endereços relativos ao próprio campo.
const RELATIVE_FLAG: u16 = 1;
/// `EM_X86_64` e `EM_AARCH64`.
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
/// As relocações ELF de 64 bits absolutas e de 32 bits relativas ao lugar.
const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_PREL32: u32 = 261;
/// `SHF_ALLOC` e `SHF_GNU_RETAIN`.
const SHF_ALLOC: u64 = 0x2;
const SHF_GNU_RETAIN: u64 = 0x20_0000;
/// `IMAGE_FILE_MACHINE_AMD64`.
const MACHINE_X86_64: u16 = 0x8664;
/// `IMAGE_REL_AMD64_ADDR64` e `IMAGE_REL_AMD64_ADDR32NB`.
const REL_ADDR64: u16 = 1;
const REL_ADDR32NB: u16 = 3;
/// `IMAGE_SCN_LNK_NRELOC_OVFL`: o número de relocações não coube em 16 bits.
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;

/// As raízes de um registro: (tipo, deslocamento em bytes do slot ou número
/// DWARF do registrador), ordenadas ([`NO_SP`], [`NO_FP`], [`IN_REGISTER`]).
type Roots = Vec<(u8, i32)>;
/// Uma função do mapa: (símbolo da relocação, tamanho do quadro em bytes — 0
/// se dinâmico —, registros por deslocamento de retorno).
type Function = (u32, u64, Vec<(u32, Roots)>);

pub(crate) fn u16_at(d: &[u8], p: usize) -> Result<u16, String> {
    d.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u32_at(d: &[u8], p: usize) -> Result<u32, String> {
    d.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u64_at(d: &[u8], p: usize) -> Result<u64, String> {
    Ok(u64::from(u32_at(d, p)?) | (u64::from(u32_at(d, p + 4)?) << 32))
}

pub(crate) fn varint(exit: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            exit.push(b | 0x80);
        } else {
            exit.push(b);
            return;
        }
    }
}

pub(crate) fn read_varint(d: &[u8], p: &mut usize) -> Result<u64, String> {
    let mut r = 0u64;
    let mut s = 0u32;
    loop {
        let b = *d.get(*p).ok_or_else(|| "mapa DFGM truncado".to_string())?;
        *p += 1;
        if s < 64 {
            r |= u64::from(b & 0x7f) << s;
        }
        s += 7;
        if b & 0x80 == 0 {
            return Ok(r);
        }
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn without_zigzag(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

/// Decodifica os blobs `.llvm_stackmaps` v3 de `sec`. `symbol_in` dá, pelo
/// deslocamento de cada `StkSizeRecord`, a relocação dele (o símbolo no
/// COFF, o índice da relocação no ELF); `sp` e `fp` são os registradores
/// DWARF do alvo e `preserved`, a máscara dos que sobrevivem à chamada.
fn decode_v3(sec: &[u8], symbol_in: &std::collections::HashMap<u32, u32>, sp: u16, fp: u16, preserved: u32) -> Result<Vec<Function>, String> {
    let mut functions = Vec::new();
    let mut p = 0usize;
    while p + 16 <= sec.len() {
        if sec[p] == 0 {
            p += 1;
            continue;
        }
        if sec[p] != 3 {
            return Err(format!("`.llvm_stackmaps` na versão {} (o conversor lê a 3)", sec[p]));
        }
        let n_functions = u32_at(sec, p + 4)? as usize;
        let n_constants = u32_at(sec, p + 8)? as usize;
        let n_records = u32_at(sec, p + 12)? as usize;
        let table = p + 16;
        let constants = table + 24 * n_functions;
        let mut r = constants + 8 * n_constants;
        let mut total = 0usize;
        for i in 0..n_functions {
            let frame = u64_at(sec, table + 24 * i + 8)?;
            let function_records = u64_at(sec, table + 24 * i + 16)? as usize;
            let symbol = *symbol_in
                .get(&((table + 24 * i) as u32))
                .ok_or_else(|| "função do mapa de pilha sem relocação".to_string())?;
            let mut records: Vec<(u32, Roots)> = Vec::with_capacity(function_records);
            for _ in 0..function_records {
                let offset = u32_at(sec, r + 8)?;
                let n_locals = u16_at(sec, r + 14)? as usize;
                let locals = r + 16;
                // (tipo, tamanho, registrador, deslocamento ou constante)
                let local = |j: usize| -> Result<(u8, u16, u16, i32), String> {
                    let l = locals + 12 * j;
                    let ty = *sec.get(l).ok_or_else(|| "mapa de pilha truncado".to_string())?;
                    Ok((ty, u16_at(sec, l + 2)?, u16_at(sec, l + 4)?, u32_at(sec, l + 8)? as i32))
                };
                if n_locals < 3 || local(0)?.0 != 4 || local(1)?.0 != 4 || local(2)?.0 != 4 {
                    return Err("registro de statepoint sem os três locais constantes iniciais".to_string());
                }
                let n_deopt = local(2)?.3.max(0) as usize;
                if (n_locals - 3).saturating_sub(n_deopt) % 2 != 0 {
                    return Err("registro de statepoint com pares (base, derivado) incompletos".to_string());
                }
                if 3 + n_deopt > n_locals {
                    return Err("registro de statepoint com mais locais de `deopt` que locais".to_string());
                }
                let mut roots: Roots = Vec::new();
                // As raízes: cada local do `"deopt"` (a forma do §14.12) e a
                // base de cada par (base, derivado) do `"gc-live"`.
                for j in (3..3 + n_deopt).chain((3 + n_deopt..n_locals.saturating_sub(1)).step_by(2)) {
                    let (ty, size, register, d) = local(j)?;
                    // Constante: null ou `Smi` (ímpar), ou o `undef` que o
                    // otimizador deixou num operando (0xFEFEFEFE); não é
                    // raiz. Outra constante par não nula seria um valor
                    // bruto tratado como referência: defeito do emissor (§3.5).
                    if ty == 4 || ty == 5 {
                        let c = if ty == 4 {
                            i64::from(d)
                        } else {
                            u64_at(sec, constants + 8 * usize::try_from(d).map_err(|_| "`ConstIndex` negativo no mapa de pilha".to_string())?)? as i64
                        };
                        if c != 0 && c & 1 == 0 && c != UNDEFINED {
                            return Err(format!("constante par não nula ({c:#x}) como raiz no mapa de pilha: um valor bruto tratado como referência"));
                        }
                        continue;
                    }
                    let in_register = ty == 1 && size == 8 && register < 32 && preserved & (1 << register) != 0;
                    if !in_register && (ty != 3 || size != 8 || (register != sp && register != fp)) {
                        return Err(format!(
                            "local recusado no mapa de pilha: tipo {ty}, tamanho {size}, registrador {register} (só `Indirect [SP|FP + d]` de 8 bytes ou `Register` preservado)"
                        ));
                    }
                    if !in_register && d % 8 != 0 {
                        return Err(format!("slot de raiz fora de múltiplo de 8 no mapa de pilha: {d}"));
                    }
                    let root = if in_register {
                        (IN_REGISTER, i32::from(register))
                    } else {
                        (if register == fp { NO_FP } else { NO_SP }, d)
                    };
                    if !roots.contains(&root) {
                        roots.push(root);
                    }
                }
                roots.sort_unstable();
                records.push((offset, roots));
                let mut l = (locals + 12 * n_locals + 7) & !7;
                let n_exits = u16_at(sec, l + 2)? as usize;
                l = (l + 4 + 4 * n_exits + 7) & !7;
                r = l;
            }
            records.sort_by_key(|g| g.0);
            if records.windows(2).any(|par| par[0].0 == par[1].0) {
                return Err("dois registros do mapa de pilha no mesmo endereço de retorno".to_string());
            }
            total += records.len();
            functions.push((symbol, if frame == u64::MAX { 0 } else { frame }, records));
        }
        if total != n_records {
            return Err("o cabeçalho do `.llvm_stackmaps` não casa com os registros".to_string());
        }
        p = r;
    }
    Ok(functions)
}

/// Codifica o blob DFGM v2 com as `flags` e o `alvo` dados; devolve
/// também a posição, no blob, do campo de endereço de cada função (onde vai
/// a relocação).
fn encode(functions: &[Function], flags: u16, alvo: u8) -> (Vec<u8>, Vec<u32>) {
    let mut flow: Vec<u8> = Vec::new();
    let mut starts: Vec<u32> = Vec::with_capacity(functions.len());
    for (_, frame, records) in functions {
        starts.push(flow.len() as u32);
        varint(&mut flow, records.len() as u64);
        varint(&mut flow, frame / 8);
        let mut previous_return = 0u32;
        let mut previous: Option<&Roots> = None;
        for (offset, roots) in records {
            varint(&mut flow, u64::from(offset - previous_return));
            previous_return = *offset;
            if previous == Some(roots) {
                varint(&mut flow, 1);
                continue;
            }
            let n_slots = roots.iter().filter(|r| r.0 != IN_REGISTER).count();
            varint(&mut flow, (n_slots as u64) << 1);
            let mut previous_slot = 0i64;
            let mut mask = 0u64;
            for &(ty, d) in roots {
                if ty == IN_REGISTER {
                    mask |= 1 << d;
                    continue;
                }
                let slot = i64::from(d) / 8;
                varint(&mut flow, (zigzag(slot - previous_slot) << 1) | u64::from(ty == NO_FP));
                previous_slot = slot;
            }
            varint(&mut flow, mask);
            previous = Some(roots);
        }
    }
    while flow.len() % 4 != 0 {
        flow.push(0);
    }
    let header = 16 + 8 * functions.len();
    let mut blob: Vec<u8> = Vec::with_capacity(header + flow.len());
    blob.extend_from_slice(b"DFGM");
    blob.extend_from_slice(&[VERSION, alvo]);
    blob.extend_from_slice(&flags.to_le_bytes());
    blob.extend_from_slice(&((header + flow.len()) as u32).to_le_bytes());
    blob.extend_from_slice(&(functions.len() as u32).to_le_bytes());
    let mut fields = Vec::with_capacity(functions.len());
    for start in &starts {
        fields.push(blob.len() as u32);
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.extend_from_slice(&start.to_le_bytes());
    }
    blob.extend_from_slice(&flow);
    (blob, fields)
}

/// Decodifica um blob DFGM v2: por função, (tamanho do quadro em bytes,
/// registros). É o leitor de referência (o do runtime é o mesmo algoritmo).
fn decode(blob: &[u8]) -> Result<Vec<(u64, Vec<(u32, Roots)>)>, String> {
    if blob.len() < 16 || &blob[..4] != b"DFGM" || blob[4] != VERSION {
        return Err("blob DFGM sem o cabeçalho da versão 2".to_string());
    }
    if u32_at(blob, 8)? as usize != blob.len() {
        return Err("blob DFGM com o tamanho errado no cabeçalho".to_string());
    }
    if u16_at(blob, 6)? & !RELATIVE_FLAG != 0 {
        return Err("blob DFGM com bandeiras desconhecidas".to_string());
    }
    let n_functions = u32_at(blob, 12)? as usize;
    let base = 16 + 8 * n_functions;
    let mut exit = Vec::with_capacity(n_functions);
    for i in 0..n_functions {
        let mut p = base + u32_at(blob, 16 + 8 * i + 4)? as usize;
        let n = read_varint(blob, &mut p)?;
        let frame = read_varint(blob, &mut p)? * 8;
        let mut records = Vec::new();
        let mut ret = 0u32;
        let mut previous: Roots = Vec::new();
        for _ in 0..n {
            ret += read_varint(blob, &mut p)? as u32;
            let c = read_varint(blob, &mut p)?;
            if c & 1 == 0 {
                previous = Vec::new();
                let mut slot = 0i64;
                for _ in 0..(c >> 1) {
                    let s = read_varint(blob, &mut p)?;
                    slot += without_zigzag(s >> 1);
                    previous.push((if s & 1 == 1 { NO_FP } else { NO_SP }, (slot * 8) as i32));
                }
                let mask = read_varint(blob, &mut p)?;
                for r in 0..64 {
                    if mask & (1 << r) != 0 {
                        previous.push((IN_REGISTER, r));
                    }
                }
                previous.sort_unstable();
            }
            records.push((ret, previous.clone()));
        }
        exit.push((frame, records));
    }
    Ok(exit)
}

/// Converte, no lugar, o `.llvm_stackmaps` do objeto COFF x86-64 `object` em
/// `.dfgcm$m`. Devolve `false`, sem mexer no objeto, quando não há o que
/// converter ou o objeto tem uma forma que o conversor não reescreve (COFF
/// grande, relocações além de 16 bits, outra máquina): o mapa do LLVM fica, e
/// o runtime o lê do mesmo jeito (`heap::ler_stackmaps`).
///
/// # Errors
/// Mapa que o runtime não saberia ler (um local que não é slot de quadro), ou
/// objeto malformado.
pub fn convert_coff(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 20 {
        return Ok(false);
    }
    let machine = u16_at(object, 0)?;
    let n_sections = u16_at(object, 2)? as usize;
    if machine != MACHINE_X86_64 {
        return Ok(false);
    }
    let symbols = u32_at(object, 8)? as usize;
    let n_symbols = u32_at(object, 12)? as usize;
    let optional = u16_at(object, 16)? as usize;
    let strings = symbols + 18 * n_symbols;
    for s in 0..n_sections {
        let h = 20 + optional + 40 * s;
        let name = object.get(h..h + 8).ok_or_else(|| "objeto COFF truncado".to_string())?;
        // O nome tem mais de 8 bytes: `/<offset na table de strings>`.
        let is_the_map = match name.strip_prefix(b"/") {
            Some(rest) => {
                let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
                let shift = std::str::from_utf8(&rest[..end]).ok().and_then(|t| t.parse::<usize>().ok());
                shift.is_some_and(|d| object.get(strings + d..).is_some_and(|t| t.starts_with(b".llvm_stackmaps\0")))
            }
            None => false,
        };
        if !is_the_map {
            continue;
        }
        let size = u32_at(object, h + 16)? as usize;
        let data = u32_at(object, h + 20)? as usize;
        let relocations = u32_at(object, h + 24)? as usize;
        let n_relocations = u16_at(object, h + 32)? as usize;
        let features = u32_at(object, h + 36)?;
        if features & SCN_NRELOC_OVFL != 0 {
            return Ok(false);
        }
        let sec = object.get(data..data + size).ok_or_else(|| "objeto COFF truncado".to_string())?.to_vec();
        let mut symbol_in: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_relocations {
            let e = relocations + 10 * k;
            if u16_at(object, e + 8)? != REL_ADDR64 {
                return Err("relocação inesperada no `.llvm_stackmaps` (esperava ADDR64)".to_string());
            }
            symbol_in.insert(u32_at(object, e)?, u32_at(object, e + 4)?);
        }
        let functions = decode_v3(&sec, &symbol_in, REG_SP, REG_FP, PRESERVED_WINDOWS)?;
        let (blob, fields) = encode(&functions, 0, 1);
        // Ida e volta: o que o runtime vai ler é o que o LLVM escreveu.
        let back = decode(&blob)?;
        let equal = back.len() == functions.len()
            && back.iter().zip(&functions).all(|((frame, records), (_, q, r))| *frame == (q / 8) * 8 && records == r);
        if !equal {
            return Err("o mapa de pilha compacto não reproduz o do LLVM (ida e volta)".to_string());
        }
        if blob.len() > size || fields.len() != n_relocations {
            return Ok(false);
        }
        object[data..data + blob.len()].copy_from_slice(&blob);
        object[data + blob.len()..data + size].fill(0);
        object[h + 16..h + 20].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        object[h..h + 8].copy_from_slice(b".dfgcm$m");
        // Alinhamento 4 (`IMAGE_SCN_ALIGN_4BYTES`).
        let new_ones = (features & !0x00F0_0000) | 0x0030_0000;
        object[h + 36..h + 40].copy_from_slice(&new_ones.to_le_bytes());
        for (k, (field, (symbol, _, _))) in fields.iter().zip(&functions).enumerate() {
            let e = relocations + 10 * k;
            object[e..e + 4].copy_from_slice(&field.to_le_bytes());
            object[e + 4..e + 8].copy_from_slice(&symbol.to_le_bytes());
            object[e + 8..e + 10].copy_from_slice(&REL_ADDR32NB.to_le_bytes());
        }
        return Ok(true);
    }
    Ok(false)
}

/// Converte, no lugar, o `.llvm_stackmaps` do objeto ELF de 64 bits
/// (x86-64 ou aarch64) em `dfgcm`, com `SHF_GNU_RETAIN`, alinhamento 4 e o
/// índice relativo ao próprio campo (`R_X86_64_PC32`, `R_AARCH64_PREL32`).
/// Devolve `false`, sem mexer, quando não há mapa. No ELF a conversão é
/// obrigatória: o mapa do LLVM tem relocações absolutas de 64 bits numa
/// seção só de leitura, que o `ld.lld` recusa num executável PIE (a
/// [#75074](https://github.com/llvm/llvm-project/issues/75074)) — uma forma
/// que o conversor não reescreve é erro, não volta ao mapa do LLVM.
///
/// # Errors
/// Mapa que o runtime não saberia ler, objeto malformado ou numa forma que o
/// conversor não reescreve.
pub fn convert_elf(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 64 || &object[..4] != b"\x7fELF" {
        return Ok(false);
    }
    if object[4] != 2 || object[5] != 1 {
        return Err("objeto ELF com raízes por mapas que não é de 64 bits little-endian".to_string());
    }
    let (alvo, sp, fp, preserved, absolute, relative) = match u16_at(object, 0x12)? {
        ELF_X86_64 => (1u8, REG_SP, REG_FP, PRESERVED_SYSV, R_X86_64_64, R_X86_64_PC32),
        ELF_AARCH64 => (2u8, REG_SP_AARCH64, REG_FP_AARCH64, PRESERVED_AARCH64, R_AARCH64_ABS64, R_AARCH64_PREL32),
        another => return Err(format!("objeto ELF da máquina {another}: as raízes por mapas só existem no x86-64 e no aarch64")),
    };
    let table = u64_at(object, 0x28)? as usize;
    let entry_size = u16_at(object, 0x3A)? as usize;
    let n_sections = u16_at(object, 0x3C)? as usize;
    let names_index = u16_at(object, 0x3E)? as usize;
    if entry_size != 64 || n_sections == 0 || names_index >= n_sections {
        return Err("objeto ELF com a tabela de seções numa forma que o conversor do mapa não lê (numeração estendida)".to_string());
    }
    let header = |i: usize| table + 64 * i;
    let names = u64_at(object, header(names_index) + 24)? as usize;
    let name_of = |o: &[u8], i: usize| -> Result<(usize, Vec<u8>), String> {
        let start = names + u32_at(o, header(i))? as usize;
        let rest = o.get(start..).ok_or_else(|| "objeto ELF truncado".to_string())?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok((start, rest[..end].to_vec()))
    };
    let mut map = None;
    for i in 0..n_sections {
        if name_of(object, i)?.1 == b".llvm_stackmaps" {
            map = Some(i);
        }
    }
    let Some(m) = map else { return Ok(false) };
    // A seção `SHT_RELA` (4) que reloca o mapa.
    let mut relocations = None;
    for i in 0..n_sections {
        let ty = u32_at(object, header(i) + 4)?;
        if (ty == 4 || ty == 9) && u32_at(object, header(i) + 44)? as usize == m {
            if ty == 9 {
                return Err("`.llvm_stackmaps` com relocações `SHT_REL` (o conversor lê `SHT_RELA`)".to_string());
            }
            relocations = Some(i);
        }
    }
    let Some(r) = relocations else {
        return Err("`.llvm_stackmaps` sem a seção das relocações".to_string());
    };
    let data = u64_at(object, header(m) + 24)? as usize;
    let size = u64_at(object, header(m) + 32)? as usize;
    let sec = object.get(data..data + size).ok_or_else(|| "objeto ELF truncado".to_string())?.to_vec();
    let rel_start = u64_at(object, header(r) + 24)? as usize;
    let rel_size = u64_at(object, header(r) + 32)? as usize;
    if u64_at(object, header(r) + 56)? != 24 || rel_size % 24 != 0 {
        return Err("relocações do `.llvm_stackmaps` fora do formato `Elf64_Rela`".to_string());
    }
    let n_relocations = rel_size / 24;
    // Pelo deslocamento no mapa, o índice da relocação; por índice, o
    // símbolo e o adendo.
    let mut symbol_in: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut originals: Vec<(u64, u64)> = Vec::with_capacity(n_relocations);
    for k in 0..n_relocations {
        let e = rel_start + 24 * k;
        let place = u64_at(object, e)?;
        let info = u64_at(object, e + 8)?;
        if info as u32 != absolute {
            return Err(format!("relocação {} no `.llvm_stackmaps` (esperava a absoluta de 64 bits)", info as u32));
        }
        symbol_in.insert(u32::try_from(place).map_err(|_| "`.llvm_stackmaps` grande demais".to_string())?, k as u32);
        originals.push((info >> 32, u64_at(object, e + 16)?));
    }
    let functions = decode_v3(&sec, &symbol_in, sp, fp, preserved)?;
    let (blob, fields) = encode(&functions, RELATIVE_FLAG, alvo);
    let back = decode(&blob)?;
    let equal = back.len() == functions.len()
        && back.iter().zip(&functions).all(|((frame, records), (_, q, r))| *frame == (q / 8) * 8 && records == r);
    if !equal {
        return Err("o mapa de pilha compacto não reproduz o do LLVM (ida e volta)".to_string());
    }
    if blob.len() > size || fields.len() != n_relocations {
        return Err("o mapa de pilha compacto não cabe no lugar do `.llvm_stackmaps`".to_string());
    }
    object[data..data + blob.len()].copy_from_slice(&blob);
    object[data + blob.len()..data + size].fill(0);
    let h = header(m);
    object[h + 8..h + 16].copy_from_slice(&(SHF_ALLOC | SHF_GNU_RETAIN).to_le_bytes());
    object[h + 32..h + 40].copy_from_slice(&(blob.len() as u64).to_le_bytes());
    object[h + 48..h + 56].copy_from_slice(&4u64.to_le_bytes());
    // O nome: `dfgcm` sobre o texto `.llvm_stackmaps` (mais curto). Se o
    // ligador de textos o dividia com `.rela.llvm_stackmaps`, o nome da
    // seção de relocações muda junto, sem efeito: ela é achada pelo tipo.
    let (location, _) = name_of(object, m)?;
    object[location..location + 6].copy_from_slice(b"dfgcm\0");
    for (k, (field, (ident, _, _))) in fields.iter().zip(&functions).enumerate() {
        let (symbol, addendum) = originals[*ident as usize];
        let e = rel_start + 24 * k;
        object[e..e + 8].copy_from_slice(&u64::from(*field).to_le_bytes());
        object[e + 8..e + 16].copy_from_slice(&((symbol << 32) | u64::from(relative)).to_le_bytes());
        object[e + 16..e + 24].copy_from_slice(&addendum.to_le_bytes());
    }
    Ok(true)
}

/// Se o objeto gerado neste hospedeiro passa pelo conversor: no ELF sempre
/// (obrigatório, [`convert_elf`]); no COFF salvo a medida
/// `DARTFORGE_SEM_MAPA_COMPACTO=1`; no Mach-O nunca.
pub fn convert_here() -> bool {
    match crate::alvo::sistema() {
        crate::alvo::Sistema::Linux => true,
        crate::alvo::Sistema::Windows => !std::env::var_os("DARTFORGE_SEM_MAPA_COMPACTO").is_some_and(|v| !v.is_empty() && v != "0"),
        crate::alvo::Sistema::MacOs => false,
    }
}

/// O conversor do formato do objeto: COFF ou ELF. Devolve `false` quando não
/// converteu (sem mapa; ou Mach-O, que fica no formato do LLVM).
///
/// # Errors
/// Os dos conversores.
pub fn convert(object: &mut [u8]) -> Result<bool, String> {
    if object.starts_with(b"\x7fELF") {
        convert_elf(object)
    } else {
        convert_coff(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_and_zigzag_round_trip() {
        for n in [0u64, 1, 127, 128, 300, 1 << 20, u64::MAX >> 1] {
            let mut b = Vec::new();
            varint(&mut b, n);
            let mut p = 0;
            assert_eq!(read_varint(&b, &mut p).unwrap(), n);
            assert_eq!(p, b.len());
        }
        for v in [0i64, 1, -1, 5, -5, 1 << 30, -(1 << 30)] {
            assert_eq!(without_zigzag(zigzag(v)), v);
        }
    }

    #[test]
    fn blob_round_trips() {
        let functions: Vec<Function> = vec![
            (0, 72, vec![(20, vec![(NO_SP, 32)]), (44, vec![(NO_SP, 32), (NO_SP, 40)]), (60, vec![(NO_SP, 32), (NO_SP, 40)])]),
            (1, 0, vec![(48, vec![(NO_SP, 40), (NO_FP, -16), (IN_REGISTER, 3), (IN_REGISTER, 14)]), (52, vec![(IN_REGISTER, 5)])]),
            (2, 40, Vec::new()),
        ];
        let (blob, fields) = encode(&functions, 0, 1);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(fields, vec![16, 24, 32]);
        let back = decode(&blob).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0], (72, functions[0].2.clone()));
        assert_eq!(back[1], (0, functions[1].2.clone()));
        assert_eq!(back[2], (40, Vec::new()));
    }

    #[test]
    fn object_without_map_stays_as_is() {
        let mut empty = vec![0u8; 8];
        assert_eq!(convert_coff(&mut empty), Ok(false));
        // Cabeçalho x86-64 sem seções.
        let mut o = vec![0u8; 20];
        o[0] = 0x64;
        o[1] = 0x86;
        let before = o.clone();
        assert_eq!(convert_coff(&mut o), Ok(false));
        assert_eq!(o, before);
    }
}
