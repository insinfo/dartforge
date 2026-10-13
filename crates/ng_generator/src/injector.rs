//! The code of a `@GenerateInjector`: what the official `InjectorEmitter`
//! (`angular_compiler/emitter/injector.dart`) builds with `code_builder`, and
//! the `Compiler`'s `DartFormatter(pageWidth: 1000000)` formats.
//!
//! With an "infinite" page, the formatter only breaks where `code_builder` puts
//! a trailing comma — in a call, list or map with more than one item. There
//! each item goes on its own line, two spaces in from the line where the call
//! starts; the body of a `=>` already starts four in from the member. The
//! imports are those of `Allocator.simplePrefixing` (`_i1`, `_i2`…, in the
//! order each library appears in the text; `dart:core` without a prefix), and
//! the whole unit of the file shares the same table.
use crate::directives::{DI_TOKENS, Dependency, Token};
use crate::metadata::{Injector, InjectorSource, Revived, WrittenType};

const INJECTOR: &str = "package:ngdart/src/di/injector.dart";
const UTILITIES: &str = "package:ngdart/src/utilities.dart";

/// The import table of `Allocator.simplePrefixing`.
#[derive(Default)]
struct Allocator {
    uris: Vec<String>,
}

impl Allocator {
    /// `symbol` from `uri`, with the library prefix (allocated now if it was
    /// not yet).
    fn r(&mut self, uri: &str, symbol: &str) -> String {
        if uri.is_empty() || uri == "dart:core" {
            return symbol.to_string();
        }
        let i = match self.uris.iter().position(|u| u == uri) {
            Some(i) => i,
            None => {
                self.uris.push(uri.to_string());
                self.uris.len() - 1
            }
        };
        format!("_i{}.{symbol}", i + 1)
    }

    fn ty(&mut self, t: &WrittenType) -> String {
        let mut s = self.r(&t.uri, &t.symbol);
        if !t.args.is_empty() {
            let args: Vec<String> = t.args.iter().map(|a| self.ty(a)).collect();
            s = format!("{s}<{}>", args.join(", "));
        }
        s
    }
}

/// A `code_builder` expression, before formatting.
enum Ex {
    Atom(String),
    /// `target(args)`: with more than one argument, a trailing comma.
    Call {
        target: String,
        args: Vec<(Option<String>, Ex)>,
    },
    /// `[..]`, with `const ` or not in `prefix`.
    List {
        prefix: String,
        items: Vec<Ex>,
    },
}

impl Ex {
    /// The formatted text, with `indent` being the indentation of the line where
    /// the expression starts.
    fn text(&self, indent: usize) -> String {
        let (opens, closes, items): (String, &str, Vec<(Option<&str>, &Ex)>) = match self {
            Ex::Atom(s) => return s.clone(),
            Ex::Call { target, args } => (
                format!("{target}("),
                ")",
                args.iter().map(|(n, e)| (n.as_deref(), e)).collect(),
            ),
            Ex::List { prefix, items } => (
                format!("{prefix}["),
                "]",
                items.iter().map(|e| (None, e)).collect(),
            ),
        };
        let item = |n: Option<&str>, e: &Ex, r: usize| match n {
            Some(n) => format!("{n}: {}", e.text(r)),
            None => e.text(r),
        };
        if items.len() > 1 {
            let inside = " ".repeat(indent + 2);
            let mut s = format!("{opens}\n");
            for (n, e) in items {
                s += &format!("{inside}{},\n", item(n, e, indent + 2));
            }
            s + &" ".repeat(indent) + closes
        } else {
            let middle: Vec<String> = items.into_iter().map(|(n, e)| item(n, e, indent)).collect();
            format!("{opens}{}{closes}", middle.join(""))
        }
    }
}

/// `literalString`: single quotes, with `'` and line breaks escaped.
fn string_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'").replace('\n', "\\n"))
}

/// `_reviveString`: `\` doubled, `$` escaped (except after `\`), line breaks as
/// `\n` and whatever is outside visible ASCII as `\u{..}`.
fn revived_text(s: &str) -> String {
    let s = s.replace('\\', "\\\\");
    let cs: Vec<char> = s.chars().collect();
    let mut escaped = String::new();
    for (i, &c) in cs.iter().enumerate() {
        if c == '$' && (i == 0 || cs[i - 1] != '\\') {
            escaped += "\\$";
        } else if c == '\n' {
            escaped += "\\n";
        } else {
            escaped.push(c);
        }
    }
    let unicode: String = escaped
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
    string_literal(&unicode)
}

struct Emitter<'a> {
    al: &'a mut Allocator,
}

impl Emitter<'_> {
    /// `_tokenToIdentifier`.
    fn token(&mut self, t: &Token) -> Result<Ex, String> {
        Ok(match t {
            Token::Class { uri, class } => Ex::Atom(self.al.r(uri, class)),
            Token::Opaque { name, ty } | Token::Multi { name, ty } => {
                let class = if matches!(t, Token::Multi { .. }) {
                    "MultiToken"
                } else {
                    "OpaqueToken"
                };
                let c = self.al.r(DI_TOKENS, class);
                let t = self.al.ty(&crate::metadata::written_token_type(ty));
                let args = if name.is_empty() {
                    Vec::new()
                } else {
                    vec![(None, Ex::Atom(string_literal(name)))]
                };
                Ex::Call {
                    target: format!("const {c}<{t}>"),
                    args,
                }
            }
            Token::Element | Token::Detector => {
                return Err("HtmlElement/ChangeDetectorRef in the injector".into());
            }
        })
    }

    /// `_computeDependencies`.
    fn dependency(&mut self, d: &Dependency) -> Result<Ex, String> {
        let call = |target: &str, tok: Ex, optional: bool| {
            let mut args = vec![(None, tok)];
            if optional {
                args.push((None, Ex::Atom("null".into())));
            }
            Ex::Call {
                target: target.to_string(),
                args,
            }
        };
        Ok(if d.self_only {
            let target = if d.optional {
                "injectFromSelfOptional"
            } else {
                "injectFromSelf"
            };
            call(target, self.token(&d.token)?, d.optional)
        } else if d.skip_self {
            if d.optional {
                let cast = self.al.r(UTILITIES, "unsafeCast");
                let inside = call("injectFromAncestryOptional", self.token(&d.token)?, true);
                Ex::Call {
                    target: cast,
                    args: vec![(None, inside)],
                }
            } else {
                call("injectFromAncestry", self.token(&d.token)?, false)
            }
        } else if d.host {
            let target = if d.optional {
                "injectFromParentOptional"
            } else {
                "injectFromParent"
            };
            call(target, self.token(&d.token)?, d.optional)
        } else if d.optional {
            call("provideUntyped", self.token(&d.token)?, true)
        } else {
            call("this.get", self.token(&d.token)?, false)
        })
    }

    fn dependencies(&mut self, deps: &[Dependency]) -> Result<Vec<(Option<String>, Ex)>, String> {
        deps.iter()
            .map(|d| Ok((None, self.dependency(d)?)))
            .collect()
    }

    /// A revived value; `in_const`: already inside a `const` expression (the
    /// `const` of the inner ones is dropped).
    fn value(&mut self, v: &Revived, in_const: bool) -> Ex {
        let konst = if in_const { "" } else { "const " };
        match v {
            Revived::Null => Ex::Atom("null".into()),
            Revived::Str(s) => Ex::Atom(revived_text(s)),
            Revived::Integer(i) => Ex::Atom(i.to_string()),
            Revived::Boolean(b) => Ex::Atom(b.to_string()),
            Revived::Access { uri, name } => Ex::Atom(self.al.r(uri, name)),
            Revived::List(items) => Ex::List {
                prefix: konst.to_string(),
                items: items.iter().map(|i| self.value(i, true)).collect(),
            },
            Revived::Object {
                uri,
                class,
                constructor,
                positional,
                named,
            } => {
                let mut target = format!("{konst}{}", self.al.r(uri, class));
                if let Some(c) = constructor {
                    target = format!("{target}.{c}");
                }
                let mut args: Vec<(Option<String>, Ex)> = positional
                    .iter()
                    .map(|p| (None, self.value(p, true)))
                    .collect();
                for (n, x) in named {
                    args.push((Some(n.clone()), self.value(x, true)));
                }
                Ex::Call { target, args }
            }
        }
    }
}

/// The indentation of the body of a `=>` in a class member.
const BODY_INDENT: usize = 6;

/// The text of the file's injectors: the imports (in table order) and the body
/// (from the `// ignore_for_file` on).
pub fn emit(injectors: &[&Injector]) -> Result<(String, String), String> {
    let mut al = Allocator::default();
    let mut blocks = Vec::new();
    for inj in injectors {
        blocks.push(emit_one(&mut al, inj)?);
    }
    let imports: String = al
        .uris
        .iter()
        .enumerate()
        .map(|(i, u)| format!("import '{}' as _i{};\n", crate::dialect::writing(u), i + 1))
        .collect();
    let body = format!(
        "// ignore_for_file: no_leading_underscores_for_library_prefixes\n{}",
        blocks.join("\n")
    );
    Ok((imports, body))
}

fn emit_one(al: &mut Allocator, inj: &Injector) -> Result<String, String> {
    let class = format!("_Injector${}", inj.name);
    let injector = al.r(INJECTOR, "Injector");
    let mut s = format!(
        "{injector} {}$Injector({injector} parent) => {class}._(parent);\n\n",
        inj.name
    );
    let hierarchical = al.r(INJECTOR, "HierarchicalInjector");
    s += &format!("class {class} extends {hierarchical} implements {injector} {{\n");
    s += &format!("  {class}._({injector} parent) : super(parent);\n\n");
    // The fields come before the methods in the text (`visitClass`), and it is
    // in that order that the imports are allocated.
    let mut em = Emitter { al };
    let mut fields = Vec::new();
    for (i, p) in inj.providers.iter().enumerate() {
        match &p.source {
            InjectorSource::Class { uri, class, .. } => {
                let t = em.al.r(uri, class);
                fields.push(format!("  {t}? _field{i};\n\n"));
            }
            InjectorSource::Factory { .. } => {
                let t = em.al.ty(&p.ty);
                fields.push(format!("  {t}? _field{i};\n\n"));
            }
            _ => {}
        }
    }
    let mut methods = Vec::new();
    let mut unique: Vec<String> = Vec::new();
    let mut multi: Vec<(Token, Vec<String>)> = Vec::new();
    for (i, p) in inj.providers.iter().enumerate() {
        let (return_type, name, body) = match &p.source {
            InjectorSource::Class {
                uri,
                class,
                constructor,
                deps,
            } => {
                let t = em.al.r(uri, class);
                let target = match constructor {
                    Some(c) => format!("{t}.{c}"),
                    None => t.clone(),
                };
                let args = em.dependencies(deps)?;
                let call = Ex::Call { target, args };
                (
                    t,
                    format!("_get{class}${i}"),
                    format!("_field{i} ??= {}", call.text(BODY_INDENT)),
                )
            }
            InjectorSource::Factory { uri, name, deps } => {
                let t = em.al.ty(&p.ty);
                let f = em.al.r(uri, name);
                let args = em.dependencies(deps)?;
                let call = Ex::Call { target: f, args };
                (
                    t,
                    format!("_get{}${i}", p.ty.symbol),
                    format!("_field{i} ??= {}", call.text(BODY_INDENT)),
                )
            }
            InjectorSource::Existing(target) => {
                let t = em.al.ty(&p.ty);
                let tok = em.token(target)?;
                let call = Ex::Call {
                    target: "this.get".into(),
                    args: vec![(None, tok)],
                };
                (t, format!("_getExisting${i}"), call.text(BODY_INDENT))
            }
            InjectorSource::Value(v) => {
                let t = em.al.ty(&p.ty);
                let value = em.value(v, false);
                (
                    t,
                    format!("_get{}${i}", p.ty.symbol),
                    value.text(BODY_INDENT),
                )
            }
        };
        methods.push(format!("  {return_type} {name}() => {body};\n\n"));
        if p.multi {
            match multi.iter_mut().find(|(t, _)| *t == p.token) {
                Some((_, l)) => l.push(name),
                None => multi.push((p.token.clone(), vec![name])),
            }
        } else {
            unique.push(name);
        }
    }
    let n = inj.providers.len();
    methods.push(format!("  {injector} _getInjector${n}() => this;\n\n"));
    // The `injectFromSelfOptional`: the tokens enter the text (and the table)
    // here, after all the methods.
    let not_found = em.al.r(INJECTOR, "throwIfNotFound");
    let mut body = String::new();
    let mut k = 0;
    for p in inj.providers.iter() {
        if p.multi {
            continue;
        }
        let tok = em.token(&p.token)?.text(6);
        body += &format!(
            "    if (identical(token, {tok})) {{\n      return {}();\n    }}\n",
            unique[k]
        );
        k += 1;
    }
    body += &format!(
        "    if (identical(token, {injector})) {{\n      return _getInjector${n}();\n    }}\n"
    );
    for (t, ms) in &multi {
        let tok = em.token(t)?.text(6);
        let list = Ex::List {
            prefix: String::new(),
            items: ms.iter().map(|m| Ex::Atom(format!("{m}()"))).collect(),
        };
        body += &format!(
            "    if (identical(token, {tok})) {{\n      return {};\n    }}\n",
            list.text(6)
        );
    }
    s += &fields.concat();
    s += &methods.concat();
    s += &format!(
        "  @override\n  Object? injectFromSelfOptional(\n    Object token, [\n    Object? orElse = {not_found},\n  ]) {{\n{body}    return orElse;\n  }}\n}}\n"
    );
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revived_text_as_official() {
        assert_eq!(revived_text("it's $x\n"), r"'it\'s \$x\n'");
        assert_eq!(revived_text("é 😀 \\"), r"'\u{e9} \u{1f600} \\'");
    }

    #[test]
    fn call_with_two_arguments_breaks() {
        let e = Ex::Call {
            target: "f".into(),
            args: vec![
                (None, Ex::Atom("a".into())),
                (
                    None,
                    Ex::Call {
                        target: "g".into(),
                        args: vec![(None, Ex::Atom("b".into()))],
                    },
                ),
            ],
        };
        assert_eq!(e.text(6), "f(\n        a,\n        g(b),\n      )");
    }
}
