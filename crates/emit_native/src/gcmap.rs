//! O mapa de pilha compacto (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.4):
//! o conversor do `.llvm_stackmaps` versão 3 de um objeto COFF para o
//! **DFGM v1**, reescrevendo o objeto no lugar.
//!
//! O formato do LLVM é feito para o patching de um JIT: ~98 bytes por
//! registro. O coletor só precisa, por chamada, do conjunto de slots do
//! quadro que guardam raízes; o DFGM guarda isso em ~8 bytes por registro
//! (deltas em varint, e um bit para "o mesmo conjunto do registro anterior").
//! É o porte do `gcmap.py` do experimento `x12`, que roda.
//!
//! ```text
//! cabeçalho (16 bytes)
//!   "DFGM"; u8 versão = 1; u8 alvo (1 = x86-64, 2 = aarch64); u16 bandeiras
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
//!     cabeçalho par: (cabeçalho >> 1) raízes, cada uma um varint `s`:
//!       bit 0 de s: base (0 = SP, 1 = FP)
//!       s >> 1: zigzag do delta do slot, em palavras de 8 bytes
//! zeros até múltiplo de 4
//! ```
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
//! Regras (as do §14.4): de cada par (base, derivado) só a base entra, sem
//! repetição; constantes são descartadas; um local que não seja
//! `Indirect [SP|FP + d]` de 8 bytes com `d` múltiplo de 8 é **erro**; dois
//! registros no mesmo deslocamento de retorno são erro; depois de codificar,
//! o blob é decodificado e comparado com a origem.

/// O registrador DWARF do SP e o do FP no x86-64 e no aarch64.
const REG_SP: u16 = 7;
const REG_FP: u16 = 6;
const REG_SP_AARCH64: u16 = 31;
const REG_FP_AARCH64: u16 = 29;
/// As bandeiras do blob: o índice com endereços relativos ao próprio campo.
const BANDEIRA_RELATIVA: u16 = 1;
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
const MAQUINA_X86_64: u16 = 0x8664;
/// `IMAGE_REL_AMD64_ADDR64` e `IMAGE_REL_AMD64_ADDR32NB`.
const REL_ADDR64: u16 = 1;
const REL_ADDR32NB: u16 = 3;
/// `IMAGE_SCN_LNK_NRELOC_OVFL`: o número de relocações não coube em 16 bits.
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;

/// As raízes de um registro: (é FP?, deslocamento em bytes), ordenadas.
type Raizes = Vec<(bool, i32)>;
/// Uma função do mapa: (símbolo da relocação, tamanho do quadro em bytes — 0
/// se dinâmico —, registros por deslocamento de retorno).
type Funcao = (u32, u64, Vec<(u32, Raizes)>);

pub(crate) fn u16_em(d: &[u8], p: usize) -> Result<u16, String> {
    d.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u32_em(d: &[u8], p: usize) -> Result<u32, String> {
    d.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u64_em(d: &[u8], p: usize) -> Result<u64, String> {
    Ok(u64::from(u32_em(d, p)?) | (u64::from(u32_em(d, p + 4)?) << 32))
}

pub(crate) fn varint(saida: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            saida.push(b | 0x80);
        } else {
            saida.push(b);
            return;
        }
    }
}

pub(crate) fn ler_varint(d: &[u8], p: &mut usize) -> Result<u64, String> {
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

fn sem_zigzag(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

/// Decodifica os blobs `.llvm_stackmaps` v3 de `sec`. `simbolo_em` dá, pelo
/// deslocamento de cada `StkSizeRecord`, a relocação dele (o símbolo no
/// COFF, o índice da relocação no ELF); `sp` e `fp` são os registradores
/// DWARF do alvo.
fn decodificar_v3(sec: &[u8], simbolo_em: &std::collections::HashMap<u32, u32>, sp: u16, fp: u16) -> Result<Vec<Funcao>, String> {
    let mut funcoes = Vec::new();
    let mut p = 0usize;
    while p + 16 <= sec.len() {
        if sec[p] == 0 {
            p += 1;
            continue;
        }
        if sec[p] != 3 {
            return Err(format!("`.llvm_stackmaps` na versão {} (o conversor lê a 3)", sec[p]));
        }
        let n_funcoes = u32_em(sec, p + 4)? as usize;
        let n_constantes = u32_em(sec, p + 8)? as usize;
        let n_registros = u32_em(sec, p + 12)? as usize;
        let tabela = p + 16;
        let constantes = tabela + 24 * n_funcoes;
        let mut r = constantes + 8 * n_constantes;
        let mut total = 0usize;
        for i in 0..n_funcoes {
            let quadro = u64_em(sec, tabela + 24 * i + 8)?;
            let registros_da_funcao = u64_em(sec, tabela + 24 * i + 16)? as usize;
            let simbolo = *simbolo_em
                .get(&((tabela + 24 * i) as u32))
                .ok_or_else(|| "função do mapa de pilha sem relocação".to_string())?;
            let mut registros: Vec<(u32, Raizes)> = Vec::with_capacity(registros_da_funcao);
            for _ in 0..registros_da_funcao {
                let deslocamento = u32_em(sec, r + 8)?;
                let n_locais = u16_em(sec, r + 14)? as usize;
                let locais = r + 16;
                // (tipo, tamanho, registrador, deslocamento ou constante)
                let local = |j: usize| -> Result<(u8, u16, u16, i32), String> {
                    let l = locais + 12 * j;
                    let tipo = *sec.get(l).ok_or_else(|| "mapa de pilha truncado".to_string())?;
                    Ok((tipo, u16_em(sec, l + 2)?, u16_em(sec, l + 4)?, u32_em(sec, l + 8)? as i32))
                };
                if n_locais < 3 || local(0)?.0 != 4 || local(1)?.0 != 4 || local(2)?.0 != 4 {
                    return Err("registro de statepoint sem os três locais constantes iniciais".to_string());
                }
                let n_deopt = local(2)?.3.max(0) as usize;
                if (n_locais - 3).saturating_sub(n_deopt) % 2 != 0 {
                    return Err("registro de statepoint com pares (base, derivado) incompletos".to_string());
                }
                let mut raizes: Raizes = Vec::new();
                let mut j = 3 + n_deopt;
                while j + 1 < n_locais {
                    let (tipo, tamanho, registrador, d) = local(j)?;
                    j += 2;
                    // Constante: null ou `Smi` (ímpar); não é raiz. Uma
                    // constante par não nula seria um valor bruto tratado
                    // como referência: defeito do emissor (§3.5).
                    if tipo == 4 || tipo == 5 {
                        let c = if tipo == 4 {
                            i64::from(d)
                        } else {
                            u64_em(sec, constantes + 8 * usize::try_from(d).map_err(|_| "`ConstIndex` negativo no mapa de pilha".to_string())?)? as i64
                        };
                        if c != 0 && c & 1 == 0 {
                            return Err(format!("constante par não nula ({c:#x}) como raiz no mapa de pilha: um valor bruto tratado como referência"));
                        }
                        continue;
                    }
                    if tipo != 3 || tamanho != 8 || (registrador != sp && registrador != fp) {
                        return Err(format!(
                            "local recusado no mapa de pilha: tipo {tipo}, tamanho {tamanho}, registrador {registrador} (só `Indirect [SP|FP + d]` de 8 bytes)"
                        ));
                    }
                    if d % 8 != 0 {
                        return Err(format!("slot de raiz fora de múltiplo de 8 no mapa de pilha: {d}"));
                    }
                    let raiz = (registrador == fp, d);
                    if !raizes.contains(&raiz) {
                        raizes.push(raiz);
                    }
                }
                raizes.sort_unstable();
                registros.push((deslocamento, raizes));
                let mut l = (locais + 12 * n_locais + 7) & !7;
                let n_saidas = u16_em(sec, l + 2)? as usize;
                l = (l + 4 + 4 * n_saidas + 7) & !7;
                r = l;
            }
            registros.sort_by_key(|g| g.0);
            if registros.windows(2).any(|par| par[0].0 == par[1].0) {
                return Err("dois registros do mapa de pilha no mesmo endereço de retorno".to_string());
            }
            total += registros.len();
            funcoes.push((simbolo, if quadro == u64::MAX { 0 } else { quadro }, registros));
        }
        if total != n_registros {
            return Err("o cabeçalho do `.llvm_stackmaps` não casa com os registros".to_string());
        }
        p = r;
    }
    Ok(funcoes)
}

/// Codifica o blob DFGM v1 com as `bandeiras` e o `alvo` dados; devolve
/// também a posição, no blob, do campo de endereço de cada função (onde vai
/// a relocação).
fn codificar(funcoes: &[Funcao], bandeiras: u16, alvo: u8) -> (Vec<u8>, Vec<u32>) {
    let mut fluxo: Vec<u8> = Vec::new();
    let mut inicios: Vec<u32> = Vec::with_capacity(funcoes.len());
    for (_, quadro, registros) in funcoes {
        inicios.push(fluxo.len() as u32);
        varint(&mut fluxo, registros.len() as u64);
        varint(&mut fluxo, quadro / 8);
        let mut retorno_anterior = 0u32;
        let mut anterior: Option<&Raizes> = None;
        for (deslocamento, raizes) in registros {
            varint(&mut fluxo, u64::from(deslocamento - retorno_anterior));
            retorno_anterior = *deslocamento;
            if anterior == Some(raizes) {
                varint(&mut fluxo, 1);
                continue;
            }
            varint(&mut fluxo, (raizes.len() as u64) << 1);
            let mut slot_anterior = 0i64;
            for (fp, d) in raizes {
                let slot = i64::from(*d) / 8;
                varint(&mut fluxo, (zigzag(slot - slot_anterior) << 1) | u64::from(*fp));
                slot_anterior = slot;
            }
            anterior = Some(raizes);
        }
    }
    while fluxo.len() % 4 != 0 {
        fluxo.push(0);
    }
    let cabecalho = 16 + 8 * funcoes.len();
    let mut blob: Vec<u8> = Vec::with_capacity(cabecalho + fluxo.len());
    blob.extend_from_slice(b"DFGM");
    blob.extend_from_slice(&[1, alvo]);
    blob.extend_from_slice(&bandeiras.to_le_bytes());
    blob.extend_from_slice(&((cabecalho + fluxo.len()) as u32).to_le_bytes());
    blob.extend_from_slice(&(funcoes.len() as u32).to_le_bytes());
    let mut campos = Vec::with_capacity(funcoes.len());
    for inicio in &inicios {
        campos.push(blob.len() as u32);
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.extend_from_slice(&inicio.to_le_bytes());
    }
    blob.extend_from_slice(&fluxo);
    (blob, campos)
}

/// Decodifica um blob DFGM v1: por função, (tamanho do quadro em bytes,
/// registros). É o leitor de referência (o do runtime é o mesmo algoritmo).
fn decodificar(blob: &[u8]) -> Result<Vec<(u64, Vec<(u32, Raizes)>)>, String> {
    if blob.len() < 16 || &blob[..4] != b"DFGM" || blob[4] != 1 {
        return Err("blob DFGM sem o cabeçalho da versão 1".to_string());
    }
    if u32_em(blob, 8)? as usize != blob.len() {
        return Err("blob DFGM com o tamanho errado no cabeçalho".to_string());
    }
    if u16_em(blob, 6)? & !BANDEIRA_RELATIVA != 0 {
        return Err("blob DFGM com bandeiras desconhecidas".to_string());
    }
    let n_funcoes = u32_em(blob, 12)? as usize;
    let base = 16 + 8 * n_funcoes;
    let mut saida = Vec::with_capacity(n_funcoes);
    for i in 0..n_funcoes {
        let mut p = base + u32_em(blob, 16 + 8 * i + 4)? as usize;
        let n = ler_varint(blob, &mut p)?;
        let quadro = ler_varint(blob, &mut p)? * 8;
        let mut registros = Vec::new();
        let mut retorno = 0u32;
        let mut anterior: Raizes = Vec::new();
        for _ in 0..n {
            retorno += ler_varint(blob, &mut p)? as u32;
            let c = ler_varint(blob, &mut p)?;
            if c & 1 == 0 {
                anterior = Vec::new();
                let mut slot = 0i64;
                for _ in 0..(c >> 1) {
                    let s = ler_varint(blob, &mut p)?;
                    slot += sem_zigzag(s >> 1);
                    anterior.push((s & 1 == 1, (slot * 8) as i32));
                }
            }
            registros.push((retorno, anterior.clone()));
        }
        saida.push((quadro, registros));
    }
    Ok(saida)
}

/// Converte, no lugar, o `.llvm_stackmaps` do objeto COFF x86-64 `objeto` em
/// `.dfgcm$m`. Devolve `false`, sem mexer no objeto, quando não há o que
/// converter ou o objeto tem uma forma que o conversor não reescreve (COFF
/// grande, relocações além de 16 bits, outra máquina): o mapa do LLVM fica, e
/// o runtime o lê do mesmo jeito (`heap::ler_stackmaps`).
///
/// # Errors
/// Mapa que o runtime não saberia ler (um local que não é slot de quadro), ou
/// objeto malformado.
pub fn converter_coff(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.len() < 20 {
        return Ok(false);
    }
    let maquina = u16_em(objeto, 0)?;
    let n_secoes = u16_em(objeto, 2)? as usize;
    if maquina != MAQUINA_X86_64 {
        return Ok(false);
    }
    let simbolos = u32_em(objeto, 8)? as usize;
    let n_simbolos = u32_em(objeto, 12)? as usize;
    let opcional = u16_em(objeto, 16)? as usize;
    let textos = simbolos + 18 * n_simbolos;
    for s in 0..n_secoes {
        let h = 20 + opcional + 40 * s;
        let nome = objeto.get(h..h + 8).ok_or_else(|| "objeto COFF truncado".to_string())?;
        // O nome tem mais de 8 bytes: `/<deslocamento na tabela de textos>`.
        let e_o_mapa = match nome.strip_prefix(b"/") {
            Some(resto) => {
                let fim = resto.iter().position(|&b| b == 0).unwrap_or(resto.len());
                let desloc = std::str::from_utf8(&resto[..fim]).ok().and_then(|t| t.parse::<usize>().ok());
                desloc.is_some_and(|d| objeto.get(textos + d..).is_some_and(|t| t.starts_with(b".llvm_stackmaps\0")))
            }
            None => false,
        };
        if !e_o_mapa {
            continue;
        }
        let tamanho = u32_em(objeto, h + 16)? as usize;
        let dados = u32_em(objeto, h + 20)? as usize;
        let relocacoes = u32_em(objeto, h + 24)? as usize;
        let n_relocacoes = u16_em(objeto, h + 32)? as usize;
        let caracteristicas = u32_em(objeto, h + 36)?;
        if caracteristicas & SCN_NRELOC_OVFL != 0 {
            return Ok(false);
        }
        let sec = objeto.get(dados..dados + tamanho).ok_or_else(|| "objeto COFF truncado".to_string())?.to_vec();
        let mut simbolo_em: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_relocacoes {
            let e = relocacoes + 10 * k;
            if u16_em(objeto, e + 8)? != REL_ADDR64 {
                return Err("relocação inesperada no `.llvm_stackmaps` (esperava ADDR64)".to_string());
            }
            simbolo_em.insert(u32_em(objeto, e)?, u32_em(objeto, e + 4)?);
        }
        let funcoes = decodificar_v3(&sec, &simbolo_em, REG_SP, REG_FP)?;
        let (blob, campos) = codificar(&funcoes, 0, 1);
        // Ida e volta: o que o runtime vai ler é o que o LLVM escreveu.
        let volta = decodificar(&blob)?;
        let igual = volta.len() == funcoes.len()
            && volta.iter().zip(&funcoes).all(|((quadro, registros), (_, q, r))| *quadro == (q / 8) * 8 && registros == r);
        if !igual {
            return Err("o mapa de pilha compacto não reproduz o do LLVM (ida e volta)".to_string());
        }
        if blob.len() > tamanho || campos.len() != n_relocacoes {
            return Ok(false);
        }
        objeto[dados..dados + blob.len()].copy_from_slice(&blob);
        objeto[dados + blob.len()..dados + tamanho].fill(0);
        objeto[h + 16..h + 20].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        objeto[h..h + 8].copy_from_slice(b".dfgcm$m");
        // Alinhamento 4 (`IMAGE_SCN_ALIGN_4BYTES`).
        let novas = (caracteristicas & !0x00F0_0000) | 0x0030_0000;
        objeto[h + 36..h + 40].copy_from_slice(&novas.to_le_bytes());
        for (k, (campo, (simbolo, _, _))) in campos.iter().zip(&funcoes).enumerate() {
            let e = relocacoes + 10 * k;
            objeto[e..e + 4].copy_from_slice(&campo.to_le_bytes());
            objeto[e + 4..e + 8].copy_from_slice(&simbolo.to_le_bytes());
            objeto[e + 8..e + 10].copy_from_slice(&REL_ADDR32NB.to_le_bytes());
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
pub fn converter_elf(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.len() < 64 || &objeto[..4] != b"\x7fELF" {
        return Ok(false);
    }
    if objeto[4] != 2 || objeto[5] != 1 {
        return Err("objeto ELF com raízes por mapas que não é de 64 bits little-endian".to_string());
    }
    let (alvo, sp, fp, absoluta, relativa) = match u16_em(objeto, 0x12)? {
        ELF_X86_64 => (1u8, REG_SP, REG_FP, R_X86_64_64, R_X86_64_PC32),
        ELF_AARCH64 => (2u8, REG_SP_AARCH64, REG_FP_AARCH64, R_AARCH64_ABS64, R_AARCH64_PREL32),
        outra => return Err(format!("objeto ELF da máquina {outra}: as raízes por mapas só existem no x86-64 e no aarch64")),
    };
    let tabela = u64_em(objeto, 0x28)? as usize;
    let tamanho_da_entrada = u16_em(objeto, 0x3A)? as usize;
    let n_secoes = u16_em(objeto, 0x3C)? as usize;
    let indice_dos_nomes = u16_em(objeto, 0x3E)? as usize;
    if tamanho_da_entrada != 64 || n_secoes == 0 || indice_dos_nomes >= n_secoes {
        return Err("objeto ELF com a tabela de seções numa forma que o conversor do mapa não lê (numeração estendida)".to_string());
    }
    let cabecalho = |i: usize| tabela + 64 * i;
    let nomes = u64_em(objeto, cabecalho(indice_dos_nomes) + 24)? as usize;
    let nome_de = |o: &[u8], i: usize| -> Result<(usize, Vec<u8>), String> {
        let inicio = nomes + u32_em(o, cabecalho(i))? as usize;
        let resto = o.get(inicio..).ok_or_else(|| "objeto ELF truncado".to_string())?;
        let fim = resto.iter().position(|&b| b == 0).unwrap_or(resto.len());
        Ok((inicio, resto[..fim].to_vec()))
    };
    let mut mapa = None;
    for i in 0..n_secoes {
        if nome_de(objeto, i)?.1 == b".llvm_stackmaps" {
            mapa = Some(i);
        }
    }
    let Some(m) = mapa else { return Ok(false) };
    // A seção `SHT_RELA` (4) que reloca o mapa.
    let mut relocacoes = None;
    for i in 0..n_secoes {
        let tipo = u32_em(objeto, cabecalho(i) + 4)?;
        if (tipo == 4 || tipo == 9) && u32_em(objeto, cabecalho(i) + 44)? as usize == m {
            if tipo == 9 {
                return Err("`.llvm_stackmaps` com relocações `SHT_REL` (o conversor lê `SHT_RELA`)".to_string());
            }
            relocacoes = Some(i);
        }
    }
    let Some(r) = relocacoes else {
        return Err("`.llvm_stackmaps` sem a seção das relocações".to_string());
    };
    let dados = u64_em(objeto, cabecalho(m) + 24)? as usize;
    let tamanho = u64_em(objeto, cabecalho(m) + 32)? as usize;
    let sec = objeto.get(dados..dados + tamanho).ok_or_else(|| "objeto ELF truncado".to_string())?.to_vec();
    let inicio_rel = u64_em(objeto, cabecalho(r) + 24)? as usize;
    let tamanho_rel = u64_em(objeto, cabecalho(r) + 32)? as usize;
    if u64_em(objeto, cabecalho(r) + 56)? != 24 || tamanho_rel % 24 != 0 {
        return Err("relocações do `.llvm_stackmaps` fora do formato `Elf64_Rela`".to_string());
    }
    let n_relocacoes = tamanho_rel / 24;
    // Pelo deslocamento no mapa, o índice da relocação; por índice, o
    // símbolo e o adendo.
    let mut simbolo_em: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut originais: Vec<(u64, u64)> = Vec::with_capacity(n_relocacoes);
    for k in 0..n_relocacoes {
        let e = inicio_rel + 24 * k;
        let lugar = u64_em(objeto, e)?;
        let info = u64_em(objeto, e + 8)?;
        if info as u32 != absoluta {
            return Err(format!("relocação {} no `.llvm_stackmaps` (esperava a absoluta de 64 bits)", info as u32));
        }
        simbolo_em.insert(u32::try_from(lugar).map_err(|_| "`.llvm_stackmaps` grande demais".to_string())?, k as u32);
        originais.push((info >> 32, u64_em(objeto, e + 16)?));
    }
    let funcoes = decodificar_v3(&sec, &simbolo_em, sp, fp)?;
    let (blob, campos) = codificar(&funcoes, BANDEIRA_RELATIVA, alvo);
    let volta = decodificar(&blob)?;
    let igual = volta.len() == funcoes.len()
        && volta.iter().zip(&funcoes).all(|((quadro, registros), (_, q, r))| *quadro == (q / 8) * 8 && registros == r);
    if !igual {
        return Err("o mapa de pilha compacto não reproduz o do LLVM (ida e volta)".to_string());
    }
    if blob.len() > tamanho || campos.len() != n_relocacoes {
        return Err("o mapa de pilha compacto não cabe no lugar do `.llvm_stackmaps`".to_string());
    }
    objeto[dados..dados + blob.len()].copy_from_slice(&blob);
    objeto[dados + blob.len()..dados + tamanho].fill(0);
    let h = cabecalho(m);
    objeto[h + 8..h + 16].copy_from_slice(&(SHF_ALLOC | SHF_GNU_RETAIN).to_le_bytes());
    objeto[h + 32..h + 40].copy_from_slice(&(blob.len() as u64).to_le_bytes());
    objeto[h + 48..h + 56].copy_from_slice(&4u64.to_le_bytes());
    // O nome: `dfgcm` sobre o texto `.llvm_stackmaps` (mais curto). Se o
    // ligador de textos o dividia com `.rela.llvm_stackmaps`, o nome da
    // seção de relocações muda junto, sem efeito: ela é achada pelo tipo.
    let (onde, _) = nome_de(objeto, m)?;
    objeto[onde..onde + 6].copy_from_slice(b"dfgcm\0");
    for (k, (campo, (ident, _, _))) in campos.iter().zip(&funcoes).enumerate() {
        let (simbolo, adendo) = originais[*ident as usize];
        let e = inicio_rel + 24 * k;
        objeto[e..e + 8].copy_from_slice(&u64::from(*campo).to_le_bytes());
        objeto[e + 8..e + 16].copy_from_slice(&((simbolo << 32) | u64::from(relativa)).to_le_bytes());
        objeto[e + 16..e + 24].copy_from_slice(&adendo.to_le_bytes());
    }
    Ok(true)
}

/// Se o objeto gerado neste hospedeiro passa pelo conversor: no ELF sempre
/// (obrigatório, [`converter_elf`]); no COFF salvo a medida
/// `DARTFORGE_SEM_MAPA_COMPACTO=1`; no Mach-O nunca.
pub fn converter_aqui() -> bool {
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
pub fn converter(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.starts_with(b"\x7fELF") {
        converter_elf(objeto)
    } else {
        converter_coff(objeto)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn varint_e_zigzag_vao_e_voltam() {
        for n in [0u64, 1, 127, 128, 300, 1 << 20, u64::MAX >> 1] {
            let mut b = Vec::new();
            varint(&mut b, n);
            let mut p = 0;
            assert_eq!(ler_varint(&b, &mut p).unwrap(), n);
            assert_eq!(p, b.len());
        }
        for v in [0i64, 1, -1, 5, -5, 1 << 30, -(1 << 30)] {
            assert_eq!(sem_zigzag(zigzag(v)), v);
        }
    }

    #[test]
    fn blob_vai_e_volta() {
        let funcoes: Vec<Funcao> = vec![
            (0, 72, vec![(20, vec![(false, 32)]), (44, vec![(false, 32), (false, 40)]), (60, vec![(false, 32), (false, 40)])]),
            (1, 0, vec![(48, vec![(true, -16), (false, 40)])]),
            (2, 40, Vec::new()),
        ];
        let (blob, campos) = codificar(&funcoes, 0, 1);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(campos, vec![16, 24, 32]);
        let volta = decodificar(&blob).unwrap();
        assert_eq!(volta.len(), 3);
        assert_eq!(volta[0], (72, funcoes[0].2.clone()));
        assert_eq!(volta[1], (0, funcoes[1].2.clone()));
        assert_eq!(volta[2], (40, Vec::new()));
    }

    #[test]
    fn objeto_sem_mapa_fica_como_esta() {
        let mut vazio = vec![0u8; 8];
        assert_eq!(converter_coff(&mut vazio), Ok(false));
        // Cabeçalho x86-64 sem seções.
        let mut o = vec![0u8; 20];
        o[0] = 0x64;
        o[1] = 0x86;
        let antes = o.clone();
        assert_eq!(converter_coff(&mut o), Ok(false));
        assert_eq!(o, antes);
    }
}
