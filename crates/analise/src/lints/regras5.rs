//! O quinto lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `sort_constructors_first`,
//! `sort_unnamed_constructors_first`,
//! `always_put_required_named_parameters_first`,
//! `avoid_multiple_declarations_per_line`, `eol_at_end_of_file` e
//! `unnecessary_raw_strings`.
//!
//! Escritas com os emissores do `main` do SDK abertos e depois conferidas
//! contra os da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`), que
//! diferem em três pontos, já seguidos aqui: nas duas regras de ordem de
//! construtores o relato vai no nome da classe (`member.returnType`);
//! `sort_unnamed_constructors_first` só olha um extension type que tem
//! construtor primário nomeado; e `always_put_required_named_parameters_first`
//! não isenta os parâmetros `super.x`. `eol_at_end_of_file` tem um código
//! só.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberId, MemberKind, ParameterKind, StmtKind, VariableList};
use dartforge_intern::Interner;

/// O literal `lexema` é uma única string crua (`r'…'`, `r"""…"""`): começa
/// com `r` e a aspa de abertura só volta a aparecer no fim.
fn crua_simples(lexema: &str) -> bool {
    let Some(resto) = lexema.strip_prefix('r') else { return false };
    let aspa = if resto.starts_with("'''") {
        "'''"
    } else if resto.starts_with("\"\"\"") {
        "\"\"\""
    } else if resto.starts_with('\'') {
        "'"
    } else if resto.starts_with('"') {
        "\""
    } else {
        return false;
    };
    let miolo = &resto[aspa.len()..];
    miolo.find(aspa).is_some_and(|k| k + aspa.len() == miolo.len())
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // As duas regras de ordem de construtores, em classes, enums e
    // extension types.
    let (primeiro, sem_nome_primeiro) = (ligada("sort_constructors_first"), ligada("sort_unnamed_constructors_first"));
    if primeiro || sem_nome_primeiro {
        for d in a.decls.iter() {
            let membros: &[MemberId] = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            // Num extension type, a regra dos sem nome só vale com
            // construtor primário nomeado.
            let sem_nome_aqui = sem_nome_primeiro && !matches!(&d.kind, DeclKind::ExtensionType(x) if x.constructor.is_none());
            let (mut outro, mut nomeado) = (false, false);
            for id in membros {
                let MemberKind::Constructor(k) = &a.member(*id).kind else {
                    outro = true;
                    continue;
                };
                // `member.returnType`: o nome da classe.
                let faixa = k.class_name.span;
                if primeiro && outro {
                    relatar(&c::SORT_CONSTRUCTORS_FIRST, faixa, &[]);
                }
                match k.name {
                    None if sem_nome_aqui && nomeado => relatar(&c::SORT_UNNAMED_CONSTRUCTORS_FIRST, faixa, &[]),
                    None => {}
                    Some(_) => nomeado = true,
                }
            }
        }
    }
    // `always_put_required_named_parameters_first`.
    if ligada("always_put_required_named_parameters_first") {
        fn visitar(lista: &[ast::Parameter], interner: &Interner, achados: &mut Vec<ast::Name>) {
            let mut viu_opcional = false;
            for p in lista {
                if let Some(internos) = &p.function_parameters {
                    visitar(internos, interner, achados);
                }
                if p.kind != ParameterKind::Named {
                    continue;
                }
                let requerido = p.required
                    || p.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| interner.resolve(n.sym) == "required"));
                if requerido {
                    if let (true, Some(n)) = (viu_opcional, p.name) {
                        achados.push(n);
                    }
                } else {
                    viu_opcional = true;
                }
            }
        }
        let mut achados: Vec<ast::Name> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                visitar(ps, interner, &mut achados);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                visitar(&k.parameters, interner, &mut achados);
            }
        }
        for n in achados {
            relatar(&c::ALWAYS_PUT_REQUIRED_NAMED_PARAMETERS_FIRST, n.span, &[]);
        }
    }
    // `avoid_multiple_declarations_per_line`: na segunda variável de toda
    // lista, menos a da inicialização de um `for`.
    if ligada("avoid_multiple_declarations_per_line") {
        let mut checar = |l: &VariableList| {
            if let Some(segunda) = l.variables.get(1) {
                relatar(&c::AVOID_MULTIPLE_DECLARATIONS_PER_LINE, segunda.name.span, &[]);
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                checar(l);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind {
                checar(l);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Variables(l) = &s.kind {
                checar(l);
            }
        }
    }
    // `eol_at_end_of_file`: nenhuma quebra no fim, ou mais de uma.
    if ligada("eol_at_end_of_file") && !fonte.is_empty() {
        let demais = fonte.ends_with("\n\n") || fonte.ends_with("\r\r") || fonte.ends_with("\r\n\r\n");
        let falta = !(fonte.ends_with('\n') || fonte.ends_with('\r'));
        if demais || falta {
            let pos = fonte.trim_end().len();
            // Um caractere a partir do fim do conteúdo (ou o fim, se não há).
            let fim = fonte[pos..].chars().next().map_or(pos, |x| pos + x.len_utf8());
            relatar(&c::EOL_AT_END_OF_FILE, Span { start: pos, end: fim }, &[]);
        }
    }
    // `unnecessary_raw_strings`: string crua sem `\` nem `$`.
    if ligada("unnecessary_raw_strings") {
        for e in a.exprs.iter() {
            let ExprKind::String(_) = &e.kind else { continue };
            let Some(lexema) = fonte.get(e.span.start..e.span.end) else { continue };
            if crua_simples(lexema) && !lexema.contains('\\') && !lexema.contains('$') {
                relatar(&c::UNNECESSARY_RAW_STRINGS, e.span, &[]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|regra| regra != "eol_at_end_of_file");
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn ordem_dos_construtores() {
        assert_eq!(
            achados("class A {\n  int x = 0;\n  A.n();\n  A();\n}\n"),
            vec![
                ("sort_constructors_first", "A".to_string()),
                ("sort_constructors_first", "A".to_string()),
                ("sort_unnamed_constructors_first", "A".to_string()),
            ]
        );
        assert!(achados("class A {\n  A();\n  A.n();\n  int x = 0;\n}\n").is_empty());
    }

    #[test]
    fn parametros_e_declaracoes() {
        assert_eq!(
            achados("void f({int? a, required int b}) {}\n"),
            vec![("always_put_required_named_parameters_first", "b".to_string())]
        );
        assert_eq!(achados("int a = 1, b = 2;\n"), vec![("avoid_multiple_declarations_per_line", "b".to_string())]);
        assert!(achados("void f() {\n  for (var i = 0, j = 1; i < j; i++) {}\n}\n").is_empty());
    }

    #[test]
    fn strings_cruas_e_fim_de_arquivo() {
        assert_eq!(achados("var s = r'abc';\nvar t = r'a\\b';\n"), vec![("unnecessary_raw_strings", "r'abc'".to_string())]);
        assert!(crua_simples("r'''a'b'''") && !crua_simples("'a'") && !crua_simples("r'a' r'b'"));
        let fim = |fonte: &str| {
            let mut nomes = Interner::new();
            let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
            executar(Unidade { ast: &p.ast, unit: &p.unit, fonte }, &nomes, &|regra| regra == "eol_at_end_of_file").len()
        };
        assert_eq!((fim("var a = 1;"), fim("var a = 1;\n"), fim("var a = 1;\n\n")), (1, 0, 1));
    }
}
