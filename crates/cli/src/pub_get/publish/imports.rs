//! The `import`, `export` and `part` directives of a Dart file as Pub's
//! `StrictDependenciesValidator` reads them (`parseImportsAndExports` of the
//! analyzer): files with syntax errors are skipped, and each directive spans
//! from its documentation comment or metadata to its `;`.
use super::paths as p;
use super::span::SourceFile;
use dartforge_frontend::ast::DirectiveKind;
use dartforge_frontend::{LanguageVersion, LibraryFeatures};

/// A `package:` URI used by a directive.
pub(crate) struct Usage {
    pub file: String,
    pub package: String,
    contents: String,
    start: usize,
    end: usize,
}

impl Usage {
    /// `_Usage.errorMessage`: the directive highlighted with `message`.
    pub fn message(&self, message: &str) -> String {
        SourceFile::new(&self.contents, &self.file).message(self.start, self.end, message)
    }
}

/// The UTF-16 offset of the byte offset `at` of `text`.
fn utf16_offset(text: &str, at: usize) -> usize {
    text[..at].encode_utf16().count()
}

/// The comments between `from` and `to` (both byte offsets at token
/// boundaries), as `(start, text)`.
fn comments(source: &str, from: usize, to: usize) -> Vec<(usize, &str)> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut i = from;
    while i < to {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            let end = source[i..to].find('\n').map_or(to, |n| i + n);
            result.push((i, &source[i..end]));
            i = end;
        } else if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            let mut depth = 0;
            while i < to {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            result.push((start, &source[start..i]));
        } else {
            i += 1;
        }
    }
    result
}

/// `_parseDocumentationCommentOpt`: the start of the documentation comment
/// among the comments before a directive (the last `/** */`, or the `///`
/// lines after it).
fn doc_comment_start(comments: &[(usize, &str)]) -> Option<usize> {
    let mut tokens: Vec<(usize, bool)> = Vec::new();
    for (start, text) in comments {
        let line = text.starts_with("///") && !text.starts_with("////");
        let block = text.starts_with("/**") && *text != "/**/";
        if !(line || block) {
            continue;
        }
        if !tokens.is_empty() && (!line || !tokens[0].1) {
            tokens.clear();
        }
        tokens.push((*start, line));
    }
    tokens.first().map(|(start, _)| *start)
}

/// Calls `found` with each `package:` usage of `file` (an `Err` for an
/// invalid URI), in directive order; nothing for files that do not parse.
pub(crate) fn directives(
    file: &str,
    language: (u64, u64),
    found: &mut dyn FnMut(Result<Usage, String>),
) {
    let Ok(bytes) = std::fs::read(p::normalize(&p::absolute(file))) else {
        return;
    };
    let contents = String::from_utf8_lossy(&bytes).into_owned();
    let marker = dartforge_frontend::features::version_marker(&contents).map(|(v, _)| v);
    let package_version = LanguageVersion::new(language.0 as u16, language.1 as u16);
    let features =
        LibraryFeatures::for_library(marker, package_version, LanguageVersion::FLOOR, &[]);
    let mut names = dartforge_frontend::Interner::new();
    let parsed = dartforge_frontend::parser::parse_with(&contents, &mut names, features);
    if !parsed.diagnostics.is_empty() {
        return;
    }
    let mut previous_end = parsed.unit.script_tag.map_or(0, |s| s.end);
    for directive in &parsed.unit.directives {
        let uri = match &directive.kind {
            DirectiveKind::Import { uri, .. }
            | DirectiveKind::Export { uri, .. }
            | DirectiveKind::Part { uri }
            | DirectiveKind::ImportAugment { uri } => uri,
            _ => {
                previous_end = directive.span.end;
                continue;
            }
        };
        let mut start = directive.span.start;
        if let Some(doc) = doc_comment_start(&comments(&contents, previous_end, start)) {
            start = doc;
        }
        let end = directive.span.end;
        previous_end = end;
        let value = uri
            .constant_value()
            .and_then(|v| v.as_str().map(str::to_owned));
        let usage = |package: String| Usage {
            file: file.to_owned(),
            package,
            contents: contents.clone(),
            start: utf16_offset(&contents, start),
            end: utf16_offset(&contents, end),
        };
        match value.as_deref().map(parse_uri) {
            Some(Some(Parsed::Package(package))) => found(Ok(usage(package))),
            Some(Some(Parsed::Other)) => {}
            _ => found(Err(usage(String::new()).message("Invalid URL."))),
        }
    }
}

enum Parsed {
    /// A valid `package:` URI, by its package name.
    Package(String),
    Other,
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// `Uri.tryParse` and the checks of `_findPackages`: `None` for an invalid
/// URI (or a `package:` URI without a package and a path).
fn parse_uri(text: &str) -> Option<Parsed> {
    let scheme_end = text.find(':');
    let scheme = scheme_end.and_then(|i| {
        let s = &text[..i];
        let valid = !s.is_empty()
            && s.as_bytes()[0].is_ascii_alphabetic()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.');
        valid.then(|| s.to_ascii_lowercase())
    });
    // Malformed percent escapes make `Uri.tryParse` fail.
    let rest = match (&scheme, scheme_end) {
        (Some(_), Some(i)) => &text[i + 1..],
        _ => text,
    };
    let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = &rest[..path_end];
    percent_decode(path)?;
    if scheme.as_deref() != Some("package") {
        return Some(Parsed::Other);
    }
    let path = path.strip_prefix('/').unwrap_or(path);
    let segments: Vec<String> = if path.is_empty() {
        Vec::new()
    } else {
        path.split('/')
            .map(|s| percent_decode(s).unwrap_or_default())
            .collect()
    };
    if segments.len() < 2 || segments.iter().any(String::is_empty) {
        return None;
    }
    Some(Parsed::Package(segments[0].clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_uris_are_checked_like_pub() {
        assert!(matches!(parse_uri("package:a/b.dart"), Some(Parsed::Package(p)) if p == "a"));
        assert!(parse_uri("package:a").is_none());
        assert!(parse_uri("package:a//b.dart").is_none());
        assert!(matches!(parse_uri("dart:io"), Some(Parsed::Other)));
        assert!(matches!(parse_uri("src/x.dart"), Some(Parsed::Other)));
        assert!(parse_uri("package:a/%zz.dart").is_none());
    }

    #[test]
    fn documentation_comments_start_the_directive() {
        let source = "// plain\n/// doc\n/// more\nimport 'a';";
        let start = source.find("import").unwrap();
        let found = doc_comment_start(&comments(source, 0, start));
        assert_eq!(found, Some(source.find("/// doc").unwrap()));
        assert_eq!(doc_comment_start(&comments("// x\n", 0, 5)), None);
    }
}
