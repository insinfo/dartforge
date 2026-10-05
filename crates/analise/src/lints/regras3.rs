//! O terceiro lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `unnecessary_new`,
//! `prefer_typing_uninitialized_variables`, `always_declare_return_types`,
//! `library_names`, `slash_for_doc_comments` e
//! `unnecessary_brace_in_string_interps`.
//!
//! Escritas com os emissores do `main` do SDK abertos e depois conferidas
//! contra os da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`, extraído
//! da tag em 2026-10-05). Nenhuma das seis difere na regra.
//! Diferenças conhecidas:
//! `always_declare_return_types` isenta os métodos `test_*`/`solo_test_*`
//! quando a unidade que define a biblioteca está no `test/` do pacote (a
//! pasta do `pubspec.yaml` mais próximo; pede a semântica da unidade);
//! `slash_for_doc_comments` acha o comentário de documentação de cada
//! declaração como o `_findComment` do parser (`findDartDoc` depois da
//! metadata, e senão antes de cada anotação, da última para a primeira),
//! com os comentários varridos da fonte (`dartforge_frontend::comentarios`),
//! nas declarações que o original visita (não o tipo de extensão), e o
//! primeiro comentário antes da função local.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    CreationKeyword, DeclKind, DirectiveKind, ExprKind, ForInit, FunctionId, FunctionKind, MemberKind, StmtKind, StringPart, TypedefKind,
    VariableList,
};
use dartforge_intern::Interner;

/// `isLowerCaseUnderScoreWithDots`: `^_?[_a-z\d]*(?:\.[a-z][_a-z\d]*)*$`.
pub fn e_nome_de_biblioteca(nome: &str) -> bool {
    let do_miolo = |x: char| x == '_' || x.is_ascii_lowercase() || x.is_ascii_digit();
    let mut partes = nome.split('.');
    let primeira = partes.next().unwrap_or("");
    primeira.chars().all(do_miolo) && partes.all(|p| p.chars().next().is_some_and(|x| x.is_ascii_lowercase()) && p.chars().all(do_miolo))
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `unnecessary_new`: na palavra `new`.
    if ligada("unnecessary_new") {
        for e in a.exprs.iter() {
            if matches!(e.kind, ExprKind::InstanceCreation { keyword: Some(CreationKeyword::New), .. }) && fonte.get(e.span.start..).is_some_and(|t| t.starts_with("new")) {
                relatar(&c::UNNECESSARY_NEW, Span { start: e.span.start, end: e.span.start + 3 }, &[]);
            }
        }
    }
    // `prefer_typing_uninitialized_variables`: lista sem tipo com variável
    // sem inicializador.
    if ligada("prefer_typing_uninitialized_variables") {
        let mut checar = |lista: &VariableList, de_campo: bool| {
            if lista.ty.is_some() {
                return;
            }
            let codigo =
                if de_campo { &c::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_FIELD } else { &c::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_LOCAL_VARIABLE };
            for v in lista.variables.iter().filter(|v| v.initializer.is_none()) {
                relatar(codigo, v.name.span, &[]);
            }
        };
        for d in a.decls.iter().filter(|d| !d.augment) {
            if let DeclKind::Variables(l) = &d.kind {
                checar(l, false);
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind {
                checar(l, true);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => checar(l, false),
                _ => {}
            }
        }
    }
    // `always_declare_return_types`.
    if ligada("always_declare_return_types") {
        let de_metodo: Vec<FunctionId> = a
            .members
            .iter()
            .filter_map(|m| match &m.kind {
                MemberKind::Method(f) => Some(*f),
                _ => None,
            })
            .collect();
        let aumentados: Vec<FunctionId> = a
            .members
            .iter()
            .filter(|m| m.augment)
            .filter_map(|m| match &m.kind {
                MemberKind::Method(f) => Some(*f),
                _ => None,
            })
            .chain(a.decls.iter().filter(|d| d.augment).filter_map(|d| match &d.kind {
                DeclKind::Function(f) => Some(*f),
                _ => None,
            }))
            .collect();
        for (i, f) in a.functions.iter().enumerate() {
            let id = FunctionId(i as u32);
            let Some(nome) = f.name else { continue };
            if f.return_type.is_some() || f.kind == FunctionKind::Setter || aumentados.contains(&id) {
                continue;
            }
            let texto = interner.resolve(nome.sym);
            if de_metodo.contains(&id) {
                let de_teste = (texto.starts_with("test_") || texto.starts_with("solo_test_")) && sem.is_some_and(super::em_teste_do_pacote);
                if texto != "[]=" && !de_teste {
                    relatar(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_METHODS, nome.span, &[texto]);
                }
            } else {
                relatar(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS, nome.span, &[texto]);
            }
        }
        // O `typedef` da forma antiga sem tipo de retorno.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && matches!(x.kind, TypedefKind::Legacy { return_type: None, .. })
            {
                relatar(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS, x.name.span, &[interner.resolve(x.name.sym)]);
            }
        }
    }
    // `library_names`.
    if ligada("library_names") {
        for d in u.unit.directives.iter() {
            if let DirectiveKind::Library { name } = &d.kind
                && let (Some(p), Some(ult)) = (name.first(), name.last())
            {
                let texto = name.iter().map(|n| interner.resolve(n.sym)).collect::<Vec<_>>().join(".");
                if !e_nome_de_biblioteca(&texto) {
                    relatar(&c::LIBRARY_NAMES, Span { start: p.span.start, end: ult.span.end }, &[texto.as_str()]);
                }
            }
        }
    }
    // `slash_for_doc_comments`: `/** … */` como documentação.
    if ligada("slash_for_doc_comments") {
        let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
        // `_findComment(metadata, tokenAfterMetadata)`.
        let doc = |metadata: &[dartforge_frontend::ast::Annotation], inicio: usize| {
            let depois = match metadata.last() {
                Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
                None => inicio,
            };
            comentarios.dart_doc(fonte, depois).or_else(|| metadata.iter().rev().find_map(|m| comentarios.dart_doc(fonte, m.span.start)))
        };
        let mut docs: Vec<Span> = Vec::new();
        if let Some(primeira) = u.unit.directives.first() {
            docs.extend(doc(&primeira.metadata, primeira.span.start));
        }
        for d in a.decls.iter() {
            if matches!(d.kind, DeclKind::ExtensionType(_)) {
                continue;
            }
            docs.extend(doc(&d.metadata, d.span.start));
            if let DeclKind::Enum(x) = &d.kind {
                for k in x.constants.iter() {
                    docs.extend(doc(&k.metadata, k.span.start));
                }
            }
        }
        for m in a.members.iter() {
            docs.extend(doc(&m.metadata, m.span.start));
        }
        let mut locais: Vec<Span> = Vec::new();
        for (i, s) in a.stmts.iter().enumerate() {
            if !matches!(s.kind, StmtKind::Function(_)) {
                continue;
            }
            let metadata = a
                .metadados_locais
                .iter()
                .find(|(x, _)| x.0 as usize == i)
                .map(|(_, m)| &m[..])
                .unwrap_or(&[]);
            docs.extend(doc(metadata, s.span.start));
            // `visitFunctionDeclarationStatement`: o primeiro comentário
            // antes do comando.
            if let Some(primeiro) = comentarios.antes_de(fonte, s.span.start).first()
                && fonte[primeiro.start..primeiro.end].starts_with("/**")
            {
                locais.push(*primeiro);
            }
        }
        // `isJavaStyle`.
        docs.retain(|s| fonte[s.start..s.end].starts_with("/**"));
        docs.extend(locais);
        docs.sort_by_key(|s| (s.start, s.end));
        docs.dedup();
        for span in docs {
            relatar(&c::SLASH_FOR_DOC_COMMENTS, span, &[]);
        }
    }
    // `unnecessary_brace_in_string_interps`: `${nome}` e `${this}` quando o
    // que vem depois não continuaria o identificador.
    if ligada("unnecessary_brace_in_string_interps") {
        let b = fonte.as_bytes();
        for e in a.exprs.iter() {
            let ExprKind::String(lit) = &e.kind else { continue };
            for parte in lit.parts.iter() {
                let StringPart::Interpolation(x) = parte else { continue };
                let dentro = a.expr(*x);
                let simples = match &dentro.kind {
                    ExprKind::Identifier(n) => !interner.resolve(n.sym).contains('$'),
                    ExprKind::This => true,
                    _ => false,
                };
                let (ini, fim) = (dentro.span.start, dentro.span.end);
                // Escrita exatamente como `${x}`.
                if !simples || ini < 2 || b.get(ini - 1) != Some(&b'{') || b.get(ini - 2) != Some(&b'$') || b.get(fim) != Some(&b'}') {
                    continue;
                }
                let continua = b.get(fim + 1).is_some_and(|x| x.is_ascii_alphanumeric() || *x == b'_' || *x == b'$');
                if !continua {
                    relatar(&c::UNNECESSARY_BRACE_IN_STRING_INTERPS, Span { start: ini - 2, end: fim + 1 }, &[]);
                }
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
        let mut relatos = executar(u, &nomes, &|_| true, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.unico, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn nomes_de_biblioteca() {
        assert!(e_nome_de_biblioteca("a.b_c") && e_nome_de_biblioteca("_a") && e_nome_de_biblioteca(""));
        assert!(!e_nome_de_biblioteca("A.b") && !e_nome_de_biblioteca("a.1b"));
        assert_eq!(achados("library Minha.lib;\n"), vec![("library_names", "Minha.lib".to_string())]);
    }

    #[test]
    fn declaracoes() {
        assert_eq!(achados("int g() => 1;\nf() {}\n"), vec![("always_declare_return_types_of_functions", "f".to_string())]);
        assert_eq!(
            achados("class A {\n  var x;\n  m() {}\n  operator []=(int i, int v) {}\n}\n"),
            vec![("prefer_typing_uninitialized_variables_for_field", "x".to_string()), ("always_declare_return_types_of_methods", "m".to_string())]
        );
        assert_eq!(achados("int y = 0;\nvar z;\n"), vec![("prefer_typing_uninitialized_variables_for_local_variable", "z".to_string())]);
        assert_eq!(achados("/** doc */\nclass A {}\n/// ok\nclass B {}\n"), vec![("slash_for_doc_comments", "/** doc */".to_string())]);
    }

    #[test]
    fn expressoes() {
        assert_eq!(achados("class A {}\nvar a = new A();\n"), vec![("unnecessary_new", "new".to_string())]);
        assert_eq!(achados("String f(int x) => '${x} e ${x}y e ${x + 1}';\n"), vec![("unnecessary_brace_in_string_interps", "${x}".to_string())]);
    }
}
