//! Mundo fechado sobre o `dart_sdk.js`.
//!
//! O `dart2js` calcula o alcance sobre elementos Dart porque compila o SDK
//! junto com o programa. Nós ligamos contra um `dart_sdk.js` **já compilado
//! pelo DDC** — a decisão de `docs/EMISSAO-DDC.md`, que nos deu semântica
//! exata de graça — então o nosso alcance tem de ser calculado dentro de um
//! artefato JS pronto. Dá, porque o arquivo não é JS arbitrário: é saída de
//! compilador, com uma gramática pequena e regular (`docs/JS-PRODUCAO.md` §1.3
//! tem a tabela completa das formas).
//!
//! O que este módulo faz, em ordem:
//!
//! 1. parte o arquivo em declarações de topo (`varredura::declaracoes`);
//! 2. classifica cada uma: que símbolo ela **define**, ou em qual símbolo ela
//!    **se pendura** (um `dart.setLibraryUri(C, …)` não é raiz, é metadado de
//!    `C`), sub-dividindo as três declarações gigantes que, inteiras,
//!    referenciam o programa todo;
//! 3. extrai as referências — inclusive as que nenhum analisador de JS enxerga:
//!    os nomes de classe dentro das **receitas rti**, porque
//!    `dart_rti._Universe.eval("core|List<core|int>")` resolve o tipo por nome
//!    em execução;
//! 4. roda o ponto fixo (`alcance`) e reemite as unidades vivas na ordem do
//!    arquivo.

use std::collections::HashSet;

use crate::alcance::{self, Simbolos, Unidade};
use crate::bundle::Modulo;
use crate::varredura;

/// As 38 bibliotecas que o `dart_sdk.js` declara com
/// `var X = Object.create(dart.library)`. Um identificador só é lido como
/// `biblioteca.membro` quando o prefixo está nesta lista — senão `this.foo` e
/// `opts.bar` virariam símbolos.
const LIBS: &[&str] = &[
    "dart", "_ddc_only", "_async_status_codes", "_js_shared_embedded_names", "_recipe_syntax",
    "dart_rti", "_debugger", "_foreign_helper", "_interceptors", "_internal", "_isolate_helper",
    "_js_annotations", "_js_helper", "_js_names", "_js_primitives", "_js_types", "_metadata",
    "_native_typed_data", "async", "collection", "convert", "developer", "io", "isolate", "js",
    "js_interop", "js_interop_unsafe", "js_util", "math", "typed_data", "indexed_db", "html$",
    "html_common", "svg$", "web_audio", "web_gl", "core", "_http", "dartx",
];

/// Nas receitas rti as bibliotecas `dart:html` e `dart:svg` aparecem sem o `$`
/// que o JS usa na variável (`"html|Element"` mas `html$.Element`).
fn ns_da_receita(p: &str) -> &str {
    match p {
        "html" => "html$",
        "svg" => "svg$",
        outro => outro,
    }
}

/// Tags de `dart.registerExtension(tag, C)` que são tipos **embutidos do
/// JavaScript**. Para essas, a ligação entre o valor e a classe Dart é feita em
/// execução pelo tipo do objeto, não por nenhum nome que apareça no programa:
/// `"olá"` é um `String` do JS e só vira `core.String` porque
/// `registerExtension("String", _interceptors.JSString)` rodou. Logo a classe
/// alvo é **raiz**, sem depender de citação textual.
///
/// É o equivalente dos "impactos" que o dart2js declara em
/// `js_backend/backend_impact.dart:93` (jsStringClass, jsIntClass, jsArrayClass…)
/// — a mesma ideia: o runtime tem pontos de entrada que o alcance não vê.
///
/// Os tags de dados tipados (`ArrayBuffer`, `DataView`) ficam de fora de
/// propósito: só se chega a um deles por `dart:typed_data`/`dart:html`, que o
/// programa cita, e incluí-los custaria o `_native_typed_data` inteiro.
const TAGS_EMBUTIDAS: &[&str] = &[
    "Object", "String", "Number", "Boolean", "Array", "Function", "Symbol", "BigInt",
    "Error", "TypeError", "RangeError",
];

/// Bibliotecas cujas classes são **sempre** raiz. Uma só: `dart:_interceptors`.
/// Os interceptadores são o piso do sistema de tipos do DDC — `null`, os
/// números, as funções e os objetos JS desconhecidos chegam a eles por despacho
/// de tipo em execução, e vários nem têm `registerExtension` (o `JSNull` é
/// instalado direto pelo runtime).
const LIBS_RAIZ: &[&str] = &["_interceptors"];

fn e_lib(nome: &str) -> bool {
    LIBS.contains(&nome)
}

fn ident_em(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
        j += 1;
    }
    j
}

fn inicio_de_ident(b: &[u8], i: usize) -> bool {
    b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$'
}

/// Extrai de `t` todos os símbolos referenciados.
///
/// Cinco formas, e a terceira é a que só existe em código do Dart para a web:
/// `lib.Nome`, `lib['A|b']`, o nome de classe dentro de uma **receita rti**
/// (`"core|List<core|int>"`), o índice de constante (`C[42]`, `CT.C42`) e o
/// identificador simples (que casa com os `var` de topo do arquivo, como as
/// aplicações de mixin `EventTarget_ListBase$36`).
fn referencias(t: &str, fora: &mut Vec<String>, modulo_usuario: bool) {
    let b = t.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if !inicio_de_ident(b, i) {
            i += 1;
            continue;
        }
        // Não é o começo de um identificador se o byte anterior também é de ident.
        if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$') {
            i = ident_em(b, i);
            continue;
        }
        let fim = ident_em(b, i);
        let nome = &t[i..fim];
        let mut avanco = fim;
        if fim < b.len() && b[fim] == b'.' && fim + 1 < b.len() && inicio_de_ident(b, fim + 1) {
            let fim2 = ident_em(b, fim + 1);
            let membro = &t[fim + 1..fim2];
            if e_lib(nome) {
                fora.push(format!("{nome}.{membro}"));
            } else if modulo_usuario && matches!(nome, "html" | "svg") {
                // O DDC exporta `html$ as html` e `svg$ as svg`. A emissão
                // do usuário cita o alias; o mundo do SDK usa o nome local.
                fora.push(format!("{}.{membro}", ns_da_receita(nome)));
            } else {
                fora.push(nome.to_string());
            }
            // `CT.C42` é uma constante.
            if nome == "CT" && membro.len() > 1 && membro.starts_with('C') && membro[1..].bytes().all(|c| c.is_ascii_digit()) {
                fora.push(format!("C#{}", &membro[1..]));
            }
            avanco = fim2;
        } else if fim < b.len() && b[fim] == b'[' && (e_lib(nome) || (modulo_usuario && matches!(nome, "html" | "svg"))) {
            // `lib['A|b']` — membro de extensão, cujo nome tem `|`.
            let mut j = fim + 1;
            if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
                let aspa = b[j];
                let ini = j + 1;
                j = ini;
                while j < b.len() && b[j] != aspa {
                    j += 1;
                }
                fora.push(format!("{}.{}", ns_da_receita(nome), &t[ini..j]));
                avanco = j + 1;
            } else {
                fora.push(nome.to_string());
            }
        } else if nome == "C" && fim < b.len() && b[fim] == b'[' {
            let mut j = fim + 1;
            let ini = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > ini && j < b.len() && b[j] == b']' {
                fora.push(format!("C#{}", &t[ini..j]));
                avanco = j + 1;
            } else {
                fora.push(nome.to_string());
            }
        } else {
            fora.push(nome.to_string());
        }
        i = avanco;
    }
    // Receitas rti: `lib|Nome` em qualquer lugar do texto (elas vivem dentro de
    // strings, e é por isso que a varredura de identificadores não as pega).
    let b = t.as_bytes();
    for (i, _) in t.match_indices('|') {
        if i == 0 || i + 1 >= b.len() || !inicio_de_ident(b, i + 1) {
            continue;
        }
        let mut ini = i;
        while ini > 0 && (b[ini - 1].is_ascii_alphanumeric() || b[ini - 1] == b'_' || b[ini - 1] == b'$') {
            ini -= 1;
        }
        if ini == i || !inicio_de_ident(b, ini) {
            continue;
        }
        let fim = ident_em(b, i + 1);
        let ns = ns_da_receita(&t[ini..i]);
        if e_lib(ns) {
            fora.push(format!("{ns}.{}", &t[i + 1..fim]));
        }
    }
}

/// Extrai de `t` os **seletores** que ele usa: `x.nome`, `x[$nome]`,
/// `x[_nome]` e toda string que pareça um identificador — esta última é o lado
/// conservador, porque `dart.dsend(o, "nome")` passa o seletor como string e
/// não há como distinguir isso de uma string de dados sem analisar o programa.
fn seletores(t: &str, fora: &mut Vec<String>) {
    let b = t.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'.' if i + 1 < b.len() && inicio_de_ident(b, i + 1) => {
                let fim = ident_em(b, i + 1);
                let nome = &t[i + 1..fim];
                fora.push(format!("sel:{nome}"));
                // `S.$head` e `S$2.$console` citam os seletores Dart
                // `head` e `console`. Preserva também o nome JS literal.
                if nome.starts_with('$') && nome.len() > 1 {
                    fora.push(format!("sel:{}", &nome[1..]));
                }
                i = fim;
            }
            b'[' if i + 1 < b.len() => {
                let mut j = i + 1;
                if b[j] == b'$' {
                    j += 1;
                }
                if j < b.len() && inicio_de_ident(b, j) {
                    let fim = ident_em(b, j);
                    if fim < b.len() && b[fim] == b']' {
                        fora.push(format!("sel:{}", &t[j..fim]));
                    }
                }
                i += 1;
            }
            b'"' | b'\'' => {
                let aspa = b[i];
                let ini = i + 1;
                let mut j = ini;
                while j < b.len() && b[j] != aspa {
                    if b[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                let s = &t[ini..j.min(t.len())];
                // `#` e `|` entram porque os nomes gerados pelo DDC os usam:
                // `C["_#new#tearOff"]`, `async["FutureRecord2|get#wait"]`. Sem
                // eles o tearoff de construtor seria podado com alguém ainda o
                // citando.
                if s.len() < 64 && e_nome_de_seletor(s) {
                    fora.push(format!("sel:{s}"));
                }
                i = j + 1;
            }
            _ => i += 1,
        }
    }
}

// ------------------------------------------------------------------ classificação

/// Uma fatia do arquivo, com a sua condição de vida.
struct Fatia {
    ini: usize,
    fim: usize,
    /// Fatias de um mesmo grupo são entradas de um objeto/vetor literal e
    /// precisam de vírgula entre as vivas. `None` é declaração de topo.
    grupo: Option<u32>,
    /// A vírgula já vem no texto (e então não se recoloca).
    sempre: bool,
    gatilhos: Vec<String>,
    requisitos: Vec<String>,
}

fn primeiro_ident(t: &str) -> Option<(&str, usize)> {
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= b.len() || !inicio_de_ident(b, i) {
        return None;
    }
    let fim = ident_em(b, i);
    Some((&t[i..fim], fim))
}

/// `lib.Nome` no começo de `t`, se houver.
fn alvo_qualificado(t: &str) -> Option<String> {
    let (a, i) = primeiro_ident(t)?;
    let b = t.as_bytes();
    if i >= b.len() || b[i] != b'.' {
        return None;
    }
    let (m, _) = primeiro_ident(&t[i + 1..])?;
    e_lib(a).then(|| format!("{a}.{m}"))
}

/// Como [`alvo_qualificado`], mas só quando `lib.Nome` é mesmo **declarado**:
/// o que vem depois tem de ser `=` ou `[`.
///
/// A distinção não é cosmética. Sem ela, `dart.applyMixin(V, M);` seria lida
/// como declaração de `dart.applyMixin` — e aí a instrução só viveria se a
/// função `dart.applyMixin` vivesse, que por sua vez só é referenciada pelos
/// sítios de chamada: um laço que se apaga sozinho. Foi assim que as 93
/// aplicações de mixin sumiram do primeiro bundle podado, e com elas os
/// getters que `defineExtensionAccessors` depois procurava no protótipo.
fn declaracao_qualificada(t: &str) -> Option<String> {
    let (a, i) = primeiro_ident(t)?;
    let b = t.as_bytes();
    if i >= b.len() || b[i] != b'.' {
        return None;
    }
    if !e_lib(a) {
        return None;
    }
    let (m, j) = primeiro_ident(&t[i + 1..])?;
    let mut k = i + 1 + j;
    while k < b.len() && (b[k] == b' ' || b[k] == b'\t') {
        k += 1;
    }
    // `lib.Nome = …`, `lib.Nome[x] = …` — mas nunca `lib.Nome === …`.
    let decl = match b.get(k) {
        Some(b'=') => b.get(k + 1) != Some(&b'='),
        Some(b'[') => true,
        _ => false,
    };
    decl.then(|| format!("{a}.{m}"))
}

/// Nome do membro numa entrada de corpo de classe (`get x() {…}`, `[_priv](…)`,
/// `static f(…)`, `'A|b'(…)`).
fn nome_do_membro(t: &str) -> Option<String> {
    let t = t.trim_start();
    let t = t.strip_prefix("static").map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix('*').map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'[' {
        // `[_priv](…)` ou `[$nome]` ou `['A|b']`
        let mut j = 1;
        if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
            let aspa = b[j];
            let ini = j + 1;
            j = ini;
            while j < b.len() && b[j] != aspa {
                j += 1;
            }
            return Some(t[ini..j].to_string());
        }
        if j < b.len() && b[j] == b'$' {
            j += 1;
        }
        if j < b.len() && inicio_de_ident(b, j) {
            let fim = ident_em(b, j);
            // No DDC `get [S.$head]()` e `get [S$2.$console]()` usam um
            // alias de símbolos. O seletor é o campo do alias, não `S`.
            let prefixo = &t[j..fim];
            if (prefixo == "S" || prefixo.strip_prefix("S$").is_some_and(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit())))
                && b.get(fim) == Some(&b'.') && fim + 1 < b.len()
            {
                let inicio = fim + 1 + usize::from(b[fim + 1] == b'$');
                if inicio < b.len() && inicio_de_ident(b, inicio) {
                    let fim_nome = ident_em(b, inicio);
                    if b.get(fim_nome) == Some(&b']') {
                        return Some(t[inicio..fim_nome].to_string());
                    }
                }
            }
            return Some(t[j..fim].to_string());
        }
        return None;
    }
    if b[0] == b'\'' || b[0] == b'"' {
        let aspa = b[0];
        let mut j = 1;
        while j < b.len() && b[j] != aspa {
            j += 1;
        }
        return Some(t[1..j].to_string());
    }
    if !inicio_de_ident(b, 0) {
        return None;
    }
    let fim = ident_em(b, 0);
    // Só é membro se o que vem depois é `(` — senão é campo de objeto.
    let resto = t[fim..].trim_start();
    (resto.starts_with('(')).then(|| t[..fim].to_string())
}

/// Um nome de membro que dá para casar com um seletor extraído do texto.
///
/// Identificador (com `_`, `$`) mais `#` e `|`, que os nomes gerados pelo DDC
/// usam (`_#new#tearOff`, `FutureRecord2|get#wait`). O que não casa este
/// formato — os operadores — **não é podável por seletor**: o membro é
/// declarado `['+'](outro) {…}`, mas quem chama escreve `p[$plus](x)`, o alias
/// do símbolo `dartx['+']`, cujo nome JS não tem relação textual com `+`.
/// Casar os dois exigiria conhecer a tabela `dartx`; enquanto não a
/// conhecemos, podar o operador dá `p.+ is not a function`.
fn e_nome_de_seletor(n: &str) -> bool {
    !n.is_empty()
        && inicio_de_ident(n.as_bytes(), 0)
        && n.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'#' | b'|'))
}

/// Sub-divide `X = class … { … }` em casca + um membro por entrada.
fn partir_classe(src: &str, ini: usize, fim: usize, sym: &str, fatias: &mut Vec<Fatia>, grupo: &mut u32) -> bool {
    let t = &src[ini..fim];
    let Some(eq) = t.find("= class ") else { return false };
    let Some(rel) = t[eq..].find('{') else { return false };
    let abre = ini + eq + rel;
    let Some(fecha) = varredura::fecha_chave(src, abre) else { return false };
    let corpo = &src[abre + 1..fecha];
    let membros = varredura::entradas_de_classe(corpo);
    if membros.len() < 3 {
        return false;
    }
    // Casca: `X = class N extends S {` … `};` mais o que vier depois.
    fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: false, gatilhos: vec![sym.to_string()], requisitos: vec![] });
    for (a, b) in membros {
        let txt = &corpo[a..b];
        let mut req = vec![sym.to_string()];
        // Sem nome casável com um seletor, o membro vive com a classe: é o
        // lado conservador, e é o que mantém os operadores de pé.
        if let Some(n) = nome_do_membro(txt) {
            if e_nome_de_seletor(&n) {
                req.push(format!("sel:{n}"));
            }
        }
        fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + b, grupo: None, sempre: false, gatilhos: vec![], requisitos: req });
    }
    let _ = grupo;
    fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: false, gatilhos: vec![sym.to_string()], requisitos: vec![] });
    true
}

/// Sub-divide `f(ALVO, { … })` ou `f(ALVO, () => ({ … }))` por entrada, com a
/// entrada condicionada a `[alvo, sel:chave]`.
fn partir_objeto(
    src: &str,
    ini: usize,
    fim: usize,
    abre: usize,
    alvo: &str,
    por_seletor: bool,
    fatias: &mut Vec<Fatia>,
    grupo: &mut u32,
) -> bool {
    let Some(fecha) = varredura::fecha_chave(src, abre) else { return false };
    let corpo = &src[abre + 1..fecha];
    let entradas = varredura::entradas_de_objeto(corpo);
    if entradas.len() < 2 {
        return false;
    }
    let g = *grupo;
    *grupo += 1;
    fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: false, gatilhos: vec![alvo.to_string()], requisitos: vec![] });
    for (a, b) in entradas {
        let txt = corpo[a..b].trim_end().trim_end_matches(',');
        let chave = chave_de_entrada(txt);
        let (gat, req) = match (&chave, por_seletor) {
            (Some(k), true) => (vec![], vec![alvo.to_string(), format!("sel:{k}")]),
            (Some(k), false) => (vec![format!("{alvo}.{k}")], vec![]),
            (None, _) => (vec![alvo.to_string()], vec![]),
        };
        let desl = corpo[a..b].len() - corpo[a..b].trim_end().len();
        let corte = b - desl - if corpo[a..b].trim_end().ends_with(',') { 1 } else { 0 };
        fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: false, gatilhos: gat, requisitos: req });
    }
    fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: false, gatilhos: vec![alvo.to_string()], requisitos: vec![] });
    true
}

/// A chave de uma entrada de objeto: `nome:`, `get nome()`, `'a|b':`,
/// `[Sym]:` (esta última não é minificável nem casável, então devolve `None`).
fn chave_de_entrada(t: &str) -> Option<String> {
    let t = t.trim_start();
    // Comentário de posição que o DDC emite antes das entradas de `defineLazy`.
    let t = if let Some(r) = t.strip_prefix("/*") { r.split_once("*/").map(|(_, x)| x.trim_start()).unwrap_or(r) } else { t };
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'\'' || b[0] == b'"' {
        let aspa = b[0];
        let mut j = 1;
        while j < b.len() && b[j] != aspa {
            if b[j] == b'\\' {
                j += 1;
            }
            j += 1;
        }
        return Some(t[1..j.min(t.len())].to_string());
    }
    if !inicio_de_ident(b, 0) {
        return None;
    }
    let fim = ident_em(b, 0);
    Some(t[..fim].to_string())
}

/// Classifica o arquivo inteiro em fatias com condição, e devolve junto as
/// raízes que o próprio arquivo impõe (ver [`TAGS_EMBUTIDAS`] e [`LIBS_RAIZ`]).
fn classificar(src: &str, por_membro: bool) -> (Vec<Fatia>, Vec<String>) {
    let mut fatias = Vec::new();
    let mut raizes = Vec::new();
    let mut grupo = 0u32;
    for (ini, fim) in varredura::declaracoes(src) {
        let t = &src[ini..fim];
        let tt = t.trim_start();
        let simples = |g: Vec<String>| Fatia { ini, fim, grupo: None, sempre: false, gatilhos: g, requisitos: vec![] };
        let sempre = || Fatia { ini, fim, grupo: None, sempre: true, gatilhos: vec![], requisitos: vec![] };

        // `dart.defineLazy(ALVO, { get x() {…}, … })` — estáticos preguiçosos.
        // As 549 constantes do `CT` são a maior declaração do arquivo (124 KB).
        if let Some(r) = tt.strip_prefix("dart.defineLazy(") {
            // O alvo pode ser uma biblioteca (`core`) **ou uma classe**
            // (`html$.Event`, para os estáticos preguiçosos dela). Ler só o
            // primeiro identificador prendia a cabeça do grupo ao espaço de
            // nomes `html$`, que está sempre vivo, e a cabeça cita a classe:
            // `dart:html` inteiro entrava num programa que só faz `print`.
            if let Some(alvo) = alvo_qualificado(r).or_else(|| primeiro_ident(r).map(|(a, _)| a.to_string())) {
                let alvo = alvo.as_str();
                let e_ct = alvo == "CT";
                let e_classe = alvo.contains('.');
                if let Some(rel) = t.find('{') {
                    let abre = ini + rel;
                    let Some(fecha) = varredura::fecha_chave(src, abre) else { fatias.push(sempre()); continue };
                    let corpo = &src[abre + 1..fecha];
                    let entradas = varredura::entradas_de_objeto(corpo);
                    if entradas.len() >= 2 {
                        let g = grupo;
                        grupo += 1;
                        let alvo_s = alvo.to_string();
                        fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: false, gatilhos: vec![alvo_s.clone()], requisitos: vec![] });
                        for (a, b) in entradas {
                            let txt = corpo[a..b].trim_end();
                            let chave = chave_de_entrada(txt);
                            // Estático de **biblioteca** é citado qualificado
                            // (`core.x`), então vira gatilho. Estático de
                            // **classe** é citado com três partes
                            // (`html$.Event.MOUSEDOWN`), que a extração de
                            // referências não modela: vive com a classe, e só
                            // o seletor o poda.
                            let (gat, req) = match (&chave, e_ct, e_classe) {
                                (Some(k), true, _) => (vec![format!("C#{}", k.trim_start_matches('C'))], vec![]),
                                (Some(k), false, true) if por_membro && e_nome_de_seletor(k) => (vec![], vec![alvo_s.clone(), format!("sel:{k}")]),
                                (_, false, true) => (vec![alvo_s.clone()], vec![]),
                                (Some(k), false, false) => (vec![format!("{alvo_s}.{k}")], vec![]),
                                (None, _, _) => (vec![alvo_s.clone()], vec![]),
                            };
                            let corte = a + txt.len() - usize::from(txt.ends_with(','));
                            fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: false, gatilhos: gat, requisitos: req });
                        }
                        fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: false, gatilhos: vec![alvo_s.clone()], requisitos: vec![] });
                        continue;
                    }
                }
            }
        }
        // `dart.copyProperties(io, { … })` — membros de topo de biblioteca.
        if let Some(r) = tt.strip_prefix("dart.copyProperties(") {
            if let Some((alvo, _)) = primeiro_ident(r) {
                if e_lib(alvo) {
                    if let Some(rel) = t.find('{') {
                        let alvo = alvo.to_string();
                        if partir_objeto(src, ini, fim, ini + rel, &alvo, false, &mut fatias, &mut grupo) {
                            continue;
                        }
                    }
                }
            }
        }
        // `dart_rti._Universe.addRules(u, JSON.parse('{"core|List":{…},…}'))`
        // — 315 KB de regras de subtipagem, uma chave por classe.
        if tt.starts_with("dart_rti._Universe.add") && t.contains("JSON.parse(") {
            let jp = t.find("JSON.parse(").unwrap();
            if let Some(rel) = t[jp..].find('{') {
                let abre = ini + jp + rel;
                if let Some(fecha) = varredura::fecha_chave(src, abre) {
                    let corpo = &src[abre + 1..fecha];
                    let entradas = varredura::entradas_de_objeto(corpo);
                    if entradas.len() >= 2 {
                        let g = grupo;
                        grupo += 1;
                        fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: true, gatilhos: vec![], requisitos: vec![] });
                        for (a, b) in entradas {
                            let txt = corpo[a..b].trim_end();
                            // A chave vem escapada dentro da string do JS: `\"core|List\"`.
                            let sym = txt
                                .trim_start()
                                .trim_start_matches(['\\', '"', '\''])
                                .split_once('|')
                                .and_then(|(ns, resto)| {
                                    let fim = resto.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
                                    let ns = ns_da_receita(ns);
                                    e_lib(ns).then(|| format!("{ns}.{}", &resto[..fim]))
                                });
                            let corte = a + txt.len() - usize::from(txt.ends_with(','));
                            match sym {
                                Some(s) => fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: false, gatilhos: vec![s], requisitos: vec![] }),
                                None => fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: true, gatilhos: vec![], requisitos: vec![] }),
                            }
                        }
                        fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: true, gatilhos: vec![], requisitos: vec![] });
                        continue;
                    }
                }
            }
        }
        // `var S = { $delete: dartx.delete = Symbol("dartx.delete"), … }`
        // A entrada define dois nomes: `S.$delete` (usado nos corpos) e
        // `dartx.delete` (usado pelos nossos módulos) — qualquer um a acende.
        if (tt.starts_with("var S = {") || tt.starts_with("var S$")) && t.contains("Symbol(") {
            if let Some(rel) = t.find('{') {
                let abre = ini + rel;
                if let Some(fecha) = varredura::fecha_chave(src, abre) {
                    let corpo = &src[abre + 1..fecha];
                    let entradas = varredura::entradas_de_objeto(corpo);
                    if entradas.len() >= 2 {
                        let var = tt.trim_start_matches("var ").split(' ').next().unwrap_or("S").to_string();
                        let g = grupo;
                        grupo += 1;
                        fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: true, gatilhos: vec![], requisitos: vec![] });
                        for (a, b) in entradas {
                            let txt = corpo[a..b].trim_end();
                            let mut gat = Vec::new();
                            if let Some(k) = chave_de_entrada(txt) {
                                gat.push(format!("{var}.{k}"));
                            }
                            // `dartx.nome = Symbol(…)` ou `dart.privateName(lib, "_nome")`
                            if let Some(p) = txt.find("dartx.") {
                                let resto = &txt[p + 6..];
                                let f = ident_em(resto.as_bytes(), 0);
                                if f > 0 {
                                    gat.push(format!("dartx.{}", &resto[..f]));
                                }
                            }
                            let corte = a + txt.len() - usize::from(txt.ends_with(','));
                            let vazio = gat.is_empty();
                            fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: vazio, gatilhos: gat, requisitos: vec![] });
                        }
                        fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: true, gatilhos: vec![], requisitos: vec![] });
                        continue;
                    }
                }
            }
        }
        // `dart.defineExtensionMethods(C, [a, b])` e o irmão de
        // acessores: instalam em `C.prototype[dartx.a]` o que está em
        // `C.prototype.a`. Se o membro `a` foi podado e o nome ficou na lista,
        // o runtime faz `defineProperty(proto, dartx.a, undefined)` e lança
        // "Property description must be an object" — foi o primeiro defeito que
        // a granularidade por membro produziu. Cada nome vira uma unidade com a
        // **mesma** condição do membro: `[a classe, o seletor]`.
        if tt.starts_with("dart.defineExtensionMethods(") || tt.starts_with("dart.defineExtensionAccessors(") {
            if let Some(p) = tt.find('(') {
                if let Some(alvo) = alvo_qualificado(&tt[p + 1..]) {
                    if let Some(rel) = t.find('[') {
                        let abre = ini + rel;
                        if let Some(fecha) = varredura::fecha_colchete(src, abre) {
                            let corpo = &src[abre + 1..fecha];
                            let entradas = varredura::entradas_de_objeto(corpo);
                            if !entradas.is_empty() && por_membro {
                                let g = grupo;
                                grupo += 1;
                                fatias.push(Fatia { ini, fim: abre + 1, grupo: None, sempre: false, gatilhos: vec![alvo.clone()], requisitos: vec![] });
                                for (a, b) in entradas {
                                    let txt = corpo[a..b].trim_end();
                                    let nome = txt.trim().trim_end_matches(',').trim().trim_matches(['\x27', '"']);
                                    let corte = a + txt.len() - usize::from(txt.ends_with(','));
                                    let req = if e_nome_de_seletor(nome) { vec![alvo.clone(), format!("sel:{nome}")] } else { vec![alvo.clone()] };
                                    fatias.push(Fatia { ini: abre + 1 + a, fim: abre + 1 + corte, grupo: Some(g), sempre: false, gatilhos: vec![], requisitos: req });
                                }
                                fatias.push(Fatia { ini: fecha, fim, grupo: None, sempre: false, gatilhos: vec![alvo.clone()], requisitos: vec![] });
                                continue;
                            }
                        }
                    }
                    fatias.push(simples(vec![alvo]));
                    continue;
                }
            }
        }
        // `dart.registerExtension("XMLHttpRequest", html$.HttpRequest);`
        // — o alvo é o **segundo** argumento.
        if let Some(r) = tt.strip_prefix("dart.registerExtension(") {
            if let Some((tag, resto)) = r.split_once(',') {
                if let Some(v) = alvo_qualificado(resto) {
                    let tag = tag.trim().trim_matches(['"', '\x27']);
                    if TAGS_EMBUTIDAS.contains(&tag) {
                        raizes.push(v.clone());
                    }
                    fatias.push(simples(vec![v]));
                    continue;
                }
            }
        }
        // `lib.Nome = …` / `lib.Nome[…] = …`
        if let Some(v) = declaracao_qualificada(tt) {
            if por_membro && tt.contains("= class ") && partir_classe(src, ini, fim, &v, &mut fatias, &mut grupo) {
                continue;
            }
            fatias.push(simples(vec![v]));
            continue;
        }
        // `lib['A|b'] = …` — membro de extensão, cujo nome Dart tem `|` e `#`
        // e por isso só existe como string. A string é lida inteira: uma
        // janela de tamanho fixo truncava
        // `collection['NullableIterableExtensions|get#nonNulls']` e a
        // declaração ficava presa a um símbolo que ninguém cita.
        if let Some((a, i)) = primeiro_ident(tt) {
            if e_lib(a) && tt.as_bytes().get(i) == Some(&b'[') {
                let b = tt.as_bytes();
                if matches!(b.get(i + 1), Some(b'\'') | Some(b'"')) {
                    let aspa = b[i + 1];
                    let ini = i + 2;
                    let mut j = ini;
                    while j < b.len() && b[j] != aspa {
                        j += 1;
                    }
                    if j < b.len() {
                        fatias.push(simples(vec![format!("{a}.{}", &tt[ini..j])]));
                        continue;
                    }
                }
            }
        }
        // `(lib.Nome.ctor = function …).prototype = …` / `(Var[dart.mixinNew] = …)`
        if let Some(r) = tt.strip_prefix('(') {
            if let Some(v) = alvo_qualificado(r) {
                fatias.push(simples(vec![v]));
                continue;
            }
            if let Some((a, i)) = primeiro_ident(r) {
                if matches!(r.as_bytes().get(i), Some(b'.') | Some(b'[')) {
                    fatias.push(simples(vec![a.to_string()]));
                    continue;
                }
            }
        }
        // `dart.qualquerCoisa(lib.Nome, …)` / `dart.applyMixin(Var, …)`
        if tt.starts_with("dart.") || tt.starts_with("dart_rti.") {
            if let Some(p) = tt.find('(') {
                let arg = &tt[p + 1..];
                if let Some(v) = alvo_qualificado(arg) {
                    fatias.push(simples(vec![v]));
                    continue;
                }
                if let Some((a, i)) = primeiro_ident(arg) {
                    if !e_lib(a) && matches!(arg.as_bytes().get(i), Some(b',') | Some(b')')) {
                        fatias.push(simples(vec![a.to_string()]));
                        continue;
                    }
                }
            }
        }
        // `var X = …`
        for pref in ["var ", "let ", "const "] {
            if let Some(r) = tt.strip_prefix(pref) {
                if let Some((a, _)) = primeiro_ident(r) {
                    fatias.push(simples(vec![a.to_string()]));
                    break;
                }
            }
        }
        if fatias.last().map(|f| f.ini) != Some(ini) {
            fatias.push(sempre());
        }
    }
    // As classes das bibliotecas-raiz: varre o que já foi classificado.
    for f in &fatias {
        for g in &f.gatilhos {
            if let Some((lib, _)) = g.split_once('.') {
                if LIBS_RAIZ.contains(&lib) {
                    raizes.push(g.clone());
                }
            }
        }
    }
    raizes.sort();
    raizes.dedup();
    (fatias, raizes)
}

/// Símbolos e seletores que os nossos módulos usam — as raízes do alcance.
pub fn raizes_do_usuario(modulos: &[Modulo]) -> Vec<String> {
    let mut v = Vec::new();
    for m in modulos {
        referencias(&m.corpo, &mut v, true);
        seletores(&m.corpo, &mut v);
        for ns in &m.namespaces {
            referencias(ns, &mut v, true);
        }
    }
    // O bundle chama `main` no fim, e o `dart_sdk.js` precisa dos seus próprios
    // pontos de entrada de inicialização mesmo sem ninguém os citar.
    for r in ["sel:main", "dart.trackLibraries", "dart._checkModuleNullSafetyMode", "dart.global", "dart.typeUniverse", "dart_rti._theUniverse", "dart_rti._Universe", "core.Object", "core.Error", "_interceptors.Interceptor", "_js_helper.Primitives"] {
        v.push(r.to_string());
    }
    // Só símbolos **qualificados** viram raiz. Um identificador simples do
    // nosso módulo (`main`, `_is`, uma variável local) não pode significar um
    // `var` de topo do `dart_sdk.js`: aqueles são internos ao arquivo do DDC e
    // nunca foram exportados — o que atravessa a fronteira são os 38 espaços de
    // nomes de biblioteca, e esses nós citamos qualificados. Sem este filtro o
    // alcance explodia (4,8 MB contra 1,5 MB), porque nomes comuns como `name`
    // ou `length` casavam com os aliases de símbolo do SDK e acendiam o mundo.
    v.retain(|s| s.contains('.') || s.starts_with("C#") || s.starts_with("sel:"));
    v.sort();
    v.dedup();
    v
}

/// Nomes que o `dart_sdk.js` chama **por string** em objetos quaisquer:
/// `dart.dsend(o, "toJson", …)`, `dgsend(o, targs, "m", …)`, `dload(o, "x")`,
/// `dput(o, "x", v)`, `bind(o, "m")`. É o contrato do DDC — chamada tipada
/// vira propriedade direta, a não tipada leva o nome como dado
/// (`pkg/dev_compiler/lib/src/kernel/compiler.dart:6102-6126`) — e é a regra
/// (iii) da fronteira SDK→usuário (`docs/JS-PRODUCAO.md` §1.7): um membro do
/// usuário com um desses nomes pode ser chamado pelo runtime sem que o
/// programa o cite. O arquivo inteiro, não só o que sobrevive à poda: é
/// conservador e não cria dependência circular entre os dois mundos.
pub fn seletores_dinamicos(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out: Vec<String> = Vec::new();
    // Uma passada só: cada `(` olha o identificador que o precede (7 MB em
    // poucos milissegundos; nove `match_indices` custavam dezenas).
    let e_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    for (p, _) in b.iter().enumerate().filter(|(_, c)| **c == b'(') {
        let mut i = p;
        while i > 0 && e_id(b[i - 1]) {
            i -= 1;
        }
        let pos_arg = match &src[i..p] {
            "dsend" | "dload" | "dput" | "bind" | "dsendRepl" | "dloadRepl" | "dputRepl" => 1usize,
            "dgsend" | "dgsendRepl" => 2,
            _ => continue,
        };
        {
            let mut j = p + 1;
            let mut arg = 0usize;
            let mut prof = 0i32;
            // Avança até o início do argumento `pos_arg` (vírgula em profundidade zero).
            while j < b.len() && arg < pos_arg {
                match b[j] {
                    b'(' | b'[' | b'{' => prof += 1,
                    b')' | b']' | b'}' => {
                        if prof == 0 {
                            break;
                        }
                        prof -= 1;
                    }
                    b'"' | b'\'' => {
                        let aspa = b[j];
                        j += 1;
                        while j < b.len() && b[j] != aspa {
                            if b[j] == b'\\' {
                                j += 1;
                            }
                            j += 1;
                        }
                    }
                    b',' if prof == 0 => arg += 1,
                    _ => {}
                }
                j += 1;
            }
            if arg != pos_arg {
                continue;
            }
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let aspa = b[j];
                let ini = j + 1;
                let mut k = ini;
                while k < b.len() && b[k] != aspa && b[k] != b'\n' {
                    k += 1;
                }
                let s = &src[ini..k];
                if !s.is_empty() && s.len() < 64 && e_nome_de_seletor(s) {
                    out.push(s.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Poda o `dart_sdk.js`: devolve o texto podado, o total de unidades e as vivas.
pub fn podar(src: &str, raizes: &[String], por_membro: bool) -> (String, usize, usize) {
    let aliases = crate::bundle::aliases_exportados(src);
    let (fatias, raizes_do_runtime) = classificar(src, por_membro);
    let mut simbolos = Simbolos::default();
    let mut unidades: Vec<Unidade> = Vec::with_capacity(fatias.len());
    let mut buf_refs: Vec<String> = Vec::new();
    let mut buf_sels: Vec<String> = Vec::new();
    for f in &fatias {
        let t = &src[f.ini..f.fim];
        buf_refs.clear();
        referencias(t, &mut buf_refs, false);
        buf_sels.clear();
        if por_membro {
            seletores(t, &mut buf_sels);
        }
        let mut u = Unidade {
            sempre: f.sempre,
            gatilhos: f.gatilhos.iter().map(|s| simbolos.interna(s)).collect(),
            requisitos: f.requisitos.iter().map(|s| simbolos.interna(s)).collect(),
            refs: Vec::with_capacity(buf_refs.len()),
            seletores: Vec::with_capacity(buf_sels.len()),
        };
        let mut vistos: HashSet<&str> = HashSet::new();
        for r in &buf_refs {
            if vistos.insert(r.as_str()) {
                u.refs.push(simbolos.interna(r));
            }
        }
        vistos.clear();
        for r in &buf_sels {
            if vistos.insert(r.as_str()) {
                u.seletores.push(simbolos.interna(r));
            }
        }
        unidades.push(u);
    }
    let raizes: Vec<u32> = raizes.iter().chain(raizes_do_runtime.iter())
        // As variáveis locais exportadas sob outro nome precisam existir
        // mesmo quando o programa só cita o alias da biblioteca.
        .chain(aliases.iter().map(|(original, _)| original))
        .map(|r| simbolos.interna(r)).collect();
    let viva = alcance::resolver(&unidades, &raizes, simbolos.total());
    let vivas = viva.iter().filter(|v| **v).count();

    // `DARTFORGE_JSPROD_DEBUG=1` diz **quem** está ocupando o arquivo. É por
    // onde se acha o símbolo que puxa o mundo (foi assim que se descobriu que
    // `dart.applyMixin` estava sendo lido como declaração, e não como chamada).
    if std::env::var("DARTFORGE_JSPROD_DEBUG").is_ok_and(|v| v != "0") {
        let mut por_simbolo: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, f) in fatias.iter().enumerate() {
            if !viva[i] {
                continue;
            }
            let chave = f.gatilhos.first().or(f.requisitos.first()).map(String::as_str).unwrap_or("(sempre)");
            *por_simbolo.entry(chave).or_default() += f.fim - f.ini;
        }
        let mut v: Vec<(&str, usize)> = por_simbolo.into_iter().collect();
        // Empate no tamanho desempata pelo símbolo (o `HashMap` não tem ordem).
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        eprintln!("[jsprod] {} raízes, {} símbolos, {vivas}/{} unidades vivas", raizes.len(), simbolos.total(), fatias.len());
        for (s, n) in v.iter().take(25) {
            eprintln!("[jsprod] {n:>8}  {s}");
        }
    }
    // `DARTFORGE_JSPROD_GATILHO=html$` lista as unidades vivas presas a um símbolo.
    if let Ok(alvo) = std::env::var("DARTFORGE_JSPROD_GATILHO") {
        for alvo in alvo.split(',') {
            for (i, f) in fatias.iter().enumerate() {
                if viva[i] && (f.gatilhos.iter().any(|g| g == alvo) || f.requisitos.iter().any(|g| g == alvo)) {
                    let t = &src[f.ini..f.fim];
                    eprintln!("[jsprod] [{alvo}] {:>7}B {:?}", f.fim - f.ini, &t[..t.len().min(100)]);
                }
            }
        }
    }
    // `DARTFORGE_JSPROD_QUEM=html$.Element` lista as unidades vivas que citam
    // o símbolo — com o texto, que é o que permite corrigir a classificação.
    if let Ok(alvo) = std::env::var("DARTFORGE_JSPROD_QUEM") {
        for alvo in alvo.split(',') {
            let Some(d) = simbolos.procura(alvo) else {
                eprintln!("[jsprod] {alvo}: símbolo inexistente");
                continue;
            };
            let mut n = 0;
            for (i, u) in unidades.iter().enumerate() {
                if viva[i] && u.refs.contains(&d) && n < 6 {
                    let t = &src[fatias[i].ini..fatias[i].fim];
                    let chave = fatias[i].gatilhos.first().or(fatias[i].requisitos.first()).map(String::as_str).unwrap_or("(sempre)");
                    eprintln!("[jsprod] {alvo} citado por [{chave}] {:?}", &t[..t.len().min(110)]);
                    n += 1;
                }
            }
        }
    }
    // `DARTFORGE_JSPROD_CAMINHO=core._BigIntImpl` responde quem o puxou.
    if let Ok(alvo) = std::env::var("DARTFORGE_JSPROD_CAMINHO") {
        for alvo in alvo.split(',') {
            match simbolos.procura(alvo).and_then(|d| alcance::caminho(&unidades, &raizes, simbolos.total(), d)) {
                Some(c) => eprintln!("[jsprod] {alvo} <- {}", c.iter().rev().map(|&s| simbolos.nome(s)).collect::<Vec<_>>().join(" <- ")),
                None => eprintln!("[jsprod] {alvo}: não alcançado"),
            }
        }
    }

    // Reemite na ordem do arquivo. As entradas de um grupo (objeto ou JSON)
    // precisam de vírgula entre as vivas — e no JSON a vírgula sobrando é erro
    // de sintaxe, não tolerância.
    let mut out = String::with_capacity(src.len() / 2);
    let mut grupo_aberto: Option<u32> = None;
    for (i, f) in fatias.iter().enumerate() {
        if !viva[i] {
            continue;
        }
        match f.grupo {
            Some(g) => {
                if grupo_aberto == Some(g) {
                    out.push(',');
                }
                grupo_aberto = Some(g);
            }
            None => grupo_aberto = None,
        }
        let t = &src[f.ini..f.fim];
        if t.starts_with("export {") {
            continue;
        }
        out.push_str(t);
    }
    for (original, alias) in aliases {
        out.push_str(&format!("var {alias} = {original};\n"));
    }
    (out, fatias.len(), vivas)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn referencia_receita_rti() {
        let mut v = Vec::new();
        referencias("eval(u, \"core|List<core|int>\", true)", &mut v, false);
        assert!(v.contains(&"core.List".to_string()), "{v:?}");
        assert!(v.contains(&"core.int".to_string()), "{v:?}");
    }

    #[test]
    fn referencia_html_com_cifrao() {
        let mut v = Vec::new();
        referencias("\"html|Element\"", &mut v, false);
        assert!(v.contains(&"html$.Element".to_string()), "{v:?}");
    }

    #[test]
    fn alias_do_sdk_em_modulo_usuario_aponta_para_o_nome_local() {
        let mut v = Vec::new();
        referencias("html.Element; svg['SvgElement'];", &mut v, true);
        assert!(v.contains(&"html$.Element".to_string()), "{v:?}");
        assert!(v.contains(&"svg$.SvgElement".to_string()), "{v:?}");
        let src = "var html$ = Object.create(dart.library);\nexport { html$ as html };\nhtml$.Element = class Element {};\n";
        let (out, _, _) = podar(src, &["html$.Element".into()], false);
        assert!(out.contains("var html$ ="), "{out}");
        assert!(out.contains("var html = html$;"), "{out}");
        assert!(out.contains("html$.Element ="), "{out}");
    }

    #[test]
    fn referencia_constante_e_membro_com_barra() {
        let mut v = Vec::new();
        referencias("C[42] + CT.C7 + async['FutureRecord2|get#wait']", &mut v, false);
        assert!(v.contains(&"C#42".to_string()), "{v:?}");
        assert!(v.contains(&"C#7".to_string()), "{v:?}");
        assert!(v.contains(&"async.FutureRecord2|get#wait".to_string()), "{v:?}");
    }

    #[test]
    fn nao_confunde_campo_com_biblioteca() {
        let mut v = Vec::new();
        referencias("this.core = opts.io;", &mut v, false);
        assert!(!v.contains(&"core.x".to_string()));
        assert!(v.contains(&"this".to_string()));
    }

    #[test]
    fn membros_de_classe() {
        assert_eq!(nome_do_membro("  toString() {\n  }").as_deref(), Some("toString"));
        assert_eq!(nome_do_membro("  get length() { return 1; }").as_deref(), Some("length"));
        assert_eq!(nome_do_membro("  static f(a) {}").as_deref(), Some("f"));
        assert_eq!(nome_do_membro("  [_priv](a) {}").as_deref(), Some("_priv"));
        assert_eq!(nome_do_membro("  [$add](a) {}").as_deref(), Some("add"));
        assert_eq!(nome_do_membro("  ['A|b'](a) {}").as_deref(), Some("A|b"));
        assert_eq!(nome_do_membro("  get [S.$head]() { return this.head; }").as_deref(), Some("head"));
        assert_eq!(nome_do_membro("  get [S$2.$console]() { return x; }").as_deref(), Some("console"));
        let mut refs = Vec::new();
        seletores("document[S.$head]; window[S$2.$console];", &mut refs);
        assert!(refs.contains(&"sel:head".to_string()), "{refs:?}");
        assert!(refs.contains(&"sel:console".to_string()), "{refs:?}");
    }

    /// Regressão do defeito que fazia `dart.applyMixin(V, M);` ser lida como
    /// declaração de `dart.applyMixin` — símbolo que só os próprios sítios de
    /// chamada referenciam, então o conjunto inteiro se apagava.
    #[test]
    fn chamada_nao_e_declaracao() {
        assert_eq!(declaracao_qualificada("core.Object = class {};"), Some("core.Object".into()));
        assert_eq!(declaracao_qualificada("core.Object[dart.x] = 1;"), Some("core.Object".into()));
        assert_eq!(declaracao_qualificada("dart.applyMixin(V, M);"), None);
        assert_eq!(declaracao_qualificada("dart.setLibraryUri(core.Uri, I[1]);"), None);
        assert_eq!(declaracao_qualificada("core.a === core.b;"), None);
    }

    /// Regressão do defeito que punha `dart:html` inteiro num programa que só
    /// faz `print`: o alvo de `defineLazy` pode ser uma **classe**, e lendo só
    /// o primeiro identificador a cabeça do grupo ficava presa ao espaço de
    /// nomes `html$`, que está sempre vivo e cuja cabeça cita a classe.
    #[test]
    fn define_lazy_de_classe_nao_prende_no_espaco_de_nomes() {
        let src = concat!(
            "var html$ = Object.create(dart.library);\n",
            "html$.Event = class Event {};\n",
            "dart.defineLazy(html$.Event, {\n",
            "  get A() { return 1; },\n",
            "  get B() { return 2; }\n",
            "});\n",
        );
        // `html$` sozinho não pode acender `html$.Event`.
        let (out, _, _) = podar(src, &["html$".into()], false);
        assert!(!out.contains("html$.Event = class"), "{out}");
        let (out, _, _) = podar(src, &["html$.Event".into()], false);
        assert!(out.contains("html$.Event = class"), "{out}");
    }

    /// A lista de `defineExtensionMethods` tem de perder o nome junto com o
    /// membro: o runtime copia `C.prototype[nome]` para `C.prototype[dartx.nome]`
    /// e lança se o descritor não existir.
    #[test]
    fn lista_de_extensao_acompanha_o_membro() {
        let src = concat!(
            "var core = Object.create(dart.library);\n",
            "core.C = class C {\n  viva() { return 1; }\n  morta() { return 2; }\n  outra() { return 3; }\n};\n",
            "dart.defineExtensionMethods(core.C, ['viva', 'morta', 'outra']);\n",
        );
        let (out, _, _) = podar(src, &["core.C".into(), "sel:viva".into()], true);
        assert!(out.contains("'viva'"), "{out}");
        assert!(!out.contains("'morta'"), "{out}");
        // A lista continua sintaticamente válida (vírgulas recolocadas).
        assert!(out.contains("dart.defineExtensionMethods(core.C, ["), "{out}");
    }

    #[test]
    fn poda_conserva_o_que_vive_e_some_com_o_resto() {
        let src = concat!(
            "var core = Object.create(dart.library);\n",
            "core.Usada = class Usada { f() { return 1; } };\n",
            "core.Morta = class Morta { g() { return 2; } };\n",
        );
        let (out, total, vivas) = podar(src, &["core.Usada".into(), "sel:f".into()], false);
        assert!(out.contains("core.Usada"), "{out}");
        assert!(!out.contains("core.Morta"), "{out}");
        assert!(vivas < total);
    }
}
