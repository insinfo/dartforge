//! A ligação no Linux pelo `ld.lld`, sem o driver do Clang e sem o
//! GCC/`libc6-dev` na máquina de quem usa o dartforge.
//!
//! O driver do Clang só acrescentava à ligação o que a glibc e a libgcc
//! exigem: os objetos de partida (`Scrt1.o`, `crti.o`, `crtbeginS.o`…), o
//! carregador dinâmico e as bibliotecas do sistema. Aqui esses arquivos vêm
//! de um **sysroot de ligação** ([`SysrootLinux`]): o da distribuição
//! (`lib/sysroot/<triple>/`, copiado pelo `dartforge empacotar` da máquina
//! que montou a distribuição), ou, numa árvore de desenvolvimento, os do
//! sistema, localizados pelo `clang -print-file-name`. As bibliotecas
//! compartilhadas do sysroot são só entrada da ligação (os símbolos e as
//! versões); o programa carrega as do sistema ao rodar — e o dartforge roda
//! na mesma glibc mínima da máquina que o montou, então as versões dos
//! símbolos são compatíveis.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Os arquivos do sysroot de ligação, na ordem em que o driver do Clang os
/// passa.
pub const SYSROOT_FILES: &[&str] = &[
    "Scrt1.o",
    "crti.o",
    "crtbeginS.o",
    "crtendS.o",
    "crtn.o",
    "libc.so.6",
    "libc_nonshared.a",
    "libm.so.6",
    "libpthread.so.0",
    "libdl.so.2",
    "librt.so.1",
    "libutil.so.1",
    "libgcc_s.so.1",
    "libgcc.a",
    // O carregador dinâmico (`__tls_get_addr`…), que o `libc.only` de
    // desenvolvimento traz como `AS_NEEDED`.
    LOADER,
];

/// O arquivo do carregador dinâmico do hospedeiro.
const LOADER: &str = if cfg!(target_arch = "aarch64") { "ld-linux-aarch64.so.1" } else { "ld-linux-x86-64.so.2" };

/// O triple do sysroot deste hospedeiro.
pub fn triple_do_sysroot() -> &'static str {
    if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" }
}

/// O carregador dinâmico do hospedeiro e a emulação do `ld.lld`.
fn loader_and_emulation() -> (&'static str, &'static str) {
    if cfg!(target_arch = "aarch64") {
        ("/lib/ld-linux-aarch64.so.1", "aarch64linux")
    } else {
        ("/lib64/ld-linux-x86-64.so.2", "elf_x86_64")
    }
}

/// Onde estão os arquivos de [`SYSROOT_FILES`].
#[derive(Debug, Clone)]
pub struct SysrootLinux {
    files: Vec<PathBuf>,
}

impl SysrootLinux {
    /// O da distribuição, senão o do sistema (pelo Clang de `clang`).
    pub fn locate(clang: &Path) -> Result<&'static SysrootLinux, String> {
        static S: std::sync::OnceLock<Result<SysrootLinux, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            if let Some(dir) = dartforge_elements::distribuicao::em_lib(&format!("sysroot/{}", triple_do_sysroot())) {
                return Self::from_dir(&dir);
            }
            Self::do_sistema(clang)
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    fn from_dir(dir: &Path) -> Result<SysrootLinux, String> {
        let files: Vec<PathBuf> = SYSROOT_FILES.iter().map(|a| dir.join(a)).collect();
        if let Some(missing) = files.iter().find(|a| !a.is_file()) {
            return Err(format!("o sysroot da distribuição está incompleto: falta {}", missing.display()));
        }
        Ok(SysrootLinux { files })
    }

    /// Os arquivos que o driver do Clang usaria (`-print-file-name`).
    pub fn do_sistema(clang: &Path) -> Result<SysrootLinux, String> {
        let mut files = Vec::new();
        for a in SYSROOT_FILES {
            let exit = Command::new(clang)
                .arg(format!("-print-file-name={a}"))
                .output()
                .map_err(|e| format!("falha ao executar {} para achar {a}: {e}", clang.display()))?;
            let path = PathBuf::from(String::from_utf8_lossy(&exit.stdout).trim());
            if !path.is_file() {
                return Err(format!(
                    "{a} não encontrado no sistema: instale os pacotes de desenvolvimento da glibc e do GCC, \
                     ou use a distribuição do dartforge (que traz o sysroot de ligação)"
                ));
            }
            files.push(path);
        }
        Ok(SysrootLinux { files })
    }

    fn file(&self, name: &str) -> &Path {
        let i = SYSROOT_FILES.iter().position(|a| *a == name).expect("arquivo do sysroot");
        &self.files[i]
    }

    /// Copia os arquivos para `dest` (o `dartforge empacotar`).
    pub fn copiar_para(&self, dest: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        for (name, de) in SYSROOT_FILES.iter().zip(&self.files) {
            std::fs::copy(de, dest.join(name)).map_err(|e| format!("copiar {}: {e}", de.display()))?;
        }
        Ok(())
    }
}

/// O que se liga.
pub enum Product<'a> {
    /// Um executável PIE.
    Executable,
    /// Uma biblioteca compartilhada com o `soname`, exportando (e trazendo
    /// das `staticlib`) os símbolos dados.
    Shared { soname: &'a str, exported: &'a [String] },
}

/// Uma ligação pelo `ld.lld`.
pub struct Linkage<'a> {
    pub product: Product<'a>,
    /// Objetos e bibliotecas do programa (na ordem), antes das do sistema.
    pub entries: Vec<PathBuf>,
    /// Procurar as bibliotecas compartilhadas ao lado do executável.
    pub origin_rpath: bool,
    /// LTO dos bitcodes de entrada, com a geração de código em partições.
    pub lto: bool,
    /// A CPU-alvo da geração de código da LTO (`--cpu`, o nome do LLVM);
    /// `None`, a base (`x86-64` no x86-64).
    pub cpu: Option<&'static str>,
    /// Tirar as seções que nada alcança e a tabela de símbolos (produção).
    pub prune: bool,
    /// Manter os símbolos e as seções de depuração mesmo podando (J05).
    pub keep_debug_info: bool,
    pub exit: &'a Path,
}

/// O `ld.lld`: o da distribuição, o ao lado do Clang, ou o do `PATH`.
pub fn ld_lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribuicao::ferramenta_llvm("ld.lld") {
        return l;
    }
    let alongside = clang.with_file_name("ld.lld");
    if alongside.is_file() { alongside } else { PathBuf::from("ld.lld") }
}

/// Liga com o `ld.lld` e o sysroot de ligação.
pub fn link(ld: &Path, sysroot: &SysrootLinux, l: &Linkage<'_>) -> Result<(), String> {
    let mut cmd = statement(ld, sysroot, l)?;
    let exit = cmd.output().map_err(|e| format!("falha ao executar {}: {e}", ld.display()))?;
    if !exit.status.success() {
        let text = String::from_utf8_lossy(&exit.stderr);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("o ld.lld falhou na ligação ({}):\n{}", exit.status, lines.join("\n")));
    }
    Ok(())
}

/// O comando do `ld.lld` para a ligação `l`, pronto para rodar (o
/// [`link`]) ou para receber mais opções (o `--thinlto-index-only` do
/// ThinLTO distribuído, `distributed_lto.rs`).
pub fn statement(ld: &Path, sysroot: &SysrootLinux, l: &Linkage<'_>) -> Result<Command, String> {
    let (loader, emulation) = loader_and_emulation();
    let mut cmd = Command::new(ld);
    cmd.args(["-z", "relro", "--hash-style=gnu", "--eh-frame-hdr", "-m", emulation]);
    match &l.product {
        Product::Executable => {
            cmd.args(["-pie", "-dynamic-linker", loader]);
            cmd.arg(sysroot.file("Scrt1.o"));
        }
        Product::Shared { soname, .. } => {
            cmd.args(["-shared", "-soname", soname]);
            // Raízes por mapas: o índice do mapa compacto (`dfgcm`) aponta
            // cada função por uma relocação relativa de 32 bits, que o
            // `ld.lld` só resolve contra símbolo que não pode ser trocado na
            // carga — numa biblioteca compartilhada, as funções exportadas
            // passam a ligar-se a si mesmas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md,
            // Etapa 4).
            if crate::alvo::raizes_por_mapas().unwrap_or(false) {
                cmd.arg("-Bsymbolic-functions");
            }
        }
    }
    cmd.arg("-o").arg(l.exit);
    cmd.arg(sysroot.file("crti.o")).arg(sysroot.file("crtbeginS.o"));
    if let Product::Shared { exported, .. } = &l.product {
        // Cada nome entra como não definido, para o ligador trazer da
        // `staticlib` também o que o SDK não usa (o programa usa); o
        // símbolo sai exportado.
        let mut rsp = String::new();
        for n in exported.iter() {
            rsp.push_str(&format!("--undefined={n}\n"));
        }
        let file = l.exit.with_extension("rsp");
        std::fs::write(&file, rsp).map_err(|e| format!("{}: {e}", file.display()))?;
        cmd.arg(format!("@{}", file.display()));
    }
    if l.origin_rpath {
        cmd.args(["-rpath", "$ORIGIN"]);
    }
    if l.lto {
        let partitions = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
        let cpu = l.cpu.unwrap_or(if cfg!(target_arch = "x86_64") { "x86-64" } else { "generic" });
        cmd.arg(format!("--lto-O{}", crate::driver::lto_level())).arg(format!("--lto-partitions={partitions}")).arg(format!("-plugin-opt=mcpu={cpu}"));
        cmd.arg(format!("--thinlto-jobs={}", crate::driver::generation_tasks()));
        if let Some(d) = crate::driver::thinlto_cache() {
            cmd.arg(format!("--thinlto-cache-dir={}", d.display()));
        }
    }
    if l.prune {
        cmd.arg("--gc-sections");
        // O ICF seguro: junta só as funções idênticas cujo endereço ninguém
        // toma (`.llvm_addrsig`), então o `==` de tear-offs continua certo
        // (`ligador_windows.rs`, corpus/native 80).
        cmd.arg("--icf=safe");
        // `DARTFORGE_MANTER_SIMBOLOS=1`: a tabela de símbolos fica (perfis).
        if !keep_symbols() && !l.keep_debug_info {
            cmd.arg("--strip-all");
        }
    }
    if l.prune
        && let Some(m) = link_map(l.exit)
    {
        cmd.arg(format!("-Map={}", m.display()));
    }
    cmd.args(&l.entries);
    // As bibliotecas do sistema (o `-lgcc_s -lutil -lrt -lpthread -lm -ldl
    // -lc` do runtime), pelos arquivos do sysroot.
    cmd.arg("--as-needed");
    for b in ["libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1", "libutil.so.1", "libgcc_s.so.1"] {
        cmd.arg(sysroot.file(b));
    }
    cmd.arg("--no-as-needed");
    cmd.arg(sysroot.file("libgcc.a"));
    cmd.arg(sysroot.file("libc.so.6")).arg(sysroot.file("libc_nonshared.a"));
    cmd.arg("--as-needed").arg(sysroot.file(LOADER)).arg("--no-as-needed");
    cmd.arg(sysroot.file("libgcc.a"));
    cmd.arg(sysroot.file("crtendS.o")).arg(sysroot.file("crtn.o"));
    Ok(cmd)
}

/// `DARTFORGE_MAPA_DA_LIGACAO=1`: o mapa da ligação de um executável de
/// produção ao lado dele (`<saída>.map`), com o endereço e o tamanho de cada
/// símbolo depois do LTO — o que `tools/size/quebra.py` lê e o teste de
/// ausência de código confere (docs/NATIVO-PODA-DE-TABELAS.md §3.10).
/// [`request_link_maps`] também liga.
pub fn link_map(exit: &Path) -> Option<PathBuf> {
    let request = MAPS.load(std::sync::atomic::Ordering::Relaxed)
        || std::env::var("DARTFORGE_MAPA_DA_LIGACAO").is_ok_and(|v| v == "1");
    request.then(|| exit.with_extension("map"))
}

/// Os mapas pedidos pelo próprio processo (os testes), sem mexer no
/// ambiente.
static MAPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Liga [`link_map`] neste processo.
pub fn request_link_maps() {
    MAPS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// `DARTFORGE_MANTER_SIMBOLOS=1` deixa a tabela de símbolos no executável,
/// para perfilar (`perf`, `callgrind`) o código gerado.
pub fn keep_symbols() -> bool {
    std::env::var("DARTFORGE_MANTER_SIMBOLOS").is_ok_and(|v| v == "1")
}
