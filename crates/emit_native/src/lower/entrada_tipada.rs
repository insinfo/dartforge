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
//! invocação, sem variáveis de tipo livres. Só a chamada cujos argumentos
//! têm todos o tipo garantido (`args_conferidos`) usa o seletor tipado; a
//! outra volta ao que confere tudo.

use super::fn_builder::FnBuilder;
use super::membros::Avaliado;
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
    fechado_ate(ctx, t, subst, 8)
}

fn fechado_ate(ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>, prof: u32) -> bool {
    let fechado = |ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>| fechado_ate(ctx, t, subst, prof);
    match ctx.table.get(t) {
        // A troca pode levar a outro parâmetro (o do supertipo ao da classe
        // do receptor), trocado pelo mesmo mapa.
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            prof > 0 && subst.and_then(|s| s.get(param)).is_some_and(|&x| fechado_ate(ctx, x, subst, prof - 1))
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

/// `t` menciona um parâmetro de tipo da classe `c` numa posição covariante
/// (ou invariante): o que torna o parâmetro de tipo `t` *genérico
/// covariante* (`isGenericCovariantImpl` do kernel). `positivo` é a
/// polaridade corrente: o parâmetro de um tipo de função a inverte.
fn menciona_covariante(ctx: &Context, t: TypeId, c: ClassId, positivo: bool) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            positivo && matches!(ctx.table.param(*param).owner, TypeParamOwner::Class(d) if d == c)
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args.iter().any(|a| menciona_covariante(ctx, *a, c, positivo)),
        T::FutureOr { arg, .. } => menciona_covariante(ctx, *arg, c, positivo),
        T::Function { ret, positional, optional, named, .. } => {
            menciona_covariante(ctx, *ret, c, positivo)
                || positional.iter().chain(optional.iter()).any(|p| menciona_covariante(ctx, *p, c, !positivo))
                || named.iter().any(|(_, p, _)| menciona_covariante(ctx, *p, c, !positivo))
        }
        T::Record { positional, named, .. } => {
            positional.iter().any(|p| menciona_covariante(ctx, *p, c, positivo))
                || named.iter().any(|(_, p)| menciona_covariante(ctx, *p, c, positivo))
        }
        T::Dynamic | T::Void | T::Never | T::Null => false,
    }
}

/// O parâmetro de `g` que corresponde ao `i`-ésimo de `fid` (mesma posição
/// entre os posicionais, ou mesmo nome entre os nomeados).
fn correspondente(ctx: &Context, fid: usize, i: usize, g: usize) -> Option<usize> {
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
fn covariante_em(ctx: &Context, g: usize, j: usize) -> bool {
    let Some(q) = ctx.outline.functions.get(g).and_then(|o| o.parameters.get(j)) else { return false };
    let nome = (q.kind == ParameterKind::Named).then_some(q.externo).flatten();
    let escrito = declarados_covariantes(ctx, g).iter().any(|(k, n)| match nome {
        Some(x) => *n == Some(x),
        None => n.is_none() && *k == j,
    });
    escrito || ctx.program.functions[g].class.is_some_and(|c| menciona_covariante(ctx, q.ty, c, true))
}

/// O parâmetro `i` de `fid` é covariante: no próprio membro, ou porque o
/// correspondente de um membro homônimo de algum supertipo da classe o é
/// (a covariância se herda pela sobrescrita: `ParameterElement.isCovariant`
/// e o `isGenericCovariantImpl` que o CFE propaga).
fn covariante_herdado(ctx: &Context, fid: usize, i: usize) -> bool {
    if covariante_em(ctx, fid, i) {
        return true;
    }
    let f = &ctx.program.functions[fid];
    let Some(c) = f.class else { return false };
    let Some(dados) = ctx.outline.hierarchy.get(c) else { return false };
    let nome = ctx.symbol_name(f.name);
    let chave = if f.kind == dartforge_elements::model::FunctionKind::Setter && !nome.ends_with("_=") {
        ctx.interner.lookup(&format!("{nome}_="))
    } else {
        Some(f.name)
    };
    let Some(chave) = chave else { return false };
    dados.supertypes.keys().any(|&s| {
        s != c
            && ctx.program.classes[s.0 as usize].instance_members.get(&chave).is_some_and(|g| {
                let g = g.0 as usize;
                correspondente(ctx, fid, i, g).is_some_and(|j| covariante_em(ctx, g, j))
            })
    })
}

/// A entrada `$c` confere o parâmetro `ty` de `fid`
/// ([`FnBuilder::conferir_argumentos_da_entrada`]): tipo nominal (classe),
/// ou um tipo que menciona parâmetro de tipo da classe em posição
/// covariante (`T`, `FutureOr<T>`, `T Function()`), fora do topo. Os
/// outros tipos de função e os parâmetros de tipo da função ficam com a
/// conversão de sempre.
pub fn conferido_na_entrada(ctx: &Context, fid: usize, ty: TypeId) -> bool {
    let f = &ctx.program.functions[fid];
    let generico = f.class.is_some_and(|c| menciona_covariante(ctx, ty, c, true));
    let nominal = matches!(ctx.table.get(ty), T::Interface { .. });
    !topo(ctx, ty) && (generico || nominal)
}

/// O parâmetro `i` de `fid` precisa de conferência mesmo numa chamada
/// tipada: é covariante (ver [`covariante_herdado`]).
pub fn parametro_covariante(ctx: &Context, fid: usize, i: usize) -> bool {
    covariante_herdado(ctx, fid, i)
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
    /// `dynamic`, o cast implícito para o tipo do parâmetro na invocação
    /// armada; e a marca de conferido quando o tipo fica garantido.
    pub(super) fn cast_implicito_do_argumento(
        &mut self,
        tipos: Option<&TiposDaInvocacao>,
        expr: ast::ExprId,
        nome: Option<SymbolId>,
        i_pos: usize,
        valor: &Operand,
    ) {
        let Some(estatico) = self.ctx.get_type(self.unit_id, expr) else { return };
        if estatico != self.ctx.core.dynamic_ {
            self.marcar_conferido(valor);
            return;
        }
        let Some(p) = tipos.and_then(|t| t.do_argumento(nome, i_pos)).cloned() else { return };
        if topo(self.ctx, p.ty) {
            self.marcar_conferido(valor);
        } else if fechado(self.ctx, p.ty, p.subst.as_ref()) {
            let r = self.receita_de_tipo_substituida(p.ty, p.subst.as_ref());
            let t = self.rti_da_receita(&r);
            self.cast_rti_em(valor.clone(), t, super::rti::ContextoDoCast::Implicito);
            self.marcar_conferido(valor);
        }
    }

    /// O tipo de `valor` é garantido pelo estático (ver `args_conferidos`).
    pub(super) fn marcar_conferido(&mut self, valor: &Operand) {
        if let Operand::Val(id) = valor {
            self.args_conferidos.insert(*id);
        }
    }

    /// `valor`, resultado de `expr`, tem o tipo garantido pelo estático
    /// quando a expressão não é `dynamic` (operandos de operador e de
    /// índice, que não passam pelo `avaliar_args`).
    pub(super) fn marcar_se_tipado(&mut self, expr: ast::ExprId, valor: &Operand) {
        if self.ctx.get_type(self.unit_id, expr).is_some_and(|t| t != self.ctx.core.dynamic_) {
            self.marcar_conferido(valor);
        }
    }

    /// Todos os argumentos têm o tipo garantido (constantes ou marcados):
    /// a chamada pode usar a entrada tipada.
    pub(super) fn todos_conferidos(&self, avaliados: &[Avaliado]) -> bool {
        // Um escalar sem caixa (`i64`, `double`, `bool`) já diz o tipo.
        avaliados.iter().all(|(_, v)| match v {
            Operand::Constant(_) => true,
            Operand::Val(id) => self.args_conferidos.contains(id) || self.operand_type(v) != Type::Ref,
        })
    }

    /// Antes da chamada direta ao membro `alvo` do programa (despacho de
    /// `chamar_membro`): cada argumento de parâmetro covariante de `alvo`
    /// conferido contra o tipo declarado nele — avaliado com o receptor
    /// `recv`, cujos argumentos de tipo valem para os da classe —, com a
    /// mensagem da VM (" of 'nome'"), antes de converter o argumento à
    /// representação do parâmetro. É a conferência que a VM faz na entrada
    /// do método; aqui a chamada é direta, então ela vai no ponto de
    /// chamada, por implementação.
    pub(super) fn conferir_covariantes_do_alvo(&mut self, alvo: usize, recv: &Operand, avaliados: &[Avaliado]) {
        if !super::funcao_do_usuario(self.ctx, alvo) {
            return;
        }
        let Some(o) = self.ctx.outline.functions.get(alvo) else { return };
        if !o.type_params.is_empty() {
            return;
        }
        let params = o.parameters.clone();
        let mut posicionais = avaliados.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| v.clone());
        for (i, p) in params.iter().enumerate() {
            let v = if p.kind == ParameterKind::Named {
                avaliados.iter().find(|(n, _)| n.is_some() && *n == p.externo).map(|(_, v)| v.clone())
            } else {
                posicionais.next()
            };
            let Some(v) = v else { continue };
            if topo(self.ctx, p.ty) || !parametro_covariante(self.ctx, alvo, i) || self.is_terminated() {
                continue;
            }
            // Um escalar sem caixa na representação do parâmetro já é dele.
            let repr = self.repr(p.ty);
            if repr != Type::Ref && self.operand_type(&v) == repr {
                continue;
            }
            let r = self.coagir(recv.clone(), Type::Ref);
            let salvo = (
                self.this_param.replace(r),
                std::mem::replace(&mut self.enclosing_class, self.ctx.program.functions[alvo].class),
                std::mem::replace(&mut self.classe_por_tupla, false),
                self.tupla_de_tipos.take(),
            );
            let tipo = self.rti_de_tipo(p.ty);
            (self.this_param, self.enclosing_class, self.classe_por_tupla, self.tupla_de_tipos) = salvo;
            let nome = p.name.map(|n| self.ctx.symbol_name(n).to_string()).unwrap_or_default();
            let v = self.coagir(v, Type::Ref);
            self.cast_rti_em(v, tipo, super::rti::ContextoDoCast::Parametro(nome));
        }
    }

    /// O membro `nome` na interface da classe `c` (declarado nela ou num
    /// supertipo).
    fn membro_da_interface(&self, c: ClassId, nome: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(nome)?;
        if let Some(f) = self.ctx.program.classes[c.0 as usize].instance_members.get(&sym) {
            return Some(f.0 as usize);
        }
        let dados = self.ctx.outline.hierarchy.get(c)?;
        let mut supers: Vec<ClassId> = dados.supertypes.keys().copied().filter(|s| *s != c).collect();
        supers.sort_by_key(|s| s.0);
        supers.into_iter().find_map(|s| self.ctx.program.classes[s.0 as usize].instance_members.get(&sym).map(|f| f.0 as usize))
    }

    /// O argumento `valor` (expressão `arg`) do `i`-ésimo parâmetro do
    /// operador `nome` (`[]`, `[]=`, `+`…) sobre o receptor `receptor`:
    /// se ele é `dynamic`, o cast implícito para o tipo do parâmetro visto
    /// pelo tipo estático do receptor (os argumentos de tipo dele trocados
    /// pelos parâmetros da classe que declara o operador), como o CFE; e a
    /// marca de conferido quando o tipo fica garantido.
    pub(super) fn cast_implicito_de_operador(
        &mut self,
        arg: ast::ExprId,
        valor: &Operand,
        receptor: ast::ExprId,
        nome: &str,
        i: usize,
    ) {
        let Some(estatico) = self.ctx.get_type(self.unit_id, arg) else { return };
        if estatico != self.ctx.core.dynamic_ {
            self.marcar_conferido(valor);
            return;
        }
        let Some(rt) = self.ctx.get_type(self.unit_id, receptor) else { return };
        let T::Interface { class: rc, args: rargs, .. } = self.ctx.table.get(rt).clone() else { return };
        let Some(fid) = self.membro_da_interface(rc, nome) else { return };
        let Some(p) = self.ctx.outline.functions.get(fid).and_then(|o| o.parameters.get(i)).cloned() else { return };
        if topo(self.ctx, p.ty) {
            self.marcar_conferido(valor);
            return;
        }
        let mut subst: HashMap<TypeParamId, TypeId> = HashMap::new();
        if let Some(dados) = self.ctx.outline.classes.get(rc.0 as usize) {
            subst.extend(dados.type_params.iter().copied().zip(rargs.iter().copied()));
        }
        if let Some(dc) = self.ctx.program.functions[fid].class
            && dc != rc
            && let Some(dados) = self.ctx.outline.classes.get(dc.0 as usize)
            && let Some(&sup) = self.ctx.outline.hierarchy.get(rc).and_then(|h| h.supertypes.get(&dc))
            && let T::Interface { args: sargs, .. } = self.ctx.table.get(sup)
        {
            subst.extend(dados.type_params.iter().copied().zip(sargs.iter().copied()));
        }
        if !fechado(self.ctx, p.ty, Some(&subst)) {
            return;
        }
        let r = self.receita_de_tipo_substituida(p.ty, Some(&subst));
        let t = self.rti_da_receita(&r);
        self.cast_rti_em(valor.clone(), t, super::rti::ContextoDoCast::Implicito);
        self.marcar_conferido(valor);
    }
}
