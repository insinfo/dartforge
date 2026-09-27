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
use dartforge_elements::unidades::CacheUnidades;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::Saida;
use dartforge_macros_host::cache::CacheDeMacros;
use dartforge_macros_host::executor::{
    CanalGravado, CanalGravador, Disponibilidade, ExecutorDfexec, ExecutorMacros, PedidoDeExecucao, ServicoDeConsultas, sessao_em_texto,
};
use dartforge_macros_host::modelo::Vista;
use dartforge_macros_host::montagem::Resultado;
use dartforge_macros_host::protocolo::Apresentacao;
use serde_json::Value;
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

/// Como [`aplicar`], com o cache de expansões e a recarga por diferença.
fn aplicar_com_cache(c: &Caso, sdk: &SdkLayout, executor: &mut dyn ExecutorMacros, cache: &mut CacheDeMacros) -> Saida {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(&c.entrada, sdk, Some(&c.config), &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut carregar =
        |i: &mut Interner, g, u: &mut CacheUnidades| load_lenient_gerados(&c.entrada, sdk, Some(&c.config), i, None, Some(u), g);
    match dartforge_macros_host::aplicar_incremental(p, &mut nomes, None, &mut carregar, executor, Some(cache)) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    }
}

/// Um executor que reprova o teste se for tocado.
struct Proibido;

impl ExecutorMacros for Proibido {
    fn disponibilidade(&self) -> Disponibilidade {
        panic!("o executor não podia ser consultado")
    }
    fn iniciar(&mut self) -> Result<Apresentacao, String> {
        panic!("o executor não podia ser iniciado")
    }
    fn instanciar(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        panic!("o executor não podia instanciar")
    }
    fn executar(&mut self, _: &PedidoDeExecucao, _: &mut dyn ServicoDeConsultas) -> Result<Resultado, String> {
        panic!("o executor não podia executar")
    }
    fn encerrar(&mut self) {
        panic!("o executor não podia ser encerrado")
    }
}

/// O `@JsonCodable` de verdade (sessão gravada) com o cache: a primeira
/// compilação fala com o executor exatamente como a sessão gravada (o cache
/// não muda o protocolo) e a segunda reaproveita as 8 expansões sem tocar
/// executor nenhum; as duas dão o texto do CFE.
#[test]
#[ignore = "exige o SDK (DARTFORGE_SDK_LIB) e o pub get de corpus/macros"]
fn cache_reaproveita_o_json_codable_sem_executor() {
    let (Some(sdk), Some(c)) = (sdk(), caso("410_json_codable")) else {
        eprintln!("sem SDK ou sem o pub get de corpus/macros/410_json_codable: pulado");
        return;
    };
    let gravada = std::fs::read_to_string(c.dir.join("esperado/sessao.dfexec")).expect("sessão gravada");
    let mut executor = ExecutorDfexec::novo(CanalGravado::de_texto(&gravada).unwrap());
    let mut cache = CacheDeMacros::novo("sessão gravada");
    let primeira = aplicar_com_cache(&c, &sdk, &mut executor, &mut cache);
    assert_eq!(executor.canal().restante(), 0, "sobrou sessão gravada sem consumir");
    assert_eq!((primeira.macros_executadas, primeira.medicao.reutilizadas), (8, 0));
    let textos: Vec<_> = primeira.textos.iter().map(|t| (t.caminho.clone(), t.texto.clone())).collect();
    conferir(&c, &textos);

    let segunda = aplicar_com_cache(&c, &sdk, &mut Proibido, &mut cache);
    assert_eq!((segunda.macros_executadas, segunda.medicao.reutilizadas), (0, 8));
    let textos: Vec<_> = segunda.textos.iter().map(|t| (t.caminho.clone(), t.texto.clone())).collect();
    conferir(&c, &textos);
    let ordem = |s: &Saida| s.expansoes.iter().map(|e| (e.aplicacao.clone(), e.fase)).collect::<Vec<_>>();
    assert_eq!(ordem(&primeira), ordem(&segunda));
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

/// O executor **nativo** (D4): o bootstrap compilado pelo `compile-native` do
/// próprio DartForge, sem VM Dart, executa as macros de cada caso do corpus e
/// a augmentation montada é a do CFE 3.6.2, byte a byte.
///
/// Exige um `dartforge` com a feature `nativo` em `DARTFORGE_COMPILADOR_NATIVO`
/// (o `compile-native` dele compila o executor) e o `pub get` de cada caso.
#[test]
#[ignore = "exige DARTFORGE_COMPILADOR_NATIVO (dartforge com a feature nativo), o SDK e o pub get de corpus/macros"]
fn executor_nativo_bate_com_o_cfe() {
    let compilador = PathBuf::from(std::env::var_os("DARTFORGE_COMPILADOR_NATIVO").expect("DARTFORGE_COMPILADOR_NATIVO"));
    let sdk = sdk().expect("SDK (DARTFORGE_SDK_LIB)");
    let mut conferidos = 0;
    for nome in ["410_json_codable", "411_pedido_independente", "412_argumento_posicional", "413_argumentos_nomeados"] {
        let Some(c) = caso(nome) else { panic!("sem o pub get de corpus/macros/{nome}") };
        let apps = {
            let mut nomes = Interner::new();
            let (p, _) = load_lenient_gerados(&c.entrada, &sdk, Some(&c.config), &mut nomes, None, None, None);
            dartforge_macros_host::aplicacoes::detectar(&Vista { program: &p, interner: &nomes })
        };
        assert!(!apps.is_empty(), "{nome}: nenhuma aplicação");
        let trabalho = tempfile::tempdir().unwrap();
        let cfg = dartforge_macros_host::nativo::ConfigNativa {
            compilador: compilador.clone(),
            sdk_lib: sdk.root.clone(),
            api: raiz().join("pacotes/macros"),
            trabalho: trabalho.path().to_path_buf(),
        };
        let t0 = std::time::Instant::now();
        let exe = dartforge_macros_host::nativo::compilar(&cfg, &apps, Some(&c.config)).unwrap();
        let compilacao = t0.elapsed();
        // A segunda vez vem do cache: a mesma chave, sem compilar.
        assert_eq!(dartforge_macros_host::nativo::compilar(&cfg, &apps, Some(&c.config)).unwrap(), exe);
        let mut executor = dartforge_macros_host::nativo::iniciar(&cfg, &apps, Some(&c.config)).unwrap();
        let t1 = std::time::Instant::now();
        let textos = aplicar(&c, &sdk, &mut executor);
        println!("{nome}: compilação {:.1} s, aplicação {:.0} ms", compilacao.as_secs_f64(), t1.elapsed().as_secs_f64() * 1000.0);
        conferir(&c, &textos);
        conferidos += 1;
    }
    assert_eq!(conferidos, 4);
}
