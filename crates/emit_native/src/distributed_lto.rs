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
//!    (`<entry>.thinlto.bc`): o que importar de quem;
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
//! 1 e 2 é o de `E:\dftemp\spec-maps\dtlto` (dois arquivos C); a escala
//! (as dezenas de partes de um programa grande) não foi medida.

use crate::object_cache::{self, ObjectCache};
use crate::gerador::Cpu;
use crate::ligador_windows as lw;
use std::path::{Path, PathBuf};
use std::process::Command;

/// O que a ligação distribuída recebe.
pub struct Request<'a> {
    pub clang: &'a Path,
    /// Todas as entradas da ligação, na ordem do ligador: as partes do
    /// programa, os módulos do SDK e a biblioteca do runtime.
    pub entries: &'a [PathBuf],
    pub cpu: Option<Cpu>,
    /// Onde ficam os arquivos intermediários.
    pub staging: &'a Path,
    pub cache: Option<&'static ObjectCache>,
}

/// Se o arquivo começa como bitcode do LLVM (cru ou no envoltório).
fn is_bitcode(path: &Path) -> bool {
    use std::io::Read;
    let mut start = [0u8; 4];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    if f.read_exact(&mut start).is_err() {
        return false;
    }
    start == *b"BC\xC0\xDE" || start == [0xDE, 0xC0, 0x17, 0x0B]
}

/// O índice que o `lld-link /thinlto-index-only` grava para uma entrada.
fn index_of(entry: &Path) -> PathBuf {
    let mut name = entry.as_os_str().to_os_string();
    name.push(".thinlto.bc");
    PathBuf::from(name)
}

fn failure_of(statement: &str, exit: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&exit.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&exit.stdout));
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
    format!("{statement} falhou ({}):\n{}", exit.status, lines.join("\n"))
}

/// As entradas da ligação final: cada bitcode trocado pelo objeto nativo
/// dele (com o mapa compacto), as demais como vieram, na mesma ordem.
///
/// # Erros
/// O `lld-link` não gerou os índices, o Clang recusou uma parte, ou o
/// passe dos mapas, o verificador ou o gerador de código falharam nela.
pub fn objects(p: &Request<'_>) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(p.staging).map_err(|e| format!("{}: {e}", p.staging.display()))?;
    let bitcodes: Vec<bool> = p.entries.iter().map(|e| is_bitcode(e)).collect();
    if !bitcodes.iter().any(|b| *b) {
        return Ok(p.entries.to_vec());
    }
    // Um diretório só desta ligação. O ligador grava o índice de cada
    // entrada ao lado dela, e os bitcodes do SDK são de todas as compilações:
    // duas ligações ao mesmo tempo (o harness com `--jobs`) gravavam o índice
    // uma da outra, e a ligação falhava ou importava errado. Cada entrada
    // bitcode entra por uma ligação física (cópia, noutro volume) aqui
    // dentro, e os intermediários também ficam aqui.
    static SEQUENCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let private = p.staging.join(format!(
        "lto-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&private).map_err(|e| format!("{}: {e}", private.display()))?;
    let mut entries: Vec<PathBuf> = Vec::with_capacity(p.entries.len());
    for (i, e) in p.entries.iter().enumerate() {
        if !bitcodes[i] {
            entries.push(e.clone());
            continue;
        }
        let name = e.file_name().map_or_else(|| format!("e{i}"), |n| format!("e{i}-{}", n.to_string_lossy()));
        let dest = private.join(name);
        if std::fs::hard_link(e, &dest).is_err() {
            std::fs::copy(e, &dest).map_err(|x| format!("{} → {}: {x}", e.display(), dest.display()))?;
        }
        entries.push(dest);
    }
    let p = &Request { clang: p.clang, entries: &entries, cpu: p.cpu, staging: &private, cache: p.cache };
    // 1. Os índices: a ligação inteira, só até a resolução de símbolos. O
    // `lld-link` (Windows) e o `ld.lld` (Linux) aceitam o modo; o `ld64.lld`
    // não (no macOS a produção com mapas usa a LTO do ligador com o passe
    // dos mapas, `ligador_macos.rs`).
    let (fictitious_exit, mut statement) = match crate::alvo::sistema() {
        crate::alvo::Sistema::Windows => {
            let sysroot = lw::SysrootWindows::locate(p.clang)?;
            let lld = lw::lld_link(p.clang);
            let fictitious_exit = p.staging.join("indice-thinlto.exe");
            let mut args = lw::arguments(
                sysroot,
                &lw::Linkage {
                    product: lw::Product::Executable,
                    entries: p.entries.to_vec(),
                    lto: true,
                    cpu: p.cpu.map(Cpu::name),
                    prune: false,
                    debug_info: false,
                    exit: &fictitious_exit,
                },
            );
            args.push("/thinlto-index-only".into());
            let mut c = Command::new(&lld);
            c.args(&args);
            (fictitious_exit, c)
        }
        crate::alvo::Sistema::Linux => {
            let sysroot = crate::ligador::SysrootLinux::locate(p.clang)?;
            let ld = crate::ligador::ld_lld(p.clang);
            let fictitious_exit = p.staging.join("indice-thinlto");
            let mut c = crate::ligador::statement(
                &ld,
                sysroot,
                &crate::ligador::Linkage {
                    product: crate::ligador::Product::Executable,
                    entries: p.entries.to_vec(),
                    origin_rpath: false,
                    lto: true,
                    cpu: p.cpu.map(Cpu::name),
                    prune: false,
                    keep_debug_info: false,
                    exit: &fictitious_exit,
                },
            )?;
            c.arg("--thinlto-index-only");
            (fictitious_exit, c)
        }
        crate::alvo::Sistema::MacOs => {
            return Err("o ThinLTO distribuído não existe no macOS (o ld64.lld não separa os índices)".to_string());
        }
    };
    let exit = statement.output().map_err(|e| format!("falha ao executar o ligador (índices do ThinLTO): {e}"))?;
    if !exit.status.success() {
        return Err(failure_of("o ligador (índices do ThinLTO)", &exit));
    }
    // 2 e 3. O backend do ThinLTO de cada bitcode e o fecho com os mapas.
    let identity = format!("{}|{}", dartforge_llvm::identity()?, object_cache::clang_identity(p.clang)?);
    let level = format!("-O{}", crate::driver::lto_level());
    let cpu_description = p.cpu.map_or(String::new(), Cpu::description);
    let convert_map = crate::gcmap::convert_here();
    let convert_stack_trace = crate::compact_stack_trace::convert_here();
    let keep = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    let n = p.entries.len();
    let next_one = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> = std::sync::Mutex::new((0..n).map(|_| None).collect());
    let one = |i: usize| -> Result<PathBuf, String> {
        let entry = &p.entries[i];
        if !bitcodes[i] {
            return Ok(entry.clone());
        }
        let index = index_of(entry);
        let pos = p.staging.join(format!("parte{i}.pos.bc"));
        let mut statement = Command::new(p.clang);
        statement.arg(&level).args(["-x", "ir"]).arg(entry);
        // Sem índice, o bitcode não tem resumo do ThinLTO (o do gerador
        // embutido): é otimizado sozinho, sem importar nada.
        if index.is_file() {
            let mut option = std::ffi::OsString::from("-fthinlto-index=");
            option.push(index.as_os_str());
            statement.arg(option);
        }
        if let Some(c) = p.cpu {
            statement.arg(c.march());
        }
        statement.args(["-emit-llvm", "-c", "-o"]).arg(&pos);
        let exit = statement.output().map_err(|e| format!("falha ao executar {}: {e}", p.clang.display()))?;
        if !exit.status.success() {
            return Err(failure_of(&format!("o backend do ThinLTO de {}", entry.display()), &exit));
        }
        let bitcode = std::fs::read(&pos).map_err(|e| format!("{}: {e}", pos.display()))?;
        let name = format!("parte{i}");
        let close = |dest: &Path| -> Result<(), String> {
            let (mut object, with_maps) = dartforge_llvm::generate_from_bitcode(&name, &bitcode, p.cpu.map(Cpu::name))?;
            if with_maps && convert_map {
                crate::gcmap::convert(&mut object)?;
            }
            // A tabela do rastro, quando o objeto a tem (a seção é achada pelo
            // nome; sem ela, nada muda).
            if convert_stack_trace {
                crate::compact_stack_trace::convert(&mut object)?;
            }
            std::fs::write(dest, object).map_err(|e| format!("falha ao gravar {}: {e}", dest.display()))
        };
        let object = match p.cache {
            Some(c) => {
                let compact = if convert_map { "mapa-compacto" } else { "mapa-cru" };
                let stack_trace = if convert_stack_trace { "rastro-compacto" } else { "rastro-cru" };
                let key = object_cache::bytes_key(&bitcode, &identity, &["lto-distribuida", compact, stack_trace, cpu_description.as_str()]);
                c.get_or_create(key, close).map(|(o, _)| o)
            }
            None => {
                let o = p.staging.join(format!("parte{i}.{}", crate::alvo::ext_object()));
                close(&o).map(|()| o)
            }
        };
        if !keep {
            let _ = std::fs::remove_file(&pos);
            let _ = std::fs::remove_file(&index);
        }
        object
    };
    let parallel = crate::driver::generation_tasks();
    std::thread::scope(|s| {
        for _ in 0..parallel.min(n) {
            s.spawn(|| loop {
                let i = next_one.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let r = one(i);
                results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    if !keep {
        let _ = std::fs::remove_file(&fictitious_exit);
    }
    let objects: Result<Vec<PathBuf>, String> = results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("a parte {i} da LTO distribuída não foi gerada"))))
        .collect();
    // Com o cache, os objetos moram nele e o diretório desta ligação sai;
    // sem ele, os objetos são daqui e ficam para a ligação final.
    if !keep && p.cache.is_some() {
        let _ = std::fs::remove_dir_all(p.staging);
    }
    objects
}
