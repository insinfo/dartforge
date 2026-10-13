//! Quem transforma o LLVM IR do AOT em objeto nativo (ou em bitcode para a
//! otimização na ligação).
//!
//! Dois geradores com o mesmo contrato:
//!
//! * **embutido** (feature `llvm-embedded`, o padrão da distribuição): o LLVM
//!   ligado ao próprio dartforge (`crates/llvm`), no processo — sem o Clang
//!   na máquina de quem usa o dartforge, sem processo por módulo e sem
//!   gravar o IR em disco;
//! * **Clang** (`DARTFORGE_GERADOR=clang`, ou uma build sem a feature): o
//!   `clang -x ir -c` de antes.
//!
//! O que se pede de uma geração é uma [`Generation`]; a chave dos caches
//! (objeto do programa, SDK da fonte) leva a [`Gerador::identity`] e a
//! [`Generation::description`], então objetos de um gerador nunca servem ao outro.
//!
//! Sem `-ffunction-sections`: na produção a LTO do `lld` já emite cada
//! função na sua seção, e a biblioteca compartilhada do SDK (desenvolvimento)
//! é ligada inteira.

use std::path::{Path, PathBuf};
use std::process::Command;

/// O formato da saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Objeto nativo do hospedeiro.
    Objeto,
    /// Bitcode, otimizado com o programa inteiro na ligação (produção).
    Bitcode,
}

/// O que se pede de uma geração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation {
    /// `-O2` em vez de `-O0`.
    pub optimize: bool,
    pub format: Format,
    /// O objeto vai para uma biblioteca compartilhada (código independente
    /// de posição; o gerador embutido já o emite fora do Windows).
    pub shared: bool,
    /// A CPU-alvo (`--cpu`); `None`, a base do alvo.
    pub cpu: Option<Cpu>,
    /// O nível da otimização do bitcode antes da ligação (0–2; o ThinLTO
    /// refaz a otimização no módulo de cada parte). Só o do programa muda
    /// (`DARTFORGE_PRELINK_O`, medida); o SDK fica no 2.
    pub prelink: u8,
}

impl Generation {
    /// O objeto do programa no perfil pedido: `-O0` em desenvolvimento;
    /// em produção com o SDK da fonte, bitcode (`lto`).
    pub fn of_program(optimize: bool, lto: bool) -> Self {
        let prelink = std::env::var("DARTFORGE_PRELINK_O").ok().and_then(|v| v.trim().parse::<u8>().ok()).filter(|n| *n <= 2).unwrap_or(2);
        Generation { optimize, format: if lto { Format::Bitcode } else { Format::Objeto }, shared: false, cpu: None, prelink }
    }

    /// A descrição que entra nas chaves de cache.
    pub fn description(&self) -> String {
        // O pipeline trocado para medida (`dartforge_llvm::gerar`) muda o
        // objeto: entra na chave.
        let pipeline = match std::env::var("DARTFORGE_PIPELINE_OBJETO") {
            Ok(p) if self.optimize && self.format == Format::Objeto && !p.is_empty() => format!(" {p}"),
            _ => String::new(),
        };
        format!(
            "{} {}{}{}{pipeline}",
            self.level(),
            match self.format {
                Format::Objeto => "objeto",
                Format::Bitcode => "bitcode-lto",
            },
            if self.shared { " compartilhado" } else { "" },
            self.cpu.map_or(String::new(), |c| format!(" {}", c.description()))
        )
    }

    /// O nível da geração: `-O0` sem otimizar; `-O2` com, salvo o bitcode
    /// com outra [`Generation::prelink`].
    fn level(&self) -> &'static str {
        &self.clang_level()[1..]
    }

    fn clang_level(&self) -> &'static str {
        if !self.optimize {
            return "-O0";
        }
        match (self.format, self.prelink) {
            (Format::Bitcode, 0) => "-O0",
            (Format::Bitcode, 1) => "-O1",
            _ => "-O2",
        }
    }

    /// As bandeiras do `clang -x ir -c` equivalentes.
    pub fn args_clang(&self) -> Vec<&'static str> {
        let mut args = vec!["-x", "ir", "-c", self.clang_level()];
        // No COFF, zerar o TimeDateStamp: o objeto é função da chave.
        args.extend(crate::alvo::object_flags());
        if self.shared {
            args.extend(crate::alvo::shared_object_flags());
        }
        if self.format == Format::Bitcode {
            args.push("-flto=thin");
        }
        if let Some(c) = self.cpu {
            args.push(c.march());
        }
        args
    }
}

/// A CPU-alvo pedida (`--cpu`): sem ela, a base do alvo (no x86-64, a
/// `x86-64` — SSE2 —, a mesma do Clang sem `-march`). Vale para o objeto do
/// programa e, na produção, para a geração de código da LTO (programa e
/// SDK); o runtime (Rust) e a biblioteca do SDK de desenvolvimento
/// continuam na base. O executável gerado com uma CPU acima da base só roda
/// em máquinas que a têm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    /// `x86-64`: SSE2 (a base).
    X86_64,
    /// `x86-64-v2`: + SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT.
    X86_64V2,
    /// `x86-64-v3`: + AVX, AVX2, BMI1/2, FMA, F16C, LZCNT, MOVBE.
    X86_64V3,
    /// `x86-64-v4`: + AVX-512 (F, BW, CD, DQ, VL).
    X86_64V4,
    /// `native`: a CPU desta máquina (a do LLVM, `sys::getHostCPUName`).
    Nativa,
}

impl Cpu {
    /// A CPU de `--cpu <name>`.
    pub fn do_nome(name: &str) -> Result<Cpu, String> {
        let x86 = cfg!(target_arch = "x86_64");
        match name {
            "native" => Ok(Cpu::Nativa),
            "x86-64" if x86 => Ok(Cpu::X86_64),
            "x86-64-v2" if x86 => Ok(Cpu::X86_64V2),
            "x86-64-v3" if x86 => Ok(Cpu::X86_64V3),
            "x86-64-v4" if x86 => Ok(Cpu::X86_64V4),
            _ if x86 => Err(format!("--cpu {name}: use x86-64, x86-64-v2, x86-64-v3, x86-64-v4 ou native")),
            _ => Err(format!("--cpu {name}: neste alvo, só `native`")),
        }
    }

    /// O nome do LLVM (`-march=`/`-mcpu=`).
    pub fn name(self) -> &'static str {
        match self {
            Cpu::X86_64 => "x86-64",
            Cpu::X86_64V2 => "x86-64-v2",
            Cpu::X86_64V3 => "x86-64-v3",
            Cpu::X86_64V4 => "x86-64-v4",
            Cpu::Nativa => "native",
        }
    }

    /// `-march=<cpu>` do Clang.
    pub(crate) fn march(self) -> &'static str {
        match self {
            Cpu::X86_64 => "-march=x86-64",
            Cpu::X86_64V2 => "-march=x86-64-v2",
            Cpu::X86_64V3 => "-march=x86-64-v3",
            Cpu::X86_64V4 => "-march=x86-64-v4",
            Cpu::Nativa => "-march=native",
        }
    }

    /// O que entra nas chaves de cache: `native` leva os recursos da CPU
    /// desta máquina (o cache pode ser copiado para outra).
    pub fn description(self) -> String {
        match self {
            Cpu::Nativa => format!("cpu=native({})", machine_resources()),
            c => format!("cpu={}", c.name()),
        }
    }
}

/// Os recursos de vetor e de bits da CPU desta máquina, para distinguir o
/// `native` de duas máquinas nas chaves de cache.
fn machine_resources() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let mut r: Vec<&str> = Vec::new();
        macro_rules! resources {
            ($($f:tt),*) => { $( if std::arch::is_x86_feature_detected!($f) { r.push($f); } )* };
        }
        resources!("sse4.2", "popcnt", "avx", "avx2", "bmi2", "fma", "f16c", "avx512f", "avx512bw", "avx512vl", "avx512dq", "gfni", "vpclmulqdq");
        r.join(",")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        std::env::consts::ARCH.to_string()
    }
}

/// Se esta build tem o gerador embutido (e não precisa do Clang para gerar
/// objetos).
pub const GERADOR_EMBUTIDO: bool = cfg!(feature = "llvm-embutido");

/// O gerador de uma compilação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gerador {
    /// O LLVM ligado ao dartforge.
    #[cfg(feature = "llvm-embutido")]
    Builtin,
    /// O `clang` indicado.
    Clang(PathBuf),
}

impl Gerador {
    /// O gerador desta build e deste ambiente: o embutido quando a build o
    /// tem, salvo `DARTFORGE_GERADOR=clang`; senão o `clang`.
    pub fn escolher(clang: &Path) -> Gerador {
        #[cfg(feature = "llvm-embutido")]
        if std::env::var("DARTFORGE_GERADOR").map_or(true, |g| g != "clang") {
            return Gerador::Builtin;
        }
        Gerador::Clang(clang.to_path_buf())
    }

    /// O Clang, para o bitcode com o resumo do ThinLTO, quando ele existe;
    /// senão o gerador de sempre ([`Gerador::escolher`]). A distribuição sem
    /// o toolchain do sistema (`dartforge empacotar`, N15/N16) não traz o
    /// Clang: a produção usa o gerador embutido, e o `lld` faz a LTO completa
    /// (mais lenta, o mesmo programa).
    pub fn clang_if_any(clang: &Path) -> Gerador {
        if crate::object_cache::clang_identity(clang).is_ok() {
            Gerador::Clang(clang.to_path_buf())
        } else {
            Gerador::escolher(clang)
        }
    }

    /// Se o bitcode deste gerador leva o resumo do ThinLTO. O embutido emite
    /// bitcode sem resumo (a API C do LLVM não escreve o índice): o `lld`
    /// faz a LTO completa, dividida em partições paralelas na geração de
    /// código.
    pub fn thin_bitcode(&self) -> bool {
        matches!(self, Gerador::Clang(_))
    }

    /// A identidade para as chaves de cache (versão, alvo).
    pub fn identity(&self) -> Result<String, String> {
        match self {
            #[cfg(feature = "llvm-embutido")]
            Gerador::Builtin => dartforge_llvm::identity(),
            Gerador::Clang(clang) => crate::object_cache::clang_identity(clang),
        }
    }

    /// Gera `exit` a partir do IR. Com o Clang, o IR é gravado ao lado da
    /// saída e o Clang roda no diretório dela com nomes relativos, para o
    /// `source_filename` e o `.file` do objeto não carregarem o caminho de
    /// quem compilou (com o cache, o nome é o hash, e o mesmo IR dá o mesmo
    /// objeto byte a byte).
    pub fn gerar(&self, ir: &str, generation: Generation, exit: &Path) -> Result<(), String> {
        match self {
            #[cfg(feature = "llvm-embutido")]
            Gerador::Builtin => {
                let name = exit.file_stem().unwrap_or_default().to_string_lossy();
                let options = dartforge_llvm::Opcoes {
                    optimize: generation.optimize,
                    format: match generation.format {
                        Format::Objeto => dartforge_llvm::Format::Objeto,
                        Format::Bitcode => dartforge_llvm::Format::Bitcode,
                    },
                    cpu: generation.cpu.map(Cpu::name),
                };
                #[allow(unused_mut)]
                let mut bytes = dartforge_llvm::gerar(&name, ir, &options)?;
                // Raízes por mapas: o mapa de pilha do objeto, do formato do
                // LLVM (~98 bytes por registro) para o compacto (`gcmap.rs`).
                // `DARTFORGE_SEM_MAPA_COMPACTO=1` deixa o do LLVM no COFF
                // (medida); no ELF a conversão é obrigatória, e o Mach-O fica
                // no formato do LLVM.
                if generation.format == Format::Objeto && ir.contains(dartforge_llvm::MARCA_DE_GC) && crate::gcmap::convert_here() {
                    crate::gcmap::convert(&mut bytes)?;
                }
                // O rastro simbólico: a tabela de 12 bytes por entrada vira o
                // DFPC (`compact_stack_trace.rs`); `DARTFORGE_RASTRO_CRU=1` a deixa.
                if generation.format == Format::Objeto && ir.contains("@df.pcf.") && crate::compact_stack_trace::convert_here() {
                    crate::compact_stack_trace::convert(&mut bytes)?;
                }
                std::fs::write(exit, bytes).map_err(|e| format!("falha ao gravar {}: {e}", exit.display()))
            }
            Gerador::Clang(clang) => {
                // Raízes por mapas: o `clang -x ir` não roda o
                // `rewrite-statepoints-for-gc`, e o objeto sairia sem mapa —
                // raízes perdidas em silêncio.
                // Em bitcode pode: é a parte de um programa grande na
                // produção, e o passe roda no fecho do ThinLTO distribuído
                // (`distributed_lto.rs`), que é o único caminho que leva
                // esse bitcode ao ligador.
                if ir.contains("gc \"statepoint-example\"") && generation.format != Format::Bitcode {
                    return Err("as raízes por mapas (--raizes=mapas) exigem o gerador embutido do dartforge, não o Clang".to_string());
                }
                let ll = exit.with_extension("ll");
                std::fs::write(&ll, ir).map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll.display()))?;
                let dir = exit.parent().unwrap_or(Path::new("."));
                let result = Command::new(clang)
                    .current_dir(dir)
                    .args(generation.args_clang())
                    .arg(ll.file_name().unwrap_or_default())
                    .arg("-o")
                    .arg(exit.file_name().unwrap_or_default())
                    .output()
                    .map_err(|e| format!("falha ao executar Clang em {clang:?}: {e}"));
                let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
                if !keep_ir {
                    let _ = std::fs::remove_file(&ll);
                }
                let result = result?;
                if !result.status.success() {
                    let text = String::from_utf8_lossy(&result.stderr);
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
                    return Err(format!(
                        "Clang falhou na compilação do IR ({}):\n{}",
                        result.status,
                        lines.join("\n")
                    ));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_distinguishes_profiles() {
        let a = Generation::of_program(false, false).description();
        let b = Generation::of_program(true, false).description();
        let c = Generation::of_program(true, true).description();
        assert!(a != b && b != c && a != c);
    }

    #[test]
    fn clang_flags() {
        let g = Generation { optimize: true, format: Format::Bitcode, shared: false, cpu: None, prelink: 2 };
        assert!(g.args_clang().contains(&"-flto=thin"));
        assert!(Generation::of_program(false, false).args_clang().contains(&"-O0"));
    }

    #[cfg(feature = "llvm-embutido")]
    #[test]
    fn embedded_generates_object() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir.path().join(format!("x.{}", crate::alvo::ext_object()));
        let ir = format!("{}define i64 @f() {{\n  ret i64 7\n}}\n", crate::alvo::cabecalho_ir());
        Gerador::Builtin.gerar(&ir, Generation::of_program(true, false), &obj).unwrap();
        assert!(std::fs::metadata(&obj).unwrap().len() > 0);
    }
}
