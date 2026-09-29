//! Posições de variância dos parâmetros de tipo de classes, mixins, enums e
//! extension types, como o `ErrorVerifier` do analyzer 6.11.0
//! (`src/generated/error_verifier.dart`):
//!
//! * `_checkForWrongTypeParameterVarianceInField` e
//!   `_checkForWrongTypeParameterVarianceInMethod` —
//!   `WRONG_TYPE_PARAMETER_VARIANCE_POSITION` para um parâmetro de tipo com
//!   variância escrita (`in`/`out`/`inout`) usado numa posição de variância
//!   incompatível no tipo de um campo, de um parâmetro, do retorno ou do
//!   limite de um parâmetro de tipo de método;
//! * `_checkForWrongTypeParameterVarianceInSuperinterfaces` —
//!   `WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE` (com
//!   variância escrita) e `WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE`
//!   (sem, o "legado covariante") nos supertipos diretos.
//!
//! A variância de um parâmetro num tipo é a de
//! `TypeParameterElementImpl.computeVarianceInType`, com o reticulado
//! `Variance` do `_fe_analyzer_shared` (`combine`, `meet`,
//! `greaterThanOrEqual`). O analyzer guarda a variância escrita no elemento
//! mesmo com o experimento `variance` desligado; aqui ela vem da árvore
//! (`TypeParameter.variance`), gravada na tabela por `resolve`.

use crate::despejo::formatar;
use crate::resolve::OutlineTypes;
use crate::table::{Type, TypeId, TypeParamId, TypeTable, Variance as Declarada};
use dartforge_diagnostics::{Diagnostic, Span, codigos::compile_time_error as c};
use dartforge_elements::model::{FunctionRef, LibraryId, Program, UnitId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::HashMap;

/// `Variance` do `_fe_analyzer_shared` (a ordem é a codificação: `meet` é o
/// `|` dos índices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V {
    Unrelated = 0,
    Covariant = 1,
    Contravariant = 2,
    Invariant = 3,
}

impl V {
    fn de(bits: u8) -> V {
        match bits {
            0 => V::Unrelated,
            1 => V::Covariant,
            2 => V::Contravariant,
            _ => V::Invariant,
        }
    }

    fn combine(self, o: V) -> V {
        if self == V::Unrelated || o == V::Unrelated {
            return V::Unrelated;
        }
        if self == V::Invariant || o == V::Invariant {
            return V::Invariant;
        }
        if self == o {
            V::Covariant
        } else {
            V::Contravariant
        }
    }

    fn meet(self, o: V) -> V {
        V::de(self as u8 | o as u8)
    }

    fn maior_ou_igual(self, o: V) -> bool {
        match self {
            V::Unrelated => true,
            V::Covariant => matches!(o, V::Covariant | V::Invariant),
            V::Contravariant => matches!(o, V::Contravariant | V::Invariant),
            V::Invariant => o == V::Invariant,
        }
    }

    fn palavra(self) -> &'static str {
        match self {
            V::Unrelated => "",
            V::Covariant => "out",
            V::Contravariant => "in",
            V::Invariant => "inout",
        }
    }

    /// A variância de um parâmetro de tipo (sem modificador: covariante).
    fn do_parametro(d: Declarada) -> V {
        match d {
            Declarada::Unspecified | Declarada::Covariant => V::Covariant,
            Declarada::Contravariant => V::Contravariant,
            Declarada::Invariant => V::Invariant,
        }
    }
}

/// `computeVarianceInType`: a variância de `x` em `t`.
fn variancia_em(table: &TypeTable, outline: &OutlineTypes, x: TypeParamId, t: TypeId) -> V {
    match table.get(t) {
        Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
            if *param == x {
                V::Covariant
            } else {
                V::Unrelated
            }
        }
        Type::Interface { class, args, .. }
        | Type::ExtensionType {
            decl: class, args, ..
        } => {
            let params = outline
                .classes
                .get(class.0 as usize)
                .map(|d| &d.type_params[..])
                .unwrap_or(&[]);
            let mut r = V::Unrelated;
            for (i, &a) in args.iter().enumerate() {
                let pv = params
                    .get(i)
                    .map_or(V::Covariant, |&p| V::do_parametro(table.param(p).variance));
                r = r.meet(pv.combine(variancia_em(table, outline, x, a)));
            }
            r
        }
        // `FutureOr<T>` é uma interface de parâmetro covariante no analyzer.
        Type::FutureOr { arg, .. } => {
            V::Unrelated.meet(V::Covariant.combine(variancia_em(table, outline, x, *arg)))
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            let mut r = variancia_em(table, outline, x, *ret);
            for &tp in type_params.iter() {
                let d = table.param(tp);
                if variancia_em(table, outline, x, d.bound) != V::Unrelated {
                    r = V::Invariant;
                }
            }
            for &p in positional
                .iter()
                .chain(optional.iter())
                .chain(named.iter().map(|(_, t, _)| t))
            {
                r = r.meet(V::Contravariant.combine(variancia_em(table, outline, x, p)));
            }
            r
        }
        _ => V::Unrelated,
    }
}

/// Os diagnósticos de variância das declarações de `lib`.
/// Os parâmetros de tipo sem modificador só entram na conferência dos
/// supertipos (`WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE`).
pub fn variancia(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let unidades = &program.library(lib).units;
    let mut funcoes: HashMap<(UnitId, u32), usize> = HashMap::new();
    for (i, e) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = e.node
            && unidades.contains(&unit)
        {
            funcoes.insert((unit, function.0), i);
        }
    }
    let mut campos: HashMap<(UnitId, u32), usize> = HashMap::new();
    for (i, v) in program.variables.iter().enumerate() {
        if let VariableRef::Field {
            unit,
            member,
            index: 0,
        } = v.node
            && unidades.contains(&unit)
        {
            campos.insert((unit, member.0), i);
        }
    }
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let Some(dados) = outline.classes.get(i) else {
            continue;
        };
        let ast_ = &program.unit(decl.unit).ast;
        let (tps, membros): (&[ast::TypeParameter], &[ast::MemberId]) =
            match &ast_.decl(decl.decl).kind {
                DeclKind::Class(d) => (&d.type_params, &d.members),
                DeclKind::Mixin(d) => (&d.type_params, &d.members),
                DeclKind::Enum(d) => (&d.type_params, &d.members),
                DeclKind::ExtensionType(d) => (&d.type_params, &d.members),
                _ => continue,
            };
        if tps.len() != dados.type_params.len() {
            continue;
        }
        let mut diags = Vec::new();
        let relatar = |span: Span, x: TypeParamId, v: V, diags: &mut Vec<Diagnostic>| {
            let d = V::do_parametro(table.param(x).variance);
            if !v.maior_ou_igual(d) {
                let nome = interner.resolve(table.param(x).name);
                diags.push(Diagnostic::com_codigo(
                    c::WRONG_TYPE_PARAMETER_VARIANCE_POSITION,
                    span,
                    [d.palavra(), nome, v.palavra()],
                ));
            }
        };
        let explicitos: Vec<TypeParamId> = dados
            .type_params
            .iter()
            .copied()
            .filter(|&p| table.param(p).variance != Declarada::Unspecified)
            .collect();
        // Membros, na ordem da fonte.
        if !explicitos.is_empty() {
            for &m in membros {
                match &ast_.member(m).kind {
                    MemberKind::Field(vl) => {
                        let Some(primeira) = vl.variables.first() else {
                            continue;
                        };
                        let Some(tipo) = campos
                            .get(&(decl.unit, m.0))
                            .and_then(|&v| outline.variables.get(v))
                            .and_then(|d| d.declared_type.or(d.inferred))
                        else {
                            continue;
                        };
                        for &x in &explicitos {
                            let v = variancia_em(table, outline, x, tipo);
                            relatar(primeira.name.span, x, v, &mut diags);
                            if !vl.final_ && !vl.covariant {
                                relatar(
                                    primeira.name.span,
                                    x,
                                    V::Contravariant.combine(v),
                                    &mut diags,
                                );
                            }
                        }
                    }
                    MemberKind::Method(f) => {
                        let Some(&fid) = funcoes.get(&(decl.unit, f.0)) else {
                            continue;
                        };
                        let Some(dados_f) = outline.functions.get(fid) else {
                            continue;
                        };
                        let af = ast_.function(*f);
                        for &x in &explicitos {
                            for (tp, &p) in af.type_params.iter().zip(dados_f.type_params.iter()) {
                                if tp.bound.is_none() {
                                    continue;
                                }
                                let v = V::Invariant.combine(variancia_em(
                                    table,
                                    outline,
                                    x,
                                    table.param(p).bound,
                                ));
                                relatar(tp.span, x, v, &mut diags);
                            }
                            if let Some(ps) = &af.parameters {
                                for (p, dp) in ps.iter().zip(dados_f.parameters.iter()) {
                                    if p.covariant {
                                        continue;
                                    }
                                    let v = V::Contravariant
                                        .combine(variancia_em(table, outline, x, dp.ty));
                                    relatar(p.span, x, v, &mut diags);
                                }
                            }
                            if let Some(rt) = af.return_type {
                                let v = variancia_em(table, outline, x, dados_f.return_type);
                                relatar(ast_.ty(rt).span, x, v, &mut diags);
                            }
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
        }
        // Supertipos diretos: superclasse, interfaces, mixins e restrições `on`.
        let supers: Vec<TypeId> = dados
            .supertype
            .iter()
            .chain(dados.interfaces.iter())
            .chain(dados.mixins.iter())
            .chain(dados.on.iter())
            .copied()
            .collect();
        for &s in &supers {
            for (tp, &x) in tps.iter().zip(dados.type_params.iter()) {
                let v = variancia_em(table, outline, x, s);
                let d = V::do_parametro(table.param(x).variance);
                if v.maior_ou_igual(d) {
                    continue;
                }
                let nome = interner.resolve(tp.name.sym);
                let exibido = formatar(table, s, interner, program);
                if table.param(x).variance == Declarada::Unspecified {
                    diags.push(Diagnostic::com_codigo(
                        c::WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE,
                        tp.name.span,
                        [nome, exibido.as_str()],
                    ));
                } else {
                    diags.push(Diagnostic::com_codigo(
                        c::WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE,
                        tp.name.span,
                        [nome, d.palavra(), v.palavra(), exibido.as_str()],
                    ));
                }
            }
        }
        saida.extend(diags.into_iter().map(|d| (decl.unit, d)));
    }
    saida
}
