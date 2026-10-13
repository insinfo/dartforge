//! Conversion of template expressions to Dart.
//!
//! `ngcompiler` has an expression parser of its own (1,620 lines in
//! `compiler/expression_parser`). We do not need it: a template expression is a
//! Dart expression, and the new track's parser already parses it. The work here
//! is only to rewrite the tree with the right receiver — `name` becomes
//! `_ctx.name` — and with the parentheses the official emitter adds.
//!
//! What `expression_converter.dart` writes and this module reproduces:
//! `_ctx.item.nome`, `_ctx.titulo()`, `(!_ctx.item.ativo)`, `'fixo'`.
//!
//! Three properties of the expression decide the shape of the generated code,
//! and each one follows the official rule (`analyzed_class.dart`):
//! - `isImmutable`: only a literal, `a ?? b` and a binary of immutables, and a
//!   `final`/`const` field read with the implicit receiver. An explicit getter
//!   is **mutable** (its `variable` is synthetic); `a.b`, `!x`, a call and a
//!   ternary too.
//! - `canBeNull`: only a literal is not null (and `a ?? b` when one side is
//!   not).
//! - the type (`_TypeResolver`): of the member or of the getter; the rest is
//!   `dynamic`.
//!
//! Event handlers (`parseAction`) are the same language with two more shapes:
//! `$event` and assignment (`x = y`, `a.b = y`).
use crate::component::Member;
use crate::resolucao::Resolution;
use crate::view::{Reason, Refusal, refusal};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Start and end of the mark of a pipe call in the converted text:
/// `\u{3}name/arguments\u{4}`. The converter knows neither the view nor the
/// order of the calls; the view replaces the mark with the proxy.
pub const PIPE_MARK: char = '\u{3}';

/// Prefix of [`Converted::is_static`] for an `exports:` receiver without an
/// `AnalyzedClass` ([`crate::resolucao::Exported::TypeWithoutClass`]).
const WITHOUT_CLASS: char = '\u{7}';
pub const PIPE_END: char = '\u{4}';

/// A local of an embedded view: the name in Dart and the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// How it comes out in the generated code: `local_item`.
    pub dart: String,
    /// Static type, for choosing the interpolation shape.
    pub ty: String,
    /// File in whose scope the type text was written (the type of a member of
    /// another class is written in that class's library). `None`: the
    /// component's own.
    pub scope: Option<PathBuf>,
}

/// A template expression already converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// The Dart that goes into the generated file.
    pub text: String,
    /// ngcompiler's `isImmutable`: a value that does not change needs no
    /// `checkBinding` and is written once, on the first check.
    pub immutable: bool,
    /// ngcompiler's `canBeNull`: decides the `if (x != null)` around the
    /// constant binding and `setAttribute` × `updateAttribute`.
    pub can_be_null: bool,
    /// Static type, when it can be known without leaving the component class.
    /// It is what chooses between `interpolateString`, `interpolate` and
    /// `updateTextWithPrimitive`. `None`: unknown.
    pub ty: Option<String>,
    /// File in whose scope `ty` was written, when it is not the component's.
    pub scope: Option<PathBuf>,
    /// The shape of the expression (`call`, `ternary`…), so the scoreboard says
    /// what is missing when it is refused in context.
    pub shape: &'static str,
    /// It is a primitive literal (`'x'`, `1`, `true`, `null`): the value is the
    /// text itself.
    pub literal: bool,
    /// The view locals the expression reads, in the order the official
    /// converter asks the `ViewNameResolver` for them — the order in which the
    /// `final local_x = …` declarations come out in the method.
    pub locals: Vec<String>,
    /// A class of `exports:` (the simple name): only valid as the receiver of a
    /// static member (`Class.name`).
    pub is_static: Option<String>,
}

impl Converted {
    fn new(text: String, shape: &'static str) -> Self {
        Converted {
            text,
            immutable: false,
            can_be_null: true,
            ty: None,
            scope: None,
            shape,
            literal: false,
            locals: Vec::new(),
            is_static: None,
        }
    }
}

/// Line break of the `DartFormatter` (dart_style 2.3.8, page of 1,000,000
/// columns) that the ngdart builder runs over the output: with an infinite
/// page, only the trailing comma of an argument list breaks it, and the emitter
/// writes a comma after each named argument (`visitAllNamedExpressions`). The
/// mark is [`BREAK`] followed by one [`INDENT`] per space beyond the
/// indentation of the line where the expression is, which is only known when
/// the file is assembled ([`resolve_breaks`]).
pub const BREAK: char = '\u{10}';
pub const INDENT: char = '\u{11}';

/// Adds `n` spaces to each [`BREAK`] of `t`.
pub fn indent_breaks(t: &str, n: usize) -> String {
    let more: String = std::iter::repeat_n(INDENT, n).collect();
    t.replace(BREAK, &format!("{BREAK}{more}"))
}

/// Replaces each [`BREAK`] with the line break with the indentation of the
/// line it is on plus its [`INDENT`]s.
pub fn resolve_breaks(text: &str) -> String {
    if !text.contains(BREAK) {
        return text.to_string();
    }
    let mut output = String::with_capacity(text.len() + text.len() / 8);
    for (k, line) in text.split('\n').enumerate() {
        if k > 0 {
            output.push('\n');
        }
        let base = line.len() - line.trim_start_matches(' ').len();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c != BREAK {
                output.push(c);
                continue;
            }
            let mut n = base;
            while chars.next_if_eq(&INDENT).is_some() {
                n += 1;
            }
            output.push('\n');
            output.extend(std::iter::repeat_n(' ', n));
        }
    }
    output
}

/// Joins lists of locals keeping the first occurrence of each.
fn merge_unique(parts: &[&[String]]) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    for p in parts {
        for l in p.iter() {
            if !output.contains(l) {
                output.push(l.clone());
            }
        }
    }
    output
}

/// What a component's expressions see: members, methods, the view locals and
/// the semantic database.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub members: &'a HashMap<String, Member>,
    pub methods: &'a HashMap<String, String>,
    /// Positional parameters of each method (`rewriteTearOff`).
    pub arities: &'a HashMap<String, usize>,
    pub locals: &'a HashMap<String, Local>,
    pub types: Option<(&'a dyn Resolution, &'a Path)>,
    /// The component class as the generated file writes it (`import1.Class`),
    /// for static members; `None` when unknown, and the static is refused.
    pub class: Option<&'a str>,
    /// The names of `exports:` (`_matchExport`), with the deferred qualifier.
    pub exports: Option<&'a crate::view::Exports>,
    /// The component's type parameters (`T` of `Class<T>`): in scope in all its
    /// views, they go in as they are when substituting the inherited types.
    pub generics: &'a [String],
}

/// Converts an expression written in the template.
///
/// `members` are the fields and getters of the component: a name that is not
/// there does not belong to `_ctx` and the expression is refused, because
/// translating it wrong would give a file that compiles and does something
/// else.
pub fn converter(
    expression: &str,
    members: &HashMap<String, Member>,
    interner: &mut Interner,
) -> Result<Converted, Refusal> {
    convert_with_methods(expression, members, &HashMap::new(), interner, None)
}

/// Like [`converter`], also knowing the class's methods — which are only valid
/// as call targets.
pub fn convert_with_methods(
    expression: &str,
    members: &HashMap<String, Member>,
    methods: &HashMap<String, String>,
    interner: &mut Interner,
    types: Option<(&dyn Resolution, &Path)>,
) -> Result<Converted, Refusal> {
    convert_with_locals(
        expression,
        members,
        methods,
        &HashMap::new(),
        interner,
        types,
    )
}

/// Like [`convert_with_methods`], with the locals of an embedded view (`let
/// item of itens` puts `item` in scope). A local wins over the component
/// member, as the template scope dictates.
pub fn convert_with_locals(
    expression: &str,
    members: &HashMap<String, Member>,
    methods: &HashMap<String, String>,
    locals: &HashMap<String, Local>,
    interner: &mut Interner,
    types: Option<(&dyn Resolution, &Path)>,
) -> Result<Converted, Refusal> {
    let arities = HashMap::new();
    let scope = Scope {
        members,
        methods,
        arities: &arities,
        locals,
        types,
        class: None,
        exports: None,
        generics: &[],
    };
    convert_in_scope(expression, &scope, interner)
}

/// Converts a binding expression (`parseBinding`/`parseInterpolation`).
pub fn convert_in_scope(
    expression: &str,
    scope: &Scope,
    interner: &mut Interner,
) -> Result<Converted, Refusal> {
    let (parsed, source, root) = parse(expression, interner)?;
    let c = Converter {
        ast: &parsed.ast,
        source: &source,
        interner,
        scope,
        action: false,
    };
    c.expr(root, true)
}

/// A converted event handler: `parseAction` followed by
/// `handlerTypeFromExpression` (`parse_utils.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `m()` or `m($event)` with the implicit receiver (or the tear-off `m`
    /// rewritten into one of these by `rewriteTearOff`): comes out as a
    /// tear-off, `this.eventHandlerN(_ctx.m)`.
    Simple {
        method: String,
        arity: u8,
        /// The same call as a statement (`_ctx.m($event)`), for when this
        /// handler is merged with another one of the same event.
        statement: String,
    },
    /// Any other shape: a statement of the `_handleEvent_N` method.
    Complexa(Converted),
}

/// Converts the text of an `(event)="..."`.
pub fn convert_action(
    expression: &str,
    scope: &Scope,
    interner: &mut Interner,
) -> Result<Action, Refusal> {
    let (parsed, source, root) = parse(expression, interner)?;
    let c = Converter {
        ast: &parsed.ast,
        source: &source,
        interner,
        scope,
        action: true,
    };
    c.action(root)
}

/// Parses the text as a Dart expression: a top-level `var` is the smallest
/// context in which the parser accepts any expression.
fn parse(
    expression: &str,
    interner: &mut Interner,
) -> Result<(dartforge_frontend::parser::Parsed, String, ast::ExprId), Refusal> {
    let source = format!("var _e = {expression};");
    let parsed = dartforge_frontend::parser::parse(&source, interner);
    let invalid = || {
        refusal(
            Reason::Expression,
            "expression the Dart parser does not accept",
        )
    };
    if !parsed.diagnostics.is_empty() {
        return Err(invalid());
    }
    let Some(&id) = parsed.unit.declarations.first() else {
        return Err(invalid());
    };
    let ast::DeclKind::Variables(list) = &parsed.ast.decl(id).kind else {
        return Err(invalid());
    };
    let Some(v) = list.variables.first() else {
        return Err(invalid());
    };
    let Some(initial) = v.initializer else {
        return Err(invalid());
    };
    Ok((parsed, source, initial))
}

struct Converter<'a> {
    ast: &'a ast::Ast,
    source: &'a str,
    interner: &'a Interner,
    scope: &'a Scope<'a>,
    /// Event handler: `$event` and assignment are allowed.
    action: bool,
}

fn unsupported(shape: &'static str) -> Refusal {
    refusal(Reason::Expression, shape)
}

impl Converter<'_> {
    /// An instance field or getter that the component class inherits (the
    /// analyzer's `lookUpGetter` walks up the hierarchy): the type and the
    /// scope come from whoever declares it, and the immutability is that of the
    /// field there. Without a written type, or with a type that mentions a type
    /// parameter or a private of another library: `None`, and the name is
    /// refused.
    fn inherited_member(&self, name: &str) -> Option<Converted> {
        if name.starts_with('_') {
            return None;
        }
        let (r, file) = self.scope.types?;
        let class = self.scope.class?.rsplit('.').next()?;
        let (ty, scope) =
            r.member_type_with_free(file, &self.receptor(class), name, self.scope.generics)?;
        let immutable = r.final_member(file, class, name)?;
        Some(Converted {
            immutable,
            ty: Some(ty),
            scope: Some(scope),
            ..Converted::new(format!("_ctx.{name}"), "inherited member")
        })
    }

    /// An instance setter (`set name(..)`) of the class or above it: a write
    /// target that is neither a field nor a getter ([`Resolution::has_setter`]).
    fn setter(&self, name: &str) -> bool {
        if name.starts_with('_') {
            return false;
        }
        let (Some((r, file)), Some(class)) = (self.scope.types, self.scope.class) else {
            return false;
        };
        r.has_setter(file, class.rsplit('.').next().unwrap_or(class), name)
    }

    /// An inherited instance method ([`Resolution::method`]), looked up only
    /// when the name is neither the class's own nor private.
    fn inherited_method(&self, name: &str) -> Option<crate::resolucao::Method> {
        if name.starts_with('_') {
            return None;
        }
        let (r, file) = self.scope.types?;
        let class = self.scope.class?.rsplit('.').next()?;
        r.free_method(file, &self.receptor(class), name, self.scope.generics)
    }

    /// The implicit receiver as a type: the class with its own parameters
    /// (`Class<T>`), which the hierarchy lookup substitutes as free.
    fn receptor(&self, class: &str) -> String {
        if self.scope.generics.is_empty() {
            class.to_string()
        } else {
            format!("{class}<{}>", self.scope.generics.join(", "))
        }
    }

    /// O handler inteiro: classifica (simples × complexo) e converte.
    fn action(&self, id: ast::ExprId) -> Result<Action, Refusal> {
        let id = self.without_parens(id);
        let e = self.ast.expr(id);
        // Tear-off (`(click)="m"`): `rewriteTearOff` replaces it with `m()` or
        // `m($event)`, according to the method's positional parameters.
        if let ast::ExprKind::Identifier(n) = &e.kind {
            let name = self.interner.resolve(n.sym);
            if !self.scope.locals.contains_key(name) && name != "$event" {
                if let Some(&positional) = self.scope.arities.get(name) {
                    let arity = u8::from(positional > 0);
                    return self.simple(name, arity);
                }
                if self.scope.methods.contains_key(name) {
                    return Err(unsupported("tear-off of a method without a known arity"));
                }
                if !self.scope.members.contains_key(name)
                    && let Some(m) = self.inherited_method(name)
                {
                    return self.simple(name, u8::from(m.positional > 0));
                }
            }
        }
        // `handlerTypeFromExpression`: `m()` or `m($event)`, implicit receiver,
        // no named argument.
        if let ast::ExprKind::Call { target, arguments } = &e.kind
            && let ast::ExprKind::Identifier(n) = &self.ast.expr(*target).kind
            && arguments.type_args.is_empty()
            && arguments.args.iter().all(|a| a.name.is_none())
        {
            let name = self.interner.resolve(n.sym);
            let arity = match &arguments.args[..] {
                [] => Some(0),
                [a] if self.e_event(a.value) => Some(1),
                _ => None,
            };
            if let Some(arity) = arity {
                // A local with the method's name: the official compiler calls
                // the local's function (`InvokeFunctionExpr`) and refuses the
                // tear-off.
                if self.scope.locals.contains_key(name) {
                    return Err(unsupported("handler that calls a local"));
                }
                return self.simple(name, arity);
            }
        }
        let c = match &e.kind {
            // An assignment at the top of the statement comes out without
            // parentheses (`lineWasEmpty` in `visitWritePropExpr`).
            ast::ExprKind::Assign { .. } => self.assignment(id)?,
            _ => self.expr(id, true)?,
        };
        Ok(Action::Complexa(c))
    }

    /// A simple handler: the method must belong to the component (or be a
    /// field, which the tear-off reads the same way).
    fn simple(&self, name: &str, arity: u8) -> Result<Action, Refusal> {
        if !self.scope.methods.contains_key(name)
            && !self.scope.members.contains_key(name)
            && self.inherited_method(name).is_none()
            && self.inherited_member(name).is_none()
        {
            return Err(unsupported("handler that is not a member of the component"));
        }
        if self.scope.members.get(name).is_some_and(|m| m.is_static) {
            return Err(unsupported("handler on a static member"));
        }
        let arg = if arity == 1 { "$event" } else { "" };
        Ok(Action::Simple {
            method: name.to_string(),
            arity,
            statement: format!("_ctx.{name}({arg})"),
        })
    }

    /// `$pipe.name(input, args)`: the official parser's `BindingPipe`
    /// (`_createPipeOrThrow`). The call comes out as `MARK(input, args)`, and
    /// whoever knows the view replaces the [`PIPE_MARK`] with the
    /// `pureProxyN` of that point (`CompilePipe._call`). Type `dynamic`
    /// (`visitPipe` of `_TypeResolver`), mutable and possibly null.
    fn pipe(&self, name: &str, arguments: &ast::Arguments) -> Result<Converted, Refusal> {
        if self.action {
            return Err(refusal(Reason::UsedPipes, "pipe in an event"));
        }
        if arguments.args.iter().any(|a| a.name.is_some()) {
            return Err(refusal(Reason::UsedPipes, "pipe with a named argument"));
        }
        if arguments.args.is_empty() {
            return Err(refusal(Reason::UsedPipes, "pipe without an argument"));
        }
        // `visitPipe` converts the input and then the arguments: that is the
        // order in which the locals are requested.
        let mut args = Vec::new();
        let mut locals = Vec::new();
        // The inner pipe gets its proxy first (it is converted earlier), but
        // the outer mark comes first in the text: whoever replaces the marks
        // sorts them by the end of the call.
        for a in arguments.args.iter() {
            let v = self.expr(a.value, true)?;
            locals.extend(v.locals);
            args.push(v.text);
        }
        Ok(Converted {
            ty: Some("dynamic".into()),
            locals: merge_unique(&[&locals]),
            ..Converted::new(
                format!(
                    "{PIPE_MARK}{name}/{}{PIPE_END}({})",
                    args.len(),
                    args.join(", ")
                ),
                "pipe",
            )
        })
    }

    fn e_event(&self, id: ast::ExprId) -> bool {
        matches!(&self.ast.expr(self.without_parens(id)).kind,
            ast::ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "$event")
    }

    fn without_parens(&self, mut id: ast::ExprId) -> ast::ExprId {
        while let ast::ExprKind::Parenthesized(i) = &self.ast.expr(id).kind {
            id = *i;
        }
        id
    }

    /// `x = y`, `a.b = y`, `a[i] = y` (`PropertyWrite`/`KeyedWrite`), without
    /// the parentheses — whoever is inside another expression adds them.
    fn assignment(&self, id: ast::ExprId) -> Result<Converted, Refusal> {
        let ast::ExprKind::Assign { op, target, value } = &self.ast.expr(id).kind else {
            return Err(unsupported("assignment"));
        };
        if !self.action {
            return Err(unsupported("assignment outside an event"));
        }
        // The official compiler ignores the operator of a compound assignment
        // and writes `x = y`: a shape deliberately not reproduced.
        if *op != ast::AssignOp::Assign {
            return Err(unsupported("compound assignment (`+=`)"));
        }
        let (target, target_locals) = match &self.ast.expr(*target).kind {
            ast::ExprKind::Identifier(n) => {
                let name = self.interner.resolve(n.sym);
                if self.scope.locals.contains_key(name) || name == "$event" {
                    return Err(unsupported("assignment to a local"));
                }
                match self.scope.members.get(name) {
                    // Inherited field: the write also walks up the hierarchy
                    // (`PropertyWrite` with the implicit receiver becomes
                    // `_ctx.x`).
                    None if self.inherited_member(name).is_some() || self.setter(name) => {
                        (format!("_ctx.{name}"), Vec::new())
                    }
                    None => return Err(unsupported("assignment to a name outside the component")),
                    Some(m) if m.is_static => match self.scope.class {
                        Some(class) => (format!("{class}.{name}"), Vec::new()),
                        None => {
                            return Err(unsupported("static member without the qualified class"));
                        }
                    },
                    Some(_) => (format!("_ctx.{name}"), Vec::new()),
                }
            }
            ast::ExprKind::Property {
                target: t,
                name,
                null_aware: false,
            } => {
                let r = self.expr(*t, true)?;
                (
                    format!("{}.{}", r.text, self.interner.resolve(name.sym)),
                    r.locals,
                )
            }
            ast::ExprKind::Index { .. } => {
                return Err(unsupported("assignment to an index `a[i] = x`"));
            }
            _ => return Err(unsupported("assignment to an unknown target")),
        };
        let v = self.expr(*value, true)?;
        Ok(Converted {
            locals: merge_unique(&[&target_locals, &v.locals]),
            ..Converted::new(format!("{target} = {}", v.text), "assignment")
        })
    }

    /// `root` says whether this node is the implicit receiver — only then does
    /// an identifier become `_ctx.name`; in `a.b`, `b` is a member of `a`.
    fn expr(&self, id: ast::ExprId, root: bool) -> Result<Converted, Refusal> {
        let v = self.node_expr(id, root)?;
        // A call that breaks ([`BREAK`]) only as the whole expression or as an
        // argument of another call: inside an operator, an access or a
        // conditional the formatter indents it differently, no case.
        if v.text.contains(BREAK) && !matches!(self.ast.expr(id).kind, ast::ExprKind::Call { .. }) {
            return Err(unsupported(
                "call with a named argument inside an expression",
            ));
        }
        Ok(v)
    }

    fn node_expr(&self, id: ast::ExprId, root: bool) -> Result<Converted, Refusal> {
        let e = self.ast.expr(id);
        match &e.kind {
            // `LiteralPrimitive`: immutable and never null (`canBeNull`). The
            // type is `String` for a string and `dynamic` for the rest
            // (`visitLiteralPrimitive` of `_TypeResolver`). The text is what
            // `o.literal(value)` writes, not what the template wrote.
            ast::ExprKind::Int(_) => {
                let t = self.text(id);
                let Ok(v) = t.parse::<u64>() else {
                    return Err(unsupported("integer literal outside the decimal form"));
                };
                Ok(Converted {
                    immutable: true,
                    can_be_null: false,
                    ty: Some("dynamic".into()),
                    literal: true,
                    ..Converted::new(v.to_string(), "literal")
                })
            }
            ast::ExprKind::Double(_) => {
                let t = self.text(id);
                // Dart's `double.toString()` and Rust's `{:?}` agree on the
                // short form (`1.5`, `2.0`); on exponents, they do not.
                match t.parse::<f64>() {
                    Ok(v) if format!("{v:?}") == t && !t.contains(['e', 'E']) => Ok(Converted {
                        immutable: true,
                        can_be_null: false,
                        ty: Some("dynamic".into()),
                        literal: true,
                        ..Converted::new(t, "literal")
                    }),
                    _ => Err(unsupported("double literal outside the canonical form")),
                }
            }
            ast::ExprKind::Bool(b) => Ok(Converted {
                immutable: true,
                can_be_null: false,
                ty: Some("dynamic".into()),
                literal: true,
                ..Converted::new(b.to_string(), "literal")
            }),
            ast::ExprKind::Null => Ok(Converted {
                immutable: true,
                can_be_null: false,
                ty: Some("dynamic".into()),
                literal: true,
                ..Converted::new("null".into(), "literal nulo")
            }),
            ast::ExprKind::String(lit) => {
                // Only a literal without interpolation: `'a$b'` inside the
                // template is something else and does not appear in the owner's
                // projects.
                let Some(value) = lit.constant_value() else {
                    return Err(unsupported("string with Dart interpolation"));
                };
                Ok(Converted {
                    immutable: true,
                    can_be_null: false,
                    ty: Some("String".into()),
                    literal: true,
                    ..Converted::new(crate::view::literal(&value.to_string_lossy()), "literal")
                })
            }
            ast::ExprKind::Identifier(n) => {
                let name = self.interner.resolve(n.sym);
                if !root {
                    return Ok(Converted::new(name.to_string(), "name"));
                }
                // `getLocal('$event')` returns the handler's variable.
                if name == "$event" {
                    if !self.action {
                        return Err(unsupported("`$event` outside an event"));
                    }
                    return Ok(Converted {
                        ty: Some("dynamic".into()),
                        ..Converted::new("$event".into(), "`$event`")
                    });
                }
                // An `exports:` name (`_matchExport`, at parse time: before the
                // locals and the members, `analyzer_parser.dart:435-455`):
                // through the import of the library that declares it, type
                // `dynamic`. A `StaticRead` is always immutable
                // (`analyzed_class.dart:116`), even of a mutable variable or a
                // getter.
                if let Some((q, what)) = self.scope.exports.and_then(|e| e.get(name)) {
                    use crate::resolucao::Exported;
                    let text = format!("{q}{name}");
                    return match what {
                        Exported::Class => Ok(Converted {
                            ty: Some("dynamic".into()),
                            is_static: Some(name.to_string()),
                            ..Converted::new(text, "class from exports:")
                        }),
                        // Without an `AnalyzedClass` (enum, mixin): `X.y` is a
                        // plain `PropertyRead`, mutable and `dynamic`.
                        Exported::TypeWithoutClass => Ok(Converted {
                            ty: Some("dynamic".into()),
                            is_static: Some(format!("{WITHOUT_CLASS}{name}")),
                            ..Converted::new(text, "enum or mixin from exports:")
                        }),
                        Exported::Variable { .. } => Ok(Converted {
                            immutable: true,
                            ty: Some("dynamic".into()),
                            ..Converted::new(text, "variable from exports:")
                        }),
                        Exported::Getter => Ok(Converted {
                            immutable: true,
                            ty: Some("dynamic".into()),
                            ..Converted::new(text, "getter from exports:")
                        }),
                        Exported::Function => Err(unsupported("function from exports: as a value")),
                    };
                }
                // The loop local shadows the component member (but not the
                // export, resolved earlier, at parse time). `isImmutable` does
                // not know about locals: the component field with the same name
                // counts (`lookUpGetter`), or the method ("methods are
                // immutable").
                if let Some(l) = self.scope.locals.get(name) {
                    let immutable = match self.scope.members.get(name) {
                        Some(m) => m.immutable,
                        None => {
                            self.scope.methods.contains_key(name)
                                || self.inherited_member(name).is_some_and(|c| c.immutable)
                                || self.inherited_method(name).is_some()
                        }
                    };
                    return Ok(Converted {
                        immutable,
                        ty: Some(l.ty.clone()),
                        scope: l.scope.clone(),
                        locals: vec![name.to_string()],
                        ..Converted::new(l.dart.clone(), "local")
                    });
                }
                // A method read as a value (the `trackBy: rastrear` of
                // `*ngFor`, a callback passed to a child): `isImmutable` says
                // "methods are immutable"; `canBeNull` does not exempt it; and
                // the `_TypeResolver` looks for a *getter* with the name —
                // there is none, so it is `dynamic`.
                if !self.scope.members.contains_key(name) && self.scope.methods.contains_key(name) {
                    return Ok(Converted {
                        immutable: true,
                        ty: Some("dynamic".into()),
                        ..Converted::new(format!("_ctx.{name}"), "method as a value")
                    });
                }
                let Some(m) = self.scope.members.get(name) else {
                    return self
                        .inherited_member(name)
                        .ok_or_else(|| unsupported("name outside the component"));
                };
                // Static: the class's `importExpr` and the name; `isImmutable`
                // by the field (`final`/`const`), type `dynamic` and, outside a
                // literal, possibly null (case j25).
                if m.is_static {
                    let Some(class) = self.scope.class else {
                        return Err(unsupported("static member without the qualified class"));
                    };
                    return Ok(Converted {
                        immutable: m.immutable,
                        ty: Some("dynamic".into()),
                        ..Converted::new(format!("{class}.{name}"), "static member")
                    });
                }
                // `isImmutable` of a `PropertyRead` with the implicit receiver:
                // a `final`/`const` field (a getter already arrives here
                // mutable). Without a written type, what the semantic database
                // infers from the declaration (`Enum.values`,
                // `Class<A>.named()`…), with its scope.
                let (ty, scope) = if m.ty.is_empty() {
                    match self.inherited_member(name) {
                        Some(c) => (c.ty, c.scope),
                        None => (None, None),
                    }
                } else {
                    (Some(m.ty.clone()), None)
                };
                Ok(Converted {
                    immutable: m.immutable,
                    ty,
                    scope,
                    ..Converted::new(format!("_ctx.{name}"), "member")
                })
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware,
            } => {
                let target = self.expr(*target, root)?;
                let name = self.interner.resolve(name.sym);
                let ponto = if *null_aware { "?." } else { "." };
                // `Class.name` of an `exports:` class: a `const`/`final` field
                // (and an enum value) is immutable, a getter is not, and a
                // method is ("methods are immutable",
                // `analyzed_class.dart:140-150`). Type `dynamic` (case j51).
                if target
                    .is_static
                    .as_deref()
                    .is_some_and(|c| c.starts_with(WITHOUT_CLASS))
                {
                    return Ok(Converted {
                        ty: Some("dynamic".into()),
                        locals: target.locals,
                        ..Converted::new(
                            format!("{}{ponto}{name}", target.text),
                            "member of an enum or mixin",
                        )
                    });
                }
                if let Some(class) = &target.is_static {
                    use crate::resolucao::Static;
                    let Some((r, file)) = self.scope.types else {
                        return Err(unsupported(
                            "static member from exports: without the semantic database",
                        ));
                    };
                    let immutable = match r.static_member(file, class, name) {
                        Some(Static::Field { immutable }) => immutable,
                        Some(Static::Getter) => false,
                        Some(Static::Method) => true,
                        None => return Err(unsupported("unknown static member from exports:")),
                    };
                    return Ok(Converted {
                        immutable,
                        ty: Some("dynamic".into()),
                        locals: target.locals,
                        ..Converted::new(format!("{}{ponto}{name}", target.text), "static member")
                    });
                }
                // The type at the end of the chain is in another class: the
                // semantic database answers, as the analyzer answers the
                // official compiler. A `dynamic` receiver (the `#ref`,
                // `$event`): `_lookupGetterReturnType` only looks at
                // `InterfaceType`, and the rest gives `dynamic`.
                let found = match (&target.ty, self.scope.types) {
                    (Some(t), Some((r, file))) if t != "dynamic" => r.member_type_with_free(
                        target.scope.as_deref().unwrap_or(file),
                        t,
                        name,
                        self.scope.generics,
                    ),
                    _ => None,
                };
                // A receiver of a type that does not exist in scope
                // (`InvalidType`: the class of a file the ngdart phase does not
                // see yet) also gives `dynamic`.
                let nonexistent = match (&target.ty, self.scope.types) {
                    (Some(t), Some((r, file))) => {
                        r.nonexistent_type(target.scope.as_deref().unwrap_or(file), t)
                    }
                    _ => false,
                };
                let (ty, scope) = match found {
                    Some((t, e)) => (Some(t), Some(e)),
                    None if target.ty.as_deref() == Some("dynamic") || nonexistent => {
                        (Some("dynamic".to_string()), None)
                    }
                    None => (None, None),
                };
                // The receiver is not implicit: `a.b` is always mutable.
                Ok(Converted {
                    ty,
                    scope,
                    locals: target.locals,
                    ..Converted::new(
                        format!("{}{ponto}{name}", target.text),
                        if *null_aware { "`?.`" } else { "propriedade" },
                    )
                })
            }
            ast::ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() {
                    return Err(unsupported("call with a type argument"));
                }
                if let ast::ExprKind::Property {
                    target: t,
                    name,
                    null_aware: false,
                } = &self.ast.expr(*target).kind
                    && root
                    && matches!(&self.ast.expr(*t).kind,
                        ast::ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "$pipe")
                {
                    return self.pipe(self.interner.resolve(name.sym), arguments);
                }
                // `visitMethodCall` converts the arguments **before** the
                // receiver: that is the order in which the locals are
                // requested.
                let mut args = Vec::new();
                let mut args_locals = Vec::new();
                for a in arguments.args.iter() {
                    let v = self.expr(a.value, true)?;
                    args_locals.extend(v.locals);
                    match &a.name {
                        Some(n) => {
                            args.push(format!("{}: {}", self.interner.resolve(n.sym), v.text))
                        }
                        None => args.push(v.text),
                    }
                }
                // With a named argument the list ends in a comma and the
                // formatter puts one argument per line, two spaces in (and
                // whatever already broke inside them, along with it). In a list
                // without a named argument, an argument that breaks would get
                // the continuation indentation — a shape without a case in the
                // corpus.
                let named = arguments.args.iter().any(|a| a.name.is_some());
                if !named && args.iter().any(|a| a.contains(BREAK)) {
                    return Err(unsupported("call with a named argument inside an argument"));
                }
                let list = if named {
                    let mut l: String = args
                        .iter()
                        .map(|a| format!("{BREAK}{INDENT}{INDENT}{},", indent_breaks(a, 2)))
                        .collect();
                    l.push(BREAK);
                    l
                } else {
                    args.join(", ")
                };
                // The target of a call may be a class method, which is not
                // valid as a bare value. The type is that of `_TypeResolver`:
                // the method's return (`visitMethodCall`); a field or local
                // called as a function is not a method, and gives `dynamic`.
                let mut return_scope = None;
                let (target, return_type, target_locals) = match &self.ast.expr(*target).kind {
                    ast::ExprKind::Identifier(n)
                        if root
                            && !self.scope.locals.contains_key(self.interner.resolve(n.sym)) =>
                    {
                        let name = self.interner.resolve(n.sym);
                        let exported = self
                            .scope
                            .exports
                            .and_then(|e| e.get(name))
                            .filter(|(_, o)| *o == crate::resolucao::Exported::Function);
                        match self.scope.methods.get(name) {
                            // An `exports:` function: through the import,
                            // `dynamic`.
                            _ if exported.is_some() => (
                                format!(
                                    "{}{name}",
                                    exported.map(|(q, _)| q.as_str()).unwrap_or_default()
                                ),
                                Some("dynamic".to_string()),
                                Vec::new(),
                            ),
                            Some(t) => (format!("_ctx.{name}"), Some(t.clone()), Vec::new()),
                            // Inherited method: the return type in the scope of
                            // whoever declares it; without a written type,
                            // unknown.
                            None if !self.scope.members.contains_key(name)
                                && let Some(m) = self.inherited_method(name) =>
                            {
                                let (t, e) = m.return_type.unzip();
                                return_scope = e;
                                (format!("_ctx.{name}"), t, Vec::new())
                            }
                            None => {
                                let t = self.expr(*target, root)?;
                                (t.text, Some("dynamic".to_string()), t.locals)
                            }
                        }
                    }
                    // A local called as a function: the type is still looked up
                    // as a class method (`visitMethodCall` looks at the name).
                    ast::ExprKind::Identifier(n) => {
                        let t = self.expr(*target, root)?;
                        let ty = self
                            .scope
                            .methods
                            .get(self.interner.resolve(n.sym))
                            .cloned()
                            .unwrap_or_else(|| "dynamic".to_string());
                        (t.text, Some(ty), t.locals)
                    }
                    // `a.m()`: the semantic database answers the member `m` of
                    // the type of `a` — the return type, when it is a method.
                    _ => {
                        let t = self.expr(*target, root)?;
                        (t.text, t.ty, t.locals)
                    }
                };
                Ok(Converted {
                    ty: return_type,
                    scope: return_scope,
                    locals: merge_unique(&[&args_locals, &target_locals]),
                    ..Converted::new(format!("{target}({list})"), "call")
                })
            }
            ast::ExprKind::Unary { op, operand } => {
                // `!`, `-x` and `x!`. ngcompiler's expression parser reads `-x`
                // as `0 - x` (output `(0 - _ctx.x)`) and `x!` as `(x!)`; `~`
                // does not even exist there.
                // `x!` is `PostfixNotNull`: `.notNull()`, which the emitter
                // writes as `(x!)`; type `dynamic`, mutable.
                if *op == ast::UnaryOp::NullAssert {
                    let v = self.expr(*operand, root)?;
                    return Ok(Converted {
                        ty: Some("dynamic".into()),
                        locals: v.locals,
                        ..Converted::new(format!("({}!)", v.text), "`x!`")
                    });
                }
                // `-x` is `Binary('-', 0, x)` (`analyzer_parser.dart:607-609`):
                // output `(0 - x)`, immutable if `x` is, "may be null" and
                // `dynamic`, like any binary.
                if *op == ast::UnaryOp::Neg {
                    let v = self.expr(*operand, root)?;
                    return Ok(Converted {
                        immutable: v.immutable,
                        ty: Some("dynamic".into()),
                        locals: v.locals,
                        ..Converted::new(format!("(0 - {})", v.text), "`-x`")
                    });
                }
                if *op != ast::UnaryOp::Not {
                    return Err(unsupported("unary operator outside the template"));
                }
                let v = self.expr(*operand, root)?;
                let op = self.unary_operator(id);
                // `PrefixNot` is not part of `isImmutable`: it is mutable.
                Ok(Converted {
                    ty: Some("dynamic".into()),
                    locals: v.locals,
                    ..Converted::new(format!("({op}{})", v.text), "`!`")
                })
            }
            ast::ExprKind::Binary { op, .. } if !template_operator(*op) => {
                // `a | b` in the template is a pipe, not a bitwise OR:
                // translating it as Dart would give `(_ctx.a | _ctx.b)`, which
                // compiles and does something else. The others (`&`, `^`, `<<`,
                // `~/`…) do not exist in ngdart's expression language.
                Err(if *op == ast::BinaryOp::BitOr {
                    refusal(Reason::UsedPipes, "pipe `x | name`")
                } else {
                    unsupported("binary operator outside the template")
                })
            }
            ast::ExprKind::Binary { op, left, right } => {
                let a = self.expr(*left, root)?;
                let b = self.expr(*right, root)?;
                let op_text = self.binary_operator(id);
                let if_null = *op == ast::BinaryOp::IfNull;
                // `canBeNull(IfNull)`: not null if the left one is not;
                // otherwise, the right one decides. A plain binary is always
                // "may be null" for the official compiler.
                let can_be_null = if if_null {
                    a.can_be_null && b.can_be_null
                } else {
                    true
                };
                // `visitBinary`: `String + String` is `String`; the rest,
                // `dynamic`. `??` is `dynamic`.
                let ty = if *op == ast::BinaryOp::Add
                    && a.ty.as_deref() == Some("String")
                    && b.ty.as_deref() == Some("String")
                {
                    "String"
                } else {
                    "dynamic"
                };
                Ok(Converted {
                    // `isImmutable` of `Binary` and `IfNull`: both sides.
                    immutable: a.immutable && b.immutable,
                    can_be_null,
                    ty: Some(ty.into()),
                    locals: merge_unique(&[&a.locals, &b.locals]),
                    ..Converted::new(
                        format!("({} {op_text} {})", a.text, b.text),
                        if if_null { "`??`" } else { "binary" },
                    )
                })
            }
            ast::ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let c = self.expr(*condition, root)?;
                let t = self.expr(*then, root)?;
                let f = self.expr(*else_, root)?;
                Ok(Converted {
                    ty: Some("dynamic".into()),
                    locals: merge_unique(&[&c.locals, &t.locals, &f.locals]),
                    ..Converted::new(format!("({} ? {} : {})", c.text, t.text, f.text), "ternary")
                })
            }
            ast::ExprKind::Parenthesized(inner) => self.expr(*inner, root),
            // Inside another expression the assignment gets parentheses.
            ast::ExprKind::Assign { .. } => {
                let a = self.assignment(id)?;
                Ok(Converted {
                    text: format!("({})", a.text),
                    ..a
                })
            }
            // `KeyedRead`: `receiver.key(key)`, without parentheses; `dynamic`.
            ast::ExprKind::Index {
                target,
                index,
                null_aware: false,
            } => {
                let r = self.expr(*target, root)?;
                let k = self.expr(*index, true)?;
                Ok(Converted {
                    ty: Some("dynamic".into()),
                    locals: merge_unique(&[&r.locals, &k.locals]),
                    ..Converted::new(format!("{}[{}]", r.text, k.text), "index")
                })
            }
            ast::ExprKind::Index { .. } => Err(unsupported("index `a?[i]`")),
            ast::ExprKind::List { .. } | ast::ExprKind::SetOrMap { .. } => {
                Err(unsupported("collection literal"))
            }
            ast::ExprKind::FunctionExpression(_) => Err(unsupported("anonymous function")),
            ast::ExprKind::Is { .. } | ast::ExprKind::As { .. } => Err(unsupported("`is`/`as`")),
            _ => Err(unsupported("expression shape outside the template")),
        }
    }

    fn text(&self, id: ast::ExprId) -> String {
        let s = self.ast.expr(id).span;
        self.source.get(s.start..s.end).unwrap_or("").to_string()
    }

    /// The operator as written in the source (the first character of the span).
    fn unary_operator(&self, id: ast::ExprId) -> String {
        let t = self.text(id);
        match t.chars().next() {
            Some('!') => "!".into(),
            Some('-') => "-".into(),
            Some('~') => "~".into(),
            _ => String::new(),
        }
    }

    /// The binary operator as written, extracted from between the two operands.
    fn binary_operator(&self, id: ast::ExprId) -> String {
        let ast::ExprKind::Binary { left, right, .. } = &self.ast.expr(id).kind else {
            return String::new();
        };
        let left_end = self.ast.expr(*left).span.end;
        let begin_right = self.ast.expr(*right).span.start;
        self.source
            .get(left_end..begin_right)
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

/// Binary operators of ngdart's expression language
/// (`expression_parser/parser.dart`): arithmetic, comparison, logical and `??`.
/// What Dart has in addition does not pass.
fn template_operator(op: ast::BinaryOp) -> bool {
    use ast::BinaryOp as B;
    matches!(
        op,
        B::Add
            | B::Sub
            | B::Mul
            | B::Div
            | B::Rem
            | B::Eq
            | B::NotEq
            | B::Lt
            | B::Gt
            | B::LtEq
            | B::GtEq
            | B::And
            | B::Or
            | B::IfNull
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn members() -> HashMap<String, Member> {
        HashMap::from([
            (
                "item".to_string(),
                Member {
                    ty: "Item".into(),
                    immutable: false,
                    is_static: false,
                },
            ),
            (
                "nome".to_string(),
                Member {
                    ty: "String".into(),
                    immutable: false,
                    is_static: false,
                },
            ),
            (
                "fixo".to_string(),
                Member {
                    ty: "int".into(),
                    immutable: true,
                    is_static: false,
                },
            ),
            (
                "titulo".to_string(),
                Member {
                    ty: "String".into(),
                    immutable: true,
                    is_static: false,
                },
            ),
        ])
    }

    fn conv(e: &str) -> Converted {
        converter(e, &members(), &mut Interner::new()).expect("converte")
    }

    /// The shapes that corpus case c05 showed coming out of the official
    /// compiler.
    #[test]
    fn official_shapes() {
        assert_eq!(conv("item.nome").text, "_ctx.item.nome");
        assert_eq!(conv("!item.ativo").text, "(!_ctx.item.ativo)");
        assert_eq!(conv("'fixo'").text, "'fixo'");
        assert_eq!(conv("\"fixo\"").text, "'fixo'");
        assert!(conv("'fixo'").immutable);
    }

    #[test]
    fn direct_member_type() {
        assert_eq!(conv("nome").ty.as_deref(), Some("String"));
        assert!(!conv("nome").immutable);
        assert!(conv("fixo").immutable);
    }

    /// The official `isImmutable`: `a.b`, `!x` and a ternary are mutable even
    /// with everything `final`; `a ?? b` and a binary of immutables are not.
    #[test]
    fn immutability_as_in_official() {
        assert!(!conv("titulo.length").immutable);
        assert!(!conv("!fixo").immutable);
        assert!(!conv("fixo > 1 ? 1 : 2").immutable);
        assert!(conv("fixo ?? 1").immutable);
        assert!(conv("fixo + 1").immutable);
    }

    /// `canBeNull`: only a literal escapes, and `a ?? b` with a non-null side.
    #[test]
    fn nullability_as_in_official() {
        assert!(!conv("'x'").can_be_null);
        assert!(!conv("1").can_be_null);
        assert!(conv("fixo").can_be_null);
        assert!(conv("fixo + 1").can_be_null);
        assert!(!conv("fixo ?? 1").can_be_null);
    }

    #[test]
    fn binary_and_conditional_with_parens() {
        assert_eq!(conv("nome == 'x'").text, "(_ctx.nome == 'x')");
        assert_eq!(
            conv("fixo > 1 ? nome : 'y'").text,
            "((_ctx.fixo > 1) ? _ctx.nome : 'y')"
        );
    }

    /// `|` in the template is a pipe: refused at any depth, and `||` is still
    /// the logical OR.
    #[test]
    fn pipe_is_refused_and_logical_or_passes() {
        let m = &members();
        let mut i = Interner::new();
        assert_eq!(
            converter("nome | fixo", m, &mut i).map_err(|r| r.reason),
            Err(Reason::UsedPipes)
        );
        assert_eq!(
            converter("(nome | fixo) == 'x'", m, &mut i).map_err(|r| r.reason),
            Err(Reason::UsedPipes)
        );
        assert_eq!(
            conv("fixo > 1 || fixo < 0").text,
            "((_ctx.fixo > 1) || (_ctx.fixo < 0))"
        );
    }

    /// `$pipe.name(input, args)` becomes the mark with the name and the number
    /// of arguments, type `dynamic`, also inside another pipe; with a named
    /// argument, it refuses.
    #[test]
    fn pipe_becomes_mark() {
        let c = conv("$pipe.date(fixo, 'dd/MM')");
        assert_eq!(c.text, "\u{3}date/2\u{4}(_ctx.fixo, 'dd/MM')");
        assert_eq!(c.ty.as_deref(), Some("dynamic"));
        assert!(!c.immutable);
        assert_eq!(
            conv("$pipe.a($pipe.b(fixo))").text,
            "\u{3}a/1\u{4}(\u{3}b/1\u{4}(_ctx.fixo))"
        );
        let m = &members();
        let mut i = Interner::new();
        assert_eq!(
            converter("$pipe.a(fixo, x: 1)", m, &mut i).map_err(|r| r.reason),
            Err(Reason::UsedPipes)
        );
    }

    /// What ngdart's parser reads differently from Dart: `-x` is `0 - x` there,
    /// `&` does not exist. `x!` comes out as `(x!)` and the index without
    /// parentheses.
    #[test]
    fn operators_outside_template_are_refused() {
        let m = &members();
        let mut i = Interner::new();
        assert_eq!(conv("-fixo").text, "(0 - _ctx.fixo)");
        assert!(converter("fixo & 1", m, &mut i).is_err());
        assert_eq!(conv("nome!").text, "(_ctx.nome!)");
        assert_eq!(conv("item[fixo]").text, "_ctx.item[_ctx.fixo]");
    }

    /// A name that does not belong to the component does not become `_ctx.name`
    /// by mistake.
    #[test]
    fn unknown_name_is_refused() {
        assert!(converter("outro", &members(), &mut Interner::new()).is_err());
    }

    fn action(e: &str) -> Action {
        let m = members();
        let methods = HashMap::from([
            ("salvar".to_string(), "void".to_string()),
            ("ir".to_string(), "void".to_string()),
        ]);
        let arities = HashMap::from([("salvar".to_string(), 0), ("ir".to_string(), 1)]);
        let locals = HashMap::from([(
            "x".to_string(),
            Local {
                dart: "local_x".into(),
                ty: "int".into(),
                scope: None,
            },
        )]);
        let scope = Scope {
            members: &m,
            methods: &methods,
            arities: &arities,
            locals: &locals,
            types: None,
            class: None,
            exports: None,
            generics: &[],
        };
        convert_action(e, &scope, &mut Interner::new()).expect("converte")
    }

    /// `handlerTypeFromExpression` e `rewriteTearOff`.
    #[test]
    fn simple_handlers_and_tear_off() {
        assert!(matches!(
            action("salvar()"),
            Action::Simple { arity: 0, .. }
        ));
        assert!(matches!(
            action("ir($event)"),
            Action::Simple { arity: 1, .. }
        ));
        assert!(matches!(action("salvar"), Action::Simple { arity: 0, .. }));
        assert!(matches!(action("ir"), Action::Simple { arity: 1, .. }));
    }

    /// Complex ones: top-level assignment without parentheses, arguments,
    /// locals in the order the official compiler asks for them (arguments
    /// before the receiver).
    #[test]
    fn complex_handlers() {
        let Action::Complexa(c) = action("nome = $event") else {
            panic!()
        };
        assert_eq!(c.text, "_ctx.nome = $event");
        let Action::Complexa(c) = action("ir(x)") else {
            panic!()
        };
        assert_eq!(c.text, "_ctx.ir(local_x)");
        assert_eq!(c.locals, vec!["x".to_string()]);
        let Action::Complexa(c) = action("$event.stopPropagation()") else {
            panic!()
        };
        assert_eq!(c.text, "$event.stopPropagation()");
    }
}
