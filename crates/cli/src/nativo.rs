//! Comandos do backend nativo (JIT ORCv2, AOT, ABI), atrás do feature `nativo`.
//!
//! O caminho JavaScript não depende do LLVM: sem o feature, os comandos
//! existem mas explicam como habilitá-los. Isso tira o `llvm-sys` do build
//! padrão do CLI (era ele que custava minutos e exigia `LLVM-C.dll`).
#![cfg_attr(not(feature = "nativo"), allow(unused))]
use std::{fs, path::PathBuf};

type Resultado = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "nativo"))]
fn desabilitado(comando: &str) -> Resultado {
    Err(format!("`dartforge {comando}` exige o backend nativo: compile com `cargo build -p dartforge-cli --features nativo` (precisa do LLVM em D:/LLVM/22.1.8)").into())
}

#[cfg(not(feature = "nativo"))]
pub fn abi_info(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("abi-info") }
#[cfg(not(feature = "nativo"))]
pub fn aot(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("aot") }
#[cfg(not(feature = "nativo"))]
pub fn run_compile_native(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("compile-native") }

#[cfg(feature = "nativo")]
pub fn abi_info(args: &[std::ffi::OsString]) -> Resultado {
        use dartforge_abi::{NativeType, Target};
        if args.len() != 2 {
            return Err("usage: dartforge abi-info <windows-x64|linux-x64|wasm32>".into());
        }
        let target = match args[1].to_str() {
            Some("windows-x64") => Target::WindowsX64,
            Some("linux-x64") => Target::LinuxX64,
            Some("wasm32") => Target::Wasm32,
            _ => return Err("perfil ABI desconhecido".into()),
        };
        let layouts: Vec<_> = [NativeType::Int32, NativeType::Int64, NativeType::Double, NativeType::Pointer]
            .iter().map(|ty| {
                let (size, alignment) = ty.layout(target).expect("tipo escalar");
                serde_json::json!({"native_type":format!("{ty:?}"),"llvm":ty.llvm(),"size":size,"alignment":alignment})
            }).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version":1,"triple":target.triple(),"pointer_bits":target.pointer_bits(),
                "native_type_layouts":layouts,"profile_supports_dynamic_libraries":target.supports_dynamic_libraries(),
                "ffi_execution_implemented":false,"native_static_scalar_bindings_implemented":true,"wasm_emission_implemented":false,
                "note":"FFI completo ainda pendente. AOT host aceita @Native Int32/Int64/Void com objetos ligados explicitamente; não carrega bibliotecas dinamicamente. Perfis não habilitam cross-compilation."
            }))?
        );
        return Ok(());
}

#[cfg(feature = "nativo")]
/// `dartforge aot <entrada.dart> <saida.exe>` — AOT de produção.
///
/// Antes este comando chamava `dartforge_compiler::compile_path_llvm_*`, da
/// trilha velha (`lexer`/`parser`/`hir`/`llvm`), que não é mais dependência do
/// CLI: o crate saiu do `Cargo.toml` e o comando deixou de compilar com a
/// feature `nativo`. O compilador nativo vivo é o `emit_native`
/// (frontend -> elements/types -> HIR própria -> LLVM IR -> Clang), o mesmo do
/// `compile-native`; `aot` passa a ser o apelido de produção dele, para que
/// exista uma única trilha nativa em vez de duas medindo coisas diferentes.
///
/// As opções `--merge-identical-functions` e `--link-object` eram da trilha
/// velha e não têm equivalente aqui; são recusadas explicitamente em vez de
/// aceitas e ignoradas, porque silenciosamente não fazer o que a bandeira diz
/// é pior do que não ter a bandeira.
pub fn aot(args: &[std::ffi::OsString]) -> Resultado {
    let usage = "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--timings] [--sdk <lib>] [--packages <package_config.json>]";
    if args.len() < 3 {
        return Err(usage.into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let mut optimize = false;
    let mut timings = false;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut flags = args[3..].iter();
    while let Some(flag) = flags.next() {
        match flag.to_str() {
            Some("--optimize") => optimize = true,
            Some("--timings") => timings = true,
            Some("--sdk") => sdk = Some(PathBuf::from(flags.next().ok_or("--sdk exige caminho")?)),
            Some("--packages") => {
                packages = Some(PathBuf::from(flags.next().ok_or("--packages exige caminho")?))
            }
            Some("--merge-identical-functions") | Some("--link-object") => {
                return Err(format!(
                    "{} era da trilha velha e não existe no compilador nativo atual",
                    flag.to_string_lossy()
                )
                .into())
            }
            _ => {
                return Err(
                    format!("opção AOT desconhecida: {}", flag.to_string_lossy()).into()
                )
            }
        }
    }

    // Pilha grande: o lowering é recursivo sobre a AST e programas reais
    // estouram a pilha padrão de 8 MiB da thread principal.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: sdk.as_deref(),
                packages: packages.as_deref(),
                timings,
                optimize,
            };
            dartforge_emit_native::compilar(&input, &output, &options)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação AOT abortou".to_string())??;
    Ok(())
}

#[cfg(feature = "nativo")]
pub fn run_compile_native(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: dartforge compile-native <input.dart> -o <output.exe> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--optimize]
       dartforge compile-native <input.dart> --emit-ir -o <saida.ll> [--resumo] [...]
       dartforge compile-native <input.dart> --resumo [...]
  --emit-ir  grava o LLVM IR em -o, sem Clang nem ligação
  --resumo   imprime `<hash de 32 dígitos>  <bytes>` do LLVM IR (o mesmo resumo
             do `dartforge-diferencial determinismo --nativo`), sem Clang";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut optimize = false;
    let mut emit_ir = false;
    let mut resumo = false;

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--optimize") => optimize = true,
            Some("--emit-ir") => emit_ir = true,
            Some("--resumo") => resumo = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let Some(input) = input else {
        return Err(usage.into());
    };
    if emit_ir || resumo {
        if emit_ir && out.is_none() {
            return Err(usage.into());
        }
        return emitir_ir_nativo(input, out.filter(|_| emit_ir), sdk, packages, timings, resumo);
    }
    let Some(out) = out else {
        return Err(usage.into());
    };

    let (i2, o2) = (input.clone(), out.clone());
    let s2 = sdk.clone();
    let p2 = packages.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: s2.as_deref(),
                packages: p2.as_deref(),
                timings,
                optimize,
            };
            dartforge_emit_native::compilar(&i2, &o2, &options)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação nativa abortou".to_string())??;

    Ok(())
}

#[cfg(feature = "nativo")]
/// `compile-native --emit-ir` / `--resumo`: só a emissão, sem Clang nem
/// ligação — o que o teste de determinismo compara, e o que muda quando o
/// emissor muda.
fn emitir_ir_nativo(
    input: PathBuf,
    out: Option<PathBuf>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    timings: bool,
    resumo: bool,
) -> Resultado {
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: sdk.as_deref(),
                packages: packages.as_deref(),
                timings,
                optimize: false,
            };
            dartforge_emit_native::emitir_ir(&input, &options)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a emissão do LLVM IR abortou".to_string())??;
    if timings {
        eprintln!("--- Tempos da Emissão Nativa ---");
        ir.imprimir_tempos();
    }
    if let Some(out) = out {
        fs::write(&out, &ir.texto).map_err(|e| format!("falha ao escrever {}: {e}", out.display()))?;
    }
    if resumo {
        let r = dartforge_emit_native::resumo::ResumoIr::de(&ir.texto);
        println!("{}  {}", r.hex(), r.bytes);
    }
    Ok(())
}

