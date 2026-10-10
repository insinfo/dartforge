//! Prova AOT de retorno Owned produzido a partir de um corpo Dart da fonte.
//! O harness nativo observa identidade, sobrevivência após soltar o argumento
//! e morte após soltar o retorno. Não certifica todo o programa/SDK ou dispatch.

use dartforge_emit_native::{
    context::Context, driver, hir::*, llvm::LlvmEmitter, otimizar, otimizar::arc::*,
};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn modulo(entrada: &Path) -> Result<Module, String> {
    std::fs::write(
        entrada,
        "Object? identidade(Object? valor) => valor;\nvoid main() {}\n",
    )
    .map_err(|e| e.to_string())?;
    let lib = std::env::var_os("DARTFORGE_SDK_LIB").ok_or("defina DARTFORGE_SDK_LIB")?;
    let sdk = dartforge_emit_native::sdk_modulo::carregar_sdk_nativo(Path::new(&lib))?;
    let mut interner = Interner::new();
    let (programa, diags) =
        dartforge_elements::load::load_lenient(entrada, &sdk, None, &mut interner);
    if !diags.is_empty() {
        return Err(format!("carga: {diags:?}"));
    }
    let mut tipos = TypeTable::new();
    let core = CoreTypes::init(&mut tipos, &programa, &interner);
    let (mut outline, diags) =
        dartforge_types::resolve_outline(&programa, &interner, &mut tipos, &core);
    let (corpos, diags_corpos) = dartforge_types::infer_program_bodies(
        &programa,
        &interner,
        &mut tipos,
        &core,
        &mut outline,
    );
    for d in diags.iter().chain(&diags_corpos) {
        if dartforge_types::codes::e_erro_de_compilacao(&d.message) {
            return Err(format!("tipagem: {d:?}"));
        }
    }
    let te = dartforge_emit_native::apagamento::calcular(&programa, &outline, &mut tipos);
    let mut ctx = Context::new(&programa, &interner, &tipos, &core, &outline, &corpos);
    ctx.te = te;
    ctx.memoria_arc = true;
    let fonte = dartforge_emit_native::lower::lower_program(&ctx);
    let f = fonte
        .functions
        .iter()
        .find(|f| f.name == "identidade")
        .ok_or("corpo ausente")?;
    if !fonte.retornos_ref_dart.contains(&f.symbol)
        || f.params.len() != 1
        || f.params[0].2 != Type::Ref
    {
        return Err("ABI/fato do lowering incompatível".into());
    }
    // Grupo fechado contendo o corpo real da fonte. A chamada nativa abaixo
    // sustenta o argumento; o produtor deve entregar outro token ao retorno.
    let mut m = Module::new();
    m.memoria_arc = true;
    m.modo_sdk = true;
    m.biblioteca_sdk = true;
    m.functions.push(f.clone());
    m.retornos_ref_dart.insert(f.symbol.clone());
    otimizar::otimizar(&mut m);
    otimizar::excecoes_por_tabelas(&mut m);
    let mut planos = HashMap::from([(f.symbol.clone(), PlanoFuncaoDart::default())]);
    if preparar_arc_modulo_tabelado(&mut m, &mut planos)? != (1, 0) {
        return Err("retenção não produzida".into());
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_tabelado(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
    {
        return Err("preparação não idempotente".into());
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(provar)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "prova entrou em panic".to_string())?
}

fn provar() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(outro) => return Err(format!("modo desconhecido: {outro}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(outro) => return Err(format!("nível desconhecido: {outro}")),
    };
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut m = modulo(&saida.with_extension("dart"))?;
    // Tracing compara o mesmo protocolo explícito de tokens neste harness.
    m.memoria_arc = arc;
    let simbolo = m.functions[0].symbol.clone();
    let mut ir = LlvmEmitter::new(&m).emit_all();
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_retorno_da_fonte() {
  %argumento = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  %retorno = call i64 @SIMBOLO_FONTE(i64 %argumento)
  %igual = icmp eq i64 %retorno, %argumento
  br i1 %igual, label %soltar_argumento, label %falha
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
    ir = ir.replace("SIMBOLO_FONTE", &simbolo);
    ir.push_str("\ndefine void @dartforge_entry() {\n");
    if arc {
        ir.push_str("  call void @dartforge_memoria_arc_v1()\n");
    }
    ir.push_str("  call void @prova_retorno_da_fonte()\n  ret void\n}\n");
    ir.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n  ret i64 0\n}\n");
    if let Some(controle) = args.next() {
        let alvo = match controle.as_str() {
            "sem-retencao" => "  call void @dartforge_arc_retain(i64 %v0)\n",
            "sem-drop-retorno" => "  call void @dartforge_arc_release(i64 %retorno)\n",
            _ => return Err(format!("controle desconhecido: {controle}")),
        };
        if ir.matches(alvo).count() != 1 {
            return Err("controle sem alvo único".into());
        }
        ir = ir.replace(alvo, "");
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )?;
    Ok(())
}
