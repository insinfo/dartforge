//! O caminho inteiro do hospedeiro contra o oráculo: detecção, ordem, as três
//! fases com o programa recarregado, o modelo e as consultas, o protocolo
//! `dfexec/1`, a nossa API de macros (Dart, `pacotes/macros`) rodando o
//! `@JsonCodable` do `package:json` 0.20.4 **sem mudança**, e a montagem —
//! o texto sai byte a byte igual ao que o CFE 3.6.2 gera
//! (`corpus/macros/*/esperado/*.augmentation.dart`, extraído do `.dill` por
//! `scripts/oraculo_augmentation.dart`).
//!
//! Dois testes:
//!
//! * [`sessao_gravada_reproduz_o_texto_do_cfe`] — sem executor nenhum: o
//!   executor falso reproduz a sessão gravada (`esperado/sessao.dfexec`) e
//!   confere que cada mensagem do hospedeiro — pedidos, modelo, respostas às
//!   consultas — é a gravada. Não precisa de VM: só do SDK (as bibliotecas)
//!   e do `pub get` do caso (o `package:json`), que o CI faz antes dos
//!   testes `#[ignore]`.
//! * [`vm_executa_a_macro_e_bate_com_o_cfe`] (`#[ignore]`) — o executor de
//!   materialização: a mesma API numa VM Dart 3.6.2 (DARTFORGE_DART_SDK) com
//!   o `pub get` do caso. `DARTFORGE_GRAVAR_SESSAO=1` regrava a sessão.
use dartforge_elements::load::load_lenient_gerados;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::executor::{CanalGravado, CanalGravador, ExecutorDfexec, ExecutorMacros, sessao_em_texto};
use dartforge_macros_host::modelo::Vista;
use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sdk() -> Option<SdkLayout> {
    let lib = SdkLayout::discover()?;
    let mut sdk = SdkLayout::load(&lib, "dartdevc").ok()?;
    sdk.versao_corrente = LanguageVersion::new(3, 6);
    sdk.experimentos = vec![Feature::Macros];
    Some(sdk)
}

struct Caso {
    dir: PathBuf,
    entrada: PathBuf,
    config: PathBuf,
}

fn caso(nome: &str) -> Option<Caso> {
    let dir = raiz().join("corpus/macros").join(nome);
    let config = dir.join(".dart_tool/package_config.json");
    config.is_file().then(|| Caso { entrada: dir.join("main.dart"), config, dir })
}

/// Roda o hospedeiro inteiro com `executor` e devolve o texto de cada
/// biblioteca aumentada.
fn aplicar(c: &Caso, sdk: &SdkLayout, executor: &mut dyn ExecutorMacros) -> Vec<(PathBuf, String)> {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(&c.entrada, sdk, Some(&c.config), &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut carregar = |i: &mut Interner, g| load_lenient_gerados(&c.entrada, sdk, Some(&c.config), i, None, None, g);
    let saida = match dartforge_macros_host::aplicar(p, &mut nomes, None, &mut carregar, executor) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    };
    saida.textos.into_iter().map(|t| (t.caminho, t.texto)).collect()
}

fn conferir(c: &Caso, textos: &[(PathBuf, String)]) {
    assert!(!textos.is_empty(), "nenhuma augmentation montada");
    for (caminho, texto) in textos {
        let nome = caminho.file_name().unwrap().to_string_lossy().replace(".macro.dart", ".augmentation.dart");
        let esperado = std::fs::read_to_string(c.dir.join("esperado").join(&nome)).expect("golden do CFE");
        if *texto != esperado {
            let linha = texto.lines().zip(esperado.lines()).position(|(a, b)| a != b).unwrap_or(0);
            panic!(
                "{nome}: difere do CFE 3.6.2 na linha {}\n  nosso: {:?}\n  CFE:   {:?}\n--- nosso ---\n{texto}",
                linha + 1,
                texto.lines().nth(linha),
                esperado.lines().nth(linha)
            );
        }
    }
}

#[test]
#[ignore = "exige o SDK (DARTFORGE_SDK_LIB) e o pub get de corpus/macros"]
fn sessao_gravada_reproduz_o_texto_do_cfe() {
    let (Some(sdk), Some(c)) = (sdk(), caso("410_json_codable")) else {
        eprintln!("sem SDK ou sem o pub get de corpus/macros/410_json_codable: pulado");
        return;
    };
    let gravada = std::fs::read_to_string(c.dir.join("esperado/sessao.dfexec")).expect("sessão gravada");
    let mut executor = ExecutorDfexec::novo(CanalGravado::de_texto(&gravada).unwrap());
    let textos = aplicar(&c, &sdk, &mut executor);
    assert_eq!(executor.canal().restante(), 0, "sobrou sessão gravada sem consumir");
    conferir(&c, &textos);
}

#[test]
#[ignore = "exige o SDK Dart 3.6.2 (DARTFORGE_DART_SDK) e o pub get de corpus/macros"]
fn vm_executa_a_macro_e_bate_com_o_cfe() {
    let dart = std::env::var_os("DARTFORGE_DART_SDK")
        .map(PathBuf::from)
        .or_else(|| SdkLayout::discover().and_then(|l| l.parent().map(Path::to_path_buf)))
        .map(|s| s.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }))
        .expect("DARTFORGE_DART_SDK");
    let (Some(sdk), Some(c)) = (sdk(), caso("410_json_codable")) else {
        panic!("sem SDK ou sem o pub get de corpus/macros/410_json_codable");
    };
    let apps = {
        let mut nomes = Interner::new();
        let (p, _) = load_lenient_gerados(&c.entrada, &sdk, Some(&c.config), &mut nomes, None, None, None);
        dartforge_macros_host::aplicacoes::detectar(&Vista { program: &p, interner: &nomes })
    };
    assert_eq!(apps.len(), 4, "{apps:?}");
    let trabalho = tempfile::tempdir().unwrap();
    let cfg = dartforge_macros_host::vm::ConfigDaVm {
        dart,
        api: raiz().join("pacotes/macros"),
        trabalho: trabalho.path().to_path_buf(),
    };
    let vm = dartforge_macros_host::vm::iniciar(&cfg, &apps, Some(&c.config)).unwrap();
    let canal = CanalGravador { interno: vm.into_canal(), sessao: Vec::new() };
    let mut executor = ExecutorDfexec::novo(canal);
    let textos = aplicar(&c, &sdk, &mut executor);
    if std::env::var_os("DARTFORGE_GRAVAR_SESSAO").is_some() {
        let s = sessao_em_texto(&executor.canal().sessao);
        std::fs::write(c.dir.join("esperado/sessao.dfexec"), s).unwrap();
    }
    conferir(&c, &textos);
}
