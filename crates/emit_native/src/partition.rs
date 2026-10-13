//! O módulo do programa em partes (docs/NATIVO-PROJETOS-REAIS.md, C9).
//!
//! O IR de um programa grande não cabe num Clang: o `-O0` gasta ~44 bytes
//! de memória por byte de IR (24,8 MB de IR, 1,08 GB de pico, medido), e o
//! new_sali/backend tem 1,3 GB de IR — 55 GB para um processo só. Aqui o
//! texto do módulo é dividido em partes de ~[`DEFAULT_TARGET`] bytes, cada uma
//! um módulo LLVM completo, compiladas uma a uma (ou poucas por vez) e
//! ligadas juntas. Cada parte tem a sua chave no cache de objetos, então uma
//! edição recompila só as partes que mudaram.
//!
//! O que o emissor escreve (`llvm/mod.rs`) e como cada coisa vai às partes:
//!
//! * `define … @f(…) { … }` e `@g = …` com ligação externa ou
//!   `linkonce_odr`: numa parte só; as outras que os citam recebem a
//!   declaração (`declare` com os tipos dos parâmetros, ou
//!   `@g = external global|constant <ty>`);
//! * `private`/`internal` constantes e funções (os textos `@.str.N`, os
//!   vetores, os ajudantes `@df.*` `alwaysinline`): copiados em cada parte
//!   que os cita, com o que eles citam (fecho transitivo). São imutáveis, e
//!   duas cópias privadas não se veem;
//! * `internal global` mutável (o `@df.area_id`, o índice da área de
//!   globais do módulo, e a variável por thread do contexto,
//!   `sdk_modulo::with_context_in_tls`): uma cópia por parte quebraria o
//!   estado — passa a ter ligação externa na parte 0 e é declarada nas
//!   outras (a por thread, `thread_local`);
//! * `declare`, o cabeçalho (`target`, `source_filename`), atributos e
//!   metadados: em toda parte; `$x = comdat any`, na parte do item que o usa;
//!   os comentários (`; df.class`, `; df.fields`, que a recarga do JIT lê
//!   do IR inteiro) só na parte 0.
//!
//! As partes agrupam os itens pela biblioteca do símbolo
//! (`df.<library>.…`), em ordem, para que o conteúdo de uma parte dependa
//! das bibliotecas dela.

use std::collections::{HashMap, HashSet};

/// Tamanho de IR a partir do qual o módulo é dividido.
pub const DEFAULT_THRESHOLD: usize = 48 << 20;
/// Tamanho de cada parte.
pub const DEFAULT_TARGET: usize = 16 << 20;

/// O limiar e o alvo, ajustáveis (`DARTFORGE_PARTE_MB`, em MiB; `0` desliga
/// a divisão).
pub fn limits() -> Option<(usize, usize)> {
    match std::env::var("DARTFORGE_PARTE_MB").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(0) => None,
        Some(mb) => Some((2 * (mb << 20), mb << 20)),
        None => Some((DEFAULT_THRESHOLD, DEFAULT_TARGET)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemKind {
    Function,
    Global,
}

/// Um item definido no módulo.
struct Item<'a> {
    name: &'a str,
    ty: ItemKind,
    text: &'a str,
    local: bool,
    /// `internal global` (não `constant`): estado, não pode ser copiado.
    mutable: bool,
    comdat: Option<&'a str>,
}

/// O módulo lido.
struct Module<'a> {
    header: Vec<&'a str>,
    comments: Vec<&'a str>,
    declarations: Vec<(&'a str, &'a str)>,
    comdats: HashMap<&'a str, &'a str>,
    items: Vec<Item<'a>>,
    index: HashMap<&'a str, usize>,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'$' | b'-')
}

/// O nome em `@name…` a partir de `s` (que começa depois do `@`) e quantos
/// bytes ele ocupa no texto: `@"…"` (o emissor cita assim os nomes com `$`
/// numa referência, `@"df.ffi.S1$p_$ent"`) vale o nome sem as aspas, para
/// casar com a definição escrita sem elas.
fn name_and_width(s: &str) -> (&str, usize) {
    let b = s.as_bytes();
    if b.first() == Some(&b'"') {
        return s[1..].find('"').map_or(("", 0), |f| (&s[1..f + 1], f + 2));
    }
    let end = b.iter().position(|c| !is_ident(*c)).unwrap_or(b.len());
    (&s[..end], end)
}

fn name_in(s: &str) -> &str {
    name_and_width(s).0
}

/// Todas as referências `@name` de um texto.
fn references<'a>(text: &'a str, exit: &mut Vec<&'a str>) {
    let b = text.as_bytes();
    let mut i = 0;
    while let Some(p) = memchr_at(b, b'@', i) {
        let (n, width) = name_and_width(&text[p + 1..]);
        if !n.is_empty() {
            exit.push(n);
        }
        i = p + 1 + width;
    }
}

fn memchr_at(b: &[u8], c: u8, de: usize) -> Option<usize> {
    b.get(de..)?.iter().position(|x| *x == c).map(|p| p + de)
}

/// A primeira linha de um texto, com a quebra de linha.
fn first_line(t: &str) -> &str {
    t.find(char::from(10)).map_or(t, |i| &t[..=i])
}

/// `$x`/`$"x"` → `x`.
fn comdat_name(s: &str) -> &str {
    let s = s.strip_prefix('$').unwrap_or(s);
    s.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(s)
}

/// O comdat de um `define`/global: `comdat($x)` explícito, ou só `comdat`
/// (o do mesmo nome do item — os objetos estáticos da produção,
/// `@df.img.…`).
fn line_comdat<'a>(line: &'a str, name: &'a str) -> Option<&'a str> {
    if let Some(c) = line.find("comdat($") {
        let r = &line[c + 7..];
        return Some(comdat_name(&r[..r.find(')').unwrap_or(r.len())]));
    }
    let i = line.find(" comdat")?;
    let after = &line[i + " comdat".len()..];
    after.chars().next().is_none_or(|c| c == ',' || c.is_whitespace()).then_some(name)
}

fn read(ir: &str) -> Module<'_> {
    let mut m = Module {
        header: Vec::new(),
        comments: Vec::new(),
        declarations: Vec::new(),
        comdats: HashMap::new(),
        items: Vec::new(),
        index: HashMap::new(),
    };
    let mut pos = 0;
    let bytes = ir.as_bytes();
    while pos < bytes.len() {
        let line_end = memchr_at(bytes, b'\n', pos).map_or(bytes.len(), |p| p + 1);
        let line = &ir[pos..line_end];
        if line.starts_with("define ") {
            // Até a linha que é só `}`.
            let mut end = line_end;
            loop {
                if end >= bytes.len() {
                    break;
                }
                let next = memchr_at(bytes, b'\n', end).map_or(bytes.len(), |p| p + 1);
                let l = &ir[end..next];
                end = next;
                if l.starts_with('}') {
                    break;
                }
            }
            let hdr = &line[..line.find('(').unwrap_or(line.len())];
            let name = hdr.rfind('@').map_or("", |a| name_in(&hdr[a + 1..]));
            let local = hdr.contains(" internal ") || hdr.contains(" private ");
            let comdat = line_comdat(line, name);
            m.index.insert(name, m.items.len());
            m.items.push(Item { name, ty: ItemKind::Function, text: &ir[pos..end], local, mutable: false, comdat });
            pos = end;
            continue;
        }
        if let Some(r) = line.strip_prefix('@') {
            let (name, width) = name_and_width(r);
            let after = &r[width..];
            let after = after.trim_start().strip_prefix('=').unwrap_or(after).trim_start();
            let local = after.starts_with("private ") || after.starts_with("internal ");
            let mutable = local && global_head(after).0 == "global";
            let comdat = line_comdat(line, name);
            m.index.insert(name, m.items.len());
            m.items.push(Item { name, ty: ItemKind::Global, text: line, local, mutable, comdat });
        } else if line.starts_with("declare ") {
            let hdr = &line[..line.find('(').unwrap_or(line.len())];
            let name = hdr.rfind('@').map_or("", |a| name_in(&hdr[a + 1..]));
            m.declarations.push((name, line));
        } else if line.starts_with('$') {
            let n = line.split(' ').next().unwrap_or("");
            m.comdats.insert(comdat_name(n), line);
        } else if line.starts_with(';') {
            m.comments.push(line);
        } else {
            m.header.push(line);
        }
        pos = line_end;
    }
    m
}

/// `global`/`constant`, e o resto depois da palavra, de `<ligação…>
/// global|constant <tipo> <inicial>`.
fn global_head(s: &str) -> (&str, &str) {
    let mut rest = s;
    loop {
        let (word, after) = rest.split_once(' ').unwrap_or((rest, ""));
        if word == "global" || word == "constant" || word.is_empty() {
            return (word, after);
        }
        rest = after;
    }
}

/// O tipo LLVM no começo de `s` (com colchetes, chaves e `<…>` casados).
fn type_at_start(s: &str) -> &str {
    let b = s.as_bytes();
    let mut prof = 0i32;
    for (i, c) in b.iter().enumerate() {
        match c {
            b'[' | b'{' | b'<' | b'(' => prof += 1,
            b']' | b'}' | b'>' | b')' => {
                prof -= 1;
                if prof == 0 {
                    return &s[..=i];
                }
            }
            b' ' | b',' if prof == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// A declaração de um item definido em outra parte.
fn declaration(item: &Item) -> String {
    match item.ty {
        ItemKind::Global => {
            let r = &item.text[1 + name_and_width(&item.text[1..]).1..];
            let r = r.trim_start().trim_start_matches('=').trim_start();
            let (word, after) = global_head(r);
            let ty = type_at_start(after);
            // A variável por thread (`@df.ctx.tls`, `sdk_modulo::with_context_in_tls`)
            // continua por thread na declaração.
            let tls = if r[..r.len() - after.len()].split_whitespace().any(|w| w == "thread_local") { "thread_local " } else { "" };
            format!("@{} = external {tls}{word} {ty}\n", item.name)
        }
        ItemKind::Function => {
            let line = item.text.lines().next().unwrap_or("");
            let a = line.find('@').unwrap_or(0);
            let before = &line["define ".len()..a];
            let ret: Vec<&str> = before
                .split_whitespace()
                .filter(|w| {
                    !matches!(
                        *w,
                        "private" | "internal" | "linkonce_odr" | "weak_odr" | "external" | "hidden" | "dso_local" | "dllexport" | "fastcc" | "ccc"
                    )
                })
                .collect();
            let opens = a + line[a..].find('(').unwrap_or(0);
            // Os parâmetros, com vírgulas só no nível de cima.
            let mut prof = 0i32;
            let mut closes = opens;
            for (i, c) in line[opens..].char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => {
                        prof -= 1;
                        if prof == 0 {
                            closes = opens + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let params = &line[opens + 1..closes];
            let mut types = Vec::new();
            let mut prof = 0i32;
            let mut ini = 0;
            let mut parts = Vec::new();
            for (i, c) in params.char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => prof -= 1,
                    ',' if prof == 0 => {
                        parts.push(&params[ini..i]);
                        ini = i + 1;
                    }
                    _ => {}
                }
            }
            if !params.trim().is_empty() {
                parts.push(&params[ini..]);
            }
            for p in parts {
                let p = p.trim();
                let without_name = match p.rfind(" %") {
                    Some(k) => &p[..k],
                    None => p,
                };
                types.push(without_name.to_string());
            }
            format!("declare {} @{}({})\n", ret.join(" "), item.name, types.join(", "))
        }
    }
}

/// A chave de agrupamento de um símbolo: a biblioteca dele, quando o nome a
/// tem (`df.<library>.…`, `df.mt.<library>.…`, `df.rti.<h>.<library>`).
fn group(name: &str) -> &str {
    let parts: Vec<&str> = name.splitn(4, '.').collect();
    match parts.as_slice() {
        ["df", "mt", lib, ..] => lib,
        ["df", "rti", _, lib] => lib,
        ["df", lib, ..] => lib,
        _ => name,
    }
}

/// O módulo dividido: as partes são montadas uma a uma ([`Plan::part`]),
/// para que só as que estão sendo geradas existam ao mesmo tempo.
pub struct Plan<'a> {
    m: Module<'a>,
    parts: Vec<Vec<usize>>,
    declared_set: HashMap<&'a str, &'a str>,
    alvo: usize,
}

impl<'a> Plan<'a> {
    /// Quantas partes.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Nenhuma parte (nunca: um módulo tem ao menos uma).
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// O texto da parte `p`.
    pub fn part(&self, p: usize) -> String {
        let m = &self.m;
        let alvo = self.alvo;
        let declared_set = &self.declared_set;
        let items = &self.parts[p];
        // O fecho: os itens da parte, os locais que eles citam (copiados), e
        // as declarações do que mora fora.
        let mut included: HashSet<usize> = items.iter().copied().collect();
        let mut queue: Vec<usize> = items.to_vec();
        let mut externals: Vec<usize> = Vec::new();
        let mut seen_ext: HashSet<usize> = HashSet::new();
        let mut decls: Vec<&str> = Vec::new();
        let mut seen_decl: HashSet<&str> = HashSet::new();
        let mut refs = Vec::new();
        while let Some(i) = queue.pop() {
            refs.clear();
            references(m.items[i].text, &mut refs);
            for r in refs.iter() {
                if let Some(&j) = m.index.get(r) {
                    if j == i || included.contains(&j) {
                        continue;
                    }
                    let it = &m.items[j];
                    if it.local && !it.mutable {
                        included.insert(j);
                        queue.push(j);
                    } else if seen_ext.insert(j) {
                        externals.push(j);
                    }
                } else if let Some(d) = declared_set.get(r)
                    && seen_decl.insert(*r)
                {
                    decls.push(d);
                }
            }
        }
        let mut text = String::with_capacity(alvo + alvo / 4);
        for l in &m.header {
            text.push_str(l);
        }
        if p == 0 {
            for l in &m.comments {
                text.push_str(l);
            }
        }
        for d in &decls {
            text.push_str(d);
            if !d.ends_with('\n') {
                text.push('\n');
            }
        }
        externals.sort_unstable();
        for j in externals {
            text.push_str(&declaration(&m.items[j]));
        }
        let mut list: Vec<usize> = included.into_iter().collect();
        list.sort_unstable();
        for i in list {
            let it = &m.items[i];
            if let Some(c) = it.comdat
                && let Some(l) = m.comdats.get(c)
            {
                text.push_str(l);
            }
            if it.mutable {
                // `@df.area_id = internal global …` → ligação externa.
                text.push_str(&it.text.replacen(" = internal ", " = ", 1).replacen(" = private ", " = ", 1));
            } else if !it.local && first_line(it.text).contains(" linkonce_odr ") {
                // `linkonce_odr` → `weak_odr`: a definição tem de ficar no
                // objeto da parte dela. Com o ThinLTO (produção) o
                // `linkonce_odr` que só outras partes citam era descartado e
                // a ligação dava "undefined symbol: dfc.….get" (as
                // constantes canônicas do new_sali/backend).
                let first = first_line(it.text);
                text.push_str(&first.replacen(" linkonce_odr ", " weak_odr ", 1));
                text.push_str(&it.text[first.len()..]);
            } else {
                text.push_str(it.text);
            }
        }
        text
    }
}

/// Planeja a divisão do módulo em partes de ~`alvo` bytes.
pub fn make_plan(ir: &str, alvo: usize) -> Plan<'_> {
    let m = read(ir);
    // Os itens com ligação externa, agrupados pela biblioteca na ordem em
    // que aparecem; os grupos, em partes de ~`alvo` bytes.
    let mut group_order: Vec<&str> = Vec::new();
    let mut by_group: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, it) in m.items.iter().enumerate() {
        if it.local {
            continue;
        }
        let g = group(it.name);
        by_group.entry(g).or_insert_with(|| {
            group_order.push(g);
            Vec::new()
        }).push(i);
    }
    let mut parts: Vec<Vec<usize>> = vec![Vec::new()];
    let mut size = 0usize;
    for g in &group_order {
        for &i in &by_group[g] {
            let t = m.items[i].text.len();
            if size > 0 && size + t > alvo {
                parts.push(Vec::new());
                size = 0;
            }
            parts.last_mut().expect("uma parte").push(i);
            size += t;
        }
    }
    // O estado mutável local vai à parte 0, com ligação externa.
    for (i, it) in m.items.iter().enumerate() {
        if it.mutable {
            parts[0].push(i);
        }
    }
    let declared_set: HashMap<&str, &str> = m.declarations.iter().copied().collect();
    Plan { m, parts, declared_set, alvo }
}

/// Divide o módulo em partes de ~`alvo` bytes (todas de uma vez; os testes).
/// Um módulo menor que `alvo` volta inteiro, como está.
pub fn split(ir: &str, alvo: usize) -> Vec<String> {
    if ir.len() <= alvo {
        return vec![ir.to_string()];
    }
    let plan = make_plan(ir, alvo);
    (0..plan.len()).map(|p| plan.part(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOD: &str = "target triple = \"x86_64-pc-windows-msvc\"\n\
; df.classe 1 lib A\n\
declare i64 @dartforge_x(i64)\n\
@.str.0 = private unnamed_addr constant [2 x i8] c\"oi\"\n\
@df.area_id = internal global i64 0, align 8\n\
@df.a.g = global { i64, [2 x ptr] } zeroinitializer\n\
$\"df.b.k\" = comdat any\n\
define internal i64 @df.ajuda(i64 %x) alwaysinline {\n\
  %a = load i64, ptr @df.area_id\n\
  ret i64 %x\n\
}\n\
define i64 @df.a.f(i64 %v0, { i64, i64 } %v1) {\n\
b0:\n\
  %r = call i64 @df.ajuda(i64 %v0)\n\
  %s = call i64 @\"df.b.k\"(ptr @.str.0)\n\
  ret i64 %s\n\
}\n\
define linkonce_odr i64 @df.b.k(ptr %p) comdat($\"df.b.k\") {\n\
b0:\n\
  %q = call i64 @dartforge_x(i64 1)\n\
  %g = load i64, ptr @df.a.g\n\
  ret i64 %q\n\
}\n";

    #[test]
    fn splits_with_declarations_and_copies() {
        let p = split(MOD, 120);
        assert_eq!(p.len(), 3, "{p:#?}");
        // a parte de `df.a.f` copia o ajudante e o texto, declara `df.b.k`.
        let pa = p.iter().find(|t| t.contains("define i64 @df.a.f")).unwrap();
        assert!(pa.contains("define internal i64 @df.ajuda"));
        assert!(pa.contains("@.str.0 = private"));
        assert!(pa.contains("declare i64 @df.b.k(ptr)"));
        assert!(pa.contains("@df.area_id = external global i64") || pa.contains("@df.area_id = global i64 0"));
        let pb = p.iter().find(|t| t.contains("define weak_odr i64 @df.b.k")).unwrap();
        assert!(pb.contains("$\"df.b.k\" = comdat any"));
        assert!(pb.contains("declare i64 @dartforge_x(i64)"));
        assert!(pb.contains("@df.a.g = external global { i64, [2 x ptr] }"), "{pb}");
        // O estado mutável: definido uma vez, com ligação externa.
        assert_eq!(p.iter().filter(|t| t.contains("@df.area_id = global i64 0")).count(), 1);
        assert!(p[0].contains("; df.classe"));
    }

    /// A variável por thread do contexto (`sdk_modulo::with_context_in_tls`)
    /// é estado: uma definição, e a declaração nas outras partes continua
    /// `thread_local`.
    #[test]
    fn thread_local_variable_stays_thread_local() {
        let m = read("@df.ctx.tls.programa = internal thread_local global ptr null, align 8\n");
        assert!(m.items[0].mutable);
        assert_eq!(declaration(&m.items[0]), "@df.ctx.tls.programa = external thread_local global ptr\n");
    }

    #[test]
    fn declares_function_with_composite_parameters() {
        let m = read(MOD);
        let i = m.index["df.a.f"];
        assert_eq!(declaration(&m.items[i]), "declare i64 @df.a.f(i64, { i64, i64 })\n");
    }

    #[test]
    fn implicit_comdat_goes_with_item() {
        let m = "$df.img.a = comdat any
@df.img.a = linkonce_odr constant i64 1, comdat
define i64 @df.x.f() {
b0:
  %v = load i64, ptr @df.img.a
  ret i64 %v
}
";
        let p = split(m, 40);
        let def = p.iter().find(|t| t.contains("@df.img.a = weak_odr")).unwrap();
        assert!(def.contains("$df.img.a = comdat any"), "{def}");
        let uses = p.iter().find(|t| t.contains("define i64 @df.x.f")).unwrap();
        assert!(uses.contains("@df.img.a = external constant i64") || uses.contains("@df.img.a = weak_odr"), "{uses}");
    }
}
