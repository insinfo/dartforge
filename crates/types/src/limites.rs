//! `TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` dos tipos escritos: o
//! `TypeArgumentsVerifier._checkForTypeArgumentNotMatchingBounds` do analyzer
//! 7.x (`src/error/type_arguments_verifier.dart`).
//!
//! Para cada `C<A1, …, An>` escrito (classe ou alias de tipo), cada
//! argumento precisa ser subtipo do limite do parâmetro com os argumentos
//! substituídos (*regular-bounded*). Se não for, e o lugar aceita tipo
//! *super-bounded* (não é `extends`/`with`/`implements`/`on`, criação de
//! instância, corpo de `typedef` nem tipo de extensão), o tipo invertido
//! (`invertido`: topo por `Never` nas posições não contravariantes, fundo
//! por `Object?` nas contravariantes) é conferido do mesmo jeito, e são os
//! argumentos e limites invertidos que o diagnóstico mostra.
//!
//! O tipo de cada `NamedType` é o resolvido (`NamedType.type`): o
//! `tipos_escritos` do esboço ou o `tipos_de_anotacoes` dos corpos, com os
//! tipos crus instanciados para os limites. Os tipos nomeados que o
//! analyzer cria a partir de expressões (literal de tipo `C<T>`, criação
//! `C<T>()` sem `new`, referência de construtor `C<T>.new`) entram pela
//! árvore de expressões. Um nó que nenhuma resolução registrou cai na
//! resolução no escopo da biblioteca, que pula nomes de parâmetro de tipo.

use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Diagnostic, Span};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, ParameterKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Variância de uma posição (`Variance` do analyzer).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variancia {
    Nenhuma,
    Co,
    Contra,
    In,
}

impl Variancia {
    /// `Variance.combine`: o sinal da posição dentro de outra.
    fn combinar(self, outra: Variancia) -> Variancia {
        use Variancia::*;
        match (self, outra) {
            (Nenhuma, _) | (_, Nenhuma) => Nenhuma,
            (In, _) | (_, In) => In,
            (a, b) if a == b => Co,
            _ => Contra,
        }
    }

    /// `Variance.meet`: duas ocorrências do mesmo parâmetro.
    fn juntar(self, outra: Variancia) -> Variancia {
        use Variancia::*;
        match (self, outra) {
            (Nenhuma, x) | (x, Nenhuma) => x,
            (a, b) if a == b => a,
            _ => In,
        }
    }
}

/// O elemento de um tipo nomeado e os argumentos dele.
struct Partes {
    nome: String,
    params: Vec<TypeParamId>,
    tipos: Vec<TypeId>,
    de_extensao: bool,
    /// O alvo do alias (as variâncias dos parâmetros na inversão).
    alvo_do_alias: Option<TypeId>,
}

struct Verificador<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    unit: UnitId,
    /// Nomes de parâmetros de tipo declarados em algum lugar da unidade.
    nomes_de_parametros: HashSet<SymbolId>,
    /// Os tipos resolvidos dos corpos da unidade.
    corpo: Option<&'a crate::resolved::UnitBodyTypes>,
}

/// Os `type_argument_not_matching_bounds` dos tipos escritos numa unidade.
pub fn argumentos_fora_dos_limites(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: Option<&crate::resolved::UnitBodyTypes>,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verificador { program, interner, table, core, outline, unit, nomes_de_parametros: HashSet::new(), corpo };
    v.nomes_de_parametros = nomes_de_parametros(a, &program.unit(unit).unit);
    let sem_super = sem_super_limite(a, &program.unit(unit).unit);
    // A criação sem argumentos escritos é conferida com os inferidos
    // (`argumentos_inferidos_fora_dos_limites`).
    let criacoes_cruas: HashSet<ast::TypeId> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::InstanceCreation { ty, .. } if matches!(&a.ty(*ty).kind, TypeKind::Named { args, .. } if args.is_empty()) => Some(*ty),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (i, t) in a.types.iter().enumerate() {
        if let TypeKind::Named { name, args } = &t.kind {
            let id = ast::TypeId(i as u32);
            if t.span.start == t.span.end || criacoes_cruas.contains(&id) {
                continue;
            }
            let super_permitido = !sem_super.contains(&id);
            match v.registrado(id) {
                Some(r) => v.conferir_tipo(r, t.span, args, super_permitido, &mut out),
                None if !args.is_empty() => v.conferir(t.span, name, args, super_permitido, &mut out),
                None => {}
            }
        }
    }
    if corpo.is_some() {
        v.tipos_nomeados_de_expressoes(&mut out);
    }
    out
}

/// `ReplaceTopBottomVisitor.process` sobre um tipo já resolvido: topo vira
/// `Never` fora das posições contravariantes; o que é subtipo de `Never` vira
/// `Object?` nas contravariantes. Um alias leva cada argumento com a
/// variância do parâmetro dele no alvo combinada com a de fora; classes (e
/// `FutureOr`, tipos de extensão) levam a variância adiante; funções invertem
/// nos parâmetros (os parâmetros de tipo ficam).
fn trocar_topo_e_fundo(v: &mut Verificador<'_>, t: TypeId, va: Variancia) -> TypeId {
    if va == Variancia::Contra {
        let never = v.core.never;
        if v.sub(t, never) {
            return v.core.object_nullable;
        }
    } else if v.e_topo(t) {
        return v.core.never;
    }
    let anulavel = v.table.get(t).is_declared_nullable();
    if let Some(crate::table::Exibicao::Alias { typedef, args }) = v.table.exibicao(t).cloned() {
        let d = &v.outline.typedefs[typedef.0 as usize];
        let (params, alvo) = (d.type_params.to_vec(), d.target_type);
        if params.len() == args.len() {
            let mut novos = Vec::with_capacity(args.len());
            for (&x, &p) in args.iter().zip(params.iter()) {
                let vp = variancia_em(v.table, alvo, p, Variancia::Co);
                novos.push(trocar_topo_e_fundo(v, x, vp.combinar(va)));
            }
            let r = v.subst(alvo, &params, &novos);
            let r = if anulavel { crate::ops::nullable(r, v.table) } else { r };
            return v.table.decorar(r, crate::table::Exibicao::Alias { typedef, args: novos.into_boxed_slice() });
        }
    }
    match v.table.get(t).clone() {
        Type::Interface { class, args, nullable } if !args.is_empty() => {
            let novos: Vec<TypeId> = args.iter().map(|&x| trocar_topo_e_fundo(v, x, va)).collect();
            v.table.intern(Type::Interface { class, args: novos.into_boxed_slice(), nullable })
        }
        Type::ExtensionType { decl, args, nullable } if !args.is_empty() => {
            let novos: Vec<TypeId> = args.iter().map(|&x| trocar_topo_e_fundo(v, x, va)).collect();
            v.table.intern(Type::ExtensionType { decl, args: novos.into_boxed_slice(), nullable })
        }
        Type::FutureOr { arg, nullable } => {
            let arg = trocar_topo_e_fundo(v, arg, va);
            v.table.intern(Type::FutureOr { arg, nullable })
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let ret = trocar_topo_e_fundo(v, ret, va);
            let contra = va.combinar(Variancia::Contra);
            let positional: Vec<TypeId> = positional.iter().map(|&x| trocar_topo_e_fundo(v, x, contra)).collect();
            let optional: Vec<TypeId> = optional.iter().map(|&x| trocar_topo_e_fundo(v, x, contra)).collect();
            let named: Vec<(SymbolId, TypeId, bool)> = named.iter().map(|&(n, x, r)| (n, trocar_topo_e_fundo(v, x, contra), r)).collect();
            v.table.intern(Type::Function {
                type_params,
                ret,
                positional: positional.into_boxed_slice(),
                optional: optional.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable,
            })
        }
        _ => t,
    }
}

/// `enum_instantiated_to_bounds_is_not_well_bounded`
/// (`ErrorVerifier._checkForEnumInstantiatedToBoundsIsNotWellBounded`,
/// `analyzer/lib/src/generated/error_verifier.dart:3239-3256`): o argumento do
/// tipo do campo `values` (`List<E<instanciado aos limites>>`) passa por
/// `isWellBounded(…, allowSuperBounded: true)`, que só confere o invertido
/// (`_isSuperBounded`: o resultado regular é descartado). No nome do enum.
pub fn enum_instanciado_aos_limites(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for (i, ce) in program.classes.iter().enumerate() {
        if ce.library != lib || ce.kind != dartforge_elements::model::ClassKind::Enum {
            continue;
        }
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let decl = a.decl(d.decl);
        let DeclKind::Enum(x) = &decl.kind else { continue };
        if decl.augment {
            continue;
        }
        let params = outline.classes[i].type_params.to_vec();
        if params.is_empty() {
            continue;
        }
        let args = crate::ops::instanciar_para_limites(&params, &[], table, core);
        let mut v = Verificador { program, interner, table: &mut *table, core, outline, unit: d.unit, nomes_de_parametros: HashSet::new(), corpo: None };
        let invertidos: Vec<TypeId> = args.iter().map(|&t| trocar_topo_e_fundo(&mut v, t, Variancia::Co)).collect();
        let mut bem_limitado = true;
        for (k, &p) in params.iter().enumerate() {
            let dados = v.table.param(p).clone();
            if !dados.explicito {
                continue;
            }
            let limite = v.subst(dados.bound, &params, &invertidos);
            if !v.sub(invertidos[k], limite) {
                bem_limitado = false;
                break;
            }
        }
        if !bem_limitado {
            saida.push((d.unit, Diagnostic::com_codigo(c::ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED, x.name.span, Vec::<String>::new())));
        }
    }
    saida
}

/// Os `type_argument_not_matching_bounds` dos argumentos de tipo
/// **inferidos** de uma criação de instância sem argumentos escritos
/// (`C(x)`, `new C.nome(x)`): o analyzer confere o `NamedType` do
/// construtor com o tipo já inferido (`checkNamedType`,
/// `an611:src/error/type_arguments_verifier.dart`), no nome da classe.
/// Nunca super-bounded (criação de instância).
pub fn argumentos_inferidos_fora_dos_limites(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &crate::resolved::UnitBodyTypes,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verificador { program, interner, table, core, outline, unit, nomes_de_parametros: HashSet::new(), corpo: Some(corpo) };
    let mut out = Vec::new();
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        let (span, args) = match &e.kind {
            ExprKind::InstanceCreation { ty, arguments, .. } => {
                let TypeKind::Named { args: escritos, .. } = &a.ty(*ty).kind else { continue };
                if !escritos.is_empty() {
                    continue;
                }
                (a.ty(*ty).span, arguments)
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() {
                    continue;
                }
                let sp = match &a.expr(*target).kind {
                    ExprKind::Identifier(_) => a.expr(*target).span,
                    // `C.nome(x)`: o nome da classe; `p.C(x)`: o nome inteiro.
                    ExprKind::Property { target: t, .. } => match &a.expr(*t).kind {
                        ExprKind::Identifier(_) => a.expr(*t).span,
                        _ => continue,
                    },
                    _ => continue,
                };
                (sp, arguments)
            }
            _ => continue,
        };
        let Some(crate::resolved::Resolved::Constructor(f)) = corpo.get_resolved(id) else { continue };
        let Some(c) = program.function(*f).class else { continue };
        let params = outline.classes[c.0 as usize].type_params.to_vec();
        if params.is_empty() {
            continue;
        }
        let Some(tipos) = corpo.instanciacao(args.span.start).map(|x| x.to_vec()) else { continue };
        if tipos.len() != params.len() {
            continue;
        }
        for (i, &p) in params.iter().enumerate() {
            let limite = v.table.param(p).bound;
            let limite = v.subst(limite, &params, &tipos);
            if !v.sub(tipos[i], limite) {
                let ta = v.formatar(tipos[i]);
                let nome = interner.resolve(v.table.param(p).name).to_string();
                let l = v.formatar(limite);
                out.push(Diagnostic::com_codigo(c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, span, [ta.as_str(), nome.as_str(), l.as_str()]));
            }
        }
    }
    out
}

impl Verificador<'_> {
    fn ast(&self) -> &ast::Ast {
        &self.program.unit(self.unit).ast
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    fn e_topo(&mut self, t: TypeId) -> bool {
        let env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        crate::bounds::is_top(t, &env)
    }

    fn subst(&mut self, t: TypeId, params: &[TypeParamId], args: &[TypeId]) -> TypeId {
        let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        crate::ops::substitute(t, &mapa, self.table)
    }

    fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// O tipo que a resolução registrou para o nó (`NamedType.type`).
    fn registrado(&self, t: ast::TypeId) -> Option<TypeId> {
        self.outline
            .tipos_escritos
            .get(&(self.unit, t))
            .or_else(|| self.corpo.and_then(|c| c.tipos_de_anotacoes.get(&t)))
            .copied()
    }

    /// O tipo de um argumento escrito: o registrado, ou o resolvido aqui.
    fn tipo_do_no(&mut self, t: ast::TypeId) -> Option<TypeId> {
        match self.registrado(t) {
            Some(r) => Some(r),
            None => self.resolver(t),
        }
    }

    /// `_checkForTypeArgumentNotMatchingBounds` sobre o tipo resolvido `t`
    /// de um `NamedType` de intervalo `tipo`, com os argumentos escritos
    /// `escritos` (vazio: tipo cru).
    fn conferir_tipo(&mut self, t: TypeId, tipo: Span, escritos: &[ast::TypeId], super_permitido: bool, out: &mut Vec<Diagnostic>) {
        if self.table.e_invalido(t) {
            return;
        }
        let Some(partes) = self.partes(t) else { return };
        self.conferir_partes(t, partes, tipo, escritos, super_permitido, out);
    }

    /// O mesmo, com o elemento e os argumentos já conhecidos (o tipo montado
    /// aqui, que sem a decoração de exibição não guarda o alias).
    fn conferir_partes(&mut self, t: TypeId, partes: Partes, tipo: Span, escritos: &[ast::TypeId], super_permitido: bool, out: &mut Vec<Diagnostic>) {
        let Partes { nome, params, tipos, de_extensao, alvo_do_alias } = partes;
        if params.is_empty() || params.len() != tipos.len() {
            return;
        }
        let mut problemas = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            let dados = self.table.param(p).clone();
            if !dados.explicito {
                continue;
            }
            let limite = self.subst(dados.bound, &params, &tipos);
            if !self.sub(tipos[i], limite) {
                problemas.push((i, tipos[i], limite));
            }
        }
        if problemas.is_empty() {
            return;
        }
        let lista = |v: &mut Self, xs: &[TypeId]| xs.iter().map(|&x| v.formatar(x)).collect::<Vec<_>>().join(", ");
        let mut contexto: Vec<String> = Vec::new();
        if escritos.is_empty() {
            contexto.push(format!("The raw type was instantiated as '{nome}<{}>', and is not regular-bounded.", lista(self, &tipos)));
        }
        let no_do_erro = |i: usize| escritos.get(i).copied();
        if !super_permitido || de_extensao {
            for (i, arg, limite) in problemas {
                self.relatar_em(no_do_erro(i), tipo, params[i], arg, limite, &contexto, out);
            }
            return;
        }
        // `replaceTopAndBottom(type)`: o tipo inteiro topo vira `Never` (nem
        // alias nem interface); senão cada argumento pela variância do
        // parâmetro no alvo do alias (numa classe, covariante).
        if self.e_topo(t) {
            return;
        }
        let mut inv = Vec::with_capacity(tipos.len());
        for (&x, &p) in tipos.iter().zip(params.iter()) {
            let va = match alvo_do_alias {
                Some(alvo) => variancia_em(self.table, alvo, p, Variancia::Co).combinar(Variancia::Co),
                None => Variancia::Co,
            };
            inv.push(trocar_topo_e_fundo(self, x, va));
        }
        contexto.push(format!("The inverted type '{nome}<{}>' is also not regular-bounded, so the type is not well-bounded.", lista(self, &inv)));
        for (i, &p) in params.iter().enumerate() {
            let dados = self.table.param(p).clone();
            if !dados.explicito {
                continue;
            }
            let limite = self.subst(dados.bound, &params, &inv);
            if !self.sub(inv[i], limite) {
                self.relatar_em(no_do_erro(i), tipo, p, inv[i], limite, &contexto, out);
            }
        }
    }

    /// O nome do elemento, os parâmetros e os argumentos de tipo de `t`
    /// (`type.alias` antes de `InterfaceType`), e se é tipo de extensão.
    fn partes(&self, t: TypeId) -> Option<Partes> {
        if let Some(crate::table::Exibicao::Alias { typedef, args }) = self.table.exibicao(t) {
            return Some(self.partes_do_alias(*typedef, args.to_vec(), t));
        }
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some(self.partes_da_classe(*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some(self.partes_da_classe(*decl, args.to_vec())),
            // `FutureOr<T>`: o `T` não tem limite.
            _ => None,
        }
    }

    fn partes_do_alias(&self, typedef: dartforge_elements::model::TypedefId, tipos: Vec<TypeId>, t: TypeId) -> Partes {
        let d = &self.outline.typedefs[typedef.0 as usize];
        Partes {
            nome: self.interner.resolve(self.program.typedef(typedef).name).to_string(),
            params: d.type_params.to_vec(),
            tipos,
            de_extensao: matches!(self.table.get(t), Type::ExtensionType { .. }),
            alvo_do_alias: Some(d.target_type),
        }
    }

    fn partes_da_classe(&self, c: dartforge_elements::model::ClassId, tipos: Vec<TypeId>) -> Partes {
        Partes {
            nome: self.interner.resolve(self.program.class(c).name).to_string(),
            params: self.outline.classes[c.0 as usize].type_params.to_vec(),
            tipos,
            de_extensao: self.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType,
            alvo_do_alias: None,
        }
    }

    fn relatar_em(
        &mut self,
        no: Option<ast::TypeId>,
        tipo: Span,
        p: TypeParamId,
        arg: TypeId,
        limite: TypeId,
        contexto: &[String],
        out: &mut Vec<Diagnostic>,
    ) {
        let span = no.map(|x| self.ast().ty(x).span).unwrap_or(tipo);
        let a = self.formatar(arg);
        let nome = self.interner.resolve(self.table.param(p).name).to_string();
        let l = self.formatar(limite);
        let mut d = Diagnostic::com_codigo(c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, span, [a.as_str(), nome.as_str(), l.as_str()]);
        for m in contexto {
            d.contexto.push(dartforge_diagnostics::Contexto { arquivo: None, span: tipo, mensagem: m.as_str().into() });
        }
        out.push(d);
    }

    /// Os `NamedType` que o analyzer cria de expressões: o literal de tipo
    /// `C<T>` (aceita *super-bounded*), a referência de construtor
    /// `C<T>.nome` e a criação `C<T>()` sem `new` (`ConstructorName`: não
    /// aceita).
    fn tipos_nomeados_de_expressoes(&mut self, out: &mut Vec<Diagnostic>) {
        let Some(corpo) = self.corpo else { return };
        let a = &self.program.unit(self.unit).ast;
        // Os `C<T>` que são alvo de `.nome`.
        let mut de_construtor: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Property { target, .. } = &e.kind
                && matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. })
            {
                de_construtor.insert(*target);
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let (alvo, escritos, super_permitido, span) = match &e.kind {
                ExprKind::TypeArguments { target, type_args } => {
                    let construtor = de_construtor.contains(&id);
                    // O literal tem o tipo `Type`; a referência de função não.
                    if !construtor && corpo.get_type(id) != Some(self.core.type_) {
                        continue;
                    }
                    (*target, type_args.to_vec(), !construtor, e.span)
                }
                ExprKind::Call { target, arguments } if !arguments.type_args.is_empty() => {
                    if !matches!(corpo.get_resolved(id), Some(crate::resolved::Resolved::Constructor(_))) {
                        continue;
                    }
                    let fim = a.ty(*arguments.type_args.last().unwrap()).span.end;
                    let fonte = self.program.unit(self.unit).source.as_str();
                    let fim = fonte.get(fim..).and_then(|r| r.find('>')).map_or(fim, |k| fim + k + 1);
                    (*target, arguments.type_args.to_vec(), false, Span { start: a.expr(*target).span.start, end: fim })
                }
                _ => continue,
            };
            let el = match &a.expr(alvo).kind {
                ExprKind::Identifier(n) => self.program.lookup_na_unidade(self.unit, n.sym).filter(|b| !b.ambiguous).and_then(|b| b.getter),
                ExprKind::Property { target: p, name, .. } => match &a.expr(*p).kind {
                    ExprKind::Identifier(p) if self.program.prefixos_na_unidade(self.unit).contains_key(&p.sym) => {
                        self.program.lookup_prefixed_na_unidade(self.unit, p.sym, name.sym).filter(|b| !b.ambiguous).and_then(|b| b.getter)
                    }
                    _ => None,
                },
                _ => None,
            };
            // O nome resolvido no corpo tem de ser o mesmo elemento.
            if let Some(crate::resolved::Resolved::Element(r)) = corpo.get_resolved(alvo)
                && Some(*r) != el
            {
                continue;
            }
            let Some(el) = el else { continue };
            let Some((params, alvo_do_alias)) = self.parametros(el) else { continue };
            if params.len() != escritos.len() || params.is_empty() {
                continue;
            }
            let Some(tipos) = escritos.iter().map(|&x| self.tipo_do_no(x)).collect::<Option<Vec<TypeId>>>() else { continue };
            let Some((t, partes)) = self.montar(el, alvo_do_alias, &params, tipos) else { continue };
            self.conferir_partes(t, partes, span, &escritos, super_permitido, out);
        }
    }

    /// O elemento que o nome escrito designa no escopo da unidade.
    fn elemento(&self, name: &[ast::Name]) -> Option<Element> {
        if name.iter().any(|n| self.nomes_de_parametros.contains(&n.sym)) {
            return None;
        }
        let b = match name {
            [n] => self.program.lookup_na_unidade(self.unit, n.sym),
            [p, n] => self.program.lookup_prefixed_na_unidade(self.unit, p.sym, n.sym),
            _ => None,
        }?;
        if b.ambiguous {
            return None;
        }
        b.getter
    }

    /// Parâmetros de tipo (e o alvo, se alias) do elemento genérico.
    fn parametros(&self, el: Element) -> Option<(Vec<TypeParamId>, Option<TypeId>)> {
        match el {
            Element::Class(c) => Some((self.outline.classes[c.0 as usize].type_params.to_vec(), None)),
            Element::Typedef(t) => {
                let d = &self.outline.typedefs[t.0 as usize];
                Some((d.type_params.to_vec(), Some(d.target_type)))
            }
            _ => None,
        }
    }

    /// Um `NamedType` com argumentos escritos que nenhuma resolução
    /// registrou: o elemento pelo escopo da unidade (`Tipo.construtor` pelo
    /// primeiro nome) e os argumentos registrados ou resolvidos aqui.
    fn conferir(&mut self, tipo: Span, name: &[ast::Name], args: &[ast::TypeId], super_permitido: bool, out: &mut Vec<Diagnostic>) {
        let el = match self.elemento(name) {
            Some(el) => Some(el),
            None if name.len() == 2 => self.elemento(&name[..1]),
            None => None,
        };
        let Some(el) = el else { return };
        let Some((params, alvo)) = self.parametros(el) else { return };
        if params.len() != args.len() || params.is_empty() {
            return;
        }
        let Some(tipos) = args.iter().map(|&x| self.tipo_do_no(x)).collect::<Option<Vec<TypeId>>>() else { return };
        let Some((t, partes)) = self.montar(el, alvo, &params, tipos) else { return };
        self.conferir_partes(t, partes, tipo, args, super_permitido, out);
    }

    /// O tipo `el<tipos>` e as partes dele.
    fn montar(&mut self, el: Element, alvo: Option<TypeId>, params: &[TypeParamId], tipos: Vec<TypeId>) -> Option<(TypeId, Partes)> {
        match (el, alvo) {
            (Element::Typedef(td), Some(alvo)) => {
                let r = self.subst(alvo, params, &tipos);
                let t = self.table.decorar(r, crate::table::Exibicao::Alias { typedef: td, args: tipos.clone().into_boxed_slice() });
                let partes = self.partes_do_alias(td, tipos, t);
                Some((t, partes))
            }
            (Element::Class(c), _) => {
                let t = if self.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
                    self.table.intern(Type::ExtensionType { decl: c, args: tipos.clone().into_boxed_slice(), nullable: false })
                } else {
                    self.table.intern(Type::Interface { class: c, args: tipos.clone().into_boxed_slice(), nullable: false })
                };
                Some((t, self.partes_da_classe(c, tipos)))
            }
            _ => None,
        }
    }

    /// O tipo escrito, resolvido no escopo da biblioteca; `None` se depender
    /// de algo que aqui não se resolve com certeza.
    fn resolver(&mut self, t: ast::TypeId) -> Option<TypeId> {
        let no = self.ast().ty(t);
        let anulavel = no.nullable;
        let r = match &no.kind {
            TypeKind::Void => self.core.void_,
            TypeKind::Named { name, args } => {
                let (name, args) = (name.clone(), args.clone());
                if let [n] = &name[..] {
                    match self.interner.resolve(n.sym) {
                        "dynamic" if args.is_empty() => return Some(self.core.dynamic_),
                        "Never" if args.is_empty() => {
                            return Some(if anulavel {
                                self.table.decorar(self.core.null, crate::table::Exibicao::NeverAnulavel)
                            } else {
                                self.core.never
                            });
                        }
                        _ => {}
                    }
                }
                let el = self.elemento(&name)?;
                let tipos = args.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>()?;
                match el {
                    Element::Class(cid) => {
                        let cl = self.program.class(cid);
                        let nome = self.interner.resolve(cl.name);
                        if Some(cid) == self.core.null_class {
                            self.core.null
                        } else if nome == "FutureOr" && self.program.library(cl.library).is_sdk {
                            let [arg] = tipos[..] else { return None };
                            self.table.intern(Type::FutureOr { arg, nullable: false })
                        } else {
                            let params = self.outline.classes[cid.0 as usize].type_params.clone();
                            if params.len() != tipos.len() {
                                // Cru e genérico (instanciado para os limites): fora.
                                return None;
                            }
                            if cl.kind == dartforge_elements::model::ClassKind::ExtensionType {
                                self.table.intern(Type::ExtensionType { decl: cid, args: tipos.into_boxed_slice(), nullable: false })
                            } else {
                                self.table.intern(Type::Interface { class: cid, args: tipos.into_boxed_slice(), nullable: false })
                            }
                        }
                    }
                    Element::Typedef(tid) => {
                        let d = &self.outline.typedefs[tid.0 as usize];
                        let (params, alvo) = (d.type_params.to_vec(), d.target_type);
                        if params.len() != tipos.len() {
                            return None;
                        }
                        let r = self.subst(alvo, &params, &tipos);
                        self.table.decorar(r, crate::table::Exibicao::Alias { typedef: tid, args: tipos.into_boxed_slice() })
                    }
                    _ => return None,
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                if !type_params.is_empty() {
                    return None;
                }
                let (ret, parameters) = (*return_type, parameters.clone_params());
                let ret = match ret {
                    Some(r) => self.resolver(r)?,
                    None => self.core.dynamic_,
                };
                let mut pos = Vec::new();
                let mut opt = Vec::new();
                let mut named = Vec::new();
                for (kind, ty, nome, req) in parameters {
                    let t = match ty {
                        Some(x) => self.resolver(x)?,
                        None => self.core.dynamic_,
                    };
                    match kind {
                        ParameterKind::Required => pos.push(t),
                        ParameterKind::Optional => opt.push(t),
                        ParameterKind::Named => named.push((nome?, t, req)),
                    }
                }
                self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            TypeKind::Record { positional, named } => {
                let (positional, named) = (positional.clone(), named.clone());
                let pos = positional.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>()?;
                let mut nm = Vec::new();
                for (n, x) in named.iter() {
                    nm.push((n.sym, self.resolver(*x)?));
                }
                self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            }
        };
        Some(if anulavel { crate::ops::nullable(r, self.table) } else { r })
    }
}

/// Os parâmetros de uma lista escrita, só o que a resolução usa.
trait ParametrosSimples {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)>;
}

impl ParametrosSimples for Box<[ast::Parameter]> {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)> {
        self.iter().map(|p| (p.kind, p.ty, p.name.map(|n| n.sym), p.required)).collect()
    }
}

/// Variância do parâmetro `p` no tipo `t` (a posição de fora é `va`).
fn variancia_em(table: &TypeTable, t: TypeId, p: TypeParamId, va: Variancia) -> Variancia {
    match table.get(t) {
        Type::TypeParameter { param, .. } if *param == p => va,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().fold(Variancia::Nenhuma, |acc, &a| acc.juntar(variancia_em(table, a, p, va)))
        }
        Type::FutureOr { arg, .. } => variancia_em(table, *arg, p, va),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            let contra = va.combinar(Variancia::Contra);
            let mut v = variancia_em(table, *ret, p, va);
            for &x in positional.iter().chain(optional.iter()) {
                v = v.juntar(variancia_em(table, x, p, contra));
            }
            for (_, x, _) in named.iter() {
                v = v.juntar(variancia_em(table, *x, p, contra));
            }
            // Nos limites dos parâmetros de tipo da função: invariante.
            for &tp in type_params.iter() {
                if variancia_em(table, table.param(tp).bound, p, va) != Variancia::Nenhuma {
                    v = v.juntar(Variancia::In);
                }
            }
            v
        }
        Type::Record { positional, named, .. } => {
            let mut v = Variancia::Nenhuma;
            for &x in positional.iter() {
                v = v.juntar(variancia_em(table, x, p, va));
            }
            for (_, x) in named.iter() {
                v = v.juntar(variancia_em(table, *x, p, va));
            }
            v
        }
        Type::Intersection { bound, .. } => variancia_em(table, *bound, p, va),
        _ => Variancia::Nenhuma,
    }
}

/// Todo nome de parâmetro de tipo declarado na unidade.
fn nomes_de_parametros(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<SymbolId> {
    let mut s = HashSet::new();
    let mut tps = |l: &[ast::TypeParameter]| {
        for tp in l {
            s.insert(tp.name.sym);
        }
    };
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => tps(&x.type_params),
            DeclKind::Mixin(x) => tps(&x.type_params),
            DeclKind::Enum(x) => tps(&x.type_params),
            DeclKind::Extension(x) => tps(&x.type_params),
            DeclKind::ExtensionType(x) => tps(&x.type_params),
            DeclKind::Typedef(x) => tps(&x.type_params),
            DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for f in &a.functions {
        tps(&f.type_params);
    }
    for t in &a.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            tps(type_params);
            for p in parameters.iter() {
                tps(&p.function_type_params);
            }
        }
    }
    for f in &a.functions {
        for p in f.parameters.iter().flatten() {
            tps(&p.function_type_params);
        }
    }
    s
}

/// Tipos escritos onde o analyzer não aceita *super-bounded*: `extends`,
/// `with`, `implements`, `on` de mixin, o corpo de `typedef X = T` e o tipo
/// da criação de instância.
fn sem_super_limite(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<ast::TypeId> {
    let mut s = HashSet::new();
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => {
                s.extend(x.extends);
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Mixin(x) => {
                s.extend(x.on.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Enum(x) => {
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::ExtensionType(x) => s.extend(x.implements.iter().copied()),
            DeclKind::Typedef(x) => {
                if let ast::TypedefKind::Alias(t) = &x.kind {
                    s.insert(*t);
                }
            }
            DeclKind::Extension(_) | DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for e in &a.exprs {
        if let ExprKind::InstanceCreation { ty, .. } = &e.kind {
            s.insert(*ty);
        }
    }
    for m in &a.members {
        if let ast::MemberKind::Constructor(k) = &m.kind
            && let Some(r) = &k.redirect
        {
            s.insert(r.ty);
        }
    }
    s
}
