//! O vigésimo lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `document_ignores`: o comentário `// ignore:`/`// ignore_for_file:`
//!   (os `ignoreComments`) com elementos e sem texto livre depois deles
//!   (`IgnoredDiagnosticComment`), sem comentário `//` na linha de cima.
//! * `sort_child_properties_last`: o `child:`/`children:` de um widget com
//!   argumentos nomeados que não são closures depois dele.
//! * `test_types_in_equals`: `outro as T` no `operator ==` com o parâmetro.
//! * `matching_super_parameters`: os `super.x` posicionais com o nome
//!   diferente do parâmetro do construtor da superclasse.
//! * `unnecessary_null_aware_operator_on_extension_on_nullable`: `?.`/`?[`
//!   que chama membro de extensão sobre tipo anulável.
//! * `avoid_slow_async_io`: os métodos assíncronos de `File`, `Directory` e
//!   `FileSystemEntity` do `dart:io`.
//! * `tighten_type_of_initializing_formals`: o `assert(x != null)` de um
//!   `this.x`/`super.x` anulável.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionRef, LibraryId};
use dartforge_frontend::ast::{BinaryOp, DeclKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};

fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Never => false,
    }
}

fn nome_da_biblioteca(s: &super::Semantica<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

/// `isSameAs`.
fn mesma(s: &super::Semantica<'_>, interner: &Interner, c: ClassId, nome: &str, lib: &str) -> bool {
    let k = s.program.class(c);
    interner.resolve(k.name) == nome && nome_da_biblioteca(s, interner, k.library).as_deref() == Some(lib)
}

/// `extendsClass`: a cadeia de superclasses.
fn estende(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, nome: &str, lib: &str) -> bool {
    let Type::Interface { class, .. } = s.table.get(t) else { return false };
    let mut vistos = std::collections::HashSet::new();
    let mut atual = Some(*class);
    while let Some(x) = atual {
        if !vistos.insert(x) {
            return false;
        }
        if mesma(s, interner, x, nome, lib) {
            return true;
        }
        atual = s.program.class(x).supertype_class;
    }
    false
}

/// `isWidgetProperty`.
pub(super) fn propriedade_de_widget(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, prof: u32) -> bool {
    if prof > 8 {
        return false;
    }
    if super::flutter::e_widget_tipo(s, interner, t) {
        return true;
    }
    let mut x = t;
    if let Type::TypeParameter { param, .. } | Type::Intersection { param, .. } = s.table.get(x) {
        x = s.table.param(*param).bound;
    }
    let Type::Interface { class, args, .. } = s.table.get(x) else { return false };
    let colecoes = [("List", "dart.core"), ("Map", "dart.core"), ("LinkedHashMap", "dart.collection"), ("Set", "dart.core"), ("LinkedHashSet", "dart.collection")];
    let e_colecao = |c: ClassId| colecoes.iter().any(|(n, l)| mesma(s, interner, c, n, l));
    let implementa = e_colecao(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e_colecao(*k))));
    if !implementa {
        return false;
    }
    let params = s.outline.classes.get(class.0 as usize).map(|d| d.type_params.len()).unwrap_or(0);
    params == 1 && args.first().is_some_and(|a| propriedade_de_widget(s, interner, *a, prof + 1))
}

/// Os elementos de um comentário `ignore` (`ignoredElements`): `(há
/// elementos, o último é texto livre)`.
fn elementos_de_ignore(lexema: &str) -> (bool, bool) {
    let b = lexema.as_bytes();
    let n = b.len();
    let Some(dp) = lexema.find(':') else { return (false, false) };
    let mut i = dp + 1;
    let branco = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r');
    let espaco = |c: u8| matches!(c, b' ' | b'\t');
    let palavra = |i: &mut usize| {
        if *i < n && b[*i].is_ascii_alphabetic() {
            *i += 1;
            while *i < n && (b[*i].is_ascii_alphanumeric() || b[*i] == b'_') {
                *i += 1;
            }
        }
    };
    let mut tem = false;
    loop {
        while i < n && branco(b[i]) {
            i += 1;
        }
        if i == n {
            return (tem, false);
        }
        let ini = i;
        palavra(&mut i);
        if ini == i {
            return (tem, tem);
        }
        if lexema[ini..i].eq_ignore_ascii_case("type") {
            while i < n && branco(b[i]) {
                i += 1;
            }
            if i == n || b[i] != b'=' {
                return (tem, false);
            }
            i += 1;
            while i < n && branco(b[i]) {
                i += 1;
            }
            if i == n {
                return (tem, false);
            }
            let tini = i;
            palavra(&mut i);
            if tini == i {
                return (tem, tem);
            }
            if i < n && !espaco(b[i]) && b[i] != b',' {
                return (tem, tem);
            }
            tem = true;
        } else {
            if i < n && !espaco(b[i]) && b[i] != b',' {
                return (tem, tem);
            }
            tem = true;
        }
        if i == n {
            return (tem, false);
        }
        while i < n && branco(b[i]) {
            i += 1;
        }
        if i == n {
            return (tem, false);
        }
        if b[i] != b',' {
            return (tem, true);
        }
        i += 1;
        if i == n {
            return (tem, false);
        }
    }
}

/// O token `?.` ou `?` depois de `alvo`.
fn token_null_aware(fonte: &str, fim_do_alvo: usize, tamanho: usize) -> Span {
    let i = dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), fim_do_alvo);
    Span { start: i, end: (i + tamanho).min(fonte.len()) }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `document_ignores`.
    if ligada("document_ignores") {
        let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
        let mut inicios = vec![0usize];
        let bs = fonte.as_bytes();
        let mut i = 0;
        while i < bs.len() {
            match bs[i] {
                b'\r' => {
                    if bs.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    inicios.push(i + 1);
                }
                b'\n' => inicios.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        for sp in comentarios.todos() {
            let lexema = &fonte[sp.start..sp.end];
            // `//+[ ]*ignore:` ou `//[ ]*ignore_for_file:` no começo.
            let depois = lexema.trim_start_matches('/');
            let barras = lexema.len() - depois.len();
            let corpo = depois.trim_start_matches(' ');
            let e_ignore = barras >= 2 && corpo.starts_with("ignore:");
            let e_arquivo = barras == 2 && corpo.starts_with("ignore_for_file:");
            if !e_ignore && !e_arquivo {
                continue;
            }
            let (tem, ultimo_livre) = elementos_de_ignore(lexema);
            if !tem || ultimo_livre {
                continue;
            }
            let linha = inicios.partition_point(|&x| x <= sp.start);
            if linha > 1 {
                let anterior = inicios[linha - 2];
                let mut o = anterior;
                while o < bs.len() && matches!(bs[o], b' ' | b'\t') {
                    o += 1;
                }
                if o + 1 < bs.len() && bs[o] == b'/' && bs[o + 1] == b'/' {
                    continue;
                }
            }
            relatar(&c::DOCUMENT_IGNORES, *sp, &[]);
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;
    let table = s.table;

    // `sort_child_properties_last`.
    if ligada("sort_child_properties_last") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !matches!(s.corpo.get_resolved(id), Some(Resolved::Constructor(_))) {
                continue;
            }
            if !s.corpo.get_type(id).is_some_and(|t| super::flutter::e_widget_tipo(s, interner, t)) {
                continue;
            }
            let args = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            let e_filho = |x: &dartforge_frontend::ast::Argument| {
                x.name.is_some_and(|n| matches!(interner.resolve(n.sym), "child" | "children"))
                    && s.corpo.get_type(x.value).is_some_and(|t| propriedade_de_widget(s, interner, t, 0))
            };
            let n = args.args.len();
            if n < 2 || e_filho(&args.args[n - 1]) || args.args.iter().filter(|x| e_filho(x)).count() != 1 {
                continue;
            }
            let depois: Vec<&dartforge_frontend::ast::Argument> = args.args.iter().rev().take_while(|x| !e_filho(x)).collect();
            let so_closures = depois.iter().all(|x| x.name.is_none() || matches!(a.expr(x.value).kind, ExprKind::FunctionExpression(_)));
            if !so_closures {
                let filho = args.args.iter().find(|x| e_filho(x)).expect("um filho");
                let nome = filho.name.expect("nomeado");
                relatar(&c::SORT_CHILD_PROPERTIES_LAST, Span { start: nome.span.start, end: a.expr(filho.value).span.end }, &[interner.resolve(nome.sym)]);
            }
        }
    }

    // `test_types_in_equals`.
    if ligada("test_types_in_equals") {
        // Os `operator ==` de um parâmetro, com o nome do tipo da declaração.
        let mut iguais: Vec<(Span, dartforge_intern::SymbolId, String)> = Vec::new();
        for d in a.decls.iter() {
            let (membros, nome_tipo) = match &d.kind {
                DeclKind::Class(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Enum(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Mixin(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Extension(x) => (&x.members, dartforge_frontend::fonte::de_tipo(a, fonte, interner, x.on)),
                DeclKind::ExtensionType(x) => (&x.members, {
                    let _ = x;
                    "unknown".to_string()
                }),
                _ => continue,
            };
            for &mid in membros {
                let MemberKind::Method(f) = &a.member(mid).kind else { continue };
                let f = a.function(*f);
                if f.kind != dartforge_frontend::ast::FunctionKind::Operator || f.name.is_none_or(|n| interner.resolve(n.sym) != "==") {
                    continue;
                }
                let Some(ps) = &f.parameters else { continue };
                let [p] = &ps[..] else { continue };
                let Some(pn) = p.name else { continue };
                iguais.push((f.span, pn.sym, nome_tipo.clone()));
            }
        }
        for e in a.exprs.iter() {
            let ExprKind::As { value, .. } = &e.kind else { continue };
            let ExprKind::Identifier(n) = &a.expr(*value).kind else { continue };
            // O método mais próximo que contém a expressão.
            let metodo = a
                .members
                .iter()
                .filter_map(|m| match &m.kind {
                    MemberKind::Method(f) => Some(a.function(*f).span),
                    _ => None,
                })
                .filter(|sp| sp.start <= e.span.start && e.span.end <= sp.end)
                .min_by_key(|sp| sp.end - sp.start);
            let Some(metodo) = metodo else { continue };
            if let Some((_, p, nome)) = iguais.iter().find(|(sp, _, _)| *sp == metodo)
                && *p == n.sym
            {
                relatar(&c::TEST_TYPES_IN_EQUALS, e.span, &[nome.as_str()]);
            }
        }
    }

    // `matching_super_parameters`.
    if ligada("matching_super_parameters") {
        for (k, d) in a.decls.iter().enumerate() {
            let (membros, e_classe) = match &d.kind {
                DeclKind::Class(x) => (&x.members, true),
                DeclKind::Enum(x) => (&x.members, false),
                DeclKind::ExtensionType(x) => (&x.members, false),
                _ => continue,
            };
            let classe = (0..program.classes.len())
                .map(|i| ClassId(i as u32))
                .find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl.0 as usize == k));
            for &mid in membros {
                let MemberKind::Constructor(kc) = &a.member(mid).kind else { continue };
                let posicionais: Vec<&dartforge_frontend::ast::Parameter> = kc.parameters.iter().filter(|p| p.super_ && p.kind != ParameterKind::Named).collect();
                if posicionais.is_empty() {
                    continue;
                }
                let chamada = kc.initializers.iter().find_map(|i| match i {
                    Initializer::Super { constructor, .. } => Some(*constructor),
                    _ => None,
                });
                let Some(sup) = classe.and_then(|c| program.class(c).supertype_class) else { continue };
                let construtor = match chamada {
                    Some(nome) => nome.map(|n| n.sym).or_else(|| interner.lookup("")).and_then(|ch| program.class(sup).constructors.get(&ch).copied()),
                    None if e_classe => interner.lookup("").and_then(|ch| program.class(sup).constructors.get(&ch).copied()),
                    None => None,
                };
                let Some(fsup) = construtor else { continue };
                let FunctionRef::Constructor { unit, member } = program.function(fsup).node else { continue };
                let MemberKind::Constructor(ks) = &program.unit(unit).ast.member(member).kind else { continue };
                let do_super: Vec<String> = ks.parameters.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| interner.resolve(n.sym).to_string())).collect();
                if do_super.len() < posicionais.len() {
                    continue;
                }
                for (p, nome_sup) in posicionais.iter().zip(do_super.iter()) {
                    let Some(n) = p.name else { continue };
                    let nome = interner.resolve(n.sym);
                    if nome != nome_sup {
                        relatar(&c::MATCHING_SUPER_PARAMETERS, p.span, &[nome, nome_sup.as_str()]);
                    }
                }
            }
        }
    }

    // `unnecessary_null_aware_operator_on_extension_on_nullable`.
    if ligada("unnecessary_null_aware_operator_on_extension_on_nullable") {
        let extensao_anulavel = |e: ExprId| -> bool {
            match s.corpo.get_resolved(e) {
                Some(Resolved::ExtensionMember { extension, .. }) => s.outline.extensions.get(extension.0 as usize).is_some_and(|x| anulavel(table, x.on)),
                _ => false,
            }
        };
        let mut alvos_de_atribuicao: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
        for (k, e) in a.exprs.iter().enumerate() {
            if let ExprKind::Assign { target, .. } = &e.kind {
                let mut t = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(t).kind {
                    t = *x;
                }
                alvos_de_atribuicao.insert(t, ExprId(k as u32));
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            match &e.kind {
                ExprKind::Index { target, null_aware: true, .. } => {
                    let elemento = alvos_de_atribuicao.get(&id).copied().unwrap_or(id);
                    if extensao_anulavel(elemento) || extensao_anulavel(id) {
                        relatar(&c::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, token_null_aware(fonte, a.expr(*target).span.end, 1), &[]);
                    }
                }
                ExprKind::Property { target, null_aware: true, .. } => {
                    let elemento = alvos_de_atribuicao.get(&id).copied().unwrap_or(id);
                    if extensao_anulavel(elemento) || extensao_anulavel(id) {
                        relatar(&c::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, token_null_aware(fonte, a.expr(*target).span.end, 2), &[]);
                    }
                }
                _ => {}
            }
        }
    }

    // `avoid_slow_async_io`.
    if ligada("avoid_slow_async_io") {
        for e in a.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
            let metodo = interner.resolve(name.sym);
            if arguments.args.is_empty() {
                let Some(t) = s.corpo.get_type(*alvo) else { continue };
                let arquivo = estende(s, interner, t, "File", "dart.io") && matches!(metodo, "lastModified" | "exists" | "stat");
                let diretorio = estende(s, interner, t, "Directory", "dart.io") && matches!(metodo, "exists" | "stat");
                if arquivo {
                    relatar(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
                }
                if diretorio {
                    relatar(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
                }
            } else if matches!(a.expr(*alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                && let Some(Resolved::Element(Element::Class(cl))) = s.corpo.get_resolved(*alvo)
                && interner.resolve(program.class(*cl).name) == "FileSystemEntity"
                && matches!(metodo, "isDirectory" | "isFile" | "isLink" | "type")
            {
                relatar(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
            }
        }
    }

    // `tighten_type_of_initializing_formals`.
    if ligada("tighten_type_of_initializing_formals") {
        for m in a.members.iter() {
            let MemberKind::Constructor(kc) = &m.kind else { continue };
            for i in kc.initializers.iter() {
                let Initializer::Assert { condition, .. } = i else { continue };
                let ExprKind::Binary { op: BinaryOp::NotEq, left, right } = &a.expr(*condition).kind else { continue };
                let identificador = if matches!(a.expr(*right).kind, ExprKind::Null) {
                    Some(*left)
                } else if matches!(a.expr(*left).kind, ExprKind::Null) {
                    Some(*right)
                } else {
                    None
                };
                let Some(x) = identificador else { continue };
                let ExprKind::Identifier(n) = &a.expr(x).kind else { continue };
                if !s.corpo.get_type(x).is_some_and(|t| anulavel(table, t)) {
                    continue;
                }
                if !matches!(s.corpo.get_resolved(x), Some(Resolved::Parameter { .. })) {
                    continue;
                }
                if let Some(p) = kc.parameters.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym))
                    && (p.this_ || p.super_)
                {
                    relatar(&c::TIGHTEN_TYPE_OF_INITIALIZING_FORMALS, p.span, &[]);
                }
            }
        }
    }

    out
}
