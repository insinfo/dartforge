//! Reading of the `.dart_tool/build/entrypoint/build.dart` that `build_runner`
//! generated — the plan's oracle. It is **read, never generated** (running
//! `build_runner` in the user's project is not allowed). From it comes the
//! same canonical form as `Application::canonical_text`, to compare line by
//! line.
//!
//! The grammar is the one `code_builder` + `dart_style` produce in that file:
//! calls with positional and named arguments, lists and maps with optional
//! type arguments and `const`, strings (`r'…'` and regular ones), numbers,
//! `true`/`false`/`null` and `_iN.name` references.
use crate::config::InputSet;
use crate::plan::{Application, ApplyFilter};
use crate::value::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Expr {
    Call {
        target: String,
        pos: Vec<Expr>,
        named: Vec<(String, Expr)>,
    },
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Ref(String),
}

struct Lx<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lx<'_> {
    fn skip_whitespace(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.s[self.i..].starts_with(b"/*") {
                while self.i + 1 < self.s.len() && !self.s[self.i..].starts_with(b"*/") {
                    self.i += 1;
                }
                self.i = (self.i + 2).min(self.s.len());
                continue;
            }
            break;
        }
    }
    fn peek(&mut self, t: &str) -> bool {
        self.skip_whitespace();
        self.s[self.i..].starts_with(t.as_bytes())
    }
    fn take(&mut self, t: &str) -> bool {
        if self.peek(t) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: &str) -> Result<(), String> {
        if self.take(t) {
            Ok(())
        } else {
            Err(format!("build.dart: expected `{t}` at position {}", self.i))
        }
    }
    fn ident(&mut self) -> Option<String> {
        self.skip_whitespace();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric()
                || self.s[self.i] == b'_'
                || self.s[self.i] == b'$')
        {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).to_string())
    }

    /// `<...>` of type arguments: ignored.
    fn type_args(&mut self) {
        if self.peek("<") {
            let mut n = 0;
            while self.i < self.s.len() {
                match self.s[self.i] {
                    b'<' => n += 1,
                    b'>' => {
                        n -= 1;
                        if n == 0 {
                            self.i += 1;
                            return;
                        }
                    }
                    _ => {}
                }
                self.i += 1;
            }
        }
    }

    fn string(&mut self) -> Result<Option<String>, String> {
        self.skip_whitespace();
        let raw = self.s[self.i..].starts_with(b"r'") || self.s[self.i..].starts_with(b"r\"");
        if raw {
            self.i += 1;
        }
        let Some(&q) = self.s.get(self.i) else {
            return Ok(None);
        };
        if q != b'\'' && q != b'"' {
            if raw {
                self.i -= 1;
            }
            return Ok(None);
        }
        self.i += 1;
        let mut v: Vec<u8> = Vec::new();
        while self.i < self.s.len() && self.s[self.i] != q {
            let c = self.s[self.i];
            if c == b'\\' && !raw {
                self.i += 1;
                let e = self.s.get(self.i).copied().unwrap_or(b'\\');
                v.push(match e {
                    b'n' => b'\n',
                    b't' => b'\t',
                    b'r' => b'\r',
                    x => x,
                });
            } else {
                v.push(c);
            }
            self.i += 1;
        }
        self.i += 1;
        let s = String::from_utf8(v).map_err(|e| e.to_string())?;
        // Adjacent strings are concatenated.
        if let Some(more) = self.string()? {
            return Ok(Some(s + &more));
        }
        Ok(Some(s))
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.take("const ");
        if let Some(t) = self.string()? {
            return Ok(Expr::Str(t));
        }
        self.skip_whitespace();
        if self.peek("<") {
            self.type_args();
        }
        if self.take("[") {
            let mut v = Vec::new();
            while !self.take("]") {
                v.push(self.expr()?);
                if !self.take(",") {
                    self.expect("]")?;
                    break;
                }
            }
            return Ok(Expr::List(v));
        }
        if self.take("{") {
            let mut v = Vec::new();
            while !self.take("}") {
                let k = self.expr()?;
                self.expect(":")?;
                let val = self.expr()?;
                v.push((k, val));
                if !self.take(",") {
                    self.expect("}")?;
                    break;
                }
            }
            return Ok(Expr::Map(v));
        }
        self.skip_whitespace();
        let c = self.s.get(self.i).copied().unwrap_or(0);
        if c.is_ascii_digit() || c == b'-' {
            let start = self.i;
            self.i += 1;
            while self.i < self.s.len()
                && (self.s[self.i].is_ascii_alphanumeric()
                    || matches!(self.s[self.i], b'.' | b'+' | b'-'))
            {
                self.i += 1;
            }
            return Ok(Expr::Num(
                String::from_utf8_lossy(&self.s[start..self.i]).to_string(),
            ));
        }
        let mut name = self
            .ident()
            .ok_or_else(|| format!("build.dart: unexpected expression at position {}", self.i))?;
        match name.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "null" => return Ok(Expr::Null),
            _ => {}
        }
        while self.take(".") {
            name.push('.');
            name.push_str(&self.ident().ok_or("build.dart: expected a name")?);
        }
        self.type_args();
        if !self.take("(") {
            return Ok(Expr::Ref(name));
        }
        let (mut pos, mut named) = (Vec::new(), Vec::new());
        while !self.take(")") {
            let back = self.i;
            let named_arg = self.ident().filter(|_| self.take(":") && !self.peek(":"));
            match named_arg {
                Some(n) => named.push((n, self.expr()?)),
                None => {
                    self.i = back;
                    pos.push(self.expr()?);
                }
            }
            if !self.take(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(Expr::Call {
            target: name,
            pos,
            named,
        })
    }
}

fn value(e: &Expr) -> Result<Value, String> {
    Ok(match e {
        Expr::Str(t) => Value::Text(t.clone()),
        Expr::Num(n) => match n.parse::<i64>() {
            Ok(i) if !n.contains('.') && !n.contains('e') => Value::Int(i),
            _ => Value::Real(n.clone()),
        },
        Expr::Bool(b) => Value::Bool(*b),
        Expr::Null => Value::Null,
        Expr::List(l) => Value::List(l.iter().map(value).collect::<Result<_, _>>()?),
        Expr::Map(m) => {
            let mut r = Map::default();
            for (k, v) in m {
                r.insert(value(k)?, value(v)?);
            }
            Value::Map(r)
        }
        other => return Err(format!("build.dart: unexpected option value: {other:?}")),
    })
}

fn strings(e: &Expr) -> Result<Vec<String>, String> {
    match e {
        Expr::List(l) => l
            .iter()
            .map(|x| match x {
                Expr::Str(t) => Ok(t.clone()),
                o => Err(format!("build.dart: expected a string, got {o:?}")),
            })
            .collect(),
        o => Err(format!("build.dart: expected a list, got {o:?}")),
    }
}

fn options(e: &Expr) -> Result<Map, String> {
    match e {
        Expr::Call { pos, .. } if pos.len() == 1 => match value(&pos[0])? {
            Value::Map(m) => Ok(m),
            o => Err(format!("build.dart: BuilderOptions without a map: {o:?}")),
        },
        o => Err(format!(
            "build.dart: expected BuilderOptions(...), got {o:?}"
        )),
    }
}

fn method(target: &str) -> &str {
    target.rsplit('.').next().unwrap_or(target)
}

/// The applications of `build.dart`, in order, as `Application` (the fields the
/// script does not carry are left empty).
pub fn read_build_dart(text: &str) -> Result<Vec<Application>, String> {
    let mut lx = Lx {
        s: text.as_bytes(),
        i: 0,
    };
    // Imports: `import 'uri' as _iN;`
    let mut prefixes: HashMap<String, String> = HashMap::new();
    loop {
        lx.skip_whitespace();
        if !lx.take("import") {
            break;
        }
        let uri = lx.string()?.ok_or("build.dart: import without a URI")?;
        if lx.take("as") {
            let p = lx.ident().ok_or("build.dart: import without a prefix")?;
            prefixes.insert(p, uri);
        }
        lx.expect(";")?;
    }
    lx.expect("final")?;
    lx.ident();
    lx.expect("=")?;
    let Expr::List(items) = lx.expr()? else {
        return Err("build.dart: `_builders` is not a list".into());
    };
    let resolve = |r: &str| -> String {
        match r.split_once('.') {
            Some((p, n)) => format!(
                "{}#{n}",
                prefixes.get(p).cloned().unwrap_or_else(|| p.to_string())
            ),
            None => r.to_string(),
        }
    };
    let mut v = Vec::new();
    for it in items {
        let Expr::Call { target, pos, named } = it else {
            return Err("build.dart: `_builders` item is not a call".into());
        };
        let is_post = match method(&target) {
            "apply" => false,
            "applyPostProcess" => true,
            o => return Err(format!("build.dart: unexpected call `{o}`")),
        };
        let key = match pos.first() {
            Some(Expr::Str(t)) => t.clone(),
            _ => return Err("build.dart: application without a key".into()),
        };
        let refs: Vec<String> = match pos.get(1) {
            Some(Expr::List(l)) => l
                .iter()
                .map(|x| match x {
                    Expr::Ref(r) => Ok(resolve(r)),
                    o => Err(format!("build.dart: unexpected factory {o:?}")),
                })
                .collect::<Result<_, _>>()?,
            Some(Expr::Ref(r)) => vec![resolve(r)],
            _ => return Err(format!("build.dart: `{key}` without factories")),
        };
        let import = refs
            .first()
            .and_then(|r| r.split_once('#'))
            .map(|(i, _)| i.to_string())
            .unwrap_or_default();
        let factories: Vec<String> = refs
            .iter()
            .map(|r| r.split_once('#').map(|(_, n)| n).unwrap_or(r).to_string())
            .collect();
        let filter = if is_post {
            ApplyFilter::NoneByDefault
        } else {
            match pos.get(2) {
                Some(Expr::Call { target, pos, .. }) => match method(target) {
                    "toNoneByDefault" => ApplyFilter::NoneByDefault,
                    "toAllPackages" => ApplyFilter::All,
                    "toRoot" => ApplyFilter::Root,
                    "toDependentsOf" => match pos.first() {
                        Some(Expr::Str(p)) => ApplyFilter::DependentsOf(p.clone()),
                        _ => return Err("build.dart: toDependentsOf without a package".into()),
                    },
                    o => return Err(format!("build.dart: unexpected filter `{o}`")),
                },
                _ => return Err(format!("build.dart: `{key}` without a filter")),
            }
        };
        let mut a = Application {
            key,
            post_process: is_post,
            import,
            factories,
            filter,
            optional: false,
            hidden: is_post,
            generate_for_pattern: None,
            default_options: Map::default(),
            dev_options: Map::default(),
            release_options: Map::default(),
            applies: Vec::new(),
            package: String::new(),
            declared_extensions: Vec::new(),
        };
        for (n, e) in &named {
            match n.as_str() {
                "isOptional" => a.optional = matches!(e, Expr::Bool(true)),
                "hideOutput" => a.hidden = matches!(e, Expr::Bool(true)),
                "defaultGenerateFor" => {
                    let Expr::Call { named, .. } = e else {
                        return Err("build.dart: defaultGenerateFor is not an InputSet".into());
                    };
                    let mut s = InputSet::default();
                    for (k, v) in named {
                        match k.as_str() {
                            "include" => s.include = Some(strings(v)?),
                            "exclude" => s.exclude = Some(strings(v)?),
                            o => return Err(format!("build.dart: InputSet.{o}")),
                        }
                    }
                    a.generate_for_pattern = Some(s);
                }
                "defaultOptions" => a.default_options = options(e)?,
                "defaultDevOptions" => a.dev_options = options(e)?,
                "defaultReleaseOptions" => a.release_options = options(e)?,
                "appliesBuilders" => a.applies = strings(e)?,
                o => return Err(format!("build.dart: unexpected named argument `{o}`")),
            }
        }
        v.push(a);
    }
    Ok(v)
}

/// Compares the plan with `build.dart`: `Ok(())` if they are equal, otherwise
/// the list of differences per line (`-` oracle, `+` engine).
pub fn compare(plan: &[Application], build_dart: &str) -> Result<Result<(), Vec<String>>, String> {
    let oracle: Vec<String> = read_build_dart(build_dart)?
        .iter()
        .map(Application::canonical_text)
        .collect();
    let ours: Vec<String> = plan.iter().map(Application::canonical_text).collect();
    if oracle == ours {
        return Ok(Ok(()));
    }
    let mut diff = Vec::new();
    for i in 0..oracle.len().max(ours.len()) {
        match (oracle.get(i), ours.get(i)) {
            (Some(a), Some(b)) if a == b => {}
            (a, b) => {
                if let Some(a) = a {
                    diff.push(format!("- [{i}] {a}"));
                }
                if let Some(b) = b {
                    diff.push(format!("+ [{i}] {b}"));
                }
            }
        }
    }
    Ok(Err(diff))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXCERPT: &str = r#"// @dart=3.6
// ignore_for_file: directives_ordering
import 'package:build_runner_core/build_runner_core.dart' as _i1;
import 'package:source_gen/builder.dart' as _i2;
import 'package:sass_builder/sass_builder.dart' as _i3;
import 'package:build/build.dart' as _i4;
import 'package:build_config/build_config.dart' as _i8;
import 'dart:isolate' as _i11;

final _builders = <_i1.BuilderApplication>[
  _i1.apply(
    r'source_gen:combining_builder',
    [_i2.combiningBuilder],
    _i1.toNoneByDefault(),
    hideOutput: false,
    appliesBuilders: const [r'source_gen:part_cleanup'],
  ),
  _i1.apply(
    r'sass_builder:sass_builder',
    [_i3.sassBuilder],
    _i1.toDependentsOf(r'sass_builder'),
    hideOutput: true,
    defaultDevOptions:
        const _i4.BuilderOptions(<String, dynamic>{r'sourceMaps': true}),
    defaultReleaseOptions: const _i4.BuilderOptions(<String, dynamic>{
      r'outputStyle': r'compressed',
      r'sourceMaps': false,
    }),
    appliesBuilders: const [r'sass_builder:sass_source_cleanup'],
  ),
  _i1.applyPostProcess(
    r'source_gen:part_cleanup',
    _i2.partCleanup,
    defaultGenerateFor: const _i8.InputSet(include: [r'lib/**']),
  ),
];
void main(
  List<String> args, [
  _i11.SendPort? sendPort,
]) async {}
"#;

    #[test]
    fn reads_the_build_dart() {
        let v = read_build_dart(EXCERPT).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(
            v[1].canonical_text(),
            r#"apply "sass_builder:sass_builder" ["package:sass_builder/sass_builder.dart#sassBuilder"] toDependentsOf("sass_builder") isOptional=false hideOutput=true defaultGenerateFor=- defaultOptions={} defaultDevOptions={"sourceMaps": true} defaultReleaseOptions={"outputStyle": "compressed", "sourceMaps": false} appliesBuilders=["sass_builder:sass_source_cleanup"]"#
        );
        assert_eq!(
            v[2].canonical_text(),
            r#"applyPostProcess "source_gen:part_cleanup" ["package:source_gen/builder.dart#partCleanup"] defaultGenerateFor=InputSet(include: ["lib/**"], exclude: -) defaultOptions={} defaultDevOptions={} defaultReleaseOptions={}"#
        );
    }
}
