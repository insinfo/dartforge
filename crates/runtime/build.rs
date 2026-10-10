//! A fonte única do runtime nativo, compilada também como módulo deste crate.
//!
//! O runtime que o código gerado chama é a concatenação, na ordem de
//! [`FRAGMENTOS`], dos arquivos `src/<fragmento>.rs` (um por tema: núcleo,
//! raízes do coletor, exceções, saída, strings, coleções, closures). Os
//! fragmentos não são módulos Rust: são pedaços de UM programa, que se enxergam
//! sem `use`. A lista é uma só, e é esta; o texto é compilado de duas formas:
//!
//! * pelo AOT, como texto (`RUNTIME_MAIN`), com `rustc` avulso e `-O`
//!   (`crates/emit_native/src/cache.rs`), virando a `.lib` que o executável
//!   liga — com o `main` C que chama `@dartforge_entry`;
//! * por este crate, como o módulo `abi`, para o JIT (`crates/jit`), que
//!   publica os endereços das funções como símbolos absolutos — **sem** o
//!   `main` C, que colidiria com o `main` de qualquer binário Rust.
//!
//! Este script:
//!
//! 1. grava em `OUT_DIR` o texto concatenado (`runtime_main.rs`, que o
//!    `RUNTIME_MAIN` inclui) e o corpo do módulo `abi` (`abi.rs`: um
//!    `include!` por fragmento, na mesma ordem, para os erros de compilação
//!    apontarem o arquivo de verdade);
//! 2. liga a cfg `dartforge_runtime_embutido`, que tira o `main` C e a
//!    declaração de `dartforge_entry` (o `rustc` avulso do AOT não a recebe);
//! 3. gera `simbolos.rs`: a tabela `(nome, endereço)` de todo
//!    `#[unsafe(no_mangle)] pub [unsafe] extern "C" fn` de **todos** os
//!    fragmentos, exceto o `main`. Nenhum nome é escrito à mão; tomar o
//!    endereço também impede o linker de descartar as funções do rlib.
//!
//! Um arquivo novo em `src/` que não seja `lib.rs`, um dos `MODULOS` nem fragmento
//! listado derruba o build: um fragmento esquecido fora da lista seria código
//! do runtime que nenhum dos dois perfis compila.
use std::path::PathBuf;

/// Os fragmentos do runtime, na ordem de concatenação. Acrescentar um
/// fragmento é acrescentar o nome aqui (e o arquivo em `src/`).
const FRAGMENTOS: &[&str] = &[
    "nucleo",
    "gc_raizes",
    "arc_abi",
    "excecoes",
    // As portas Rust → Dart e a personalidade das exceções por tabelas.
    "excecoes_tabelas",
    "rastro",
    // `DARTFORGE_EFEITOS=conferir`: a conferência da tabela de efeitos.
    "efeitos_conferir",
    "saida",
    "strings",
    "colecoes",
    "closures",
    // δ (P5b): os natives do SDK da fonte (crates/emit_native/src/nativos.rs).
    "nativos_numeros",
    "nativos_strings",
    // O caminho rápido do `_Map`/`_Set` padrão para chaves `int` e `String`.
    "nativos_hash",
    // Relógio, fuso horário e entropia.
    "nativos_sistema",
    // O motor de expressões regulares do `RegExp`.
    "regexp",
    // P2 (α): operadores sobre dynamic/num (tapa-buraco até P5).
    "despacho",
    // δ (P5c): tabelas de métodos e busca por seletor do SDK da fonte.
    "seletores",
    "nativos_listas",
    // As listas tipadas do `typed_data_patch.dart` da VM.
    "typed_data",
    // Os tipos SIMD (`Float32x4`, `Int32x4`, `Float64x2`).
    "simd",
    // P6: o laço de eventos (microtarefas e timers).
    "eventos",
    // Portas e a fila de mensagens do isolado.
    "portas",
    // Os isolados (`Isolate.spawn`, a porta de controle).
    "isolados",
    // O dart:ffi (bibliotecas dinâmicas, memória nativa, trampolins de chamada).
    "ffi",
    "ffi_callbacks",
    // A API nativa de portas (Dart_PostCObject…) e os dados da API DL.
    "ffi_api_nativa",
    // Finalizer e NativeFinalizer sobre os anexos do coletor.
    "finalizadores",
    // O `dart:io` da VM: arquivos, diretórios, o IOService e a plataforma.
    "io_arquivos",
    "io_diretorios",
    "io_servico",
    "io_plataforma",
    // O manipulador de eventos, os soquetes e os processos.
    "io_eventos",
    "io_soquetes",
    "io_soquetes_unix",
    // A observação de arquivos (`FileSystemEntity.watch`).
    "io_observador",
    "io_processos",
    // O mesmo no Windows: a porta de conclusão, o Winsock e os processos.
    "io_windows_eventos",
    "io_windows_soquetes",
    "io_windows_processos",
    // A TLS do `dart:io` (`SecureSocket`, `SecurityContext`), sobre o rustls.
    "tls",
    // Os formatos de chave e certificado do `SecurityContext` (PKCS#12,
    // chaves cifradas).
    "tls_formatos",
    // Os filtros zlib/gzip do `dart:io` (`ZLibEncoder`, `GZipCodec`).
    "zlib",
    // O que um programa vê na VM JIT e o AOT não dá, para os builders: o
    // subconjunto de `dart:mirrors` (`reflectClass`) e o package config.
    "compat_jit",
    // `dart:developer` e a timeline no perfil de produção.
    "nativos_desenvolvedor",
    // RTI: tipos em tempo de execução.
    "tipos",
    // O alocador global do executável, com listas livres por thread (N17).
    "alocador",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_dll)");
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_embutido)");
    // As features escolhem o perfil do módulo `abi`: sem nenhuma, o JIT
    // (sem o `main` C); `aot`, a `staticlib` do executável (com o `main`);
    // `dll`, a da biblioteca compartilhada do SDK da fonte (sem o `main`).
    // As duas `staticlib` saem de `crates/runtime_estatico`.
    let aot = std::env::var_os("CARGO_FEATURE_AOT").is_some();
    let dll = std::env::var_os("CARGO_FEATURE_DLL").is_some();
    assert!(!(aot && dll), "as features aot e dll do runtime são exclusivas");
    if dll {
        println!("cargo::rustc-cfg=dartforge_runtime_dll");
    } else if !aot {
        println!("cargo::rustc-cfg=dartforge_runtime_embutido");
    }

    let manifesto =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifesto.join("src");
    conferir_lista(&src);

    let mut texto = String::new();
    let mut abi = String::from("// GERADO por crates/runtime/build.rs — não editar.\n");
    for f in FRAGMENTOS {
        let caminho = src.join(format!("{f}.rs"));
        let conteudo = std::fs::read_to_string(&caminho)
            .unwrap_or_else(|e| panic!("ler {}: {e}", caminho.display()));
        assert!(
            conteudo.ends_with('\n'),
            "{} não termina em fim de linha",
            caminho.display()
        );
        texto.push_str(&conteudo);
        let literal = format!("{:?}", caminho.to_str().expect("caminho UTF-8"));
        abi.push_str(&format!("include!({literal});\n"));
    }

    let mut nomes = nomes_exportados(&texto);
    let mut ordenados = nomes.clone();
    ordenados.sort_unstable();
    ordenados.dedup();
    assert!(
        nomes.len() - ordenados.len() == 1
            && nomes.iter().filter(|nome| nome.as_str() == "dartforge_personalidade").count() == 2
            || ordenados.len() == nomes.len(),
        "o runtime define algum símbolo #[unsafe(no_mangle)] duas vezes"
    );
    // As duas assinaturas da personalidade são exclusivas por plataforma.
    let mut vistos = std::collections::HashSet::new();
    nomes.retain(|nome| vistos.insert(nome.clone()));

    let mut saida = String::from(
        "// GERADO por crates/runtime/build.rs a partir dos fragmentos de src/ — não editar.\n\n\
         /// Os fragmentos do runtime, na ordem em que são concatenados.\n\
         pub const FRAGMENTOS: &[&str] = &[\n",
    );
    for f in FRAGMENTOS {
        saida.push_str(&format!("    \"{f}\",\n"));
    }
    saida.push_str(
        "];\n\n\
         /// Nomes dos símbolos que o runtime exporta ao código gerado, na ordem da fonte.\n\
         pub const NOMES: &[&str] = &[\n",
    );
    for nome in &nomes {
        // Só de unix, como na [`tabela`].
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            saida.push_str("    #[cfg(unix)]\n");
        }
        saida.push_str(&format!("    \"{nome}\",\n"));
    }
    saida.push_str(
        "];\n\n/// Endereço de cada função do runtime neste processo, na ordem de [`NOMES`].\n\
         pub fn tabela() -> Vec<(&'static str, usize)> {\n    vec![\n",
    );
    for nome in &nomes {
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            saida.push_str("        #[cfg(unix)]\n");
        }
        saida.push_str(&format!(
            "        (\"{nome}\", crate::abi::{nome} as *const () as usize),\n"
        ));
    }
    saida.push_str("    ]\n}\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("simbolos.rs"), saida).expect("gravar simbolos.rs");
    std::fs::write(out.join("efeitos.rs"), tabela_de_efeitos(&manifesto, &nomes)).expect("gravar efeitos.rs");
    std::fs::write(out.join("ownership.rs"), tabela_de_ownership(&manifesto, &nomes)).expect("gravar ownership.rs");
    std::fs::write(out.join("runtime_main.rs"), texto).expect("gravar runtime_main.rs");
    std::fs::write(out.join("abi.rs"), abi).expect("gravar abi.rs");
}

/// Contratos auditados ARC; não inventa contratos para as externs ainda ausentes.
/// A coerção dos ponteiros gerados confere aridade e representação com Rust.
fn tabela_de_ownership(manifesto: &std::path::Path, nomes: &[String]) -> String {
    let caminho = manifesto.join("ownership.tsv");
    println!("cargo::rerun-if-changed={}", caminho.display());
    let texto = std::fs::read_to_string(caminho).expect("ler ownership.tsv");
    let efeitos = std::fs::read_to_string(manifesto.join("efeitos.tsv")).expect("ler efeitos.tsv");
    let mut linhas = std::collections::BTreeMap::new();
    let tipo = |s: &str, resultado: bool| -> (&str, String) {
        if resultado {
            if let Some(n) = s
                .strip_prefix("ref:borrow(")
                .and_then(|s| s.strip_suffix(')'))
            {
                let n: usize = n.parse().expect("ownership.tsv: índice de borrow inválido");
                return ("i64", format!("ModoResultado::BorrowArg({n})"));
            }
        }
        let (ty, modo) = match (s, resultado) {
            ("ref:borrow", false) => ("i64", "ModoParametro::Borrow"),
            ("ref:consume", false) => ("i64", "ModoParametro::Consume"),
            ("ref:consume-success", false) => ("i64", "ModoParametro::ConsumeSuccess"),
            ("ref:consume-error", false) => ("i64", "ModoParametro::ConsumeError"),
            ("i64:scalar", false) => ("i64", "ModoParametro::Scalar"),
            ("f64:scalar", false) => ("f64", "ModoParametro::ScalarF64"),
            ("i64:native", false) => ("i64", "ModoParametro::Native"),
            ("tabela:native", false) => ("extern \"C\" fn() -> *const i64", "ModoParametro::TabelaEstatica"),
            ("ref:owned", true) => ("i64", "ModoResultado::Owned"),
            ("i64:scalar", true) => ("i64", "ModoResultado::ScalarI64"),
            ("f64:scalar", true) => ("f64", "ModoResultado::ScalarF64"),
            ("i8:scalar", true) => ("i8", "ModoResultado::ScalarI8"),
            ("u8:scalar", true) => ("u8", "ModoResultado::ScalarI8"),
            ("void:scalar", true) => ("()", "ModoResultado::Void"),
            _ => panic!("ownership.tsv: tipo/contrato não suportado: {s}"),
        };
        (ty, modo.to_string())
    };
    for (n, linha) in texto.lines().enumerate() {
        if linha.trim().is_empty() || linha.starts_with('#') {
            continue;
        }
        let c: Vec<_> = linha.split('\t').collect();
        assert_eq!(c.len(), 6, "ownership.tsv:{}: seis campos", n + 1);
        assert!(
            nomes.iter().any(|nome| nome == c[0]),
            "ownership.tsv: símbolo desconhecido {}",
            c[0]
        );
        // Novas entradas exigem auditoria explícita. Ausência não gera default.
        let efeito = efeitos
            .lines()
            .find(|linha| linha.split('\t').next() == Some(c[0]))
            .expect("ownership.tsv: símbolo sem efeitos");
        let efeito: Vec<_> = efeito.split('\t').collect();
        let pode_falhar = match c[5] {
            "normal" => false,
            "pending" | "pending-dart" => true,
            _ => panic!("ownership.tsv: saída desconhecida {}", c[5]),
        };
        assert!(
            efeito.len() == 4
                && efeito[2] == if pode_falhar { "1" } else { "0" }
                && efeito[3] == if c[5] == "pending-dart" { "1" } else { "0" },
            "ownership.tsv: saídas incompatíveis com efeitos de {}",
            c[0]
        );
        let parametros: Vec<_> = if c[1] == "-" {
            vec![]
        } else {
            c[1].split(',').map(|s| tipo(s, false)).collect()
        };
        let resultado = tipo(c[2], true);
        if let Some(n) = c[2]
            .strip_prefix("ref:borrow(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n: usize = n.parse().unwrap();
            assert!(
                parametros
                    .get(n)
                    .is_some_and(|(_, modo)| modo == "ModoParametro::Borrow"),
                "ownership.tsv: resultado borrowed exige parâmetro Ref borrowed existente"
            );
        }
        assert!(
            pode_falhar
                || parametros.iter().all(|(_, modo)| !matches!(
                    modo.as_str(),
                    "ModoParametro::ConsumeSuccess" | "ModoParametro::ConsumeError"
                )),
            "ownership.tsv: consumo por aresta exige pending"
        );
        let marca = |s: &str| match s {
            "0" => false,
            "1" => true,
            _ => panic!("ownership.tsv: marca inválida {s}"),
        };
        let chama_dart = c[5] == "pending-dart";
        assert!(!chama_dart || marca(c[4]), "ownership.tsv: reentrada Dart exige invalidação de borrows");
        assert!(
            linhas
                .insert(
                    c[0],
                    (parametros, resultado, marca(c[3]), marca(c[4]), pode_falhar, chama_dart)
                )
                .is_none(),
            "ownership.tsv: símbolo duplicado {}",
            c[0]
        );
    }
    for nome in nomes.iter().filter(|n| {
        n.starts_with("dartforge_arc_")
            || matches!(
                n.as_str(),
                "dartforge_gc_global_root"
                    | "dartforge_marcar_constante"
                    | "dartforge_gc_collect"
                    | "dartforge_marcar_permanente"
                    | "dartforge_nativo_DartForge_record_fieldAt"
                    | "dartforge_nativo_DartForge_record_numFields"
                    | "dartforge_nativo_DartForge_record_shape"
                    | "dartforge_print_handle"
                    | "dartforge_exception_pending"
                    | "dartforge_unbox_int"
                    | "dartforge_unbox_double"
                    | "dartforge_unbox_bool"
            )
    }) {
        assert!(
            linhas.contains_key(nome.as_str()),
            "ownership.tsv: extern auditada sem contrato {nome}"
        );
    }
    let mut saida = String::from(
        r#"
/// Modo semântico do argumento, independente da largura i64.
///
/// ```
/// use dartforge_runtime::ownership::ModoParametro;
/// assert_ne!(ModoParametro::Borrow, ModoParametro::Consume);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoParametro { Borrow, Consume, ConsumeSuccess, ConsumeError, Scalar, ScalarF64, Native, TabelaEstatica }
/// Resultado das externs auditadas, sem convenção implícita.
///
/// ```
/// use dartforge_runtime::ownership::ModoResultado;
/// assert_ne!(ModoResultado::Owned, ModoResultado::Void);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoResultado { Owned, BorrowArg(usize), ScalarI64, ScalarF64, ScalarI8, Void }
/// Contrato auditado, com resultado disponível somente no sucesso.
///
/// ```
/// use dartforge_runtime::ownership::{CONTRATOS, ModoResultado};
/// let c = CONTRATOS.iter().find(|c| c.nome == "dartforge_arc_quadro_carregar_v1").unwrap();
/// assert_eq!(c.resultado, ModoResultado::Owned);
/// ```
#[derive(Debug)]
pub struct Contrato {
    /// Símbolo C exato, associado à tabela de efeitos.
    pub nome: &'static str,
    /// Um modo por parâmetro, na ordem da assinatura Rust.
    pub parametros: &'static [ModoParametro],
    /// Convenção semântica do resultado normal.
    pub resultado: ModoResultado,
    /// Pode guardar ocorrências proprietárias após retornar.
    pub retencao_persistente: bool,
    /// Pode remover owners ou invalidar empréstimos associados.
    pub invalida_borrows: bool,
    /// Exige saída de exceção pendente preparada no CFG.
    pub pode_falhar: bool,
    /// Pode reentrar em código Dart; exige invalidação de empréstimos.
    pub chama_dart: bool,
}
/// Catálogo parcial auditado; nomes ausentes continuam sem contrato.
pub const CONTRATOS: &[Contrato] = &[
"#,
    );
    for (nome, (params, (_, resultado), retencao, invalida, pode_falhar, chama_dart)) in &linhas {
        let modos = params
            .iter()
            .map(|(_, modo)| modo.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        saida.push_str(&format!("Contrato {{ nome: {nome:?}, parametros: &[{modos}], resultado: {resultado}, retencao_persistente: {retencao}, invalida_borrows: {invalida}, pode_falhar: {pode_falhar}, chama_dart: {chama_dart} }},\n"));
    }
    saida.push_str("];\n");
    for (nome, (params, (ret, _), _, _, _, _)) in &linhas {
        let tipos = params
            .iter()
            .map(|(ty, _)| *ty)
            .collect::<Vec<_>>()
            .join(", ");
        saida.push_str(&format!(
            "const _: unsafe extern \"C\" fn({tipos}) -> {ret} = crate::abi::{nome};\n"
        ));
    }
    saida
}

#[cfg(test)]
mod testes_ownership {
    use super::*;

    #[test]
    fn tabela_recusa_contratos_incompletos_duplicados_e_invalidos() {
        let raiz = std::env::temp_dir().join(format!("dartforge-ownership-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::write(raiz.join("efeitos.tsv"), "dartforge_arc_retain\t0\t0\t0\n").unwrap();
        let boa = "dartforge_arc_retain\tref:borrow\tvoid:scalar\t1\t0\tnormal\n";
        let nomes = vec!["dartforge_arc_retain".to_string()];
        for ruim in [
            String::new(),
            format!("{boa}{boa}"),
            boa.replace("ref:borrow", "ref:inventado"),
            boa.replace("\t1\t0", "\t2\t0"),
            boa.replace("arc_retain", "arc_ausente"),
            boa.replace("normal", "inventado"),
            boa.replace("ref:borrow", "ref:consume-success"),
            boa.replace("void:scalar", "ref:borrow(1)"),
            boa.replace("ref:borrow", "i64:scalar")
                .replace("void:scalar", "ref:borrow(0)"),
        ] {
            std::fs::write(raiz.join("ownership.tsv"), ruim).unwrap();
            assert!(std::panic::catch_unwind(|| tabela_de_ownership(&raiz, &nomes)).is_err());
        }
        std::fs::write(raiz.join("ownership.tsv"), boa).unwrap();
        assert!(tabela_de_ownership(&raiz, &nomes).contains("const _: extern \"C\" fn(i64) -> ()"));
        std::fs::write(raiz.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        assert!(std::panic::catch_unwind(|| tabela_de_ownership(&raiz, &nomes)).is_err());
        std::fs::write(raiz.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        for modo in ["consume-success", "consume-error", "consume"] {
            std::fs::write(
                raiz.join("ownership.tsv"),
                boa.replace("ref:borrow", &format!("ref:{modo}"))
                    .replace("normal", "pending"),
            )
            .unwrap();
            let gerado = tabela_de_ownership(&raiz, &nomes);
            assert!(gerado.contains("pode_falhar: true"));
        }
        std::fs::write(raiz.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        let reentrada = boa.replace("normal", "pending-dart").replace("\t1\t0\t", "\t1\t1\t");
        std::fs::write(raiz.join("ownership.tsv"), &reentrada).unwrap();
        assert!(tabela_de_ownership(&raiz, &nomes).contains("chama_dart: true"));
        std::fs::write(raiz.join("ownership.tsv"), reentrada.replace("\t1\t1\t", "\t1\t0\t")).unwrap();
        assert!(std::panic::catch_unwind(|| tabela_de_ownership(&raiz, &nomes)).is_err());
        std::fs::write(raiz.join("ownership.tsv"), &reentrada).unwrap();
        std::fs::write(raiz.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        assert!(std::panic::catch_unwind(|| tabela_de_ownership(&raiz, &nomes)).is_err());
        std::fs::remove_file(raiz.join("ownership.tsv")).unwrap();
        std::fs::remove_file(raiz.join("efeitos.tsv")).unwrap();
        std::fs::remove_dir(raiz).unwrap();
    }
}

/// A tabela de efeitos das externs (`efeitos.tsv`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8) como código para o
/// emissor: `(nome, coleta, lança, roda Dart)`, em ordem alfabética.
///
/// O arquivo é a fonte única e tem de ter exatamente uma linha por função
/// `#[unsafe(no_mangle)]` dos fragmentos: uma extern nova sem linha, uma
/// linha de uma extern que saiu, um nome repetido ou uma marca fora de `0`/`1`
/// derrubam o build — uma extern sem marca seria tratada no escuro.
/// `roda_dart = 1` exige `coleta = 1` e `lanca = 1`.
fn tabela_de_efeitos(manifesto: &std::path::Path, nomes: &[String]) -> String {
    let caminho = manifesto.join("efeitos.tsv");
    println!("cargo::rerun-if-changed={}", caminho.display());
    let texto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("ler {}: {e}", caminho.display()));
    let mut linhas: Vec<(String, bool, bool, bool)> = Vec::new();
    for (n, linha) in texto.lines().enumerate() {
        let linha = linha.trim_end();
        if linha.is_empty() || linha.starts_with('#') {
            continue;
        }
        let campos: Vec<&str> = linha.split('\t').collect();
        let marca = |i: usize| match campos.get(i).copied() {
            Some("0") => false,
            Some("1") => true,
            _ => panic!("efeitos.tsv:{}: a linha é `nome<TAB>coleta<TAB>lanca<TAB>roda_dart`, com marcas 0 ou 1", n + 1),
        };
        assert!(campos.len() == 4, "efeitos.tsv:{}: quatro campos separados por TAB", n + 1);
        let (coleta, lanca, roda_dart) = (marca(1), marca(2), marca(3));
        assert!(!roda_dart || (coleta && lanca), "efeitos.tsv:{}: roda_dart = 1 exige coleta = 1 e lanca = 1", n + 1);
        linhas.push((campos[0].to_string(), coleta, lanca, roda_dart));
    }
    linhas.sort();
    for par in linhas.windows(2) {
        assert!(par[0].0 != par[1].0, "efeitos.tsv: `{}` aparece duas vezes", par[0].0);
    }
    let na_tabela: std::collections::BTreeSet<&str> = linhas.iter().map(|l| l.0.as_str()).collect();
    let no_runtime: std::collections::BTreeSet<&str> = nomes.iter().map(String::as_str).collect();
    let sem_linha: Vec<&&str> = no_runtime.difference(&na_tabela).collect();
    let sem_extern: Vec<&&str> = na_tabela.difference(&no_runtime).collect();
    assert!(
        sem_linha.is_empty() && sem_extern.is_empty(),
        "crates/runtime/efeitos.tsv não casa com as externs do runtime.\n  \
         sem linha na tabela (acrescente `nome<TAB>1<TAB>1<TAB>0`, a marca conservadora): {sem_linha:?}\n  \
         na tabela e fora do runtime (tire a linha): {sem_extern:?}"
    );
    let mut saida = String::from(
        "// GERADO por crates/runtime/build.rs a partir de efeitos.tsv — não editar.\n\n\
         /// `(nome, coleta, lança, roda Dart)` de cada extern do runtime, em ordem\n\
         /// alfabética do nome.\n\
         pub const EFEITOS: &[(&str, bool, bool, bool)] = &[\n",
    );
    for (nome, coleta, lanca, roda_dart) in &linhas {
        saida.push_str(&format!("    (\"{nome}\", {coleta}, {lanca}, {roda_dart}),\n"));
    }
    saida.push_str("];\n");
    saida
}

/// Os arquivos de `src/` que são módulos Rust (e não fragmentos): o `lib.rs` os
/// declara com `pub mod` e o `RUNTIME_MAIN` os embrulha em `mod x { … }`. O
/// espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.1–§3.3) acrescenta o
/// contrato de layout, o espaço de objetos e as vistas por pacote.
const MODULOS: &[&str] = &["heap", "hash", "arc", "layout", "espaco", "textos", "caixas", "listas", "tipadas"];

/// Todo `src/*.rs` é `lib.rs`, um dos [`MODULOS`] ou um fragmento listado.
fn conferir_lista(src: &std::path::Path) {
    let entradas = std::fs::read_dir(src).unwrap_or_else(|e| panic!("ler {}: {e}", src.display()));
    for entrada in entradas {
        let caminho = entrada.expect("entrada de src/").path();
        if caminho.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let nome = caminho.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            nome == "lib" || MODULOS.contains(&nome) || FRAGMENTOS.contains(&nome),
            "{} não é fragmento do runtime: acrescente \"{nome}\" a FRAGMENTOS em crates/runtime/build.rs",
            caminho.display()
        );
    }
}

/// Nomes das funções `#[unsafe(no_mangle)]` do runtime, exceto o `main` C.
///
/// A forma reconhecida é a que os fragmentos usam: o atributo numa linha e, na
/// seguinte, `pub [unsafe] extern "C" fn nome(`. Entre as duas pode haver
/// outros atributos (`#[cfg(...)]`). Qualquer outra forma derruba o build,
/// porque seria um símbolo que a tabela não saberia publicar.
fn nomes_exportados(texto: &str) -> Vec<String> {
    let mut nomes = Vec::new();
    let mut linhas = texto.lines();
    while let Some(linha) = linhas.next() {
        if linha.trim() != "#[unsafe(no_mangle)]" {
            continue;
        }
        let seguinte = loop {
            match linhas.next() {
                Some(l) if l.trim().starts_with("#[") => continue,
                Some(l) => break l.trim(),
                None => break "",
            }
        };
        let assinatura = seguinte
            .strip_prefix("pub unsafe extern \"C\" fn ")
            .or_else(|| seguinte.strip_prefix("pub extern \"C\" fn "))
            .unwrap_or_else(|| {
                panic!(
                    "#[unsafe(no_mangle)] seguido de forma não reconhecida no runtime: `{seguinte}`"
                )
            });
        let nome: String = assinatura
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if nome != "main" {
            nomes.push(nome);
        }
    }
    nomes
}
