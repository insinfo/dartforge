//! Os projetos reais do proprietário (máquina local): o oráculo sobre o
//! projeto inteiro (medido: 0 diagnósticos nos três), os nossos falsos
//! positivos por código, e as **mutações determinísticas** — um erro plantado
//! num arquivo do projeto, o oráculo regravado sobre o mutante.
//!
//! Nada é escrito no projeto do proprietário: cada projeto é copiado para
//! `target/scratch-paridade/projetos/<nome>` (só os `.dart`, o `pubspec.yaml`,
//! o `analysis_options.yaml` e um `package_config.json` com as raízes
//! absolutas), e é a cópia que se muta. O registro fica em
//! `corpus/diagnosticos/projetos.json` e `corpus/diagnosticos/mutacoes.jsonl`;
//! uma mutação cujo arquivo original mudou (hash) é acusada como desatualizada.

use crate::oraculo::{FNV_INICIO, Registro, SdkOraculo, fnv};
use dartforge_frontend::ast::{DirectiveKind, ExprKind, TypeKind, UnaryOp};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Um projeto real.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projeto {
    pub nome: String,
    pub caminho: PathBuf,
    pub sdk: SdkOraculo,
    /// Diagnósticos do oráculo no projeto inteiro (registrado).
    #[serde(default)]
    pub oraculo: Option<usize>,
}

/// Os três projetos do plano.
pub fn padrao() -> Vec<Projeto> {
    let p = |nome: &str, c: &str| Projeto { nome: nome.into(), caminho: c.into(), sdk: SdkOraculo::V362, oraculo: None };
    vec![
        p("new_sali-core", "C:/MyDartProjects/new_sali/core"),
        p("new_sali-frontend", "C:/MyDartProjects/new_sali/frontend"),
        p("limitless_ui", "D:/Projects/dartforge/references/limitless_ui"),
    ]
}

/// Tipos de mutação.
pub const TIPOS: &[&str] = &["nome", "import", "tipo", "bang", "await"];

/// Uma mutação e o que o oráculo disse do arquivo mutado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutacao {
    pub projeto: String,
    /// Relativo à raiz do projeto, com `/`.
    pub arquivo: String,
    pub tipo: String,
    /// Bytes substituídos no original.
    pub inicio: usize,
    pub fim: usize,
    pub inserir: String,
    /// FNV-1a do arquivo original.
    pub hash_original: String,
    pub oraculo: Vec<Registro>,
}

impl Mutacao {
    pub fn aplicar(&self, original: &str) -> String {
        format!("{}{}{}", &original[..self.inicio], self.inserir, &original[self.fim..])
    }
}

pub fn hash_texto(t: &str) -> String {
    format!("{:016x}", fnv(t.as_bytes(), FNV_INICIO))
}

/// Arquivos `.dart` do projeto que o `dart analyze` veria (sem `exclude`).
pub fn arquivos(raiz: &Path, opcoes: &crate::filtros::Opcoes) -> Vec<PathBuf> {
    crate::corpus::arquivos_dart(raiz)
        .into_iter()
        .filter(|a| crate::oraculo::relativo(a, raiz).is_some_and(|r| !opcoes.excluido(&r)))
        .collect()
}

/// Copia o projeto para `destino` (fontes, pubspec, opções) e escreve um
/// `package_config.json` com raízes absolutas (o do próprio pacote, `../`).
/// Devolve o caminho do `package_config.json` da cópia.
pub fn copiar(p: &Projeto, destino: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(destino);
    for a in crate::corpus::arquivos_dart(&p.caminho) {
        let r = a.strip_prefix(&p.caminho).map_err(|_| "prefixo")?;
        let d = destino.join(r);
        std::fs::create_dir_all(d.parent().expect("pai")).map_err(|e| e.to_string())?;
        std::fs::copy(&a, &d).map_err(|e| e.to_string())?;
    }
    for f in ["pubspec.yaml", "analysis_options.yaml"] {
        if p.caminho.join(f).is_file() {
            std::fs::copy(p.caminho.join(f), destino.join(f)).map_err(|e| e.to_string())?;
        }
    }
    let orig = p.caminho.join(".dart_tool/package_config.json");
    let texto = std::fs::read_to_string(&orig).map_err(|e| format!("{}: {e}", orig.display()))?;
    let mut cfg: serde_json::Value = serde_json::from_str(&texto).map_err(|e| e.to_string())?;
    let base = orig.parent().expect("pai");
    if let Some(pkgs) = cfg.get_mut("packages").and_then(|v| v.as_array_mut()) {
        for pkg in pkgs {
            let Some(root) = pkg.get("rootUri").and_then(|v| v.as_str()).map(str::to_string) else { continue };
            let abs = if root.starts_with("file:") {
                root.clone()
            } else {
                let alvo = crate::analise::chave(&base.join(&root));
                if alvo == crate::analise::chave(&p.caminho) {
                    "../".to_string()
                } else {
                    format!("file:///{}", alvo.to_string_lossy().replace('\\', "/"))
                }
            };
            pkg["rootUri"] = serde_json::Value::String(abs);
        }
    }
    let d = destino.join(".dart_tool/package_config.json");
    std::fs::create_dir_all(d.parent().expect("pai")).map_err(|e| e.to_string())?;
    std::fs::write(&d, serde_json::to_string_pretty(&cfg).expect("JSON")).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Os sítios de um tipo de mutação num arquivo: `(início, fim, inserir)`.
pub fn sitios(texto: &str, tipo: &str) -> Vec<(usize, usize, String)> {
    let mut interner = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(texto, &mut interner);
    if !parsed.diagnostics.is_empty() {
        return Vec::new();
    }
    let ast = &parsed.ast;
    let fonte = |s: dartforge_diagnostics::Span| texto.get(s.start..s.end).unwrap_or("");
    let mut v = Vec::new();
    match tipo {
        "nome" => {
            for e in &ast.exprs {
                if let ExprKind::Identifier(n) = &e.kind {
                    let t = fonte(n.span);
                    if t.len() > 1 && t.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                        v.push((n.span.start, n.span.end, format!("{t}NaoExisteDf")));
                    }
                }
            }
        }
        "import" => {
            for d in &parsed.unit.directives {
                if let DirectiveKind::Import { .. } = d.kind {
                    v.push((d.span.start, d.span.end, String::new()));
                }
            }
        }
        "tipo" => {
            for t in &ast.types {
                if let TypeKind::Named { name, args } = &t.kind {
                    if name.len() == 1 && args.is_empty() && !t.nullable {
                        match fonte(t.span) {
                            "String" => v.push((t.span.start, t.span.end, "int".into())),
                            "int" => v.push((t.span.start, t.span.end, "String".into())),
                            _ => {}
                        }
                    }
                }
            }
        }
        "bang" => {
            for e in &ast.exprs {
                if let ExprKind::Unary { op: UnaryOp::NullAssert, .. } = e.kind {
                    if e.span.end > 0 && texto.as_bytes().get(e.span.end - 1) == Some(&b'!') {
                        v.push((e.span.end - 1, e.span.end, String::new()));
                    }
                }
            }
        }
        "await" => {
            for e in &ast.exprs {
                if let ExprKind::Await(op) = e.kind {
                    let alvo = ast.exprs[op.0 as usize].span.start;
                    if fonte(e.span).starts_with("await") && alvo > e.span.start {
                        v.push((e.span.start, alvo, String::new()));
                    }
                }
            }
        }
        _ => {}
    }
    v.sort();
    v
}

/// Escolhe até `por_tipo` mutações por tipo, em arquivos diferentes, de forma
/// determinística (ordem dos arquivos; o sítio pelo hash do caminho).
pub fn escolher(p: &Projeto, raiz_copia: &Path, arquivos: &[PathBuf], por_tipo: usize) -> Vec<Mutacao> {
    let mut out = Vec::new();
    // Só `lib/`: é o código do proprietário que o editor mostra.
    let libs: Vec<&PathBuf> = arquivos
        .iter()
        .filter(|a| crate::oraculo::relativo(a, raiz_copia).is_some_and(|r| r.starts_with("lib/") && !r.ends_with(".g.dart")))
        .collect();
    if libs.is_empty() {
        return out;
    }
    for (k, tipo) in TIPOS.iter().enumerate() {
        let mut n = 0;
        // Passo primo sobre a lista: arquivos espalhados pelo projeto.
        let passo = 7919 % libs.len().max(1) + 1;
        let mut idx = (k * 131) % libs.len();
        for _ in 0..libs.len() {
            if n >= por_tipo {
                break;
            }
            let a = libs[idx];
            idx = (idx + passo) % libs.len();
            let Ok(texto) = std::fs::read_to_string(a) else { continue };
            let s = sitios(&texto, tipo);
            if s.is_empty() {
                continue;
            }
            let rel = crate::oraculo::relativo(a, raiz_copia).expect("relativo");
            if out.iter().any(|m: &Mutacao| m.arquivo == rel) {
                continue;
            }
            let h = fnv(rel.as_bytes(), FNV_INICIO) as usize;
            let (inicio, fim, inserir) = s[h % s.len()].clone();
            out.push(Mutacao {
                projeto: p.nome.clone(),
                arquivo: rel,
                tipo: tipo.to_string(),
                inicio,
                fim,
                inserir,
                hash_original: hash_texto(&texto),
                oraculo: Vec::new(),
            });
            n += 1;
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sitios_de_cada_tipo() {
        let t = "import 'dart:async';\nFuture<int> f(String? ss) async { String xx = ss!; return await gg(xx); }\n";
        assert_eq!(sitios(t, "import").len(), 1);
        assert!(!sitios(t, "nome").is_empty());
        assert_eq!(sitios(t, "bang").len(), 1);
        assert_eq!(sitios(t, "await").len(), 1);
        let tipo = sitios(t, "tipo");
        assert_eq!(tipo.len(), 2, "{tipo:?}"); // `int` de `Future<int>` e o `String` local
        let m = Mutacao {
            projeto: "p".into(),
            arquivo: "a".into(),
            tipo: "await".into(),
            inicio: sitios(t, "await")[0].0,
            fim: sitios(t, "await")[0].1,
            inserir: String::new(),
            hash_original: String::new(),
            oraculo: vec![],
        };
        assert!(m.aplicar(t).contains("return gg(xx)"));
    }
}
