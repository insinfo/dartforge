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
                let imports = crate::sdk_modulo::BIBLIOTECAS_DA_FONTE
                    .iter()
                    .map(|lib| format!("import 'dart:{lib}';\n"))
                    .collect::<String>();
                std::fs::write(
                    &entrada,
                    format!(
                        "{imports}{}",
                        r#"
class Base {
  int inteiro = 1;
  double fracao = 3.25;
  bool ligado = true;
  Object? referencia;
  late int pendente;
  late double preparado = 2.5;
  static int fora = 0;
  Object? identidade(Object? valor) => valor;
  Object? get atual => referencia;
  bool exercitar(Object valor) {
    inteiro = 7;
    fracao = -0.0;
    ligado = false;
    referencia = valor;
    pendente = 9;
    preparado = 4.5;
    return inteiro == 7 && fracao == 0.0 && !ligado &&
        referencia == valor && pendente == 9 && preparado == 4.5;
  }
}
mixin Mistura { bool misto = false; }
class Folha extends Base with Mistura { String? texto; }
enum Cor { azul, vermelho }
void main() { Base().exercitar(Object()); }
"#
                    ),
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
                    let acessos = modulo
                        .functions
                        .iter()
                        .filter(|f| f.name.contains("exercitar"))
                        .flat_map(|f| &f.blocks)
                        .flat_map(|b| &b.instructions)
                        .collect::<Vec<_>>();
                    assert!(!acessos.is_empty(), "corpo do método ausente");
                    if !arc {
                        assert!(modulo.layouts_campos_arc.is_empty());
                        assert!(modulo.retornos_ref_dart.is_empty());
                        assert!(modulo.sitios_arc.is_empty());
                        assert!(!acessos.iter().any(|(_, i, _)| matches!(
                            i,
                            Instruction::GetField { .. } | Instruction::SetField { .. }
                        )));
                        continue;
                    }
                    assert!(!modulo.sitios_arc.is_empty());
                    for f in &modulo.functions {
                        for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                            if crate::otimizar::arc::analise::origens::alocacao(i) {
                                let sitio = &modulo.sitios_arc[&f.symbol][v];
                                assert_eq!(sitio.funcao, f.symbol);
                            }
                        }
                    }
                    let principal = modulo.functions.iter().find(|f| f.name == "main").unwrap();
                    let analise = crate::otimizar::arc::analise::modulo::analisar_no_modulo(
                        &modulo, &principal.symbol, 8,
                    ).unwrap().unwrap();
                    let mut alocacoes = 0;
                    for (v, i, _) in principal.blocks.iter().flat_map(|b| &b.instructions) {
                        if matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_object_new") {
                            if let Some(nos) = analise.valor(*v).and_then(|p| p.nos()) {
                                assert_eq!(nos.len(), 1);
                                let esperado = &modulo.sitios_arc[&principal.symbol][v];
                                assert!(nos.iter().all(|n| matches!(n,
                                    crate::otimizar::arc::analise::modelo::NoAbstrato::Alocacao { sitio, .. }
                                        if sitio == esperado)));
                                alocacoes += 1;
                            }
                        }
                    }
                    assert!(alocacoes > 0, "origem/layout da fonte não alimentou points-to");
                    for (indice, tipo) in [
                        (0, Type::I64),
                        (1, Type::F64),
                        (2, Type::I1),
                        (3, Type::Ref),
                    ] {
                        assert!(
                            acessos.iter().any(|(_, i, t)| matches!(i,
                            Instruction::GetField { index, .. } if *index == indice)
                                && *t == tipo),
                            "leitura {indice}: {tipo:?}"
                        );
                        assert!(
                            acessos.iter().any(|(_, i, _)| matches!(i,
                            Instruction::SetField { index, .. } if *index == indice)),
                            "gravação {indice}"
                        );
                    }
                    // late conserva seu protocolo, inclusive os indicadores de
                    // inicialização; não pode virar um acesso direto tipado.
                    assert!(!acessos.iter().any(|(_, i, _)| matches!(
                        i,
                        Instruction::GetField { index: 4 | 5, .. }
                            | Instruction::SetField { index: 4 | 5, .. }
                    )));
                    assert!(acessos.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. }
                        if name == "dartforge_late_field_initialized")));
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
                    let identidade = modulo
                        .functions
                        .iter()
                        .find(|f| f.name == "identidade")
                        .unwrap();
                    assert!(modulo.retornos_ref_dart.contains(&identidade.symbol));
                    assert!(
                        modulo
                            .functions
                            .iter()
                            .filter(|f| f.name == "atual")
                            .any(|f| modulo.retornos_ref_dart.contains(&f.symbol))
                    );
                    assert!(
                        modulo
                            .functions
                            .iter()
                            .filter(|f| f.name == "exercitar")
                            .all(|f| !modulo.retornos_ref_dart.contains(&f.symbol))
                    );
                    // Corpo realmente baixado da fonte: apenas o fato nominal
                    // de retorno, sem plano de ownership escrito para o método.
                    let mut prova = Module::new();
                    prova.functions.push(identidade.clone());
                    prova.retornos_ref_dart.insert(identidade.symbol.clone());
                    crate::otimizar::preparar_para_emissao(&mut prova, true, true);
                    let mut planos = std::collections::HashMap::from([(
                        identidade.symbol.clone(),
                        crate::otimizar::arc::PlanoFuncaoDart::default(),
                    )]);
                    assert_eq!(
                        crate::otimizar::arc::preparar_arc_modulo_tabelado(&mut prova, &mut planos)
                            .unwrap(),
                        (1, 0)
                    );
                    assert_eq!(
                        planos[&identidade.symbol].tokens.retorno,
                        crate::otimizar::arc::RetornoTokens::Owned
                    );
                    let ir = crate::llvm::LlvmEmitter::new(&prova).emit_all();
                    assert!(ir.contains("call void @dartforge_arc_retain("));
                }
                // Usa a mesma etapa de registro da produção para auditar os
                // layouts declarados do SDK, sem confundir isso com seus corpos
                // ou com a representação física das instâncias no runtime.
                let mut auditadas = 0;
                for (i, lib) in program.libraries.iter().enumerate() {
                    let Some(nome) = lib.uri.strip_prefix("dart:") else {
                        continue;
                    };
                    if !crate::sdk_modulo::BIBLIOTECAS_DA_FONTE.contains(&nome) {
                        continue;
                    }
                    auditadas += 1;
                    let mut sdk_ctx =
                        Context::new(&program, &interner, &table, &core, &outline, &bodies)
                            .com_sdk_da_fonte()
                            .so_a_biblioteca(dartforge_elements::model::LibraryId(i as u32));
                    sdk_ctx.te = std::mem::take(&mut ctx.te);
                    sdk_ctx.memoria_arc = true;
                    let mut m = Module::default();
                    m.memoria_arc = true;
                    m.classes.push(ClassDef {
                        id: 0,
                        name: "Object".into(),
                        field_count: 0,
                        vtable: vec![],
                        to_string_symbol: None,
                    });
                    super::super::registrar_classes_do_programa(&sdk_ctx, &mut m);
                    let mut planos = std::collections::HashMap::new();
                    assert_eq!(
                        crate::otimizar::arc::preparar_arc_modulo_dart(&mut m, &mut planos)
                            .unwrap_or_else(|e| panic!(
                                "{}: {e}; classes {:?}",
                                lib.uri, m.classes
                            )),
                        (0, 0)
                    );
                    ctx.te = std::mem::take(&mut sdk_ctx.te);
                }
                assert_eq!(auditadas, crate::sdk_modulo::BIBLIOTECAS_DA_FONTE.len());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
