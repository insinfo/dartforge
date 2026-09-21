//! Custo de recompilação por tipo de edição, com tempo por fase e memória.
//!
//! O objetivo é medir **quanto trabalho** uma edição pequena provoca, não apenas
//! quanto tempo a compilação levou. Cada cenário reescreve um único arquivo de um
//! corpus sintético e recompila pela sessão; os contadores mostram quantas
//! unidades foram relidas, retokenizadas, reanalisadas e reemitidas.
//!
//! Estes números descrevem o DartForge comparado a si mesmo. Não são comparação
//! com DDC ou dart2js: aquela exige programas semanticamente equivalentes e está
//! registrada separadamente em docs/BENCHMARKS.md.
use dartforge_compiler::{CompilerSession, Optimization, compile_path_with_report};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

/// Forma do grafo e do conteúdo, porque as duas coisas mudam onde está o custo.
///
/// Um leque plano de bibliotecas de funções exercita bem o paralelismo por
/// unidade, mas esconde dois custos reais: a descoberta em profundidade, que é
/// sequencial por natureza, e a análise de classes, onde o trabalho semântico de
/// código Dart de verdade se concentra.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A entrada importa todas as bibliotecas; profundidade 1.
    Flat,
    /// Cada biblioteca importa a anterior; a descoberta é uma cadeia.
    Deep,
    /// Leque plano, mas com classes, campos, métodos e interpolação.
    Classes,
}

impl Shape {
    /// Nome usado nas chaves do relatório JSON.
    fn label(self) -> &'static str {
        match self {
            Shape::Flat => "plano",
            Shape::Deep => "profundo",
            Shape::Classes => "classes",
        }
    }
}

/// Diretório temporário exclusivo desta execução do benchmark.
struct Corpus(PathBuf, Shape);

impl Corpus {
    /// Cria o diretório e escreve `libraries` bibliotecas mais a entrada.
    fn new(shape: Shape, libraries: usize, functions: usize) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dartforge-incremental-{}-{}",
            std::process::id(),
            shape.label()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("criar diretório do corpus");
        let corpus = Self(root, shape);
        for index in 0..libraries {
            corpus.write(
                &format!("lib{index}.dart"),
                &corpus.library(index, functions, 1),
            );
        }
        corpus.write("main.dart", &corpus.entry_source(libraries));
        corpus
    }
    /// Escreve a entrada conforme a forma do grafo.
    fn entry_source(&self, libraries: usize) -> String {
        let mut entry = String::new();
        if self.1 == Shape::Deep {
            // Só a última da cadeia é importada: a descoberta desce em profundidade.
            entry.push_str(&format!("import 'lib{}.dart';\n", libraries - 1));
            entry.push_str("void main() {\n");
            entry.push_str(&format!("  print(lib{}f0(1));\n", libraries - 1));
            entry.push_str("}\n");
            return entry;
        }
        for index in 0..libraries {
            entry.push_str(&format!("import 'lib{index}.dart';\n"));
        }
        entry.push_str("void main() {\n");
        for index in 0..libraries {
            entry.push_str(&format!("  print(lib{index}f0(1));\n"));
        }
        entry.push_str("}\n");
        entry
    }
    /// Gera o texto de uma biblioteca conforme a forma escolhida.
    fn library(&self, index: usize, functions: usize, revision: i32) -> String {
        match self.1 {
            Shape::Flat => library(index, functions, revision),
            Shape::Deep => deep_library(index, functions, revision),
            Shape::Classes => class_library(index, functions, revision),
        }
    }
    /// Grava um arquivo do corpus, criando diretórios intermediários.
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).expect("gravar fonte do corpus");
        path
    }
    /// Caminho da entrada usada em todas as compilações.
    fn entry(&self) -> PathBuf {
        self.0.join("main.dart")
    }
}

impl Drop for Corpus {
    /// Remove o diretório exclusivo ao final, sem seguir links.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Biblioteca da cadeia: importa a anterior e chama uma função dela.
///
/// A descoberta do grafo precisa ler cada arquivo para saber o próximo, então
/// esta forma mede exatamente o que a pré-busca paralela não consegue acelerar.
fn deep_library(index: usize, functions: usize, revision: i32) -> String {
    let mut source = String::new();
    if index > 0 {
        source.push_str(&format!("import 'lib{}.dart';\n", index - 1));
    }
    source.push_str(&library(index, functions, revision));
    if index > 0 {
        source.push_str(&format!(
            "int lib{index}Encadeado(int n) {{ return lib{}f0(n); }}\n",
            index - 1
        ));
    }
    source
}

/// Biblioteca com classes, campos, métodos e interpolação.
///
/// É a forma mais próxima de código Dart real: o custo semântico de um projeto
/// de produção está em hierarquias e membros, não em funções de topo soltas.
fn class_library(index: usize, functions: usize, revision: i32) -> String {
    let mut source = format!(
        "// biblioteca {index}\n\
         class Base{index} {{\n\
         \x20 final int semente;\n\
         \x20 Base{index}(this.semente);\n\
         \x20 int calcular(int n) {{ return semente + n + {revision}; }}\n\
         \x20 String descrever() {{ return 'Base{index}($semente)'; }}\n\
         }}\n\
         class Derivada{index} extends Base{index} {{\n\
         \x20 Derivada{index}(int s) : super(s);\n\
         }}\n"
    );
    for function in 0..functions {
        source.push_str(&format!(
            "int lib{index}f{function}(int n) {{ var alvo = Base{index}({revision}); var soma = 0; for (var i = 0; i < n; i++) {{ soma += alvo.calcular(i); }} return soma; }}\n"
        ));
    }
    source
}

/// Gera uma biblioteca com `functions` funções públicas e um valor dobrável.
///
/// `revision` altera apenas corpos, preservando assinaturas: é assim que o
/// cenário de "edição de corpo" mantém a interface estável. O subconjunto ainda
/// não tem variáveis constantes de topo, portanto o valor que a otimização de
/// constantes dobra fica dentro de uma função.
fn library(index: usize, functions: usize, revision: i32) -> String {
    let mut source =
        format!("// biblioteca {index}\nint lib{index}Limite() {{ return {revision} + 1; }}\n");
    for function in 0..functions {
        source.push_str(&format!(
            "int lib{index}f{function}(int n) {{ var soma = {revision}; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\n"
        ));
    }
    source
}

/// Executa `operation` após aquecimento e devolve mediana, p95 e amostras.
fn measure(mut operation: impl FnMut(), samples: usize) -> Value {
    for _ in 0..3 {
        operation();
    }
    let mut times = Vec::with_capacity(samples);
    dartforge_instrument::reset_peak();
    let live_before = dartforge_instrument::live_bytes();
    let allocations_before = dartforge_instrument::allocation_count();
    for _ in 0..samples {
        let start = Instant::now();
        operation();
        times.push(start.elapsed().as_nanos() as f64);
    }
    let peak = dartforge_instrument::peak_bytes();
    let allocations = dartforge_instrument::allocation_count() - allocations_before;
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let midpoint = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[midpoint - 1] + sorted[midpoint]) / 2.0
    } else {
        sorted[midpoint]
    };
    json!({
        "median_ns": median,
        "p95_ns": sorted[(sorted.len() * 95).div_ceil(100) - 1],
        "peak_live_bytes": peak,
        "live_bytes_growth": dartforge_instrument::live_bytes().saturating_sub(live_before),
        "allocations_per_run": allocations / samples,
    })
}

/// Converte o relatório da sessão em JSON com fases e trabalho realizado.
fn report(compilation: &dartforge_compiler::Compilation) -> Value {
    let link = compilation.report.link;
    json!({
        "cache_hit": compilation.report.cache_hit,
        "load_ns": compilation.report.load_ns,
        "lex_ns": link.lex_ns,
        "outline_ns": link.outline_ns,
        "namespace_ns": link.namespace_ns,
        "merge_ns": link.merge_ns,
        "parse_ns": link.parse_ns,
        "macros_ns": link.macros_ns,
        "analyze_ns": link.analyze_ns,
        "optimize_ns": link.optimize_ns,
        "emit_ns": link.emit_ns,
        "units": link.units,
        "source_bytes": link.source_bytes,
        "tokens": link.tokens,
        "classes": link.classes,
        "functions": link.functions,
        "output_bytes": link.output_bytes,
    })
}

/// Mede um cenário de edição: aplica `edit`, recompila e restaura o estado.
///
/// A alternância entre duas revisões garante que cada amostra recompile de fato,
/// em vez de medir um acerto de cache depois da primeira iteração.
fn edit_scenario(
    corpus: &Corpus,
    session: &mut CompilerSession,
    samples: usize,
    mut edit: impl FnMut(&Corpus, usize),
) -> Value {
    let entry = corpus.entry();
    let mut revision = 0usize;
    let mut last = None;
    let value = measure(
        || {
            revision += 1;
            edit(corpus, revision);
            let compilation = session
                .compile_path(&entry, Optimization::None)
                .expect("recompilação do corpus");
            last = Some(report(&compilation));
        },
        samples,
    );
    json!({"timing": value, "last_request": last})
}

/// Mede compilação fria, acerto de cache e edição de corpo numa forma de corpus.
///
/// Serve para comparar formas entre si: o leque plano esconde o custo da
/// descoberta em profundidade e o custo semântico das classes, que é onde está
/// o trabalho de um projeto Dart de verdade.
fn shape_report(shape: Shape, libraries: usize, functions: usize, samples: usize) -> Value {
    let corpus = Corpus::new(shape, libraries, functions);
    let entry = corpus.entry();
    let mut cold_report = None;
    let cold = measure(
        || {
            let (_, report) = compile_path_with_report(&entry, Default::default())
                .unwrap_or_else(|error| panic!("compilação fria: {}", error.message));
            cold_report = Some(json!({
                "load_ns": report.load_ns,
                "lex_ns": report.link.lex_ns,
                "outline_ns": report.link.outline_ns,
                "namespace_ns": report.link.namespace_ns,
                "parse_ns": report.link.parse_ns,
                "merge_ns": report.link.merge_ns,
                "analyze_ns": report.link.analyze_ns,
                "emit_ns": report.link.emit_ns,
                "units": report.link.units,
                "source_bytes": report.link.source_bytes,
                "tokens": report.link.tokens,
                "classes": report.link.classes,
                "output_bytes": report.link.output_bytes,
            }));
        },
        samples,
    );
    let mut session = CompilerSession::new();
    session
        .compile_path(&entry, Optimization::None)
        .expect("primeira compilação da sessão");
    let warm = measure(
        || {
            session
                .compile_path(&entry, Optimization::None)
                .expect("recompilação sem edição");
        },
        samples,
    );
    let body = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        let revisao = revision as i32 % 7 + 1;
        corpus.write("lib0.dart", &corpus.library(0, functions, revisao));
    });
    json!({
        "frio": {"timing": cold, "last_request": cold_report},
        "sem_edicao": {"timing": warm},
        "corpo": body,
    })
}

fn main() {
    let libraries = 24usize;
    let functions = 16usize;
    let samples = 20usize;
    let formas = json!({
        "plano": shape_report(Shape::Flat, libraries, functions, samples),
        "profundo": shape_report(Shape::Deep, libraries, functions, samples),
        "classes": shape_report(Shape::Classes, libraries, functions, samples),
    });
    let corpus = Corpus::new(Shape::Flat, libraries, functions);
    let entry = corpus.entry();

    // Compilação fria: sem sessão, sem cache, é a linha de base.
    let mut cold_report = None;
    let cold = measure(
        || {
            let (_, report) = compile_path_with_report(&entry, Default::default())
                .expect("compilação fria do corpus");
            cold_report = Some(json!({
                "load_ns": report.load_ns,
                "lex_ns": report.link.lex_ns,
                "outline_ns": report.link.outline_ns,
                "namespace_ns": report.link.namespace_ns,
                "merge_ns": report.link.merge_ns,
                "parse_ns": report.link.parse_ns,
                "macros_ns": report.link.macros_ns,
                "analyze_ns": report.link.analyze_ns,
                "optimize_ns": report.link.optimize_ns,
                "emit_ns": report.link.emit_ns,
                "units": report.link.units,
                "source_bytes": report.link.source_bytes,
                "tokens": report.link.tokens,
                "output_bytes": report.link.output_bytes,
            }));
        },
        samples,
    );

    let mut session = CompilerSession::new();
    session
        .compile_path(&entry, Optimization::None)
        .expect("primeira compilação da sessão");
    let mut warm_report = None;
    let warm = measure(
        || {
            let compilation = session
                .compile_path(&entry, Optimization::None)
                .expect("recompilação sem edição");
            warm_report = Some(report(&compilation));
        },
        samples,
    );

    let comment = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        let mut source = corpus.library(0, functions, 1);
        source.push_str(&format!("// revisão {revision}\n"));
        corpus.write("lib0.dart", &source);
    });
    let body = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        corpus.write("lib0.dart", &corpus.library(0, functions, revision as i32 % 7 + 1));
    });
    let signature = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        let mut source = corpus.library(0, functions, 1);
        source.push_str(&format!("int lib0extra{}(int n) => n;\n", revision % 3));
        corpus.write("lib0.dart", &source);
    });
    let constant = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        let source = corpus.library(0, functions, 1).replace(
            "int lib0Limite() { return 1 + 1; }",
            &format!("int lib0Limite() {{ return {} + 1; }}", revision % 5 + 1),
        );
        corpus.write("lib0.dart", &source);
    });
    let import = edit_scenario(&corpus, &mut session, samples, |corpus, revision| {
        let mut entry = String::new();
        let used = if revision.is_multiple_of(2) {
            libraries
        } else {
            libraries - 1
        };
        for index in 0..used {
            entry.push_str(&format!("import 'lib{index}.dart';\n"));
        }
        entry.push_str("void main() {\n");
        for index in 0..used {
            entry.push_str(&format!("  print(lib{index}f0(1));\n"));
        }
        entry.push_str("}\n");
        corpus.write("main.dart", &entry);
    });

    let output = json!({
        "corpus": {"libraries": libraries, "functions_per_library": functions, "samples": samples},
        "formas": formas,
        "frio": {"timing": cold, "last_request": cold_report},
        "sem_edicao": {"timing": warm, "last_request": warm_report},
        "comentario": comment,
        "corpo": body,
        "assinatura": signature,
        "constante": constant,
        "import": import,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    let _ = Path::new(".");
}
