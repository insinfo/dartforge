//! Poda das tabelas de métodos na ligação do AOT de produção
//! (docs/NATIVO-PODA-DE-TABELAS.md).
//!
//! No perfil de produção o SDK da fonte não define as tabelas de métodos das
//! classes dele: cada biblioteca grava, junto do bitcode em cache, um
//! **resumo** ([`resumir`]) com o que cada função e cada global cita, os
//! seletores que cada função chama e o conteúdo das tabelas. Na ligação do
//! programa, [`montar`] lê o IR do programa com as mesmas regras, faz um
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
const CABECALHO: &str = "dartforge-poda\t2";

/// Seletores que vivem em toda classe viva, sem chamada no IR (§3.6):
/// `c:call` o runtime procura pelo texto (`dartforge_closure_entry`, a
/// classe chamável e o `Function.apply`); os outros são o protocolo de
/// `Object` que o runtime alcança por ajudantes do SDK — baratos, porque
/// quase toda classe viva já os tem vivos.
pub const SELETORES_RAIZ: &[&str] = &["c:call", "c:toString", "c:==", "g:hashCode", "c:noSuchMethod", "g:runtimeType"];

/// Funções do runtime que procuram na tabela por um nome que só existe em
/// tempo de execução (§3.7). Citada por função viva, toda entrada de tabela
/// viva fica. Vazia: nenhum mecanismo de hoje faz isso (o `dart:mirrors`
/// nativo não invoca por nome); o próximo entra aqui.
pub const CHAMA_POR_NOME: &[&str] = &[];

/// Uma tabela de métodos como o lowering a monta (`Module::tabelas_de_metodos`):
/// o cid, a função que devolve a tabela e os pares `(seletor, entrada)`.
pub type TabelaDoModulo = (u32, String, Vec<(String, String)>);

/// O hash de um seletor (FNV-1a de 64 bits, `lower::closures::hash_nome`).
fn hash_seletor(texto: &str) -> i64 {
    crate::lower::closures::hash_nome(texto)
}

/// Os pares `(hash, seletor, entrada)` de uma tabela de métodos, ordenados
/// pelo hash e sem hash repetido: a ordem da busca binária do runtime
/// (`dartforge_registrar_tabela`).
///
/// # Panics
/// Dois seletores diferentes com o mesmo hash na mesma classe.
pub fn pares_da_tabela(cid: u32, metodos: &[(String, String)]) -> Vec<(i64, String, String)> {
    let mut pares: Vec<(i64, String, String)> =
        metodos.iter().map(|(s, f)| (hash_seletor(s), s.clone(), f.clone())).collect();
    pares.sort();
    for par in pares.windows(2) {
        assert!(
            par[0].0 != par[1].0 || par[0].1 == par[1].1,
            "hash de seletor repetido na classe {cid}: {} e {}",
            par[0].1,
            par[1].1
        );
    }
    pares.dedup_by(|a, b| a.0 == b.0);
    pares
}

/// A linha de uma tabela de métodos: `prefixo` (`@X$d = private unnamed_addr
/// constant `), os tipos e `{ cid, n, (hash, entrada)… }`. `pares` já vem na
/// ordem da tabela, com as entradas escritas como no IR (`@f`).
///
/// ```
/// use dartforge_emit_native::poda::linha_de_tabela;
/// let l = linha_de_tabela("@t$d = private unnamed_addr constant ", 7, &[(5, "@f".to_string())]);
/// assert_eq!(l, "@t$d = private unnamed_addr constant { i64, i64, i64, ptr } { i64 7, i64 1, i64 5, ptr @f }\n");
/// ```
pub fn linha_de_tabela(prefixo: &str, cid: i64, pares: &[(i64, String)]) -> String {
    let mut tipos = String::from("i64, i64");
    let mut itens = format!("i64 {cid}, i64 {}", pares.len());
    for (h, f) in pares {
        tipos.push_str(", i64, ptr");
        write!(itens, ", i64 {h}, ptr {f}").unwrap();
    }
    format!("{prefixo}{{ {tipos} }} {{ {itens} }}\n")
}

/// Um caractere de identificador LLVM sem aspas.
fn char_de_nome(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'$' | b'.' | b'_')
}

/// Um nome escrito com `@`: sem aspas se ele permite.
fn nome_llvm(nome: &str) -> String {
    let simples = !nome.is_empty()
        && !nome.as_bytes()[0].is_ascii_digit()
        && nome.bytes().all(char_de_nome);
    if simples { format!("@{nome}") } else { format!("@\"{nome}\"") }
}

/// Os símbolos citados (`@x` e `@"x"`) em `texto`, sem o `@` e sem as aspas.
/// Um `@` dentro de uma constante de texto dá um nome que nada define — é
/// conservador, nunca tira nada.
fn citados(texto: &str) -> impl Iterator<Item = &str> {
    let b = texto.as_bytes();
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
                let fim = texto[ini..].find('"').map_or(b.len(), |k| ini + k);
                i = fim + 1;
                return Some(&texto[ini..fim]);
            }
            let ini = i;
            while i < b.len() && char_de_nome(b[i]) {
                i += 1;
            }
            if i > ini {
                return Some(&texto[ini..i]);
            }
        }
        None
    })
}

/// O nome definido por uma linha `define … @f(`, e se ele é local
/// (`internal`/`private`).
fn nome_do_define(linha: &str) -> Option<(&str, bool)> {
    let resto = linha.strip_prefix("define ")?;
    let arroba = resto.find('@')?;
    let local = resto[..arroba].split_whitespace().any(|p| p == "internal" || p == "private");
    let nome = citados(&resto[arroba..]).next()?;
    Some((nome, local))
}

/// O nome de uma linha de global `@g = …`, se ele é local, e o resto depois
/// do `=`.
fn nome_do_global(linha: &str) -> Option<(&str, bool, &str)> {
    if !linha.starts_with('@') {
        return None;
    }
    let nome = citados(linha).next()?;
    let igual = linha.find(" = ")?;
    let resto = &linha[igual + 3..];
    let local = resto.starts_with("private ") || resto.starts_with("internal ");
    Some((nome, local, resto))
}

/// Decodifica o `c"…"` de uma constante de bytes (`\XX`).
fn texto_de_constante(linha: &str) -> Option<String> {
    let ini = linha.find("c\"")? + 2;
    let fim = ini + linha[ini..].find('"')?;
    let s = &linha[ini..fim];
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
fn chamada_de_seletor(linha: &str) -> Option<(i64, &str)> {
    let i = linha.find("@df.seletor(")? + "@df.seletor(".len();
    let mut args = linha[i..].split(", ");
    args.next()?;
    args.next()?;
    let h: i64 = args.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let nome = args.next()?.strip_prefix("ptr @")?;
    Some((h, nome.trim_end_matches(')')))
}

/// O hash e o nome (`@df.seln.<k>`) do descritor `@df.seld.<k> = …
/// { i64 <hash>, ptr @df.seln.<k>, i64 <len> }` (o despacho compacto,
/// `llvm/seletores.rs`).
fn descritor_de_seletor(resto: &str) -> Option<(i64, &str)> {
    let valores = resto[resto.find("} {")? + 3..].trim_start();
    let mut it = valores.split(", ");
    let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let nome = it.next()?.trim().strip_prefix("ptr @")?;
    Some((h, nome))
}

/// O descritor da chamada `@df.seletor_d(ptr …, i64 …, ptr @df.seld.<k>)`.
fn chamada_de_seletor_compacta(linha: &str) -> Option<&str> {
    let i = linha.find("@df.seletor_d(")? + "@df.seletor_d(".len();
    let mut args = linha[i..].split(", ");
    args.next()?;
    args.next()?;
    let d = args.next()?.strip_prefix("ptr @")?;
    Some(d.trim_end_matches(')'))
}

/// Uma tabela de métodos lida de uma linha `@X$d = … constant { … } { i64
/// cid, i64 n, i64 h, ptr @f, … }`: `(cid, [(hash, entrada)])`.
fn tabela_da_linha(resto: &str) -> Option<(i64, Vec<(i64, String)>)> {
    let valores = &resto[resto.find("} {")? + 3..];
    let valores = valores.trim_end().strip_suffix('}')?.trim();
    let mut it = valores.split(", ");
    let cid: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let n: usize = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let mut pares = Vec::with_capacity(n);
    for _ in 0..n {
        let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
        let f = it.next()?.strip_prefix("ptr ")?.trim();
        pares.push((h, citados(f).next()?.to_string()));
    }
    Some((cid, pares))
}

/// Uma definição do resumo: o nome (índice), o que ela cita e os seletores
/// que ela chama.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Definicao {
    pub nome: u32,
    pub refs: Vec<u32>,
    pub chama: Vec<i64>,
}

/// Uma tabela de métodos do resumo: o nó-tabela (`X$d`, índice), o cid e os
/// pares `(hash, entrada)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabelaResumida {
    pub no: u32,
    pub cid: i64,
    pub pares: Vec<(i64, u32)>,
}

/// O resumo de um módulo (uma biblioteca do SDK ou o programa): §3.3.
///
/// Os nomes locais (`private`/`internal`) levam o prefixo `<módulo>#`,
/// porque se repetem em todo módulo (`@df.seln.0`, `@df.classe`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resumo {
    pub modulo: String,
    pub nomes: Vec<String>,
    pub definicoes: Vec<Definicao>,
    pub tabelas: Vec<TabelaResumida>,
    /// Os textos dos seletores (para o relatório).
    pub seletores: Vec<(i64, String)>,
    /// As tabelas cuja função é deste resumo, mas que o módulo não define
    /// (o SDK de produção): o programa as define (`montar`). Índices do nó
    /// da função.
    pub externas: Vec<u32>,
    /// As receitas RTI com um tipo de função (`F<…>`) e o nó que as tem (a
    /// função, ou a constante do texto no IR): de onde saem as assinaturas
    /// nativas que o programa pode pedir ao runtime (§3.14).
    pub receitas: Vec<(u32, String)>,
    indice: HashMap<String, u32>,
}

impl Resumo {
    fn novo(modulo: &str) -> Resumo {
        Resumo { modulo: modulo.to_string(), ..Default::default() }
    }

    fn id(&mut self, nome: &str) -> u32 {
        if let Some(&i) = self.indice.get(nome) {
            return i;
        }
        let i = self.nomes.len() as u32;
        self.nomes.push(nome.to_string());
        self.indice.insert(nome.to_string(), i);
        i
    }

    /// O texto do resumo (o formato de §3.3).
    pub fn para_texto(&self) -> String {
        let mut s = format!("{CABECALHO}\t{}\n", self.modulo);
        for n in &self.nomes {
            writeln!(s, "N\t{n}").unwrap();
        }
        let juntar = |v: &mut dyn Iterator<Item = String>| v.collect::<Vec<_>>().join(" ");
        for d in &self.definicoes {
            writeln!(
                s,
                "D\t{}\t{}\t{}",
                d.nome,
                juntar(&mut d.refs.iter().map(u32::to_string)),
                juntar(&mut d.chama.iter().map(i64::to_string))
            )
            .unwrap();
        }
        for t in &self.tabelas {
            writeln!(s, "T\t{}\t{}\t{}", t.no, t.cid, juntar(&mut t.pares.iter().map(|(h, f)| format!("{h}:{f}")))).unwrap();
        }
        for e in &self.externas {
            writeln!(s, "X\t{e}").unwrap();
        }
        for (h, t) in &self.seletores {
            writeln!(s, "S\t{h}\t{t}").unwrap();
        }
        for (no, t) in &self.receitas {
            writeln!(s, "R\t{no}\t{t}").unwrap();
        }
        s
    }

    /// Lê o texto de [`Resumo::para_texto`].
    ///
    /// # Erros
    /// Cabeçalho de outra versão ou linha malformada.
    pub fn de_texto(texto: &str) -> Result<Resumo, String> {
        let mut linhas = texto.lines();
        let primeira = linhas.next().unwrap_or_default();
        let modulo = primeira
            .strip_prefix(CABECALHO)
            .and_then(|r| r.strip_prefix('\t'))
            .ok_or_else(|| format!("resumo de poda com cabeçalho desconhecido: {primeira:?}"))?;
        let mut r = Resumo::novo(modulo);
        let ruim = |l: &str| format!("resumo de poda malformado: {l:?}");
        let numeros = |c: Option<&str>| -> Result<Vec<u32>, String> {
            c.unwrap_or_default().split(' ').filter(|x| !x.is_empty()).map(|x| x.parse().map_err(|_| ruim(x))).collect()
        };
        for l in linhas {
            let mut c = l.splitn(4, '\t');
            match c.next() {
                Some("N") => {
                    let n = c.next().ok_or_else(|| ruim(l))?;
                    r.indice.insert(n.to_string(), r.nomes.len() as u32);
                    r.nomes.push(n.to_string());
                }
                Some("D") => {
                    let nome = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?;
                    let refs = numeros(c.next())?;
                    let chama = c
                        .next()
                        .unwrap_or_default()
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse().map_err(|_| ruim(l)))
                        .collect::<Result<_, _>>()?;
                    r.definicoes.push(Definicao { nome, refs, chama });
                }
                Some("T") => {
                    let no = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?;
                    let cid = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?;
                    let mut pares = Vec::new();
                    for p in c.next().unwrap_or_default().split(' ').filter(|x| !x.is_empty()) {
                        let (h, f) = p.split_once(':').ok_or_else(|| ruim(l))?;
                        pares.push((h.parse().map_err(|_| ruim(l))?, f.parse().map_err(|_| ruim(l))?));
                    }
                    r.tabelas.push(TabelaResumida { no, cid, pares });
                }
                Some("X") => r.externas.push(c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?),
                Some("S") => {
                    let h = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?;
                    r.seletores.push((h, c.next().unwrap_or_default().to_string()));
                }
                Some("R") => {
                    let no = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| ruim(l))?;
                    r.receitas.push((no, c.next().unwrap_or_default().to_string()));
                }
                _ if l.is_empty() => {}
                _ => return Err(ruim(l)),
            }
        }
        let n = r.nomes.len() as u32;
        let fora = r.definicoes.iter().any(|d| d.nome >= n || d.refs.iter().any(|&x| x >= n))
            || r.tabelas.iter().any(|t| t.no >= n || t.pares.iter().any(|&(_, f)| f >= n))
            || r.externas.iter().any(|&e| e >= n);
        if fora {
            return Err("resumo de poda com índice fora da tabela de nomes".to_string());
        }
        Ok(r)
    }
}

/// O resumo de um módulo a partir do IR dele (§3.3). `modulo` é o prefixo
/// dos nomes locais; `externas` são as tabelas que o módulo cita, mas não
/// define (o SDK de produção, com `com_tabelas_na_ligacao`): `(cid,
/// função da tabela, métodos)`, como `Module::tabelas_de_metodos`.
///
/// ```
/// use dartforge_emit_native::poda::resumir;
/// let ir = "define i64 @f(i64 %x) {\n  %a = call i64 @g(i64 %x)\n  ret i64 %a\n}\n";
/// let r = resumir(ir, "m", &[]);
/// assert_eq!(r.nomes, vec!["f".to_string(), "g".to_string()]);
/// assert_eq!(r.definicoes[0].refs, vec![1]);
/// ```
pub fn resumir(ir: &str, modulo: &str, externas: &[TabelaDoModulo]) -> Resumo {
    // Passo 1: os nomes locais e os textos dos seletores.
    let mut locais: HashSet<&str> = HashSet::new();
    let mut textos_seln: HashMap<&str, String> = HashMap::new();
    let mut descritores: HashMap<&str, (i64, &str)> = HashMap::new();
    let mut receitas: Vec<(&str, String)> = Vec::new();
    for l in ir.lines() {
        if let Some((n, true)) = nome_do_define(l) {
            locais.insert(n);
        } else if let Some((n, local, resto)) = nome_do_global(l) {
            if local {
                locais.insert(n);
            }
            if n.starts_with("df.seln.")
                && let Some(t) = texto_de_constante(l)
            {
                textos_seln.insert(n, t);
            } else if n.starts_with("df.seld.")
                && let Some(d) = descritor_de_seletor(resto)
            {
                descritores.insert(n, d);
            } else if (resto.contains("F\\3C") || resto.contains("F<"))
                && let Some(t) = texto_de_constante(l)
                && t.contains("F<")
            {
                // `F<` no IR: `F\3C` nos textos privados
                // (`emit_string_constants`), `F<` nos objetos estáticos.
                receitas.push((n, t.trim_end_matches('\0').to_string()));
            }
        }
    }
    let mut r = Resumo::novo(modulo);
    let chave = |n: &str| if locais.contains(n) { format!("{modulo}#{n}") } else { n.to_string() };
    let mut vistos_seletores: HashSet<i64> = HashSet::new();
    // Passo 2: as definições.
    let mut atual: Option<Definicao> = None;
    for l in ir.lines() {
        if let Some(d) = atual.as_mut() {
            if l == "}" {
                r.definicoes.push(atual.take().expect("definição corrente"));
                continue;
            }
            let chamada = if l.contains("@df.seletor(") {
                chamada_de_seletor(l)
            } else if l.contains("@df.seletor_d(") {
                chamada_de_seletor_compacta(l).and_then(|d| descritores.get(d).copied())
            } else {
                None
            };
            if let Some((h, seln)) = chamada {
                d.chama.push(h);
                if let Some(t) = textos_seln.get(seln) {
                    if vistos_seletores.insert(h) {
                        r.seletores.push((h, t.clone()));
                    }
                    // A volta do seletor tipado (`dartforge_seletor`).
                    if let Some(sem_t) = t.strip_prefix('t') {
                        let h2 = hash_seletor(sem_t);
                        d.chama.push(h2);
                        if vistos_seletores.insert(h2) {
                            r.seletores.push((h2, sem_t.to_string()));
                        }
                    }
                }
            }
            if l.contains('@') {
                let refs: Vec<String> = citados(l).map(chave).collect();
                for x in refs {
                    let i = r.id(&x);
                    d.refs.push(i);
                }
            }
            continue;
        }
        if let Some((n, _)) = nome_do_define(l) {
            let k = chave(n);
            let nome = r.id(&k);
            let depois = &l[l.find('(').unwrap_or(l.len())..];
            let refs: Vec<String> = citados(depois).map(chave).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            if l.trim_end().ends_with('}') {
                // Uma função numa linha só.
                r.definicoes.push(Definicao { nome, refs, chama: Vec::new() });
            } else {
                atual = Some(Definicao { nome, refs, chama: Vec::new() });
            }
            continue;
        }
        if let Some((n, _, resto)) = nome_do_global(l) {
            let k = chave(n);
            let no = r.id(&k);
            if n.starts_with("df.mt.")
                && n.ends_with("$d")
                && let Some((cid, pares)) = tabela_da_linha(resto)
            {
                let pares = pares.iter().map(|(h, f)| (*h, r.id(&chave(f)))).collect();
                r.tabelas.push(TabelaResumida { no, cid, pares });
                continue;
            }
            let refs: Vec<String> = citados(resto).map(chave).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            r.definicoes.push(Definicao { nome: no, refs, chama: Vec::new() });
        }
    }
    for (cid, simbolo, metodos) in externas {
        let funcao = r.id(simbolo);
        let no = r.id(&format!("{simbolo}$d"));
        let pares: Vec<(i64, u32)> = pares_da_tabela(*cid, metodos)
            .into_iter()
            .map(|(h, s, f)| {
                if vistos_seletores.insert(h) {
                    r.seletores.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definicoes.push(Definicao { nome: funcao, refs: vec![no], chama: Vec::new() });
        r.tabelas.push(TabelaResumida { no, cid: i64::from(*cid), pares });
        r.externas.push(funcao);
    }
    for (n, t) in receitas {
        let no = r.id(&chave(n));
        r.receitas.push((no, t));
    }
    r
}

/// O fim do termo de receita RTI que começa em `i` (letra, dígitos, `<…>`,
/// `?`; `lower/rti.rs`, `escrever_tipo`).
fn fim_do_termo(b: &[u8], mut i: usize) -> Option<usize> {
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

/// Os termos separados por vírgula a partir de `i`, até `fim` (que fica na
/// posição devolvida).
fn termos_ate(b: &[u8], mut i: usize, fim: u8) -> Option<(Vec<(usize, usize)>, usize)> {
    let mut v = Vec::new();
    if b.get(i) == Some(&fim) {
        return Some((v, i));
    }
    loop {
        let f = fim_do_termo(b, i)?;
        v.push((i, f));
        match *b.get(f)? {
            b',' => i = f + 1,
            c if c == fim => return Some((v, f)),
            _ => return None,
        }
    }
}

/// O número decimal em `i` e a posição depois dele.
fn numero_em(b: &[u8], mut i: usize) -> Option<(i64, usize)> {
    let ini = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    std::str::from_utf8(&b[ini..i]).ok()?.parse().ok().map(|n| (n, i))
}

/// A letra (ou `S<rti>.`) de um tipo nativo `C<id>` na chave da FFI.
fn letra_ffi(b: &[u8], i: usize, letras: &HashMap<i64, char>, compostos: &HashSet<i64>, s: &mut String) -> Option<i64> {
    if b.get(i) != Some(&b'C') {
        return None;
    }
    let (id, _) = numero_em(b, i + 1)?;
    if compostos.contains(&id) {
        s.push_str(&format!("S{id}."));
    } else {
        s.push(*letras.get(&id)?);
    }
    Some(id)
}

/// As chaves da FFI (`lower/ffi.rs::chave_da_assinatura`, a mesma do
/// runtime, `crates/runtime/src/ffi.rs::chave_da_assinatura`) das
/// assinaturas nativas escritas na receita RTI `texto`: cada termo
/// `F<0;R;n;P1,…,Pn;>` sem parâmetros opcionais nem nomeados cujo retorno e
/// parâmetros são classes de tipo nativo (`letras`, a tabela que o runtime
/// recebe; `S<rti>.` para as structs e unions por valor, `compostos`;
/// `VarArgs`, a letra `*`, abre os tipos variádicos). O Dart exige o tipo
/// nativo constante em `asFunction`, `lookupFunction`, `fromFunction` e
/// `NativeCallable` (o transformador de FFI do front-end recusa um
/// genérico), então toda assinatura que o programa pede ao runtime está
/// escrita numa receita do sítio da chamada (§3.14).
pub fn chaves_ffi_da_receita(texto: &str, letras: &HashMap<i64, char>, compostos: &HashSet<i64>, saida: &mut HashSet<String>) {
    let b = texto.as_bytes();
    // A parte da chave do termo `b[i..f]`.
    let parte = |i: usize, f: usize, s: &mut String| -> Option<()> {
        let antes = s.len();
        let id = letra_ffi(b, i, letras, compostos, s)?;
        if !s[antes..].ends_with('*') || compostos.contains(&id) {
            return Some(());
        }
        // `VarArgs<(T1, T2…)>` ou `VarArgs<T>`: os tipos depois da marca.
        let (_, j) = numero_em(b, i + 1)?;
        if j >= f || b[j] != b'<' {
            return Some(());
        }
        let tipos: Vec<(usize, usize)> = if b.get(j + 1) == Some(&b'R') && b.get(j + 2) == Some(&b'<') {
            let (v, fim) = termos_ate(b, j + 3, b';')?;
            if b.get(fim + 1) != Some(&b'>') {
                return None;
            }
            v
        } else {
            vec![(j + 1, fim_do_termo(b, j + 1)?)]
        };
        for (a, _) in tipos {
            let antes = s.len();
            letra_ffi(b, a, letras, compostos, s)?;
            if s[antes..].ends_with('*') {
                return None;
            }
        }
        Some(())
    };
    let mut i = 0;
    while let Some(d) = texto[i..].find("F<") {
        let ini = i + d;
        i = ini + 2;
        let chave = (|| -> Option<String> {
            let (genericos, j) = numero_em(b, ini + 2)?;
            if genericos != 0 || b.get(j) != Some(&b';') {
                return None;
            }
            let r_fim = fim_do_termo(b, j + 1)?;
            if b.get(r_fim) != Some(&b';') {
                return None;
            }
            let (n, k) = numero_em(b, r_fim + 1)?;
            if b.get(k) != Some(&b';') {
                return None;
            }
            let (params, fim) = termos_ate(b, k + 1, b';')?;
            // Sem opcionais nem nomeados.
            if params.len() as i64 != n || b.get(fim + 1) != Some(&b'>') {
                return None;
            }
            let mut s = String::new();
            parte(j + 1, r_fim, &mut s)?;
            s.push('_');
            for (a, z) in params {
                parte(a, z, &mut s)?;
            }
            Some(s)
        })();
        if let Some(c) = chave {
            saida.insert(c);
        }
    }
}


/// Por que um nó está vivo (a primeira vez que ele entrou).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Causa {
    Raiz,
    /// Citado pelo nó.
    Ref(u32),
    /// Par da tabela (nó) pelo seletor.
    Tabela(u32, i64),
}

/// O resultado do ponto fixo sobre vários resumos (§3.5).
pub struct Alcance {
    pub nomes: Vec<String>,
    /// A causa de cada nó vivo.
    pub causas: HashMap<u32, Causa>,
    /// Os seletores chamados e a primeira função viva que chamou cada um
    /// (`None` nas raízes).
    pub chamados: HashMap<i64, Option<u32>>,
    /// Alguma função de [`CHAMA_POR_NOME`] ficou viva.
    pub todos_os_pares: bool,
    pub textos: HashMap<i64, String>,
}

impl Alcance {
    pub fn vivo(&self, no: u32) -> bool {
        self.causas.contains_key(&no)
    }

    pub fn par_vivo(&self, h: i64) -> bool {
        self.todos_os_pares || self.chamados.contains_key(&h)
    }
}

/// Os nós, as arestas e as tabelas de todos os resumos, com os nomes
/// unificados.
struct Grafo {
    nomes: Vec<String>,
    indice: HashMap<String, u32>,
    refs: Vec<Vec<u32>>,
    chama: Vec<Vec<i64>>,
    /// Nó-tabela → (cid, pares).
    tabelas: HashMap<u32, (i64, Vec<(i64, u32)>)>,
    textos: HashMap<i64, String>,
    /// Os nós que algum módulo define (os outros são do runtime ou de fora).
    definidos: HashSet<u32>,
}

impl Grafo {
    fn id(&mut self, nome: &str) -> u32 {
        if let Some(&i) = self.indice.get(nome) {
            return i;
        }
        let i = self.nomes.len() as u32;
        self.nomes.push(nome.to_string());
        self.indice.insert(nome.to_string(), i);
        self.refs.push(Vec::new());
        self.chama.push(Vec::new());
        i
    }

    fn de(resumos: &[&Resumo]) -> Grafo {
        let mut g = Grafo {
            nomes: Vec::new(),
            indice: HashMap::new(),
            refs: Vec::new(),
            chama: Vec::new(),
            tabelas: HashMap::new(),
            textos: HashMap::new(),
            definidos: HashSet::new(),
        };
        for r in resumos {
            let mapa: Vec<u32> = r.nomes.iter().map(|n| g.id(n)).collect();
            for d in &r.definicoes {
                let n = mapa[d.nome as usize] as usize;
                g.definidos.insert(n as u32);
                g.refs[n].extend(d.refs.iter().map(|&x| mapa[x as usize]));
                g.chama[n].extend_from_slice(&d.chama);
            }
            for t in &r.tabelas {
                let pares = t.pares.iter().map(|&(h, f)| (h, mapa[f as usize])).collect();
                g.tabelas.insert(mapa[t.no as usize], (t.cid, pares));
                g.definidos.insert(mapa[t.no as usize]);
            }
            for (h, s) in &r.seletores {
                g.textos.entry(*h).or_insert_with(|| s.clone());
            }
        }
        g
    }

    /// Uma raiz de nome (§3.6), se algum módulo a define: a entrada do
    /// processo, o que o runtime pode citar por símbolo e os globais do
    /// LLVM. (Uma função do runtime citada pelo código Dart não é raiz: o
    /// relatório diz quem a cita.)
    fn raiz(nome: &str) -> bool {
        nome == "main" || nome.starts_with("dartforge_") || nome.starts_with("llvm.")
    }

    /// O ponto fixo (§3.5). `podar = false`: todo par de tabela viva vive.
    fn alcance(self, podar: bool) -> Alcance {
        let mut causas: HashMap<u32, Causa> = HashMap::new();
        let mut chamados: HashMap<i64, Option<u32>> = HashMap::new();
        let mut pendentes: HashMap<i64, Vec<(u32, u32)>> = HashMap::new();
        let mut fila: Vec<(u32, Causa)> = Vec::new();
        let mut todos = !podar;
        for (i, n) in self.nomes.iter().enumerate() {
            if Self::raiz(n) && self.definidos.contains(&(i as u32)) {
                fila.push((i as u32, Causa::Raiz));
            }
        }
        for s in SELETORES_RAIZ {
            chamados.insert(hash_seletor(s), None);
        }
        let por_nome: HashSet<u32> = CHAMA_POR_NOME.iter().filter_map(|n| self.indice.get(*n).copied()).collect();
        while let Some((no, causa)) = fila.pop() {
            if causas.contains_key(&no) {
                continue;
            }
            causas.insert(no, causa);
            if por_nome.contains(&no) && !todos {
                todos = true;
                for (h, v) in pendentes.drain() {
                    fila.extend(v.into_iter().map(|(t, f)| (f, Causa::Tabela(t, h))));
                }
            }
            for &h in &self.chama[no as usize] {
                if let std::collections::hash_map::Entry::Vacant(e) = chamados.entry(h) {
                    e.insert(Some(no));
                    if let Some(v) = pendentes.remove(&h) {
                        fila.extend(v.into_iter().map(|(t, f)| (f, Causa::Tabela(t, h))));
                    }
                }
            }
            if let Some((_, pares)) = self.tabelas.get(&no) {
                for &(h, f) in pares {
                    if todos || chamados.contains_key(&h) {
                        fila.push((f, Causa::Tabela(no, h)));
                    } else {
                        pendentes.entry(h).or_default().push((no, f));
                    }
                }
            }
            fila.extend(self.refs[no as usize].iter().map(|&r| (r, Causa::Ref(no))));
        }
        Alcance { nomes: self.nomes, causas, chamados, todos_os_pares: todos, textos: self.textos }
    }
}

/// O ponto fixo sobre os resumos (§3.5), para quem quer só o alcance.
pub fn alcancar(resumos: &[&Resumo], podar: bool) -> Alcance {
    Grafo::de(resumos).alcance(podar)
}

/// Números da poda (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Estatisticas {
    pub tabelas: usize,
    pub tabelas_vivas: usize,
    pub pares: usize,
    pub pares_vivos: usize,
    pub nos: usize,
    pub nos_vivos: usize,
    /// Definições do programa tiradas antes da geração (§3.12).
    pub funcoes_podadas: usize,
    pub globais_podados: usize,
    pub bytes_podados: usize,
    pub tempo: std::time::Duration,
}

/// O IR do programa com as tabelas montadas.
pub struct Montagem {
    pub ir: String,
    pub estatisticas: Estatisticas,
    pub alcance: Alcance,
}

/// Monta as tabelas de métodos no IR do programa (§3.8): as do programa
/// reescritas só com os pares vivos, e as dos resumos do SDK definidas,
/// podadas, com a função que devolve cada uma. `podar = false`: todos os
/// pares (`DARTFORGE_SEM_PODA_DE_TABELAS`).
///
/// # Erros
/// Uma linha de tabela do programa que não se lê (formato fora do de
/// [`linha_de_tabela`]).
pub fn montar(ir: &str, resumos: &[&Resumo], podar: bool) -> Result<Montagem, String> {
    let t0 = std::time::Instant::now();
    const PROGRAMA: &str = "programa";
    let programa = resumir(ir, PROGRAMA, &[]);
    let mut todos: Vec<&Resumo> = resumos.to_vec();
    todos.push(&programa);
    let alcance = alcancar(&todos, podar);
    let indice: HashMap<&str, u32> = alcance.nomes.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let mut est = Estatisticas { nos: alcance.nomes.len(), nos_vivos: alcance.causas.len(), ..Default::default() };
    // Os pares vivos de uma tabela, escritos como no IR.
    let pares_vivos = |no: Option<u32>, pares: &[(i64, String)], est: &mut Estatisticas| -> Vec<(i64, String)> {
        est.tabelas += 1;
        est.pares += pares.len();
        if !no.is_some_and(|n| alcance.vivo(n)) {
            return Vec::new();
        }
        est.tabelas_vivas += 1;
        let v: Vec<(i64, String)> = pares.iter().filter(|(h, _)| alcance.par_vivo(*h)).cloned().collect();
        est.pares_vivos += v.len();
        v
    };
    // As funções das tabelas que o programa define.
    let externas: HashSet<&str> =
        resumos.iter().flat_map(|r| r.externas.iter().map(move |&e| r.nomes[e as usize].as_str())).collect();
    let mut declarados: HashSet<String> = HashSet::new();
    let mut saida = String::with_capacity(ir.len() + ir.len() / 4);
    // Com a poda, as definições do programa que o ponto fixo não alcançou
    // saem já aqui (§3.12): a LTO e o `/OPT:REF` as tirariam depois, mas só
    // depois de otimizá-las e gerá-las — no new_sali/backend, a maior parte
    // do IR.
    let morto = |n: &str, local: bool| -> bool {
        if !podar {
            return false;
        }
        let chave = if local { format!("{PROGRAMA}#{n}") } else { n.to_string() };
        indice.get(chave.as_str()).is_some_and(|&no| !alcance.vivo(no))
    };
    let mut pulando = false;
    for l in ir.split_inclusive('\n') {
        let sem_fim = l.trim_end_matches(['\n', '\r']);
        if pulando {
            est.bytes_podados += l.len();
            if sem_fim == "}" {
                pulando = false;
            }
            continue;
        }
        if let Some(d) = sem_fim.strip_prefix("declare ") {
            if let Some(n) = d.find('@').and_then(|i| citados(&d[i..]).next()) {
                if externas.contains(n) {
                    continue;
                }
                declarados.insert(n.to_string());
            }
        } else if let Some((n, local)) = nome_do_define(sem_fim) {
            if morto(n, local) {
                est.funcoes_podadas += 1;
                est.bytes_podados += l.len();
                pulando = !sem_fim.trim_end().ends_with('}');
                continue;
            }
            declarados.insert(n.to_string());
        } else if let Some((n, local, resto)) = nome_do_global(sem_fim) {
            let tabela = n.starts_with("df.mt.") && n.ends_with("$d");
            if !tabela && morto(n, local) {
                est.globais_podados += 1;
                est.bytes_podados += l.len();
                continue;
            }
            declarados.insert(n.to_string());
            if tabela {
                let (cid, pares) = tabela_da_linha(resto).ok_or_else(|| format!("tabela de métodos ilegível no programa: {n}"))?;
                let pares: Vec<(i64, String)> = pares.into_iter().map(|(h, f)| (h, nome_llvm(&f))).collect();
                let chave = if local { format!("{PROGRAMA}#{n}") } else { n.to_string() };
                let vivos = pares_vivos(indice.get(chave.as_str()).copied(), &pares, &mut est);
                let prefixo = &sem_fim[..sem_fim.len() - resto.len()];
                let constante = resto.find("constant ").map_or(0, |i| i + "constant ".len());
                saida.push_str(&linha_de_tabela(&format!("{prefixo}{}", &resto[..constante]), cid, &vivos));
                continue;
            }
        }
        saida.push_str(l);
    }
    // As tabelas dos resumos.
    saida.push_str("\n; Tabelas de métodos do SDK montadas na ligação (docs/NATIVO-PODA-DE-TABELAS.md)\n");
    let mut declarar: Vec<String> = Vec::new();
    for r in resumos {
        let tabela_de: HashMap<u32, &TabelaResumida> = r.tabelas.iter().map(|t| (t.no, t)).collect();
        for &e in &r.externas {
            let funcao = &r.nomes[e as usize];
            let Some(t) = r.indice.get(&format!("{funcao}$d")).and_then(|no| tabela_de.get(no)) else {
                return Err(format!("o resumo de {} cita a tabela {funcao} sem o conteúdo dela", r.modulo));
            };
            let no = &r.nomes[t.no as usize];
            let pares: Vec<(i64, String)> = t.pares.iter().map(|&(h, f)| (h, nome_llvm(&r.nomes[f as usize]))).collect();
            let vivos = pares_vivos(indice.get(no.as_str()).copied(), &pares, &mut est);
            for (_, f) in &vivos {
                let nome = f.trim_start_matches('@').trim_matches('"');
                if declarados.insert(nome.to_string()) {
                    declarar.push(format!("declare i64 {f}(i64, ptr, ptr)\n"));
                }
            }
            let dados = nome_llvm(no);
            saida.push_str(&linha_de_tabela(&format!("{dados} = private unnamed_addr constant "), t.cid, &vivos));
            writeln!(saida, "define ptr {}() {{\nb0:\n  ret ptr {dados}\n}}", nome_llvm(funcao)).unwrap();
        }
    }
    for d in declarar {
        saida.push_str(&d);
    }
    est.tempo = t0.elapsed();
    Ok(Montagem { ir: saida, estatisticas: est, alcance })
}

/// O relatório "por que isto ficou" (§3.10): uma linha por nó vivo.
pub fn relatorio(alcance: &Alcance) -> String {
    let texto = |h: i64| alcance.textos.get(&h).cloned().unwrap_or_else(|| h.to_string());
    let nome = |i: u32| alcance.nomes[i as usize].as_str();
    let mut linhas: Vec<String> = alcance
        .causas
        .iter()
        .map(|(&no, causa)| match causa {
            Causa::Raiz => format!("{}\traiz\t-\t-\t-", nome(no)),
            Causa::Ref(p) => format!("{}\tref\t{}\t-\t-", nome(no), nome(*p)),
            Causa::Tabela(t, h) => {
                let quem = alcance.chamados.get(h).copied().flatten().map_or("raiz", nome);
                format!("{}\ttabela\t{}\t{}\t{}", nome(no), nome(*t), texto(*h), quem)
            }
        })
        .collect();
    linhas.sort();
    let mut s = String::from("simbolo\tcausa\tpai\tseletor\tchamador\n");
    for l in linhas {
        s.push_str(&l);
        s.push('\n');
    }
    s
}

/// Os resumos do SDK de produção em `arquivos`, lidos uma vez por processo.
///
/// # Erros
/// Arquivo ausente (um SDK de produção sem resumo) ou ilegível.
pub fn ler_resumos(arquivos: &[PathBuf]) -> Result<Vec<std::sync::Arc<Resumo>>, String> {
    use std::sync::{Arc, Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Arc<Resumo>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    arquivos
        .iter()
        .map(|a| {
            if let Some(r) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(a) {
                return Ok(r.clone());
            }
            let texto = std::fs::read_to_string(a).map_err(|e| {
                format!("o resumo de poda {} não se lê ({e}); apague o SDK de produção em cache para recompilá-lo", a.display())
            })?;
            let r = Arc::new(Resumo::de_texto(&texto).map_err(|e| format!("{}: {e}", a.display()))?);
            memo.lock().unwrap_or_else(|e| e.into_inner()).insert(a.clone(), r.clone());
            Ok(r)
        })
        .collect()
}

/// A montagem da ligação de produção: lê os resumos, monta, grava o
/// relatório se `DARTFORGE_POR_QUE` pede e mostra os números com `timings`.
///
/// # Erros
/// Os de [`ler_resumos`] e [`montar`], ou o relatório que não se grava.
pub fn montar_producao(ir: &str, arquivos: &[PathBuf], timings: bool) -> Result<String, String> {
    let resumos = ler_resumos(arquivos)?;
    let refs: Vec<&Resumo> = resumos.iter().map(|r| r.as_ref()).collect();
    let podar = !std::env::var("DARTFORGE_SEM_PODA_DE_TABELAS").is_ok_and(|v| v == "1");
    let m = montar(ir, &refs, podar)?;
    if let Some(arq) = std::env::var_os("DARTFORGE_POR_QUE") {
        let arq = Path::new(&arq);
        std::fs::write(arq, relatorio(&m.alcance)).map_err(|e| format!("{}: {e}", arq.display()))?;
    }
    if timings {
        let e = m.estatisticas;
        eprintln!(
            "  Poda:      {} de {} tabelas vivas, {} de {} pares, {} de {} símbolos; fora do IR: {} funções, {} globais, {} MB ({:?}){}",
            e.tabelas_vivas,
            e.tabelas,
            e.pares_vivos,
            e.pares,
            e.nos_vivos,
            e.nos,
            e.funcoes_podadas,
            e.globais_podados,
            e.bytes_podados / 1_000_000,
            e.tempo,
            if podar { "" } else { " [sem poda]" }
        );
    }
    Ok(m.ir)
}

/// O resumo do módulo HIR do programa, antes da emissão (§3.13): o mesmo
/// grafo que [`resumir`] leria do IR que a emissão escreveria, com as
/// referências que o emissor acrescenta sozinho postas como raízes da
/// entrada (`dartforge_entry`): o `main`, os registros das bibliotecas, as
/// tabelas de métodos do programa (o registro as cita todas), o `toString`
/// e as vtables das classes, os trampolins e callbacks da FFI, os
/// ajudantes e as raízes da fonte.
pub fn resumo_da_hir(m: &crate::hir::Module) -> Resumo {
    resumo_da_hir_com(m, None)
}

/// [`resumo_da_hir`] com os trampolins e callbacks da FFI como raízes só
/// para as chaves de `ffi_vivas` (`None`: todos, §3.14).
fn resumo_da_hir_com(m: &crate::hir::Module, ffi_vivas: Option<&HashSet<String>>) -> Resumo {
    use crate::hir::{Constant, Instruction, Operand};
    let mut r = Resumo::novo("programa");
    let mut vistos_seletores: HashSet<i64> = HashSet::new();
    let mut seletor = |r: &mut Resumo, d: &mut Definicao, texto: &str| {
        let h = hash_seletor(texto);
        d.chama.push(h);
        if vistos_seletores.insert(h) {
            r.seletores.push((h, texto.to_string()));
        }
        // A volta do seletor tipado (`dartforge_seletor`), como no IR.
        if let Some(sem_t) = texto.strip_prefix('t') {
            let h2 = hash_seletor(sem_t);
            d.chama.push(h2);
            if vistos_seletores.insert(h2) {
                r.seletores.push((h2, sem_t.to_string()));
            }
        }
    };
    for f in &m.functions {
        let mut d = Definicao { nome: r.id(&f.symbol), ..Default::default() };
        let mut nomes: Vec<&str> = Vec::new();
        // Os `Constant::Funcao` dos operandos (o visitante não empresta o
        // operando além da chamada).
        let mut funcoes: Vec<String> = Vec::new();
        let mut receitas: Vec<String> = Vec::new();
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    // As receitas RTI com tipo de função (§3.14).
                    Instruction::Const(Constant::String(t)) if t.contains("F<") => receitas.push(t.clone()),
                    Instruction::CallStatic { symbol, .. } => nomes.push(symbol),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => nomes.push(code_symbol),
                    Instruction::AllocClosureTipada { code_symbol, tipado, .. } => {
                        nomes.push(code_symbol);
                        nomes.push(tipado);
                    }
                    Instruction::CallRuntime { name, args, .. } => {
                        nomes.push(name);
                        // A alocação cita a tabela de métodos da classe
                        // (`dartforge_object_new_t`, `llvm/mod.rs`).
                        if name.starts_with("dartforge_object_new")
                            && let Some((Operand::Constant(Constant::Int(cid)), _)) = args.first()
                            && let Some(t) = m.funcoes_de_tabela.get(&(*cid as u32))
                        {
                            nomes.push(t);
                        }
                    }
                    Instruction::CallSeletor { seletor: s, .. } | Instruction::CallSeletorRepasse { seletor: s, .. } => {
                        seletor(&mut r, &mut d, s);
                    }
                    Instruction::TabelaDeFuncoes(v) => nomes.extend(v.iter().map(String::as_str)),
                    _ => {}
                }
                crate::otimizar::operandos::operandos(inst, &mut |o| match o {
                    Operand::Constant(Constant::Funcao(s)) => funcoes.push(s.clone()),
                    Operand::Constant(Constant::String(t)) if t.contains("F<") => receitas.push(t.clone()),
                    _ => {}
                });
            }
            crate::otimizar::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Funcao(s)) = o {
                    funcoes.push(s.clone());
                }
            });
        }
        d.refs = nomes.into_iter().chain(funcoes.iter().map(String::as_str)).map(|n| r.id(n)).collect();
        r.receitas.extend(receitas.into_iter().map(|t| (d.nome, t)));
        r.definicoes.push(d);
    }
    // As tabelas de métodos do programa (`df.mt.X` devolve `df.mt.X$d`).
    for (cid, simbolo, metodos) in &m.tabelas_de_metodos {
        let funcao = r.id(simbolo);
        let no = r.id(&format!("{simbolo}$d"));
        let pares: Vec<(i64, u32)> = pares_da_tabela(*cid, metodos)
            .into_iter()
            .map(|(h, s, f)| {
                if vistos_seletores.insert(h) {
                    r.seletores.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definicoes.push(Definicao { nome: funcao, refs: vec![no], chama: Vec::new() });
        r.tabelas.push(TabelaResumida { no, cid: i64::from(*cid), pares });
    }
    // A entrada: o que o emissor cita sem instrução da HIR.
    let mut raizes: Vec<String> = Vec::new();
    raizes.extend(m.entry_symbol.iter().cloned());
    raizes.extend(m.registros_do_sdk.iter().cloned());
    raizes.extend(m.registro.iter().cloned());
    raizes.extend(m.iniciar_rti.iter().cloned());
    raizes.extend(m.chamar_dart.iter().cloned());
    raizes.extend(m.tabelas_de_metodos.iter().map(|(_, s, _)| s.clone()));
    raizes.extend(m.funcoes_de_tabela.values().cloned());
    for c in &m.classes {
        raizes.extend(c.to_string_symbol.iter().cloned());
        raizes.extend(c.vtable.iter().map(|(_, s)| s.clone()));
    }
    let ffi_viva = |k: &str| ffi_vivas.is_none_or(|v| v.contains(k));
    raizes.extend(m.ffi_trampolins.iter().filter(|(k, _)| ffi_viva(k)).map(|(_, s)| s.clone()));
    raizes.extend(m.ffi_callbacks.iter().filter(|c| ffi_viva(&c.chave)).map(|c| c.corpo.clone()));
    raizes.extend(m.ajudantes.iter().map(|(_, s)| s.clone()));
    raizes.extend(m.raizes_da_fonte.iter().cloned());
    raizes.extend(m.globais.iter().map(|(_, _, s)| s.clone()));
    let mut entrada = Definicao { nome: r.id("dartforge_entry"), ..Default::default() };
    entrada.refs = raizes.iter().map(|n| r.id(n)).collect();
    r.definicoes.push(entrada);
    r
}

/// Números da poda da HIR (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct PodaDaHir {
    pub funcoes: usize,
    pub funcoes_vivas: usize,
    pub pares: usize,
    pub pares_vivos: usize,
    /// As assinaturas da FFI que ficaram, e se a poda delas valeu (§3.14).
    pub ffi: (usize, bool),
}

/// A poda da HIR do programa antes da otimização e da emissão (§3.13): o
/// ponto fixo de [`montar`] sobre o resumo da HIR ([`resumo_da_hir`]) e os
/// resumos das bibliotecas do SDK, e o módulo sem as funções que ele não
/// alcança e sem os pares de tabela de seletor que ninguém chama. É a
/// decisão que a montagem da ligação tomaria depois (o mesmo grafo), sem
/// otimizar nem emitir o que vai sair.
pub fn podar_hir(m: &mut crate::hir::Module, sdk: &[&Resumo]) -> PodaDaHir {
    // Os trampolins e callbacks da FFI (um par por assinatura nativa da
    // tabela de tipos inteira) só ficam pelas assinaturas escritas nas
    // receitas vivas (§3.14). Um trampolim vivo pode alcançar receitas
    // novas: o ponto fixo repete até as chaves pararem de crescer.
    let letras: HashMap<i64, char> = m.ffi_tipos.iter().copied().collect();
    let compostos: HashSet<i64> = m.ffi_compostos.iter().map(|c| c.rti).collect();
    let podar_ffi = !m.ffi_trampolins.is_empty() && !std::env::var("DARTFORGE_SEM_PODA_FFI").is_ok_and(|v| v == "1");
    let mut ffi_vivas: HashSet<String> = HashSet::new();
    let (programa, alcance) = loop {
        let programa = resumo_da_hir_com(m, podar_ffi.then_some(&ffi_vivas));
        let mut todos: Vec<&Resumo> = sdk.to_vec();
        todos.push(&programa);
        let alcance = alcancar(&todos, true);
        if !podar_ffi {
            break (programa, alcance);
        }
        let indice: HashMap<&str, u32> = alcance.nomes.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
        let antes = ffi_vivas.len();
        for r in &todos {
            for (no, t) in &r.receitas {
                if indice.get(r.nomes[*no as usize].as_str()).is_some_and(|&i| alcance.vivo(i)) {
                    chaves_ffi_da_receita(t, &letras, &compostos, &mut ffi_vivas);
                }
            }
        }
        if ffi_vivas.len() == antes {
            break (programa, alcance);
        }
    };
    drop(programa);
    if podar_ffi {
        m.ffi_trampolins.retain(|(k, _)| ffi_vivas.contains(k));
        m.ffi_callbacks.retain(|c| ffi_vivas.contains(&c.chave));
    }
    let indice: HashMap<&str, u32> = alcance.nomes.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let vivo = |n: &str| indice.get(n).is_none_or(|&i| alcance.vivo(i));
    let mut e = PodaDaHir { funcoes: m.functions.len(), ffi: (ffi_vivas.len(), podar_ffi), ..Default::default() };
    let mut removidas: HashSet<String> = HashSet::new();
    m.functions.retain(|f| {
        let fica = vivo(&f.symbol) || f.symbol.starts_with("dartforge_");
        if !fica {
            removidas.insert(f.symbol.clone());
        }
        fica
    });
    e.funcoes_vivas = m.functions.len();
    // O par de tabela cujo seletor ninguém chama sai, e o da função que saiu
    // também (a entrada de um par vivo é sempre alcançada).
    for (_, _, pares) in &mut m.tabelas_de_metodos {
        e.pares += pares.len();
        pares.retain(|(s, f)| alcance.par_vivo(hash_seletor(s)) && !removidas.contains(f));
        e.pares_vivos += pares.len();
    }
    e
}

#[cfg(test)]
mod testes {
    use super::*;

    fn h(s: &str) -> i64 {
        hash_seletor(s)
    }

    /// Um SDK de brinquedo: a classe `C` tem `usado` e `morto`; a tabela é
    /// externa, e a função `aloca` cita a função da tabela.
    fn sdk() -> Resumo {
        let ir = String::from(
            "define i64 @C.usado$c(i64 %t, ptr %a, ptr %d) {\n  ret i64 0\n}\n\
             define i64 @C.morto$c(i64 %t, ptr %a, ptr %d) {\n  %x = call i64 @C.morto(i64 %t)\n  ret i64 %x\n}\n\
             define i64 @C.morto(i64 %t) {\n  ret i64 1\n}\n\
             define internal i64 @df.classe(i64 %h) {\n  ret i64 0\n}\n\
             define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n\
             declare ptr @df.mt.C()\n"
        );
        let metodos = vec![("c:usado".to_string(), "C.usado$c".to_string()), ("c:morto".to_string(), "C.morto$c".to_string())];
        resumir(&ir, "dart:x", &[(200, "df.mt.C".to_string(), metodos)])
    }

    fn programa(chamada: &str) -> String {
        format!(
            "@df.seln.0 = private unnamed_addr constant [{} x i8] c\"{chamada}\"\n\
             define internal i64 @df.classe(i64 %h) {{\n  ret i64 0\n}}\n\
             define i32 @main() {{\n  %o = call i64 @aloca()\n  %f = call ptr @df.seletor(ptr %ic, i64 %o, i64 {}, ptr @df.seln.0, i64 {})\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr }} {{ i64 300, i64 1, i64 {}, ptr @P.nunca$c }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n\
             define i64 @P.nunca$c(i64 %t, ptr %a, ptr %d) {{\n  ret i64 0\n}}\n\
             declare ptr @df.mt.C()\ndeclare i64 @aloca()\n",
            chamada.len(),
            h(chamada),
            chamada.len(),
            h("c:nunca")
        )
    }

    #[test]
    fn citados_le_nomes_simples_e_com_aspas() {
        let v: Vec<&str> = citados("  call void @f(ptr @\"df.s.1\", ptr @g.h$c, i64 %x) ; @").collect();
        assert_eq!(v, vec!["f", "df.s.1", "g.h$c"]);
        assert_eq!(nome_llvm("df.dart$3acore.x"), "@df.dart$3acore.x");
        assert_eq!(nome_llvm("a b"), "@\"a b\"");
    }

    #[test]
    fn resumo_ida_e_volta() {
        let r = sdk();
        let de_novo = Resumo::de_texto(&r.para_texto()).unwrap();
        assert_eq!(de_novo.nomes, r.nomes);
        assert_eq!(de_novo.definicoes, r.definicoes);
        assert_eq!(de_novo.tabelas, r.tabelas);
        assert_eq!(de_novo.externas, r.externas);
        assert_eq!(de_novo.seletores, r.seletores);
        assert!(Resumo::de_texto("outra coisa\n").is_err());
    }

    #[test]
    fn locais_levam_o_prefixo_do_modulo() {
        let r = sdk();
        assert!(r.nomes.contains(&"dart:x#df.classe".to_string()));
        assert!(!r.nomes.contains(&"df.classe".to_string()));
    }

    #[test]
    fn seletor_tipado_chama_tambem_o_sem_t() {
        let ir = programa("tc:usado");
        let r = resumir(&ir, "p", &[]);
        let main = r.definicoes.iter().find(|d| r.nomes[d.nome as usize] == "main").unwrap();
        assert!(main.chama.contains(&h("tc:usado")));
        assert!(main.chama.contains(&h("c:usado")));
    }

    #[test]
    fn so_o_par_do_seletor_chamado_fica() {
        let s = sdk();
        let m = montar(&programa("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        assert!(ir.contains("ptr @C.usado$c"), "{ir}");
        assert!(!ir.contains("ptr @C.morto$c"), "{ir}");
        assert!(ir.contains("declare i64 @C.usado$c(i64, ptr, ptr)"));
        assert!(ir.contains("define ptr @df.mt.C()"));
        assert!(!ir.contains("declare ptr @df.mt.C()"), "a declaração da função da tabela sai: {ir}");
        // Ninguém cita `df.mt.P`: a tabela do programa sai vazia.
        assert!(ir.contains("@df.mt.P$d = private unnamed_addr constant { i64, i64 } { i64 300, i64 0 }"), "{ir}");
        assert!(!m.alcance.vivo(m.alcance.nomes.iter().position(|n| n == "C.morto").unwrap() as u32));
        let e = m.estatisticas;
        assert_eq!((e.tabelas, e.tabelas_vivas, e.pares, e.pares_vivos), (2, 1, 3, 1));
    }

    #[test]
    fn chamada_compacta_le_o_seletor_do_descritor() {
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"tc:usado\"\n\
             @df.seld.0 = private unnamed_addr constant {{ i64, ptr, i64 }} {{ i64 {}, ptr @df.seln.0, i64 8 }}\n\
             define i32 @main() {{\n  %f = call ptr @df.seletor_d(ptr %ic, i64 %o, ptr @df.seld.0)\n  ret i32 0\n}}\n",
            h("tc:usado")
        );
        let r = resumir(&ir, "programa", &[]);
        let main = r.definicoes.iter().find(|d| r.nomes[d.nome as usize] == "main").unwrap();
        assert!(main.chama.contains(&h("tc:usado")), "{:?}", main.chama);
        // A volta do seletor tipado.
        assert!(main.chama.contains(&h("c:usado")), "{:?}", main.chama);
        // O descritor é citado (fica vivo com quem chama).
        assert!(main.refs.iter().any(|&i| r.nomes[i as usize] == "programa#df.seld.0"));
    }

    /// Um módulo HIR: `main` chama `f` e o seletor `c:usado`; `g` ninguém
    /// cita; a classe 300 tem os pares `c:usado` e `c:morto`.
    fn modulo_hir() -> crate::hir::Module {
        use crate::hir::*;
        let funcao = |nome: &str, instrucoes: Vec<(ValueId, Instruction, Type)>| Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: Vec::new(),
            return_ty: Type::Void,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: instrucoes, terminator: Terminator::Return(None) }],
        };
        let mut m = Module::new();
        m.functions.push(funcao(
            "dart_main",
            vec![
                (ValueId(0), Instruction::CallStatic { symbol: "f".into(), args: Vec::new(), ret_ty: Type::Void }, Type::Void),
                (
                    ValueId(1),
                    Instruction::CallSeletor {
                        seletor: "c:usado".into(),
                        recv: Operand::Constant(Constant::Null),
                        args: Vec::new(),
                        nomes: Vec::new(),
                        tupla_tipos: Operand::Constant(Constant::Int(0)),
                    },
                    Type::Ref,
                ),
            ],
        ));
        m.functions.push(funcao("f", Vec::new()));
        m.functions.push(funcao("g", Vec::new()));
        m.functions.push(funcao("P.usado$c", Vec::new()));
        m.functions.push(funcao("P.morto$c", Vec::new()));
        m.entry_symbol = Some("dart_main".into());
        m.registros_do_sdk = vec!["df.registrar.core".into()];
        m.tabelas_de_metodos = vec![(
            300,
            "df.mt.P".into(),
            vec![("c:usado".into(), "P.usado$c".into()), ("c:morto".into(), "P.morto$c".into())],
        )];
        m
    }

    #[test]
    fn poda_da_hir_tira_o_que_nao_alcanca() {
        let mut m = modulo_hir();
        let e = podar_hir(&mut m, &[]);
        let nomes: Vec<&str> = m.functions.iter().map(|f| f.symbol.as_str()).collect();
        assert_eq!(nomes, vec!["dart_main", "f", "P.usado$c"], "{e:?}");
        assert_eq!(m.tabelas_de_metodos[0].2, vec![("c:usado".to_string(), "P.usado$c".to_string())]);
        assert_eq!((e.funcoes, e.funcoes_vivas, e.pares, e.pares_vivos), (5, 3, 2, 1));
    }

    #[test]
    fn poda_da_hir_ve_o_seletor_chamado_pelo_sdk() {
        // O SDK chama `c:morto` numa função que o registro dele alcança.
        let ir = "define void @df.registrar.core() {\n  call void @x()\n  ret void\n}\n\
                  define void @x() {\n  %f = call ptr @df.seletor(ptr %ic, i64 %o, i64 HASH, ptr @df.seln.0, i64 7)\n  ret void\n}\n\
                  @df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:morto\"\n"
            .replace("HASH", &h("c:morto").to_string());
        let sdk = resumir(&ir, "dart:core", &[]);
        let mut m = modulo_hir();
        podar_hir(&mut m, &[&sdk]);
        assert!(m.functions.iter().any(|f| f.symbol == "P.morto$c"));
        assert_eq!(m.tabelas_de_metodos[0].2.len(), 2);
        assert!(!m.functions.iter().any(|f| f.symbol == "g"));
    }

    #[test]
    fn definicoes_mortas_do_programa_saem_do_ir() {
        let s = sdk();
        let m = montar(&programa("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        // `df.mt.P` (ninguém cita) e o par dela saem; `main` e o `df.classe`
        // local não citado também: só o que o ponto fixo alcança fica.
        assert!(!ir.contains("define ptr @df.mt.P()"), "{ir}");
        assert!(!ir.contains("define i64 @P.nunca$c"), "{ir}");
        assert!(!ir.contains("define internal i64 @df.classe"), "{ir}");
        assert!(ir.contains("define i32 @main()"), "{ir}");
        assert!(ir.contains("@df.seln.0 = private"), "o texto do seletor que main cita fica: {ir}");
        let e = m.estatisticas;
        assert_eq!(e.funcoes_podadas, 3, "{ir}");
        // Sem a poda, nada sai.
        let m = montar(&programa("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("define i64 @P.nunca$c"));
        assert_eq!(m.estatisticas.funcoes_podadas, 0);
    }

    #[test]
    fn sem_poda_todos_os_pares_da_tabela_viva() {
        let s = sdk();
        let m = montar(&programa("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("ptr @C.morto$c"));
    }

    #[test]
    fn seletor_raiz_mantem_o_par() {
        let metodos = vec![("c:call".to_string(), "C.call$c".to_string())];
        let ir = "define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n";
        let s = resumir(ir, "dart:x", &[(200, "df.mt.C".to_string(), metodos)]);
        let m = montar(&programa("c:outro"), &[&s], true).unwrap();
        assert!(m.ir.contains("ptr @C.call$c"));
    }

    #[test]
    fn relatorio_diz_a_causa() {
        let s = sdk();
        let m = montar(&programa("c:usado"), &[&s], true).unwrap();
        let r = relatorio(&m.alcance);
        assert!(r.contains("C.usado$c\ttabela\tdf.mt.C$d\tc:usado\tmain\n"), "{r}");
        assert!(r.contains("main\traiz\t"), "{r}");
        assert!(r.contains("aloca\tref\tmain\t"), "{r}");
    }

    #[test]
    fn tabela_do_programa_reescrita_so_com_pares_vivos() {
        let mut pares = [(h("c:vivo"), "@P.vivo$c"), (h("c:morto"), "@P.morto$c")];
        pares.sort();
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:vivo\"\n\
             define i32 @main() {{\n  %p = call ptr @df.mt.P()\n  %f = call ptr @df.seletor(ptr %ic, i64 0, i64 {}, ptr @df.seln.0, i64 6)\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr, i64, ptr }} {{ i64 300, i64 2, i64 {}, ptr {}, i64 {}, ptr {} }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n",
            h("c:vivo"),
            pares[0].0,
            pares[0].1,
            pares[1].0,
            pares[1].1,
        );
        let m = montar(&ir, &[], true).unwrap();
        assert!(m.ir.contains("ptr @P.vivo$c"));
        assert!(!m.ir.contains("@P.morto$c"));
        assert!(m.ir.contains("{ i64, i64, i64, ptr } { i64 300, i64 1,"));
    }
}

#[cfg(test)]
mod testes_ffi {
    use super::*;

    fn chaves(texto: &str) -> Vec<String> {
        let letras: HashMap<i64, char> = [(703, 'i'), (719, 'p'), (737, 'v'), (736, '*'), (698, 'd')].into_iter().collect();
        let compostos: HashSet<i64> = [900].into_iter().collect();
        let mut s = HashSet::new();
        chaves_ffi_da_receita(texto, &letras, &compostos, &mut s);
        let mut v: Vec<String> = s.into_iter().collect();
        v.sort();
        v
    }

    #[test]
    fn chave_da_assinatura_na_receita() {
        assert_eq!(chaves("F<0;C703;2;C703,C719<C900>;>"), vec!["i_ip"]);
        // Dentro de outra receita (a tupla de `asFunction`, `NativeFunction<F>`).
        assert_eq!(chaves("L<C597<F<0;C737;0;;>>,F<0;D;0;;>>"), vec!["v_"]);
        // Struct por valor e variádicas.
        assert_eq!(chaves("F<0;C900;1;C719?;>"), vec!["S900._p"]);
        assert_eq!(chaves("F<0;C703;2;C719,C736<R<C703,C698;>>;>"), vec!["i_p*id"]);
        assert_eq!(chaves("F<0;C703;2;C719,C736<C698>;>"), vec!["i_p*d"]);
        // Não são assinaturas nativas: tipo Dart, genérica, opcional, nomeado.
        assert!(chaves("F<0;D;1;C703;>").is_empty());
        assert!(chaves("F<1;C703;1;B0;>").is_empty());
        assert!(chaves("F<0;C703;0;C703;>").is_empty());
        assert!(chaves("F<0;C703;0;;x:C703>").is_empty());
    }

    #[test]
    fn receitas_do_ir_vao_ao_resumo() {
        let ir = "@\"df.s.1\" = linkonce_odr constant { i64, [24 x i8] } { i64 1, [24 x i8] c\"F<0;C703;1;C719;>\\00\\00\\00\\00\\00\\00\\00\" }\n\
                  @.str.2 = private unnamed_addr constant [9 x i8] c\"F\\3C0\\3BV\\3B0\\3B\\3B\\3E\"\n\
                  define i64 @f() {\n  %a = call i64 @g(ptr @\"df.s.1\", ptr @.str.2)\n  ret i64 %a\n}\n";
        let r = resumir(ir, "m", &[]);
        let textos: Vec<&str> = r.receitas.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(textos, vec!["F<0;C703;1;C719;>", "F<0;V;0;;>"]);
        assert_eq!(r.nomes[r.receitas[0].0 as usize], "df.s.1");
        assert_eq!(r.nomes[r.receitas[1].0 as usize], "m#.str.2");
        let de_novo = Resumo::de_texto(&r.para_texto()).unwrap();
        assert_eq!(de_novo.receitas, r.receitas);
    }
}
