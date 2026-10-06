//! O ThinLTO **distribuído** da produção com raízes por mapas
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4 e Etapa 3, E3.1).
//!
//! O `lld-link` não aceita passes extras na LTO dele (`-lto-newpm-passes`
//! é "unknown argument"), e o `rewrite-statepoints-for-gc` tem de rodar
//! **depois** de toda a otimização, inclusive do inlining entre o programa e
//! o SDK que o ThinLTO faz. Então a LTO sai do ligador:
//!
//! 1. `lld-link /thinlto-index-only` resolve os símbolos do programa inteiro
//!    e grava, ao lado de cada bitcode de entrada, o índice dele
//!    (`<entrada>.thinlto.bc`): o que importar de quem;
//! 2. para cada bitcode, `clang -x ir <entrada> -fthinlto-index=<índice>
//!    -emit-llvm -c` roda o backend do ThinLTO (importação e otimização) e
//!    devolve o módulo **em bitcode**, já com o que foi importado embutido;
//! 3. o LLVM embutido aplica o passe dos mapas e o verificador, gera o
//!    objeto, e o conversor troca o `.llvm_stackmaps` pelo mapa compacto
//!    (`gcmap.rs`) — o que tira da produção os ~98 bytes por registro do
//!    mapa cru;
//! 4. quem chama liga os objetos nativos, sem LTO.
//!
//! O cache do ThinLTO do ligador (`/lldltocache`) deixa de valer; o cache é
//! o de objetos, com a chave no bitcode **depois** da importação (o passo
//! 2): duas ligações em que a parte importou as mesmas coisas reaproveitam
//! o passo 3. O passo 2 roda sempre — o índice de uma parte resume o que
//! ela importa, mas não o conteúdo, e uma chave feita só dele entregaria um
//! objeto velho.
//!
//! Escrito sem compilar nem executar: o experimento que sustenta os passos
//! 1 e 2 é o de `E:\dftemp\spec-mapas\dtlto` (dois arquivos C); a escala
//! (as dezenas de partes de um programa grande) não foi medida.

use crate::cache_objeto::{self, CacheObjeto};
use crate::gerador::Cpu;
use crate::ligador_windows as lw;
use std::path::{Path, PathBuf};
use std::process::Command;

/// O que a ligação distribuída recebe.
pub struct Pedido<'a> {
    pub clang: &'a Path,
    /// Todas as entradas da ligação, na ordem do ligador: as partes do
    /// programa, os módulos do SDK e a biblioteca do runtime.
    pub entradas: &'a [PathBuf],
    pub cpu: Option<Cpu>,
    /// Onde ficam os arquivos intermediários.
    pub staging: &'a Path,
    pub cache: Option<&'static CacheObjeto>,
}

/// Se o arquivo começa como bitcode do LLVM (cru ou no envoltório).
fn e_bitcode(caminho: &Path) -> bool {
    use std::io::Read;
    let mut inicio = [0u8; 4];
    let Ok(mut f) = std::fs::File::open(caminho) else { return false };
    if f.read_exact(&mut inicio).is_err() {
        return false;
    }
    inicio == *b"BC\xC0\xDE" || inicio == [0xDE, 0xC0, 0x17, 0x0B]
}

/// O índice que o `lld-link /thinlto-index-only` grava para uma entrada.
fn indice_de(entrada: &Path) -> PathBuf {
    let mut nome = entrada.as_os_str().to_os_string();
    nome.push(".thinlto.bc");
    PathBuf::from(nome)
}

fn falha_de(comando: &str, saida: &std::process::Output) -> String {
    let mut texto = String::from_utf8_lossy(&saida.stderr).into_owned();
    texto.push_str(&String::from_utf8_lossy(&saida.stdout));
    let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
    format!("{comando} falhou ({}):\n{}", saida.status, linhas.join("\n"))
}

/// As entradas da ligação final: cada bitcode trocado pelo objeto nativo
/// dele (com o mapa compacto), as demais como vieram, na mesma ordem.
///
/// # Erros
/// O `lld-link` não gerou os índices, o Clang recusou uma parte, ou o
/// passe dos mapas, o verificador ou o gerador de código falharam nela.
pub fn objetos(p: &Pedido<'_>) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(p.staging).map_err(|e| format!("{}: {e}", p.staging.display()))?;
    let bitcodes: Vec<bool> = p.entradas.iter().map(|e| e_bitcode(e)).collect();
    if !bitcodes.iter().any(|b| *b) {
        return Ok(p.entradas.to_vec());
    }
    // 1. Os índices: a ligação inteira, só até a resolução de símbolos. O
    // `lld-link` (Windows) e o `ld.lld` (Linux) aceitam o modo; o `ld64.lld`
    // não (no macOS a produção com mapas usa a LTO do ligador com o passe
    // dos mapas, `ligador_macos.rs`).
    let (saida_ficticia, mut comando) = match crate::alvo::sistema() {
        crate::alvo::Sistema::Windows => {
            let sysroot = lw::SysrootWindows::localizar(p.clang)?;
            let lld = lw::lld_link(p.clang);
            let saida_ficticia = p.staging.join("indice-thinlto.exe");
            let mut args = lw::argumentos(
                sysroot,
                &lw::Ligacao {
                    produto: lw::Produto::Executavel,
                    entradas: p.entradas.to_vec(),
                    lto: true,
                    cpu: p.cpu.map(Cpu::nome),
                    podar: false,
                    depuracao: false,
                    saida: &saida_ficticia,
                },
            );
            args.push("/thinlto-index-only".into());
            let mut c = Command::new(&lld);
            c.args(&args);
            (saida_ficticia, c)
        }
        crate::alvo::Sistema::Linux => {
            let sysroot = crate::ligador::SysrootLinux::localizar(p.clang)?;
            let ld = crate::ligador::ld_lld(p.clang);
            let saida_ficticia = p.staging.join("indice-thinlto");
            let mut c = crate::ligador::comando(
                &ld,
                sysroot,
                &crate::ligador::Ligacao {
                    produto: crate::ligador::Produto::Executavel,
                    entradas: p.entradas.to_vec(),
                    rpath_origem: false,
                    lto: true,
                    cpu: p.cpu.map(Cpu::nome),
                    podar: false,
                    manter_depuracao: false,
                    saida: &saida_ficticia,
                },
            )?;
            c.arg("--thinlto-index-only");
            (saida_ficticia, c)
        }
        crate::alvo::Sistema::MacOs => {
            return Err("o ThinLTO distribuído não existe no macOS (o ld64.lld não separa os índices)".to_string());
        }
    };
    let saida = comando.output().map_err(|e| format!("falha ao executar o ligador (índices do ThinLTO): {e}"))?;
    if !saida.status.success() {
        return Err(falha_de("o ligador (índices do ThinLTO)", &saida));
    }
    // 2 e 3. O backend do ThinLTO de cada bitcode e o fecho com os mapas.
    let identidade = format!("{}|{}", dartforge_llvm::identidade()?, cache_objeto::identidade_clang(p.clang)?);
    let nivel = format!("-O{}", crate::driver::nivel_da_lto());
    let descricao_da_cpu = p.cpu.map_or(String::new(), Cpu::descricao);
    let converter_mapa = crate::gcmap::converter_aqui();
    let converter_rastro = crate::rastro_compacto::converter_aqui();
    let manter = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    let n = p.entradas.len();
    let proxima = std::sync::atomic::AtomicUsize::new(0);
    let resultados: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> = std::sync::Mutex::new((0..n).map(|_| None).collect());
    let uma = |i: usize| -> Result<PathBuf, String> {
        let entrada = &p.entradas[i];
        if !bitcodes[i] {
            return Ok(entrada.clone());
        }
        let indice = indice_de(entrada);
        let pos = p.staging.join(format!("parte{i}.pos.bc"));
        let mut comando = Command::new(p.clang);
        comando.arg(&nivel).args(["-x", "ir"]).arg(entrada);
        // Sem índice, o bitcode não tem resumo do ThinLTO (o do gerador
        // embutido): é otimizado sozinho, sem importar nada.
        if indice.is_file() {
            let mut opcao = std::ffi::OsString::from("-fthinlto-index=");
            opcao.push(indice.as_os_str());
            comando.arg(opcao);
        }
        if let Some(c) = p.cpu {
            comando.arg(c.march());
        }
        comando.args(["-emit-llvm", "-c", "-o"]).arg(&pos);
        let saida = comando.output().map_err(|e| format!("falha ao executar {}: {e}", p.clang.display()))?;
        if !saida.status.success() {
            return Err(falha_de(&format!("o backend do ThinLTO de {}", entrada.display()), &saida));
        }
        let bitcode = std::fs::read(&pos).map_err(|e| format!("{}: {e}", pos.display()))?;
        let nome = format!("parte{i}");
        let fechar = |destino: &Path| -> Result<(), String> {
            let (mut objeto, com_mapas) = dartforge_llvm::gerar_de_bitcode(&nome, &bitcode, p.cpu.map(Cpu::nome))?;
            if com_mapas && converter_mapa {
                crate::gcmap::converter(&mut objeto)?;
            }
            // A tabela do rastro, quando o objeto a tem (a seção é achada pelo
            // nome; sem ela, nada muda).
            if converter_rastro {
                crate::rastro_compacto::converter(&mut objeto)?;
            }
            std::fs::write(destino, objeto).map_err(|e| format!("falha ao gravar {}: {e}", destino.display()))
        };
        let objeto = match p.cache {
            Some(c) => {
                let compacto = if converter_mapa { "mapa-compacto" } else { "mapa-cru" };
                let rastro = if converter_rastro { "rastro-compacto" } else { "rastro-cru" };
                let chave = cache_objeto::chave_de_bytes(&bitcode, &identidade, &["lto-distribuida", compacto, rastro, descricao_da_cpu.as_str()]);
                c.obter_ou_criar(chave, fechar).map(|(o, _)| o)
            }
            None => {
                let o = p.staging.join(format!("parte{i}.{}", crate::alvo::ext_objeto()));
                fechar(&o).map(|()| o)
            }
        };
        if !manter {
            let _ = std::fs::remove_file(&pos);
            let _ = std::fs::remove_file(&indice);
        }
        objeto
    };
    let paralelas = crate::driver::tarefas_de_geracao();
    std::thread::scope(|s| {
        for _ in 0..paralelas.min(n) {
            s.spawn(|| loop {
                let i = proxima.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let r = uma(i);
                resultados.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    if !manter {
        let _ = std::fs::remove_file(&saida_ficticia);
    }
    resultados
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("a parte {i} da LTO distribuída não foi gerada"))))
        .collect()
}
