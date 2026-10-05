//! A forma compacta da tabela do rastro no formato da VM
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14, item 1): o conversor da
//! seção `dfpcl` de um objeto COFF ou ELF, que troca as entradas de 12 bytes
//! (duas relocações cada) pelo **DFPC v1**, reescrevendo o objeto no lugar,
//! como o conversor do mapa de pilha (`gcmap.rs`).
//!
//! ```text
//! cabeçalho (20 bytes)
//!   "DFPC"; u8 versão = 1; u8 forma (1 = ELF, 2 = COFF); u16 0
//!   u32 tamanho total do blob (múltiplo de 4); u32 n_funcoes; u32 n_registros
//! índice: n_funcoes × { i32 endereço da função; u32 início no fluxo }
//! registros: n_registros × i32 endereço do registro
//!   cada endereço é relocado relativo ao próprio campo: no ELF, `campo + v`
//!   (`R_X86_64_PC32`, `R_AARCH64_PREL32`); no COFF, `campo + 4 + v`
//!   (`IMAGE_REL_AMD64_REL32`, relativo ao byte seguinte)
//! fluxo (varints LEB128 sem sinal), por função:
//!   n; n × { delta do rótulo desde o anterior (o primeiro, desde o começo
//!            da função); registro << 2 | espécie; palavra }
//! zeros até múltiplo de 4
//! ```
//!
//! As espécies e as palavras são as da forma de 12 bytes (`llvm/rastro.rs`);
//! as entradas do mesmo rótulo ficam na ordem da seção (a cadeia de quadros
//! embutidos). Uma entrada de espécie 3 (o elo) não tem registro (0). O
//! runtime lê as duas formas na mesma seção, pela assinatura `DFPC`
//! (`RT/rastro.rs`). O Mach-O não é convertido (como o mapa).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::gcmap::{ler_varint, u16_em, u32_em, u64_em, varint};

const MAQUINA_X86_64: u16 = 0x8664;
const COFF_REL32: u16 = 0x0004;
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;
/// O tipo de símbolo de função do COFF (`DTYPE_FUNCTION << 4`).
const COFF_FUNCAO: u16 = 0x20;
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
const R_X86_64_PC32: u32 = 2;
const R_AARCH64_PREL32: u32 = 261;
const STT_FUNC: u8 = 2;

const FORMA_ELF: u8 = 1;
const FORMA_COFF: u8 = 2;

/// Uma entrada lida: a função do rótulo (o símbolo) e o deslocamento nela,
/// o registro (símbolo, adendo), a espécie e a palavra.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entrada {
    funcao: u32,
    deslocamento: u64,
    registro: Option<(u32, i64)>,
    especie: u32,
    palavra: u32,
}

/// Uma relocação a escrever: a posição no blob, o símbolo e o adendo.
type Relocacao = (u32, u32, i64);

/// Codifica as entradas. Devolve o blob (com os adendos já no lugar quando
/// `adendo_no_lugar`, a forma do COFF) e as relocações: uma por função, na
/// ordem do índice, e uma por registro.
fn codificar(entradas: &[Entrada], forma: u8, adendo_no_lugar: bool) -> (Vec<u8>, Vec<Relocacao>) {
    // As funções na ordem da primeira aparição; as entradas de cada uma pelo
    // deslocamento, estável (a ordem da seção nos empates).
    let mut funcoes: Vec<u32> = Vec::new();
    let mut por_funcao: std::collections::HashMap<u32, Vec<&Entrada>> = std::collections::HashMap::new();
    for e in entradas {
        if !por_funcao.contains_key(&e.funcao) {
            funcoes.push(e.funcao);
        }
        por_funcao.entry(e.funcao).or_default().push(e);
    }
    let mut registros: Vec<(u32, i64)> = Vec::new();
    let mut indice_do_registro: std::collections::HashMap<(u32, i64), usize> = std::collections::HashMap::new();
    for e in entradas {
        if let Some(r) = e.registro
            && !indice_do_registro.contains_key(&r)
        {
            indice_do_registro.insert(r, registros.len());
            registros.push(r);
        }
    }
    let nf = funcoes.len();
    let nr = registros.len();
    let inicio_do_indice = 20;
    let inicio_dos_registros = inicio_do_indice + 8 * nf;
    let inicio_do_fluxo = inicio_dos_registros + 4 * nr;
    let mut fluxo: Vec<u8> = Vec::new();
    let mut inicios = Vec::with_capacity(nf);
    for f in &funcoes {
        inicios.push(inicio_do_fluxo + fluxo.len());
        let mut lista = por_funcao[f].clone();
        lista.sort_by_key(|e| e.deslocamento);
        varint(&mut fluxo, lista.len() as u64);
        let mut anterior = 0u64;
        for e in lista {
            varint(&mut fluxo, e.deslocamento - anterior);
            anterior = e.deslocamento;
            let r = e.registro.map_or(0, |r| indice_do_registro[&r]) as u64;
            varint(&mut fluxo, r << 2 | u64::from(e.especie & 3));
            varint(&mut fluxo, u64::from(e.palavra));
        }
    }
    let mut blob = Vec::with_capacity(inicio_do_fluxo + fluxo.len() + 4);
    blob.extend_from_slice(b"DFPC");
    blob.push(1);
    blob.push(forma);
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u32.to_le_bytes());
    blob.extend_from_slice(&(nf as u32).to_le_bytes());
    blob.extend_from_slice(&(nr as u32).to_le_bytes());
    let mut relocacoes = Vec::with_capacity(nf + nr);
    for (k, f) in funcoes.iter().enumerate() {
        relocacoes.push(((inicio_do_indice + 8 * k) as u32, *f, 0));
        blob.extend_from_slice(&0i32.to_le_bytes());
        blob.extend_from_slice(&(inicios[k] as u32).to_le_bytes());
    }
    for (k, (s, a)) in registros.iter().enumerate() {
        relocacoes.push(((inicio_dos_registros + 4 * k) as u32, *s, *a));
        let no_lugar = if adendo_no_lugar { *a as i32 } else { 0 };
        blob.extend_from_slice(&no_lugar.to_le_bytes());
    }
    blob.extend_from_slice(&fluxo);
    while blob.len() % 4 != 0 {
        blob.push(0);
    }
    let total = blob.len() as u32;
    blob[8..12].copy_from_slice(&total.to_le_bytes());
    (blob, relocacoes)
}

/// Decodifica o fluxo de um blob (a ida e volta): por função do índice, as
/// entradas `(deslocamento, registro, espécie, palavra)`.
fn decodificar(blob: &[u8]) -> Result<Vec<Vec<(u64, usize, u32, u32)>>, String> {
    if blob.get(..4) != Some(b"DFPC") || blob.get(4) != Some(&1) {
        return Err("blob DFPC sem a assinatura".to_string());
    }
    let nf = u32_em(blob, 12)? as usize;
    let mut saida = Vec::with_capacity(nf);
    for k in 0..nf {
        let mut p = u32_em(blob, 20 + 8 * k + 4)? as usize;
        let n = ler_varint(blob, &mut p)?;
        let mut lista = Vec::new();
        let mut deslocamento = 0u64;
        for _ in 0..n {
            deslocamento += ler_varint(blob, &mut p)?;
            let rk = ler_varint(blob, &mut p)?;
            let palavra = ler_varint(blob, &mut p)? as u32;
            lista.push((deslocamento, (rk >> 2) as usize, (rk & 3) as u32, palavra));
        }
        saida.push(lista);
    }
    Ok(saida)
}

/// A ida e volta: o que o runtime vai ler é o que a seção dizia.
fn conferir_ida_e_volta(blob: &[u8], entradas: &[Entrada], relocacoes: &[Relocacao]) -> Result<(), String> {
    let volta = decodificar(blob)?;
    let nf = u32_em(blob, 12)? as usize;
    let mut contadas = 0;
    for (k, lista) in volta.iter().enumerate() {
        let f = relocacoes[k].1;
        let mut esperadas: Vec<&Entrada> = entradas.iter().filter(|e| e.funcao == f).collect();
        esperadas.sort_by_key(|e| e.deslocamento);
        if esperadas.len() != lista.len() {
            return Err("a tabela do rastro compacta não reproduz a original (ida e volta)".to_string());
        }
        for (e, &(d, r, especie, palavra)) in esperadas.iter().zip(lista) {
            let registro = e.registro.map(|_| relocacoes[nf + r].1);
            let mesmo_registro = match (e.registro, registro) {
                (Some((s, a)), Some(_)) => relocacoes.get(nf + r).is_some_and(|x| x.1 == s && x.2 == a),
                (None, _) => especie == 3,
                _ => false,
            };
            if e.deslocamento != d || e.especie != especie || e.palavra != palavra || !mesmo_registro {
                return Err("a tabela do rastro compacta não reproduz a original (ida e volta)".to_string());
            }
        }
        contadas += lista.len();
    }
    if contadas != entradas.len() {
        return Err("a tabela do rastro compacta perdeu entradas".to_string());
    }
    Ok(())
}

/// Um símbolo de função: `(seção, valor, tamanho)` (tamanho 0: até o
/// próximo).
type SimboloDeFuncao = (u32, u64, u64);

/// O símbolo de função que contém `(secao, deslocamento)`: o de maior valor
/// que não passa dele (e, com tamanho, que o cobre).
fn funcao_em(funcoes: &[(u32, SimboloDeFuncao)], secao: u32, deslocamento: u64) -> Option<(u32, u64)> {
    funcoes
        .iter()
        .filter(|(_, (s, v, t))| *s == secao && *v <= deslocamento && (*t == 0 || deslocamento < v + t || deslocamento == *v))
        .max_by_key(|(_, (_, v, _))| *v)
        .map(|(i, (_, v, _))| (*i, deslocamento - v))
}

/// Converte, no lugar, a seção `.dfpcl$m` de um objeto COFF x86-64.
/// Devolve `false`, sem mexer, quando não há seção ou ela não tem entradas,
/// ou numa forma que o conversor não reescreve (a original fica, e o runtime
/// a lê do mesmo jeito).
///
/// # Errors
/// Objeto malformado, ou uma entrada que o conversor não entende.
pub fn converter_coff(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.len() < 20 || u16_em(objeto, 0)? != MAQUINA_X86_64 {
        return Ok(false);
    }
    let n_secoes = u16_em(objeto, 2)? as usize;
    let simbolos = u32_em(objeto, 8)? as usize;
    let n_simbolos = u32_em(objeto, 12)? as usize;
    let opcional = u16_em(objeto, 16)? as usize;
    // Os símbolos: o valor e a seção de cada um; os de função à parte.
    let mut valor_de: Vec<(u32, u64)> = vec![(0, 0); n_simbolos];
    let mut funcoes: Vec<(u32, SimboloDeFuncao)> = Vec::new();
    let mut i = 0;
    while i < n_simbolos {
        let s = simbolos + 18 * i;
        let valor = u64::from(u32_em(objeto, s + 8)?);
        let secao = u16_em(objeto, s + 12)? as i16;
        let tipo = u16_em(objeto, s + 14)?;
        let aux = *objeto.get(s + 17).ok_or_else(|| "objeto COFF truncado".to_string())? as usize;
        if secao > 0 {
            valor_de[i] = (secao as u32, valor);
            if tipo == COFF_FUNCAO {
                funcoes.push((i as u32, (secao as u32, valor, 0)));
            }
        }
        i += 1 + aux;
    }
    for s in 0..n_secoes {
        let h = 20 + opcional + 40 * s;
        if objeto.get(h..h + 8) != Some(b".dfpcl$m") {
            continue;
        }
        let tamanho = u32_em(objeto, h + 16)? as usize;
        let dados = u32_em(objeto, h + 20)? as usize;
        let inicio_rel = u32_em(objeto, h + 24)? as usize;
        let n_rel = u16_em(objeto, h + 32)? as usize;
        let caracteristicas = u32_em(objeto, h + 36)?;
        if caracteristicas & SCN_NRELOC_OVFL != 0 || n_rel == 0 {
            return Ok(false);
        }
        let sec = objeto.get(dados..dados + tamanho).ok_or_else(|| "objeto COFF truncado".to_string())?.to_vec();
        if sec.starts_with(b"DFPC") {
            return Ok(false);
        }
        // Pelo deslocamento na seção, o símbolo relocado.
        let mut rel: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_rel {
            let e = inicio_rel + 10 * k;
            if u16_em(objeto, e + 8)? != COFF_REL32 {
                return Err("relocação inesperada na seção do rastro (esperava REL32)".to_string());
            }
            rel.insert(u32_em(objeto, e)?, u32_em(objeto, e + 4)?);
        }
        let mut entradas = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let campo = |d: usize| i64::from(i32::from_le_bytes([sec[p + d], sec[p + d + 1], sec[p + d + 2], sec[p + d + 3]]));
            let palavra = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&s0) = rel.get(&(p as u32)) else {
                if campo(0) != 0 || campo(4) != 0 || palavra != 0 {
                    return Err("entrada do rastro sem relocação no rótulo".to_string());
                }
                p += 12;
                continue;
            };
            // `alvo − campo` com o adendo no lugar: o alvo é `S + A − 4`.
            let (secao, valor) = *valor_de.get(s0 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
            let rotulo = (valor as i64 + campo(0) - 4) as u64;
            let Some((funcao, deslocamento)) = funcao_em(&funcoes, secao, rotulo) else {
                return Ok(false);
            };
            let (registro, especie) = match rel.get(&((p + 4) as u32)) {
                Some(&s1) => {
                    let (_, v1) = *valor_de.get(s1 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
                    let alvo = v1 as i64 + campo(4) - 4;
                    let especie = (alvo & 3) as u32;
                    (Some((s1, campo(4) - 4 - i64::from(especie))), especie)
                }
                None if campo(4) == 3 => (None, 3),
                None => return Err("entrada do rastro sem relocação no registro".to_string()),
            };
            entradas.push(Entrada { funcao, deslocamento, registro, especie, palavra });
            p += 12;
        }
        if entradas.is_empty() {
            return Ok(false);
        }
        let (blob, relocacoes) = codificar(&entradas, FORMA_COFF, true);
        conferir_ida_e_volta(&blob, &entradas, &relocacoes)?;
        if blob.len() > tamanho || relocacoes.len() > n_rel {
            return Ok(false);
        }
        objeto[dados..dados + blob.len()].copy_from_slice(&blob);
        objeto[dados + blob.len()..dados + tamanho].fill(0);
        objeto[h + 16..h + 20].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        objeto[h + 32..h + 34].copy_from_slice(&(relocacoes.len() as u16).to_le_bytes());
        for (k, (posicao, simbolo, _)) in relocacoes.iter().enumerate() {
            let e = inicio_rel + 10 * k;
            objeto[e..e + 4].copy_from_slice(&posicao.to_le_bytes());
            objeto[e + 4..e + 8].copy_from_slice(&simbolo.to_le_bytes());
            objeto[e + 8..e + 10].copy_from_slice(&COFF_REL32.to_le_bytes());
        }
        return Ok(true);
    }
    Ok(false)
}

/// Converte, no lugar, cada seção `dfpcl` de um objeto ELF de 64 bits
/// (x86-64 ou aarch64): uma por grupo `comdat`, cada uma com as relocações
/// dela. Devolve se converteu alguma.
///
/// # Errors
/// Objeto malformado, ou uma entrada que o conversor não entende.
pub fn converter_elf(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.len() < 64 || &objeto[..4] != b"\x7fELF" || objeto[4] != 2 || objeto[5] != 1 {
        return Ok(false);
    }
    let relativa = match u16_em(objeto, 0x12)? {
        ELF_X86_64 => R_X86_64_PC32,
        ELF_AARCH64 => R_AARCH64_PREL32,
        _ => return Ok(false),
    };
    let tabela = u64_em(objeto, 0x28)? as usize;
    let n_secoes = u16_em(objeto, 0x3C)? as usize;
    let indice_dos_nomes = u16_em(objeto, 0x3E)? as usize;
    if u16_em(objeto, 0x3A)? != 64 || n_secoes == 0 || indice_dos_nomes >= n_secoes {
        return Ok(false);
    }
    let cabecalho = |i: usize| tabela + 64 * i;
    let nomes = u64_em(objeto, cabecalho(indice_dos_nomes) + 24)? as usize;
    let nome_de = |o: &[u8], i: usize| -> Result<Vec<u8>, String> {
        let inicio = nomes + u32_em(o, cabecalho(i))? as usize;
        let resto = o.get(inicio..).ok_or_else(|| "objeto ELF truncado".to_string())?;
        let fim = resto.iter().position(|&b| b == 0).unwrap_or(resto.len());
        Ok(resto[..fim].to_vec())
    };
    let mut convertidas = false;
    for m in 0..n_secoes {
        if nome_de(objeto, m)? != b"dfpcl" {
            continue;
        }
        // A `SHT_RELA` (4) da seção, e a tabela de símbolos dela.
        let mut relocacoes = None;
        for i in 0..n_secoes {
            if u32_em(objeto, cabecalho(i) + 4)? == 4 && u32_em(objeto, cabecalho(i) + 44)? as usize == m {
                relocacoes = Some(i);
            }
        }
        let Some(r) = relocacoes else { continue };
        let simbolos_sec = u32_em(objeto, cabecalho(r) + 40)? as usize;
        let inicio_sim = u64_em(objeto, cabecalho(simbolos_sec) + 24)? as usize;
        let tamanho_sim = u64_em(objeto, cabecalho(simbolos_sec) + 32)? as usize;
        let n_sim = tamanho_sim / 24;
        let mut valor_de: Vec<(u32, u64)> = Vec::with_capacity(n_sim);
        let mut funcoes: Vec<(u32, SimboloDeFuncao)> = Vec::new();
        for k in 0..n_sim {
            let s = inicio_sim + 24 * k;
            let info = *objeto.get(s + 4).ok_or_else(|| "objeto ELF truncado".to_string())?;
            let secao = u32::from(u16_em(objeto, s + 6)?);
            let valor = u64_em(objeto, s + 8)?;
            let tamanho = u64_em(objeto, s + 16)?;
            valor_de.push((secao, valor));
            if info & 0xf == STT_FUNC && secao != 0 {
                funcoes.push((k as u32, (secao, valor, tamanho)));
            }
        }
        let dados = u64_em(objeto, cabecalho(m) + 24)? as usize;
        let tamanho = u64_em(objeto, cabecalho(m) + 32)? as usize;
        let sec = objeto.get(dados..dados + tamanho).ok_or_else(|| "objeto ELF truncado".to_string())?.to_vec();
        if sec.starts_with(b"DFPC") {
            continue;
        }
        let inicio_rel = u64_em(objeto, cabecalho(r) + 24)? as usize;
        let tamanho_rel = u64_em(objeto, cabecalho(r) + 32)? as usize;
        if u64_em(objeto, cabecalho(r) + 56)? != 24 || tamanho_rel % 24 != 0 {
            return Err("relocações da seção do rastro fora do formato `Elf64_Rela`".to_string());
        }
        let mut rel: std::collections::HashMap<u64, (u32, i64)> = std::collections::HashMap::new();
        for k in 0..tamanho_rel / 24 {
            let e = inicio_rel + 24 * k;
            let info = u64_em(objeto, e + 8)?;
            if info as u32 != relativa {
                return Err(format!("relocação {} na seção do rastro (esperava a relativa de 32 bits)", info as u32));
            }
            rel.insert(u64_em(objeto, e)?, ((info >> 32) as u32, u64_em(objeto, e + 16)? as i64));
        }
        if rel.is_empty() {
            continue;
        }
        let mut entradas = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let crua = |d: usize| i64::from(i32::from_le_bytes([sec[p + d], sec[p + d + 1], sec[p + d + 2], sec[p + d + 3]]));
            let palavra = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&(s0, a0)) = rel.get(&(p as u64)) else {
                if crua(0) != 0 || crua(4) != 0 || palavra != 0 {
                    return Err("entrada do rastro sem relocação no rótulo".to_string());
                }
                p += 12;
                continue;
            };
            // `S + A − P`: o alvo é `S + A`.
            let (secao, valor) = *valor_de.get(s0 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
            let rotulo = (valor as i64 + a0) as u64;
            let Some((funcao, deslocamento)) = funcao_em(&funcoes, secao, rotulo) else {
                return Err("rótulo do rastro fora de uma função".to_string());
            };
            let (registro, especie) = match rel.get(&((p + 4) as u64)) {
                Some(&(s1, a1)) => {
                    let (_, v1) = *valor_de.get(s1 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
                    let especie = ((v1 as i64 + a1) & 3) as u32;
                    (Some((s1, a1 - i64::from(especie))), especie)
                }
                None if crua(4) == 3 => (None, 3),
                None => return Err("entrada do rastro sem relocação no registro".to_string()),
            };
            entradas.push(Entrada { funcao, deslocamento, registro, especie, palavra });
            p += 12;
        }
        if entradas.is_empty() {
            continue;
        }
        let (blob, novas) = codificar(&entradas, FORMA_ELF, false);
        conferir_ida_e_volta(&blob, &entradas, &novas)?;
        if blob.len() > tamanho || 24 * novas.len() > tamanho_rel {
            continue;
        }
        objeto[dados..dados + blob.len()].copy_from_slice(&blob);
        objeto[dados + blob.len()..dados + tamanho].fill(0);
        let h = cabecalho(m);
        objeto[h + 32..h + 40].copy_from_slice(&(blob.len() as u64).to_le_bytes());
        for (k, (posicao, simbolo, adendo)) in novas.iter().enumerate() {
            let e = inicio_rel + 24 * k;
            objeto[e..e + 8].copy_from_slice(&u64::from(*posicao).to_le_bytes());
            objeto[e + 8..e + 16].copy_from_slice(&((u64::from(*simbolo) << 32) | u64::from(relativa)).to_le_bytes());
            objeto[e + 16..e + 24].copy_from_slice(&adendo.to_le_bytes());
        }
        let hr = cabecalho(r);
        objeto[hr + 32..hr + 40].copy_from_slice(&((24 * novas.len()) as u64).to_le_bytes());
        convertidas = true;
    }
    Ok(convertidas)
}

/// Se a tabela do rastro do objeto gerado neste hospedeiro passa pelo
/// conversor: no COFF e no ELF, salvo a medida `DARTFORGE_RASTRO_CRU=1`; no
/// Mach-O nunca.
pub fn converter_aqui() -> bool {
    crate::alvo::sistema() != crate::alvo::Sistema::MacOs && !std::env::var_os("DARTFORGE_RASTRO_CRU").is_some_and(|v| !v.is_empty() && v != "0")
}

/// O conversor do formato do objeto. Devolve se converteu.
///
/// # Errors
/// Os dos conversores.
pub fn converter(objeto: &mut [u8]) -> Result<bool, String> {
    if objeto.starts_with(b"\x7fELF") { converter_elf(objeto) } else { converter_coff(objeto) }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn e(funcao: u32, deslocamento: u64, registro: Option<(u32, i64)>, especie: u32, palavra: u32) -> Entrada {
        Entrada { funcao, deslocamento, registro, especie, palavra }
    }

    #[test]
    fn blob_vai_e_volta() {
        let entradas = vec![
            e(7, 40, Some((3, 16)), 0, 3 << 12 | 5),
            e(7, 12, Some((3, 0)), 0, 2 << 12),
            // A cadeia de um ponto embutido: o mesmo rótulo, na ordem.
            e(7, 40, Some((3, 32)), 0, 9 << 12 | 1),
            e(9, 0, Some((4, 8)), 1, 1 << 12 | 6),
            e(9, 0, Some((4, 8)), 2, 5 << 12 | 9),
            e(9, 0, None, 3, 1 << 2 | 1),
        ];
        let (blob, relocacoes) = codificar(&entradas, FORMA_ELF, false);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(&blob[..4], b"DFPC");
        // Duas funções e três registros distintos.
        assert_eq!(relocacoes.len(), 5);
        assert_eq!(relocacoes[0].1, 7);
        assert_eq!(relocacoes[1].1, 9);
        assert!(conferir_ida_e_volta(&blob, &entradas, &relocacoes).is_ok());
        let volta = decodificar(&blob).unwrap();
        // A ordem dos empates é a da seção: 3:5 antes de 9:1.
        assert_eq!(volta[0].iter().map(|x| x.3).collect::<Vec<_>>(), vec![2 << 12, 3 << 12 | 5, 9 << 12 | 1]);
        // O COFF leva o adendo no lugar.
        let (coff, _) = codificar(&entradas, FORMA_COFF, true);
        let primeiro_registro = 20 + 8 * 2;
        assert_eq!(i32::from_le_bytes(coff[primeiro_registro..primeiro_registro + 4].try_into().unwrap()), 16);
    }

    #[test]
    fn funcao_do_rotulo() {
        let funcoes = vec![(1, (2, 0, 100)), (5, (2, 100, 50)), (6, (3, 0, 10))];
        assert_eq!(funcao_em(&funcoes, 2, 120), Some((5, 20)));
        assert_eq!(funcao_em(&funcoes, 2, 99), Some((1, 99)));
        assert_eq!(funcao_em(&funcoes, 3, 40), None);
    }

    #[test]
    fn objeto_sem_secao_fica_como_esta() {
        let mut o = vec![0u8; 20];
        o[0] = 0x64;
        o[1] = 0x86;
        let antes = o.clone();
        assert_eq!(converter_coff(&mut o), Ok(false));
        assert_eq!(o, antes);
        assert_eq!(converter_elf(&mut vec![0u8; 8]), Ok(false));
    }
}
