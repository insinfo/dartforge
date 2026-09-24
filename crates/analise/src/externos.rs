//! Inicializadores em campos e variáveis `external`, conforme
//! `_checkForAbstractOrExternalVariableInitializer` e
//! `_checkForAbstractOrExternalFieldConstructorInitializer` do
//! `ErrorVerifier` do analyzer. O erro fica no nome da variável/campo.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{DeclKind, Initializer, MemberId, MemberKind};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

fn relatar(codigo: dartforge_diagnostics::Codigo, span: Span) -> Diagnostic {
    Diagnostic::com_codigo(codigo, span, std::iter::empty::<&str>())
}

/// Diagnósticos desta unidade que não exigem inferência de tipos.
pub fn inicializadores(unidade: Unidade<'_>) -> Vec<Diagnostic> {
    let ast = unidade.ast;
    let mut out = Vec::new();
    for &id in &unidade.unit.declarations {
        match &ast.decl(id).kind {
            DeclKind::Variables(v) => {
                if v.external {
                    for var in v.variables.iter() {
                        if var.initializer.is_some() {
                            out.push(relatar(c::EXTERNAL_VARIABLE_INITIALIZER, var.name.span));
                        }
                    }
                }
            }
            kind => {
                let members: &[MemberId] = match kind {
                    DeclKind::Class(x) => &x.members,
                    DeclKind::Mixin(x) => &x.members,
                    DeclKind::Enum(x) => &x.members,
                    DeclKind::Extension(x) => &x.members,
                    DeclKind::ExtensionType(x) => &x.members,
                    DeclKind::Typedef(_) | DeclKind::Function(_) | DeclKind::Variables(_) => continue,
                };
                let mut campos_externos = HashSet::<SymbolId>::new();
                for &m in members {
                    if let MemberKind::Field(v) = &ast.member(m).kind {
                        if v.external {
                            for var in v.variables.iter() {
                                if !v.static_ {
                                    campos_externos.insert(var.name.sym);
                                }
                                if var.initializer.is_some() {
                                    out.push(relatar(c::EXTERNAL_FIELD_INITIALIZER, var.name.span));
                                }
                            }
                        }
                    }
                }
                for &m in members {
                    if let MemberKind::Constructor(k) = &ast.member(m).kind {
                        for p in k.parameters.iter() {
                            if p.this_ {
                                if let Some(nome) = p.name.filter(|n| campos_externos.contains(&n.sym)) {
                                    out.push(relatar(c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER, nome.span));
                                }
                            }
                        }
                        for init in &k.initializers {
                            if let Initializer::Field { name, .. } = init {
                                if campos_externos.contains(&name.sym) {
                                    out.push(relatar(c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER, name.span));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::parser::parse;
    use dartforge_intern::Interner;

    #[test]
    fn tres_formas_iguais_ao_oraculo_gravado() {
        for (fonte, codigo, inicio, mensagem, correcao) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__topLevelVariabl_9e68b99b.dart")),
                c::EXTERNAL_VARIABLE_INITIALIZER,
                13,
                "External variables can't have initializers.",
                "Try removing the initializer or the 'external' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_2eafc447.dart")),
                c::EXTERNAL_FIELD_INITIALIZER,
                31,
                "External fields can't have initializers.",
                "Try removing the initializer or the 'external' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/external_field_constructor_initializer/ExternalFieldConstructorInitializer__ex_7c1a26a5.dart")),
                c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                43,
                "External fields can't have initializers.",
                "Try removing the field initializer or the 'external' keyword from the field declaration.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/external_field_constructor_initializer/ExternalFieldConstructorInitializer__ex_4fd7b47b.dart")),
                c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                42,
                "External fields can't have initializers.",
                "Try removing the field initializer or the 'external' keyword from the field declaration.",
            ),
        ] {
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = inicializadores(Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte });
            assert!(achados.iter().any(|d|
                d.code == Some(codigo) && d.span.start == inicio && d.span.end == inicio + 1
                    && d.message == mensagem && d.correcao().as_deref() == Some(correcao)
            ), "{fonte}: {achados:?}");
        }
    }

    #[test]
    fn externo_sem_inicializador_e_campo_estatico_nao_criam_erro() {
        let fonte = "external int topo; class A { external static int x; A() : x = 1; }";
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        let achados = inicializadores(Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte });
        assert!(achados.is_empty(), "{achados:?}");
    }
}
