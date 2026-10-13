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
    "core",
    "gc_roots",
    "arc_abi",
    "arc_native_ports",
    "exceptions",
    // As portas Rust → Dart e a personalidade das exceções por tabelas.
    "exceptions_tables",
    "stack_trace",
    // `DARTFORGE_EFEITOS=conferir`: a conferência da tabela de efeitos.
    "effects_check",
    "output",
    "strings",
    "collections",
    "closures",
    // δ (P5b): os natives do SDK da fonte (crates/emit_native/src/nativos.rs).
    "natives_numbers",
    "natives_strings",
    // O caminho rápido do `_Map`/`_Set` padrão para chaves `int` e `String`.
    "natives_hash",
    // Relógio, fuso horário e entropia.
    "natives_system",
    // O motor de expressões regulares do `RegExp`.
    "regexp",
    // P2 (α): operadores sobre dynamic/num (tapa-buraco até P5).
    "dispatch",
    // δ (P5c): tabelas de métodos e busca por seletor do SDK da fonte.
    "selectors",
    "natives_lists",
    // As listas tipadas do `typed_data_patch.dart` da VM.
    "typed_data",
    // Os tipos SIMD (`Float32x4`, `Int32x4`, `Float64x2`).
    "simd",
    // P6: o laço de eventos (microtarefas e timers).
    "events",
    // Portas e a fila de mensagens do isolado.
    "ports",
    // Os isolados (`Isolate.spawn`, a porta de controle).
    "isolates",
    // O dart:ffi (bibliotecas dinâmicas, memória nativa, trampolins de chamada).
    "ffi",
    "ffi_callbacks",
    // A API nativa de portas (Dart_PostCObject…) e os dados da API DL.
    "ffi_api_nativa",
    // Finalizer e NativeFinalizer sobre os anexos do coletor.
    "finalizers",
    // O `dart:io` da VM: arquivos, diretórios, o IOService e a plataforma.
    "io_files",
    "io_directories",
    "io_service",
    "io_platform",
    // O manipulador de eventos, os soquetes e os processos.
    "io_events",
    "io_sockets",
    "io_sockets_unix",
    // A observação de arquivos (`FileSystemEntity.watch`).
    "io_observador",
    "io_processes",
    // O mesmo no Windows: a porta de conclusão, o Winsock e os processos.
    "io_windows_eventos",
    "io_windows_sockets",
    "io_windows_processes",
    // A TLS do `dart:io` (`SecureSocket`, `SecurityContext`), sobre o rustls.
    "tls",
    // Os formatos de chave e certificado do `SecurityContext` (PKCS#12,
    // chaves cifradas).
    "tls_formats",
    // Os filtros zlib/gzip do `dart:io` (`ZLibEncoder`, `GZipCodec`).
    "zlib",
    // O que um programa vê na VM JIT e o AOT não dá, para os builders: o
    // subconjunto de `dart:mirrors` (`reflectClass`) e o package config.
    "compat_jit",
    // `dart:developer` e a timeline no perfil de produção.
    "natives_developer",
    // RTI: tipos em tempo de execução.
    "types",
    // O alocador global do executável, com listas livres por thread (N17).
    "allocator",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_dll)");
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_embutido)");
    // As features escolhem o perfil do módulo `abi`: sem nenhuma, o JIT
    // (sem o `main` C); `aot`, a `staticlib` do executável (com o `main`);
    // `dll`, a da biblioteca compartilhada do SDK da fonte (sem o `main`).
    // As duas `staticlib` saem de `crates/runtime_static`.
    let aot = std::env::var_os("CARGO_FEATURE_AOT").is_some();
    let dll = std::env::var_os("CARGO_FEATURE_DLL").is_some();
    assert!(!(aot && dll), "as features aot e dll do runtime são exclusivas");
    if dll {
        println!("cargo::rustc-cfg=dartforge_runtime_dll");
    } else if !aot {
        println!("cargo::rustc-cfg=dartforge_runtime_embutido");
    }

    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    check_list(&src);

    let mut text = String::new();
    let mut abi = String::from("// GERADO por crates/runtime/build.rs — não editar.\n");
    for f in FRAGMENTOS {
        let file_path = src.join(format!("{f}.rs"));
        let content = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("ler {}: {e}", file_path.display()));
        assert!(
            content.ends_with('\n'),
            "{} não termina em fim de linha",
            file_path.display()
        );
        text.push_str(&content);
        let literal = format!("{:?}", file_path.to_str().expect("caminho UTF-8"));
        abi.push_str(&format!("include!({literal});\n"));
    }

    let mut names = exported_names(&text);
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert!(
        names.len() - sorted.len() == 1
            && names.iter().filter(|nome| nome.as_str() == "dartforge_personalidade").count() == 2
            || sorted.len() == names.len(),
        "o runtime define algum símbolo #[unsafe(no_mangle)] duas vezes"
    );
    // As duas assinaturas da personalidade são exclusivas por plataforma.
    let mut seen_set = std::collections::HashSet::new();
    names.retain(|nome| seen_set.insert(nome.clone()));

    let mut output = String::from(
        "// GERADO por crates/runtime/build.rs a partir dos fragmentos de src/ — não editar.\n\n\
         /// Os fragmentos do runtime, na ordem em que são concatenados.\n\
         pub const FRAGMENTOS: &[&str] = &[\n",
    );
    for f in FRAGMENTOS {
        output.push_str(&format!("    \"{f}\",\n"));
    }
    output.push_str(
        "];\n\n\
         /// Nomes dos símbolos que o runtime exporta ao código gerado, na ordem da fonte.\n\
         pub const NOMES: &[&str] = &[\n",
    );
    for nome in &names {
        // Só de unix, como na [`tabela`].
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            output.push_str("    #[cfg(unix)]\n");
        }
        output.push_str(&format!("    \"{nome}\",\n"));
    }
    output.push_str(
        "];\n\n/// Endereço de cada função do runtime neste processo, na ordem de [`NOMES`].\n\
         pub fn tabela() -> Vec<(&'static str, usize)> {\n    vec![\n",
    );
    for nome in &names {
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            output.push_str("        #[cfg(unix)]\n");
        }
        output.push_str(&format!(
            "        (\"{nome}\", crate::abi::{nome} as *const () as usize),\n"
        ));
    }
    output.push_str("    ]\n}\n");
    // @Native para a ABI reservada precisa do runtime que atende o programa,
    // não de um homônimo no executável hospedeiro de uma biblioteca RTLD_LOCAL.
    // Gera a mesma seleção no módulo Cargo e no texto avulso do AOT.
    let mut resolver = String::from("fn endereco_nativo_do_runtime(nome: &str) -> Option<usize> {\n    match nome {\n");
    for nome in &names {
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            resolver.push_str("        #[cfg(unix)]\n");
        }
        resolver.push_str(&format!("        \"{nome}\" => Some({nome} as *const () as usize),\n"));
    }
    resolver.push_str("        _ => None,\n    }\n}\n");
    text.insert_str(0, &resolver);
    abi.push_str(&resolver);
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("simbolos.rs"), output).expect("gravar simbolos.rs");
    std::fs::write(out.join("efeitos.rs"), effects_table(&manifest, &names)).expect("gravar efeitos.rs");
    std::fs::write(out.join("ownership.rs"), ownership_table(&manifest, &names)).expect("gravar ownership.rs");
    std::fs::write(out.join("runtime_main.rs"), text).expect("gravar runtime_main.rs");
    std::fs::write(out.join("abi.rs"), abi).expect("gravar abi.rs");
}

/// Contratos auditados ARC; não inventa contratos para as externs ainda ausentes.
/// A coerção dos ponteiros gerados confere aridade e representação com Rust.
fn ownership_table(manifest: &std::path::Path, names: &[String]) -> String {
    let file_path = manifest.join("ownership.tsv");
    println!("cargo::rerun-if-changed={}", file_path.display());
    let text = std::fs::read_to_string(file_path).expect("ler ownership.tsv");
    let efeitos = std::fs::read_to_string(manifest.join("efeitos.tsv")).expect("ler efeitos.tsv");
    let mut line_list = std::collections::BTreeMap::new();
    let tipo = |s: &str, resultado: bool| -> (&str, String) {
        if resultado
            && let Some(n) = s
                .strip_prefix("ref:borrow(")
                .and_then(|s| s.strip_suffix(')'))
        {
            let n: usize = n.parse().expect("ownership.tsv: índice de borrow inválido");
            return ("i64", format!("ModoResultado::BorrowArg({n})"));
        }
        let (ty, mode) = match (s, resultado) {
            ("ref:borrow", false) => ("i64", "ModoParametro::Borrow"),
            ("ref:consume", false) => ("i64", "ModoParametro::Consume"),
            ("ref:consume-success", false) => ("i64", "ModoParametro::ConsumeSuccess"),
            ("ref:consume-error", false) => ("i64", "ModoParametro::ConsumeError"),
            ("i64:scalar", false) => ("i64", "ModoParametro::Scalar"),
            ("f64:scalar", false) => ("f64", "ModoParametro::ScalarF64"),
            ("u8:scalar", false) => ("u8", "ModoParametro::ScalarI8"),
            ("i8:scalar", false) => ("i8", "ModoParametro::ScalarI8"),
            ("i64:native", false) => ("i64", "ModoParametro::Native"),
            ("ptr:native", false) => ("*const i64", "ModoParametro::EnderecoNative"),
            // Bytes constantes do módulo (um nome UTF-8), o mesmo endereço nativo.
            ("bytes:native", false) => ("*const u8", "ModoParametro::EnderecoNative"),
            ("vetor:native", false) => ("*const i64", "ModoParametro::VetorEstatico"),
            ("tabela:native", false) => ("extern \"C\" fn() -> *const i64", "ModoParametro::TabelaEstatica"),
            ("ref:owned", true) => ("i64", "ModoResultado::Owned"),
            ("i64:scalar", true) => ("i64", "ModoResultado::ScalarI64"),
            ("f64:scalar", true) => ("f64", "ModoResultado::ScalarF64"),
            ("i8:scalar", true) => ("i8", "ModoResultado::ScalarI8"),
            ("i1:scalar", true) => ("bool", "ModoResultado::ScalarI1"),
            ("u8:scalar", true) => ("u8", "ModoResultado::ScalarI8"),
            ("void:scalar", true) => ("()", "ModoResultado::Void"),
            _ => panic!("ownership.tsv: tipo/contrato não suportado: {s}"),
        };
        (ty, mode.to_string())
    };
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 6, "ownership.tsv:{}: seis campos", n + 1);
        assert!(
            names.iter().any(|nome| nome == c[0]),
            "ownership.tsv: símbolo desconhecido {}",
            c[0]
        );
        // Novas entradas exigem auditoria explícita. Ausência não gera default.
        let effect = efeitos
            .lines()
            .find(|line| line.split('\t').next() == Some(c[0]))
            .expect("ownership.tsv: símbolo sem efeitos");
        let effect: Vec<_> = effect.split('\t').collect();
        let pode_falhar = match c[5] {
            "normal" => false,
            "pending" | "pending-dart" => true,
            _ => panic!("ownership.tsv: saída desconhecida {}", c[5]),
        };
        assert!(
            effect.len() == 4
                && effect[2] == if pode_falhar { "1" } else { "0" }
                && effect[3] == if c[5] == "pending-dart" { "1" } else { "0" },
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
                    .is_some_and(|(_, mode)| mode == "ModoParametro::Borrow"),
                "ownership.tsv: resultado borrowed exige parâmetro Ref borrowed existente"
            );
        }
        assert!(
            pode_falhar
                || parametros.iter().all(|(_, mode)| !matches!(
                    mode.as_str(),
                    "ModoParametro::ConsumeSuccess" | "ModoParametro::ConsumeError"
                )),
            "ownership.tsv: consumo por aresta exige pending"
        );
        let mark = |s: &str| match s {
            "0" => false,
            "1" => true,
            _ => panic!("ownership.tsv: marca inválida {s}"),
        };
        let chama_dart = c[5] == "pending-dart";
        assert!(!chama_dart || mark(c[4]), "ownership.tsv: reentrada Dart exige invalidação de borrows");
        assert!(
            line_list
                .insert(
                    c[0],
                    (parametros, resultado, mark(c[3]), mark(c[4]), pode_falhar, chama_dart)
                )
                .is_none(),
            "ownership.tsv: símbolo duplicado {}",
            c[0]
        );
    }
    for nome in names.iter().filter(|n| {
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
            line_list.contains_key(nome.as_str()),
            "ownership.tsv: extern auditada sem contrato {nome}"
        );
    }
    let mut output = String::from(
        r#"
/// Modo semântico do argumento, independente da largura i64.
///
/// ```
/// use dartforge_runtime::ownership::ModoParametro;
/// assert_ne!(ModoParametro::Borrow, ModoParametro::Consume);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoParametro { Borrow, Consume, ConsumeSuccess, ConsumeError, Scalar, ScalarF64, ScalarI8, Native, EnderecoNative, TabelaEstatica, VetorEstatico }
/// Resultado das externs auditadas, sem convenção implícita.
///
/// ```
/// use dartforge_runtime::ownership::ModoResultado;
/// assert_ne!(ModoResultado::Owned, ModoResultado::Void);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoResultado { Owned, BorrowArg(usize), ScalarI64, ScalarF64, ScalarI8, ScalarI1, Void }
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
    for (nome, (params, (_, resultado), retention, invalid, pode_falhar, chama_dart)) in &line_list {
        let modes = params
            .iter()
            .map(|(_, mode)| mode.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("Contrato {{ nome: {nome:?}, parametros: &[{modes}], resultado: {resultado}, retencao_persistente: {retention}, invalida_borrows: {invalid}, pode_falhar: {pode_falhar}, chama_dart: {chama_dart} }},\n"));
    }
    output.push_str("];\n");
    for (nome, (params, (ret, _), _, _, _, _)) in &line_list {
        let types = params
            .iter()
            .map(|(ty, _)| *ty)
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "const _: unsafe extern \"C\" fn({types}) -> {ret} = crate::abi::{nome};\n"
        ));
    }
    output
}

#[cfg(test)]
mod testes_ownership {
    use super::*;

    #[test]
    fn table_rejects_incomplete_duplicate_and_invalid_contracts() {
        let root = std::env::temp_dir().join(format!("dartforge-ownership-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t0\t0\t0\n").unwrap();
        let good = "dartforge_arc_retain\tref:borrow\tvoid:scalar\t1\t0\tnormal\n";
        let names = vec!["dartforge_arc_retain".to_string()];
        for bad in [
            String::new(),
            format!("{good}{good}"),
            good.replace("ref:borrow", "ref:inventado"),
            good.replace("\t1\t0", "\t2\t0"),
            good.replace("arc_retain", "arc_ausente"),
            good.replace("normal", "inventado"),
            good.replace("ref:borrow", "ref:consume-success"),
            good.replace("void:scalar", "ref:borrow(1)"),
            good.replace("ref:borrow", "i64:scalar")
                .replace("void:scalar", "ref:borrow(0)"),
        ] {
            std::fs::write(root.join("ownership.tsv"), bad).unwrap();
            assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        }
        std::fs::write(root.join("ownership.tsv"), good).unwrap();
        assert!(ownership_table(&root, &names).contains("const _: unsafe extern \"C\" fn(i64) -> () = crate::abi::dartforge_arc_retain;"));
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        for mode in ["consume-success", "consume-error", "consume"] {
            std::fs::write(
                root.join("ownership.tsv"),
                good.replace("ref:borrow", &format!("ref:{mode}"))
                    .replace("normal", "pending"),
            )
            .unwrap();
            let generated = ownership_table(&root, &names);
            assert!(generated.contains("pode_falhar: true"));
        }
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        let reentry = good.replace("normal", "pending-dart").replace("\t1\t0\t", "\t1\t1\t");
        std::fs::write(root.join("ownership.tsv"), &reentry).unwrap();
        assert!(ownership_table(&root, &names).contains("chama_dart: true"));
        std::fs::write(root.join("ownership.tsv"), reentry.replace("\t1\t1\t", "\t1\t0\t")).unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        std::fs::write(root.join("ownership.tsv"), &reentry).unwrap();
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        // O callback tem representação Rust própria: não aceitar um inteiro
        // ou ponteiro genérico no lugar da função que devolve a tabela estática.
        let factory = "dartforge_arc_objeto_owned_t_v1".to_string();
        std::fs::write(root.join("efeitos.tsv"), format!("{factory}\t1\t0\t0\n")).unwrap();
        std::fs::write(root.join("ownership.tsv"), format!("{factory}\ti64:native,i64:scalar,tabela:native\tref:owned\t0\t1\tnormal\n")).unwrap();
        let generated = ownership_table(&root, &[factory]);
        assert!(generated.contains("const _: unsafe extern \"C\" fn(i64, i64, extern \"C\" fn() -> *const i64) -> i64 = crate::abi::dartforge_arc_objeto_owned_t_v1;"));
        assert!(generated.contains("ModoParametro::TabelaEstatica"));
        std::fs::remove_file(root.join("ownership.tsv")).unwrap();
        std::fs::remove_file(root.join("efeitos.tsv")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

/// A tabela de efeitos das externs (`efeitos.tsv`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8) como código para o
/// emissor: `(nome, collection, lança, roda Dart)`, em ordem alfabética.
///
/// O arquivo é a fonte única e tem de ter exatamente uma linha por função
/// `#[unsafe(no_mangle)]` dos fragmentos: uma extern nova sem linha, uma
/// linha de uma extern que saiu, um nome repetido ou uma marca fora de `0`/`1`
/// derrubam o build — uma extern sem marca seria tratada no escuro.
/// `roda_dart = 1` exige `coleta = 1` e `lanca = 1`.
fn effects_table(manifest: &std::path::Path, names: &[String]) -> String {
    let file_path = manifest.join("efeitos.tsv");
    println!("cargo::rerun-if-changed={}", file_path.display());
    let text = std::fs::read_to_string(&file_path).unwrap_or_else(|e| panic!("ler {}: {e}", file_path.display()));
    let mut line_list: Vec<(String, bool, bool, bool)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let mark = |i: usize| match fields.get(i).copied() {
            Some("0") => false,
            Some("1") => true,
            _ => panic!("efeitos.tsv:{}: a linha é `nome<TAB>coleta<TAB>lanca<TAB>roda_dart`, com marcas 0 ou 1", n + 1),
        };
        assert!(fields.len() == 4, "efeitos.tsv:{}: quatro campos separados por TAB", n + 1);
        let (collection, throws, roda_dart) = (mark(1), mark(2), mark(3));
        assert!(!roda_dart || (collection && throws), "efeitos.tsv:{}: roda_dart = 1 exige coleta = 1 e lanca = 1", n + 1);
        line_list.push((fields[0].to_string(), collection, throws, roda_dart));
    }
    line_list.sort();
    for pair in line_list.windows(2) {
        assert!(pair[0].0 != pair[1].0, "efeitos.tsv: `{}` aparece duas vezes", pair[0].0);
    }
    let in_table: std::collections::BTreeSet<&str> = line_list.iter().map(|l| l.0.as_str()).collect();
    let in_runtime: std::collections::BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let without_line: Vec<&&str> = in_runtime.difference(&in_table).collect();
    let without_extern: Vec<&&str> = in_table.difference(&in_runtime).collect();
    assert!(
        without_line.is_empty() && without_extern.is_empty(),
        "crates/runtime/efeitos.tsv não casa com as externs do runtime.\n  \
         sem linha na tabela (acrescente `nome<TAB>1<TAB>1<TAB>0`, a marca conservadora): {without_line:?}\n  \
         na tabela e fora do runtime (tire a linha): {without_extern:?}"
    );
    let mut output = String::from(
        "// GERADO por crates/runtime/build.rs a partir de efeitos.tsv — não editar.\n\n\
         /// `(nome, coleta, lança, roda Dart)` de cada extern do runtime, em ordem\n\
         /// alfabética do nome.\n\
         pub const EFEITOS: &[(&str, bool, bool, bool)] = &[\n",
    );
    for (nome, collection, throws, roda_dart) in &line_list {
        output.push_str(&format!("    (\"{nome}\", {collection}, {throws}, {roda_dart}),\n"));
    }
    output.push_str("];\n");
    output
}

/// Os arquivos de `src/` que são módulos Rust (e não fragmentos): o `lib.rs` os
/// declara com `pub mod` e o `RUNTIME_MAIN` os embrulha em `mod x { … }`. O
/// espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.1–§3.3) acrescenta o
/// contrato de layout, o espaço de objetos e as vistas por pacote; a forma
/// comprimida da tabela da RTI (`tabela_rti`) serve ao compilador e ao runtime.
const MODULOS: &[&str] = &["heap", "hash", "arc", "layout", "space", "textos", "boxes", "lists", "tipadas", "tabela_rti"];

/// Todo `src/*.rs` é `lib.rs`, um dos [`MODULOS`] ou um fragmento listado.
fn check_list(src: &std::path::Path) {
    let entries = std::fs::read_dir(src).unwrap_or_else(|e| panic!("ler {}: {e}", src.display()));
    for input in entries {
        let file_path = input.expect("entrada de src/").path();
        if file_path.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let nome = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            nome == "lib" || MODULOS.contains(&nome) || FRAGMENTOS.contains(&nome),
            "{} não é fragmento do runtime: acrescente \"{nome}\" a FRAGMENTOS em crates/runtime/build.rs",
            file_path.display()
        );
    }
}

/// Nomes das funções `#[unsafe(no_mangle)]` do runtime, exceto o `main` C.
///
/// A forma reconhecida é a que os fragmentos usam: o atributo numa linha e, na
/// seguinte, `pub [unsafe] extern "C" fn nome(`. Entre as duas pode haver
/// outros atributos (`#[cfg(...)]`). Qualquer outra forma derruba o build,
/// porque seria um símbolo que a tabela não saberia publicar.
fn exported_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut line_list = text.lines();
    while let Some(line) = line_list.next() {
        if line.trim() != "#[unsafe(no_mangle)]" {
            continue;
        }
        let next_one = loop {
            match line_list.next() {
                Some(l) if l.trim().starts_with("#[") => continue,
                Some(l) => break l.trim(),
                None => break "",
            }
        };
        let fn_signature = next_one
            .strip_prefix("pub unsafe extern \"C\" fn ")
            .or_else(|| next_one.strip_prefix("pub extern \"C\" fn "))
            .unwrap_or_else(|| {
                panic!(
                    "#[unsafe(no_mangle)] seguido de forma não reconhecida no runtime: `{next_one}`"
                )
            });
        let nome: String = fn_signature
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if nome != "main" {
            names.push(nome);
        }
    }
    names
}
