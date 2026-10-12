//! Harness de paridade de **projeto** (docs/ANALYZER-ESPECIFICACAO-INFRA.md,
//! III.1): compara a saída inteira do `dartforge analyze` com a do
//! `dart analyze` 3.6.2 gravada, nos três formatos, e o código de saída.
//!
//! Cada caso é uma pasta de `<dir>` com:
//! * `projeto/`: o projeto mínimo (`pubspec.yaml`, fontes, opcionalmente
//!   `analysis_options.yaml` e `.dart_tool/package_config.json`);
//! * `ARGS`: os argumentos depois de `analyze`, um por linha, relativos ao
//!   projeto (sem o arquivo, `.`);
//! * `ESPERADO.default.txt`, `ESPERADO.json.txt`, `ESPERADO.machine.txt` e
//!   `ESPERADO.codigo` (este, opcional).
//!
//! A normalização troca o caminho absoluto do projeto por `<raiz>` (nas duas
//! grafias do JSON, com `\\` e com `/`). `--gravar` roda o `dart` (o do PATH,
//! ou `DARTFORGE_DART`) com o estado do analisador numa pasta temporária de
//! `E:` e regrava os esperados.
//!
use std::path::{Path, PathBuf};
use std::process::Command;

const FORMATOS: [&str; 3] = ["default", "json", "machine"];

/// A saída de um comando: o texto normalizado e o código.
struct Saida {
    texto: String,
    codigo: i32,
}

fn normalizar(texto: &str, raiz: &Path) -> String {
    let absoluto = std::path::absolute(raiz).unwrap_or_else(|_| raiz.to_path_buf());
    let a = absoluto.to_string_lossy().to_string();
    let mut s = texto.replace("\r\n", "\n");
    // JSON: barras invertidas escapadas.
    s = s.replace(&a.replace('\\', "\\\\"), "<raiz>");
    s = s.replace(&a, "<raiz>");
    s = s.replace(&a.replace('\\', "/"), "<raiz>");
    s
}

fn args_do_caso(caso: &Path) -> Vec<String> {
    let v: Vec<String> = std::fs::read_to_string(caso.join("ARGS"))
        .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    if v.is_empty() { vec![".".to_string()] } else { v }
}

/// O executável do `dartforge` ao lado deste (o mesmo `target/<perfil>`).
fn dartforge() -> PathBuf {
    let nome = if cfg!(windows) { "dartforge.exe" } else { "dartforge" };
    std::env::current_exe().ok().map(|e| e.with_file_name(nome)).unwrap_or_else(|| PathBuf::from(nome))
}

fn dart() -> PathBuf {
    std::env::var_os("DARTFORGE_DART").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("dart"))
}

fn rodar(programa: &Path, prefixo: &[&str], formato: &str, args: &[String], projeto: &Path, estado: Option<&Path>) -> Option<Saida> {
    let mut c = Command::new(programa);
    c.args(prefixo).arg(format!("--format={formato}")).args(args).current_dir(projeto);
    if let Some(e) = estado {
        c.env("ANALYZER_STATE_LOCATION_OVERRIDE", e);
    }
    let out = c.output().ok()?;
    Some(Saida { texto: normalizar(&String::from_utf8_lossy(&out.stdout), projeto), codigo: out.status.code().unwrap_or(-1) })
}

/// O separador de caminho do sistema não conta: a barra invertida escapada
/// do JSON e a simples viram `/` dos dois lados.
fn separadores(s: &str) -> String {
    s.replace("\\\\", "/").replace('\\', "/")
}

/// A primeira linha diferente (1-based), com as duas versões.
fn primeira_diferenca(esperado: &str, obtido: &str) -> Option<(usize, String, String)> {
    let (e, o): (Vec<&str>, Vec<&str>) = (esperado.split('\n').collect(), obtido.split('\n').collect());
    for i in 0..e.len().max(o.len()) {
        let (a, b) = (e.get(i).copied().unwrap_or("<fim>"), o.get(i).copied().unwrap_or("<fim>"));
        if a != b {
            return Some((i + 1, a.to_string(), b.to_string()));
        }
    }
    None
}

/// Os casos de `dir`, em ordem.
fn casos(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("projeto").is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Regrava os esperados de cada caso com o `dart analyze`.
pub fn gravar(dir: &Path) -> bool {
    let estado = std::env::temp_dir().join("dartforge-saida-analyze");
    let _ = std::fs::create_dir_all(&estado);
    let mut ok = true;
    for caso in casos(dir) {
        let projeto = caso.join("projeto");
        let args = args_do_caso(&caso);
        let mut codigo = None;
        for f in FORMATOS {
            match rodar(&dart(), &["analyze"], f, &args, &projeto, Some(&estado)) {
                Some(s) => {
                    let _ = std::fs::write(caso.join(format!("ESPERADO.{f}.txt")), &s.texto);
                    codigo = Some(s.codigo);
                }
                None => {
                    eprintln!("{}: o dart não rodou", caso.display());
                    ok = false;
                }
            }
        }
        if let Some(c) = codigo {
            let _ = std::fs::write(caso.join("ESPERADO.codigo"), format!("{c}\n"));
        }
    }
    ok
}

/// Compara cada caso nos três formatos; devolve se tudo bateu.
pub fn comparar(dir: &Path) -> bool {
    let mut tudo = true;
    let todos = casos(dir);
    if todos.is_empty() {
        eprintln!("nenhum caso em {}", dir.display());
        return false;
    }
    for caso in todos {
        let projeto = caso.join("projeto");
        let args = args_do_caso(&caso);
        let nome = caso.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for f in FORMATOS {
            let Ok(esperado) = std::fs::read_to_string(caso.join(format!("ESPERADO.{f}.txt"))) else { continue };
            let esperado = separadores(&esperado.replace("\r\n", "\n"));
            // `--todos`: a saída inteira, sem a regra de publicação (que só
            // deixa no CLI os códigos verificados); é o formato, a ordem, os
            // caminhos e o resumo que se comparam aqui.
            let Some(obtido) = rodar(&dartforge(), &["analyze", "--todos"], f, &args, &projeto, None) else {
                println!("{nome} [{f}]: o dartforge não rodou");
                tudo = false;
                continue;
            };
            if let Some((linha, e, o)) = primeira_diferenca(&esperado, &separadores(&obtido.texto)) {
                println!("{nome} [{f}] linha {linha}:\n  esperado: {e}\n  obtido:   {o}");
                tudo = false;
            }
            if f == "default"
                && let Ok(c) = std::fs::read_to_string(caso.join("ESPERADO.codigo"))
                && let Ok(c) = c.trim().parse::<i32>()
                && c != obtido.codigo
            {
                println!("{nome}: código de saída esperado {c}, obtido {}", obtido.codigo);
                tudo = false;
            }
        }
    }
    if tudo {
        println!("saída de projeto: tudo igual");
    }
    tudo
}
