//! O conferidor de dominância das raízes no modo sombra
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4: "o `store` no slot domina
//! todo ponto de coleta em que o valor está vivo"), sobre o texto do módulo
//! que o emissor gerou, depois de tudo o que o emissor acrescenta (os
//! `invoke`, os pousos, as raízes dos `phi`).
//!
//! Cada função em conferência traz, na linha antes do `define`, o comentário
//! `; df.refs v1 v5 …`: os valores `Ref` da HIR (sem constantes nem
//! `alloca`), que o emissor escreve com `DARTFORGE_CONFERIR_SOMBRA=1`. O
//! conferidor calcula, só do texto:
//!
//! * a vivacidade de cada `%v<n>` (os `phi` contam no fim do predecessor);
//! * o conteúdo dos slots do quadro em cada ponto, para a frente: o
//!   `store i64 %v<n>, ptr %gcs<k>` põe `n` no slot `k` (também pelo
//!   endereço de um local que mora no quadro, `%v<a> = getelementptr … ptr
//!   %gcs<k>, i64 0`); outro `store` no slot o apaga; na junção de caminhos
//!   o slot só vale se todos concordam;
//! * os pontos de coleta: toda chamada que não é folha (a extern que coleta
//!   ou chama Dart pela tabela de efeitos, a função Dart, a indireta).
//!
//! Em cada ponto de coleta, todo valor `Ref` vivo na entrada dele (os
//! operandos inclusive, o contrato C1) tem de estar em algum slot. É o
//! defeito da raiz esquecida, do slot compartilhado por dois vivos e do
//! `store` que não domina o ponto.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use std::collections::{HashMap, HashSet};

/// O comentário com os valores `Ref` de uma função.
pub(super) const MARCA_DE_REFS: &str = "; df.refs";

/// A conferência está ligada (`DARTFORGE_CONFERIR_SOMBRA=1`).
pub fn ligada() -> bool {
    std::env::var("DARTFORGE_CONFERIR_SOMBRA").is_ok_and(|v| v == "1")
}

/// Os `%x` de `linha` (sem o `%`).
fn nomes(linha: &str) -> Vec<&str> {
    let b = linha.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let ini = i + 1;
            let mut fim = ini;
            while fim < b.len() && (b[fim].is_ascii_alphanumeric() || matches!(b[fim], b'.' | b'_' | b'$' | b'-')) {
                fim += 1;
            }
            if fim > ini {
                v.push(&linha[ini..fim]);
            }
            i = fim.max(ini);
        } else {
            i += 1;
        }
    }
    v
}

/// O número de `v<n>` (só esse nome exato).
fn numero_de_v(nome: &str) -> Option<u32> {
    let resto = nome.strip_prefix('v')?;
    if resto.is_empty() || !resto.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    resto.parse().ok()
}

/// O número de `gcs<k>`.
fn numero_de_slot(nome: &str) -> Option<usize> {
    let resto = nome.strip_prefix("gcs")?;
    if resto.is_empty() || !resto.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    resto.parse().ok()
}

/// Uma instrução do texto.
#[derive(Default)]
struct Linha<'a> {
    texto: &'a str,
    /// O `%v<n>` definido.
    def: Option<u32>,
    /// Os `%v<n>` usados (sem os de um `phi`).
    usos: Vec<u32>,
    /// `phi`: `(valor, bloco de origem)`.
    phi: Vec<(u32, &'a str)>,
    coleta: bool,
}

struct Bloco<'a> {
    nome: &'a str,
    linhas: Vec<Linha<'a>>,
    sucessores: Vec<&'a str>,
}

/// A chamada da linha pode coletar.
fn coleta(linha: &str) -> bool {
    match super::rastro::alvo_da_chamada(linha) {
        None => false,
        Some(None) => true,
        Some(Some(nome)) => {
            if nome.starts_with("llvm.") {
                return false;
            }
            if matches!(nome, "df.corpo" | "df.barreira" | "df.barreira_elemento" | "df.e_objeto" | "df.filho_jovem" | "df.lancar") {
                return false;
            }
            if nome.starts_with("df.") || nome.starts_with("df_") || nome == "dart_main" {
                return true;
            }
            let ef = super::externs::efeitos_de(nome);
            ef.aloca || ef.chama_dart
        }
    }
}

/// O rótulo de uma linha de bloco (`b3:`, `b3:  ; preds = …`).
fn rotulo(linha: &str) -> Option<&str> {
    if linha.starts_with(' ') || linha.starts_with(';') || linha.is_empty() {
        return None;
    }
    let antes = linha.split(';').next()?.trim_end();
    let nome = antes.strip_suffix(':')?;
    (!nome.is_empty() && !nome.contains(' ')).then_some(nome.trim_matches('"'))
}

/// Divide o corpo de uma função em blocos.
fn blocos<'a>(corpo: &[&'a str]) -> Vec<Bloco<'a>> {
    let mut v: Vec<Bloco<'a>> = Vec::new();
    let mut em_switch = false;
    for &l in corpo {
        if let Some(r) = rotulo(l) {
            v.push(Bloco { nome: r, linhas: Vec::new(), sucessores: Vec::new() });
            continue;
        }
        let t = l.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        if v.is_empty() {
            v.push(Bloco { nome: "", linhas: Vec::new(), sucessores: Vec::new() });
        }
        let b = v.last_mut().expect("bloco");
        // Sucessores: `label %x` em terminadores (o `switch` em várias linhas).
        let terminador = em_switch
            || t.starts_with("br ")
            || t.starts_with("switch ")
            || t.contains(" invoke ")
            || t.starts_with("invoke ");
        if t.starts_with("switch ") {
            em_switch = !t.contains(']');
        } else if em_switch && t.contains(']') {
            em_switch = false;
        }
        if terminador {
            let mut k = 0;
            let palavras: Vec<&str> = t.split_whitespace().collect();
            while k + 1 < palavras.len() {
                if palavras[k] == "label"
                    && let Some(alvo) = palavras[k + 1].strip_prefix('%')
                {
                    let alvo = alvo.trim_end_matches([',', ']']).trim_matches('"');
                    b.sucessores.push(alvo);
                }
                k += 1;
            }
        }
        let mut linha = Linha { texto: t, ..Default::default() };
        let (def, resto) = match t.split_once(" = ") {
            Some((d, r)) if d.starts_with('%') => (numero_de_v(&d[1..]), r),
            _ => (None, t),
        };
        linha.def = def;
        if resto.starts_with("phi ") {
            // `[ %v1, %b2 ], [ 0, %b3 ]`
            for par in resto.split('[').skip(1) {
                let dentro = par.split(']').next().unwrap_or_default();
                let mut partes = dentro.split(',');
                let valor = partes.next().unwrap_or_default().trim();
                let origem = partes.next().unwrap_or_default().trim().trim_start_matches('%').trim_matches('"');
                if let Some(n) = valor.strip_prefix('%').and_then(numero_de_v) {
                    linha.phi.push((n, origem));
                }
            }
        } else {
            linha.usos = nomes(resto).into_iter().filter_map(numero_de_v).collect();
            linha.coleta = coleta(t);
        }
        b.linhas.push(linha);
    }
    v
}

/// Confere as funções do módulo que trazem [`MARCA_DE_REFS`].
///
/// # Errors
/// A primeira violação: a função, o ponto de coleta e o valor vivo sem slot.
pub fn verificar(ir: &str) -> Result<(), String> {
    let linhas: Vec<&str> = ir.lines().collect();
    let mut i = 0;
    let mut refs: Option<HashSet<u32>> = None;
    while i < linhas.len() {
        let l = linhas[i];
        if let Some(resto) = l.strip_prefix(MARCA_DE_REFS) {
            refs = Some(resto.split_whitespace().filter_map(|x| x.strip_prefix('v')?.parse().ok()).collect());
            i += 1;
            continue;
        }
        if !l.starts_with("define ") {
            i += 1;
            continue;
        }
        let nome = l.split('@').nth(1).and_then(|x| x.split('(').next()).unwrap_or_default().to_string();
        let ini = i + 1;
        let mut fim = ini;
        while fim < linhas.len() && linhas[fim] != "}" {
            fim += 1;
        }
        if let Some(r) = refs.take() {
            conferir_funcao(&linhas[ini..fim], &r).map_err(|e| format!("conferidor do modo sombra, `{nome}`: {e}"))?;
        }
        i = fim + 1;
    }
    Ok(())
}

/// Confere uma função: em cada ponto de coleta, todo `Ref` vivo está num slot.
fn conferir_funcao(corpo: &[&str], refs: &HashSet<u32>) -> Result<(), String> {
    let bs = blocos(corpo);
    if bs.is_empty() {
        return Ok(());
    }
    let indice: HashMap<&str, usize> = bs.iter().enumerate().map(|(i, b)| (b.nome, i)).collect();
    let n = bs.len();
    // Predecessores.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, b) in bs.iter().enumerate() {
        for s in &b.sucessores {
            if let Some(&j) = indice.get(s)
                && !preds[j].contains(&i)
            {
                preds[j].push(i);
            }
        }
    }
    // --- vivacidade (para trás) ---
    let mut vivo_na_entrada: Vec<HashSet<u32>> = vec![HashSet::new(); n];
    let mut mudou = true;
    while mudou {
        mudou = false;
        for i in (0..n).rev() {
            let b = &bs[i];
            let mut vivo = vivo_na_saida(&bs, &indice, &vivo_na_entrada, i);
            for l in b.linhas.iter().rev() {
                if let Some(d) = l.def {
                    vivo.remove(&d);
                }
                if l.phi.is_empty() {
                    vivo.extend(l.usos.iter().copied().filter(|v| refs.contains(v)));
                }
            }
            if vivo != vivo_na_entrada[i] {
                vivo_na_entrada[i] = vivo;
                mudou = true;
            }
        }
    }
    // --- os slots (para a frente) ---
    // O endereço de um local no quadro: `%v<a> = getelementptr … ptr %gcs<k>`.
    let mut local_no_slot: HashMap<u32, usize> = HashMap::new();
    for b in &bs {
        for l in &b.linhas {
            if let Some(d) = l.def
                && l.texto.contains("getelementptr")
                && let Some(k) = nomes(l.texto).into_iter().find_map(numero_de_slot)
            {
                local_no_slot.insert(d, k);
            }
        }
    }
    let gravacao = |l: &Linha<'_>| -> Option<(usize, Option<u32>)> {
        let t = l.texto;
        let resto = t.strip_prefix("store ")?;
        let (valor, destino) = resto.rsplit_once(", ptr ")?;
        let destino = destino.split(',').next()?.trim().strip_prefix('%')?;
        let slot = numero_de_slot(destino).or_else(|| numero_de_v(destino).and_then(|a| local_no_slot.get(&a).copied()))?;
        let v = valor.split_whitespace().last().and_then(|x| x.strip_prefix('%')).and_then(numero_de_v);
        Some((slot, v))
    };
    type Slots = HashMap<usize, u32>;
    let mut na_entrada: Vec<Option<Slots>> = vec![None; n];
    na_entrada[0] = Some(HashMap::new());
    let transferir = |b: &Bloco<'_>, mut s: Slots| -> Slots {
        for l in &b.linhas {
            if let Some((k, v)) = gravacao(l) {
                match v {
                    Some(v) => {
                        s.insert(k, v);
                    }
                    None => {
                        s.remove(&k);
                    }
                }
            }
        }
        s
    };
    let mut mudou = true;
    while mudou {
        mudou = false;
        for i in 0..n {
            let Some(entrada) = na_entrada[i].clone() else { continue };
            let saida = transferir(&bs[i], entrada);
            for s in &bs[i].sucessores {
                let Some(&j) = indice.get(s) else { continue };
                let novo = match &na_entrada[j] {
                    None => saida.clone(),
                    Some(atual) => atual.iter().filter(|(k, v)| saida.get(k) == Some(v)).map(|(k, v)| (*k, *v)).collect(),
                };
                if na_entrada[j].as_ref() != Some(&novo) {
                    na_entrada[j] = Some(novo);
                    mudou = true;
                }
            }
        }
    }
    // --- a conferência em cada ponto de coleta ---
    for (i, b) in bs.iter().enumerate() {
        let Some(mut slots) = na_entrada[i].clone() else { continue };
        // A vivacidade antes de cada linha, de trás para a frente.
        let mut vivo = vivo_na_saida(&bs, &indice, &vivo_na_entrada, i);
        let mut vivo_antes: Vec<HashSet<u32>> = vec![HashSet::new(); b.linhas.len()];
        for (k, l) in b.linhas.iter().enumerate().rev() {
            if let Some(d) = l.def {
                vivo.remove(&d);
            }
            if l.phi.is_empty() {
                vivo.extend(l.usos.iter().copied().filter(|v| refs.contains(v)));
            }
            vivo_antes[k] = vivo.clone();
        }
        for (k, l) in b.linhas.iter().enumerate() {
            if l.coleta {
                let guardados: HashSet<u32> = slots.values().copied().collect();
                let mut faltam: Vec<u32> = vivo_antes[k].iter().copied().filter(|v| !guardados.contains(v)).collect();
                faltam.sort_unstable();
                if let Some(v) = faltam.first() {
                    return Err(format!("`%v{v}` vivo sem slot no ponto de coleta `{}` (bloco `{}`)", l.texto, b.nome));
                }
            }
            if let Some((s, v)) = gravacao(l) {
                match v {
                    Some(v) => {
                        slots.insert(s, v);
                    }
                    None => {
                        slots.remove(&s);
                    }
                }
            }
        }
    }
    Ok(())
}

/// O que está vivo no fim do bloco `i`: o vivo na entrada de cada sucessor
/// (menos os `phi` dele) e os operandos dos `phi` que vêm deste bloco.
fn vivo_na_saida(bs: &[Bloco<'_>], indice: &HashMap<&str, usize>, vivo_na_entrada: &[HashSet<u32>], i: usize) -> HashSet<u32> {
    let mut vivo = HashSet::new();
    for s in &bs[i].sucessores {
        let Some(&j) = indice.get(s) else { continue };
        let defs_de_phi: HashSet<u32> = bs[j].linhas.iter().filter(|l| !l.phi.is_empty()).filter_map(|l| l.def).collect();
        vivo.extend(vivo_na_entrada[j].iter().copied().filter(|v| !defs_de_phi.contains(v)));
        for l in &bs[j].linhas {
            for (v, origem) in &l.phi {
                if *origem == bs[i].nome {
                    vivo.insert(*v);
                }
            }
        }
    }
    vivo
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modulo(corpo: &str) -> String {
        format!("; df.refs v1 v2 v3\ndefine i64 @df.lib.f(i64 %v1) {{\n{corpo}}}\n")
    }

    #[test]
    fn aceita_o_valor_guardado_antes_da_coleta() {
        let ir = modulo(
            "b0:\n  store i64 %v1, ptr %gcs0\n  %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1)\n  store i64 %v2, ptr %gcs1\n  %v3 = call i64 @df.lib.g(i64 %v2)\n  ret i64 %v1\n",
        );
        assert_eq!(verificar(&ir), Ok(()));
    }

    #[test]
    fn acusa_a_raiz_esquecida_e_o_slot_reusado() {
        // `%v1` vivo depois da chamada e nunca guardado.
        let ir = modulo("b0:\n  %v2 = call i64 @df.lib.g()\n  ret i64 %v1\n");
        assert!(verificar(&ir).is_err_and(|e| e.contains("%v1")));
        // O slot de `%v1` reusado por `%v2` com `%v1` ainda vivo.
        let ir = modulo(
            "b0:\n  store i64 %v1, ptr %gcs0\n  %v2 = call i64 @df.lib.g()\n  store i64 %v2, ptr %gcs0\n  %v3 = call i64 @df.lib.g()\n  ret i64 %v1\n",
        );
        assert!(verificar(&ir).is_err());
    }

    #[test]
    fn o_store_de_um_ramo_so_nao_domina_a_juncao() {
        let ir = modulo(
            "b0:\n  br i1 true, label %b1, label %b2\nb1:\n  store i64 %v1, ptr %gcs0\n  br label %b3\nb2:\n  br label %b3\nb3:\n  %v2 = call i64 @df.lib.g()\n  ret i64 %v1\n",
        );
        assert!(verificar(&ir).is_err());
        // Uma folha não é ponto de coleta.
        let ir = modulo("b0:\n  %v2 = call i64 @llvm.ctpop.i64(i64 %v1)\n  ret i64 %v1\n");
        assert_eq!(verificar(&ir), Ok(()));
    }
}
