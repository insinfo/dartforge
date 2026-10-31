//! Library-level test annotations, read with the rules of `test_core`'s
//! `parseMetadata` (package:test 1.25/1.26).
//!
//! The official runner parses the suite without resolving it and interprets
//! only the annotations of the first directive: `@TestOn`, `@Timeout`, `@Skip`,
//! `@OnPlatform`, `@Tags` and `@Retry`. This module reproduces the structural
//! part of that interpretation on the DartForge AST, including the ambiguity
//! between import prefixes and named constructors, and records the order in
//! which platform selectors and tag names must be validated. The planner then
//! validates those strings and builds the `Metadata` with package:test's own
//! classes, so selector grammar and tag syntax are never re-implemented here.
use dartforge_frontend::{
    Interner, Span,
    ast::{Ast, CollectionElement, DirectiveKind, ExprId, ExprKind, StringPart, TypeKind},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// The analyzer's view of an unresolved expression, as far as
/// `parseMetadata` distinguishes node classes.
enum Node<'a> {
    /// `new`/`const` creation: the written type parts followed by the
    /// constructor name, as `[class]`, `[prefix, class]` or
    /// `[prefix, class, constructor]` once the analyzer's unresolved reading
    /// is applied.
    Creation {
        parts: Vec<&'a str>,
        arguments: Vec<(Option<&'a str>, ExprId)>,
    },
    /// `m(args)`, `a.m(args)` or `a.b.m(args)`.
    Invocation {
        target: Target<'a>,
        method: &'a str,
        arguments: Vec<(Option<&'a str>, ExprId)>,
    },
    /// `a.b` with two simple identifiers.
    Prefixed(&'a str, &'a str),
    List(&'a [CollectionElement]),
    Map(&'a [CollectionElement]),
    /// A string literal and its value when it has no interpolation.
    String(Option<String>),
    Integer(Option<i64>),
    Double(f64),
    Other,
}

enum Target<'a> {
    None,
    Simple(&'a str),
    /// `prefix.Name`; only `Name` matters to the official parser.
    Prefixed(&'a str),
    Other,
}

/// A failure that `parseMetadata` reports as a `SourceSpanFormatException`.
struct Failure {
    message: String,
    span: Span,
    /// Not a format error: `parseMetadata` fails with a null check or an
    /// empty argument list, which escapes the official loader.
    fatal: bool,
}

type Parsed<T> = Result<T, Failure>;

/// Reads the library metadata of a suite.
///
/// The result is a JSON object with an ordered `checks` list (selectors and
/// tag names to validate, in the order the official parser validates them),
/// then either `error` (message and UTF-16 span) or `metadata`.
pub(super) fn library_metadata(source: &str) -> Value {
    let mut interner = Interner::new();
    let parsed = dartforge_frontend::parser::parse(source, &mut interner);
    let mut reader = Reader {
        ast: &parsed.ast,
        interner: &interner,
        source,
        prefixes: BTreeSet::new(),
        checks: Vec::new(),
    };
    for directive in &parsed.unit.directives {
        if let DirectiveKind::Import {
            prefix: Some(prefix),
            ..
        } = &directive.kind
        {
            reader.prefixes.insert(interner.resolve(prefix.sym));
        }
    }
    let annotations = parsed
        .unit
        .directives
        .first()
        .map(|d| d.metadata.as_slice())
        .unwrap_or_default();
    let result = reader.read(annotations);
    let mut value = json!({ "checks": reader.checks });
    match result {
        Ok(metadata) => value["metadata"] = metadata,
        Err(failure) => {
            value["error"] = json!({
                "message": failure.message,
                "fatal": failure.fatal,
                "start": utf16_offset(source, failure.span.start),
                "end": utf16_offset(source, failure.span.end),
            })
        }
    }
    value
}

/// `SourceFile` offsets count UTF-16 code units, not UTF-8 bytes.
fn utf16_offset(source: &str, byte: usize) -> usize {
    source
        .get(..byte.min(source.len()))
        .map_or(0, |prefix| prefix.encode_utf16().count())
}

struct Reader<'a> {
    ast: &'a Ast,
    interner: &'a Interner,
    source: &'a str,
    prefixes: BTreeSet<&'a str>,
    checks: Vec<Value>,
}

fn fail<T>(message: impl Into<String>, span: Span) -> Parsed<T> {
    Err(Failure {
        message: message.into(),
        span,
        fatal: false,
    })
}

fn crash<T>(message: &str, span: Span) -> Parsed<T> {
    Err(Failure {
        message: message.into(),
        span,
        fatal: true,
    })
}

impl<'a> Reader<'a> {
    fn name(&self, name: &dartforge_frontend::ast::Name) -> &'a str {
        self.interner.resolve(name.sym)
    }

    fn span(&self, expression: ExprId) -> Span {
        self.ast.expr(expression).span
    }

    fn read(&mut self, annotations: &[dartforge_frontend::ast::Annotation]) -> Parsed<Value> {
        let mut test_on = Value::Null;
        let mut timeout = Value::Null;
        let mut skip = Value::Null;
        let mut on_platform = Value::Null;
        let mut tags = Value::Null;
        let mut retry = Value::Null;
        for annotation in annotations {
            let names: Vec<&str> = annotation.name.iter().map(|n| self.name(n)).collect();
            // `@a.b` is a named constructor unless `a` is an import prefix;
            // `@a.b.c` is always prefix, class and constructor.
            let (class, constructor) = match names.as_slice() {
                [class] => (*class, None),
                [prefix, member] if !self.prefixes.contains(prefix) => (*prefix, Some(*member)),
                [_, class] => (*class, None),
                [_, class, constructor] => (*class, Some(*constructor)),
                _ => continue,
            };
            let arguments: Vec<ExprId> = annotation
                .arguments
                .as_ref()
                .map(|a| a.args.iter().map(|arg| arg.value).collect())
                .unwrap_or_default();
            let first = || -> Parsed<ExprId> {
                if annotation.arguments.is_none() {
                    return crash("Null check operator used on a null value", annotation.span);
                }
                arguments
                    .first()
                    .copied()
                    .map_or_else(|| crash("Bad state: No element", annotation.span), Ok)
            };
            match class {
                "TestOn" => {
                    single(&test_on, "TestOn", annotation.span)?;
                    test_on = json!(self.selector(first()?)?);
                }
                "Timeout" => {
                    single(&timeout, "Timeout", annotation.span)?;
                    timeout = match constructor {
                        Some("none") => json!({"none": true}),
                        None => self.duration(first()?)?,
                        Some(_) => json!({"factor": self.number(first()?)?}),
                    };
                }
                "Skip" => {
                    single(&skip, "Skip", annotation.span)?;
                    if annotation.arguments.is_none() {
                        return crash("Null check operator used on a null value", annotation.span);
                    }
                    skip = match arguments.first() {
                        None => json!(true),
                        Some(reason) => json!(self.string(*reason)?),
                    };
                }
                "OnPlatform" => {
                    single(&on_platform, "OnPlatform", annotation.span)?;
                    on_platform = self.on_platform(first()?)?;
                }
                "Tags" => {
                    single(&tags, "Tags", annotation.span)?;
                    let mut names = Vec::new();
                    for element in self.list(first()?)? {
                        let name = self.string(element)?;
                        let span = self.span(element);
                        self.check("tag", &name, span);
                        names.push(json!(name));
                    }
                    tags = Value::Array(names);
                }
                "Retry" => retry = json!(self.integer(first()?)?),
                _ => {}
            }
        }
        Ok(json!({
            "testOn": test_on,
            "timeout": timeout,
            "skip": skip,
            "onPlatform": on_platform,
            "tags": tags,
            "retry": retry,
        }))
    }

    fn check(&mut self, kind: &str, text: &str, span: Span) -> &mut Value {
        self.checks.push(json!({
            "kind": kind,
            "text": text,
            "start": utf16_offset(self.source, span.start),
            "end": utf16_offset(self.source, span.end),
        }));
        self.checks.last_mut().expect("just pushed")
    }

    fn selector(&mut self, expression: ExprId) -> Parsed<String> {
        let text = self.string(expression)?;
        let span = self.span(expression);
        let offsets = self.rune_offsets(span, &text);
        self.check("selector", &text, span)["offsets"] = offsets;
        Ok(text)
    }

    /// The UTF-16 source offset of each code unit of a literal's value, plus
    /// its end, for the literals whose source spells the value directly
    /// (one quoted piece without escapes). The planner maps selector errors
    /// into the file with it, as `test_core`'s `contextualizeSpan` does.
    fn rune_offsets(&self, span: Span, value: &str) -> Value {
        let written = &self.source[span.start..span.end];
        let (raw, body) = match written.strip_prefix('r') {
            Some(rest) => (true, rest),
            None => (false, written),
        };
        let quote = match body.chars().next() {
            Some(q @ ('\'' | '"')) => q,
            _ => return Value::Null,
        };
        let triple: String = std::iter::repeat_n(quote, 3).collect();
        if body.starts_with(&triple) || body.len() < 2 || !body.ends_with(quote) {
            return Value::Null;
        }
        let content = &body[1..body.len() - 1];
        if content != value || (!raw && (content.contains('\\') || content.contains('$'))) {
            return Value::Null;
        }
        let start = utf16_offset(self.source, span.start + (written.len() - body.len()) + 1);
        Value::Array(
            (0..=content.encode_utf16().count())
                .map(|i| json!(start + i))
                .collect(),
        )
    }

    fn node(&self, expression: ExprId) -> Node<'a> {
        let ast = self.ast;
        let arguments = |a: &'a dartforge_frontend::ast::Arguments| {
            a.args
                .iter()
                .map(|arg| (arg.name.as_ref().map(|n| self.name(n)), arg.value))
                .collect::<Vec<_>>()
        };
        match &ast.expr(expression).kind {
            ExprKind::InstanceCreation {
                ty,
                constructor,
                arguments: args,
                ..
            } => {
                let TypeKind::Named { name, .. } = &ast.ty(*ty).kind else {
                    return Node::Other;
                };
                // Without resolution, `new a.b()` reads `a` as an import
                // prefix and `b` as the class, whatever the parser decided.
                let mut parts: Vec<&str> = name.iter().map(|n| self.name(n)).collect();
                parts.extend(constructor.as_ref().map(|n| self.name(n)));
                Node::Creation {
                    parts,
                    arguments: arguments(args),
                }
            }
            ExprKind::Call {
                target,
                arguments: args,
            } => {
                let mut callee = *target;
                if let ExprKind::TypeArguments { target, .. } = &ast.expr(callee).kind {
                    callee = *target;
                }
                let (target, method) = match &ast.expr(callee).kind {
                    ExprKind::Identifier(name) => (Target::None, self.name(name)),
                    ExprKind::Property { target, name, .. } => {
                        let receiver = match &ast.expr(*target).kind {
                            ExprKind::Identifier(simple) => Target::Simple(self.name(simple)),
                            ExprKind::Property {
                                target: inner,
                                name: member,
                                null_aware: false,
                            } => match &ast.expr(*inner).kind {
                                ExprKind::Identifier(_) => Target::Prefixed(self.name(member)),
                                _ => Target::Other,
                            },
                            _ => Target::Other,
                        };
                        (receiver, self.name(name))
                    }
                    _ => return Node::Other,
                };
                Node::Invocation {
                    target,
                    method,
                    arguments: arguments(args),
                }
            }
            ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => match &ast.expr(*target).kind {
                ExprKind::Identifier(prefix) => Node::Prefixed(self.name(prefix), self.name(name)),
                _ => Node::Other,
            },
            ExprKind::List { elements, .. } => Node::List(elements),
            ExprKind::SetOrMap { elements, .. } => Node::Map(elements),
            ExprKind::String(literal) => {
                let mut value = String::new();
                for part in literal.parts.iter() {
                    match part {
                        StringPart::Text(text) => value.push_str(&text.to_string_lossy()),
                        StringPart::Interpolation(_) => return Node::String(None),
                    }
                }
                Node::String(Some(value))
            }
            ExprKind::Int(span) => Node::Integer(parse_integer(&self.source[span.start..span.end])),
            ExprKind::Double(span) => self.source[span.start..span.end]
                .parse()
                .map_or(Node::Other, Node::Double),
            _ => Node::Other,
        }
    }

    fn string(&self, expression: ExprId) -> Parsed<String> {
        match self.node(expression) {
            Node::String(Some(value)) => Ok(value),
            _ => fail("Expected a String literal.", self.span(expression)),
        }
    }

    fn integer(&self, expression: ExprId) -> Parsed<i64> {
        match self.node(expression) {
            Node::Integer(Some(value)) => Ok(value),
            _ => fail("Expected an integer.", self.span(expression)),
        }
    }

    fn number(&self, expression: ExprId) -> Parsed<Value> {
        match self.node(expression) {
            Node::Integer(Some(value)) => Ok(json!(value)),
            Node::Double(value) => Ok(json!(value)),
            _ => fail("Expected a number.", self.span(expression)),
        }
    }

    fn list(&self, expression: ExprId) -> Parsed<Vec<ExprId>> {
        let Node::List(elements) = self.node(expression) else {
            return fail("Expected a List.", self.span(expression));
        };
        elements
            .iter()
            .map(|element| match element {
                CollectionElement::Expression(e) => Ok(*e),
                _ => fail("Expected only literal elements.", self.span(expression)),
            })
            .collect()
    }

    fn arguments(&self, expression: ExprId) -> Parsed<Vec<(Option<&'a str>, ExprId)>> {
        match self.node(expression) {
            Node::Creation { arguments, .. } | Node::Invocation { arguments, .. } => Ok(arguments),
            _ => fail("Expected an instantiation", self.span(expression)),
        }
    }

    /// `_findConstructorName`: the named constructor used for `class`.
    fn constructor_name(&self, expression: ExprId, class: &str) -> Parsed<Option<&'a str>> {
        let expected = || fail(format!("Expected a {class}."), self.span(expression));
        match self.node(expression) {
            Node::Creation { parts, .. } => {
                let (actual, constructor) = match parts.as_slice() {
                    [actual] => (*actual, None),
                    [_, actual] => (*actual, None),
                    [_, actual, constructor] => (*actual, Some(*constructor)),
                    _ => return expected(),
                };
                if actual != class {
                    return expected();
                }
                Ok(constructor)
            }
            Node::Invocation { target, method, .. } => match target {
                Target::None if method == class => Ok(None),
                Target::None => expected(),
                _ if method == class => Ok(None),
                Target::Simple(name) | Target::Prefixed(name) if name == class => Ok(Some(method)),
                _ => expected(),
            },
            _ => expected(),
        }
    }

    fn duration(&self, expression: ExprId) -> Parsed<Value> {
        self.constructor_name(expression, "Duration")?;
        let mut values = serde_json::Map::new();
        for (name, value) in self.arguments(expression)? {
            if let Some(name) = name {
                values.insert(name.into(), json!(self.integer(value)?));
            }
        }
        let mut duration = serde_json::Map::new();
        for unit in [
            "days",
            "hours",
            "minutes",
            "seconds",
            "milliseconds",
            "microseconds",
        ] {
            duration.insert(
                unit.into(),
                values.get(unit).cloned().unwrap_or_else(|| json!(0)),
            );
        }
        Ok(json!({ "duration": duration }))
    }

    /// `_parseTimeoutConstructor`, used inside `@OnPlatform`.
    fn timeout_constructor(&self, expression: ExprId) -> Parsed<Value> {
        let name = self.constructor_name(expression, "Timeout")?;
        let arguments = self.arguments(expression)?;
        let first = || {
            arguments
                .first()
                .map(|(_, e)| *e)
                .map_or_else(|| crash("Bad state: No element", self.span(expression)), Ok)
        };
        match name {
            None => self.duration(first()?),
            Some("factor") => Ok(json!({"factor": self.number(first()?)?})),
            Some(_) => fail("Invalid timeout", self.span(expression)),
        }
    }

    /// `_parseSkipConstructor`, used inside `@OnPlatform`.
    fn skip_constructor(&self, expression: ExprId) -> Parsed<Value> {
        self.constructor_name(expression, "Skip")?;
        match self.arguments(expression)?.first() {
            None => Ok(json!(true)),
            Some((_, reason)) => Ok(json!(self.string(*reason)?)),
        }
    }

    fn on_platform(&mut self, expression: ExprId) -> Parsed<Value> {
        let Node::Map(elements) = self.node(expression) else {
            return fail("Expected a Map.", self.span(expression));
        };
        let mut entries = Vec::new();
        for element in elements {
            let CollectionElement::MapEntry { key, value, .. } = element else {
                return fail("Expected a map entry.", self.span(expression));
            };
            let selector = self.selector(*key)?;
            let expressions = match self.node(*value) {
                Node::List(_) => self.list(*value)?,
                Node::Creation { .. } | Node::Prefixed(..) | Node::Invocation { .. } => {
                    vec![*value]
                }
                _ => {
                    return fail(
                        "Expected a Timeout, Skip, or List of those.",
                        self.span(*value),
                    );
                }
            };
            let mut timeout = Value::Null;
            let mut skip = Value::Null;
            for item in expressions {
                let span = self.span(item);
                match self.node(item) {
                    Node::Creation { parts, .. }
                        if matches!(class_of(&parts), Some("Timeout" | "Skip")) =>
                    {
                        if class_of(&parts) == Some("Timeout") {
                            single(&timeout, "Timeout", span)?;
                            timeout = self.timeout_constructor(item)?;
                        } else {
                            single(&skip, "Skip", span)?;
                            skip = self.skip_constructor(item)?;
                        }
                    }
                    Node::Prefixed("Timeout", member) => {
                        if member != "none" {
                            return fail("Undefined value.", span);
                        }
                        single(&timeout, "Timeout", span)?;
                        timeout = json!({"none": true});
                    }
                    Node::Invocation { target, method, .. } => {
                        match invocation_class(&target, method) {
                            Some("Timeout") => {
                                single(&timeout, "Timeout", span)?;
                                timeout = self.timeout_constructor(item)?;
                            }
                            Some(_) => {
                                single(&skip, "Skip", span)?;
                                skip = self.skip_constructor(item)?;
                            }
                            None => return fail("Expected a Timeout or Skip.", span),
                        }
                    }
                    _ => return fail("Expected a Timeout or Skip.", span),
                }
            }
            entries.push(json!([selector, {"timeout": timeout, "skip": skip}]));
        }
        Ok(Value::Array(entries))
    }
}

/// The class an unresolved creation names: the last type part before an
/// optional constructor (`[class]`, `[prefix, class]`, `[prefix, class, ctor]`).
fn class_of<'a>(parts: &[&'a str]) -> Option<&'a str> {
    match parts {
        [class] | [_, class] | [_, class, _] => Some(class),
        _ => None,
    }
}

/// `_typeNameFromMethodInvocation` with the candidates `Timeout` and `Skip`.
fn invocation_class<'a>(target: &Target<'a>, method: &'a str) -> Option<&'a str> {
    let candidate = |name: &'a str| matches!(name, "Timeout" | "Skip").then_some(name);
    candidate(method).or(match target {
        Target::Simple(name) | Target::Prefixed(name) => candidate(name),
        Target::None | Target::Other => None,
    })
}

fn single(existing: &Value, name: &str, span: Span) -> Parsed<()> {
    if existing.is_null() {
        Ok(())
    } else {
        fail(format!("Only a single {name} may be used."), span)
    }
}

/// The analyzer's `IntegerLiteral.value`: `null` outside the 64-bit range.
fn parse_integer(text: &str) -> Option<i64> {
    let text = text.replace('_', "");
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        // Hexadecimal literals may denote negative values up to 64 bits.
        return u64::from_str_radix(hex, 16).ok().map(|v| v as i64);
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(source: &str) -> Value {
        library_metadata(source)
    }

    /// Our timeout description in `Timeout.serialize()`'s shape.
    fn serialized_timeout(value: &Value) -> Value {
        if value.is_null() {
            return json!({"duration": null, "scaleFactor": 1});
        }
        if value["none"] == true {
            return json!("none");
        }
        if !value["factor"].is_null() {
            return json!({"duration": null, "scaleFactor": value["factor"]});
        }
        let part = |unit: &str| value["duration"][unit].as_i64().unwrap();
        // Dart's `Duration` constructor wraps like 64-bit integers.
        let micros = part("microseconds")
            .wrapping_add(part("milliseconds").wrapping_mul(1_000))
            .wrapping_add(part("seconds").wrapping_mul(1_000_000))
            .wrapping_add(part("minutes").wrapping_mul(60_000_000))
            .wrapping_add(part("hours").wrapping_mul(3_600_000_000))
            .wrapping_add(part("days").wrapping_mul(86_400_000_000));
        json!({"duration": micros, "scaleFactor": null})
    }

    fn serialized_skip(value: &Value) -> (Value, Value) {
        match value {
            Value::Null => (Value::Null, Value::Null),
            Value::String(reason) => (json!(true), json!(reason)),
            _ => (json!(true), Value::Null),
        }
    }

    /// Selector parse errors, undefined variables and tag syntax are left to
    /// package:test's own classes in the planner.
    const VALIDATED_BY_PLANNER: [&str; 3] = [
        "Invalid tag name. Tags must be (optionally hyphenated) Dart identifiers.",
        "Undefined variable.",
        "Expected expression.",
    ];

    #[test]
    fn agrees_with_the_official_parse_metadata_on_recorded_cases() {
        let oracle: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/test_metadata/parse_metadata_oracle.json"
        )))
        .unwrap();
        let cases = oracle["cases"].as_array().unwrap();
        assert!(cases.len() >= 50);
        for case in cases {
            let source = case["source"].as_str().unwrap();
            let ours = metadata(source);
            if let Some(expected) = case.get("metadata") {
                assert!(ours["error"].is_null(), "{source}: {ours}");
                let m = &ours["metadata"];
                assert_eq!(m["testOn"], expected["testOn"], "{source}");
                assert_eq!(
                    serialized_timeout(&m["timeout"]),
                    expected["timeout"],
                    "{source}"
                );
                let (skip, reason) = serialized_skip(&m["skip"]);
                assert_eq!(skip, expected["skip"], "{source}");
                assert_eq!(reason, expected["skipReason"], "{source}");
                assert_eq!(m["retry"], expected["retry"], "{source}");
                let tags: BTreeSet<&str> = m["tags"]
                    .as_array()
                    .map(|t| t.iter().map(|t| t.as_str().unwrap()).collect())
                    .unwrap_or_default();
                let expected_tags: BTreeSet<&str> = expected["tags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t.as_str().unwrap())
                    .collect();
                assert_eq!(tags, expected_tags, "{source}");
                // A Dart map keeps the first position of a repeated key and
                // its last value.
                let mut platforms: Vec<(Value, Value)> = Vec::new();
                for entry in m["onPlatform"].as_array().into_iter().flatten() {
                    let (skip, reason) = serialized_skip(&entry[1]["skip"]);
                    let inner = json!({
                        "timeout": serialized_timeout(&entry[1]["timeout"]),
                        "skip": skip,
                        "skipReason": reason,
                    });
                    match platforms.iter_mut().find(|(k, _)| *k == entry[0]) {
                        Some(slot) => slot.1 = inner,
                        None => platforms.push((entry[0].clone(), inner)),
                    }
                }
                let expected_platforms: Vec<(Value, Value)> = expected["onPlatform"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| {
                        (
                            e[0].clone(),
                            json!({
                                "timeout": e[1]["timeout"],
                                "skip": e[1]["skip"],
                                "skipReason": e[1]["skipReason"],
                            }),
                        )
                    })
                    .collect();
                assert_eq!(platforms, expected_platforms, "{source}");
            } else if let Some(message) = case["error"].as_str() {
                if VALIDATED_BY_PLANNER.contains(&message) {
                    assert!(ours["error"].is_null(), "{source}: {ours}");
                    assert!(!ours["checks"].as_array().unwrap().is_empty(), "{source}");
                } else {
                    assert_eq!(ours["error"]["message"], message, "{source}");
                    assert_eq!(ours["error"]["fatal"], false, "{source}");
                }
            } else {
                assert_eq!(ours["error"]["message"], case["crash"], "{source}");
                assert_eq!(ours["error"]["fatal"], true, "{source}");
            }
        }
    }

    #[test]
    fn reads_every_supported_annotation_from_the_first_directive() {
        let value = metadata(
            "@TestOn('vm && !windows')\n@Timeout(Duration(seconds: 2, milliseconds: 5))\n\
             @Skip('later')\n@Tags(['a', 'b-c'])\n@Retry(3)\n\
             @OnPlatform({'mac-os': Skip(), 'linux': [Timeout.factor(2), Skip('x')], \
             'windows': Timeout.none})\nlibrary;\nimport 'package:test/test.dart';\nvoid main() {}\n",
        );
        let m = &value["metadata"];
        assert_eq!(m["testOn"], "vm && !windows");
        assert_eq!(m["timeout"]["duration"]["seconds"], 2);
        assert_eq!(m["timeout"]["duration"]["milliseconds"], 5);
        assert_eq!(m["skip"], "later");
        assert_eq!(m["tags"], json!(["a", "b-c"]));
        assert_eq!(m["retry"], 3);
        assert_eq!(
            m["onPlatform"][0],
            json!(["mac-os", {"timeout": null, "skip": true}])
        );
        assert_eq!(
            m["onPlatform"][1],
            json!(["linux", {"timeout": {"factor": 2}, "skip": "x"}])
        );
        assert_eq!(m["onPlatform"][2][1]["timeout"], json!({"none": true}));
        let kinds: Vec<_> = value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                format!(
                    "{}:{}",
                    c["kind"].as_str().unwrap(),
                    c["text"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "selector:vm && !windows",
                "tag:a",
                "tag:b-c",
                "selector:mac-os",
                "selector:linux",
                "selector:windows"
            ]
        );
    }

    #[test]
    fn prefixes_and_named_constructors_follow_the_unresolved_reading() {
        let value =
            metadata("@t.Timeout.factor(1.5)\n@t.Skip()\nimport 'package:test/test.dart' as t;\n");
        assert_eq!(value["metadata"]["timeout"], json!({"factor": 1.5}));
        assert_eq!(value["metadata"]["skip"], true);
        // Without a matching prefix, `@Timeout.none` is the named constructor.
        let value = metadata("@Timeout.none\nlibrary;\n");
        assert_eq!(value["metadata"]["timeout"], json!({"none": true}));
        // `@Timeout(...)` behind an unrelated prefix is not the test annotation.
        let value = metadata("@x.Other(1)\nimport 'a.dart' as x;\n");
        assert_eq!(value["metadata"]["timeout"], Value::Null);
    }

    #[test]
    fn only_the_first_directive_and_real_annotations_count() {
        let value = metadata(
            "// @Skip('comment')\nimport 'package:test/test.dart';\n@Skip('second')\nimport 'b.dart';\n",
        );
        assert_eq!(value["metadata"]["skip"], Value::Null);
        assert!(metadata("void main() {}")["metadata"]["skip"].is_null());
    }

    #[test]
    fn structural_errors_carry_the_official_messages_and_utf16_spans() {
        let source = "// é\n@Timeout(Duration(seconds: 1))\n@Timeout.none\nlibrary;\n";
        let value = metadata(source);
        assert_eq!(
            value["error"]["message"],
            "Only a single Timeout may be used."
        );
        // `é` is two UTF-8 bytes but a single UTF-16 unit.
        let start = source.find("@Timeout.none").unwrap() - 1;
        assert_eq!(value["error"]["start"], start);
        for (source, message) in [
            ("@TestOn(1)\nlibrary;", "Expected a String literal."),
            ("@Tags('a')\nlibrary;", "Expected a List."),
            ("@Retry('a')\nlibrary;", "Expected an integer."),
            ("@Timeout(seconds(1))\nlibrary;", "Expected a Duration."),
            ("@Timeout.factor('a')\nlibrary;", "Expected a number."),
            (
                "@OnPlatform({'vm': 1})\nlibrary;",
                "Expected a Timeout, Skip, or List of those.",
            ),
            (
                "@OnPlatform({'vm': Timeout.never})\nlibrary;",
                "Undefined value.",
            ),
            (
                "@OnPlatform({'vm': Other()})\nlibrary;",
                "Expected a Timeout or Skip.",
            ),
            ("@OnPlatform(['vm'])\nlibrary;", "Expected a Map."),
            ("@TestOn('vm $x')\nlibrary;", "Expected a String literal."),
        ] {
            assert_eq!(metadata(source)["error"]["message"], message, "{source}");
        }
    }
}
