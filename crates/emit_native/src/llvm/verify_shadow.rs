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
//!   %gcs<k>, i64 0`); other `store` no slot o apaga; na junção de caminhos
//!   o slot só vale se todos concordam;
//! * os pontos de coleta: toda chamada que não é folha (a extern que coleta
//!   ou chama Dart pela tabela de efeitos, a função Dart, a indireta). Um
//!   ajudante interno do módulo (`define internal … @df.*`, os `alwaysinline`
//!   de textos, caixas, listas) coleta se o corpo dele tem uma chamada que
//!   coleta, pelo próprio texto do módulo.
//!
//! Em cada ponto de coleta, todo valor `Ref` vivo na entrada dele (os
//! operandos inclusive, o contrato C1) tem de estar em algum slot. Os
//! parâmetros da função não entram: pelo mesmo C1, quem chama os mantém
//! vivos enquanto a chamada roda. É o
//! defeito da raiz esquecida, do slot compartilhado por dois vivos e do
//! `store` que não domina o ponto.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use std::collections::{HashMap, HashSet};

/// O comentário com os valores `Ref` de uma função.
pub(super) const REFS_MARK: &str = "; df.refs";

/// A conferência está ligada (`DARTFORGE_CONFERIR_SOMBRA=1`).
pub fn linked() -> bool {
    std::env::var("DARTFORGE_CONFERIR_SOMBRA").is_ok_and(|v| v == "1")
}

/// Os `%x` de `line` (sem o `%`).
fn names(line: &str) -> Vec<&str> {
    let b = line.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let ini = i + 1;
            let mut end = ini;
            while end < b.len() && (b[end].is_ascii_alphanumeric() || matches!(b[end], b'.' | b'_' | b'$' | b'-')) {
                end += 1;
            }
            if end > ini {
                v.push(&line[ini..end]);
            }
            i = end.max(ini);
        } else {
            i += 1;
        }
    }
    v
}

/// O número de `v<n>` (só esse nome exato).
fn v_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix('v')?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// O número de `gcs<k>`.
fn slot_number(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("gcs")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// Uma instrução do texto.
#[derive(Default)]
struct Line<'a> {
    text: &'a str,
    /// O `%v<n>` definido.
    def: Option<u32>,
    /// Os `%v<n>` usados (sem os de um `phi`).
    uses: Vec<u32>,
    /// `phi`: `(value, block de origin)`.
    phi: Vec<(u32, &'a str)>,
    collection: bool,
}

struct Block<'a> {
    name: &'a str,
    lines: Vec<Line<'a>>,
    successors: Vec<&'a str>,
}

/// A chamada da linha pode coletar. `helpers`: os ajudantes internos do
/// módulo, cada um com o que o corpo dele decide ([`helpers_that_collect`]).
fn collection(line: &str, helpers: &HashMap<&str, bool>) -> bool {
    match super::stack_trace::call_target(line) {
        None => false,
        Some(None) => true,
        Some(Some(name)) => {
            if name.starts_with("llvm.") {
                return false;
            }
            if let Some(&c) = helpers.get(name) {
                return c;
            }
            if matches!(name, "df.corpo" | "df.barreira" | "df.barreira_elemento" | "df.e_objeto" | "df.filho_jovem" | "df.lancar") {
                return false;
            }
            if name.starts_with("df.") || name.starts_with("df_") || name == "dart_main" {
                return true;
            }
            let ef = super::externs::effects_of(name);
            ef.allocates || ef.calls_dart
        }
    }
}

/// Os ajudantes internos do módulo (`define internal … @df.<name>(`) e se
/// cada um coleta: algum `call`/`invoke` do corpo coleta, pela regra de
/// [`collection`] e pelos outros ajudantes. Ponto fixo a partir de "não coleta"
/// (o que só sobe), para os que se chamam entre si.
fn helpers_that_collect<'a>(lines: &[&'a str]) -> HashMap<&'a str, bool> {
    let mut bodies: Vec<(&str, &[&str])> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if let Some(rest) = l.strip_prefix("define internal ")
            && let Some(name) = rest.split('@').nth(1).and_then(|x| x.split('(').next())
            && name.starts_with("df.")
        {
            let ini = i + 1;
            let mut end = ini;
            while end < lines.len() && lines[end] != "}" {
                end += 1;
            }
            bodies.push((name.trim_matches('"'), &lines[ini..end]));
            i = end + 1;
            continue;
        }
        i += 1;
    }
    let mut helpers: HashMap<&str, bool> = bodies.iter().map(|(n, _)| (*n, false)).collect();
    loop {
        let mut changed = false;
        for (name, body) in &bodies {
            if helpers[name] {
                continue;
            }
            if body.iter().any(|l| collection(l.trim(), &helpers)) {
                helpers.insert(name, true);
                changed = true;
            }
        }
        if !changed {
            return helpers;
        }
    }
}

/// O rótulo de uma linha de bloco (`b3:`, `b3:  ; preds = …`).
fn label(line: &str) -> Option<&str> {
    if line.starts_with(' ') || line.starts_with(';') || line.is_empty() {
        return None;
    }
    let before = line.split(';').next()?.trim_end();
    let name = before.strip_suffix(':')?;
    (!name.is_empty() && !name.contains(' ')).then_some(name.trim_matches('"'))
}

/// Divide o corpo de uma função em blocos.
fn blocks<'a>(body: &[&'a str], helpers: &HashMap<&str, bool>) -> Vec<Block<'a>> {
    let mut v: Vec<Block<'a>> = Vec::new();
    let mut in_switch = false;
    for &l in body {
        if let Some(r) = label(l) {
            v.push(Block { name: r, lines: Vec::new(), successors: Vec::new() });
            continue;
        }
        let t = l.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        if v.is_empty() {
            v.push(Block { name: "", lines: Vec::new(), successors: Vec::new() });
        }
        let b = v.last_mut().expect("bloco");
        // Sucessores: `label %x` em terminadores (o `switch` em várias linhas).
        let terminator = in_switch
            || t.starts_with("br ")
            || t.starts_with("switch ")
            || t.contains(" invoke ")
            || t.starts_with("invoke ");
        if t.starts_with("switch ") {
            in_switch = !t.contains(']');
        } else if in_switch && t.contains(']') {
            in_switch = false;
        }
        if terminator {
            let mut k = 0;
            let words: Vec<&str> = t.split_whitespace().collect();
            while k + 1 < words.len() {
                if words[k] == "label"
                    && let Some(alvo) = words[k + 1].strip_prefix('%')
                {
                    let alvo = alvo.trim_end_matches([',', ']']).trim_matches('"');
                    b.successors.push(alvo);
                }
                k += 1;
            }
        }
        let mut line = Line { text: t, ..Default::default() };
        let (def, rest) = match t.split_once(" = ") {
            Some((d, r)) if d.starts_with('%') => (v_number(&d[1..]), r),
            _ => (None, t),
        };
        line.def = def;
        if rest.starts_with("phi ") {
            // `[ %v1, %b2 ], [ 0, %b3 ]`
            for par in rest.split('[').skip(1) {
                let inside = par.split(']').next().unwrap_or_default();
                let mut parts = inside.split(',');
                let value = parts.next().unwrap_or_default().trim();
                let origin = parts.next().unwrap_or_default().trim().trim_start_matches('%').trim_matches('"');
                if let Some(n) = value.strip_prefix('%').and_then(v_number) {
                    line.phi.push((n, origin));
                }
            }
        } else {
            line.uses = names(rest).into_iter().filter_map(v_number).collect();
            line.collection = collection(t, helpers);
        }
        b.lines.push(line);
    }
    v
}

/// Confere as funções do módulo que trazem `REFS_MARK`.
///
/// # Errors
/// A primeira violação: a função, o ponto de coleta e o valor vivo sem slot.
pub fn verify(ir: &str) -> Result<(), String> {
    let lines: Vec<&str> = ir.lines().collect();
    let helpers = helpers_that_collect(&lines);
    let mut i = 0;
    let mut refs: Option<HashSet<u32>> = None;
    while i < lines.len() {
        let l = lines[i];
        if let Some(rest) = l.strip_prefix(REFS_MARK) {
            refs = Some(rest.split_whitespace().filter_map(|x| x.strip_prefix('v')?.parse().ok()).collect());
            i += 1;
            continue;
        }
        if !l.starts_with("define ") {
            i += 1;
            continue;
        }
        let name = l.split('@').nth(1).and_then(|x| x.split('(').next()).unwrap_or_default().to_string();
        let ini = i + 1;
        let mut end = ini;
        while end < lines.len() && lines[end] != "}" {
            end += 1;
        }
        if let Some(mut r) = refs.take() {
            // Os parâmetros (`define … @f(i64 %v0, i64 %v3)`): raízes de quem chama.
            let params = l.split_once('(').map_or("", |(_, x)| x);
            for n in names(params).into_iter().filter_map(v_number) {
                r.remove(&n);
            }
            check_function(&lines[ini..end], &r, &helpers).map_err(|e| format!("conferidor do modo sombra, `{name}`: {e}\n  {l}"))?;
        }
        i = end + 1;
    }
    Ok(())
}

/// Confere uma função: em cada ponto de coleta, todo `Ref` vivo está num slot.
fn check_function(body: &[&str], refs: &HashSet<u32>, helpers: &HashMap<&str, bool>) -> Result<(), String> {
    let bs = blocks(body, helpers);
    if bs.is_empty() {
        return Ok(());
    }
    let index: HashMap<&str, usize> = bs.iter().enumerate().map(|(i, b)| (b.name, i)).collect();
    let n = bs.len();
    // Predecessores.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, b) in bs.iter().enumerate() {
        for s in &b.successors {
            if let Some(&j) = index.get(s)
                && !preds[j].contains(&i)
            {
                preds[j].push(i);
            }
        }
    }
    // --- vivacidade (para trás) ---
    let mut live_at_entry: Vec<HashSet<u32>> = vec![HashSet::new(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let b = &bs[i];
            let mut live = live_at_exit(&bs, &index, &live_at_entry, i, refs);
            for l in b.lines.iter().rev() {
                if let Some(d) = l.def {
                    live.remove(&d);
                }
                if l.phi.is_empty() {
                    live.extend(l.uses.iter().copied().filter(|v| refs.contains(v)));
                }
            }
            if live != live_at_entry[i] {
                live_at_entry[i] = live;
                changed = true;
            }
        }
    }
    // --- os slots (para a frente) ---
    // O endereço de um local no quadro: `%v<a> = getelementptr … ptr %gcs<k>`.
    let mut local_in_slot: HashMap<u32, usize> = HashMap::new();
    for b in &bs {
        for l in &b.lines {
            if let Some(d) = l.def
                && l.text.contains("getelementptr")
                && let Some(k) = names(l.text).into_iter().find_map(slot_number)
            {
                local_in_slot.insert(d, k);
            }
        }
    }
    let write_op = |l: &Line<'_>| -> Option<(usize, Option<u32>)> {
        let t = l.text;
        let rest = t.strip_prefix("store ")?;
        let (value, dest) = rest.rsplit_once(", ptr ")?;
        let dest = dest.split(',').next()?.trim().strip_prefix('%')?;
        let slot = slot_number(dest).or_else(|| v_number(dest).and_then(|a| local_in_slot.get(&a).copied()))?;
        let v = value.split_whitespace().last().and_then(|x| x.strip_prefix('%')).and_then(v_number);
        Some((slot, v))
    };
    type Slots = HashMap<usize, u32>;
    let mut at_entry: Vec<Option<Slots>> = vec![None; n];
    at_entry[0] = Some(HashMap::new());
    let transfer = |b: &Block<'_>, mut s: Slots| -> Slots {
        for l in &b.lines {
            if let Some((k, v)) = write_op(l) {
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
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            let Some(entry) = at_entry[i].clone() else { continue };
            let exit = transfer(&bs[i], entry);
            for s in &bs[i].successors {
                let Some(&j) = index.get(s) else { continue };
                let new = match &at_entry[j] {
                    None => exit.clone(),
                    Some(current) => current.iter().filter(|(k, v)| exit.get(k) == Some(v)).map(|(k, v)| (*k, *v)).collect(),
                };
                if at_entry[j].as_ref() != Some(&new) {
                    at_entry[j] = Some(new);
                    changed = true;
                }
            }
        }
    }
    // --- a conferência em cada ponto de coleta ---
    for (i, b) in bs.iter().enumerate() {
        let Some(mut slots) = at_entry[i].clone() else { continue };
        // A vivacidade antes de cada linha, de trás para a frente.
        let mut live = live_at_exit(&bs, &index, &live_at_entry, i, refs);
        let mut live_before: Vec<HashSet<u32>> = vec![HashSet::new(); b.lines.len()];
        for (k, l) in b.lines.iter().enumerate().rev() {
            if let Some(d) = l.def {
                live.remove(&d);
            }
            if l.phi.is_empty() {
                live.extend(l.uses.iter().copied().filter(|v| refs.contains(v)));
            }
            live_before[k] = live.clone();
        }
        for (k, l) in b.lines.iter().enumerate() {
            if l.collection {
                let saved_set: HashSet<u32> = slots.values().copied().collect();
                let mut missing_set: Vec<u32> = live_before[k].iter().copied().filter(|v| !saved_set.contains(v)).collect();
                missing_set.sort_unstable();
                if let Some(v) = missing_set.first() {
                    return Err(format!("`%v{v}` vivo sem slot no ponto de coleta `{}` (bloco `{}`)", l.text, b.name));
                }
            }
            if let Some((s, v)) = write_op(l) {
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
fn live_at_exit(bs: &[Block<'_>], index: &HashMap<&str, usize>, live_at_entry: &[HashSet<u32>], i: usize, refs: &HashSet<u32>) -> HashSet<u32> {
    let mut live = HashSet::new();
    for s in &bs[i].successors {
        let Some(&j) = index.get(s) else { continue };
        let phi_defs: HashSet<u32> = bs[j].lines.iter().filter(|l| !l.phi.is_empty()).filter_map(|l| l.def).collect();
        live.extend(live_at_entry[j].iter().copied().filter(|v| !phi_defs.contains(v)));
        for l in &bs[j].lines {
            for (v, origin) in &l.phi {
                if *origin == bs[i].name && refs.contains(v) {
                    live.insert(*v);
                }
            }
        }
    }
    live
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(body: &str) -> String {
        format!("; df.refs v1 v2 v3\ndefine i64 @df.lib.f(i64 %v9) {{\n{body}}}\n")
    }

    /// O parâmetro é raiz de quem chama (C1): não precisa de slot.
    #[test]
    fn parameter_needs_no_slot() {
        let ir = "; df.refs v1 v2\ndefine i64 @df.lib.f(i64 %v1) {\nb0:\n  %v2 = call i64 @df.lib.g()\n  br label %b1\nb1:\n  %v3 = phi i64 [ %v1, %b0 ]\n  ret i64 %v1\n}\n";
        assert_eq!(verify(ir), Ok(()));
    }

    #[test]
    fn accepts_the_value_saved_before_collection() {
        let ir = module(
            "b0:\n  store i64 %v1, ptr %gcs0\n  %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1)\n  store i64 %v2, ptr %gcs1\n  %v3 = call i64 @df.lib.g(i64 %v2)\n  ret i64 %v1\n",
        );
        assert_eq!(verify(&ir), Ok(()));
    }

    #[test]
    fn reports_forgotten_root_and_reused_slot() {
        // `%v1` vivo depois da chamada e nunca guardado.
        let ir = module("b0:\n  %v2 = call i64 @df.lib.g()\n  ret i64 %v1\n");
        assert!(verify(&ir).is_err_and(|e| e.contains("%v1")));
        // O slot de `%v1` reusado por `%v2` com `%v1` ainda vivo.
        let ir = module(
            "b0:\n  store i64 %v1, ptr %gcs0\n  %v2 = call i64 @df.lib.g()\n  store i64 %v2, ptr %gcs0\n  %v3 = call i64 @df.lib.g()\n  ret i64 %v1\n",
        );
        assert!(verify(&ir).is_err());
    }

    #[test]
    fn store_in_one_branch_does_not_dominate_join() {
        let ir = module(
            "b0:\n  br i1 true, label %b1, label %b2\nb1:\n  store i64 %v1, ptr %gcs0\n  br label %b3\nb2:\n  br label %b3\nb3:\n  %v2 = call i64 @df.lib.g()\n  ret i64 %v1\n",
        );
        assert!(verify(&ir).is_err());
        // Uma folha não é ponto de coleta.
        let ir = module("b0:\n  %v2 = call i64 @llvm.ctpop.i64(i64 %v1)\n  ret i64 %v1\n");
        assert_eq!(verify(&ir), Ok(()));
    }
}
