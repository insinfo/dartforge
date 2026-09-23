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
}
