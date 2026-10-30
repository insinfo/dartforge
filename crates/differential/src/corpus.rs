//! Enumeration of the corpus programs and reading of each one's header.
//!
//! Headers recognized in the first lines of the entry file (line comments, in
//! any order):
//!
//! * `// @dart=x.y` — the **official** language version marker: it applies
//!   to the VM, to DDC and to DartForge, with nothing of the harness in
//!   between;
//! * `// requer-dart: x.y` — the **current** version the program requires:
//!   the oracle is the smallest configured SDK with version ≥ x.y, and
//!   DartForge receives `--language-version x.y`. Without it, 3.6 (the floor;
//!   `corpus/js`);
//! * `// erro-of-compilacao` — negative program: the VM, DDC and DartForge
//!   must **reject it**, and the report compares the line of the first error;
//! * `// experiments: a,b` — `--enable-experiment=a,b` in all three;
//! * `// diverge-ddc: reason` — the web prints, by definition, something else;
//! * `// diverge-hosts: reason` — the WebAssembly hosts (the browser and the
//!   executor) print, by definition of the host ABI, different things: the
//!   browser cross-check (`DARTFORGE_BROWSER`) does not apply.
//!
//! A `PENDENTES` file in the corpus directory lists (one per line) the
//! programs of features not implemented yet: they fail without failing the
//! run, and passing without leaving the list fails it (the list only shrinks).

use std::path::{Path, PathBuf};

use dartforge_frontend::LanguageVersion;

/// A corpus program: a standalone `x.dart` or a directory with `main.dart`.
#[derive(Debug, Clone)]
pub struct Program {
    /// Display name (`01_print`, `113_imports_prefixo`).
    pub name: String,
    /// File with `main`.
    pub entry: PathBuf,
    /// Alternative entry for DartForge only; the VM and DDC keep `entry`.
    pub forge_entry: Option<PathBuf>,
    /// Every `.dart` that makes up the program (entry included), for the cache hash.
    pub files: Vec<PathBuf>,
    /// Reason declared in the header (`// diverge-ddc: …`) when the program,
    /// by definition, prints different things on the VM and on the web.
    pub diverge_ddc: Option<String>,
    /// Reason declared in the header (`// diverge-hosts: …`) when the browser
    /// and the executor print different things by definition.
    pub diverge_hosts: Option<String>,
    /// Reason declared in the header (`// ddc-wrong: …`) when `dartdevc` has a
    /// known defect on the program (it prints something else or throws) and the VM
    /// is right: the reference stays the VM, and DDC's failure does not invalidate
    /// the oracle — the harness checks that the defect still happens.
    pub ddc_wrong: Option<String>,
    /// `// requer-dart: x.y`: the current version the program runs in (3.6 without the header).
    pub requires: LanguageVersion,
    /// The entry's `// @dart=x.y`, when present.
    pub marker: Option<LanguageVersion>,
    /// `// erro-of-compilacao`: the program must be rejected.
    pub compile_error: bool,
    /// `// experiments: a,b`.
    pub experiments: Vec<String>,
    /// Listed in `PENDENTES`: a feature not implemented in DartForge yet.
    pub pending: bool,
}

/// What a program's header declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    pub diverge_ddc: Option<String>,
    pub diverge_hosts: Option<String>,
    pub ddc_wrong: Option<String>,
    pub requires: Option<LanguageVersion>,
    pub compile_error: bool,
    pub experiments: Vec<String>,
}

/// Reads the harness headers in the first lines of the file.
pub fn read_header(source: &str) -> Header {
    let mut c = Header::default();
    for l in source.lines().take(20) {
        let Some(rest) = l.trim().strip_prefix("//") else {
            continue;
        };
        let rest = rest.trim_start();
        if let Some(m) = rest.strip_prefix("diverge-ddc:") {
            c.diverge_ddc.get_or_insert_with(|| m.trim().to_string());
        } else if let Some(m) = rest.strip_prefix("diverge-hosts:") {
            c.diverge_hosts.get_or_insert_with(|| m.trim().to_string());
        } else if let Some(m) = rest.strip_prefix("ddc-wrong:") {
            c.ddc_wrong.get_or_insert_with(|| m.trim().to_string());
        } else if let Some(v) = rest.strip_prefix("requer-dart:") {
            c.requires = c.requires.or(LanguageVersion::parse(v));
        } else if rest.trim_end() == "erro-de-compilacao" {
            c.compile_error = true;
        } else if let Some(e) = rest.strip_prefix("experimentos:") {
            c.experiments.extend(
                e.split(',')
                    .map(str::trim)
                    .filter(|x| !x.is_empty())
                    .map(str::to_string),
            );
        }
    }
    c
}

impl Program {
    /// A program with the headers read from `entry`.
    pub fn new(name: String, entry: PathBuf, files: Vec<PathBuf>) -> Program {
        let source = std::fs::read_to_string(&entry).unwrap_or_default();
        let c = read_header(&source);
        let forge_entry = entry.parent().and_then(|dir| {
            let manifest = std::fs::read_to_string(dir.join("dartforge-entrada.txt")).ok()?;
            let rel = manifest
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))?;
            Some(dir.join(rel))
        });
        Program {
            name,
            entry,
            forge_entry,
            files,
            diverge_ddc: c.diverge_ddc,
            diverge_hosts: c.diverge_hosts,
            ddc_wrong: c.ddc_wrong,
            requires: c.requires.unwrap_or(LanguageVersion::FLOOR),
            marker: dartforge_frontend::features::version_marker(&source).map(|m| m.0),
            compile_error: c.compile_error,
            experiments: c.experiments,
            pending: false,
        }
    }

    /// Synthetic program, without a file (harness tests).
    pub fn test(name: &str) -> Program {
        Program {
            name: name.to_string(),
            entry: PathBuf::from("x.dart"),
            forge_entry: None,
            files: vec![],
            diverge_ddc: None,
            diverge_hosts: None,
            ddc_wrong: None,
            requires: LanguageVersion::FLOOR,
            marker: None,
            compile_error: false,
            experiments: vec![],
            pending: false,
        }
    }

    /// Concatenated contents of all the files (basis of the cache hash).
    pub fn contents(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for a in &self.files {
            v.extend_from_slice(a.to_string_lossy().as_bytes());
            v.push(0);
            v.extend(std::fs::read(a).unwrap_or_default());
            v.push(0);
        }
        v
    }

    /// The program's directory (where `dart run` and `dartdevc` are run).
    pub fn dir(&self) -> &Path {
        self.entry.parent().unwrap_or(Path::new("."))
    }

    /// The materialized copy feeds only the DartForge backends.
    pub fn dartforge_entry(&self) -> &Path {
        self.forge_entry.as_deref().unwrap_or(&self.entry)
    }

    pub fn dartforge_dir(&self) -> &Path {
        self.dartforge_entry().parent().unwrap_or(Path::new("."))
    }

    /// Reference for DartForge: the VM, except when the header declares a
    /// divergence — then the reference is DDC, because the web is the real
    /// target.
    pub fn reference_is_ddc(&self) -> bool {
        self.diverge_ddc.is_some()
    }

    /// DDC is expected to differ from the VM: a declared web divergence or a known
    /// DDC defect.
    pub fn expects_ddc_difference(&self) -> bool {
        self.diverge_ddc.is_some() || self.ddc_wrong.is_some()
    }
}

/// Reads the `// diverge-ddc: reason` marker in the first lines of the file.
pub fn read_diverge_ddc(source: &str) -> Option<String> {
    read_header(source).diverge_ddc
}

/// The names listed in `<dir>/PENDENTES` (empty lines and `#` comments ignored).
pub fn read_pending(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("PENDENTES"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Lists the programs in `dir`: direct `*.dart` files and subdirectories with
/// `main.dart`, in alphabetical order. An optional filter selects by a
/// substring of the name.
pub fn list(dir: &Path, filter: Option<&str>) -> Vec<Program> {
    let mut programs = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return programs;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_file() && p.extension().is_some_and(|e| e == "dart") {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            programs.push(Program::new(name, p.clone(), vec![p]));
        } else if p.is_dir() {
            let entry = p.join("main.dart");
            if !entry.is_file() {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let mut files = vec![entry.clone()];
            collect_dart_files(&p, &mut files);
            files.sort();
            files.dedup();
            programs.push(Program::new(name, entry, files));
        }
    }
    let pending = read_pending(dir);
    for p in &mut programs {
        p.pending = pending.contains(&p.name);
    }
    if let Some(f) = filter {
        programs.retain(|p| p.name.contains(f));
    }
    // Numeric order by the prefix (`10_` before `100_`), then by name.
    programs.sort_by(|a, b| {
        number_prefix(&a.name)
            .cmp(&number_prefix(&b.name))
            .then_with(|| a.name.cmp(&b.name))
    });
    programs
}

/// Parses `K/N` (shard K of N, counted from 1) — the argument of `--shard`.
pub fn parse_shard(text: &str) -> Result<(usize, usize), String> {
    let error = || format!("--shard expects K/N with 1 <= K <= N, got `{text}`");
    let (k, n) = text.split_once('/').ok_or_else(error)?;
    let k: usize = k.trim().parse().map_err(|_| error())?;
    let n: usize = n.trim().parse().map_err(|_| error())?;
    if n == 0 || k == 0 || k > n {
        return Err(error());
    }
    Ok((k, n))
}

/// Shard `k` of `n` of the list (already in [`list`] order): the programs at
/// index `i` with `i % n == k - 1`.
///
/// Partitioning by remainder, and not by contiguous blocks, because a
/// program's cost follows its theme (the numeric prefix): blocks would give
/// one shard all of `async` and another only literals. The `n` shards cover
/// the list without repeating any program, and each one keeps the original
/// order — which is what lets CI run the native corpus on `n` machines and
/// add up the reports.
pub fn shard(programs: Vec<Program>, k: usize, n: usize) -> Vec<Program> {
    assert!(n > 0 && (1..=n).contains(&k), "invalid shard {k}/{n}");
    programs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % n == k - 1)
        .map(|(_, p)| p)
        .collect()
}

fn number_prefix(name: &str) -> u32 {
    name.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(u32::MAX)
}

fn collect_dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_dart_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "dart") {
            out.push(p);
        }
    }
}

/// A program's theme by the numeric prefix of its name (organizes
/// CONTRATO-DDC.md). The titles are the text of that Portuguese document and
/// stay as they are.
pub fn theme(name: &str) -> &'static str {
    match number_prefix(name) {
        0..=9 => "Literais e strings",
        10..=19 => "Aritmética int/double",
        20..=29 => "Controle de fluxo",
        30..=39 => "Funções e closures",
        40..=59 => "Classes",
        60..=69 => "Coleções e Iterable",
        70..=79 => "Exceções",
        80..=89 => "async/await, Future e Stream",
        90..=99 => "dynamic, cascatas e null-aware",
        100..=109 => "Records e padrões",
        110..=119 => "Extensions, typedef e bibliotecas",
        120..=139 => "Bibliotecas do SDK",
        140..=299 => "Convertidos dos fixtures antigos",
        _ => "Outros",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diverge_header() {
        assert_eq!(
            read_diverge_ddc("// diverge-ddc: 64-bit int\nvoid main() {}"),
            Some("64-bit int".into())
        );
        assert_eq!(read_diverge_ddc("void main() {}"), None);
    }

    #[test]
    fn ddc_defect_header() {
        let h = read_header("// ddc-wrong: wrong capture\nvoid main() {}");
        assert_eq!(h.ddc_wrong, Some("wrong capture".into()));
        assert_eq!(h.diverge_ddc, None);
    }

    #[test]
    fn version_negative_and_experiment_headers() {
        let c = read_header(
            "// @dart=3.6\n// requer-dart: 3.13\n// erro-de-compilacao\n// experimentos: macros, augmentations\nvoid main() {}",
        );
        assert_eq!(c.requires, Some(LanguageVersion::new(3, 13)));
        assert!(c.compile_error);
        assert_eq!(
            c.experiments,
            vec!["macros".to_string(), "augmentations".to_string()]
        );
        assert_eq!(c.diverge_ddc, None);
        assert_eq!(read_header("void main() {}"), Header::default());
    }

    #[test]
    fn themes() {
        assert_eq!(theme("01_print"), "Literais e strings");
        assert_eq!(theme("113_imports"), "Extensions, typedef e bibliotecas");
        assert_eq!(theme("abc"), "Outros");
    }

    #[test]
    fn shards_partition_the_corpus() {
        let p = |name: String| Program::test(&name);
        let all: Vec<Program> = (0..23).map(|i| p(format!("{i:02}_x"))).collect();
        let n = 4;
        let mut seen: Vec<String> = Vec::new();
        for k in 1..=n {
            let f = shard(all.clone(), k, n);
            // Order preserved and balanced sizes (23 = 6 + 6 + 6 + 5).
            assert!(f.windows(2).all(|w| w[0].name < w[1].name));
            assert!(f.len() == 5 || f.len() == 6, "shard {k}: {}", f.len());
            seen.extend(f.into_iter().map(|p| p.name));
        }
        seen.sort();
        let expected: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        assert_eq!(
            seen, expected,
            "the union of the shards is the corpus, without repetition"
        );
        assert_eq!(shard(all.clone(), 1, 1).len(), 23);
        assert_eq!(shard(all, 2, 4)[0].name, "01_x");
    }

    #[test]
    fn shard_argument() {
        assert_eq!(parse_shard("3/8"), Ok((3, 8)));
        assert!(parse_shard("0/8").is_err());
        assert!(parse_shard("9/8").is_err());
        assert!(parse_shard("1/0").is_err());
        assert!(parse_shard("3").is_err());
        assert!(parse_shard("a/b").is_err());
    }
}
