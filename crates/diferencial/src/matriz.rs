//! A matriz de suporte (V02): o estado de cada programa do corpus em cada
//! perfil, numa forma que se soma entre corpora, perfis e sistemas.
//!
//! O relatório diz "234/234 ok", e isso mistura coisas diferentes: um programa
//! que o perfil executa igual à referência, um que o perfil **recusa** com
//! diagnóstico do mesmo jeito que a referência (uma biblioteca que não existe
//! na plataforma), e um recurso que o perfil recusa e a referência executa.
//! Aqui cada par (programa, perfil) tem um estado só:
//!
//! * `suportado`: compilou e a saída (stdout e código) é a da referência;
//! * `recusado`: o perfil recusou na compilação, com o diagnóstico — igual à
//!   referência (`como a referência`) ou não (o recurso falta no perfil);
//! * `divergente`: executou e a saída difere da referência (inclusive os de
//!   `PENDENTES`, marcados);
//! * `não medido`: a referência não pôde ser obtida (ferramenta ausente,
//!   tempo esgotado), então nada se afirma.
//!
//! O arquivo é TSV (`programa  perfil  estado  detalhe`), uma linha por par;
//! `scripts/matriz.py` junta os de cada corpus, perfil e sistema na
//! `docs/MATRIZ.md`.

use std::fmt::Write;

use crate::processo::{CODIGO_TEMPO_ESGOTADO, Saida};
use crate::relatorio::{Divergencia, Resultado, comparar};

/// O estado de um programa num perfil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Suportado,
    Recusado,
    Divergente,
    NaoMedido,
}

impl Estado {
    pub fn nome(self) -> &'static str {
        match self {
            Estado::Suportado => "suportado",
            Estado::Recusado => "recusado",
            Estado::Divergente => "divergente",
            Estado::NaoMedido => "não medido",
        }
    }
}

/// A saída é a de uma compilação recusada: o harness prefixa o stderr com a
/// etapa (`[compile-js …]`, `[compile-native]`, `[jsprod …]`) quando o
/// compilador não produziu programa.
fn recusado_na_compilacao(s: &Saida) -> bool {
    ["[compile-js", "[compile-native]", "[jsprod"].iter().any(|p| s.stderr.starts_with(p))
}

/// A referência não foi obtida: ferramenta ausente (`Saida::erro`) ou tempo
/// esgotado.
fn referencia_ausente(s: &Saida) -> bool {
    s.codigo == -1 || s.codigo == CODIGO_TEMPO_ESGOTADO
}

/// Uma linha, sem tabulação nem quebra (o detalhe vai numa coluna do TSV).
fn uma_linha(s: &str) -> String {
    let t: String = s.lines().next().unwrap_or("").replace('\t', " ");
    if t.chars().count() > 160 {
        let mut c: String = t.chars().take(159).collect();
        c.push('…');
        c
    } else {
        t
    }
}

/// O estado de `obtido` contra a referência, com o detalhe.
pub fn classificar(referencia: &Saida, obtido: &Saida, pendente: bool) -> (Estado, String) {
    if referencia_ausente(referencia) {
        return (Estado::NaoMedido, uma_linha(referencia.primeira_linha_stderr()));
    }
    match comparar(referencia, obtido) {
        None if recusado_na_compilacao(obtido) => (Estado::Recusado, "como a referência".into()),
        None => (Estado::Suportado, String::new()),
        Some(_) if recusado_na_compilacao(obtido) => (Estado::Recusado, uma_linha(obtido.primeira_linha_stderr())),
        Some(d) => {
            let onde = match d {
                Divergencia::Codigo => format!("código de saída {} (referência {})", obtido.codigo, referencia.codigo),
                Divergencia::Linha(n) => format!("stdout linha {n}"),
            };
            (Estado::Divergente, if pendente { format!("PENDENTES; {onde}") } else { onde })
        }
    }
}

/// As linhas TSV dos resultados: o executor principal no perfil `perfil` e,
/// quando houve, a produção do JS em `js-prod`.
pub fn tsv(resultados: &[Resultado], perfil: &str) -> String {
    let mut out = String::new();
    for r in resultados {
        let referencia = r.referencia();
        if let Some(f) = &r.forge {
            let (e, d) = classificar(referencia, f, r.programa.pendente);
            let _ = writeln!(out, "{}\t{perfil}\t{}\t{d}", r.programa.nome, e.nome());
        }
        if let Some(p) = &r.producao {
            let (e, d) = classificar(referencia, p, r.programa.pendente);
            let _ = writeln!(out, "{}\tjs-prod\t{}\t{d}", r.programa.nome, e.nome());
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(stdout: &str, stderr: &str, codigo: i32) -> Saida {
        Saida { stdout: stdout.into(), stderr: stderr.into(), codigo }
    }

    #[test]
    fn os_quatro_estados() {
        let vm = s("1\n", "", 0);
        assert_eq!(classificar(&vm, &s("1\n", "", 0), false).0, Estado::Suportado);
        assert_eq!(classificar(&vm, &s("2\n", "", 0), false), (Estado::Divergente, "stdout linha 1".into()));
        assert_eq!(classificar(&vm, &s("1\n", "", 255), true).1, "PENDENTES; código de saída 255 (referência 0)");
        let recusa = s("", "[compile-native] não suportado no backend nativo: X\ndetalhe", 1);
        assert_eq!(classificar(&vm, &recusa, false), (Estado::Recusado, "[compile-native] não suportado no backend nativo: X".into()));
        // A VM também recusa (biblioteca ausente na plataforma): igual.
        let vm_recusa = s("", "main.dart:1:8: Error: Dart library 'dart:js' is not available", 254);
        let forge_recusa = s("", "[compile-native] biblioteca indisponível", 254);
        assert_eq!(classificar(&vm_recusa, &forge_recusa, false), (Estado::Recusado, "como a referência".into()));
        assert_eq!(classificar(&Saida::erro("dart ausente"), &vm, false).0, Estado::NaoMedido);
    }
}
