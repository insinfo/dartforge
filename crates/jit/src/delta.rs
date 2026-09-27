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
pub(crate) struct Publicada {
    /// Impressão digital do código (texto e fecho local).
    pub impressao: u64,
    /// A assinatura como o IR a escreve, sem nomes de parâmetro.
    pub assinatura: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Funcao { local: bool },
    Global { mutavel: bool },
    Comdat,
}

#[derive(Debug)]
struct Item<'a> {
    nome: String,
    tipo: Tipo,
    texto: &'a str,
}

/// O módulo lido pelas linhas de topo: definições de função, globais e
/// comdats; o resto (cabeçalho, comentários, `declare`) passa como está.
struct Modulo<'a> {
    /// Trechos na ordem do texto: `Ok(item)` ou `Err(texto que passa)`.
    trechos: Vec<Result<usize, &'a str>>,
    itens: Vec<Item<'a>>,
    por_nome: HashMap<String, usize>,
}

/// O nome depois do `@` (ou `$` de comdat) no começo de `s`: entre aspas, ou
/// até o primeiro caractere que não compõe identificador do LLVM.
fn nome_em(s: &str) -> Option<(String, usize)> {
    if let Some(resto) = s.strip_prefix('"') {
        let fim = resto.find('"')?;
        return Some((resto[..fim].to_string(), fim + 2));
    }
    let fim = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
        .map_or(s.len(), |(i, _)| i);
    (fim > 0).then(|| (s[..fim].to_string(), fim))
}

/// As referências `@nome` de um texto.
fn referencias(texto: &str) -> impl Iterator<Item = String> + '_ {
    texto.match_indices('@').filter_map(|(i, _)| nome_em(&texto[i + 1..]).map(|(n, _)| n))
}

fn ler(ir: &str) -> Option<Modulo<'_>> {
    let mut m = Modulo { trechos: Vec::new(), itens: Vec::new(), por_nome: HashMap::new() };
    let mut pos = 0;
    let mut passa_inicio = 0;
    while pos < ir.len() {
        let fim_linha = ir[pos..].find('\n').map_or(ir.len(), |i| pos + i + 1);
        let linha = &ir[pos..fim_linha];
        let item = if let Some(resto) = linha.strip_prefix("define ") {
            // Até a linha `}` que fecha o corpo.
            let fim = ir[pos..].find("\n}\n").map(|i| pos + i + 3)?;
            let at = resto.find('@')?;
            let (nome, _) = nome_em(&resto[at + 1..])?;
            let antes = &resto[..at];
            let local = antes.split_whitespace().any(|p| p == "internal" || p == "private");
            Some((nome, Tipo::Funcao { local }, fim))
        } else if let Some(resto) = linha.strip_prefix('@') {
            let (nome, n) = nome_em(resto)?;
            let depois = resto[n..].strip_prefix(" = ")?;
            // Uma global por linha; o `declare` de dado externo é `external global`.
            if depois.starts_with("external ") {
                None
            } else {
                let mutavel = depois.split_whitespace().find(|p| *p == "constant" || *p == "global") == Some("global");
                Some((nome, Tipo::Global { mutavel }, fim_linha))
            }
        } else if let Some(resto) = linha.strip_prefix('$') {
            let (nome, n) = nome_em(resto)?;
            resto[n..].starts_with(" = comdat ").then_some((nome, Tipo::Comdat, fim_linha))
        } else {
            None
        };
        match item {
            Some((nome, tipo, fim)) => {
                if passa_inicio < pos {
                    m.trechos.push(Err(&ir[passa_inicio..pos]));
                }
                let chave = match tipo {
                    Tipo::Comdat => format!("${nome}"),
                    _ => nome.clone(),
                };
                if m.por_nome.insert(chave, m.itens.len()).is_some() {
                    return None;
                }
                m.trechos.push(Ok(m.itens.len()));
                m.itens.push(Item { nome, tipo, texto: &ir[pos..fim] });
                pos = fim;
                passa_inicio = pos;
            }
            None => pos = fim_linha,
        }
    }
    if passa_inicio < ir.len() {
        m.trechos.push(Err(&ir[passa_inicio..]));
    }
    Some(m)
}

/// A assinatura de uma definição: a linha `define` sem ligação, sem nomes de
/// parâmetro, sem `comdat` e sem o `{`. `declare <isto>` é a declaração dela.
fn assinatura(texto: &str) -> Option<String> {
    let linha = texto.lines().next()?.strip_prefix("define ")?;
    let linha = linha.trim_end().strip_suffix('{')?.trim_end();
    let abre = linha.find('(')?;
    // O `)` que fecha a lista de parâmetros.
    let mut nivel = 0i32;
    let mut fecha = None;
    for (i, c) in linha[abre..].char_indices() {
        match c {
            '(' | '{' | '[' | '<' => nivel += 1,
            ')' | '}' | ']' | '>' => {
                nivel -= 1;
                if nivel == 0 {
                    fecha = Some(abre + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let fecha = fecha?;
    let cabeca: Vec<&str> = linha[..abre]
        .split_whitespace()
        .filter(|p| !matches!(*p, "linkonce_odr" | "internal" | "private" | "dso_local" | "weak_odr" | "linkonce"))
        .collect();
    // `comdat` e `comdat($nome)`: a declaração não pertence a comdat.
    let depois = linha[fecha + 1..]
        .split_whitespace()
        .filter(|p| *p != "comdat" && !p.starts_with("comdat("))
        .collect::<Vec<_>>()
        .join(" ");
    // Parâmetros sem o nome (`%x`) de cada um, no nível de fora.
    let mut params = Vec::new();
    let mut atual = String::new();
    let mut nivel = 0i32;
    for c in linha[abre + 1..fecha].chars() {
        match c {
            '(' | '{' | '[' | '<' => nivel += 1,
            ')' | '}' | ']' | '>' => nivel -= 1,
            ',' if nivel == 0 => {
                params.push(std::mem::take(&mut atual));
                continue;
            }
            _ => {}
        }
        atual.push(c);
    }
    if !atual.trim().is_empty() {
        params.push(atual);
    }
    let params: Vec<String> = params
        .iter()
        .map(|p| p.split_whitespace().filter(|t| !t.starts_with('%')).collect::<Vec<_>>().join(" "))
        .collect();
    let mut s = format!("{}({})", cabeca.join(" "), params.join(", "));
    if !depois.is_empty() {
        s.push(' ');
        s.push_str(&depois);
    }
    Some(s)
}

/// A infraestrutura da área de globais de cada geração: o descritor
/// (`@df.area`) e o id do módulo (`@df.area_id`), lidos pelo `df.obter_area`
/// local. Não entram na impressão digital nem impedem manter a função: o
/// layout da área só cresce entre gerações, e o runtime aceita o descritor
/// de uma geração anterior como prefixo do layout vigente, devolvendo a
/// mesma área (`dartforge_area_de_globais`).
const INFRAESTRUTURA: [&str; 2] = ["df.area", "df.area_id"];

/// O fecho local de `i`: os itens da geração alcançados por ele (funções
/// locais e globais, transitivamente), as funções do programa alcançadas
/// (pelo nome) e se alguma global mutável está no caminho.
fn fecho(m: &Modulo, i: usize) -> (BTreeSet<usize>, BTreeSet<String>, bool) {
    let mut locais = BTreeSet::new();
    let mut programa = BTreeSet::new();
    let mut mutavel = false;
    let mut pilha = vec![i];
    let mut vistos = HashSet::from([i]);
    while let Some(k) = pilha.pop() {
        for r in referencias(m.itens[k].texto) {
            if INFRAESTRUTURA.contains(&r.as_str()) {
                continue;
            }
            let Some(&j) = m.por_nome.get(&r) else { continue };
            match m.itens[j].tipo {
                Tipo::Funcao { local: false } => {
                    if j != i {
                        programa.insert(r);
                    }
                }
                Tipo::Global { mutavel: true } => mutavel = true,
                Tipo::Comdat => {}
                _ => {
                    if vistos.insert(j) {
                        locais.insert(j);
                        pilha.push(j);
                    }
                }
            }
        }
    }
    (locais, programa, mutavel)
}

fn impressao(m: &Modulo, i: usize, locais: &BTreeSet<usize>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.itens[i].texto.hash(&mut h);
    let mut nomes: Vec<(&str, &str)> = locais.iter().map(|&j| (m.itens[j].nome.as_str(), m.itens[j].texto)).collect();
    nomes.sort();
    nomes.hash(&mut h);
    h.finish()
}

/// O IR da geração nova só com o que mudou.
pub(crate) struct Delta {
    /// O IR a compilar.
    pub ir: String,
    /// As funções que continuam na implementação publicada (declaradas no IR).
    pub mantidas: Vec<String>,
    /// O que a geração publica: as funções do programa definidas no IR que
    /// sai, com a impressão digital de cada uma.
    pub novas: HashMap<String, Publicada>,
}

/// O delta de `ir` contra as implementações `vivas`. `excluidas`: funções que
/// a geração nova sempre define (os registros). `None` quando o IR não está
/// na forma esperada — então ele segue inteiro.
pub(crate) fn delta(ir: &str, vivas: &HashMap<String, Publicada>, excluidas: &[&str]) -> Option<Delta> {
    let m = ler(ir)?;
    let mut novas = HashMap::new();
    let mut mantidas: HashSet<usize> = HashSet::new();
    let mut declaracoes: HashMap<usize, String> = HashMap::new();
    let assinaturas: HashMap<&str, String> = m
        .itens
        .iter()
        .filter(|it| it.tipo == Tipo::Funcao { local: false })
        .filter_map(|it| Some((it.nome.as_str(), assinatura(it.texto)?)))
        .collect();
    for (i, item) in m.itens.iter().enumerate() {
        if item.tipo != (Tipo::Funcao { local: false }) {
            continue;
        }
        let assinatura = assinaturas.get(item.nome.as_str())?.clone();
        let (locais, programa, mutavel) = fecho(&m, i);
        let publicada = Publicada { impressao: impressao(&m, i, &locais), assinatura };
        let manter = !mutavel
            && !excluidas.contains(&item.nome.as_str())
            && vivas.get(&item.nome) == Some(&publicada)
            && programa.iter().all(|r| match (assinaturas.get(r.as_str()), vivas.get(r)) {
                (Some(nova), Some(viva)) => *nova == viva.assinatura,
                // Função do programa que o IR novo não define: a
                // referência é a mesma que a viva já fazia.
                (None, _) => true,
                (Some(_), None) => false,
            });
        if manter {
            mantidas.insert(i);
            declaracoes.insert(i, format!("declare {}\n", publicada.assinatura));
        } else {
            novas.insert(item.nome.clone(), publicada);
        }
    }
    if mantidas.is_empty() {
        return Some(Delta { ir: ir.to_string(), mantidas: Vec::new(), novas });
    }
    // O que ainda é alcançado: das funções do programa que continuam
    // definidas e das globais que não são locais (as locais só existem por
    // quem as usa).
    let mut vivos: HashSet<usize> = HashSet::new();
    let mut pilha: Vec<usize> = m
        .itens
        .iter()
        .enumerate()
        .filter(|(i, it)| match it.tipo {
            Tipo::Funcao { local: false } => !mantidas.contains(i),
            Tipo::Global { .. } => !it.texto.split_whitespace().nth(2).is_some_and(|p| p == "private" || p == "internal"),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect();
    vivos.extend(pilha.iter().copied());
    while let Some(k) = pilha.pop() {
        for r in referencias(m.itens[k].texto) {
            let Some(&j) = m.por_nome.get(&r) else { continue };
            if !mantidas.contains(&j) && vivos.insert(j) {
                pilha.push(j);
            }
        }
    }
    // Comdat só com o membro definido.
    for (i, it) in m.itens.iter().enumerate() {
        if it.tipo == Tipo::Comdat
            && m.por_nome.get(&it.nome).is_some_and(|&f| vivos.contains(&f))
        {
            vivos.insert(i);
        }
    }
    let mut saida = String::with_capacity(ir.len() / 2);
    for t in &m.trechos {
        match t {
            Err(texto) => saida.push_str(texto),
            Ok(i) if mantidas.contains(i) => saida.push_str(&declaracoes[i]),
            Ok(i) if vivos.contains(i) => saida.push_str(m.itens[*i].texto),
            Ok(i) => {
                // Função local só alcançada pelas mantidas: some; a geração
                // delas continua com a própria cópia.
                let _ = i;
            }
        }
    }
    let mut nomes: Vec<String> = mantidas.iter().map(|&i| m.itens[i].nome.clone()).collect();
    nomes.sort();
    Some(Delta { ir: saida, mantidas: nomes, novas })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O que a primeira geração de `ir` publica.
    fn publicadas(ir: &str) -> HashMap<String, Publicada> {
        delta(ir, &HashMap::new(), &[]).expect("forma conhecida").novas
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
    fn assinatura_sem_nomes_nem_ligacao() {
        assert_eq!(assinatura("define linkonce_odr i64 @\"g\"(i64 %v0) comdat {\n}").unwrap(), "i64 @\"g\"(i64)");
        assert_eq!(
            assinatura("define linkonce_odr i64 @g$t(i64 %v0, ptr %v1) comdat($\"g$t\") {\n}").unwrap(),
            "i64 @g$t(i64, ptr)"
        );
        assert_eq!(
            assinatura("define internal signext i8 @k({ i64, ptr } %a, ptr noalias %b) {\n}").unwrap(),
            "signext i8 @k({ i64, ptr }, ptr noalias)"
        );
    }

    #[test]
    fn mantem_o_que_nao_mudou_e_compila_o_resto() {
        let vivas = publicadas(V1);
        assert_eq!(vivas.len(), 3, "{vivas:?}");
        // `g` mudou: só ela é compilada; `f` (que chama `g` pelo trampolim)
        // fica na implementação viva; `h` usa global mutável e nunca fica.
        let v2 = V1.replace("  ret i64 %v0\n}\n\ndefine i64 @h", "  %y = add i64 %v0, 1\n  ret i64 %y\n}\n\ndefine i64 @h");
        let d = delta(&v2, &vivas, &[]).expect("forma conhecida");
        assert_eq!(d.mantidas, vec!["f".to_string()]);
        assert!(d.ir.contains("declare i64 @f(i64, ptr)\n"), "{}", d.ir);
        // A local só alcançada por `f` some; o comdat de `g` fica.
        assert!(!d.ir.contains("@ajuda("), "{}", d.ir);
        assert!(d.ir.contains("$\"g\" = comdat any"));
        assert!(d.ir.contains("%y = add i64 %v0, 1"));
        assert!(d.ir.contains("define i64 @h("));
        assert_eq!(d.novas.keys().collect::<BTreeSet<_>>(), BTreeSet::from([&"g".to_string(), &"h".to_string()]));
    }

    #[test]
    fn local_mudada_recompila_quem_a_usa() {
        let vivas = publicadas(V1);
        let v2 = V1.replace("c\"ab\"", "c\"xy\"");
        let d = delta(&v2, &vivas, &[]).unwrap();
        // A string muda o fecho de `ajuda` e portanto o de `f`.
        assert!(!d.mantidas.contains(&"f".to_string()), "{:?}", d.mantidas);
        assert!(d.mantidas.contains(&"g".to_string()));
    }

    #[test]
    fn assinatura_mudada_de_quem_e_chamado_recompila_o_chamador() {
        let vivas = publicadas(V1);
        let v2 = V1
            .replace("define linkonce_odr i64 @\"g\"(i64 %v0)", "define linkonce_odr i64 @\"g\"(i64 %v0, i64 %v1)")
            .replace("call i64 @g(i64 %r)", "call i64 @g(i64 %r)");
        let d = delta(&v2, &vivas, &[]).unwrap();
        assert!(d.mantidas.is_empty(), "{:?}", d.mantidas);
    }

    #[test]
    fn excluidas_e_sem_viva_sao_compiladas() {
        let vivas = publicadas(V1);
        let d = delta(V1, &vivas, &["f"]).unwrap();
        assert_eq!(d.mantidas, vec!["g".to_string()]);
        let d = delta(V1, &HashMap::new(), &[]).unwrap();
        assert!(d.mantidas.is_empty());
        assert_eq!(d.ir, V1);
    }
}
