//! Verificações do `BestPracticesVerifier` sobre declarações e tipos escritos
//! (docs/ANALYZER-ESPECIFICACAO.md §6, família B):
//!
//! * `non_nullable_equals_parameter` (`_checkForNullableEqualsParameterType`,
//!   `an611:src/error/best_practices_verifier.dart:1150-1184`);
//! * `unnecessary_no_such_method` (`_checkForUnnecessaryNoSuchMethod`,
//!   `:1234-1280`);
//! * `unnecessary_question_mark` (`visitNamedType`, `:658-676`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::BodyTypes;
use crate::table::{CoreTypes, Exibicao, Type, TypeId, TypeTable};
use dartforge_diagnostics::{codigos::warning as w, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, ExprKind, FunctionBody, StmtKind};
use dartforge_intern::{Interner, SymbolId};

/// `TypeSystemImpl.isNullable`.
fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Never => false,
        Type::Interface { nullable, .. }
        | Type::Function { nullable, .. }
        | Type::Record { nullable, .. }
        | Type::TypeParameter { nullable, .. }
        | Type::ExtensionType { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Intersection { bound, .. } => anulavel(table, *bound),
    }
}

/// `NON_NULLABLE_EQUALS_PARAMETER`: um `operator ==` com exatamente um
/// parâmetro (de qualquer espécie) cujo tipo, escrito ou herdado, é
/// `Object?` ou `dynamic` — no nome do operador.
pub fn parametro_de_igualdade_anulavel(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || (f.class.is_none() && f.extension.is_none()) || interner.resolve(f.name) != "==" {
            continue;
        }
        let FunctionRef::Function { unit, function } = f.node else { continue };
        let af = program.unit(unit).ast.function(function);
        let Some(ps) = &af.parameters else { continue };
        if ps.len() != 1 {
            continue;
        }
        let Some(p) = outline.functions.get(i).and_then(|d| d.parameters.first()) else { continue };
        if matches!(table.exibicao(p.ty), Some(Exibicao::Invalido)) {
            continue;
        }
        let objeto_ou_dynamic = match table.get(p.ty) {
            Type::Dynamic => true,
            Type::Interface { class, .. } => Some(*class) == core.object_class,
            _ => false,
        };
        if !objeto_ou_dynamic || !anulavel(table, p.ty) {
            continue;
        }
        let Some(nome) = af.name else { continue };
        saida.push((unit, Diagnostic::com_codigo(w::NON_NULLABLE_EQUALS_PARAMETER, nome.span, Vec::<&str>::new())));
    }
    saida
}

/// A implementação de `nome` que `super.nome` alcança a partir de `c`
/// (`getMember(…, forSuper: true)`): os mixins de `c` do último ao primeiro,
/// depois a superclasse, os mixins dela e assim por diante; só membros
/// concretos.
fn membro_do_super(program: &Program, c: ClassId, nome: SymbolId) -> Option<FunctionElementId> {
    let concreto = |f: FunctionElementId| !program.function(f).abstract_;
    let mut atual = c;
    let mut primeiro = true;
    for _ in 0..256 {
        let classe = program.class(atual);
        if !primeiro
            && let Some(&f) = classe.instance_members.get(&nome)
            && concreto(f)
        {
            return Some(f);
        }
        for &m in classe.mixin_classes.iter().rev() {
            if let Some(&f) = program.class(m).instance_members.get(&nome)
                && concreto(f)
            {
                return Some(f);
            }
        }
        primeiro = false;
        atual = classe.supertype_class?;
    }
    None
}

/// `UNNECESSARY_NO_SUCH_METHOD`: um método `noSuchMethod` cujo corpo é só
/// `super.noSuchMethod(x)` (com `=>` ou `{ return …; }`, um argumento) e cujo
/// `noSuchMethod` chamado é um método de uma classe que não é `Object` — no
/// nome do método.
pub fn no_such_method_desnecessario(program: &Program, interner: &Interner, core: &CoreTypes, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for f in program.functions.iter() {
        if f.library != lib || interner.resolve(f.name) != "noSuchMethod" || f.kind != FunctionKind::Function {
            continue;
        }
        let (Some(c), FunctionRef::Function { unit, function }) = (f.class, f.node) else { continue };
        let a = &program.unit(unit).ast;
        let af = a.function(function);
        let invocacao = match af.body {
            FunctionBody::Expression(e) => Some(e),
            FunctionBody::Block(s) => match &a.stmt(s).kind {
                StmtKind::Block(corpo) if corpo.len() == 1 => match &a.stmt(corpo[0]).kind {
                    StmtKind::Return(Some(e)) => Some(*e),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(e) = invocacao else { continue };
        let ExprKind::Call { target, arguments } = &a.expr(e).kind else { continue };
        if arguments.args.len() != 1 {
            continue;
        }
        let ExprKind::Property { target: receptor, name, null_aware: false } = &a.expr(*target).kind else { continue };
        if !matches!(a.expr(*receptor).kind, ExprKind::Super) || interner.resolve(name.sym) != "noSuchMethod" {
            continue;
        }
        let Some(chamado) = membro_do_super(program, c, f.name) else { continue };
        let fc = program.function(chamado);
        let Some(dona) = fc.class else { continue };
        // `methodElement is MethodElement && classElement is ClassElement`.
        if fc.kind != FunctionKind::Function
            || !matches!(program.class(dona).kind, ClassKind::Class | ClassKind::MixinApplication)
            || Some(dona) == core.object_class
        {
            continue;
        }
        let Some(nome) = af.name else { continue };
        saida.push((unit, Diagnostic::com_codigo(w::UNNECESSARY_NO_SUCH_METHOD, nome.span, Vec::<&str>::new())));
    }
    saida
}

/// `UNNECESSARY_QUESTION_MARK`: um tipo nomeado com `?` que resolve para o
/// `Null` do `dart:core`, ou `dynamic` escrito assim, sem alias — no `?`, com
/// o nome qualificado escrito.
pub fn interrogacoes_desnecessarias(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    corpos: Option<&BodyTypes>,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for &u in &program.library(lib).units {
        let a = &program.unit(u).ast;
        for (i, ty) in a.types.iter().enumerate() {
            if !ty.nullable {
                continue;
            }
            let ast::TypeKind::Named { name, .. } = &ty.kind else { continue };
            let id = ast::TypeId(i as u32);
            let resolvido = outline
                .tipos_escritos
                .get(&(u, id))
                .copied()
                .or_else(|| corpos.and_then(|c| c.units.get(u.0 as usize)).and_then(|b| b.tipos_de_anotacoes.get(&id).copied()));
            let Some(t) = resolvido else { continue };
            if matches!(table.exibicao(t), Some(Exibicao::Alias { .. } | Exibicao::Invalido | Exibicao::NeverAnulavel)) {
                continue;
            }
            let ultimo = name.last().map(|n| interner.resolve(n.sym)).unwrap_or("");
            let relata = match table.get(t) {
                Type::Null => true,
                Type::Dynamic => ultimo == "dynamic",
                _ => false,
            };
            if !relata || ty.span.end == ty.span.start {
                continue;
            }
            let texto: Vec<&str> = name.iter().map(|n| interner.resolve(n.sym)).collect();
            let qualificado = texto.join(".");
            let sp = Span { start: ty.span.end - 1, end: ty.span.end };
            saida.push((u, Diagnostic::com_codigo(w::UNNECESSARY_QUESTION_MARK, sp, [qualificado.as_str()])));
        }
    }
    saida
}
