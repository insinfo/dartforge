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
//! unidade está (pedido T1). Até lá: corpos, uma passada de inferência por
//! biblioteca do lote (o que ela acrescenta depois dos diagnósticos dos
//! inicializadores é daquela biblioteca); inicializadores, na ordem de
//! `program.variables`, cada um no primeiro inicializador (a partir do
//! anterior) que o contém; outline, a unidade com um nó (expressão, comando,
//! tipo ou padrão) de intervalo exato, ou a do lote cuja declaração de topo o
//! contém. Casos com mais de uma candidata são contados em
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

/// O arquivo `alvo` (de um `lib/` de pacote) existe como saída do `build_runner`
/// (`.dart_tool/build/generated/<pacote>/lib/<rel>`)? O analyzer os enxerga.
fn gerado_pelo_build(c: &dartforge_elements::PackageConfig, alvo: &Path) -> bool {
    c.package_dirs.iter().any(|(nome, dir)| {
        alvo.strip_prefix(dir)
            .ok()
            .and_then(|rel| c.generated_path(nome, &rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/")))
            .is_some()
    })
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
        // O que ali não é sintático (marcador de versão) entra depois, fora da
        // faixa publicada sempre.
        let mut da_carga_semanticos: Vec<(PathBuf, Diagnostic)> = Vec::new();
        for d in &diags_carga {
            for (k, u) in &unidade_de {
                let Some(p) = &program.unit(*u).path else { continue };
                let prefixo = format!("{}:{}: ", p.display(), d.span.start);
                if let Some(msg) = d.message.strip_prefix(&prefixo) {
                    let mut cru = d.clone();
                    cru.message = msg.to_string();
                    let sintatico = cru.code.is_none_or(|c| c.info().tipo == dartforge_diagnostics::TipoErro::SyntacticError);
                    if sintatico {
                        let a = analise.arquivos.get_mut(k).expect("próprio");
                        a.diags.push(ponte::codificar_sintaxe(&cru));
                        a.sintaticos += 1;
                    } else {
                        da_carga_semanticos.push((k.clone(), cru));
                    }
                    break;
                }
            }
        }
        for (k, d) in da_carga_semanticos {
            analise.arquivos.get_mut(&k).expect("próprio").diags.push(d);
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

        // 3. Nomes duplicados (`crates/analise`), por biblioteca do lote.
        for lib in &libs_proprias {
            let biblioteca = program.library(*lib);
            let ids: Vec<UnitId> = biblioteca
                .units
                .iter()
                .copied()
                .filter(|u| program.unit(*u).role != dartforge_elements::model::UnitRole::Patch)
                .collect();
            let unidades: Vec<dartforge_analise::Unidade<'_>> = ids
                .iter()
                .map(|u| dartforge_analise::Unidade { ast: &program.unit(*u).ast, unit: &program.unit(*u).unit, fonte: &program.unit(*u).source })
                .collect();
            let curinga = biblioteca.features.tem(dartforge_frontend::features::Feature::WildcardVariables);
            let mut achados = dartforge_analise::duplicatas::duplicatas(&unidades, &interner, curinga);
            achados.extend(dartforge_analise::enums::sem_constantes(&unidades));
            for (i, u) in unidades.iter().enumerate() {
                achados.extend(dartforge_analise::locais::nao_usados(*u, &interner, curinga).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::externos::inicializadores(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::operadores::aridade(*u, &interner).into_iter().map(|d| (i, d)));
            }
            for (i, d) in achados {
                if let Some(p) = &program.unit(ids[i]).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
            for (u, d) in dartforge_analise::heranca::estatico_contra_super(&program, *lib, &interner) {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
            for (u, d) in dartforge_analise::heranca::classe_usada_como_mixin(&program, *lib, &interner) {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
        }

        // 4. Tipos.
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, diags_outline) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let indice = Indice::novo(&program, &unidades_proprias);
        // Inicializadores: `types` os infere em toda passada, de qualquer
        // biblioteca. Uma passada sem corpo nenhum (`&[]`) estabiliza os tipos
        // inferidos; a segunda dá só os diagnósticos deles.
        let _ = dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[]);
        let (_, diags_init) =
            dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[]);
        let mut atribuidos: Vec<(UnitId, Diagnostic)> = Vec::new();
        // Corpos: uma passada por biblioteca do lote.
        for lib in &libs_proprias {
            let (_, ds) = dartforge_types::infer_bodies_das_bibliotecas(
                &program,
                &interner,
                &mut table,
                &core,
                &mut outline,
                std::slice::from_ref(lib),
            );
            let corpo = if ds.len() >= diags_init.len() && ds[..diags_init.len()] == diags_init[..] {
                &ds[diags_init.len()..]
            } else {
                // O prefixo mudou: não dá para separar; conta como ambíguo.
                analise.ambiguos += 1;
                &ds[..]
            };
            let unidades: Vec<UnitId> = program.library(*lib).units.clone();
            for d in corpo {
                let u = if unidades.len() == 1 {
                    Some(unidades[0])
                } else {
                    indice.atribuir_entre(&program, d.span, &unidades)
                };
                if let Some(u) = u {
                    atribuidos.push((u, d.clone()));
                }
            }
        }
        // Inicializadores, em ordem, no primeiro inicializador que os contém.
        let inits = inicializadores(&program);
        let mut pos = 0;
        for d in &diags_init {
            match (pos..inits.len()).find(|&i| inits[i].1.start <= d.span.start && d.span.end <= inits[i].1.end) {
                Some(i) => {
                    pos = i;
                    atribuidos.push((inits[i].0, d.clone()));
                }
                None => {
                    if let Some((u, amb)) = indice.atribuir(d.span) {
                        analise.ambiguos += usize::from(amb);
                        atribuidos.push((u, d.clone()));
                    }
                }
            }
        }
        // Outline: pelo intervalo exato do tipo anotado.
        for d in &diags_outline {
            if let Some((u, amb)) = indice.atribuir(d.span) {
                analise.ambiguos += usize::from(amb);
                atribuidos.push((u, d.clone()));
            }
        }
        let mut vistos: BTreeSet<(UnitId, usize, usize, String)> = BTreeSet::new();
        for (unidade, d) in &atribuidos {
            let unidade = *unidade;
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

        // 5. Imports não usados, depois de tudo (a supressão olha os
        // diagnósticos da biblioteca).
        for lib in &libs_proprias {
            let chaves: Vec<PathBuf> =
                program.library(*lib).units.iter().filter_map(|u| program.unit(*u).path.as_deref().map(chave)).collect();
            let ja: Vec<Diagnostic> = chaves
                .iter()
                .filter_map(|k| analise.arquivos.get(k))
                .flat_map(|a| a.diags.iter().cloned())
                .collect();
            for (u, d) in dartforge_analise::importacoes::nao_usados(&program, *lib, &interner, &ja) {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
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
            let alvo = chave(&base.join(uri));
            alvo.is_file() || config.is_some_and(|c| gerado_pelo_build(c, &alvo))
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

/// `(unidade, intervalo do inicializador)` das variáveis fora do SDK, na
/// ordem de `program.variables` (a ordem em que `types` os infere).
fn inicializadores(program: &Program) -> Vec<(UnitId, Span)> {
    use dartforge_elements::model::VariableRef;
    use dartforge_frontend::ast::{DeclKind, MemberKind};
    let mut v = Vec::new();
    for var in &program.variables {
        if program.library(var.library).is_sdk {
            continue;
        }
        let (unit, init) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => match &program.unit(unit).ast.decls[decl.0 as usize].kind {
                DeclKind::Variables(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => continue,
            },
            VariableRef::Field { unit, member, index } => {
                match &program.unit(unit).ast.members[member.0 as usize].kind {
                    MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                    _ => continue,
                }
            }
            _ => continue,
        };
        if let Some(e) = init {
            v.push((unit, program.unit(unit).ast.exprs[e.0 as usize].span));
        }
    }
    v
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

    /// Entre as `unidades` dadas (as de uma biblioteca): a do nó com o
    /// intervalo exato; senão a primeira cuja declaração de topo o contém.
    fn atribuir_entre(&self, program: &Program, s: Span, unidades: &[UnitId]) -> Option<UnitId> {
        if let Some(v) = self.exato.get(&(s.start, s.end)) {
            if let Some(u) = v.iter().find(|u| unidades.contains(u)) {
                return Some(*u);
            }
        }
        unidades.iter().copied().find(|u| {
            let unit = program.unit(*u);
            unit.unit.declarations.iter().any(|d| {
                let sp = unit.ast.decls[d.0 as usize].span;
                sp.start <= s.start && s.end <= sp.end
            })
        })
    }

    /// A unidade do diagnóstico e se houve mais de uma candidata.
    fn atribuir(&self, s: Span) -> Option<(UnitId, bool)> {
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

    #[test]
    fn conflito_herdado_chega_ao_arquivo_certo_no_motor() {
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/motor-heranca-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        std::fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        std::fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object {} class int extends Object {}").unwrap();
        let entrada = raiz.join("main.dart");
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/conflicting_static_and_instance/ConflictingStaticAndInstanceClass__inSu_7966ede4.dart"));
        std::fs::write(&entrada, fonte).unwrap();
        let motor = Motor::novo(&raiz.join("sdk/lib")).unwrap();
        let resultado = motor.analisar(&raiz, std::slice::from_ref(&entrada), None);
        let arquivo = &resultado.arquivos[&chave(&entrada)];
        let achados: Vec<_> = arquivo.diags.iter().filter(|d| d.code == Some(codigos::compile_time_error::CONFLICTING_STATIC_AND_INSTANCE)).collect();
        assert_eq!(achados.len(), 1, "{:?}", arquivo.diags);
        assert_eq!(&fonte[achados[0].span.start as usize..achados[0].span.end as usize], "foo");
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
