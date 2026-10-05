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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
    /// Os achados das regras de lint que pedem o programa resolvido (a
    /// hierarquia, os tipos): saem só se a regra estiver ligada nas opções.
    pub lints_semanticos: Vec<LintSemantico>,
    /// Os relatos de todas as regras de lint sobre a árvore do programa,
    /// com a semântica do motor (`dartforge_analise::lints::executar_com`):
    /// quem publica filtra pelas regras ligadas. `None` quando o arquivo
    /// não é unidade do programa resolvido.
    pub relatos_de_lint: Option<Vec<dartforge_analise::lints::regras::RelatoDeLint>>,
}

/// Um achado de regra de lint calculado com o programa resolvido.
#[derive(Debug, Clone)]
pub struct LintSemantico {
    /// O nome único do código (`LinterLintCode`), que é também a chave em
    /// `dartforge_analise::lints::codigos_g::TODOS`.
    pub unico: &'static str,
    pub span: dartforge_diagnostics::Span,
    pub args: Vec<String>,
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

/// O arquivo `alvo` existe como saída do `build_runner`
/// (`.dart_tool/build/generated/<pacote>/<rel>`, de `lib/` ou de qualquer
/// outra pasta do pacote)? O analyzer os enxerga.
fn gerado_pelo_build(c: &dartforge_elements::PackageConfig, alvo: &Path) -> bool {
    c.gerado_no_lugar_de(alvo).is_some()
        || c.package_dirs.iter().any(|(nome, dir)| {
            alvo.strip_prefix(dir)
                .ok()
                .and_then(|rel| c.generated_path(nome, &rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/")))
                .is_some()
        })
}

impl Motor {
    pub fn novo(sdk_lib: &Path) -> Result<Motor, String> {
        let mut sdk = SdkLayout::load(sdk_lib, "dartdevc")?;
        // O `dart analyze` não usa perfil de compilação: enxerga toda
        // biblioteca pública da plataforma, pela fonte e sem patches. As que
        // o perfil do DDC não tem (`dart:ffi`, `dart:mirrors`, `dart:cli`,
        // `dart:nativewrappers`) vêm das seções da VM, sem os patches dela;
        // sem isso, `import 'dart:ffi'` não resolvia e cada `Pointer`,
        // `Struct`… virava `undefined_class`.
        for secao in ["vm_common", "vm"] {
            if let Ok(vm) = SdkLayout::load(sdk_lib, secao) {
                for (nome, mut lib) in vm.libraries {
                    if !nome.starts_with('_') && !sdk.libraries.contains_key(&nome) {
                        lib.patches.clear();
                        sdk.libraries.insert(nome, lib);
                    }
                }
            }
        }
        // Importação condicional: o analyzer escolhe a configuração cujo
        // `dart.library.x` está nas variáveis declaradas do contexto
        // (`an611:src/dart/analysis/file_state.dart:785-800`), e o `dart
        // analyze` não declara nenhuma — vale sempre a URI principal. Sem
        // biblioteca "suportada", a carga também fica com a principal
        // (`import 'fake.dart' if (dart.library.js_interop) 'real.dart'`
        // resolve para `fake.dart`, como no analyzer). Exceção conhecida: uma
        // condição `== 'false'` seria escolhida aqui e não no analyzer.
        for lib in sdk.libraries.values_mut() {
            lib.supported = false;
        }
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
        self.analisar_com(raiz, arquivos, packages, &HashMap::new(), &|| false)
            .expect("sem cancelamento, a análise sempre conclui")
    }

    /// Como [`Motor::analisar`], com textos em memória e cancelamento.
    ///
    /// `textos` (pela [`chave`] do caminho) vale mais que o disco, tanto
    /// para os `arquivos` quanto para qualquer arquivo que a carga alcance:
    /// é o que o LSP passa com os documentos abertos ainda não salvos.
    /// `cancelado` é consultado entre as fases (carga, verificadores, cada
    /// biblioteca na inferência de corpos); quando responde verdadeiro, a
    /// análise para ali e devolve `None` — o LSP a descarta porque o texto
    /// mudou. Nada é retido pelo motor entre chamadas.
    pub fn analisar_com(
        &self,
        raiz: &Path,
        arquivos: &[PathBuf],
        packages: Option<&Path>,
        textos: &HashMap<PathBuf, String>,
        cancelado: &dyn Fn() -> bool,
    ) -> Option<Analise> {
        let mut analise = Analise::default();
        let mut proprios: BTreeMap<PathBuf, String> = BTreeMap::new();
        let mut imports = String::new();
        for (i, a) in arquivos.iter().enumerate() {
            let texto = match textos.get(&chave(a)) {
                Some(t) => t.clone(),
                None => {
                    let Ok(t) = std::fs::read_to_string(a) else { continue };
                    t
                }
            };
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
            analise.arquivos.insert(p.clone(), Arquivo { texto: t.clone(), diags: Vec::new(), sintaticos: 0, lints_semanticos: Vec::new(), relatos_de_lint: None });
        }
        if imports.is_empty() {
            return Some(analise);
        }
        let entrada = raiz.join(ENTRADA);
        let mut c = dartforge_elements::gerado::Construtor::nova();
        for (caminho, texto) in textos {
            c.por(caminho.clone(), texto.clone(), "lsp", vec![]);
        }
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
        if cancelado() {
            return None;
        }

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
            // `_resolveLibraryDocImportDirective`: os `@docImport` do doc da
            // diretiva `library` com alvo ausente (`URI_DOES_NOT_EXIST_IN_DOC_IMPORT`).
            let arvore = dartforge_analise::Unidade { ast: &unit.ast, unit: &unit.unit, fonte: &unit.source };
            for i in dartforge_analise::a_doc::imports_de_doc_da_biblioteca(arvore) {
                let Some(uri) = i.uri else { continue };
                // `_reportImportDirectiveErrors`: `dart-ext:` no lugar do alvo ausente.
                if uri.starts_with("dart-ext:") {
                    let d = Diagnostic::com_codigo(codigos::compile_time_error::USE_OF_NATIVE_EXTENSION, i.literal, [] as [&str; 0]);
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                    continue;
                }
                if textos.contains_key(&chave(&base.join(&uri))) {
                    continue;
                }
                if self.diretiva_sem_alvo(&uri, &base, config.as_ref(), i.literal, false).is_some() {
                    let d = Diagnostic::com_codigo(codigos::warning::URI_DOES_NOT_EXIST_IN_DOC_IMPORT, i.literal, [uri.as_str()]);
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                }
            }
            for dir in &unit.unit.directives {
                let (lit, parte) = match &dir.kind {
                    DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => (uri, false),
                    DirectiveKind::Part { uri } => (uri, true),
                    _ => continue,
                };
                let Some(texto) = dartforge_elements::load::string_lit_value(lit) else { continue };
                // `USE_OF_NATIVE_EXTENSION` (`library_analyzer.dart:672-684`, `:906-916`):
                // import ou export de `dart-ext:`, no lugar do alvo ausente.
                if !parte && texto.starts_with("dart-ext:") {
                    let d = Diagnostic::com_codigo(codigos::compile_time_error::USE_OF_NATIVE_EXTENSION, lit.span, [] as [&str; 0]);
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                    continue;
                }
                if textos.contains_key(&chave(&base.join(&texto))) {
                    // Documento aberto ainda não salvo: existe para o editor.
                    continue;
                }
                if let Some(d) = self.diretiva_sem_alvo(&texto, &base, config.as_ref(), lit.span, parte) {
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                } else if parte && let Some(uri_do_alvo) = self.parte_sem_part_of(&texto, &base, config.as_ref(), &program, &textos) {
                    // `_resolvePartDirective` (`library_analyzer.dart:971-1032`):
                    // o arquivo existe e não é parte.
                    let d = Diagnostic::com_codigo(codigos::compile_time_error::PART_OF_NON_PART, lit.span, [uri_do_alvo.as_str()]);
                    analise.arquivos.get_mut(k).expect("próprio").diags.push(d);
                }
            }
        }

        // Bibliotecas com erro de sintaxe: o verificador de elementos não
        // usados não roda nelas.
        let mut libs_com_erro_de_sintaxe: BTreeSet<LibraryId> = BTreeSet::new();
        // Bibliotecas sem erro de sintaxe: o `UnusedLocalElementsVerifier`
        // roda pelo elemento depois da inferência
        // (`dartforge_types::fase_nao_usados`); o relato pelo nome fica
        // guardado para quando não houver corpos.
        let mut privados_adiados: HashMap<LibraryId, Vec<(UnitId, dartforge_diagnostics::Diagnostic)>> = HashMap::new();
        // T5 (docs/ANALYZER-ESPECIFICACAO.md §G): com
        // `DARTFORGE_PORTAS_DE_SINTAXE=pulados`, as portas por erro de
        // sintaxe (biblioteca inteira, declaração executável, arquivo) dão
        // lugar à regra dos trechos que o parser pulou. O padrão continua o
        // das portas: a troca só pode virar padrão depois de medida no
        // placar (nenhum FP novo em código publicado).
        let por_pulados = std::env::var("DARTFORGE_PORTAS_DE_SINTAXE").is_ok_and(|v| v == "pulados");
        // O outline sai antes das verificações de declaração: a porta das
        // cláusulas (`analise::clausulas`) usa as decisões dos mixins, que
        // precisam de tipos (`dartforge_types::fase_mixins`).
        let mut table = dartforge_types::TypeTable::new();
        // Mensagens com o alias de `typedef` e o `Never?` escritos (C9).
        table.preservar_exibicao = true;
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, diags_outline, unidades_outline) =
            dartforge_types::resolve_outline_com_unidades(&program, &interner, &mut table, &core);
        /// Limpa as decisões dos mixins ao sair (por qualquer caminho).
        struct LimparDecisoes;
        impl Drop for LimparDecisoes {
            fn drop(&mut self) {
                dartforge_analise::clausulas::definir_decisoes_de_mixins(None);
            }
        }
        let _limpar_decisoes = LimparDecisoes;
        dartforge_analise::clausulas::definir_decisoes_de_mixins(Some(dartforge_types::fase_mixins::decisoes(
            &program,
            &interner,
            &mut table,
            &core,
            &outline,
            &libs_proprias,
        )));
        // `computeSimplyBounded`, uma vez para o programa.
        let limites_simples = dartforge_analise::limites_simples::calcular(&program);
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
            // O 6.11 junta declarações de mesmo nome; o 3.13.4 (referência
            // de uma biblioteca com sintaxe posterior ao 3.6) não.
            // A referência da biblioteca é a maior das unidades dela (T2):
            // o parser marca a unidade que usa sintaxe que o 3.6.2 não
            // conhece, e a versão acima da 3.6 marca todas.
            let sintaxe_nova = program.referencia_da_biblioteca(*lib) == dartforge_diagnostics::Referencia::V3_13;
            let mut achados = dartforge_analise::duplicatas::duplicatas(&unidades, &interner, curinga, !sintaxe_nova);
            achados.extend(dartforge_analise::enums::sem_constantes(&unidades));
            achados.extend(dartforge_analise::inicializacao::finais_nao_inicializados(&unidades, &interner));
            achados.extend(dartforge_analise::construtores::verificar(&unidades, &interner));
            for (i, u) in unidades.iter().enumerate() {
                // Os erros de sintaxe da unidade (a fase 1 já os pôs no arquivo).
                let sintaticos: Vec<Span> = program
                    .unit(ids[i])
                    .path
                    .as_ref()
                    .and_then(|p| analise.arquivos.get(&chave(p)))
                    .map(|a| a.diags[..a.sintaticos].iter().filter(|d| recuperacao_do_parser(d, &a.texto)).map(|d| d.span).collect())
                    .unwrap_or_default();
                if por_pulados {
                    let pulados = dartforge_analise::Pulados { fonte: u.fonte, trechos: &program.unit(ids[i]).pulados };
                    achados.extend(
                        dartforge_analise::locais::nao_usados_com_pulados(*u, &interner, curinga, pulados).into_iter().map(|d| (i, d)),
                    );
                } else {
                    achados.extend(
                        dartforge_analise::locais::nao_usados(*u, &interner, curinga, &sintaticos).into_iter().map(|d| (i, d)),
                    );
                }
                achados.extend(dartforge_analise::externos::inicializadores(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::a_contexto::verificar(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::a_doc::verificar(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::c2_sintaticos::verificar(*u, &interner).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::fases::texto_bidirecional(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::todos::verificar(u.fonte).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::versao_de_linguagem::verificar(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::registros::verificar(u, &interner, biblioteca.features).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::inicializacao::constantes_nao_inicializadas(u, &interner).into_iter().map(|d| (i, d)));
                achados.extend(
                    dartforge_analise::fases::dois_pontos_no_padrao(*u, biblioteca.features.versao().major < 3).into_iter().map(|d| (i, d)),
                );
                achados.extend(dartforge_analise::fases::embutido_como_tipo(*u, &interner).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::nativos::fora_do_sdk(*u).into_iter().map(|d| (i, d)));
                achados.extend(dartforge_analise::operadores::aridade(*u, &interner).into_iter().map(|d| (i, d)));
            }
            // Privados não usados: pela biblioteca inteira, sem erro de sintaxe.
            let com_erro = ids.iter().any(|u| {
                program.unit(*u).path.as_ref().and_then(|p| analise.arquivos.get(&chave(p))).is_none_or(|a| a.diags[..a.sintaticos].iter().any(|d| recuperacao_do_parser(d, &a.texto)))
            });
            if por_pulados {
                let pulados: Vec<dartforge_analise::Pulados<'_>> = ids
                    .iter()
                    .map(|u| dartforge_analise::Pulados { fonte: &program.unit(*u).source, trechos: &program.unit(*u).pulados })
                    .collect();
                achados.extend(dartforge_analise::privados::nao_usados_com_pulados(&unidades, &interner, &pulados));
            } else {
                if com_erro {
                    libs_com_erro_de_sintaxe.insert(*lib);
                    achados.extend(dartforge_analise::privados::nao_usados(&unidades, &interner, com_erro));
                } else {
                    let adiados = dartforge_analise::privados::nao_usados(&unidades, &interner, com_erro);
                    privados_adiados.insert(*lib, adiados.into_iter().map(|(i, d)| (ids[i], d)).collect());
                }
            }
            for (i, d) in achados {
                if let Some(p) = &program.unit(ids[i]).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
            for (u, d) in dartforge_analise::importacoes::extensoes_adiadas(&program, *lib)
                .into_iter()
                .chain(dartforge_analise::importacoes::diretivas_internas_e_adiadas(&program, *lib))
                .chain(dartforge_analise::importacoes::nomes_mostrados_indefinidos(&program, *lib, &interner))
                .chain(dartforge_analise::importacoes::exports_ambiguos(&program, *lib, &interner))
                .chain(dartforge_analise::importacoes::tipos_adiados(&program, *lib, &interner))
                // FASES NOVAS (INFRA etapa 6): os verificadores de aviso por biblioteca.
                .chain(dartforge_analise::fases::diretivas_repetidas(&program, *lib, &interner))
                // As anotações do `package:meta` (lote II.7). API pública:
                // a biblioteca em `lib/`, fora de `lib/src/`.
                .chain({
                    let publica = program
                        .library(*lib)
                        .units
                        .first()
                        .and_then(|u| program.unit(*u).path.as_deref())
                        .and_then(|p| p.strip_prefix(raiz).ok())
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .is_some_and(|rel| rel.starts_with("lib/") && !rel.starts_with("lib/src/"));
                    dartforge_analise::meta::verificar(&program, *lib, &interner, publica)
                })
            {
                if let Some(p) = &program.unit(u).path {
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
            for (u, d) in dartforge_analise::modificadores::fora_da_biblioteca(&program, *lib, &interner) {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
            for (u, d) in dartforge_analise::clausulas::verificar(&program, *lib, &interner)
                .into_iter()
                .chain(dartforge_analise::membros::verificar(&program, *lib, &interner))
                .chain(dartforge_analise::limites_simples::verificar(&program, *lib, &interner, &limites_simples))
            {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
        }

        if cancelado() {
            return None;
        }

        // 4. Tipos (o outline já foi resolvido antes das cláusulas).
        let indice = Indice::novo(&program, &unidades_proprias);
        // Inicializadores: `types` os infere em toda passada, de qualquer
        // biblioteca. Uma passada sem corpo nenhum (`&[]`) estabiliza os tipos
        // inferidos; a segunda dá só os diagnósticos deles.
        let _ = dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[]);
        let (_, diags_init, unidades_init) = dartforge_types::infer_bodies_das_bibliotecas_com_unidades(
            &program,
            &interner,
            &mut table,
            &core,
            &mut outline,
            &[],
            false,
        );
        let mut atribuidos: Vec<(UnitId, Diagnostic)> = Vec::new();
        // As tabelas laterais das bibliotecas do lote, cada unidade da passada
        // da sua biblioteca (a avaliação de constantes lê todas).
        let mut corpos: Option<dartforge_types::BodyTypes> = None;
        // Corpos: uma passada por biblioteca do lote.
        for lib in &libs_proprias {
            if cancelado() {
                return None;
            }
            let (bt, ds, us) = dartforge_types::infer_bodies_das_bibliotecas_com_unidades(
                &program,
                &interner,
                &mut table,
                &core,
                &mut outline,
                std::slice::from_ref(lib),
                true,
            );
            match &mut corpos {
                None => corpos = Some(bt),
                Some(c) => {
                    let mut bt = bt;
                    for u in &program.library(*lib).units {
                        let i = u.0 as usize;
                        if i < c.units.len() && i < bt.units.len() {
                            c.units[i] = std::mem::take(&mut bt.units[i]);
                        }
                    }
                }
            }
            let inicio = if ds.len() >= diags_init.len() && ds[..diags_init.len()] == diags_init[..] {
                diags_init.len()
            } else {
                // O prefixo mudou: não dá para separar; conta como ambíguo.
                analise.ambiguos += 1;
                0
            };
            let unidades: Vec<UnitId> = program.library(*lib).units.clone();
            for (d, registrada) in ds[inicio..].iter().zip(&us[inicio..]) {
                // A unidade do corpo em inferência, quando é desta
                // biblioteca; senão, pelo intervalo.
                let u = match registrada {
                    Some(u) if unidades.contains(u) => Some(*u),
                    _ if unidades.len() == 1 => Some(unidades[0]),
                    _ => indice.atribuir_entre(&program, d.span, &unidades),
                };
                if let Some(u) = u {
                    atribuidos.push((u, d.clone()));
                }
            }
        }
        // Inicializadores, em ordem, no primeiro inicializador que os contém.
        let inits = inicializadores(&program);
        let mut pos = 0;
        for (d, registrada) in diags_init.iter().zip(&unidades_init) {
            if let Some(u) = registrada {
                atribuidos.push((*u, d.clone()));
                continue;
            }
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
        // Outline: na unidade da anotação resolvida.
        for (d, u) in diags_outline.iter().zip(&unidades_outline) {
            atribuidos.push((*u, d.clone()));
        }
        // Constantes (`ConstantVerifier`): as bibliotecas do lote são as
        // inferidas; as outras (SDK, pacotes) ficam opacas.
        if let Some(corpos) = &corpos {
            let inferidas: HashSet<LibraryId> = libs_proprias.iter().copied().collect();
            atribuidos.extend(dartforge_types::constantes::verificar(
                &program,
                &interner,
                &mut table,
                &core,
                &outline,
                corpos,
                &inferidas,
                &libs_proprias,
            ));
            // `BestPracticesVerifier`: `non_const_call_to_literal_constructor`,
            // com o `canBeConst` pela verificação de constantes.
            atribuidos.extend(dartforge_types::fase_literal::construtores_literais(
                &program,
                &interner,
                &mut table,
                &core,
                &outline,
                corpos,
                &inferidas,
                &libs_proprias,
            ));
        }
        // Sobrescritas inválidas, nas classes em que o `verify()` do
        // `InheritanceOverrideVerifier` chega a conferi-las.
        for lib in &libs_proprias {
            let classes = dartforge_analise::clausulas::verificador_de_heranca_prossegue(&program, *lib, &interner);
            // `verify()`: os conflitos da interface antes dos membros.
            atribuidos.extend(dartforge_types::fase_heranca::inconsistencias(&program, &interner, &mut table, &core, &outline, &classes));
            atribuidos.extend(dartforge_types::sobrescritas::sobrescritas_invalidas(
                &program, &interner, &mut table, &core, &outline, &classes,
            ));
            atribuidos.extend(dartforge_types::sobrescritas::membros_de_enum(
                &program, &interner, &mut table, &core, &outline, &classes,
            ));
            atribuidos.extend(dartforge_types::sobrescritas::membros_abstratos(
                &program, &interner, &mut table, &core, &outline, &classes,
            ));
            atribuidos.extend(dartforge_types::sobrescritas::membros_em_conflito(&program, &interner, &mut table, &core, &outline, *lib));
            atribuidos.extend(dartforge_types::sobrescritas::valores_padrao(&program, &interner, &mut table, &outline, *lib));
            atribuidos.extend(dartforge_types::sobrescritas::variaveis_nao_inicializadas(&program, &interner, &table, &outline, *lib));
            // `BestPracticesVerifier` sobre declarações e tipos escritos.
            atribuidos.extend(dartforge_types::boas_praticas::parametro_de_igualdade_anulavel(&program, &interner, &table, &core, &outline, *lib));
            atribuidos.extend(dartforge_types::boas_praticas::no_such_method_desnecessario(&program, &interner, &core, *lib));
            atribuidos.extend(dartforge_types::boas_praticas::interrogacoes_desnecessarias(&program, &interner, &table, &outline, corpos.as_ref(), *lib));
            atribuidos.extend(dartforge_types::sobrescritas::getters_e_setters(
                &program, &interner, &mut table, &core, &outline, *lib, &classes,
            ));
            atribuidos.extend(dartforge_types::variancia::variancia(&program, &interner, &table, &outline, *lib));
            atribuidos.extend(dartforge_types::variancia::posicoes_nao_covariantes_na_representacao(&program, &table, &outline, *lib));
            atribuidos.extend(dartforge_types::a_main::funcao_main(&program, &interner, &mut table, &core, &outline, *lib));
            // `conflicting_generic_interfaces`, com a porta das cláusulas.
            {
                let aberta = |id: dartforge_elements::model::ClassId| {
                    dartforge_analise::clausulas::porta(&program, *lib, &interner, id) == dartforge_analise::clausulas::Porta::Aberta
                };
                atribuidos.extend(dartforge_types::fase_genericos::conflitos_genericos(&program, &interner, &mut table, &core, &outline, *lib, &aberta));
            }
            // Declarações `extension type`: ciclos, fundo, conflitos e `implements`.
            atribuidos.extend(dartforge_types::tipos_de_extensao::verificar(&program, &interner, &mut table, &core, &outline, *lib));
            // FASES NOVAS (INFRA etapa 6): `OverrideVerifier`.
            atribuidos.extend(dartforge_types::fase_override::sem_sobrescrita(&program, &interner, &mut table, &core, &outline, *lib));
            atribuidos.extend(dartforge_types::fase_override::sem_redeclaracao(&program, &interner, &mut table, &core, &outline, *lib));
            // `BestPracticesVerifier`: `invalid_override_of_non_virtual_member`.
            atribuidos.extend(dartforge_types::fase_override::sobrescritas_de_nao_virtuais(&program, &interner, &mut table, &core, &outline, *lib));
            // O lint `annotate_overrides`, guardado à parte: só sai com a
            // regra ligada.
            for (u, span, nome) in dartforge_types::fase_override::sem_anotacao_de_override(&program, &interner, &mut table, &core, &outline, *lib) {
                if let Some(p) = &program.unit(u).path
                    && let Some(a) = analise.arquivos.get_mut(&chave(p))
                {
                    a.lints_semanticos.push(LintSemantico { unico: "annotate_overrides", span, args: vec![nome] });
                }
            }
            // `MustCallSuperVerifier`.
            atribuidos.extend(dartforge_types::fase_super::sem_chamada_ao_super(&program, &interner, *lib));
            if let Some(corpos) = &corpos
                && !libs_com_erro_de_sintaxe.contains(lib)
            {
                // Roda também nas bibliotecas julgadas pelo 3.13.4: lá o
                // código sai como `unused_element_parameter`, pela variante
                // (`Diagnostic::na_referencia`, no laço final).
                atribuidos.extend(dartforge_types::parametros::parametros_nao_usados(&program, &interner, &outline, corpos, *lib));
            }
            // `UnusedLocalElementsVerifier` (declarações de biblioteca).
            if let Some(adiados) = privados_adiados.remove(lib) {
                match &corpos {
                    Some(corpos) => {
                        let inferidas: HashSet<LibraryId> = libs_proprias.iter().copied().collect();
                        atribuidos.extend(dartforge_types::fase_nao_usados::elementos_nao_usados(
                            &program, &interner, &mut table, &core, &outline, corpos, &inferidas, *lib,
                        ));
                    }
                    None => atribuidos.extend(adiados),
                }
            }
        }
        // `SdkConstraintVerifier`: só com `environment: sdk:` legível no
        // `pubspec.yaml` da raiz.
        let restricao_de_sdk = std::fs::read_to_string(raiz.join("pubspec.yaml"))
            .ok()
            .and_then(|t| yaml_rust2::YamlLoader::load_from_str(&t).ok())
            .and_then(|docs| docs.into_iter().next())
            .and_then(|y| y["environment"]["sdk"].as_str().and_then(dartforge_types::fase_sdk::RestricaoDeSdk::de_texto));
        // Argumentos de tipo fora dos limites, unidade a unidade.
        // As bibliotecas inferidas, para o motor de constantes dos lints.
        let inferidas_dos_lints: HashSet<LibraryId> = libs_proprias.iter().copied().collect();
        for &u in &unidades_proprias {
            for d in dartforge_types::limites::argumentos_fora_dos_limites(&program, &interner, &mut table, &core, &outline, u) {
                atribuidos.push((u, d));
            }
            // `DeprecatedMemberUseVerifier`: "mesmo pacote" é a biblioteca
            // cujo arquivo fica dentro da raiz analisada.
            let mesmo_pacote = |l: dartforge_elements::model::LibraryId| {
                program.library(l).units.first().and_then(|x| program.unit(*x).path.as_deref()).is_some_and(|p| p.starts_with(raiz))
            };
            let corpo_da_unidade = corpos.as_ref().and_then(|c| c.units.get(u.0 as usize));
            for d in dartforge_types::fase_deprecado::usos_de_deprecados(&program, &interner, corpo_da_unidade, u, &mesmo_pacote) {
                atribuidos.push((u, d));
            }
            // `_InvalidAccessVerifier` e as classes `@sealed`.
            for d in dartforge_types::fase_acesso::acessos_invalidos(&program, &interner, &outline, corpo_da_unidade, u, &mesmo_pacote) {
                atribuidos.push((u, d));
            }
            if let Some(restricao) = &restricao_de_sdk {
                for d in dartforge_types::fase_sdk::restricao_de_sdk(&program, &interner, corpo_da_unidade, u, restricao) {
                    atribuidos.push((u, d));
                }
            }
            if let Some(todos) = corpos.as_ref()
                && let Some(corpo) = todos.units.get(u.0 as usize)
            {
                // As regras de lint tipadas, guardadas à parte: só saem
                // com a regra ligada.
                let mut de_lint = dartforge_types::lints_tipados::achados(&program, &table, &core, &outline, corpo, u);
                de_lint.extend(dartforge_types::lints_tipados2::achados(&program, &interner, &mut table, &core, &outline, todos, &inferidas_dos_lints, u));
                de_lint.extend(dartforge_types::lints_tipados3::achados(&program, &interner, &mut table, &core, &outline, corpo, u));
                if !de_lint.is_empty()
                    && let Some(p) = &program.unit(u).path
                    && let Some(a) = analise.arquivos.get_mut(&chave(p))
                {
                    a.lints_semanticos.extend(de_lint.into_iter().map(|(span, unico, args)| LintSemantico { unico, span, args }));
                }
                // As regras de lint da árvore, sobre a árvore do programa e
                // com a semântica do motor.
                {
                    let un = program.unit(u);
                    let sem = dartforge_analise::lints::Semantica { program: &program, unidade: u, corpo, corpos: todos, table: &table, core: &core, outline: &outline };
                    let arvore = dartforge_analise::Unidade { ast: &un.ast, unit: &un.unit, fonte: &un.source };
                    let relatos = dartforge_analise::lints::executar_com(arvore, &interner, &|_| true, Some(&sem));
                    if let Some(p) = &un.path
                        && let Some(a) = analise.arquivos.get_mut(&chave(p))
                    {
                        a.relatos_de_lint = Some(relatos);
                    }
                }
                // `RequiredParametersVerifier`: o `@required` do `package:meta`.
                for d in dartforge_types::fase_requeridos::requeridos_ausentes(&program, &interner, corpo, u) {
                    atribuidos.push((u, d));
                }
                // `ErrorHandlerVerifier`: o retorno do `onError` de `catchError`.
                for d in dartforge_types::fase_catch_error::retornos_de_catch_error(&program, &interner, &mut table, &core, &outline, corpo, u) {
                    atribuidos.push((u, d));
                }
                // `FfiVerifier`.
                for d in dartforge_types::fase_ffi::verificar(&program, &interner, &mut table, &core, &outline, todos, &inferidas_dos_lints, u) {
                    atribuidos.push((u, d));
                }
                // `BestPracticesVerifier`: `assignment_of_do_not_store` e `return_of_do_not_store`.
                for d in dartforge_types::fase_nao_guardar::guardados_e_devolvidos(&program, &interner, corpo, u) {
                    atribuidos.push((u, d));
                }
                // `BestPracticesVerifier` com `strict-inference` (filtrado sem a opção).
                for d in dartforge_types::fase_estrita::falhas_de_inferencia(&program, &interner, &mut table, &core, &outline, corpo, u) {
                    atribuidos.push((u, d));
                }
                // `UseResultVerifier`.
                for d in dartforge_types::fase_resultado::resultados_nao_usados(&program, &interner, corpo, u) {
                    atribuidos.push((u, d));
                }
                for d in dartforge_types::limites::argumentos_inferidos_fora_dos_limites(&program, &interner, &mut table, &core, &outline, corpo, u) {
                    atribuidos.push((u, d));
                }
            }
        }
        // Nos tipos de `extends`/`implements`/`with` o analyzer não relata
        // nome indefinido nem nome que não é tipo: o erro é o `*_non_class`
        // de `analise::clausulas`.
        let clausulas: BTreeSet<(UnitId, usize)> = libs_proprias
            .iter()
            .flat_map(|lib| dartforge_analise::clausulas::nomes_de_clausulas(&program, *lib))
            .collect();
        let mut vistos: BTreeSet<(UnitId, Option<dartforge_diagnostics::Codigo>, usize, usize, String)> = BTreeSet::new();
        for (unidade, d) in &atribuidos {
            let unidade = *unidade;
            if !unidades_proprias.contains(&unidade) {
                continue;
            }
            let fonte = &program.unit(unidade).source;
            let trecho = fonte.get(d.span.start..d.span.end).unwrap_or("");
            // Os códigos de constantes vêm de `types::constantes`; o que a
            // inferência emite deles (sem código) fica de fora.
            if d.code.is_none() {
                let cod = ponte::codificar_tipos(d, trecho);
                if cod.code.is_some_and(|c| dartforge_types::constantes::CODIGOS.contains(&c.info().nome)) {
                    continue;
                }
            }
            let cod = ponte::codificar_tipos(d, trecho);
            // A validade de uma anotação depende do SDK (`@Deprecated.optional()`
            // só existe depois do 3.6.2): numa biblioteca julgada pelo 3.13.4,
            // o nosso SDK 3.6.2 não decide.
            if cod.code.is_some_and(|c| matches!(c.info().nome, "invalid_annotation" | "undefined_annotation"))
                && program.referencia_da_biblioteca(program.unit(unidade).library) == dartforge_diagnostics::Referencia::V3_13
            {
                continue;
            }
            if cod.code.is_some_and(|c| matches!(c.info().nome, "undefined_class" | "not_a_type"))
                && clausulas.contains(&(unidade, cod.span.start))
            {
                continue;
            }
            if !vistos.insert((unidade, cod.code, cod.span.start, cod.span.end, cod.message.clone())) {
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
            for (u, d) in dartforge_analise::importacoes::nao_usados(&program, *lib, &interner, &ja, corpos.as_ref()) {
                if let Some(p) = &program.unit(u).path {
                    if let Some(a) = analise.arquivos.get_mut(&chave(p)) {
                        a.diags.push(d);
                    }
                }
            }
        }

        // 6. Num arquivo com erro de sintaxe, a recuperação do nosso parser
        // pode perder declarações que a do fasta mantém (`class E<inout T>`
        // sem o recurso, `native`…): um nome que não resolve ali não é
        // prova de nome indefinido. Os códigos de resolução de nome saem só
        // de arquivos sem erro de sintaxe (depois dos imports, que usam
        // esses diagnósticos para a supressão deles).
        // Os códigos que comparam declarações entre si (tipos de getter e
        // setter) saem de uma linha em que o parser se recuperou: a
        // declaração ali não é a do analyzer.
        let fontes: HashMap<PathBuf, &str> = program
            .units
            .iter()
            .filter_map(|u| u.path.as_ref().map(|p| (chave(p), u.source.as_str())))
            .collect();
        // A regra dos trechos pulados (T5, estrutura (c)): "não definido" de
        // um nome citado num trecho pulado da biblioteca; e os dois códigos
        // de forma de declaração, quando o intervalo toca um trecho pulado.
        if por_pulados {
            let mut da_biblioteca: HashMap<PathBuf, Vec<UnitId>> = HashMap::new();
            for lib in &libs_proprias {
                let unidades: Vec<UnitId> = program.library(*lib).units.clone();
                for u in &unidades {
                    if let Some(p) = &program.unit(*u).path {
                        da_biblioteca.insert(chave(p), unidades.clone());
                    }
                }
            }
            for (k, a) in analise.arquivos.iter_mut() {
                let Some(unidades) = da_biblioteca.get(k) else { continue };
                let Some(&propria) = unidade_de.get(k) else { continue };
                let fonte = program.unit(propria).source.as_str();
                let proprios_pulados = dartforge_analise::Pulados { fonte, trechos: &program.unit(propria).pulados };
                let n = a.sintaticos;
                let mut i = 0;
                a.diags.retain(|d| {
                    i += 1;
                    if i <= n {
                        return true;
                    }
                    let Some(c) = d.code.map(|c| c.info().nome) else { return true };
                    if depende_de_declaracoes(c) {
                        let nome = fonte.get(d.span.start..d.span.end).unwrap_or("");
                        return !unidades.iter().any(|u| {
                            dartforge_analise::Pulados { fonte: &program.unit(*u).source, trechos: &program.unit(*u).pulados }.cita(nome)
                        });
                    }
                    if depende_da_linha(c) {
                        return !proprios_pulados.intersecta(d.span);
                    }
                    true
                });
            }
        }
        for (k, a) in analise.arquivos.iter_mut() {
            if !por_pulados && a.diags[..a.sintaticos].iter().any(|d| recuperacao_do_parser(d, &a.texto)) {
                let n = a.sintaticos;
                let erros: Vec<usize> =
                    a.diags[..n].iter().filter(|d| recuperacao_do_parser(d, &a.texto)).map(|d| d.span.start).collect();
                let fonte = fontes.get(k).copied().unwrap_or("");
                let linha = |pos: usize| fonte.get(..pos).map_or(0, |t| t.matches('\n').count());
                let mut i = 0;
                a.diags.retain(|d| {
                    i += 1;
                    let Some(c) = d.code.map(|c| c.info().nome) else { return true };
                    i <= n
                        || !(depende_de_declaracoes(c)
                            || depende_da_linha(c) && erros.iter().any(|&e| linha(e) == linha(d.span.start)))
                });
            }
        }
        // O analyzer não relata duas vezes o mesmo diagnóstico (código,
        // intervalo e mensagem) — o `_checkDuplicateIdentifier` de uma
        // variável mutável repetida pede o mesmo erro pelo getter e pelo
        // setter, e o resultado tem um só.
        for (k, a) in analise.arquivos.iter_mut() {
            // O ponto único do T2: cada diagnóstico sai com o nome, o texto
            // e os argumentos do analyzer que é a referência do arquivo.
            // Nenhum emissor consulta a referência; eles só passam, depois
            // dos argumentos do molde 3.6, os que só o 3.13 usa.
            let referencia = unidade_de.get(k).map_or(dartforge_diagnostics::Referencia::V3_6, |u| program.referencia(*u));
            if referencia == dartforge_diagnostics::Referencia::V3_13 {
                let n = a.sintaticos;
                let mut sintaticos_fora = 0;
                let antes = std::mem::take(&mut a.diags);
                for (i, d) in antes.into_iter().enumerate() {
                    match d.na_referencia(referencia) {
                        Some(d) => a.diags.push(d),
                        None if i < n => sintaticos_fora += 1,
                        None => {}
                    }
                }
                a.sintaticos -= sintaticos_fora;
            }
            let mut vistos: BTreeSet<(Option<dartforge_diagnostics::Codigo>, usize, usize, String)> = BTreeSet::new();
            let n = a.sintaticos;
            let mut i = 0;
            let mut removidos_sintaticos = 0;
            a.diags.retain(|d| {
                i += 1;
                let novo = vistos.insert((d.code, d.span.start, d.span.end, d.message.clone()));
                if !novo && i <= n {
                    removidos_sintaticos += 1;
                }
                novo
            });
            a.sintaticos -= removidos_sintaticos;
        }
        Some(analise)
    }

    /// O alvo de uma `part` existe e não tem `part of` (o `kind` dele não é
    /// `PartFileKind`): a `uriStr` dele (a da unidade carregada, senão
    /// `dart:`/`package:` como escrita, senão `file:///…`).
    fn parte_sem_part_of(
        &self,
        uri: &str,
        base: &Path,
        config: Option<&dartforge_elements::PackageConfig>,
        program: &dartforge_elements::model::Program,
        textos: &HashMap<std::path::PathBuf, String>,
    ) -> Option<String> {
        let caminho = if let Some(nome) = uri.strip_prefix("dart:") {
            match nome.split_once('/') {
                Some((lib, resto)) => self.sdk.libraries.get(lib)?.path.parent()?.join(resto),
                None => self.sdk.libraries.get(nome)?.path.clone(),
            }
        } else if uri.starts_with("package:") {
            config?.resolve_package_uri(uri).ok()?
        } else if uri.contains(':') {
            return None;
        } else {
            base.join(uri)
        };
        let k = chave(&caminho);
        let carregada = program.units.iter().find(|u| u.path.as_deref().map(chave).as_ref() == Some(&k));
        let tem_part_of = |diretivas: &[dartforge_frontend::ast::Directive]| diretivas.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }));
        let parte = match (textos.get(&k), carregada) {
            (Some(texto), _) => {
                let mut nomes = dartforge_intern::Interner::new();
                tem_part_of(&dartforge_frontend::parser::parse(texto, &mut nomes).unit.directives)
            }
            (None, Some(u)) => tem_part_of(&u.unit.directives),
            (None, None) => {
                let texto = std::fs::read_to_string(&caminho).ok()?;
                let mut nomes = dartforge_intern::Interner::new();
                tem_part_of(&dartforge_frontend::parser::parse(&texto, &mut nomes).unit.directives)
            }
        };
        if parte {
            return None;
        }
        if let Some(u) = carregada
            && !u.uri.is_empty()
        {
            return Some(u.uri.clone());
        }
        if uri.contains(':') {
            return Some(uri.to_string());
        }
        let abs = k.to_string_lossy().replace('\\', "/");
        Some(format!("file://{}{abs}", if abs.starts_with('/') { "" } else { "/" }))
    }

    /// `URI_DOES_NOT_EXIST` / `URI_HAS_NOT_BEEN_GENERATED`
    /// (`library_analyzer.dart:670-720`): o alvo da diretiva não existe.
    fn diretiva_sem_alvo(
        &self,
        uri: &str,
        base: &Path,
        config: Option<&dartforge_elements::PackageConfig>,
        span: Span,
        parte: bool,
    ) -> Option<Diagnostic> {
        if uri.is_empty() {
            // `import ''` resolve para a própria biblioteca, que existe.
            return None;
        }
        let existe = if let Some(nome) = uri.strip_prefix("dart:") {
            match nome.split_once('/') {
                // `dart:async/future.dart`: arquivo ao lado da biblioteca `dart:async`.
                Some((lib, resto)) => self
                    .sdk
                    .libraries
                    .get(lib)
                    .and_then(|l| l.path.parent())
                    .is_some_and(|d| d.join(resto).is_file()),
                None => self.bibliotecas_sdk.contains(nome),
            }
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
        // Numa `part` com URI relativa, o analyzer mostra a URI resolvida
        // (`file:///…/a.dart`); em `import`/`export`, o texto da diretiva.
        if parte && !uri.contains(':') {
            let abs = chave(&base.join(uri)).to_string_lossy().replace('\\', "/");
            let uri_arquivo = format!("file://{}{abs}", if abs.starts_with('/') { "" } else { "/" });
            return Some(Diagnostic::com_codigo(codigo, span, [uri_arquivo.as_str()]));
        }
        Some(Diagnostic::com_codigo(codigo, span, [uri]))
    }
}

/// Erro de sintaxe do qual o parser se recuperou descartando ou remontando
/// trechos (`fonte` é o texto do arquivo). Não são: `experiment_not_enabled`
/// e `experiment_not_enabled_off_by_default` (a sintaxe do recurso desligado
/// foi lida inteira, a árvore é a mesma do recurso ligado) e
/// `missing_function_body` num `;` (o corpo vazio fica na árvore, como no
/// fasta), `unexpected_separator_in_number` (o literal é lido inteiro) e o
/// `;` que falta diante do começo de outra declaração ou comando (o
/// `ensureSemicolon` do fasta o insere, e `Parser::garantir_ponto_e_virgula`
/// também: a árvore segue a do analyzer).
pub fn recuperacao_do_parser(d: &Diagnostic, fonte: &str) -> bool {
    match d.code.map(|c| c.info().nome) {
        Some("experiment_not_enabled" | "experiment_not_enabled_off_by_default" | "unexpected_separator_in_number") => false,
        // Erros que o fasta relata sem descartar nem inventar código (o
        // nome, o modificador e a expressão continuam na árvore).
        Some(
            "async_keyword_used_as_identifier"
            | "extraneous_modifier"
            | "extraneous_modifier_in_extension_type"
            | "extraneous_modifier_in_primary_constructor"
            | "modifier_out_of_order"
            | "duplicated_modifier"
            | "missing_assignable_selector"
            | "illegal_assignment_to_non_assignable"
            | "equality_cannot_be_equality_operand"
            | "invalid_operator_questionmark_period_for_super"
            | "var_return_type"
            | "extension_declares_abstract_member"
            | "extension_declares_constructor"
            | "extension_declares_instance_field"
            | "mixin_declares_constructor"
            | "member_with_class_name"
            | "const_class"
            | "static_constructor"
            | "static_operator"
            | "getter_with_parameters"
            | "covariant_member"
            | "invalid_use_of_covariant_in_extension"
            | "pattern_assignment_declares_variable"
            | "variable_pattern_keyword_in_declaration_context"
            | "illegal_pattern_variable_name"
            | "illegal_pattern_assignment_variable_name"
            | "illegal_pattern_identifier_name"
            | "switch_has_case_after_default_case"
            | "switch_has_multiple_default_cases",
        ) => false,
        Some("missing_function_body") => fonte.get(d.span.start..d.span.end) != Some(";"),
        Some("expected_token") if d.args.first().is_some_and(|a| &**a == ";") => !ponto_e_virgula_inserido(fonte, d.span.end),
        _ => true,
    }
}

/// O token depois de `fim` é o que `garantir_ponto_e_virgula` aceita para
/// inserir o `;` que falta: identificador ou palavra-chave, `}`, `@` ou o fim.
fn ponto_e_virgula_inserido(fonte: &str, fim: usize) -> bool {
    let b = fonte.as_bytes();
    let mut i = fim.min(b.len());
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut prof = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            break;
        }
    }
    match b.get(i) {
        None => true,
        Some(&c) => c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c == b'}' || c == b'@',
    }
}

/// Códigos de nome que não resolve numa **expressão**: dependem de todas as
/// declarações da biblioteca terem sido recuperadas pelo parser, e a nossa
/// recuperação de comandos e expressões ainda diverge da do fasta em casos
/// medidos (FP de `undefined_identifier`/`undefined_getter` sem esta porta).
/// Os nomes de **tipo** (`undefined_class`, `not_a_type` e os do
/// `NamedTypeResolver`) não passam por aqui: a recuperação de declarações é
/// a do fasta (docs/ANALISADOR-PARIDADE-PLANO.md §3.2), e o analyzer resolve
/// a árvore recuperada como qualquer outra.
fn depende_de_declaracoes(codigo: &str) -> bool {
    matches!(
        codigo,
        "undefined_identifier"
            | "undefined_function"
            | "undefined_method"
            | "undefined_getter"
            | "undefined_setter"
            | "undefined_operator"
    )
}

/// Códigos que dependem da forma exata de uma declaração (tipos de getter e
/// setter; variável sem inicializador, que a recuperação pode inventar a
/// partir de um getter com nome inválido): não saem de uma linha
/// em que o parser se recuperou.
fn depende_da_linha(codigo: &str) -> bool {
    matches!(codigo, "getter_not_subtype_setter_types" | "not_initialized_non_nullable_variable")
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
    fn textos_em_memoria_e_cancelamento() {
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/motor-memoria-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        std::fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        std::fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object {} class bool {} class num {} class int extends num {} class Null {}").unwrap();
        let a = raiz.join("a.dart");
        let b = raiz.join("b.dart");
        // No disco, tudo certo; em memória, `b.dart` muda o tipo de `x`.
        std::fs::write(&a, "import 'b.dart';\nvoid f() { if (x) {} }\n").unwrap();
        std::fs::write(&b, "bool x = true;\n").unwrap();
        let motor = Motor::novo(&raiz.join("sdk/lib")).unwrap();
        let nao_bool = |r: &Analise| {
            r.arquivos[&chave(&a)].diags.iter().filter(|d| d.code == Some(codigos::compile_time_error::NON_BOOL_CONDITION)).count()
        };
        let disco = motor.analisar(&raiz, std::slice::from_ref(&a), None);
        assert_eq!(nao_bool(&disco), 0);
        let textos = HashMap::from([(chave(&b), "int x = 0;\n".to_string())]);
        let memoria = motor.analisar_com(&raiz, std::slice::from_ref(&a), None, &textos, &|| false).unwrap();
        assert_eq!(nao_bool(&memoria), 1, "{:?}", memoria.arquivos[&chave(&a)].diags);
        assert!(motor.analisar_com(&raiz, std::slice::from_ref(&a), None, &textos, &|| true).is_none());
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::caminho(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&raiz).unwrap();
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
