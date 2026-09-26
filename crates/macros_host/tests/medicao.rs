//! Medição do hospedeiro de macros (docs/MACROS-PROTOCOLO.md §8): o tempo de
//! execução das macros, das consultas, da montagem e da recarga, separados,
//! no `corpus/macros/410_json_codable` e num caso sintético com `N` classes
//! `@JsonCodable` numa biblioteca só (onde a remontagem acumulada da fase de
//! declarações aparece). Para cada caso:
//!
//! 1. compilação limpa com [`aplicar`] (recarga completa, sem cache — o
//!    caminho anterior a esta medição);
//! 2. compilação limpa com [`aplicar_incremental`] (recarga por diferença,
//!    cache vazio);
//! 3. recompilação sem mudança com o cache (sem executor nenhum);
//! 4. recompilação depois de acrescentar um campo a uma classe (o cache
//!    reexecuta só as expansões que o observam).
//!
//! Mede desempenho, não correção (CONTRIBUTING.md): o teste imprime os
//! números e confere só o essencial. Exige a VM Dart 3.6.2
//! (`DARTFORGE_DART_SDK`) e o `pub get` de `corpus/macros/410_json_codable`:
//!
//! ```text
//! cargo test --release -p dartforge-macros-host --test medicao -- --ignored --nocapture
//! ```
use dartforge_elements::load::load_lenient_gerados;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::unidades::CacheUnidades;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::cache::CacheDeMacros;
use dartforge_macros_host::executor::{ExecutorDfexec, ExecutorMacros};
use dartforge_macros_host::modelo::Vista;
use dartforge_macros_host::protocolo::CanalDeProcesso;
use dartforge_macros_host::{Saida, aplicar, aplicar_incremental};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sdk() -> SdkLayout {
    let lib = SdkLayout::discover().expect("SDK (DARTFORGE_SDK_LIB)");
    let mut sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
    sdk.versao_corrente = LanguageVersion::new(3, 6);
    sdk.experimentos = vec![Feature::Macros];
    sdk
}

fn dart() -> PathBuf {
    std::env::var_os("DARTFORGE_DART_SDK")
        .map(PathBuf::from)
        .or_else(|| SdkLayout::discover().and_then(|l| l.parent().map(Path::to_path_buf)))
        .map(|s| s.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }))
        .expect("DARTFORGE_DART_SDK")
}

/// A VM como executor, com o bootstrap das aplicações de `entrada`.
fn vm(entrada: &Path, config: &Path, sdk: &SdkLayout, trabalho: &Path) -> ExecutorDfexec<CanalDeProcesso> {
    let mut nomes = Interner::new();
    let (p, _) = load_lenient_gerados(entrada, sdk, Some(config), &mut nomes, None, None, None);
    let apps = dartforge_macros_host::aplicacoes::detectar(&Vista { program: &p, interner: &nomes });
    let cfg = dartforge_macros_host::vm::ConfigDaVm {
        dart: dart(),
        api: raiz().join("pacotes/macros"),
        trabalho: trabalho.to_path_buf(),
    };
    dartforge_macros_host::vm::iniciar(&cfg, &apps, Some(config)).unwrap()
}

fn falhou(ds: Vec<dartforge_diagnostics::Diagnostic>) -> ! {
    panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n"))
}

/// Uma compilação; `cache: None` usa [`aplicar`] (recarga completa).
fn compilar(
    entrada: &Path,
    config: &Path,
    sdk: &SdkLayout,
    executor: &mut dyn ExecutorMacros,
    modo: Option<&mut CacheDeMacros>,
    por_diferenca: bool,
) -> Saida {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(entrada, sdk, Some(config), &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    if por_diferenca {
        let mut carregar =
            |i: &mut Interner, g, u: &mut CacheUnidades| load_lenient_gerados(entrada, sdk, Some(config), i, None, Some(u), g);
        aplicar_incremental(p, &mut nomes, None, &mut carregar, executor, modo).unwrap_or_else(|d| falhou(d))
    } else {
        let mut carregar = |i: &mut Interner, g| load_lenient_gerados(entrada, sdk, Some(config), i, None, None, g);
        aplicar(p, &mut nomes, None, &mut carregar, executor).unwrap_or_else(|d| falhou(d))
    }
}

fn relatar(caso: &str, etapa: &str, total: std::time::Duration, s: &Saida) {
    eprintln!(
        "[medição] {caso} | {etapa}: total {:.1} ms | {}",
        total.as_secs_f64() * 1000.0,
        s.medicao.resumo()
    );
}

/// As quatro etapas num caso; `editar` faz a edição da etapa 4.
fn medir(caso: &str, entrada: &Path, config: &Path, sdk: &SdkLayout, editar: impl FnOnce()) {
    let trabalho = tempfile::tempdir().unwrap();

    let t = Instant::now();
    let mut executor = vm(entrada, config, sdk, trabalho.path());
    let limpa = compilar(entrada, config, sdk, &mut executor, None, false);
    relatar(caso, "1 limpa, recarga completa", t.elapsed(), &limpa);

    let t = Instant::now();
    let mut executor = vm(entrada, config, sdk, trabalho.path());
    let mut cache = CacheDeMacros::novo("vm dart 3.6.2");
    let diferenca = compilar(entrada, config, sdk, &mut executor, Some(&mut cache), true);
    relatar(caso, "2 limpa, recarga por diferença", t.elapsed(), &diferenca);
    let textos = |s: &Saida| s.textos.iter().map(|t| t.texto.clone()).collect::<Vec<_>>();
    assert_eq!(textos(&limpa), textos(&diferenca));

    // Sem mudança: nenhum executor é criado.
    let t = Instant::now();
    let mut indisponivel = dartforge_macros_host::executor::Indisponivel::default();
    let sem_mudanca = compilar(entrada, config, sdk, &mut indisponivel, Some(&mut cache), true);
    relatar(caso, "3 recompilação sem mudança", t.elapsed(), &sem_mudanca);
    assert_eq!(sem_mudanca.macros_executadas, 0);
    assert_eq!(textos(&limpa), textos(&sem_mudanca));

    editar();
    let t = Instant::now();
    let mut executor = vm(entrada, config, sdk, trabalho.path());
    let editada = compilar(entrada, config, sdk, &mut executor, Some(&mut cache), true);
    relatar(caso, "4 recompilação com um campo novo", t.elapsed(), &editada);
}

/// `n` classes `@JsonCodable` com três campos cada, numa biblioteca.
fn sintetico(n: usize, extra: bool) -> String {
    let mut s = String::from("import 'package:json/json.dart';\n\n");
    for i in 0..n {
        let campo = if extra && i == 0 { "  final bool? novo;\n" } else { "" };
        s.push_str(&format!(
            "@JsonCodable()\nclass C{i} {{\n  final String nome;\n  final int? valor;\n  final List<int> notas;\n{campo}}}\n\n"
        ));
    }
    s.push_str("void main() {\n  print(C0.fromJson({'nome': 'a', 'notas': <int>[]}).toJson());\n}\n");
    s
}

#[test]
#[ignore = "medição: exige a VM Dart 3.6.2 (DARTFORGE_DART_SDK) e o pub get de corpus/macros/410_json_codable"]
fn medicao_do_hospedeiro() {
    let sdk = sdk();
    let caso = raiz().join("corpus/macros/410_json_codable");
    let config = caso.join(".dart_tool/package_config.json");
    assert!(config.is_file(), "falta o pub get de {}", caso.display());
    // O 410 numa cópia (a etapa 4 edita a biblioteca).
    let copia = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(copia.path().join("lib")).unwrap();
    std::fs::copy(caso.join("main.dart"), copia.path().join("main.dart")).unwrap();
    let modelos = copia.path().join("lib/modelos.dart");
    std::fs::copy(caso.join("lib/modelos.dart"), &modelos).unwrap();
    let entrada = copia.path().join("main.dart");
    // `package:caso_json/` aponta para a cópia.
    let mut pc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    for p in pc["packages"].as_array_mut().unwrap() {
        if p["name"] == "caso_json" {
            p["rootUri"] = serde_json::json!(format!("file://{}/", copia.path().display()));
        } else if let Some(r) = p["rootUri"].as_str().filter(|r| !r.contains(':')) {
            let abs = config.parent().unwrap().join(r);
            p["rootUri"] = serde_json::json!(format!("file://{}/", abs.display()));
        }
    }
    let config_copia = copia.path().join("package_config.json");
    std::fs::write(&config_copia, pc.to_string()).unwrap();
    medir("410_json_codable", &entrada, &config_copia, &sdk, || {
        let texto = std::fs::read_to_string(&modelos).unwrap();
        std::fs::write(&modelos, texto.replacen("  final int? numero;\n", "  final int? numero;\n  final bool? novo;\n", 1)).unwrap();
    });

    let n: Vec<usize> = std::env::var("DARTFORGE_MEDICAO_N")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![8, 32]);
    for n in n {
        let dir = tempfile::tempdir().unwrap();
        let entrada = dir.path().join("main.dart");
        std::fs::write(&entrada, sintetico(n, false)).unwrap();
        medir(&format!("sintético N={n}"), &entrada, &config, &sdk, || std::fs::write(&entrada, sintetico(n, true)).unwrap());
    }
}
