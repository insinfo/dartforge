//! Recarga proporcional à edição (J04): a geração nova compila só as funções
//! que mudaram.
//!
//! O emissor escreve o programa inteiro a cada recarga, e analisar e ligar o
//! IR inteiro é o grosso do custo dela num projeto grande (200 classes:
//! 12 MB de IR, 0,55 s de análise e 2,0 s de ligação por edição de um corpo).
//! Mas toda chamada entre funções do programa passa pela entrada estável
//! (`version_definitions` troca os usos pelo trampolim), então uma função
//! cujo código é o mesmo da implementação publicada pode continuar nela: a
//! geração nova a **declara**, e quem a chama cai no trampolim, que aponta
//! para a implementação viva.
//!
//! "O mesmo código" é textual e conservador. A impressão digital de uma
//! função é o texto dela mais o texto de tudo o que ela alcança e que é da
//! própria geração: funções locais (`internal`/`private`, sem trampolim) e
//! globais do módulo, transitivamente. Uma função do programa alcançada entra
//! só pelo nome (a chamada vai ao trampolim, que publica a versão vigente).
//! O resto do contexto que o texto não carrega é estável por construção: ids
//! de classe (J03), índices da área de globais (só cresce), deslocamentos de
//! campo (no texto) e o runtime.
//!
//! Não é mantida (é compilada de novo):
//! * a função que alcança uma global **mutável** do módulo (o cache de um
//!   ponto de chamada por seletor): a geração antiga guarda nele endereços de
//!   implementações antigas;
//! * a função que alcança uma função do programa cuja assinatura no IR novo
//!   não é a da entrada publicada (J03: o nome passa a ser outra entrada);
//! * as funções de registro da geração (área, tabelas, RTI), que a
//!   publicação executa pela geração nova;
//! * a que não tem implementação publicada com a mesma impressão digital.
//!
//! O IR que sai tem as mantidas como `declare`, sem as funções e globais
//! locais que só elas alcançavam; qualquer coisa fora da forma que o emissor
//! escreve faz o IR seguir inteiro, sem delta.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// O que se sabe da implementação publicada de uma entrada estável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Published {
    /// Impressão digital do código (texto e fecho local).
    pub fingerprint: u64,
    /// A assinatura como o IR a escreve, sem nomes de parâmetro.
    pub fn_signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Function { local: bool },
    Global { mutable: bool },
    Comdat,
}

#[derive(Debug)]
struct Item<'a> {
    name: String,
    tipo: Type,
    text: &'a str,
}

/// O módulo lido pelas linhas de topo: definições de função, globais e
/// comdats; o resto (cabeçalho, comentários, `declare`) passa como está.
struct IrModule<'a> {
    /// Trechos na ordem do texto: `Ok(item)` ou `Err(text que passa)`.
    fragments: Vec<Result<usize, &'a str>>,
    items: Vec<Item<'a>>,
    by_name: HashMap<String, usize>,
}

/// O nome depois do `@` (ou `$` de comdat) no começo de `s`: entre aspas, ou
/// até o primeiro caractere que não compõe identificador do LLVM.
fn name_at(s: &str) -> Option<(String, usize)> {
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some((rest[..end].to_string(), end + 2));
    }
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
        .map_or(s.len(), |(i, _)| i);
    (end > 0).then(|| (s[..end].to_string(), end))
}

/// As referências `@name` de um texto.
fn references(text: &str) -> impl Iterator<Item = String> + '_ {
    text.match_indices('@').filter_map(|(i, _)| name_at(&text[i + 1..]).map(|(n, _)| n))
}

fn do_read(ir: &str) -> Option<IrModule<'_>> {
    let mut m = IrModule { fragments: Vec::new(), items: Vec::new(), by_name: HashMap::new() };
    let mut pos = 0;
    let mut start_pass = 0;
    while pos < ir.len() {
        let line_end = ir[pos..].find('\n').map_or(ir.len(), |i| pos + i + 1);
        let line = &ir[pos..line_end];
        let item = if let Some(rest) = line.strip_prefix("define ") {
            // Até a linha `}` que fecha o corpo.
            let end = ir[pos..].find("\n}\n").map(|i| pos + i + 3)?;
            let at = rest.find('@')?;
            let (name, _) = name_at(&rest[at + 1..])?;
            let before = &rest[..at];
            let local = before.split_whitespace().any(|p| p == "internal" || p == "private");
            Some((name, Type::Function { local }, end))
        } else if let Some(rest) = line.strip_prefix('@') {
            let (name, n) = name_at(rest)?;
            let after = rest[n..].strip_prefix(" = ")?;
            // Uma global por linha; o `declare` de dado externo é `external global`.
            if after.starts_with("external ") {
                None
            } else {
                let mutable = after.split_whitespace().find(|p| *p == "constant" || *p == "global") == Some("global");
                Some((name, Type::Global { mutable }, line_end))
            }
        } else if let Some(rest) = line.strip_prefix('$') {
            let (name, n) = name_at(rest)?;
            rest[n..].starts_with(" = comdat ").then_some((name, Type::Comdat, line_end))
        } else {
            None
        };
        match item {
            Some((name, tipo, end)) => {
                if start_pass < pos {
                    m.fragments.push(Err(&ir[start_pass..pos]));
                }
                let key = match tipo {
                    Type::Comdat => format!("${name}"),
                    _ => name.clone(),
                };
                if m.by_name.insert(key, m.items.len()).is_some() {
                    return None;
                }
                m.fragments.push(Ok(m.items.len()));
                m.items.push(Item { name, tipo, text: &ir[pos..end] });
                pos = end;
                start_pass = pos;
            }
            None => pos = line_end,
        }
    }
    if start_pass < ir.len() {
        m.fragments.push(Err(&ir[start_pass..]));
    }
    Some(m)
}

/// A assinatura de uma definição: a linha `define` sem ligação, sem nomes de
/// parâmetro, sem `comdat` e sem o `{`. `declare <isto>` é a declaração dela.
fn fn_signature(text: &str) -> Option<String> {
    let line = text.lines().next()?.strip_prefix("define ")?;
    let line = line.trim_end().strip_suffix('{')?.trim_end();
    let opens = line.find('(')?;
    // O `)` que fecha a lista de parâmetros.
    let mut level = 0i32;
    let mut closes = None;
    for (i, c) in line[opens..].char_indices() {
        match c {
            '(' | '{' | '[' | '<' => level += 1,
            ')' | '}' | ']' | '>' => {
                level -= 1;
                if level == 0 {
                    closes = Some(opens + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let closes = closes?;
    let head: Vec<&str> = line[..opens]
        .split_whitespace()
        .filter(|p| !matches!(*p, "linkonce_odr" | "internal" | "private" | "dso_local" | "weak_odr" | "linkonce"))
        .collect();
    // `comdat` e `comdat($name)`: a declaração não pertence a comdat.
    let after = line[closes + 1..]
        .split_whitespace()
        .filter(|p| *p != "comdat" && !p.starts_with("comdat("))
        .collect::<Vec<_>>()
        .join(" ");
    // Parâmetros sem o nome (`%x`) de cada um, no nível de fora.
    let mut params = Vec::new();
    let mut current = String::new();
    let mut level = 0i32;
    for c in line[opens + 1..closes].chars() {
        match c {
            '(' | '{' | '[' | '<' => level += 1,
            ')' | '}' | ']' | '>' => level -= 1,
            ',' if level == 0 => {
                params.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        params.push(current);
    }
    let params: Vec<String> = params
        .iter()
        .map(|p| p.split_whitespace().filter(|t| !t.starts_with('%')).collect::<Vec<_>>().join(" "))
        .collect();
    let mut s = format!("{}({})", head.join(" "), params.join(", "));
    if !after.is_empty() {
        s.push(' ');
        s.push_str(&after);
    }
    Some(s)
}

/// A infraestrutura da área de globais de cada geração: o descritor
/// (`@df.area`) e o id do módulo (`@df.area_id`), lidos pelo `df.obter_area`
/// local. Não entram na impressão digital nem impedem manter a função: o
/// layout da área só cresce entre gerações, e o runtime aceita o descritor
/// de uma geração anterior como prefixo do layout vigente, devolvendo a
/// mesma área (`dartforge_area_de_globais`).
const INFRASTRUCTURE: [&str; 2] = ["df.area", "df.area_id"];

/// O fecho local de `i`: os itens da geração alcançados por ele (funções
/// locais e globais, transitivamente), as funções do programa alcançadas
/// (pelo nome) e se alguma global mutável está no caminho.
fn closure_set(m: &IrModule, i: usize) -> (BTreeSet<usize>, BTreeSet<String>, bool) {
    let mut locals = BTreeSet::new();
    let mut program = BTreeSet::new();
    let mut mutable = false;
    let mut stack = vec![i];
    let mut seen_set = HashSet::from([i]);
    while let Some(k) = stack.pop() {
        for r in references(m.items[k].text) {
            if INFRASTRUCTURE.contains(&r.as_str()) {
                continue;
            }
            let Some(&j) = m.by_name.get(&r) else { continue };
            match m.items[j].tipo {
                Type::Function { local: false } => {
                    if j != i {
                        program.insert(r);
                    }
                }
                Type::Global { mutable: true } => mutable = true,
                Type::Comdat => {}
                _ => {
                    if seen_set.insert(j) {
                        locals.insert(j);
                        stack.push(j);
                    }
                }
            }
        }
    }
    (locals, program, mutable)
}

fn fingerprint(m: &IrModule, i: usize, locals: &BTreeSet<usize>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.items[i].text.hash(&mut h);
    let mut names: Vec<(&str, &str)> = locals.iter().map(|&j| (m.items[j].name.as_str(), m.items[j].text)).collect();
    names.sort();
    names.hash(&mut h);
    h.finish()
}

/// O IR da geração nova só com o que mudou.
pub(crate) struct Delta {
    /// O IR a compilar.
    pub ir: String,
    /// As funções que continuam na implementação publicada (declaradas no IR).
    pub kept_fns: Vec<String>,
    /// O que a geração publica: as funções do programa definidas no IR que
    /// sai, com a impressão digital de cada uma.
    pub new_items: HashMap<String, Published>,
}

/// O delta de `ir` contra as implementações `live_list`. `excluded`: funções que
/// a geração nova sempre define (os registros). `None` quando o IR não está
/// na forma esperada — então ele segue inteiro.
pub(crate) fn delta(ir: &str, live_list: &HashMap<String, Published>, excluded: &[&str]) -> Option<Delta> {
    let m = do_read(ir)?;
    let mut new_items = HashMap::new();
    let mut kept_fns: HashSet<usize> = HashSet::new();
    let mut declarations: HashMap<usize, String> = HashMap::new();
    let fn_signatures: HashMap<&str, String> = m
        .items
        .iter()
        .filter(|it| it.tipo == Type::Function { local: false })
        .filter_map(|it| Some((it.name.as_str(), fn_signature(it.text)?)))
        .collect();
    for (i, item) in m.items.iter().enumerate() {
        if item.tipo != (Type::Function { local: false }) {
            continue;
        }
        let fn_signature = fn_signatures.get(item.name.as_str())?.clone();
        let (locals, program, mutable) = closure_set(&m, i);
        let published = Published { fingerprint: fingerprint(&m, i, &locals), fn_signature };
        let keep = !mutable
            && !excluded.contains(&item.name.as_str())
            && live_list.get(&item.name) == Some(&published)
            && program.iter().all(|r| match (fn_signatures.get(r.as_str()), live_list.get(r)) {
                (Some(new_area), Some(live_one)) => *new_area == live_one.fn_signature,
                // Função do programa que o IR novo não define: a
                // referência é a mesma que a viva já fazia.
                (None, _) => true,
                (Some(_), None) => false,
            });
        if keep {
            kept_fns.insert(i);
            declarations.insert(i, format!("declare {}\n", published.fn_signature));
        } else {
            new_items.insert(item.name.clone(), published);
        }
    }
    if kept_fns.is_empty() {
        return Some(Delta { ir: ir.to_string(), kept_fns: Vec::new(), new_items });
    }
    // O que ainda é alcançado: das funções do programa que continuam
    // definidas e das globais que não são locais (as locais só existem por
    // quem as usa).
    let mut live_set: HashSet<usize> = HashSet::new();
    let mut stack: Vec<usize> = m
        .items
        .iter()
        .enumerate()
        .filter(|(i, it)| match it.tipo {
            Type::Function { local: false } => !kept_fns.contains(i),
            Type::Global { .. } => !it.text.split_whitespace().nth(2).is_some_and(|p| p == "private" || p == "internal"),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect();
    live_set.extend(stack.iter().copied());
    while let Some(k) = stack.pop() {
        for r in references(m.items[k].text) {
            let Some(&j) = m.by_name.get(&r) else { continue };
            if !kept_fns.contains(&j) && live_set.insert(j) {
                stack.push(j);
            }
        }
    }
    // Comdat só com o membro definido.
    for (i, it) in m.items.iter().enumerate() {
        if it.tipo == Type::Comdat
            && m.by_name.get(&it.name).is_some_and(|&f| live_set.contains(&f))
        {
            live_set.insert(i);
        }
    }
    let mut out = String::with_capacity(ir.len() / 2);
    for t in &m.fragments {
        match t {
            Err(text) => out.push_str(text),
            Ok(i) if kept_fns.contains(i) => out.push_str(&declarations[i]),
            Ok(i) if live_set.contains(i) => out.push_str(m.items[*i].text),
            Ok(i) => {
                // Função local só alcançada pelas mantidas: some; a geração
                // delas continua com a própria cópia.
                let _ = i;
            }
        }
    }
    let mut names: Vec<String> = kept_fns.iter().map(|&i| m.items[i].name.clone()).collect();
    names.sort();
    Some(Delta { ir: out, kept_fns: names, new_items })
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    /// O que a primeira geração de `ir` publica.
    fn published_list(ir: &str) -> HashMap<String, Published> {
        delta(ir, &HashMap::new(), &[]).expect("forma conhecida").new_items
    }

    const V1: &str = r#"; cabeçalho
$"g" = comdat any
@.str.0 = private unnamed_addr constant [2 x i8] c"ab"
@cache = internal global [2 x i64] zeroinitializer
declare i64 @dartforge_print(i64)

define i64 @f(i64 %v0, ptr %v1) {
b0:
  %r = call i64 @ajuda(i64 %v0)
  %s = call i64 @g(i64 %r)
  ret i64 %s
}

define internal i64 @ajuda(i64 %x) {
b0:
  %p = call i64 @dartforge_print(i64 ptrtoint (ptr @.str.0 to i64))
  ret i64 %x
}

define linkonce_odr i64 @"g"(i64 %v0) comdat {
b0:
  ret i64 %v0
}

define i64 @h(i64 %v0) {
b0:
  %c = load i64, ptr @cache
  ret i64 %c
}
"#;

    #[test]
    fn signature_without_names_or_linkage() {
        assert_eq!(fn_signature("define linkonce_odr i64 @\"g\"(i64 %v0) comdat {\n}").unwrap(), "i64 @\"g\"(i64)");
        assert_eq!(
            fn_signature("define linkonce_odr i64 @g$t(i64 %v0, ptr %v1) comdat($\"g$t\") {\n}").unwrap(),
            "i64 @g$t(i64, ptr)"
        );
        assert_eq!(
            fn_signature("define internal signext i8 @k({ i64, ptr } %a, ptr noalias %b) {\n}").unwrap(),
            "signext i8 @k({ i64, ptr }, ptr noalias)"
        );
    }

    #[test]
    fn keeps_unchanged_and_compiles_rest() {
        let live_list = published_list(V1);
        assert_eq!(live_list.len(), 3, "{live_list:?}");
        // `g` mudou: só ela é compilada; `f` (que chama `g` pelo trampolim)
        // fica na implementação viva; `h` usa global mutável e nunca fica.
        let v2 = V1.replace("  ret i64 %v0\n}\n\ndefine i64 @h", "  %y = add i64 %v0, 1\n  ret i64 %y\n}\n\ndefine i64 @h");
        let d = delta(&v2, &live_list, &[]).expect("forma conhecida");
        assert_eq!(d.kept_fns, vec!["f".to_string()]);
        assert!(d.ir.contains("declare i64 @f(i64, ptr)\n"), "{}", d.ir);
        // A local só alcançada por `f` some; o comdat de `g` fica.
        assert!(!d.ir.contains("@ajuda("), "{}", d.ir);
        assert!(d.ir.contains("$\"g\" = comdat any"));
        assert!(d.ir.contains("%y = add i64 %v0, 1"));
        assert!(d.ir.contains("define i64 @h("));
        assert_eq!(d.new_items.keys().collect::<BTreeSet<_>>(), BTreeSet::from([&"g".to_string(), &"h".to_string()]));
    }

    #[test]
    fn changed_local_recompiles_users() {
        let live_list = published_list(V1);
        let v2 = V1.replace("c\"ab\"", "c\"xy\"");
        let d = delta(&v2, &live_list, &[]).unwrap();
        // A string muda o fecho de `ajuda` e portanto o de `f`.
        assert!(!d.kept_fns.contains(&"f".to_string()), "{:?}", d.kept_fns);
        assert!(d.kept_fns.contains(&"g".to_string()));
    }

    #[test]
    fn changed_callee_signature_recompiles_caller() {
        let live_list = published_list(V1);
        let v2 = V1
            .replace("define linkonce_odr i64 @\"g\"(i64 %v0)", "define linkonce_odr i64 @\"g\"(i64 %v0, i64 %v1)")
            .replace("call i64 @g(i64 %r)", "call i64 @g(i64 %r)");
        let d = delta(&v2, &live_list, &[]).unwrap();
        assert!(d.kept_fns.is_empty(), "{:?}", d.kept_fns);
    }

    #[test]
    fn excluded_and_non_live_are_compiled() {
        let live_list = published_list(V1);
        let d = delta(V1, &live_list, &["f"]).unwrap();
        assert_eq!(d.kept_fns, vec!["g".to_string()]);
        let d = delta(V1, &HashMap::new(), &[]).unwrap();
        assert!(d.kept_fns.is_empty());
        assert_eq!(d.ir, V1);
    }
}
