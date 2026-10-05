//! O sistema para o qual o backend nativo compila: o do processo que compila.
//!
//! Tudo o que muda entre Windows, Linux e macOS mora aqui — o cabeçalho do
//! LLVM IR, as extensões dos artefatos, as bandeiras do Clang e da ligação e o
//! formato da biblioteca compartilhada do SDK da fonte. O resto do backend
//! pergunta a este módulo em vez de testar `cfg!` espalhado.
//!
//! Não há compilação cruzada: o alvo é sempre o hospedeiro. O IR no Windows é
//! byte a byte o de antes do porte (mesmo cabeçalho, mesmas bandeiras), para
//! que os resumos de determinismo e as chaves de cache continuem valendo.

/// Os sistemas suportados pelo backend nativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sistema {
    /// COFF, MSVC ABI, `.obj`/`.lib`/`.dll`/`.exe`.
    Windows,
    /// ELF, `.o`/`.a`/`.so`, executável sem extensão.
    Linux,
    /// Mach-O, `.o`/`.a`/`.dylib`, executável sem extensão.
    MacOs,
}

/// O sistema do processo corrente.
///
/// ```
/// use dartforge_emit_native::alvo::{Sistema, sistema};
/// let s = sistema();
/// assert_eq!(s == Sistema::Windows, cfg!(windows));
/// ```
pub const fn sistema() -> Sistema {
    if cfg!(windows) {
        Sistema::Windows
    } else if cfg!(target_os = "macos") {
        Sistema::MacOs
    } else {
        Sistema::Linux
    }
}

/// O modelo de exceções do código gerado
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13), pela variável
/// `DARTFORGE_EXCECOES` (o `--excecoes=` de `compile-native` a define):
///
/// * ausente, vazia ou `checagem`: a exceção é uma pendência do runtime que
///   quem chama confere depois de cada chamada — o IR de sempre, byte a byte;
/// * `tabelas`: a chamada Dart que lança desenrola a pilha até o
///   `landingpad` de quem a trata (`otimizar/tabelas.rs`), com a
///   personalidade do runtime (`dartforge_personalidade`). Só no Windows
///   x86-64, o único alvo com a personalidade escrita, e sem o rastro de
///   funções (`DARTFORGE_RASTRO=1` conta entradas e saídas, e o
///   desenrolamento pula as saídas).
///
/// O programa e o SDK têm de sair no mesmo modelo: a variável entra na chave
/// do SDK compilado (`sdk_modulo::chave_do_sdk`).
///
/// # Errors
/// Valor desconhecido, ou `tabelas` onde o modo não existe.
pub fn excecoes_por_tabelas() -> Result<bool, String> {
    let valor = std::env::var("DARTFORGE_EXCECOES").unwrap_or_default();
    match valor.as_str() {
        "" | "checagem" => Ok(false),
        "tabelas" => {
            // Windows x86-64 (SEH) e, pela personalidade Itanium do
            // runtime (Etapa 4, escrita e nunca executada), Linux e macOS
            // em x86-64 e aarch64.
            let seh = cfg!(windows) && cfg!(target_arch = "x86_64");
            let itanium = cfg!(unix) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64"));
            if !(seh || itanium) {
                return Err(
                    "as exceções por tabelas (--excecoes=tabelas) só existem no Windows x86-64 e, em Linux e macOS, em x86-64 e aarch64".to_string()
                );
            }
            if std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1") {
                return Err("as exceções por tabelas (--excecoes=tabelas) não combinam com DARTFORGE_RASTRO=1".to_string());
            }
            Ok(true)
        }
        outro => Err(format!("DARTFORGE_EXCECOES={outro}: os modelos de exceção são `checagem` e `tabelas`")),
    }
}

/// Onde ficam as raízes do coletor no código gerado
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8), pela variável
/// `DARTFORGE_RAIZES` (o `--raizes=` de `compile-native` a define):
///
/// * ausente, vazia ou `sombra`: o quadro de raízes de cada função (a
///   pilha-sombra) — o IR de sempre, byte a byte;
/// * `mapas`: os valores SSA vivos através de uma chamada saem nos mapas de
///   pilha do LLVM (`rewrite-statepoints-for-gc`), e o coletor os acha
///   percorrendo a pilha nativa. No Windows x86-64 (o percorredor do SEH,
///   `RtlVirtualUnwind`) e, desde a Etapa 4, no Linux e no macOS em x86-64 e
///   aarch64 (o percorredor do `_Unwind_Backtrace`); o Windows arm64 fica na
///   pilha-sombra até um percorredor dele ser testado. Só com o gerador
///   embutido (o `clang` não roda o passe).
///
/// O programa e o SDK têm de sair no mesmo modo: a variável entra na chave
/// do SDK compilado.
///
/// # Errors
/// Valor desconhecido, ou `mapas` onde o modo não existe.
pub fn raizes_por_mapas() -> Result<bool, String> {
    let valor = std::env::var("DARTFORGE_RAIZES").unwrap_or_default();
    match valor.as_str() {
        "" | "sombra" => Ok(false),
        "mapas" => {
            let alvo_com_percorredor = (cfg!(windows) && cfg!(target_arch = "x86_64"))
                || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
            if !alvo_com_percorredor {
                return Err(
                    "as raízes por mapas (--raizes=mapas) existem no Windows x86-64 e no Linux e no macOS em x86-64 e aarch64".to_string(),
                );
            }
            if !crate::gerador::GERADOR_EMBUTIDO || std::env::var("DARTFORGE_GERADOR").is_ok_and(|g| g == "clang") {
                return Err("as raízes por mapas (--raizes=mapas) exigem o gerador embutido do dartforge (o Clang não roda o passe dos mapas)".to_string());
            }
            Ok(true)
        }
        outro => Err(format!("DARTFORGE_RAIZES={outro}: os modos de raízes são `sombra` e `mapas`")),
    }
}

/// O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §13.14), pela variável `DARTFORGE_RASTRO_VM` (o `--rastro=` de
/// `compile-native` a define):
///
/// * ausente, vazia ou `nenhum`: sem tabela, o IR de sempre, e o texto do
///   rastro é o de hoje;
/// * `simbolico`: cada chamada que pode aparecer num rastro ganha um rótulo
///   antes dela e uma entrada na seção `dfpcl` (`llvm/rastro.rs`): o
///   endereço do rótulo, a função (nome e url) e a linha e a coluna. O
///   runtime guarda os endereços de retorno no `throw` e simboliza só quando
///   o texto é pedido.
///
/// Só onde o runtime percorre a pilha nativa (Windows x86-64; Linux e macOS
/// em x86-64 e aarch64). O JIT não emite a tabela. A variável entra na chave
/// do SDK compilado.
///
/// # Errors
/// Valor desconhecido, ou `simbolico` num alvo sem o percorredor.
pub fn rastro_simbolico() -> Result<bool, String> {
    let valor = std::env::var("DARTFORGE_RASTRO_VM").unwrap_or_default();
    match valor.as_str() {
        "" | "nenhum" => Ok(false),
        "simbolico" => {
            let com_percorredor = (cfg!(windows) && cfg!(target_arch = "x86_64"))
                || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
            if !com_percorredor {
                return Err("o rastro simbólico (--rastro=simbolico) existe no Windows x86-64 e no Linux e no macOS em x86-64 e aarch64".to_string());
            }
            Ok(true)
        }
        outro => Err(format!("DARTFORGE_RASTRO_VM={outro}: os rastros são `simbolico` e `nenhum`")),
    }
}

/// O teto do orçamento do `rewrite-statepoints-for-gc` por função
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4): `vivos × pontos de
/// coleta`. Acima dele a função fica na pilha-sombra (o quadro residual,
/// §3.7). `DARTFORGE_ORCAMENTO_MAPAS=<n>` muda o teto (entra na chave do SDK
/// compilado); o padrão, 250 000, é 500 valores enraizados através de 500
/// pontos.
pub fn orcamento_dos_mapas() -> u64 {
    std::env::var("DARTFORGE_ORCAMENTO_MAPAS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(250_000)
}

/// O build de conferência do percurso por mapas (§15.2, E2.5):
/// `DARTFORGE_RAIZES_CONFERIR=1` na compilação, com `--raizes=mapas`. Cada
/// função `gc` grava também, antes de cada ponto de coleta, os vivos dele
/// em slots do quadro da pilha-sombra; com `DARTFORGE_GC_PERCURSO=conferir`
/// na execução, o runtime exige que o percurso por mapas os tenha visitado.
pub fn conferir_raizes() -> bool {
    std::env::var("DARTFORGE_RAIZES_CONFERIR").is_ok_and(|v| v == "1")
}

/// Uma sabotagem de teste ligada na compilação (`DARTFORGE_SABOTAGEM=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 e §14.7). Cada teste
/// dirigido do modo mapas tem de falhar com a dele; um teste que não falha
/// com a sabotagem não conta. As do emissor:
///
/// * `folha:<extern>`: declara a extern `"gc-leaf-function"` mesmo coletando
///   (o mapa perde os registros das chamadas a ela);
/// * `pouso_sem_topo`: o pouso de um `invoke` não restaura o topo da
///   pilha-sombra;
/// * `sem_uso_ficticio`: nenhum `llvm.fake.use` depois dos pontos de coleta
///   (os operandos e os vivos saem do mapa);
/// * `bruto_no_mapa`: um valor bruto par (4098) vivo como raiz em toda
///   função `gc`. O valor tem a forma de um handle de objeto (resto 2 por 8,
///   `layout::e_objeto`) e não aponta para bloco nenhum: a validação de
///   handle do coletor o recusa.
///
/// A variável entra na chave do SDK compilado.
pub fn sabotagem(nome: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').any(|s| s.trim() == nome)
}

/// A extern `nome` está na sabotagem `folha:<nome>`.
pub fn folha_sabotada(nome: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').any(|s| s.trim().strip_prefix("folha:") == Some(nome))
}

/// O cabeçalho `target datalayout`/`target triple` do módulo.
///
/// Windows e Linux x86-64 fixam as strings exatas que a `LLJIT` do processo
/// usa (o JIT recusa um módulo com alvo diferente do seu). Nos demais
/// hospedeiros (macOS, Linux aarch64) o módulo sai sem cabeçalho e o Clang e
/// a `LLJIT` impõem o do hospedeiro: o triple do macOS carrega a versão do
/// sistema (`arm64-apple-darwin24.1.0`), que não cabe numa constante.
pub fn cabecalho_ir() -> &'static str {
    match sistema() {
        Sistema::Windows => {
            "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n\n"
        }
        Sistema::Linux if cfg!(target_arch = "x86_64") => {
            "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\n"
        }
        _ => "",
    }
}

/// A seção dos objetos estáticos da imagem (os literais de string,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.11), só de leitura: `.dfimg$m` no COFF
/// (o ligador junta `.dfimg$a`, `$m` e `$z` em ordem), `dfimg` no ELF (um nome
/// de identificador C, para o ligador definir `__start_dfimg`/`__stop_dfimg`) e
/// `__DATA_CONST,__dfimg` no Mach-O.
pub const fn secao_da_imagem() -> &'static str {
    match sistema() {
        Sistema::Windows => ".dfimg$m",
        Sistema::Linux => "dfimg",
        Sistema::MacOs => "__DATA_CONST,__dfimg",
    }
}

/// Os marcadores de início e fim da seção dos estáticos de uma imagem
/// ([`secao_da_imagem`]): as declarações de nível de módulo que os definem (ou
/// os referenciam) e os dois operandos `ptr`. Cada módulo que registra a imagem
/// os escreve; no COFF são `linkonce_odr` em `comdat` (uma cópia por imagem), e
/// no ELF e no Mach-O o ligador define os símbolos de início e fim de seção — a
/// âncora garante que a seção exista mesmo sem literal nenhum.
pub fn marcadores_da_imagem() -> (String, &'static str, &'static str) {
    match sistema() {
        Sistema::Windows => (
            "$df.img.a = comdat any\n$df.img.z = comdat any\n\
             @df.img.a = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$a\", comdat, align 8\n\
             @df.img.z = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$z\", comdat, align 8\n"
                .to_string(),
            "@df.img.a",
            "@df.img.z",
        ),
        Sistema::Linux => (
            "$df.img.ancora = comdat any\n\
             @df.img.ancora = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"dfimg\", comdat, align 8\n\
             @__start_dfimg = external hidden global i8\n@__stop_dfimg = external hidden global i8\n"
                .to_string(),
            "@__start_dfimg",
            "@__stop_dfimg",
        ),
        Sistema::MacOs => (
            "@df.img.ancora = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"__DATA_CONST,__dfimg\", align 8\n\
             @\"\\01section$start$__DATA_CONST$__dfimg\" = external hidden global i8\n\
             @\"\\01section$end$__DATA_CONST$__dfimg\" = external hidden global i8\n"
                .to_string(),
            "@\"\\01section$start$__DATA_CONST$__dfimg\"",
            "@\"\\01section$end$__DATA_CONST$__dfimg\"",
        ),
    }
}

/// Se o formato de objeto tem `comdat`. O Mach-O não tem: lá a definição
/// `linkonce_odr` sozinha vira símbolo fraco, e o ligador fica com uma.
pub const fn tem_comdat() -> bool {
    !matches!(sistema(), Sistema::MacOs)
}

/// Extensão do objeto (sem o ponto).
pub const fn ext_objeto() -> &'static str {
    match sistema() {
        Sistema::Windows => "obj",
        _ => "o",
    }
}

/// Extensão da biblioteca estática (sem o ponto).
pub const fn ext_estatica() -> &'static str {
    match sistema() {
        Sistema::Windows => "lib",
        _ => "a",
    }
}

/// Nome do arquivo da biblioteca compartilhada `base` (`base.dll`,
/// `libbase.so`, `libbase.dylib`).
///
/// ```
/// let n = dartforge_emit_native::alvo::nome_compartilhada("dfsdk_0");
/// assert!(n.contains("dfsdk_0"));
/// ```
pub fn nome_compartilhada(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.dll"),
        Sistema::Linux => format!("lib{base}.so"),
        Sistema::MacOs => format!("lib{base}.dylib"),
    }
}

/// Nome do executável `base` (`base.exe` no Windows, `base` nos outros).
pub fn nome_executavel(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.exe"),
        _ => base.to_string(),
    }
}

/// Nome do Clang na pasta `bin` de uma distribuição do LLVM.
pub const fn nome_clang() -> &'static str {
    match sistema() {
        Sistema::Windows => "clang.exe",
        _ => "clang",
    }
}

/// Bandeiras do Clang que dependem do formato de objeto, para todo IR.
///
/// No Windows, `-mno-incremental-linker-compatible` zera o `TimeDateStamp` do
/// cabeçalho COFF: sem ele, o mesmo IR dava objetos diferentes no byte 4
/// (medido), e o objeto deixava de ser função da chave. ELF e Mach-O não têm
/// carimbo de tempo.
pub const fn bandeiras_objeto() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &["-mno-incremental-linker-compatible"],
        _ => &[],
    }
}

/// Bandeiras do Clang para os objetos que vão para a biblioteca
/// compartilhada do SDK: código independente de posição fora do Windows.
pub const fn bandeiras_objeto_compartilhado() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &[],
        _ => &["-fPIC"],
    }
}

/// Bibliotecas do sistema que o runtime (uma `staticlib` do Rust) exige na
/// ligação, no formato do driver do Clang.
///
/// É a lista que o `rustc --print native-static-libs` imprime para cada
/// hospedeiro. A ligação do AOT não passa mais pelo driver do Clang (cada
/// sistema tem o seu ligador: `ligador.rs`, `ligador_windows.rs`,
/// `ligador_macos.rs`); a lista serve a quem liga pelo Clang numa árvore de
/// desenvolvimento (os testes do JIT).
pub const fn bibliotecas_do_sistema() -> &'static [&'static str] {
    match sistema() {
        // `crypt32`: o repositório de certificados do sistema (as raízes da
        // TLS, `tls.rs`).
        Sistema::Windows => &["-lws2_32", "-luserenv", "-lntdll", "-liphlpapi", "-lbcrypt", "-ladvapi32", "-lcrypt32", "-lkernel32"],
        Sistema::Linux => &["-lgcc_s", "-lutil", "-lrt", "-lpthread", "-lm", "-ldl", "-lc"],
        // `CoreFoundation`: o `Platform.localeName` (io_plataforma.rs);
        // `Security`: o chaveiro do sistema (as raízes da TLS, `tls.rs`).
        Sistema::MacOs => &["-lSystem", "-lc", "-lm", "-liconv", "-framework", "CoreFoundation", "-framework", "Security"],
    }
}

/// A raiz do SDK do macOS numa árvore de desenvolvimento: o `SDKROOT` do
/// ambiente ou o SDK que o `xcrun` indica (consultado uma vez). A
/// distribuição não a usa: leva os `.tbd` no sysroot de ligação
/// (`ligador_macos.rs`). `None` fora do macOS ou sem Xcode.
pub fn raiz_do_sdk_macos() -> Option<&'static std::path::Path> {
    static RAIZ: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    RAIZ.get_or_init(|| {
        if sistema() != Sistema::MacOs {
            return None;
        }
        if let Some(r) = std::env::var_os("SDKROOT").filter(|r| !r.is_empty()) {
            return Some(std::path::PathBuf::from(r));
        }
        let saida = std::process::Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().ok()?;
        let caminho = String::from_utf8(saida.stdout).ok()?;
        let caminho = caminho.trim();
        (saida.status.success() && !caminho.is_empty()).then(|| std::path::PathBuf::from(caminho))
    })
    .as_deref()
}

/// Os argumentos de ligação que o sistema exige em toda ligação do Clang
/// (executável, DLL do SDK, runtime): as bibliotecas do sistema e, no
/// macOS, a raiz do SDK (`-isysroot`), sem a qual nem o `ld` da Apple
/// chamado pelo Clang do LLVM nem o `ld64.lld` acham `-lSystem`.
pub fn argumentos_de_ligacao() -> Vec<std::ffi::OsString> {
    let mut v: Vec<std::ffi::OsString> = Vec::new();
    if let Some(raiz) = raiz_do_sdk_macos() {
        v.push("-isysroot".into());
        v.push(raiz.as_os_str().to_owned());
    }
    v.extend(bibliotecas_do_sistema().iter().map(std::ffi::OsString::from));
    v
}
