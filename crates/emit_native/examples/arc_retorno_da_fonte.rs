//! AOT proof of an Owned return and cleanup in a chain of Dart bodies from source.
//! The native harness observes identity, survival after releasing the argument
//! and death after releasing the return. Does not certify the whole program/SDK or dispatch.

use dartforge_emit_native::{
    context::Context, driver, hir::*, llvm::LlvmEmitter, optimize, optimize::arc::*,
};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn module(entry: &Path) -> Result<Module, String> {
    std::fs::write(
        entry,
        "Object? identidade(Object? valor) => valor;\nObject? repassar(Object? valor) => identidade(identidade(valor));\nvoid main() {}\n",
    )
    .map_err(|e| e.to_string())?;
    let lib = std::env::var_os("DARTFORGE_SDK_LIB").ok_or("defina DARTFORGE_SDK_LIB")?;
    let sdk = dartforge_emit_native::sdk_modulo::load_native_sdk(Path::new(&lib))?;
    let mut interner = Interner::new();
    let (program, diags) =
        dartforge_elements::load::load_lenient(entry, &sdk, None, &mut interner);
    if !diags.is_empty() {
        return Err(format!("carga: {diags:?}"));
    }
    let mut types = TypeTable::new();
    let core = CoreTypes::init(&mut types, &program, &interner);
    let (mut outline, diags) =
        dartforge_types::resolve_outline(&program, &interner, &mut types, &core);
    let (bodies, diags_corpos) = dartforge_types::infer_program_bodies(
        &program,
        &interner,
        &mut types,
        &core,
        &mut outline,
    );
    for d in diags.iter().chain(&diags_corpos) {
        if dartforge_types::codes::e_erro_de_compilacao(&d.message) {
            return Err(format!("tipagem: {d:?}"));
        }
    }
    let te = dartforge_emit_native::erasure::compute(&program, &outline, &mut types);
    let mut ctx = Context::new(&program, &interner, &types, &core, &outline, &bodies);
    ctx.te = te;
    ctx.arc_memory = true;
    // The debug mode preserves the call boundaries in the inliner;
    // so the proof exercises the intermediate token instead of only the copied body.
    ctx.link_debug();
    let source = dartforge_emit_native::lower::lower_program(&ctx);
    let f = source
        .functions
        .iter()
        .find(|f| f.name == "repassar")
        .ok_or("corpo ausente")?;
    if !source.dart_ref_returns.contains(&f.symbol)
        || f.params.len() != 1
        || f.params[0].2 != Type::Ref
    {
        return Err("ABI/fato do lowering incompatível".into());
    }
    // A closed group containing the real source bodies. The native call below
    // sustains the argument; the producer must hand another token to the return
    // and release the token of the first call, used as the argument of the second.
    let mut m = Module::new();
    m.arc_memory = true;
    m.sdk_mode = true;
    m.sdk_library = true;
    for body in source
        .functions
        .iter()
        .filter(|f| matches!(f.name.as_str(), "identidade" | "repassar"))
    {
        m.functions.push(body.clone());
        if source.dart_ref_returns.contains(&body.symbol) {
            m.dart_ref_returns.insert(body.symbol.clone());
        }
    }
    optimize::optimize(&mut m);
    optimize::excecoes_por_tabelas(&mut m);
    let mut plans: HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
        .collect();
    let produced = prepare_arc_tabled_module(&mut m, &mut plans)?;
    if produced.0 == 0 || produced.1 == 0 {
        return Err(format!(
            "retenção/cleanup da cadeia não produzidos: {produced:?}"
        ));
    }
    let before = format!("{m:?}/{plans:?}");
    if prepare_arc_tabled_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}/{plans:?}") != before
    {
        return Err("preparação não idempotente".into());
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(prove)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "prova entrou em panic".to_string())?
}

fn prove() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(other) => return Err(format!("modo desconhecido: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("nível desconhecido: {other}")),
    };
    if let Some(dir) = exit.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut m = module(&exit.with_extension("dart"))?;
    // Tracing compares the same explicit token protocol in this harness.
    m.arc_memory = arc;
    let symbol = m
        .functions
        .iter()
        .find(|f| f.name == "repassar")
        .ok_or("cadeia ausente")?
        .symbol
        .clone();
    let mut ir = LlvmEmitter::new(&m).emit_all();
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_retorno_da_fonte() {
  %argumento = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  %retorno = call i64 @SIMBOLO_FONTE(i64 %argumento)
  call void @dartforge_arc_collect()
  %antes = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %tem_owner = icmp eq i8 %antes, 3
  %igual = icmp eq i64 %retorno, %argumento
  %valido = and i1 %igual, %tem_owner
  br i1 %valido, label %soltar_argumento, label %falha
soltar_argumento:
  call void @dartforge_arc_release(i64 %argumento)
  call void @dartforge_arc_collect()
  %vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %owned = icmp eq i8 %vivo, 3
  br i1 %owned, label %soltar_retorno, label %falha
soltar_retorno:
  call void @dartforge_arc_release(i64 %retorno)
  call void @dartforge_arc_collect()
  %fim = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %morto = icmp eq i8 %fim, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    ir = ir.replace("SIMBOLO_FONTE", &symbol);
    ir.push_str("\ndefine void @dartforge_entry() {\n");
    if arc {
        ir.push_str("  call void @dartforge_memoria_arc_v1()\n");
    }
    ir.push_str("  call void @prova_retorno_da_fonte()\n  ret void\n}\n");
    ir.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n  ret i64 0\n}\n");
    if let Some(control) = args.next() {
        let alvo = match control.as_str() {
            "sem-retencao" => "  call void @dartforge_arc_retain(i64 %v0)\n".to_string(),
            "sem-drop-retorno" => "  call void @dartforge_arc_release(i64 %retorno)\n".to_string(),
            "sem-drop-intermediario" => {
                let body = m.functions.iter().find(|f| f.name == "repassar").unwrap();
                let drops: Vec<_> = body
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .filter_map(|(_, i, _)| {
                        if let Instruction::ArcDrop {
                            value: Operand::Val(v),
                        } = i
                        {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if drops.len() != 1 {
                    return Err("cleanup intermediário sem alvo único".into());
                }
                format!("  call void @dartforge_arc_release(i64 %v{})\n", drops[0].0)
            }
            _ => return Err(format!("controle desconhecido: {control}")),
        };
        if ir.matches(&alvo).count() != 1 {
            return Err("controle sem alvo único".into());
        }
        ir = ir.replace(&alvo, "");
    }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &exit,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )?;
    Ok(())
}
