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
        self.na_interface_ex(s, chave, prof, false)
    }

    /// `InheritanceManager3.getInherited2`: como [`Self::na_interface`], sem
    /// o que a própria classe declara.
    fn herdado(&mut self, s: TypeId, chave: SymbolId) -> Option<(Achado, TypeId)> {
        self.na_interface_ex(s, chave, 0, true)
    }

    fn na_interface_ex(&mut self, s: TypeId, chave: SymbolId, prof: u32, pular_proprio: bool) -> Option<(Achado, TypeId)> {
        if prof > 32 {
            return None;
        }
        let Type::Interface { class: d, .. } = self.table.get(s).clone() else { return None };
        let classe = self.program.class(d);
        if classe.decl.is_some()
            && !pular_proprio
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
        if let Some(sc) = self.superclasse(d)
            && let Some(st) = visto(self, sc)
        {
            lado_super = self.na_interface(st, chave, prof + 1);
        }
        for &m in &classe.mixin_classes {
            if self.fora_da_hierarquia(d, m) {
                continue;
            }
            let mt = visto(self, m)?;
            if let Some(x) = self.na_interface(mt, chave, prof + 1) {
                match lado_super {
                    Some(y) if y.0 != x.0 && !self.program.class(m).instance_members.contains_key(&chave) => return None,
                    _ => lado_super = Some(x),
                }
            }
        }
        candidatos.extend(lado_super);
        for &i in classe.interface_classes.iter().chain(classe.on_classes.iter()) {
            // Enum e tipo de extensão não entram nas interfaces de uma classe
            // (`implements_non_class`).
            if self.fora_da_hierarquia(d, i) {
                continue;
            }
            let it = visto(self, i)?;
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
    type Acessores = (Option<FunctionElementId>, Option<FunctionElementId>);
    let mut por_variavel: HashMap<(UnitId, u32, usize), Acessores> = HashMap::new();
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
            (None, dartforge_elements::model::ClassKind::Enum) => {
                if let (Some(e), Some(este)) = (cx.classe_do_core("Enum"), cx.tipo_proprio(cid))
                    && let Some(t) = outline.hierarchy.supertype_of(este, e, cx.table, core)
                {
                    supers.push(t);
                }
            }
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
        supers.retain(|&t| match cx.table.get(t) {
            Type::Interface { class, .. } => !cx.fora_da_hierarquia(cid, *class),
            _ => true,
        });
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
            VariableRef::EnumConstant { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::Enum(d) => Some((unit, d.constants.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::Representation { unit, decl } => match &program.unit(unit).ast.decl(decl).kind {
                DeclKind::ExtensionType(d) => Some((unit, d.representation_name.span)),
                _ => None,
            },
            VariableRef::None => None,
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
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                let (escrito, t) = if getter {
                    (af.return_type, dados.return_type)
                } else {
                    (af.parameters.as_deref().and_then(|p| p.first()).and_then(|p| p.ty), dados.parameters.first()?.ty)
                };
                // Tipo escrito que não resolve (`InvalidType` no analyzer, que
                // é subtipo de tudo e aceita tudo): não se decide.
                if let Some(e) = escrito
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
                let dados_v = self.outline.variables.get(v.0 as usize)?;
                match self.program.variable(v).node {
                    // Constante de enum e representação: o tipo é conhecido.
                    VariableRef::EnumConstant { .. } | VariableRef::Representation { .. } => {
                        dados_v.declared_type.or(dados_v.inferred)
                    }
                    _ => dados_v.declared_type,
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
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    // Acessores locais: `(recipiente, nome)` → getters e setters.
    #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
    enum Recipiente {
        Unidade(UnitId),
        Classe(ClassId),
        Instancia(ClassId),
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
                dartforge_elements::model::ClassKind::Class
                | dartforge_elements::model::ClassKind::Enum
                | dartforge_elements::model::ClassKind::ExtensionType => Recipiente::Classe(c),
                _ => continue,
            },
            // Membros de instância de tipo de extensão: os declarados nele
            // (`checkExtensionType`, a parte da interface que ele mesmo
            // declara), com a representação.
            (Some(c), None) if program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType => {
                Recipiente::Instancia(c)
            }
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
        // Num tipo de extensão, o getter da representação cede o lugar ao setter.
        let representacao = matches!(k.0, Recipiente::Instancia(_))
            && program.function(*g).variable.is_some_and(|v| matches!(program.variable(v).node, VariableRef::Representation { .. }));
        let lugar = if representacao { nome_de(program, *s) } else { nome_de(program, *g) };
        if !ok && let Some((u, span)) = lugar {
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

/// Começo de um nó anotado (`AnnotatedNodeImpl.beginToken`): o comentário
/// de documentação logo antes, se houver.
fn inicio_com_documentacao(fonte: &str, inicio: usize) -> usize {
    let sem_espaco = fonte[..inicio].trim_end();
    if let Some(sem_fim) = sem_espaco.strip_suffix("*/") {
        if let Some(i) = sem_fim.rfind("/**")
            && !sem_fim[i..].contains("*/")
        {
            return i;
        }
        return inicio;
    }
    let mut resultado = inicio;
    let mut fim = sem_espaco.len();
    loop {
        let linha_ini = sem_espaco[..fim].rfind('\n').map_or(0, |i| i + 1);
        let linha = sem_espaco[linha_ini..fim].trim_start();
        if !(linha.starts_with("///") && !linha.starts_with("////")) {
            break;
        }
        resultado = fim - linha.len();
        if linha_ini == 0 {
            break;
        }
        fim = sem_espaco[..linha_ini].trim_end().len();
    }
    resultado
}

impl Ctx<'_> {
    /// O membro é concreto (`!isAbstract`): tem corpo, é `external`, ou é
    /// acessor de campo que não é `abstract`.
    fn concreto(&self, f: FunctionElementId) -> bool {
        let func = self.program.function(f);
        // O `index` do `Enum` conta como implementado (`ElementBuilder`).
        if let Some(c) = func.class
            && self.interner.resolve(func.name) == "index"
            && Some(c) == self.classe_do_core("Enum")
        {
            return true;
        }
        match func.node {
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                af.external || !matches!(af.body, ast::FunctionBody::Empty)
            }
            FunctionRef::None => match func.variable.map(|v| self.program.variable(v).node) {
                Some(VariableRef::Field { unit, member, .. }) => match &self.program.unit(unit).ast.member(member).kind {
                    MemberKind::Field(vl) => !vl.abstract_,
                    _ => true,
                },
                _ => true,
            },
            FunctionRef::Constructor { .. } => false,
        }
    }

    /// `Interface.implemented[nome]`: o membro concreto da classe `d` (os
    /// dela, por cima os dos mixins do último ao primeiro, por baixo os da
    /// superclasse). Os de `Object` que vêm por mixin não contam.
    fn implementado(&self, d: ClassId, chave: SymbolId, prof: u32) -> Option<Achado> {
        if prof > 32 {
            return None;
        }
        let classe = self.program.class(d);
        if classe.decl.is_some()
            && let Some(&f) = classe.instance_members.get(&chave)
            && self.concreto(f)
        {
            return Some(Achado { dono: d, funcao: f });
        }
        // `_addMixinMembers`: o `implemented` do mixin inteiro, menos o que
        // vem de `Object`.
        for &m in classe.mixin_classes.iter().rev() {
            if self.fora_da_hierarquia(d, m) {
                continue;
            }
            if let Some(a) = self.implementado(m, chave, prof + 1)
                && !self.de_object(a.dono)
            {
                return Some(a);
            }
        }
        self.implementado(self.superclasse(d)?, chave, prof + 1)
    }

    /// `InterfaceElement.supertype`: a do modelo, e `Enum` num enum
    /// (`LibraryBuilder.buildEnumChildren`).
    fn superclasse(&self, d: ClassId) -> Option<ClassId> {
        let classe = self.program.class(d);
        match classe.supertype_class {
            // `extends` de enum ou tipo de extensão (`extends_non_class`): o
            // supertipo fica `Object`.
            Some(s) if self.fora_da_hierarquia(d, s) => self.classe_do_core("Object"),
            Some(s) => Some(s),
            None if classe.kind == dartforge_elements::model::ClassKind::Enum => self.classe_do_core("Enum"),
            None => None,
        }
    }

    /// O supertipo `s` de `d` é enum ou tipo de extensão numa classe, mixin
    /// ou enum (o analyzer o descarta da cláusula).
    fn fora_da_hierarquia(&self, d: ClassId, s: ClassId) -> bool {
        use dartforge_elements::model::ClassKind as K;
        self.program.class(d).kind != K::ExtensionType && matches!(self.program.class(s).kind, K::Enum | K::ExtensionType)
    }

    fn de_object(&self, c: ClassId) -> bool {
        let classe = self.program.class(c);
        self.interner.resolve(classe.name) == "Object" && self.program.library(classe.library).uri == "dart:core"
    }

    fn classe_do_core(&self, nome: &str) -> Option<ClassId> {
        let lib = self.program.core?;
        let sym = self.interner.lookup(nome)?;
        match self.program.library(lib).declared.get(&sym)?.getter? {
            dartforge_elements::model::Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// Todas as chaves de membros de instância de `c` e dos supertipos.
    fn chaves_da_hierarquia(&self, c: ClassId) -> Vec<SymbolId> {
        let mut v: Vec<SymbolId> = Vec::new();
        let supers = self.outline.hierarchy.get(c).map(|d| d.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default();
        for s in std::iter::once(c).chain(supers) {
            if s != c && self.fora_da_hierarquia(c, s) {
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

    fn tipo_proprio(&mut self, c: ClassId) -> Option<TypeId> {
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
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    let nsm = interner.lookup("noSuchMethod");
    for &cid in classes {
        let classe = program.class(cid);
        let Some(decl) = classe.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let fonte = &program.unit(decl.unit).source;
        let (nome_classe, membros): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) if !d.modifiers.abstract_ && !d.modifiers.sealed => (d.name, &d.members),
            DeclKind::Enum(d) => (d.name, &d.members),
            _ => continue,
        };
        let e_enum = classe.kind == dartforge_elements::model::ClassKind::Enum;
        // Membro de `augment`, ou método cujo nome vem depois de `.` (um
        // construtor com o nome da classe errado): a recuperação do parser
        // difere da do analyzer e a classe não se decide.
        let recuperado = membros.iter().any(|&m| {
            let membro = ast_.member(m);
            membro.augment
                || matches!(&membro.kind, MemberKind::Method(f)
                    if ast_.function(*f).name.is_some_and(|n| fonte[..n.span.start].trim_end().ends_with('.')))
        });
        // A palavra `augment` no começo de um membro, sem o experimento, é
        // lida de outro jeito pelo parser do analyzer.
        let sp = ast_.decl(decl.decl).span;
        let texto_decl = fonte.get(sp.start..sp.end).unwrap_or("");
        let augment_no_texto = texto_decl.lines().any(|l| l.trim_start().starts_with("augment "));
        if recuperado || augment_no_texto || ast_.decl(decl.decl).augment {
            continue;
        }
        let Some(este) = cx.tipo_proprio(cid) else { continue };
        // `noSuchMethod` implementado que não é o de `Object`: encaminha.
        if let Some(n) = nsm
            && let Some(a) = cx.implementado(cid, n, 0)
            && program.library(program.class(a.dono).library).uri != "dart:core"
        {
            continue;
        }
        // `_isNotImplementedInConcreteSuperClass`: a superclasse declarada,
        // se concreta.
        let superclasse_concreta = classe.supertype_class.filter(|&s| {
            let sc = program.class(s);
            sc.decl.is_some()
                && sc.kind == dartforge_elements::model::ClassKind::Class
                && !sc.modifiers.abstract_
                && !sc.modifiers.sealed
        });
        let mut herdados: Vec<String> = Vec::new();
        let mut incerto = false;
        for chave in cx.chaves_da_hierarquia(cid) {
            let texto = interner.resolve(chave).to_string();
            let Some((membro, tipo_membro)) = cx.na_interface(este, chave, 0) else {
                // Conflito ou assinatura combinada que não se decide aqui.
                if !program.class(cid).instance_members.contains_key(&chave) {
                    incerto = true;
                }
                continue;
            };
            if texto.starts_with('_') && program.class(membro.dono).library != classe.library {
                continue;
            }
            let Some(especie) = cx.especie(membro.funcao) else { continue };
            let exibido = texto.strip_suffix("_=").unwrap_or(&texto).to_string();
            match cx.implementado(cid, chave, 0) {
                None => {
                    // `_reportConcreteClassWithAbstractMember`.
                    let declarado = membros.iter().find(|&&m| match &ast_.member(m).kind {
                        MemberKind::Method(f) => {
                            let af = ast_.function(*f);
                            af.name.is_some_and(|n| {
                                let base = interner.resolve(n.sym);
                                if af.kind == ast::FunctionKind::Setter { format!("{base}_=") == texto } else { base == texto && af.kind != ast::FunctionKind::Setter }
                            }) && !af.static_
                        }
                        MemberKind::Field(vl) => {
                            !vl.static_
                                && vl.variables.iter().any(|v| {
                                    let n = interner.resolve(v.name.sym);
                                    n == texto || (!vl.final_ && !vl.const_ && format!("{n}_=") == texto)
                                })
                        }
                        MemberKind::Constructor(_) => false,
                    });
                    if let Some(&m) = declarado {
                        let mut sp = ast_.member(m).span;
                        // `var get a;`: o `var` (erro de sintaxe) fica fora do nó.
                        if let MemberKind::Method(_) = &ast_.member(m).kind
                            && let Some(resto) = fonte[sp.start..sp.end].strip_prefix("var")
                            && resto.starts_with(char::is_whitespace)
                        {
                            sp.start = sp.end - resto.trim_start().len();
                        }
                        let span = Span { start: inicio_com_documentacao(fonte, sp.start), end: sp.end };
                        let codigo = if e_enum { c::ENUM_WITH_ABSTRACT_MEMBER } else { c::CONCRETE_CLASS_WITH_ABSTRACT_MEMBER };
                        saida.push((decl.unit, Diagnostic::com_codigo(codigo, span, [exibido.as_str(), interner.resolve(classe.name)])));
                        continue;
                    }
                    if let Some(s) = superclasse_concreta
                        && let Some(st) = outline.hierarchy.supertype_of(este, s, cx.table, core)
                        && cx.na_interface(st, chave, 0).is_some()
                    {
                        continue;
                    }
                    if e_enum && (texto == "values" || texto == "values_=") {
                        continue;
                    }
                    let prefixo = match especie {
                        Especie::Getter => "getter ",
                        Especie::Setter => "setter ",
                        Especie::Metodo => "",
                    };
                    herdados.push(format!("{prefixo}{}.{exibido}", interner.resolve(program.class(membro.dono).name)));
                }
                Some(concreto) => {
                    // A implementação concreta contra a assinatura da interface.
                    if concreto == membro || cx.especie(concreto.funcao) != Some(especie) {
                        continue;
                    }
                    if !cx.inferencia_confiavel(concreto.funcao) || !cx.inferencia_confiavel(membro.funcao) {
                        continue;
                    }
                    let Some(tipo_concreto) = cx.tipo_visto(este, concreto) else { continue };
                    // `_inheritCovariance`: a covariância vem de todos os
                    // membros homônimos na hierarquia da classe.
                    let (pos, nomes) = cx.covariantes_efetivos(cid, concreto.funcao, chave);
                    let para_sub = cx.para_subtipo(tipo_concreto, &pos, &nomes);
                    let ok = {
                        let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
                        is_subtype(para_sub, tipo_membro, &mut env)
                    };
                    if !ok {
                        let codigo = if especie == Especie::Setter {
                            c::INVALID_IMPLEMENTATION_OVERRIDE_SETTER
                        } else {
                            c::INVALID_IMPLEMENTATION_OVERRIDE
                        };
                        let args = [
                            exibido.clone(),
                            interner.resolve(program.class(concreto.dono).name).to_string(),
                            formatar(cx.table, tipo_concreto, interner, program),
                            interner.resolve(program.class(membro.dono).name).to_string(),
                            formatar(cx.table, tipo_membro, interner, program),
                        ];
                        saida.push((decl.unit, Diagnostic::com_codigo(codigo, nome_classe.span, args)));
                    }
                }
            }
        }
        if incerto || herdados.is_empty() {
            continue;
        }
        herdados.sort();
        let codigo = match herdados.len() {
            1 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE,
            2 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO,
            3 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE,
            4 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR,
            _ => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS,
        };
        let mut args: Vec<String> = herdados.iter().take(4).cloned().collect();
        if herdados.len() > 4 {
            args.push((herdados.len() - 4).to_string());
        }
        saida.push((decl.unit, Diagnostic::com_codigo(codigo, nome_classe.span, args)));
    }
    saida
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
    let mut saida = Vec::new();
    for (i, v) in program.variables.iter().enumerate() {
        if v.library != lib {
            continue;
        }
        let (unit, lista, topo) = match v.node {
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
        let (l, index) = lista;
        if l.const_ || (topo && l.final_) || l.late || l.external || l.ty.is_none() {
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
        let nome = var.name;
        if nome.span.start == nome.span.end {
            continue;
        }
        saida.push((unit, Diagnostic::com_codigo(c::NOT_INITIALIZED_NON_NULLABLE_VARIABLE, nome.span, [interner.resolve(nome.sym)])));
    }
    saida
}

/// `ErrorVerifier._checkUseOfDefaultValuesInParameters`: parâmetro opcional
/// sem valor padrão cujo tipo pode não aceitar `null`
/// (`missing_default_value_for_parameter`, `_POSITIONAL`,
/// `_WITH_ANNOTATION` com `@required`), nas funções de topo, métodos e
/// construtores da biblioteca em que o valor padrão é esperado (nem
/// abstrato, nem `external`, nem nativo, nem `factory` redirecionador). O
/// tipo vale quando escrito ou num `this.x`; sem tipo escrito (inferido da
/// sobrescrita ou do construtor da superclasse), não se decide.
pub fn valores_padrao(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        let Some(dados) = outline.functions.get(i) else { continue };
        let (unit, params, esperado): (UnitId, &[ast::Parameter], bool) = match f.node {
            FunctionRef::Function { unit, function } => {
                let af = &program.unit(unit).ast.functions[function.0 as usize];
                let nativo = matches!(af.body, ast::FunctionBody::Native(_));
                let abstrato = f.class.is_some() && matches!(af.body, ast::FunctionBody::Empty);
                (unit, af.parameters.as_deref().unwrap_or(&[]), !af.external && !nativo && !abstrato)
            }
            FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (unit, &k.parameters[..], !k.external && !(k.factory && k.redirect.is_some())),
                _ => continue,
            },
            FunctionRef::None => continue,
        };
        if !esperado || params.len() != dados.parameters.len() {
            continue;
        }
        // A linha da própria declaração, ou a da classe que a contém, começa
        // com `augment`: sem o experimento, o analyzer a lê de outro jeito.
        let fonte = &program.unit(unit).source;
        let linha_comeca_com_augment = |pos: usize| {
            let ini = fonte[..pos.min(fonte.len())].rfind('\n').map_or(0, |i| i + 1);
            fonte[ini..].trim_start().starts_with("augment ")
        };
        let inicio = match f.node {
            FunctionRef::Function { function, .. } => program.unit(unit).ast.functions[function.0 as usize].span.start,
            FunctionRef::Constructor { member, .. } => program.unit(unit).ast.member(member).span.start,
            FunctionRef::None => 0,
        };
        let da_classe = f.class.and_then(|c| program.class(c).decl).is_some_and(|d| {
            d.unit == unit && linha_comeca_com_augment(program.unit(unit).ast.decl(d.decl).span.start)
        });
        let aumentada = linha_comeca_com_augment(inicio) || da_classe;
        if aumentada {
            continue;
        }
        for (p, pd) in params.iter().zip(dados.parameters.iter()) {
            if p.kind == ast::ParameterKind::Required || p.required || p.default_value.is_some() || p.super_ {
                continue;
            }
            if p.ty.is_none() && !p.this_ {
                continue;
            }
            // Tipo escrito que não resolve (`dynamic` no outline): nada.
            if anulavel(table, pd.ty) {
                continue;
            }
            let Some(nome) = p.name else { continue };
            let anotado = p.metadata.iter().any(|a| a.name.last().is_some_and(|n| interner.resolve(n.sym) == "required"));
            let d = if anotado {
                Diagnostic::com_codigo(c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION, nome.span, [] as [&str; 0])
            } else {
                let codigo = if p.kind == ast::ParameterKind::Optional {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL
                } else {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER
                };
                Diagnostic::com_codigo(codigo, nome.span, [interner.resolve(nome.sym)])
            };
            saida.push((unit, d));
        }
    }
    saida
}

/// `ErrorVerifier._checkForConflictingClassMembers` (classes, mixins e tipos
/// de extensão da biblioteca), menos o `conflicting_static_and_instance`, que
/// `analise::heranca` relata: método declarado contra campo herdado
/// (`conflicting_method_and_field`), acessor declarado contra método herdado
/// (`conflicting_field_and_method`), e método e setter herdados com o mesmo
/// nome (`conflicting_inherited_method_and_setter`), pelo `getInherited2`.
pub fn membros_em_conflito(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    for (ci, classe) in program.classes.iter().enumerate() {
        let cid = ClassId(ci as u32);
        if classe.library != lib || !matches!(classe.kind, K::Class | K::Mixin | K::ExtensionType) {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        if ast_.decl(decl.decl).augment {
            continue;
        }
        let fonte = &program.unit(decl.unit).source;
        if fonte.lines().any(|l| l.trim_start().starts_with("augment ")) {
            continue;
        }
        if program.classes.iter().filter(|c| c.library == lib && c.name == classe.name).count() > 1 {
            continue;
        }
        let (nome_classe, membros): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            DeclKind::ExtensionType(d) => (d.name, &d.members),
            _ => continue,
        };
        let Some(este) = cx.tipo_proprio(cid) else { continue };
        let texto_classe = interner.resolve(classe.name).to_string();
        let herdado_visivel = |cx: &mut Ctx<'_>, nome: &str| -> Option<Achado> {
            let chave = interner.lookup(nome)?;
            let (a, _) = cx.herdado(este, chave)?;
            if nome.starts_with('_') && program.class(a.dono).library != lib {
                return None;
            }
            Some(a)
        };
        let dono = |a: Achado| interner.resolve(program.class(a.dono).name).to_string();
        let mut conflitantes: Vec<String> = Vec::new();
        // Métodos declarados contra acessores herdados.
        for &m in membros {
            let MemberKind::Method(f) = &ast_.member(m).kind else { continue };
            let af = ast_.function(*f);
            if !matches!(af.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                continue;
            }
            let Some(n) = af.name else { continue };
            let nome = interner.resolve(n.sym).to_string();
            let getter = herdado_visivel(&mut cx, &nome);
            let setter = herdado_visivel(&mut cx, &format!("{nome}_="));
            if af.static_ && (getter.is_some() || setter.is_some()) {
                continue;
            }
            if classe.kind == K::ExtensionType {
                continue;
            }
            let acessor = |a: Option<Achado>| a.filter(|a| cx.especie(a.funcao).is_some_and(|e| e != Especie::Metodo));
            if let Some(a) = acessor(getter).or(acessor(setter)) {
                saida.push((
                    decl.unit,
                    Diagnostic::com_codigo(c::CONFLICTING_METHOD_AND_FIELD, n.span, [texto_classe.clone(), nome.clone(), dono(a)]),
                ));
            }
        }
        // Acessores declarados (getters, setters e os dos campos) contra
        // métodos herdados.
        for &m in membros {
            let mut acessores: Vec<(String, Span, bool)> = Vec::new();
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if matches!(af.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter)
                        && let Some(n) = af.name
                    {
                        acessores.push((interner.resolve(n.sym).to_string(), n.span, af.static_));
                    }
                }
                MemberKind::Field(vl) => {
                    for v in vl.variables.iter() {
                        let nome = interner.resolve(v.name.sym).to_string();
                        acessores.push((nome.clone(), v.name.span, vl.static_));
                        if !vl.final_ && !vl.const_ {
                            acessores.push((nome, v.name.span, vl.static_));
                        }
                    }
                }
                MemberKind::Constructor(_) => {}
            }
            for (nome, span, estatico) in acessores {
                let herdado = herdado_visivel(&mut cx, &nome).or_else(|| herdado_visivel(&mut cx, &format!("{nome}_=")));
                if estatico && herdado.is_some() {
                    conflitantes.push(nome);
                } else if let Some(a) = herdado
                    && cx.especie(a.funcao) == Some(Especie::Metodo)
                {
                    if classe.kind == K::ExtensionType {
                        continue;
                    }
                    saida.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::CONFLICTING_FIELD_AND_METHOD, span, [texto_classe.clone(), nome.clone(), dono(a)]),
                    ));
                    conflitantes.push(nome);
                }
            }
        }
        // Método e setter herdados com o mesmo nome.
        for chave in cx.chaves_da_hierarquia(cid) {
            let nome = interner.resolve(chave).to_string();
            if nome.ends_with("_=") || conflitantes.contains(&nome) {
                continue;
            }
            let Some(metodo) = herdado_visivel(&mut cx, &nome) else { continue };
            if cx.especie(metodo.funcao) != Some(Especie::Metodo) {
                continue;
            }
            if let Some(setter) = herdado_visivel(&mut cx, &format!("{nome}_="))
                && cx.especie(setter.funcao) == Some(Especie::Setter)
            {
                let tipo = match classe.kind {
                    K::Mixin => "mixin",
                    K::ExtensionType => "extension type",
                    _ => "class",
                };
                saida.push((
                    decl.unit,
                    Diagnostic::com_codigo(c::CONFLICTING_INHERITED_METHOD_AND_SETTER, nome_classe.span, [tipo, texto_classe.as_str(), nome.as_str()]),
                ));
            }
        }
    }
    saida
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
    let mut cx = Ctx { program, interner, table, core, outline };
    let mut saida = Vec::new();
    let Some(enum_core) = cx.classe_do_core("Enum") else { return saida };
    for &cid in classes {
        let classe = program.class(cid);
        let Some(decl) = classe.decl else { continue };
        let implementa_enum = classe.kind == K::Enum
            || outline.hierarchy.get(cid).is_some_and(|d| d.supertypes.contains_key(&enum_core));
        if !implementa_enum || cid == enum_core {
            continue;
        }
        let ast_ = &program.unit(decl.unit).ast;
        let fonte = &program.unit(decl.unit).source;
        if fonte.lines().any(|l| l.trim_start().starts_with("augment ")) {
            continue;
        }
        let (nome_classe, membros): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Enum(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            _ => continue,
        };
        let e_enum = classe.kind == K::Enum;
        let proibido = |n: &str| matches!(n, "index" | "hashCode" | "==");
        // Declarados.
        for &m in membros {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let n = interner.resolve(v.name.sym);
                        if !e_enum && n == "values" {
                            saida.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_DECLARATION, v.name.span, [] as [&str; 0])));
                        }
                        if proibido(n) {
                            saida.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION, v.name.span, [n])));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    let Some(nome) = af.name else { continue };
                    let n = interner.resolve(nome.sym);
                    let abstrato = !af.external && matches!(af.body, ast::FunctionBody::Empty);
                    let setter = af.kind == ast::FunctionKind::Setter;
                    if !(af.static_ || abstrato || setter) && proibido(n) {
                        saida.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION, nome.span, [n])));
                    }
                    if !af.static_ && !e_enum && n == "values" {
                        saida.push((decl.unit, Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_DECLARATION, nome.span, [] as [&str; 0])));
                    }
                }
                _ => {}
            }
        }
        // Herdados (`getInheritedConcreteMap2`; mixins não herdam).
        if classe.kind != K::Mixin {
            for n in ["hashCode", "==", "index"] {
                let Some(chave) = interner.lookup(n) else { continue };
                let herdado = classe
                    .mixin_classes
                    .iter()
                    .rev()
                    .filter(|&&m| !cx.fora_da_hierarquia(cid, m))
                    .find_map(|&m| cx.implementado(m, chave, 1).filter(|a| !cx.de_object(a.dono)))
                    .or_else(|| cx.superclasse(cid).and_then(|s| cx.implementado(s, chave, 1)));
                let Some(a) = herdado else { continue };
                let dono = program.class(a.dono);
                let conta = if dono.kind != K::Class && dono.kind != K::MixinApplication {
                    true
                } else if n == "index" {
                    a.dono != enum_core
                } else {
                    !cx.de_object(a.dono)
                };
                if conta {
                    saida.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE, nome_classe.span, [n, interner.resolve(dono.name)]),
                    ));
                }
            }
        }
        let Some(este) = cx.tipo_proprio(cid) else { continue };
        let values = interner.lookup("values");
        let values_set = interner.lookup("values_=");
        let herdado = values.and_then(|k| cx.herdado(este, k)).or_else(|| values_set.and_then(|k| cx.herdado(este, k)));
        if let Some((a, _)) = herdado {
            saida.push((
                decl.unit,
                Diagnostic::com_codigo(c::ILLEGAL_ENUM_VALUES_INHERITANCE, nome_classe.span, [interner.resolve(program.class(a.dono).name)]),
            ));
        }
    }
    saida
}
