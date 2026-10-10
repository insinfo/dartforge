//! Fatos declarados de layout; não certificam receivers nem versões do heap.

use crate::{context::Context, hir::*};
use dartforge_elements::model::{VariableId, VariableRef};
use dartforge_frontend::ast::{DeclKind, ExprId, MemberKind};

pub(super) fn inicializador(ctx: &Context, vid: VariableId) -> Option<ExprId> {
    match ctx.program.variables[vid.0 as usize].node {
        VariableRef::Field {
            unit,
            member,
            index,
        } => match &ctx.program.unit(unit).ast.member(member).kind {
            MemberKind::Field(list) => list.variables.get(index)?.initializer,
            _ => None,
        },
        VariableRef::TopLevel { unit, decl, index } => {
            match &ctx.program.unit(unit).ast.decl(decl).kind {
                DeclKind::Variables(list) => list.variables.get(index)?.initializer,
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn representacao(ctx: &Context, vid: VariableId) -> Type {
    let repr = match ctx.to_hir_type(super::membros::tipo_da_variavel(ctx, vid)) {
        Type::Void => Type::Ref,
        t => t,
    };
    if ctx.program.variables[vid.0 as usize].late
        && repr != Type::Ref
        && inicializador(ctx, vid).is_some()
    {
        Type::Ref
    } else {
        repr
    }
}

pub(super) fn registrar(
    ctx: &Context,
    module: &mut Module,
    classe: u32,
    campos: &[VariableId],
    base: usize,
) {
    if !ctx.memoria_arc {
        return;
    }
    // Enum guarda index int e name Ref antes dos campos declarados.
    let mut tipos = if base == 2 {
        vec![Type::I64, Type::Ref]
    } else {
        vec![]
    };
    assert_eq!(tipos.len(), base, "prefixo de layout desconhecido");
    tipos.extend(campos.iter().map(|v| representacao(ctx, *v)));
    module.layouts_campos_arc.insert(classe, tipos);
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn fonte_preserva_layout_herdado_mixin_enum_e_late_somente_em_arc() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entrada = dir.path().join("main.dart");
                std::fs::write(
                    &entrada,
                    r#"
class Base {
  int inteiro = 1;
  double fracao = 3.25;
  bool ligado = true;
  Object? referencia;
  late int pendente;
  late double preparado = 2.5;
  static int fora = 0;
}
mixin Mistura { bool misto = false; }
class Folha extends Base with Mistura { String? texto; }
enum Cor { azul, vermelho }
void main() {}
"#,
                )
                .unwrap();
                let sdk = crate::sdk_modulo::carregar_sdk_nativo(crate::sdk_testes()).unwrap();
                let mut interner = Interner::new();
                let (program, diags) =
                    dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &program, &interner);
                let (mut outline, diags) =
                    dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
                let (bodies, diags_corpos) = dartforge_types::infer_program_bodies(
                    &program,
                    &interner,
                    &mut table,
                    &core,
                    &mut outline,
                );
                for d in diags.iter().chain(diags_corpos.iter()) {
                    assert!(
                        !dartforge_types::codes::e_erro_de_compilacao(&d.message),
                        "{d:?}"
                    );
                }
                let te = crate::apagamento::calcular(&program, &outline, &mut table);
                let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
                ctx.te = te;
                let base = vec![
                    Type::I64,
                    Type::F64,
                    Type::I1,
                    Type::Ref,
                    Type::I64,
                    Type::Ref,
                ];
                let mut folha = base.clone();
                folha.extend([Type::I1, Type::Ref]);
                for arc in [false, true] {
                    ctx.memoria_arc = arc;
                    let mut modulo = crate::lower::lower_program(&ctx);
                    if !arc {
                        assert!(modulo.layouts_campos_arc.is_empty());
                        continue;
                    }
                    for (nome, esperado) in [
                        ("Base", &base),
                        ("Folha", &folha),
                        ("Cor", &vec![Type::I64, Type::Ref]),
                    ] {
                        let classe = modulo.classes.iter().find(|c| c.name == nome).unwrap();
                        assert_eq!(modulo.layouts_campos_arc[&classe.id], *esperado, "{nome}");
                        assert_eq!(classe.field_count, esperado.len());
                    }
                    let antes = modulo.layouts_campos_arc.clone();
                    crate::otimizar::otimizar(&mut modulo);
                    assert_eq!(modulo.layouts_campos_arc, antes);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
