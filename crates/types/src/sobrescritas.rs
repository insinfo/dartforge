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
    fn membro_da_heranca(&mut self, d: ClassId, chave: SymbolId, herdado: bool) -> Option<crate::heranca::Membro> {
        let nome = crate::heranca::Nome::novo(self.interner, self.biblioteca, chave);
        let mut h = std::mem::take(&mut self.heranca);
        let r = {
            let mut p = crate::heranca::ProvedorDoOutline {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            if herdado { h.herdado(&mut p, d, nome) } else { h.membro(&mut p, d, nome, false, None, false) }
        };
        self.heranca = h;
        r
    }

    /// `InheritanceManager3.getMember(s, nome)`: o membro na interface do
    /// tipo de interface `s` (a assinatura combinada, com o `topMerge`), com
    /// o tipo visto por `s`.
    fn na_interface(&mut self, s: TypeId, chave: SymbolId, _prof: u32) -> Option<(Achado, TypeId)> {
        self.na_interface_ex(s, chave, false)
    }

    /// `InheritanceManager3.getInherited(s, nome)`.
    fn herdado(&mut self, s: TypeId, chave: SymbolId) -> Option<(Achado, TypeId)> {
        self.na_interface_ex(s, chave, true)
    }

    fn na_interface_ex(&mut self, s: TypeId, chave: SymbolId, herdado: bool) -> Option<(Achado, TypeId)> {
        let (d, args) = match self.table.get(s).clone() {
            Type::Interface { class, args, .. } => (class, args),
            _ => return None,
        };
        let m = self.membro_da_heranca(d, chave, herdado)?;
        let formais: Box<[TypeParamId]> = self.outline.classes.get(d.0 as usize)?.type_params.clone();
        let t = if !formais.is_empty() && formais.len() == args.len() {
            let subst: HashMap<TypeParamId, TypeId> = formais.iter().copied().zip(args.iter().copied()).collect();
            substitute(m.tipo, &subst, self.table)
        } else {
            m.tipo
        };
        Some((Achado { dono: m.classe, funcao: m.funcao }, t))
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

    /// `ParameterElement.isCovariant` do membro `f` visto pela classe `c`: o
    /// declarado nela (escrito mais o herdado dos sobrescritos) ou o
    /// implementado (com a covariância herdada).
    fn covariantes_efetivos(&mut self, c: ClassId, f: FunctionElementId, chave: SymbolId) -> (Vec<usize>, Vec<SymbolId>) {
        let nome = crate::heranca::Nome::novo(self.interner, self.program.class(c).library, chave);
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
            .get(&nome)
            .filter(|m| m.funcao == f)
            .or_else(|| i.implemented.get(&nome).filter(|m| m.funcao == f))
            .or_else(|| i.declared.get(&nome))
            .or_else(|| i.implemented.get(&nome));
        let mut pos = Vec::new();
        let mut nomes = Vec::new();
        for d in m.map(|m| &m.covariantes[..]).unwrap_or(&[]) {
            match d {
                crate::heranca::Param::Indice(k) => pos.push(*k),
                crate::heranca::Param::Nome(n) => nomes.push(*n),
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
        let nome = self.interner.resolve(self.program.function(m.funcao).name).to_string();
        let biblioteca = self.program.class(m.declarante).library;
        let (pos, nomes) = self.covariantes_efetivos(m.declarante, m.funcao, chave);
        let para_sub = self.para_subtipo(m.tipo, &pos, &nomes);
        for &s in supers {
            let Some((achado, do_super)) = self.na_interface(s, chave, 0) else { continue };
            if nome.starts_with('_') && self.program.class(achado.dono).library != biblioteca {
                continue;
            }
            if self.especie(achado.funcao) != Some(especie) {
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
                // `invalidOverride`: o membro sobrescrito, no nome não
                // sintético dele (no arquivo dele, se é outro).
                let mut d = Diagnostic::com_codigo(codigo, m.span, args);
                if let Some((u, span)) = self.program.nome_nao_sintetico_da_funcao(achado.funcao) {
                    let unidade_do_erro = self.program.nome_nao_sintetico_da_funcao(m.funcao).map(|(x, _)| x);
                    let arquivo = (Some(u) != unidade_do_erro).then(|| self.program.caminho_da_unidade(u).into());
                    let texto = if especie == Especie::Setter { "The setter being overridden." } else { "The member being overridden." };
                    d.contexto.push(dartforge_diagnostics::Contexto { arquivo, span, mensagem: texto.into() });
                }
                saida.push(d);
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
    let sem_assinatura: std::collections::HashSet<FunctionElementId> =
        crate::fase_heranca::metodos_sem_assinatura_combinada(program, interner, table, core, outline, classes).into_iter().map(|(_, f, _, _)| f).collect();
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
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
        cx.biblioteca = classe.library;
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
            if sem_assinatura.contains(&f) {
                continue;
            }
            let Some(tipo) = outline.functions.get(f.0 as usize).map(|d| d.signature) else { continue };
            let conf = Conferencia { funcao: f, tipo, declarante: cid, span };
            cx.conferir(Conferencia { ..conf }, &supers, &mut diags);
            if let Some(chave) = cx.chave(f) {
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
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut saida = Vec::new();
    // Acessores locais: `(recipiente, nome)` → getters e setters.
    #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
    enum Recipiente {
        Unidade(UnitId),
        Classe(ClassId),
        Instancia(ClassId),
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
    let mut grupos: HashMap<(Recipiente, SymbolId), (Vec<Acessor>, Vec<Acessor>)> = HashMap::new();
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
        // Os sintéticos do enum: `values` é o getter estático (no nome do
        // enum); `index` e `name` vêm de `Enum` pela interface.
        if matches!(f.node, FunctionRef::None) && f.variable.is_none() {
            if f.static_
                && let Some(c) = f.class
                && program.class(c).kind == dartforge_elements::model::ClassKind::Enum
                && interner.resolve(f.name) == "values"
            {
                grupos.entry((Recipiente::Classe(c), f.name)).or_default().0.push(Acessor::Values(c));
            }
            continue;
        }
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
        if especie == Especie::Getter { g.0.push(Acessor::Funcao(id)) } else { g.1.push(Acessor::Funcao(id)) }
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
                grupos.entry((Recipiente::Classe(c), v.name)).or_default().0.push(Acessor::Variavel(vid));
            }
            (VariableRef::Representation { .. }, Some(c), _) => {
                grupos.entry((Recipiente::Instancia(c), v.name)).or_default().0.push(Acessor::Variavel(vid));
            }
            (VariableRef::Field { .. }, None, Some(e)) => {
                let g = grupos.entry((Recipiente::Extensao(e.0), v.name)).or_default();
                g.0.push(Acessor::Variavel(vid));
                if !v.final_ && !v.const_ {
                    g.1.push(Acessor::Variavel(vid));
                }
            }
            _ => {}
        }
    }
    let tipo_this = |table: &mut TypeTable, c: ClassId| -> TypeId {
        let params = outline.classes.get(c.0 as usize).map(|d| d.type_params.clone()).unwrap_or_default();
        let args: Box<[TypeId]> = params.iter().map(|&p| table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        table.intern(Type::Interface { class: c, args, nullable: false })
    };
    let mut chaves: Vec<_> = grupos.keys().copied().collect();
    chaves.sort_by_key(|(r, n)| (interner.resolve(*n).to_string(), format!("{r:?}")));
    for k in chaves {
        let (gs, ss) = &grupos[&k];
        let ([g], [s]) = (gs.as_slice(), ss.as_slice()) else { continue };
        let tipo = |cx: &mut Ctx<'_>, a: Acessor, getter: bool| -> Option<TypeId> {
            match a {
                Acessor::Funcao(f) => cx.tipo_de_acessor(f, getter),
                Acessor::Variavel(v) => outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)),
                Acessor::Values(c) => {
                    let lista = core.list_class?;
                    let e = tipo_this(cx.table, c);
                    Some(cx.table.intern(Type::Interface { class: lista, args: Box::new([e]), nullable: false }))
                }
            }
        };
        let (Some(tg), Some(ts)) = (tipo(&mut cx, *g, true), tipo(&mut cx, *s, false)) else { continue };
        let ok = {
            let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
            is_subtype(tg, ts, &mut env)
        };
        // Num tipo de extensão, o getter da representação cede o lugar ao setter.
        let representacao = matches!(k.0, Recipiente::Instancia(_))
            && matches!(*g, Acessor::Variavel(v) if matches!(program.variable(v).node, VariableRef::Representation { .. }));
        let lugar_de = |a: Acessor| -> Option<(UnitId, Span)> {
            match a {
                Acessor::Funcao(f) => nome_de(program, f),
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
        let lugar = if representacao { lugar_de(*s) } else { lugar_de(*g) };
        if !ok && let Some((u, span)) = lugar {
            let nome = interner.resolve(k.1).to_string();
            let args = [nome.clone(), formatar(cx.table, tg, interner, program), formatar(cx.table, ts, interner, program), nome];
            saida.push((u, Diagnostic::com_codigo(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args)));
        }
    }
    // O `Enum` do `dart:core`: os getters sintéticos `index` e `name` de um
    // enum são, na interface, os dele (`'Enum.index'`).
    let enum_do_core = program
        .classes
        .iter()
        .position(|k| interner.resolve(k.name) == "Enum" && program.library(k.library).uri == "dart:core")
        .map(|i| ClassId(i as u32));
    let sintetico_de_enum = |f: FunctionElementId| {
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
    let extensoes: Vec<ClassId> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, k)| k.library == lib && k.kind == dartforge_elements::model::ClassKind::ExtensionType)
        .map(|(i, _)| ClassId(i as u32))
        .filter(|c| !classes.contains(c))
        .collect();
    for &cid in classes.iter().chain(extensoes.iter()) {
        let classe = program.class(cid);
        let de_extensao = classe.kind == dartforge_elements::model::ClassKind::ExtensionType;
        cx.biblioteca = classe.library;
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
            let Some((mut g, tg)) = cx.na_interface(este, nome, 0) else { continue };
            let Some((s, ts)) = cx.na_interface(este, chave_setter, 0) else { continue };
            if de_extensao && g.dono == cid && s.dono == cid {
                continue;
            }
            if sintetico_de_enum(g.funcao)
                && let Some(e) = enum_do_core
            {
                g.dono = e;
            }
            if texto.starts_with('_')
                && (program.class(g.dono).library != classe.library || program.class(s.dono).library != classe.library)
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
                    DeclKind::ExtensionType(d) => Some((decl.unit, d.name.span)),
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
    /// `getMember2(d, nome, concrete: true)`: o `implemented` da interface
    /// (com os encaminhadores de `noSuchMethod` e a covariância herdada).
    fn implementado(&mut self, d: ClassId, chave: SymbolId, _prof: u32) -> Option<Achado> {
        let nome = crate::heranca::Nome::novo(self.interner, self.biblioteca, chave);
        let mut h = std::mem::take(&mut self.heranca);
        let r = {
            let mut p = crate::heranca::ProvedorDoOutline {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            h.membro(&mut p, d, nome, true, None, false)
        };
        self.heranca = h;
        r.map(|m| Achado { dono: m.classe, funcao: m.funcao })
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

    /// O nome privado `k` é declarado por `c` ou por um supertipo da
    /// biblioteca `lib`.
    fn privado_da_biblioteca(&self, c: ClassId, k: SymbolId, lib: dartforge_elements::model::LibraryId) -> bool {
        let supers = self.outline.hierarchy.get(c).map(|d| d.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default();
        std::iter::once(c).chain(supers).any(|s| self.program.class(s).library == lib && self.program.class(s).instance_members.contains_key(&k))
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
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut saida = Vec::new();
    let nsm = interner.lookup("noSuchMethod");
    for &cid in classes {
        let classe = program.class(cid);
        cx.biblioteca = classe.library;
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
        // Sem o experimento, o parser lê `augment` como identificador e a
        // recuperação é a do analyzer (T1.1 f): nada a pular.
        let augment_no_texto = program.biblioteca_com_augmentations(classe.library)
            && texto_decl.lines().any(|l| l.trim_start().starts_with("augment "));
        if recuperado || augment_no_texto || ast_.decl(decl.decl).augment {
            continue;
        }
        // T1.1 e: a interface é a do dono do grupo de homônimos (o cache do
        // analyzer é por elemento), e o membro abstrato é procurado na árvore
        // da própria declaração (`_reportConcreteClassWithAbstractMember`).
        let intf = program.dono_da_classe(cid);
        let Some(este) = cx.tipo_proprio(intf) else { continue };
        // `noSuchMethod` implementado que não é o de `Object`: encaminha.
        if let Some(n) = nsm
            && let Some(a) = cx.implementado(intf, n, 0)
            && program.library(program.class(a.dono).library).uri != "dart:core"
        {
            continue;
        }
        // `_isNotImplementedInConcreteSuperClass`: a superclasse declarada,
        // se concreta.
        let superclasse_concreta = program.class(intf).supertype_class.filter(|&s| {
            let sc = program.class(s);
            sc.decl.is_some()
                && sc.kind == dartforge_elements::model::ClassKind::Class
                && !sc.modifiers.abstract_
                && !sc.modifiers.sealed
        });
        let mut herdados: Vec<String> = Vec::new();
        let mut incerto = false;
        for chave in cx.chaves_da_hierarquia(intf) {
            let texto = interner.resolve(chave).to_string();
            // Um nome privado só declarado em outras bibliotecas (`_name` de
            // `_Enum`) é outro `Name` para esta classe: não está na interface
            // que ela vê.
            if texto.starts_with('_') && !cx.privado_da_biblioteca(intf, chave, classe.library) {
                continue;
            }
            let Some((membro, tipo_membro)) = cx.na_interface(este, chave, 0) else {
                // Conflito ou assinatura combinada que não se decide aqui.
                if !program.class(intf).instance_members.contains_key(&chave) {
                    incerto = true;
                }
                continue;
            };
            if texto.starts_with('_') && program.class(membro.dono).library != classe.library {
                continue;
            }
            let Some(especie) = cx.especie(membro.funcao) else { continue };
            let exibido = texto.strip_suffix("_=").unwrap_or(&texto).to_string();
            match cx.implementado(intf, chave, 0) {
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
                    let Some(tipo_concreto) = cx.tipo_visto(este, concreto) else { continue };
                    // `_inheritCovariance`: a covariância vem de todos os
                    // membros homônimos na hierarquia da classe.
                    let (pos, nomes) = cx.covariantes_efetivos(intf, concreto.funcao, chave);
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
        // Também no nome sintético (vazio), como o analyzer.
        let nome = var.name;
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
    let nome_super = k.initializers.iter().find_map(|x| match x {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    let chave = match nome_super {
        Some(Some(n)) if interner.resolve(n.sym) != "new" => Some(n.sym),
        _ => interner.lookup(""),
    }?;
    let fs = *program.class(s).constructors.get(&chave)?;
    let FunctionRef::Constructor { unit: su, member: sm } = program.function(fs).node else { return None };
    let MemberKind::Constructor(sk) = &program.unit(su).ast.member(sm).kind else { return None };
    if p.kind == ast::ParameterKind::Named {
        let nome = p.nome_externo()?.sym;
        let j = sk.parameters.iter().position(|q| q.kind == ast::ParameterKind::Named && q.nome_externo().is_some_and(|n| n.sym == nome))?;
        return Some((fs, j));
    }
    let indice = k.parameters[..i].iter().filter(|q| q.super_).count();
    let posicionais: Vec<usize> = sk.parameters.iter().enumerate().filter(|(_, q)| q.kind != ast::ParameterKind::Named).map(|(j, _)| j).collect();
    posicionais.get(indice).map(|&j| (fs, j))
}

/// O tipo do valor padrão do parâmetro `i` de `f` (o `computeConstantValue`
/// dele: o valor escrito, ou o encaminhado pela cadeia de `super.x`), se
/// houver; `None` sem valor. O tipo do valor é o estático do padrão escrito
/// (dos corpos), ou o do parâmetro sem eles.
fn valor_padrao_do_parametro(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    corpos: Option<&crate::resolved::BodyTypes>,
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
    let tipo_p = outline.functions.get(f.0 as usize)?.parameters.get(i)?.ty;
    if let Some(d) = p.default_value {
        return Some(corpos.and_then(|c| c.units.get(unit.0 as usize)).and_then(|u| u.get_type(d)).unwrap_or(tipo_p));
    }
    if !p.super_ {
        return None;
    }
    // `_superConstructorParameterDefaultValue`: o valor do parâmetro da
    // superclasse, se o tipo dele cabe no deste.
    let (fs, j) = parametro_do_super(program, interner, f, i)?;
    let v = valor_padrao_do_parametro(program, interner, table, core, outline, corpos, fs, j, prof + 1)?;
    let core = core?;
    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
    is_subtype(v, tipo_p, &mut env).then_some(v)
}

/// `defaultValueCode != null` do parâmetro `i` de `f` (o `hasDefaultValue`):
/// o padrão escrito; num `super.x` opcional sem padrão, o do parâmetro da
/// superclasse, se o valor encaminhado existir
/// (`DefaultSuperFormalParameterElementImpl.defaultValueCode`,
/// `element.dart:1732-1748`); o `required` nunca tem.
#[allow(clippy::too_many_arguments)]
fn tem_padrao(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    corpos: Option<&crate::resolved::BodyTypes>,
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
    if valor_padrao_do_parametro(program, interner, table, core, outline, corpos, f, i, prof).is_none() {
        return false;
    }
    match parametro_do_super(program, interner, f, i) {
        Some((fs, j)) => tem_padrao(program, interner, table, core, outline, corpos, fs, j, prof + 1),
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
    corpos: Option<&crate::resolved::BodyTypes>,
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
        let aumentada =
            program.biblioteca_com_augmentations(program.unit(unit).library) && (linha_comeca_com_augment(inicio) || da_classe);
        if aumentada {
            continue;
        }
        for (idx, (p, pd)) in params.iter().zip(dados.parameters.iter()).enumerate() {
            if p.kind == ast::ParameterKind::Required || p.required || p.default_value.is_some() {
                continue;
            }
            // `super.x` opcional: o padrão encaminhado do construtor da
            // superclasse conta (`hasDefaultValue`); o curinga posicional
            // `_` com o recurso fica de fora
            // (`_isWildcardSuperFormalPositionalParameter`).
            if p.super_ {
                if tem_padrao(program, interner, table, core, outline, corpos, FunctionElementId(i as u32), idx, 0) {
                    continue;
                }
                let curinga = p.kind == ast::ParameterKind::Optional
                    && p.name.is_some_and(|n| interner.resolve(n.sym) == "_")
                    && program.library(lib).features.tem(dartforge_frontend::Feature::WildcardVariables);
                if curinga {
                    continue;
                }
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
    let mut saida = Vec::new();
    for (ci, classe) in program.classes.iter().enumerate() {
        cx.biblioteca = classe.library;
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
        if program.biblioteca_com_augmentations(lib) && fonte.lines().any(|l| l.trim_start().starts_with("augment ")) {
            continue;
        }
        let (nome_classe, membros): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            DeclKind::ExtensionType(d) => (d.name, &d.members),
            _ => continue,
        };
        // T1.1 e: a interface herdada é a do dono do grupo de homônimos; os
        // membros declarados são os da própria declaração.
        let intf = program.dono_da_classe(cid);
        let Some(este) = cx.tipo_proprio(intf) else { continue };
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
            if af.static_
                && let Some(a) = getter.or(setter)
            {
                saida.push((
                    decl.unit,
                    Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, n.span, [texto_classe.clone(), nome.clone(), dono(a)]),
                ));
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
                if estatico && let Some(a) = herdado {
                    saida.push((
                        decl.unit,
                        Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [texto_classe.clone(), nome.clone(), dono(a)]),
                    ));
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
        for chave in cx.chaves_da_hierarquia(intf) {
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
                // As duas mensagens de contexto: de onde vêm o método e o
                // setter (o tipo e o nome da classe dona, no nome do membro).
                let mut d = Diagnostic::com_codigo(c::CONFLICTING_INHERITED_METHOD_AND_SETTER, nome_classe.span, [tipo, texto_classe.as_str(), nome.as_str()]);
                for (a, molde) in [(metodo, "The method is inherited from the"), (setter, "The setter is inherited from the")] {
                    let p = program;
                    let (f, dono) = (a.funcao, a.dono);
                    let especie_do_dono = match p.class(dono).kind {
                        K::Mixin => "mixin",
                        K::Enum => "enum",
                        K::ExtensionType => "extension type",
                        _ => "class",
                    };
                    let Some((u, span)) = p.nome_nao_sintetico_da_funcao(f) else { continue };
                    let arquivo = (u != decl.unit).then(|| p.caminho_da_unidade(u).into());
                    let texto = format!("{molde} {especie_do_dono} '{}'.", interner.resolve(p.class(dono).name));
                    d.contexto.push(dartforge_diagnostics::Contexto { arquivo, span, mensagem: texto.into() });
                }
                saida.push((decl.unit, d));
            }
        }
    }
    saida
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
    let mut saida = Vec::new();
    for (ci, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || classe.kind != K::Enum {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let DeclKind::Enum(d) = &ast_.decl(decl.decl).kind else { continue };
        let dono = program.dono_da_classe(ClassId(ci as u32));
        let texto_enum = interner.resolve(program.class(dono).name).to_string();
        let na_interface = |cx: &mut Ctx<'_>, nome: &str| -> bool {
            let achou = |cx: &mut Ctx<'_>, texto: &str| interner.lookup(texto).is_some_and(|k| cx.membro_da_heranca(dono, k, false).is_some());
            achou(cx, nome) || achou(cx, &format!("{nome}_="))
        };
        // Os acessores: as constantes (getters), os campos estáticos (getter
        // e, se não final, setter) e os getters e setters declarados.
        let mut acessores: Vec<(String, Span)> = Vec::new();
        for k in d.constants.iter() {
            acessores.push((interner.resolve(k.name.sym).to_string(), k.name.span));
        }
        let mut metodos: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if vl.static_ => {
                    for v in vl.variables.iter() {
                        let nome = interner.resolve(v.name.sym).to_string();
                        acessores.push((nome.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            acessores.push((nome, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if !af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let nome = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => acessores.push((nome, n.span)),
                        _ => metodos.push((nome, n.span)),
                    }
                }
                _ => {}
            }
        }
        for (nome, span) in acessores {
            if nome != "values" && na_interface(&mut cx, &nome) {
                saida.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [texto_enum.as_str(), nome.as_str(), texto_enum.as_str()])));
            }
        }
        for (nome, span) in metodos {
            if na_interface(&mut cx, &nome) {
                saida.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_STATIC_AND_INSTANCE, span, [texto_enum.as_str(), nome.as_str(), texto_enum.as_str()])));
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
        let homonimo = program.classes.iter().enumerate().any(|(j, k)| {
            j != ci && k.kind == K::Enum && k.name == classe.name && k.decl.is_some_and(|x| x.unit == decl.unit)
        });
        if homonimo {
            continue;
        }
        let Some(este) = cx.tipo_proprio(dono) else { continue };
        let herdado = |cx: &mut Ctx<'_>, nome: &str| -> Option<Achado> {
            let visivel = |cx: &mut Ctx<'_>, texto: &str| -> Option<Achado> {
                let chave = interner.lookup(texto)?;
                let (a, _) = cx.herdado(este, chave)?;
                if texto.starts_with('_') && program.class(a.dono).library != lib {
                    return None;
                }
                Some(a)
            };
            visivel(cx, nome).or_else(|| visivel(cx, &format!("{nome}_=")))
        };
        let mut acessores: Vec<(String, Span)> = Vec::new();
        let mut metodos: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let nome = interner.resolve(v.name.sym).to_string();
                        acessores.push((nome.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            acessores.push((nome, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let nome = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => acessores.push((nome, n.span)),
                        _ => metodos.push((nome, n.span)),
                    }
                }
                _ => {}
            }
        }
        let dono_de = |a: &Achado| interner.resolve(program.class(a.dono).name).to_string();
        for (nome, span) in acessores {
            if let Some(a) = herdado(&mut cx, &nome)
                && cx.especie(a.funcao) == Some(Especie::Metodo)
            {
                let d_ = dono_de(&a);
                saida.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_FIELD_AND_METHOD, span, [texto_enum.as_str(), nome.as_str(), d_.as_str()])));
            }
        }
        for (nome, span) in metodos {
            if let Some(a) = herdado(&mut cx, &nome)
                && cx.especie(a.funcao).is_some_and(|e| e != Especie::Metodo)
            {
                let d_ = dono_de(&a);
                saida.push((decl.unit, Diagnostic::com_codigo(c::CONFLICTING_METHOD_AND_FIELD, span, [texto_enum.as_str(), nome.as_str(), d_.as_str()])));
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
    let mut cx = Ctx { program, interner, table, core, outline, heranca: crate::heranca::Heranca::default(), biblioteca: dartforge_elements::model::LibraryId(0) };
    let mut saida = Vec::new();
    let Some(enum_core) = cx.classe_do_core("Enum") else { return saida };
    for &cid in classes {
        let classe = program.class(cid);
        cx.biblioteca = classe.library;
        let Some(decl) = classe.decl else { continue };
        let implementa_enum = classe.kind == K::Enum
            || outline.hierarchy.get(cid).is_some_and(|d| d.supertypes.contains_key(&enum_core));
        if !implementa_enum || cid == enum_core {
            continue;
        }
        let ast_ = &program.unit(decl.unit).ast;
        let fonte = &program.unit(decl.unit).source;
        if program.biblioteca_com_augmentations(classe.library) && fonte.lines().any(|l| l.trim_start().starts_with("augment ")) {
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
                let mut herdado = None;
                for &m in classe.mixin_classes.iter().rev() {
                    if cx.fora_da_hierarquia(cid, m) {
                        continue;
                    }
                    if let Some(a) = cx.implementado(m, chave, 1)
                        && !cx.de_object(a.dono)
                    {
                        herdado = Some(a);
                        break;
                    }
                }
                if herdado.is_none()
                    && let Some(s) = cx.superclasse(cid)
                {
                    herdado = cx.implementado(s, chave, 1);
                }
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
