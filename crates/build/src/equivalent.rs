//! Builder factories recognized by what they do, not by their name.
//!
//! A package may register its own builder that merely instantiates a known
//! builder with constant arguments — `ngcomponents|scss_builder` is
//!
//! ```dart
//! Builder scssBuilder(BuilderOptions options) =>
//!     SassBuilder(outputExtension: '.scss.css');
//! ```
//!
//! Then there is nothing to run in Dart: it is `sass_builder`'s
//! `SassBuilder` with fixed options, and its native generator handles it.
//! Recognition is strict: the factory must directly return the construction
//! of a `SassBuilder` imported from `package:sass_builder/sass_builder.dart`,
//! with literal named arguments that the native generator knows, and the
//! extensions declared in `build.yaml` must be the ones the object writes.
//! Any other shape goes to the builder executor.
use crate::packages::PackageGraph;
use crate::value::{Map, Value};
use dartforge_frontend::Interner;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind, FunctionBody, StmtKind};

const SASS_BUILDER: &str = "package:sass_builder/sass_builder.dart";

/// A factory that, at run time, is a builder DartForge already has.
#[derive(Debug, Clone, PartialEq)]
pub struct Equivalent {
    /// The key of the known builder (`sass_builder:sass_builder`).
    pub key: &'static str,
    /// The options its generator runs with — those of the constructor, not
    /// those of `build.yaml` (the factory ignores them).
    pub options: Map,
}

/// Is the factory `factory` of `import` (`package:p/x.dart`) a known
/// builder? `declared`: the extensions from `build.yaml`.
pub fn equivalent(
    graph: &PackageGraph,
    import: &str,
    factory: &str,
    declared: &[(String, Vec<String>)],
) -> Option<Equivalent> {
    let rest = import.strip_prefix("package:")?;
    let (package, path) = rest.split_once('/')?;
    let file = graph.node(package)?.root.join("lib").join(path);
    let source = std::fs::read_to_string(file).ok()?;
    from_source(&source, factory, declared)
}

/// [`equivalent`] over the library's text.
pub fn from_source(
    source: &str,
    factory: &str,
    declared: &[(String, Vec<String>)],
) -> Option<Equivalent> {
    let mut interner = Interner::default();
    let p = dartforge_frontend::parser::parse(source, &mut interner);
    if !p.diagnostics.is_empty() {
        return None;
    }
    let (unit, tree) = (&p.unit, &p.ast);
    let name_of = |n: &ast::Name| interner.resolve(n.sym).to_string();
    // The `sass_builder` import (without `show`/`hide`, which the name would
    // have to pass through) and its prefix.
    let mut prefixes: Vec<Option<String>> = Vec::new();
    for d in &unit.directives {
        if let DirectiveKind::Import {
            uri,
            configurations,
            deferred,
            prefix,
            combinators,
        } = &d.kind
        {
            let target = uri.constant_value()?.to_string_lossy();
            if target == SASS_BUILDER
                && configurations.is_empty()
                && !deferred
                && combinators.is_empty()
            {
                prefixes.push(prefix.as_ref().map(name_of));
            }
        }
    }
    if prefixes.is_empty() {
        return None;
    }
    // No local declaration with that name (it would shadow the imported one).
    let mut function = None;
    for &id in &unit.declarations {
        let decl = tree.decl(id);
        let n = match &decl.kind {
            DeclKind::Class(c) => Some(name_of(&c.name)),
            DeclKind::Function(f) => tree.function(*f).name.as_ref().map(name_of),
            _ => None,
        };
        if n.as_deref() == Some("SassBuilder") {
            return None;
        }
        if let DeclKind::Function(f) = &decl.kind
            && n.as_deref() == Some(factory)
        {
            function = Some(*f);
        }
    }
    let f = tree.function(function?);
    if f.parameters.as_ref().map_or(0, |p| p.len()) != 1
        || !matches!(f.modifier, ast::AsyncModifier::None)
    {
        return None;
    }
    let body = match f.body {
        FunctionBody::Expression(e) => e,
        FunctionBody::Block(b) => match &tree.stmt(b).kind {
            StmtKind::Block(s) if s.len() == 1 => match &tree.stmt(s[0]).kind {
                StmtKind::Return(Some(e)) => *e,
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    };
    // `SassBuilder(..)` or `p.SassBuilder(..)` (the call without `new`), or
    // the explicit creation.
    let (is_sass, args) = match &tree.expr(body).kind {
        ExprKind::Call { target, arguments } => {
            let e = match &tree.expr(*target).kind {
                ExprKind::Identifier(n) => name_of(n) == "SassBuilder" && prefixes.contains(&None),
                ExprKind::Property {
                    target: t,
                    name,
                    null_aware: false,
                } => {
                    name_of(name) == "SassBuilder"
                        && matches!(&tree.expr(*t).kind,
                            ExprKind::Identifier(p) if prefixes.contains(&Some(name_of(p))))
                }
                _ => false,
            };
            (e, arguments)
        }
        ExprKind::InstanceCreation {
            ty,
            constructor: None,
            arguments,
            ..
        } => {
            let e = match &tree.ty(*ty).kind {
                ast::TypeKind::Named { name, args } if args.is_empty() => match &name[..] {
                    [n] => name_of(n) == "SassBuilder" && prefixes.contains(&None),
                    [p, n] => name_of(n) == "SassBuilder" && prefixes.contains(&Some(name_of(p))),
                    _ => false,
                },
                _ => false,
            };
            (e, arguments)
        }
        _ => return None,
    };
    if !is_sass || !args.type_args.is_empty() {
        return None;
    }
    // `SassBuilder({outputExtension = '.css', outputStyle, generateSourceMaps = false})`.
    let mut extension = ".css".to_string();
    let mut style = Value::Null;
    let mut source_maps = false;
    for a in args.args.iter() {
        let n = name_of(a.name.as_ref()?);
        let v = &tree.expr(a.value).kind;
        match (n.as_str(), v) {
            ("outputExtension", ExprKind::String(s)) => {
                extension = s.constant_value()?.to_string_lossy()
            }
            ("outputStyle", ExprKind::String(s)) => {
                style = Value::Text(s.constant_value()?.to_string_lossy())
            }
            ("outputStyle", ExprKind::Null) => style = Value::Null,
            ("generateSourceMaps", ExprKind::Bool(b)) => source_maps = *b,
            _ => return None,
        }
    }
    // The object writes `x<outputExtension>` (and `x.css.map`) for `x.scss`
    // and `x.sass`: the declared extensions must say the same, otherwise the
    // graph would expect other outputs.
    let mut expected = vec![extension];
    if source_maps {
        expected.push(".css.map".into());
    }
    if declared.is_empty()
        || declared.iter().any(|(input, outputs)| {
            !matches!(input.as_str(), ".scss" | ".sass") || *outputs != expected
        })
    {
        return None;
    }
    let mut options = Map::default();
    options.insert(Value::Text("outputStyle".into()), style);
    options.insert(Value::Text("sourceMaps".into()), Value::Bool(source_maps));
    Some(Equivalent {
        key: "sass_builder:sass_builder",
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(pairs: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        pairs
            .iter()
            .map(|(e, s)| (e.to_string(), s.iter().map(|x| x.to_string()).collect()))
            .collect()
    }

    const NGCOMPONENTS: &str = "import 'package:build/build.dart';\n\
        import 'package:sass_builder/sass_builder.dart';\n\
        Builder scssBuilder(BuilderOptions options) =>\n    SassBuilder(outputExtension: '.scss.css');\n";

    #[test]
    fn ngcomponents_scss_builder() {
        let e = from_source(
            NGCOMPONENTS,
            "scssBuilder",
            &ext(&[(".scss", &[".scss.css"]), (".sass", &[".scss.css"])]),
        )
        .expect("recognized");
        assert_eq!(e.key, "sass_builder:sass_builder");
        assert_eq!(e.options.get("outputStyle"), Some(&Value::Null));
        assert_eq!(e.options.get("sourceMaps"), Some(&Value::Bool(false)));
    }

    #[test]
    fn shapes_that_are_not_recognized() {
        let decl = ext(&[(".scss", &[".scss.css"])]);
        // Declared extension different from the one the object writes.
        assert!(from_source(NGCOMPONENTS, "scssBuilder", &ext(&[(".scss", &[".css"])])).is_none());
        // Another factory.
        assert!(from_source(NGCOMPONENTS, "other", &decl).is_none());
        // Option coming from the configuration (not constant).
        let cfg = "import 'package:sass_builder/sass_builder.dart';\n\
            Builder b(BuilderOptions o) => SassBuilder(outputExtension: '.scss.css', outputStyle: o.config['x']);\n";
        assert!(from_source(cfg, "b", &decl).is_none());
        // Without the `sass_builder` import, or with a local `SassBuilder`.
        let without = "Builder b(BuilderOptions o) => SassBuilder(outputExtension: '.scss.css');\n";
        assert!(from_source(without, "b", &decl).is_none());
        let local = format!("{NGCOMPONENTS}class SassBuilder {{}}\n");
        assert!(from_source(&local, "scssBuilder", &decl).is_none());
        // Prefixed and in a block, with style and source map.
        let pref = "import 'package:sass_builder/sass_builder.dart' as s;\n\
            Builder b(BuilderOptions o) { return s.SassBuilder(outputExtension: '.x.css', outputStyle: 'compressed', generateSourceMaps: true); }\n";
        let e =
            from_source(pref, "b", &ext(&[(".scss", &[".x.css", ".css.map"])])).expect("prefixed");
        assert_eq!(
            e.options.get("outputStyle"),
            Some(&Value::Text("compressed".into()))
        );
        assert_eq!(e.options.get("sourceMaps"), Some(&Value::Bool(true)));
    }
}
