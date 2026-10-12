//! Chaves curtas para os parâmetros nomeados (`docs/JS-PRODUCAO-TAMANHO.md`
//! §8.2).
//!
//! No contrato do DDC um argumento nomeado é a chave de um objeto
//! (`f(1, {limite: 2})`), lida no prólogo da função (`opts && "limite" in
//! opts ? opts.limite : …`), e o nome também está na receita rti do tipo da
//! função (`~(core|int{limite:core|int})`), que a checagem da chamada dinâmica
//! (`_checkAndCall`) compara com as chaves do objeto. O nome, então, não pode
//! passar pelo renomeio de propriedades do `oxc`: a receita é uma *string*.
//!
//! Aqui o emissor troca o nome por uma chave curta (`$a`, `$bK`) de um mapa
//! **global por nome**, aplicado ao mesmo tempo na chamada, no prólogo e na
//! receita (`Ctx::nomeado`). Sendo função só do nome, chamador e chamado
//! concordam sem saber um do outro, inclusive nas chamadas dinâmicas.
//!
//! Ficam com o nome de origem:
//! * os parâmetros nomeados de funções `external` e de classes de interop
//!   (viram chaves de objeto do JavaScript);
//! * as palavras dos *templates* `JS()` vivos;
//! * **tudo**, se o programa tiver um `noSuchMethod` próprio vivo (a
//!   `Invocation` de uma chamada dinâmica mostraria as chaves).
//!
//! O `Function.apply` monta as chaves a partir do `Symbol` do nome: com ele
//! vivo, o mapa vai também para o runtime (`dart.nomeadoJS`).
//!
//! O que muda de visível: o `toString` de um tipo de função com parâmetros
//! nomeados mostra a chave curta.

use dartforge_elements::model::{ClassKind, Element, FunctionElementId};
use dartforge_types::table::{Type, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};

/// O `n`-ésimo sufixo de chave: uma letra entre 54, depois as seguintes
/// entre 64 (a numeração dos ids de receita, `dartforge_emit_js::numero_de_id`).
fn sufixo(mut n: usize) -> String {
    let (a, b) = (dartforge_emit_js::ID_PRIMEIRA, dartforge_emit_js::ID_OUTRAS);
    let mut s = vec![a[n % a.len()]];
    n /= a.len();
    while n > 0 {
        n -= 1;
        s.push(b[n % b.len()]);
        n /= b.len();
    }
    String::from_utf8(s).unwrap_or_default()
}

/// O mapa nome → chave curta, ou `None` quando o programa precisa dos nomes
/// de origem em execução.
pub fn calcular(a: &dartforge_emit_js::Analise<'_>, mundo: &dartforge_world::Mundo) -> Option<Nomeados> {
    let p = a.program;
    let i = a.interner;
    let mut tabela = false;
    // `Function.apply` vivo.
    if let (Some(core), Some(fsym), Some(asym)) = (p.core, i.lookup("Function"), i.lookup("apply")) {
        if let Some(Element::Class(fc)) = p.library(core).declared.get(&fsym).and_then(|b| b.getter) {
            if p.class(fc).static_members.get(&asym).is_some_and(|f| mundo.funcao(*f)) {
                // O `Function.apply` traduz os nomes em execução
                // (`dart.nomeadoJS`, emitido com a tabela).
                tabela = true;
            }
        }
    }
    // `noSuchMethod` próprio vivo fora do SDK.
    for (k, f) in p.functions.iter().enumerate() {
        if f.class.is_some() && !p.library(f.library).is_sdk && i.resolve(f.name) == "noSuchMethod" && mundo.funcao(FunctionElementId(k as u32)) {
            if std::env::var("DARTFORGE_JSPROD_RASTRO").is_ok() {
                eprintln!("[jsprod] nomeados: noSuchMethod vivo em {}; nomes de origem", p.library(f.library).uri);
            }
            return None;
        }
    }
    let ctx = a.ctx();
    let mut todos: BTreeSet<String> = BTreeSet::new();
    // Quantas funções declaram cada nome: os mais comuns ficam com as chaves
    // de uma letra.
    let mut frequencia: HashMap<String, usize> = HashMap::new();
    let mut fixos: HashSet<String> = mundo.nomes_js.clone();
    for (k, f) in p.functions.iter().enumerate() {
        let Some(ft) = a.outline.functions.get(k) else { continue };
        let interop = f.external || f.class.is_some_and(|c| ctx.is_js_class(c) || p.class(c).kind == ClassKind::ExtensionType);
        for par in ft.parameters.iter() {
            if par.kind != dartforge_frontend::ast::ParameterKind::Named {
                continue;
            }
            let Some(n) = par.name else { continue };
            let n = i.resolve(n).to_string();
            if interop {
                fixos.insert(n.clone());
            }
            *frequencia.entry(n.clone()).or_default() += 1;
            todos.insert(n);
        }
    }
    // Nomes que só aparecem em tipos de função escritos (sem declaração).
    for t in 0..a.table.len() {
        if let Type::Function { named, .. } = a.table.get(TypeId(t as u32)) {
            for (s, _, _) in named.iter() {
                todos.insert(i.resolve(*s).to_string());
            }
        }
    }
    let mut mapa: HashMap<String, String> = HashMap::new();
    let mut k = 0usize;
    let existentes: HashSet<String> = todos.iter().cloned().collect();
    let mut ordem: Vec<String> = todos.into_iter().collect();
    ordem.sort_by(|x, y| frequencia.get(y).unwrap_or(&0).cmp(frequencia.get(x).unwrap_or(&0)).then_with(|| x.cmp(y)));
    for n in ordem {
        if fixos.contains(&n) {
            continue;
        }
        // A chave curta não pode coincidir com um nome de origem que fica.
        let curto = loop {
            let c = format!("${}", sufixo(k));
            k += 1;
            if !fixos.contains(&c) && !existentes.contains(&c) {
                break c;
            }
        };
        if curto.len() < n.len() {
            mapa.insert(n, curto);
        }
    }
    Some(Nomeados { mapa, tabela })
}

/// O mapa e se o runtime precisa dele em execução.
#[derive(Clone, Default, Debug)]
pub struct Nomeados {
    pub mapa: HashMap<String, String>,
    pub tabela: bool,
}

impl Nomeados {
    /// O JSON nome → chave, para o `dart.nomeadoJS` (determinístico).
    pub fn json(&self) -> String {
        let mut v: Vec<(&String, &String)> = self.mapa.iter().collect();
        v.sort();
        let itens: Vec<String> = v.iter().map(|(k, c)| format!("{}:{}", json_str(k), json_str(c))).collect();
        format!("{{{}}}", itens.join(","))
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}
