//! Nosso analisador sobre um conjunto de arquivos: carga, outline, inferência
//! e os diagnósticos de cada arquivo, já com código (pela [`crate::ponte`]).
//!
//! Um conjunto de arquivos vira **um programa só**: uma entrada sintética em
//! memória (`__dartforge_analise__.dart`, pela `Geracao` do `elements`)
//! importa cada biblioteca com um prefixo próprio. Cada arquivo continua
//! sendo a sua biblioteca, como no `dart analyze` de um diretório, e o SDK e
//! os pacotes são carregados e tipados uma vez por lote em vez de uma vez por
//! arquivo. Até o banco semântico (plano B2) existir, este é o único
//! condutor de análise por diretório.
//!
//! **Atribuição a arquivo.** O `Diagnostic` de `types` ainda não diz em que
//! unidade está (pedido T1). A inferência de corpos roda só nas bibliotecas
//! do lote (`infer_bodies_das_bibliotecas`), e cada diagnóstico é atribuído à
//! unidade que tem um nó (expressão, comando, tipo ou padrão) com exatamente
//! aquele intervalo; sem nó exato, à unidade do lote cuja declaração de topo
//! o contém. Casos com mais de uma candidata são contados em
//! [`Analise::ambiguos`] — o placar os mostra.

use crate::ponte;
use dartforge_diagnostics::{Diagnostic, Span, codigos};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::DirectiveKind;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Nome da entrada sintética (nunca existe no disco).
pub const ENTRADA: &str = "__dartforge_analise__.dart";

/// O SDK carregado uma vez e reaproveitado por todas as análises.
pub struct Motor {
    sdk: SdkLayout,
    /// Todas as bibliotecas `dart:` que o analyzer conhece: as de qualquer
    /// seção do `libraries.json` (o analyzer não se limita ao DDC).
    bibliotecas_sdk: BTreeSet<String>,
}

/// Diagnósticos de um arquivo, com o texto dele (para converter posições).
#[derive(Debug, Default)]
pub struct Arquivo {
    pub texto: String,
    pub diags: Vec<Diagnostic>,
    /// Quantos dos primeiros `diags` são sintáticos (publicados sempre).
    pub sintaticos: usize,
}

/// Resultado de uma análise: por caminho absoluto normalizado.
#[derive(Debug, Default)]
pub struct Analise {
    pub arquivos: BTreeMap<PathBuf, Arquivo>,
    /// Diagnósticos com mais de uma unidade candidata.
    pub ambiguos: usize,
}

/// Caminho normalizado (lexical, sem `\\?\`), a chave de comparação de arquivos.
pub fn chave(p: &Path) -> PathBuf {
    dartforge_elements::gerado::chave(p)
}

/// O arquivo é uma parte (`part of`)? Olha as diretivas antes da primeira declaração.
pub fn e_parte(texto: &str) -> bool {
    let mut em_bloco = false;
    for linha in texto.lines() {
        let t = linha.trim();
        if em_bloco {
            if t.contains("*/") {
                em_bloco = false;
            }
            continue;
        }
        if t.is_empty() || t.starts_with("//") || t.starts_with('@') || t.starts_with("#!") {
            continue;
        }
        if t.starts_with("/*") {
            em_bloco = !t.contains("*/");
            continue;
        }
        return t.starts_with("part of");
    }
    false
}

/// Sufixos de arquivo gerado (`file_paths.isGenerated`, analyzer 6.11.0).
fn gerado(uri: &str) -> bool {
    [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart"]
        .iter()
        .any(|s| uri.ends_with(s))
}

impl Motor {
    pub fn novo(sdk_lib: &Path) -> Result<Motor, String> {
        let sdk = SdkLayout::load(sdk_lib, "dartdevc")?;
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(sdk_lib.join("libraries.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let mut bibliotecas_sdk = BTreeSet::new();
        if let Some(o) = json.as_object() {
            for secao in o.values() {
                if let Some(libs) = secao.get("libraries").and_then(|l| l.as_object()) {
                    bibliotecas_sdk.extend(libs.keys().cloned());
                }
            }
        }
        Ok(Motor { sdk, bibliotecas_sdk })
    }

    /// O SDK do `DARTFORGE_SDK_LIB`/`DART_SDK`/`PATH`, ou o 3.6.2 padrão da máquina.
    pub fn descobrir() -> Result<Motor, String> {
        let lib = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
        Motor::novo(&lib)
    }

    /// Analisa `arquivos` (absolutos, dentro de `raiz`), cada um como a sua
    /// biblioteca. `packages` é o `package_config.json`.
    pub fn analisar(&self, raiz: &Path, arquivos: &[PathBuf], packages: Option<&Path>) -> Analise {
        let mut analise = Analise::default();
        let mut proprios: BTreeMap<PathBuf, String> = BTreeMap::new();
        let mut imports = String::new();
        for (i, a) in arquivos.iter().enumerate() {
            let Ok(texto) = std::fs::read_to_string(a) else { continue };
            if !e_parte(&texto) {
                let rel = a.strip_prefix(raiz).unwrap_or(a).to_string_lossy().replace('\\', "/");
                let rel: String = rel
                    .chars()
                    .map(|c| if c == ' ' { "%20".to_string() } else { c.to_string() })
                    .collect();
                imports.push_str(&format!("import '{rel}' as _dfp{i};\n"));
            }
            proprios.insert(chave(a), texto);
        }
        for (p, t) in &proprios {
            analise.arquivos.insert(p.clone(), Arquivo { texto: t.clone(), diags: Vec::new(), sintaticos: 0 });
        }
        if imports.is_empty() {
            return analise;
        }
        let entrada = raiz.join(ENTRADA);
        let mut c = dartforge_elements::gerado::Construtor::nova();
        c.por(entrada.clone(), imports, "paridade", vec![]);
        let geracao = c.concluir(1).ok();
        let cache = dartforge_elements::SdkCache::abrir_ou_construir(&self.sdk, "dartdevc").ok().map(|(c, _)| c);
        let mut interner = dartforge_intern::Interner::new();
        let (program, diags_carga) = dartforge_elements::load::load_lenient_gerados(
            &entrada,
            &self.sdk,
            packages,
            &mut interner,
            cache,
            None,
            geracao,
        );

        // Unidades do lote.
        let mut unidade_de: HashMap<PathBuf, UnitId> = HashMap::new();
        for (i, u) in program.units.iter().enumerate() {
            if let Some(p) = &u.path {
                let k = chave(p);
                if proprios.contains_key(&k) && !program.library(u.library).is_sdk {
                    unidade_de.insert(k, UnitId(i as u32));
                }
            }
        }
        let libs_proprias: Vec<LibraryId> = {
            let s: BTreeSet<LibraryId> = unidade_de.values().map(|u| program.unit(*u).library).collect();
            s.into_iter().collect()
        };
        let unidades_proprias: BTreeSet<UnitId> = unidade_de.values().copied().collect();

        // 1. Sintaxe: o `elements` prefixa o caminho e o offset na mensagem.
        for d in &diags_carga {
            for (k, u) in &unidade_de {
                let Some(p) = &program.unit(*u).path else { continue };
                let prefixo = format!("{}:{}: ", p.display(), d.span.start);
                if let Some(msg) = d.message.strip_prefix(&prefixo) {
                    let cru = Diagnostic::new(msg, d.span);
                    let a = analise.arquivos.get_mut(k).expect("próprio");
                    a.diags.push(ponte::codificar_sintaxe(&cru));
                    a.sintaticos += 1;
                    break;
                }
            }
        }

        // 2. Diretivas cujo alvo não existe.
        let config = packages.and_then(|p| dartforge_elements::PackageConfig::load(p).ok());
        for (k, u) in &unidade_de {
            let unit = program.unit(*u);
            let base = unit.path.as_deref().and_then(Path::parent).unwrap_or(raiz).to_path_buf();
            for dir in &unit.unit.directives {
                let lit = match &dir.kind {
                    DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } | DirectiveKind::Part { uri } => uri,
                    _ => continue,
                };
                let Some(texto) = dartforge_elements::load::string_lit_value(lit) else { continue };
                if let Some(d) = self.diretiva_sem_alvo(&texto, &base, config.as_ref(), lit.span) {
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                }
            }
        }

        // 3. Tipos: outline e corpos das bibliotecas do lote.
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, diags_outline) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let (_corpos, diags_corpos) = dartforge_types::infer_bodies_das_bibliotecas(
            &program,
            &interner,
            &mut table,
            &core,
            &mut outline,
            &libs_proprias,
        );
        let indice = Indice::novo(&program, &unidades_proprias);
        let mut vistos: BTreeSet<(UnitId, usize, usize, String)> = BTreeSet::new();
        for d in diags_outline.iter().chain(diags_corpos.iter()) {
            let (unidade, ambiguo) = match indice.atribuir(&program, d.span) {
                Some(x) => x,
                None => continue,
            };
            if ambiguo {
                analise.ambiguos += 1;
            }
            if !unidades_proprias.contains(&unidade) {
                continue;
            }
            let fonte = &program.unit(unidade).source;
            let trecho = fonte.get(d.span.start..d.span.end).unwrap_or("");
            let cod = ponte::codificar_tipos(d, trecho);
            if !vistos.insert((unidade, cod.span.start, cod.span.end, cod.message.clone())) {
                continue;
            }
            let k = chave(program.unit(unidade).path.as_deref().expect("próprio tem caminho"));
            analise.arquivos.get_mut(&k).expect("próprio").diags.push(cod);
        }
        analise
    }

    /// `URI_DOES_NOT_EXIST` / `URI_HAS_NOT_BEEN_GENERATED`
    /// (`library_analyzer.dart:670-720`): o alvo da diretiva não existe.
    fn diretiva_sem_alvo(
        &self,
        uri: &str,
        base: &Path,
        config: Option<&dartforge_elements::PackageConfig>,
        span: Span,
    ) -> Option<Diagnostic> {
        let existe = if let Some(nome) = uri.strip_prefix("dart:") {
            self.bibliotecas_sdk.contains(nome)
        } else if uri.starts_with("package:") {
            match config.map(|c| c.resolve_package_uri(uri)) {
                Some(Ok(p)) => p.is_file(),
                _ => false,
            }
        } else if uri.contains(':') {
            // Outros esquemas (`dart-ext:`, `http:`): fora do escopo.
            return None;
        } else {
            base.join(uri).is_file()
        };
        if existe {
            return None;
        }
        let codigo = if !uri.starts_with("dart:") && gerado(uri) {
            codigos::compile_time_error::URI_HAS_NOT_BEEN_GENERATED
        } else {
            codigos::compile_time_error::URI_DOES_NOT_EXIST
        };
        Some(Diagnostic::com_codigo(codigo, span, [uri]))
    }
}

/// Intervalos de nós por unidade, para atribuir diagnósticos sem unidade.
struct Indice {
    exato: HashMap<(usize, usize), Vec<UnitId>>,
    /// Intervalos das declarações de topo das unidades do lote.
    topo: Vec<(UnitId, Span)>,
    proprias: BTreeSet<UnitId>,
}

impl Indice {
    fn novo(program: &Program, proprias: &BTreeSet<UnitId>) -> Indice {
        let mut exato: HashMap<(usize, usize), Vec<UnitId>> = HashMap::new();
        let mut topo = Vec::new();
        for (i, u) in program.units.iter().enumerate() {
            if program.library(u.library).is_sdk {
                continue;
            }
            let id = UnitId(i as u32);
            let mut por = |s: Span| {
                let v = exato.entry((s.start, s.end)).or_default();
                if v.last() != Some(&id) {
                    v.push(id);
                }
            };
            u.ast.exprs.iter().for_each(|e| por(e.span));
            u.ast.stmts.iter().for_each(|e| por(e.span));
            u.ast.types.iter().for_each(|e| por(e.span));
            u.ast.patterns.iter().for_each(|e| por(e.span));
            if proprias.contains(&id) {
                for d in &u.unit.declarations {
                    topo.push((id, u.ast.decls[d.0 as usize].span));
                }
            }
        }
        Indice { exato, topo, proprias: proprias.clone() }
    }

    /// A unidade do diagnóstico e se houve mais de uma candidata.
    fn atribuir(&self, _program: &Program, s: Span) -> Option<(UnitId, bool)> {
        if let Some(v) = self.exato.get(&(s.start, s.end)) {
            let escolhida = v.iter().copied().find(|u| self.proprias.contains(u)).unwrap_or(v[0]);
            return Some((escolhida, v.len() > 1));
        }
        let mut cands = self.topo.iter().filter(|(_, d)| d.start <= s.start && s.end <= d.end).map(|(u, _)| *u);
        let primeira = cands.next()?;
        let outra = cands.any(|u| u != primeira);
        Some((primeira, outra))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn parte_pela_primeira_diretiva() {
        assert!(e_parte("// c\n/* x\n y */\npart of 'a.dart';\n"));
        assert!(!e_parte("library a;\npart 'b.dart';\n"));
        assert!(!e_parte("void main() {}\n"));
    }
}
