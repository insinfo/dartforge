//! Montagem da biblioteca de augmentation a partir dos resultados
//! estruturados das aplicações (spec de macros, `:288-498`, Regras 1–4).
//!
//! O critério é **byte a byte** com o texto que o CFE 3.6.2 gera (o oráculo
//! é o texto que ele grava no `.dill`, extraído por
//! `scripts/oraculo_augmentation.dart`): o objetivo declarado da spec
//! (`:290-294`) é que ferramentas diferentes produzam a mesma biblioteca. O
//! algoritmo segue o do `buildAugmentationLibrary` da 1ª geração, que é o que
//! o CFE executa:
//!
//! * as declarações de topo de cada resultado saem na ordem dos resultados,
//!   cada uma seguida de `\n`;
//! * tudo o que aumenta um tipo é fundido num único `augment <tipo> Nome`
//!   (Regra 1), na ordem em que o tipo apareceu pela primeira vez, com
//!   `extends`, `with`, `implements`, os valores de enum e os membros (cada
//!   um seguido de `\n`) na ordem dos resultados (Regra 2);
//! * um [`Identificador`](Parte::Ident) vira o nome com o prefixo do import
//!   da sua biblioteca (`prefix0`, `prefix1`… na ordem de primeiro uso; o
//!   nome-base é o primeiro de `prefix`, `prefix0`, `prefix1`… que não
//!   aparece em nenhum texto, com `_` se precisou de índice), `this.` para
//!   membro de instância sem receptor, e `Classe.` para membro estático;
//! * o cabeçalho, os imports e uma linha em branco vêm antes.
use serde_json::Value;

/// Um pedaço de código estruturado, como a macro o montou.
#[derive(Debug, Clone, PartialEq)]
pub struct Codigo {
    /// `CodeKind` (`declaration`, `raw`…); não muda o texto.
    pub tipo: String,
    pub partes: Vec<Parte>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parte {
    Texto(String),
    /// Identificador do hospedeiro (chave do modelo).
    Ident(u64),
    /// Tipo omitido (chave do modelo): vira o tipo inferido.
    Omitido(u64),
    Codigo(Codigo),
}

/// O `MacroExecutionResult` de uma aplicação numa fase. Os pares
/// `(identificador do tipo, …)` estão na ordem em que a macro os produziu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resultado {
    pub diagnosticos: Vec<Value>,
    pub excecao: Option<Value>,
    pub valores_de_enum: Vec<(u64, Vec<Codigo>)>,
    pub extends: Vec<(u64, Codigo)>,
    pub interfaces: Vec<(u64, Vec<Codigo>)>,
    pub biblioteca: Vec<Codigo>,
    pub mixins: Vec<(u64, Vec<Codigo>)>,
    pub tipos_novos: Vec<String>,
    pub tipos: Vec<(u64, Vec<Codigo>)>,
}

/// Como um identificador aparece no texto (`IdentifierKind` + URI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDeIdentificador {
    /// Declaração de topo: prefixo do import da biblioteca, se houver URI.
    Topo,
    /// Membro estático ou construtor: `prefixo.Classe.nome`.
    Estatico,
    /// Membro de instância: `this.nome` sem receptor explícito.
    Instancia,
    /// Parâmetro ou local: só o nome.
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentificadorResolvido {
    pub nome: String,
    pub tipo: TipoDeIdentificador,
    /// URI de import da biblioteca da declaração (a do CFE: `package:` ou a
    /// `file:` canônica), quando o identificador precisa de prefixo.
    pub uri: Option<String>,
    /// Nome da classe, para membro estático.
    pub escopo: Option<String>,
}

/// O que a montagem precisa saber de um tipo aumentado para o cabeçalho
/// `augment <modificadores> <palavra> Nome<T…> `.
#[derive(Debug, Clone, PartialEq)]
pub struct TipoAumentado {
    /// `class`, `enum`, `extension` ou `mixin`.
    pub palavra: &'static str,
    /// Na ordem do CFE: `abstract base external final interface mixin
    /// sealed` (classe) ou `base` (mixin).
    pub modificadores: Vec<&'static str>,
    pub nome: String,
    /// O código de cada parâmetro de tipo (`T extends B`).
    pub parametros: Vec<Codigo>,
}

/// As respostas do hospedeiro de que a montagem depende.
pub trait Resolvedor {
    fn identificador(&self, id: u64) -> Result<IdentificadorResolvido, String>;
    fn tipo_aumentado(&self, id: u64) -> Result<TipoAumentado, String>;
    /// O tipo inferido de um tipo omitido (`None`: não inferido).
    fn tipo_inferido(&self, omitido: u64) -> Option<Codigo>;
}

/// O cabeçalho da biblioteca montada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forma<'a> {
    /// `augment library '<uri>';` — a do CFE 3.6.2 (oráculo e materialização
    /// para o SDK 3.6).
    BibliotecaDeAugmentation(&'a str),
    /// `part of '<uri>';` — a forma atual (materialização para o 3.13.4).
    Parte(&'a str),
}

#[derive(Debug, Clone)]
enum Aplicada {
    Texto(String),
    /// Nome sintetizado (prefixo de import ou tipo omitido), por índice.
    Nome(usize),
}

struct Montador<'r> {
    r: &'r dyn Resolvedor,
    /// URI → índice do nome sintetizado do prefixo, na ordem de primeiro uso.
    imports: Vec<(String, usize)>,
    nomes: Vec<String>,
    partes_de_import: Vec<Aplicada>,
    partes: Vec<Aplicada>,
    textos: Vec<String>,
    buffer: Vec<String>,
    ultima: String,
    erro: Option<String>,
}

impl<'r> Montador<'r> {
    fn texto(&mut self, s: &str) {
        self.ultima = s.to_string();
        self.buffer.push(s.to_string());
    }

    fn descarregar(&mut self) {
        for s in self.buffer.drain(..) {
            self.partes.push(Aplicada::Texto(s.clone()));
            self.textos.push(s);
        }
    }

    fn nome(&mut self, n: usize) {
        self.descarregar();
        self.ultima.clear();
        self.partes.push(Aplicada::Nome(n));
    }

    fn falhar(&mut self, e: String) {
        self.erro.get_or_insert(e);
    }

    fn ident(&mut self, id: u64) {
        let r = match self.r.identificador(id) {
            Ok(r) => r,
            Err(e) => return self.falhar(e),
        };
        let prefixo = r.uri.as_ref().map(|uri| match self.imports.iter().find(|(u, _)| u == uri) {
            Some(&(_, n)) => n,
            None => {
                let n = self.nomes.len();
                self.nomes.push(String::new());
                self.imports.push((uri.clone(), n));
                self.partes_de_import.push(Aplicada::Texto(format!("import '{uri}' as ")));
                self.partes_de_import.push(Aplicada::Nome(n));
                self.partes_de_import.push(Aplicada::Texto(";\n".into()));
                n
            }
        });
        if r.tipo == TipoDeIdentificador::Instancia {
            if !self.ultima.trim_end().ends_with('.') {
                self.texto("this.");
            }
        } else if let Some(p) = prefixo {
            self.nome(p);
            self.texto(".");
        }
        if r.tipo == TipoDeIdentificador::Estatico {
            let escopo = r.escopo.clone().unwrap_or_default();
            self.texto(&format!("{escopo}."));
        }
        self.texto(&r.nome);
    }

    fn codigo(&mut self, c: &Codigo) {
        for p in &c.partes {
            match p {
                Parte::Texto(s) => self.texto(s),
                Parte::Codigo(c) => self.codigo(c),
                Parte::Ident(id) => self.ident(*id),
                Parte::Omitido(chave) => match self.r.tipo_inferido(*chave) {
                    Some(c) => self.codigo(&c),
                    None => self.falhar(format!("nenhum tipo inferido para o tipo omitido {chave}")),
                },
            }
        }
    }

    fn texto_final(&self, forma: Forma<'_>) -> String {
        fn nome<'a>(nomes: &'a [String], a: &'a Aplicada) -> &'a str {
            match a {
                Aplicada::Texto(s) => s.as_str(),
                Aplicada::Nome(n) => nomes[*n].as_str(),
            }
        }
        let mut s = match forma {
            Forma::BibliotecaDeAugmentation(uri) => format!("augment library '{uri}';\n\n"),
            Forma::Parte(uri) => format!("part of '{uri}';\n\n"),
        };
        for a in &self.partes_de_import {
            s.push_str(nome(&self.nomes, a));
        }
        if !self.partes_de_import.is_empty() {
            s.push('\n');
        }
        for a in &self.partes {
            s.push_str(nome(&self.nomes, a));
        }
        s
    }
}

/// Um prefixo que não aparece em nenhum texto: `nome`, senão `nome0`,
/// `nome1`… (com `_` no fim quando precisou de índice > 0, para os dígitos
/// do índice de import não grudarem). O algoritmo é o da 1ª geração, com a
/// mesma peculiaridade: cada texto só faz o índice avançar enquanto o
/// contém, sem voltar aos anteriores.
fn prefixo_novo(textos: &[String], nome: &str) -> String {
    let mut indice: i64 = -1;
    let mut prefixo = nome.to_string();
    for t in textos {
        while t.contains(&prefixo) {
            indice += 1;
            prefixo = format!("{nome}{indice}");
        }
    }
    if indice > 0 {
        prefixo.push('_');
    }
    prefixo
}

/// Um mapa por identificador de tipo que guarda a ordem da primeira
/// inserção (a do `Map` do Dart).
struct Ordenado<V> {
    itens: Vec<(u64, V)>,
}

impl<V> Ordenado<V> {
    fn novo() -> Self {
        Ordenado { itens: Vec::new() }
    }
    fn entrada(&mut self, k: u64, f: impl FnOnce() -> V) -> &mut V {
        let i = match self.itens.iter().position(|(c, _)| *c == k) {
            Some(i) => i,
            None => {
                self.itens.push((k, f()));
                self.itens.len() - 1
            }
        };
        &mut self.itens[i].1
    }
    fn get(&self, k: u64) -> Option<&V> {
        self.itens.iter().find(|(c, _)| *c == k).map(|(_, v)| v)
    }
    fn chaves(&self) -> impl Iterator<Item = u64> + '_ {
        self.itens.iter().map(|(k, _)| *k)
    }
}

/// Monta o texto da biblioteca de augmentation de `resultados` (na ordem de
/// execução: fase, depois aplicação). Erro se um identificador ou tipo não
/// resolve, ou se um tipo recebe dois `extends`.
pub fn montar(resultados: &[Resultado], r: &dyn Resolvedor, forma: Forma<'_>) -> Result<String, String> {
    let mut m = Montador {
        r,
        imports: Vec::new(),
        nomes: Vec::new(),
        partes_de_import: Vec::new(),
        partes: Vec::new(),
        textos: Vec::new(),
        buffer: Vec::new(),
        ultima: String::new(),
        erro: None,
    };
    let mut membros: Ordenado<Vec<&Codigo>> = Ordenado::novo();
    let mut valores: Ordenado<Vec<&Codigo>> = Ordenado::novo();
    let mut extends: Ordenado<&Codigo> = Ordenado::novo();
    let mut interfaces: Ordenado<Vec<&Codigo>> = Ordenado::novo();
    let mut mixins: Ordenado<Vec<&Codigo>> = Ordenado::novo();
    // A ordem dos tipos aumentados: a do conjunto `{...enum, ...extends,
    // ...interfaces, ...mixins, ...membros}` da 1ª geração — por categoria,
    // e dentro dela por primeira aparição.
    for res in resultados {
        for c in &res.biblioteca {
            m.codigo(c);
            m.texto("\n");
        }
        for (id, cs) in &res.valores_de_enum {
            valores.entrada(*id, Vec::new).extend(cs.iter());
        }
        for (id, c) in &res.extends {
            if extends.get(*id).is_some() {
                let nome = r.identificador(*id).map(|i| i.nome).unwrap_or_default();
                return Err(format!("A class cannot extend multiple classes: {nome}"));
            }
            *extends.entrada(*id, || c) = c;
        }
        for (id, cs) in &res.interfaces {
            interfaces.entrada(*id, Vec::new).extend(cs.iter());
        }
        for (id, cs) in &res.mixins {
            mixins.entrada(*id, Vec::new).extend(cs.iter());
        }
        for (id, cs) in &res.tipos {
            membros.entrada(*id, Vec::new).extend(cs.iter());
        }
    }
    let mut tipos: Vec<u64> = Vec::new();
    for k in valores.chaves().chain(extends.chaves()).chain(interfaces.chaves()).chain(mixins.chaves()).chain(membros.chaves()) {
        if !tipos.contains(&k) {
            tipos.push(k);
        }
    }
    for id in tipos {
        let t = r.tipo_aumentado(id)?;
        let mut palavras = t.modificadores.join(" ");
        if !palavras.is_empty() {
            palavras.push(' ');
        }
        let com_parametros = !t.parametros.is_empty();
        m.texto(&format!("augment {palavras}{} {}{}", t.palavra, t.nome, if com_parametros { "" } else { " " }));
        if com_parametros {
            m.texto("<");
            for (i, p) in t.parametros.iter().enumerate() {
                if i > 0 {
                    m.texto(", ");
                }
                m.codigo(p);
            }
            m.texto("> ");
        }
        if let Some(sup) = extends.get(id) {
            m.texto("extends ");
            m.codigo(sup);
            m.texto(" ");
        }
        if let Some(ms) = mixins.get(id).filter(|v| !v.is_empty()) {
            m.texto("with ");
            for (i, c) in ms.iter().enumerate() {
                if i > 0 {
                    m.texto(", ");
                }
                m.codigo(c);
            }
            m.texto(" ");
        }
        if let Some(is) = interfaces.get(id).filter(|v| !v.is_empty()) {
            m.texto("implements ");
            for (i, c) in is.iter().enumerate() {
                if i > 0 {
                    m.texto(", ");
                }
                m.codigo(c);
            }
            m.texto(" ");
        }
        m.texto("{\n");
        if t.palavra == "enum" {
            for c in valores.get(id).into_iter().flatten() {
                m.codigo(c);
            }
            m.texto(";\n");
        }
        for c in membros.get(id).into_iter().flatten() {
            m.codigo(c);
            m.texto("\n");
        }
        m.texto("}\n");
    }
    m.descarregar();
    if let Some(e) = m.erro.take() {
        return Err(e);
    }
    if !m.imports.is_empty() {
        let base = prefixo_novo(&m.textos, "prefix");
        for (i, (_, n)) in m.imports.clone().into_iter().enumerate() {
            m.nomes[n] = format!("{base}{i}");
        }
    }
    Ok(m.texto_final(forma))
}

// ----------------------------------------------------------------- JSON

/// `Code` do protocolo (`{"k", "p"}`; parte = texto, `{"i", "n"}`, `{"o"}` ou
/// código aninhado).
pub fn codigo_de_json(v: &Value) -> Result<Codigo, String> {
    let tipo = v.get("k").and_then(Value::as_str).unwrap_or("raw").to_string();
    let partes = v
        .get("p")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("código sem partes: {v}"))?
        .iter()
        .map(|p| match p {
            Value::String(s) => Ok(Parte::Texto(s.clone())),
            Value::Object(o) if o.contains_key("i") => {
                o["i"].as_u64().map(Parte::Ident).ok_or_else(|| format!("identificador inválido: {p}"))
            }
            Value::Object(o) if o.contains_key("o") => {
                o["o"].as_u64().map(Parte::Omitido).ok_or_else(|| format!("tipo omitido inválido: {p}"))
            }
            _ => codigo_de_json(p).map(Parte::Codigo),
        })
        .collect::<Result<_, _>>()?;
    Ok(Codigo { tipo, partes })
}

fn pares_de_listas(v: &Value, campo: &str) -> Result<Vec<(u64, Vec<Codigo>)>, String> {
    let Some(l) = v.get(campo).and_then(Value::as_array) else { return Ok(Vec::new()) };
    l.iter()
        .map(|par| {
            let id = par.get(0).and_then(Value::as_u64).ok_or_else(|| format!("{campo}: par inválido"))?;
            let cs = par
                .get(1)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{campo}: lista inválida"))?
                .iter()
                .map(codigo_de_json)
                .collect::<Result<_, _>>()?;
            Ok((id, cs))
        })
        .collect()
}

/// O `resultado` de um `macro.resultado`.
pub fn resultado_de_json(v: &Value) -> Result<Resultado, String> {
    let extends = match v.get("extends").and_then(Value::as_array) {
        None => Vec::new(),
        Some(l) => l
            .iter()
            .map(|par| {
                let id = par.get(0).and_then(Value::as_u64).ok_or("extends: par inválido")?;
                Ok((id, codigo_de_json(par.get(1).ok_or("extends: sem código")?)?))
            })
            .collect::<Result<_, String>>()?,
    };
    Ok(Resultado {
        diagnosticos: v.get("diagnosticos").and_then(Value::as_array).cloned().unwrap_or_default(),
        excecao: v.get("excecao").filter(|e| !e.is_null()).cloned(),
        valores_de_enum: pares_de_listas(v, "valoresDeEnum")?,
        extends,
        interfaces: pares_de_listas(v, "interfaces")?,
        biblioteca: v
            .get("biblioteca")
            .and_then(Value::as_array)
            .map(|l| l.iter().map(codigo_de_json).collect::<Result<_, _>>())
            .transpose()?
            .unwrap_or_default(),
        mixins: pares_de_listas(v, "mixins")?,
        tipos_novos: v
            .get("tiposNovos")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        tipos: pares_de_listas(v, "tipos")?,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    fn t(s: &str) -> Parte {
        Parte::Texto(s.into())
    }
    fn c(partes: Vec<Parte>) -> Codigo {
        Codigo { tipo: "raw".into(), partes }
    }

    struct Tabela {
        ids: HashMap<u64, IdentificadorResolvido>,
        tipos: HashMap<u64, TipoAumentado>,
    }
    impl Resolvedor for Tabela {
        fn identificador(&self, id: u64) -> Result<IdentificadorResolvido, String> {
            self.ids.get(&id).cloned().ok_or(format!("id {id}"))
        }
        fn tipo_aumentado(&self, id: u64) -> Result<TipoAumentado, String> {
            self.tipos.get(&id).cloned().ok_or(format!("tipo {id}"))
        }
        fn tipo_inferido(&self, _: u64) -> Option<Codigo> {
            None
        }
    }

    fn topo(nome: &str, uri: &str) -> IdentificadorResolvido {
        IdentificadorResolvido { nome: nome.into(), tipo: TipoDeIdentificador::Topo, uri: Some(uri.into()), escopo: None }
    }
    fn inst(nome: &str) -> IdentificadorResolvido {
        IdentificadorResolvido { nome: nome.into(), tipo: TipoDeIdentificador::Instancia, uri: None, escopo: None }
    }

    /// O `@JsonCodable` de `Usuario` (fases 2 e 3), montado à mão a partir
    /// do que o `package:json` 0.20.4 produz: sai o texto que o CFE 3.6.2
    /// grava, byte a byte (`corpus/macros/402_aug_json_saida_cfe`).
    #[test]
    fn json_codable_como_o_cfe() {
        let core = "dart:core";
        let mut ids = HashMap::new();
        ids.insert(1, topo("Map", core));
        ids.insert(2, topo("String", core));
        ids.insert(3, topo("Object", core));
        ids.insert(4, topo("int", core));
        ids.insert(5, topo("List", core));
        ids.insert(10, inst("nome"));
        ids.insert(11, inst("idade"));
        ids.insert(12, inst("apelido"));
        ids.insert(13, inst("notas"));
        ids.insert(20, IdentificadorResolvido { nome: "json".into(), tipo: TipoDeIdentificador::Local, uri: None, escopo: None });
        let mut tipos = HashMap::new();
        tipos.insert(
            100,
            TipoAumentado { palavra: "class", modificadores: vec![], nome: "Usuario".into(), parametros: vec![] },
        );
        let tabela = Tabela { ids, tipos };
        let map_so = || c(vec![Parte::Ident(1), t("<"), Parte::Ident(2), t(", "), Parte::Codigo(c(vec![Parte::Ident(3), t("?")])), t(">")]);
        let fase2 = Resultado {
            tipos: vec![(
                100,
                vec![
                    c(vec![t("  external "), t("Usuario"), t(".fromJson("), Parte::Codigo(map_so()), t(" json);")]),
                    c(vec![t("  external "), Parte::Codigo(map_so()), t(" toJson();")]),
                ],
            )],
            ..Default::default()
        };
        let campo = |id: u64, conv: Vec<Parte>| c(vec![Parte::Ident(id), t(" = "), Parte::Codigo(c(conv))]);
        let json_de = |nome: &str| c(vec![Parte::Ident(20), t("[r'"), t(nome), t("']")]);
        let ctor = c(vec![
            t("  "),
            t("augment "),
            t("Usuario"),
            t("."),
            t("fromJson"),
            t("("),
            Parte::Codigo(c(vec![Parte::Codigo(map_so()), t(" "), t("json")])),
            t(", "),
            t(")"),
            t("\n      : "),
            Parte::Codigo(campo(10, vec![Parte::Codigo(json_de("nome")), t(" as "), Parte::Ident(2)])),
            t(",\n        "),
            Parte::Codigo(campo(11, vec![Parte::Codigo(json_de("idade")), t(" as "), Parte::Ident(4)])),
            t(",\n        "),
            Parte::Codigo(campo(12, vec![Parte::Codigo(json_de("apelido")), t(" as "), Parte::Codigo(c(vec![Parte::Ident(2), t("?")]))])),
            t(",\n        "),
            Parte::Codigo(campo(
                13,
                vec![
                    t("[ for (final item in "),
                    Parte::Codigo(json_de("notas")),
                    t(" as "),
                    Parte::Codigo(c(vec![Parte::Ident(5), t("<"), Parte::Codigo(c(vec![Parte::Ident(3), t("?")])), t(">")])),
                    t(") "),
                    Parte::Codigo(c(vec![Parte::Codigo(c(vec![t("item")])), t(" as "), Parte::Ident(4)])),
                    t("]"),
                ],
            )),
            t(";"),
        ]);
        let entrada = |id: u64, nome: &str, nulo: bool, valor: Vec<Parte>| {
            let mut p = vec![];
            if nulo {
                p.extend([t("if ("), Parte::Ident(id), t(" != null) {\n      ")]);
            }
            p.extend([t("json[r'"), t(nome), t("'] = "), Parte::Codigo(c(valor)), t(";\n    ")]);
            if nulo {
                p.push(t("}\n    "));
            }
            c(p)
        };
        let corpo = c(vec![
            t("{\n    final json = "),
            t("<"),
            Parte::Ident(2),
            t(", "),
            Parte::Codigo(c(vec![Parte::Ident(3), t("?")])),
            t(">{}"),
            t(";\n    "),
            Parte::Codigo(entrada(10, "nome", false, vec![Parte::Ident(10)])),
            Parte::Codigo(entrada(11, "idade", false, vec![Parte::Ident(11)])),
            Parte::Codigo(entrada(12, "apelido", true, vec![Parte::Ident(12), t("!")])),
            Parte::Codigo(entrada(
                13,
                "notas",
                false,
                vec![t("[ for (final item in "), Parte::Codigo(c(vec![Parte::Ident(13)])), t(") "), Parte::Codigo(c(vec![t("item")])), t("]")],
            )),
            t("return json;\n  }"),
        ]);
        let metodo = c(vec![t("  "), t("augment "), Parte::Codigo(map_so()), t(" "), t("toJson"), t("("), t(")"), t(" "), Parte::Codigo(corpo)]);
        let fase3 = Resultado { tipos: vec![(100, vec![ctor, metodo])], ..Default::default() };
        let texto = montar(&[fase2, fase3], &tabela, Forma::BibliotecaDeAugmentation("package:caso/modelos.dart")).unwrap();
        let esperado = "augment library 'package:caso/modelos.dart';

import 'dart:core' as prefix0;

augment class Usuario {
  external Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.nome = json[r'nome'] as prefix0.String,
        this.idade = json[r'idade'] as prefix0.int,
        this.apelido = json[r'apelido'] as prefix0.String?,
        this.notas = [ for (final item in json[r'notas'] as prefix0.List<prefix0.Object?>) item as prefix0.int];
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'nome'] = this.nome;
    json[r'idade'] = this.idade;
    if (this.apelido != null) {
      json[r'apelido'] = this.apelido!;
    }
    json[r'notas'] = [ for (final item in this.notas) item];
    return json;
  }
}
";
        assert_eq!(texto, esperado);
    }

    #[test]
    fn prefixo_foge_de_textos_do_usuario() {
        assert_eq!(prefixo_novo(&["abc".into()], "prefix"), "prefix");
        assert_eq!(prefixo_novo(&["x prefix y".into()], "prefix"), "prefix0");
        assert_eq!(prefixo_novo(&["prefix prefix0".into()], "prefix"), "prefix1_");
    }

    #[test]
    fn declaracoes_de_topo_regra_4_e_supertipos() {
        let mut ids = HashMap::new();
        ids.insert(1, topo("Comparable", "dart:core"));
        ids.insert(2, IdentificadorResolvido { nome: "Ponto".into(), tipo: TipoDeIdentificador::Topo, uri: Some("package:a/a.dart".into()), escopo: None });
        let mut tipos = HashMap::new();
        tipos.insert(
            2,
            TipoAumentado {
                palavra: "class",
                modificadores: vec!["abstract", "base"],
                nome: "Ponto".into(),
                parametros: vec![c(vec![t("T")])],
            },
        );
        let tabela = Tabela { ids, tipos };
        let fase1 = Resultado {
            biblioteca: vec![c(vec![t("class PontoGemeo {}")])],
            tipos_novos: vec!["PontoGemeo".into()],
            interfaces: vec![(2, vec![c(vec![Parte::Ident(1), t("<"), Parte::Ident(2), t(">")])])],
            ..Default::default()
        };
        let fase2 = Resultado { tipos: vec![(2, vec![c(vec![t("  int get x => 1;")])])], ..Default::default() };
        let texto = montar(&[fase1, fase2], &tabela, Forma::Parte("a.dart")).unwrap();
        assert_eq!(
            texto,
            "part of 'a.dart';\n\nimport 'dart:core' as prefix0;\nimport 'package:a/a.dart' as prefix1;\n\n\
             class PontoGemeo {}\naugment abstract base class Ponto<T> implements prefix0.Comparable<prefix1.Ponto> {\n  int get x => 1;\n}\n"
        );
    }
}
