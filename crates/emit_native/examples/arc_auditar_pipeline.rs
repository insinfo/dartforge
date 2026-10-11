//! Diagnóstico da preparação ARC sobre módulos completos baixados da fonte.
//! Não publica corpos nem emite um programa com convenções parcialmente mudadas.

use dartforge_emit_native::{context::Context, driver, otimizar, otimizar::arc::*, sdk_modulo};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::{collections::HashMap, path::Path};

fn auditar(entrada: &Path) -> Result<(), String> {
    let lib = std::env::var_os("DARTFORGE_SDK_LIB").ok_or("defina DARTFORGE_SDK_LIB")?;
    let sdk_dir = Path::new(&lib);
    let sdk = sdk_modulo::carregar_sdk_nativo(sdk_dir)?;
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
    ctx.mundo = dartforge_emit_native::mundo_nativo::calcular(
        &programa, &interner, &tipos, &outline, &corpos,
    );
    let ctx = ctx.com_sdk_da_fonte_e_ids(sdk_modulo::ids_de_classe_do_sdk(sdk_dir)?);
    let mut m = dartforge_emit_native::lower::lower_program(&ctx);
    m.ids_do_programa = ctx.ids_do_programa();
    m.campos_do_programa = ctx.campos_do_programa();
    m.registros_do_sdk = sdk_modulo::registros_do_sdk();
    m.cids_do_runtime = sdk_modulo::cids_do_runtime(&ctx);
    if !m.erros.is_empty() {
        return Err(format!("lowering: {:?}", m.erros));
    }
    let clang = driver::NativeDriverOptions::default().clang;
    let sdk_compilado =
        sdk_modulo::sdk_compilado_no_perfil(sdk_dir, &clang, sdk_modulo::PerfilDoSdk::Producao)?;
    let resumos = dartforge_emit_native::poda::ler_resumos(&sdk_compilado.resumos)?;
    let refs = resumos.iter().map(|r| r.as_ref()).collect::<Vec<_>>();
    dartforge_emit_native::poda::podar_hir(&mut m, &refs);
    otimizar::otimizar(&mut m);
    let por_tabelas = dartforge_emit_native::alvo::excecoes_por_tabelas()?;
    if por_tabelas {
        otimizar::excecoes_por_tabelas(&mut m);
    }
    let mut planos: HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
        .collect();
    println!(
        "{}: {} funções, {} layouts, {} sítios",
        entrada.display(),
        m.functions.len(),
        m.layouts_campos_arc.len(),
        m.sitios_arc.len()
    );
    let antes = format!("{m:?}/{planos:?}");
    let resultado = if por_tabelas {
        preparar_arc_modulo_tabelado(&mut m, &mut planos)
    } else {
        preparar_arc_modulo_dart(&mut m, &mut planos)
    };
    println!("Preparação do módulo inteiro: {resultado:?}");
    if let Err(erro) = resultado {
        if format!("{m:?}/{planos:?}") != antes {
            return Err("preparação recusada alterou o módulo ou os planos".into());
        }
        for f in &m.functions {
            if erro.contains(&f.symbol) || f.blocks.iter().flat_map(|b| &b.instructions)
                .any(|(_, i, _)| match i {
                    dartforge_emit_native::hir::Instruction::CallRuntime { name, .. } => erro.contains(name),
                    dartforge_emit_native::hir::Instruction::CallStatic { symbol, .. } => erro.contains(symbol),
                    _ => false,
                }) {
                println!("Corpo associado ao diagnóstico: {f:#?}");
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let mut entradas = Vec::new();
            let mut selecionou_arc = false;
            for argumento in std::env::args_os().skip(1) {
                if argumento == "--memoria=arc" {
                    selecionou_arc = true;
                } else {
                    entradas.push(argumento);
                }
            }
            // A seleção explícita do diagnóstico deve coincidir com a chave
            // de memória usada pelo cache/compilação das bibliotecas do SDK.
            if !selecionou_arc || !dartforge_emit_native::alvo::memoria_arc()? {
                return Err("auditoria exige --memoria=arc e DARTFORGE_MEMORIA=arc".into());
            }
            if entradas.is_empty() {
                return Err("informe os arquivos Dart a auditar".into());
            }
            for entrada in entradas {
                auditar(Path::new(&entrada))?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "panic na auditoria".to_string())?
}
