//! `dartforge macros`: runs the macro host and shows or **materializes** the
//! augmentation of each library (docs/MACROS-COMPATIBILIDADE.md).
//!
//! The product's executor is the native one (D4, `--native`): the bootstrap
//! compiled by the `compile-native` of this same executable (requires the
//! `native` feature), without a Dart VM (`macros_host::native`). With
//! `--dart`, the same macro runs with our API on a Dart VM (the
//! materialization executor, `macros_host::vm`) — the compatibility path with
//! the official toolchain, not a dependency of the compiler.
use dartforge_elements::sdk::{LanguageOptions, SdkLayout};
use dartforge_intern::Interner;
use dartforge_macros_host::executor::{MacroExecutor, Unavailable};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: dartforge macros <input.dart> [--materialize] [--shape 3.6|current] [--dart <dart executable> | --native] \
[--api <packages/macros>] [--packages <package_config.json>] [--sdk <lib>] [--language-version x.y] [--enable-experiment=a,b]";

/// The shape of the materialized file, by version of the official SDK.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `augment library 'x.dart';` — the library includes it with
    /// `import augment 'x.macro.dart';` (SDK 3.6.2, `--enable-experiment=macros`).
    Sdk36,
    /// `part of 'x.dart';` — the library includes it with `part 'x.macro.dart';`
    /// (SDK 3.13.4, `--enable-experiment=augmentations,enhanced-parts`).
    Current,
}

/// The assembled text with the header of the requested shape (the rest is
/// the same, byte for byte).
fn with_header(text: &str, library: &Path, shape: Shape) -> String {
    let name = library.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let rest = text.split_once('\n').map_or("", |(_, r)| r);
    match shape {
        Shape::Sdk36 => format!("augment library '{name}';\n{rest}"),
        Shape::Current => format!("part of '{name}';\n{rest}"),
    }
}

/// Our macro API: `--api`, `DARTFORGE_MACROS_API`, or `packages/macros` in a
/// directory above the executable or the current directory.
fn find_api(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = explicit.or_else(|| std::env::var_os("DARTFORGE_MACROS_API").map(PathBuf::from)) {
        return Some(p);
    }
    let bases = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    for base in bases.into_iter().flatten() {
        for d in base.ancestors() {
            let p = d.join("packages").join("macros");
            if p.join("pubspec.yaml").is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn run(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let texts: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut it = texts.iter().map(String::as_str);
    let mut input: Option<PathBuf> = None;
    let mut materialize = false;
    let mut shape = Shape::Sdk36;
    let mut dart: Option<PathBuf> = None;
    let mut native = false;
    let mut api: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut sdk_lib: Option<PathBuf> = None;
    let mut language = LanguageOptions::default();
    while let Some(a) = it.next() {
        if language.read_option(a, &mut it)? {
            continue;
        }
        match a {
            "--materialize" => materialize = true,
            "--shape" => {
                shape = match it.next() {
                    Some("3.6") => Shape::Sdk36,
                    Some("current" | "3.13") => Shape::Current,
                    _ => return Err(USAGE.into()),
                }
            }
            "--dart" => dart = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--native" => native = true,
            "--api" => api = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--packages" => packages = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--sdk" => sdk_lib = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(USAGE.into()),
        }
    }
    let input = input.ok_or(USAGE)?;
    let sdk_dir = sdk_lib.or_else(SdkLayout::discover).ok_or("SDK not found (use --sdk)")?;
    let mut sdk = SdkLayout::load(&sdk_dir, "dartdevc")?;
    language.apply(&mut sdk);
    let packages = packages.or_else(|| dartforge_elements::config::PackageConfig::discover(&input));

    let mut names = Interner::new();
    let (program, diags) =
        dartforge_elements::load::load_lenient_generated(&input, &sdk, packages.as_deref(), &mut names, None, None, None);
    if !diags.is_empty() {
        for d in &diags {
            eprintln!("error: {d}");
        }
        return Err(format!("{} error(s) loading the program", diags.len()).into());
    }
    let apps = dartforge_macros_host::applications::detect(&dartforge_macros_host::model::View { program: &program, interner: &names });
    if apps.is_empty() {
        println!("no macro applications");
        return Ok(());
    }
    if native && dart.is_some() {
        return Err("use --dart or --native, not both".into());
    }
    let mut executor: Box<dyn MacroExecutor> = match &dart {
        None if native => {
            if !cfg!(feature = "native") {
                return Err("--native requires dartforge built with the nativo feature (compile-native compiles the executor)".into());
            }
            let api = find_api(api).ok_or("the macro API (packages/macros) was not found: use --api")?;
            let root = input.parent().unwrap_or(Path::new("."));
            let cfg = dartforge_macros_host::native::NativeConfig {
                compiler: std::env::current_exe()?,
                sdk_lib: sdk.root.clone(),
                api,
                work_dir: root.join(".dart_tool").join("dartforge").join("macros").join("nativo"),
            };
            Box::new(dartforge_macros_host::native::start(&cfg, &apps, packages.as_deref())?)
        }
        Some(d) => {
            let api = find_api(api).ok_or("the macro API (packages/macros) was not found: use --api")?;
            let root = input.parent().unwrap_or(Path::new("."));
            let cfg = dartforge_macros_host::vm::VmConfig {
                dart: d.clone(),
                api,
                work_dir: root.join(".dart_tool").join("dartforge").join("macros"),
            };
            Box::new(dartforge_macros_host::vm::start(&cfg, &apps, packages.as_deref())?)
        }
        None => Box::new(Unavailable::default()),
    };
    let mut load = |i: &mut Interner, g| {
        dartforge_elements::load::load_lenient_generated(&input, &sdk, packages.as_deref(), i, None, None, g)
    };
    let output = match dartforge_macros_host::apply(program, &mut names, None, &mut load, executor.as_mut()) {
        Ok(s) => s,
        Err(ds) => {
            for d in &ds {
                eprintln!("error: {d}");
            }
            return Err(format!("{} error(s) in the macros", ds.len()).into());
        }
    };
    for a in &output.warnings {
        eprintln!("warning: {a}");
    }
    for t in &output.texts {
        let library = t.path.with_file_name(
            t.path.file_name().map(|n| n.to_string_lossy().replace(".macro.dart", ".dart")).unwrap_or_default(),
        );
        if materialize {
            let text = with_header(&t.text, &library, shape);
            std::fs::write(&t.path, text).map_err(|e| format!("{}: {e}", t.path.display()))?;
            let name = t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let directive = match shape {
                Shape::Sdk36 => format!("import augment '{name}';"),
                Shape::Current => format!("part '{name}';"),
            };
            println!("{} ({}; include `{directive}` in {})", t.path.display(), t.library, library.display());
        } else {
            println!("// {} ({})", t.path.display(), t.library);
            print!("{}", t.text);
        }
    }
    eprintln!("{} application(s), {} macro run(s)", apps.len(), output.executed_macros);
    Ok(())
}
