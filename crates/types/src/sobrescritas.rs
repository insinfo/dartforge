//! Sobrescritas inválidas (`invalid_override`), como o
//! `_ClassVerifier._checkDeclaredMember` do `InheritanceOverrideVerifier`
//! (analyzer 6.11.0, `src/error/inheritance_override.dart`) e o
//! `CorrectOverrideHelper` (`src/error/correct_override.dart`): cada membro
//! de instância declarado na classe é comparado, por subtipagem do tipo de
//! função, com o membro homônimo da interface de cada superinterface direta
//! (superclasse, restrições `on`, mixins e interfaces, na ordem do
//! analyzer). Parâmetros covariantes valem `Object?`.
//!
//! Onde a resposta dependeria do que aqui não se calcula, nada se relata:
//!
//! * a interface de uma superinterface que herda o nome de mais de um
//!   supertipo com membros diferentes (assinatura combinada do
//!   `InheritanceManager3`);
//! * membro sem todos os tipos escritos (dele ou do sobrescrito): a
//!   inferência de sobrescrita do analyzer daria o tipo herdado;
//! * a covariância herdada, quando algum membro homônimo na hierarquia tem
//!   parâmetro `covariant` (o parâmetro vale `Object?` inteiro).
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

    /// Todos os tipos da assinatura estão escritos na fonte?
    fn tipos_escritos(&self, f: FunctionElementId) -> bool {
        let func = self.program.function(f);
        match func.node {
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                let retorno = af.return_type.is_some() || matches!(af.kind, ast::FunctionKind::Setter);
                let params = af
                    .parameters
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .all(|p| p.ty.is_some() && !p.this_ && !p.super_);
                retorno && params
            }
            FunctionRef::None => func.variable.is_some_and(|v| self.outline.variables[v.0 as usize].declared_type.is_some()),
            FunctionRef::Constructor { .. } => false,
        }
    }

    /// O tipo do membro no outline é o do analyzer? Com todos os tipos
    /// escritos, sim; com algum omitido, só quando um único supertipo declara
    /// o nome (a inferência de sobrescrita de `types` segue o primeiro) e o
    /// membro não é genérico (aí `types` não infere).
    fn inferencia_confiavel(&self, f: FunctionElementId) -> bool {
        if self.tipos_escritos(f) {
            return true;
        }
        let func = self.program.function(f);
        let (Some(dono), Some(chave), FunctionRef::Function { unit, function }) = (func.class, self.chave(f), func.node) else {
            return false;
        };
        match self.declarantes(dono, chave) {
            // Nada a herdar: o omitido é `dynamic`, aqui e no analyzer.
            0 => true,
            1 => self.program.unit(unit).ast.functions[function.0 as usize].type_params.is_empty(),
            _ => false,
        }
    }

    /// Os parâmetros declarados `covariant` (posicionais pelo índice,
    /// nomeados pelo nome), inclusive o do setter de um campo `covariant`.
    fn covariantes(&self, f: FunctionElementId) -> (Vec<usize>, Vec<SymbolId>) {
        let func = self.program.function(f);
        let mut pos = Vec::new();
        let mut nomes = Vec::new();
        match func.node {
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                let mut i = 0;
                for p in af.parameters.as_deref().unwrap_or(&[]) {
                    if p.kind == ast::ParameterKind::Named {
                        if p.covariant
                            && let Some(n) = p.nome_externo()
                        {
                            nomes.push(n.sym);
                        }
                    } else {
                        if p.covariant {
                            pos.push(i);
                        }
                        i += 1;
                    }
                }
            }
            FunctionRef::None => {
                if let Some(v) = func.variable
                    && let VariableRef::Field { unit, member, .. } = self.program.variable(v).node
                    && let MemberKind::Field(vl) = &self.program.unit(unit).ast.member(member).kind
                    && vl.covariant
                {
                    pos.push(0);
                }
            }
            FunctionRef::Constructor { .. } => {}
        }
        (pos, nomes)
    }

    /// `InheritanceManager3.getMember(s, nome)`: o membro na interface do
    /// tipo de interface `s`, com o tipo visto por `s`. A classe declara ou
    /// herda; herdando de mais de um supertipo, vale o primeiro candidato
    /// cujo tipo é subtipo de todos os outros (`combineSignatures`), e sem
    /// ele (conflito) não há membro. Os candidatos vêm da superclasse (com os
    /// mixins por cima, cada um substituindo o anterior quando declara o
    /// nome), depois das interfaces e das restrições `on`.
    fn na_interface(&mut self, s: TypeId, chave: SymbolId, prof: u32) -> Option<(Achado, TypeId)> {
        if prof > 32 {
            return None;
        }
        let Type::Interface { class: d, .. } = self.table.get(s).clone() else { return None };
        let classe = self.program.class(d);
        if classe.decl.is_some()
            && let Some(&f) = classe.instance_members.get(&chave)
        {
            let a = Achado { dono: d, funcao: f };
            let t = self.tipo_visto(s, a)?;
            return Some((a, t));
        }
        let visto = |cx: &mut Self, alvo: ClassId| cx.outline.hierarchy.supertype_of(s, alvo, cx.table, cx.core);
        let mut candidatos: Vec<(Achado, TypeId)> = Vec::new();
        // Superclasse, com os mixins por cima.
        let mut lado_super: Option<(Achado, TypeId)> = None;
        if let Some(sc) = classe.supertype_class
            && let Some(st) = visto(self, sc)
        {
            lado_super = self.na_interface(st, chave, prof + 1);
        }
        for &m in &classe.mixin_classes {
            let Some(mt) = visto(self, m) else { return None };
            if let Some(x) = self.na_interface(mt, chave, prof + 1) {
                match lado_super {
                    Some(y) if y.0 != x.0 && !self.program.class(m).instance_members.contains_key(&chave) => return None,
                    _ => lado_super = Some(x),
                }
            }
        }
        candidatos.extend(lado_super);
        for &i in classe.interface_classes.iter().chain(classe.on_classes.iter()) {
            let Some(it) = visto(self, i) else { return None };
            if let Some(x) = self.na_interface(it, chave, prof + 1)
                && !candidatos.iter().any(|c| c.0 == x.0)
            {
                candidatos.push(x);
            }
        }
        match candidatos.len() {
            0 => None,
            1 => Some(candidatos[0]),
            _ => {
                let especie = self.especie(candidatos[0].0.funcao);
                if candidatos.iter().any(|c| self.especie(c.0.funcao) != especie) {
                    return None;
                }
                for i in 0..candidatos.len() {
                    let ok = (0..candidatos.len()).all(|j| {
                        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                        is_subtype(candidatos[i].1, candidatos[j].1, &mut env)
                    });
                    if ok {
                        return Some(candidatos[i]);
                    }
                }
                None
            }
        }
    }

    /// Quantas classes (sem `c`) declaram `chave` na hierarquia de `c`.
    fn declarantes(&self, c: ClassId, chave: SymbolId) -> usize {
        let Some(dados) = self.outline.hierarchy.get(c) else { return usize::MAX };
        dados
            .supertypes
            .keys()
            .filter(|&&s| s != c && self.program.class(s).decl.is_some() && self.program.class(s).instance_members.contains_key(&chave))
            .count()
    }

    /// O tipo do membro `a` visto pelo supertipo `s` (que o herda ou declara).
    fn tipo_visto(&mut self, s: TypeId, a: Achado) -> Option<TypeId> {
        let sig = self.outline.functions.get(a.funcao.0 as usize)?.signature;
        let visto = self.outline.hierarchy.supertype_of(s, a.dono, self.table, self.core)?;
        let args = match self.table.get(visto) {
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

    /// `ParameterElement.isCovariant`: o parâmetro é `covariant` escrito, ou
    /// o correspondente (mesma posição, ou mesmo nome) de algum membro
    /// homônimo nos supertipos de `c` é.
    fn covariantes_efetivos(&self, c: ClassId, f: FunctionElementId, chave: SymbolId) -> (Vec<usize>, Vec<SymbolId>) {
        let (mut pos, mut nomes) = self.covariantes(f);
        if let Some(dados) = self.outline.hierarchy.get(c) {
            for &s in dados.supertypes.keys() {
                if s == c {
                    continue;
                }
                if let Some(&g) = self.program.class(s).instance_members.get(&chave) {
                    let (p, n) = self.covariantes(g);
                    pos.extend(p.into_iter().filter(|x| !pos.contains(x)).collect::<Vec<_>>());
                    nomes.extend(n.into_iter().filter(|x| !nomes.contains(x)).collect::<Vec<_>>());
                }
            }
        }
        (pos, nomes)
    }

    /// `CovariantParametersVerifier`: cada parâmetro covariante contra o
    /// correspondente do membro homônimo declarado em cada supertipo da
    /// classe; tipos sem relação de subtipo em nenhum sentido são erro. A
    /// mensagem mostra o membro do supertipo sem substituição.
    fn covariantes_contra_supertipos(&mut self, m: &Conferencia, chave: SymbolId, saida: &mut Vec<Diagnostic>) {
        let (pos, nomes) = self.covariantes_efetivos(m.declarante, m.funcao, chave);
        if pos.is_empty() && nomes.is_empty() {
            return;
        }
        let Some(proprio) = self.outline.functions.get(m.funcao.0 as usize) else { return };
        let proprios: Vec<(usize, ParameterKind, Option<SymbolId>, TypeId)> =
            proprio.parameters.iter().enumerate().map(|(i, p)| (i, p.kind, p.externo, p.ty)).collect();
        if !proprio.type_params.is_empty() {
            return;
        }
        let Some(dados) = self.outline.hierarchy.get(m.declarante) else { return };
        let mut supers: Vec<(ClassId, TypeId)> =
            dados.supertypes.iter().filter(|(k, _)| **k != m.declarante).map(|(k, v)| (*k, *v)).collect();
        supers.sort_by_key(|(k, _)| k.0);
        let especie = self.especie(m.funcao);
        for (s, st) in supers {
            let Some(&g) = self.program.class(s).instance_members.get(&chave) else { continue };
            if self.especie(g) != especie {
                continue;
            }
            let Some(dados_g) = self.outline.functions.get(g.0 as usize) else { continue };
            if !dados_g.type_params.is_empty() {
                continue;
            }
            let args = match self.table.get(st) {
                Type::Interface { args, .. } => args.clone(),
                _ => continue,
            };
            let Some(params_s) = self.outline.classes.get(s.0 as usize).map(|d| d.type_params.clone()) else { continue };
            if params_s.len() != args.len() {
                continue;
            }
            let subst: HashMap<TypeParamId, TypeId> = params_s.iter().copied().zip(args.iter().copied()).collect();
            let super_params: Vec<(ParameterKind, Option<SymbolId>, TypeId)> =
                dados_g.parameters.iter().map(|p| (p.kind, p.externo, p.ty)).collect();
            let super_sig = dados_g.signature;
            let mut posicional = 0usize;
            for &(i, kind, nome, ty) in &proprios {
                let covariante = if kind == ParameterKind::Named {
                    nome.is_some_and(|n| nomes.contains(&n))
                } else {
                    pos.contains(&posicional)
                };
                let indice_pos = posicional;
                if kind != ParameterKind::Named {
                    posicional += 1;
                }
                let _ = i;
                if !covariante {
                    continue;
                }
                let correspondente = if kind == ParameterKind::Named {
                    super_params.iter().find(|p| p.0 == ParameterKind::Named && p.1 == nome).map(|p| p.2)
                } else {
                    super_params.iter().filter(|p| p.0 != ParameterKind::Named).nth(indice_pos).map(|p| p.2)
                };
                let Some(sup_ty) = correspondente else { continue };
                let sup_ty = substitute(sup_ty, &subst, self.table);
                let relacionados = {
                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                    is_subtype(sup_ty, ty, &mut env) || is_subtype(ty, sup_ty, &mut env)
                };
                if !relacionados {
                    let mut nome_m = self.interner.resolve(self.program.function(m.funcao).name).to_string();
                    if especie == Some(Especie::Setter) {
                        nome_m.push('=');
                    }
                    let args = [
                        nome_m,
                        self.interner.resolve(self.program.class(m.declarante).name).to_string(),
                        formatar(self.table, m.tipo, self.interner, self.program),
                        self.interner.resolve(self.program.class(s).name).to_string(),
                        formatar(self.table, super_sig, self.interner, self.program),
                    ];
                    saida.push(Diagnostic::com_codigo(c::INVALID_OVERRIDE, m.span, args));
                }
            }
        }
    }

    /// `_computeThisTypeForSubtype`: parâmetros covariantes viram `Object?`.
    fn para_subtipo(&mut self, sig: TypeId, pos: &[usize], nomes: &[SymbolId]) -> TypeId {
        if pos.is_empty() && nomes.is_empty() {
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
            named.iter().map(|&(n, t, r)| (n, if nomes.contains(&n) { obj } else { t }, r)).collect();
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
    fn conferir(&mut self, m: Conferencia, supers: &[TypeId], saida: &mut Vec<Diagnostic>) {
        let (Some(especie), Some(chave)) = (self.especie(m.funcao), self.chave(m.funcao)) else { return };
        // Tipo omitido: o do analyzer vem da inferência de sobrescrita, igual
        // à de `types` quando só um supertipo declara o nome.
        if !self.inferencia_confiavel(m.funcao) {
            return;
        }
        let nome = self.interner.resolve(self.program.function(m.funcao).name).to_string();
        let biblioteca = self.program.class(m.declarante).library;
        let (pos, nomes) = self.covariantes_efetivos(m.declarante, m.funcao, chave);
        let para_sub = self.para_subtipo(m.tipo, &pos, &nomes);
        for &s in supers {
            let Some((achado, do_super)) = self.na_interface(s, chave, 0) else { continue };
            if nome.starts_with('_') && self.program.class(achado.dono).library != biblioteca {
                continue;
            }
            if self.especie(achado.funcao) != Some(especie) || !self.inferencia_confiavel(achado.funcao) {
                continue;
            }
            let ok = {
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                is_subtype(para_sub, do_super, &mut env)
            };
            if !ok {
                let codigo = if especie == Especie::Setter { c::INVALID_OVERRIDE_SETTER } else { c::INVALID_OVERRIDE };
                let args = [
                    nome.clone(),
                    self.interner.resolve(self.program.class(m.declarante).name).to_string(),
                    formatar(self.table, m.tipo, self.interner, self.program),
                    self.interner.resolve(self.program.class(achado.dono).name).to_string(),
                    formatar(self.table, do_super, self.interner, self.program),
                ];
                saida.push(Diagnostic::com_codigo(codigo, m.span, args));
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
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    // Acessores e métodos por nó da fonte.
    let mut por_funcao: HashMap<(UnitId, u32), FunctionElementId> = HashMap::new();
    let mut por_variavel: HashMap<(UnitId, u32, usize), (Option<FunctionElementId>, Option<FunctionElementId>)> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.class.is_none() || f.static_ {
            continue;
        }
        let id = FunctionElementId(i as u32);
        match f.node {
            FunctionRef::Function { unit, function } => {
                por_funcao.insert((unit, function.0), id);
            }
            FunctionRef::None => {
                if let Some(v) = f.variable {
                    let var = program.variable(v);
                    if let VariableRef::Field { unit, member, index } = var.node {
                        por_variavel.insert((unit, member.0, index), (var.getter, var.setter));
                    }
                }
            }
            FunctionRef::Constructor { .. } => {}
        }
    }
    // Os membros de instância declarados numa classe: `(função, nome no erro)`.
    let declarados = |cid: ClassId| -> Vec<(FunctionElementId, Span)> {
        let mut v = Vec::new();
        let Some(decl) = program.class(cid).decl else { return v };
        let ast_ = &program.unit(decl.unit).ast;
        let membros: &[ast::MemberId] = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            DeclKind::Enum(d) => &d.members,
            _ => return v,
        };
        for &m in membros {
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if let (Some(&id), Some(n)) = (por_funcao.get(&(decl.unit, f.0)), af.name) {
                        v.push((id, n.span));
                    }
                }
                MemberKind::Field(vl) if !vl.static_ => {
                    for (i, var) in vl.variables.iter().enumerate() {
                        if let Some(&(g, s)) = por_variavel.get(&(decl.unit, m.0, i)) {
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
        let classe = program.class(cid);
        let Some(decl) = classe.decl else { continue };
        let Some(dados) = outline.classes.get(cid.0 as usize) else { continue };
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
        match (dados.supertype, classe.kind) {
            (Some(t), _) => supers.push(t),
            (None, dartforge_elements::model::ClassKind::Class) => supers.push(core.object),
            _ => {}
        }
        supers.extend(dados.on.iter().copied());
        let mut diags = Vec::new();
        for (i, &mt) in dados.mixins.iter().enumerate() {
            if let Type::Interface { class: m, .. } = cx.table.get(mt).clone()
                && with.len() == dados.mixins.len()
            {
                let span = ast_.ty(with[i]).span;
                for (f, _) in declarados(m) {
                    // O tipo do membro do mixin aplicado (`M<args>`).
                    let Some(tipo) = cx.tipo_visto(mt, Achado { dono: m, funcao: f }) else { continue };
                    cx.conferir(Conferencia { funcao: f, tipo, declarante: m, span }, &supers.clone(), &mut diags);
                }
            }
            supers.push(mt);
        }
        supers.extend(dados.interfaces.iter().copied());
        for (f, span) in declarados(cid) {
            let Some(tipo) = outline.functions.get(f.0 as usize).map(|d| d.signature) else { continue };
            let conf = Conferencia { funcao: f, tipo, declarante: cid, span };
            cx.conferir(Conferencia { ..conf }, &supers, &mut diags);
            if let Some(chave) = cx.chave(f)
                && cx.inferencia_confiavel(f)
            {
                cx.covariantes_contra_supertipos(&conf, chave, &mut diags);
            }
        }
        saida.extend(diags.into_iter().map(|d| (decl.unit, d)));
    }
    saida
}

/// Onde está o nome da função `f` (o do campo ou variável, num acessor
/// implícito).
fn nome_de(program: &Program, f: FunctionElementId) -> Option<(UnitId, Span)> {
    let func = program.function(f);
    match func.node {
        FunctionRef::Function { unit, function } => {
            Some((unit, program.unit(unit).ast.functions[function.0 as usize].name?.span))
        }
        FunctionRef::None => match program.variable(func.variable?).node {
            VariableRef::Field { unit, member, index } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::TopLevel { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::Variables(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            _ => None,
        },
        FunctionRef::Constructor { .. } => None,
    }
}

impl Ctx<'_> {
    /// Os tipos de getter e setter de um acessor fora de interface (topo,
    /// estático, extensão): o escrito, `dynamic` quando omitido numa função,
    /// nada num campo ou variável sem tipo escrito (o tipo viria do
    /// inicializador).
    fn tipo_de_acessor(&mut self, f: FunctionElementId, getter: bool) -> Option<TypeId> {
        let func = self.program.function(f);
        let dados = self.outline.functions.get(f.0 as usize)?;
        match func.node {
            FunctionRef::Function { .. } => {
                if getter {
                    Some(dados.return_type)
                } else {
                    dados.parameters.first().map(|p| p.ty)
                }
            }
            FunctionRef::None => self.outline.variables.get(func.variable?.0 as usize)?.declared_type,
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
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    let saida_sem_augment = Vec::new;
    // Acessores locais: `(recipiente, nome)` → getters e setters.
    #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
    enum Recipiente {
        Unidade(UnitId),
        Classe(ClassId),
        Extensao(u32),
    }
    let mut grupos: HashMap<(Recipiente, SymbolId), (Vec<FunctionElementId>, Vec<FunctionElementId>)> = HashMap::new();
    // Acessores de `augment` (e o que eles aumentam) ficam de fora: a
    // recuperação do parser sem o experimento os lê de outro jeito.
    let mut aumentados: std::collections::HashSet<(UnitId, u32)> = std::collections::HashSet::new();
    for &u in &program.library(lib).units {
        let ast_ = &program.unit(u).ast;
        for m in &ast_.members {
            if m.augment {
                match &m.kind {
                    MemberKind::Method(f) => {
                        aumentados.insert((u, f.0));
                    }
                    MemberKind::Field(_) => return saida_sem_augment(),
                    MemberKind::Constructor(_) => {}
                }
            }
        }
        for d in &ast_.decls {
            if d.augment
                && let DeclKind::Function(f) = &d.kind
            {
                aumentados.insert((u, f.0));
            }
        }
    }
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        if let FunctionRef::Function { unit, function } = f.node
            && aumentados.contains(&(unit, function.0))
        {
            continue;
        }
        let id = FunctionElementId(i as u32);
        let Some(especie) = cx.especie(id) else { continue };
        if especie == Especie::Metodo {
            continue;
        }
        let recipiente = match (f.class, f.extension) {
            (_, Some(e)) => Recipiente::Extensao(e.0),
            (Some(c), None) if f.static_ => match program.class(c).kind {
                dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::Enum => Recipiente::Classe(c),
                _ => continue,
            },
            (Some(_), None) => continue,
            (None, None) => match nome_de(program, id) {
                Some((u, _)) => Recipiente::Unidade(u),
                None => continue,
            },
        };
        let g = grupos.entry((recipiente, f.name)).or_default();
        if especie == Especie::Getter { g.0.push(id) } else { g.1.push(id) }
    }
    let mut chaves: Vec<_> = grupos.keys().copied().collect();
    chaves.sort_by_key(|(r, n)| (interner.resolve(*n).to_string(), format!("{r:?}")));
    for k in chaves {
        let (gs, ss) = &grupos[&k];
        let ([g], [s]) = (gs.as_slice(), ss.as_slice()) else { continue };
        let (Some(tg), Some(ts)) = (cx.tipo_de_acessor(*g, true), cx.tipo_de_acessor(*s, false)) else { continue };
        let ok = {
            let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
            is_subtype(tg, ts, &mut env)
        };
        if !ok && let Some((u, span)) = nome_de(program, *g) {
            let nome = interner.resolve(k.1).to_string();
            let args = [nome.clone(), formatar(cx.table, tg, interner, program), formatar(cx.table, ts, interner, program), nome];
            saida.push((u, Diagnostic::com_codigo(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args)));
        }
    }
    // A interface de cada classe.
    for &cid in classes {
        let classe = program.class(cid);
        let Some(decl) = classe.decl else { continue };
        let Some(dados_h) = outline.hierarchy.get(cid) else { continue };
        let Some(params) = outline.classes.get(cid.0 as usize).map(|d| d.type_params.clone()) else { continue };
        let args: Box<[TypeId]> = params.iter().map(|&p| cx.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let este = cx.table.intern(Type::Interface { class: cid, args, nullable: false });
        let mut nomes: Vec<SymbolId> = Vec::new();
        for s in std::iter::once(cid).chain(dados_h.supertypes.keys().copied()) {
            for (&chave, &f) in &program.class(s).instance_members {
                if cx.especie(f) == Some(Especie::Getter) && !nomes.contains(&chave) {
                    nomes.push(chave);
                }
            }
        }
        nomes.sort_by_key(|n| interner.resolve(*n).to_string());
        for nome in nomes {
            let texto = interner.resolve(nome).to_string();
            let Some(chave_setter) = interner.lookup(&format!("{texto}_=")) else { continue };
            let Some((g, tg)) = cx.na_interface(este, nome, 0) else { continue };
            let Some((s, ts)) = cx.na_interface(este, chave_setter, 0) else { continue };
            if texto.starts_with('_')
                && (program.class(g.dono).library != classe.library || program.class(s.dono).library != classe.library)
            {
                continue;
            }
            if cx.especie(g.funcao) != Some(Especie::Getter) || cx.especie(s.funcao) != Some(Especie::Setter) {
                continue;
            }
            if !cx.inferencia_confiavel(g.funcao) || !cx.inferencia_confiavel(s.funcao) {
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
            let lugar = if g.dono == cid {
                nome_de(program, g.funcao)
            } else if s.dono == cid {
                nome_de(program, s.funcao)
            } else {
                let ast_ = &program.unit(decl.unit).ast;
                match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Enum(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Mixin(d) => Some((decl.unit, d.name.span)),
                    _ => None,
                }
            };
            let Some((u, span)) = lugar else { continue };
            let qualificado = |dono: ClassId| {
                if dono == cid { texto.clone() } else { format!("{}.{texto}", interner.resolve(program.class(dono).name)) }
            };
            let args = [qualificado(g.dono), formatar(cx.table, ret, interner, program), formatar(cx.table, param, interner, program), qualificado(s.dono)];
            saida.push((u, Diagnostic::com_codigo(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args)));
        }
    }
    saida
}
