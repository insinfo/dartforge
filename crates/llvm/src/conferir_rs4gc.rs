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

/// Os nomes `%x` usados em `texto` (sem o `%`).
fn usos(texto: &str) -> impl Iterator<Item = &str> {
    let b = texto.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] == b'%' {
                let ini = i + 1;
                let mut fim = ini;
                while fim < b.len() && (b[fim].is_ascii_alphanumeric() || matches!(b[fim], b'.' | b'_' | b'$' | b'-')) {
                    fim += 1;
                }
                i = fim.max(ini);
                if fim > ini {
                    return Some(&texto[ini..fim]);
                }
            } else {
                i += 1;
            }
        }
        None
    })
}

/// O conteúdo do operando `"deopt"(…)` de uma linha, se ela o tem.
fn operando_deopt(linha: &str) -> Option<&str> {
    let k = linha.find("\"deopt\"(")?;
    let resto = &linha[k + "\"deopt\"(".len()..];
    let mut profundidade = 1;
    for (i, c) in resto.char_indices() {
        match c {
            '(' => profundidade += 1,
            ')' => {
                profundidade -= 1;
                if profundidade == 0 {
                    return Some(&resto[..i]);
                }
            }
            _ => {}
        }
    }
    Some(resto)
}

/// A linha sem os operandos (`[ … ]` depois dos argumentos).
fn sem_operandos(corpo: &str) -> &str {
    corpo.find(" [ \"").map_or(corpo, |p| &corpo[..p])
}

/// O nome definido pela linha (`%x = …`), sem o `%`.
fn definido(linha: &str) -> Option<&str> {
    let l = linha.trim_start().strip_prefix('%')?;
    let fim = l.find(" = ")?;
    Some(&l[..fim])
}

/// `refs` sem os valores de carga `!invariant.load` e sem os `phi` cujas
/// entradas são todas desses (ou constantes).
fn sem_invariantes<'a>(linhas: &[&'a str], mut refs: HashSet<&'a str>) -> HashSet<&'a str> {
    // Os endereços dos canônicos: `getelementptr … i64 360|368`, direto ou
    // pelo deslocamento escolhido num `select` (o `df.caixa_bool`).
    let selecoes: HashSet<&str> = linhas
        .iter()
        .filter(|l| l.contains("= select ") && l.contains("i64 360") && l.contains("i64 368"))
        .filter_map(|l| definido(l))
        .collect();
    let canonicos: HashSet<&str> = linhas
        .iter()
        .filter(|l| {
            l.contains("= getelementptr ")
                && (l.ends_with("i64 360") || l.ends_with("i64 368") || selecoes.iter().any(|s| l.ends_with(&format!("i64 %{s}"))))
        })
        .filter_map(|l| definido(l))
        .collect();
    let mut invariantes: HashSet<&str> = linhas
        .iter()
        .filter(|l| {
            l.contains("!invariant.load")
                || l.contains("= load i64, ptr %")
                    && usos(&l[l.find("= load").unwrap_or(0)..]).next().is_some_and(|p| canonicos.contains(p))
        })
        .filter_map(|l| definido(l))
        .collect();
    // Os `phi` (com o bloco de cada um), os `icmp eq` e os desvios
    // condicionais de cada bloco: a entrada de um `phi` que chega pela aresta
    // em que `X == canônico` é o próprio canônico (o otimizador troca um pelo
    // outro).
    let mut phis: Vec<(&str, &str, Vec<(&str, &str)>)> = Vec::new();
    let mut iguais: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut desvios: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut bloco = "";
    for l in linhas {
        if !l.starts_with(' ') && !l.starts_with("define") && let Some(k) = l.find(':') {
            bloco = &l[..k];
            continue;
        }
        let t = l.trim_start();
        if let Some(resto) = t.strip_prefix("br i1 %") {
            // `br i1 %c, label %v, label %f`
            let mut partes = resto.split(", label %");
            if let (Some(c), Some(v)) = (partes.next(), partes.next()) {
                desvios.insert(bloco, (c, v.trim()));
            }
            continue;
        }
        let Some(d) = definido(t) else { continue };
        let corpo = &t[t.find(" = ").map_or(0, |k| k + 3)..];
        if corpo.starts_with("icmp eq i64 ") {
            let mut u = usos(corpo);
            if let (Some(a), Some(b)) = (u.next(), u.next()) {
                iguais.insert(d, (a, b));
            }
        } else if corpo.starts_with("phi ") {
            let entradas = corpo
                .split('[')
                .skip(1)
                .filter_map(|par| {
                    let mut x = par.split(',');
                    let valor = x.next()?.trim().strip_prefix('%')?;
                    let de = x.next()?.trim().trim_end_matches(']').trim().strip_prefix('%')?;
                    Some((valor, de))
                })
                .collect();
            phis.push((d, bloco, entradas));
        }
    }
    loop {
        let antes = invariantes.len();
        for (d, bloco, entradas) in &phis {
            if invariantes.contains(d) || entradas.is_empty() {
                continue;
            }
            let canonica = |valor: &str, de: &str| {
                invariantes.contains(valor)
                    || desvios.get(de).is_some_and(|(c, v)| {
                        v == bloco
                            && iguais.get(c).is_some_and(|(a, b)| (*a == valor && invariantes.contains(b)) || (*b == valor && invariantes.contains(a)))
                    })
            };
            if entradas.iter().all(|(valor, de)| canonica(valor, de)) {
                invariantes.insert(d);
            }
        }
        if invariantes.len() == antes {
            break;
        }
    }
    refs.retain(|r| !invariantes.contains(r));
    refs
}

/// Um statepoint já visto no bloco: as raízes vivas nele e os nomes
/// definidos depois dele.
struct Ponto<'a> {
    vivos: HashSet<&'a str>,
    depois: HashSet<&'a str>,
    linha: usize,
}

/// Confere o IR de um módulo depois do RS4GC.
///
/// # Errors
/// A primeira violação: a linha do uso, o valor e a do statepoint.
pub fn conferir(ir: &str) -> Result<(), String> {
    let linhas: Vec<&str> = ir.lines().collect();
    let mut i = 0;
    while i < linhas.len() {
        if !linhas[i].starts_with("define ") {
            i += 1;
            continue;
        }
        let ini = i;
        let mut fim = i + 1;
        while fim < linhas.len() && !linhas[fim].starts_with('}') {
            fim += 1;
        }
        conferir_funcao(&linhas[ini..fim.min(linhas.len())], ini)?;
        i = fim + 1;
    }
    Ok(())
}

fn conferir_funcao(linhas: &[&str], deslocamento: usize) -> Result<(), String> {
    // Os `Ref` da função: os nomes de algum `"deopt"` de statepoint.
    let refs: HashSet<&str> = linhas
        .iter()
        .filter(|l| l.contains("@llvm.experimental.gc.statepoint"))
        .filter_map(|l| operando_deopt(l))
        .flat_map(usos)
        .collect();
    if refs.is_empty() {
        return Ok(());
    }
    let refs = sem_invariantes(linhas, refs);
    let mut pontos: Vec<Ponto<'_>> = Vec::new();
    for (k, l) in linhas.iter().enumerate().skip(1) {
        let n = deslocamento + k + 1;
        let t = l.trim_start();
        // Um rótulo começa um bloco novo; comentários e linhas vazias não contam.
        if !l.starts_with(' ') && !t.is_empty() {
            pontos.clear();
            continue;
        }
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        let def = definido(t);
        let corpo = match t.find(" = ") {
            Some(p) if def.is_some() => &t[p + 3..],
            _ => t,
        };
        let e_statepoint = corpo.contains("@llvm.experimental.gc.statepoint");
        // Os usos desta linha depois dos statepoints anteriores do bloco.
        if !corpo.contains("@llvm.experimental.gc.result") {
            for nome in usos(sem_operandos(corpo)).filter(|x| refs.contains(x)) {
                for p in &pontos {
                    if !p.depois.contains(nome) && !p.vivos.contains(nome) {
                        let sp = linhas[p.linha - deslocamento - 1].trim_start();
                        return Err(format!(
                            "conferidor do RS4GC, linha {n}: `%{nome}` usado depois do statepoint da linha {} sem estar no \"deopt\" dele: `{t}`\n  {sp}",
                            p.linha
                        ));
                    }
                }
            }
        }
        if e_statepoint {
            let vivos: HashSet<&str> = operando_deopt(corpo).map(|d| usos(d).collect()).unwrap_or_default();
            // C1: o argumento `Ref` está vivo na chamada.
            for nome in usos(sem_operandos(corpo)).filter(|x| refs.contains(x)) {
                if !vivos.contains(nome) {
                    return Err(format!(
                        "conferidor do RS4GC, linha {n}: o argumento `%{nome}` não está vivo na chamada (fora do \"deopt\"): `{t}`"
                    ));
                }
            }
            pontos.push(Ponto { vivos, depois: HashSet::new(), linha: n });
        }
        if let Some(d) = def {
            for p in &mut pontos {
                p.depois.insert(d);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
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
    fn aceita_a_raiz_viva_no_statepoint() {
        assert_eq!(conferir(BOM), Ok(()));
    }

    /// As violações plantadas: cada uma tem de ser achada. O `%v1` continua
    /// `Ref` pelo `"deopt"` de um segundo statepoint.
    #[test]
    fn acha_as_violacoes_plantadas() {
        let outro = "  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ \"deopt\"(i64 %v1) ]\n  ret i64 %v3";
        let sem_raiz = BOM.replace(r#"[ "deopt"(i64 %v1) ]"#, r#"[ "deopt"() ]"#).replace("  ret i64 %v3", outro);
        assert_ne!(sem_raiz, BOM);
        // O uso depois, sem o valor entre os argumentos.
        let so_depois = sem_raiz.replace("i32 1, i32 0, i64 %v1, i32 0, i32 0)", "i32 0, i32 0, i32 0, i32 0)");
        assert!(conferir(&so_depois).unwrap_err().contains("usado depois"), "{so_depois}");
        // O argumento sem raiz.
        assert!(conferir(&sem_raiz).unwrap_err().contains("argumento"), "{sem_raiz}");
    }

    /// O valor definido depois do statepoint não precisa estar nele; um
    /// rótulo novo zera os statepoints do bloco.
    #[test]
    fn nao_acusa_o_definido_depois_nem_outro_bloco() {
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
        assert_eq!(conferir(ir), Ok(()));
    }

    /// A carga invariante (o `true` canônico do contexto) reaproveitada por
    /// cima de um statepoint não é acusada.
    #[test]
    fn nao_acusa_a_carga_invariante() {
        let ir = r#"define i64 @f(ptr %ctx) gc "statepoint-example" {
b0:
  %vh = load i64, ptr %ctx, align 8, !invariant.load !0
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %vh, i32 0, i32 0) [ "deopt"(i64 %vh) ]
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  %v2 = add i64 %vh, 1
  ret i64 %v2
}
"#;
        assert_eq!(conferir(ir), Ok(()));
    }

    /// O `phi` que junta o canônico com o valor que a aresta compara a ele.
    #[test]
    fn nao_acusa_o_phi_guardado_pela_igualdade() {
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
        assert_eq!(conferir(ir), Ok(()));
        // Sem a guarda, o `phi` é referência e o uso depois é acusado.
        let sem = ir.replace("br i1 %c, label %b2, label %b1", "br label %b2");
        assert!(conferir(&sem).is_err());
    }

    #[test]
    fn le_os_nomes_e_o_operando() {
        assert_eq!(usos("add i64 %v1, %v2.i").collect::<Vec<_>>(), ["v1", "v2.i"]);
        assert_eq!(definido("  %v3 = add i64 1, 2"), Some("v3"));
        assert_eq!(operando_deopt("call void @g() [ \"deopt\"(i64 %v1, i64 %v2) ]"), Some("i64 %v1, i64 %v2"));
        assert_eq!(sem_operandos("call void @g(i64 %v3) [ \"deopt\"(i64 %v1) ]"), "call void @g(i64 %v3)");
    }
}
