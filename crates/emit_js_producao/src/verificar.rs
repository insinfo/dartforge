//! Verificador do texto emitido com o mundo fechado (`docs/JS-PRODUCAO.md`
//! §1.7, §5): o bundle podado é conferido **pelo texto**, que é a verdade do
//! que o emissor escreveu.
//!
//! 1. **Nenhuma referência pendente**: todo `L$x.Nome`, `L$x.Classe.membro`
//!    estático, `L$x['k']` e toda receita rti `"x|Nome"` citada tem de estar
//!    definida no texto.
//! 2. **Nenhum seletor esquecido**: todo nome que o texto usa como membro
//!    (`.nome`, `[$nome]`, `dart.dsend(o, "nome")`) e que é membro podado de
//!    classe instanciada.
//!
//! O que falta volta como [`Faltas`], já traduzido para elementos. Quem chama
//! acrescenta às raízes e recalcula o mundo: o mundo final é ponto fixo do
//! mundo **e** do texto. Uma falta aqui é lacuna da análise (registrada no
//! relatório), não do programa — e nunca vira código quebrado em silêncio.

use crate::varredura;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId, Program, VariableId};
use dartforge_intern::Interner;
use dartforge_mundo::{Mundo, NivelClasse};
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct Faltas {
    pub funcoes: Vec<FunctionElementId>,
    pub variaveis: Vec<VariableId>,
    pub classes: Vec<ClassId>,
    /// Classes sem construtor declarado cujo `new` o texto chama.
    pub classes_instanciadas: Vec<ClassId>,
    pub tearoffs: Vec<FunctionElementId>,
    pub seletores: Vec<String>,
    /// Referências pendentes que não correspondem a elemento nenhum.
    pub sem_elemento: Vec<String>,
    /// Texto legível do que faltou, para o relatório.
    pub descricao: Vec<String>,
}

impl Faltas {
    pub fn vazia(&self) -> bool {
        self.funcoes.is_empty() && self.variaveis.is_empty() && self.classes.is_empty() && self.classes_instanciadas.is_empty() && self.tearoffs.is_empty() && self.seletores.is_empty()
    }
}

fn e_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

fn ident_em(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && e_ident(b[j]) {
        j += 1;
    }
    j
}

/// Chave de uma entrada de objeto/corpo de classe: `nome(`, `get nome()`,
/// `static nome(`, `['k'](`, `"k":`.
fn chave(t: &str) -> Option<(bool, String)> {
    let mut t = t.trim_start();
    // Comentário de posição antes da entrada.
    while let Some(r) = t.strip_prefix("/*") {
        t = r.split_once("*/").map(|(_, x)| x.trim_start()).unwrap_or("");
    }
    let (estatico, t) = match t.strip_prefix("static ") {
        Some(r) => (true, r.trim_start()),
        None => (false, t),
    };
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix('*').map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'[' && b.len() > 1 && (b[1] == b'"' || b[1] == b'\'') {
        let aspa = b[1];
        let fim = t[2..].find(aspa as char)? + 2;
        return Some((estatico, t[2..fim].to_string()));
    }
    if b[0] == b'"' || b[0] == b'\'' {
        let fim = t[1..].find(b[0] as char)? + 1;
        return Some((estatico, t[1..fim].to_string()));
    }
    let f = ident_em(b, 0);
    (f > 0).then(|| (estatico, t[..f].to_string()))
}

/// `L$x` no começo de `t` (depois de espaços): o identificador e onde acaba.
fn namespace(t: &str) -> Option<(&str, usize)> {
    if !t.starts_with("L$") {
        return None;
    }
    let f = ident_em(t.as_bytes(), 0);
    Some((&t[..f], f))
}

struct Definicoes {
    nomes: HashSet<String>,
    classes: HashSet<String>,
}

fn definicoes(modulos: &[(String, String)]) -> Definicoes {
    let mut d = Definicoes { nomes: HashSet::new(), classes: HashSet::new() };
    for (_, texto) in modulos {
        for (ini, fim) in varredura::declaracoes(texto) {
            let t = texto[ini..fim].trim_start();
            // `(L$x.C.n = function …).prototype = …` e `(L$x.C[dart.mixinNew] = …)`
            let t2 = t.strip_prefix('(').unwrap_or(t);
            if let Some((ns, f)) = namespace(t2) {
                let b = t2.as_bytes();
                if b.get(f) == Some(&b'.') {
                    let g = ident_em(b, f + 1);
                    let nome = format!("{ns}.{}", &t2[f + 1..g]);
                    let resto = t2[g..].trim_start();
                    if let Some(r) = resto.strip_prefix('.') {
                        let h = ident_em(r.as_bytes(), 0);
                        d.nomes.insert(format!("{nome}.{}", &r[..h]));
                    } else if resto.starts_with("= class ") {
                        d.classes.insert(nome.clone());
                        // Membros estáticos no corpo da classe.
                        if let Some(rel) = t2.find('{') {
                            let abre = rel;
                            if let Some(fecha) = varredura::fecha_chave(t2, abre) {
                                let corpo = &t2[abre + 1..fecha];
                                for (a, z) in varredura::entradas_de_classe(corpo) {
                                    if let Some((true, k)) = chave(&corpo[a..z]) {
                                        d.nomes.insert(format!("{nome}.{k}"));
                                    }
                                }
                            }
                        }
                    }
                    d.nomes.insert(nome);
                } else if b.get(f) == Some(&b'[') {
                    if let Some((_, k)) = chave(&t2[f..]) {
                        d.nomes.insert(format!("{ns}.{k}"));
                    }
                }
                continue;
            }
            for pre in ["dart.copyProperties(", "dart.defineLazy("] {
                let Some(r) = t.strip_prefix(pre) else { continue };
                let Some(virg) = r.find(',') else { continue };
                let alvo = r[..virg].trim();
                if !alvo.starts_with("L$") {
                    continue;
                }
                let Some(rel) = r.find('{') else { continue };
                let abre = t.len() - r.len() + rel;
                let Some(fecha) = varredura::fecha_chave(t, abre) else { continue };
                let corpo = &t[abre + 1..fecha];
                for (a, z) in varredura::entradas_de_objeto(corpo) {
                    if let Some((_, k)) = chave(&corpo[a..z]) {
                        d.nomes.insert(format!("{alvo}.{k}"));
                    }
                }
            }
        }
    }
    d
}

/// Referências a `L$…` e receitas `ident|Nome` no texto.
fn referencias(texto: &str, idents: &HashSet<&str>, classes: &HashSet<String>, out: &mut HashSet<String>) {
    let b = texto.as_bytes();
    let mut i = 0usize;
    while let Some(p) = texto[i..].find("L$") {
        let s = i + p;
        i = s + 2;
        if s > 0 && e_ident(b[s - 1]) {
            continue;
        }
        let f = ident_em(b, s);
        let ns = &texto[s..f];
        i = f;
        if !idents.contains(&ns[2..]) {
            continue;
        }
        let nome = match b.get(f) {
            Some(b'.') => {
                let g = ident_em(b, f + 1);
                if g == f + 1 {
                    continue;
                }
                i = g;
                texto[f + 1..g].to_string()
            }
            Some(b'[') => {
                if let Some((_, k)) = chave(&texto[f..]) {
                    out.insert(format!("{ns}.{k}"));
                }
                continue;
            }
            _ => continue,
        };
        if nome == "$constCache" || nome == "$C" {
            continue;
        }
        let q = format!("{ns}.{nome}");
        // Membro estático de classe: `L$x.C.m` ou `L$x.C["_#new#tearOff"]`.
        if classes.contains(&q) {
            match b.get(i) {
                Some(b'.') => {
                    let h = ident_em(b, i + 1);
                    let m = &texto[i + 1..h];
                    if !m.is_empty() && m != "prototype" {
                        out.insert(format!("{q}.{m}"));
                    }
                }
                Some(b'[') => {
                    if let Some((_, k)) = chave(&texto[i..]) {
                        out.insert(format!("{q}.{k}"));
                    }
                }
                _ => {}
            }
        }
        out.insert(q);
    }
    // Receitas rti: `ident|Nome` (dentro de strings).
    for (p, _) in texto.match_indices('|') {
        if p == 0 || p + 1 >= b.len() || !(b[p + 1].is_ascii_alphabetic() || b[p + 1] == b'_' || b[p + 1] == b'$') {
            continue;
        }
        let mut ini = p;
        while ini > 0 && e_ident(b[ini - 1]) {
            ini -= 1;
        }
        if ini == p {
            continue;
        }
        let ident = &texto[ini..p];
        if !idents.contains(ident) {
            continue;
        }
        let fim = ident_em(b, p + 1);
        out.insert(format!("L${ident}.{}", &texto[p + 1..fim]));
    }
}

/// Espécie de um uso de membro no texto: leitura/chamada (`.w`, `.w()`),
/// escrita (`.w = v`) ou as duas (`.w++`, `.w += v`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Especie {
    Leitura,
    Escrita,
    Ambas,
}

/// Classifica o que vem depois de `.nome` (ou `[$nome]`, a partir de depois
/// do `]`): `=` simples é escrita; `++`/`--` e composto (`+=`, `<<=`, `??=`…)
/// é leitura+escrita; o resto — inclusive `==`/`===` e `=>` — é leitura.
fn especie_apos(b: &[u8], f: usize) -> Especie {
    let mut i = f;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    if b.get(i) == Some(&b'+') && b.get(i + 1) == Some(&b'+') {
        return Especie::Ambas;
    }
    if b.get(i) == Some(&b'-') && b.get(i + 1) == Some(&b'-') {
        return Especie::Ambas;
    }
    let mut j = i;
    if b.get(j) == Some(&b'?') && b.get(j + 1) == Some(&b'?') {
        j += 2;
    } else {
        while matches!(b.get(j).copied(), Some(b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'<' | b'>')) {
            j += 1;
        }
    }
    if b.get(j) != Some(&b'=') {
        return Especie::Leitura;
    }
    if j == i {
        if b.get(j + 1) == Some(&b'=') || b.get(j + 1) == Some(&b'>') {
            return Especie::Leitura;
        }
        return Especie::Escrita;
    }
    Especie::Ambas
}

/// Nomes usados como membro no texto, por espécie: `.nome`, `[$nome]` e as
/// strings de `dsend`/`dload`/`dput`/`bind` (`dput` é escrita, o resto é
/// leitura).
fn seletores_do_texto(texto: &str, leituras: &mut HashSet<String>, escritas: &mut HashSet<String>) {
    let b = texto.as_bytes();
    let mut i = 0usize;
    let registra = |nome: &str, e: Especie, leituras: &mut HashSet<String>, escritas: &mut HashSet<String>| {
        match e {
            Especie::Leitura => {
                leituras.insert(nome.to_string());
            }
            Especie::Escrita => {
                escritas.insert(nome.to_string());
            }
            Especie::Ambas => {
                leituras.insert(nome.to_string());
                escritas.insert(nome.to_string());
            }
        }
    };
    while i < b.len() {
        match b[i] {
            // Conteúdo de string não é acesso a membro (`dart.privateName(L,
            // "C.campo")`); os nomes passados como dado a `dsend`/`dload` vêm
            // de `seletores_dinamicos`, abaixo.
            b'"' | b'\'' | b'`' => {
                let aspa = b[i];
                i += 1;
                while i < b.len() && b[i] != aspa {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'.' if i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_' || b[i + 1] == b'$') => {
                let f = ident_em(b, i + 1);
                // `opts.nome` é a leitura de um parâmetro nomeado no prólogo
                // da função emitida, não acesso a membro.
                if !(i >= 4 && &b[i - 4..i] == b"opts" && (i == 4 || !e_ident(b[i - 5]))) {
                    let e = especie_apos(b, f);
                    registra(&texto[i + 1..f], e, leituras, escritas);
                }
                i = f;
            }
            b'[' if i + 2 < b.len() && b[i + 1] == b'$' => {
                let f = ident_em(b, i + 2);
                if b.get(f) == Some(&b']') {
                    let e = especie_apos(b, f + 1);
                    registra(&texto[i + 2..f], e, leituras, escritas);
                }
                i = f.max(i + 1);
            }
            _ => i += 1,
        }
    }
    let (leituras_d, escritas_d) = crate::sdk::seletores_dinamicos_por_especie(texto);
    leituras.extend(leituras_d);
    escritas.extend(escritas_d);
}

/// Identificador JS → biblioteca, lido do `dart.trackLibraries("…", {"uri":
/// L$ident, …})` que cada módulo emite — sem montar um `Ctx` só para isso.
pub fn bibliotecas_do_texto(modulos: &[(String, String)], program: &Program) -> HashMap<String, LibraryId> {
    let por_uri: HashMap<&str, LibraryId> = program.libraries.iter().enumerate().map(|(i, l)| (l.uri.as_str(), LibraryId(i as u32))).collect();
    let mut out = HashMap::new();
    for (_, t) in modulos {
        let Some(p) = t.find("dart.trackLibraries(") else { continue };
        let Some(rel) = t[p..].find('{') else { continue };
        let abre = p + rel;
        let Some(fecha) = varredura::fecha_chave(t, abre) else { continue };
        for linha in t[abre + 1..fecha].split(",\n") {
            let linha = linha.trim();
            let Some((uri, var)) = linha.rsplit_once(": ") else { continue };
            let uri = uri.trim().trim_matches('"');
            let Some(ident) = var.trim().strip_prefix("L$") else { continue };
            if let Some(&l) = por_uri.get(uri) {
                out.insert(ident.to_string(), l);
            }
        }
    }
    out
}

/// Confere o texto emitido contra o mundo e devolve o que falta.
pub fn conferir(modulos: &[(String, String)], libs: &HashMap<String, LibraryId>, program: &Program, interner: &Interner, mundo: &Mundo) -> Faltas {
    let defs = definicoes(modulos);
    let idents: HashSet<&str> = libs.keys().map(String::as_str).collect();
    let mut refs: HashSet<String> = HashSet::new();
    for (_, t) in modulos {
        referencias(t, &idents, &defs.classes, &mut refs);
    }
    let mut faltas = Faltas::default();
    let mut pendentes: Vec<&String> = refs.iter().filter(|r| !defs.nomes.contains(*r)).collect();
    pendentes.sort();
    for r in pendentes {
        if !traduzir(r, libs, program, interner, &mut faltas) {
            faltas.sem_elemento.push(r.clone());
        }
    }
    // Seletores do texto que são membros podados de classes instanciadas,
    // por espécie: leitura cura getter/método, escrita cura setter. Uma
    // leitura nunca ressuscita um setter — é a precisão que o mundo apura.
    let mut sel_l: HashSet<String> = HashSet::new();
    let mut sel_e: HashSet<String> = HashSet::new();
    for (_, t) in modulos {
        seletores_do_texto(t, &mut sel_l, &mut sel_e);
    }
    let mut novos: HashSet<String> = HashSet::new();
    for (i, c) in program.classes.iter().enumerate() {
        if program.library(c.library).is_sdk || mundo.classe(ClassId(i as u32)) != NivelClasse::Instanciada {
            continue;
        }
        for (&chave, &f) in c.instance_members.iter() {
            let func = program.function(f);
            if mundo.funcao(f) || func.abstract_ {
                continue;
            }
            // A chave carrega a espécie (`foo_=` é escrita); `==` não termina
            // em `_=`, então o corte é seguro.
            let k = interner.resolve(chave);
            let (base, escrita) = match k.strip_suffix("_=") {
                Some(b) => (b, true),
                None => (k, false),
            };
            let js = dartforge_emit_js::body::js_member_name(base);
            let usado = if escrita {
                sel_e.contains(base) || sel_e.contains(js.as_str())
            } else {
                sel_l.contains(base) || sel_l.contains(js.as_str())
            };
            if usado {
                novos.insert(if escrita { format!("{base}_=") } else { base.to_string() });
            }
        }
    }
    let mut novos: Vec<String> = novos.into_iter().collect();
    novos.sort();
    for n in &novos {
        faltas.descricao.push(format!("seletor `{n}` usado no texto e podado em classe instanciada"));
    }
    faltas.seletores = novos;
    faltas
}

/// `L$x.Nome` / `L$x.C.m` → elemento.
fn traduzir(r: &str, libs: &HashMap<String, LibraryId>, program: &Program, interner: &Interner, f: &mut Faltas) -> bool {
    let Some(resto) = r.strip_prefix("L$") else { return false };
    let Some((ident, nomes)) = resto.split_once('.') else { return false };
    let Some(&lib) = libs.get(ident) else { return false };
    let (nome, membro) = match nomes.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (nomes, None),
    };
    let Some(sym) = interner.lookup(nome) else { return false };
    let Some(b) = program.library(lib).declared.get(&sym) else { return false };
    let mut ok = false;
    for el in [b.getter, b.setter].into_iter().flatten() {
        // Tipo de extensão que não é de interop: o emissor nunca escreve a
        // classe (só a receita, nas regras rti dos subtipos).
        if let Element::Class(c) = el {
            if program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
                ok = true;
                continue;
            }
        }
        match (el, membro) {
            (Element::Class(c), None) => {
                f.classes.push(c);
                f.descricao.push(format!("classe {r} citada no texto e podada"));
                ok = true;
            }
            (Element::Class(c), Some(m)) => {
                let class = program.class(c);
                let (tearoff, m) = match m.strip_prefix("_#").and_then(|x| x.strip_suffix("#tearOff")) {
                    Some(x) => (true, x),
                    None => (false, m),
                };
                let ctor_nome = if m == "new" { "" } else { m };
                let mut achou = None;
                for cand in [m, m.strip_suffix('_').unwrap_or(m)] {
                    if let Some(s) = interner.lookup(cand) {
                        if let Some(&x) = class.static_members.get(&s) {
                            achou = Some(x);
                            break;
                        }
                    }
                }
                if achou.is_none() {
                    for cand in [ctor_nome, ctor_nome.strip_suffix('_').unwrap_or(ctor_nome)] {
                        if let Some(s) = interner.lookup(cand) {
                            if let Some(&x) = class.constructors.get(&s) {
                                achou = Some(x);
                                break;
                            }
                        }
                    }
                }
                if achou.is_none() && ctor_nome.is_empty() && class.constructors.is_empty() {
                    f.classes_instanciadas.push(c);
                    f.descricao.push(format!("{r} (classe sem construtor declarado) citado no texto e podado"));
                    ok = true;
                }
                if let Some(x) = achou {
                    if tearoff {
                        f.tearoffs.push(x);
                    } else {
                        f.funcoes.push(x);
                    }
                    f.descricao.push(format!("{r} citado no texto e podado"));
                    ok = true;
                }
            }
            (Element::Function(x), _) => {
                f.funcoes.push(x);
                f.descricao.push(format!("função {r} citada no texto e podada"));
                ok = true;
            }
            (Element::Variable(v), _) => {
                f.variaveis.push(v);
                f.descricao.push(format!("variável {r} citada no texto e podada"));
                ok = true;
            }
            _ => {}
        }
    }
    ok
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn definicoes_de_topo_classe_e_preguicosos() {
        let m = vec![(
            "a.js".to_string(),
            concat!(
                "L$a.C = class C extends core.Object {\n  static f() { return 1; }\n  static ['_#new#tearOff'](...args) { return new L$a.C.new(...args); }\n  g() {}\n};\n",
                "(L$a.C.new = function() {\n}).prototype = L$a.C.prototype;\n",
                "dart.defineLazy(L$a, {\n  get x() {\n    return 1;\n  },\n  get y() {\n    return 2;\n  }\n});\n",
                "L$a.h = function h() { return L$a.C.f() + L$a.x + L$a.z; };\n",
            )
            .to_string(),
        )];
        let d = definicoes(&m);
        for n in ["L$a.C", "L$a.C.f", "L$a.C._#new#tearOff", "L$a.C.new", "L$a.x", "L$a.y", "L$a.h"] {
            assert!(d.nomes.contains(n), "{n}: {:?}", d.nomes);
        }
        assert!(!d.nomes.contains("L$a.C.g"), "membro de instância não é definição estática");
        let idents: HashSet<&str> = ["a"].into_iter().collect();
        let mut r = HashSet::new();
        referencias(&m[0].1, &idents, &d.classes, &mut r);
        let pend: Vec<&String> = r.iter().filter(|x| !d.nomes.contains(*x)).collect();
        assert_eq!(pend, vec!["L$a.z"]);
    }

    #[test]
    fn receita_rti_e_referencia() {
        let idents: HashSet<&str> = ["main"].into_iter().collect();
        let mut r = HashSet::new();
        referencias("x[_is](dart_rti._Universe.eval(u, \"main|D<core|int>\"))", &idents, &HashSet::new(), &mut r);
        assert!(r.contains("L$main.D"), "{r:?}");
        assert!(!r.iter().any(|x| x.contains("core")));
    }

    #[test]
    fn seletores_de_texto() {
        let mut l = HashSet::new();
        let mut e = HashSet::new();
        seletores_do_texto("a.foo(); b[$bar]; dart.dsend(o, \"baz\", []); q.qux = 1; r.cor += 2; s.ok == 3;", &mut l, &mut e);
        assert!(l.contains("foo") && l.contains("bar") && l.contains("baz"), "{l:?} {e:?}");
        assert!(!l.contains("qux") && e.contains("qux"), "{l:?} {e:?}");
        assert!(l.contains("cor") && e.contains("cor"), "{l:?} {e:?}");
        assert!(l.contains("ok") && !e.contains("ok"), "{l:?} {e:?}");
    }
}
