//! Encaminhadores `noSuchMethod` estáticos (casos 57, 216, 223).
//!
//! A especificação (§17.21 "Ordinary Invocation" + §12.12 `noSuchMethod`) e a
//! implementação original da VM (`dart:core` `object.dart` + `invocation.dart`)
//! mandam: uma chamada estática a um membro abstrato sem implementação concreta
//! não é erro de compilação quando a classe concreta tem `noSuchMethod` — o
//! CFE sintetiza um encaminhador com a assinatura completa do membro, que monta
//! a `Invocation` (todos os posicionais, todos os nomeados com defaults, tipos)
//! e chama `noSuchMethod`. O despacho estático vs dinâmico decide: onde o tipo
//! estático resolve o membro (`chamar_membro`), o encaminhador; onde não
//! resolve (`d.qualquerCoisa`), o miss do seletor em runtime.
//!
//! Este módulo implementa o encaminhador **em linha** no ponto da chamada
//! estática (`chamar_membro` sem implementação compilada): monta
//! `Invocation.method(simbolo, posicionais)` pela factory do SDK da fonte e
//! chama o `noSuchMethod` concreto. Só dispara onde antes era
//! "chamada de membro sem implementação compilada"; o resto devolve `None` e o
//! diagnóstico antigo prevalece.
//!
//! Limites da versão atual (cada um vira o próximo item, um por vez):
//! método e getter (não setter), só sem nomeados, só não genérico, só um
//! `noSuchMethod` concreto distinto, só nome público.

use super::fn_builder::FnBuilder;
use super::membros::{Avaliado, linearizacao, subclasse_de};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind};

/// Tenta encaminhar `recv.<membro>(avaliados)` para `noSuchMethod`.
///
/// `None` = fora do escopo da versão atual (o chamador mantém o diagnóstico
/// antigo, sem mudar o placar dos casos restantes).
pub fn encaminhar_metodo_para_nsm(
    b: &mut FnBuilder<'_, '_>,
    recv: Operand,
    decl_fid: usize,
    avaliados: &[Avaliado],
    span: Span,
) -> Option<Operand> {
    if !b.ctx.sdk_da_fonte {
        return None;
    }
    let decl = &b.ctx.program.functions[decl_fid];
    // Só método de instância não estático; o getter tem
    // `encaminhar_getter_para_nsm` (`Invocation.getter`) e o setter é o
    // próximo item (`Invocation.setter`).
    if decl.static_ || decl.factory {
        return None;
    }
    if !matches!(decl.kind, FunctionKind::Function) {
        return None;
    }
    let cdecl = decl.class?;
    // Só posicionais nesta versão; nomeado exige `Map<Symbol, Object?>` com
    // os nomes chegando ao runtime (item do miss dinâmico).
    if avaliados.iter().any(|(n, _)| n.is_some()) {
        return None;
    }
    // Só não genérico; genérico exige `Invocation.genericMethod` + tupla RTI.
    if b.ctx.outline.functions.get(decl_fid).is_some_and(|d| !d.type_params.is_empty()) {
        return None;
    }
    let nome = b.ctx.symbol_name(decl.name).to_string();
    if nome.starts_with('_') {
        // Privado leva `@biblioteca` no `Symbol` (mangled da VM); fica para o
        // item dos nomeados/privados.
        return None;
    }
    let nsm = nsm_concreto_unico(b, cdecl)?;
    let simbolo = simbolo_do_nome(b, &nome, span)?;
    let mut elems = Vec::with_capacity(avaliados.len());
    for (_, v) in avaliados {
        let v = b.coagir(v.clone(), Type::Ref);
        let tag = b.operand_tag(&v);
        elems.push((v, tag));
    }
    let lista = b.emit(Instruction::AllocList { elements: elems }, Type::Ref);
    let invocacao = instanciar_invocation_method(b, simbolo, lista, span)?;
    let args = b.casar_args(nsm, &[(None, invocacao)]);
    let r = b.chamar_direto(nsm, Some(recv), args);
    let ret = b.repr_retorno(decl_fid);
    Some(if matches!(ret, Type::Void) {
        Operand::Constant(Constant::Null)
    } else {
        b.coagir(r, ret)
    })
}

/// Tenta encaminhar a leitura `recv.<getter>` para `noSuchMethod`.
///
/// `None` = fora do escopo da versão atual (o chamador mantém o diagnóstico
/// antigo, sem mudar o placar dos casos restantes). Monta
/// `Invocation.getter(simbolo)` pela factory do SDK da fonte e chama o
/// `noSuchMethod` concreto (caso 216, `p.versao`).
pub fn encaminhar_getter_para_nsm(
    b: &mut FnBuilder<'_, '_>,
    recv: Operand,
    decl_fid: usize,
    avaliados: &[Avaliado],
    span: Span,
) -> Option<Operand> {
    if !b.ctx.sdk_da_fonte {
        return None;
    }
    let decl = &b.ctx.program.functions[decl_fid];
    if decl.static_ || decl.factory {
        return None;
    }
    // Só getter explícito; método, setter e acessor implícito têm seus
    // próprios caminhos.
    if !matches!(decl.kind, FunctionKind::Getter) {
        return None;
    }
    // Leitura de getter não leva argumentos.
    if !avaliados.is_empty() {
        return None;
    }
    let cdecl = decl.class?;
    let nome = b.ctx.symbol_name(decl.name).to_string();
    if nome.starts_with('_') {
        // Privado leva `@biblioteca` no `Symbol` (mangled da VM); fica para o
        // item dos nomeados/privados.
        return None;
    }
    let nsm = nsm_concreto_unico(b, cdecl)?;
    let simbolo = simbolo_do_nome(b, &nome, span)?;
    let invocacao = instanciar_invocation_getter(b, simbolo, span)?;
    let args = b.casar_args(nsm, &[(None, invocacao)]);
    let r = b.chamar_direto(nsm, Some(recv), args);
    let ret = b.repr_retorno(decl_fid);
    Some(if matches!(ret, Type::Void) {
        Operand::Constant(Constant::Null)
    } else {
        b.coagir(r, ret)
    })
}

/// O único `noSuchMethod` concreto distinto nas subclasses concretas de
/// `cdecl` (o encaminhador da classe). `None` sem override (o `Object`
/// padrão só lança) ou com mais de um (o `switch` por classe é o próximo
/// item depois dos nomeados).
fn nsm_concreto_unico(b: &FnBuilder<'_, '_>, cdecl: ClassId) -> Option<usize> {
    let sym = b.ctx.interner.lookup("noSuchMethod")?;
    let objeto = b.ctx.classe_do_sdk("core", "Object");
    let mut distintos: Vec<usize> = Vec::new();
    for (k, classe) in b.ctx.program.classes.iter().enumerate() {
        let kid = ClassId(k as u32);
        if !b.ctx.biblioteca_compilada(classe.library)
            || classe.modifiers.abstract_
            || super::membros::e_mixin(b.ctx, kid)
            || !subclasse_de(b.ctx, kid, cdecl)
        {
            continue;
        }
        for c in linearizacao(b.ctx, kid) {
            let cl = &b.ctx.program.classes[c.0 as usize];
            if !b.ctx.biblioteca_compilada(cl.library) {
                break;
            }
            if let Some(&f) = cl.instance_members.get(&sym) {
                let f = f.0 as usize;
                let func = &b.ctx.program.functions[f];
                // O padrão de `Object` (external que lança) não é override:
                // ele não salva a chamada, só mantém o erro antigo.
                if func.class == objeto {
                    break;
                }
                if super::membros::tem_corpo(b.ctx, f) && super::funcao_do_usuario(b.ctx, f) {
                    if !distintos.contains(&f) {
                        distintos.push(f);
                    }
                    break;
                }
            }
        }
    }
    match distintos[..] {
        [unico] => Some(unico),
        _ => None,
    }
}

/// `Symbol(<nome>)` pelo construtor de `dart:_internal.Symbol`.
fn simbolo_do_nome(b: &mut FnBuilder<'_, '_>, nome: &str, span: Span) -> Option<Operand> {
    let classe = b.ctx.classe_do_sdk("_internal", "Symbol")?;
    let vazio = b.ctx.interner.lookup("")?;
    let ctor = *b.ctx.program.classes[classe.0 as usize].constructors.get(&vazio)?;
    let texto = b.emit(Instruction::Const(Constant::String(nome.to_string())), Type::Ref);
    Some(b.instanciar_avaliados(ctor, &[(None, texto)], span))
}

/// `Invocation.getter(simbolo)` pela factory do SDK da fonte.
fn instanciar_invocation_getter(
    b: &mut FnBuilder<'_, '_>,
    simbolo: Operand,
    span: Span,
) -> Option<Operand> {
    let classe = b.ctx.classe_do_sdk("core", "Invocation")?;
    let getter = b.ctx.interner.lookup("getter")?;
    let ctor = *b.ctx.program.classes[classe.0 as usize].constructors.get(&getter)?;
    Some(b.instanciar_avaliados(ctor, &[(None, simbolo)], span))
}

/// `Invocation.method(simbolo, posicionais)` pela factory do SDK da fonte.
fn instanciar_invocation_method(
    b: &mut FnBuilder<'_, '_>,
    simbolo: Operand,
    posicionais: Operand,
    span: Span,
) -> Option<Operand> {
    let classe = b.ctx.classe_do_sdk("core", "Invocation")?;
    let metodo = b.ctx.interner.lookup("method")?;
    let ctor = *b.ctx.program.classes[classe.0 as usize].constructors.get(&metodo)?;
    Some(b.instanciar_avaliados(ctor, &[(None, simbolo), (None, posicionais)], span))
}
