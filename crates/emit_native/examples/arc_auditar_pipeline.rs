//! Diagnóstico da preparação ARC sobre módulos completos baixados da fonte.
//! Não publica corpos nem emite um programa com convenções parcialmente mudadas.

use dartforge_emit_native::{context::Context, driver, otimizar, otimizar::arc::*, sdk_modulo};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::{collections::HashMap, path::Path};

fn auditar(entrada: &Path, sdk_no_grupo: bool) -> Result<(), String> {
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
    if !sdk_no_grupo {
        let clang = driver::NativeDriverOptions::default().clang;
        let sdk_compilado = sdk_modulo::sdk_compilado_no_perfil(
            sdk_dir,
            &clang,
            sdk_modulo::PerfilDoSdk::Producao,
        )?;
        let resumos = dartforge_emit_native::poda::ler_resumos(&sdk_compilado.resumos)?;
        let refs = resumos.iter().map(|r| r.as_ref()).collect::<Vec<_>>();
        dartforge_emit_native::poda::podar_hir(&mut m, &refs);
    }
    otimizar::otimizar(&mut m);
    let por_tabelas = dartforge_emit_native::alvo::excecoes_por_tabelas()?;
    if por_tabelas {
        otimizar::excecoes_por_tabelas(&mut m);
    }
    if sdk_no_grupo {
        let mut modulos = vec![m];
        modulos.extend(baixar_sdk_para_auditoria(sdk_dir, por_tabelas)?);
        let mut planos: Vec<HashMap<_, _>> = modulos
            .iter()
            .map(|m| {
                m.functions
                    .iter()
                    .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
                    .collect()
            })
            .collect();
        let quantidade: usize = modulos.iter().map(|m| m.functions.len()).sum();
        let stopwatch = modulos
            .iter()
            .flat_map(|m| &m.functions)
            .filter(|f| f.symbol == "df.dart$3acore.Stopwatch.new")
            .count();
        println!(
            "{}: {} módulos, {quantidade} funções, {stopwatch} corpo(s) de Stopwatch.new",
            entrada.display(),
            modulos.len()
        );
        for (indice, modulo) in modulos.iter().enumerate() {
            match auditar_slots_ref_compartilhados(modulo) {
                Ok(slots) => println!(
                    "Slots Ref compartilhados no módulo {indice}: {} funções, {} endereços",
                    slots.len(), slots.values().map(|s| s.len()).sum::<usize>()
                ),
                Err(erro) => println!(
                    "Slots Ref compartilhados no módulo {indice}: {erro}"
                ),
            }
            match baixar_slots_ref_compartilhados(modulo) {
                Ok(rascunho) => println!(
                    "Baixamento intermediário dos slots no módulo {indice}: {} quadros locais, {} assinaturas novas",
                    rascunho.locais.len(), rascunho.simbolos.len()
                ),
                Err(erro) => println!("Baixamento intermediário dos slots no módulo {indice}: {erro}"),
            }
        }
        let antes = format!("{modulos:?}/{planos:?}");
        let resultado = preparar_arc_modulos_dart(&mut modulos, &mut planos);
        println!("Preparação conjunta com SDK da fonte: {resultado:?}");
        if let Err(erro) = &resultado {
            let tamanho = modulos.iter().flat_map(|m| &m.functions)
                .filter(|f| erro.contains(&f.symbol)).map(|f| f.symbol.len()).max();
            for (indice, m) in modulos.iter().enumerate() {
                for f in m.functions.iter().filter(|f| Some(f.symbol.len()) == tamanho && erro.contains(&f.symbol)) {
                    println!("Corpo associado no módulo {indice}: {f:#?}");
                }
            }
        }

        if resultado.is_err() && format!("{modulos:?}/{planos:?}") != antes {
            return Err("preparação conjunta recusada alterou módulos/planos".into());
        }
        return Ok(());
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
            if erro.contains(&f.symbol)
                || f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| match i {
                        dartforge_emit_native::hir::Instruction::CallRuntime { name, .. } => {
                            erro.contains(name)
                        }
                        dartforge_emit_native::hir::Instruction::CallStatic { symbol, .. } => {
                            erro.contains(symbol)
                        }
                        _ => false,
                    })
            {
                println!("Corpo associado ao diagnóstico: {f:#?}");
            }
        }
    }
    Ok(())
}

// Baixa os mesmos corpos/bibliotecas que a compilação do SDK usa. O grupo
// permanece privado ao diagnóstico: não reutiliza bitcode com ABI antiga.
fn baixar_sdk_para_auditoria(
    sdk_dir: &Path,
    por_tabelas: bool,
) -> Result<Vec<dartforge_emit_native::hir::Module>, String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let entrada = tmp.path().join("main.dart");
    let mut fonte: String = sdk_modulo::BIBLIOTECAS_DA_FONTE
        .iter()
        .map(|b| format!("import 'dart:{b}';\n"))
        .collect();
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| e.to_string())?;
    let sdk = sdk_modulo::carregar_sdk_nativo(sdk_dir)?;
    let mut interner = Interner::new();
    let (programa, diags) =
        dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    if !diags.is_empty() {
        return Err(format!("carga SDK: {diags:?}"));
    }
    let mut tipos = TypeTable::new();
    let core = CoreTypes::init(&mut tipos, &programa, &interner);
    let (mut outline, _) =
        dartforge_types::resolve_outline(&programa, &interner, &mut tipos, &core);
    let libs: Vec<_> = sdk_modulo::BIBLIOTECAS_DA_FONTE
        .iter()
        .filter_map(|b| {
            let uri = format!("dart:{b}");
            programa
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| dartforge_elements::model::LibraryId(i as u32))
        })
        .collect();
    if libs.len() != sdk_modulo::BIBLIOTECAS_DA_FONTE.len() {
        return Err("bibliotecas SDK incompletas".into());
    }
    let (corpos, _) = dartforge_types::infer_bodies_das_bibliotecas(
        &programa,
        &interner,
        &mut tipos,
        &core,
        &mut outline,
        &libs,
    );
    let ids = sdk_modulo::ids_de_classe_do_sdk(sdk_dir)?;
    let mut modulos = Vec::new();
    for lib in libs {
        let mut ctx = Context::new(&programa, &interner, &tipos, &core, &outline, &corpos)
            .com_sdk_da_fonte_e_ids(ids.clone())
            .so_a_biblioteca(lib);
        ctx.memoria_arc = true;
        let mut m = dartforge_emit_native::lower::lower_program(&ctx);
        m.biblioteca_sdk = true;
        m.cids_do_runtime = sdk_modulo::cids_do_runtime(&ctx);
        if !m.erros.is_empty() {
            return Err(format!(
                "lowering SDK {}: {:?}",
                programa.library(lib).uri,
                m.erros
            ));
        }
        otimizar::otimizar(&mut m);
        if por_tabelas { otimizar::excecoes_por_tabelas(&mut m); }
        println!(
            "SDK {}: {} funções",
            programa.library(lib).uri,
            m.functions.len()
        );
        // Confere compatibilidade com o conjunto compartilhado pelo emissor
        // anterior; esses nomes não produzem contratos de ownership.
        let anteriores: std::collections::HashSet<_> = m.functions.iter()
            .filter(|f| f.symbol.ends_with("$tear") || f.symbol.ends_with("$tearm") || f.symbol.starts_with("dfc."))
            .map(|f| f.symbol.clone()).collect();
        if anteriores != m.funcoes_compartilhadas_arc {
            return Err(format!("declarações de compartilhamento SDK incompletas em {}", programa.library(lib).uri));
        }
        modulos.push(m);
    }
    Ok(modulos)
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let mut entradas = Vec::new();
            let mut selecionou_arc = false;
            let mut sdk_no_grupo = false;
            for argumento in std::env::args_os().skip(1) {
                if argumento == "--memoria=arc" {
                    selecionou_arc = true;
                } else if argumento == "--sdk-no-grupo" {
                    sdk_no_grupo = true;
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
                auditar(Path::new(&entrada), sdk_no_grupo)?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "panic na auditoria".to_string())?
}
