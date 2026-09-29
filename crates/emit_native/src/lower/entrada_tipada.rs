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
//! ([`precisa_entrada_tipada`]) — e o runtime (`dartforge_seletor`) cai no
//! seletor sem o `t` quando a classe não tem a tipada (métodos sem o que
//! dispensar, encaminhadores de `noSuchMethod`, classes do programa). A
//! entrada tipada também não confere a aridade (`dartforge_args_casam`): a
//! sobrescrita válida aceita os argumentos da interface.
//!
//! O cast implícito dos argumentos `dynamic` (e a regra de quando a chamada
//! pode ser tipada) é o de [`FnBuilder::avaliar_args`] com os tipos
//! armados por [`FnBuilder::armar_tipos_dos_args`]: o tipo do parâmetro na
//! invocação, sem variáveis de tipo livres. Sem ele, o argumento fica em
//! `args_sem_cast` e a chamada volta ao seletor que confere tudo.

use super::fn_builder::FnBuilder;
use super::membros::Avaliado;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{FunctionRef, MemberKind};
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
pub struct TiposDaInvocacao {
    posicionais: Vec<TipoDoParametro>,
    nomeados: Vec<(SymbolId, TipoDoParametro)>,
}

/// O tipo de um parâmetro na invocação e, quando ele menciona parâmetros de
/// tipo da classe criada, a substituição que o fecha.
#[derive(Clone, Debug)]
struct TipoDoParametro {
    ty: TypeId,
    subst: Option<HashMap<TypeParamId, TypeId>>,
}

impl TiposDaInvocacao {
    fn do_argumento(&self, nome: Option<SymbolId>, i_pos: usize) -> Option<&TipoDoParametro> {
        match nome {
            Some(n) => self.nomeados.iter().find(|(m, _)| *m == n).map(|(_, t)| t),
            None => self.posicionais.get(i_pos),
        }
    }
}

/// `t` não menciona parâmetro de tipo (nem de classe, nem de função, nem os
/// ligados de um tipo de função genérico): a receita é constante e o cast
/// pode ser feito no ponto de chamada.
fn fechado(ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            subst.and_then(|s| s.get(param)).is_some_and(|&x| fechado(ctx, x, None))
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args.iter().all(|a| fechado(ctx, *a, subst)),
        T::FutureOr { arg, .. } => fechado(ctx, *arg, subst),
        T::Function { type_params, ret, positional, optional, named, .. } => {
            type_params.is_empty()
                && fechado(ctx, *ret, subst)
                && positional.iter().chain(optional.iter()).all(|p| fechado(ctx, *p, subst))
                && named.iter().all(|(_, p, _)| fechado(ctx, *p, subst))
        }
        T::Record { positional, named, .. } => {
            positional.iter().all(|p| fechado(ctx, *p, subst)) && named.iter().all(|(_, p)| fechado(ctx, *p, subst))
        }
        T::Dynamic | T::Void | T::Never | T::Null => true,
    }
}

/// O topo (`dynamic`, `void`, `Object?`): nada a conferir.
fn topo(ctx: &Context, t: TypeId) -> bool {
    t == ctx.core.dynamic_ || t == ctx.core.object_nullable || matches!(ctx.table.get(t), T::Void)
}

/// `t` menciona algum parâmetro de tipo da classe `c` (em qualquer posição).
fn menciona_da_classe(ctx: &Context, t: TypeId, c: dartforge_elements::model::ClassId) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            matches!(ctx.table.param(*param).owner, TypeParamOwner::Class(d) if d == c)
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args.iter().any(|a| menciona_da_classe(ctx, *a, c)),
        T::FutureOr { arg, .. } => menciona_da_classe(ctx, *arg, c),
        T::Function { ret, positional, optional, named, .. } => {
            menciona_da_classe(ctx, *ret, c)
                || positional.iter().chain(optional.iter()).any(|p| menciona_da_classe(ctx, *p, c))
                || named.iter().any(|(_, p, _)| menciona_da_classe(ctx, *p, c))
        }
        T::Record { positional, named, .. } => {
            positional.iter().any(|p| menciona_da_classe(ctx, *p, c)) || named.iter().any(|(_, p)| menciona_da_classe(ctx, *p, c))
        }
        T::Dynamic | T::Void | T::Never | T::Null => false,
    }
}

/// Os parâmetros de `fid` escritos `covariant` (pelo índice na declaração),
/// inclusive o setter implícito de um campo `covariant`.
fn declarados_covariantes(ctx: &Context, fid: usize) -> Vec<(usize, Option<SymbolId>)> {
    let f = &ctx.program.functions[fid];
    let mut saida = Vec::new();
    match f.node {
        FunctionRef::Function { unit, function } => {
            let af = ctx.program.unit(unit).ast.function(function);
            for (i, p) in af.parameters.as_deref().unwrap_or(&[]).iter().enumerate() {
                if p.covariant {
                    let nome = (p.kind == ParameterKind::Named).then(|| p.nome_externo().map(|n| n.sym)).flatten();
                    saida.push((i, nome));
                }
            }
        }
        FunctionRef::None => {
            if let Some(v) = f.variable
                && let dartforge_elements::model::VariableRef::Field { unit, member, .. } = ctx.program.variables[v.0 as usize].node
                && let MemberKind::Field(vl) = &ctx.program.unit(unit).ast.member(member).kind
                && vl.covariant
            {
                saida.push((0, None));
            }
        }
        FunctionRef::Constructor { .. } => {}
    }
    saida
}

/// O parâmetro `i` de `fid` é covariante pela palavra `covariant`: escrita
/// nele ou no correspondente (mesma posição, ou mesmo nome) de um membro
/// homônimo de algum supertipo da classe (`ParameterElement.isCovariant`).
fn covariante_por_declaracao(ctx: &Context, fid: usize, i: usize) -> bool {
    let f = &ctx.program.functions[fid];
    let Some(p) = ctx.outline.functions.get(fid).and_then(|o| o.parameters.get(i)) else { return false };
    let nome = (p.kind == ParameterKind::Named).then_some(p.externo).flatten();
    let casa = |g: usize| {
        declarados_covariantes(ctx, g).iter().any(|(j, n)| match nome {
            Some(x) => *n == Some(x),
            None => n.is_none() && *j == i,
        })
    };
    if casa(fid) {
        return true;
    }
    let Some(c) = f.class else { return false };
    let Some(dados) = ctx.outline.hierarchy.get(c) else { return false };
    dados.supertypes.keys().any(|&s| {
        s != c && ctx.program.classes[s.0 as usize].instance_members.get(&f.name).is_some_and(|g| casa(g.0 as usize))
    })
}

/// A entrada `$c` confere o parâmetro `ty` de `fid` (o mesmo critério de
/// [`FnBuilder::conferir_argumentos_da_entrada`]): tipo nominal (classe)
/// ou exatamente um parâmetro de tipo da classe, fora do topo.
pub fn conferido_na_entrada(ctx: &Context, fid: usize, ty: TypeId) -> bool {
    let f = &ctx.program.functions[fid];
    let covariante = matches!(ctx.table.get(ty), T::TypeParameter { param, .. }
        if matches!(ctx.table.param(*param).owner, TypeParamOwner::Class(c) if Some(c) == f.class));
    let nominal = matches!(ctx.table.get(ty), T::Interface { .. });
    !topo(ctx, ty) && (covariante || nominal)
}

/// O parâmetro `i` de `fid` precisa de conferência mesmo numa chamada
/// tipada: menciona um parâmetro de tipo da classe, ou é `covariant`.
pub fn parametro_covariante(ctx: &Context, fid: usize, i: usize) -> bool {
    let Some(p) = ctx.outline.functions.get(fid).and_then(|o| o.parameters.get(i)) else { return true };
    let f = &ctx.program.functions[fid];
    f.class.is_some_and(|c| menciona_da_classe(ctx, p.ty, c)) || covariante_por_declaracao(ctx, fid, i)
}

/// O método `fid` ganha a entrada tipada: a `$c` dele confere algum
/// parâmetro que a chamada tipada dispensa.
pub fn precisa_entrada_tipada(ctx: &Context, fid: usize) -> bool {
    let Some(o) = ctx.outline.functions.get(fid) else { return false };
    o.parameters.iter().enumerate().any(|(i, p)| conferido_na_entrada(ctx, fid, p.ty) && !parametro_covariante(ctx, fid, i))
}

/// O seletor da chamada tipada correspondente a `seletor` (`c:m` → `tc:m`).
pub fn seletor_tipado(seletor: &str) -> String {
    format!("t{seletor}")
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Arma os tipos dos parâmetros da invocação `expr_id` (alvo `target`)
    /// para o [`FnBuilder::avaliar_args`] dos argumentos `args`: o tipo
    /// estático do alvo, se é um tipo de função (o do membro visto pelo
    /// receptor, ou o da função), senão o do construtor resolvido.
    /// Devolve o que estava armado, para quem chama restaurar.
    pub(super) fn armar_tipos_dos_args(
        &mut self,
        target: ast::ExprId,
        expr_id: ast::ExprId,
        args: &[ast::Argument],
    ) -> Option<(usize, TiposDaInvocacao)> {
        let tipos = self.tipos_do_alvo(target, expr_id);
        let chave = args.as_ptr() as usize;
        std::mem::replace(&mut self.tipos_dos_args, tipos.map(|t| (chave, t)))
    }

    /// Arma os tipos dos parâmetros do construtor `fid` de uma criação cujo
    /// tipo estático é `tipo` (os argumentos de tipo da classe substituem os
    /// parâmetros dela).
    pub(super) fn armar_tipos_do_construtor(
        &mut self,
        fid: usize,
        tipo: Option<TypeId>,
        args: &[ast::Argument],
    ) -> Option<(usize, TiposDaInvocacao)> {
        let tipos = self.tipos_da_funcao(fid, tipo);
        let chave = args.as_ptr() as usize;
        std::mem::replace(&mut self.tipos_dos_args, tipos.map(|t| (chave, t)))
    }

    fn tipos_do_alvo(&self, target: ast::ExprId, expr_id: ast::ExprId) -> Option<TiposDaInvocacao> {
        if let Some(t) = self.ctx.get_type(self.unit_id, target)
            && let T::Function { type_params, positional, optional, named, .. } = self.ctx.table.get(t)
        {
            if !type_params.is_empty() {
                return None;
            }
            let sem = |ty: &TypeId| TipoDoParametro { ty: *ty, subst: None };
            return Some(TiposDaInvocacao {
                posicionais: positional.iter().chain(optional.iter()).map(sem).collect(),
                nomeados: named.iter().map(|(n, ty, _)| (*n, sem(ty))).collect(),
            });
        }
        let resolvido = self
            .ctx
            .get_resolved(self.unit_id, expr_id)
            .or_else(|| self.ctx.get_resolved(self.unit_id, target))
            .cloned();
        let fid = match resolvido {
            Some(Resolved::Constructor(f)) => f.0 as usize,
            Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => f.0 as usize,
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => f.0 as usize,
            _ => return None,
        };
        let tipo = self.ctx.get_type(self.unit_id, expr_id);
        self.tipos_da_funcao(fid, tipo)
    }

    /// Os tipos dos parâmetros de `fid`; num construtor (ou fábrica) de
    /// classe genérica, com os argumentos de tipo de `criado`.
    fn tipos_da_funcao(&self, fid: usize, criado: Option<TypeId>) -> Option<TiposDaInvocacao> {
        let o = self.ctx.outline.functions.get(fid)?;
        if !o.type_params.is_empty() {
            return None;
        }
        let f = &self.ctx.program.functions[fid];
        let mut subst = None;
        if let Some(c) = f.class
            && matches!(f.node, FunctionRef::Constructor { .. })
            && let Some(dados) = self.ctx.outline.classes.get(c.0 as usize)
            && !dados.type_params.is_empty()
            && let Some(t) = criado
            && let T::Interface { class, args, .. } = self.ctx.table.get(t)
            && *class == c
            && args.len() == dados.type_params.len()
        {
            subst = Some(dados.type_params.iter().copied().zip(args.iter().copied()).collect::<HashMap<_, _>>());
        }
        let mut t = TiposDaInvocacao::default();
        for p in o.parameters.iter() {
            let x = TipoDoParametro { ty: p.ty, subst: subst.clone() };
            if p.kind == ParameterKind::Named {
                if let Some(n) = p.externo {
                    t.nomeados.push((n, x));
                }
            } else {
                t.posicionais.push(x);
            }
        }
        Some(t)
    }

    /// Depois de avaliar o argumento `valor` (expressão `expr`, nome
    /// `nome`, `i_pos`-ésimo posicional): se o tipo estático dele é
    /// `dynamic` (ou desconhecido), o cast implícito para o tipo do
    /// parâmetro na invocação armada, ou a marca de que ficou sem ele.
    pub(super) fn cast_implicito_do_argumento(
        &mut self,
        tipos: Option<&TiposDaInvocacao>,
        expr: ast::ExprId,
        nome: Option<SymbolId>,
        i_pos: usize,
        valor: &Operand,
    ) {
        let estatico = self.ctx.get_type(self.unit_id, expr);
        if estatico.is_some_and(|t| t != self.ctx.core.dynamic_) {
            return;
        }
        let alvo = tipos.and_then(|t| t.do_argumento(nome, i_pos)).cloned();
        match (estatico, alvo) {
            (_, Some(p)) if topo(self.ctx, p.ty) => {}
            (Some(_), Some(p)) if fechado(self.ctx, p.ty, p.subst.as_ref()) => {
                let r = self.receita_de_tipo_substituida(p.ty, p.subst.as_ref());
                let t = self.rti_da_receita(&r);
                self.cast_rti_em(valor.clone(), t, super::rti::ContextoDoCast::Implicito);
            }
            _ => {
                if let Operand::Val(id) = valor {
                    self.args_sem_cast.insert(*id);
                }
            }
        }
    }

    /// Algum dos argumentos avaliados ficou sem o cast implícito (ou sem
    /// tipo estático): a chamada não pode usar a entrada tipada.
    pub(super) fn algum_sem_cast(&self, avaliados: &[Avaliado]) -> bool {
        avaliados.iter().any(|(_, v)| matches!(v, Operand::Val(id) if self.args_sem_cast.contains(id)))
    }
}
