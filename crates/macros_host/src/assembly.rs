//! Assembly of the augmentation library from the structured results of the
//! applications (macro spec, `:288-498`, Rules 1–4).
//!
//! The criterion is **byte for byte** with the text that CFE 3.6.2 generates
//! (the oracle is the text it writes into the `.dill`, extracted by
//! `scripts/oracle_augmentation.dart`): the spec's declared goal
//! (`:290-294`) is that different tools produce the same library. The
//! algorithm follows the first generation's `buildAugmentationLibrary`, which
//! is what the CFE runs:
//!
//! * the top-level declarations of each result come out in result order,
//!   each followed by `\n`;
//! * everything that augments a type is merged into a single
//!   `augment <kind> Name` (Rule 1), in the order in which the type first
//!   appeared, with `extends`, `with`, `implements`, the enum values and the
//!   members (each followed by `\n`) in result order (Rule 2);
//! * an [`Identifier`](Part::Ident) becomes the name with the import prefix
//!   of its library (`prefix0`, `prefix1`… in order of first use; the base
//!   name is the first of `prefix`, `prefix0`, `prefix1`… that does not
//!   appear in any text, with `_` if it needed an index), `this.` for an
//!   instance member without a receiver, and `Class.` for a static member;
//! * the header, the imports and a blank line come first.
use serde_json::Value;

/// A piece of structured code, as the macro built it.
#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    /// `CodeKind` (`declaration`, `raw`…); does not change the text.
    pub kind: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    /// Host identifier (model key).
    Ident(u64),
    /// Omitted type (model key): becomes the inferred type.
    Omitted(u64),
    Code(Code),
}

/// The `MacroExecutionResult` of an application in a phase. The
/// `(type identifier, …)` pairs are in the order the macro produced them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExecutionResult {
    pub diagnostics: Vec<Value>,
    pub exception: Option<Value>,
    pub enum_values: Vec<(u64, Vec<Code>)>,
    pub extends: Vec<(u64, Code)>,
    pub interfaces: Vec<(u64, Vec<Code>)>,
    pub library: Vec<Code>,
    pub mixins: Vec<(u64, Vec<Code>)>,
    pub new_types: Vec<String>,
    pub types: Vec<(u64, Vec<Code>)>,
}

/// How an identifier appears in the text (`IdentifierKind` + URI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierKind {
    /// Top-level declaration: the import prefix of the library, if there is
    /// a URI.
    TopLevel,
    /// Static member or constructor: `prefix.Class.name`.
    Static,
    /// Instance member: `this.name` without an explicit receiver.
    Instance,
    /// Parameter or local: just the name.
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIdentifier {
    pub name: String,
    pub kind: IdentifierKind,
    /// Import URI of the declaration's library (the CFE's: `package:` or the
    /// canonical `file:`), when the identifier needs a prefix.
    pub uri: Option<String>,
    /// Class name, for a static member.
    pub scope: Option<String>,
}

/// What the assembly needs to know about an augmented type for the header
/// `augment <modifiers> <keyword> Name<T…> `.
#[derive(Debug, Clone, PartialEq)]
pub struct AugmentedType {
    /// `class`, `enum`, `extension` or `mixin`.
    pub keyword: &'static str,
    /// In CFE order: `abstract base external final interface mixin sealed`
    /// (class) or `base` (mixin).
    pub modifiers: Vec<&'static str>,
    pub name: String,
    /// The code of each type parameter (`T extends B`).
    pub parameters: Vec<Code>,
}

/// The host answers the assembly depends on.
pub trait Resolver {
    fn identifier(&self, id: u64) -> Result<ResolvedIdentifier, String>;
    fn augmented_type(&self, id: u64) -> Result<AugmentedType, String>;
    /// The inferred type of an omitted type (`None`: not inferred).
    fn inferred_type(&self, omitted: u64) -> Option<Code>;
}

/// The header of the assembled library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header<'a> {
    /// `augment library '<uri>';` — the CFE 3.6.2 one (oracle and
    /// materialization for SDK 3.6).
    AugmentationLibrary(&'a str),
    /// `part of '<uri>';` — the current form (materialization for 3.13.4).
    Part(&'a str),
}

#[derive(Debug, Clone)]
enum Applied {
    Text(String),
    /// Synthesized name (import prefix or omitted type), by index.
    Name(usize),
}

struct Assembler<'r> {
    r: &'r dyn Resolver,
    /// URI → index of the prefix's synthesized name, in order of first use.
    imports: Vec<(String, usize)>,
    names: Vec<String>,
    import_parts: Vec<Applied>,
    parts: Vec<Applied>,
    texts: Vec<String>,
    buffer: Vec<String>,
    last: String,
    error: Option<String>,
}

impl<'r> Assembler<'r> {
    fn text(&mut self, s: &str) {
        self.last = s.to_string();
        self.buffer.push(s.to_string());
    }

    fn flush(&mut self) {
        for s in self.buffer.drain(..) {
            self.parts.push(Applied::Text(s.clone()));
            self.texts.push(s);
        }
    }

    fn name(&mut self, n: usize) {
        self.flush();
        self.last.clear();
        self.parts.push(Applied::Name(n));
    }

    fn fail(&mut self, e: String) {
        self.error.get_or_insert(e);
    }

    fn ident(&mut self, id: u64) {
        let r = match self.r.identifier(id) {
            Ok(r) => r,
            Err(e) => return self.fail(e),
        };
        let prefix = r.uri.as_ref().map(|uri| match self.imports.iter().find(|(u, _)| u == uri) {
            Some(&(_, n)) => n,
            None => {
                let n = self.names.len();
                self.names.push(String::new());
                self.imports.push((uri.clone(), n));
                self.import_parts.push(Applied::Text(format!("import '{uri}' as ")));
                self.import_parts.push(Applied::Name(n));
                self.import_parts.push(Applied::Text(";\n".into()));
                n
            }
        });
        if r.kind == IdentifierKind::Instance {
            if !self.last.trim_end().ends_with('.') {
                self.text("this.");
            }
        } else if let Some(p) = prefix {
            self.name(p);
            self.text(".");
        }
        if r.kind == IdentifierKind::Static {
            let scope = r.scope.clone().unwrap_or_default();
            self.text(&format!("{scope}."));
        }
        self.text(&r.name);
    }

    fn code(&mut self, c: &Code) {
        for p in &c.parts {
            match p {
                Part::Text(s) => self.text(s),
                Part::Code(c) => self.code(c),
                Part::Ident(id) => self.ident(*id),
                Part::Omitted(key) => match self.r.inferred_type(*key) {
                    Some(c) => self.code(&c),
                    None => self.fail(format!("no inferred type for the omitted type {key}")),
                },
            }
        }
    }

    fn final_text(&self, header: Header<'_>) -> String {
        fn name<'a>(names: &'a [String], a: &'a Applied) -> &'a str {
            match a {
                Applied::Text(s) => s.as_str(),
                Applied::Name(n) => names[*n].as_str(),
            }
        }
        let mut s = match header {
            Header::AugmentationLibrary(uri) => format!("augment library '{uri}';\n\n"),
            Header::Part(uri) => format!("part of '{uri}';\n\n"),
        };
        for a in &self.import_parts {
            s.push_str(name(&self.names, a));
        }
        if !self.import_parts.is_empty() {
            s.push('\n');
        }
        for a in &self.parts {
            s.push_str(name(&self.names, a));
        }
        s
    }
}

/// A prefix that does not appear in any text: `name`, otherwise `name0`,
/// `name1`… (with a trailing `_` when it needed an index > 0, so that the
/// digits of the import index do not stick to it). The algorithm is the
/// first generation's, with the same quirk: each text only advances the
/// index while it contains the prefix, without going back to the previous
/// texts.
fn fresh_prefix(texts: &[String], name: &str) -> String {
    let mut index: i64 = -1;
    let mut prefix = name.to_string();
    for t in texts {
        while t.contains(&prefix) {
            index += 1;
            prefix = format!("{name}{index}");
        }
    }
    if index > 0 {
        prefix.push('_');
    }
    prefix
}

/// A map keyed by type identifier that keeps the order of first insertion
/// (that of a Dart `Map`).
struct Ordered<V> {
    items: Vec<(u64, V)>,
}

impl<V> Ordered<V> {
    fn new() -> Self {
        Ordered { items: Vec::new() }
    }
    fn entry(&mut self, k: u64, f: impl FnOnce() -> V) -> &mut V {
        let i = match self.items.iter().position(|(c, _)| *c == k) {
            Some(i) => i,
            None => {
                self.items.push((k, f()));
                self.items.len() - 1
            }
        };
        &mut self.items[i].1
    }
    fn get(&self, k: u64) -> Option<&V> {
        self.items.iter().find(|(c, _)| *c == k).map(|(_, v)| v)
    }
    fn keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.items.iter().map(|(k, _)| *k)
    }
}

/// Assembles the text of the augmentation library from `results` (in
/// execution order: phase, then application). Fails if an identifier or type
/// does not resolve, or if a type receives two `extends`.
pub fn assemble(results: &[ExecutionResult], r: &dyn Resolver, header: Header<'_>) -> Result<String, String> {
    let mut m = Assembler {
        r,
        imports: Vec::new(),
        names: Vec::new(),
        import_parts: Vec::new(),
        parts: Vec::new(),
        texts: Vec::new(),
        buffer: Vec::new(),
        last: String::new(),
        error: None,
    };
    let mut members: Ordered<Vec<&Code>> = Ordered::new();
    let mut values: Ordered<Vec<&Code>> = Ordered::new();
    let mut extends: Ordered<&Code> = Ordered::new();
    let mut interfaces: Ordered<Vec<&Code>> = Ordered::new();
    let mut mixins: Ordered<Vec<&Code>> = Ordered::new();
    // The order of the augmented types: that of the first generation's set
    // `{...enum, ...extends, ...interfaces, ...mixins, ...members}` — by
    // category, and within it by first appearance.
    for res in results {
        for c in &res.library {
            m.code(c);
            m.text("\n");
        }
        for (id, cs) in &res.enum_values {
            values.entry(*id, Vec::new).extend(cs.iter());
        }
        for (id, c) in &res.extends {
            if extends.get(*id).is_some() {
                let name = r.identifier(*id).map(|i| i.name).unwrap_or_default();
                return Err(format!("A class cannot extend multiple classes: {name}"));
            }
            *extends.entry(*id, || c) = c;
        }
        for (id, cs) in &res.interfaces {
            interfaces.entry(*id, Vec::new).extend(cs.iter());
        }
        for (id, cs) in &res.mixins {
            mixins.entry(*id, Vec::new).extend(cs.iter());
        }
        for (id, cs) in &res.types {
            members.entry(*id, Vec::new).extend(cs.iter());
        }
    }
    let mut types: Vec<u64> = Vec::new();
    for k in values.keys().chain(extends.keys()).chain(interfaces.keys()).chain(mixins.keys()).chain(members.keys()) {
        if !types.contains(&k) {
            types.push(k);
        }
    }
    for id in types {
        let t = r.augmented_type(id)?;
        let mut words = t.modifiers.join(" ");
        if !words.is_empty() {
            words.push(' ');
        }
        let has_parameters = !t.parameters.is_empty();
        m.text(&format!("augment {words}{} {}{}", t.keyword, t.name, if has_parameters { "" } else { " " }));
        if has_parameters {
            m.text("<");
            for (i, p) in t.parameters.iter().enumerate() {
                if i > 0 {
                    m.text(", ");
                }
                m.code(p);
            }
            m.text("> ");
        }
        if let Some(sup) = extends.get(id) {
            m.text("extends ");
            m.code(sup);
            m.text(" ");
        }
        if let Some(ms) = mixins.get(id).filter(|v| !v.is_empty()) {
            m.text("with ");
            for (i, c) in ms.iter().enumerate() {
                if i > 0 {
                    m.text(", ");
                }
                m.code(c);
            }
            m.text(" ");
        }
        if let Some(is) = interfaces.get(id).filter(|v| !v.is_empty()) {
            m.text("implements ");
            for (i, c) in is.iter().enumerate() {
                if i > 0 {
                    m.text(", ");
                }
                m.code(c);
            }
            m.text(" ");
        }
        m.text("{\n");
        if t.keyword == "enum" {
            for c in values.get(id).into_iter().flatten() {
                m.code(c);
            }
            m.text(";\n");
        }
        for c in members.get(id).into_iter().flatten() {
            m.code(c);
            m.text("\n");
        }
        m.text("}\n");
    }
    m.flush();
    if let Some(e) = m.error.take() {
        return Err(e);
    }
    if !m.imports.is_empty() {
        let base = fresh_prefix(&m.texts, "prefix");
        for (i, (_, n)) in m.imports.clone().into_iter().enumerate() {
            m.names[n] = format!("{base}{i}");
        }
    }
    Ok(m.final_text(header))
}

// ----------------------------------------------------------------- JSON

/// A protocol `Code` (`{"k", "p"}`; a part is text, `{"i", "n"}`, `{"o"}` or
/// nested code).
pub fn code_from_json(v: &Value) -> Result<Code, String> {
    let kind = v.get("k").and_then(Value::as_str).unwrap_or("raw").to_string();
    let parts = v
        .get("p")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("code without parts: {v}"))?
        .iter()
        .map(|p| match p {
            Value::String(s) => Ok(Part::Text(s.clone())),
            Value::Object(o) if o.contains_key("i") => {
                o["i"].as_u64().map(Part::Ident).ok_or_else(|| format!("invalid identifier: {p}"))
            }
            Value::Object(o) if o.contains_key("o") => {
                o["o"].as_u64().map(Part::Omitted).ok_or_else(|| format!("invalid omitted type: {p}"))
            }
            _ => code_from_json(p).map(Part::Code),
        })
        .collect::<Result<_, _>>()?;
    Ok(Code { kind, parts })
}

fn list_pairs(v: &Value, field: &str) -> Result<Vec<(u64, Vec<Code>)>, String> {
    let Some(l) = v.get(field).and_then(Value::as_array) else { return Ok(Vec::new()) };
    l.iter()
        .map(|pair| {
            let id = pair.get(0).and_then(Value::as_u64).ok_or_else(|| format!("{field}: invalid pair"))?;
            let cs = pair
                .get(1)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{field}: invalid list"))?
                .iter()
                .map(code_from_json)
                .collect::<Result<_, _>>()?;
            Ok((id, cs))
        })
        .collect()
}

/// The `resultado` of a `macro.result` message.
pub fn result_from_json(v: &Value) -> Result<ExecutionResult, String> {
    let extends = match v.get("extends").and_then(Value::as_array) {
        None => Vec::new(),
        Some(l) => l
            .iter()
            .map(|pair| {
                let id = pair.get(0).and_then(Value::as_u64).ok_or("extends: invalid pair")?;
                Ok((id, code_from_json(pair.get(1).ok_or("extends: no code")?)?))
            })
            .collect::<Result<_, String>>()?,
    };
    Ok(ExecutionResult {
        diagnostics: v.get("diagnostics").and_then(Value::as_array).cloned().unwrap_or_default(),
        exception: v.get("exception").filter(|e| !e.is_null()).cloned(),
        enum_values: list_pairs(v, "enumValues")?,
        extends,
        interfaces: list_pairs(v, "interfaces")?,
        library: v
            .get("library")
            .and_then(Value::as_array)
            .map(|l| l.iter().map(code_from_json).collect::<Result<_, _>>())
            .transpose()?
            .unwrap_or_default(),
        mixins: list_pairs(v, "mixins")?,
        new_types: v
            .get("newTypes")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        types: list_pairs(v, "types")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn t(s: &str) -> Part {
        Part::Text(s.into())
    }
    fn c(parts: Vec<Part>) -> Code {
        Code { kind: "raw".into(), parts }
    }

    struct Table {
        ids: HashMap<u64, ResolvedIdentifier>,
        types: HashMap<u64, AugmentedType>,
    }
    impl Resolver for Table {
        fn identifier(&self, id: u64) -> Result<ResolvedIdentifier, String> {
            self.ids.get(&id).cloned().ok_or(format!("id {id}"))
        }
        fn augmented_type(&self, id: u64) -> Result<AugmentedType, String> {
            self.types.get(&id).cloned().ok_or(format!("type {id}"))
        }
        fn inferred_type(&self, _: u64) -> Option<Code> {
            None
        }
    }

    fn top(name: &str, uri: &str) -> ResolvedIdentifier {
        ResolvedIdentifier { name: name.into(), kind: IdentifierKind::TopLevel, uri: Some(uri.into()), scope: None }
    }
    fn inst(name: &str) -> ResolvedIdentifier {
        ResolvedIdentifier { name: name.into(), kind: IdentifierKind::Instance, uri: None, scope: None }
    }

    /// The `@JsonCodable` of `Usuario` (phases 2 and 3), assembled by hand
    /// from what `package:json` 0.20.4 produces: the output is the text that
    /// CFE 3.6.2 writes, byte for byte (`corpus/macros/402_aug_json_saida_cfe`).
    #[test]
    fn json_codable_like_the_cfe() {
        let core = "dart:core";
        let mut ids = HashMap::new();
        ids.insert(1, top("Map", core));
        ids.insert(2, top("String", core));
        ids.insert(3, top("Object", core));
        ids.insert(4, top("int", core));
        ids.insert(5, top("List", core));
        ids.insert(10, inst("nome"));
        ids.insert(11, inst("idade"));
        ids.insert(12, inst("apelido"));
        ids.insert(13, inst("notas"));
        ids.insert(20, ResolvedIdentifier { name: "json".into(), kind: IdentifierKind::Local, uri: None, scope: None });
        let mut types = HashMap::new();
        types.insert(
            100,
            AugmentedType { keyword: "class", modifiers: vec![], name: "Usuario".into(), parameters: vec![] },
        );
        let table = Table { ids, types };
        let map_so = || c(vec![Part::Ident(1), t("<"), Part::Ident(2), t(", "), Part::Code(c(vec![Part::Ident(3), t("?")])), t(">")]);
        let phase2 = ExecutionResult {
            types: vec![(
                100,
                vec![
                    c(vec![t("  external "), t("Usuario"), t(".fromJson("), Part::Code(map_so()), t(" json);")]),
                    c(vec![t("  external "), Part::Code(map_so()), t(" toJson();")]),
                ],
            )],
            ..Default::default()
        };
        let field = |id: u64, conv: Vec<Part>| c(vec![Part::Ident(id), t(" = "), Part::Code(c(conv))]);
        let json_of = |name: &str| c(vec![Part::Ident(20), t("[r'"), t(name), t("']")]);
        let ctor = c(vec![
            t("  "),
            t("augment "),
            t("Usuario"),
            t("."),
            t("fromJson"),
            t("("),
            Part::Code(c(vec![Part::Code(map_so()), t(" "), t("json")])),
            t(", "),
            t(")"),
            t("\n      : "),
            Part::Code(field(10, vec![Part::Code(json_of("nome")), t(" as "), Part::Ident(2)])),
            t(",\n        "),
            Part::Code(field(11, vec![Part::Code(json_of("idade")), t(" as "), Part::Ident(4)])),
            t(",\n        "),
            Part::Code(field(12, vec![Part::Code(json_of("apelido")), t(" as "), Part::Code(c(vec![Part::Ident(2), t("?")]))])),
            t(",\n        "),
            Part::Code(field(
                13,
                vec![
                    t("[ for (final item in "),
                    Part::Code(json_of("notas")),
                    t(" as "),
                    Part::Code(c(vec![Part::Ident(5), t("<"), Part::Code(c(vec![Part::Ident(3), t("?")])), t(">")])),
                    t(") "),
                    Part::Code(c(vec![Part::Code(c(vec![t("item")])), t(" as "), Part::Ident(4)])),
                    t("]"),
                ],
            )),
            t(";"),
        ]);
        let entry = |id: u64, name: &str, nullable: bool, value: Vec<Part>| {
            let mut p = vec![];
            if nullable {
                p.extend([t("if ("), Part::Ident(id), t(" != null) {\n      ")]);
            }
            p.extend([t("json[r'"), t(name), t("'] = "), Part::Code(c(value)), t(";\n    ")]);
            if nullable {
                p.push(t("}\n    "));
            }
            c(p)
        };
        let body = c(vec![
            t("{\n    final json = "),
            t("<"),
            Part::Ident(2),
            t(", "),
            Part::Code(c(vec![Part::Ident(3), t("?")])),
            t(">{}"),
            t(";\n    "),
            Part::Code(entry(10, "nome", false, vec![Part::Ident(10)])),
            Part::Code(entry(11, "idade", false, vec![Part::Ident(11)])),
            Part::Code(entry(12, "apelido", true, vec![Part::Ident(12), t("!")])),
            Part::Code(entry(
                13,
                "notas",
                false,
                vec![t("[ for (final item in "), Part::Code(c(vec![Part::Ident(13)])), t(") "), Part::Code(c(vec![t("item")])), t("]")],
            )),
            t("return json;\n  }"),
        ]);
        let method = c(vec![t("  "), t("augment "), Part::Code(map_so()), t(" "), t("toJson"), t("("), t(")"), t(" "), Part::Code(body)]);
        let phase3 = ExecutionResult { types: vec![(100, vec![ctor, method])], ..Default::default() };
        let text = assemble(&[phase2, phase3], &table, Header::AugmentationLibrary("package:caso/modelos.dart")).unwrap();
        let expected = "augment library 'package:caso/modelos.dart';

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
        assert_eq!(text, expected);
    }

    #[test]
    fn prefix_avoids_user_texts() {
        assert_eq!(fresh_prefix(&["abc".into()], "prefix"), "prefix");
        assert_eq!(fresh_prefix(&["x prefix y".into()], "prefix"), "prefix0");
        assert_eq!(fresh_prefix(&["prefix prefix0".into()], "prefix"), "prefix1_");
    }

    #[test]
    fn top_level_declarations_rule_4_and_supertypes() {
        let mut ids = HashMap::new();
        ids.insert(1, top("Comparable", "dart:core"));
        ids.insert(2, ResolvedIdentifier { name: "Point".into(), kind: IdentifierKind::TopLevel, uri: Some("package:a/a.dart".into()), scope: None });
        let mut types = HashMap::new();
        types.insert(
            2,
            AugmentedType {
                keyword: "class",
                modifiers: vec!["abstract", "base"],
                name: "Point".into(),
                parameters: vec![c(vec![t("T")])],
            },
        );
        let table = Table { ids, types };
        let phase1 = ExecutionResult {
            library: vec![c(vec![t("class PointTwin {}")])],
            new_types: vec!["PointTwin".into()],
            interfaces: vec![(2, vec![c(vec![Part::Ident(1), t("<"), Part::Ident(2), t(">")])])],
            ..Default::default()
        };
        let phase2 = ExecutionResult { types: vec![(2, vec![c(vec![t("  int get x => 1;")])])], ..Default::default() };
        let text = assemble(&[phase1, phase2], &table, Header::Part("a.dart")).unwrap();
        assert_eq!(
            text,
            "part of 'a.dart';\n\nimport 'dart:core' as prefix0;\nimport 'package:a/a.dart' as prefix1;\n\n\
             class PointTwin {}\naugment abstract base class Point<T> implements prefix0.Comparable<prefix1.Point> {\n  int get x => 1;\n}\n"
        );
    }
}
