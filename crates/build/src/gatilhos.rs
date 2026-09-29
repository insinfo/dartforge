//! Triggers de builder (`build_runner` ≥ 2.7.0, `build_config` ≥ 1.2.0):
//! porte de `build_plan/build_triggers.dart` e do `_allowedByTriggers` de
//! `build/build.dart` do `build_runner` 2.16.1.
//!
//! Um passo cujo builder tem a opção `run_only_if_triggered: true` (nas
//! opções já fundidas da fase) só roda se algum trigger do builder dispara na
//! entrada primária; sem trigger nenhum, nunca roda. Os triggers vêm da seção
//! `triggers` do `build.yaml` de **todos** os pacotes do build, acumulados por
//! nome de builder (sem normalizar a chave: `anotacao` não é
//! `pacote:anotacao`).
//!
//! * `import <biblioteca>`: a unidade primária tem um `import` cujo URI é
//!   literalmente `package:<biblioteca>` (não vale `export`, nem import
//!   relativo, nem as partes).
//! * `annotation <Nome>`: alguma declaração **de topo** da unidade primária
//!   ou de uma parte incluída (`part '...'`) tem metadado cujo nome escrito é
//!   `Nome` — inteiro (`@Nome`, `@p.Nome` com prefixo, `@Nome.c`) ou sem o
//!   primeiro trecho (`@p.Nome` casa `Nome`; `@p.Nome.c` casa `Nome.c`).
//!
//! O oficial analisa com `parseString(throwIfDiagnostics: false)` e percorre
//! `directives` e `declarations` da `CompilationUnit`. Aqui a mesma
//! informação vem dos tokens do lexer do DartForge, no nível de topo (fora
//! de parênteses, colchetes e chaves): metadado seguido de diretiva é da
//! diretiva, seguido de qualquer outra coisa é da declaração. Uma fonte que o
//! lexer recusa (string ou comentário não terminado) conta como disparada:
//! o builder roda e reporta o erro, em vez de o passo sumir em silêncio.
use crate::config::BuildConfig;
use crate::valor::Valor;
use dartforge_frontend::token::{Kind, Op};
use std::collections::BTreeMap;

/// Um trigger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Gatilho {
    /// `import <biblioteca>`: compara com `package:<biblioteca>`.
    Import(String),
    /// `annotation <Nome>`.
    Anotacao(String),
}

impl Gatilho {
    /// `BuildTrigger._tryParse`: o trigger e, se houver, o aviso.
    fn ler(texto: &str) -> (Option<Gatilho>, Option<String>) {
        if let Some(i) = texto.strip_prefix("import ") {
            let aviso = (!import_valido(i)).then(|| format!("Invalid import trigger: `{i}`"));
            (Some(Gatilho::Import(i.to_string())), aviso)
        } else if let Some(a) = texto.strip_prefix("annotation ") {
            let aviso =
                (!anotacao_valida(a)).then(|| format!("Invalid annotation trigger: `{a}`"));
            (Some(Gatilho::Anotacao(a.to_string())), aviso)
        } else {
            (None, Some(format!("Invalid trigger: `{texto}`")))
        }
    }

    /// Precisa das partes (`checksParts`)?
    fn le_partes(&self) -> bool {
        matches!(self, Gatilho::Anotacao(_))
    }

    /// `triggersOn`.
    fn dispara(&self, unidades: &[Unidade]) -> bool {
        match self {
            Gatilho::Import(i) => {
                let alvo = format!("package:{i}");
                unidades.iter().any(|u| u.imports.iter().any(|x| *x == alvo))
            }
            Gatilho::Anotacao(a) => unidades.iter().any(|u| {
                u.anotacoes.iter().any(|nome| {
                    nome == a
                        || nome
                            .split_once('.')
                            .is_some_and(|(_, resto)| resto == a)
                })
            }),
        }
    }
}

/// `^[a-z][a-z0-9_/.]*$`.
fn import_valido(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|p| p.is_ascii_lowercase())
        && c.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || "_/.".contains(x))
}

/// `^[a-zA-Z_][a-zA-Z0-9]*$` (o `_` só na primeira posição, como no oficial).
fn anotacao_valida(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|p| p.is_ascii_alphabetic() || p == '_')
        && c.all(|x| x.is_ascii_alphanumeric())
}

/// `^[a-z][a-z0-9_]*:[a-z][a-z0-9_]` procurado com `contains`: basta o
/// começo casar.
fn nome_de_builder_valido(s: &str) -> bool {
    let Some((pacote, nome)) = s.split_once(':') else {
        return false;
    };
    let mut p = pacote.chars();
    let mut n = nome.chars();
    p.next().is_some_and(|x| x.is_ascii_lowercase())
        && p.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_')
        && n.next().is_some_and(|x| x.is_ascii_lowercase())
}

/// `BuildTriggers`: os triggers acumulados de todos os pacotes.
#[derive(Debug, Clone, Default)]
pub struct Gatilhos {
    /// Por nome de builder (como escrito no `build.yaml`), sem repetição, na
    /// ordem em que apareceram.
    pub por_builder: BTreeMap<String, Vec<Gatilho>>,
    /// Avisos de leitura, por pacote (`warningsByPackage`).
    pub avisos: Vec<(String, Vec<String>)>,
}

impl Gatilhos {
    /// `BuildTriggers.fromConfigs`, na ordem dos pacotes dada.
    ///
    /// ```
    /// use dartforge_build::config::BuildConfig;
    /// use dartforge_build::gatilhos::{Gatilho, Gatilhos};
    /// use dartforge_build::perfil::Perfil;
    /// let perfil = Perfil::das_versoes(Some("1.3.3"), Some("2.16.1"));
    /// let t = "triggers:\n  p:b:\n    - annotation A\n    - import p/a.dart\n    - outra\n";
    /// let c = BuildConfig::de_texto_no_perfil("p", &[], t, "build.yaml", perfil).unwrap();
    /// let g = Gatilhos::das_configs([&c]);
    /// assert_eq!(g.por_builder["p:b"], [Gatilho::Anotacao("A".into()), Gatilho::Import("p/a.dart".into())]);
    /// assert_eq!(g.avisos[0].1, ["Invalid trigger: `outra`"]);
    /// ```
    pub fn das_configs<'a>(configs: impl IntoIterator<Item = &'a BuildConfig>) -> Gatilhos {
        let mut g = Gatilhos::default();
        for c in configs {
            let mut avisos = Vec::new();
            for (builder, valor) in &c.gatilhos {
                if !nome_de_builder_valido(builder) {
                    avisos.push(format!("Invalid builder name: `{builder}`"));
                }
                let Valor::Lista(itens) = valor else {
                    avisos.push(format!(
                        "Invalid `triggers`, should be a list of triggers: {}",
                        valor.texto_canonico()
                    ));
                    continue;
                };
                let mut novos = Vec::new();
                for item in itens {
                    // Um item que não é texto não é trigger nem aviso.
                    let Valor::Texto(t) = item else { continue };
                    let (gatilho, aviso) = Gatilho::ler(t);
                    novos.extend(gatilho);
                    avisos.extend(aviso);
                }
                if !novos.is_empty() {
                    let v = g.por_builder.entry(builder.clone()).or_default();
                    for n in novos {
                        if !v.contains(&n) {
                            v.push(n);
                        }
                    }
                }
            }
            if !avisos.is_empty() {
                g.avisos.push((c.pacote.clone(), avisos));
            }
        }
        g
    }

    /// Os triggers do builder `chave` (`pacote:nome`). `abreviado`: o
    /// `build_runner` entre 2.7.0 e 2.10.0 procurava `p:p` como `p`.
    pub fn do_builder(&self, chave: &str, abreviado: bool) -> Option<&[Gatilho]> {
        let nome = match chave.split_once(':') {
            Some((p, n)) if abreviado && p == n => p,
            _ => chave,
        };
        self.por_builder.get(nome).map(Vec::as_slice)
    }

    /// `renderWarnings`.
    pub fn texto_dos_avisos(&self) -> Option<String> {
        if self.avisos.is_empty() {
            return None;
        }
        let mut s = String::new();
        for (pacote, avisos) in &self.avisos {
            s.push_str(&format!("build.yaml of package:{pacote}:\n"));
            for a in avisos {
                s.push_str(&format!("  {a}\n"));
            }
        }
        s.push_str("See https://pub.dev/packages/build_config#triggers for valid usage.");
        Some(s)
    }
}

/// O que os triggers olham numa unidade de compilação.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unidade {
    /// `stringValue` de cada `import` (`None` se tem interpolação).
    pub imports: Vec<String>,
    /// URIs das diretivas `part` (não `part of`).
    pub partes: Vec<String>,
    /// Nome escrito do metadado de cada declaração de topo: `Nome`,
    /// `p.Nome`, `Nome.c`, `p.Nome.c`.
    pub anotacoes: Vec<String>,
}

/// Lê diretivas e metadados de topo de uma fonte Dart. `None` quando o lexer
/// recusa a fonte.
///
/// ```
/// use dartforge_build::gatilhos::analisar;
/// let u = analisar("import 'package:a/b.dart' as b;\npart 'x.g.dart';\n@b.Gerar()\nclass A { @Outra() void f() {} }\n").unwrap();
/// assert_eq!(u.imports, ["package:a/b.dart"]);
/// assert_eq!(u.partes, ["x.g.dart"]);
/// assert_eq!(u.anotacoes, ["b.Gerar"]);
/// ```
pub fn analisar(fonte: &str) -> Option<Unidade> {
    let tokens = dartforge_frontend::lexer::lex(fonte).ok()?;
    let texto = |i: usize| tokens[i].text(fonte);
    let e_ident = |i: usize, t: &str| tokens[i].kind == Kind::Ident && texto(i) == t;
    let e_string = |i: usize| {
        matches!(
            tokens[i].kind,
            Kind::Str(_) | Kind::StrBegin(..)
        )
    };
    let mut u = Unidade::default();
    let mut pendentes: Vec<String> = Vec::new();
    let mut profundidade = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let k = tokens[i].kind;
        match k {
            Kind::Eof => break,
            Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                profundidade += 1;
                i += 1;
                continue;
            }
            Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                profundidade = profundidade.saturating_sub(1);
                i += 1;
                continue;
            }
            _ if profundidade > 0 => {
                i += 1;
                continue;
            }
            _ => {}
        }
        if k == Kind::Op(Op::At) {
            // `@a`, `@a.b`, `@a.b.c`, com argumentos de tipo e argumentos
            // opcionais (os parênteses são pulados pelo laço).
            let mut nome = Vec::new();
            let mut j = i + 1;
            if tokens[j].kind == Kind::Ident {
                nome.push(texto(j));
                j += 1;
                while tokens[j].kind == Kind::Op(Op::Dot) && tokens[j + 1].kind == Kind::Ident {
                    nome.push(texto(j + 1));
                    j += 2;
                }
            }
            if tokens[j].kind == Kind::Op(Op::Lt) {
                let mut angulos = 0usize;
                while tokens[j].kind != Kind::Eof {
                    match tokens[j].kind {
                        Kind::Op(Op::Lt) => angulos += 1,
                        Kind::Op(Op::Gt) => angulos = angulos.saturating_sub(1),
                        _ => {}
                    }
                    j += 1;
                    if angulos == 0 {
                        break;
                    }
                }
            }
            if !nome.is_empty() {
                pendentes.push(nome.join("."));
            }
            i = j;
            continue;
        }
        // Diretivas: o metadado antes delas é delas.
        let diretiva = (e_ident(i, "import") || e_ident(i, "export")) && e_string(i + 1)
            || e_ident(i, "part") && (e_string(i + 1) || e_ident(i + 1, "of"))
            || e_ident(i, "library")
                && (tokens[i + 1].kind == Kind::Ident || tokens[i + 1].kind == Kind::Op(Op::Semicolon));
        if diretiva {
            pendentes.clear();
            let valor = valor_de_string(&tokens, fonte, i + 1);
            if e_ident(i, "import") {
                u.imports.extend(valor);
            } else if e_ident(i, "part") && !e_ident(i + 1, "of") {
                u.partes.extend(valor);
            }
            // Até o `;` de topo.
            let mut j = i + 1;
            let mut p = 0usize;
            while tokens[j].kind != Kind::Eof {
                match tokens[j].kind {
                    Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => p += 1,
                    Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => p = p.saturating_sub(1),
                    Kind::Op(Op::Semicolon) if p == 0 => break,
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        // Qualquer outro token de topo começa (ou continua) uma declaração.
        u.anotacoes.append(&mut pendentes);
        i += 1;
    }
    Some(u)
}

/// `StringLiteral.stringValue` a partir do token `i`: strings adjacentes
/// concatenadas; `None` se alguma tem interpolação.
fn valor_de_string(
    tokens: &[dartforge_frontend::token::Token],
    fonte: &str,
    mut i: usize,
) -> Option<String> {
    let mut s = String::new();
    let mut alguma = false;
    loop {
        match tokens[i].kind {
            Kind::Str(f) => {
                let t = tokens[i].text(fonte);
                let t = if f.raw { &t[1..] } else { t };
                let aspas = if f.triple { 3 } else { 1 };
                let conteudo = &t[aspas..t.len() - aspas];
                let v = dartforge_frontend::lexer::decode_string(conteudo, f.raw, f.triple).ok()?;
                s.push_str(&v.to_string_lossy());
                alguma = true;
                i += 1;
            }
            Kind::StrBegin(..) => return None,
            _ => break,
        }
    }
    alguma.then_some(s)
}

/// `_allowedByTriggers` depois da opção: algum trigger dispara? `primaria`:
/// a fonte da entrada; `parte(uri)`: o texto de uma parte legível pela fase
/// (a chamada registra a consulta), ou `None`.
pub fn disparado(
    gatilhos: &[Gatilho],
    primaria: &str,
    mut parte: impl FnMut(&str) -> Option<String>,
) -> bool {
    let Some(unidade) = analisar(primaria) else {
        return true;
    };
    let mut todas: Option<Vec<Unidade>> = None;
    for g in gatilhos {
        if g.le_partes() {
            let unidades = todas.get_or_insert_with(|| {
                let mut v = vec![unidade.clone()];
                for uri in &unidade.partes {
                    if let Some(t) = parte(uri) {
                        // Parte que o lexer recusa: o oficial ainda a
                        // analisa com erros; aqui fica sem declarações.
                        v.extend(analisar(&t));
                    }
                }
                v
            });
            if g.dispara(unidades) {
                return true;
            }
        } else if g.dispara(std::slice::from_ref(&unidade)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod testes {
    use super::*;

    fn anotacoes(f: &str) -> Vec<String> {
        analisar(f).unwrap().anotacoes
    }

    #[test]
    fn metadados_de_topo() {
        assert_eq!(anotacoes("@A() class X {}"), ["A"]);
        assert_eq!(anotacoes("@p.A() class X {}"), ["p.A"]);
        assert_eq!(anotacoes("@p.A.c() class X {}"), ["p.A.c"]);
        assert_eq!(anotacoes("@A<int>() class X {}"), ["A"]);
        assert_eq!(anotacoes("@A\n@B(1, [2]) void f() {}"), ["A", "B"]);
        // Membro, parâmetro e valor de enum não são de topo.
        assert!(anotacoes("class X { @A() void f(@B() int x) {} }").is_empty());
        assert!(anotacoes("enum E { @A() a }").is_empty());
        // Metadado de diretiva não é de declaração.
        assert!(anotacoes("@A() library x;\n@B() import 'a.dart';").is_empty());
        // Comentário e string não contam.
        assert!(anotacoes("// @A()\nconst s = '@A()';").is_empty());
    }

    #[test]
    fn diretivas() {
        let u = analisar(
            "library;\nimport 'package:a/b.dart';\nimport \"package:\" 'a/c.dart';\nimport 'x$y.dart';\nexport 'package:a/d.dart';\nimport 'package:a/e.dart' if (dart.library.io) 'package:a/f.dart';\npart 'p.dart';\n",
        )
        .unwrap();
        assert_eq!(
            u.imports,
            ["package:a/b.dart", "package:a/c.dart", "package:a/e.dart"]
        );
        assert_eq!(u.partes, ["p.dart"]);
        assert!(analisar("part of 'x.dart';").unwrap().partes.is_empty());
    }

    #[test]
    fn disparo() {
        let a = [Gatilho::Anotacao("Gerar".into())];
        assert!(disparado(&a, "@Gerar() class X {}", |_| None));
        assert!(disparado(&a, "@an.Gerar() class X {}", |_| None));
        assert!(!disparado(&a, "@an.Gerar.nomeado() class X {}", |_| None));
        assert!(!disparado(&a, "@Gerar.nomeado() class X {}", |_| None));
        assert!(disparado(&[Gatilho::Anotacao("Gerar.nomeado".into())], "@Gerar.nomeado() class X {}", |_| None));
        // Pelas partes.
        let mut lidas = Vec::new();
        assert!(disparado(&a, "part 'y.dart';\nclass X {}", |u| {
            lidas.push(u.to_string());
            Some("part of 'x.dart';\n@Gerar() class Y {}".into())
        }));
        assert_eq!(lidas, ["y.dart"]);
        // O import não olha partes.
        let i = [Gatilho::Import("a/b.dart".into())];
        assert!(!disparado(&i, "part 'y.dart';", |_| Some("import 'package:a/b.dart';".into())));
        assert!(disparado(&i, "import 'package:a/b.dart';", |_| None));
        assert!(!disparado(&i, "import 'b.dart';", |_| None));
        // Sem triggers, não dispara; fonte que o lexer recusa, dispara.
        assert!(!disparado(&[], "@Gerar() class X {}", |_| None));
        assert!(disparado(&a, "const s = 'aberta", |_| None));
    }

    #[test]
    fn validacao_dos_nomes() {
        assert!(nome_de_builder_valido("p:b"));
        assert!(nome_de_builder_valido("p_1:b"));
        assert!(!nome_de_builder_valido("b"));
        assert!(!nome_de_builder_valido("P:b"));
        assert!(import_valido("a/b_c.dart"));
        assert!(!import_valido("package:a/b.dart"));
        assert!(anotacao_valida("_Gerar"));
        assert!(!anotacao_valida("Ge_rar"));
    }
}
