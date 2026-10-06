//! Verificadores de aviso que o analyzer roda por arquivo e o DartForge não
//! tinha (docs/ANALYZER-ESPECIFICACAO-INFRA.md, Parte II, lote II.8, e
//! Parte III, etapa 6):
//!
//! * do `ImportsVerifier` (`analyzer/lib/src/error/imports_verifier.dart`):
//!   `duplicate_import`, `duplicate_export`, `duplicate_shown_name`,
//!   `duplicate_hidden_name`; e, do `DeadCodeVerifier._checkCombinator`,
//!   `undefined_hidden_name`;
//! * o `UnicodeTextVerifier` (`unicode_text_verifier.dart`):
//!   `text_direction_code_point_in_comment` e `…_in_literal`.
//!
//! Escrito sem compilar nem executar (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, Combinator, DirectiveKind, ExprKind};
use dartforge_intern::{Interner, SymbolId};

/// Uma diretiva de import ou export para a comparação de duplicatas.
struct Diretiva<'a> {
    indice: usize,
    uri_da_biblioteca: &'a str,
    prefixo: Option<SymbolId>,
    combinadores: &'a [Combinator],
}

/// `areSyntacticallyIdenticalExceptUri` (`ast.dart:10005-10055`): mesmo
/// prefixo e os mesmos combinadores, na mesma ordem, com os mesmos nomes na
/// mesma ordem. `deferred` e as configurações não entram.
fn identicas(a: &Diretiva<'_>, b: &Diretiva<'_>) -> bool {
    if a.prefixo != b.prefixo || a.combinadores.len() != b.combinadores.len() {
        return false;
    }
    a.combinadores.iter().zip(b.combinadores.iter()).all(|(x, y)| match (x, y) {
        (Combinator::Show(p), Combinator::Show(q)) | (Combinator::Hide(p), Combinator::Hide(q)) => {
            p.len() == q.len() && p.iter().zip(q.iter()).all(|(m, n)| m.sym == n.sym)
        }
        _ => false,
    })
}

/// `_duplicates` (`imports_verifier.dart:368-397`): a lista ordenada pela
/// URI absoluta da biblioteca; de cada par CONSECUTIVO igual, a diretiva de
/// maior offset é a duplicata.
fn duplicatas(mut lista: Vec<Diretiva<'_>>) -> Vec<usize> {
    let mut saida = Vec::new();
    if lista.len() <= 1 {
        return saida;
    }
    lista.sort_by(|a, b| a.uri_da_biblioteca.cmp(b.uri_da_biblioteca).then(a.indice.cmp(&b.indice)));
    for par in lista.windows(2) {
        let (atual, prox) = (&par[0], &par[1]);
        if atual.uri_da_biblioteca == prox.uri_da_biblioteca && identicas(atual, prox) {
            saida.push(atual.indice.max(prox.indice));
        }
    }
    saida
}

/// Os avisos de diretivas repetidas e de nomes repetidos em combinadores da
/// biblioteca `lib`, unidade por unidade.
pub fn diretivas_repetidas(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let mut out = Vec::new();
    let biblioteca = program.library(lib);
    for &u in &biblioteca.units {
        let diretivas = &program.unit(u).unit.directives;
        let uri_de = |indice: usize| -> Option<Span> {
            match &diretivas.get(indice)?.kind {
                DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => Some(uri.span),
                _ => None,
            }
        };
        let imports: Vec<Diretiva<'_>> = biblioteca
            .imports
            .iter()
            .filter(|i| i.unit == u)
            .map(|i| Diretiva {
                indice: i.directive,
                uri_da_biblioteca: program.library(i.library).uri.as_str(),
                prefixo: i.prefix,
                combinadores: &i.combinators,
            })
            .collect();
        let exports: Vec<Diretiva<'_>> = biblioteca
            .exports
            .iter()
            .filter(|e| e.unit == u)
            .map(|e| Diretiva {
                indice: e.directive,
                uri_da_biblioteca: program.library(e.library).uri.as_str(),
                prefixo: None,
                combinadores: &e.combinators,
            })
            .collect();
        // A ordem fixa do oficial: exports, imports, nomes escondidos e
        // mostrados.
        for indice in duplicatas(exports) {
            if let Some(sp) = uri_de(indice) {
                out.push((u, Diagnostic::com_codigo(w::DUPLICATE_EXPORT, sp, std::iter::empty::<&str>())));
            }
        }
        for indice in duplicatas(imports) {
            if let Some(sp) = uri_de(indice) {
                out.push((u, Diagnostic::com_codigo(w::DUPLICATE_IMPORT, sp, std::iter::empty::<&str>())));
            }
        }
        // Nomes dos combinadores: repetidos no MESMO combinador (só os que
        // resolvem: o que a biblioteca alvo exporta), e os de `hide` que ela
        // não exporta.
        let alvos = biblioteca
            .imports
            .iter()
            .filter(|i| i.unit == u)
            .map(|i| (i.library, &i.combinators))
            .chain(biblioteca.exports.iter().filter(|e| e.unit == u).map(|e| (e.library, &e.combinators)));
        for (alvo, combinadores) in alvos {
            let exportado = &program.library(alvo).exported;
            let uri = program.library(alvo).uri.as_str();
            for k in combinadores.iter() {
                let (nomes, mostra) = match k {
                    Combinator::Show(n) => (n, true),
                    Combinator::Hide(n) => (n, false),
                };
                let mut vistos: Vec<SymbolId> = Vec::new();
                for n in nomes.iter() {
                    let resolve = exportado.contains_key(&n.sym);
                    if !resolve {
                        if !mostra {
                            out.push((u, Diagnostic::com_codigo(w::UNDEFINED_HIDDEN_NAME, n.span, [uri, interner.resolve(n.sym)])));
                        }
                        continue;
                    }
                    if vistos.contains(&n.sym) {
                        let codigo = if mostra { w::DUPLICATE_SHOWN_NAME } else { w::DUPLICATE_HIDDEN_NAME };
                        out.push((u, Diagnostic::com_codigo(codigo, n.span, [interner.resolve(n.sym)])));
                    } else {
                        vistos.push(n.sym);
                    }
                }
            }
        }
    }
    out
}

/// `built_in_identifier_as_type`: um tipo nomeado cujo nome é identificador
/// embutido (`abstract`, `import`, `get`…), fora `dynamic` e `Function`,
/// que são tipos. O parser do analyzer o relata ao ler o tipo
/// (`TypeReferenceIdentifierContext`, `identifier_context_impl.dart`), e o
/// `NamedTypeResolver` no nome depois de um prefixo (`p.import`). Aqui o
/// relato sai da árvore: no nome sem prefixo, ou no nome depois do prefixo.
/// Uma palavra embutida seguida de `.` é prefixo, não tipo. Num nome de
/// duas partes, só depois de um prefixo de import (`A.factory` de
/// `new A.factory()` é a classe e o construtor, cujo nome aceita embutidas).
/// `augment` só é embutida com `augmentations` ligado (`augmentacoes`): sem
/// ele o scanner a lê como identificador comum (`abstract_scanner.dart`).
pub fn embutido_como_tipo(u: Unidade<'_>, interner: &Interner, augmentacoes: bool) -> Vec<Diagnostic> {
    const EMBUTIDAS: [&str; 22] = [
        "abstract", "as", "augment", "covariant", "deferred", "export", "extension", "external", "factory", "get", "implements",
        "import", "interface", "late", "library", "mixin", "operator", "part", "required", "set", "static", "typedef",
    ];
    let prefixos: Vec<dartforge_intern::SymbolId> = u
        .unit
        .directives
        .iter()
        .filter_map(|d| match &d.kind {
            ast::DirectiveKind::Import { prefix: Some(p), .. } => Some(p.sym),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for t in u.ast.types.iter() {
        let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
        let Some(ultimo) = name.last() else { continue };
        if name.len() >= 2 && !prefixos.contains(&name[0].sym) {
            continue;
        }
        let texto = interner.resolve(ultimo.sym);
        if texto == "augment" && !augmentacoes {
            continue;
        }
        if EMBUTIDAS.contains(&texto) {
            out.push(Diagnostic::com_codigo(
                dartforge_diagnostics::codigos::compile_time_error::BUILT_IN_IDENTIFIER_AS_TYPE,
                ultimo.span,
                [texto],
            ));
        }
    }
    out
}

/// `obsolete_colon_for_default_value`
/// (`BestPracticesVerifier.visitDefaultFormalParameter`,
/// `best_practices_verifier.dart:288-306`): o parâmetro nomeado cujo valor
/// padrão vem depois de `:` em vez de `=`; no `:`. Para a versão da
/// linguagem anterior à 3.0 o original dá a dica
/// `deprecated_colon_for_default_value`, que aqui não sai.
///
/// `antes_da_3_0`: a versão de linguagem da biblioteca é anterior à 3.0; o
/// separador `:` é então só `DEPRECATED_COLON_FOR_DEFAULT_VALUE` (dica), e
/// não o erro (`best_practices_verifier.dart:288-306` do analyzer 3.6.2).
pub fn dois_pontos_no_padrao(u: Unidade<'_>, antes_da_3_0: bool) -> Vec<Diagnostic> {
    let codigo = if antes_da_3_0 {
        dartforge_diagnostics::codigos::hint::DEPRECATED_COLON_FOR_DEFAULT_VALUE
    } else {
        dartforge_diagnostics::codigos::compile_time_error::OBSOLETE_COLON_FOR_DEFAULT_VALUE
    };
    fn visitar(u: Unidade<'_>, lista: &[ast::Parameter], codigo: dartforge_diagnostics::Codigo, out: &mut Vec<Diagnostic>) {
        for p in lista {
            if let Some(internos) = &p.function_parameters {
                visitar(u, internos, codigo, out);
            }
            let (ast::ParameterKind::Named, Some(padrao)) = (p.kind, p.default_value) else { continue };
            // O separador é o último `:` ou `=` antes do valor, depois do
            // início do parâmetro.
            let ate = u.ast.expr(padrao).span.start;
            let Some(antes) = u.fonte.get(p.span.start..ate) else { continue };
            let sem_brancos = antes.trim_end();
            if sem_brancos.ends_with(':') {
                let pos = p.span.start + sem_brancos.len() - 1;
                out.push(Diagnostic::com_codigo(codigo, Span { start: pos, end: pos + 1 }, std::iter::empty::<&str>()));
            }
        }
    }
    let mut out = Vec::new();
    for f in u.ast.functions.iter() {
        if let Some(ps) = &f.parameters {
            visitar(u, ps, codigo, &mut out);
        }
    }
    for m in u.ast.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            visitar(u, &k.parameters, codigo, &mut out);
        }
    }
    out
}

/// Um caractere de controle de direção de texto: U+202A..U+202E ou
/// U+2066..U+2069.
fn de_direcao(c: char) -> bool {
    matches!(c as u32, 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Os intervalos de texto de literal de string de uma unidade: os trechos
/// literais de cada string (as expressões interpoladas ficam de fora) e as
/// URIs das diretivas.
fn trechos_de_literal(u: Unidade<'_>) -> Vec<Span> {
    let mut literais: Vec<Span> = Vec::new();
    let mut interpoladas: Vec<Span> = Vec::new();
    for e in u.ast.exprs.iter() {
        if let ExprKind::String(lit) = &e.kind {
            literais.push(lit.span);
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(x) = p {
                    interpoladas.push(u.ast.expr(*x).span);
                }
            }
        }
    }
    for d in u.unit.directives.iter() {
        match &d.kind {
            DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } | DirectiveKind::Part { uri } => literais.push(uri.span),
            _ => {}
        }
    }
    // Um offset é de literal se está em alguma string e fora de toda
    // expressão interpolada que não contenha, ela mesma, outra string ali.
    literais.sort_by_key(|s| (s.start, s.end));
    interpoladas.sort_by_key(|s| (s.start, s.end));
    let mut saida = literais;
    saida.extend(interpoladas.into_iter().map(|s| Span { start: s.end, end: s.start }));
    saida
}

/// `UnicodeTextVerifier.verify`: um relato por caractere de direção, de
/// comprimento 1, com o código em hexadecimal maiúsculo; o código depende
/// de o caractere estar no texto de um literal de string ou em qualquer
/// outro lugar (comentário, código).
pub fn texto_bidirecional(u: Unidade<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !u.fonte.chars().any(de_direcao) {
        return out;
    }
    // Os intervalos invertidos (`start > end`) marcam as expressões
    // interpoladas; ver `trechos_de_literal`.
    let trechos = trechos_de_literal(u);
    for (offset, c) in u.fonte.char_indices() {
        if !de_direcao(c) {
            continue;
        }
        // O nó mais interno: a string ou a interpolação de intervalo menor
        // que cobre o offset.
        let mut melhor: Option<(usize, bool)> = None;
        for t in &trechos {
            let (ini, fim, literal) = if t.start <= t.end { (t.start, t.end, true) } else { (t.end, t.start, false) };
            if ini <= offset && offset < fim {
                let tamanho = fim - ini;
                if melhor.is_none_or(|(m, _)| tamanho < m) {
                    melhor = Some((tamanho, literal));
                }
            }
        }
        let em_literal = melhor.is_some_and(|(_, literal)| literal);
        let codigo = if em_literal { w::TEXT_DIRECTION_CODE_POINT_IN_LITERAL } else { w::TEXT_DIRECTION_CODE_POINT_IN_COMMENT };
        let hex = format!("{:04X}", c as u32);
        out.push(Diagnostic::com_codigo(codigo, Span { start: offset, end: offset + c.len_utf8() }, [hex.as_str()]));
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn bidirecionais(fonte: &str) -> Vec<&'static str> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        texto_bidirecional(u).into_iter().map(|d| d.code.map_or("", |c| c.info().nome)).collect()
    }

    #[test]
    fn direcao_em_literal_e_em_comentario() {
        assert_eq!(bidirecionais("var s = 'a\u{202E}b';\n"), vec!["text_direction_code_point_in_literal"]);
        assert_eq!(bidirecionais("// a\u{2066}b\nvar s = 1;\n"), vec!["text_direction_code_point_in_comment"]);
        assert!(bidirecionais("var s = 'ab';\n").is_empty());
    }

    #[test]
    fn palavra_embutida_como_tipo() {
        let fonte = "void f(List<int> a, dynamic b, Function c) {}
";
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        assert!(embutido_como_tipo(Unidade { ast: &p.ast, unit: &p.unit, fonte }, &nomes, false).is_empty());
    }

    #[test]
    fn dois_pontos_como_separador_do_padrao() {
        let fonte = "void f({int a: 1, int b = 2}) {}
";
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let achados = dois_pontos_no_padrao(u, false);
        assert_eq!(achados.len(), 1);
        assert_eq!(&fonte[achados[0].span.start..achados[0].span.end], ":");
        assert_eq!(achados[0].code.unwrap().info().nome, "obsolete_colon_for_default_value");
        let antigos = dois_pontos_no_padrao(u, true);
        assert_eq!(antigos[0].code.unwrap().info().nome, "deprecated_colon_for_default_value");
    }

    #[test]
    fn par_consecutivo_identico_e_duplicata() {
        let uri = "package:a/a.dart";
        let d = |indice: usize| Diretiva { indice, uri_da_biblioteca: uri, prefixo: None, combinadores: &[] };
        assert_eq!(duplicatas(vec![d(0), d(2)]), vec![2]);
        assert!(duplicatas(vec![d(0)]).is_empty());
    }
}
