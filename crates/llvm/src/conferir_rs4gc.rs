//! O conferidor das raízes **depois** do `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, no estilo do
//! `--statepoints` do Perry), sobre a forma implementada do §14.8: o valor
//! `Ref` continua `i64` (`%v<n>`), e a raiz dele é `%raiz<n> = inttoptr i64
//! %v<n> to ptr addrspace(1)`, mantida viva por `llvm.fake.use`.
//!
//! O RS4GC põe no `"gc-live"` de cada statepoint as raízes vivas ali, e a
//! coleta só enxerga essas. O defeito que este conferidor pega é o valor
//! `%v<n>` usado depois de um statepoint sem que a raiz dele esteja no
//! `"gc-live"` desse statepoint: o objeto pode ter sido liberado (o
//! otimizador que estica a vida de um `Ref` por cima de uma chamada, o
//! emissor que esquece o uso fictício). Também os argumentos: um `%v<n>`
//! passado à chamada do statepoint tem de estar vivo nela (contrato C1).
//!
//! A conferência é por bloco, sobre o texto do módulo: um uso depois de um
//! statepoint do mesmo bloco, de um valor que não foi definido depois dele.
//! O valor que atravessa o statepoint para um `phi` de outro bloco não é
//! conferido. Só os pares `%v<n>`/`%raiz<n>` com os nomes do emissor
//! entram (o otimizador pode ter renomeado o resto).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use std::collections::{HashMap, HashSet};

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

/// A raiz de base de um nome do `"gc-live"`: `raiz5.relocated3` → `raiz5`.
fn base(nome: &str) -> &str {
    nome.find(".relocated").map_or(nome, |k| &nome[..k])
}

/// As raízes do `"gc-live"` de uma linha de statepoint (as bases).
fn vivos_do_statepoint(linha: &str) -> HashSet<&str> {
    let Some(k) = linha.find("\"gc-live\"(") else { return HashSet::new() };
    let resto = &linha[k + "\"gc-live\"(".len()..];
    let mut profundidade = 1;
    let mut fim = resto.len();
    for (i, c) in resto.char_indices() {
        match c {
            '(' => profundidade += 1,
            ')' => {
                profundidade -= 1;
                if profundidade == 0 {
                    fim = i;
                    break;
                }
            }
            _ => {}
        }
    }
    usos(&resto[..fim]).map(base).collect()
}

/// O nome definido pela linha (`%x = …`), sem o `%`.
fn definido(linha: &str) -> Option<&str> {
    let l = linha.trim_start().strip_prefix('%')?;
    let fim = l.find(" = ")?;
    Some(&l[..fim])
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
    // `v<n>` → `raiz<n>`, das definições das raízes.
    let mut raiz_de: HashMap<&str, &str> = HashMap::new();
    for l in linhas {
        let t = l.trim_start();
        if let Some(r) = definido(t)
            && r.starts_with("raiz")
            && let Some(k) = t.find("inttoptr i64 %")
        {
            let v = usos(&t[k + "inttoptr i64 ".len()..]).next().unwrap_or_default();
            if r.strip_prefix("raiz").is_some_and(|n| v.strip_prefix('v') == Some(n)) {
                raiz_de.insert(v, r);
            }
        }
    }
    if raiz_de.is_empty() {
        return Ok(());
    }
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
        if !corpo.contains("@llvm.experimental.gc.relocate") && !corpo.contains("@llvm.experimental.gc.result") {
            let sem_vivos = corpo.find("[ \"gc-live\"").map_or(corpo, |p| &corpo[..p]);
            for nome in usos(sem_vivos) {
                let Some(raiz) = raiz_de.get(nome) else { continue };
                for p in &pontos {
                    if !p.depois.contains(nome) && !p.vivos.contains(raiz) {
                        return Err(format!(
                            "conferidor do RS4GC, linha {n}: `%{nome}` usado depois do statepoint da linha {} sem a raiz `%{raiz}` no \"gc-live\": `{t}`",
                            p.linha
                        ));
                    }
                }
            }
        }
        if e_statepoint {
            let vivos = vivos_do_statepoint(corpo);
            // C1: o argumento `Ref` está vivo na chamada.
            let args = corpo.find("[ \"gc-live\"").map_or(corpo, |p| &corpo[..p]);
            for nome in usos(args) {
                if let Some(raiz) = raiz_de.get(nome)
                    && !vivos.contains(raiz)
                {
                    return Err(format!(
                        "conferidor do RS4GC, linha {n}: o argumento `%{nome}` não está vivo na chamada (sem `%{raiz}` no \"gc-live\"): `{t}`"
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
  %raiz1 = inttoptr i64 %v1 to ptr addrspace(1)
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %v1, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %raiz1) ]
  %v2 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %raiz1.relocated = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %sp, i32 0, i32 0)
  %v3 = add i64 %v1, %v2
  call void (...) @llvm.fake.use(ptr addrspace(1) %raiz1.relocated)
  ret i64 %v3
}
"#;

    #[test]
    fn aceita_a_raiz_viva_no_statepoint() {
        assert_eq!(conferir(BOM), Ok(()));
    }

    /// As violações plantadas: cada uma tem de ser achada.
    #[test]
    fn acha_as_violacoes_plantadas() {
        let sem_raiz = BOM.replace(r#"[ "gc-live"(ptr addrspace(1) %raiz1) ]"#, r#"[ "gc-live"() ]"#);
        assert_ne!(sem_raiz, BOM);
        // O uso depois, sem o valor entre os argumentos.
        let so_depois = sem_raiz.replace("i32 1, i32 0, i64 %v1, i32 0, i32 0)", "i32 0, i32 0, i32 0, i32 0)");
        assert!(conferir(&so_depois).unwrap_err().contains("usado depois"), "{so_depois}");
        // O argumento sem raiz, sem uso depois.
        let so_argumento = sem_raiz.replace("%v3 = add i64 %v1, %v2", "%v3 = add i64 %v2, 1");
        assert!(conferir(&so_argumento).unwrap_err().contains("argumento"), "{so_argumento}");
    }

    /// O valor definido depois do statepoint não precisa de raiz nele; um
    /// rótulo novo zera os statepoints do bloco.
    #[test]
    fn nao_acusa_o_definido_depois_nem_outro_bloco() {
        let ir = r#"define i64 @f() gc "statepoint-example" {
b0:
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 ()) @g, i32 0, i32 0, i32 0, i32 0) [ "gc-live"() ]
  %v4 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %raiz4 = inttoptr i64 %v4 to ptr addrspace(1)
  %v5 = add i64 %v4, 1
  br label %b1
b1:
  ret i64 %v5
}
"#;
        assert_eq!(conferir(ir), Ok(()));
    }

    #[test]
    fn le_os_nomes_e_as_bases() {
        assert_eq!(usos("add i64 %v1, %raiz2.relocated").collect::<Vec<_>>(), ["v1", "raiz2.relocated"]);
        assert_eq!(base("raiz2.relocated3"), "raiz2");
        assert_eq!(definido("  %v3 = add i64 1, 2"), Some("v3"));
    }
}
