//! Enumeração dos programas do corpus e leitura do cabeçalho de cada um.

use std::path::{Path, PathBuf};

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
}

impl Programa {
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
    fonte.lines().take(20).find_map(|l| {
        let l = l.trim();
        let resto = l.strip_prefix("//")?.trim_start();
        let motivo = resto.strip_prefix("diverge-ddc:")?;
        Some(motivo.trim().to_string())
    })
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
            let fonte = std::fs::read_to_string(&p).unwrap_or_default();
            programas.push(Programa {
                nome,
                entrada: p.clone(),
                arquivos: vec![p],
                diverge_ddc: ler_diverge_ddc(&fonte),
            });
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
            let fonte = std::fs::read_to_string(&entrada).unwrap_or_default();
            programas.push(Programa { nome, entrada, arquivos, diverge_ddc: ler_diverge_ddc(&fonte) });
        }
    }
    if let Some(f) = filtro {
        programas.retain(|p| p.nome.contains(f));
    }
    // Ordem numérica pelo prefixo (`10_` antes de `100_`), depois pelo nome.
    programas.sort_by(|a, b| numero(&a.nome).cmp(&numero(&b.nome)).then_with(|| a.nome.cmp(&b.nome)));
    programas
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
    fn temas() {
        assert_eq!(tema("01_print"), "Literais e strings");
        assert_eq!(tema("113_imports"), "Extensions, typedef e bibliotecas");
        assert_eq!(tema("abc"), "Outros");
    }
}
