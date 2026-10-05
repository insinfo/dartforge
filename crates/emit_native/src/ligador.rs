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
pub const ARQUIVOS_DO_SYSROOT: &[&str] = &[
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
    // O carregador dinâmico (`__tls_get_addr`…), que o `libc.so` de
    // desenvolvimento traz como `AS_NEEDED`.
    CARREGADOR,
];

/// O arquivo do carregador dinâmico do hospedeiro.
const CARREGADOR: &str = if cfg!(target_arch = "aarch64") { "ld-linux-aarch64.so.1" } else { "ld-linux-x86-64.so.2" };

/// O triple do sysroot deste hospedeiro.
pub fn triple_do_sysroot() -> &'static str {
    if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" }
}

/// O carregador dinâmico do hospedeiro e a emulação do `ld.lld`.
fn carregador_e_emulacao() -> (&'static str, &'static str) {
    if cfg!(target_arch = "aarch64") {
        ("/lib/ld-linux-aarch64.so.1", "aarch64linux")
    } else {
        ("/lib64/ld-linux-x86-64.so.2", "elf_x86_64")
    }
}

/// Onde estão os arquivos de [`ARQUIVOS_DO_SYSROOT`].
#[derive(Debug, Clone)]
pub struct SysrootLinux {
    arquivos: Vec<PathBuf>,
}

impl SysrootLinux {
    /// O da distribuição, senão o do sistema (pelo Clang de `clang`).
    pub fn localizar(clang: &Path) -> Result<&'static SysrootLinux, String> {
        static S: std::sync::OnceLock<Result<SysrootLinux, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            if let Some(dir) = dartforge_elements::distribuicao::em_lib(&format!("sysroot/{}", triple_do_sysroot())) {
                return Self::do_diretorio(&dir);
            }
            Self::do_sistema(clang)
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    fn do_diretorio(dir: &Path) -> Result<SysrootLinux, String> {
        let arquivos: Vec<PathBuf> = ARQUIVOS_DO_SYSROOT.iter().map(|a| dir.join(a)).collect();
        if let Some(falta) = arquivos.iter().find(|a| !a.is_file()) {
            return Err(format!("o sysroot da distribuição está incompleto: falta {}", falta.display()));
        }
        Ok(SysrootLinux { arquivos })
    }

    /// Os arquivos que o driver do Clang usaria (`-print-file-name`).
    pub fn do_sistema(clang: &Path) -> Result<SysrootLinux, String> {
        let mut arquivos = Vec::new();
        for a in ARQUIVOS_DO_SYSROOT {
            let saida = Command::new(clang)
                .arg(format!("-print-file-name={a}"))
                .output()
                .map_err(|e| format!("falha ao executar {} para achar {a}: {e}", clang.display()))?;
            let caminho = PathBuf::from(String::from_utf8_lossy(&saida.stdout).trim());
            if !caminho.is_file() {
                return Err(format!(
                    "{a} não encontrado no sistema: instale os pacotes de desenvolvimento da glibc e do GCC, \
                     ou use a distribuição do dartforge (que traz o sysroot de ligação)"
                ));
            }
            arquivos.push(caminho);
        }
        Ok(SysrootLinux { arquivos })
    }

    fn arquivo(&self, nome: &str) -> &Path {
        let i = ARQUIVOS_DO_SYSROOT.iter().position(|a| *a == nome).expect("arquivo do sysroot");
        &self.arquivos[i]
    }

    /// Copia os arquivos para `destino` (o `dartforge empacotar`).
    pub fn copiar_para(&self, destino: &Path) -> Result<(), String> {
        std::fs::create_dir_all(destino).map_err(|e| format!("{}: {e}", destino.display()))?;
        for (nome, de) in ARQUIVOS_DO_SYSROOT.iter().zip(&self.arquivos) {
            std::fs::copy(de, destino.join(nome)).map_err(|e| format!("copiar {}: {e}", de.display()))?;
        }
        Ok(())
    }
}

/// O que se liga.
pub enum Produto<'a> {
    /// Um executável PIE.
    Executavel,
    /// Uma biblioteca compartilhada com o `soname`, exportando (e trazendo
    /// das `staticlib`) os símbolos dados.
    Compartilhada { soname: &'a str, exportados: &'a [String] },
}

/// Uma ligação pelo `ld.lld`.
pub struct Ligacao<'a> {
    pub produto: Produto<'a>,
    /// Objetos e bibliotecas do programa (na ordem), antes das do sistema.
    pub entradas: Vec<PathBuf>,
    /// Procurar as bibliotecas compartilhadas ao lado do executável.
    pub rpath_origem: bool,
    /// LTO dos bitcodes de entrada, com a geração de código em partições.
    pub lto: bool,
    /// A CPU-alvo da geração de código da LTO (`--cpu`, o nome do LLVM);
    /// `None`, a base (`x86-64` no x86-64).
    pub cpu: Option<&'static str>,
    /// Tirar as seções que nada alcança e a tabela de símbolos (produção).
    pub podar: bool,
    /// Manter os símbolos e as seções de depuração mesmo podando (J05).
    pub manter_depuracao: bool,
    pub saida: &'a Path,
}

/// O `ld.lld`: o da distribuição, o ao lado do Clang, ou o do `PATH`.
pub fn ld_lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribuicao::ferramenta_llvm("ld.lld") {
        return l;
    }
    let ao_lado = clang.with_file_name("ld.lld");
    if ao_lado.is_file() { ao_lado } else { PathBuf::from("ld.lld") }
}

/// Liga com o `ld.lld` e o sysroot de ligação.
pub fn ligar(ld: &Path, sysroot: &SysrootLinux, l: &Ligacao<'_>) -> Result<(), String> {
    let mut cmd = comando(ld, sysroot, l)?;
    let saida = cmd.output().map_err(|e| format!("falha ao executar {}: {e}", ld.display()))?;
    if !saida.status.success() {
        let texto = String::from_utf8_lossy(&saida.stderr);
        let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("o ld.lld falhou na ligação ({}):\n{}", saida.status, linhas.join("\n")));
    }
    Ok(())
}

/// O comando do `ld.lld` para a ligação `l`, pronto para rodar (o
/// [`ligar`]) ou para receber mais opções (o `--thinlto-index-only` do
/// ThinLTO distribuído, `lto_distribuida.rs`).
pub fn comando(ld: &Path, sysroot: &SysrootLinux, l: &Ligacao<'_>) -> Result<Command, String> {
    let (carregador, emulacao) = carregador_e_emulacao();
    let mut cmd = Command::new(ld);
    cmd.args(["-z", "relro", "--hash-style=gnu", "--eh-frame-hdr", "-m", emulacao]);
    match &l.produto {
        Produto::Executavel => {
            cmd.args(["-pie", "-dynamic-linker", carregador]);
            cmd.arg(sysroot.arquivo("Scrt1.o"));
        }
        Produto::Compartilhada { soname, .. } => {
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
    cmd.arg("-o").arg(l.saida);
    cmd.arg(sysroot.arquivo("crti.o")).arg(sysroot.arquivo("crtbeginS.o"));
    if let Produto::Compartilhada { exportados, .. } = &l.produto {
        // Cada nome entra como não definido, para o ligador trazer da
        // `staticlib` também o que o SDK não usa (o programa usa); o
        // símbolo sai exportado.
        let mut rsp = String::new();
        for n in exportados.iter() {
            rsp.push_str(&format!("--undefined={n}\n"));
        }
        let arquivo = l.saida.with_extension("rsp");
        std::fs::write(&arquivo, rsp).map_err(|e| format!("{}: {e}", arquivo.display()))?;
        cmd.arg(format!("@{}", arquivo.display()));
    }
    if l.rpath_origem {
        cmd.args(["-rpath", "$ORIGIN"]);
    }
    if l.lto {
        let particoes = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
        let cpu = l.cpu.unwrap_or(if cfg!(target_arch = "x86_64") { "x86-64" } else { "generic" });
        cmd.arg(format!("--lto-O{}", crate::driver::nivel_da_lto())).arg(format!("--lto-partitions={particoes}")).arg(format!("-plugin-opt=mcpu={cpu}"));
        cmd.arg(format!("--thinlto-jobs={}", crate::driver::tarefas_de_geracao()));
        if let Some(d) = crate::driver::cache_do_thinlto() {
            cmd.arg(format!("--thinlto-cache-dir={}", d.display()));
        }
    }
    if l.podar {
        cmd.arg("--gc-sections");
        // O ICF seguro: junta só as funções idênticas cujo endereço ninguém
        // toma (`.llvm_addrsig`), então o `==` de tear-offs continua certo
        // (`ligador_windows.rs`, corpus/nativo 80).
        cmd.arg("--icf=safe");
        // `DARTFORGE_MANTER_SIMBOLOS=1`: a tabela de símbolos fica (perfis).
        if !manter_simbolos() && !l.manter_depuracao {
            cmd.arg("--strip-all");
        }
    }
    if l.podar
        && let Some(m) = mapa_da_ligacao(l.saida)
    {
        cmd.arg(format!("-Map={}", m.display()));
    }
    cmd.args(&l.entradas);
    // As bibliotecas do sistema (o `-lgcc_s -lutil -lrt -lpthread -lm -ldl
    // -lc` do runtime), pelos arquivos do sysroot.
    cmd.arg("--as-needed");
    for b in ["libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1", "libutil.so.1", "libgcc_s.so.1"] {
        cmd.arg(sysroot.arquivo(b));
    }
    cmd.arg("--no-as-needed");
    cmd.arg(sysroot.arquivo("libgcc.a"));
    cmd.arg(sysroot.arquivo("libc.so.6")).arg(sysroot.arquivo("libc_nonshared.a"));
    cmd.arg("--as-needed").arg(sysroot.arquivo(CARREGADOR)).arg("--no-as-needed");
    cmd.arg(sysroot.arquivo("libgcc.a"));
    cmd.arg(sysroot.arquivo("crtendS.o")).arg(sysroot.arquivo("crtn.o"));
    Ok(cmd)
}

/// `DARTFORGE_MAPA_DA_LIGACAO=1`: o mapa da ligação de um executável de
/// produção ao lado dele (`<saída>.map`), com o endereço e o tamanho de cada
/// símbolo depois do LTO — o que `tools/tamanho/quebra.py` lê e o teste de
/// ausência de código confere (docs/NATIVO-PODA-DE-TABELAS.md §3.10).
/// [`pedir_mapas_da_ligacao`] também liga.
pub fn mapa_da_ligacao(saida: &Path) -> Option<PathBuf> {
    let pedido = MAPAS.load(std::sync::atomic::Ordering::Relaxed)
        || std::env::var("DARTFORGE_MAPA_DA_LIGACAO").is_ok_and(|v| v == "1");
    pedido.then(|| saida.with_extension("map"))
}

/// Os mapas pedidos pelo próprio processo (os testes), sem mexer no
/// ambiente.
static MAPAS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Liga [`mapa_da_ligacao`] neste processo.
pub fn pedir_mapas_da_ligacao() {
    MAPAS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// `DARTFORGE_MANTER_SIMBOLOS=1` deixa a tabela de símbolos no executável,
/// para perfilar (`perf`, `callgrind`) o código gerado.
pub fn manter_simbolos() -> bool {
    std::env::var("DARTFORGE_MANTER_SIMBOLOS").is_ok_and(|v| v == "1")
}
