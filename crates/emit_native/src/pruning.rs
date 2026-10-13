//! Poda das tabelas de métodos na ligação do AOT de produção
//! (docs/NATIVO-PODA-DE-TABELAS.md).
//!
//! No perfil de produção o SDK da fonte não define as tabelas de métodos das
//! classes dele: cada biblioteca grava, junto do bitcode em cache, um
//! **resumo** ([`summarize`]) com o que cada função e cada global cita, os
//! seletores que cada função chama e o conteúdo das tabelas. Na ligação do
//! programa, [`assemble`] lê o IR do programa com as mesmas regras, faz um
//! ponto fixo por seletor (o RTA de Bacon: uma entrada da tabela de uma
//! classe viva só entra se o seletor dela é chamado por alguma função viva) e
//! devolve o IR do programa com todas as tabelas definidas, só com os pares
//! vivos. O LTO e o `/OPT:REF` tiram o resto.
//!
//! O JIT, a recarga e a DLL de desenvolvimento não passam por aqui: lá as
//! tabelas continuam inteiras, nos módulos do SDK (§3.8 da especificação).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Primeira linha de um resumo; o número muda com o formato.
const HEADER: &str = "dartforge-poda\t2";

/// Seletores que vivem em toda classe viva, sem chamada no IR (§3.6):
/// `c:call` o runtime procura pelo texto (`dartforge_closure_entry`, a
/// classe chamável e o `Function.apply`); os outros são o protocolo de
/// `Object` que o runtime alcança por ajudantes do SDK — baratos, porque
/// quase toda classe viva já os tem vivos.
pub const ROOT_SELECTORS: &[&str] = &["c:call", "c:toString", "c:==", "g:hashCode", "c:noSuchMethod", "g:runtimeType"];

/// Funções do runtime que procuram na tabela por um nome que só existe em
/// tempo de execução (§3.7). Citada por função viva, toda entrada de tabela
/// viva fica. Vazia: nenhum mecanismo de hoje faz isso (o `dart:mirrors`
/// nativo não invoca por nome); o próximo entra aqui.
pub const CALL_BY_NAME: &[&str] = &[];

/// Uma tabela de métodos como o lowering a monta (`Module::method_tables`):
/// o cid, a função que devolve a tabela e os pares `(selector, entry)`.
pub type ModuleTable = (u32, String, Vec<(String, String)>);

/// O hash de um seletor (FNV-1a de 64 bits, `lower::closures::hash_name`).
fn selector_hash(text: &str) -> i64 {
    crate::lower::closures::hash_name(text)
}

/// Os pares `(hash, selector, entry)` de uma tabela de métodos, ordenados
/// pelo hash e sem hash repetido: a ordem da busca binária do runtime
/// (`dartforge_registrar_tabela`).
///
/// # Panics
/// Dois seletores diferentes com o mesmo hash na mesma classe.
pub fn table_pairs(cid: u32, methods: &[(String, String)]) -> Vec<(i64, String, String)> {
    let mut pairs: Vec<(i64, String, String)> =
        methods.iter().map(|(s, f)| (selector_hash(s), s.clone(), f.clone())).collect();
    pairs.sort();
    for par in pairs.windows(2) {
        assert!(
            par[0].0 != par[1].0 || par[0].1 == par[1].1,
            "hash de seletor repetido na classe {cid}: {} e {}",
            par[0].1,
            par[1].1
        );
    }
    pairs.dedup_by(|a, b| a.0 == b.0);
    pairs
}

/// A linha de uma tabela de métodos: `prefix` (`@X$d = private unnamed_addr
/// constant `), os types e `{ cid, n, (hash, entrada)… }`. `pares` já vem na
/// ordem da tabela, com as entradas escritas como no IR (`@f`).
///
/// ```
/// use dartforge_emit_native::poda::linha_de_tabela;
/// let l = linha_de_tabela("@t$d = private unnamed_addr constant ", 7, &[(5, "@f".to_string())]);
/// assert_eq!(l, "@t$d = private unnamed_addr constant { i64, i64, i64, ptr } { i64 7, i64 1, i64 5, ptr @f }\n");
/// ```
pub fn table_line(prefix: &str, cid: i64, pairs: &[(i64, String)]) -> String {
    let mut types = String::from("i64, i64");
    let mut items = format!("i64 {cid}, i64 {}", pairs.len());
    for (h, f) in pairs {
        types.push_str(", i64, ptr");
        write!(items, ", i64 {h}, ptr {f}").unwrap();
    }
    format!("{prefix}{{ {types} }} {{ {items} }}\n")
}

/// Um caractere de identificador LLVM sem aspas.
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'$' | b'.' | b'_')
}

/// Um nome escrito com `@`: sem aspas se ele permite.
fn llvm_name(name: &str) -> String {
    let simple = !name.is_empty()
        && !name.as_bytes()[0].is_ascii_digit()
        && name.bytes().all(name_char);
    if simple { format!("@{name}") } else { format!("@\"{name}\"") }
}

/// Os símbolos citados (`@x` e `@"x"`) em `text`, sem o `@` e sem as aspas.
/// Um `@` dentro de uma constante de texto dá um nome que nada define — é
/// conservador, nunca tira nada.
fn cited(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] != b'@' {
                i += 1;
                continue;
            }
            i += 1;
            if i < b.len() && b[i] == b'"' {
                let ini = i + 1;
                let end = text[ini..].find('"').map_or(b.len(), |k| ini + k);
                i = end + 1;
                return Some(&text[ini..end]);
            }
            let ini = i;
            while i < b.len() && name_char(b[i]) {
                i += 1;
            }
            if i > ini {
                return Some(&text[ini..i]);
            }
        }
        None
    })
}

/// O nome definido por uma linha `define … @f(`, e se ele é local
/// (`internal`/`private`).
fn define_name(line: &str) -> Option<(&str, bool)> {
    let rest = line.strip_prefix("define ")?;
    let at_sign = rest.find('@')?;
    let local = rest[..at_sign].split_whitespace().any(|p| p == "internal" || p == "private");
    let name = cited(&rest[at_sign..]).next()?;
    Some((name, local))
}

/// O nome de uma linha de global `@g = …`, se ele é local, e o resto depois
/// do `=`.
fn global_name(line: &str) -> Option<(&str, bool, &str)> {
    if !line.starts_with('@') {
        return None;
    }
    let name = cited(line).next()?;
    let equal = line.find(" = ")?;
    let rest = &line[equal + 3..];
    let local = rest.starts_with("private ") || rest.starts_with("internal ");
    Some((name, local, rest))
}

/// Decodifica o `c"…"` de uma constante de bytes (`\XX`).
fn constant_text(line: &str) -> Option<String> {
    let ini = line.find("c\"")? + 2;
    let end = ini + line[ini..].find('"')?;
    let s = &line[ini..end];
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && let Some(h) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            v.push(h);
            i += 3;
            continue;
        }
        v.push(b[i]);
        i += 1;
    }
    Some(String::from_utf8_lossy(&v).into_owned())
}

/// O hash e o índice do nome de uma chamada `@df.seletor(ptr …, i64 …, i64
/// <hash>, ptr @df.seln.<k>, i64 …)`; `None` se o hash não é constante.
fn selector_call(line: &str) -> Option<(i64, &str)> {
    let i = line.find("@df.seletor(")? + "@df.seletor(".len();
    let mut args = line[i..].split(", ");
    args.next()?;
    args.next()?;
    let h: i64 = args.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let name = args.next()?.strip_prefix("ptr @")?;
    Some((h, name.trim_end_matches(')')))
}

/// O hash e o nome (`@df.seln.<k>`) do descritor `@df.seld.<k> = …
/// { i64 <hash>, ptr @df.seln.<k>, i64 <len> }` (o despacho compacto,
/// `llvm/selectors.rs`).
fn selector_descriptor(rest: &str) -> Option<(i64, &str)> {
    let values = rest[rest.find("} {")? + 3..].trim_start();
    let mut it = values.split(", ");
    let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let name = it.next()?.trim().strip_prefix("ptr @")?;
    Some((h, name))
}

/// O descritor da chamada `@df.seletor_d(ptr …, i64 …, ptr @df.seld.<k>)`.
fn compact_selector_call(line: &str) -> Option<&str> {
    let i = line.find("@df.seletor_d(")? + "@df.seletor_d(".len();
    let mut args = line[i..].split(", ");
    args.next()?;
    args.next()?;
    let d = args.next()?.strip_prefix("ptr @")?;
    Some(d.trim_end_matches(')'))
}

/// Uma tabela de métodos lida de uma linha `@X$d = … constant { … } { i64
/// cid, i64 n, i64 h, ptr @f, … }`: `(cid, [(hash, entrada)])`.
fn line_table(rest: &str) -> Option<(i64, Vec<(i64, String)>)> {
    let values = &rest[rest.find("} {")? + 3..];
    let values = values.trim_end().strip_suffix('}')?.trim();
    let mut it = values.split(", ");
    let cid: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let n: usize = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let mut pairs = Vec::with_capacity(n);
    for _ in 0..n {
        let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
        let f = it.next()?.strip_prefix("ptr ")?.trim();
        pairs.push((h, cited(f).next()?.to_string()));
    }
    Some((cid, pairs))
}

/// Uma definição do resumo: o nome (índice), o que ela cita e os seletores
/// que ela chama.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Definition {
    pub name: u32,
    pub refs: Vec<u32>,
    pub calls: Vec<i64>,
}

/// Uma tabela de métodos do resumo: o nó-tabela (`X$d`, índice), o cid e os
/// pares `(hash, entry)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SummarizedTable {
    pub node: u32,
    pub cid: i64,
    pub pairs: Vec<(i64, u32)>,
}

/// O resumo de um módulo (uma biblioteca do SDK ou o programa): §3.3.
///
/// Os nomes locais (`private`/`internal`) levam o prefixo `<módulo>#`,
/// porque se repetem em todo módulo (`@df.seln.0`, `@df.class`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub module: String,
    pub names: Vec<String>,
    pub definitions: Vec<Definition>,
    pub tables: Vec<SummarizedTable>,
    /// Os textos dos seletores (para o relatório).
    pub selectors: Vec<(i64, String)>,
    /// As tabelas cuja função é deste resumo, mas que o módulo não define
    /// (o SDK de produção): o programa as define (`assemble`). Índices do nó
    /// da função.
    pub externals: Vec<u32>,
    /// As receitas RTI com um tipo de função (`F<…>`) e o nó que as tem (a
    /// função, ou a constante do texto no IR): de onde saem as assinaturas
    /// nativas que o programa pode pedir ao runtime (§3.14).
    pub recipes: Vec<(u32, String)>,
    index: HashMap<String, u32>,
}

impl Summary {
    fn new(module: &str) -> Summary {
        Summary { module: module.to_string(), ..Default::default() }
    }

    fn id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        i
    }

    /// O texto do resumo (o formato de §3.3).
    pub fn to_text(&self) -> String {
        let mut s = format!("{HEADER}\t{}\n", self.module);
        for n in &self.names {
            writeln!(s, "N\t{n}").unwrap();
        }
        let join = |v: &mut dyn Iterator<Item = String>| v.collect::<Vec<_>>().join(" ");
        for d in &self.definitions {
            writeln!(
                s,
                "D\t{}\t{}\t{}",
                d.name,
                join(&mut d.refs.iter().map(u32::to_string)),
                join(&mut d.calls.iter().map(i64::to_string))
            )
            .unwrap();
        }
        for t in &self.tables {
            writeln!(s, "T\t{}\t{}\t{}", t.node, t.cid, join(&mut t.pairs.iter().map(|(h, f)| format!("{h}:{f}")))).unwrap();
        }
        for e in &self.externals {
            writeln!(s, "X\t{e}").unwrap();
        }
        for (h, t) in &self.selectors {
            writeln!(s, "S\t{h}\t{t}").unwrap();
        }
        for (node, t) in &self.recipes {
            writeln!(s, "R\t{node}\t{t}").unwrap();
        }
        s
    }

    /// Lê o texto de [`Summary::to_text`].
    ///
    /// # Erros
    /// Cabeçalho de outra versão ou linha malformada.
    pub fn from_string(text: &str) -> Result<Summary, String> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or_default();
        let module = first
            .strip_prefix(HEADER)
            .and_then(|r| r.strip_prefix('\t'))
            .ok_or_else(|| format!("resumo de poda com cabeçalho desconhecido: {first:?}"))?;
        let mut r = Summary::new(module);
        let bad = |l: &str| format!("resumo de poda malformado: {l:?}");
        let numbers = |c: Option<&str>| -> Result<Vec<u32>, String> {
            c.unwrap_or_default().split(' ').filter(|x| !x.is_empty()).map(|x| x.parse().map_err(|_| bad(x))).collect()
        };
        for l in lines {
            let mut c = l.splitn(4, '\t');
            match c.next() {
                Some("N") => {
                    let n = c.next().ok_or_else(|| bad(l))?;
                    r.index.insert(n.to_string(), r.names.len() as u32);
                    r.names.push(n.to_string());
                }
                Some("D") => {
                    let name = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let refs = numbers(c.next())?;
                    let calls = c
                        .next()
                        .unwrap_or_default()
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse().map_err(|_| bad(l)))
                        .collect::<Result<_, _>>()?;
                    r.definitions.push(Definition { name, refs, calls });
                }
                Some("T") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let cid = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let mut pairs = Vec::new();
                    for p in c.next().unwrap_or_default().split(' ').filter(|x| !x.is_empty()) {
                        let (h, f) = p.split_once(':').ok_or_else(|| bad(l))?;
                        pairs.push((h.parse().map_err(|_| bad(l))?, f.parse().map_err(|_| bad(l))?));
                    }
                    r.tables.push(SummarizedTable { node, cid, pairs });
                }
                Some("X") => r.externals.push(c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?),
                Some("S") => {
                    let h = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.selectors.push((h, c.next().unwrap_or_default().to_string()));
                }
                Some("R") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.recipes.push((node, c.next().unwrap_or_default().to_string()));
                }
                _ if l.is_empty() => {}
                _ => return Err(bad(l)),
            }
        }
        let n = r.names.len() as u32;
        let outside = r.definitions.iter().any(|d| d.name >= n || d.refs.iter().any(|&x| x >= n))
            || r.tables.iter().any(|t| t.node >= n || t.pairs.iter().any(|&(_, f)| f >= n))
            || r.externals.iter().any(|&e| e >= n);
        if outside {
            return Err("resumo de poda com índice fora da tabela de nomes".to_string());
        }
        Ok(r)
    }
}

/// O resumo de um módulo a partir do IR dele (§3.3). `module` é o prefixo
/// dos nomes locais; `externals` são as tabelas que o módulo cita, mas não
/// define (o SDK de produção, com `with_tables_at_link`): `(cid,
/// função da tabela, métodos)`, as_type `Module::tabelas_de_metodos`.
///
/// ```
/// use dartforge_emit_native::poda::resumir;
/// let ir = "define i64 @f(i64 %x) {\n  %a = call i64 @g(i64 %x)\n  ret i64 %a\n}\n";
/// let r = resumir(ir, "m", &[]);
/// assert_eq!(r.nomes, vec!["f".to_string(), "g".to_string()]);
/// assert_eq!(r.definicoes[0].refs, vec![1]);
/// ```
pub fn summarize(ir: &str, module: &str, externals: &[ModuleTable]) -> Summary {
    // Passo 1: os nomes locais e os textos dos seletores.
    let mut locals: HashSet<&str> = HashSet::new();
    let mut seln_strings: HashMap<&str, String> = HashMap::new();
    let mut descriptors: HashMap<&str, (i64, &str)> = HashMap::new();
    let mut recipes: Vec<(&str, String)> = Vec::new();
    for l in ir.lines() {
        if let Some((n, true)) = define_name(l) {
            locals.insert(n);
        } else if let Some((n, local, rest)) = global_name(l) {
            if local {
                locals.insert(n);
            }
            if n.starts_with("df.seln.")
                && let Some(t) = constant_text(l)
            {
                seln_strings.insert(n, t);
            } else if n.starts_with("df.seld.")
                && let Some(d) = selector_descriptor(rest)
            {
                descriptors.insert(n, d);
            } else if (rest.contains("F\\3C") || rest.contains("F<"))
                && let Some(t) = constant_text(l)
                && t.contains("F<")
            {
                // `F<` no IR: `F\3C` nos textos privados
                // (`emit_string_constants`), `F<` nos objetos estáticos.
                recipes.push((n, t.trim_end_matches('\0').to_string()));
            }
        }
    }
    let mut r = Summary::new(module);
    let key = |n: &str| if locals.contains(n) { format!("{module}#{n}") } else { n.to_string() };
    let mut seen_selectors: HashSet<i64> = HashSet::new();
    // Passo 2: as definições.
    let mut current: Option<Definition> = None;
    for l in ir.lines() {
        if let Some(d) = current.as_mut() {
            if l == "}" {
                r.definitions.push(current.take().expect("definição corrente"));
                continue;
            }
            let call = if l.contains("@df.seletor(") {
                selector_call(l)
            } else if l.contains("@df.seletor_d(") {
                compact_selector_call(l).and_then(|d| descriptors.get(d).copied())
            } else {
                None
            };
            if let Some((h, seln)) = call {
                d.calls.push(h);
                if let Some(t) = seln_strings.get(seln) {
                    if seen_selectors.insert(h) {
                        r.selectors.push((h, t.clone()));
                    }
                    // A volta do seletor tipado (`dartforge_seletor`).
                    if let Some(without_t) = t.strip_prefix('t') {
                        let h2 = selector_hash(without_t);
                        d.calls.push(h2);
                        if seen_selectors.insert(h2) {
                            r.selectors.push((h2, without_t.to_string()));
                        }
                    }
                }
            }
            if l.contains('@') {
                let refs: Vec<String> = cited(l).map(key).collect();
                for x in refs {
                    let i = r.id(&x);
                    d.refs.push(i);
                }
            }
            continue;
        }
        if let Some((n, _)) = define_name(l) {
            let k = key(n);
            let name = r.id(&k);
            let after = &l[l.find('(').unwrap_or(l.len())..];
            let refs: Vec<String> = cited(after).map(key).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            if l.trim_end().ends_with('}') {
                // Uma função numa linha só.
                r.definitions.push(Definition { name, refs, calls: Vec::new() });
            } else {
                current = Some(Definition { name, refs, calls: Vec::new() });
            }
            continue;
        }
        if let Some((n, _, rest)) = global_name(l) {
            let k = key(n);
            let node = r.id(&k);
            if n.starts_with("df.mt.")
                && n.ends_with("$d")
                && let Some((cid, pairs)) = line_table(rest)
            {
                let pairs = pairs.iter().map(|(h, f)| (*h, r.id(&key(f)))).collect();
                r.tables.push(SummarizedTable { node, cid, pairs });
                continue;
            }
            let refs: Vec<String> = cited(rest).map(key).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            r.definitions.push(Definition { name: node, refs, calls: Vec::new() });
        }
    }
    for (cid, sym_name, methods) in externals {
        let function = r.id(sym_name);
        let node = r.id(&format!("{sym_name}$d"));
        let pairs: Vec<(i64, u32)> = table_pairs(*cid, methods)
            .into_iter()
            .map(|(h, s, f)| {
                if seen_selectors.insert(h) {
                    r.selectors.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definitions.push(Definition { name: function, refs: vec![node], calls: Vec::new() });
        r.tables.push(SummarizedTable { node, cid: i64::from(*cid), pairs });
        r.externals.push(function);
    }
    for (n, t) in recipes {
        let node = r.id(&key(n));
        r.recipes.push((node, t));
    }
    r
}

/// O fim do termo de receita RTI que começa em `i` (letra, dígitos, `<…>`,
/// `?`; `lower/rti.rs`, `write_type`).
fn term_end(b: &[u8], mut i: usize) -> Option<usize> {
    if !b.get(i)?.is_ascii_uppercase() {
        return None;
    }
    i += 1;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if b.get(i) == Some(&b'<') {
        let mut n = 0usize;
        loop {
            match b.get(i)? {
                b'<' => n += 1,
                b'>' => {
                    n -= 1;
                    if n == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    if b.get(i) == Some(&b'?') {
        i += 1;
    }
    Some(i)
}

/// Os termos separados por vírgula a partir de `i`, até `end` (que fica na
/// posição devolvida).
fn terms_until(b: &[u8], mut i: usize, end: u8) -> Option<(Vec<(usize, usize)>, usize)> {
    let mut v = Vec::new();
    if b.get(i) == Some(&end) {
        return Some((v, i));
    }
    loop {
        let f = term_end(b, i)?;
        v.push((i, f));
        match *b.get(f)? {
            b',' => i = f + 1,
            c if c == end => return Some((v, f)),
            _ => return None,
        }
    }
}

/// O número decimal em `i` e a posição depois dele.
fn number_in(b: &[u8], mut i: usize) -> Option<(i64, usize)> {
    let ini = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    std::str::from_utf8(&b[ini..i]).ok()?.parse().ok().map(|n| (n, i))
}

/// A letra (ou `S<rti>.`) de um tipo nativo `C<id>` na chave da FFI.
fn ffi_letter(b: &[u8], i: usize, letters: &HashMap<i64, char>, composites: &HashSet<i64>, s: &mut String) -> Option<i64> {
    if b.get(i) != Some(&b'C') {
        return None;
    }
    let (id, _) = number_in(b, i + 1)?;
    if composites.contains(&id) {
        s.push_str(&format!("S{id}."));
    } else {
        s.push(*letters.get(&id)?);
    }
    Some(id)
}

/// As chaves da FFI (`lower/ffi.rs::signature_key`, a mesma do
/// runtime, `crates/runtime/src/ffi.rs::signature_key`) das
/// assinaturas nativas escritas na receita RTI `text`: cada termo
/// `F<0;R;n;P1,…,Pn;>` sem parâmetros opcionais nem nomeados cujo retorno e
/// parâmetros são classes de tipo nativo (`letters`, a tabela que o runtime
/// recebe; `S<rti>.` para as structs e unions por valor, `composites`;
/// `VarArgs`, a letra `*`, abre os tipos variádicos). O Dart exige o tipo
/// nativo constante em `asFunction`, `lookupFunction`, `fromFunction` e
/// `NativeCallable` (o transformador de FFI do front-end recusa um
/// genérico), então toda assinatura que o programa pede ao runtime está
/// escrita numa receita do sítio da chamada (§3.14).
pub fn recipe_ffi_keys(text: &str, letters: &HashMap<i64, char>, composites: &HashSet<i64>, exit: &mut HashSet<String>) {
    let b = text.as_bytes();
    // A parte da chave do termo `b[i..f]`.
    let part = |i: usize, f: usize, s: &mut String| -> Option<()> {
        let before = s.len();
        let id = ffi_letter(b, i, letters, composites, s)?;
        if !s[before..].ends_with('*') || composites.contains(&id) {
            return Some(());
        }
        // `VarArgs<(T1, T2…)>` ou `VarArgs<T>`: os tipos depois da marca.
        let (_, j) = number_in(b, i + 1)?;
        if j >= f || b[j] != b'<' {
            return Some(());
        }
        let types: Vec<(usize, usize)> = if b.get(j + 1) == Some(&b'R') && b.get(j + 2) == Some(&b'<') {
            let (v, end) = terms_until(b, j + 3, b';')?;
            if b.get(end + 1) != Some(&b'>') {
                return None;
            }
            v
        } else {
            vec![(j + 1, term_end(b, j + 1)?)]
        };
        for (a, _) in types {
            let before = s.len();
            ffi_letter(b, a, letters, composites, s)?;
            if s[before..].ends_with('*') {
                return None;
            }
        }
        Some(())
    };
    let mut i = 0;
    while let Some(d) = text[i..].find("F<") {
        let ini = i + d;
        i = ini + 2;
        let key = (|| -> Option<String> {
            let (generics, j) = number_in(b, ini + 2)?;
            if generics != 0 || b.get(j) != Some(&b';') {
                return None;
            }
            let r_end = term_end(b, j + 1)?;
            if b.get(r_end) != Some(&b';') {
                return None;
            }
            let (n, k) = number_in(b, r_end + 1)?;
            if b.get(k) != Some(&b';') {
                return None;
            }
            let (params, end) = terms_until(b, k + 1, b';')?;
            // Sem opcionais nem nomeados.
            if params.len() as i64 != n || b.get(end + 1) != Some(&b'>') {
                return None;
            }
            let mut s = String::new();
            part(j + 1, r_end, &mut s)?;
            s.push('_');
            for (a, z) in params {
                part(a, z, &mut s)?;
            }
            Some(s)
        })();
        if let Some(c) = key {
            exit.insert(c);
        }
    }
}


/// Por que um nó está vivo (a primeira vez que ele entrou).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Root,
    /// Citado pelo nó.
    Ref(u32),
    /// Par da tabela (nó) pelo seletor.
    Tabela(u32, i64),
}

/// O resultado do ponto fixo sobre vários resumos (§3.5).
pub struct Reach {
    pub names: Vec<String>,
    /// A causa de cada nó vivo.
    pub causes: HashMap<u32, Cause>,
    /// Os seletores chamados e a primeira função viva que chamou cada um
    /// (`None` nas raízes).
    pub callees: HashMap<i64, Option<u32>>,
    /// Alguma função de [`CALL_BY_NAME`] ficou viva.
    pub all_pairs: bool,
    pub strings: HashMap<i64, String>,
}

impl Reach {
    pub fn live(&self, node: u32) -> bool {
        self.causes.contains_key(&node)
    }

    pub fn live_pair(&self, h: i64) -> bool {
        self.all_pairs || self.callees.contains_key(&h)
    }
}

/// Os nós, as arestas e as tabelas de todos os resumos, com os nomes
/// unificados.
struct Graph {
    names: Vec<String>,
    index: HashMap<String, u32>,
    refs: Vec<Vec<u32>>,
    calls: Vec<Vec<i64>>,
    /// Nó-tabela → (cid, pares).
    tables: HashMap<u32, (i64, Vec<(i64, u32)>)>,
    strings: HashMap<i64, String>,
    /// Os nós que algum módulo define (os outros são do runtime ou de fora).
    defined: HashSet<u32>,
}

impl Graph {
    fn id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        self.refs.push(Vec::new());
        self.calls.push(Vec::new());
        i
    }

    fn de(summaries: &[&Summary]) -> Graph {
        let mut g = Graph {
            names: Vec::new(),
            index: HashMap::new(),
            refs: Vec::new(),
            calls: Vec::new(),
            tables: HashMap::new(),
            strings: HashMap::new(),
            defined: HashSet::new(),
        };
        for r in summaries {
            let map: Vec<u32> = r.names.iter().map(|n| g.id(n)).collect();
            for d in &r.definitions {
                let n = map[d.name as usize] as usize;
                g.defined.insert(n as u32);
                g.refs[n].extend(d.refs.iter().map(|&x| map[x as usize]));
                g.calls[n].extend_from_slice(&d.calls);
            }
            for t in &r.tables {
                let pairs = t.pairs.iter().map(|&(h, f)| (h, map[f as usize])).collect();
                g.tables.insert(map[t.node as usize], (t.cid, pairs));
                g.defined.insert(map[t.node as usize]);
            }
            for (h, s) in &r.selectors {
                g.strings.entry(*h).or_insert_with(|| s.clone());
            }
        }
        g
    }

    /// Uma raiz de nome (§3.6), se algum módulo a define: a entrada do
    /// processo, o que o runtime pode citar por símbolo e os globais do
    /// LLVM. (Uma função do runtime citada pelo código Dart não é raiz: o
    /// relatório diz quem a cita.)
    fn root(name: &str) -> bool {
        name == "main" || name.starts_with("dartforge_") || name.starts_with("llvm.")
    }

    /// O ponto fixo (§3.5). `prune = false`: todo par de tabela viva vive.
    fn reachability(self, prune: bool) -> Reach {
        let mut causes: HashMap<u32, Cause> = HashMap::new();
        let mut callees: HashMap<i64, Option<u32>> = HashMap::new();
        let mut pending_set: HashMap<i64, Vec<(u32, u32)>> = HashMap::new();
        let mut queue: Vec<(u32, Cause)> = Vec::new();
        let mut all = !prune;
        for (i, n) in self.names.iter().enumerate() {
            if Self::root(n) && self.defined.contains(&(i as u32)) {
                queue.push((i as u32, Cause::Root));
            }
        }
        for s in ROOT_SELECTORS {
            callees.insert(selector_hash(s), None);
        }
        let by_name: HashSet<u32> = CALL_BY_NAME.iter().filter_map(|n| self.index.get(*n).copied()).collect();
        while let Some((node, cause)) = queue.pop() {
            if causes.contains_key(&node) {
                continue;
            }
            causes.insert(node, cause);
            if by_name.contains(&node) && !all {
                all = true;
                for (h, v) in pending_set.drain() {
                    queue.extend(v.into_iter().map(|(t, f)| (f, Cause::Tabela(t, h))));
                }
            }
            for &h in &self.calls[node as usize] {
                if let std::collections::hash_map::Entry::Vacant(e) = callees.entry(h) {
                    e.insert(Some(node));
                    if let Some(v) = pending_set.remove(&h) {
                        queue.extend(v.into_iter().map(|(t, f)| (f, Cause::Tabela(t, h))));
                    }
                }
            }
            if let Some((_, pairs)) = self.tables.get(&node) {
                for &(h, f) in pairs {
                    if all || callees.contains_key(&h) {
                        queue.push((f, Cause::Tabela(node, h)));
                    } else {
                        pending_set.entry(h).or_default().push((node, f));
                    }
                }
            }
            queue.extend(self.refs[node as usize].iter().map(|&r| (r, Cause::Ref(node))));
        }
        Reach { names: self.names, causes, callees, all_pairs: all, strings: self.strings }
    }
}

/// O ponto fixo sobre os resumos (§3.5), para quem quer só o alcance.
pub fn reach(summaries: &[&Summary], prune: bool) -> Reach {
    Graph::de(summaries).reachability(prune)
}

/// Números da poda (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Statistics {
    pub tables: usize,
    pub live_tables: usize,
    pub pairs: usize,
    pub live_pairs: usize,
    pub nodes: usize,
    pub live_nodes: usize,
    /// Definições do programa tiradas antes da geração (§3.12).
    pub pruned_functions: usize,
    pub pruned_globals: usize,
    pub pruned_bytes: usize,
    pub time: std::time::Duration,
}

/// O IR do programa com as tabelas montadas.
pub struct Assembly {
    pub ir: String,
    pub statistics: Statistics,
    pub reachability: Reach,
}

/// Monta as tabelas de métodos no IR do programa (§3.8): as do programa
/// reescritas só com os pares vivos, e as dos resumos do SDK definidas,
/// podadas, com a função que devolve cada uma. `prune = false`: todos os
/// pares (`DARTFORGE_SEM_PODA_DE_TABELAS`).
///
/// # Erros
/// Uma linha de tabela do programa que não se lê (formato fora do de
/// [`table_line`]).
pub fn assemble(ir: &str, summaries: &[&Summary], prune: bool) -> Result<Assembly, String> {
    let t0 = std::time::Instant::now();
    const PROGRAM: &str = "programa";
    let program = summarize(ir, PROGRAM, &[]);
    let mut all: Vec<&Summary> = summaries.to_vec();
    all.push(&program);
    let reachability = reach(&all, prune);
    let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let mut st = Statistics { nodes: reachability.names.len(), live_nodes: reachability.causes.len(), ..Default::default() };
    // Os pares vivos de uma tabela, escritos como no IR.
    let live_pairs = |node: Option<u32>, pairs: &[(i64, String)], st: &mut Statistics| -> Vec<(i64, String)> {
        st.tables += 1;
        st.pairs += pairs.len();
        if !node.is_some_and(|n| reachability.live(n)) {
            return Vec::new();
        }
        st.live_tables += 1;
        let v: Vec<(i64, String)> = pairs.iter().filter(|(h, _)| reachability.live_pair(*h)).cloned().collect();
        st.live_pairs += v.len();
        v
    };
    // As funções das tabelas que o programa define.
    let externals: HashSet<&str> =
        summaries.iter().flat_map(|r| r.externals.iter().map(move |&e| r.names[e as usize].as_str())).collect();
    let mut declared_set: HashSet<String> = HashSet::new();
    let mut exit = String::with_capacity(ir.len() + ir.len() / 4);
    // Com a poda, as definições do programa que o ponto fixo não alcançou
    // saem já aqui (§3.12): a LTO e o `/OPT:REF` as tirariam depois, mas só
    // depois de otimizá-las e gerá-las — no new_sali/backend, a maior parte
    // do IR.
    let dead = |n: &str, local: bool| -> bool {
        if !prune {
            return false;
        }
        let key = if local { format!("{PROGRAM}#{n}") } else { n.to_string() };
        index.get(key.as_str()).is_some_and(|&node| !reachability.live(node))
    };
    let mut skipping = false;
    for l in ir.split_inclusive('\n') {
        let without_end = l.trim_end_matches(['\n', '\r']);
        if skipping {
            st.pruned_bytes += l.len();
            if without_end == "}" {
                skipping = false;
            }
            continue;
        }
        if let Some(d) = without_end.strip_prefix("declare ") {
            if let Some(n) = d.find('@').and_then(|i| cited(&d[i..]).next()) {
                if externals.contains(n) {
                    continue;
                }
                declared_set.insert(n.to_string());
            }
        } else if let Some((n, local)) = define_name(without_end) {
            if dead(n, local) {
                st.pruned_functions += 1;
                st.pruned_bytes += l.len();
                skipping = !without_end.trim_end().ends_with('}');
                continue;
            }
            declared_set.insert(n.to_string());
        } else if let Some((n, local, rest)) = global_name(without_end) {
            let table = n.starts_with("df.mt.") && n.ends_with("$d");
            if !table && dead(n, local) {
                st.pruned_globals += 1;
                st.pruned_bytes += l.len();
                continue;
            }
            declared_set.insert(n.to_string());
            if table {
                let (cid, pairs) = line_table(rest).ok_or_else(|| format!("tabela de métodos ilegível no programa: {n}"))?;
                let pairs: Vec<(i64, String)> = pairs.into_iter().map(|(h, f)| (h, llvm_name(&f))).collect();
                let key = if local { format!("{PROGRAM}#{n}") } else { n.to_string() };
                let live_set = live_pairs(index.get(key.as_str()).copied(), &pairs, &mut st);
                let prefix = &without_end[..without_end.len() - rest.len()];
                let constant = rest.find("constant ").map_or(0, |i| i + "constant ".len());
                exit.push_str(&table_line(&format!("{prefix}{}", &rest[..constant]), cid, &live_set));
                continue;
            }
        }
        exit.push_str(l);
    }
    // As tabelas dos resumos.
    exit.push_str("\n; Tabelas de métodos do SDK montadas na ligação (docs/NATIVO-PODA-DE-TABELAS.md)\n");
    let mut declare: Vec<String> = Vec::new();
    for r in summaries {
        let table_of: HashMap<u32, &SummarizedTable> = r.tables.iter().map(|t| (t.node, t)).collect();
        for &e in &r.externals {
            let function = &r.names[e as usize];
            let Some(t) = r.index.get(&format!("{function}$d")).and_then(|node| table_of.get(node)) else {
                return Err(format!("o resumo de {} cita a tabela {function} sem o conteúdo dela", r.module));
            };
            let node = &r.names[t.node as usize];
            let pairs: Vec<(i64, String)> = t.pairs.iter().map(|&(h, f)| (h, llvm_name(&r.names[f as usize]))).collect();
            let live_set = live_pairs(index.get(node.as_str()).copied(), &pairs, &mut st);
            for (_, f) in &live_set {
                let name = f.trim_start_matches('@').trim_matches('"');
                if declared_set.insert(name.to_string()) {
                    declare.push(format!("declare i64 {f}(i64, ptr, ptr)\n"));
                }
            }
            let data = llvm_name(node);
            exit.push_str(&table_line(&format!("{data} = private unnamed_addr constant "), t.cid, &live_set));
            writeln!(exit, "define ptr {}() {{\nb0:\n  ret ptr {data}\n}}", llvm_name(function)).unwrap();
        }
    }
    for d in declare {
        exit.push_str(&d);
    }
    st.time = t0.elapsed();
    Ok(Assembly { ir: exit, statistics: st, reachability })
}

/// O relatório "por que isto ficou" (§3.10): uma linha por nó vivo.
pub fn report(reachability: &Reach) -> String {
    let text = |h: i64| reachability.strings.get(&h).cloned().unwrap_or_else(|| h.to_string());
    let name = |i: u32| reachability.names[i as usize].as_str();
    let mut lines: Vec<String> = reachability
        .causes
        .iter()
        .map(|(&node, cause)| match cause {
            Cause::Root => format!("{}\traiz\t-\t-\t-", name(node)),
            Cause::Ref(p) => format!("{}\tref\t{}\t-\t-", name(node), name(*p)),
            Cause::Tabela(t, h) => {
                let who = reachability.callees.get(h).copied().flatten().map_or("raiz", name);
                format!("{}\ttabela\t{}\t{}\t{}", name(node), name(*t), text(*h), who)
            }
        })
        .collect();
    lines.sort();
    let mut s = String::from("simbolo\tcausa\tpai\tseletor\tchamador\n");
    for l in lines {
        s.push_str(&l);
        s.push('\n');
    }
    s
}

/// Os resumos do SDK de produção em `files`, lidos uma vez por processo.
///
/// # Erros
/// Arquivo ausente (um SDK de produção sem resumo) ou ilegível.
pub fn read_summaries(files: &[PathBuf]) -> Result<Vec<std::sync::Arc<Summary>>, String> {
    use std::sync::{Arc, Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Arc<Summary>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    files
        .iter()
        .map(|a| {
            if let Some(r) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(a) {
                return Ok(r.clone());
            }
            let text = std::fs::read_to_string(a).map_err(|e| {
                format!("o resumo de poda {} não se lê ({e}); apague o SDK de produção em cache para recompilá-lo", a.display())
            })?;
            let r = Arc::new(Summary::from_string(&text).map_err(|e| format!("{}: {e}", a.display()))?);
            memo.lock().unwrap_or_else(|e| e.into_inner()).insert(a.clone(), r.clone());
            Ok(r)
        })
        .collect()
}

/// A montagem da ligação de produção: lê os resumos, monta, grava o
/// relatório se `DARTFORGE_POR_QUE` pede e mostra os números com `timings`.
///
/// # Erros
/// Os de [`read_summaries`] e [`assemble`], ou o relatório que não se grava.
pub fn assemble_production(ir: &str, files: &[PathBuf], timings: bool) -> Result<String, String> {
    let summaries = read_summaries(files)?;
    let refs: Vec<&Summary> = summaries.iter().map(|r| r.as_ref()).collect();
    let prune = !std::env::var("DARTFORGE_SEM_PODA_DE_TABELAS").is_ok_and(|v| v == "1");
    let m = assemble(ir, &refs, prune)?;
    if let Some(file) = std::env::var_os("DARTFORGE_POR_QUE") {
        let file = Path::new(&file);
        std::fs::write(file, report(&m.reachability)).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    if timings {
        let e = m.statistics;
        eprintln!(
            "  Poda:      {} de {} tabelas vivas, {} de {} pares, {} de {} símbolos; fora do IR: {} funções, {} globais, {} MB ({:?}){}",
            e.live_tables,
            e.tables,
            e.live_pairs,
            e.pairs,
            e.live_nodes,
            e.nodes,
            e.pruned_functions,
            e.pruned_globals,
            e.pruned_bytes / 1_000_000,
            e.time,
            if prune { "" } else { " [sem poda]" }
        );
    }
    Ok(m.ir)
}

/// O resumo do módulo HIR do programa, antes da emissão (§3.13): o mesmo
/// grafo que [`summarize`] leria do IR que a emissão escreveria, com as
/// referências que o emissor acrescenta sozinho postas como raízes da
/// entrada (`dartforge_entry`): o `main`, os registros das bibliotecas, as
/// tabelas de métodos do programa (o registro as cita todas), o `toString`
/// e as vtables das classes, os trampolins e callbacks da FFI, os
/// ajudantes e as raízes da fonte.
pub fn hir_summary(m: &crate::hir::Module) -> Summary {
    hir_summary_with(m, None)
}

/// [`hir_summary`] com os trampolins e callbacks da FFI como raízes só
/// para as chaves de `live_ffis` (`None`: todos, §3.14).
fn hir_summary_with(m: &crate::hir::Module, live_ffis: Option<&HashSet<String>>) -> Summary {
    use crate::hir::{Constant, Instruction, Operand};
    let mut r = Summary::new("programa");
    let mut seen_selectors: HashSet<i64> = HashSet::new();
    let mut selector = |r: &mut Summary, d: &mut Definition, text: &str| {
        let h = selector_hash(text);
        d.calls.push(h);
        if seen_selectors.insert(h) {
            r.selectors.push((h, text.to_string()));
        }
        // A volta do seletor tipado (`dartforge_seletor`), como no IR.
        if let Some(without_t) = text.strip_prefix('t') {
            let h2 = selector_hash(without_t);
            d.calls.push(h2);
            if seen_selectors.insert(h2) {
                r.selectors.push((h2, without_t.to_string()));
            }
        }
    };
    for f in &m.functions {
        let mut d = Definition { name: r.id(&f.symbol), ..Default::default() };
        let mut names: Vec<&str> = Vec::new();
        // Os `Constant::Function` dos operandos (o visitante não empresta o
        // operando além da chamada).
        let mut functions: Vec<String> = Vec::new();
        let mut recipes: Vec<String> = Vec::new();
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    // As receitas RTI com tipo de função (§3.14).
                    Instruction::Const(Constant::String(t)) if t.contains("F<") => recipes.push(t.clone()),
                    Instruction::CallStatic { symbol, .. } => names.push(symbol),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => names.push(code_symbol),
                    Instruction::AllocClosureTyped { code_symbol, typed, .. } => {
                        names.push(code_symbol);
                        names.push(typed);
                    }
                    Instruction::CallRuntime { name, args, .. } => {
                        names.push(name);
                        // A alocação cita a tabela de métodos da classe
                        // (`dartforge_object_new_t`, `llvm/mod.rs`).
                        if name.starts_with("dartforge_object_new")
                            && let Some((Operand::Constant(Constant::Int(cid)), _)) = args.first()
                            && let Some(t) = m.table_functions.get(&(*cid as u32))
                        {
                            names.push(t);
                        }
                    }
                    Instruction::CallSelector { selector: s, .. } | Instruction::CallSelectorForward { selector: s, .. } => {
                        selector(&mut r, &mut d, s);
                    }
                    Instruction::FunctionTable(v) => names.extend(v.iter().map(String::as_str)),
                    _ => {}
                }
                crate::optimize::operands::operands(inst, &mut |o| match o {
                    Operand::Constant(Constant::Function(s)) => functions.push(s.clone()),
                    Operand::Constant(Constant::String(t)) if t.contains("F<") => recipes.push(t.clone()),
                    _ => {}
                });
            }
            crate::optimize::operands::terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    functions.push(s.clone());
                }
            });
        }
        d.refs = names.into_iter().chain(functions.iter().map(String::as_str)).map(|n| r.id(n)).collect();
        r.recipes.extend(recipes.into_iter().map(|t| (d.name, t)));
        r.definitions.push(d);
    }
    // As tabelas de métodos do programa (`df.mt.X` devolve `df.mt.X$d`).
    for (cid, sym_name, methods) in &m.method_tables {
        let function = r.id(sym_name);
        let node = r.id(&format!("{sym_name}$d"));
        let pairs: Vec<(i64, u32)> = table_pairs(*cid, methods)
            .into_iter()
            .map(|(h, s, f)| {
                if seen_selectors.insert(h) {
                    r.selectors.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definitions.push(Definition { name: function, refs: vec![node], calls: Vec::new() });
        r.tables.push(SummarizedTable { node, cid: i64::from(*cid), pairs });
    }
    // A entrada: o que o emissor cita sem instrução da HIR.
    let mut roots: Vec<String> = Vec::new();
    roots.extend(m.entry_symbol.iter().cloned());
    roots.extend(m.sdk_records.iter().cloned());
    roots.extend(m.record.iter().cloned());
    roots.extend(m.start_rti.iter().cloned());
    roots.extend(m.call_dart.iter().cloned());
    roots.extend(m.method_tables.iter().map(|(_, s, _)| s.clone()));
    roots.extend(m.table_functions.values().cloned());
    for c in &m.classes {
        roots.extend(c.to_string_symbol.iter().cloned());
        roots.extend(c.vtable.iter().map(|(_, s)| s.clone()));
    }
    let live_ffi = |k: &str| live_ffis.is_none_or(|v| v.contains(k));
    roots.extend(m.ffi_trampolines.iter().filter(|(k, _)| live_ffi(k)).map(|(_, s)| s.clone()));
    roots.extend(m.ffi_callbacks.iter().filter(|c| live_ffi(&c.key)).map(|c| c.body.clone()));
    roots.extend(m.helpers.iter().map(|(_, s)| s.clone()));
    roots.extend(m.source_roots.iter().cloned());
    roots.extend(m.globals.iter().map(|(_, _, s)| s.clone()));
    let mut entry = Definition { name: r.id("dartforge_entry"), ..Default::default() };
    entry.refs = roots.iter().map(|n| r.id(n)).collect();
    r.definitions.push(entry);
    r
}

/// Números da poda da HIR (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct HirPruning {
    pub functions: usize,
    pub live_functions: usize,
    pub pairs: usize,
    pub live_pairs: usize,
    /// As assinaturas da FFI que ficaram, e se a poda delas valeu (§3.14).
    pub ffi: (usize, bool),
}

/// A poda da HIR do programa antes da otimização e da emissão (§3.13): o
/// ponto fixo de [`assemble`] sobre o resumo da HIR ([`hir_summary`]) e os
/// resumos das bibliotecas do SDK, e o módulo sem as funções que ele não
/// alcança e sem os pares de tabela de seletor que ninguém chama. É a
/// decisão que a montagem da ligação tomaria depois (o mesmo grafo), sem
/// otimizar nem emitir o que vai sair.
pub fn prune_hir(m: &mut crate::hir::Module, sdk: &[&Summary]) -> HirPruning {
    // Os trampolins e callbacks da FFI (um par por assinatura nativa da
    // tabela de tipos inteira) só ficam pelas assinaturas escritas nas
    // receitas vivas (§3.14). Um trampolim vivo pode alcançar receitas
    // novas: o ponto fixo repete até as chaves pararem de crescer.
    let letters: HashMap<i64, char> = m.ffi_types.iter().copied().collect();
    let composites: HashSet<i64> = m.ffi_composites.iter().map(|c| c.rti).collect();
    let prune_ffi = !m.ffi_trampolines.is_empty() && !std::env::var("DARTFORGE_SEM_PODA_FFI").is_ok_and(|v| v == "1");
    let mut live_ffis: HashSet<String> = HashSet::new();
    let (program, reachability) = loop {
        let program = hir_summary_with(m, prune_ffi.then_some(&live_ffis));
        let mut all: Vec<&Summary> = sdk.to_vec();
        all.push(&program);
        let reachability = reach(&all, true);
        if !prune_ffi {
            break (program, reachability);
        }
        let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
        let before = live_ffis.len();
        for r in &all {
            for (node, t) in &r.recipes {
                if index.get(r.names[*node as usize].as_str()).is_some_and(|&i| reachability.live(i)) {
                    recipe_ffi_keys(t, &letters, &composites, &mut live_ffis);
                }
            }
        }
        if live_ffis.len() == before {
            break (program, reachability);
        }
    };
    drop(program);
    if prune_ffi {
        m.ffi_trampolines.retain(|(k, _)| live_ffis.contains(k));
        m.ffi_callbacks.retain(|c| live_ffis.contains(&c.key));
    }
    let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let live = |n: &str| index.get(n).is_none_or(|&i| reachability.live(i));
    let mut e = HirPruning { functions: m.functions.len(), ffi: (live_ffis.len(), prune_ffi), ..Default::default() };
    let mut removed_set: HashSet<String> = HashSet::new();
    m.functions.retain(|f| {
        let stays = live(&f.symbol) || f.symbol.starts_with("dartforge_");
        if !stays {
            removed_set.insert(f.symbol.clone());
        }
        stays
    });
    e.live_functions = m.functions.len();
    m.dart_scalar_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.dart_rti_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.dart_vector_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.arc_uniform_returns.retain(|sym_name| !removed_set.contains(sym_name));
    m.dart_ref_returns.retain(|sym_name| !removed_set.contains(sym_name));
    m.arc_direct_local_functions.retain(|sym_name| !removed_set.contains(sym_name));
    m.arc_sites.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.arc_shared_functions.retain(|sym_name| !removed_set.contains(sym_name));
    // O par de tabela cujo seletor ninguém chama sai, e o da função que saiu
    // também (a entrada de um par vivo é sempre alcançada).
    for (_, _, pairs) in &mut m.method_tables {
        e.pairs += pairs.len();
        pairs.retain(|(s, f)| reachability.live_pair(selector_hash(s)) && !removed_set.contains(f));
        e.live_pairs += pairs.len();
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> i64 {
        selector_hash(s)
    }

    /// Um SDK de brinquedo: a classe `C` tem `used` e `dead`; a tabela é
    /// externa, e a função `allocates` cita a função da tabela.
    fn sdk() -> Summary {
        let ir = String::from(
            "define i64 @C.usado$c(i64 %t, ptr %a, ptr %d) {\n  ret i64 0\n}\n\
             define i64 @C.morto$c(i64 %t, ptr %a, ptr %d) {\n  %x = call i64 @C.morto(i64 %t)\n  ret i64 %x\n}\n\
             define i64 @C.morto(i64 %t) {\n  ret i64 1\n}\n\
             define internal i64 @df.classe(i64 %h) {\n  ret i64 0\n}\n\
             define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n\
             declare ptr @df.mt.C()\n"
        );
        let methods = vec![("c:usado".to_string(), "C.usado$c".to_string()), ("c:morto".to_string(), "C.morto$c".to_string())];
        summarize(&ir, "dart:x", &[(200, "df.mt.C".to_string(), methods)])
    }

    fn program(call: &str) -> String {
        format!(
            "@df.seln.0 = private unnamed_addr constant [{} x i8] c\"{call}\"\n\
             define internal i64 @df.classe(i64 %h) {{\n  ret i64 0\n}}\n\
             define i32 @main() {{\n  %o = call i64 @aloca()\n  %f = call ptr @df.seletor(ptr %ic, i64 %o, i64 {}, ptr @df.seln.0, i64 {})\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr }} {{ i64 300, i64 1, i64 {}, ptr @P.nunca$c }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n\
             define i64 @P.nunca$c(i64 %t, ptr %a, ptr %d) {{\n  ret i64 0\n}}\n\
             declare ptr @df.mt.C()\ndeclare i64 @aloca()\n",
            call.len(),
            h(call),
            call.len(),
            h("c:nunca")
        )
    }

    #[test]
    fn cited_reads_plain_and_quoted_names() {
        let v: Vec<&str> = cited("  call void @f(ptr @\"df.s.1\", ptr @g.h$c, i64 %x) ; @").collect();
        assert_eq!(v, vec!["f", "df.s.1", "g.h$c"]);
        assert_eq!(llvm_name("df.dart$3acore.x"), "@df.dart$3acore.x");
        assert_eq!(llvm_name("a b"), "@\"a b\"");
    }

    #[test]
    fn summary_round_trip() {
        let r = sdk();
        let again = Summary::from_string(&r.to_text()).unwrap();
        assert_eq!(again.names, r.names);
        assert_eq!(again.definitions, r.definitions);
        assert_eq!(again.tables, r.tables);
        assert_eq!(again.externals, r.externals);
        assert_eq!(again.selectors, r.selectors);
        assert!(Summary::from_string("outra coisa\n").is_err());
    }

    #[test]
    fn locals_carry_module_prefix() {
        let r = sdk();
        assert!(r.names.contains(&"dart:x#df.classe".to_string()));
        assert!(!r.names.contains(&"df.classe".to_string()));
    }

    #[test]
    fn typed_selector_also_calls_untyped() {
        let ir = program("tc:usado");
        let r = summarize(&ir, "p", &[]);
        let main = r.definitions.iter().find(|d| r.names[d.name as usize] == "main").unwrap();
        assert!(main.calls.contains(&h("tc:usado")));
        assert!(main.calls.contains(&h("c:usado")));
    }

    #[test]
    fn only_the_called_selector_pair_stays() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        assert!(ir.contains("ptr @C.usado$c"), "{ir}");
        assert!(!ir.contains("ptr @C.morto$c"), "{ir}");
        assert!(ir.contains("declare i64 @C.usado$c(i64, ptr, ptr)"));
        assert!(ir.contains("define ptr @df.mt.C()"));
        assert!(!ir.contains("declare ptr @df.mt.C()"), "a declaração da função da tabela sai: {ir}");
        // Ninguém cita `df.mt.P`: a tabela do programa sai vazia.
        assert!(ir.contains("@df.mt.P$d = private unnamed_addr constant { i64, i64 } { i64 300, i64 0 }"), "{ir}");
        assert!(!m.reachability.live(m.reachability.names.iter().position(|n| n == "C.morto").unwrap() as u32));
        let e = m.statistics;
        assert_eq!((e.tables, e.live_tables, e.pairs, e.live_pairs), (2, 1, 3, 1));
    }

    #[test]
    fn compact_call_reads_selector_from_descriptor() {
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"tc:usado\"\n\
             @df.seld.0 = private unnamed_addr constant {{ i64, ptr, i64 }} {{ i64 {}, ptr @df.seln.0, i64 8 }}\n\
             define i32 @main() {{\n  %f = call ptr @df.seletor_d(ptr %ic, i64 %o, ptr @df.seld.0)\n  ret i32 0\n}}\n",
            h("tc:usado")
        );
        let r = summarize(&ir, "programa", &[]);
        let main = r.definitions.iter().find(|d| r.names[d.name as usize] == "main").unwrap();
        assert!(main.calls.contains(&h("tc:usado")), "{:?}", main.calls);
        // A volta do seletor tipado.
        assert!(main.calls.contains(&h("c:usado")), "{:?}", main.calls);
        // O descritor é citado (fica vivo com quem chama).
        assert!(main.refs.iter().any(|&i| r.names[i as usize] == "programa#df.seld.0"));
    }

    /// Um módulo HIR: `main` chama `f` e o seletor `c:used`; `g` ninguém
    /// cita; a classe 300 tem os pares `c:used` e `c:dead`.
    fn hir_module() -> crate::hir::Module {
        use crate::hir::*;
        let function = |name: &str, instructions: Vec<(ValueId, Instruction, Type)>| Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: Vec::new(),
            return_ty: Type::Void,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: instructions, terminator: Terminator::Return(None) }],
        };
        let mut m = Module::new();
        m.functions.push(function(
            "dart_main",
            vec![
                (ValueId(0), Instruction::CallStatic { symbol: "f".into(), args: Vec::new(), ret_ty: Type::Void }, Type::Void),
                (
                    ValueId(1),
                    Instruction::CallSelector {
                        selector: "c:usado".into(),
                        recv: Operand::Constant(Constant::Null),
                        args: Vec::new(),
                        names: Vec::new(),
                        tuple_types: Operand::Constant(Constant::Int(0)),
                    },
                    Type::Ref,
                ),
            ],
        ));
        m.functions.push(function("f", Vec::new()));
        m.functions.push(function("g", Vec::new()));
        m.functions.push(function("P.usado$c", Vec::new()));
        m.functions.push(function("P.morto$c", Vec::new()));
        m.entry_symbol = Some("dart_main".into());
        m.sdk_records = vec!["df.registrar.core".into()];
        m.method_tables = vec![(
            300,
            "df.mt.P".into(),
            vec![("c:usado".into(), "P.usado$c".into()), ("c:morto".into(), "P.morto$c".into())],
        )];
        m
    }

    #[test]
    fn hir_pruning_removes_unreachable() {
        let mut m = hir_module();
        m.dart_scalar_parameters.insert("f".into(), HashSet::new());
        m.dart_scalar_parameters.insert("g".into(), HashSet::new());
        m.dart_rti_parameters.insert("f".into(), HashSet::new());
        m.dart_rti_parameters.insert("g".into(), HashSet::new());
        m.dart_vector_parameters.insert("f".into(), Default::default());
        m.dart_vector_parameters.insert("g".into(), Default::default());
        m.arc_uniform_returns.extend(["f".into(), "g".into()]);
        m.arc_shared_functions.extend(["f".into(), "g".into()]);
        let e = prune_hir(&mut m, &[]);
        assert!(m.dart_scalar_parameters.contains_key("f"));
        assert!(!m.dart_scalar_parameters.contains_key("g"));
        assert!(m.dart_rti_parameters.contains_key("f"));
        assert!(!m.dart_rti_parameters.contains_key("g"));
        assert!(m.dart_vector_parameters.contains_key("f"));
        assert!(!m.dart_vector_parameters.contains_key("g"));
        assert!(m.arc_uniform_returns.contains("f"));
        assert!(!m.arc_uniform_returns.contains("g"));
        assert!(m.arc_shared_functions.contains("f"));
        assert!(!m.arc_shared_functions.contains("g"));
        let names: Vec<&str> = m.functions.iter().map(|f| f.symbol.as_str()).collect();
        assert_eq!(names, vec!["dart_main", "f", "P.usado$c"], "{e:?}");
        assert_eq!(m.method_tables[0].2, vec![("c:usado".to_string(), "P.usado$c".to_string())]);
        assert_eq!((e.functions, e.live_functions, e.pairs, e.live_pairs), (5, 3, 2, 1));
        let f = m.functions[1].clone();
        let mut dead_sdk = f.clone();
        dead_sdk.symbol = "df.dart$3acore.morto".into();
        m.functions = vec![f, dead_sdk];
        m.dart_scalar_parameters.insert("df.dart$3acore.morto".into(), HashSet::new());
        m.dart_rti_parameters.insert("df.dart$3acore.morto".into(), HashSet::new());
        m.dart_vector_parameters.insert("df.dart$3acore.morto".into(), Default::default());
        m.arc_uniform_returns.insert("df.dart$3acore.morto".into());
        m.arc_shared_functions.insert("df.dart$3acore.morto".into());
        crate::source::prune(&mut m);
        assert!(m.dart_scalar_parameters.contains_key("f"));
        assert!(!m.dart_scalar_parameters.contains_key("df.dart$3acore.morto"));
        assert!(!m.dart_rti_parameters.contains_key("df.dart$3acore.morto"));
        assert!(m.dart_vector_parameters.contains_key("f"));
        assert!(!m.dart_vector_parameters.contains_key("df.dart$3acore.morto"));
        assert!(m.arc_uniform_returns.contains("f"));
        assert!(!m.arc_uniform_returns.contains("df.dart$3acore.morto"));
        assert!(!m.arc_shared_functions.contains("df.dart$3acore.morto"));
    }

    #[test]
    fn hir_pruning_sees_selector_called_by_sdk() {
        // O SDK chama `c:dead` numa função que o registro dele alcança.
        let ir = "define void @df.registrar.core() {\n  call void @x()\n  ret void\n}\n\
                  define void @x() {\n  %f = call ptr @df.seletor(ptr %ic, i64 %o, i64 HASH, ptr @df.seln.0, i64 7)\n  ret void\n}\n\
                  @df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:morto\"\n"
            .replace("HASH", &h("c:morto").to_string());
        let sdk = summarize(&ir, "dart:core", &[]);
        let mut m = hir_module();
        prune_hir(&mut m, &[&sdk]);
        assert!(m.functions.iter().any(|f| f.symbol == "P.morto$c"));
        assert_eq!(m.method_tables[0].2.len(), 2);
        assert!(!m.functions.iter().any(|f| f.symbol == "g"));
    }

    #[test]
    fn dead_program_definitions_leave_the_ir() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        // `df.mt.P` (ninguém cita) e o par dela saem; `main` e o `df.class`
        // local não citado também: só o que o ponto fixo alcança fica.
        assert!(!ir.contains("define ptr @df.mt.P()"), "{ir}");
        assert!(!ir.contains("define i64 @P.nunca$c"), "{ir}");
        assert!(!ir.contains("define internal i64 @df.classe"), "{ir}");
        assert!(ir.contains("define i32 @main()"), "{ir}");
        assert!(ir.contains("@df.seln.0 = private"), "o texto do seletor que main cita fica: {ir}");
        let e = m.statistics;
        assert_eq!(e.pruned_functions, 3, "{ir}");
        // Sem a poda, nada sai.
        let m = assemble(&program("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("define i64 @P.nunca$c"));
        assert_eq!(m.statistics.pruned_functions, 0);
    }

    #[test]
    fn without_pruning_all_live_table_pairs() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("ptr @C.morto$c"));
    }

    #[test]
    fn root_selector_keeps_the_pair() {
        let methods = vec![("c:call".to_string(), "C.call$c".to_string())];
        let ir = "define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n";
        let s = summarize(ir, "dart:x", &[(200, "df.mt.C".to_string(), methods)]);
        let m = assemble(&program("c:outro"), &[&s], true).unwrap();
        assert!(m.ir.contains("ptr @C.call$c"));
    }

    #[test]
    fn report_tells_the_cause() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let r = report(&m.reachability);
        assert!(r.contains("C.usado$c\ttabela\tdf.mt.C$d\tc:usado\tmain\n"), "{r}");
        assert!(r.contains("main\traiz\t"), "{r}");
        assert!(r.contains("aloca\tref\tmain\t"), "{r}");
    }

    #[test]
    fn program_table_rewritten_with_only_live_pairs() {
        let mut pairs = [(h("c:vivo"), "@P.vivo$c"), (h("c:morto"), "@P.morto$c")];
        pairs.sort();
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:vivo\"\n\
             define i32 @main() {{\n  %p = call ptr @df.mt.P()\n  %f = call ptr @df.seletor(ptr %ic, i64 0, i64 {}, ptr @df.seln.0, i64 6)\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr, i64, ptr }} {{ i64 300, i64 2, i64 {}, ptr {}, i64 {}, ptr {} }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n",
            h("c:vivo"),
            pairs[0].0,
            pairs[0].1,
            pairs[1].0,
            pairs[1].1,
        );
        let m = assemble(&ir, &[], true).unwrap();
        assert!(m.ir.contains("ptr @P.vivo$c"));
        assert!(!m.ir.contains("@P.morto$c"));
        assert!(m.ir.contains("{ i64, i64, i64, ptr } { i64 300, i64 1,"));
    }
}

#[cfg(test)]
mod ffi_tests {
    use super::*;

    fn keys(text: &str) -> Vec<String> {
        let letters: HashMap<i64, char> = [(703, 'i'), (719, 'p'), (737, 'v'), (736, '*'), (698, 'd')].into_iter().collect();
        let composites: HashSet<i64> = [900].into_iter().collect();
        let mut s = HashSet::new();
        recipe_ffi_keys(text, &letters, &composites, &mut s);
        let mut v: Vec<String> = s.into_iter().collect();
        v.sort();
        v
    }

    #[test]
    fn signature_key_in_recipe() {
        assert_eq!(keys("F<0;C703;2;C703,C719<C900>;>"), vec!["i_ip"]);
        // Dentro de outra receita (a tupla de `asFunction`, `NativeFunction<F>`).
        assert_eq!(keys("L<C597<F<0;C737;0;;>>,F<0;D;0;;>>"), vec!["v_"]);
        // Struct por valor e variádicas.
        assert_eq!(keys("F<0;C900;1;C719?;>"), vec!["S900._p"]);
        assert_eq!(keys("F<0;C703;2;C719,C736<R<C703,C698;>>;>"), vec!["i_p*id"]);
        assert_eq!(keys("F<0;C703;2;C719,C736<C698>;>"), vec!["i_p*d"]);
        // Não são assinaturas nativas: tipo Dart, genérica, opcional, nomeado.
        assert!(keys("F<0;D;1;C703;>").is_empty());
        assert!(keys("F<1;C703;1;B0;>").is_empty());
        assert!(keys("F<0;C703;0;C703;>").is_empty());
        assert!(keys("F<0;C703;0;;x:C703>").is_empty());
    }

    #[test]
    fn ir_recipes_go_to_summary() {
        let ir = "@\"df.s.1\" = linkonce_odr constant { i64, [24 x i8] } { i64 1, [24 x i8] c\"F<0;C703;1;C719;>\\00\\00\\00\\00\\00\\00\\00\" }\n\
                  @.str.2 = private unnamed_addr constant [9 x i8] c\"F\\3C0\\3BV\\3B0\\3B\\3B\\3E\"\n\
                  define i64 @f() {\n  %a = call i64 @g(ptr @\"df.s.1\", ptr @.str.2)\n  ret i64 %a\n}\n";
        let r = summarize(ir, "m", &[]);
        let strings: Vec<&str> = r.recipes.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(strings, vec!["F<0;C703;1;C719;>", "F<0;V;0;;>"]);
        assert_eq!(r.names[r.recipes[0].0 as usize], "df.s.1");
        assert_eq!(r.names[r.recipes[1].0 as usize], "m#.str.2");
        let again = Summary::from_string(&r.to_text()).unwrap();
        assert_eq!(again.recipes, r.recipes);
    }
}
