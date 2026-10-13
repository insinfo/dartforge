//! Chamada tipada e a entrada que só confere o que o chamador estático não
//! garante (N17, docs/NATIVO-PLANO.md §9).
//!
//! **A regra do Dart.** Numa chamada cujo receptor tem tipo estático
//! conhecido, o analisador já garantiu que cada argumento é subtipo do tipo
//! do parâmetro na interface — menos nos parâmetros *covariantes*: os
//! declarados `covariant` (ou que herdam a palavra de um membro sobrescrito)
//! e os que mencionam um parâmetro de tipo da classe (a covariância das
//! classes genéricas: `List<Object?> l = <int>[]; l.add('x')` passa na
//! análise e lança `TypeError` na execução). A VM compila por isso duas
//! entradas por método: a *checked entry*, dos encaminhadores dinâmicos
//! (`dyn:m`), que confere tudo, e a *unchecked entry*, das chamadas
//! tipadas, que só confere os covariantes. Um argumento `dynamic` numa
//! chamada tipada ganha do CFE um cast implícito em volta dele (`as T`, com
//! o `T` da invocação), avaliado logo depois do próprio argumento.
//!
//! **Aqui.** A chamada por seletor de um membro do SDK da fonte com o
//! receptor tipado usa o seletor `t` + o de sempre (`tc:m`, `ts:x`); a
//! tabela da classe o liga à entrada `$tc`/`$ts` do método quando ela
//! existe — só quando a `$c` confere algum parâmetro que a tipada dispensa
//! ([`needs_typed_entry`]) — e o runtime (`dartforge_seletor`) cai no
//! seletor sem o `t` quando a classe não tem a tipada (métodos sem o que
//! dispensar, encaminhadores de `noSuchMethod`, classes do programa). A
//! entrada tipada também não confere a aridade (`dartforge_args_casam`): a
//! sobrescrita válida aceita os argumentos da interface.
//!
//! O cast implícito dos argumentos `dynamic` (e a regra de quando a chamada
//! pode ser tipada) é o de [`FnBuilder::evaluate_args`] com os tipos
//! armados por `FnBuilder::arm_arg_types`: o tipo do parâmetro na
//! invocação, sem variáveis de tipo livres. Só a chamada cujos argumentos
//! têm todos o tipo garantido (`checked_args`) usa o seletor tipado; a
//! outra volta ao que confere tudo.

use super::fn_builder::FnBuilder;
use super::members::Evaluated;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionRef};
use dartforge_frontend::ast::MemberKind;
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::TypeId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type as T, TypeParamId, TypeParamOwner};
use std::collections::HashMap;

/// Os tipos dos parâmetros de uma invocação, já com os argumentos de tipo
/// do receptor ou da criação substituídos: posicionais em ordem e nomeados
/// pelo nome externo.
#[derive(Clone, Debug, Default)]
pub struct InvocationTypes {
    positionals: Vec<ParameterType>,
    named: Vec<(SymbolId, ParameterType)>,
}

/// O tipo de um parâmetro na invocação e, quando ele menciona parâmetros de
/// tipo da classe criada, a substituição que o fecha.
#[derive(Clone, Debug)]
struct ParameterType {
    ty: TypeId,
    subst: Option<HashMap<TypeParamId, TypeId>>,
}

impl InvocationTypes {
    fn of_argument(&self, name: Option<SymbolId>, i_pos: usize) -> Option<&ParameterType> {
        match name {
            Some(n) => self.named.iter().find(|(m, _)| *m == n).map(|(_, t)| t),
            None => self.positionals.get(i_pos),
        }
    }
}

/// `t` não menciona parâmetro de tipo (nem de classe, nem de função, nem os
/// ligados de um tipo de função genérico): a receita é constante e o cast
/// pode ser feito no ponto de chamada.
fn closed(ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>) -> bool {
    closed_until(ctx, t, subst, 8)
}

fn closed_until(ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>, prof: u32) -> bool {
    let closed = |ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>| closed_until(ctx, t, subst, prof);
    match ctx.table.get(t) {
        // A troca pode levar a outro parâmetro (o do supertipo ao da classe
        // do receptor), trocado pelo mesmo mapa.
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            prof > 0 && subst.and_then(|s| s.get(param)).is_some_and(|&x| closed_until(ctx, x, subst, prof - 1))
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args.iter().all(|a| closed(ctx, *a, subst)),
        T::FutureOr { arg, .. } => closed(ctx, *arg, subst),
        T::Function { type_params, ret, positional, optional, named, .. } => {
            type_params.is_empty()
                && closed(ctx, *ret, subst)
                && positional.iter().chain(optional.iter()).all(|p| closed(ctx, *p, subst))
                && named.iter().all(|(_, p, _)| closed(ctx, *p, subst))
        }
        T::Record { positional, named, .. } => {
            positional.iter().all(|p| closed(ctx, *p, subst)) && named.iter().all(|(_, p)| closed(ctx, *p, subst))
        }
        T::Dynamic | T::Void | T::Never | T::Null => true,
    }
}

/// O topo (`dynamic`, `void`, `Object?`): nada a conferir.
fn top(ctx: &Context, t: TypeId) -> bool {
    t == ctx.core.dynamic_ || t == ctx.core.object_nullable || matches!(ctx.table.get(t), T::Void)
}

/// Os parâmetros de `fid` escritos `covariant` (pelo índice na declaração),
/// inclusive o setter implícito de um campo `covariant`.
fn declared_covariant(ctx: &Context, fid: usize) -> Vec<(usize, Option<SymbolId>)> {
    let f = &ctx.program.functions[fid];
    let mut exit = Vec::new();
    match f.node {
        FunctionRef::Function { unit, function } => {
            let af = ctx.program.unit(unit).ast.function(function);
            for (i, p) in af.parameters.as_deref().unwrap_or(&[]).iter().enumerate() {
                if p.covariant {
                    let name = (p.kind == ParameterKind::Named).then(|| p.nome_externo().map(|n| n.sym)).flatten();
                    exit.push((i, name));
                }
            }
        }
        FunctionRef::None => {
            if let Some(v) = f.variable
                && let dartforge_elements::model::VariableRef::Field { unit, member, .. } = ctx.program.variables[v.0 as usize].node
                && let MemberKind::Field(vl) = &ctx.program.unit(unit).ast.member(member).kind
                && vl.covariant
            {
                exit.push((0, None));
            }
        }
        FunctionRef::Constructor { .. } => {}
    }
    exit
}

/// `t` menciona um parâmetro de tipo da classe `c` numa posição covariante
/// (ou invariante): o que torna o parâmetro de tipo `t` *genérico
/// covariante* (`isGenericCovariantImpl` do kernel). `positive` é a
/// polaridade corrente: o parâmetro de um tipo de função a inverte.
fn mentions_covariant(ctx: &Context, t: TypeId, c: ClassId, positive: bool) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            positive && matches!(ctx.table.param(*param).owner, TypeParamOwner::Class(d) if d == c)
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args.iter().any(|a| mentions_covariant(ctx, *a, c, positive)),
        T::FutureOr { arg, .. } => mentions_covariant(ctx, *arg, c, positive),
        T::Function { ret, positional, optional, named, .. } => {
            mentions_covariant(ctx, *ret, c, positive)
                || positional.iter().chain(optional.iter()).any(|p| mentions_covariant(ctx, *p, c, !positive))
                || named.iter().any(|(_, p, _)| mentions_covariant(ctx, *p, c, !positive))
        }
        T::Record { positional, named, .. } => {
            positional.iter().any(|p| mentions_covariant(ctx, *p, c, positive))
                || named.iter().any(|(_, p)| mentions_covariant(ctx, *p, c, positive))
        }
        T::Dynamic | T::Void | T::Never | T::Null => false,
    }
}

/// O parâmetro de `g` que corresponde ao `i`-ésimo de `fid` (mesma posição
/// entre os posicionais, ou mesmo nome entre os nomeados).
fn corresponding(ctx: &Context, fid: usize, i: usize, g: usize) -> Option<usize> {
    let p = ctx.outline.functions.get(fid)?.parameters.get(i)?;
    let gp = &ctx.outline.functions.get(g)?.parameters;
    if p.kind == ParameterKind::Named {
        gp.iter().position(|q| q.kind == ParameterKind::Named && q.externo == p.externo)
    } else {
        (gp.get(i)?.kind != ParameterKind::Named).then_some(i)
    }
}

/// O `j`-ésimo parâmetro de `g` é covariante no próprio `g`: escrito
/// `covariant`, ou genérico covariante da classe de `g`.
fn covariant_in(ctx: &Context, g: usize, j: usize) -> bool {
    let Some(q) = ctx.outline.functions.get(g).and_then(|o| o.parameters.get(j)) else { return false };
    let name = (q.kind == ParameterKind::Named).then_some(q.externo).flatten();
    let written = declared_covariant(ctx, g).iter().any(|(k, n)| match name {
        Some(x) => *n == Some(x),
        None => n.is_none() && *k == j,
    });
    written || ctx.program.functions[g].class.is_some_and(|c| mentions_covariant(ctx, q.ty, c, true))
}

/// O parâmetro `i` de `fid` é covariante: no próprio membro, ou porque o
/// correspondente de um membro homônimo de algum supertipo da classe o é
/// (a covariância se herda pela sobrescrita: `ParameterElement.isCovariant`
/// e o `isGenericCovariantImpl` que o CFE propaga).
fn inherited_covariant(ctx: &Context, fid: usize, i: usize) -> bool {
    if covariant_in(ctx, fid, i) {
        return true;
    }
    let f = &ctx.program.functions[fid];
    let Some(c) = f.class else { return false };
    let Some(data) = ctx.outline.hierarchy.get(c) else { return false };
    let name = ctx.symbol_name(f.name);
    let key = if f.kind == dartforge_elements::model::FunctionKind::Setter && !name.ends_with("_=") {
        ctx.interner.lookup(&format!("{name}_="))
    } else {
        Some(f.name)
    };
    let Some(key) = key else { return false };
    data.supertypes.keys().any(|&s| {
        s != c
            && ctx.program.classes[s.0 as usize].instance_members.get(&key).is_some_and(|g| {
                let g = g.0 as usize;
                corresponding(ctx, fid, i, g).is_some_and(|j| covariant_in(ctx, g, j))
            })
    })
}

/// A entrada `$c` confere o parâmetro `ty` de `fid`
/// ([`FnBuilder::check_entry_arguments`]): tipo nominal (classe),
/// ou um tipo que menciona parâmetro de tipo da classe em posição
/// covariante (`T`, `FutureOr<T>`, `T Function()`), fora do topo. Os
/// outros tipos de função e os parâmetros de tipo da função ficam com a
/// conversão de sempre.
pub fn checked_at_entry(ctx: &Context, fid: usize, ty: TypeId) -> bool {
    let f = &ctx.program.functions[fid];
    let generic = f.class.is_some_and(|c| mentions_covariant(ctx, ty, c, true));
    let nominal = matches!(ctx.table.get(ty), T::Interface { .. });
    !top(ctx, ty) && (generic || nominal)
}

/// O parâmetro `i` de `fid` precisa de conferência mesmo numa chamada
/// tipada: é covariante (ver `inherited_covariant`).
pub fn covariant_parameter(ctx: &Context, fid: usize, i: usize) -> bool {
    inherited_covariant(ctx, fid, i)
}

/// O método `fid` ganha a entrada tipada: a `$c` dele confere algum
/// parâmetro que a chamada tipada dispensa.
pub fn needs_typed_entry(ctx: &Context, fid: usize) -> bool {
    let Some(o) = ctx.outline.functions.get(fid) else { return false };
    o.parameters.iter().enumerate().any(|(i, p)| checked_at_entry(ctx, fid, p.ty) && !covariant_parameter(ctx, fid, i))
}

/// O seletor da chamada tipada correspondente a `selector` (`c:m` → `tc:m`).
pub fn typed_selector(selector: &str) -> String {
    format!("t{selector}")
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Arma os tipos dos parâmetros da invocação `expr_id` (alvo `target`)
    /// para o [`FnBuilder::evaluate_args`] dos argumentos `args`: o tipo
    /// estático do alvo, se é um tipo de função (o do membro visto pelo
    /// receptor, ou o da função), senão o do construtor resolvido.
    /// Devolve o que estava armado, para quem chama restaurar.
    pub(super) fn arm_arg_types(
        &mut self,
        target: ast::ExprId,
        expr_id: ast::ExprId,
        args: &[ast::Argument],
    ) -> Option<(usize, InvocationTypes)> {
        let types = self.target_types(target, expr_id);
        let key = args.as_ptr() as usize;
        std::mem::replace(&mut self.arg_types, types.map(|t| (key, t)))
    }

    /// Arma os tipos dos parâmetros do construtor `fid` de uma criação cujo
    /// tipo estático é `ty` (os argumentos de tipo da classe substituem os
    /// parâmetros dela).
    pub(super) fn arm_constructor_types(
        &mut self,
        fid: usize,
        typ: Option<TypeId>,
        args: &[ast::Argument],
    ) -> Option<(usize, InvocationTypes)> {
        let types = self.function_types(fid, typ);
        let key = args.as_ptr() as usize;
        std::mem::replace(&mut self.arg_types, types.map(|t| (key, t)))
    }

    fn target_types(&self, target: ast::ExprId, expr_id: ast::ExprId) -> Option<InvocationTypes> {
        if let Some(t) = self.ctx.get_type(self.unit_id, target)
            && let T::Function { type_params, positional, optional, named, .. } = self.ctx.table.get(t)
        {
            if !type_params.is_empty() {
                return None;
            }
            let without = |ty: &TypeId| ParameterType { ty: *ty, subst: None };
            return Some(InvocationTypes {
                positionals: positional.iter().chain(optional.iter()).map(without).collect(),
                named: named.iter().map(|(n, ty, _)| (*n, without(ty))).collect(),
            });
        }
        let resolved = self
            .ctx
            .get_resolved(self.unit_id, expr_id)
            .or_else(|| self.ctx.get_resolved(self.unit_id, target))
            .cloned();
        let fid = match resolved {
            Some(Resolved::Constructor(f)) => f.0 as usize,
            Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => f.0 as usize,
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => f.0 as usize,
            _ => return None,
        };
        let typ = self.ctx.get_type(self.unit_id, expr_id);
        self.function_types(fid, typ)
    }

    /// Os tipos dos parâmetros de `fid`; num construtor (ou fábrica) de
    /// classe genérica, com os argumentos de tipo de `created`.
    fn function_types(&self, fid: usize, created: Option<TypeId>) -> Option<InvocationTypes> {
        let o = self.ctx.outline.functions.get(fid)?;
        if !o.type_params.is_empty() {
            return None;
        }
        let f = &self.ctx.program.functions[fid];
        let mut subst = None;
        if let Some(c) = f.class
            && matches!(f.node, FunctionRef::Constructor { .. })
            && let Some(data) = self.ctx.outline.classes.get(c.0 as usize)
            && !data.type_params.is_empty()
            && let Some(t) = created
            && let T::Interface { class, args, .. } = self.ctx.table.get(t)
            && *class == c
            && args.len() == data.type_params.len()
        {
            subst = Some(data.type_params.iter().copied().zip(args.iter().copied()).collect::<HashMap<_, _>>());
        }
        let mut t = InvocationTypes::default();
        for p in o.parameters.iter() {
            let x = ParameterType { ty: p.ty, subst: subst.clone() };
            if p.kind == ParameterKind::Named {
                if let Some(n) = p.externo {
                    t.named.push((n, x));
                }
            } else {
                t.positionals.push(x);
            }
        }
        Some(t)
    }

    /// Depois de avaliar o argumento `value` (expressão `expr`, nome
    /// `name`, `i_pos`-ésimo posicional): se o tipo estático dele é
    /// `dynamic`, o cast implícito para o tipo do parâmetro na invocação
    /// armada; e a marca de conferido quando o tipo fica garantido.
    pub(super) fn implicit_argument_cast(
        &mut self,
        types: Option<&InvocationTypes>,
        expr: ast::ExprId,
        name: Option<SymbolId>,
        i_pos: usize,
        value: &Operand,
    ) {
        let Some(static_ty) = self.ctx.get_type(self.unit_id, expr) else { return };
        if static_ty != self.ctx.core.dynamic_ {
            self.mark_checked(value);
            return;
        }
        let Some(p) = types.and_then(|t| t.of_argument(name, i_pos)).cloned() else { return };
        if top(self.ctx, p.ty) {
            self.mark_checked(value);
        } else if closed(self.ctx, p.ty, p.subst.as_ref()) {
            let r = self.substituted_type_recipe(p.ty, p.subst.as_ref());
            let t = self.recipe_rti(&r);
            self.cast_rti_in(value.clone(), t, super::rti::CastContext::Implicit);
            self.mark_checked(value);
        }
    }

    /// O tipo de `value` é garantido pelo estático (ver `checked_args`).
    pub(super) fn mark_checked(&mut self, value: &Operand) {
        if let Operand::Val(id) = value {
            self.checked_args.insert(*id);
        }
    }

    /// `value`, resultado de `expr`, tem o tipo garantido pelo estático
    /// quando a expressão não é `dynamic` (operandos de operador e de
    /// índice, que não passam pelo `evaluate_args`).
    pub(super) fn mark_if_typed(&mut self, expr: ast::ExprId, value: &Operand) {
        if self.ctx.get_type(self.unit_id, expr).is_some_and(|t| t != self.ctx.core.dynamic_) {
            self.mark_checked(value);
        }
    }

    /// Todos os argumentos têm o tipo garantido (constantes ou marcados):
    /// a chamada pode usar a entrada tipada.
    pub(super) fn all_checked(&self, evaluated: &[Evaluated]) -> bool {
        // Um escalar sem caixa (`i64`, `double`, `bool`) já diz o tipo.
        evaluated.iter().all(|(_, v)| match v {
            Operand::Constant(_) => true,
            Operand::Val(id) => self.checked_args.contains(id) || self.operand_type(v) != Type::Ref,
        })
    }

    /// Antes da chamada direta ao membro `alvo` do programa (despacho de
    /// `call_member`): cada argumento de parâmetro covariante de `alvo`
    /// conferido contra o tipo declarado nele — avaliado com o receptor
    /// `recv`, cujos argumentos de tipo valem para os da classe —, com a
    /// mensagem da VM (" of 'nome'"), antes de converter o argumento à
    /// representação do parâmetro. É a conferência que a VM faz na entrada
    /// do método; aqui a chamada é direta, então ela vai no ponto de
    /// chamada, por implementação.
    pub(super) fn check_target_covariants(&mut self, alvo: usize, recv: &Operand, evaluated: &[Evaluated]) {
        if !super::user_function(self.ctx, alvo) {
            return;
        }
        let Some(o) = self.ctx.outline.functions.get(alvo) else { return };
        if !o.type_params.is_empty() {
            return;
        }
        let params = o.parameters.clone();
        let mut positionals = evaluated.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| v.clone());
        for (i, p) in params.iter().enumerate() {
            let v = if p.kind == ParameterKind::Named {
                evaluated.iter().find(|(n, _)| n.is_some() && *n == p.externo).map(|(_, v)| v.clone())
            } else {
                positionals.next()
            };
            let Some(v) = v else { continue };
            if top(self.ctx, p.ty) || !covariant_parameter(self.ctx, alvo, i) || self.is_terminated() {
                continue;
            }
            // Um escalar sem caixa na representação do parâmetro já é dele.
            let repr = self.repr(p.ty);
            if repr != Type::Ref && self.operand_type(&v) == repr {
                continue;
            }
            let r = self.coerce(recv.clone(), Type::Ref);
            let saved = (
                self.this_param.replace(r),
                std::mem::replace(&mut self.enclosing_class, self.ctx.program.functions[alvo].class),
                std::mem::replace(&mut self.class_by_tuple, false),
                self.type_tuple.take(),
            );
            let typ = self.type_rti(p.ty);
            (self.this_param, self.enclosing_class, self.class_by_tuple, self.type_tuple) = saved;
            let name = p.name.map(|n| self.ctx.symbol_name(n).to_string()).unwrap_or_default();
            let v = self.coerce(v, Type::Ref);
            self.cast_rti_in(v, typ, super::rti::CastContext::Parametro(name));
        }
    }

    /// O membro `name` na interface da classe `c` (declarado nela ou num
    /// supertipo).
    fn interface_member(&self, c: ClassId, name: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(name)?;
        if let Some(f) = self.ctx.program.classes[c.0 as usize].instance_members.get(&sym) {
            return Some(f.0 as usize);
        }
        let data = self.ctx.outline.hierarchy.get(c)?;
        let mut supers: Vec<ClassId> = data.supertypes.keys().copied().filter(|s| *s != c).collect();
        supers.sort_by_key(|s| s.0);
        supers.into_iter().find_map(|s| self.ctx.program.classes[s.0 as usize].instance_members.get(&sym).map(|f| f.0 as usize))
    }

    /// O argumento `value` (expressão `arg`) do `i`-ésimo parâmetro do
    /// operador `name` (`[]`, `[]=`, `+`…) sobre o receptor `receiver`:
    /// se ele é `dynamic`, o cast implícito para o tipo do parâmetro visto
    /// pelo tipo estático do receptor (os argumentos de tipo dele trocados
    /// pelos parâmetros da classe que declara o operador), como o CFE; e a
    /// marca de conferido quando o tipo fica garantido.
    pub(super) fn implicit_operator_cast(
        &mut self,
        arg: ast::ExprId,
        value: &Operand,
        receiver: ast::ExprId,
        name: &str,
        i: usize,
    ) {
        let Some(static_ty) = self.ctx.get_type(self.unit_id, arg) else { return };
        if static_ty != self.ctx.core.dynamic_ {
            self.mark_checked(value);
            return;
        }
        let Some(rt) = self.ctx.get_type(self.unit_id, receiver) else { return };
        let T::Interface { class: rc, args: rargs, .. } = self.ctx.table.get(rt).clone() else { return };
        let Some(fid) = self.interface_member(rc, name) else { return };
        let Some(p) = self.ctx.outline.functions.get(fid).and_then(|o| o.parameters.get(i)).cloned() else { return };
        if top(self.ctx, p.ty) {
            self.mark_checked(value);
            return;
        }
        let mut subst: HashMap<TypeParamId, TypeId> = HashMap::new();
        if let Some(data) = self.ctx.outline.classes.get(rc.0 as usize) {
            subst.extend(data.type_params.iter().copied().zip(rargs.iter().copied()));
        }
        if let Some(dc) = self.ctx.program.functions[fid].class
            && dc != rc
            && let Some(data) = self.ctx.outline.classes.get(dc.0 as usize)
            && let Some(&sup) = self.ctx.outline.hierarchy.get(rc).and_then(|h| h.supertypes.get(&dc))
            && let T::Interface { args: sargs, .. } = self.ctx.table.get(sup)
        {
            subst.extend(data.type_params.iter().copied().zip(sargs.iter().copied()));
        }
        if !closed(self.ctx, p.ty, Some(&subst)) {
            return;
        }
        let r = self.substituted_type_recipe(p.ty, Some(&subst));
        let t = self.recipe_rti(&r);
        self.cast_rti_in(value.clone(), t, super::rti::CastContext::Implicit);
        self.mark_checked(value);
    }

    /// A implementação única do membro de instância `fid` de uma classe
    /// do SDK cujo membro é fechado (só a biblioteca dela o implementa), com
    /// os mesmos parâmetros; `None` sem ela.
    pub(super) fn single_implementation(&self, fid: usize) -> Option<usize> {
        let f = &self.ctx.program.functions[fid];
        let cid = f.class?;
        if f.static_ || !self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk {
            return None;
        }
        let name = self.ctx.symbol_name(f.name).to_string();
        if !super::sdk_source::closed_member(self.ctx, cid, &name) {
            return None;
        }
        let [super::sdk_source::Implementation::Function(g)] = super::sdk_source::implementations(self.ctx, cid, &name)[..] else {
            return None;
        };
        let n = |x: usize| self.ctx.outline.functions.get(x).map(|o| o.parameters.len());
        (n(g) == n(fid)).then_some(g)
    }
}
