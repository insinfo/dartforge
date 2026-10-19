//! Declared layout facts; they do not certify receivers nor heap versions.

use crate::{context::Context, hir::*};
use dartforge_elements::model::{VariableId, VariableRef};
use dartforge_frontend::ast::{DeclKind, ExprId, MemberKind};

pub(super) fn initializer(ctx: &Context, vid: VariableId) -> Option<ExprId> {
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

pub(super) fn representation(ctx: &Context, vid: VariableId) -> Type {
    let repr = match ctx.to_hir_type(super::members::variable_type(ctx, vid)) {
        Type::Void => Type::Ref,
        t => t,
    };
    if ctx.program.variables[vid.0 as usize].late
        && repr != Type::Ref
        && initializer(ctx, vid).is_some()
    {
        Type::Ref
    } else {
        repr
    }
}

pub(super) fn register(
    ctx: &Context,
    module: &mut Module,
    class: u32,
    fields: &[VariableId],
    base: usize,
) {
    if !ctx.arc_memory {
        return;
    }
    // An enum keeps the int index and the Ref name before the declared fields.
    let mut types = if base == 2 {
        vec![Type::I64, Type::Ref]
    } else {
        vec![]
    };
    assert_eq!(types.len(), base, "unknown layout prefix");
    types.extend(fields.iter().map(|v| representation(ctx, *v)));
    module.arc_field_layouts.insert(class, types);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn source_preserves_inherited_mixin_enum_and_late_layout_only_in_arc() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entry = dir.path().join("main.dart");
                let imports = crate::sdk_module::SOURCE_LIBRARIES
                    .iter()
                    .map(|lib| format!("import 'dart:{lib}';\n"))
                    .collect::<String>();
                std::fs::write(
                    &entry,
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
                let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
                let mut interner = Interner::new();
                let (program, diags) =
                    dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &program, &interner);
                let (mut outline, diags) =
                    dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
                let (bodies, body_diags) = dartforge_types::infer_program_bodies(
                    &program,
                    &interner,
                    &mut table,
                    &core,
                    &mut outline,
                );
                for d in diags.iter().chain(body_diags.iter()) {
                    assert!(
                        !dartforge_types::codes::is_compile_error(&d.message),
                        "{d:?}"
                    );
                }
                let te = crate::erasure::compute(&program, &outline, &mut table);
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
                let mut leaf = base.clone();
                leaf.extend([Type::I1, Type::Ref]);
                for arc in [false, true] {
                    ctx.arc_memory = arc;
                    let mut module = crate::lower::lower_program(&ctx);
                    let accesses = module
                        .functions
                        .iter()
                        .filter(|f| f.name.contains("exercitar"))
                        .flat_map(|f| &f.blocks)
                        .flat_map(|b| &b.instructions)
                        .collect::<Vec<_>>();
                    assert!(!accesses.is_empty(), "missing method body");
                    if !arc {
                        assert!(module.arc_field_layouts.is_empty());
                        assert!(module.dart_ref_returns.is_empty());
                        assert!(module.arc_sites.is_empty());
                        assert!(!accesses.iter().any(|(_, i, _)| matches!(
                            i,
                            Instruction::GetField { .. } | Instruction::SetField { .. }
                        )));
                        continue;
                    }
                    assert!(!module.arc_sites.is_empty());
                    for f in &module.functions {
                        for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                            if crate::optimize::arc::analysis::origins::allocation(i) {
                                let site = &module.arc_sites[&f.symbol][v];
                                assert_eq!(site.function, f.symbol);
                            }
                        }
                    }
                    let main = module.functions.iter().find(|f| f.name == "main").unwrap();
                    let analysis = crate::optimize::arc::analysis::module::analyze_in_module(
                        &module, &main.symbol, 8,
                    ).unwrap().unwrap();
                    let mut allocations = 0;
                    for (v, i, _) in main.blocks.iter().flat_map(|b| &b.instructions) {
                        if matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_object_new")
                            && let Some(nodes) = analysis.value(*v).and_then(|p| p.nodes()) {
                                assert_eq!(nodes.len(), 1);
                                let expected = &module.arc_sites[&main.symbol][v];
                                assert!(nodes.iter().all(|n| matches!(n,
                                    crate::optimize::arc::analysis::model::AbstractNode::Allocation { site, .. }
                                        if site == expected)));
                                allocations += 1;
                            }
                    }
                    assert!(allocations > 0, "source origin/layout did not feed points-to");
                    for (idx_value, ty) in [
                        (0, Type::I64),
                        (1, Type::F64),
                        (2, Type::I1),
                        (3, Type::Ref),
                    ] {
                        assert!(
                            accesses.iter().any(|(_, i, t)| matches!(i,
                            Instruction::GetField { index, .. } if *index == idx_value)
                                && *t == ty),
                            "read {idx_value}: {ty:?}"
                        );
                        assert!(
                            accesses.iter().any(|(_, i, _)| matches!(i,
                            Instruction::SetField { index, .. } if *index == idx_value)),
                            "store {idx_value}"
                        );
                    }
                    // Late reads still keep the inspection of the empty state.
                    // Typed writes keep the later initialization mark.
                    assert!(!accesses.iter().any(|(_, i, _)| matches!(
                        i,
                        Instruction::GetField { index: 4 | 5, .. }
                    )));
                    assert!(accesses.iter().any(|(_, i, _)| matches!(i,
                        Instruction::SetField { index: 4, .. })));
                    assert!(accesses.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. }
                        if name == "dartforge_late_field_mark_initialized")));
                    assert!(accesses.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. }
                        if name == "dartforge_late_field_initialized")));
                    for (name_str, expected) in [
                        ("Base", &base),
                        ("Folha", &leaf),
                        ("Cor", &vec![Type::I64, Type::Ref]),
                    ] {
                        let class = module.classes.iter().find(|c| c.name == name_str).unwrap();
                        assert_eq!(module.arc_field_layouts[&class.id], *expected, "{name_str}");
                        assert_eq!(class.field_count, expected.len());
                    }
                    let before = module.arc_field_layouts.clone();
                    crate::optimize::optimize(&mut module);
                    assert_eq!(module.arc_field_layouts, before);
                    let identity = module
                        .functions
                        .iter()
                        .find(|f| f.name == "identidade")
                        .unwrap();
                    assert!(module.dart_ref_returns.contains(&identity.symbol));
                    let summary = crate::optimize::arc::analysis::summaries::extract(identity, 8).unwrap();
                    assert_eq!(summary.fields(), crate::optimize::arc::analysis::summaries::ArcHeapFields::Preserved);
                    assert!(matches!(summary.ret(),
                        crate::optimize::arc::analysis::summaries::ArcHeapReturn::Aliases { parameters, .. }
                            if !parameters.is_empty() && parameters.iter().all(|&p| identity.params[p].2 == Type::Ref)));
                    assert!(
                        module
                            .functions
                            .iter()
                            .filter(|f| f.name == "atual")
                            .any(|f| module.dart_ref_returns.contains(&f.symbol))
                    );
                    assert!(
                        module
                            .functions
                            .iter()
                            .filter(|f| f.name == "exercitar")
                            .all(|f| !module.dart_ref_returns.contains(&f.symbol))
                    );
                    // A body really lowered from source: only the nominal return
                    // fact, with no ownership plan written for the method.
                    let mut proof = Module::new();
                    proof.functions.push(identity.clone());
                    proof.dart_ref_returns.insert(identity.symbol.clone());
                    crate::optimize::prepare_for_emission(&mut proof, true, true);
                    let mut plans = std::collections::HashMap::from([(
                        identity.symbol.clone(),
                        crate::optimize::arc::DartFunctionPlan::default(),
                    )]);
                    assert_eq!(
                        crate::optimize::arc::prepare_arc_tabled_module(&mut proof, &mut plans)
                            .unwrap(),
                        (1, 0)
                    );
                    assert_eq!(
                        plans[&identity.symbol].tokens.ret,
                        crate::optimize::arc::TokenReturn::Owned
                    );
                    let ir = crate::llvm::LlvmEmitter::new(&proof).emit_all();
                    assert!(ir.contains("call void @dartforge_arc_retain("));
                }
                // Uses the same registration step as production to audit the
                // declared layouts of the SDK, without confusing that with their bodies
                // or with the physical representation of the instances in the runtime.
                let mut audited = 0;
                for (i, lib) in program.libraries.iter().enumerate() {
                    let Some(name_str) = lib.uri.strip_prefix("dart:") else {
                        continue;
                    };
                    if !crate::sdk_module::SOURCE_LIBRARIES.contains(&name_str) {
                        continue;
                    }
                    audited += 1;
                    let mut sdk_ctx =
                        Context::new(&program, &interner, &table, &core, &outline, &bodies)
                            .with_source_sdk()
                            .only_the_library(dartforge_elements::model::LibraryId(i as u32));
                    sdk_ctx.te = std::mem::take(&mut ctx.te);
                    sdk_ctx.arc_memory = true;
                    let mut m = Module { arc_memory: true, ..Module::default() };
                    m.classes.push(ClassDef {
                        id: 0,
                        name: "Object".into(),
                        field_count: 0,
                        vtable: vec![],
                        to_string_symbol: None,
                    });
                    super::super::register_program_classes(&sdk_ctx, &mut m);
                    let mut plans = std::collections::HashMap::new();
                    assert_eq!(
                        crate::optimize::arc::prepare_arc_dart_module(&mut m, &mut plans)
                            .unwrap_or_else(|e| panic!(
                                "{}: {e}; classes {:?}",
                                lib.uri, m.classes
                            )),
                        (0, 0)
                    );
                    ctx.te = std::mem::take(&mut sdk_ctx.te);
                }
                assert_eq!(audited, crate::sdk_module::SOURCE_LIBRARIES.len());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
