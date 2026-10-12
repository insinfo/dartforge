//! Verificadores do analyzer que não dependem de tipos, sobre a árvore do
//! `frontend` (plano A3). Cada um reproduz um verificador do
//! `package:analyzer` 6.11.0 — os códigos, as mensagens em inglês e as
//! posições são os dele — e é conferido pelo placar de `crates/parity`.
//!
//! * [`locais`]: variáveis e funções locais não usadas (`UnusedLocalElementsVerifier`).
//! * [`duplicatas`]: `DuplicateDefinitionVerifier` e
//!   `MemberDuplicateDefinitionVerifier` (`src/error/duplicate_definition_verifier.dart`).
//! * [`externos`]: inicializadores de campos e variáveis `external`
//!   (`ErrorVerifier`).
//! * [`private_decls`]: declarações privadas nunca referenciadas
//!   (`UnusedLocalElementsVerifier`, a parte de biblioteca).
//! * [`operadores`]: aridade, parâmetros opcionais e retorno de `[]=` em
//!   métodos `operator` (`ErrorVerifier`).
//! * [`enums`]: enum sem constantes após augmentations (`ErrorVerifier`).
//! * [`clauses`]: cláusulas de herança (`subtype_of_disallowed_type`,
//!   erros de mixin, `class_used_as_mixin`) e a porta do `ErrorVerifier`
//!   que desliga as verificações seguintes.
//! * [`modifiers`]: `base`/`final`/`interface`/`sealed` usados fora da
//!   biblioteca (`ErrorVerifier` e `BaseOrFinalTypeVerifier`).
//! * [`members`]: verificações locais de declarações e membros do
//!   `ErrorVerifier` (parâmetros de tipo em conflito, campos de enum e de
//!   tipo de extensão, setters, `this.x` fora de construtor, lista de
//!   inicializadores, `return` em construtor gerador).
//! * [`natives`]: `native` (cláusula de classe e corpo) fora do SDK
//!   (`ErrorVerifier`).

pub mod a_context;
pub mod arvore_analyzer;
pub mod saida;
pub mod a_doc;
pub mod c2_syntactic;
pub mod clauses;
pub mod simple_bounds;
pub mod records;
pub mod constructors;
pub mod duplicatas;
pub mod enums;
pub mod externos;
pub mod phases;
pub mod importacoes;
pub mod inicializacao;
pub mod lints;
pub mod locais;
pub mod members;
pub mod meta;
pub mod modifiers;
pub mod naodart;
pub mod natives;
pub mod operadores;
pub mod private_decls;
pub mod publicacao;
pub mod todos;
pub mod language_version;

use dartforge_frontend::ast::{Ast, CompilationUnit};

/// Uma unidade (arquivo) de uma biblioteca, na ordem da biblioteca: a que a
/// declara primeiro, depois as partes.
#[derive(Clone, Copy)]
pub struct Unidade<'a> {
    pub ast: &'a Ast,
    pub unit: &'a CompilationUnit,
    /// O texto da unidade (anotações locais não ficam na árvore).
    pub fonte: &'a str,
}

/// Os trechos de uma unidade que a recuperação de erro do parser descartou
/// (`Unit::pulados`), com o texto dela: a única supressão por sintaxe que
/// resta (docs/ANALYZER-ESPECIFICACAO.md §G, T5, estrutura (c)). Um nome
/// citado num trecho pulado pode ter ali um uso ou uma declaração que o
/// fasta manteve e a nossa árvore não tem.
#[derive(Clone, Copy)]
pub struct Pulados<'a> {
    pub fonte: &'a str,
    pub trechos: &'a [dartforge_diagnostics::Span],
}

impl Pulados<'_> {
    pub fn vazio(&self) -> bool {
        self.trechos.is_empty()
    }

    /// O intervalo `s` toca algum trecho pulado?
    pub fn intersecta(&self, s: dartforge_diagnostics::Span) -> bool {
        self.trechos.iter().any(|t| t.start < s.end && s.start < t.end)
    }

    /// O identificador `nome` aparece, como palavra inteira, em algum
    /// trecho pulado?
    pub fn cita(&self, nome: &str) -> bool {
        self.trechos.iter().any(|t| cita_no_trecho(self.fonte, *t, nome))
    }

    /// Como [`Pulados::cita`], só nos trechos que tocam `dentro` (a
    /// declaração executável de um local).
    /// Só a citação que começa dentro de `dentro` conta (o próprio nome
    /// declarado, num trecho pulado que o contém, não é uso: `var enum;`).
    pub fn cita_em(&self, nome: &str, dentro: dartforge_diagnostics::Span) -> bool {
        self.trechos.iter().filter(|t| t.start < dentro.end && dentro.start < t.end).any(|t| {
            let inicio = t.start.max(dentro.start);
            let fim = t.end.min(dentro.end);
            // O trecho inteiro decide a fronteira de palavra; a posição, o recorte.
            citacoes(self.fonte, *t, nome).any(|p| p >= inicio && p < fim)
        })
    }
}

/// As posições (na unidade) das citações de `nome` como palavra no trecho `t`.
fn citacoes<'a>(fonte: &'a str, t: dartforge_diagnostics::Span, nome: &'a str) -> impl Iterator<Item = usize> + 'a {
    let texto = fonte.get(t.start..t.end.min(fonte.len())).unwrap_or("");
    let bytes = texto.as_bytes();
    let de_palavra = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    texto
        .match_indices(nome)
        .filter(move |(ini, _)| {
            let fim = ini + nome.len();
            !nome.is_empty() && (*ini == 0 || !de_palavra(bytes[ini - 1])) && (fim >= bytes.len() || !de_palavra(bytes[fim]))
        })
        .map(move |(ini, _)| t.start + ini)
}

fn cita_no_trecho(fonte: &str, t: dartforge_diagnostics::Span, nome: &str) -> bool {
    if nome.is_empty() {
        return false;
    }
    let Some(texto) = fonte.get(t.start..t.end.min(fonte.len())) else { return false };
    let bytes = texto.as_bytes();
    let de_palavra = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut de = 0;
    while let Some(i) = texto[de..].find(nome) {
        let ini = de + i;
        let fim = ini + nome.len();
        let antes_ok = ini == 0 || !de_palavra(bytes[ini - 1]);
        let depois_ok = fim >= bytes.len() || !de_palavra(bytes[fim]);
        if antes_ok && depois_ok {
            return true;
        }
        de = ini + 1;
        while de < texto.len() && !texto.is_char_boundary(de) {
            de += 1;
        }
    }
    false
}
