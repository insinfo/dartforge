//! The thirty-first batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `always_specify_types`: the `for-in` variable and the pattern variable
//!   without a type (by the keyword, or by the name), the collection literal
//!   without type arguments, the generic named type without arguments
//!   (outside `is` and `@optionalTypeArgs`; also that of a creation without
//!   `new`, which here is a call), the simple parameter without a type (that
//!   of a closure too, by the inferred type) and the variable list without a
//!   type (by the initializers' types: a local read without the promotion).
//!   The codes and arguments are the emitter's (`add_type`,
//!   `replace_keyword`, `specify_type`, `split_to_types`).
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::Element;
use dartforge_frontend::ast::{
    self, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, ParameterKind,
    PatternKind, StmtKind, TypeKind,
};
use dartforge_frontend::token::Token;
use dartforge_intern::Interner;
use dartforge_types::display::Displayer;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId};
use std::collections::HashSet;

/// The token immediately before offset `pos` (the start of a token).
fn token_before(t: &[Token], pos: usize) -> Option<Span> {
    let i = t.partition_point(|x| x.span.start < pos);
    (i > 0).then(|| t[i - 1].span)
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if !enabled("always_specify_types") {
        return out;
    }
    let Ok(tokens) = dartforge_frontend::lexer::lex(u.source) else {
        return out;
    };
    let a = u.ast;
    let source = u.source;
    let program = s.program;
    let table = s.table;
    let body = s.body;
    let ex = Displayer {
        table,
        interner,
        program,
    };
    // The `DartType` argument (with alias) and the text of `getDisplayString()`.
    let type_arg = |t: TypeId| ex.ty(t, true);
    let text = |t: TypeId| ex.ty(t, false);
    let is_dynamic = |t: TypeId| matches!(table.get(t), Type::Dynamic);
    let mut findings: Vec<(Span, &'static LintCode, Vec<String>)> = Vec::new();
    let word = |sp: Span| &source[sp.start..sp.end];

    // `visitDeclaredIdentifier`: the variable of `for (var x in …)`.
    let mut declared: Vec<(bool, bool, ast::Name)> = Vec::new();
    for st in a.stmts.iter() {
        if let StmtKind::ForIn {
            target: ForInTarget::Declared { ty: None, name, .. },
            ..
        } = &st.kind
        {
            declared.push((false, false, *name));
        }
    }
    fn collect_elements(x: &CollectionElement, v: &mut Vec<(bool, bool, ast::Name)>) {
        match x {
            CollectionElement::ForIn {
                target: ForInTarget::Declared { ty: None, name, .. },
                body,
                ..
            } => {
                v.push((false, false, *name));
                collect_elements(body, v);
            }
            CollectionElement::ForIn { body, .. } | CollectionElement::For { body, .. } => {
                collect_elements(body, v)
            }
            CollectionElement::If { then, else_, .. } => {
                collect_elements(then, v);
                if let Some(e) = else_ {
                    collect_elements(e, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            for x in elements.iter() {
                collect_elements(x, &mut declared);
            }
        }
    }
    for (_, _, name) in declared {
        // The keyword: the token before the name (`var`, `final`, `const`).
        let Some(k) = token_before(&tokens, name.span.start) else {
            continue;
        };
        let p = word(k);
        if !matches!(p, "var" | "final" | "const") {
            continue;
        }
        let Some(t) = body.local_type(name.span.start) else {
            continue;
        };
        if p == "var" {
            findings.push((
                k,
                &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
                vec![p.to_string(), type_arg(t)],
            ));
        } else {
            findings.push((k, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![type_arg(t)]));
        }
    }

    // `visitDeclaredVariablePattern` (outside assignment patterns).
    let assignments: Vec<Span> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::PatternAssign { pattern, .. } => Some(a.pattern(*pattern).span),
            _ => None,
        })
        .collect();
    for (k, p) in a.patterns.iter().enumerate() {
        let PatternKind::Variable {
            final_,
            var_,
            ty: None,
            name,
        } = &p.kind
        else {
            continue;
        };
        if assignments
            .iter()
            .any(|sp| sp.start <= p.span.start && p.span.end <= sp.end)
        {
            continue;
        }
        let Some(t) = body.matched_types.get(&ast::PatternId(k as u32)).copied() else {
            continue;
        };
        let keyword = if *final_ || *var_ {
            token_before(&tokens, name.span.start)
        } else {
            None
        };
        match keyword {
            Some(kw) if *var_ => findings.push((
                kw,
                &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
                vec![word(kw).to_string(), type_arg(t)],
            )),
            Some(kw) => {
                findings.push((kw, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![type_arg(t)]))
            }
            None => findings.push((
                name.span,
                &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
                vec![type_arg(t)],
            )),
        }
    }

    // `checkLiteral`: a collection literal without type arguments.
    for e in a.exprs.iter() {
        if let ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } = &e.kind
            && type_args.is_empty()
        {
            let i = tokens.partition_point(|x| x.span.start < e.span.start);
            if let Some(t) = tokens.get(i) {
                findings.push((t.span, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
            }
        }
    }

    // `visitNamedType`: a generic interface type without arguments.
    let optional_type_args = |metadata: &[ast::Annotation], unit| {
        metadata.iter().any(|m| {
            dartforge_types::annotations::is_getter_of(
                program,
                interner,
                unit,
                m,
                "meta",
                "optionalTypeArgs",
            )
        })
    };
    // The element's type parameters and `hasOptionalTypeArgs`.
    let generic = |el: Element| -> bool {
        match el {
            Element::Class(c) => {
                let x = program.class(c);
                !x.type_params.is_empty()
                    && !x.decl.is_some_and(|r| {
                        optional_type_args(&program.unit(r.unit).ast.decl(r.decl).metadata, r.unit)
                    })
            }
            Element::Typedef(t) => {
                let x = program.typedef(t);
                let r = x.decl;
                !x.type_params.is_empty()
                    && !optional_type_args(&program.unit(r.unit).ast.decl(r.decl).metadata, r.unit)
            }
            _ => false,
        }
    };
    let is_interface = |t: TypeId| {
        matches!(
            table.get(t),
            Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. }
        )
    };
    let in_is: HashSet<ast::TypeId> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::Is { ty, .. } => Some(*ty),
            _ => None,
        })
        .collect();
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        let TypeKind::Named { name, args } = &t.kind else {
            continue;
        };
        if !args.is_empty() || in_is.contains(&id) {
            continue;
        }
        let Some(r) = body
            .annotation_types
            .get(&id)
            .copied()
            .or_else(|| s.outline.written_types.get(&(s.unit, id)).copied())
        else {
            continue;
        };
        if !is_interface(r) {
            continue;
        }
        let b = match &name[..] {
            [n] => program.lookup_in_unit(s.unit, n.sym),
            [p, n] => program.lookup_prefixed_in_unit(s.unit, p.sym, n.sym),
            _ => None,
        };
        let Some(el) = b.and_then(|b| b.getter) else {
            continue;
        };
        if generic(el) {
            findings.push((t.span, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
        }
    }
    // A creation without `new` (`Foo()`, `Foo.name()`, `p.Foo()`): the
    // `ConstructorName`'s `NamedType`.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let ExprKind::Call { target, arguments } = &e.kind else {
            continue;
        };
        if !matches!(body.get_resolved(id), Some(Resolved::Constructor(_)))
            || !arguments.type_args.is_empty()
        {
            continue;
        }
        // The type name: the identifier (or `p.Foo`) before the constructor
        // name.
        let mut x = *target;
        if matches!(a.expr(x).kind, ExprKind::TypeArguments { .. }) {
            continue;
        }
        let type_node = loop {
            match &a.expr(x).kind {
                ExprKind::Identifier(_) => break Some(x),
                ExprKind::Property { target: t, .. } => {
                    if matches!(
                        body.get_resolved(x),
                        Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))
                    ) {
                        break Some(x);
                    }
                    if matches!(a.expr(*t).kind, ExprKind::TypeArguments { .. }) {
                        break None;
                    }
                    x = *t;
                }
                _ => break None,
            }
        };
        let Some(node) = type_node else { continue };
        let el = match body.get_resolved(node) {
            Some(Resolved::Element(el @ (Element::Class(_) | Element::Typedef(_)))) => *el,
            _ => continue,
        };
        let Some(r) = body.get_type(id) else { continue };
        if is_interface(r) && generic(el) {
            findings.push((
                a.expr(node).span,
                &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE,
                Vec::new(),
            ));
        }
    }

    // `visitSimpleFormalParameter`.
    // The element type of each parameter: that of the declared function, of
    // the constructor, the closure's inferred one, that of the local function.
    let function_param_types = |f: ast::FunctionId| -> Option<Vec<TypeId>> {
        let fid = program.function_of_node(s.unit, f).map(|x| x.0 as usize);
        if let Some(i) = fid {
            return s
                .outline
                .functions
                .get(i)
                .map(|d| d.parameters.iter().map(|p| p.ty).collect());
        }
        let g = a.function(f);
        let fn_type = match g.name {
            Some(n) => body.local_type(n.span.start),
            None => a
                .exprs
                .iter()
                .position(|e| matches!(e.kind, ExprKind::FunctionExpression(x) if x == f))
                .and_then(|k| body.get_type(ExprId(k as u32))),
        }?;
        let Type::Function {
            positional,
            optional,
            named,
            ..
        } = table.get(fn_type)
        else {
            return None;
        };
        let ps = g.parameters.as_deref().unwrap_or(&[]);
        let (mut i, mut j) = (0, 0);
        let mut v = Vec::new();
        for p in ps {
            let t = match p.kind {
                ParameterKind::Required => {
                    i += 1;
                    positional.get(i - 1).copied()
                }
                ParameterKind::Optional => {
                    j += 1;
                    optional.get(j - 1).copied()
                }
                ParameterKind::Named => p.external_name().and_then(|n| {
                    named
                        .iter()
                        .find(|(m, _, _)| *m == n.sym)
                        .map(|(_, t, _)| *t)
                }),
            };
            v.push(t.unwrap_or(s.core.dynamic_));
        }
        Some(v)
    };
    let mut lists: Vec<(&[ast::Parameter], Option<Vec<TypeId>>)> = Vec::new();
    for (k, f) in a.functions.iter().enumerate() {
        if let Some(ps) = f.parameters.as_deref() {
            lists.push((ps, function_param_types(ast::FunctionId(k as u32))));
        }
    }
    for (k, m) in a.members.iter().enumerate() {
        if let MemberKind::Constructor(kc) = &m.kind {
            let fid = program
                .constructor_of_member(s.unit, ast::MemberId(k as u32))
                .map(|x| x.0 as usize);
            let types = fid
                .and_then(|i| s.outline.functions.get(i))
                .map(|d| d.parameters.iter().map(|p| p.ty).collect());
            lists.push((&kc.parameters, types));
        }
    }
    // The inner parameters (of function parameters, of function types and of
    // old-style typedefs): without a written type, `dynamic`.
    let mut inner: Vec<&[ast::Parameter]> = Vec::new();
    fn with_nested<'x>(ps: &'x [ast::Parameter], v: &mut Vec<&'x [ast::Parameter]>) {
        for p in ps {
            if let Some(i) = &p.function_parameters {
                v.push(i);
                with_nested(i, v);
            }
        }
    }
    for (ps, _) in lists.iter() {
        with_nested(ps, &mut inner);
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            inner.push(parameters);
            with_nested(parameters, &mut inner);
        }
    }
    for d in a.decls.iter() {
        if let ast::DeclKind::Typedef(x) = &d.kind
            && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            inner.push(parameters);
            with_nested(parameters, &mut inner);
        }
    }
    for ps in inner {
        lists.push((ps, None));
    }
    for (ps, types) in lists {
        for (j, p) in ps.iter().enumerate() {
            // Only the `SimpleFormalParameter`.
            if p.this_ || p.super_ || p.function_parameters.is_some() || p.ty.is_some() {
                continue;
            }
            let Some(n) = p.name else { continue };
            let t = interner.resolve(n.sym);
            if !t.is_empty() && t.bytes().all(|b| b == b'_') {
                continue;
            }
            let param_type = types
                .as_ref()
                .and_then(|v| v.get(j).copied())
                .unwrap_or(s.core.dynamic_);
            if p.var_ || p.final_ || p.const_ {
                let Some(kw) = token_before(&tokens, n.span.start) else {
                    continue;
                };
                if p.var_ && !is_dynamic(param_type) {
                    findings.push((
                        kw,
                        &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
                        vec![word(kw).to_string(), type_arg(param_type)],
                    ));
                } else {
                    findings.push((kw, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
                }
            } else {
                let node = Span {
                    start: p.span.start,
                    end: n.span.end,
                };
                if is_dynamic(param_type) {
                    findings.push((node, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
                } else {
                    findings.push((
                        node,
                        &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
                        vec![type_arg(param_type)],
                    ));
                }
            }
        }
    }

    // `visitVariableDeclarationList`.
    let mut variable_lists: Vec<&ast::VariableList> = Vec::new();
    for d in a.decls.iter() {
        if let ast::DeclKind::Variables(l) = &d.kind {
            variable_lists.push(l);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            variable_lists.push(l);
        }
    }
    for st in a.stmts.iter() {
        match &st.kind {
            StmtKind::Variables(l)
            | StmtKind::For {
                init: Some(ForInit::Variables(l)),
                ..
            } => variable_lists.push(l),
            _ => {}
        }
    }
    fn from_collection<'x>(x: &'x CollectionElement, v: &mut Vec<&'x ast::VariableList>) {
        match x {
            CollectionElement::For {
                init: Some(ForInit::Variables(l)),
                body,
                ..
            } => {
                v.push(l);
                from_collection(body, v);
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                from_collection(body, v)
            }
            CollectionElement::If { then, else_, .. } => {
                from_collection(then, v);
                if let Some(e) = else_ {
                    from_collection(e, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            for x in elements.iter() {
                from_collection(x, &mut variable_lists);
            }
        }
    }
    for l in variable_lists {
        if l.ty.is_some() || !(l.var_ || l.final_ || l.const_) {
            continue;
        }
        let Some(first) = l.variables.first() else {
            continue;
        };
        let Some(kw) = token_before(&tokens, first.name.span.start) else {
            continue;
        };
        let p = word(kw);
        if !matches!(p, "var" | "final" | "const") {
            continue;
        }
        // `_getTypes`: the type of each initializer (a local read by the
        // element's type, without the promotion).
        let mut types: Vec<String> = Vec::new();
        for v in l.variables.iter() {
            let Some(i) = v.initializer else { continue };
            let t = match body.get_resolved(i) {
                Some(Resolved::Local(_)) if matches!(a.expr(i).kind, ExprKind::Identifier(_)) => {
                    body.local_declaration(i).and_then(|d| body.local_type(d))
                }
                _ => None,
            }
            .or_else(|| body.get_type(i));
            if let Some(t) = t {
                let x = text(t);
                if !types.contains(&x) {
                    types.push(x);
                }
            }
        }
        let single = types.len() == 1;
        let (code, args): (&'static LintCode, Vec<String>) = if types.is_empty() {
            (&c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new())
        } else if p == "var" {
            if single {
                (
                    &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
                    vec![p.to_string(), types[0].clone()],
                )
            } else {
                (&c::ALWAYS_SPECIFY_TYPES_SPLIT_TO_TYPES, Vec::new())
            }
        } else if single {
            (
                &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
                vec![types[0].clone()],
            )
        } else {
            (&c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new())
        };
        findings.push((kw, code, args));
    }

    findings.sort_by_key(|(sp, _, _)| (sp.start, sp.end));
    for (span, code, args) in findings {
        out.push(LintReport { code, span, args });
    }
    out
}
