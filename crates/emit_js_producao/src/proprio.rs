//! Perfil de produção com o SDK compilado pela nossa trilha
//! (`docs/JS-PRODUCAO-SDK-PROPRIO.md`).
//!
//! O arquivo final é o *bootstrap* (`dart`, namespaces, `dartx`), o módulo do
//! SDK emitido pelo modo SDK de `crates/emit_js` e os módulos do programa,
//! como no caminho antigo — mas sem nenhum byte do `dart_sdk.js`.
//!
//! O mundo fechado é um só, com o SDK dentro (§4 da especificação): o que o
//! emissor escreve sem estar na AST (`dart.fn`, `JSArray.of`, `dart.str`…) é
//! achado pelo verificador do texto e volta como raiz, até o ponto fixo.

use crate::{bundle, filtro, minificar, verificar, Opcoes, Producao, RelatorioMundo};
use dartforge_elements::model::{Element, LibraryId};
use dartforge_emit_js::EmitidoComSdk;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tags de `registerExtension` que são tipos **embutidos do JavaScript**: o
/// valor (`"olá"`, `42`, `[]`, uma função) vira instância Dart pela tag, sem
/// nenhum nome no programa. São raiz, como os impactos de
/// `js_backend/backend_impact.dart` no dart2js.
const TAGS_EMBUTIDAS: &[&str] = &["Object", "String", "Number", "Boolean", "Array", "Function", "Symbol", "BigInt", "Error", "TypeError", "RangeError"];

/// Raízes iniciais: `main`, `core.Object` e as classes das tags embutidas.
/// O resto vem do ponto fixo com o texto.
fn raizes(a: &dartforge_emit_js::Analise<'_>, e: &dartforge_mundo::Entrada<'_>) -> dartforge_mundo::Raizes {
    let mut r = dartforge_mundo::Raizes::default();
    if let (Some(lib), Some(sym)) = (a.program.entry, a.interner.lookup("main")) {
        if let Some(Element::Function(f)) = a.program.library(lib).declared.get(&sym).and_then(|b| b.getter) {
            r.funcoes.push(f);
        }
    }
    if let (Some(core), Some(sym)) = (a.program.core, a.interner.lookup("Object")) {
        if let Some(Element::Class(c)) = a.program.library(core).declared.get(&sym).and_then(|b| b.getter) {
            r.classes_instanciadas.push(c);
        }
    }
    for (c, tags) in dartforge_mundo::tags_nativas(e) {
        if tags.iter().any(|t| TAGS_EMBUTIDAS.contains(&t.as_str())) {
            r.classes_instanciadas.push(c);
        }
    }
    r
}

/// Namespace JS → biblioteca, para o verificador: os do programa (`L$…`) e os
/// do SDK (`core`, `dart`, `dart_rti`…).
fn bibliotecas(a: &dartforge_emit_js::Analise<'_>, modulos: &[(String, String)]) -> HashMap<String, LibraryId> {
    let mut libs = verificar::bibliotecas_do_texto(modulos, a.program);
    for (i, l) in a.program.libraries.iter().enumerate() {
        let Some(n) = l.uri.strip_prefix("dart:") else { continue };
        let var = match n {
            "_runtime" => "dart",
            "_rti" => "dart_rti",
            x => x,
        };
        libs.insert(var.to_string(), LibraryId(i as u32));
    }
    libs
}

/// Mundo fechado (com o SDK) → emissão filtrada → verificação do texto, até o
/// ponto fixo.
pub fn emitir(a: &dartforge_emit_js::Analise<'_>, op: Opcoes) -> Result<(EmitidoComSdk, Option<RelatorioMundo>), String> {
    if !op.podar_usuario {
        return Ok((a.emitir_com_sdk(None)?, None));
    }
    let entrada = dartforge_mundo::Entrada { program: a.program, interner: a.interner, table: a.table, outline: a.outline, bodies: a.bodies };
    let mut rel = RelatorioMundo::default();
    let mut raizes = raizes(a, &entrada);
    let opm = dartforge_mundo::Opcoes { incluir_sdk: true };
    let max_rodadas: usize = std::env::var("DARTFORGE_JSPROD_RODADAS").ok().and_then(|v| v.parse().ok()).unwrap_or(40).max(1);
    let rastro = std::env::var("DARTFORGE_JSPROD_RASTRO").is_ok();
    for rodada in 1..=max_rodadas {
        let t = Instant::now();
        let mundo = dartforge_mundo::calcular_com(entrada, &raizes, opm);
        rel.tempo_mundo += t.elapsed();
        let ad = filtro::Adaptador { mundo: &mundo, program: a.program, stub: op.stub };
        let t = Instant::now();
        let e = a.emitir_com_sdk(Some(&ad))?;
        rel.tempo_emissao += t.elapsed();
        let t = Instant::now();
        let mut textos = e.emitido.modulos.clone();
        textos.push(("dart_sdk.js".to_string(), format!("{}{}", e.bootstrap, e.sdk)));
        let libs = bibliotecas(a, &textos);
        let faltas = verificar::conferir(&textos, &libs, a.program, a.interner, &mundo);
        rel.tempo_verificacao += t.elapsed();
        rel.rodadas = rodada;
        rel.sem_elemento = faltas.sem_elemento.clone();
        // `DARTFORGE_JSPROD_DESPEJO=arq`: o texto da rodada (diagnóstico).
        if let Ok(p) = std::env::var("DARTFORGE_JSPROD_DESPEJO") {
            let _ = std::fs::write(p, textos.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n"));
        }
        if rastro {
            eprintln!("[jsprod] rodada {rodada}: {} falta(s)", faltas.descricao.len());
            for d in faltas.descricao.iter().take(40) {
                eprintln!("[jsprod]   {d}");
            }
        }
        if faltas.vazia() {
            rel.estat = mundo.estat.clone();
            return Ok((e, Some(rel)));
        }
        rel.curas.extend(faltas.descricao.iter().cloned());
        raizes.funcoes.extend(faltas.funcoes);
        raizes.variaveis.extend(faltas.variaveis);
        raizes.classes_tipo.extend(faltas.classes);
        raizes.classes_instanciadas.extend(faltas.classes_instanciadas);
        raizes.tearoffs.extend(faltas.tearoffs);
        raizes.seletores.extend(faltas.seletores);
    }
    // Sem ponto fixo a poda não foi validada: sai tudo (o superconjunto seguro).
    rel.poda_desligada = true;
    rel.curas.push(format!("ponto fixo não atingido em {max_rodadas} rodada(s); poda desligada"));
    Ok((a.emitir_com_sdk(None)?, Some(rel)))
}

/// Monta o arquivo único.
pub fn montar(e: &EmitidoComSdk, op: Opcoes) -> Producao {
    let modulos: Vec<bundle::Modulo> = e
        .emitido
        .modulos
        .iter()
        .filter(|(p, _)| p != "preambulo.js")
        .map(|(p, t)| bundle::separar(p, t))
        .collect();
    let (modulos, ciclos) = bundle::ordenar(modulos);
    let entrada = bundle::entrada_do_mjs(&e.emitido.entrada, &modulos).unwrap_or_else(|| "L$main".to_string());
    // O módulo do SDK numa IIFE, como os do programa: os `var` do prelúdio
    // dele (aliases `dartx`, nomes privados) são locais; o que é
    // compartilhado (`dart`, os namespaces, `dartx`) está no *bootstrap*.
    let sdk = format!("{}(function () {{\n{}}})();\n", e.bootstrap, e.sdk);
    let usuario = modulos.iter().map(|m| m.corpo.len() + m.namespaces.iter().map(|n| n.len() + 1).sum::<usize>()).sum();
    let preambulo = if op.stub { bundle::PREAMBULO_STUB } else { "" };
    let js = bundle::montar(&sdk, &modulos, &entrada, preambulo);
    let antes_de_compactar = js.len();
    let js = if op.minificar { minificar::compactar(&js) } else { js };
    Producao {
        js,
        modulos: modulos.len(),
        ciclos,
        sdk_antes: sdk.len(),
        sdk_depois: sdk.len(),
        sdk_unidades: 0,
        sdk_vivas: 0,
        usuario,
        antes_de_compactar,
        mundo: None,
        indice: None,
        tempo_poda: Duration::ZERO,
    }
}
