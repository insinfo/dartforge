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
    Err(format!("`dartforge {comando}` exige o backend nativo: compile com `cargo build -p dartforge-cli --features nativo` (configure DARTFORGE_CLANG para o LLVM 22)").into())
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
/// `--excecoes checagem|tabelas`: o modelo de exceções do código gerado
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13). O emissor o lê da variável
/// `DARTFORGE_EXCECOES` (`dartforge_emit_native::alvo::excecoes_por_tabelas`),
/// que também entra na chave do SDK compilado — o programa e o SDK saem
/// sempre no mesmo modelo; a opção só a define, para este processo.
#[allow(unsafe_code)]
fn definir_modelo_de_excecoes(valor: &str) -> Result<(), String> {
    if valor != "checagem" && valor != "tabelas" {
        return Err(format!("--excecoes={valor}: os modelos de exceção são `checagem` e `tabelas`"));
    }
    // SAFETY: a CLI ainda está lendo as opções, na thread principal, antes
    // de criar a thread da compilação: nenhuma outra thread lê o ambiente.
    unsafe { std::env::set_var("DARTFORGE_EXCECOES", valor) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--memoria tracing|arc`: a política de memória do código gerado
/// (docs/ARC-CICLOS-ESPECIFICACAO.md §18.1 e §24; docs/ARC-IMPLEMENTACAO.md).
/// Como [`definir_modelo_de_excecoes`]: o emissor lê `DARTFORGE_MEMORIA`, que
/// entra na chave do SDK compilado.
#[allow(unsafe_code)]
pub(crate) fn definir_memoria(valor: &str) -> Result<(), String> {
    if valor != "tracing" && valor != "arc" {
        return Err(format!("--memoria={valor}: as políticas de memória são `tracing` e `arc`"));
    }
    // SAFETY: a CLI ainda está lendo as opções, na thread principal, antes
    // de criar a thread da compilação: nenhuma outra thread lê o ambiente.
    unsafe { std::env::set_var("DARTFORGE_MEMORIA", valor) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--raizes sombra|mapas`: onde ficam as raízes do coletor no código gerado
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8). Como
/// [`definir_modelo_de_excecoes`]: o emissor lê `DARTFORGE_RAIZES`.
#[allow(unsafe_code)]
fn definir_modo_de_raizes(valor: &str) -> Result<(), String> {
    if valor != "sombra" && valor != "mapas" {
        return Err(format!("--raizes={valor}: os modos de raízes são `sombra` e `mapas`"));
    }
    // SAFETY: a CLI ainda está lendo as opções, na thread principal, antes
    // de criar a thread da compilação: nenhuma outra thread lê o ambiente.
    unsafe { std::env::set_var("DARTFORGE_RAIZES", valor) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--rastro simbolico|nenhum`: o rastro no formato da VM
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14): `simbolico` emite a
/// tabela endereço → (função, linha, coluna) e o `StackTrace` sai com os
/// quadros Dart; `nenhum` (o padrão) não emite tabela. Como
/// [`definir_modelo_de_excecoes`]: o emissor lê `DARTFORGE_RASTRO_VM`.
#[allow(unsafe_code)]
fn definir_rastro(valor: &str) -> Result<(), String> {
    if valor != "simbolico" && valor != "nenhum" {
        return Err(format!("--rastro={valor}: os rastros são `simbolico` e `nenhum`"));
    }
    // SAFETY: a CLI ainda está lendo as opções, na thread principal, antes
    // de criar a thread da compilação: nenhuma outra thread lê o ambiente.
    unsafe { std::env::set_var("DARTFORGE_RASTRO_VM", valor) };
    Ok(())
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
    // A opção da CLI seleciona ARC; um ambiente herdado não muda o padrão.
    definir_memoria("tracing")?;
    let usage = "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--depuracao] [--timings] [--cpu x86-64|x86-64-v2|x86-64-v3|x86-64-v4|native] [--excecoes checagem|tabelas] [--memoria tracing|arc] [--rastro simbolico|nenhum] [--sdk <lib>] [--packages <package_config.json>]";
    if args.len() < 3 {
        return Err(usage.into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let mut optimize = false;
    let mut depuracao = false;
    let mut timings = false;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::gerador::Cpu> = None;
    let mut flags = args[3..].iter();
    while let Some(flag) = flags.next() {
        match flag.to_str() {
            Some("--optimize") => optimize = true,
            Some("--depuracao") => depuracao = true,
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let nome = flags.next().and_then(|v| v.to_str()).ok_or("--cpu exige o nome da CPU")?;
                cpu = Some(dartforge_emit_native::gerador::Cpu::do_nome(nome)?);
            }
            Some("--sdk") => sdk = Some(PathBuf::from(flags.next().ok_or("--sdk exige caminho")?)),
            Some("--packages") => {
                packages = Some(PathBuf::from(flags.next().ok_or("--packages exige caminho")?))
            }
            Some("--memoria") => {
                let valor = flags.next().and_then(|v| v.to_str()).ok_or("--memoria exige tracing ou arc")?;
                definir_memoria(valor)?;
            }
            Some(opcao) if opcao.starts_with("--memoria=") => {
                definir_memoria(&opcao["--memoria=".len()..])?;
            }
            Some("--raizes") => {
                let valor = flags.next().and_then(|v| v.to_str()).ok_or("--raizes exige sombra ou mapas")?;
                definir_modo_de_raizes(valor)?;
            }
            Some(opcao) if opcao.starts_with("--raizes=") => {
                definir_modo_de_raizes(&opcao["--raizes=".len()..])?;
            }
            Some("--excecoes") => {
                let valor = flags.next().and_then(|v| v.to_str()).ok_or("--excecoes exige checagem ou tabelas")?;
                definir_modelo_de_excecoes(valor)?;
            }
            Some(opcao) if opcao.starts_with("--excecoes=") => {
                definir_modelo_de_excecoes(&opcao["--excecoes=".len()..])?;
            }
            Some("--rastro") => {
                let valor = flags.next().and_then(|v| v.to_str()).ok_or("--rastro exige simbolico ou nenhum")?;
                definir_rastro(valor)?;
            }
            Some(opcao) if opcao.starts_with("--rastro=") => {
                definir_rastro(&opcao["--rastro=".len()..])?;
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
            // O motor de build do projeto, se ele usa builders (DF-BUILD-009).
            crate::motor::com_gerador(&input, packages.as_deref(), |gerador| {
                let options = dartforge_emit_native::CompileOptions {
                    sdk: sdk.as_deref(),
                    packages: packages.as_deref(),
                    timings,
                    optimize,
                    versao_linguagem: None,
                    experimentos: Vec::new(),
                    depuracao,
                    gerador,
                    cpu,
                };
                dartforge_emit_native::compilar(&input, &output, &options).map_err(|e| e.to_string())
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação AOT abortou".to_string())??;
    Ok(())
}

#[cfg(feature = "nativo")]
pub fn run_compile_native(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    definir_memoria("tracing")?;
    use dartforge_elements::sdk::Linguagem;
    let usage = "usage: dartforge compile-native <input.dart> -o <output.exe> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--optimize] [--depuracao] [--cpu <cpu>] [--excecoes checagem|tabelas] [--memoria tracing|arc] [--rastro simbolico|nenhum] [--versao-linguagem x.y] [--enable-experiment=a,b]
       dartforge compile-native <input.dart> --emit-ir -o <saida.ll> [--resumo] [...]
       dartforge compile-native <input.dart> --resumo [...]
  --depuracao  tabelas de linha para o depurador nativo (gdb, lldb, Visual Studio)
  --cpu      CPU-alvo: x86-64 (a base, SSE2), x86-64-v2, x86-64-v3, x86-64-v4 ou native
             (o executável só roda em máquinas com ela)
  --raizes   onde ficam as raízes do coletor: sombra (o padrão: o quadro de raízes de
             cada função) ou mapas (mapas de pilha do LLVM, só no Windows x86-64)
  --excecoes modelo de exceções do código gerado: checagem (o padrão: a pendência
             conferida depois de cada chamada) ou tabelas (desenrolamento por
             tabelas, só no Windows x86-64; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
  --rastro   o StackTrace: nenhum (o padrão: o texto fixo de hoje) ou simbolico (os
             quadros Dart no formato da VM, `#0      f (url:linha:coluna)`, pela
             tabela de endereços que o executável leva; §13.14)
  --emit-ir  grava o LLVM IR em -o, sem Clang nem ligação
  --resumo   imprime `<hash de 32 dígitos>  <bytes>` do LLVM IR (o mesmo resumo
             do `dartforge-diferencial determinismo --nativo`), sem Clang";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::gerador::Cpu> = None;
    let mut timings = false;
    let mut optimize = false;
    let mut depuracao = false;
    let mut emit_ir = false;
    let mut resumo = false;
    let mut linguagem = Linguagem::default();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let nome = it.next().and_then(|v| v.to_str()).ok_or("--cpu exige o nome da CPU")?;
                cpu = Some(dartforge_emit_native::gerador::Cpu::do_nome(nome)?);
            }
            Some("--optimize") => optimize = true,
            Some("--depuracao") => depuracao = true,
            Some("--emit-ir") => emit_ir = true,
            Some("--resumo") => resumo = true,
            Some("--memoria") => {
                let valor = it.next().and_then(|v| v.to_str()).ok_or("--memoria exige tracing ou arc")?;
                definir_memoria(valor)?;
            }
            Some(opcao) if opcao.starts_with("--memoria=") => {
                definir_memoria(&opcao["--memoria=".len()..])?;
            }
            Some("--raizes") => {
                let valor = it.next().and_then(|v| v.to_str()).ok_or("--raizes exige sombra ou mapas")?;
                definir_modo_de_raizes(valor)?;
            }
            Some(opcao) if opcao.starts_with("--raizes=") => {
                definir_modo_de_raizes(&opcao["--raizes=".len()..])?;
            }
            Some("--excecoes") => {
                let valor = it.next().and_then(|v| v.to_str()).ok_or("--excecoes exige checagem ou tabelas")?;
                definir_modelo_de_excecoes(valor)?;
            }
            Some(opcao) if opcao.starts_with("--excecoes=") => {
                definir_modelo_de_excecoes(&opcao["--excecoes=".len()..])?;
            }
            Some("--rastro") => {
                let valor = it.next().and_then(|v| v.to_str()).ok_or("--rastro exige simbolico ou nenhum")?;
                definir_rastro(valor)?;
            }
            Some(opcao) if opcao.starts_with("--rastro=") => {
                definir_rastro(&opcao["--rastro=".len()..])?;
            }
            Some("--versao-linguagem" | "--enable-experiment") => {
                let valor = it.next().and_then(|v| v.to_str()).ok_or(usage)?;
                linguagem.ler_opcao(a.to_str().unwrap_or_default(), &mut std::iter::once(valor))?;
            }
            Some(opcao) if opcao.starts_with("--versao-linguagem=")
                || opcao.starts_with("--enable-experiment=") => {
                linguagem.ler_opcao(opcao, &mut std::iter::empty::<&str>())?;
            }
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
        return emitir_ir_nativo(input, out.filter(|_| emit_ir), sdk, packages, timings, resumo, linguagem, depuracao);
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
            crate::motor::com_gerador(&i2, p2.as_deref(), |gerador| {
                let options = dartforge_emit_native::CompileOptions {
                    sdk: s2.as_deref(),
                    packages: p2.as_deref(),
                    timings,
                    optimize,
                    versao_linguagem: linguagem.versao_corrente,
                    experimentos: linguagem.experimentos,
                    depuracao,
                    gerador,
                    cpu,
                };
                dartforge_emit_native::compilar(&i2, &o2, &options).map_err(|e| e.to_string())
            })
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
    linguagem: dartforge_elements::sdk::Linguagem,
    depuracao: bool,
) -> Resultado {
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: sdk.as_deref(),
                packages: packages.as_deref(),
                timings,
                optimize: false,
                versao_linguagem: linguagem.versao_corrente,
                experimentos: linguagem.experimentos,
                depuracao,
                gerador: None,
                cpu: None,
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
