//! Enumeração dos programas do corpus e leitura do cabeçalho de cada um.
//!
//! Cabeçalhos reconhecidos nas primeiras linhas da entrada (comentários de
//! linha, na ordem que for):
//!
//! * `// @dart=x.y` — o marcador **oficial** de versão de linguagem: vale
//!   para a VM, para o DDC e para o DartForge, sem nada do harness no meio;
//! * `// requer-dart: x.y` — a versão **corrente** que o programa exige: o
//!   oráculo é o menor SDK configurado com versão ≥ x.y, e o DartForge recebe
//!   `--versao-linguagem x.y`. Sem ele, 3.6 (o piso; `corpus/js`);
//! * `// erro-de-compilacao` — programa negativo: VM, DDC e DartForge têm de
//!   **recusá-lo**, e o relatório compara a linha do primeiro erro;
//! * `// experimentos: a,b` — `--enable-experiment=a,b` nos três;
//! * `// diverge-ddc: motivo` — a web imprime, por definição, outra coisa.
//!
//! Um arquivo `PENDENTES` no diretório do corpus lista (um por linha) os
//! programas de recursos ainda não implementados: falham sem reprovar, e
//! passar sem sair da lista reprova (a lista só encolhe).

use std::path::{Path, PathBuf};

use dartforge_frontend::LanguageVersion;

/// Um programa do corpus: um `x.dart` solto ou um diretório com `main.dart`.
#[derive(Debug, Clone)]
pub struct Programa {
    /// Nome de exibição (`01_print`, `113_imports_prefixo`).
    pub nome: String,
    /// Arquivo com `main`.
    pub entrada: PathBuf,
    /// Todos os `.dart` que compõem o programa (entrada incluída), para o hash do cache.
    pub arquivos: Vec<PathBuf>,
    /// Motivo declarado no cabeçalho (`// diverge-ddc: …`) quando o programa,
    /// por definição, imprime coisas diferentes na VM e na web.
    pub diverge_ddc: Option<String>,
    /// `// requer-dart: x.y`: a versão corrente em que o programa roda (3.6 sem o cabeçalho).
    pub requer: LanguageVersion,
    /// `// @dart=x.y` da entrada, quando há.
    pub marcador: Option<LanguageVersion>,
    /// `// erro-de-compilacao`: o programa tem de ser recusado.
    pub erro_compilacao: bool,
    /// `// experimentos: a,b`.
    pub experimentos: Vec<String>,
    /// Listado em `PENDENTES`: recurso ainda não implementado no DartForge.
    pub pendente: bool,
}

/// O que o cabeçalho de um programa declara.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cabecalho {
    pub diverge_ddc: Option<String>,
    pub requer: Option<LanguageVersion>,
    pub erro_compilacao: bool,
    pub experimentos: Vec<String>,
}

/// Lê os cabeçalhos do harness nas primeiras linhas do arquivo.
pub fn ler_cabecalho(fonte: &str) -> Cabecalho {
    let mut c = Cabecalho::default();
    for l in fonte.lines().take(20) {
        let Some(resto) = l.trim().strip_prefix("//") else { continue };
        let resto = resto.trim_start();
        if let Some(m) = resto.strip_prefix("diverge-ddc:") {
            c.diverge_ddc.get_or_insert_with(|| m.trim().to_string());
        } else if let Some(v) = resto.strip_prefix("requer-dart:") {
            c.requer = c.requer.or(LanguageVersion::parse(v));
        } else if resto.trim_end() == "erro-de-compilacao" {
            c.erro_compilacao = true;
        } else if let Some(e) = resto.strip_prefix("experimentos:") {
            c.experimentos.extend(e.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_string));
        }
    }
    c
}

impl Programa {
    /// Um programa com os cabeçalhos lidos de `entrada`.
    pub fn novo(nome: String, entrada: PathBuf, arquivos: Vec<PathBuf>) -> Programa {
        let fonte = std::fs::read_to_string(&entrada).unwrap_or_default();
        let c = ler_cabecalho(&fonte);
        Programa {
            nome,
            entrada,
            arquivos,
            diverge_ddc: c.diverge_ddc,
            requer: c.requer.unwrap_or(LanguageVersion::PISO),
            marcador: dartforge_frontend::features::marcador_versao(&fonte).map(|m| m.0),
            erro_compilacao: c.erro_compilacao,
            experimentos: c.experimentos,
            pendente: false,
        }
    }

    /// Programa sintético, sem arquivo (testes do harness).
    pub fn teste(nome: &str) -> Programa {
        Programa {
            nome: nome.to_string(),
            entrada: PathBuf::from("x.dart"),
            arquivos: vec![],
            diverge_ddc: None,
            requer: LanguageVersion::PISO,
            marcador: None,
            erro_compilacao: false,
            experimentos: vec![],
            pendente: false,
        }
    }

    /// Conteúdo concatenado de todos os arquivos (base do hash do cache).
    pub fn conteudo(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for a in &self.arquivos {
            v.extend_from_slice(a.to_string_lossy().as_bytes());
            v.push(0);
            v.extend(std::fs::read(a).unwrap_or_default());
            v.push(0);
        }
        v
    }

    /// Diretório do programa (onde `dart run` e o `dartdevc` são executados).
    pub fn diretorio(&self) -> &Path {
        self.entrada.parent().unwrap_or(Path::new("."))
    }

    /// Referência para o DartForge: a VM, salvo quando o cabeçalho declara divergência
    /// — aí a referência é o DDC, porque a web é o alvo real.
    pub fn referencia_e_ddc(&self) -> bool {
        self.diverge_ddc.is_some()
    }
}

/// Lê o marcador `// diverge-ddc: motivo` nas primeiras linhas do arquivo.
pub fn ler_diverge_ddc(fonte: &str) -> Option<String> {
    ler_cabecalho(fonte).diverge_ddc
}

/// Os nomes listados em `<dir>/PENDENTES` (linhas vazias e comentários `#` ignorados).
pub fn ler_pendentes(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("PENDENTES"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Lista os programas de `dir`: `*.dart` diretos e subdiretórios com `main.dart`,
/// em ordem alfabética. Um filtro opcional seleciona por substring do nome.
pub fn listar(dir: &Path, filtro: Option<&str>) -> Vec<Programa> {
    let mut programas = Vec::new();
    let Ok(entradas) = std::fs::read_dir(dir) else { return programas };
    let mut caminhos: Vec<PathBuf> = entradas.flatten().map(|e| e.path()).collect();
    caminhos.sort();
    for p in caminhos {
        if p.is_file() && p.extension().is_some_and(|e| e == "dart") {
            let nome = p.file_stem().unwrap().to_string_lossy().into_owned();
            programas.push(Programa::novo(nome, p.clone(), vec![p]));
        } else if p.is_dir() {
            let entrada = p.join("main.dart");
            if !entrada.is_file() {
                continue;
            }
            let nome = p.file_name().unwrap().to_string_lossy().into_owned();
            let mut arquivos = vec![entrada.clone()];
            listar_dart_recursivo(&p, &mut arquivos);
            arquivos.sort();
            arquivos.dedup();
            programas.push(Programa::novo(nome, entrada, arquivos));
        }
    }
    let pendentes = ler_pendentes(dir);
    for p in &mut programas {
        p.pendente = pendentes.contains(&p.nome);
    }
    if let Some(f) = filtro {
        programas.retain(|p| p.nome.contains(f));
    }
    // Ordem numérica pelo prefixo (`10_` antes de `100_`), depois pelo nome.
    programas.sort_by(|a, b| numero(&a.nome).cmp(&numero(&b.nome)).then_with(|| a.nome.cmp(&b.nome)));
    programas
}


/// Lê `K/N` (fragmento K de N, contado a partir de 1) — o argumento de `--fragmento`.
pub fn ler_fragmento(texto: &str) -> Result<(usize, usize), String> {
    let erro = || format!("--fragmento espera K/N com 1 <= K <= N, recebeu `{texto}`");
    let (k, n) = texto.split_once('/').ok_or_else(erro)?;
    let k: usize = k.trim().parse().map_err(|_| erro())?;
    let n: usize = n.trim().parse().map_err(|_| erro())?;
    if n == 0 || k == 0 || k > n {
        return Err(erro());
    }
    Ok((k, n))
}

/// Fragmento `k` de `n` da lista (já na ordem de [`listar`]): os programas de
/// índice `i` com `i % n == k - 1`.
///
/// Partição por resto, e não por blocos contíguos, porque o custo de um
/// programa acompanha o tema (o prefixo numérico): blocos dariam a um
/// fragmento todo o `async` e a outro só literais. Os `n` fragmentos cobrem a
/// lista sem repetir nenhum programa, e cada um preserva a ordem original —
/// é o que permite ao CI rodar o corpus nativo em `n` máquinas e somar os
/// relatórios.
pub fn fragmento(programas: Vec<Programa>, k: usize, n: usize) -> Vec<Programa> {
    assert!(n > 0 && (1..=n).contains(&k), "fragmento {k}/{n} inválido");
    programas.into_iter().enumerate().filter(|(i, _)| i % n == k - 1).map(|(_, p)| p).collect()
}

fn numero(nome: &str) -> u32 {
    nome.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(u32::MAX)
}

fn listar_dart_recursivo(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_dir() {
            listar_dart_recursivo(&p, saida);
        } else if p.extension().is_some_and(|e| e == "dart") {
            saida.push(p);
        }
    }
}

/// Tema de um programa pelo prefixo numérico do nome (organiza o CONTRATO-DDC.md).
pub fn tema(nome: &str) -> &'static str {
    match numero(nome) {
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
mod testes {
    use super::*;

    #[test]
    fn cabecalho_diverge() {
        assert_eq!(ler_diverge_ddc("// diverge-ddc: int de 64 bits\nvoid main() {}"), Some("int de 64 bits".into()));
        assert_eq!(ler_diverge_ddc("void main() {}"), None);
    }

    #[test]
    fn cabecalhos_de_versao_negativo_e_experimentos() {
        let c = ler_cabecalho("// @dart=3.6\n// requer-dart: 3.13\n// erro-de-compilacao\n// experimentos: macros, augmentations\nvoid main() {}");
        assert_eq!(c.requer, Some(LanguageVersion::new(3, 13)));
        assert!(c.erro_compilacao);
        assert_eq!(c.experimentos, vec!["macros".to_string(), "augmentations".to_string()]);
        assert_eq!(c.diverge_ddc, None);
        assert_eq!(ler_cabecalho("void main() {}"), Cabecalho::default());
    }

    #[test]
    fn temas() {
        assert_eq!(tema("01_print"), "Literais e strings");
        assert_eq!(tema("113_imports"), "Extensions, typedef e bibliotecas");
        assert_eq!(tema("abc"), "Outros");
    }

    #[test]
    fn fragmentos_particionam_o_corpus() {
        let p = |nome: String| Programa::teste(&nome);
        let todos: Vec<Programa> = (0..23).map(|i| p(format!("{i:02}_x"))).collect();
        let n = 4;
        let mut vistos: Vec<String> = Vec::new();
        for k in 1..=n {
            let f = fragmento(todos.clone(), k, n);
            // Ordem preservada e tamanhos equilibrados (23 = 6 + 6 + 6 + 5).
            assert!(f.windows(2).all(|w| w[0].nome < w[1].nome));
            assert!(f.len() == 5 || f.len() == 6, "fragmento {k}: {}", f.len());
            vistos.extend(f.into_iter().map(|p| p.nome));
        }
        vistos.sort();
        let esperado: Vec<String> = todos.iter().map(|p| p.nome.clone()).collect();
        assert_eq!(vistos, esperado, "a união dos fragmentos é o corpus, sem repetição");
        assert_eq!(fragmento(todos.clone(), 1, 1).len(), 23);
        assert_eq!(fragmento(todos, 2, 4)[0].nome, "01_x");
    }

    #[test]
    fn argumento_do_fragmento() {
        assert_eq!(ler_fragmento("3/8"), Ok((3, 8)));
        assert!(ler_fragmento("0/8").is_err());
        assert!(ler_fragmento("9/8").is_err());
        assert!(ler_fragmento("1/0").is_err());
        assert!(ler_fragmento("3").is_err());
        assert!(ler_fragmento("a/b").is_err());
    }
}
