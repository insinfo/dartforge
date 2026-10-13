//! O `DeprecatedMemberUseVerifier` do analyzer
//! (`analyzer/lib/src/error/deprecated_member_use_verifier.dart`, lido por
//! inteiro na 3.6.2; docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.8):
//! `deprecated_member_use` e `deprecated_member_use_from_same_package`, com
//! e sem mensagem, no uso de um elemento anotado com o `deprecated` ou o
//! `Deprecated(…)` do `dart:core`; e o lint
//! `deprecated_member_use_from_same_package`, que é o mesmo verificador com
//! o visitante do linter (sem os campos de padrão, e com os parâmetros
//! obrigatórios simples e de campo também abrindo região depreciada), só
//! para os elementos do pacote.
//!
//! Cobre, como o `BaseDeprecatedMemberUseVerifier`: os identificadores
//! (fora de declaração, do nome de `ConstructorName`, do nome de
//! `super.nome(…)` e de `hide`; os de `show` também), o lado esquerdo de
//! atribuição e o operando de `++`/`--` (pelos elementos de leitura e de
//! escrita), os operadores (binário, índice, prefixo, pós-fixo, atribuição
//! composta), o `ConstructorName` (criação, sem `new` também, e tear-off),
//! os tipos nomeados, os campos de padrão de objeto (só no aviso), o
//! `call` implícito, os `super(…)`/`this(…)`, os argumentos passados a
//! parâmetros depreciados não obrigatórios (fora da própria função), e os
//! imports e exports de biblioteca depreciada. Um uso dentro de uma
//! declaração depreciada não é relatado; numa biblioteca depreciada, nada.
//! A mensagem é o `message` do `Deprecated` (a string literal, ou a de uma
//! constante de topo que a tenha).
//!
//! Escrito sem compilar nem executar (2026-10-04; refeito em 2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::table::{Type, TypeTable};
use dartforge_diagnostics::codigos::hint as h;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, Combinator, DeclKind, DirectiveKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, PatternKind, TypeKind, UnaryOp};
use dartforge_intern::Interner;

/// O texto de uma expressão constante de string: a literal, ou o
/// inicializador literal de uma constante de topo.
fn constant_text(program: &Program, u: UnitId, e: ExprId, prof: u32) -> Option<String> {
    let a = &program.unit(u).ast;
    match &a.expr(e).kind {
        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
        ExprKind::Parenthesized(x) => constant_text(program, u, *x, prof),
        ExprKind::Identifier(n) if prof < 8 => {
            let Some(Element::Variable(v)) = program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter) else { return None };
            let x = program.variable(v);
            if !x.const_ {
                return None;
            }
            let VariableRef::TopLevel { unit, decl, index } = x.node else { return None };
            let DeclKind::Variables(l) = &program.unit(unit).ast.decl(decl).kind else { return None };
            constant_text(program, unit, l.variables.get(index)?.initializer?, prof + 1)
        }
        _ => None,
    }
}

/// `ElementAnnotation.isDeprecated` da anotação `m` (escrita na unidade
/// `u`): `Some(mensagem)` quando é o getter `deprecated` (sem mensagem) ou
/// o construtor de `Deprecated` do `dart:core`.
fn deprecated_annotation(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<Option<String>> {
    let do_core = |b: Option<dartforge_elements::model::Binding>| -> Option<Element> {
        let e = b?.getter?;
        let lib = match e {
            Element::Variable(v) => program.variable(v).library,
            Element::Class(c) => program.class(c).library,
            _ => return None,
        };
        (program.library(lib).uri == "dart:core").then_some(e)
    };
    // O construtor nomeado (`Deprecated.nome(…)`): só conta se existe na
    // classe; senão a anotação não resolve (`INVALID_ANNOTATION`, sem
    // `isDeprecated`), como `@Deprecated.extend()` no 3.6.2.
    let (element, the_name, the_constructor) = match &m.name[..] {
        [n] => (do_core(program.lookup_na_unidade(u, n.sym)), n.sym, None),
        [p, n] => match do_core(program.lookup_prefixed_na_unidade(u, p.sym, n.sym)) {
            Some(e) => (Some(e), n.sym, None),
            // `Deprecated.new(…)`, `Deprecated.nome(…)`.
            None => (do_core(program.lookup_na_unidade(u, p.sym)), p.sym, Some(n.sym)),
        },
        [p, c, k] => (do_core(program.lookup_prefixed_na_unidade(u, p.sym, c.sym)), c.sym, Some(k.sym)),
        _ => return None,
    };
    if let (Some(Element::Class(c)), Some(k)) = (element, the_constructor) {
        let key = if interner.resolve(k) == "new" { interner.lookup("") } else { Some(k) };
        if !key.is_some_and(|ch| program.class(c).constructors.contains_key(&ch)) {
            return None;
        }
    }
    let text = interner.resolve(the_name);
    match element? {
        Element::Variable(_) if text == "deprecated" && m.arguments.is_none() => Some(None),
        Element::Class(_) if text == "Deprecated" => {
            let args = m.arguments.as_ref()?;
            let message = args
                .args
                .iter()
                .find(|x| x.name.is_none())
                .or_else(|| args.args.iter().find(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == "message")))
                .and_then(|x| constant_text(program, u, x.value, 0));
            Some(message)
        }
        _ => None,
    }
}

/// `Some(mensagem)` se a lista tem anotação de depreciação (a mensagem da
/// primeira).
fn deprecated(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation]) -> Option<Option<String>> {
    metadata.iter().find_map(|m| deprecated_annotation(program, interner, u, m))
}

/// As anotações de uma função, método ou construtor, com a unidade delas.
pub(crate) fn of_function(program: &Program, f: FunctionElementId) -> Option<(&[ast::Annotation], UnitId)> {
    let e = program.function(f);
    // Acessor implícito: as anotações são as da variável.
    if let Some(v) = e.variable
        && matches!(e.node, FunctionRef::None)
    {
        return of_variable(program, v);
    }
    match e.node {
        FunctionRef::Constructor { unit, member } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return Some((&m.metadata[..], unit));
            }
            a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| (&d.metadata[..], unit))
        }
        FunctionRef::None => e.variable.and_then(|v| of_variable(program, v)),
    }
}

pub(crate) fn of_variable(program: &Program, v: VariableId) -> Option<(&[ast::Annotation], UnitId)> {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, decl, .. } => Some((&program.unit(unit).ast.decl(decl).metadata[..], unit)),
        VariableRef::Field { unit, member, .. } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        VariableRef::EnumConstant { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
            DeclKind::Enum(x) => x.constants.get(index).map(|k| (&k.metadata[..], unit)),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn of_element(program: &Program, e: Element) -> Option<(&[ast::Annotation], UnitId)> {
    let of_declaration = |d: dartforge_elements::model::DeclRef| Some((&program.unit(d.unit).ast.decl(d.decl).metadata[..], d.unit));
    match e {
        Element::Class(c) => program.class(c).decl.and_then(of_declaration),
        Element::Typedef(t) => of_declaration(program.typedef(t).decl),
        Element::Extension(x) => of_declaration(program.extension(x).decl),
        Element::Function(f) => of_function(program, f),
        Element::Variable(v) => of_variable(program, v),
        Element::Prefix(..) => None,
    }
}

/// A biblioteca é depreciada: a diretiva `library` dela tem a anotação.
fn deprecated_library(program: &Program, interner: &Interner, lib: LibraryId) -> Option<Option<String>> {
    let first = *program.library(lib).units.first()?;
    let unit_id = program.unit(first);
    let directive = unit_id.unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. }))?;
    deprecated(program, interner, first, &directive.metadata)
}

/// Quem visita: o `BestPracticesVerifier` (o aviso) ou o visitante do lint.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    Aviso,
    Lint,
}

/// Um uso relatável: o nó, o nome de exibição, a mensagem e a biblioteca do
/// elemento.
struct Uso {
    span: Span,
    nome: String,
    mensagem: Option<String>,
    lib: LibraryId,
}

/// Um elemento depreciável: a metadata dele, a unidade, a biblioteca e o
/// nome de exibição.
struct Alvo<'p> {
    metadata: &'p [ast::Annotation],
    unidade: UnitId,
    nome: String,
}

/// O núcleo do verificador sobre a unidade `u`.
fn verificar(program: &Program, interner: &Interner, table: &TypeTable, body: Option<&UnitBodyTypes>, u: UnitId, mode: Modo) -> Vec<Uso> {
    let mut out: Vec<Uso> = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("deprecated").is_none() && interner.lookup("Deprecated").is_none() {
        return out;
    }
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    // `pushInDeprecatedValue(library.hasDeprecated)`.
    if deprecated_library(program, interner, unit_id.library).is_some() {
        return out;
    }
    let dep = |metadata: &[ast::Annotation]| deprecated(program, interner, u, metadata).is_some();
    // `_inDeprecatedMemberStack`: as declarações, os membros e os parâmetros
    // depreciados desta unidade.
    let mut regions: Vec<Span> = Vec::new();
    regions.extend(a.decls.iter().filter(|d| dep(&d.metadata)).map(|d| d.span));
    regions.extend(a.members.iter().filter(|m| dep(&m.metadata)).map(|m| m.span));
    {
        let mut lists: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = f.parameters.as_deref() {
                lists.push(ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                lists.push(&k.parameters);
            }
        }
        for ps in lists {
            for p in ps {
                // O `DefaultFormalParameter` nos dois; o simples e o de campo
                // obrigatórios só no lint.
                let optional = p.kind != ParameterKind::Required;
                let simple_or_field = !p.super_ && p.function_parameters.is_none();
                if (optional || (mode == Modo::Lint && simple_or_field)) && dep(&p.metadata) {
                    regions.push(p.span);
                }
            }
        }
    }
    let in_deprecated = |s: Span| regions.iter().any(|r| r.start <= s.start && s.end <= r.end);
    let mut report = |span: Span, tgt: Alvo<'_>| {
        if in_deprecated(span) {
            return;
        }
        let Some(message) = deprecated(program, interner, tgt.unidade, tgt.metadata) else { return };
        let lib = program.unit(tgt.unidade).library;
        // O ouvinte do analyzer é um conjunto: um relato por posição.
        if !out.iter().any(|x| x.span == span && x.nome == tgt.nome) {
            out.push(Uso { span, nome: tgt.nome, mensagem: message, lib });
        }
    };
    let text = |s: dartforge_intern::SymbolId| interner.resolve(s).to_string();
    // O nome de exibição de uma função (o do setter sem `=`).
    let function_name = |f: FunctionElementId| -> String {
        let k = program.function(f);
        if matches!(k.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
            let cls = k.class.map_or(String::new(), |c| text(program.class(c).name));
            let ctor = interner.resolve(k.name);
            return if ctor.is_empty() { format!("{cls}.new") } else { format!("{cls}.{ctor}") };
        }
        let t = interner.resolve(k.name);
        t.strip_suffix("_=").or_else(|| t.strip_suffix('=')).unwrap_or(t).to_string()
    };
    let function_target = |f: FunctionElementId| of_function(program, f).map(|(metadata, unit_id)| Alvo { metadata, unidade: unit_id, nome: function_name(f) });
    let element_target = |e: Element| -> Option<Alvo<'_>> {
        let the_name = match e {
            Element::Class(c) => text(program.class(c).name),
            Element::Typedef(t) => text(program.typedef(t).name),
            Element::Extension(x) => program.extension(x).name.map_or(String::new(), text),
            Element::Function(f) => return function_target(f),
            Element::Variable(v) => text(program.variable(v).name),
            Element::Prefix(..) => return None,
        };
        of_element(program, e).map(|(metadata, unit_id)| Alvo { metadata, unidade: unit_id, nome: the_name })
    };
    let variable_target = |v: VariableId| of_variable(program, v).map(|(metadata, unit_id)| Alvo { metadata, unidade: unit_id, nome: text(program.variable(v).name) });
    // O elemento de uma resolução (os locais e parâmetros não entram: o
    // `_isLocalParameter` cala o parâmetro dentro da própria função).
    let resolved_target = |r: &Resolved| -> Option<Alvo<'_>> {
        match r {
            Resolved::Constructor(f) => function_target(*f),
            Resolved::Element(e) => element_target(*e),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => function_target(*f),
            Resolved::Member { member: MemberRef::Variable(v), .. } => variable_target(*v),
            _ => None,
        }
    };
    // O getter que acompanha um setter escrito (o `readElement`).
    let read = |r: &Resolved| -> Option<Alvo<'_>> {
        let f = match r {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Element(Element::Function(f)) => *f,
            _ => return None,
        };
        let g = program.function(f);
        if g.kind != FunctionKind::Setter || g.variable.is_some() {
            return None;
        }
        let t = interner.resolve(g.name);
        let the_name = interner.lookup(t.strip_suffix("_=").unwrap_or(t))?;
        let getter = match (g.class, g.extension) {
            (Some(c), _) => program.class(c).instance_members.get(&the_name).or_else(|| program.class(c).static_members.get(&the_name)).copied(),
            (None, Some(e)) => program.extension(e).instance_members.get(&the_name).or_else(|| program.extension(e).static_members.get(&the_name)).copied(),
            _ => match program.lookup(g.library, the_name).and_then(|b| b.getter) {
                Some(Element::Function(h)) => Some(h),
                _ => None,
            },
        }?;
        function_target(getter)
    };

    // 1. Tipos nomeados (`namedType`): no último nome.
    for (k, t) in a.types.iter().enumerate() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let binding = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        // O parâmetro de tipo de mesmo nome esconde a declaração de topo.
        if body
            .and_then(|c| c.tipos_de_anotacoes.get(&ast::TypeId(k as u32)).copied())
            .is_some_and(|x| matches!(table.get(x), Type::TypeParameter { .. }))
        {
            continue;
        }
        let (Some(element), Some(last)) = (binding.and_then(|b| b.getter), name.last()) else { continue };
        if matches!(element, Element::Class(_) | Element::Typedef(_))
            && let Some(tgt) = element_target(element)
        {
            report(last.span, tgt);
        }
    }

    // 2. Imports e exports de biblioteca depreciada (a diretiva inteira, com
    // a URI como nome) e os nomes de `show`.
    let library = program.library(unit_id.library);
    let targets = library
        .imports
        .iter()
        .filter(|i| i.unit == u)
        .map(|i| (i.directive, i.library))
        .chain(library.exports.iter().filter(|e| e.unit == u).map(|e| (e.directive, e.library)));
    for (idx, tgt) in targets {
        let Some(directive) = unit_id.unit.directives.get(idx) else { continue };
        // A biblioteca depreciada: a metadata da diretiva `library` dela.
        if let Some(first) = program.library(tgt).units.first().copied()
            && let Some(d) = program.unit(first).unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. }))
        {
            report(directive.span, Alvo { metadata: &d.metadata, unidade: first, nome: program.library(tgt).uri.clone() });
        }
        let the_combinators = match &directive.kind {
            DirectiveKind::Import { combinators, .. } | DirectiveKind::Export { combinators, .. } => combinators,
            _ => continue,
        };
        for c in the_combinators.iter() {
            let Combinator::Show(names) = c else { continue };
            for n in names {
                let Some(b) = program.library(tgt).exported.get(&n.sym) else { continue };
                if let Some(e) = b.getter.or(b.setter)
                    && let Some(x) = element_target(e)
                {
                    report(n.span, x);
                }
            }
        }
    }

    let Some(body) = body else { return out };

    // Os lados esquerdos de atribuição e os operandos de `++`/`--` (sem
    // `staticElement`).
    let mut written_ones: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } => {
                written_ones.insert(*target);
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                written_ones.insert(*operand);
            }
            _ => {}
        }
    }
    // O nome de um nó de escrita: o identificador, ou o nome da propriedade.
    let node_name = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Property { name, .. } => name.span,
        _ => a.expr(x).span,
    };
    // O operador resolvido no próprio nó (`staticElement`).
    let operator = |e: ExprId| -> Option<Alvo<'_>> {
        match body.get_resolved(e)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => function_target(*f),
            _ => None,
        }
    };
    // `_invocationArguments`: os argumentos passados a parâmetros
    // depreciados não obrigatórios, fora da função que os declara.
    let args_list = |f: FunctionElementId, args: &ast::Arguments, the_call: Span, report: &mut dyn FnMut(Span, Alvo<'_>)| {
        let (ps, pu, owner) = match program.function(f).node {
            FunctionRef::Function { unit, function } => {
                let x = program.unit(unit).ast.function(function);
                (x.parameters.as_deref().unwrap_or(&[]), unit, x.span)
            }
            FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (&k.parameters[..], unit, program.unit(unit).ast.member(member).span),
                _ => return,
            },
            FunctionRef::None => return,
        };
        let local = pu == u && owner.start <= the_call.start && the_call.end <= owner.end;
        if local {
            return;
        }
        let mut positional_one = 0;
        for x in args.args.iter() {
            let p = match x.name {
                Some(n) => ps.iter().find(|p| p.kind == ParameterKind::Named && p.nome_externo().is_some_and(|m| m.sym == n.sym)),
                None => {
                    let p = ps.get(positional_one);
                    positional_one += 1;
                    p.filter(|p| p.kind != ParameterKind::Named)
                }
            };
            let Some(p) = p else { continue };
            if p.kind == ParameterKind::Required || p.required {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            let span = match x.name {
                Some(n) => n.span,
                None => a.expr(x.value).span,
            };
            report(span, Alvo { metadata: &p.metadata, unidade: pu, nome: text(the_name.sym) });
        }
    };
    // O elemento invocado por uma chamada de método.
    let invoked = |tgt: ExprId| -> Option<FunctionElementId> {
        match body.get_resolved(tgt)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Element(Element::Function(f)) => {
                (program.function(*f).kind != FunctionKind::Getter).then_some(*f)
            }
            _ => None,
        }
    };

    // 3. As expressões.
    for (i, expr) in a.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        match &expr.kind {
            // `simpleIdentifier` (o nome de propriedade também).
            ExprKind::Identifier(_) | ExprKind::Property { .. } if !written_ones.contains(&id) => {
                let Some(r) = body.get_resolved(id) else { continue };
                if matches!(r, Resolved::Constructor(_)) {
                    // O tear-off `C.nome`/`C.new`: o `ConstructorName`.
                    if let Some(tgt) = resolved_target(r) {
                        report(expr.span, tgt);
                    }
                    continue;
                }
                if let Some(tgt) = resolved_target(r) {
                    report(node_name(id), tgt);
                }
            }
            ExprKind::Assign { target, .. } => {
                // `readElement`/`writeElement` no nome, e o operador no nó.
                if let Some(r) = body.get_resolved(*target) {
                    if let Some(tgt) = resolved_target(r) {
                        report(node_name(*target), tgt);
                    }
                    if let Some(tgt) = read(r) {
                        report(node_name(*target), tgt);
                    }
                }
                if let Some(tgt) = operator(id) {
                    report(expr.span, tgt);
                }
            }
            ExprKind::Unary { op, operand } => {
                if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec)
                    && let Some(r) = body.get_resolved(*operand)
                {
                    // No operando inteiro.
                    let sp = a.expr(*operand).span;
                    if let Some(tgt) = resolved_target(r) {
                        report(sp, tgt);
                    }
                    if let Some(tgt) = read(r) {
                        report(sp, tgt);
                    }
                }
                if !matches!(op, UnaryOp::NullAssert)
                    && let Some(tgt) = operator(id)
                {
                    report(expr.span, tgt);
                }
            }
            ExprKind::Binary { .. } | ExprKind::Index { .. } => {
                if let Some(tgt) = operator(id) {
                    report(expr.span, tgt);
                }
            }
            ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    // O `ConstructorName`: do tipo ao nome do construtor.
                    let end = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                    if let Some(tgt) = function_target(*f) {
                        report(Span { start: a.ty(*ty).span.start, end: end }, tgt);
                    }
                    args_list(*f, arguments, expr.span, &mut report);
                }
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    // A criação sem `new`: o `ConstructorName` é o alvo; o tipo
                    // dele é o identificador da classe (ou do typedef).
                    if let Some(tgt) = function_target(*f) {
                        report(a.expr(*target).span, tgt);
                    }
                    let mut x = *target;
                    loop {
                        match &a.expr(x).kind {
                            ExprKind::TypeArguments { target: t, .. } => x = *t,
                            ExprKind::Property { target: t, .. } if !matches!(body.get_resolved(x), Some(Resolved::Element(_))) => x = *t,
                            _ => break,
                        }
                    }
                    let typ = match body.get_resolved(x) {
                        Some(Resolved::Element(e @ (Element::Class(_) | Element::Typedef(_)))) => Some(*e),
                        _ => program.function(*f).class.map(Element::Class),
                    };
                    if let Some(e) = typ
                        && let Some(tgt) = element_target(e)
                    {
                        report(node_name(x), tgt);
                    }
                    args_list(*f, arguments, expr.span, &mut report);
                    continue;
                }
                if crate::lints_tipados4::e_invocacao_de_metodo(program, a, body, *target) {
                    if let Some(f) = invoked(*target) {
                        args_list(f, arguments, expr.span, &mut report);
                    }
                    continue;
                }
                // `functionExpressionInvocation`: o `call` de um objeto de
                // classe chamado como função.
                if let Some(t) = body.get_type(*target)
                    && let Type::Interface { class, nullable: false, .. } = table.get(t)
                    && let Some(call) = interner.lookup("call")
                    && let Some(f) = class_member(program, *class, call)
                    && program.function(f).kind == FunctionKind::Function
                    && let Some(tgt) = function_target(f)
                {
                    report(expr.span, tgt);
                }
            }
            _ => {}
        }
    }

    // 4. `super(…)` e `this(…)`: o nó inteiro, os argumentos, e o nome de
    // `this.nome(…)` (o identificador dele tem o construtor).
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let cls = (0..program.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| matches!(program.function(*f).node, FunctionRef::Constructor { unit, member } if unit == u && std::ptr::eq(a.member(member), m)))
            .and_then(|f| program.function(f).class);
        let Some(cls) = cls else { continue };
        for i in k.initializers.iter() {
            let (span, owner, the_name, args, redirection) = match i {
                Initializer::Super { span, constructor, arguments } => (*span, program.class(cls).supertype_class, *constructor, arguments, false),
                Initializer::Redirect { span, constructor, arguments } => (*span, Some(cls), *constructor, arguments, true),
                _ => continue,
            };
            let Some(owner) = owner else { continue };
            let key = match the_name {
                Some(n) => Some(n.sym),
                None => interner.lookup(""),
            };
            let Some(f) = key.and_then(|c| program.class(owner).constructors.get(&c).copied()) else { continue };
            if let Some(tgt) = function_target(f) {
                report(span, tgt);
            }
            if redirection
                && let Some(n) = the_name
                && let Some(tgt) = function_target(f)
            {
                report(n.span, tgt);
            }
            args_list(f, args, span, &mut report);
        }
    }

    // 5. Os campos de padrão de objeto (só no aviso): no nome do campo, ou no
    // da variável do atalho.
    if mode == Modo::Aviso {
        for p in a.patterns.iter() {
            let PatternKind::Object { ty, fields } = &p.kind else { continue };
            let cls = match body.tipos_de_anotacoes.get(ty).map(|x| table.get(*x)) {
                Some(Type::Interface { class, .. }) => Some(*class),
                Some(Type::ExtensionType { decl, .. }) => Some(*decl),
                _ => None,
            };
            let Some(cls) = cls else { continue };
            for f in fields.iter() {
                let the_name = match f.name {
                    Some(n) => Some((n.sym, n.span)),
                    None => {
                        let mut q = f.pattern;
                        loop {
                            match &a.pattern(q).kind {
                                PatternKind::Variable { name, .. } => break Some((name.sym, name.span)),
                                PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => q = *x,
                                _ => break None,
                            }
                        }
                    }
                };
                let Some((n, sp)) = the_name else { continue };
                if let Some(getter) = class_member(program, cls, n)
                    && let Some(tgt) = function_target(getter)
                {
                    report(sp, tgt);
                }
            }
        }
    }
    out
}

/// O membro de instância `nome` da classe ou de um supertipo dela.
fn class_member(program: &Program, cls: dartforge_elements::model::ClassId, the_name: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
    let mut seen: Vec<dartforge_elements::model::ClassId> = Vec::new();
    let mut stack = vec![cls];
    while let Some(c) = stack.pop() {
        if seen.contains(&c) {
            continue;
        }
        seen.push(c);
        let x = program.class(c);
        if let Some(f) = x.instance_members.get(&the_name) {
            return Some(*f);
        }
        stack.extend(x.interface_classes.iter().copied());
        stack.extend(x.on_classes.iter().copied());
        stack.extend(x.mixin_classes.iter().copied());
        stack.extend(x.supertype_class);
    }
    None
}

/// Os usos de elementos depreciados na unidade `u` (o aviso). `mesmo_pacote`
/// diz se uma biblioteca está no pacote da biblioteca analisada
/// (`_isLibraryInWorkspacePackage`).
pub fn usos_de_deprecados(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    same_package: &dyn Fn(LibraryId) -> bool,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for x in verificar(program, interner, table, body, u, Modo::Aviso) {
        let the_same = same_package(x.lib);
        let d = match x.mensagem.as_deref().map(str::trim).filter(|m| !m.is_empty() && *m != ".") {
            None => {
                let code = if the_same { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE } else { h::DEPRECATED_MEMBER_USE };
                Diagnostic::com_codigo(code, x.span, [x.nome.as_str()])
            }
            Some(m) => {
                let dotted = if m.ends_with(['.', '?', '!']) { m.to_string() } else { format!("{m}.") };
                let code =
                    if the_same { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE } else { h::DEPRECATED_MEMBER_USE_WITH_MESSAGE };
                Diagnostic::com_codigo(code, x.span, [x.nome.as_str(), dotted.as_str()])
            }
        };
        if !out.iter().any(|y| y.code == d.code && y.span == d.span) {
            out.push(d);
        }
    }
    out
}

/// O lint `deprecated_member_use_from_same_package`: os usos de elementos
/// do pacote (os de fora o analyzer já relata como aviso).
pub fn lint_do_mesmo_pacote(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    same_package: &dyn Fn(LibraryId) -> bool,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    for x in verificar(program, interner, table, body, u, Modo::Lint) {
        if !same_package(x.lib) {
            continue;
        }
        let normalized = x.mensagem.as_deref().map(str::trim);
        match normalized.filter(|m| !m.is_empty() && *m != ".") {
            None => out.push((x.span, "deprecated_member_use_from_same_package_without_message", vec![x.nome])),
            Some(_) => {
                // O emissor do lint acrescenta o ponto à mensagem original
                // (sem o `trim`).
                let original = x.mensagem.clone().unwrap_or_default();
                let m = normalized.unwrap_or_default();
                let dotted = if m.ends_with(['.', '?', '!']) { m.to_string() } else { format!("{original}.") };
                out.push((x.span, "deprecated_member_use_from_same_package_with_message", vec![x.nome, dotted]));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// `DEPRECATED_OPTIONAL` (o `DeprecatedFunctionalityVerifier` do 3.13.4,
// `analyzer/lib/src/error/deprecated_functionality_verifier.dart`, lido por
// inteiro na tag; roda no `BestPracticesVerifier`).

/// O `deprecationKind` de uma anotação (`ElementAnnotationImpl` do 3.13.4):
/// `use` para o getter `deprecated` e o `Deprecated(…)`/`Deprecated.new(…)`;
/// o nome do construtor para `Deprecated.extend(…)`, `.implement(…)`,
/// `.subclass(…)`, `.instantiate(…)`, `.mixin(…)` e `.optional(…)` (o
/// `_DeprecationKind` do `dart:core` do 3.13.4, `annotations.dart:222-230`).
///
/// O nosso `dart:core` é o 3.6.2, que não tem esses construtores: a regra só
/// vale para a biblioteca julgada pelo 3.13.4
/// ([`Program::referencia_da_biblioteca`]), em que o oráculo resolve a
/// anotação contra o SDK dele.
fn deprecation_type(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<String> {
    let do_core = |b: Option<dartforge_elements::model::Binding>| -> Option<Element> {
        let e = b?.getter?;
        let lib = match e {
            Element::Variable(v) => program.variable(v).library,
            Element::Class(c) => program.class(c).library,
            _ => return None,
        };
        (program.library(lib).uri == "dart:core").then_some(e)
    };
    let (element, the_name, the_constructor) = match &m.name[..] {
        [n] => (do_core(program.lookup_na_unidade(u, n.sym)), n.sym, None),
        [p, n] => match do_core(program.lookup_prefixed_na_unidade(u, p.sym, n.sym)) {
            Some(e) => (Some(e), n.sym, None),
            None => (do_core(program.lookup_na_unidade(u, p.sym)), p.sym, Some(n.sym)),
        },
        [p, c, k] => (do_core(program.lookup_prefixed_na_unidade(u, p.sym, c.sym)), c.sym, Some(k.sym)),
        _ => return None,
    };
    let text = interner.resolve(the_name);
    match element? {
        Element::Variable(_) if text == "deprecated" && m.arguments.is_none() && the_constructor.is_none() => Some("use".to_string()),
        Element::Class(_) if text == "Deprecated" => {
            m.arguments.as_ref()?;
            match the_constructor.map(|k| interner.resolve(k)) {
                None | Some("new") => Some("use".to_string()),
                Some(k @ ("extend" | "implement" | "subclass" | "instantiate" | "mixin" | "optional")) => Some(k.to_string()),
                Some(_) => None,
            }
        }
        _ => None,
    }
}

/// `Element.isDeprecatedWithKind(kind)`: alguma anotação de depreciação da
/// lista tem o tipo `kind`.
fn deprecated_with_type(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation], kind: &str) -> bool {
    metadata.iter().any(|m| deprecation_type(program, interner, u, m).as_deref() == Some(kind))
}

/// Os parâmetros formais de uma função ou construtor, com a unidade deles.
fn formal_parameters(program: &Program, f: FunctionElementId) -> Option<(&[ast::Parameter], UnitId)> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => Some((program.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]), unit)),
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((&k.parameters[..], unit)),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

/// `ParameterElement.isOptional`: posicional opcional, ou nomeado não
/// `required`.
fn opcional(p: &ast::Parameter) -> bool {
    match p.kind {
        ParameterKind::Required => false,
        ParameterKind::Named => !p.required,
        _ => true,
    }
}

/// Os avisos `DEPRECATED_OPTIONAL` da unidade `u`: o argumento omitido para
/// um parâmetro anotado com `@Deprecated.optional()`, nas invocações (de
/// método, de construtor, de atalho de ponto e de `this(…)`), nos
/// construtores que passam pelo construtor da superclasse sem ele (os
/// parâmetros `super.` e o `super(…)`) e nos construtores redirecionados.
pub fn opcionais_depreciados(program: &Program, interner: &Interner, body: Option<&UnitBodyTypes>, u: UnitId) -> Vec<Diagnostic> {
    use dartforge_diagnostics::codigos::warning::DEPRECATED_OPTIONAL;
    let mut out: Vec<Diagnostic> = Vec::new();
    let unit_id = program.unit(u);
    if program.referencia_da_biblioteca(unit_id.library) != dartforge_diagnostics::Referencia::V3_13 {
        return out;
    }
    if interner.lookup("Deprecated").is_none() {
        return out;
    }
    let a = &unit_id.ast;
    let text = |s: dartforge_intern::SymbolId| interner.resolve(s).to_string();
    let report = |out: &mut Vec<Diagnostic>, span: Span, the_name: String| {
        let d = Diagnostic::com_codigo(DEPRECATED_OPTIONAL, span, [the_name.as_str()]);
        if !out.iter().any(|y| y.code == d.code && y.span == d.span && y.message == d.message) {
            out.push(d);
        }
    };
    // `_checkForDeprecatedOptional`: os parâmetros que nenhum argumento
    // preenche (o `correspondingParameter`), com o tipo `optional`.
    let omitted = |out: &mut Vec<Diagnostic>, f: FunctionElementId, args: &ast::Arguments, entity: Span| {
        let Some((ps, pu)) = formal_parameters(program, f) else { return };
        let mut filled = vec![false; ps.len()];
        let positional: Vec<usize> = (0..ps.len()).filter(|&i| ps[i].kind != ParameterKind::Named).collect();
        let mut k = 0;
        for x in args.args.iter() {
            match x.name {
                Some(n) => {
                    if let Some(i) = (0..ps.len()).find(|&i| ps[i].kind == ParameterKind::Named && ps[i].nome_externo().is_some_and(|m| m.sym == n.sym)) {
                        filled[i] = true;
                    }
                }
                None => {
                    if let Some(&i) = positional.get(k) {
                        filled[i] = true;
                    }
                    k += 1;
                }
            }
        }
        for (i, p) in ps.iter().enumerate() {
            if !filled[i] && deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
                report(out, entity, the_name);
            }
        }
    };
    if let Some(body) = body {
        for (i, expr) in a.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &expr.kind {
                // `instanceCreationExpression`: no `ConstructorName`.
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                        let end = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                        omitted(&mut out, *f, arguments, Span { start: a.ty(*ty).span.start, end: end });
                    }
                }
                ExprKind::Call { target, arguments } => {
                    // O atalho de ponto: `.id(…)`/`.new(…)` de construtor
                    // (`dotShorthandConstructorInvocation`) e `.m(…)` de
                    // método estático (`dotShorthandInvocation`), no nome.
                    if let ExprKind::DotShorthand { name, .. } = &a.expr(*target).kind {
                        let f = match body.get_resolved(id) {
                            Some(Resolved::Constructor(f)) => Some(*f),
                            _ => match body.get_resolved(*target) {
                                Some(Resolved::Element(Element::Class(d))) => program
                                    .class(*d)
                                    .static_members
                                    .get(&name.sym)
                                    .copied()
                                    .filter(|f| program.function(*f).kind == FunctionKind::Function),
                                _ => None,
                            },
                        };
                        if let Some(f) = f {
                            omitted(&mut out, f, arguments, name.span);
                        }
                        continue;
                    }
                    // A criação sem `new`: no `ConstructorName` (o alvo).
                    if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                        omitted(&mut out, *f, arguments, a.expr(*target).span);
                        continue;
                    }
                    // `methodInvocation`: no nome do método; a função local
                    // (`LocalFunctionElement`) não conta.
                    if crate::lints_tipados4::e_invocacao_de_metodo(program, a, body, *target) {
                        let f = match body.get_resolved(*target) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. })
                            | Some(Resolved::ExtensionMember { member: f, .. })
                            | Some(Resolved::Element(Element::Function(f))) => Some(*f),
                            _ => None,
                        };
                        if let Some(f) = f {
                            let the_name = match &a.expr(*target).kind {
                                ExprKind::Property { name, .. } => name.span,
                                _ => a.expr(*target).span,
                            };
                            omitted(&mut out, f, arguments, the_name);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Os construtores declarados (e os primários elaborados).
    for (mi, m) in a.members.iter().enumerate() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let Some(f) = (0..program.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| matches!(program.function(*f).node, FunctionRef::Constructor { unit, member } if unit == u && member.0 as usize == mi))
        else {
            continue;
        };
        let Some(cls) = program.function(f).class else { continue };
        // `ConstructorDeclaration.errorRange`: do nome da classe ao nome do
        // construtor; no primário com `const`, desde o `const`
        // (`PrimaryConstructorDeclaration.errorRange`, o `beginToken`).
        let primary = a.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(c) => c.primary_constructor.is_some_and(|p| p.0 as usize == mi),
            DeclKind::Enum(e) => e.primary_constructor.is_some_and(|p| p.0 as usize == mi),
            _ => false,
        });
        let start = if primary && k.const_ { m.span.start.min(k.class_name.span.start) } else { k.class_name.span.start };
        let error_range = Span { start: start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) };

        // `this(…)`/`this.nome(…)` na lista de inicializadores: no nome do
        // construtor, ou no `this`.
        for i in k.initializers.iter() {
            if let Initializer::Redirect { span, constructor, arguments } = i {
                let key = match constructor {
                    Some(n) => Some(n.sym),
                    None => interner.lookup(""),
                };
                if let Some(g) = key.and_then(|c| program.class(cls).constructors.get(&c).copied()) {
                    let entity = constructor.map_or(Span { start: span.start, end: span.start + 4 }, |n| n.span);
                    omitted(&mut out, g, arguments, entity);
                }
            }
        }

        // `_checkForDeprecatedOptionalRedirectedParameters`: a fábrica
        // redirecionadora, contra os parâmetros do construtor alvo.
        if let Some(r) = &k.redirect
            && let Some(g) = redirect_target(program, interner, u, r)
            && let Some((ps, pu)) = formal_parameters(program, g)
        {
            let positional = k.parameters.iter().filter(|p| p.kind != ParameterKind::Named).count();
            let named: Vec<dartforge_intern::SymbolId> =
                k.parameters.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
            let mut count = 0;
            for p in ps.iter() {
                if p.kind != ParameterKind::Named {
                    count += 1;
                }
                if !opcional(p) || !deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                    continue;
                }
                if p.kind != ParameterKind::Named {
                    if count <= positional {
                        continue;
                    }
                } else if p.name.is_some_and(|n| named.contains(&n.sym)) {
                    continue;
                }
                let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
                report(&mut out, error_range, the_name);
            }
        }

        // `_checkForDeprecatedOptionalSuperParameters`: o construtor da
        // superclasse que este invoca (o `super(…)`, ou o sem nome
        // implícito do gerador que não redireciona).
        if k.factory || k.redirect.is_some() || k.initializers.iter().any(|i| matches!(i, Initializer::Redirect { .. })) {
            continue;
        }
        let supers: Vec<&Initializer> = k.initializers.iter().filter(|i| matches!(i, Initializer::Super { .. })).collect();
        if supers.len() > 1 {
            continue;
        }
        let Some(superclass) = program.class(cls).supertype_class else { continue };
        let (super_name, super_arguments, range) = match supers.first() {
            Some(Initializer::Super { span, constructor, arguments }) => {
                let entity = constructor.map_or(Span { start: span.start, end: span.start + 5 }, |n| n.span);
                (constructor.map(|n| n.sym), Some(arguments), entity)
            }
            _ => (None, None, error_range),
        };
        let key = match super_name {
            Some(n) => Some(n),
            None => interner.lookup(""),
        };
        let Some(g) = key.and_then(|c| program.class(superclass).constructors.get(&c).copied()) else { continue };
        let Some((ps, pu)) = formal_parameters(program, g) else { continue };
        // `verifySuperFormalParameters`: os `super.` posicionais e os nomes
        // dos `super.` nomeados.
        let super_positional = k.parameters.iter().filter(|p| p.super_ && p.kind != ParameterKind::Named).count();
        let super_named: Vec<dartforge_intern::SymbolId> =
            k.parameters.iter().filter(|p| p.super_ && p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let (the_super_named, the_super_positional) = match super_arguments {
            Some(args) => (
                args.args.iter().filter_map(|x| x.name.map(|n| n.sym)).collect::<Vec<_>>(),
                args.args.iter().filter(|x| x.name.is_none()).count(),
            ),
            None => (Vec::new(), 0),
        };
        let mut count = 0;
        for p in ps.iter() {
            if p.kind != ParameterKind::Named {
                count += 1;
            }
            if !opcional(p) || !deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                continue;
            }
            if p.kind != ParameterKind::Named {
                if count <= super_positional + the_super_positional {
                    continue;
                }
            } else {
                let n = p.name.map(|n| n.sym);
                if n.is_some_and(|n| super_named.contains(&n) || the_super_named.contains(&n)) {
                    continue;
                }
            }
            let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
            report(&mut out, range, the_name);
        }
    }
    out
}

/// O construtor alvo de `factory C(…) = D.nome;` (o `redirectedConstructor`).
fn redirect_target(program: &Program, interner: &Interner, u: UnitId, r: &ast::RedirectTarget) -> Option<FunctionElementId> {
    let a = &program.unit(u).ast;
    let TypeKind::Named { name, .. } = &a.ty(r.ty).kind else { return None };
    let binding = match &name[..] {
        [n] => program.lookup_na_unidade(u, n.sym),
        [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
        _ => None,
    }?;
    let Element::Class(c) = binding.getter? else { return None };
    let key = match r.constructor {
        Some(n) => Some(n.sym),
        None => interner.lookup(""),
    }?;
    program.class(c).constructors.get(&key).copied()
}
