//! `dartforge macros`: roda o hospedeiro de macros e mostra ou **materializa**
//! a augmentation de cada biblioteca (docs/MACROS-COMPATIBILIDADE.md).
//!
//! O executor do produto é o nativo (D4), ainda indisponível; com `--dart`,
//! a mesma macro roda com a nossa API numa VM Dart (o executor de
//! materialização, `macros_host::vm`) — o caminho de compatibilidade com a
//! toolchain oficial, não uma dependência do compilador.
use dartforge_elements::sdk::{Linguagem, SdkLayout};
use dartforge_intern::Interner;
use dartforge_macros_host::executor::{ExecutorMacros, Indisponivel};
use std::path::{Path, PathBuf};

const USO: &str = "usage: dartforge macros <entrada.dart> [--materializar] [--forma 3.6|atual] [--dart <executável dart>] \
[--api <pacotes/macros>] [--packages <package_config.json>] [--sdk <lib>] [--versao-linguagem x.y] [--enable-experiment=a,b]";

/// A forma do arquivo materializado, por versão do SDK oficial.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Forma {
    /// `augment library 'x.dart';` — a biblioteca inclui com
    /// `import augment 'x.macro.dart';` (SDK 3.6.2, `--enable-experiment=macros`).
    Sdk36,
    /// `part of 'x.dart';` — a biblioteca inclui com `part 'x.macro.dart';`
    /// (SDK 3.13.4, `--enable-experiment=augmentations,enhanced-parts`).
    Atual,
}

/// O texto montado com o cabeçalho da forma pedida (o resto é o mesmo, byte
/// a byte).
fn com_cabecalho(texto: &str, biblioteca: &Path, forma: Forma) -> String {
    let nome = biblioteca.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let resto = texto.split_once('\n').map_or("", |(_, r)| r);
    match forma {
        Forma::Sdk36 => format!("augment library '{nome}';\n{resto}"),
        Forma::Atual => format!("part of '{nome}';\n{resto}"),
    }
}

/// A nossa API de macros: `--api`, `DARTFORGE_MACROS_API`, ou `pacotes/macros`
/// num diretório acima do executável ou do diretório corrente.
fn achar_api(explicita: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = explicita.or_else(|| std::env::var_os("DARTFORGE_MACROS_API").map(PathBuf::from)) {
        return Some(p);
    }
    let bases = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    for base in bases.into_iter().flatten() {
        for d in base.ancestors() {
            let p = d.join("pacotes").join("macros");
            if p.join("pubspec.yaml").is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn run(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let textos: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut it = textos.iter().map(String::as_str);
    let mut entrada: Option<PathBuf> = None;
    let mut materializar = false;
    let mut forma = Forma::Sdk36;
    let mut dart: Option<PathBuf> = None;
    let mut api: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut sdk_lib: Option<PathBuf> = None;
    let mut linguagem = Linguagem::default();
    while let Some(a) = it.next() {
        if linguagem.ler_opcao(a, &mut it)? {
            continue;
        }
        match a {
            "--materializar" => materializar = true,
            "--forma" => {
                forma = match it.next() {
                    Some("3.6") => Forma::Sdk36,
                    Some("atual" | "3.13") => Forma::Atual,
                    _ => return Err(USO.into()),
                }
            }
            "--dart" => dart = Some(PathBuf::from(it.next().ok_or(USO)?)),
            "--api" => api = Some(PathBuf::from(it.next().ok_or(USO)?)),
            "--packages" => packages = Some(PathBuf::from(it.next().ok_or(USO)?)),
            "--sdk" => sdk_lib = Some(PathBuf::from(it.next().ok_or(USO)?)),
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            _ => return Err(USO.into()),
        }
    }
    let entrada = entrada.ok_or(USO)?;
    let sdk_dir = sdk_lib.or_else(SdkLayout::discover).ok_or("SDK não encontrado (use --sdk)")?;
    let mut sdk = SdkLayout::load(&sdk_dir, "dartdevc")?;
    linguagem.aplicar(&mut sdk);
    let packages = packages.or_else(|| dartforge_elements::config::PackageConfig::discover(&entrada));

    let mut nomes = Interner::new();
    let (program, diags) =
        dartforge_elements::load::load_lenient_gerados(&entrada, &sdk, packages.as_deref(), &mut nomes, None, None, None);
    if !diags.is_empty() {
        for d in &diags {
            eprintln!("erro: {d}");
        }
        return Err(format!("{} erro(s) ao carregar o programa", diags.len()).into());
    }
    let apps = dartforge_macros_host::aplicacoes::detectar(&dartforge_macros_host::modelo::Vista { program: &program, interner: &nomes });
    if apps.is_empty() {
        println!("nenhuma aplicação de macro");
        return Ok(());
    }
    let mut executor: Box<dyn ExecutorMacros> = match &dart {
        Some(d) => {
            let api = achar_api(api).ok_or("a API de macros (pacotes/macros) não foi encontrada: use --api")?;
            let raiz = entrada.parent().unwrap_or(Path::new("."));
            let cfg = dartforge_macros_host::vm::ConfigDaVm {
                dart: d.clone(),
                api,
                trabalho: raiz.join(".dart_tool").join("dartforge").join("macros"),
            };
            Box::new(dartforge_macros_host::vm::iniciar(&cfg, &apps, packages.as_deref())?)
        }
        None => Box::new(Indisponivel::default()),
    };
    let mut carregar = |i: &mut Interner, g| {
        dartforge_elements::load::load_lenient_gerados(&entrada, &sdk, packages.as_deref(), i, None, None, g)
    };
    let saida = match dartforge_macros_host::aplicar(program, &mut nomes, None, &mut carregar, executor.as_mut()) {
        Ok(s) => s,
        Err(ds) => {
            for d in &ds {
                eprintln!("erro: {d}");
            }
            return Err(format!("{} erro(s) nas macros", ds.len()).into());
        }
    };
    for a in &saida.avisos {
        eprintln!("aviso: {a}");
    }
    for t in &saida.textos {
        let biblioteca = t.caminho.with_file_name(
            t.caminho.file_name().map(|n| n.to_string_lossy().replace(".macro.dart", ".dart")).unwrap_or_default(),
        );
        if materializar {
            let texto = com_cabecalho(&t.texto, &biblioteca, forma);
            std::fs::write(&t.caminho, texto).map_err(|e| format!("{}: {e}", t.caminho.display()))?;
            let nome = t.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let diretiva = match forma {
                Forma::Sdk36 => format!("import augment '{nome}';"),
                Forma::Atual => format!("part '{nome}';"),
            };
            println!("{} ({}; inclua `{diretiva}` em {})", t.caminho.display(), t.biblioteca, biblioteca.display());
        } else {
            println!("// {} ({})", t.caminho.display(), t.biblioteca);
            print!("{}", t.texto);
        }
    }
    eprintln!("{} aplicação(ões), {} execução(ões) de macro", apps.len(), saida.macros_executadas);
    Ok(())
}
