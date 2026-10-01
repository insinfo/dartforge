//! O módulo do programa em partes (docs/NATIVO-PROJETOS-REAIS.md, C9).
//!
//! O IR de um programa grande não cabe num Clang: o `-O0` gasta ~44 bytes
//! de memória por byte de IR (24,8 MB de IR, 1,08 GB de pico, medido), e o
//! new_sali/backend tem 1,3 GB de IR — 55 GB para um processo só. Aqui o
//! texto do módulo é dividido em partes de ~[`ALVO_PADRAO`] bytes, cada uma
//! um módulo LLVM completo, compiladas uma a uma (ou poucas por vez) e
//! ligadas juntas. Cada parte tem a sua chave no cache de objetos, então uma
//! edição recompila só as partes que mudaram.
//!
//! O que o emissor escreve (`llvm/mod.rs`) e como cada coisa vai às partes:
//!
//! * `define … @f(…) { … }` e `@g = …` com ligação externa ou
//!   `linkonce_odr`: numa parte só; as outras que os citam recebem a
//!   declaração (`declare` com os tipos dos parâmetros, ou
//!   `@g = external global|constant <tipo>`);
//! * `private`/`internal` constantes e funções (os textos `@.str.N`, os
//!   vetores, os ajudantes `@df.*` `alwaysinline`): copiados em cada parte
//!   que os cita, com o que eles citam (fecho transitivo). São imutáveis, e
//!   duas cópias privadas não se veem;
//! * `internal global` mutável (só o `@df.area_id`, o índice da área de
//!   globais do módulo): uma cópia por parte quebraria o estado — passa a
//!   ter ligação externa na parte 0 e é declarada nas outras;
//! * `declare`, o cabeçalho (`target`, `source_filename`), atributos e
//!   metadados: em toda parte; `$x = comdat any`, na parte do item que o usa;
//!   os comentários (`; df.classe`, `; df.campos`, que a recarga do JIT lê
//!   do IR inteiro) só na parte 0.
//!
//! As partes agrupam os itens pela biblioteca do símbolo
//! (`df.<biblioteca>.…`), em ordem, para que o conteúdo de uma parte dependa
//! das bibliotecas dela.

use std::collections::{HashMap, HashSet};

/// Tamanho de IR a partir do qual o módulo é dividido.
pub const LIMIAR_PADRAO: usize = 48 << 20;
/// Tamanho de cada parte.
pub const ALVO_PADRAO: usize = 24 << 20;

/// O limiar e o alvo, ajustáveis (`DARTFORGE_PARTE_MB`, em MiB; `0` desliga
/// a divisão).
pub fn limites() -> Option<(usize, usize)> {
    match std::env::var("DARTFORGE_PARTE_MB").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(0) => None,
        Some(mb) => Some((2 * (mb << 20), mb << 20)),
        None => Some((LIMIAR_PADRAO, ALVO_PADRAO)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tipo {
    Funcao,
    Global,
}

/// Um item definido no módulo.
struct Item<'a> {
    nome: &'a str,
    tipo: Tipo,
    texto: &'a str,
    local: bool,
    /// `internal global` (não `constant`): estado, não pode ser copiado.
    mutavel: bool,
    comdat: Option<&'a str>,
}

/// O módulo lido.
struct Modulo<'a> {
    cabecalho: Vec<&'a str>,
    comentarios: Vec<&'a str>,
    declaracoes: Vec<(&'a str, &'a str)>,
    comdats: HashMap<&'a str, &'a str>,
    itens: Vec<Item<'a>>,
    indice: HashMap<&'a str, usize>,
}

fn e_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'$' | b'-')
}

/// O nome em `@nome…` a partir de `s` (que começa depois do `@`); nomes
/// entre aspas (`@"…"`) também.
fn nome_em(s: &str) -> &str {
    let b = s.as_bytes();
    if b.first() == Some(&b'"') {
        return s[1..].find('"').map_or("", |f| &s[..f + 2]);
    }
    let fim = b.iter().position(|c| !e_ident(*c)).unwrap_or(b.len());
    &s[..fim]
}

/// Todas as referências `@nome` de um texto.
fn referencias<'a>(texto: &'a str, saida: &mut Vec<&'a str>) {
    let b = texto.as_bytes();
    let mut i = 0;
    while let Some(p) = memchr_at(b, b'@', i) {
        let n = nome_em(&texto[p + 1..]);
        if !n.is_empty() {
            saida.push(n);
        }
        i = p + 1 + n.len();
    }
}

fn memchr_at(b: &[u8], c: u8, de: usize) -> Option<usize> {
    b.get(de..)?.iter().position(|x| *x == c).map(|p| p + de)
}

fn ler(ir: &str) -> Modulo<'_> {
    let mut m = Modulo {
        cabecalho: Vec::new(),
        comentarios: Vec::new(),
        declaracoes: Vec::new(),
        comdats: HashMap::new(),
        itens: Vec::new(),
        indice: HashMap::new(),
    };
    let mut pos = 0;
    let bytes = ir.as_bytes();
    while pos < bytes.len() {
        let fim_linha = memchr_at(bytes, b'\n', pos).map_or(bytes.len(), |p| p + 1);
        let linha = &ir[pos..fim_linha];
        if linha.starts_with("define ") {
            // Até a linha que é só `}`.
            let mut fim = fim_linha;
            loop {
                if fim >= bytes.len() {
                    break;
                }
                let prox = memchr_at(bytes, b'\n', fim).map_or(bytes.len(), |p| p + 1);
                let l = &ir[fim..prox];
                fim = prox;
                if l.starts_with('}') {
                    break;
                }
            }
            let cab = &linha[..linha.find('(').unwrap_or(linha.len())];
            let nome = cab.rfind('@').map_or("", |a| nome_em(&cab[a + 1..]));
            let local = cab.contains(" internal ") || cab.contains(" private ");
            let comdat = linha.find("comdat($").map(|c| {
                let r = &linha[c + 7..];
                &r[..r.find(')').unwrap_or(r.len())]
            });
            m.indice.insert(nome, m.itens.len());
            m.itens.push(Item { nome, tipo: Tipo::Funcao, texto: &ir[pos..fim], local, mutavel: false, comdat });
            pos = fim;
            continue;
        }
        if let Some(r) = linha.strip_prefix('@') {
            let nome = nome_em(r);
            let depois = &r[nome.len()..];
            let depois = depois.trim_start().strip_prefix('=').unwrap_or(depois).trim_start();
            let local = depois.starts_with("private ") || depois.starts_with("internal ");
            let mutavel = local && cabeca_do_global(depois).0 == "global";
            let comdat = linha.find("comdat($").map(|c| {
                let r = &linha[c + 7..];
                &r[..r.find(')').unwrap_or(r.len())]
            });
            m.indice.insert(nome, m.itens.len());
            m.itens.push(Item { nome, tipo: Tipo::Global, texto: linha, local, mutavel, comdat });
        } else if linha.starts_with("declare ") {
            let cab = &linha[..linha.find('(').unwrap_or(linha.len())];
            let nome = cab.rfind('@').map_or("", |a| nome_em(&cab[a + 1..]));
            m.declaracoes.push((nome, linha));
        } else if linha.starts_with('$') {
            let n = linha.split(' ').next().unwrap_or("");
            m.comdats.insert(n, linha);
        } else if linha.starts_with(';') {
            m.comentarios.push(linha);
        } else {
            m.cabecalho.push(linha);
        }
        pos = fim_linha;
    }
    m
}

/// `global`/`constant`, e o resto depois da palavra, de `<ligação…>
/// global|constant <tipo> <inicial>`.
fn cabeca_do_global(s: &str) -> (&str, &str) {
    let mut resto = s;
    loop {
        let (palavra, depois) = resto.split_once(' ').unwrap_or((resto, ""));
        if palavra == "global" || palavra == "constant" || palavra.is_empty() {
            return (palavra, depois);
        }
        resto = depois;
    }
}

/// O tipo LLVM no começo de `s` (com colchetes, chaves e `<…>` casados).
fn tipo_no_comeco(s: &str) -> &str {
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
fn declaracao(item: &Item) -> String {
    match item.tipo {
        Tipo::Global => {
            let r = &item.texto[1 + item.nome.len()..];
            let r = r.trim_start().trim_start_matches('=').trim_start();
            let (palavra, depois) = cabeca_do_global(r);
            let tipo = tipo_no_comeco(depois);
            format!("@{} = external {palavra} {tipo}\n", item.nome)
        }
        Tipo::Funcao => {
            let linha = item.texto.lines().next().unwrap_or("");
            let a = linha.find('@').unwrap_or(0);
            let antes = &linha["define ".len()..a];
            let ret: Vec<&str> = antes
                .split_whitespace()
                .filter(|w| {
                    !matches!(
                        *w,
                        "private" | "internal" | "linkonce_odr" | "weak_odr" | "external" | "hidden" | "dso_local" | "dllexport" | "fastcc" | "ccc"
                    )
                })
                .collect();
            let abre = a + linha[a..].find('(').unwrap_or(0);
            // Os parâmetros, com vírgulas só no nível de cima.
            let mut prof = 0i32;
            let mut fecha = abre;
            for (i, c) in linha[abre..].char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => {
                        prof -= 1;
                        if prof == 0 {
                            fecha = abre + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let params = &linha[abre + 1..fecha];
            let mut tipos = Vec::new();
            let mut prof = 0i32;
            let mut ini = 0;
            let mut partes = Vec::new();
            for (i, c) in params.char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => prof -= 1,
                    ',' if prof == 0 => {
                        partes.push(&params[ini..i]);
                        ini = i + 1;
                    }
                    _ => {}
                }
            }
            if !params.trim().is_empty() {
                partes.push(&params[ini..]);
            }
            for p in partes {
                let p = p.trim();
                let sem_nome = match p.rfind(" %") {
                    Some(k) => &p[..k],
                    None => p,
                };
                tipos.push(sem_nome.to_string());
            }
            format!("declare {} @{}({})\n", ret.join(" "), item.nome, tipos.join(", "))
        }
    }
}

/// A chave de agrupamento de um símbolo: a biblioteca dele, quando o nome a
/// tem (`df.<biblioteca>.…`, `df.mt.<biblioteca>.…`, `df.rti.<h>.<biblioteca>`).
fn grupo(nome: &str) -> &str {
    let partes: Vec<&str> = nome.splitn(4, '.').collect();
    match partes.as_slice() {
        ["df", "mt", lib, ..] => lib,
        ["df", "rti", _, lib] => lib,
        ["df", lib, ..] => lib,
        _ => nome,
    }
}

/// Divide o módulo em partes de ~`alvo` bytes. Um módulo menor que `alvo`
/// volta inteiro, como está.
pub fn dividir(ir: &str, alvo: usize) -> Vec<String> {
    if ir.len() <= alvo {
        return vec![ir.to_string()];
    }
    let m = ler(ir);
    // Os itens com ligação externa, agrupados pela biblioteca na ordem em
    // que aparecem; os grupos, em partes de ~`alvo` bytes.
    let mut ordem_dos_grupos: Vec<&str> = Vec::new();
    let mut por_grupo: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, it) in m.itens.iter().enumerate() {
        if it.local {
            continue;
        }
        let g = grupo(it.nome);
        por_grupo.entry(g).or_insert_with(|| {
            ordem_dos_grupos.push(g);
            Vec::new()
        }).push(i);
    }
    let mut partes: Vec<Vec<usize>> = vec![Vec::new()];
    let mut tamanho = 0usize;
    for g in &ordem_dos_grupos {
        for &i in &por_grupo[g] {
            let t = m.itens[i].texto.len();
            if tamanho > 0 && tamanho + t > alvo {
                partes.push(Vec::new());
                tamanho = 0;
            }
            partes.last_mut().expect("uma parte").push(i);
            tamanho += t;
        }
    }
    let mut parte_de: Vec<Option<usize>> = vec![None; m.itens.len()];
    for (p, itens) in partes.iter().enumerate() {
        for &i in itens {
            parte_de[i] = Some(p);
        }
    }
    // O estado mutável local vai à parte 0, com ligação externa.
    for (i, it) in m.itens.iter().enumerate() {
        if it.mutavel {
            parte_de[i] = Some(0);
            partes[0].push(i);
        }
    }
    let declarados: HashMap<&str, &str> = m.declaracoes.iter().copied().collect();
    let mut saida = Vec::with_capacity(partes.len());
    for (p, itens) in partes.iter().enumerate() {
        // O fecho: os itens da parte, os locais que eles citam (copiados), e
        // as declarações do que mora fora.
        let mut incluidos: HashSet<usize> = itens.iter().copied().collect();
        let mut fila: Vec<usize> = itens.clone();
        let mut externos: Vec<usize> = Vec::new();
        let mut vistos_ext: HashSet<usize> = HashSet::new();
        let mut decls: Vec<&str> = Vec::new();
        let mut vistos_decl: HashSet<&str> = HashSet::new();
        let mut refs = Vec::new();
        while let Some(i) = fila.pop() {
            refs.clear();
            referencias(m.itens[i].texto, &mut refs);
            for r in refs.iter() {
                if let Some(&j) = m.indice.get(r) {
                    if j == i || incluidos.contains(&j) {
                        continue;
                    }
                    let it = &m.itens[j];
                    if it.local && !it.mutavel {
                        incluidos.insert(j);
                        fila.push(j);
                    } else if vistos_ext.insert(j) {
                        externos.push(j);
                    }
                } else if let Some(d) = declarados.get(r)
                    && vistos_decl.insert(*r)
                {
                    decls.push(d);
                }
            }
        }
        let mut texto = String::with_capacity(alvo + alvo / 4);
        for l in &m.cabecalho {
            texto.push_str(l);
        }
        if p == 0 {
            for l in &m.comentarios {
                texto.push_str(l);
            }
        }
        for d in &decls {
            texto.push_str(d);
            if !d.ends_with('\n') {
                texto.push('\n');
            }
        }
        externos.sort_unstable();
        for j in externos {
            texto.push_str(&declaracao(&m.itens[j]));
        }
        let mut lista: Vec<usize> = incluidos.into_iter().collect();
        lista.sort_unstable();
        for i in lista {
            let it = &m.itens[i];
            if let Some(c) = it.comdat
                && let Some(l) = m.comdats.get(c)
            {
                texto.push_str(l);
            }
            if it.mutavel {
                // `@df.area_id = internal global …` → ligação externa.
                texto.push_str(&it.texto.replacen(" = internal ", " = ", 1).replacen(" = private ", " = ", 1));
            } else {
                texto.push_str(it.texto);
            }
        }
        saida.push(texto);
    }
    saida
}

#[cfg(test)]
mod testes {
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
  %s = call i64 @df.b.k(ptr @.str.0)\n\
  ret i64 %s\n\
}\n\
define linkonce_odr i64 @df.b.k(ptr %p) comdat($\"df.b.k\") {\n\
b0:\n\
  %q = call i64 @dartforge_x(i64 1)\n\
  %g = load i64, ptr @df.a.g\n\
  ret i64 %q\n\
}\n";

    #[test]
    fn divide_com_declaracoes_e_copias() {
        let p = dividir(MOD, 120);
        assert_eq!(p.len(), 3, "{p:#?}");
        // a parte de `df.a.f` copia o ajudante e o texto, declara `df.b.k`.
        let pa = p.iter().find(|t| t.contains("define i64 @df.a.f")).unwrap();
        assert!(pa.contains("define internal i64 @df.ajuda"));
        assert!(pa.contains("@.str.0 = private"));
        assert!(pa.contains("declare i64 @df.b.k(ptr)"));
        assert!(pa.contains("@df.area_id = external global i64") || pa.contains("@df.area_id = global i64 0"));
        let pb = p.iter().find(|t| t.contains("define linkonce_odr i64 @df.b.k")).unwrap();
        assert!(pb.contains("$\"df.b.k\" = comdat any"));
        assert!(pb.contains("declare i64 @dartforge_x(i64)"));
        assert!(pb.contains("@df.a.g = external global { i64, [2 x ptr] }"), "{pb}");
        // O estado mutável: definido uma vez, com ligação externa.
        assert_eq!(p.iter().filter(|t| t.contains("@df.area_id = global i64 0")).count(), 1);
        assert!(p[0].contains("; df.classe"));
    }

    #[test]
    fn declara_funcao_com_parametros_compostos() {
        let m = ler(MOD);
        let i = m.indice["df.a.f"];
        assert_eq!(declaracao(&m.itens[i]), "declare i64 @df.a.f(i64, { i64, i64 })\n");
    }
}
