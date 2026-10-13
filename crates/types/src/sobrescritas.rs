//! Sobrescritas inválidas (`invalid_override`), como o
//! `_ClassVerifier._checkDeclaredMember` do `InheritanceOverrideVerifier`
//! (analyzer 3.6.2, `src/error/inheritance_override.dart`) e o
//! `CorrectOverrideHelper` (`src/error/correct_override.dart`): cada membro
//! de instância declarado na classe é comparado, por subtipagem do tipo de
//! função, com o membro homônimo da interface de cada superinterface direta
//! (superclasse, restrições `on`, mixins e interfaces, na ordem do
//! analyzer). Parâmetros covariantes valem `Object?`.
//!
//! As consultas à interface (`getMember`, `getInherited`, o `implemented`)
//! são as do `InheritanceManager3` de [`crate::heranca`], com a assinatura
//! combinada e o `topMerge`; a covariância de cada parâmetro é a escrita
//! mais a herdada; os tipos omitidos são os da inferência de sobrescrita
//! fiel (`inferencia::sobrescrita`), que roda antes destas fases.
//!
//! A classe entra só se o `verify()` do analyzer chegaria até aqui (quem
//! chama filtra: supertipo proibido, `Enum` em classe concreta e herança
//! recursiva encerram antes).

use crate::despejo::formatar;
use crate::ops::substitute;
use crate::resolve::OutlineTypes;
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{Diagnostic, Span, codigos::compile_time_error as c};
use dartforge_elements::model::{
    ClassId, FunctionElementId, FunctionKind, FunctionRef, Program, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// A espécie de um membro para `member.kind != superMember.kind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Especie {
    Metodo,
    Getter,
    Setter,
}

struct Ctx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    /// O `InheritanceManager3`.
    heranca: crate::heranca::Heranca,
    /// A biblioteca da classe verificada (o `Name` dos privados).
    biblioteca: dartforge_elements::model::LibraryId,
}

/// Um membro homônimo achado na interface de um supertipo.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Achado {
    dono: ClassId,
    funcao: FunctionElementId,
}

impl Ctx<'_> {
    fn especie(&self, f: FunctionElementId) -> Option<Especie> {
        let func = self.program.function(f);
        match func.kind {
            FunctionKind::Function | FunctionKind::Operator => Some(Especie::Metodo),
            FunctionKind::Getter => Some(Especie::Getter),
            FunctionKind::Setter => Some(Especie::Setter),
            FunctionKind::ImplicitAccessor => {
                let v = self.program.variable(func.variable?);
                if v.getter == Some(f) { Some(Especie::Getter) } else { Some(Especie::Setter) }
            }
            _ => None,
        }
    }

    /// A chave do membro em `instance_members` (`x_=` para setter).
    fn chave(&self, f: FunctionElementId) -> Option<SymbolId> {
        let func = self.program.function(f);
        if self.especie(f)? == Especie::Setter {
            self.interner.lookup(&format!("{}_=", self.interner.resolve(func.name)))
        } else {
            Some(func.name)
        }
    }


    /// O membro `chave` na interface da classe `d` (`getMember2`) ou herdado
    /// por ela (`getInherited2`), pelo `Name` da biblioteca verificada.
    fn inheritance_member(&mut self, d: ClassId, key: SymbolId, inherited: bool) -> Option<crate::heranca::Membro> {
        let the_name = crate::heranca::Nome::novo(self.interner, self.biblioteca, key);
        let mut h = std::mem::take(&mut self.heranca);
        let r = {
            let mut p = crate::heranca::ProvedorDoOutline {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            if inherited { h.herdado(&mut p, d, the_name) } else { h.membro(&mut p, d, the_name, false, None, false) }
        };
        self.heranca = h;
        r
    }

    /// `InheritanceManager3.getMember(s, nome)`: o membro na interface do
    /// tipo de interface `s` (a assinatura combinada, com o `topMerge`), com
    /// o tipo visto por `s`.
    fn in_interface(&mut self, s: TypeId, key: SymbolId, _prof: u32) -> Option<(Achado, TypeId)> {
        self.in_interface_ex(s, key, false)
    }

    /// `InheritanceManager3.getInherited(s, nome)`.
    fn herdado(&mut self, s: TypeId, key: SymbolId) -> Option<(Achado, TypeId)> {
        self.in_interface_ex(s, key, true)
    }

    fn in_interface_ex(&mut self, s: TypeId, key: SymbolId, inherited: bool) -> Option<(Achado, TypeId)> {
        let (d, args) = match self.table.get(s).clone() {
            Type::Interface { class, args, .. } => (class, args),
            _ => return None,
        };
        let m = self.inheritance_member(d, key, inherited)?;
        let formals: Box<[TypeParamId]> = self.outline.classes.get(d.0 as usize)?.type_params.clone();
        let t = if !formals.is_empty() && formals.len() == args.len() {
            let subst: HashMap<TypeParamId, TypeId> = formals.iter().copied().zip(args.iter().copied()).collect();
            substitute(m.tipo, &subst, self.table)
        } else {
            m.tipo
        };
        Some((Achado { dono: m.classe, funcao: m.funcao }, t))
    }

    /// O tipo do membro `a` visto pelo supertipo `s` (que o herda ou declara).
    fn seen_type(&mut self, s: TypeId, a: Achado) -> Option<TypeId> {
        let sig = self.outline.functions.get(a.funcao.0 as usize)?.signature;
        let seen = self.outline.hierarchy.supertype_of(s, a.dono, self.table, self.core)?;
        let args = match self.table.get(seen) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return None,
        };
        let params: Box<[TypeParamId]> = self.outline.classes.get(a.dono.0 as usize)?.type_params.clone();
        if params.len() != args.len() {
            return None;
        }
        let subst: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        Some(substitute(sig, &subst, self.table))
    }

    /// `ParameterElement.isCovariant` do membro `f` visto pela classe `c`: o
    /// declarado nela (escrito mais o herdado dos sobrescritos) ou o
    /// implementado (com a covariância herdada).
    fn effective_covariant(&mut self, c: ClassId, f: FunctionElementId, key: SymbolId) -> (Vec<usize>, Vec<SymbolId>) {
        let the_name = crate::heranca::Nome::novo(self.interner, self.program.class(c).library, key);
        let mut h = std::mem::take(&mut self.heranca);
        let i = {
            let mut p = crate::heranca::ProvedorDoOutline {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            h.interface(&mut p, c)
        };
        self.heranca = h;
        let m = i
            .declared
            .get(&the_name)
            .filter(|m| m.funcao == f)
            .or_else(|| i.implemented.get(&the_name).filter(|m| m.funcao == f))
            .or_else(|| i.declared.get(&the_name))
            .or_else(|| i.implemented.get(&the_name));
        let mut pos = Vec::new();
        let mut names = Vec::new();
        for d in m.map(|m| &m.the_covariant[..]).unwrap_or(&[]) {
            match d {
                crate::heranca::Param::Indice(k) => pos.push(*k),
                crate::heranca::Param::Nome(n) => names.push(*n),
            }
        }
        (pos, names)
    }

    /// `CovariantParametersVerifier`: cada parâmetro covariante contra o
    /// correspondente do membro homônimo declarado em cada supertipo da
    /// classe; tipos sem relação de subtipo em nenhum sentido são erro. A
    /// mensagem mostra o membro do supertipo sem substituição.
    fn covariant_against_supertypes(&mut self, m: &Conferencia, key: SymbolId, output: &mut Vec<Diagnostic>) {
        let (pos, names) = self.effective_covariant(m.declarante, m.funcao, key);
        if pos.is_empty() && names.is_empty() {
            return;
        }
        let Some(own) = self.outline.functions.get(m.funcao.0 as usize) else { return };
        let the_own: Vec<(usize, ParameterKind, Option<SymbolId>, TypeId)> =
            own.parameters.iter().enumerate().map(|(i, p)| (i, p.kind, p.externo, p.ty)).collect();
        let Some(data) = self.outline.hierarchy.get(m.declarante) else { return };
        let mut supers: Vec<(ClassId, TypeId)> =
            data.supertypes.iter().filter(|(k, _)| **k != m.declarante).map(|(k, v)| (*k, *v)).collect();
        supers.sort_by_key(|(k, _)| k.0);
        let the_kind = self.especie(m.funcao);
        for (s, st) in supers {
            let Some(&g) = self.program.class(s).instance_members.get(&key) else { continue };
            if self.especie(g) != the_kind {
                continue;
            }
            let Some(data_g) = self.outline.functions.get(g.0 as usize) else { continue };
            let args = match self.table.get(st) {
                Type::Interface { args, .. } => args.clone(),
                _ => continue,
            };
            let Some(params_s) = self.outline.classes.get(s.0 as usize).map(|d| d.type_params.clone()) else { continue };
            if params_s.len() != args.len() {
                continue;
            }
            let mut subst: HashMap<TypeParamId, TypeId> = params_s.iter().copied().zip(args.iter().copied()).collect();
            // `_superSubstitution` (3.6.2 `correct_override.dart:195-215`): com
            // a mesma contagem, os parâmetros de tipo do método do supertipo
            // viram os do próprio método.
            if !own.type_params.is_empty() && own.type_params.len() == data_g.type_params.len() {
                for (&sp, &tp) in data_g.type_params.iter().zip(own.type_params.iter()) {
                    let t = self.table.intern(Type::TypeParameter { param: tp, nullable: false });
                    subst.insert(sp, t);
                }
            }
            let super_params: Vec<(ParameterKind, Option<SymbolId>, TypeId)> =
                data_g.parameters.iter().map(|p| (p.kind, p.externo, p.ty)).collect();
            let super_sig = data_g.signature;
            let mut positional_one = 0usize;
            for &(i, kind, the_name, ty) in &the_own {
                let covariant = if kind == ParameterKind::Named {
                    the_name.is_some_and(|n| names.contains(&n))
                } else {
                    pos.contains(&positional_one)
                };
                let pos_index = positional_one;
                if kind != ParameterKind::Named {
                    positional_one += 1;
                }
                let _ = i;
                if !covariant {
                    continue;
                }
                let corresponding = if kind == ParameterKind::Named {
                    super_params.iter().find(|p| p.0 == ParameterKind::Named && p.1 == the_name).map(|p| p.2)
                } else {
                    super_params.iter().filter(|p| p.0 != ParameterKind::Named).nth(pos_index).map(|p| p.2)
                };
                let Some(sup_ty) = corresponding else { continue };
                let sup_ty = substitute(sup_ty, &subst, self.table);
                let related = {
                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                    is_subtype(sup_ty, ty, &mut env) || is_subtype(ty, sup_ty, &mut env)
                };
                if !related {
                    let mut name_m = self.interner.resolve(self.program.function(m.funcao).name).to_string();
                    if the_kind == Some(Especie::Setter) {
                        name_m.push('=');
                    }
                    let args = [
                        name_m,
                        self.interner.resolve(self.program.class(m.declarante).name).to_string(),
                        formatar(self.table, m.tipo, self.interner, self.program),
                        self.interner.resolve(self.program.class(s).name).to_string(),
                        formatar(self.table, super_sig, self.interner, self.program),
                    ];
                    output.push(Diagnostic::com_codigo(c::INVALID_OVERRIDE, m.span, args));
                }
            }
        }
    }

    /// `_computeThisTypeForSubtype`: parâmetros covariantes viram `Object?`.
    fn for_subtype(&mut self, sig: TypeId, pos: &[usize], names: &[SymbolId]) -> TypeId {
        if pos.is_empty() && names.is_empty() {
            return sig;
        }
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.table.get(sig).clone() else {
            return sig;
        };
        let obj = self.core.object_nullable;
        let n_pos = positional.len();
        let positional: Box<[TypeId]> =
            positional.iter().enumerate().map(|(i, &t)| if pos.contains(&i) { obj } else { t }).collect();
        let optional: Box<[TypeId]> =
            optional.iter().enumerate().map(|(i, &t)| if pos.contains(&(i + n_pos)) { obj } else { t }).collect();
        let named: Box<[(SymbolId, TypeId, bool)]> =
            named.iter().map(|&(n, t, r)| (n, if names.contains(&n) { obj } else { t }, r)).collect();
        self.table.intern(Type::Function { type_params, ret, positional, optional, named, nullable })
    }
}

/// Um membro a conferir: a função, o tipo dela na classe que o recebe, a
/// classe que o declara (nome da mensagem, covariância, inferência) e onde
/// relatar.
#[derive(Clone, Copy)]
struct Conferencia {
    funcao: FunctionElementId,
    tipo: TypeId,
    declarante: ClassId,
    span: Span,
}

impl Ctx<'_> {
    /// `_checkDeclaredMember` contra as superinterfaces `supers`.
    fn conferir(&mut self, m: Conferencia, supers: &[TypeId], output: &mut Vec<Diagnostic>) {
        let (Some(the_kind), Some(key)) = (self.especie(m.funcao), self.chave(m.funcao)) else { return };
        let the_name = self.interner.resolve(self.program.function(m.funcao).name).to_string();
        let library = self.program.class(m.declarante).library;
        let (pos, names) = self.effective_covariant(m.declarante, m.funcao, key);
        let to_sub = self.for_subtype(m.tipo, &pos, &names);
        for &s in supers {
            let Some((finding, do_super)) = self.in_interface(s, key, 0) else { continue };
            if the_name.starts_with('_') && self.program.class(finding.dono).library != library {
                continue;
            }
            if self.especie(finding.funcao) != Some(the_kind) {
                continue;
            }
            let ok = {
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                is_subtype(to_sub, do_super, &mut env)
            };
            if !ok {
                let code = if the_kind == Especie::Setter { c::INVALID_OVERRIDE_SETTER } else { c::INVALID_OVERRIDE };
                let args = [
                    the_name.clone(),
                    self.interner.resolve(self.program.class(m.declarante).name).to_string(),
                    formatar(self.table, m.tipo, self.interner, self.program),
                    self.interner.resolve(self.program.class(finding.dono).name).to_string(),
                    formatar(self.table, do_super, self.interner, self.program),
                ];
                // `invalidOverride`: o membro sobrescrito, no nome não
                // sintético dele (no arquivo dele, se é outro).
                let mut d = Diagnostic::com_codigo(code, m.span, args);
                if let Some((u, span)) = self.program.nome_nao_sintetico_da_funcao(finding.funcao) {
                    let error_unit = self.program.nome_nao_sintetico_da_funcao(m.funcao).map(|(x, _)| x);
                    let file = (Some(u) != error_unit).then(|| self.program.caminho_da_unidade(u).into());
                    let text = if the_kind == Especie::Setter { "The setter being overridden." } else { "The member being overridden." };
                    d.contexto.push(dartforge_diagnostics::Contexto { arquivo: file, span, mensagem: text.into() });
                }
                output.push(d);
            }
        }
    }
}

/// Os diagnósticos de `invalid_override` dos membros declarados nas classes
/// `classes` e dos mixins que elas aplicam (cada um com a unidade da
/// declaração da classe).
pub fn sobrescritas_invalidas(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    // `_reportNoCombinedSuperSignature` relatado: o método pula o
    // `_checkDeclaredMember` (`inheritance_override.dart:259-263`).
    let without_signature: std::collections::HashSet<FunctionElementId> =
        crate::fase_heranca::methods_without_combined_signature(program, interner, table, core, outline, classes).into_iter().map(|(_, f, _, _)| f).collect();
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut output = Vec::new();
    // Acessores e métodos por nó da fonte.
    let mut by_function: HashMap<(UnitId, u32), FunctionElementId> = HashMap::new();
    type Acessores = (Option<FunctionElementId>, Option<FunctionElementId>);
    let mut by_variable: HashMap<(UnitId, u32, usize), Acessores> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.class.is_none() || f.static_ {
            continue;
        }
        let id = FunctionElementId(i as u32);
        match f.node {
            FunctionRef::Function { unit, function } => {
                by_function.insert((unit, function.0), id);
            }
            FunctionRef::None => {
                if let Some(v) = f.variable {
                    let var = program.variable(v);
                    if let VariableRef::Field { unit, member, index } = var.node {
                        by_variable.insert((unit, member.0, index), (var.getter, var.setter));
                    }
                }
            }
            FunctionRef::Constructor { .. } => {}
        }
    }
    // Os membros de instância declarados numa classe: `(função, nome no erro)`.
    let the_declared = |cid: ClassId| -> Vec<(FunctionElementId, Span)> {
        let mut v = Vec::new();
        let Some(decl) = program.class(cid).decl else { return v };
        let ast_ = &program.unit(decl.unit).ast;
        let members: &[ast::MemberId] = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            DeclKind::Enum(d) => &d.members,
            _ => return v,
        };
        for &m in members {
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if let (Some(&id), Some(n)) = (by_function.get(&(decl.unit, f.0)), af.name) {
                        v.push((id, n.span));
                    }
                }
                MemberKind::Field(vl) if !vl.static_ => {
                    for (i, var) in vl.variables.iter().enumerate() {
                        if let Some(&(g, s)) = by_variable.get(&(decl.unit, m.0, i)) {
                            v.extend(g.into_iter().chain(s).map(|f| (f, var.name.span)));
                        }
                    }
                }
                _ => {}
            }
        }
        v
    };
    for &cid in classes {
        let cls = program.class(cid);
        cx.biblioteca = cls.library;
        let Some(decl) = cls.decl else { continue };
        let Some(data) = outline.classes.get(cid.0 as usize) else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let with: &[ast::TypeId] = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.with,
            DeclKind::Enum(d) => &d.with,
            _ => &[],
        };
        // `directSuperInterfaces`: superclasse (`Object` quando omitida numa
        // classe) e restrições `on`; os mixins entram um a um depois de
        // conferidos; por fim as interfaces.
        let mut supers: Vec<TypeId> = Vec::new();
        match (data.supertype, cls.kind) {
            (Some(t), _) => supers.push(t),
            (None, dartforge_elements::model::ClassKind::Class) => supers.push(core.object),
            (None, dartforge_elements::model::ClassKind::Enum) => {
                if let (Some(e), Some(this_one)) = (cx.core_class("Enum"), cx.own_type(cid))
                    && let Some(t) = outline.hierarchy.supertype_of(this_one, e, cx.table, core)
                {
                    supers.push(t);
                }
            }
            _ => {}
        }
        supers.extend(data.on.iter().copied());
        let mut diags = Vec::new();
        for (i, &mt) in data.mixins.iter().enumerate() {
            if let Type::Interface { class: m, .. } = cx.table.get(mt).clone()
                && with.len() == data.mixins.len()
            {
                let span = ast_.ty(with[i]).span;
                for (f, _) in the_declared(m) {
                    // O tipo do membro do mixin aplicado (`M<args>`).
                    let Some(typ) = cx.seen_type(mt, Achado { dono: m, funcao: f }) else { continue };
                    cx.conferir(Conferencia { funcao: f, tipo: typ, declarante: m, span }, &supers.clone(), &mut diags);
                }
            }
            supers.push(mt);
        }
        supers.extend(data.interfaces.iter().copied());
        supers.retain(|&t| match cx.table.get(t) {
            Type::Interface { class, .. } => !cx.outside_hierarchy(cid, *class),
            _ => true,
        });
        for (f, span) in the_declared(cid) {
            if without_signature.contains(&f) {
                continue;
            }
            let Some(typ) = outline.functions.get(f.0 as usize).map(|d| d.signature) else { continue };
            let conf = Conferencia { funcao: f, tipo: typ, declarante: cid, span };
            cx.conferir(Conferencia { ..conf }, &supers, &mut diags);
            if let Some(key) = cx.chave(f) {
                cx.covariant_against_supertypes(&conf, key, &mut diags);
            }
        }
        output.extend(diags.into_iter().map(|d| (decl.unit, d)));
    }
    output
}

/// Onde está o nome da função `f` (o do campo ou variável, num acessor
/// implícito).
fn name_of(program: &Program, f: FunctionElementId) -> Option<(UnitId, Span)> {
    let func = program.function(f);
    match func.node {
        FunctionRef::Function { unit, function } => {
            Some((unit, program.unit(unit).ast.functions[function.0 as usize].name?.span))
        }
        FunctionRef::None => nome_da_variavel(program, func.variable?),
        FunctionRef::Constructor { .. } => None,
    }
}

/// O nome declarado de uma variável (campo, de topo, constante de enum,
/// representação).
fn nome_da_variavel(program: &Program, v: dartforge_elements::model::VariableId) -> Option<(UnitId, Span)> {
    {
        match program.variable(v).node {
            VariableRef::Field { unit, member, index } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::TopLevel { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::Variables(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::Enum(d) => Some((unit, d.constants.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::Representation { unit, decl } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::ExtensionType(d) => Some((unit, d.representation_name.span)),
                _ => None,
            },
            VariableRef::None => None,
        }
    }
}

impl Ctx<'_> {
    /// Os tipos de getter e setter de um acessor fora de interface (topo,
    /// estático, extensão): o escrito, `dynamic` quando omitido numa função,
    /// nada num campo ou variável sem tipo escrito (o tipo viria do
    /// inicializador).
    fn accessor_type(&mut self, f: FunctionElementId, getter: bool) -> Option<TypeId> {
        let func = self.program.function(f);
        let data = self.outline.functions.get(f.0 as usize)?;
        match func.node {
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                let (written, t) = if getter {
                    (af.return_type, data.return_type)
                } else {
                    (af.parameters.as_deref().and_then(|p| p.first()).and_then(|p| p.ty), data.parameters.first()?.ty)
                };
                // Tipo escrito que não resolve (`InvalidType` no analyzer, que
                // é subtipo de tudo e aceita tudo): não se decide.
                if let Some(e) = written
                    && t == self.core.dynamic_
                    && !matches!(&self.program.unit(unit).ast.ty(e).kind,
                        ast::TypeKind::Named { name, .. } if name.len() == 1 && self.interner.resolve(name[0].sym) == "dynamic")
                {
                    return None;
                }
                Some(t)
            }
            FunctionRef::None => {
                let v = func.variable?;
                let data_v = self.outline.variables.get(v.0 as usize)?;
                match self.program.variable(v).node {
                    // Constante de enum e representação: o tipo é conhecido.
                    VariableRef::EnumConstant { .. } | VariableRef::Representation { .. } => {
                        data_v.declared_type.or(data_v.inferred)
                    }
                    _ => data_v.declared_type,
                }
            }
            FunctionRef::Constructor { .. } => None,
        }
    }
}

/// `GetterSetterTypesVerifier` (`src/error/getter_setter_types_verifier.dart`):
/// o tipo de retorno do getter precisa ser subtipo do tipo do parâmetro do
/// setter homônimo. Os acessores de topo (por unidade), os estáticos de
/// classe e enum e os de extensão (`checkStaticAccessors`/`checkExtension`),
/// e a interface das classes `classes` (`checkInterface`, que o
/// `InheritanceOverrideVerifier` chama depois das sobrescritas).
pub fn getters_e_setters(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut output = Vec::new();
    // Acessores locais: `(recipiente, nome)` → getters e setters.
    #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
    enum Holder {
        Unidade(UnitId),
        Classe(ClassId),
        Instance(ClassId),
        Extensao(u32),
    }
    // Um acessor local: função (getter/setter escrito ou implícito de campo
    // de classe), variável sem função de acesso no modelo (constante de
    // enum, representação, campo de extensão) ou o `values` sintético do
    // enum.
    #[derive(PartialEq, Eq, Clone, Copy, Debug)]
    enum Acessor {
        Funcao(FunctionElementId),
        Variavel(dartforge_elements::model::VariableId),
        Values(ClassId),
    }
    let mut groups: HashMap<(Holder, SymbolId), (Vec<Acessor>, Vec<Acessor>)> = HashMap::new();
    // Acessores de `augment` (e o que eles aumentam) ficam de fora: a
    // recuperação do parser sem o experimento os lê de outro jeito.
    let mut augmented: std::collections::HashSet<(UnitId, u32)> = std::collections::HashSet::new();
    for &u in &program.library(lib).units {
        let ast_ = &program.unit(u).ast;
        for m in &ast_.members {
            if m.augment {
                match &m.kind {
                    MemberKind::Method(f) => {
                        augmented.insert((u, f.0));
                    }
                    // Campo com `augment`: a biblioteca inteira fica de fora.
                    MemberKind::Field(_) => return Vec::new(),
                    MemberKind::Constructor(_) => {}
                }
            }
        }
        for d in &ast_.decls {
            if d.augment
                && let DeclKind::Function(f) = &d.kind
            {
                augmented.insert((u, f.0));
            }
        }
    }
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        if let FunctionRef::Function { unit, function } = f.node
            && augmented.contains(&(unit, function.0))
        {
            continue;
        }
        let id = FunctionElementId(i as u32);
        // Os sintéticos do enum: `values` é o getter estático (no nome do
        // enum); `index` e `name` vêm de `Enum` pela interface.
        if matches!(f.node, FunctionRef::None) && f.variable.is_none() {
            if f.static_
                && let Some(c) = f.class
                && program.class(c).kind == dartforge_elements::model::ClassKind::Enum
                && interner.resolve(f.name) == "values"
            {
                groups.entry((Holder::Classe(c), f.name)).or_default().0.push(Acessor::Values(c));
            }
            continue;
        }
        let Some(the_kind) = cx.especie(id) else { continue };
        if the_kind == Especie::Metodo {
            continue;
        }
        let holder = match (f.class, f.extension) {
            (_, Some(e)) => Holder::Extensao(e.0),
            (Some(c), None) if f.static_ => match program.class(c).kind {
                dartforge_elements::model::ClassKind::Class
                | dartforge_elements::model::ClassKind::Enum
                | dartforge_elements::model::ClassKind::ExtensionType => Holder::Classe(c),
                _ => continue,
            },
            // Membros de instância de tipo de extensão: os declarados nele
            // (`checkExtensionType`, a parte da interface que ele mesmo
            // declara), com a representação.
            (Some(c), None) if program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType => {
                Holder::Instance(c)
            }
            (Some(_), None) => continue,
            (None, None) => match name_of(program, id) {
                Some((u, _)) => Holder::Unidade(u),
                None => continue,
            },
        };
        let g = groups.entry((holder, f.name)).or_default();
        if the_kind == Especie::Getter { g.0.push(Acessor::Funcao(id)) } else { g.1.push(Acessor::Funcao(id)) }
    }
    // As variáveis sem função de acesso: a constante de enum (getter
    // estático), a representação (getter de instância do tipo de extensão)
    // e o campo de extensão (getter, e setter se não é `final` nem `const`).
    for (i, v) in program.variables.iter().enumerate() {
        if v.library != lib || v.getter.is_some() {
            continue;
        }
        let vid = dartforge_elements::model::VariableId(i as u32);
        match (v.node, v.class, v.extension) {
            (VariableRef::EnumConstant { .. }, Some(c), _) => {
                groups.entry((Holder::Classe(c), v.name)).or_default().0.push(Acessor::Variavel(vid));
            }
            (VariableRef::Representation { .. }, Some(c), _) => {
                groups.entry((Holder::Instance(c), v.name)).or_default().0.push(Acessor::Variavel(vid));
            }
            (VariableRef::Field { .. }, None, Some(e)) => {
                let g = groups.entry((Holder::Extensao(e.0), v.name)).or_default();
                g.0.push(Acessor::Variavel(vid));
                if !v.final_ && !v.const_ {
                    g.1.push(Acessor::Variavel(vid));
                }
            }
            _ => {}
        }
    }
    let this_type = |table: &mut TypeTable, c: ClassId| -> TypeId {
        let params = outline.classes.get(c.0 as usize).map(|d| d.type_params.clone()).unwrap_or_default();
        let args: Box<[TypeId]> = params.iter().map(|&p| table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        table.intern(Type::Interface { class: c, args, nullable: false })
    };
    let mut keys: Vec<_> = groups.keys().copied().collect();
    keys.sort_by_key(|(r, n)| (interner.resolve(*n).to_string(), format!("{r:?}")));
    for k in keys {
        let (gs, ss) = &groups[&k];
        let ([g], [s]) = (gs.as_slice(), ss.as_slice()) else { continue };
        let typ = |cx: &mut Ctx<'_>, a: Acessor, getter: bool| -> Option<TypeId> {
            match a {
                Acessor::Funcao(f) => cx.accessor_type(f, getter),
                Acessor::Variavel(v) => outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)),
                Acessor::Values(c) => {
                    let list = core.list_class?;
                    let e = this_type(cx.table, c);
                    Some(cx.table.intern(Type::Interface { class: list, args: Box::new([e]), nullable: false }))
                }
            }
        };
        let (Some(tg), Some(ts)) = (typ(&mut cx, *g, true), typ(&mut cx, *s, false)) else { continue };
        let ok = {
            let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
            is_subtype(tg, ts, &mut env)
        };
        // Num tipo de extensão, o getter da representação cede o lugar ao setter.
        let representation = matches!(k.0, Holder::Instance(_))
            && matches!(*g, Acessor::Variavel(v) if matches!(program.variable(v).node, VariableRef::Representation { .. }));
        let place_of = |a: Acessor| -> Option<(UnitId, Span)> {
            match a {
                Acessor::Funcao(f) => name_of(program, f),
                Acessor::Variavel(v) => nome_da_variavel(program, v),
                // O `values` sintético está no nome do enum.
                Acessor::Values(c) => {
                    let d = program.class(c).decl?;
                    match &program.unit(d.unit).ast.decl(d.decl).kind {
                        DeclKind::Enum(e) => Some((d.unit, e.name.span)),
                        _ => None,
                    }
                }
            }
        };
        let place = if representation { place_of(*s) } else { place_of(*g) };
        if !ok && let Some((u, span)) = place {
            let the_name = interner.resolve(k.1).to_string();
            let args = [the_name.clone(), formatar(cx.table, tg, interner, program), formatar(cx.table, ts, interner, program), the_name];
            output.push((u, Diagnostic::com_codigo(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args)));
        }
    }
    // O `Enum` do `dart:core`: os getters sintéticos `index` e `name` de um
    // enum são, na interface, os dele (`'Enum.index'`).
    let enum_do_core = program
        .classes
        .iter()
        .position(|k| interner.resolve(k.name) == "Enum" && program.library(k.library).uri == "dart:core")
        .map(|i| ClassId(i as u32));
    let enum_synthetic = |f: FunctionElementId| {
        let func = program.function(f);
        matches!(func.node, FunctionRef::None)
            && func.variable.is_none()
            && !func.static_
            && func.class.is_some_and(|c| program.class(c).kind == dartforge_elements::model::ClassKind::Enum)
    };
    // A interface de cada classe (`checkInterface` do
    // `InheritanceOverrideVerifier`) e de cada tipo de extensão
    // (`checkExtensionType`, do `ErrorVerifier`); no tipo de extensão, o par
    // declarado nele mesmo já saiu na conferência local acima.
    let extensions: Vec<ClassId> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, k)| k.library == lib && k.kind == dartforge_elements::model::ClassKind::ExtensionType)
        .map(|(i, _)| ClassId(i as u32))
        .filter(|c| !classes.contains(c))
        .collect();
    for &cid in classes.iter().chain(extensions.iter()) {
        let cls = program.class(cid);
        let of_extension = cls.kind == dartforge_elements::model::ClassKind::ExtensionType;
        cx.biblioteca = cls.library;
        let Some(decl) = cls.decl else { continue };
        let Some(data_h) = outline.hierarchy.get(cid) else { continue };
        let Some(params) = outline.classes.get(cid.0 as usize).map(|d| d.type_params.clone()) else { continue };
        let args: Box<[TypeId]> = params.iter().map(|&p| cx.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let this_one = cx.table.intern(Type::Interface { class: cid, args, nullable: false });
        let mut names: Vec<SymbolId> = Vec::new();
        for s in std::iter::once(cid).chain(data_h.supertypes.keys().copied()) {
            for (&key, &f) in &program.class(s).instance_members {
                if cx.especie(f) == Some(Especie::Getter) && !names.contains(&key) {
                    names.push(key);
                }
            }
        }
        names.sort_by_key(|n| interner.resolve(*n).to_string());
        for the_name in names {
            let text = interner.resolve(the_name).to_string();
            let Some(setter_key) = interner.lookup(&format!("{text}_=")) else { continue };
            let Some((mut g, tg)) = cx.in_interface(this_one, the_name, 0) else { continue };
            let Some((s, ts)) = cx.in_interface(this_one, setter_key, 0) else { continue };
            if of_extension && g.dono == cid && s.dono == cid {
                continue;
            }
            if enum_synthetic(g.funcao)
                && let Some(e) = enum_do_core
            {
                g.dono = e;
            }
            if text.starts_with('_')
                && (program.class(g.dono).library != cls.library || program.class(s.dono).library != cls.library)
            {
                continue;
            }
            if cx.especie(g.funcao) != Some(Especie::Getter) || cx.especie(s.funcao) != Some(Especie::Setter) {
                continue;
            }
            let (Type::Function { ret, .. }, Type::Function { positional, optional, named, .. }) =
                (cx.table.get(tg).clone(), cx.table.get(ts).clone())
            else {
                continue;
            };
            if positional.len() + optional.len() + named.len() != 1 {
                continue;
            }
            let Some(&param) = positional.first().or(optional.first()) else { continue };
            let ok = {
                let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
                is_subtype(ret, param, &mut env)
            };
            if ok {
                continue;
            }
            let place = if g.dono == cid {
                name_of(program, g.funcao)
            } else if s.dono == cid {
                name_of(program, s.funcao)
            } else {
                let ast_ = &program.unit(decl.unit).ast;
                match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Enum(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Mixin(d) => Some((decl.unit, d.name.span)),
                    DeclKind::ExtensionType(d) => Some((decl.unit, d.name.span)),
                    _ => None,
                }
            };
            let Some((u, span)) = place else { continue };
            let qualified = |owner: ClassId| {
                if owner == cid { text.clone() } else { format!("{}.{text}", interner.resolve(program.class(owner).name)) }
            };
            let args = [qualified(g.dono), formatar(cx.table, ret, interner, program), formatar(cx.table, param, interner, program), qualified(s.dono)];
            output.push((u, Diagnostic::com_codigo(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args)));
        }
    }
    output
}

/// Começo de um nó anotado (`AnnotatedNodeImpl.beginToken`): o comentário
/// de documentação logo antes, se houver.
fn start_with_documentation(source: &str, start: usize) -> usize {
    let without_space = source[..start].trim_end();
    if let Some(without_end) = without_space.strip_suffix("*/") {
        if let Some(i) = without_end.rfind("/**")
            && !without_end[i..].contains("*/")
        {
            return i;
        }
        return start;
    }
    let mut result = start;
    let mut end = without_space.len();
    loop {
        let start_line = without_space[..end].rfind('\n').map_or(0, |i| i + 1);
        let line = without_space[start_line..end].trim_start();
        if !(line.starts_with("///") && !line.starts_with("////")) {
            break;
        }
        result = end - line.len();
        if start_line == 0 {
            break;
        }
        end = without_space[..start_line].trim_end().len();
    }
    result
}

impl Ctx<'_> {
    /// `getMember2(d, nome, concrete: true)`: o `implemented` da interface
    /// (com os encaminhadores de `noSuchMethod` e a covariância herdada).
    fn implementado(&mut self, d: ClassId, key: SymbolId, _prof: u32) -> Option<Achado> {
        let the_name = crate::heranca::Nome::novo(self.interner, self.biblioteca, key);
        let mut h = std::mem::take(&mut self.heranca);
        let r = {
            let mut p = crate::heranca::ProvedorDoOutline {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            h.membro(&mut p, d, the_name, true, None, false)
        };
        self.heranca = h;
        r.map(|m| Achado { dono: m.classe, funcao: m.funcao })
    }

    /// `InterfaceElement.supertype`: a do modelo, e `Enum` num enum
    /// (`LibraryBuilder.buildEnumChildren`).
    fn superclass(&self, d: ClassId) -> Option<ClassId> {
        let cls = self.program.class(d);
        match cls.supertype_class {
            // `extends` de enum ou tipo de extensão (`extends_non_class`): o
            // supertipo fica `Object`.
            Some(s) if self.outside_hierarchy(d, s) => self.core_class("Object"),
            Some(s) => Some(s),
            None if cls.kind == dartforge_elements::model::ClassKind::Enum => self.core_class("Enum"),
            None => None,
        }
    }

    /// O supertipo `s` de `d` é enum ou tipo de extensão numa classe, mixin
    /// ou enum (o analyzer o descarta da cláusula).
    fn outside_hierarchy(&self, d: ClassId, s: ClassId) -> bool {
        use dartforge_elements::model::ClassKind as K;
        if self.program.class(d).kind != K::ExtensionType && matches!(self.program.class(s).kind, K::Enum | K::ExtensionType) {
            return true;
        }
        // Uma classe (não enum) só chega a `Enum`/`_Enum` do `dart:core` pelo
        // `implements` de um enum, que é `implements_non_class`: o tipo sai
        // da cláusula, e com ele os supertipos dele.
        let sc = self.program.class(s);
        self.program.class(d).kind != K::Enum
            && matches!(self.interner.resolve(sc.name), "Enum" | "_Enum")
            && self.program.library(sc.library).uri == "dart:core"
    }

    fn of_object(&self, c: ClassId) -> bool {
        let cls = self.program.class(c);
        self.interner.resolve(cls.name) == "Object" && self.program.library(cls.library).uri == "dart:core"
    }

    fn core_class(&self, the_name: &str) -> Option<ClassId> {
        let lib = self.program.core?;
        let sym = self.interner.lookup(the_name)?;
        match self.program.library(lib).declared.get(&sym)?.getter? {
            dartforge_elements::model::Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// O nome privado `k` é declarado por `c` ou por um supertipo da
    /// biblioteca `lib`.
    fn library_private(&self, c: ClassId, k: SymbolId, lib: dartforge_elements::model::LibraryId) -> bool {
        let supers = self.outline.hierarchy.get(c).map(|d| d.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default();
        std::iter::once(c).chain(supers).any(|s| self.program.class(s).library == lib && self.program.class(s).instance_members.contains_key(&k))
    }

    /// Todas as chaves de membros de instância de `c` e dos supertipos.
    fn hierarchy_keys(&self, c: ClassId) -> Vec<SymbolId> {
        let mut v: Vec<SymbolId> = Vec::new();
        let supers = self.outline.hierarchy.get(c).map(|d| d.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default();
        for s in std::iter::once(c).chain(supers) {
            if s != c && self.outside_hierarchy(c, s) {
                continue;
            }
            for &k in self.program.class(s).instance_members.keys() {
                if !v.contains(&k) {
                    v.push(k);
                }
            }
        }
        v.sort_by_key(|k| self.interner.resolve(*k).to_string());
        v
    }

    fn own_type(&mut self, c: ClassId) -> Option<TypeId> {
        let params = self.outline.classes.get(c.0 as usize)?.type_params.clone();
        let args: Box<[TypeId]> = params.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        Some(self.table.intern(Type::Interface { class: c, args, nullable: false }))
    }
}

/// A parte do `verify()` do `InheritanceOverrideVerifier` sobre classes
/// concretas e enums (`src/error/inheritance_override.dart`): para cada nome
/// da interface sem implementação concreta, `concrete_class_with_abstract_member`
/// (ou `enum_with_abstract_member`) no membro abstrato que a própria classe
/// declara, senão `non_abstract_class_inherits_abstract_member` no nome da
/// classe, com os membros herdados (menos os que a superclasse concreta já
/// deixa sem implementar); havendo `noSuchMethod` que não é o de `Object`,
/// os que faltam são encaminhados. Com implementação concreta de outra
/// assinatura, `invalid_implementation_override`.
pub fn membros_abstratos(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut output = Vec::new();
    let nsm = interner.lookup("noSuchMethod");
    for &cid in classes {
        let cls = program.class(cid);
        cx.biblioteca = cls.library;
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let source = &program.unit(decl.unit).source;
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) if !d.modifiers.abstract_ && !d.modifiers.sealed => (d.name, &d.members),
            DeclKind::Enum(d) => (d.name, &d.members),
            _ => continue,
        };
        let e_enum = cls.kind == dartforge_elements::model::ClassKind::Enum;
        // Membro de `augment`, ou método cujo nome vem depois de `.` (um
        // construtor com o nome da classe errado): a recuperação do parser
        // difere da do analyzer e a classe não se decide.
        let recovered = members.iter().any(|&m| {
            let mem = ast_.member(m);
            mem.augment
                || matches!(&mem.kind, MemberKind::Method(f)
                    if ast_.function(*f).name.is_some_and(|n| source[..n.span.start].trim_end().ends_with('.')))
        });
        // A palavra `augment` no começo de um membro, sem o experimento, é
        // lida de outro jeito pelo parser do analyzer.
        let sp = ast_.decl(decl.decl).span;
        let decl_text = source.get(sp.start..sp.end).unwrap_or("");
        // Sem o experimento, o parser lê `augment` como identificador e a
        // recuperação é a do analyzer (T1.1 f): nada a pular.
        let augment_in_text = program.biblioteca_com_augmentations(cls.library)
            && decl_text.lines().any(|l| l.trim_start().starts_with("augment "));
        if recovered || augment_in_text || ast_.decl(decl.decl).augment {
            continue;
        }
        // T1.1 e: a interface é a do dono do grupo de homônimos (o cache do
        // analyzer é por elemento), e o membro abstrato é procurado na árvore
        // da própria declaração (`_reportConcreteClassWithAbstractMember`).
        let intf = program.dono_da_classe(cid);
        let Some(this_one) = cx.own_type(intf) else { continue };
        // `noSuchMethod` implementado que não é o de `Object`: o nome sem
        // implementação concreta ganha o encaminhador (o membro da interface
        // vira o `implemented`, 3.6.2 `inheritance_manager3.dart`), então nada
        // sai por ele; a implementação concreta que existe continua conferida
        // (`Enum.index` contra um `Never get index` da interface).
        let forwards = nsm.is_some_and(|n| {
            cx.implementado(intf, n, 0).is_some_and(|a| program.library(program.class(a.dono).library).uri != "dart:core")
        });
        // `_isNotImplementedInConcreteSuperClass`: a superclasse declarada,
        // se concreta.
        let concrete_superclass = program.class(intf).supertype_class.filter(|&s| {
            let sc = program.class(s);
            sc.decl.is_some()
                && sc.kind == dartforge_elements::model::ClassKind::Class
                && !sc.modifiers.abstract_
                && !sc.modifiers.sealed
        });
        let mut the_inherited: Vec<String> = Vec::new();
        let mut uncertain = false;
        for key in cx.hierarchy_keys(intf) {
            let text = interner.resolve(key).to_string();
            // Um nome privado só declarado em outras bibliotecas (`_name` de
            // `_Enum`) é outro `Name` para esta classe: não está na interface
            // que ela vê.
            if text.starts_with('_') && !cx.library_private(intf, key, cls.library) {
                continue;
            }
            let Some((mem, member_type)) = cx.in_interface(this_one, key, 0) else {
                // Conflito ou assinatura combinada que não se decide aqui.
                if !program.class(intf).instance_members.contains_key(&key) {
                    uncertain = true;
                }
                continue;
            };
            if text.starts_with('_') && program.class(mem.dono).library != cls.library {
                continue;
            }
            let Some(the_kind) = cx.especie(mem.funcao) else { continue };
            let displayed = text.strip_suffix("_=").unwrap_or(&text).to_string();
            match cx.implementado(intf, key, 0) {
                None if forwards => {}
                None => {
                    // `_reportConcreteClassWithAbstractMember` (3.6.2
                    // `inheritance_override.dart:809-845`): o primeiro membro
                    // da declaração com o nome, estático ou não; o campo dá
                    // também o `nome=` quando não é `final` (o `isFinal` do
                    // nó não conta o `const`).
                    let declared = members.iter().find(|&&m| match &ast_.member(m).kind {
                        MemberKind::Method(f) => {
                            let af = ast_.function(*f);
                            af.name.is_some_and(|n| {
                                let base = interner.resolve(n.sym);
                                if af.kind == ast::FunctionKind::Setter { format!("{base}_=") == text } else { base == text && af.kind != ast::FunctionKind::Setter }
                            })
                        }
                        MemberKind::Field(vl) => vl.variables.iter().any(|v| {
                            let n = interner.resolve(v.name.sym);
                            n == text || (!vl.final_ && format!("{n}_=") == text)
                        }),
                        MemberKind::Constructor(_) => false,
                    });
                    if let Some(&m) = declared {
                        let mut sp = ast_.member(m).span;
                        // `var get a;`, `const m();`, `abstract void m();`…: os
                        // modificadores que o `MethodDeclaration` não guarda (erros
                        // de sintaxe) ficam fora do nó; `external`/`static` ficam.
                        if let MemberKind::Method(_) = &ast_.member(m).kind {
                            const OUTSIDE: [&str; 7] = ["var", "const", "final", "abstract", "covariant", "late", "required"];
                            while let Some(rest) =
                                OUTSIDE.iter().find_map(|k| source[sp.start..sp.end].strip_prefix(k).filter(|r| r.starts_with(char::is_whitespace)))
                            {
                                sp.start = sp.end - rest.trim_start().len();
                            }
                        }
                        let span = Span { start: start_with_documentation(source, sp.start), end: sp.end };
                        let code = if e_enum { c::ENUM_WITH_ABSTRACT_MEMBER } else { c::CONCRETE_CLASS_WITH_ABSTRACT_MEMBER };
                        output.push((decl.unit, Diagnostic::com_codigo(code, span, [displayed.as_str(), interner.resolve(cls.name)])));
                        continue;
                    }
                    if let Some(s) = concrete_superclass
                        && let Some(st) = outline.hierarchy.supertype_of(this_one, s, cx.table, core)
                        && cx.in_interface(st, key, 0).is_some()
                    {
                        continue;
                    }
                    if e_enum && (text == "values" || text == "values_=") {
                        continue;
                    }
                    let prefix = match the_kind {
                        Especie::Getter => "getter ",
                        Especie::Setter => "setter ",
                        Especie::Metodo => "",
                    };
                    the_inherited.push(format!("{prefix}{}.{displayed}", interner.resolve(program.class(mem.dono).name)));
                }
                Some(concrete) => {
                    // A implementação concreta contra a assinatura da interface.
                    if concrete == mem || cx.especie(concrete.funcao) != Some(the_kind) {
                        continue;
                    }
                    let Some(concrete_type) = cx.seen_type(this_one, concrete) else { continue };
                    // `_inheritCovariance`: a covariância vem de todos os
                    // membros homônimos na hierarquia da classe.
                    let (pos, names) = cx.effective_covariant(intf, concrete.funcao, key);
                    let to_sub = cx.for_subtype(concrete_type, &pos, &names);
                    let ok = {
                        let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
                        is_subtype(to_sub, member_type, &mut env)
                    };
                    if !ok {
                        let code = if the_kind == Especie::Setter {
                            c::INVALID_IMPLEMENTATION_OVERRIDE_SETTER
                        } else {
                            c::INVALID_IMPLEMENTATION_OVERRIDE
                        };
                        let args = [
                            displayed.clone(),
                            interner.resolve(program.class(concrete.dono).name).to_string(),
                            formatar(cx.table, concrete_type, interner, program),
                            interner.resolve(program.class(mem.dono).name).to_string(),
                            formatar(cx.table, member_type, interner, program),
                        ];
                        output.push((decl.unit, Diagnostic::com_codigo(code, class_name.span, args)));
                    }
                }
            }
        }
        if uncertain || the_inherited.is_empty() {
            continue;
        }
        the_inherited.sort();
        let code = match the_inherited.len() {
            1 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE,
            2 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO,
            3 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE,
            4 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR,
            _ => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS,
        };
        let mut args: Vec<String> = the_inherited.iter().take(4).cloned().collect();
        if the_inherited.len() > 4 {
            args.push((the_inherited.len() - 4).to_string());
        }
        output.push((decl.unit, Diagnostic::com_codigo(code, class_name.span, args)));
    }
    output
}

/// `TypeSystemImpl.isNullable`: o tipo aceita `null` com certeza.
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

/// `NOT_INITIALIZED_NON_NULLABLE_VARIABLE`
/// (`_checkForNotInitializedNonNullableVariable`,
/// `an611:src/generated/error_verifier.dart:4872-4911`): variável de topo
/// (não `final`) ou campo estático, sem `const`, `late` nem `external`, com
/// tipo escrito potencialmente não anulável e sem inicializador — no nome.
pub fn variaveis_nao_inicializadas(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::VariableRef;
    let mut output = Vec::new();
    for (i, v) in program.variables.iter().enumerate() {
        if v.library != lib {
            continue;
        }
        let (unit, list, top) = match v.node {
            VariableRef::TopLevel { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(l) => (unit, (l, index), true),
                _ => continue,
            },
            VariableRef::Field { unit, member, index } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(l) if l.static_ => (unit, (l, index), false),
                _ => continue,
            },
            _ => continue,
        };
        let (l, index) = list;
        if l.const_ || (top && l.final_) || l.late || l.external || l.ty.is_none() {
            continue;
        }
        let Some(var) = l.variables.get(index) else { continue };
        if var.initializer.is_some() {
            continue;
        }
        let Some(t) = outline.variables.get(i).and_then(|d| d.declared_type) else { continue };
        if anulavel(table, t) {
            continue;
        }
        // Também no nome sintético (vazio), como o analyzer.
        let the_name = var.name;
        output.push((unit, Diagnostic::com_codigo(c::NOT_INITIALIZED_NON_NULLABLE_VARIABLE, the_name.span, [interner.resolve(the_name.sym)])));
    }
    output
}

/// `ErrorVerifier._checkUseOfDefaultValuesInParameters`: parâmetro opcional
/// sem valor padrão cujo tipo pode não aceitar `null`
/// (`missing_default_value_for_parameter`, `_POSITIONAL`,
/// `_WITH_ANNOTATION` com `@required`), nas funções de topo, métodos e
/// construtores da biblioteca em que o valor padrão é esperado (nem
/// abstrato, nem `external`, nem nativo, nem `factory` redirecionador). O
/// tipo vale quando escrito ou num `this.x`; sem tipo escrito (inferido da
/// sobrescrita ou do construtor da superclasse), não se decide.
pub fn default_values(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    valores_padrao_com(program, interner, table, None, outline, None, lib)
}

/// O parâmetro `super.x` (de índice `i` em `f`) e o parâmetro do construtor
/// da superclasse que ele encaminha (`superConstructorParameter`, 3.6.2
/// `element.dart:9291-9310`): pelo nome, se nomeado; senão pela posição
/// entre os `super.x` do construtor, nos posicionais do construtor da
/// superclasse (o do `super.nome(…)`, ou o sem nome).
fn parametro_do_super(program: &Program, interner: &Interner, f: FunctionElementId, i: usize) -> Option<(FunctionElementId, usize)> {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return None };
    let p = k.parameters.get(i)?;
    let c = program.function(f).class?;
    let s = program.class(c).supertype_class?;
    let super_name_of = k.initializers.iter().find_map(|x| match x {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    let key = match super_name_of {
        Some(Some(n)) if interner.resolve(n.sym) != "new" => Some(n.sym),
        _ => interner.lookup(""),
    }?;
    let fs = *program.class(s).constructors.get(&key)?;
    let FunctionRef::Constructor { unit: su, member: sm } = program.function(fs).node else { return None };
    let MemberKind::Constructor(sk) = &program.unit(su).ast.member(sm).kind else { return None };
    if p.kind == ast::ParameterKind::Named {
        let the_name = p.nome_externo()?.sym;
        let j = sk.parameters.iter().position(|q| q.kind == ast::ParameterKind::Named && q.nome_externo().is_some_and(|n| n.sym == the_name))?;
        return Some((fs, j));
    }
    let the_index = k.parameters[..i].iter().filter(|q| q.super_).count();
    let positional_args: Vec<usize> = sk.parameters.iter().enumerate().filter(|(_, q)| q.kind != ast::ParameterKind::Named).map(|(j, _)| j).collect();
    positional_args.get(the_index).map(|&j| (fs, j))
}

/// O tipo do valor padrão do parâmetro `i` de `f` (o `computeConstantValue`
/// dele: o valor escrito, ou o encaminhado pela cadeia de `super.x`), se
/// houver; `None` sem valor. O tipo do valor é o estático do padrão escrito
/// (dos corpos), ou o do parâmetro sem eles.
fn parameter_default_value(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    f: FunctionElementId,
    i: usize,
    prof: u32,
) -> Option<TypeId> {
    if prof > 32 {
        return None;
    }
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return None };
    let p = k.parameters.get(i)?;
    let type_p = outline.functions.get(f.0 as usize)?.parameters.get(i)?.ty;
    if let Some(d) = p.default_value {
        return Some(bodies.and_then(|c| c.units.get(unit.0 as usize)).and_then(|u| u.get_type(d)).unwrap_or(type_p));
    }
    if !p.super_ {
        return None;
    }
    // `_superConstructorParameterDefaultValue`: o valor do parâmetro da
    // superclasse, se o tipo dele cabe no deste.
    let (fs, j) = parametro_do_super(program, interner, f, i)?;
    let v = parameter_default_value(program, interner, table, core, outline, bodies, fs, j, prof + 1)?;
    let core = core?;
    // O tipo do valor não conhecido aqui (biblioteca da superclasse sem os
    // corpos inferidos, como na análise de um arquivo só no LSP): o valor
    // existe, e a checagem fica com quem o conhece (sem falso
    // `missing_default_value_for_parameter`).
    if v == core.dynamic_ {
        return Some(type_p);
    }
    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
    is_subtype(v, type_p, &mut env).then_some(v)
}

/// `defaultValueCode != null` do parâmetro `i` de `f` (o `hasDefaultValue`):
/// o padrão escrito; num `super.x` opcional sem padrão, o do parâmetro da
/// superclasse, se o valor encaminhado existir
/// (`DefaultSuperFormalParameterElementImpl.defaultValueCode`,
/// `element.dart:1732-1748`); o `required` nunca tem.
#[allow(clippy::too_many_arguments)]
fn has_default(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    f: FunctionElementId,
    i: usize,
    prof: u32,
) -> bool {
    if prof > 32 {
        return false;
    }
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return false };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return false };
    let Some(p) = k.parameters.get(i) else { return false };
    if p.default_value.is_some() {
        return true;
    }
    if !p.super_ || p.required || p.kind == ast::ParameterKind::Required {
        return false;
    }
    if parameter_default_value(program, interner, table, core, outline, bodies, f, i, prof).is_none() {
        return false;
    }
    match parametro_do_super(program, interner, f, i) {
        Some((fs, j)) => has_default(program, interner, table, core, outline, bodies, fs, j, prof + 1),
        None => false,
    }
}

/// Como [`valores_padrao`], com os tipos núcleo e os corpos (o tipo do valor
/// padrão encaminhado por `super.x`).
#[allow(clippy::too_many_arguments)]
pub fn valores_padrao_com(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        let Some(data) = outline.functions.get(i) else { continue };
        let (unit, params, expected): (UnitId, &[ast::Parameter], bool) = match f.node {
            FunctionRef::Function { unit, function } => {
                let af = &program.unit(unit).ast.functions[function.0 as usize];
                let native = matches!(af.body, ast::FunctionBody::Native(_));
                let the_abstract_ = f.class.is_some() && matches!(af.body, ast::FunctionBody::Empty);
                (unit, af.parameters.as_deref().unwrap_or(&[]), !af.external && !native && !the_abstract_)
            }
            FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (unit, &k.parameters[..], !k.external && !(k.factory && k.redirect.is_some())),
                _ => continue,
            },
            FunctionRef::None => continue,
        };
        if !expected || params.len() != data.parameters.len() {
            continue;
        }
        // A linha da própria declaração, ou a da classe que a contém, começa
        // com `augment`: sem o experimento, o analyzer a lê de outro jeito.
        let source = &program.unit(unit).source;
        let line_starts_with_augment = |pos: usize| {
            let begin = source[..pos.min(source.len())].rfind('\n').map_or(0, |i| i + 1);
            source[begin..].trim_start().starts_with("augment ")
        };
        let start = match f.node {
            FunctionRef::Function { function, .. } => program.unit(unit).ast.functions[function.0 as usize].span.start,
            FunctionRef::Constructor { member, .. } => program.unit(unit).ast.member(member).span.start,
            FunctionRef::None => 0,
        };
        let of_class = f.class.and_then(|c| program.class(c).decl).is_some_and(|d| {
            d.unit == unit && line_starts_with_augment(program.unit(unit).ast.decl(d.decl).span.start)
        });
        let the_augmented =
            program.biblioteca_com_augmentations(program.unit(unit).library) && (line_starts_with_augment(start) || of_class);
        if the_augmented {
            continue;
        }
        for (idx, (p, pd)) in params.iter().zip(data.parameters.iter()).enumerate() {
            if p.kind == ast::ParameterKind::Required || p.required || p.default_value.is_some() {
                continue;
            }
            // `super.x` opcional: o padrão encaminhado do construtor da
            // superclasse conta (`hasDefaultValue`); o curinga posicional
            // `_` com o recurso fica de fora
            // (`_isWildcardSuperFormalPositionalParameter`).
            if p.super_ {
                // O `_` repetido (sem o recurso de curingas, `duplicate_definition`)
                // não chega ao parâmetro da superclasse: sem padrão herdado
                // (conferido no oráculo 3.6.2, `C(this.c, super._, [super._])`).
                let repeated = p.name.is_some_and(|n| {
                    interner.resolve(n.sym) == "_" && params[..idx].iter().any(|q| q.name.is_some_and(|m| m.sym == n.sym))
                });
                if !repeated && has_default(program, interner, table, core, outline, bodies, FunctionElementId(i as u32), idx, 0) {
                    continue;
                }
                let wildcard = p.kind == ast::ParameterKind::Optional
                    && p.name.is_some_and(|n| interner.resolve(n.sym) == "_")
                    && program.library(lib).features.tem(dartforge_frontend::Feature::WildcardVariables);
                if wildcard {
                    continue;
                }
            }
            // Tipo escrito que não resolve (`dynamic` no outline): nada.
            if anulavel(table, pd.ty) {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            let annotated = p.metadata.iter().any(|a| a.name.last().is_some_and(|n| interner.resolve(n.sym) == "required"));
            let d = if annotated {
                Diagnostic::com_codigo(c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION, the_name.span, [] as [&str; 0])
            } else {
                let code = if p.kind == ast::ParameterKind::Optional {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL
                } else {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER
                };
                Diagnostic::com_codigo(code, the_name.span, [interner.resolve(the_name.sym)])
            };
            output.push((unit, d));
        }
    }
    output
}

/// `ErrorVerifier._checkForConflictingClassMembers` (classes, mixins e tipos
/// de extensão da biblioteca): estático declarado contra membro herdado
/// (`conflicting_static_and_instance`, com a classe dona do herdado), método
/// declarado contra campo herdado (`conflicting_method_and_field`), acessor
/// declarado contra método herdado (`conflicting_field_and_method`), e método
/// e setter herdados com o mesmo nome
/// (`conflicting_inherited_method_and_setter`), pelo `getInherited2`.
pub fn membros_em_conflito(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut output = Vec::new();
    for (ci, cls) in program.classes.iter().enumerate() {
        cx.biblioteca = cls.library;
        let cid = ClassId(ci as u32);
        if cls.library != lib || !matches!(cls.kind, K::Class | K::Mixin | K::ExtensionType) {
            continue;
        }
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        if ast_.decl(decl.decl).augment {
            continue;
        }
        let source = &program.unit(decl.unit).source;
        if program.biblioteca_com_augmentations(lib) && source.lines().any(|l| l.trim_start().starts_with("augment ")) {
            continue;
        }
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            DeclKind::ExtensionType(d) => (d.name, &d.members),
            _ => continue,
        };
        // T1.1 e: a interface herdada é a do dono do grupo de homônimos; os
        // membros declarados são os da própria declaração.
        let intf = program.dono_da_classe(cid);
        let Some(this_one) = cx.own_type(intf) else { continue };
        let class_text = interner.resolve(cls.name).to_string();
        let visible_inherited = |cx: &mut Ctx<'_>, the_name: &str| -> Option<Achado> {
            let key = interner.lookup(the_name)?;
            let (a, _) = cx.herdado(this_one, key)?;
            if the_name.starts_with('_') && program.class(a.dono).library != lib {
                return None;
            }
            Some(a)
        };
        let owner = |a: Achado| interner.resolve(program.class(a.dono).name).to_string();
        let mut conflicting: Vec<String> = Vec::new();
        // Métodos declarados contra acessores herdados.
        for &m in members {
            let MemberKind::Method(f) = &ast_.member(m).kind else { continue };
            let af = ast_.function(*f);
            if !matches!(af.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                continue;
            }
            let Some(n) = af.name else { continue };
            let the_name = interner.resolve(n.sym).to_string();
            let getter = visible_inherited(&mut cx, &the_name);
            let setter = visible_inherited(&mut cx, &format!("{the_name}_="));
            if af.static_
                && let Some(a) = getter.or(setter)
            {
                output.push((
                    decl.unit,
                    Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, n.span, [class_text.clone(), the_name.clone(), owner(a)]),
                ));
                continue;
            }
            if cls.kind == K::ExtensionType {
                continue;
            }
            let accessor = |a: Option<Achado>| a.filter(|a| cx.especie(a.funcao).is_some_and(|e| e != Especie::Metodo));
            if let Some(a) = accessor(getter).or(accessor(setter)) {
                output.push((
                    decl.unit,
                    Diagnostic::com_codigo(c::CONFLICTING_METHOD_AND_FIELD, n.span, [class_text.clone(), the_name.clone(), owner(a)]),
                ));
            }
        }
        // Acessores declarados (getters, setters e os dos campos) contra
        // métodos herdados.
        for &m in members {
            let mut accessors: Vec<(String, Span, bool)> = Vec::new();
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if matches!(af.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter)
                        && let Some(n) = af.name
                    {
                        accessors.push((interner.resolve(n.sym).to_string(), n.span, af.static_));
                    }
                }
                MemberKind::Field(vl) => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span, vl.static_));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span, vl.static_));
                        }
                    }
                }
                MemberKind::Constructor(_) => {}
            }
            for (the_name, span, static_) in accessors {
                let inherited = visible_inherited(&mut cx, &the_name).or_else(|| visible_inherited(&mut cx, &format!("{the_name}_=")));
                if static_ && let Some(a) = inherited {
                    output.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [class_text.clone(), the_name.clone(), owner(a)]),
                    ));
                    conflicting.push(the_name);
                } else if let Some(a) = inherited
                    && cx.especie(a.funcao) == Some(Especie::Metodo)
                {
                    if cls.kind == K::ExtensionType {
                        continue;
                    }
                    output.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::CONFLICTING_FIELD_AND_METHOD, span, [class_text.clone(), the_name.clone(), owner(a)]),
                    ));
                    conflicting.push(the_name);
                }
            }
        }
        // Método e setter herdados com o mesmo nome.
        for key in cx.hierarchy_keys(intf) {
            let the_name = interner.resolve(key).to_string();
            if the_name.ends_with("_=") || conflicting.contains(&the_name) {
                continue;
            }
            let Some(method) = visible_inherited(&mut cx, &the_name) else { continue };
            if cx.especie(method.funcao) != Some(Especie::Metodo) {
                continue;
            }
            if let Some(setter) = visible_inherited(&mut cx, &format!("{the_name}_="))
                && cx.especie(setter.funcao) == Some(Especie::Setter)
            {
                let typ = match cls.kind {
                    K::Mixin => "mixin",
                    K::ExtensionType => "extension type",
                    _ => "class",
                };
                // As duas mensagens de contexto: de onde vêm o método e o
                // setter (o tipo e o nome da classe dona, no nome do membro).
                let mut d = Diagnostic::com_codigo(c::CONFLICTING_INHERITED_METHOD_AND_SETTER, class_name.span, [typ, class_text.as_str(), the_name.as_str()]);
                for (a, template) in [(method, "The method is inherited from the"), (setter, "The setter is inherited from the")] {
                    let p = program;
                    let (f, owner) = (a.funcao, a.dono);
                    let owner_kind = match p.class(owner).kind {
                        K::Mixin => "mixin",
                        K::Enum => "enum",
                        K::ExtensionType => "extension type",
                        _ => "class",
                    };
                    let Some((u, span)) = p.nome_nao_sintetico_da_funcao(f) else { continue };
                    let file = (u != decl.unit).then(|| p.caminho_da_unidade(u).into());
                    let text = format!("{template} {owner_kind} '{}'.", interner.resolve(p.class(owner).name));
                    d.contexto.push(dartforge_diagnostics::Contexto { arquivo: file, span, mensagem: text.into() });
                }
                output.push((decl.unit, d));
            }
        }
    }
    output
}

/// `DuplicateDefinitionVerifier._checkEnumStatic`: cada acessor estático (os
/// das constantes e dos campos, getters e setters declarados; menos
/// `values`) e cada método estático de um enum cujo nome está na interface
/// dele (`getMember2` do nome ou do setter, com os membros herdados de
/// `Enum` e `Object`), no nome, com o enum como dono.
pub fn estaticos_de_enum(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: lib };
    let mut output = Vec::new();
    for (ci, cls) in program.classes.iter().enumerate() {
        if cls.library != lib || cls.kind != K::Enum {
            continue;
        }
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let DeclKind::Enum(d) = &ast_.decl(decl.decl).kind else { continue };
        let owner = program.dono_da_classe(ClassId(ci as u32));
        let enum_text = interner.resolve(program.class(owner).name).to_string();
        let in_interface = |cx: &mut Ctx<'_>, the_name: &str| -> bool {
            // O getter `name` sintético do modelo (`outline.rs`, para os
            // back-ends) não está na interface do analyzer: o `name` de um enum
            // é a extensão `EnumName` do `dart:core` (o `Enum` só tem `_name`),
            // e `static int name` num enum não conflita.
            let synthetic = |m: &crate::heranca::Membro| {
                let f = program.function(m.funcao);
                m.classe == owner && f.node == dartforge_elements::model::FunctionRef::None && interner.resolve(f.name) == "name"
            };
            let found = |cx: &mut Ctx<'_>, text: &str| {
                interner.lookup(text).is_some_and(|k| cx.inheritance_member(owner, k, false).is_some_and(|m| !synthetic(&m)))
            };
            found(cx, the_name) || found(cx, &format!("{the_name}_="))
        };
        // Os acessores: as constantes (getters), os campos estáticos (getter
        // e, se não final, setter) e os getters e setters declarados.
        let mut accessors: Vec<(String, Span)> = Vec::new();
        for k in d.constants.iter() {
            accessors.push((interner.resolve(k.name.sym).to_string(), k.name.span));
        }
        let mut methods: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if vl.static_ => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if !af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let the_name = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => accessors.push((the_name, n.span)),
                        _ => methods.push((the_name, n.span)),
                    }
                }
                _ => {}
            }
        }
        for (the_name, span) in accessors {
            if the_name != "values" && in_interface(&mut cx, &the_name) {
                output.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [enum_text.as_str(), the_name.as_str(), enum_text.as_str()])));
            }
        }
        for (the_name, span) in methods {
            if in_interface(&mut cx, &the_name) {
                output.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [enum_text.as_str(), the_name.as_str(), enum_text.as_str()])));
            }
        }
        // `_checkEnum` (3.6.2 `duplicate_definition_verifier.dart:600-640`): o
        // acessor de instância declarado (getter, setter, os do campo) contra
        // o método herdado, e o método de instância contra o acessor herdado
        // (`_getInheritedMember`: o getter herdado, senão o setter; o nome
        // privado de outra biblioteca não é visto). Com um enum homônimo na
        // mesma unidade (o `augment enum E` que o 3.6.2 lê como outra
        // declaração, ou um `enum E` repetido), o 3.6.2 não relata nada, em
        // qualquer ordem: os dois elementos são iguais pela localização.
        let same_name = program.classes.iter().enumerate().any(|(j, k)| {
            j != ci && k.kind == K::Enum && k.name == cls.name && k.decl.is_some_and(|x| x.unit == decl.unit)
        });
        if same_name {
            continue;
        }
        let Some(this_one) = cx.own_type(owner) else { continue };
        let inherited = |cx: &mut Ctx<'_>, the_name: &str| -> Option<Achado> {
            let visible = |cx: &mut Ctx<'_>, text: &str| -> Option<Achado> {
                let key = interner.lookup(text)?;
                let (a, _) = cx.herdado(this_one, key)?;
                if text.starts_with('_') && program.class(a.dono).library != lib {
                    return None;
                }
                Some(a)
            };
            visible(cx, the_name).or_else(|| visible(cx, &format!("{the_name}_=")))
        };
        let mut accessors: Vec<(String, Span)> = Vec::new();
        let mut methods: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let the_name = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => accessors.push((the_name, n.span)),
                        _ => methods.push((the_name, n.span)),
                    }
                }
                _ => {}
            }
        }
        let owner_of = |a: &Achado| interner.resolve(program.class(a.dono).name).to_string();
        for (the_name, span) in accessors {
            if let Some(a) = inherited(&mut cx, &the_name)
                && cx.especie(a.funcao) == Some(Especie::Metodo)
            {
                let d_ = owner_of(&a);
                output.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_FIELD_AND_METHOD, span, [enum_text.as_str(), the_name.as_str(), d_.as_str()])));
            }
        }
        for (the_name, span) in methods {
            if let Some(a) = inherited(&mut cx, &the_name)
                && cx.especie(a.funcao).is_some_and(|e| e != Especie::Metodo)
            {
                let d_ = owner_of(&a);
                output.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_METHOD_AND_FIELD, span, [enum_text.as_str(), the_name.as_str(), d_.as_str()])));
            }
        }
    }
    output
}

/// A parte do `verify()` sobre classes que implementam `Enum`
/// (`implementsDartCoreEnum`, pelos supertipos transitivos, e todo enum):
/// `index`, `hashCode` e `==` concretos declarados
/// (`illegal_concrete_enum_member`, na classe, no enum ou no mixin) ou
/// herdados de outra classe que não `Object`/`Enum` (no nome da classe), e o
/// membro de instância `values` declarado (fora de enum) ou herdado
/// (`illegal_enum_values`).
pub fn membros_de_enum(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut output = Vec::new();
    let Some(enum_core) = cx.core_class("Enum") else { return output };
    for &cid in classes {
        let cls = program.class(cid);
        cx.biblioteca = cls.library;
        let Some(decl) = cls.decl else { continue };
        let implements_enum = cls.kind == K::Enum
            || outline.hierarchy.get(cid).is_some_and(|d| d.supertypes.contains_key(&enum_core));
        if !implements_enum || cid == enum_core {
            continue;
        }
        let ast_ = &program.unit(decl.unit).ast;
        let source = &program.unit(decl.unit).source;
        if program.biblioteca_com_augmentations(cls.library) && source.lines().any(|l| l.trim_start().starts_with("augment ")) {
            continue;
        }
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Enum(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            _ => continue,
        };
        let e_enum = cls.kind == K::Enum;
        let forbidden = |n: &str| matches!(n, "index" | "hashCode" | "==");
        // Declarados.
        for &m in members {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let n = interner.resolve(v.name.sym);
                        if !e_enum && n == "values" {
                            output.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_DECLARATION, v.name.span, [] as [&str; 0])));
                        }
                        if forbidden(n) {
                            output.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION, v.name.span, [n])));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    let Some(the_name) = af.name else { continue };
                    let n = interner.resolve(the_name.sym);
                    let the_abstract_ = !af.external && matches!(af.body, ast::FunctionBody::Empty);
                    let setter = af.kind == ast::FunctionKind::Setter;
                    if !(af.static_ || the_abstract_ || setter) && forbidden(n) {
                        output.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION, the_name.span, [n])));
                    }
                    if !af.static_ && !e_enum && n == "values" {
                        output.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_DECLARATION, the_name.span, [] as [&str; 0])));
                    }
                }
                _ => {}
            }
        }
        // Herdados (`getInheritedConcreteMap2`; mixins não herdam).
        if cls.kind != K::Mixin {
            for n in ["hashCode", "==", "index"] {
                let Some(key) = interner.lookup(n) else { continue };
                let mut inherited = None;
                for &m in cls.mixin_classes.iter().rev() {
                    if cx.outside_hierarchy(cid, m) {
                        continue;
                    }
                    if let Some(a) = cx.implementado(m, key, 1)
                        && !cx.of_object(a.dono)
                    {
                        inherited = Some(a);
                        break;
                    }
                }
                if inherited.is_none()
                    && let Some(s) = cx.superclass(cid)
                {
                    inherited = cx.implementado(s, key, 1);
                }
                let Some(a) = inherited else { continue };
                let owner = program.class(a.dono);
                let count = if owner.kind != K::Class && owner.kind != K::MixinApplication {
                    true
                } else if n == "index" {
                    a.dono != enum_core
                } else {
                    !cx.of_object(a.dono)
                };
                if count {
                    output.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE, class_name.span, [n, interner.resolve(owner.name)]),
                    ));
                }
            }
        }
        let Some(this_one) = cx.own_type(cid) else { continue };
        let values = interner.lookup("values");
        let values_set = interner.lookup("values_=");
        let inherited = values.and_then(|k| cx.herdado(this_one, k)).or_else(|| values_set.and_then(|k| cx.herdado(this_one, k)));
        if let Some((a, _)) = inherited {
            output.push((
                decl.unit,
                Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_INHERITANCE, class_name.span, [interner.resolve(program.class(a.dono).name)]),
            ));
        }
    }
    output
}
