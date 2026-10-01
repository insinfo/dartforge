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
use std::collections::{HashMap, HashSet};
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

/// O que o perfil de produção tira do texto do programa antes de conferir e
/// montar (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §6): `dart.trackLibraries(…)`
/// (registro para o depurador) e `dart._checkModuleNullSafetyMode(…)`
/// (conferência entre módulos compilados à parte) — num arquivo só, nenhum
/// dos dois tem consumidor. Tirados antes da verificação, as funções do
/// runtime que eles chamam saem do mundo também.
fn enxugar(e: &mut EmitidoComSdk) {
    for (_, t) in e.emitido.modulos.iter_mut() {
        let mut out = String::with_capacity(t.len());
        let mut pula = false;
        for linha in t.split_inclusive('\n') {
            if pula {
                if linha.starts_with('}') && linha.trim_end().ends_with(");") {
                    pula = false;
                }
                continue;
            }
            if linha.starts_with("dart._checkModuleNullSafetyMode(") {
                continue;
            }
            if linha.starts_with("dart.trackLibraries(") {
                pula = !linha.trim_end().ends_with(");");
                continue;
            }
            out.push_str(linha);
        }
        *t = out;
    }
}

/// Nomes de membro que o texto passa ao despacho dinâmico ou ao *tearoff*
/// (`dsend`/`dgsend`/`dload`/`dput`/`bind` e as variantes `Repl`): as
/// strings literais e os símbolos privados (`var x = dart.privateName(L,
/// "_n")` usado como argumento).
fn nomes_dinamicos(textos: &[(String, String)]) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for (_, t) in textos {
        let (l, e) = crate::sdk::seletores_dinamicos_por_especie(t);
        out.extend(l);
        out.extend(e);
        let mut privados: HashMap<&str, &str> = HashMap::new();
        for linha in t.lines() {
            let Some(r) = linha.strip_prefix("var ") else { continue };
            let Some((var, resto)) = r.split_once(" = dart.privateName(") else { continue };
            let Some(q) = resto.find('"') else { continue };
            let resto = &resto[q + 1..];
            let Some(fim) = resto.find('"') else { continue };
            privados.insert(var, &resto[..fim]);
        }
        let b = t.as_bytes();
        for (p, _) in t.match_indices('(') {
            let mut i = p;
            while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$') {
                i -= 1;
            }
            let posicao = match &t[i..p] {
                "dsend" | "dload" | "dput" | "bind" | "dsendRepl" | "dloadRepl" | "dputRepl" => 1,
                "dgsend" | "dgsendRepl" => 2,
                _ => continue,
            };
            // O argumento na posição pedida, até a vírgula de profundidade zero.
            let mut j = p + 1;
            let mut arg = 0;
            let mut prof = 0i32;
            let mut ini = j;
            while j < b.len() {
                match b[j] {
                    b'(' | b'[' | b'{' => prof += 1,
                    b')' | b']' | b'}' if prof == 0 => break,
                    b')' | b']' | b'}' => prof -= 1,
                    b',' if prof == 0 => {
                        if arg == posicao {
                            break;
                        }
                        arg += 1;
                        ini = j + 1;
                    }
                    _ => {}
                }
                j += 1;
            }
            if arg == posicao {
                let a = t[ini..j].trim();
                if let Some(n) = privados.get(a) {
                    out.insert(n.to_string());
                } else if a.len() >= 2 && (a.starts_with('"') && a.ends_with('"') || a.starts_with('\'') && a.ends_with('\'')) {
                    // Operadores (`dsend(a, "+", [b])`), que a extração por
                    // identificador não aceita.
                    out.insert(a[1..a.len() - 1].to_string());
                }
            }
        }
    }
    out
}

/// `DARTFORGE_JSPROD_POR=nome`: cada função (do programa ou do SDK) com esse
/// nome, se vive, a classe dona e o nível dela, e se o seletor vive para ela.
fn diagnosticar(a: &dartforge_emit_js::Analise<'_>, mundo: &dartforge_mundo::Mundo, alvo: &str) {
    let p = a.program;
    for (i, f) in p.functions.iter().enumerate() {
        if a.interner.resolve(f.name) != alvo {
            continue;
        }
        let fid = dartforge_elements::model::FunctionElementId(i as u32);
        let dono = f.class.map(|c| format!("{} [{:?}] seletor={}", a.interner.resolve(p.class(c).name), mundo.classe(c), mundo.seletor_vivo_para(p, alvo, c))).unwrap_or_default();
        eprintln!("[jsprod] {}::{alvo} viva={} {dono} causa={:?}", p.library(f.library).uri, mundo.funcao(fid), mundo.causa_funcao(fid));
    }
}

/// Uma fase do ponto fixo: mundo → emissão filtrada → verificação do texto.
/// Devolve a emissão final, ou `None` se não convergiu.
fn ponto_fixo(
    a: &dartforge_emit_js::Analise<'_>,
    entrada: dartforge_mundo::Entrada<'_>,
    op: Opcoes,
    assinaturas: Option<&HashSet<String>>,
    rel: &mut RelatorioMundo,
) -> Result<Option<EmitidoComSdk>, String> {
    let mut raizes = raizes(a, &entrada);
    let opm = dartforge_mundo::Opcoes { incluir_sdk: true };
    let max_rodadas: usize = std::env::var("DARTFORGE_JSPROD_RODADAS").ok().and_then(|v| v.parse().ok()).unwrap_or(40).max(1);
    let rastro = std::env::var("DARTFORGE_JSPROD_RASTRO").is_ok();
    for rodada in 1..=max_rodadas {
        let t = Instant::now();
        let mundo = dartforge_mundo::calcular_com(entrada, &raizes, opm);
        rel.tempo_mundo += t.elapsed();
        let ad = filtro::Adaptador { mundo: &mundo, program: a.program, stub: op.stub, assinaturas };
        let t = Instant::now();
        let mut e = a.emitir_com_sdk(Some(&ad))?;
        // O mapa namespace → biblioteca sai do `trackLibraries`, que o
        // `enxugar` tira: lido antes.
        let libs = bibliotecas(a, &e.emitido.modulos);
        enxugar(&mut e);
        rel.tempo_emissao += t.elapsed();
        let t = Instant::now();
        let mut textos = e.emitido.modulos.clone();
        textos.push(("dart_sdk.js".to_string(), format!("{}{}", e.bootstrap, e.sdk)));
        let faltas = verificar::conferir(&textos, &libs, a.program, a.interner, &mundo);
        rel.tempo_verificacao += t.elapsed();
        rel.rodadas += 1;
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
            if let Ok(alvo) = std::env::var("DARTFORGE_JSPROD_POR") {
                diagnosticar(a, &mundo, &alvo);
            }
            return Ok(Some(e));
        }
        rel.curas.extend(faltas.descricao.iter().cloned());
        raizes.funcoes.extend(faltas.funcoes);
        raizes.variaveis.extend(faltas.variaveis);
        raizes.classes_tipo.extend(faltas.classes);
        raizes.classes_instanciadas.extend(faltas.classes_instanciadas);
        raizes.tearoffs.extend(faltas.tearoffs);
        raizes.seletores.extend(faltas.seletores);
    }
    Ok(None)
}

/// Mundo fechado (com o SDK) → emissão filtrada → verificação do texto, até o
/// ponto fixo; em duas fases (§6.1 da especificação): a primeira com todas as
/// assinaturas acha os nomes do despacho dinâmico; a segunda recomeça das
/// raízes emitindo só as assinaturas desses nomes — o que elas deixavam
/// vivo só como tipo também sai. Se a segunda achar nomes novos, eles entram
/// e ela recomeça (o conjunto só cresce: termina).
pub fn emitir(a: &dartforge_emit_js::Analise<'_>, op: Opcoes) -> Result<(EmitidoComSdk, Option<RelatorioMundo>), String> {
    if !op.podar_usuario {
        let mut e = a.emitir_com_sdk(None)?;
        enxugar(&mut e);
        return Ok((e, None));
    }
    let entrada = dartforge_mundo::Entrada { program: a.program, interner: a.interner, table: a.table, outline: a.outline, bodies: a.bodies };
    let mut rel = RelatorioMundo::default();
    let Some(primeira) = ponto_fixo(a, entrada, op, None, &mut rel)? else {
        rel.poda_desligada = true;
        rel.curas.push("ponto fixo não atingido; poda desligada".to_string());
        let mut e = a.emitir_com_sdk(None)?;
        enxugar(&mut e);
        return Ok((e, Some(rel)));
    };
    if std::env::var("DARTFORGE_JSPROD_ASSINATURAS").is_ok_and(|v| v == "todas") {
        return Ok((primeira, Some(rel)));
    }
    let textos = |e: &EmitidoComSdk| {
        let mut v = e.emitido.modulos.clone();
        v.push(("dart_sdk.js".to_string(), e.sdk.clone()));
        v
    };
    let mut nomes = nomes_dinamicos(&textos(&primeira));
    for _ in 0..8 {
        let Some(e) = ponto_fixo(a, entrada, op, Some(&nomes), &mut rel)? else { break };
        let novos = nomes_dinamicos(&textos(&e));
        if novos.is_subset(&nomes) {
            return Ok((e, Some(rel)));
        }
        nomes.extend(novos);
    }
    // A segunda fase não fechou: a primeira (com todas as assinaturas) vale.
    Ok((primeira, Some(rel)))
}

/// O prefixo de toda receita rti fechada que o emissor escreve.
const PREFIXO_RTI: &str = "dart_rti._Universe.eval(dart_rti._theUniverse(), ";

/// Troca `dart_rti._Universe.eval(dart_rti._theUniverse(), "R", true)` por
/// `t$R("R")` (§6.3 da especificação): a mesma chamada, por uma função
/// declarada uma vez. `t$` é prefixo reservado do emissor (um identificador
/// Dart que comece assim é renomeado), então `t$R` não colide com nada.
pub fn apelidar_receitas(js: &str) -> String {
    let mut out = String::with_capacity(js.len());
    let mut resto = js;
    while let Some(p) = resto.find(PREFIXO_RTI) {
        let depois = &resto[p + PREFIXO_RTI.len()..];
        let b = depois.as_bytes();
        // Uma string JS com aspas duplas, depois `, true)`.
        let fim = if b.first() == Some(&b'"') {
            let mut i = 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            (i < b.len() && depois[i + 1..].starts_with(", true)")).then_some(i + 1)
        } else {
            None
        };
        match fim {
            Some(f) => {
                out.push_str(&resto[..p]);
                out.push_str("t$R(");
                out.push_str(&depois[..f]);
                out.push(')');
                resto = &depois[f + ", true)".len()..];
            }
            None => {
                out.push_str(&resto[..p + PREFIXO_RTI.len()]);
                resto = depois;
            }
        }
    }
    out.push_str(resto);
    out
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
    let sdk = format!("{}const t$R = r => {PREFIXO_RTI}r, true);\n(function () {{\n{}}})();\n", e.bootstrap, e.sdk);
    let usuario = modulos.iter().map(|m| m.corpo.len() + m.namespaces.iter().map(|n| n.len() + 1).sum::<usize>()).sum();
    let preambulo = if op.stub { bundle::PREAMBULO_STUB } else { "" };
    let js = apelidar_receitas(&bundle::montar(&sdk, &modulos, &entrada, preambulo));
    let antes_de_compactar = js.len();
    // Identificadores por escopo e espaço (oxc); `DARTFORGE_JSPROD_NOMES=0`
    // fica só na compactação de espaço.
    let nomes = std::env::var("DARTFORGE_JSPROD_NOMES").map_or(true, |v| v != "0");
    let js = if op.minificar && nomes {
        match minificar::minificar_nomes(&js) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("aviso: {e}; o arquivo sai só compactado");
                minificar::compactar(&js)
            }
        }
    } else if op.minificar {
        minificar::compactar(&js)
    } else {
        js
    };
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
