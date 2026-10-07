//! O `OverrideVerifier` e o `RedeclareVerifier` do analyzer 3.6.2
//! (`analyzer/lib/src/error/override_verifier.dart`,
//! `redeclare_verifier.dart`; docs/ANALYZER-ESPECIFICACAO-INFRA.md, Parte
//! III, etapa 6), o `invalid_override_of_non_virtual_member` do
//! `BestPracticesVerifier` e o lint `annotate_overrides`, todos sobre o
//! `InheritanceManager3` de [`crate::heranca`]:
//!
//! * `override_on_non_overriding_{method,getter,setter,field}`: o membro com
//!   `@override` (o getter `override` do `dart.core`) cujo `getOverridden2`
//!   na classe, enum ou mixin é nulo; em extensões e tipos de extensão o
//!   verificador não tem classe corrente, e todo `@override` é relatado;
//! * `redeclare_on_non_redeclaring_member`: o membro de instância de tipo de
//!   extensão com `@redeclare` (do `package:meta`) cujo nome não está no
//!   `redeclared` da interface;
//! * `invalid_override_of_non_virtual_member`: o membro cujo
//!   `getMember2(…, forSuper: true)` tem `@nonVirtual` do `package:meta`;
//! * `annotate_overrides`: o campo ou método de instância sem `@override`
//!   com `getInherited` (o mapa herdado) não nulo.
//!
//! Os nomes são os `Name` da biblioteca verificada.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::heranca::{Especie, Heranca, Membro, Nome, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, FunctionKind as Fk, FunctionRef, LibraryId, Program, UnitId, UnitRole, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, FunctionKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// `getOverridden2(classe, Name(lib, chave)) != null`.
pub(crate) fn sobrescreve(h: &mut Heranca, p: &mut ProvedorDoOutline<'_, '_>, classe: ClassId, lib: LibraryId, chave: Option<SymbolId>) -> bool {
    let Some(chave) = chave else { return false };
    let nome = Nome::novo(p.interner, lib, chave);
    h.sobrescritos(p, classe, nome).is_some()
}

/// `hasOverride`: o getter `override` do `dart.core`.
fn com_override(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::anotacoes::e_getter_de(program, interner, u, m, "dart.core", "override"))
}

/// A chave de setter de `nome`.
fn chave_do_setter(interner: &Interner, nome: SymbolId) -> Option<SymbolId> {
    interner.lookup(&format!("{}_=", interner.resolve(nome)))
}

/// O campo tem setter: não `const`, e não `final` (salvo `late final` sem
/// inicializador).
fn tem_setter(l: &ast::VariableList, v: &ast::Variable) -> bool {
    !l.const_ && (!l.final_ || (l.late && v.initializer.is_none()))
}

/// As declarações das unidades da biblioteca, com a classe de cada uma.
fn declaracoes(program: &Program, lib: LibraryId) -> Vec<(UnitId, ast::DeclId, Option<ClassId>)> {
    let mut classes: HashMap<(UnitId, ast::DeclId), ClassId> = HashMap::new();
    for (i, c) in program.classes.iter().enumerate() {
        if c.library == lib
            && let Some(d) = c.decl
        {
            classes.insert((d.unit, d.decl), ClassId(i as u32));
        }
    }
    let mut v = Vec::new();
    for &u in &program.library(lib).units {
        if program.unit(u).role == UnitRole::Patch {
            continue;
        }
        for &d in program.unit(u).unit.declarations.iter() {
            v.push((u, d, classes.get(&(u, d)).copied()));
        }
    }
    v
}

/// Os membros de uma declaração de tipo.
fn membros(d: &ast::Decl) -> &[ast::MemberId] {
    match &d.kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        DeclKind::Extension(x) => &x.members,
        DeclKind::ExtensionType(x) => &x.members,
        _ => &[],
    }
}

/// Os `override_on_non_overriding_*` da biblioteca `lib`.
pub fn sem_sobrescrita(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    let vazio: [&str; 0] = [];
    for (u, did, classe) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        // `_currentClass`: só classe, enum e mixin.
        let corrente = match &d.kind {
            DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_) => classe,
            _ => None,
        };
        let primario = match &d.kind {
            DeclKind::Class(x) => x.primary_constructor,
            DeclKind::Enum(x) => x.primary_constructor,
            _ => None,
        };
        for &mid in membros(d) {
            let membro = a.member(mid);
            // O parâmetro declarante do construtor primário (3.13) é o campo:
            // a anotação dele vale para o campo, que a elaboração cria sem
            // metadata. No nome do parâmetro.
            if primario == Some(mid)
                && let MemberKind::Constructor(k) = &membro.kind
            {
                for par in k.parameters.iter().filter(|x| x.declarante) {
                    let Some(nome) = par.name else { continue };
                    if !com_override(program, interner, u, &par.metadata) {
                        continue;
                    }
                    let pelo_getter = corrente.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, Some(nome.sym)));
                    let pelo_setter = !pelo_getter
                        && !par.final_
                        && corrente.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, chave_do_setter(interner, nome.sym)));
                    if !pelo_getter && !pelo_setter {
                        saida.push((u, Diagnostic::com_codigo(w::OVERRIDE_ON_NON_OVERRIDING_FIELD, nome.span, vazio)));
                    }
                }
            }
            if !com_override(program, interner, u, &membro.metadata) {
                continue;
            }
            match &membro.kind {
                MemberKind::Field(l) => {
                    for var in l.variables.iter() {
                        let pelo_getter = corrente.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, Some(var.name.sym)));
                        let pelo_setter = !pelo_getter
                            && tem_setter(l, var)
                            && corrente.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, chave_do_setter(interner, var.name.sym)));
                        if !pelo_getter && !pelo_setter {
                            saida.push((u, Diagnostic::com_codigo(w::OVERRIDE_ON_NON_OVERRIDING_FIELD, var.name.span, vazio)));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(nome) = func.name else { continue };
                    let (chave, codigo) = match func.kind {
                        FunctionKind::Getter => (Some(nome.sym), w::OVERRIDE_ON_NON_OVERRIDING_GETTER),
                        FunctionKind::Setter => (chave_do_setter(interner, nome.sym), w::OVERRIDE_ON_NON_OVERRIDING_SETTER),
                        FunctionKind::Function | FunctionKind::Operator => (Some(nome.sym), w::OVERRIDE_ON_NON_OVERRIDING_METHOD),
                    };
                    if !corrente.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, chave)) {
                        saida.push((u, Diagnostic::com_codigo(codigo, nome.span, vazio)));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }
    saida
}

/// O `RedeclareVerifier`: `redeclare_on_non_redeclaring_member` no nome de
/// um membro não estático de tipo de extensão com `@redeclare` cujo nome não
/// está no `redeclared` da interface.
pub fn sem_redeclaracao(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, classe) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let (DeclKind::ExtensionType(x), Some(c)) = (&d.kind, classe) else { continue };
        for &mid in x.members.iter() {
            let membro = a.member(mid);
            let MemberKind::Method(f) = &membro.kind else { continue };
            let func = a.function(*f);
            if func.static_ {
                continue;
            }
            if !membro.metadata.iter().any(|m| crate::anotacoes::e_getter_de(program, interner, u, m, "meta", "redeclare")) {
                continue;
            }
            let Some(nome) = func.name else { continue };
            let (chave, especie) = match func.kind {
                FunctionKind::Getter => (Some(nome.sym), "getter"),
                FunctionKind::Setter => (chave_do_setter(interner, nome.sym), "setter"),
                FunctionKind::Function | FunctionKind::Operator => (Some(nome.sym), "method"),
            };
            let redeclara = chave.is_some_and(|k| {
                let n = Nome::novo(interner, lib, k);
                h.interface(&mut p, c).redeclared.contains_key(&n)
            });
            if !redeclara {
                saida.push((u, Diagnostic::com_codigo(w::REDECLARE_ON_NON_REDECLARING_MEMBER, nome.span, [especie])));
            }
        }
    }
    saida
}

/// O nome de um membro herdado (`ExecutableElement.name`: o setter com `=`).
fn nome_do_membro(program: &Program, interner: &Interner, m: &Membro) -> String {
    let base = interner.resolve(program.function(m.funcao).name);
    let base = base.strip_suffix("_=").unwrap_or(base);
    if m.especie == Especie::Setter { format!("{base}=") } else { base.to_string() }
}

/// O lint `annotate_overrides` (`linter/lib/src/rules/annotate_overrides.dart`):
/// o campo ou método de instância de classe, enum ou mixin sem `@override`
/// cujo `overriddenMember` (o `getInherited` pelo `Name` da biblioteca da
/// classe; o campo pelo nome do getter) não é nulo. Devolve a unidade, a
/// posição do nome e o nome do membro herdado.
pub fn sem_anotacao_de_override(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Span, String)> {
    let mut saida = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, classe) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let Some(c) = classe else { continue };
        if !matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_)) {
            continue;
        }
        let biblioteca = program.class(c).library;
        for &mid in membros(d) {
            let membro = a.member(mid);
            if membro.augment || com_override(program, interner, u, &membro.metadata) {
                continue;
            }
            match &membro.kind {
                MemberKind::Field(l) if !l.static_ => {
                    for var in l.variables.iter() {
                        let n = Nome::novo(interner, biblioteca, var.name.sym);
                        if let Some(m) = h.herdado(&mut p, c, n) {
                            saida.push((u, var.name.span, nome_do_membro(program, interner, &m)));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(nome) = func.name else { continue };
                    if func.static_ {
                        continue;
                    }
                    let chave = match func.kind {
                        FunctionKind::Setter => chave_do_setter(interner, nome.sym),
                        _ => Some(nome.sym),
                    };
                    let Some(chave) = chave else { continue };
                    let n = Nome::novo(interner, biblioteca, chave);
                    if let Some(m) = h.herdado(&mut p, c, n) {
                        saida.push((u, nome.span, nome_do_membro(program, interner, &m)));
                    }
                }
                _ => {}
            }
        }
    }
    saida
}

/// `_hasNonVirtualAnnotation`: o `@nonVirtual` do `package:meta` no membro
/// (no acessor sintético, no campo). O sintético da herança (covariância)
/// não tem anotações.
fn nao_virtual(program: &Program, interner: &Interner, m: &Membro) -> bool {
    if m.sintetico {
        return false;
    }
    let e = program.function(m.funcao);
    let (unidade, metadata): (UnitId, &[ast::Annotation]) = if e.kind == Fk::ImplicitAccessor {
        let Some(v) = e.variable else { return false };
        match program.variable(v).node {
            VariableRef::Field { unit, member, .. } => (unit, &program.unit(unit).ast.member(member).metadata[..]),
            _ => return false,
        }
    } else {
        let unidade = match e.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
            FunctionRef::None => return false,
        };
        (unidade, crate::fase_resultado::anotacoes_da_funcao(program, m.funcao))
    };
    metadata.iter().any(|a| crate::anotacoes::e_getter_de(program, interner, unidade, a, "meta", "nonVirtual"))
}

/// `invalid_override_of_non_virtual_member` (`BestPracticesVerifier`,
/// `visitFieldDeclaration` e `visitMethodDeclaration`): o membro de classe,
/// enum ou mixin cujo `getMember2(…, forSuper: true)` (o campo: pelo getter,
/// senão pelo setter) tem `@nonVirtual`.
pub fn sobrescritas_de_nao_virtuais(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, classe) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let Some(c) = classe else { continue };
        if !matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_) | DeclKind::ExtensionType(_)) {
            continue;
        }
        for &mid in membros(d) {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    for var in l.variables.iter() {
                        let n = Nome::novo(interner, lib, var.name.sym);
                        let mut achado = h.membro(&mut p, c, n, false, None, true);
                        if achado.is_none()
                            && let Some(s) = chave_do_setter(interner, var.name.sym)
                        {
                            achado = h.membro(&mut p, c, Nome::novo(interner, lib, s), false, None, true);
                        }
                        if let Some(m) = achado
                            && nao_virtual(program, interner, &m)
                        {
                            let texto = interner.resolve(var.name.sym);
                            let dona = interner.resolve(program.class(m.classe).name);
                            saida.push((u, Diagnostic::com_codigo(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, var.name.span, [texto, dona])));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(nome) = func.name else { continue };
                    let chave = match func.kind {
                        FunctionKind::Setter => chave_do_setter(interner, nome.sym),
                        _ => Some(nome.sym),
                    };
                    let Some(chave) = chave else { continue };
                    if let Some(m) = h.membro(&mut p, c, Nome::novo(interner, lib, chave), false, None, true)
                        && nao_virtual(program, interner, &m)
                    {
                        let texto = interner.resolve(nome.sym);
                        let dona = interner.resolve(program.class(m.classe).name);
                        saida.push((u, Diagnostic::com_codigo(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, nome.span, [texto, dona])));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }
    saida
}
