//! O código de um `@GenerateInjector`: o que o `InjectorEmitter` do oficial
//! (`angular_compiler/emitter/injector.dart`) monta com o `code_builder`, e o
//! `DartFormatter(pageWidth: 1000000)` do `Compiler` formata.
//!
//! Com a página "infinita", o formatador só quebra onde o `code_builder`
//! põe vírgula no fim — em chamada, lista ou mapa com mais de um item. Aí
//! cada item vai numa linha, dois espaços para dentro da linha em que a
//! chamada começa; o corpo de um `=>` já começa quatro para dentro do
//! membro. Os imports são os do `Allocator.simplePrefixing` (`_i1`, `_i2`…,
//! na ordem em que cada biblioteca aparece no texto; `dart:core` sem
//! prefixo), e a unidade inteira do arquivo divide a mesma tabela.
use crate::diretivas::{DI_TOKENS, Dependencia, Token};
use crate::metadados::{FonteDoInjetor, Injetor, Revivido, TipoEscrito};

const INJECTOR: &str = "package:ngdart/src/di/injector.dart";
const UTILITIES: &str = "package:ngdart/src/utilities.dart";

/// A tabela de imports do `Allocator.simplePrefixing`.
#[derive(Default)]
struct Alocador {
    uris: Vec<String>,
}

impl Alocador {
    /// `simbolo` de `uri`, com o prefixo da biblioteca (alocado agora se
    /// ainda não estava).
    fn r(&mut self, uri: &str, simbolo: &str) -> String {
        if uri.is_empty() || uri == "dart:core" {
            return simbolo.to_string();
        }
        let i = match self.uris.iter().position(|u| u == uri) {
            Some(i) => i,
            None => {
                self.uris.push(uri.to_string());
                self.uris.len() - 1
            }
        };
        format!("_i{}.{simbolo}", i + 1)
    }

    fn tipo(&mut self, t: &TipoEscrito) -> String {
        let mut s = self.r(&t.uri, &t.simbolo);
        if !t.args.is_empty() {
            let args: Vec<String> = t.args.iter().map(|a| self.tipo(a)).collect();
            s = format!("{s}<{}>", args.join(", "));
        }
        s
    }
}

/// Uma expressão do `code_builder`, antes da formatação.
enum Ex {
    Atomo(String),
    /// `alvo(args)`: com mais de um argumento, vírgula no fim.
    Chamada {
        alvo: String,
        args: Vec<(Option<String>, Ex)>,
    },
    /// `[..]`, com `const ` ou não em `prefixo`.
    Lista {
        prefixo: String,
        itens: Vec<Ex>,
    },
}

impl Ex {
    /// O texto formatado, com `recuo` o da linha em que a expressão começa.
    fn texto(&self, recuo: usize) -> String {
        let (abre, fecha, itens): (String, &str, Vec<(Option<&str>, &Ex)>) = match self {
            Ex::Atomo(s) => return s.clone(),
            Ex::Chamada { alvo, args } => (
                format!("{alvo}("),
                ")",
                args.iter().map(|(n, e)| (n.as_deref(), e)).collect(),
            ),
            Ex::Lista { prefixo, itens } => (
                format!("{prefixo}["),
                "]",
                itens.iter().map(|e| (None, e)).collect(),
            ),
        };
        let item = |n: Option<&str>, e: &Ex, r: usize| match n {
            Some(n) => format!("{n}: {}", e.texto(r)),
            None => e.texto(r),
        };
        if itens.len() > 1 {
            let dentro = " ".repeat(recuo + 2);
            let mut s = format!("{abre}\n");
            for (n, e) in itens {
                s += &format!("{dentro}{},\n", item(n, e, recuo + 2));
            }
            s + &" ".repeat(recuo) + fecha
        } else {
            let meio: Vec<String> = itens.into_iter().map(|(n, e)| item(n, e, recuo)).collect();
            format!("{abre}{}{fecha}", meio.join(""))
        }
    }
}

/// `literalString`: aspas simples, com `'` e quebra de linha escapados.
fn literal_de_texto(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'").replace('\n', "\\n"))
}

/// `_reviveString`: `\` dobrada, `$` escapado (menos depois de `\`), quebra
/// de linha como `\n` e o que sai do ASCII visível como `\u{..}`.
fn texto_revivido(s: &str) -> String {
    let s = s.replace('\\', "\\\\");
    let cs: Vec<char> = s.chars().collect();
    let mut escapado = String::new();
    for (i, &c) in cs.iter().enumerate() {
        if c == '$' && (i == 0 || cs[i - 1] != '\\') {
            escapado += "\\$";
        } else if c == '\n' {
            escapado += "\\n";
        } else {
            escapado.push(c);
        }
    }
    let unicode: String = escapado
        .chars()
        .map(|c| {
            let n = c as u32;
            if !(0x20..=0x7E).contains(&n) {
                format!("\\u{{{n:x}}}")
            } else {
                c.to_string()
            }
        })
        .collect();
    literal_de_texto(&unicode)
}

struct Emissor<'a> {
    al: &'a mut Alocador,
}

impl Emissor<'_> {
    /// `_tokenToIdentifier`.
    fn token(&mut self, t: &Token) -> Result<Ex, String> {
        Ok(match t {
            Token::Classe { uri, classe } => Ex::Atomo(self.al.r(uri, classe)),
            Token::Opaco { nome, tipo } | Token::Multi { nome, tipo } => {
                let classe = if matches!(t, Token::Multi { .. }) {
                    "MultiToken"
                } else {
                    "OpaqueToken"
                };
                let c = self.al.r(DI_TOKENS, classe);
                let t = self.al.tipo(&crate::metadados::tipo_escrito_do_token(tipo));
                let args = if nome.is_empty() {
                    Vec::new()
                } else {
                    vec![(None, Ex::Atomo(literal_de_texto(nome)))]
                };
                Ex::Chamada {
                    alvo: format!("const {c}<{t}>"),
                    args,
                }
            }
            Token::Elemento | Token::Detector => {
                return Err("HtmlElement/ChangeDetectorRef no injetor".into());
            }
        })
    }

    /// `_computeDependencies`.
    fn dependencia(&mut self, d: &Dependencia) -> Result<Ex, String> {
        let chamada = |alvo: &str, tok: Ex, opcional: bool| {
            let mut args = vec![(None, tok)];
            if opcional {
                args.push((None, Ex::Atomo("null".into())));
            }
            Ex::Chamada {
                alvo: alvo.to_string(),
                args,
            }
        };
        Ok(if d.proprio {
            let alvo = if d.opcional {
                "injectFromSelfOptional"
            } else {
                "injectFromSelf"
            };
            chamada(alvo, self.token(&d.token)?, d.opcional)
        } else if d.pular {
            if d.opcional {
                let cast = self.al.r(UTILITIES, "unsafeCast");
                let dentro = chamada("injectFromAncestryOptional", self.token(&d.token)?, true);
                Ex::Chamada {
                    alvo: cast,
                    args: vec![(None, dentro)],
                }
            } else {
                chamada("injectFromAncestry", self.token(&d.token)?, false)
            }
        } else if d.hospedeiro {
            let alvo = if d.opcional {
                "injectFromParentOptional"
            } else {
                "injectFromParent"
            };
            chamada(alvo, self.token(&d.token)?, d.opcional)
        } else if d.opcional {
            chamada("provideUntyped", self.token(&d.token)?, true)
        } else {
            chamada("this.get", self.token(&d.token)?, false)
        })
    }

    fn dependencias(&mut self, deps: &[Dependencia]) -> Result<Vec<(Option<String>, Ex)>, String> {
        deps.iter()
            .map(|d| Ok((None, self.dependencia(d)?)))
            .collect()
    }

    /// Um valor revivido; `em_const`: já dentro de uma expressão `const`
    /// (o `const` dos de dentro sai).
    fn valor(&mut self, v: &Revivido, em_const: bool) -> Ex {
        let konst = if em_const { "" } else { "const " };
        match v {
            Revivido::Nulo => Ex::Atomo("null".into()),
            Revivido::Texto(s) => Ex::Atomo(texto_revivido(s)),
            Revivido::Inteiro(i) => Ex::Atomo(i.to_string()),
            Revivido::Booleano(b) => Ex::Atomo(b.to_string()),
            Revivido::Acesso { uri, nome } => Ex::Atomo(self.al.r(uri, nome)),
            Revivido::Lista(itens) => Ex::Lista {
                prefixo: konst.to_string(),
                itens: itens.iter().map(|i| self.valor(i, true)).collect(),
            },
            Revivido::Objeto {
                uri,
                classe,
                construtor,
                posicionais,
                nomeados,
            } => {
                let mut alvo = format!("{konst}{}", self.al.r(uri, classe));
                if let Some(c) = construtor {
                    alvo = format!("{alvo}.{c}");
                }
                let mut args: Vec<(Option<String>, Ex)> = posicionais
                    .iter()
                    .map(|p| (None, self.valor(p, true)))
                    .collect();
                for (n, x) in nomeados {
                    args.push((Some(n.clone()), self.valor(x, true)));
                }
                Ex::Chamada { alvo, args }
            }
        }
    }
}

/// O recuo do corpo de um `=>` num membro de classe.
const RECUO_DO_CORPO: usize = 6;

/// O texto dos injetores do arquivo: os imports (na ordem da tabela) e o
/// corpo (a partir do `// ignore_for_file`).
pub fn emitir(injetores: &[&Injetor]) -> Result<(String, String), String> {
    let mut al = Alocador::default();
    let mut blocos = Vec::new();
    for inj in injetores {
        blocos.push(emitir_um(&mut al, inj)?);
    }
    let imports: String = al
        .uris
        .iter()
        .enumerate()
        .map(|(i, u)| format!("import '{}' as _i{};\n", crate::dialeto::escrita(u), i + 1))
        .collect();
    let corpo = format!(
        "// ignore_for_file: no_leading_underscores_for_library_prefixes\n{}",
        blocos.join("\n")
    );
    Ok((imports, corpo))
}

fn emitir_um(al: &mut Alocador, inj: &Injetor) -> Result<String, String> {
    let classe = format!("_Injector${}", inj.nome);
    let injector = al.r(INJECTOR, "Injector");
    let mut s = format!(
        "{injector} {}$Injector({injector} parent) => {classe}._(parent);\n\n",
        inj.nome
    );
    let hierarquico = al.r(INJECTOR, "HierarchicalInjector");
    s += &format!("class {classe} extends {hierarquico} implements {injector} {{\n");
    s += &format!("  {classe}._({injector} parent) : super(parent);\n\n");
    // Os campos vêm antes dos métodos no texto (`visitClass`), e é nessa
    // ordem que os imports são alocados.
    let mut em = Emissor { al };
    let mut campos = Vec::new();
    for (i, p) in inj.provedores.iter().enumerate() {
        match &p.fonte {
            FonteDoInjetor::Classe { uri, classe, .. } => {
                let t = em.al.r(uri, classe);
                campos.push(format!("  {t}? _field{i};\n\n"));
            }
            FonteDoInjetor::Fabrica { .. } => {
                let t = em.al.tipo(&p.tipo);
                campos.push(format!("  {t}? _field{i};\n\n"));
            }
            _ => {}
        }
    }
    let mut metodos = Vec::new();
    let mut unicos: Vec<String> = Vec::new();
    let mut multi: Vec<(Token, Vec<String>)> = Vec::new();
    for (i, p) in inj.provedores.iter().enumerate() {
        let (retorno, nome, corpo) = match &p.fonte {
            FonteDoInjetor::Classe {
                uri,
                classe,
                construtor,
                deps,
            } => {
                let t = em.al.r(uri, classe);
                let alvo = match construtor {
                    Some(c) => format!("{t}.{c}"),
                    None => t.clone(),
                };
                let args = em.dependencias(deps)?;
                let chamada = Ex::Chamada { alvo, args };
                (
                    t,
                    format!("_get{classe}${i}"),
                    format!("_field{i} ??= {}", chamada.texto(RECUO_DO_CORPO)),
                )
            }
            FonteDoInjetor::Fabrica { uri, nome, deps } => {
                let t = em.al.tipo(&p.tipo);
                let f = em.al.r(uri, nome);
                let args = em.dependencias(deps)?;
                let chamada = Ex::Chamada { alvo: f, args };
                (
                    t,
                    format!("_get{}${i}", p.tipo.simbolo),
                    format!("_field{i} ??= {}", chamada.texto(RECUO_DO_CORPO)),
                )
            }
            FonteDoInjetor::Existente(alvo) => {
                let t = em.al.tipo(&p.tipo);
                let tok = em.token(alvo)?;
                let chamada = Ex::Chamada {
                    alvo: "this.get".into(),
                    args: vec![(None, tok)],
                };
                (
                    t,
                    format!("_getExisting${i}"),
                    chamada.texto(RECUO_DO_CORPO),
                )
            }
            FonteDoInjetor::Valor(v) => {
                let t = em.al.tipo(&p.tipo);
                let valor = em.valor(v, false);
                (
                    t,
                    format!("_get{}${i}", p.tipo.simbolo),
                    valor.texto(RECUO_DO_CORPO),
                )
            }
        };
        metodos.push(format!("  {retorno} {nome}() => {corpo};\n\n"));
        if p.multi {
            match multi.iter_mut().find(|(t, _)| *t == p.token) {
                Some((_, l)) => l.push(nome),
                None => multi.push((p.token.clone(), vec![nome])),
            }
        } else {
            unicos.push(nome);
        }
    }
    let n = inj.provedores.len();
    metodos.push(format!("  {injector} _getInjector${n}() => this;\n\n"));
    // O `injectFromSelfOptional`: os tokens entram no texto (e na tabela)
    // aqui, depois de todos os métodos.
    let nao_achado = em.al.r(INJECTOR, "throwIfNotFound");
    let mut corpo = String::new();
    let mut k = 0;
    for p in inj.provedores.iter() {
        if p.multi {
            continue;
        }
        let tok = em.token(&p.token)?.texto(6);
        corpo += &format!(
            "    if (identical(token, {tok})) {{\n      return {}();\n    }}\n",
            unicos[k]
        );
        k += 1;
    }
    corpo += &format!(
        "    if (identical(token, {injector})) {{\n      return _getInjector${n}();\n    }}\n"
    );
    for (t, ms) in &multi {
        let tok = em.token(t)?.texto(6);
        let lista = Ex::Lista {
            prefixo: String::new(),
            itens: ms.iter().map(|m| Ex::Atomo(format!("{m}()"))).collect(),
        };
        corpo += &format!(
            "    if (identical(token, {tok})) {{\n      return {};\n    }}\n",
            lista.texto(6)
        );
    }
    s += &campos.concat();
    s += &metodos.concat();
    s += &format!(
        "  @override\n  Object? injectFromSelfOptional(\n    Object token, [\n    Object? orElse = {nao_achado},\n  ]) {{\n{corpo}    return orElse;\n  }}\n}}\n"
    );
    Ok(s)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn texto_revivido_como_o_oficial() {
        assert_eq!(texto_revivido("it's $x\n"), r"'it\'s \$x\n'");
        assert_eq!(texto_revivido("é 😀 \\"), r"'\u{e9} \u{1f600} \\'");
    }

    #[test]
    fn chamada_com_dois_argumentos_quebra() {
        let e = Ex::Chamada {
            alvo: "f".into(),
            args: vec![
                (None, Ex::Atomo("a".into())),
                (
                    None,
                    Ex::Chamada {
                        alvo: "g".into(),
                        args: vec![(None, Ex::Atomo("b".into()))],
                    },
                ),
            ],
        };
        assert_eq!(e.texto(6), "f(\n        a,\n        g(b),\n      )");
    }
}
