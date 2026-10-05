//! O `OverrideVerifier` do analyzer
//! (`analyzer/lib/src/error/override_verifier.dart`;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, Parte III, etapa 6): um membro de
//! classe, enum ou mixin anotado com `@override` que não sobrescreve nada
//! das superinterfaces — `override_on_non_overriding_member`, nas variantes
//! de método, getter, setter e campo, no nome do membro.
//!
//! "Sobrescreve" é o `getOverridden2` do `InheritanceManager3`: existe, em
//! algum supertipo (direto ou não), um membro de instância com o mesmo nome,
//! visível desta biblioteca (um nome privado só casa com o da mesma
//! biblioteca). Escrito sem compilar nem executar (2026-10-04).

use crate::resolve::OutlineTypes;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, FunctionKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};

/// A lista de anotações tem `@override` (o nome simples, sem argumentos).
fn anotado(metadata: &[ast::Annotation], sym_override: SymbolId) -> bool {
    metadata.iter().any(|m| m.arguments.is_none() && m.name.len() == 1 && m.name[0].sym == sym_override)
}

/// Algum supertipo de `classe` tem um membro de instância com a chave
/// `chave` (o nome, ou `nome_=` para o setter).
fn sobrescreve(program: &Program, outline: &OutlineTypes, interner: &Interner, classe: ClassId, lib: LibraryId, chave: Option<SymbolId>) -> bool {
    let Some(chave) = chave else { return false };
    let Some(dados) = outline.hierarchy.get(classe) else { return false };
    let privado = interner.resolve(chave).starts_with('_');
    dados.supertypes.keys().any(|&sup| {
        if sup == classe {
            return false;
        }
        let s = program.class(sup);
        (!privado || s.library == lib) && s.instance_members.contains_key(&chave)
    })
}

/// Os `override_on_non_overriding_*` da biblioteca `lib`.
pub fn sem_sobrescrita(program: &Program, interner: &Interner, outline: &OutlineTypes, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let Some(sym_override) = interner.lookup("override") else { return saida };
    let vazio: Vec<&str> = Vec::new();
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || !matches!(classe.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let cid = ClassId(i as u32);
        let a = &program.unit(decl.unit).ast;
        let membros: &[ast::MemberId] = match &a.decl(decl.decl).kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            _ => continue,
        };
        for &mid in membros {
            let membro = a.member(mid);
            if !anotado(&membro.metadata, sym_override) {
                continue;
            }
            match &membro.kind {
                MemberKind::Field(l) => {
                    for var in l.variables.iter() {
                        let nome = interner.resolve(var.name.sym);
                        let chave_do_setter = interner.lookup(&format!("{nome}_="));
                        let pelo_getter = sobrescreve(program, outline, interner, cid, lib, Some(var.name.sym));
                        // O campo `final` ou `const` não tem setter.
                        let pelo_setter = !l.final_ && !l.const_ && sobrescreve(program, outline, interner, cid, lib, chave_do_setter);
                        if !pelo_getter && !pelo_setter {
                            saida.push((decl.unit, Diagnostic::com_codigo(w::OVERRIDE_ON_NON_OVERRIDING_FIELD, var.name.span, vazio.iter().copied())));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(nome) = func.name else { continue };
                    let (chave, codigo) = match func.kind {
                        FunctionKind::Getter => (Some(nome.sym), w::OVERRIDE_ON_NON_OVERRIDING_GETTER),
                        FunctionKind::Setter => {
                            (interner.lookup(&format!("{}_=", interner.resolve(nome.sym))), w::OVERRIDE_ON_NON_OVERRIDING_SETTER)
                        }
                        FunctionKind::Function | FunctionKind::Operator => (Some(nome.sym), w::OVERRIDE_ON_NON_OVERRIDING_METHOD),
                    };
                    if !sobrescreve(program, outline, interner, cid, lib, chave) {
                        saida.push((decl.unit, Diagnostic::com_codigo(codigo, nome.span, vazio.iter().copied())));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }
    saida
}

/// A lista de anotações tem `@redeclare` (ou `@prefixo.redeclare`).
fn com_redeclare(metadata: &[ast::Annotation], sym: SymbolId) -> bool {
    metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| n.sym == sym))
}

/// Alguma superinterface do extension type `classe` (as de `implements`,
/// transitivamente: as de outro extension type, ou a classe com todos os
/// supertipos dela) tem um membro de instância com a chave `chave`. É o
/// `inheritance.getInterface(extensionType).redeclared` do analyzer.
fn redeclara(program: &Program, outline: &OutlineTypes, interner: &Interner, classe: ClassId, lib: LibraryId, chave: Option<SymbolId>) -> bool {
    let Some(chave) = chave else { return false };
    let privado = interner.resolve(chave).starts_with('_');
    let tem = |c: ClassId| {
        let e = program.class(c);
        (!privado || e.library == lib) && e.instance_members.contains_key(&chave)
    };
    let mut pilha: Vec<ClassId> = program.class(classe).interface_classes.clone();
    let mut vistos: Vec<ClassId> = vec![classe];
    while let Some(c) = pilha.pop() {
        if vistos.contains(&c) {
            continue;
        }
        vistos.push(c);
        if tem(c) {
            return true;
        }
        if program.class(c).kind == ClassKind::ExtensionType {
            pilha.extend(program.class(c).interface_classes.iter().copied());
        } else if let Some(dados) = outline.hierarchy.get(c)
            && dados.supertypes.keys().any(|s| tem(*s))
        {
            return true;
        }
    }
    false
}

/// O `RedeclareVerifier` (`analyzer/lib/src/error/redeclare_verifier.dart`):
/// `redeclare_on_non_redeclaring_member` no nome de um membro não estático
/// de extension type anotado com `@redeclare` que não redeclara nada das
/// superinterfaces. A anotação é reconhecida só pelo nome.
pub fn sem_redeclaracao(program: &Program, interner: &Interner, outline: &OutlineTypes, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let Some(sym) = interner.lookup("redeclare") else { return saida };
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || classe.kind != ClassKind::ExtensionType {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let cid = ClassId(i as u32);
        let a = &program.unit(decl.unit).ast;
        let DeclKind::ExtensionType(x) = &a.decl(decl.decl).kind else { continue };
        for &mid in x.members.iter() {
            let membro = a.member(mid);
            let MemberKind::Method(f) = &membro.kind else { continue };
            let func = a.function(*f);
            if func.static_ || !com_redeclare(&membro.metadata, sym) {
                continue;
            }
            let Some(nome) = func.name else { continue };
            let (chave, especie) = match func.kind {
                FunctionKind::Getter => (Some(nome.sym), "getter"),
                FunctionKind::Setter => (interner.lookup(&format!("{}_=", interner.resolve(nome.sym))), "setter"),
                FunctionKind::Function | FunctionKind::Operator => (Some(nome.sym), "method"),
            };
            if !redeclara(program, outline, interner, cid, lib, chave) {
                saida.push((decl.unit, Diagnostic::com_codigo(w::REDECLARE_ON_NON_REDECLARING_MEMBER, nome.span, [especie])));
            }
        }
    }
    saida
}

/// O lint `annotate_overrides` (`linter/lib/src/rules/annotate_overrides.dart`,
/// lido no `main` do SDK; a conferir contra a 3.6.2): o campo ou o método de
/// instância de classe, enum ou mixin que sobrescreve um membro herdado
/// (`overriddenMember`, a mesma regra de [`sobrescreve`]) e não tem
/// `@override`. Devolve a unidade, a posição do nome e o nome do membro
/// sobrescrito; quem chama decide se a regra está ligada. Os parâmetros de
/// construtor primário não são olhados.
pub fn sem_anotacao_de_override(
    program: &Program,
    interner: &Interner,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, dartforge_diagnostics::Span, String)> {
    let mut saida = Vec::new();
    // Sem o nome internado, nenhuma declaração tem `@override`: todas contam.
    let sym_override = interner.lookup("override");
    let com_override = |metadata: &[ast::Annotation]| sym_override.is_some_and(|s| anotado(metadata, s));
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || !matches!(classe.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let cid = ClassId(i as u32);
        let a = &program.unit(decl.unit).ast;
        let membros: &[ast::MemberId] = match &a.decl(decl.decl).kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            _ => continue,
        };
        for &mid in membros {
            let membro = a.member(mid);
            if membro.augment || com_override(&membro.metadata) {
                continue;
            }
            match &membro.kind {
                MemberKind::Field(l) if !l.static_ => {
                    for var in l.variables.iter() {
                        let nome = interner.resolve(var.name.sym);
                        let chave_do_setter = interner.lookup(&format!("{nome}_="));
                        let pelo_getter = sobrescreve(program, outline, interner, cid, lib, Some(var.name.sym));
                        let pelo_setter = !l.final_ && !l.const_ && sobrescreve(program, outline, interner, cid, lib, chave_do_setter);
                        if pelo_getter || pelo_setter {
                            saida.push((decl.unit, var.name.span, nome.to_string()));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(nome) = func.name else { continue };
                    if func.static_ {
                        continue;
                    }
                    let texto = interner.resolve(nome.sym);
                    let chave = match func.kind {
                        FunctionKind::Setter => interner.lookup(&format!("{texto}_=")),
                        _ => Some(nome.sym),
                    };
                    if sobrescreve(program, outline, interner, cid, lib, chave) {
                        saida.push((decl.unit, nome.span, texto.to_string()));
                    }
                }
                _ => {}
            }
        }
    }
    saida
}
