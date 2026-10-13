//! `i18n:yamlBasedBuilder` (`package:i18n` 4.2.1, `lib/builder.dart` and
//! `lib/src/i18n_impl.dart`): each `x.i18n.yaml` becomes `x.i18n.dart` in its
//! own package (`build_to: source`) — one class per YAML map, one getter per
//! message (with the text in a documentation comment) and the `xMap` map with
//! the flattened keys —, formatted by the 80-column `DartFormatter`
//! (dart_style 2.3.8, the short style).
//!
//! The text comes out already formatted: the generator only produces a few
//! shapes (the class, the constructor, the `=>` getters, the comment, the
//! map), and each one breaks in a single place only when it exceeds 80
//! columns — the getter after the `=>`, with the body indented six columns;
//! the map entry after the `:`, with the value indented ten columns. What the
//! formatter would do differently (a class declaration or constructor that
//! is too long, the first line of a multi-line text that does not fit) is
//! refused, like what the official builder rejects (a key repeated across
//! languages out of order, an invalid language code, a value that is not a
//! string, number or boolean).
use crate::query::Query;
use crate::executor::{GeneratorCtx, NativeGenerator, NativeRequest, NativeOutput};
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};

pub struct I18nNative;

/// The source folders of a package in `build_runner` (the default `sources`
/// of the `$default` target), where `findAssets('**.i18n.yaml')` looks.
const SOURCE_FOLDERS: &[&str] = &["benchmark", "bin", "example", "lib", "test", "tool", "web"];

/// Page width of the default `DartFormatter()`.
const WIDTH: usize = 80;

/// `i18n`'s `Metadata`.
#[derive(Clone)]
struct Metadata {
    parent: Option<Box<Metadata>>,
    is_default: bool,
    default_name: String,
    default_file: Option<String>,
    name: String,
    locale: String,
    language: String,
}

impl Metadata {
    fn nest(&self, prefix: &str) -> Metadata {
        Metadata {
            parent: Some(Box::new(self.clone())),
            is_default: self.is_default,
            default_name: format!("{prefix}{}", self.default_name),
            default_file: self.default_file.clone(),
            name: format!("{prefix}{}", self.name),
            locale: self.locale.clone(),
            language: self.language.clone(),
        }
    }
}

/// `String.firstUpper`.
fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `String.firstLower`.
fn first_lower(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `String.filterHyphen`: `a-b-c` becomes `aBC`.
fn filter_hyphen(s: &str) -> String {
    let mut parts = s.split('-');
    let first = parts.next().unwrap_or_default().to_string();
    parts.fold(first, |a, p| a + &first_upper(p))
}

/// `String.filterSpaces`: the spaces before the first `(` disappear and each
/// word after the first one is capitalized; the rest (without the `(`s) comes
/// back after a `(`.
fn filter_spaces(s: &str) -> String {
    let mut f: Vec<&str> = s.split('(').collect();
    let first = f.remove(0);
    let mut words = first.split(' ');
    let start = words.next().unwrap_or_default().to_string();
    let converted = words.fold(start, |a, p| a + &first_upper(p));
    if f.is_empty() {
        return converted;
    }
    format!("{converted}({}", f.concat())
}

/// `String.convertName`: `AppMessages_en_US` becomes `AppMessagesEnUS`; each
/// part after the first `_` must have two letters.
fn converted_name(s: &str) -> Result<String, String> {
    let rep = filter_spaces(&filter_hyphen(s));
    let parts: Vec<&str> = rep.split('_').collect();
    if parts.len() == 1 {
        return Ok(rep);
    }
    let mut out = parts[0].to_string();
    for p in &parts[1..] {
        if p.chars().count() != 2 {
            return Err(format!(
                "i18n: `{p}` is neither a language nor a country code (the official builder throws)"
            ));
        }
        out += &first_upper(p);
    }
    Ok(out)
}

/// `generateMessageObjectName`.
fn file_metadata(file_name: &str) -> Result<Metadata, String> {
    let name = file_name.replace(".i18n.yaml", "");
    let parts: Vec<&str> = name.split('_').collect();
    let default_name = first_upper(parts[0]);
    if parts.len() == 1 {
        return Ok(Metadata {
            parent: None,
            is_default: true,
            default_name: default_name.clone(),
            default_file: None,
            name: default_name,
            locale: "en".into(),
            language: "en".into(),
        });
    }
    if parts.len() > 3 {
        return Err("i18n: file name with more than two `_` (the official builder throws)".into());
    }
    let language = parts[1];
    if !(language.len() == 2 && language.bytes().all(|b| b.is_ascii_lowercase())) {
        return Err(format!(
            "i18n: invalid language code `{language}` (the official builder throws)"
        ));
    }
    let mut locale = language.to_string();
    if parts.len() == 3 {
        let country = parts[2];
        if !(country.len() == 2 && country.bytes().all(|b| b.is_ascii_uppercase())) {
            return Err(format!(
                "i18n: invalid country code `{country}` (the official builder throws)"
            ));
        }
        locale = format!("{language}_{country}");
    }
    Ok(Metadata {
        parent: None,
        is_default: false,
        name: format!("{default_name}_{locale}"),
        default_name,
        default_file: Some(format!("{}.i18n.dart", parts[0])),
        locale,
        language: language.to_string(),
    })
}

/// A YAML value as Dart's `"$v"` writes it.
fn value_text(v: &Yaml) -> Result<Option<String>, String> {
    Ok(match v {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        Yaml::Null => None,
        _ => return Err("i18n: value that is not a string, integer or boolean".into()),
    })
}

/// The YAML map, with the keys as strings and in written order.
fn entries(m: &yaml_rust2::yaml::Hash) -> Result<Vec<(String, &Yaml)>, String> {
    m.iter()
        .map(|(k, v)| match k {
            Yaml::String(s) => Ok((s.clone(), v)),
            _ => Err("i18n: key that is not a string (the official builder throws)".to_string()),
        })
        .collect()
}

/// `YamlMapX.allKeys`: the keys in pre-order.
fn all_keys(m: &yaml_rust2::yaml::Hash, out: &mut Vec<String>) -> Result<(), String> {
    for (k, v) in entries(m)? {
        out.push(k);
        if let Yaml::Hash(h) = v {
            all_keys(h, out)?;
        }
    }
    Ok(())
}

/// Width of a line as `DartFormatter` measures it (`String.length`, in UTF-16
/// code units).
fn width(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `_wrapWithComments`, already indented.
fn comment(text: &str, out: &mut String) {
    let lines: Vec<&str> = text.lines().collect();
    let mut s = vec!["```dart".to_string()];
    if lines.len() > 1 {
        s.push("\"\"\"".into());
        s.extend(lines.iter().map(|l| l.to_string()));
        s.push("\"\"\"".into());
    } else {
        s.push(format!("\"{text}\""));
    }
    s.push("```".into());
    for l in s {
        // The formatter removes the trailing space of the comment line.
        out.push_str(format!("  /// {l}").trim_end());
        out.push('\n');
    }
}

/// Indentation + `head => body;`, breaking after the `=>` when the line (up
/// to the body's first line break) exceeds 80 columns.
fn getter(head: &str, body: &str, out: &mut String) -> Result<(), String> {
    let first = body.split('\n').next().unwrap_or_default();
    let multiline = body.contains('\n');
    let end = if multiline { "" } else { ";" };
    let one_line = format!("  {head} => {first}{end}");
    if width(&one_line) <= WIDTH {
        out.push_str(&format!("  {head} => {body};\n"));
        return Ok(());
    }
    if multiline {
        return Err(
            "i18n: multi-line text whose first line does not fit (formatter shape not handled)"
                .into(),
        );
    }
    if width(&format!("  {head} =>")) > WIDTH {
        return Err(
            "i18n: getter signature too long (formatter shape not handled)".into(),
        );
    }
    out.push_str(&format!("  {head} =>\n      {body};\n"));
    Ok(())
}

/// A line the formatter would leave as it is, or a refusal.
fn short_line(l: &str, out: &mut String) -> Result<(), String> {
    if width(l) > WIDTH {
        return Err(format!(
            "i18n: `{}` exceeds 80 columns (formatter shape not handled)",
            l.trim()
        ));
    }
    out.push_str(l);
    out.push('\n');
    Ok(())
}

/// `renderTranslation`.
fn render_class(
    meta: &Metadata,
    content: &yaml_rust2::yaml::Hash,
    out: &mut String,
) -> Result<(), String> {
    let default = converted_name(&meta.default_name)?;
    let name = converted_name(&meta.name)?;
    let parent = meta
        .parent
        .as_ref()
        .map(|p| converted_name(&p.name))
        .transpose()?;
    if meta.is_default {
        short_line(&format!("class {name} {{"), out)?;
    } else {
        short_line(&format!("class {name} extends {default} {{"), out)?;
    }
    match &parent {
        None => {
            short_line(&format!("  const {name}();"), out)?;
            short_line(
                &format!("  String get locale => \"{}\";", meta.locale),
                out,
            )?;
            short_line(
                &format!("  String get languageCode => \"{}\";", meta.language),
                out,
            )?;
        }
        Some(p) => {
            short_line(&format!("  final {p} _parent;"), out)?;
            if meta.is_default {
                short_line(&format!("  const {name}(this._parent);"), out)?;
            } else {
                short_line(
                    &format!("  const {name}(this._parent) : super(_parent);"),
                    out,
                )?;
            }
        }
    }
    for (k, v) in entries(content)? {
        let key = filter_hyphen(&filter_spaces(&k));
        if let Yaml::Hash(_) = v {
            let child = converted_name(&meta.nest(&first_upper(&key)).name)?;
            getter(
                &format!("{child} get {key}"),
                &format!("{child}(this)"),
                out,
            )?;
            continue;
        }
        let text = value_text(v)?;
        // The comment (or the empty line that `writeln('')` leaves) opens the
        // member with a blank line.
        out.push('\n');
        if let Some(t) = text.as_deref().filter(|t| !t.is_empty()) {
            comment(t, out);
        }
        let value = format!("\"\"\"{}\"\"\"", text.as_deref().unwrap_or("null"));
        if k.contains('(') {
            getter(&format!("String {key}"), &value, out)?;
        } else {
            getter(&format!("String get {key}"), &value, out)?;
        }
    }
    out.push_str("}\n");
    Ok(())
}

/// `prepareTranslationList` and the `renderTranslation` of each class, in
/// pre-order. The nested class's name comes from the unfiltered key
/// (`k.firstUpper()`), as in the official builder.
fn render_classes(meta: &Metadata, m: &yaml_rust2::yaml::Hash, out: &mut String) -> Result<(), String> {
    render_class(meta, m, out)?;
    out.push('\n');
    for (k, v) in entries(m)? {
        if let Yaml::Hash(h) = v {
            render_classes(&meta.nest(&first_upper(&k)), h, out)?;
        }
    }
    Ok(())
}

/// `String.containsReference`.
fn has_reference(s: &str) -> Result<bool, String> {
    let mut refs: Vec<&str> = s.split('$').collect();
    refs.pop();
    if refs.is_empty() {
        return Ok(false);
    }
    let escaped = refs.iter().filter(|r| r.ends_with('\\')).count();
    if escaped == 0 {
        return Ok(true);
    }
    if escaped == refs.len() {
        return Ok(false);
    }
    Err("i18n: text mixing escaped and unescaped `$` (the official builder throws)".into())
}

/// `renderMapEntries`: only the string values without references.
fn map_entries(
    m: &yaml_rust2::yaml::Hash,
    prefix: &str,
    out: &mut String,
) -> Result<(), String> {
    for (k, v) in entries(m)? {
        match v {
            Yaml::Hash(h) => map_entries(h, &format!("{prefix}{k}."), out)?,
            Yaml::String(s) if !has_reference(s)? => {
                let key = format!("\"\"\"{prefix}{k}\"\"\":");
                let value = format!("\"\"\"{s}\"\"\",");
                let first = value.split('\n').next().unwrap_or_default();
                if width(&format!("      {key} {first}")) <= WIDTH {
                    out.push_str(&format!("      {key} {value}\n"));
                } else if value.contains('\n') {
                    return Err("i18n: multi-line text whose first line does not fit in the map (formatter shape not handled)".into());
                } else {
                    out.push_str(&format!("      {key}\n          {value}\n"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The `_plural`, `_ordinal` and `_cardinal` functions, as the formatter
/// leaves them.
fn functions(out: &mut String) {
    for f in ["plural", "ordinal", "cardinal"] {
        out.push_str(&format!("String _{f}(\n  int count, {{\n"));
        for p in ["zero", "one", "two", "few", "many", "other"] {
            out.push_str(&format!("  String? {p},\n"));
        }
        out.push_str(&format!(
            "}}) =>\n    i18n.{f}(\n      count,\n      _languageCode,\n"
        ));
        for p in ["zero", "one", "two", "few", "many", "other"] {
            out.push_str(&format!("      {p}: {p},\n"));
        }
        out.push_str("    );\n");
    }
}

/// `generateDartContentFromYaml`, formatted.
fn generate(file_name: &str, yaml: &str) -> Result<String, String> {
    let meta = file_metadata(file_name)?;
    let docs = YamlLoader::load_from_str(yaml).map_err(|e| format!("i18n: unreadable YAML: {e}"))?;
    let Some(Yaml::Hash(m)) = docs.first() else {
        return Err("i18n: the YAML is not a map (the official builder throws)".into());
    };
    let mut s = String::new();
    s.push_str("// GENERATED FILE, do not edit!\n");
    s.push_str("// ignore_for_file: annotate_overrides, non_constant_identifier_names, prefer_single_quotes, unused_element, unused_field\n");
    s.push_str("import 'package:i18n/i18n.dart' as i18n;\n");
    if let Some(a) = &meta.default_file {
        s.push_str(&format!("import '{a}';\n"));
    }
    s.push('\n');
    s.push_str(&format!("String get _languageCode => '{}';\n", meta.language));
    functions(&mut s);
    s.push('\n');
    render_classes(&meta, m, &mut s)?;
    let map = format!("{}Map", first_lower(&converted_name(&meta.name)?));
    let mut body = String::new();
    map_entries(m, "", &mut body)?;
    if body.is_empty() {
        short_line(&format!("Map<String, String> get {map} => {{}};"), &mut s)?;
    } else {
        short_line(&format!("Map<String, String> get {map} => {{"), &mut s)?;
        s.push_str(&body);
        s.push_str("    };\n");
    }
    Ok(s)
}

/// The default file of `current` among the package's `**.i18n.yaml` (in path
/// order): the first one without `_` in its name whose name is a prefix of
/// the current one (`firstWhere`, which throws when there is none).
fn default_file<'a>(current: &str, all: &'a [String]) -> Option<&'a String> {
    let current_name = current
        .rsplit('/')
        .next()
        .unwrap_or(current)
        .replace(".i18n.yaml", "");
    all.iter().find(|c| {
        let name = c.rsplit('/').next().unwrap_or(c).replace(".i18n.yaml", "");
        !name.contains('_') && current_name.starts_with(&name)
    })
}

impl NativeGenerator for I18nNative {
    fn key(&self) -> &'static str {
        "i18n:yamlBasedBuilder"
    }

    fn covers(&self, _factory: &str) -> bool {
        true
    }

    fn by_package(&self) -> bool {
        false
    }

    fn verified(&self) -> bool {
        true
    }

    fn generate(
        &self,
        ctx: &mut GeneratorCtx<'_>,
        request: &NativeRequest,
    ) -> Result<NativeOutput, String> {
        let mut s = NativeOutput::default();
        // `findAssets(Glob('**.i18n.yaml'))` in the package's source folders.
        let glob = crate::glob::Glob::new("**.i18n.yaml").map_err(|e| e.to_string())?;
        let mut all: Vec<String> = Vec::new();
        for folder in SOURCE_FOLDERS {
            let dir = request.package_root.join(folder);
            ctx.register(Query::Glob {
                dir: dartforge_elements::gerado::chave(&dir),
                pattern: "**.i18n.yaml".into(),
            });
            all.extend(
                crate::graph::list(&dir, std::slice::from_ref(&glob))
                    .into_iter()
                    .map(|r| format!("{folder}/{r}")),
            );
        }
        all.sort();
        for a in &request.actions {
            let name = a
                .input
                .path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            let Some(source) = ctx.read(&a.natural_input) else {
                s.refusals
                    .insert(a.natural_input.clone(), "i18n: unreadable input".into());
                continue;
            };
            let text = String::from_utf8_lossy(&source).into_owned();
            let result = (|| -> Result<String, String> {
                let default = default_file(&a.input.path, &all)
                    .ok_or("i18n: no default file (the official builder throws)")?;
                // Each key in the same place as in the default file (the
                // official builder reports an error, `log.severe`, and the
                // build fails).
                if default.as_str() != &*a.input.path {
                    let path = request.package_root.join(default);
                    let bytes = ctx
                        .read(Path::new(&path))
                        .ok_or("i18n: unreadable default file")?;
                    let keys = |t: &str| -> Result<Vec<String>, String> {
                        let docs = YamlLoader::load_from_str(t)
                            .map_err(|e| format!("i18n: unreadable YAML: {e}"))?;
                        let Some(Yaml::Hash(m)) = docs.first() else {
                            return Err("i18n: the YAML is not a map".into());
                        };
                        let mut v = Vec::new();
                        all_keys(m, &mut v)?;
                        Ok(v)
                    };
                    if keys(&text)? != keys(&String::from_utf8_lossy(&bytes))? {
                        return Err(
                            "i18n: keys different from those of the default file (the official builder reports an error)"
                                .into(),
                        );
                    }
                }
                generate(&name, &text)
            })();
            match result {
                Ok(dart) => {
                    if let Some((_, n)) = a.outputs.first() {
                        s.outputs.insert(n.clone(), dart.into_bytes());
                    }
                }
                Err(f) => {
                    s.refusals.insert(a.natural_input.clone(), f);
                }
            }
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_like_i18n() {
        assert_eq!(filter_spaces("my key(int n)"), "myKey(int n)");
        assert_eq!(filter_hyphen("a-b-c"), "aBC");
        assert_eq!(converted_name("AppMessages_en").unwrap(), "AppMessagesEn");
        assert_eq!(
            converted_name("AppMessages_pt_BR").unwrap(),
            "AppMessagesPtBR"
        );
        assert!(converted_name("App_foo").is_err());
        assert!(has_reference("a $b").unwrap());
        assert!(!has_reference(r"a \$b").unwrap());
    }

    #[test]
    fn long_getter_breaks_after_the_arrow() {
        let mut s = String::new();
        getter(
            "String get x",
            &format!("\"\"\"{}\"\"\"", "a".repeat(80)),
            &mut s,
        )
        .unwrap();
        assert!(s.starts_with("  String get x =>\n      \"\"\"aaa"));
    }
}
