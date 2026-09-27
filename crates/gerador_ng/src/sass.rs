//! O Sass que os projetos reais usam — e só ele.
//!
//! `styleUrls: ['x.css']` aponta para um arquivo que não existe no disco: o
//! `sass_builder` o gera de `x.scss`. Sem isto, nenhum componente com folha
//! de estilo pode ser gerado sem o `build_runner`.
//!
//! Medido nos 155 `.scss` do `new_sali/frontend` (139 em `lib/`, 16 em
//! `web/`): aninhamento com `&`, variáveis, comentários `//`, `@media` e —
//! na folha global `web/style.scss`, que o `index.html` carrega — `@use
//! 'nome' as *`. Nenhum usa `@mixin`, `@include`, `@extend`, `@function`,
//! `@each`, `@for`, `@if`, `map-get` ou placeholder. É esse o subconjunto
//! aqui.
//!
//! O resto é **recusado**. Como a saída ainda passa pelo shim do ngdart, que
//! remove comentários e minifica, o CSS intermediário não precisa sair byte a
//! byte igual ao do `sass_builder` — o que precisa bater é o
//! `.css.shim.dart`, e é contra ele que a verificação roda.
//!
//! O `.css` do `sass_builder` (`compilar_com`) é outro contrato: byte a byte
//! nos dois estilos, com as regras de valor, de seletor, de módulo e de
//! espaçamento do dart-sass 1.102.0, conferidas por forma em
//! `tests/sass_formas`. O caminho do shim (`compilar`/`compilar_em`) mantém
//! as normalizações do `compressed` que o new_sali confirmou, mesmo quando o
//! projeto usa o `expanded` (o padrão): uma folha `.scss` de componente num
//! projeto `expanded` com `0.5rem` ou `white` pode dar shim diferente do
//! oficial — o que o shim do ngdart faz com essas formas não foi medido.
use crate::visao::Motivo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Compila Sass para CSS, sem resolver módulos (`@use`/`@import`).
pub fn compilar(fonte: &str) -> Result<String, Motivo> {
    compilar_em(fonte, None)
}

/// Compila Sass resolvendo `@use` e `@import` a partir de `dir`.
///
/// `@use 'nome' as *` carrega o módulo e põe os membros dele no escopo — na
/// prática, para gerar CSS: emite o CSS do módulo antes e compartilha as
/// variáveis. Namespace explícito (`nome.$x`) não aparece nos projetos e é
/// recusado.
pub fn compilar_em(fonte: &str, dir: Option<&Path>) -> Result<String, Motivo> {
    let mut variaveis = HashMap::new();
    let mut c = Compilacao::nova(Modo::Shim);
    compilar_modulo(fonte, dir, &mut variaveis, &mut c).map(|css| css.replace(GRUPO, ""))
}

/// Para quem é o CSS intermediário: o shim do ngdart (que só precisa do
/// mesmo `.css.shim.dart`; as normalizações de valor são as do `compressed`
/// que o new_sali confirmou) ou o `.css` do `sass_builder` byte a byte
/// (`compilar_com`) num dos dois estilos, com as regras de valor e de
/// seletor do dart-sass 1.102.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modo {
    Shim,
    Comprimido,
    Expandido,
}

/// Marca, no CSS intermediário, o fim de uma regra de estilo da raiz: o
/// dart-sass marca o último nó emitido até ali como fim de grupo
/// (`isGroupEnd`, `visitStyleRule` do `evaluate`), e o `expanded` põe uma
/// linha em branco depois dele. Um `@media` da raiz não marca.
const GRUPO: char = '\u{1}';

/// Estado de uma compilação: os módulos já carregados por `@use` (o CSS de
/// um módulo sai uma vez, e as variáveis dele ficam guardadas para outro
/// `@use ... as *`), a pilha de módulos em curso (ciclo é erro no oficial) e
/// todo arquivo lido (as consultas da ação).
struct Compilacao {
    modo: Modo,
    usados: Vec<(PathBuf, HashMap<String, String>)>,
    pilha: Vec<PathBuf>,
    lidos: Vec<PathBuf>,
}

impl Compilacao {
    fn nova(modo: Modo) -> Self {
        Compilacao { modo, usados: Vec::new(), pilha: Vec::new(), lidos: Vec::new() }
    }
}

/// O estilo de saída do `sass_builder` (`outputStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estilo {
    Expandido,
    Comprimido,
}

/// Compila como o `sass_builder` 2.2.1 (o dart-sass do lock): o CSS **byte a
/// byte** do oficial no estilo pedido — sem o comentário
/// `/*# sourceMappingURL=… */` do fim, que depende do nome do arquivo e de
/// haver mapa; quem escreve o `.css` o acrescenta —, e os módulos que
/// `@use`/`@import` abriram, que também são entradas da compilação.
///
/// Conferido forma a forma contra o `sass_builder` de verdade nos dois
/// estilos (`tests/sass_formas`, oráculo por `scripts/sass-formas.sh`): 73
/// formas, 57 iguais no `compressed` e 54 no `expanded`, o resto recusado,
/// 0 diferentes. Antes do oráculo por forma, o `compressed` tinha sido
/// medido nos 144 `.css` do new_sali (114 iguais, 0 diferentes); o oráculo
/// achou onze formas que ele escrevia diferente (nome de cor `blue` virando
/// `#00f`, `1.50em`, `-0.5`, `0.0em`, BOM de saída não ASCII, `@import`
/// repetido ou aninhado, declaração depois de regra aninhada, ordem das
/// listas aninhadas, `@media` com lista, espaço em propriedade customizada,
/// variável dentro de regra) — hoje iguais ou recusadas.
pub fn compilar_com(
    fonte: &str,
    dir: Option<&Path>,
    estilo: Estilo,
) -> Result<(String, Vec<PathBuf>), Motivo> {
    let modo = match estilo {
        Estilo::Comprimido => Modo::Comprimido,
        Estilo::Expandido => Modo::Expandido,
    };
    let mut variaveis = HashMap::new();
    let mut c = Compilacao::nova(modo);
    // Propriedade customizada guarda o texto como veio: `$x` ali não é
    // variável para o Sass, mas o nosso `substituir` a trocaria.
    if customizada_com_sass(fonte) {
        return Err(Motivo::Estilos);
    }
    let compacto = compilar_modulo(fonte, dir, &mut variaveis, &mut c)?;
    let vistos = c.lidos;
    for m in &vistos {
        let texto = std::fs::read_to_string(m).map_err(|_| Motivo::Estilos)?;
        if customizada_com_sass(&texto) {
            return Err(Motivo::Estilos);
        }
        // O `expanded` preserva os comentários `/* */` no lugar em que estão.
        if modo == Modo::Expandido && texto.contains("/*") {
            return Err(Motivo::Estilos);
        }
    }
    if modo == Modo::Expandido {
        if fonte.contains("/*") {
            return Err(Motivo::Estilos);
        }
        let mut css = expandir(&compacto)?;
        // Caractere fora do ASCII: o `expanded` declara o `@charset`.
        if !css.is_ascii() {
            css.insert_str(0, "@charset \"UTF-8\";\n");
        }
        css.push('\n');
        return Ok((css, vistos));
    }
    let compacto = compacto.replace(GRUPO, "");
    // O `compressed` tira os comentários `/* */`; o `/*! */` fica, e ainda
    // não tem caso.
    if compacto.contains("/*!") {
        return Err(Motivo::Estilos);
    }
    let compacto = sem_comentarios_de_bloco(&compacto)?;
    let mut css = String::with_capacity(compacto.len());
    comprimir_regras(&compacto, &mut css)?;
    // Folha vazia também termina em `\n` (o arquivo é `\n\n/*# … */\n`).
    css.push('\n');
    // Saída com caractere fora do ASCII: o `compressed` do dart-sass põe a
    // marca de ordem de bytes no começo (no lugar do `@charset`).
    if !css.is_ascii() {
        css.insert(0, '\u{feff}');
    }
    Ok((css, vistos))
}

/// O token como o `expanded` do dart-sass o escreve: cor como foi escrita
/// (`SpanColorFormat`), número com o zero à esquerda (`0.5`, `1.5` para
/// `1.50`), lista com barra parte a parte.
fn token_expandido(t: &str) -> Result<String, Motivo> {
    if let Some(d) = t.strip_prefix('#') {
        // Com alfa (`#rgba`, `#rrggbbaa`) até o `expanded` escreve `rgba(…)`.
        if d.chars().all(|c| c.is_ascii_hexdigit()) && matches!(d.len(), 4 | 8) {
            return Err(Motivo::Estilos);
        }
        return Ok(t.to_string());
    }
    if t.contains('/') {
        return t.split('/').map(|p| numero_ou_texto(p, false)).collect::<Result<Vec<_>, _>>().map(|v| v.join("/"));
    }
    numero_ou_texto(t, false)
}

/// Um seletor complexo do `expanded`: espaços colapsados e um espaço de
/// cada lado dos combinadores `>`, `+` e `~` (fora de `[...]`, `(...)` e
/// aspas).
fn complexo_expandido(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    for c in espacos_colapsados(s).chars() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '[' | '(' => nivel += 1,
            ']' | ')' => nivel = nivel.saturating_sub(1),
            _ => {}
        }
        if nivel == 0 && matches!(c, '>' | '+' | '~') {
            while saida.ends_with(' ') {
                saida.pop();
            }
            if !saida.is_empty() {
                saida.push(' ');
            }
            saida.push(c);
            saida.push(' ');
            continue;
        }
        if c == ' ' && saida.ends_with(' ') {
            continue;
        }
        saida.push(c);
    }
    saida.trim().to_string()
}

/// Os seletores complexos de uma lista escrita no fonte, com a marca de
/// quebra de linha do dart-sass: um complexo "quebra" quando começa numa
/// linha diferente da última quebra (`parse/selector.dart`, `lineBreak`).
fn complexos_do_fonte(cabeca: &str) -> Result<Vec<(String, bool)>, Motivo> {
    let mut v = Vec::new();
    let mut linha = 0usize;
    let mut ultima = 0usize;
    for (i, parte) in partes_de_lista(cabeca).into_iter().enumerate() {
        let inicio = parte.len() - parte.trim_start().len();
        let linha_do_complexo = linha + parte[..inicio].matches('\n').count();
        linha += parte.matches('\n').count();
        let quebra = i > 0 && linha_do_complexo != ultima;
        if quebra {
            ultima = linha_do_complexo;
        }
        let c = complexo_expandido(parte);
        if c.is_empty() {
            return Err(Motivo::Estilos);
        }
        v.push((c, quebra));
    }
    Ok(v)
}

/// Os seletores complexos do seletor intermediário do `expanded` (os
/// separadores são `,\n` para quem quebra e `, ` para quem não).
fn complexos_intermediarios(seletor: &str) -> Vec<(String, bool)> {
    partes_de_lista(seletor)
        .into_iter()
        .enumerate()
        .map(|(i, p)| (p.trim().to_string(), i > 0 && p.starts_with('\n')))
        .collect()
}

/// Divide nas vírgulas de fora de `[...]`, `(...)` e aspas.
fn partes_de_lista(s: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    let mut ini = 0;
    for (i, c) in s.char_indices() {
        if let Some(q) = aspas {
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '[' | '(' => nivel += 1,
            ']' | ')' => nivel = nivel.saturating_sub(1),
            ',' if nivel == 0 => {
                v.push(&s[ini..i]);
                ini = i + 1;
            }
            _ => {}
        }
    }
    v.push(&s[ini..]);
    v
}

/// `SelectorList.nestWithin` do dart-sass com a marca de quebra: sem `&`, o
/// pai concatena o filho (`lineBreak` de um ou de outro); com `&` no começo
/// do filho, cada complexo do pai o substitui e a quebra é a do pai. O resto
/// das formas com `&` (no meio, em pseudo, mais de um) fica de fora.
fn juntar_expandido(pai: &str, cabeca: &str) -> Result<String, Motivo> {
    let filhos = complexos_do_fonte(cabeca)?;
    let resultado: Vec<(String, bool)> = if pai.is_empty() {
        if filhos.iter().any(|(c, _)| c.contains('&')) {
            return Err(Motivo::Estilos);
        }
        filhos
    } else {
        let pais = complexos_intermediarios(pai);
        // Uma lista por filho, cada uma com um complexo por pai, e depois
        // `flattenVertically`: o primeiro de cada, o segundo de cada...
        let mut por_filho = Vec::new();
        for (f, quebra_f) in &filhos {
            let mut lista = Vec::new();
            match f.matches('&').count() {
                0 => {
                    for (p, quebra_p) in &pais {
                        lista.push((format!("{p} {f}"), *quebra_p || *quebra_f));
                    }
                }
                1 if f.starts_with('&') && !f.contains('(') => {
                    for (p, quebra_p) in &pais {
                        lista.push((f.replacen('&', p, 1), *quebra_p));
                    }
                }
                _ => return Err(Motivo::Estilos),
            }
            por_filho.push(lista);
        }
        (0..pais.len()).flat_map(|i| por_filho.iter().map(move |l| l[i].clone())).collect()
    };
    let mut s = String::new();
    for (i, (c, quebra)) in resultado.iter().enumerate() {
        if i > 0 {
            s.push_str(if *quebra { ",\n" } else { ", " });
        }
        s.push_str(c);
    }
    Ok(s)
}

/// O CSS intermediário no estilo `expanded`: grupos da raiz separados por
/// uma linha em branco, regra `seletor {` com as declarações recuadas em
/// dois espaços, uma por linha, `}` na linha dela; `@media` com as regras
/// recuadas dentro. Regra sem declaração some.
fn expandir(texto: &str) -> Result<String, Motivo> {
    // Os nós da raiz, na ordem, com a marca de fim de grupo.
    let mut nos: Vec<(String, bool)> = Vec::new();
    let mut resto = texto.trim_start();
    while !resto.is_empty() {
        if let Some(r) = resto.strip_prefix(GRUPO) {
            if let Some(ultimo) = nos.last_mut() {
                ultimo.1 = true;
            }
            resto = r.trim_start();
            continue;
        }
        let abre = resto.find('{').ok_or(Motivo::Estilos)?;
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let bloco = &resto[..=fim];
        resto = resto[fim + 1..].trim_start();
        if bloco.contains(GRUPO) {
            return Err(Motivo::Estilos);
        }
        for r in regras_expandidas(bloco, 0)? {
            nos.push((r, false));
        }
    }
    let mut saida = String::with_capacity(texto.len() * 2);
    for (i, (no, _)) in nos.iter().enumerate() {
        if i > 0 {
            saida.push('\n');
            if nos[i - 1].1 {
                saida.push('\n');
            }
        }
        saida.push_str(no);
    }
    Ok(saida)
}

fn regras_expandidas(texto: &str, nivel: usize) -> Result<Vec<String>, Motivo> {
    let recuo = "  ".repeat(nivel);
    let mut v = Vec::new();
    let mut resto = texto.trim();
    while !resto.is_empty() {
        let abre = resto.find('{').ok_or(Motivo::Estilos)?;
        let cabeca = resto[..abre].trim();
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        resto = resto[fim + 1..].trim_start();
        if let Some(consulta) = cabeca.strip_prefix("@media") {
            let dentro = regras_expandidas(corpo, nivel + 1)?;
            if dentro.is_empty() {
                continue;
            }
            v.push(format!("{recuo}@media {} {{\n{}\n{recuo}}}", consulta_expandida(consulta)?, dentro.join("\n")));
            continue;
        }
        if cabeca.starts_with('@') {
            return Err(Motivo::Estilos);
        }
        let decls = declaracoes_expandidas(corpo, &recuo)?;
        if decls.is_empty() {
            continue;
        }
        let seletor = cabeca.replace(",\n", &format!(",\n{recuo}"));
        v.push(format!("{recuo}{seletor} {{\n{decls}\n{recuo}}}"));
    }
    Ok(v)
}

/// A consulta de um `@media` no `expanded`: espaços colapsados, `, ` entre
/// as consultas. Característica sem o espaço depois de `:` o dart-sass
/// reescreveria: recusa.
fn consulta_expandida(consulta: &str) -> Result<String, Motivo> {
    let c = espacos_colapsados(consulta);
    if c.is_empty() || c.contains(":)") || c.split(':').skip(1).any(|d| !d.starts_with(' ')) {
        return Err(Motivo::Estilos);
    }
    Ok(partes_de_lista(&c).iter().map(|p| p.trim()).collect::<Vec<_>>().join(", "))
}

/// `prop: valor;` por linha, recuado um nível além da regra.
fn declaracoes_expandidas(corpo: &str, recuo: &str) -> Result<String, Motivo> {
    let mut linhas = Vec::new();
    for p in instrucoes(corpo) {
        if p.trim().is_empty() {
            continue;
        }
        if p.contains(['{', '}']) {
            return Err(Motivo::Estilos);
        }
        let (prop, valor) = p.split_once(':').ok_or(Motivo::Estilos)?;
        let prop = prop.trim();
        let valor = if prop.starts_with("--") {
            let valor = valor.trim_end();
            if valor.contains(['\n', '\r']) {
                return Err(Motivo::Estilos);
            }
            let espaco = if valor.starts_with(char::is_whitespace) { " " } else { "" };
            format!("{espaco}{}", espacos_colapsados(valor))
        } else {
            let v = espacos_colapsados(valor);
            if v.contains('!') && !v.contains(" !important") {
                return Err(Motivo::Estilos);
            }
            format!(" {}", lista_expandida(&v))
        };
        linhas.push(format!("{recuo}  {prop}:{valor};"));
    }
    Ok(linhas.join("\n"))
}

/// A vírgula de lista no nível de cima com um espaço depois e nenhum antes.
fn lista_expandida(v: &str) -> String {
    partes_de_lista(v).iter().map(|p| p.trim()).collect::<Vec<_>>().join(", ")
}

/// Reescreve o CSS intermediário (regras já achatadas) na forma do
/// `compressed` do dart-sass: seletor sem espaço em volta de `,` `>` `+`
/// `~`, declarações `prop:valor` separadas por `;` sem o último, `@media`
/// colado ao `(`, regra vazia omitida.
fn comprimir_regras(texto: &str, saida: &mut String) -> Result<(), Motivo> {
    let mut resto = texto.trim();
    while !resto.is_empty() {
        let abre = resto.find('{').ok_or(Motivo::Estilos)?;
        let cabeca = resto[..abre].trim();
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        resto = resto[fim + 1..].trim_start();
        if let Some(consulta) = cabeca.strip_prefix("@media") {
            // A lista de consultas sai sem espaço depois da vírgula
            // (`@media screen,print`).
            let consulta = lista_comprimida(&espacos_colapsados(consulta));
            let mut dentro = String::new();
            comprimir_regras(corpo, &mut dentro)?;
            if dentro.is_empty() {
                continue;
            }
            saida.push_str("@media");
            if !consulta.starts_with('(') {
                saida.push(' ');
            }
            saida.push_str(&consulta);
            saida.push('{');
            saida.push_str(&dentro);
            saida.push('}');
            continue;
        }
        if cabeca.starts_with('@') {
            return Err(Motivo::Estilos);
        }
        let decls = declaracoes_comprimidas(corpo)?;
        if decls.is_empty() {
            continue;
        }
        saida.push_str(&seletor_comprimido(cabeca));
        saida.push('{');
        saida.push_str(&decls);
        saida.push('}');
    }
    Ok(())
}

/// Alguma `--prop: …` com `$` ou `#{` no valor?
fn customizada_com_sass(texto: &str) -> bool {
    texto.match_indices("--").any(|(i, _)| {
        let resto = &texto[i + 2..];
        let nome = resto
            .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(resto.len());
        let depois = resto[nome..].trim_start();
        let Some(valor) = depois.strip_prefix(':') else {
            return false;
        };
        let fim = valor.find([';', '}']).unwrap_or(valor.len());
        valor[..fim].contains('$') || valor[..fim].contains("#{")
    })
}

/// Tira os `/* … */`, fora de aspas.
fn sem_comentarios_de_bloco(s: &str) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(s.len());
    let mut aspas: Option<char> = None;
    let mut resto = s;
    while let Some(c) = resto.chars().next() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            resto = &resto[c.len_utf8()..];
            continue;
        }
        if resto.starts_with("/*") {
            let fim = resto[2..].find("*/").ok_or(Motivo::Estilos)?;
            resto = &resto[2 + fim + 2..];
            continue;
        }
        if c == '"' || c == '\'' {
            aspas = Some(c);
        }
        saida.push(c);
        resto = &resto[c.len_utf8()..];
    }
    Ok(saida)
}

/// Espaços em sequência viram um, fora de aspas; as pontas saem.
fn espacos_colapsados(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut aspas: Option<char> = None;
    let mut espaco = false;
    for c in s.trim().chars() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco {
            saida.push(' ');
            espaco = false;
        }
        if c == '"' || c == '\'' {
            aspas = Some(c);
        }
        saida.push(c);
    }
    saida
}

/// O seletor como o `compressed` o escreve.
fn seletor_comprimido(s: &str) -> String {
    let colapsado = espacos_colapsados(s);
    let mut saida = String::with_capacity(colapsado.len());
    let mut colchetes = 0usize;
    let mut aspas: Option<char> = None;
    let cs: Vec<char> = colapsado.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '[' => colchetes += 1,
            ']' => colchetes = colchetes.saturating_sub(1),
            _ => {}
        }
        let combinador = |x: char| matches!(x, ',' | '>' | '+' | '~');
        if c == ' ' && colchetes == 0 {
            let antes = saida.chars().last();
            let depois = cs.get(i + 1).copied();
            if antes.is_some_and(combinador) || depois.is_some_and(combinador) {
                continue;
            }
        }
        saida.push(c);
    }
    saida
}

/// A vírgula de lista no nível de cima sai sem espaço (`"Inter",system-ui`).
fn lista_comprimida(v: &str) -> String {
    let mut saida = String::with_capacity(v.len());
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    for c in v.chars() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '(' => nivel += 1,
            ')' => nivel = nivel.saturating_sub(1),
            _ => {}
        }
        if nivel == 0 {
            if c == ' ' && saida.ends_with(',') {
                continue;
            }
            if c == ',' && saida.ends_with(' ') {
                saida.pop();
            }
        }
        saida.push(c);
    }
    saida
}

/// `prop: valor; …` como `prop:valor;…`, sem o `;` final.
fn declaracoes_comprimidas(corpo: &str) -> Result<String, Motivo> {
    let mut partes = Vec::new();
    let mut atual = String::new();
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    for c in corpo.chars() {
        if let Some(q) = aspas {
            atual.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '(' => nivel += 1,
            ')' => nivel = nivel.saturating_sub(1),
            '{' | '}' => return Err(Motivo::Estilos),
            ';' if nivel == 0 => {
                partes.push(std::mem::take(&mut atual));
                continue;
            }
            _ => {}
        }
        atual.push(c);
    }
    partes.push(atual);
    let mut saida = Vec::new();
    for p in partes {
        if p.trim().is_empty() {
            continue;
        }
        let (prop, valor) = p.split_once(':').ok_or(Motivo::Estilos)?;
        let prop = prop.trim();
        // Propriedade customizada: o valor como veio, espaço do começo
        // inclusive (`--x: 1rem`).
        if prop.starts_with("--") {
            let valor = valor.trim_end();
            if valor.contains(['\n', '\r']) {
                return Err(Motivo::Estilos);
            }
            // O texto como veio, com os espaços em sequência reduzidos a um
            // (`--a:  1px  2px` sai `--a: 1px 2px`; `--x:1rem` fica).
            let espaco = if valor.starts_with(char::is_whitespace) { " " } else { "" };
            saida.push(format!("{prop}:{espaco}{}", espacos_colapsados(valor)));
            continue;
        }
        saida.push(format!(
            "{prop}:{}",
            lista_comprimida(&espacos_colapsados(valor))
        ));
    }
    Ok(saida.join(";"))
}

/// Compila um módulo: os `@use`/`@import` do começo, depois as regras.
/// Devolve o CSS e os nomes que entraram em `variaveis` por um `@use ... as
/// *` (membros de outro módulo, que este não reexporta).
fn compilar_modulo(
    fonte: &str,
    dir: Option<&Path>,
    variaveis: &mut HashMap<String, String>,
    c: &mut Compilacao,
) -> Result<String, Motivo> {
    compilar_modulo_com_membros(fonte, dir, variaveis, c).map(|(css, _)| css)
}

fn compilar_modulo_com_membros(
    fonte: &str,
    dir: Option<&Path>,
    variaveis: &mut HashMap<String, String>,
    c: &mut Compilacao,
) -> Result<(String, Vec<String>), Motivo> {
    let sem_comentario = tirar_comentarios(fonte);
    let (diretivas, corpo) = separar_modulos(&sem_comentario)?;
    let mut saida = String::with_capacity(sem_comentario.len());
    let mut de_fora = Vec::new();
    for d in diretivas {
        let dir = dir.ok_or(Motivo::Estilos)?;
        match d {
            // `@use`: o módulo é carregado uma vez por compilação (o CSS sai
            // uma vez), com escopo próprio — não vê as variáveis de quem o
            // usa —, e só com `as *` os membros públicos dele entram aqui.
            Diretiva::Use { alvo, estrela } => {
                let caminho = achar_modulo(dir, &alvo)?;
                let membros = match c.usados.iter().find(|(p, _)| *p == caminho) {
                    Some((_, m)) => m.clone(),
                    None => {
                        if c.pilha.contains(&caminho) {
                            return Err(Motivo::Estilos);
                        }
                        let texto = ler_modulo(&caminho, c)?;
                        c.pilha.push(caminho.clone());
                        let mut proprias = HashMap::new();
                        let (css, herdados) =
                            compilar_modulo_com_membros(&texto, caminho.parent(), &mut proprias, c)?;
                        c.pilha.pop();
                        for h in herdados {
                            proprias.remove(&h);
                        }
                        saida.push_str(&css);
                        c.usados.push((caminho.clone(), proprias.clone()));
                        proprias
                    }
                };
                if estrela {
                    for (nome, valor) in membros {
                        // Privado (`$-x`, `$_x`) não é membro.
                        if nome.starts_with(['-', '_']) {
                            continue;
                        }
                        // O mesmo nome vindo de dois módulos é ambíguo.
                        if variaveis.contains_key(&nome) {
                            return Err(Motivo::Estilos);
                        }
                        de_fora.push(nome.clone());
                        variaveis.insert(nome, valor);
                    }
                }
            }
            // `@import`: o arquivo é avaliado de novo a cada importação (o
            // CSS sai de novo) e compartilha as variáveis globais.
            Diretiva::Import(alvos) => {
                for alvo in alvos {
                    let caminho = achar_modulo(dir, &alvo)?;
                    if c.pilha.contains(&caminho) {
                        return Err(Motivo::Estilos);
                    }
                    let texto = ler_modulo(&caminho, c)?;
                    c.pilha.push(caminho.clone());
                    let css = compilar_modulo(&texto, caminho.parent(), variaveis, c)?;
                    c.pilha.pop();
                    saida.push_str(&css);
                }
            }
        }
    }
    blocos(&corpo, "", variaveis, &mut saida, c.modo, true)?;
    Ok((saida, de_fora))
}

fn ler_modulo(caminho: &Path, c: &mut Compilacao) -> Result<String, Motivo> {
    if !c.lidos.iter().any(|p| p == caminho) {
        c.lidos.push(caminho.to_path_buf());
    }
    std::fs::read_to_string(caminho).map_err(|_| Motivo::Estilos)
}

/// Um `@use` ou um `@import` do começo do arquivo.
enum Diretiva {
    Use { alvo: String, estrela: bool },
    Import(Vec<String>),
}

/// O alvo entre aspas de uma diretiva (`'x'` ou `"x"`).
fn alvo_entre_aspas(s: &str) -> Result<String, Motivo> {
    let s = s.trim();
    let dentro = s
        .strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .or_else(|| s.strip_prefix('"').and_then(|x| x.strip_suffix('"')))
        .ok_or(Motivo::Estilos)?;
    if dentro.is_empty() || dentro.contains(['\'', '"', ':']) || dentro.ends_with(".css") {
        return Err(Motivo::Estilos);
    }
    Ok(dentro.to_string())
}

/// Tira os `@use`/`@import` do começo e devolve as diretivas, na ordem.
///
/// Recusa o que muda de sentido com a posição ou que não modelamos:
/// diretiva depois de outra coisa (regra, variável, `@import` aninhado numa
/// regra), `@use` com namespace nomeado ou `with`, `@use` e `@import` no
/// mesmo arquivo, CSS puro (`url(…)`, `.css`, esquema) e diretiva em mais de
/// uma linha.
fn separar_modulos(fonte: &str) -> Result<(Vec<Diretiva>, String), Motivo> {
    let mut diretivas = Vec::new();
    let mut corpo = String::with_capacity(fonte.len());
    let mut conteudo = false;
    let (mut usa, mut importa) = (false, false);
    for linha in fonte.lines() {
        let t = linha.trim();
        let (e_use, regra) = if let Some(r) = t.strip_prefix("@use ") {
            (true, r)
        } else if let Some(r) = t.strip_prefix("@import ") {
            (false, r)
        } else {
            if !t.is_empty() {
                conteudo = true;
            }
            corpo.push_str(linha);
            corpo.push('\n');
            continue;
        };
        if conteudo {
            return Err(Motivo::Estilos);
        }
        let regra = regra.trim().strip_suffix(';').ok_or(Motivo::Estilos)?.trim();
        if e_use {
            usa = true;
            let (alvo, apelido) = match regra.split_once(" as ") {
                Some((a, b)) => (a, Some(b.trim())),
                None => (regra, None),
            };
            if alvo.contains(" with") || alvo.contains(',') {
                return Err(Motivo::Estilos);
            }
            let estrela = match apelido {
                None => false,
                Some("*") => true,
                Some(_) => return Err(Motivo::Estilos),
            };
            diretivas.push(Diretiva::Use { alvo: alvo_entre_aspas(alvo)?, estrela });
        } else {
            importa = true;
            let alvos = regra.split(',').map(alvo_entre_aspas).collect::<Result<Vec<_>, _>>()?;
            diretivas.push(Diretiva::Import(alvos));
        }
    }
    if usa && importa {
        return Err(Motivo::Estilos);
    }
    Ok((diretivas, corpo))
}

/// Resolve um módulo como o `BuildImporter` do `sass_builder` 2.2.1: com
/// extensão, `_nome.ext` e `nome.ext`; sem, `_nome.sass`, `nome.sass`,
/// `_nome.scss` e `nome.scss`. Exatamente um candidato vale; mais de um é
/// erro no oficial ("It is not clear which file to import"). Nenhum, o
/// oficial tenta `nome/index` — não modelado. Sintaxe indentada (`.sass`)
/// não é suportada. Tudo o que não é um `.scss` único é recusa.
fn achar_modulo(dir: &Path, nome: &str) -> Result<PathBuf, Motivo> {
    let (base, extensoes): (&str, &[&str]) = match nome.rsplit_once('.') {
        Some((b, "scss")) => (b, &["scss"]),
        Some((b, "sass")) => (b, &["sass"]),
        _ => (nome, &["sass", "scss"]),
    };
    let rel = Path::new(base);
    let arquivo = rel.file_name().ok_or(Motivo::Estilos)?.to_string_lossy().to_string();
    let pai = rel.parent().map(|p| dir.join(p)).unwrap_or_else(|| dir.to_path_buf());
    let mut achados = Vec::new();
    for ext in extensoes {
        for candidato in [format!("_{arquivo}.{ext}"), format!("{arquivo}.{ext}")] {
            let c = pai.join(candidato);
            if c.is_file() {
                achados.push(c);
            }
        }
    }
    match achados.as_slice() {
        [um] if um.extension().is_some_and(|e| e == "scss") => Ok(um.clone()),
        _ => Err(Motivo::Estilos),
    }
}

/// O token como o `compressed` do dart-sass 1.102.0 o escreve: número
/// (`_writeNumber`), cor com nome ou hexadecimal (`_tryHexOrNamedRgb`: a forma
/// mais curta, o nome no empate), `transparent`. O que parece número ou cor
/// mas não sabemos escrever igual é recusa.
fn token_exato(t: &str) -> Result<String, Motivo> {
    if t == "transparent" {
        return Ok("rgba(0,0,0,0)".to_string());
    }
    if let Some(d) = t.strip_prefix('#') {
        if !d.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(t.to_string());
        }
        let rgb = match d.len() {
            3 => {
                let n = u32::from_str_radix(d, 16).map_err(|_| Motivo::Estilos)?;
                let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
                (r * 0x11) << 16 | (g * 0x11) << 8 | (b * 0x11)
            }
            6 => u32::from_str_radix(d, 16).map_err(|_| Motivo::Estilos)?,
            // Com alfa (`#rgba`, `#rrggbbaa`) sai `rgba(…)` com o alfa
            // arredondado: recusa.
            _ => return Err(Motivo::Estilos),
        };
        return Ok(cor_comprimida(rgb));
    }
    // O dart-sass reconhece o nome sem distinguir caixa (`White` é branco).
    if let Some(&(_, rgb)) = CORES_COM_NOME.iter().find(|(n, _)| n.eq_ignore_ascii_case(t)) {
        return Ok(cor_comprimida(rgb));
    }
    if t.eq_ignore_ascii_case("transparent") {
        return Err(Motivo::Estilos);
    }
    // Lista com barra (`12px/1.5`): cada parte.
    if t.contains('/') {
        return t.split('/').map(|p| numero_ou_texto(p, true)).collect::<Result<Vec<_>, _>>().map(|v| v.join("/"));
    }
    numero_ou_texto(t, true)
}

/// Número com unidade reescrito, ou o texto como veio se não começa como
/// número.
fn numero_ou_texto(t: &str, comprimido: bool) -> Result<String, Motivo> {
    let b = t.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    let comeca_numero = b.get(i).is_some_and(|c| c.is_ascii_digit())
        || (b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(|c| c.is_ascii_digit()));
    if !comeca_numero {
        return Ok(t.to_string());
    }
    while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
    }
    if b.get(i) == Some(&b'.') {
        if !b.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            return Err(Motivo::Estilos);
        }
        i += 1;
        while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
    }
    let (numero, unidade) = t.split_at(i);
    // Expoente (`1e3`) e unidade que não é identificador simples ou `%`:
    // recusa.
    let unidade_ok = unidade.is_empty()
        || unidade == "%"
        || (unidade.starts_with(|c: char| c.is_ascii_alphabetic())
            && unidade.chars().all(|c| c.is_ascii_alphanumeric())
            && !(unidade.starts_with(['e', 'E']) && unidade[1..].starts_with(|c: char| c.is_ascii_digit())));
    if !unidade_ok {
        return Err(Motivo::Estilos);
    }
    let v: f64 = numero.parse().map_err(|_| Motivo::Estilos)?;
    Ok(format!("{}{unidade}", numero_sass(v, comprimido)?))
}

/// `_writeNumber` + `_writeRounded` do dart-sass 1.102.0: inteiro exato sem
/// fração; senão a representação mais curta do `double` (`toString` do Dart
/// sem expoente); com 12 caracteres ou mais, arredondada em 10 casas por
/// texto (meio para cima) e sem zeros à direita. No `compressed`, o `0`
/// antes do ponto sai quando o texto começa por ele.
fn numero_sass(v: f64, comprimido: bool) -> Result<String, Motivo> {
    if !v.is_finite() || v.abs() >= 1e15 {
        return Err(Motivo::Estilos);
    }
    if v == v.round() {
        return Ok(format!("{}", v as i64));
    }
    // O `toString` do Dart usa expoente abaixo de 1e-6; o `_removeExponent`
    // o desfaz e dá o mesmo texto que o `Display` do Rust.
    let texto = format!("{v}");
    if texto.len() < 12 {
        return Ok(match texto.strip_prefix('0') {
            Some(r) if comprimido => r.to_string(),
            _ => texto,
        });
    }
    Ok(arredondado(&texto, comprimido))
}

/// `_writeRounded` do dart-sass.
fn arredondado(texto: &str, comprimido: bool) -> String {
    let negativo = texto.starts_with('-');
    let corpo = texto.trim_start_matches('-');
    let Some((inteira, fracao)) = corpo.split_once('.') else { return texto.to_string() };
    if fracao.len() <= 10 {
        return texto.to_string();
    }
    // Um dígito a mais à esquerda para o "vai um".
    let mut digitos: Vec<u8> = std::iter::once(0).chain(inteira.bytes().map(|c| c - b'0')).collect();
    let primeiro_fracionario = digitos.len();
    digitos.extend(fracao.bytes().take(10).map(|c| c - b'0'));
    let mut fim = digitos.len();
    if fracao.as_bytes()[10] - b'0' >= 5 {
        loop {
            digitos[fim - 1] += 1;
            if digitos[fim - 1] != 10 {
                break;
            }
            fim -= 1;
        }
    }
    while fim < primeiro_fracionario {
        digitos[fim] = 0;
        fim += 1;
    }
    while fim > primeiro_fracionario && digitos[fim - 1] == 0 {
        fim -= 1;
    }
    if fim == 2 && digitos[0] == 0 && digitos[1] == 0 {
        return "0".to_string();
    }
    let mut s = String::new();
    if negativo {
        s.push('-');
    }
    let mut i = 0;
    if digitos[0] == 0 {
        i += 1;
        if comprimido && digitos[1] == 0 {
            i += 1;
        }
    }
    while i < primeiro_fracionario {
        s.push((b'0' + digitos[i]) as char);
        i += 1;
    }
    if fim > primeiro_fracionario {
        s.push('.');
        while i < fim {
            s.push((b'0' + digitos[i]) as char);
            i += 1;
        }
    }
    s
}

/// Uma cor opaca como o `compressed` a escreve: o nome se não for mais
/// longo que o hexadecimal (o primeiro em ordem alfabética quando há dois,
/// `aqua` e não `cyan`), senão `#rgb` quando os pares se repetem, senão
/// `#rrggbb`.
fn cor_comprimida(rgb: u32) -> String {
    let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    let curto = [r, g, b].iter().all(|c| c >> 4 == c & 0xf);
    let limite = if curto { 4 } else { 7 };
    if let Some((nome, _)) = CORES_COM_NOME.iter().find(|(_, c)| *c == rgb)
        && nome.len() <= limite
    {
        return (*nome).to_string();
    }
    if curto {
        format!("#{:x}{:x}{:x}", r & 0xf, g & 0xf, b & 0xf)
    } else {
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}
/// Normalizações de valor que o Sass faz na saída e que o CSS escrito à mão
/// não teria: sem elas a comparação com o `sass_builder` acusa diferença onde
/// o navegador renderiza igual.
///
/// As quatro foram achadas comparando os 144 `.scss` do new_sali com a saída
/// dele (`--example conferir-sass`):
///
/// | escrito | Sass emite |
/// |---|---|
/// | `0.5rem` | `.5rem` |
/// | `white` | `#fff` |
/// | `transparent` | `rgba(0,0,0,0)` |
/// | `"\e9fe"` | o caractere U+E9FE |
fn normalizar(decls: &str, modo: Modo) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(decls.len());
    let mut resto = decls;
    // Percorre declaração a declaração para não mexer no nome da propriedade.
    while !resto.is_empty() {
        let fim = resto.find(';').map(|i| i + 1).unwrap_or(resto.len());
        let (decl, r) = resto.split_at(fim);
        resto = r;
        match decl.split_once(':') {
            // Propriedade customizada guarda o valor como foi escrito.
            Some((prop, valor)) if !prop.trim().starts_with("--") => {
                saida.push_str(prop);
                saida.push(':');
                saida.push_str(&valor_normalizado(valor, modo)?);
            }
            _ => saida.push_str(decl),
        }
    }
    Ok(saida)
}

/// Normaliza os tokens de um valor como o Sass comprimido os escreve,
/// preservando strings.
///
/// Uma chamada de função tem a regra do tipo dela, vista comparando a saída
/// do `sass_builder` no new_sali (`--example conferir-sass`):
/// - `var()`, `url()`, `env()` e função com `var()` dentro são especiais: o
///   texto como veio (`rgba(var(--x), 0.14)`);
/// - `calc`/`min`/`max`/`clamp` são cálculos: número sem o zero à esquerda,
///   vírgula sem espaço (`clamp(3rem,12vmin,8rem)`); o que o Sass
///   simplificaria (um termo só, `*`, `/`) é recusado;
/// - `rgba()`/`rgb()`/`hsl()`/`hsla()` o Sass avalia e escreve na forma
///   mais curta (`hsla(0,0%,100%,.2)` para `rgba(255, 255, 255, 0.2)`):
///   recusa;
/// - função CSS comum sai como veio (`scaleX(0.75)`), a não ser que traga
///   outra função dentro.
fn valor_normalizado(valor: &str, modo: Modo) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(valor.len());
    let mut resto = valor;
    while !resto.is_empty() {
        let c = resto.chars().next().unwrap();
        if c == '"' || c == '\'' {
            // String: os escapes viram o caractere, e as aspas simples viram
            // duplas (o Sass escreve a string com aspas duplas quando o
            // conteúdo não tem uma: `content: '✓'` sai `"✓"`). Aspas ou
            // barra dentro da simples têm regra própria: recusa.
            let fim = fim_da_string(resto, c);
            // O `expanded` escreve de volta o escape de caractere de uso
            // privado (`"\e9fe"`); a regra geral não modelamos.
            if modo == Modo::Expandido && resto[..fim].contains('\\') {
                return Err(Motivo::Estilos);
            }
            let texto = decodificar_escapes(&resto[..fim]);
            if c == '\'' {
                let dentro = texto
                    .strip_prefix('\'')
                    .and_then(|t| t.strip_suffix('\''))
                    .ok_or(Motivo::Estilos)?;
                if dentro.contains(['"', '\\', '\'']) {
                    return Err(Motivo::Estilos);
                }
                saida.push('"');
                saida.push_str(dentro);
                saida.push('"');
            } else {
                saida.push_str(&texto);
            }
            resto = &resto[fim..];
            continue;
        }
        let fim = resto
            .char_indices()
            .find(|(_, x)| {
                *x == '"'
                    || *x == '\''
                    || x.is_whitespace()
                    || *x == ','
                    || *x == '('
                    || *x == ')'
                    || *x == ';'
                    || *x == '!'
            })
            .map(|(i, _)| i)
            .unwrap_or(resto.len());
        // Nome de função: a chamada inteira, parênteses balanceados.
        if fim > 0 && resto[fim..].starts_with('(') {
            let nome = &resto[..fim];
            let fecha = fim_dos_parenteses(resto, fim).ok_or(Motivo::Estilos)?;
            let dentro = &resto[fim + 1..fecha];
            saida.push_str(&chamada_normalizada(nome, dentro, modo)?);
            resto = &resto[fecha + 1..];
            continue;
        }
        if c == '(' || c == ')' {
            // Parêntese solto (agrupamento): fora do subconjunto.
            return Err(Motivo::Estilos);
        }
        let token = if fim == 0 {
            &resto[..c.len_utf8()]
        } else {
            &resto[..fim]
        };
        saida.push_str(&token_normalizado(token, modo)?);
        resto = &resto[token.len()..];
    }
    Ok(saida)
}

/// Índice do `)` que fecha o `(` em `abre`.
fn fim_dos_parenteses(texto: &str, abre: usize) -> Option<usize> {
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    for (i, c) in texto[abre..].char_indices() {
        if let Some(q) = aspas {
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(abre + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Uma chamada `nome(dentro)` como o Sass comprimido a escreve.
fn chamada_normalizada(nome: &str, dentro: &str, modo: Modo) -> Result<String, Motivo> {
    let n = nome.to_ascii_lowercase();
    let especial = matches!(n.as_str(), "var" | "url" | "env") || dentro.contains("var(");
    if especial {
        return Ok(format!("{nome}({dentro})"));
    }
    if matches!(n.as_str(), "calc" | "min" | "max" | "clamp") {
        // No `expanded`, só `calc` sem vírgula tem caso conferido.
        if modo == Modo::Expandido && (n != "calc" || dentro.contains(',')) {
            return Err(Motivo::Estilos);
        }
        return Ok(format!("{nome}({})", calculo(dentro, modo)?));
    }
    // Cor com alfa: o Sass escolhe a forma mais curta (`rgba(0,0,0,.1)`,
    // mas `hsla(0,0%,100%,.2)` para o branco), regra ainda não modelada.
    if matches!(n.as_str(), "rgba" | "rgb" | "hsl" | "hsla") {
        return Err(Motivo::Estilos);
    }
    // Função CSS comum: os argumentos são avaliados e escritos no estilo
    // expandido dentro dela (`scaleX(0.75)`, `transparent` ficam); função
    // aninhada seria reescrita (`rgb(255 255 255 / 41%)`): recusa.
    if dentro.contains('(') {
        return Err(Motivo::Estilos);
    }
    if modo != Modo::Shim {
        return Ok(format!("{nome}({})", argumentos_expandidos(dentro)?));
    }
    Ok(format!("{nome}({dentro})"))
}

/// Os argumentos de uma função CSS comum como o dart-sass os escreve: cada
/// um avaliado e convertido pelo `toCssString` (o estilo expandido, mesmo no
/// `compressed`: `translate(1.5px, 0.5em)`, cor como foi escrita), separados
/// por `, `. String ou função dentro: recusa.
fn argumentos_expandidos(dentro: &str) -> Result<String, Motivo> {
    if dentro.contains(['"', '\'', '(', ')']) {
        return Err(Motivo::Estilos);
    }
    let mut args = Vec::new();
    for arg in dentro.split(',') {
        let mut termos = Vec::new();
        for t in arg.split_whitespace() {
            termos.push(numero_ou_texto(t, false)?);
        }
        if termos.is_empty() {
            return Err(Motivo::Estilos);
        }
        args.push(termos.join(" "));
    }
    Ok(args.join(", "))
}

/// O conteúdo de um cálculo (`calc`, `min`, `max`, `clamp`): números sem o
/// zero à esquerda, vírgula sem espaço, `+`/`-` com espaço. O que o Sass
/// simplificaria é recusado.
fn calculo(dentro: &str, modo: Modo) -> Result<String, Motivo> {
    if dentro.contains(['*', '/']) {
        return Err(Motivo::Estilos);
    }
    let mut partes = Vec::new();
    for arg in dentro.split(',') {
        let termos: Vec<&str> = arg.split_whitespace().collect();
        // Um termo só (`calc(1.3rem)`) o Sass reduz ao número.
        if termos.len() == 1 && !termos[0].contains('(') && n_de_argumentos(dentro) == 1 {
            return Err(Motivo::Estilos);
        }
        let mut saida = Vec::new();
        let mut unidades = Vec::new();
        for t in &termos {
            if t.contains('(') || t.contains(')') {
                // `env(...)`, função dentro do cálculo: só as especiais.
                if !(t.starts_with("env(") || t.starts_with("var(")) {
                    return Err(Motivo::Estilos);
                }
                saida.push(t.to_string());
                continue;
            }
            if matches!(*t, "+" | "-") {
                saida.push(t.to_string());
                continue;
            }
            let unidade: String = t
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-')
                .to_string();
            unidades.push(unidade);
            saida.push(token_normalizado(t, modo)?);
        }
        // Dois números da mesma unidade o Sass soma: recusa.
        let mut vistas = std::collections::HashSet::new();
        if !unidades.iter().all(|u| vistas.insert(u.clone())) {
            return Err(Motivo::Estilos);
        }
        partes.push(saida.join(" "));
    }
    Ok(partes.join(","))
}

fn n_de_argumentos(dentro: &str) -> usize {
    dentro.split(',').count()
}

/// Fim da string literal aberta em 0 (índice depois da aspa de fechamento).
fn fim_da_string(texto: &str, aspa: char) -> usize {
    let mut escape = false;
    for (i, c) in texto.char_indices().skip(1) {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == aspa {
            return i + c.len_utf8();
        }
    }
    texto.len()
}

/// `\e9fe` vira o caractere, como o Sass faz ao normalizar a string.
fn decodificar_escapes(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut saida = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            saida.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 && chars.peek().is_some_and(|x| x.is_ascii_hexdigit()) {
            hex.push(chars.next().unwrap_or('0'));
        }
        if hex.is_empty() {
            saida.push('\\');
            continue;
        }
        // Um espaço depois do escape é o terminador e não faz parte do valor.
        if chars.peek() == Some(&' ') {
            chars.next();
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            Some(x) => saida.push(x),
            None => {
                saida.push('\\');
                saida.push_str(&hex);
            }
        }
    }
    saida
}

/// Um token isolado: número com zero à esquerda e nome de cor.
fn token_normalizado(t: &str, modo: Modo) -> Result<String, Motivo> {
    match modo {
        Modo::Comprimido => token_exato(t),
        Modo::Expandido => token_expandido(t),
        Modo::Shim => Ok(token_do_shim(t)),
    }
}

/// As normalizações que o shim do ngdart usa (as do `compressed` que os
/// `.scss` do new_sali confirmaram).
fn token_do_shim(t: &str) -> String {
    if t == "transparent" {
        return "rgba(0,0,0,0)".to_string();
    }
    if let Some(hex) = cor_em_hex(t) {
        return hex.to_string();
    }
    // `#ffffff` -> `#fff`, quando os três pares se repetem.
    if let Some(d) = t.strip_prefix('#')
        && d.len() == 6
        && d.chars().all(|c| c.is_ascii_hexdigit())
    {
        let b = d.as_bytes();
        if b[0].eq_ignore_ascii_case(&b[1])
            && b[2].eq_ignore_ascii_case(&b[3])
            && b[4].eq_ignore_ascii_case(&b[5])
        {
            return format!(
                "#{}{}{}",
                d.as_bytes()[0] as char,
                d.as_bytes()[2] as char,
                d.as_bytes()[4] as char
            )
            .to_lowercase();
        }
    }
    // `0.5rem` -> `.5rem`, `-0.5rem` -> `-.5rem`.
    if let Some(r) = t.strip_prefix("0.")
        && r.starts_with(|c: char| c.is_ascii_digit())
    {
        return format!(".{r}");
    }
    if let Some(r) = t.strip_prefix("-0.")
        && r.starts_with(|c: char| c.is_ascii_digit())
    {
        return format!("-.{r}");
    }
    t.to_string()
}

/// As cores com nome (`colorsByName` do dart-sass 1.102.0,
/// `lib/src/color_names.dart`, gerada dele), em ordem alfabética, com o valor
/// RGB. `transparent` fica de fora: tem alfa e regra própria.
const CORES_COM_NOME: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff), ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff), ("beige", 0xf5f5dc), ("bisque", 0xffe4c4), ("black", 0x000000),
    ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff), ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00), ("chocolate", 0xd2691e),
    ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c),
    ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f), ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f), ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff), ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc), ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700),
    ("goldenrod", 0xdaa520), ("gray", 0x808080), ("green", 0x008000), ("greenyellow", 0xadff2f),
    ("grey", 0x808080), ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5), ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1), ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000), ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3), ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee), ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc), ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1), ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead), ("navy", 0x000080), ("oldlace", 0xfdf5e6), ("olive", 0x808000),
    ("olivedrab", 0x6b8e23), ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee), ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9), ("peru", 0xcd853f), ("pink", 0xffc0cb),
    ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6), ("purple", 0x800080), ("rebeccapurple", 0x663399),
    ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1), ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072), ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57), ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d), ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb), ("slateblue", 0x6a5acd),
    ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa), ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4), ("tan", 0xd2b48c), ("teal", 0x008080), ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347), ("turquoise", 0x40e0d0), ("violet", 0xee82ee), ("wheat", 0xf5deb3),
    ("white", 0xffffff), ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00), ("yellowgreen", 0x9acd32),
];

/// Nomes de cor que o Sass troca por hexadecimal por ser mais curto. A lista
/// é só dos casos em que o hexadecimal ganha; `red` continua `red` porque
/// `#f00` é maior.
fn cor_em_hex(nome: &str) -> Option<&'static str> {
    Some(match nome {
        "white" => "#fff",
        "black" => "#000",
        "aqua" => "#0ff",
        "blue" => "#00f",
        "fuchsia" => "#f0f",
        "lime" => "#0f0",
        "yellow" => "#ff0",
        "cyan" => "#0ff",
        "magenta" => "#f0f",
        "darkgray" => "#a9a9a9",
        "darkgrey" => "#a9a9a9",
        "lightgray" => "#d3d3d3",
        "lightgrey" => "#d3d3d3",
        _ => return None,
    })
}

/// Tira `//` até o fim da linha, sem confundir com `://` de URL.
fn tirar_comentarios(fonte: &str) -> String {
    let mut saida = String::with_capacity(fonte.len());
    for linha in fonte.lines() {
        let mut corte = None;
        let b = linha.as_bytes();
        for i in 0..b.len().saturating_sub(1) {
            if b[i] == b'/' && b[i + 1] == b'/' && (i == 0 || b[i - 1] != b':') {
                corte = Some(i);
                break;
            }
        }
        match corte {
            Some(i) => saida.push_str(&linha[..i]),
            None => saida.push_str(linha),
        }
        saida.push('\n');
    }
    saida
}

/// Percorre os blocos de um nível, achatando o aninhamento.
fn blocos(
    fonte: &str,
    pai: &str,
    variaveis: &mut HashMap<String, String>,
    saida: &mut String,
    modo: Modo,
    raiz: bool,
) -> Result<(), Motivo> {
    let mut resto = fonte.trim();
    while !resto.is_empty() {
        // Declaração ou variável soltas neste nível.
        let proximo_abre = resto.find('{');
        let proximo_ponto = resto.find(';');
        if let Some(pv) = proximo_ponto
            && proximo_abre.is_none_or(|a| pv < a)
        {
            let decl = resto[..pv].trim().to_string();
            resto = resto[pv + 1..].trim_start();
            if let Some((nome, valor)) = decl.strip_prefix('$').and_then(|d| d.split_once(':')) {
                if valor.contains("!default") {
                    return Err(Motivo::Estilos);
                }
                let valor = substituir(valor.trim(), variaveis)?;
                variaveis.insert(nome.trim().to_string(), valor);
                continue;
            }
            // Declaração fora de regra não existe em CSS.
            if pai.is_empty() {
                return Err(Motivo::Estilos);
            }
            return Err(Motivo::Estilos);
        }
        let Some(abre) = proximo_abre else {
            if resto.trim().is_empty() {
                break;
            }
            return Err(Motivo::Estilos);
        };
        let cabeca = resto[..abre].trim().to_string();
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        resto = resto[fim + 1..].trim_start();

        if cabeca.starts_with('@') {
            // `@media` aninha regras e é o único que aparece nos projetos.
            if !cabeca.starts_with("@media") {
                return Err(Motivo::Estilos);
            }
            saida.push_str(&cabeca);
            saida.push('{');
            // O bloco é um escopo: variável declarada nele não vaza.
            let mut locais = variaveis.clone();
            if !pai.is_empty() && modo != Modo::Shim {
                // `@media` dentro de regra: as declarações dele saem numa
                // regra com o seletor de fora (`@media(…){.a{width:100%}}`).
                corpo_de_regra(corpo, pai, &mut locais, saida, modo)?;
            } else {
                blocos(corpo, pai, &mut locais, saida, modo, false)?;
            }
            saida.push('}');
            continue;
        }
        if cabeca.contains("#{") {
            return Err(Motivo::Estilos); // interpolação de seletor
        }
        let seletor = if modo == Modo::Expandido {
            atributos_sem_aspas(&juntar_expandido(pai, &cabeca)?)?
        } else {
            atributos_sem_aspas(&juntar(pai, &cabeca))?
        };
        corpo_de_regra(corpo, &seletor, variaveis, saida, modo)?;
        if raiz {
            saida.push(GRUPO);
        }
    }
    Ok(())
}

/// O corpo de uma regra (ou de um `@media` dentro dela): as declarações com
/// o seletor, depois as regras aninhadas.
fn corpo_de_regra(
    corpo: &str,
    seletor: &str,
    variaveis: &mut HashMap<String, String>,
    saida: &mut String,
    modo: Modo,
) -> Result<(), Motivo> {
    {
        // Declarações deste nível saem antes das regras aninhadas, como o
        // Sass emite.
        let (decls, aninhados) = separar(corpo)?;
        // Escopo do bloco: a variável declarada aqui vale daqui em diante, no
        // bloco e nos aninhados, e não vaza para fora (`!global` e
        // `!default` ficam de fora do subconjunto).
        let mut locais = variaveis.clone();
        let mut proprias = String::with_capacity(decls.len());
        for d in instrucoes(&decls) {
            let d = d.trim();
            if d.is_empty() {
                continue;
            }
            if let Some((nome, valor)) = d.strip_prefix('$').and_then(|x| x.split_once(':')) {
                if valor.contains("!default") || valor.contains("!global") {
                    return Err(Motivo::Estilos);
                }
                let valor = substituir(valor.trim(), &locais)?;
                locais.insert(nome.trim().to_string(), valor);
                continue;
            }
            proprias.push_str(&substituir(d, &locais)?);
            proprias.push(';');
        }
        if tem_funcao_de_cor(&proprias) {
            return Err(Motivo::Estilos);
        }
        if !proprias.is_empty() {
            saida.push_str(seletor);
            saida.push('{');
            saida.push_str(&normalizar(&proprias, modo)?);
            saida.push('}');
        }
        blocos(&aninhados, seletor, &mut locais, saida, modo, false)?;
    }
    Ok(())
}

/// Separa as declarações do nível das regras aninhadas.
fn separar(corpo: &str) -> Result<(String, String), Motivo> {
    let mut decls = String::new();
    let mut aninhados = String::new();
    let mut resto = corpo;
    loop {
        let abre = resto.find('{');
        let Some(abre) = abre else {
            if !aninhados.is_empty() && tem_declaracao(resto) {
                return Err(Motivo::Estilos);
            }
            decls.push_str(resto);
            break;
        };
        // Tudo até o `;` anterior ao `{` é declaração; o resto é o cabeçalho
        // da regra aninhada.
        let corte = resto[..abre].rfind(';').map(|i| i + 1).unwrap_or(0);
        // Declaração depois de uma regra aninhada: o dart-sass 1.102 a emite
        // numa regra própria, depois da aninhada (`.a .b{…}.a{left:0}`); nós
        // juntamos as declarações antes. Recusa.
        if !aninhados.is_empty() && tem_declaracao(&resto[..corte]) {
            return Err(Motivo::Estilos);
        }
        decls.push_str(&resto[..corte]);
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        aninhados.push_str(&resto[corte..=fim]);
        aninhados.push('\n');
        resto = &resto[fim + 1..];
    }
    Ok((decls, aninhados))
}

/// Alguma instrução (declaração ou variável) no trecho? Depois de uma regra
/// aninhada, a ordem importa: a declaração sai numa regra própria depois
/// dela, e a variável não vale para a regra que veio antes.
fn tem_declaracao(trecho: &str) -> bool {
    trecho.split(';').any(|d| !d.trim().is_empty())
}

/// As instruções de um corpo, separadas por `;` fora de aspas e parênteses.
fn instrucoes(corpo: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut atual = String::new();
    let mut nivel = 0usize;
    let mut aspas: Option<char> = None;
    for c in corpo.chars() {
        if let Some(q) = aspas {
            atual.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => aspas = Some(c),
            '(' => nivel += 1,
            ')' => nivel = nivel.saturating_sub(1),
            ';' if nivel == 0 => {
                v.push(std::mem::take(&mut atual));
                continue;
            }
            _ => {}
        }
        atual.push(c);
    }
    v.push(atual);
    v
}

/// `[a='b']` como o Sass escreve: o valor que é identificador sai sem aspas
/// (`[data-color-theme=dark]`), o resto entre aspas duplas. Valor com aspas
/// dentro fica de fora.
fn atributos_sem_aspas(seletor: &str) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(seletor.len());
    let mut resto = seletor;
    while let Some(i) = resto.find('=') {
        let depois = &resto[i + 1..];
        let aspa = depois.chars().next();
        match aspa {
            Some(q @ ('"' | '\'')) => {
                let Some(f) = depois[1..].find(q) else {
                    return Err(Motivo::Estilos);
                };
                let valor = &depois[1..1 + f];
                saida.push_str(&resto[..=i]);
                if e_identificador(valor) {
                    saida.push_str(valor);
                } else if valor.contains('"') {
                    return Err(Motivo::Estilos);
                } else {
                    saida.push('"');
                    saida.push_str(valor);
                    saida.push('"');
                }
                resto = &depois[f + 2..];
            }
            _ => {
                saida.push_str(&resto[..=i]);
                resto = depois;
            }
        }
    }
    saida.push_str(resto);
    Ok(saida)
}

/// Identificador CSS simples: letra, `_` ou `-` seguido de letra, e o resto
/// letras, dígitos, `_` e `-`.
fn e_identificador(v: &str) -> bool {
    let mut c = v.chars();
    let primeiro = match c.next() {
        Some('-') => c.next(),
        x => x,
    };
    primeiro.is_some_and(|p| p.is_ascii_alphabetic() || p == '_')
        && v.chars()
            .all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// Junta o seletor do pai com o do filho, resolvendo `&`.
fn juntar(pai: &str, filho: &str) -> String {
    if pai.is_empty() {
        return filho.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    // O pai de fora, o filho de dentro: `.a,.b { .c,.d {} }` dá
    // `.a .c,.a .d,.b .c,.b .d`, como o `@extend`/aninhamento do dart-sass.
    let mut partes = Vec::new();
    for p in pai.split(',') {
        let p = p.trim();
        for f in filho.split(',') {
            let f = f.trim();
            partes.push(if f.contains('&') {
                f.replace('&', p)
            } else {
                format!("{p} {f}")
            });
        }
    }
    partes.join(",")
}

/// `rgb(…)`/`hsl(…)` o Sass avalia e escreve como cor (`rgb(47, 88, 141)`
/// sai `#2f588d`, visto no `visualiza_norma_page` do new_sali). Não
/// avaliamos funções: recusa.
fn tem_funcao_de_cor(texto: &str) -> bool {
    let t = texto.to_ascii_lowercase();
    ["rgb(", "hsl(", "hsla("].iter().any(|f| {
        t.match_indices(f).any(|(i, _)| {
            i == 0 || !t.as_bytes()[i - 1].is_ascii_alphanumeric() && t.as_bytes()[i - 1] != b'-'
        })
    })
}

/// Troca `$nome` pelo valor. Variável desconhecida é recusa, não texto vazio.
fn substituir(texto: &str, variaveis: &HashMap<String, String>) -> Result<String, Motivo> {
    if !texto.contains('$') {
        return Ok(texto.to_string());
    }
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('$') {
        saida.push_str(&resto[..i]);
        let nome: String = resto[i + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if nome.is_empty() {
            return Err(Motivo::Estilos);
        }
        let Some(v) = variaveis.get(&nome) else {
            return Err(Motivo::Estilos);
        };
        saida.push_str(v);
        resto = &resto[i + 1 + nome.len()..];
    }
    saida.push_str(resto);
    Ok(saida)
}

/// Índice da chave que fecha o bloco aberto em `ini`.
fn fim_do_bloco(texto: &str, ini: usize) -> Option<usize> {
    let mut nivel = 1usize;
    for (i, c) in texto[ini..].char_indices() {
        match c {
            '{' => nivel += 1,
            '}' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(ini + i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    /// As regras do `compressed` que os 114 `.css` do new_sali confirmam:
    /// combinador sem espaço, lista sem espaço depois da vírgula, `!important`
    /// com espaço, propriedade customizada como veio, `@media(`, comentário
    /// de bloco fora, e o `\n` final.
    #[test]
    fn comprimido_como_o_sass_builder() {
        let fonte = ".a > .b, .c {\n  /* x */\n  color: red !important;\n  --y: 1rem;\n  font-family: \"Inter\", system-ui;\n}\n@media (max-width: 767.98px) {\n  .d { margin: 0; }\n}\n";
        let (css, modulos) = compilar_com(fonte, None, Estilo::Comprimido).unwrap();
        assert_eq!(
            css,
            ".a>.b,.c{color:red !important;--y: 1rem;font-family:\"Inter\",system-ui}@media(max-width: 767.98px){.d{margin:0}}\n"
        );
        assert!(modulos.is_empty());
        assert_eq!(compilar_com("", None, Estilo::Comprimido).unwrap().0, "\n");
        assert!(compilar_com(":host{--x: $y}", None, Estilo::Comprimido).is_err());
        assert_eq!(
            compilar_com(".a{color:red}", None, Estilo::Expandido).unwrap().0,
            ".a {\n  color: red;\n}\n"
        );
    }

    /// O espaço do CSS intermediário não importa — quem normaliza é o shim.
    /// O que este teste garante é que `//` some e que `://` de URL fica.
    #[test]
    fn comentario_de_linha_some_e_url_fica() {
        let fonte = "// fora
.a {
  background: url(http://x/y.png); // atrás
}
";
        let shim = crate::css::shim(&compilar(fonte).unwrap()).unwrap();
        assert_eq!(
            shim,
            ".a._ngcontent-%ID%{background:url(\"http://x/y.png\")}"
        );
    }

    #[test]
    fn aninhamento_e_e_comercial() {
        let fonte = ".a {
  color: red;
  &:hover { color: blue; }
  .b { color: green; }
}
";
        let shim = crate::css::shim(&compilar(fonte).unwrap()).unwrap();
        assert_eq!(
            shim,
            ".a._ngcontent-%ID%{color:red}.a:hover._ngcontent-%ID%{color:#00f}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:green}"
        );
    }

    #[test]
    fn variavel() {
        let css = compilar("$c: red;\n.a { color: $c; }\n").unwrap();
        assert!(css.contains("color: red"), "{css}");
    }

    /// A folha global do new_sali (`web/style.scss`) começa com quatro
    /// `@use 'x' as *`. O CSS do módulo sai antes, e as variáveis dele valem
    /// no arquivo que o usa.
    #[test]
    fn use_carrega_o_modulo() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(
            dir.path().join("tema.scss"),
            "$c: red;\n.tema { color: $c; }\n",
        )
        .expect("escreve");
        let css = compilar_em("@use 'tema' as *;\n.a { color: $c; }\n", Some(dir.path()))
            .expect("compila");
        let shim = crate::css::shim(&css).unwrap();
        assert_eq!(
            shim,
            ".tema._ngcontent-%ID%{color:red}.a._ngcontent-%ID%{color:red}"
        );
    }

    /// As formas que o `conferir-sass` achou diferentes do `sass_builder`
    /// no new_sali, com a saída dele.
    #[test]
    fn funcoes_como_o_sass_comprimido() {
        let shim = |f: &str| crate::css::shim(&compilar(f).unwrap()).unwrap();
        assert!(compilar(".a { box-shadow: 0 0.5rem 1rem rgba(0, 0, 0, 0.1); }").is_err());
        assert_eq!(
            shim(".a { transform: translate(25%) scaleX(0.75); margin: 0.5rem; }"),
            ".a._ngcontent-%ID%{transform:translate(25%) scaleX(0.75);margin:.5rem}"
        );
        assert_eq!(
            shim(".a { font-size: clamp(3rem, 12vmin, 8rem); }"),
            ".a._ngcontent-%ID%{font-size:clamp(3rem,12vmin,8rem)}"
        );
        assert_eq!(
            shim(".a { padding: 0.65rem calc(0.75rem + env(safe-area-inset-bottom)); }"),
            ".a._ngcontent-%ID%{padding:.65rem calc(.75rem + env(safe-area-inset-bottom))}"
        );
        assert_eq!(
            shim(".a[data-t='dark'] { color: red; }"),
            ".a[data-t=dark]._ngcontent-%ID%{color:red}"
        );
        assert_eq!(
            shim(".a::after { content: '✓'; }"),
            ".a._ngcontent-%ID%::after{content:\"✓\"}"
        );
        assert!(compilar(".a { width: calc(1.3rem); }").is_err());
        assert!(compilar(".a { width: calc(1rem + 2rem); }").is_err());
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        assert!(compilar("@mixin x { color: red; }").is_err());
        assert!(compilar_em("@use 'sass:math';", None).is_err());
        // Namespace explícito não sabemos traduzir.
        assert!(compilar_em("@use 'tema' as t;", None).is_err());
        assert!(compilar(".a { color: $indefinida; }").is_err());
        assert!(compilar(".#{$x} { color: red; }").is_err());
        // Função de cor o Sass avalia (`rgb(47, 88, 141)` vira `#2f588d`).
        assert!(compilar(":host { background: rgb(47, 88, 141); }").is_err());
        assert!(compilar("$c: hsl(0, 0%, 0%);\n.a { color: $c; }").is_err());
    }

    /// O caso real do `arvore_organograma.scss`, cujo shim oficial é
    /// `._nghost-%ID%{position:relative}.fancytree-container._ngcontent-%ID%{overflow:unset}`.
    #[test]
    fn caso_real_do_new_sali() {
        let fonte = ":host {\n  position: relative;\n}\n\n.fancytree-container {\n  //     --ft-node-padding-y: 0.25rem;\n  overflow: unset;\n}\n";
        let css = compilar(fonte).unwrap();
        let shim = crate::css::shim(&css).unwrap();
        assert_eq!(
            shim,
            "._nghost-%ID%{position:relative}.fancytree-container._ngcontent-%ID%{overflow:unset}"
        );
    }
}
