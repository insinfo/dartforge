//! O SDK compilado da fonte, por biblioteca (P5, docs/NATIVO-PLANO.md §7).
//!
//! O `dart:core` do nativo vem da fonte do SDK 3.6.2 — a seção `vm` do
//! `libraries.json` com os patches dela — e de uma sobreposição fina nossa,
//! `sdk_nativo/` na raiz do repositório, que troca só os arquivos que dependem
//! da máquina interna da VM (`_SuspendState`, `Timer`/microtarefas ligados ao
//! `dart:isolate`, o `Finalizer` do GC fraco). A troca é feita no
//! carregamento (`SdkLayout::load_com_sobreposicao`, `crates/elements`).
//!
//! Este módulo reúne o que é do SDK como unidade de compilação: onde está a
//! sobreposição, quais bibliotecas vêm da fonte, e a medição de 5a — os
//! diagnósticos de inferência nos corpos dessas bibliotecas, que precisam ser
//! zero antes de o lowering compilá-los.

use dartforge_elements::sdk::SdkLayout;
use std::path::{Path, PathBuf};

/// O alvo da sobreposição em `sdk_nativo/libraries.json`.
pub const ALVO_SOBREPOSICAO: &str = "dartforge_nativo";

/// As bibliotecas que o nativo compila da fonte do SDK (decisão 1 de
/// NATIVO-PLANO §7.1), na ordem de dependência aproximada. `typed_data`
/// entra depois (P9).
pub const BIBLIOTECAS_DA_FONTE: &[&str] = &["_internal", "core", "_compact_hash", "collection", "math", "convert", "async"];

/// Diretório da sobreposição: `DARTFORGE_SDK_NATIVO`, senão o `sdk_nativo/`
/// do repositório que compilou este binário.
pub fn dir_sobreposicao() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_SDK_NATIVO") {
        return PathBuf::from(d);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk_nativo")
}

/// O SDK do nativo: a seção `vm` de `lib_dir` com a sobreposição.
pub fn carregar_sdk_nativo(lib_dir: &Path) -> Result<SdkLayout, String> {
    SdkLayout::load_com_sobreposicao(lib_dir, &dir_sobreposicao(), ALVO_SOBREPOSICAO)
}

/// Diagnósticos de inferência nos corpos de uma biblioteca do SDK.
#[derive(Debug, Clone)]
pub struct InferenciaDaBiblioteca {
    pub biblioteca: String,
    /// Funções (e métodos, construtores, acessores) declaradas nela.
    pub funcoes: usize,
    /// As mensagens, na ordem.
    pub diagnosticos: Vec<String>,
}

/// A medição de 5a: carrega um programa que importa todas as
/// [`BIBLIOTECAS_DA_FONTE`] com a sobreposição, resolve o outline e infere
/// os corpos de cada biblioteca **separadamente** (os diagnósticos não
/// trazem o arquivo; uma inferência por biblioteca os atribui).
///
/// Depende de `infer_bodies_das_bibliotecas` inferir as bibliotecas do SDK
/// pedidas (pedido em docs/NATIVO-PEDIDOS.md); sem isso, só os\n/// inicializadores sem tipo escrito são visitados e os diagnósticos ficam\n/// perto de zero sem dizer nada.
pub fn medir_inferencia_do_sdk(lib_dir: &Path) -> Result<Vec<InferenciaDaBiblioteca>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = carregar_sdk_nativo(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-medir-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entrada = tmp.join("main.dart");
    let mut fonte = String::new();
    for b in BIBLIOTECAS_DA_FONTE {
        fonte.push_str(&format!("import 'dart:{b}';\n"));
    }
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| format!("{}: {e}", entrada.display()))?;

    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {} ({} diagnósticos)", d.message, diags_carga.len()));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);

    let mut saida = Vec::new();
    for nome in BIBLIOTECAS_DA_FONTE {
        let uri = format!("dart:{nome}");
        let Some(idx) = program.libraries.iter().position(|l| l.uri == uri) else {
            return Err(format!("{uri} não carregou"));
        };
        let lib = dartforge_elements::model::LibraryId(idx as u32);
        let funcoes = program.functions.iter().filter(|f| f.library == lib).count();
        let (_corpos, diags) =
            dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[lib]);
        saida.push(InferenciaDaBiblioteca {
            biblioteca: uri,
            funcoes,
            diagnosticos: diags.into_iter().map(|d| d.message).collect(),
        });
    }
    Ok(saida)
}

/// O resultado do lowering de um membro do SDK da fonte.
#[derive(Debug, Clone)]
pub struct MembroDoSdk {
    pub biblioteca: String,
    pub simbolo: String,
    /// Diagnósticos do lowering (vazio: baixou), ou o pânico.
    pub diagnosticos: Vec<String>,
}

/// Medição de P5c: baixa cada função das [`BIBLIOTECAS_DA_FONTE`] (mundo
/// aberto) e devolve, por membro, os diagnósticos do lowering.
pub fn medir_lowering_do_sdk(lib_dir: &Path) -> Result<Vec<MembroDoSdk>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = carregar_sdk_nativo(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-lower-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entrada = tmp.join("main.dart");
    let mut fonte = String::new();
    for b in BIBLIOTECAS_DA_FONTE {
        fonte.push_str(&format!("import 'dart:{b}';\n"));
    }
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| format!("{}: {e}", entrada.display()))?;
    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {}", d.message));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| l.uri.strip_prefix("dart:").is_some_and(|n| BIBLIOTECAS_DA_FONTE.contains(&n)))
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (corpos, _) =
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &libs);
    let ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &corpos).com_sdk_da_fonte();
    let mut saida = Vec::new();
    for f in 0..program.functions.len() {
        if !libs.contains(&program.functions[f].library) {
            continue;
        }
        let simbolo = crate::lower::simbolo_de(&ctx, f);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = crate::hir::Module::new();
            crate::lower::lower_funcao(&ctx, &mut m, f);
            m.erros
        }));
        let diagnosticos = match r {
            Ok(e) => e,
            Err(p) => {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                vec![format!("pânico: {msg}")]
            }
        };
        saida.push(MembroDoSdk {
            biblioteca: program.library(program.functions[f].library).uri.clone(),
            simbolo,
            diagnosticos,
        });
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

    /// A sobreposição existe, troca os quatro arquivos e aponta para arquivos
    /// que existem.
    #[test]
    fn sobreposicao_carrega() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let sdk = carregar_sdk_nativo(Path::new(SDK)).unwrap();
        assert_eq!(sdk.substituicoes.len(), 4);
        for b in BIBLIOTECAS_DA_FONTE {
            assert!(sdk.library(b).is_some(), "dart:{b} fora do layout");
        }
    }

    /// A medição de 5a (docs/NATIVO-PLANO.md §7.4): imprime, por biblioteca,
    /// as funções e os diagnósticos de inferência dos corpos. Lenta (infere o
    /// SDK inteiro); roda à parte:
    /// `cargo test -p dartforge-emit-native medir_inferencia -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn medir_inferencia_das_bibliotecas_da_fonte() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let tabela = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| medir_inferencia_do_sdk(Path::new(SDK)).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut total = 0;
        for b in &tabela {
            println!("{:<18} funções {:>5}  diagnósticos {:>5}", b.biblioteca, b.funcoes, b.diagnosticos.len());
            total += b.diagnosticos.len();
            let mut por_codigo: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for d in &b.diagnosticos {
                let chave: String = d.split(':').next().unwrap_or(d).chars().take(60).collect();
                *por_codigo.entry(chave).or_default() += 1;
            }
            let mut v: Vec<_> = por_codigo.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (c, n) in v.iter().take(8) {
                println!("    {n:>5}  {c}");
            }
        }
        println!("total de diagnósticos: {total}");
    }

    /// A medição de P5c: quantos membros do SDK da fonte o lowering baixa.
    /// `cargo test -p dartforge-emit-native medir_lowering -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn medir_lowering_das_bibliotecas_da_fonte() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            return;
        }
        std::panic::set_hook(Box::new(|_| {}));
        let membros = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| medir_lowering_do_sdk(Path::new(SDK)).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut por_lib: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
        let mut por_construto: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for m in &membros {
            let e = por_lib.entry(&m.biblioteca).or_default();
            e.0 += 1;
            if m.diagnosticos.is_empty() {
                e.1 += 1;
            }
            let mut vistos = std::collections::BTreeSet::new();
            for d in &m.diagnosticos {
                let d = d.strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(d);
                let chave: String = d.rsplit_once(" (").map_or(d, |(a, _)| a).chars().take(90).collect();
                let c = por_construto.entry(chave.clone()).or_default();
                c.1 += 1;
                if vistos.insert(chave) {
                    c.0 += 1;
                }
            }
        }
        for (l, (n, ok)) in &por_lib {
            println!("{l:<20} membros {n:>5}  baixados {ok:>5}");
        }
        let mut v: Vec<_> = por_construto.into_iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        for (c, (membros, ocorr)) in v.iter().take(80) {
            println!("{membros:>5} {ocorr:>6}  {c}");
        }
        if let Ok(arq) = std::env::var("DARTFORGE_MEDIR_SAIDA") {
            let mut t = String::new();
            for m in &membros {
                t.push_str(&format!("{}\t{}\t{}\n", m.biblioteca, m.simbolo, m.diagnosticos.join(" | ")));
            }
            std::fs::write(arq, t).unwrap();
        }
    }
}
