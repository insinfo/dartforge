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
enum Variance_ {
    Nenhuma,
    Co,
    Contra,
    In,
}

impl Variance_ {
    /// `Variance.combine`: o sinal da posição dentro de outra.
    fn combinar(self, other: Variance_) -> Variance_ {
        use Variance_::*;
        match (self, other) {
            (Nenhuma, _) | (_, Nenhuma) => Nenhuma,
            (In, _) | (_, In) => In,
            (a, b) if a == b => Co,
            _ => Contra,
        }
    }

    /// `Variance.meet`: duas ocorrências do mesmo parâmetro.
    fn juntar(self, other: Variance_) -> Variance_ {
        use Variance_::*;
        match (self, other) {
            (Nenhuma, x) | (x, Nenhuma) => x,
            (a, b) if a == b => a,
            _ => In,
        }
    }
}

/// O elemento de um tipo nomeado e os argumentos dele.
struct Parts {
    nome: String,
    params: Vec<TypeParamId>,
    tipos: Vec<TypeId>,
    of_extension: bool,
    /// O alvo do alias (as variâncias dos parâmetros na inversão).
    alias_target: Option<TypeId>,
}

struct Verifier<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    unit: UnitId,
    /// Nomes de parâmetros de tipo declarados em algum lugar da unidade.
    parameter_names: HashSet<SymbolId>,
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
    body: Option<&crate::resolved::UnitBodyTypes>,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verifier { program, interner, table, core, outline, unit, parameter_names: HashSet::new(), corpo: body };
    v.parameter_names = parameter_names(a, &program.unit(unit).unit);
    let without_super = without_super_bound(a, &program.unit(unit).unit);
    // A criação sem argumentos escritos é conferida com os inferidos
    // (`argumentos_inferidos_fora_dos_limites`).
    let raw_creations: HashSet<ast::TypeId> = a
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
            if t.span.start == t.span.end || raw_creations.contains(&id) {
                continue;
            }
            let super_allowed = !without_super.contains(&id);
            match v.registered(id) {
                Some(r) => v.conferir_tipo(r, t.span, args, super_allowed, &mut out),
                None if !args.is_empty() => v.conferir(t.span, name, args, super_allowed, &mut out),
                None => {}
            }
        }
    }
    if body.is_some() {
        v.expression_named_types(&mut out);
    }
    out
}

/// `ReplaceTopBottomVisitor.process` sobre um tipo já resolvido: topo vira
/// `Never` fora das posições contravariantes; o que é subtipo de `Never` vira
/// `Object?` nas contravariantes. Um alias leva cada argumento com a
/// variância do parâmetro dele no alvo combinada com a de fora; classes (e
/// `FutureOr`, tipos de extensão) levam a variância adiante; funções invertem
/// nos parâmetros (os parâmetros de tipo ficam).
fn swap_top_and_bottom(v: &mut Verifier<'_>, t: TypeId, va: Variance_) -> TypeId {
    if va == Variance_::Contra {
        let never = v.core.never;
        if v.sub(t, never) {
            return v.core.object_nullable;
        }
    } else if v.the_is_top(t) {
        return v.core.never;
    }
    let is_nullable = v.table.get(t).is_declared_nullable();
    if let Some(crate::table::Exibicao::Alias { typedef, args }) = v.table.exibicao(t).cloned() {
        let d = &v.outline.typedefs[typedef.0 as usize];
        let (params, tgt) = (d.type_params.to_vec(), d.target_type);
        if params.len() == args.len() {
            let mut new_ = Vec::with_capacity(args.len());
            for (&x, &p) in args.iter().zip(params.iter()) {
                let vp = variance_in(v.table, tgt, p, Variance_::Co);
                new_.push(swap_top_and_bottom(v, x, vp.combinar(va)));
            }
            let r = v.subst(tgt, &params, &new_);
            let r = if is_nullable { crate::ops::nullable(r, v.table) } else { r };
            return v.table.decorate(r, crate::table::Exibicao::Alias { typedef, args: new_.into_boxed_slice() });
        }
    }
    match v.table.get(t).clone() {
        Type::Interface { class, args, nullable } if !args.is_empty() => {
            let new_: Vec<TypeId> = args.iter().map(|&x| swap_top_and_bottom(v, x, va)).collect();
            v.table.intern(Type::Interface { class, args: new_.into_boxed_slice(), nullable })
        }
        Type::ExtensionType { decl, args, nullable } if !args.is_empty() => {
            let new_: Vec<TypeId> = args.iter().map(|&x| swap_top_and_bottom(v, x, va)).collect();
            v.table.intern(Type::ExtensionType { decl, args: new_.into_boxed_slice(), nullable })
        }
        Type::FutureOr { arg, nullable } => {
            let arg = swap_top_and_bottom(v, arg, va);
            v.table.intern(Type::FutureOr { arg, nullable })
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let ret = swap_top_and_bottom(v, ret, va);
            let against = va.combinar(Variance_::Contra);
            let positional: Vec<TypeId> = positional.iter().map(|&x| swap_top_and_bottom(v, x, against)).collect();
            let optional: Vec<TypeId> = optional.iter().map(|&x| swap_top_and_bottom(v, x, against)).collect();
            let named: Vec<(SymbolId, TypeId, bool)> = named.iter().map(|&(n, x, r)| (n, swap_top_and_bottom(v, x, against), r)).collect();
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
    let mut output = Vec::new();
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
        let args = crate::ops::instantiate_to_bounds(&params, &[], table, core);
        let mut v = Verifier { program, interner, table: &mut *table, core, outline, unit: d.unit, parameter_names: HashSet::new(), corpo: None };
        let inverted: Vec<TypeId> = args.iter().map(|&t| swap_top_and_bottom(&mut v, t, Variance_::Co)).collect();
        let mut well_bounded = true;
        for (k, &p) in params.iter().enumerate() {
            let data = v.table.param(p).clone();
            if !data.explicito {
                continue;
            }
            let bnd = v.subst(data.bound, &params, &inverted);
            if !v.sub(inverted[k], bnd) {
                well_bounded = false;
                break;
            }
        }
        if !well_bounded {
            output.push((d.unit, Diagnostic::com_codigo(c::ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED, x.name.span, Vec::<String>::new())));
        }
    }
    output
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
    body: &crate::resolved::UnitBodyTypes,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verifier { program, interner, table, core, outline, unit, parameter_names: HashSet::new(), corpo: Some(body) };
    let mut out = Vec::new();
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        let (span, args) = match &e.kind {
            ExprKind::InstanceCreation { ty, arguments, .. } => {
                let TypeKind::Named { args: written_ones, .. } = &a.ty(*ty).kind else { continue };
                if !written_ones.is_empty() {
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
        let Some(crate::resolved::Resolved::Constructor(f)) = body.get_resolved(id) else { continue };
        let Some(c) = program.function(*f).class else { continue };
        let params = outline.classes[c.0 as usize].type_params.to_vec();
        if params.is_empty() {
            continue;
        }
        let Some(types) = body.instanciacao(args.span.start).map(|x| x.to_vec()) else { continue };
        if types.len() != params.len() {
            continue;
        }
        for (i, &p) in params.iter().enumerate() {
            let bnd = v.table.param(p).bound;
            let bnd = v.subst(bnd, &params, &types);
            if !v.sub(types[i], bnd) {
                let ta = v.formatar(types[i]);
                let the_name = interner.resolve(v.table.param(p).name).to_string();
                let l = v.formatar(bnd);
                out.push(Diagnostic::com_codigo(c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, span, [ta.as_str(), the_name.as_str(), l.as_str()]));
            }
        }
    }
    out
}

impl Verifier<'_> {
    fn ast(&self) -> &ast::Ast {
        &self.program.unit(self.unit).ast
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    fn the_is_top(&mut self, t: TypeId) -> bool {
        let env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        crate::bounds::is_top(t, &env)
    }

    fn subst(&mut self, t: TypeId, params: &[TypeParamId], args: &[TypeId]) -> TypeId {
        let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        crate::ops::substitute(t, &map, self.table)
    }

    fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// O tipo que a resolução registrou para o nó (`NamedType.type`).
    fn registered(&self, t: ast::TypeId) -> Option<TypeId> {
        self.outline
            .tipos_escritos
            .get(&(self.unit, t))
            .or_else(|| self.corpo.and_then(|c| c.tipos_de_anotacoes.get(&t)))
            .copied()
    }

    /// O tipo de um argumento escrito: o registrado, ou o resolvido aqui.
    fn tipo_do_no(&mut self, t: ast::TypeId) -> Option<TypeId> {
        match self.registered(t) {
            Some(r) => Some(r),
            None => self.resolver(t),
        }
    }

    /// `_checkForTypeArgumentNotMatchingBounds` sobre o tipo resolvido `t`
    /// de um `NamedType` de intervalo `tipo`, com os argumentos escritos
    /// `escritos` (vazio: tipo cru).
    fn conferir_tipo(&mut self, t: TypeId, typ: Span, written_ones: &[ast::TypeId], super_allowed: bool, out: &mut Vec<Diagnostic>) {
        if self.table.e_invalido(t) {
            return;
        }
        let Some(parts) = self.partes(t) else { return };
        self.check_parts(t, parts, typ, written_ones, super_allowed, out);
    }

    /// O mesmo, com o elemento e os argumentos já conhecidos (o tipo montado
    /// aqui, que sem a decoração de exibição não guarda o alias).
    fn check_parts(&mut self, t: TypeId, parts: Parts, typ: Span, written_ones: &[ast::TypeId], super_allowed: bool, out: &mut Vec<Diagnostic>) {
        let Parts { nome: the_name, params, tipos: types, of_extension, alias_target } = parts;
        if params.is_empty() || params.len() != types.len() {
            return;
        }
        let mut problems = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            let data = self.table.param(p).clone();
            if !data.explicito {
                continue;
            }
            let bnd = self.subst(data.bound, &params, &types);
            if !self.sub(types[i], bnd) {
                problems.push((i, types[i], bnd));
            }
        }
        if problems.is_empty() {
            return;
        }
        let list = |v: &mut Self, xs: &[TypeId]| xs.iter().map(|&x| v.formatar(x)).collect::<Vec<_>>().join(", ");
        let mut context: Vec<String> = Vec::new();
        if written_ones.is_empty() {
            context.push(format!("The raw type was instantiated as '{the_name}<{}>', and is not regular-bounded.", list(self, &types)));
        }
        let error_node = |i: usize| written_ones.get(i).copied();
        if !super_allowed || of_extension {
            for (i, arg, bnd) in problems {
                self.report_at(error_node(i), typ, params[i], arg, bnd, &context, out);
            }
            return;
        }
        // `replaceTopAndBottom(type)`: o tipo inteiro topo vira `Never` (nem
        // alias nem interface); senão cada argumento pela variância do
        // parâmetro no alvo do alias (numa classe, covariante).
        if self.the_is_top(t) {
            return;
        }
        let mut inv = Vec::with_capacity(types.len());
        for (&x, &p) in types.iter().zip(params.iter()) {
            let va = match alias_target {
                Some(tgt) => variance_in(self.table, tgt, p, Variance_::Co).combinar(Variance_::Co),
                None => Variance_::Co,
            };
            inv.push(swap_top_and_bottom(self, x, va));
        }
        context.push(format!("The inverted type '{the_name}<{}>' is also not regular-bounded, so the type is not well-bounded.", list(self, &inv)));
        for (i, &p) in params.iter().enumerate() {
            let data = self.table.param(p).clone();
            if !data.explicito {
                continue;
            }
            let bnd = self.subst(data.bound, &params, &inv);
            if !self.sub(inv[i], bnd) {
                self.report_at(error_node(i), typ, p, inv[i], bnd, &context, out);
            }
        }
    }

    /// O nome do elemento, os parâmetros e os argumentos de tipo de `t`
    /// (`type.alias` antes de `InterfaceType`), e se é tipo de extensão.
    fn partes(&self, t: TypeId) -> Option<Parts> {
        if let Some(crate::table::Exibicao::Alias { typedef, args }) = self.table.exibicao(t) {
            return Some(self.alias_parts(*typedef, args.to_vec(), t));
        }
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some(self.class_parts(*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some(self.class_parts(*decl, args.to_vec())),
            // `FutureOr<T>`: o `T` não tem limite.
            _ => None,
        }
    }

    fn alias_parts(&self, typedef: dartforge_elements::model::TypedefId, types: Vec<TypeId>, t: TypeId) -> Parts {
        let d = &self.outline.typedefs[typedef.0 as usize];
        Parts {
            nome: self.interner.resolve(self.program.typedef(typedef).name).to_string(),
            params: d.type_params.to_vec(),
            tipos: types,
            of_extension: matches!(self.table.get(t), Type::ExtensionType { .. }),
            alias_target: Some(d.target_type),
        }
    }

    fn class_parts(&self, c: dartforge_elements::model::ClassId, types: Vec<TypeId>) -> Parts {
        Parts {
            nome: self.interner.resolve(self.program.class(c).name).to_string(),
            params: self.outline.classes[c.0 as usize].type_params.to_vec(),
            tipos: types,
            of_extension: self.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType,
            alias_target: None,
        }
    }

    fn report_at(
        &mut self,
        no: Option<ast::TypeId>,
        typ: Span,
        p: TypeParamId,
        arg: TypeId,
        bnd: TypeId,
        context: &[String],
        out: &mut Vec<Diagnostic>,
    ) {
        let span = no.map(|x| self.ast().ty(x).span).unwrap_or(typ);
        let a = self.formatar(arg);
        let the_name = self.interner.resolve(self.table.param(p).name).to_string();
        let l = self.formatar(bnd);
        let mut d = Diagnostic::com_codigo(c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, span, [a.as_str(), the_name.as_str(), l.as_str()]);
        for m in context {
            d.contexto.push(dartforge_diagnostics::Contexto { arquivo: None, span: typ, mensagem: m.as_str().into() });
        }
        out.push(d);
    }

    /// Os `NamedType` que o analyzer cria de expressões: o literal de tipo
    /// `C<T>` (aceita *super-bounded*), a referência de construtor
    /// `C<T>.nome` e a criação `C<T>()` sem `new` (`ConstructorName`: não
    /// aceita).
    fn expression_named_types(&mut self, out: &mut Vec<Diagnostic>) {
        let Some(body) = self.corpo else { return };
        let a = &self.program.unit(self.unit).ast;
        // Os `C<T>` que são alvo de `.nome`.
        let mut of_constructor: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Property { target, .. } = &e.kind
                && matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. })
            {
                of_constructor.insert(*target);
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let (tgt, written_ones, super_allowed, span) = match &e.kind {
                ExprKind::TypeArguments { target, type_args } => {
                    let constructor = of_constructor.contains(&id);
                    // O literal tem o tipo `Type`; a referência de função não.
                    if !constructor && body.get_type(id) != Some(self.core.type_) {
                        continue;
                    }
                    (*target, type_args.to_vec(), !constructor, e.span)
                }
                ExprKind::Call { target, arguments } if !arguments.type_args.is_empty() => {
                    if !matches!(body.get_resolved(id), Some(crate::resolved::Resolved::Constructor(_))) {
                        continue;
                    }
                    let end = a.ty(*arguments.type_args.last().unwrap()).span.end;
                    let source = self.program.unit(self.unit).source.as_str();
                    let end = source.get(end..).and_then(|r| r.find('>')).map_or(end, |k| end + k + 1);
                    (*target, arguments.type_args.to_vec(), false, Span { start: a.expr(*target).span.start, end: end })
                }
                _ => continue,
            };
            let el = match &a.expr(tgt).kind {
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
            if let Some(crate::resolved::Resolved::Element(r)) = body.get_resolved(tgt)
                && Some(*r) != el
            {
                continue;
            }
            let Some(el) = el else { continue };
            let Some((params, alias_target)) = self.parametros(el) else { continue };
            if params.len() != written_ones.len() || params.is_empty() {
                continue;
            }
            let Some(types) = written_ones.iter().map(|&x| self.tipo_do_no(x)).collect::<Option<Vec<TypeId>>>() else { continue };
            let Some((t, parts)) = self.assemble(el, alias_target, &params, types) else { continue };
            self.check_parts(t, parts, span, &written_ones, super_allowed, out);
        }
    }

    /// O elemento que o nome escrito designa no escopo da unidade.
    fn elemento(&self, name: &[ast::Name]) -> Option<Element> {
        if name.iter().any(|n| self.parameter_names.contains(&n.sym)) {
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
                // O alias que chega a si mesmo instancia como `dynamic`: sem
                // conferência de limites (`auto_referencia`).
                if crate::auto_referencia::typedef_auto_referente(self.program, self.program.typedef(t).decl) {
                    return None;
                }
                let d = &self.outline.typedefs[t.0 as usize];
                Some((d.type_params.to_vec(), Some(d.target_type)))
            }
            _ => None,
        }
    }

    /// Um `NamedType` com argumentos escritos que nenhuma resolução
    /// registrou: o elemento pelo escopo da unidade (`Tipo.construtor` pelo
    /// primeiro nome) e os argumentos registrados ou resolvidos aqui.
    fn conferir(&mut self, typ: Span, name: &[ast::Name], args: &[ast::TypeId], super_allowed: bool, out: &mut Vec<Diagnostic>) {
        let el = match self.elemento(name) {
            Some(el) => Some(el),
            None if name.len() == 2 => self.elemento(&name[..1]),
            None => None,
        };
        let Some(el) = el else { return };
        let Some((params, tgt)) = self.parametros(el) else { return };
        if params.len() != args.len() || params.is_empty() {
            return;
        }
        let Some(types) = args.iter().map(|&x| self.tipo_do_no(x)).collect::<Option<Vec<TypeId>>>() else { return };
        let Some((t, parts)) = self.assemble(el, tgt, &params, types) else { return };
        self.check_parts(t, parts, typ, args, super_allowed, out);
    }

    /// O tipo `el<tipos>` e as partes dele.
    fn assemble(&mut self, el: Element, tgt: Option<TypeId>, params: &[TypeParamId], types: Vec<TypeId>) -> Option<(TypeId, Parts)> {
        match (el, tgt) {
            (Element::Typedef(td), Some(tgt)) => {
                let r = self.subst(tgt, params, &types);
                let t = self.table.decorate(r, crate::table::Exibicao::Alias { typedef: td, args: types.clone().into_boxed_slice() });
                let parts = self.alias_parts(td, types, t);
                Some((t, parts))
            }
            (Element::Class(c), _) => {
                let t = if self.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
                    self.table.intern(Type::ExtensionType { decl: c, args: types.clone().into_boxed_slice(), nullable: false })
                } else {
                    self.table.intern(Type::Interface { class: c, args: types.clone().into_boxed_slice(), nullable: false })
                };
                Some((t, self.class_parts(c, types)))
            }
            _ => None,
        }
    }

    /// O tipo escrito, resolvido no escopo da biblioteca; `None` se depender
    /// de algo que aqui não se resolve com certeza.
    fn resolver(&mut self, t: ast::TypeId) -> Option<TypeId> {
        let no = self.ast().ty(t);
        let is_nullable = no.nullable;
        let r = match &no.kind {
            TypeKind::Void => self.core.void_,
            TypeKind::Named { name, args } => {
                let (name, args) = (name.clone(), args.clone());
                if let [n] = &name[..] {
                    match self.interner.resolve(n.sym) {
                        "dynamic" if args.is_empty() => return Some(self.core.dynamic_),
                        "Never" if args.is_empty() => {
                            return Some(if is_nullable {
                                self.table.decorate(self.core.null, crate::table::Exibicao::NeverAnulavel)
                            } else {
                                self.core.never
                            });
                        }
                        _ => {}
                    }
                }
                let el = self.elemento(&name)?;
                let types = args.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>()?;
                match el {
                    Element::Class(cid) => {
                        let cl = self.program.class(cid);
                        let the_name = self.interner.resolve(cl.name);
                        if Some(cid) == self.core.null_class {
                            self.core.null
                        } else if the_name == "FutureOr" && self.program.library(cl.library).is_sdk {
                            let [arg] = types[..] else { return None };
                            self.table.intern(Type::FutureOr { arg, nullable: false })
                        } else {
                            let params = self.outline.classes[cid.0 as usize].type_params.clone();
                            if params.len() != types.len() {
                                // Cru e genérico (instanciado para os limites): fora.
                                return None;
                            }
                            if cl.kind == dartforge_elements::model::ClassKind::ExtensionType {
                                self.table.intern(Type::ExtensionType { decl: cid, args: types.into_boxed_slice(), nullable: false })
                            } else {
                                self.table.intern(Type::Interface { class: cid, args: types.into_boxed_slice(), nullable: false })
                            }
                        }
                    }
                    Element::Typedef(tid) => {
                        let d = &self.outline.typedefs[tid.0 as usize];
                        let (params, tgt) = (d.type_params.to_vec(), d.target_type);
                        if params.len() != types.len() {
                            return None;
                        }
                        let r = self.subst(tgt, &params, &types);
                        self.table.decorate(r, crate::table::Exibicao::Alias { typedef: tid, args: types.into_boxed_slice() })
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
                for (kind, ty, the_name, req) in parameters {
                    let t = match ty {
                        Some(x) => self.resolver(x)?,
                        None => self.core.dynamic_,
                    };
                    match kind {
                        ParameterKind::Required => pos.push(t),
                        ParameterKind::Optional => opt.push(t),
                        ParameterKind::Named => named.push((the_name?, t, req)),
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
        Some(if is_nullable { crate::ops::nullable(r, self.table) } else { r })
    }
}

/// Os parâmetros de uma lista escrita, só o que a resolução usa.
trait SimpleParameters {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)>;
}

impl SimpleParameters for Box<[ast::Parameter]> {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)> {
        self.iter().map(|p| (p.kind, p.ty, p.name.map(|n| n.sym), p.required)).collect()
    }
}

/// Variância do parâmetro `p` no tipo `t` (a posição de fora é `va`).
fn variance_in(table: &TypeTable, t: TypeId, p: TypeParamId, va: Variance_) -> Variance_ {
    match table.get(t) {
        Type::TypeParameter { param, .. } if *param == p => va,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().fold(Variance_::Nenhuma, |acc, &a| acc.juntar(variance_in(table, a, p, va)))
        }
        Type::FutureOr { arg, .. } => variance_in(table, *arg, p, va),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            let against = va.combinar(Variance_::Contra);
            let mut v = variance_in(table, *ret, p, va);
            for &x in positional.iter().chain(optional.iter()) {
                v = v.juntar(variance_in(table, x, p, against));
            }
            for (_, x, _) in named.iter() {
                v = v.juntar(variance_in(table, *x, p, against));
            }
            // Nos limites dos parâmetros de tipo da função: invariante.
            for &tp in type_params.iter() {
                if variance_in(table, table.param(tp).bound, p, va) != Variance_::Nenhuma {
                    v = v.juntar(Variance_::In);
                }
            }
            v
        }
        Type::Record { positional, named, .. } => {
            let mut v = Variance_::Nenhuma;
            for &x in positional.iter() {
                v = v.juntar(variance_in(table, x, p, va));
            }
            for (_, x) in named.iter() {
                v = v.juntar(variance_in(table, *x, p, va));
            }
            v
        }
        Type::Intersection { bound, .. } => variance_in(table, *bound, p, va),
        _ => Variance_::Nenhuma,
    }
}

/// Todo nome de parâmetro de tipo declarado na unidade.
fn parameter_names(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<SymbolId> {
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
fn without_super_bound(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<ast::TypeId> {
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
