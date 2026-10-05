//! Quem transforma o LLVM IR do AOT em objeto nativo (ou em bitcode para a
//! otimização na ligação).
//!
//! Dois geradores com o mesmo contrato:
//!
//! * **embutido** (feature `llvm-embutido`, o padrão da distribuição): o LLVM
//!   ligado ao próprio dartforge (`crates/llvm`), no processo — sem o Clang
//!   na máquina de quem usa o dartforge, sem processo por módulo e sem
//!   gravar o IR em disco;
//! * **Clang** (`DARTFORGE_GERADOR=clang`, ou uma build sem a feature): o
//!   `clang -x ir -c` de antes.
//!
//! O que se pede de uma geração é uma [`Geracao`]; a chave dos caches
//! (objeto do programa, SDK da fonte) leva a [`Gerador::identidade`] e a
//! [`Geracao::descricao`], então objetos de um gerador nunca servem ao outro.
//!
//! Sem `-ffunction-sections`: na produção a LTO do `lld` já emite cada
//! função na sua seção, e a biblioteca compartilhada do SDK (desenvolvimento)
//! é ligada inteira.

use std::path::{Path, PathBuf};
use std::process::Command;

/// O formato da saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    /// Objeto nativo do hospedeiro.
    Objeto,
    /// Bitcode, otimizado com o programa inteiro na ligação (produção).
    Bitcode,
}

/// O que se pede de uma geração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geracao {
    /// `-O2` em vez de `-O0`.
    pub otimizar: bool,
    pub formato: Formato,
    /// O objeto vai para uma biblioteca compartilhada (código independente
    /// de posição; o gerador embutido já o emite fora do Windows).
    pub compartilhado: bool,
    /// A CPU-alvo (`--cpu`); `None`, a base do alvo.
    pub cpu: Option<Cpu>,
    /// O nível da otimização do bitcode antes da ligação (0–2; o ThinLTO
    /// refaz a otimização no módulo de cada parte). Só o do programa muda
    /// (`DARTFORGE_PRELINK_O`, medida); o SDK fica no 2.
    pub pre_ligacao: u8,
}

impl Geracao {
    /// O objeto do programa no perfil pedido: `-O0` em desenvolvimento;
    /// em produção com o SDK da fonte, bitcode (`lto`).
    pub fn do_programa(otimizar: bool, lto: bool) -> Self {
        let pre_ligacao = std::env::var("DARTFORGE_PRELINK_O").ok().and_then(|v| v.trim().parse::<u8>().ok()).filter(|n| *n <= 2).unwrap_or(2);
        Geracao { otimizar, formato: if lto { Formato::Bitcode } else { Formato::Objeto }, compartilhado: false, cpu: None, pre_ligacao }
    }

    /// A descrição que entra nas chaves de cache.
    pub fn descricao(&self) -> String {
        // O pipeline trocado para medida (`dartforge_llvm::gerar`) muda o
        // objeto: entra na chave.
        let pipeline = match std::env::var("DARTFORGE_PIPELINE_OBJETO") {
            Ok(p) if self.otimizar && self.formato == Formato::Objeto && !p.is_empty() => format!(" {p}"),
            _ => String::new(),
        };
        format!(
            "{} {}{}{}{pipeline}",
            self.nivel(),
            match self.formato {
                Formato::Objeto => "objeto",
                Formato::Bitcode => "bitcode-lto",
            },
            if self.compartilhado { " compartilhado" } else { "" },
            self.cpu.map_or(String::new(), |c| format!(" {}", c.descricao()))
        )
    }

    /// O nível da geração: `-O0` sem otimizar; `-O2` com, salvo o bitcode
    /// com outra [`Geracao::pre_ligacao`].
    fn nivel(&self) -> &'static str {
        &self.nivel_clang()[1..]
    }

    fn nivel_clang(&self) -> &'static str {
        if !self.otimizar {
            return "-O0";
        }
        match (self.formato, self.pre_ligacao) {
            (Formato::Bitcode, 0) => "-O0",
            (Formato::Bitcode, 1) => "-O1",
            _ => "-O2",
        }
    }

    /// As bandeiras do `clang -x ir -c` equivalentes.
    pub fn args_clang(&self) -> Vec<&'static str> {
        let mut args = vec!["-x", "ir", "-c", self.nivel_clang()];
        // No COFF, zerar o TimeDateStamp: o objeto é função da chave.
        args.extend(crate::alvo::bandeiras_objeto());
        if self.compartilhado {
            args.extend(crate::alvo::bandeiras_objeto_compartilhado());
        }
        if self.formato == Formato::Bitcode {
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
    /// A CPU de `--cpu <nome>`.
    pub fn do_nome(nome: &str) -> Result<Cpu, String> {
        let x86 = cfg!(target_arch = "x86_64");
        match nome {
            "native" => Ok(Cpu::Nativa),
            "x86-64" if x86 => Ok(Cpu::X86_64),
            "x86-64-v2" if x86 => Ok(Cpu::X86_64V2),
            "x86-64-v3" if x86 => Ok(Cpu::X86_64V3),
            "x86-64-v4" if x86 => Ok(Cpu::X86_64V4),
            _ if x86 => Err(format!("--cpu {nome}: use x86-64, x86-64-v2, x86-64-v3, x86-64-v4 ou native")),
            _ => Err(format!("--cpu {nome}: neste alvo, só `native`")),
        }
    }

    /// O nome do LLVM (`-march=`/`-mcpu=`).
    pub fn nome(self) -> &'static str {
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
    pub fn descricao(self) -> String {
        match self {
            Cpu::Nativa => format!("cpu=native({})", recursos_da_maquina()),
            c => format!("cpu={}", c.nome()),
        }
    }
}

/// Os recursos de vetor e de bits da CPU desta máquina, para distinguir o
/// `native` de duas máquinas nas chaves de cache.
fn recursos_da_maquina() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let mut r: Vec<&str> = Vec::new();
        macro_rules! recursos {
            ($($f:tt),*) => { $( if std::arch::is_x86_feature_detected!($f) { r.push($f); } )* };
        }
        recursos!("sse4.2", "popcnt", "avx", "avx2", "bmi2", "fma", "f16c", "avx512f", "avx512bw", "avx512vl", "avx512dq", "gfni", "vpclmulqdq");
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
    Embutido,
    /// O `clang` indicado.
    Clang(PathBuf),
}

impl Gerador {
    /// O gerador desta build e deste ambiente: o embutido quando a build o
    /// tem, salvo `DARTFORGE_GERADOR=clang`; senão o `clang`.
    pub fn escolher(clang: &Path) -> Gerador {
        #[cfg(feature = "llvm-embutido")]
        if std::env::var("DARTFORGE_GERADOR").map_or(true, |g| g != "clang") {
            return Gerador::Embutido;
        }
        Gerador::Clang(clang.to_path_buf())
    }

    /// Se o bitcode deste gerador leva o resumo do ThinLTO. O embutido emite
    /// bitcode sem resumo (a API C do LLVM não escreve o índice): o `lld`
    /// faz a LTO completa, dividida em partições paralelas na geração de
    /// código.
    pub fn bitcode_thin(&self) -> bool {
        matches!(self, Gerador::Clang(_))
    }

    /// A identidade para as chaves de cache (versão, alvo).
    pub fn identidade(&self) -> Result<String, String> {
        match self {
            #[cfg(feature = "llvm-embutido")]
            Gerador::Embutido => dartforge_llvm::identidade(),
            Gerador::Clang(clang) => crate::cache_objeto::identidade_clang(clang),
        }
    }

    /// Gera `saida` a partir do IR. Com o Clang, o IR é gravado ao lado da
    /// saída e o Clang roda no diretório dela com nomes relativos, para o
    /// `source_filename` e o `.file` do objeto não carregarem o caminho de
    /// quem compilou (com o cache, o nome é o hash, e o mesmo IR dá o mesmo
    /// objeto byte a byte).
    pub fn gerar(&self, ir: &str, geracao: Geracao, saida: &Path) -> Result<(), String> {
        match self {
            #[cfg(feature = "llvm-embutido")]
            Gerador::Embutido => {
                let nome = saida.file_stem().unwrap_or_default().to_string_lossy();
                let opcoes = dartforge_llvm::Opcoes {
                    otimizar: geracao.otimizar,
                    formato: match geracao.formato {
                        Formato::Objeto => dartforge_llvm::Formato::Objeto,
                        Formato::Bitcode => dartforge_llvm::Formato::Bitcode,
                    },
                    cpu: geracao.cpu.map(Cpu::nome),
                };
                #[allow(unused_mut)]
                let mut bytes = dartforge_llvm::gerar(&nome, ir, &opcoes)?;
                // Raízes por mapas: o mapa de pilha do objeto, do formato do
                // LLVM (~98 bytes por registro) para o compacto (`gcmap.rs`).
                // `DARTFORGE_SEM_MAPA_COMPACTO=1` deixa o do LLVM no COFF
                // (medida); no ELF a conversão é obrigatória, e o Mach-O fica
                // no formato do LLVM.
                if geracao.formato == Formato::Objeto && ir.contains(dartforge_llvm::MARCA_DE_GC) && crate::gcmap::converter_aqui() {
                    crate::gcmap::converter(&mut bytes)?;
                }
                // O rastro simbólico: a tabela de 12 bytes por entrada vira o
                // DFPC (`rastro_compacto.rs`); `DARTFORGE_RASTRO_CRU=1` a deixa.
                if geracao.formato == Formato::Objeto && ir.contains("@df.pcf.") && crate::rastro_compacto::converter_aqui() {
                    crate::rastro_compacto::converter(&mut bytes)?;
                }
                std::fs::write(saida, bytes).map_err(|e| format!("falha ao gravar {}: {e}", saida.display()))
            }
            Gerador::Clang(clang) => {
                // Raízes por mapas: o `clang -x ir` não roda o
                // `rewrite-statepoints-for-gc`, e o objeto sairia sem mapa —
                // raízes perdidas em silêncio.
                // Em bitcode pode: é a parte de um programa grande na
                // produção, e o passe roda no fecho do ThinLTO distribuído
                // (`lto_distribuida.rs`), que é o único caminho que leva
                // esse bitcode ao ligador.
                if ir.contains("gc \"statepoint-example\"") && geracao.formato != Formato::Bitcode {
                    return Err("as raízes por mapas (--raizes=mapas) exigem o gerador embutido do dartforge, não o Clang".to_string());
                }
                let ll = saida.with_extension("ll");
                std::fs::write(&ll, ir).map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll.display()))?;
                let dir = saida.parent().unwrap_or(Path::new("."));
                let resultado = Command::new(clang)
                    .current_dir(dir)
                    .args(geracao.args_clang())
                    .arg(ll.file_name().unwrap_or_default())
                    .arg("-o")
                    .arg(saida.file_name().unwrap_or_default())
                    .output()
                    .map_err(|e| format!("falha ao executar Clang em {clang:?}: {e}"));
                let manter_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
                if !manter_ir {
                    let _ = std::fs::remove_file(&ll);
                }
                let resultado = resultado?;
                if !resultado.status.success() {
                    let texto = String::from_utf8_lossy(&resultado.stderr);
                    let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
                    return Err(format!(
                        "Clang falhou na compilação do IR ({}):\n{}",
                        resultado.status,
                        linhas.join("\n")
                    ));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn descricao_distingue_perfis() {
        let a = Geracao::do_programa(false, false).descricao();
        let b = Geracao::do_programa(true, false).descricao();
        let c = Geracao::do_programa(true, true).descricao();
        assert!(a != b && b != c && a != c);
    }

    #[test]
    fn bandeiras_do_clang() {
        let g = Geracao { otimizar: true, formato: Formato::Bitcode, compartilhado: false, cpu: None, pre_ligacao: 2 };
        assert!(g.args_clang().contains(&"-flto=thin"));
        assert!(Geracao::do_programa(false, false).args_clang().contains(&"-O0"));
    }

    #[cfg(feature = "llvm-embutido")]
    #[test]
    fn embutido_gera_objeto() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir.path().join(format!("x.{}", crate::alvo::ext_objeto()));
        let ir = format!("{}define i64 @f() {{\n  ret i64 7\n}}\n", crate::alvo::cabecalho_ir());
        Gerador::Embutido.gerar(&ir, Geracao::do_programa(true, false), &obj).unwrap();
        assert!(std::fs::metadata(&obj).unwrap().len() > 0);
    }
}
