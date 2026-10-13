//! O conferidor das raízes **depois** do `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, no estilo do
//! `--statepoints` do Perry), sobre a forma do §14.12: o valor `Ref` é `i64`
//! (`%v<n>`) em todo o código, e as raízes vivas numa chamada que coleta vão
//! no operando `"deopt"` dela — o statepoint as leva ao mapa de pilha.
//!
//! A coleta só enxerga o que está no `"deopt"` do statepoint em que ela
//! acontece. O defeito que este conferidor pega é o valor usado depois de um
//! statepoint sem estar no `"deopt"` dele: o objeto pode ter sido liberado
//! (o otimizador que estica a vida de um `Ref` por cima de uma chamada, o
//! emissor que esquece o operando). Também os argumentos: um `Ref` passado à
//! chamada do statepoint tem de estar vivo nela (contrato C1).
//!
//! Um nome é `Ref` quando aparece no `"deopt"` de algum statepoint da função
//! (o `i64` não diz se é referência). Fica de fora o valor de uma carga
//! `!invariant.load`, ou dos objetos canônicos `true`/`false` do contexto
//! (deslocamentos 360 e 368, `layout::contexto`; a otimização que funde as
//! cargas tira o metadado), e o `phi` só delas, como o `.lcssa` de um laço:
//! os canônicos são estáticos da imagem do runtime, nunca coletados, e o
//! otimizador reaproveita a carga por cima das chamadas, como deve. A conferência é por bloco, sobre o
//! texto do módulo: um uso depois de um statepoint do mesmo bloco, de um
//! valor que não foi definido depois dele. O valor que atravessa o
//! statepoint para um `phi` de outro bloco não é conferido.

use std::collections::HashSet;

/// Os nomes `%x` usados em `text` (sem o `%`).
fn uses_list(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] == b'%' {
                let begin = i + 1;
                let mut end = begin;
                while end < b.len() && (b[end].is_ascii_alphanumeric() || matches!(b[end], b'.' | b'_' | b'$' | b'-')) {
                    end += 1;
                }
                i = end.max(begin);
                if end > begin {
                    return Some(&text[begin..end]);
                }
            } else {
                i += 1;
            }
        }
        None
    })
}

/// O conteúdo do operando `"deopt"(…)` de uma linha, se ela o tem.
fn deopt_operand(line: &str) -> Option<&str> {
    let k = line.find("\"deopt\"(")?;
    let rest = &line[k + "\"deopt\"(".len()..];
    let mut depth = 1;
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[..i]);
                }
            }
            _ => {}
        }
    }
    Some(rest)
}

/// A linha sem os operandos (`[ … ]` depois dos argumentos).
fn without_operands(body: &str) -> &str {
    body.find(" [ \"").map_or(body, |p| &body[..p])
}

/// O nome definido pela linha (`%x = …`), sem o `%`.
fn defined(line: &str) -> Option<&str> {
    let l = line.trim_start().strip_prefix('%')?;
    let end = l.find(" = ")?;
    Some(&l[..end])
}

/// `refs` sem os valores de carga `!invariant.load` e sem os `phi` cujas
/// entradas são todas desses (ou constantes).
fn without_invariants<'a>(line_list: &[&'a str], mut refs: HashSet<&'a str>) -> HashSet<&'a str> {
    // Os endereços dos canônicos: `getelementptr … i64 360|368`, direto ou
    // pelo deslocamento escolhido num `select` (o `df.box_bool`).
    let selections: HashSet<&str> = line_list
        .iter()
        .filter(|l| l.contains("= select ") && l.contains("i64 360") && l.contains("i64 368"))
        .filter_map(|l| defined(l))
        .collect();
    let canonicals: HashSet<&str> = line_list
        .iter()
        .filter(|l| {
            l.contains("= getelementptr ")
                && (l.ends_with("i64 360") || l.ends_with("i64 368") || selections.iter().any(|s| l.ends_with(&format!("i64 %{s}"))))
        })
        .filter_map(|l| defined(l))
        .collect();
    let mut invariants: HashSet<&str> = line_list
        .iter()
        .filter(|l| {
            l.contains("!invariant.load")
                || l.contains("= load i64, ptr %")
                    && uses_list(&l[l.find("= load").unwrap_or(0)..]).next().is_some_and(|p| canonicals.contains(p))
        })
        .filter_map(|l| defined(l))
        .collect();
    // Os `phi` (com o bloco de cada um), os `icmp eq` e os desvios
    // condicionais de cada bloco: a entrada de um `phi` que chega pela aresta
    // em que `X == canônico` é o próprio canônico (o otimizador troca um pelo
    // outro).
    let mut phis: Vec<(&str, &str, Vec<(&str, &str)>)> = Vec::new();
    let mut equal: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut branches: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut block = "";
    for l in line_list {
        if !l.starts_with(' ') && !l.starts_with("define") && let Some(k) = l.find(':') {
            block = &l[..k];
            continue;
        }
        let t = l.trim_start();
        if let Some(rest) = t.strip_prefix("br i1 %") {
            // `br i1 %c, label %v, label %f`
            let mut part_list = rest.split(", label %");
            if let (Some(c), Some(v)) = (part_list.next(), part_list.next()) {
                branches.insert(block, (c, v.trim()));
            }
            continue;
        }
        let Some(d) = defined(t) else { continue };
        let body = &t[t.find(" = ").map_or(0, |k| k + 3)..];
        if body.starts_with("icmp eq i64 ") {
            let mut u = uses_list(body);
            if let (Some(a), Some(b)) = (u.next(), u.next()) {
                equal.insert(d, (a, b));
            }
        } else if body.starts_with("phi ") {
            let entries = body
                .split('[')
                .skip(1)
                .filter_map(|pair| {
                    let mut x = pair.split(',');
                    let value = x.next()?.trim().strip_prefix('%')?;
                    let de = x.next()?.trim().trim_end_matches(']').trim().strip_prefix('%')?;
                    Some((value, de))
                })
                .collect();
            phis.push((d, block, entries));
        }
    }
    loop {
        let before = invariants.len();
        for (d, block, entries) in &phis {
            if invariants.contains(d) || entries.is_empty() {
                continue;
            }
            let canonical_one = |value: &str, de: &str| {
                invariants.contains(value)
                    || branches.get(de).is_some_and(|(c, v)| {
                        v == block
                            && equal.get(c).is_some_and(|(a, b)| (*a == value && invariants.contains(b)) || (*b == value && invariants.contains(a)))
                    })
            };
            if entries.iter().all(|(value, de)| canonical_one(value, de)) {
                invariants.insert(d);
            }
        }
        if invariants.len() == before {
            break;
        }
    }
    refs.retain(|r| !invariants.contains(r));
    refs
}

/// Um statepoint já visto no bloco: as raízes vivas nele e os nomes
/// definidos depois dele.
struct Point<'a> {
    live_set: HashSet<&'a str>,
    after: HashSet<&'a str>,
    line: usize,
}

/// Confere o IR de um módulo depois do RS4GC.
///
/// # Errors
/// A primeira violação: a linha do uso, o valor e a do statepoint.
pub fn check(ir: &str) -> Result<(), String> {
    let line_list: Vec<&str> = ir.lines().collect();
    let mut i = 0;
    while i < line_list.len() {
        if !line_list[i].starts_with("define ") {
            i += 1;
            continue;
        }
        let begin = i;
        let mut end = i + 1;
        while end < line_list.len() && !line_list[end].starts_with('}') {
            end += 1;
        }
        check_function(&line_list[begin..end.min(line_list.len())], begin)?;
        i = end + 1;
    }
    Ok(())
}

fn check_function(line_list: &[&str], offset: usize) -> Result<(), String> {
    // Os `Ref` da função: os nomes de algum `"deopt"` de statepoint.
    let refs: HashSet<&str> = line_list
        .iter()
        .filter(|l| l.contains("@llvm.experimental.gc.statepoint"))
        .filter_map(|l| deopt_operand(l))
        .flat_map(uses_list)
        .collect();
    if refs.is_empty() {
        return Ok(());
    }
    let refs = without_invariants(line_list, refs);
    let mut points: Vec<Point<'_>> = Vec::new();
    for (k, l) in line_list.iter().enumerate().skip(1) {
        let n = offset + k + 1;
        let t = l.trim_start();
        // Um rótulo começa um bloco novo; comentários e linhas vazias não contam.
        if !l.starts_with(' ') && !t.is_empty() {
            points.clear();
            continue;
        }
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        let def = defined(t);
        let body = match t.find(" = ") {
            Some(p) if def.is_some() => &t[p + 3..],
            _ => t,
        };
        let is_statepoint = body.contains("@llvm.experimental.gc.statepoint");
        // Os usos desta linha depois dos statepoints anteriores do bloco.
        if !body.contains("@llvm.experimental.gc.result") {
            for name in uses_list(without_operands(body)).filter(|x| refs.contains(x)) {
                for p in &points {
                    if !p.after.contains(name) && !p.live_set.contains(name) {
                        let sp = line_list[p.line - offset - 1].trim_start();
                        return Err(format!(
                            "conferidor do RS4GC, linha {n}: `%{name}` usado depois do statepoint da linha {} sem estar no \"deopt\" dele: `{t}`\n  {sp}",
                            p.line
                        ));
                    }
                }
            }
        }
        if is_statepoint {
            let live_set: HashSet<&str> = deopt_operand(body).map(|d| uses_list(d).collect()).unwrap_or_default();
            // C1: o argumento `Ref` está vivo na chamada.
            for name in uses_list(without_operands(body)).filter(|x| refs.contains(x)) {
                if !live_set.contains(name) {
                    return Err(format!(
                        "conferidor do RS4GC, linha {n}: o argumento `%{name}` não está vivo na chamada (fora do \"deopt\"): `{t}`"
                    ));
                }
            }
            points.push(Point { live_set, after: HashSet::new(), line: n });
        }
        if let Some(d) = def {
            for p in &mut points {
                p.after.insert(d);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    // Strings cruas: a indentação das instruções é o que separa uma
    // instrução de um rótulo.
    const BOM: &str = r#"define i64 @f(i64 %v1) gc "statepoint-example" {
b0:
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %v1, i32 0, i32 0) [ "deopt"(i64 %v1) ]
  %v2 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %v3 = add i64 %v1, %v2
  ret i64 %v3
}
"#;

    #[test]
    fn accepts_live_root_at_statepoint() {
        assert_eq!(check(BOM), Ok(()));
    }

    /// As violações plantadas: cada uma tem de ser achada. O `%v1` continua
    /// `Ref` pelo `"deopt"` de um segundo statepoint.
    #[test]
    fn finds_planted_violations() {
        let other_one = "  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ \"deopt\"(i64 %v1) ]\n  ret i64 %v3";
        let without_root = BOM.replace(r#"[ "deopt"(i64 %v1) ]"#, r#"[ "deopt"() ]"#).replace("  ret i64 %v3", other_one);
        assert_ne!(without_root, BOM);
        // O uso depois, sem o valor entre os argumentos.
        let only_after = without_root.replace("i32 1, i32 0, i64 %v1, i32 0, i32 0)", "i32 0, i32 0, i32 0, i32 0)");
        assert!(check(&only_after).unwrap_err().contains("usado depois"), "{only_after}");
        // O argumento sem raiz.
        assert!(check(&without_root).unwrap_err().contains("argumento"), "{without_root}");
    }

    /// O valor definido depois do statepoint não precisa estar nele; um
    /// rótulo novo zera os statepoints do bloco.
    #[test]
    fn does_not_flag_defined_after_or_other_block() {
        let ir = r#"define i64 @f(i64 %v1) gc "statepoint-example" {
b0:
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 ()) @g, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  %v4 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 ()) @g, i32 0, i32 0, i32 0, i32 0) [ "deopt"(i64 %v4, i64 %v1) ]
  br label %b1
b1:
  %v5 = add i64 %v4, %v1
  ret i64 %v5
}
"#;
        assert_eq!(check(ir), Ok(()));
    }

    /// A carga invariante (o `true` canônico do contexto) reaproveitada por
    /// cima de um statepoint não é acusada.
    #[test]
    fn does_not_flag_invariant_load() {
        let ir = r#"define i64 @f(ptr %ctx) gc "statepoint-example" {
b0:
  %vh = load i64, ptr %ctx, align 8, !invariant.load !0
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %vh, i32 0, i32 0) [ "deopt"(i64 %vh) ]
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  %v2 = add i64 %vh, 1
  ret i64 %v2
}
"#;
        assert_eq!(check(ir), Ok(()));
    }

    /// O `phi` que junta o canônico com o valor que a aresta compara a ele.
    #[test]
    fn does_not_flag_phi_guarded_by_equality() {
        let ir = r#"define i64 @f(ptr %ctx, i64 %v1) gc "statepoint-example" {
b0:
  %fp = getelementptr inbounds nuw i8, ptr %ctx, i64 368
  %fh = load i64, ptr %fp, align 8
  %c = icmp eq i64 %v1, %fh
  br i1 %c, label %b2, label %b1
b1:
  br label %b2
b2:
  %x = phi i64 [ %v1, %b0 ], [ %fh, %b1 ]
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"(i64 %v1) ]
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void (i64)) @g, i32 1, i32 0, i64 %x, i32 0, i32 0) [ "deopt"(i64 %x) ]
  %sp3 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  ret i64 %x
}
"#;
        assert_eq!(check(ir), Ok(()));
        // Sem a guarda, o `phi` é referência e o uso depois é acusado.
        let without = ir.replace("br i1 %c, label %b2, label %b1", "br label %b2");
        assert!(check(&without).is_err());
    }

    #[test]
    fn reads_names_and_operand() {
        assert_eq!(uses_list("add i64 %v1, %v2.i").collect::<Vec<_>>(), ["v1", "v2.i"]);
        assert_eq!(defined("  %v3 = add i64 1, 2"), Some("v3"));
        assert_eq!(deopt_operand("call void @g() [ \"deopt\"(i64 %v1, i64 %v2) ]"), Some("i64 %v1, i64 %v2"));
        assert_eq!(without_operands("call void @g(i64 %v3) [ \"deopt\"(i64 %v1) ]"), "call void @g(i64 %v3)");
    }
}
