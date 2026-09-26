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
}

impl Geracao {
    /// O objeto do programa no perfil pedido: `-O0` em desenvolvimento;
    /// em produção com o SDK da fonte, bitcode (`lto`).
    pub fn do_programa(otimizar: bool, lto: bool) -> Self {
        Geracao { otimizar, formato: if lto { Formato::Bitcode } else { Formato::Objeto }, compartilhado: false }
    }

    /// A descrição que entra nas chaves de cache.
    pub fn descricao(&self) -> String {
        format!(
            "{} {}{}",
            if self.otimizar { "O2" } else { "O0" },
            match self.formato {
                Formato::Objeto => "objeto",
                Formato::Bitcode => "bitcode-lto",
            },
            if self.compartilhado { " compartilhado" } else { "" }
        )
    }

    /// As bandeiras do `clang -x ir -c` equivalentes.
    pub fn args_clang(&self) -> Vec<&'static str> {
        let mut args = vec!["-x", "ir", "-c", if self.otimizar { "-O2" } else { "-O0" }];
        // No COFF, zerar o TimeDateStamp: o objeto é função da chave.
        args.extend(crate::alvo::bandeiras_objeto());
        if self.compartilhado {
            args.extend(crate::alvo::bandeiras_objeto_compartilhado());
        }
        if self.formato == Formato::Bitcode {
            args.push("-flto=thin");
        }
        args
    }
}

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
                };
                let bytes = dartforge_llvm::gerar(&nome, ir, &opcoes)?;
                std::fs::write(saida, bytes).map_err(|e| format!("falha ao gravar {}: {e}", saida.display()))
            }
            Gerador::Clang(clang) => {
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
        let g = Geracao { otimizar: true, formato: Formato::Bitcode, compartilhado: false };
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
