//! Assistências (`refactor.*`) do servidor do Dart 3.6.2 que só reescrevem
//! o documento, com os títulos e espécies dele
//! (`services/correction/dart/*.dart`):
//!
//! | Assistência | Espécie | Quando |
//! | --- | --- | --- |
//! | `Convert to async function body` | `refactor.convert.bodyToAsync` | função ou método (não construtor nem *closure*) sem `async`/`sync*`, cursor antes do corpo ou no `{`/`=>`; `async` e `Future<T>` no retorno escrito |
//! | `Convert to block body` | `refactor.convert.bodyToBlock` | corpo `=> e;`: `{ return e; }` (`e;` se o retorno é `void`) |
//! | `Convert to expression body` | `refactor.convert.bodyToExpression` | corpo com um só `return e;` (ou `e;`), cursor antes de `e`; não construtor gerador |
//! | `Remove type annotation` | `refactor.remove.typeAnnotation` | local com tipo e inicializador (cursor até o fim do nome; o tipo vira `var`, ou some depois de `final`), parâmetro simples com tipo |
//! | `Split variable declaration` | `refactor.splitVariableDeclaration` | local `var`/tipo (não `final`/`const`) com uma variável e inicializador, cursor até o fim do nome: `T x;` + `x = e;` |
//! | `Use curly braces` | `refactor.surround.curlyBraces` | `if`/`else`/`for`/`while`/`do` com corpo sem chaves |
//! | `Assign value to new local variable` | `refactor.assignToVariable` | comando de expressão de tipo não `void`: `var name = …` |
//!
//! `Inline Local Variable` e `Extract Local Variable` são refatorações por
//! comando (`refactorings.rs` e `refactorings_exec.rs`).
//!
//! Os nomes novos seguem o `getVariableNameSuggestionsForExpression` do
//! Dart: o da expressão (identificador, propriedade, método sem
//! `get`/`is`/`to`, classe criada), senão o do tipo (`i`, `d`, `s`, a
//! classe em camelCase), evitando os locais visíveis.

use crate::actions::CodeAction;
use crate::project::Project;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, ExprId, ExprKind, FunctionBody, StmtId, StmtKind};
use dartforge_types::{Type, TypeId};
use std::collections::HashSet;

fn action(uri: &str, title: &str, kind: &str, edits: Vec<(Span, String)>) -> CodeAction {
    CodeAction {
        title: title.into(),
        kind: kind.into(),
        edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
        diagnostic: None,
        create_file: None, linked: Default::default(),
    }
}

/// A indentação da linha que contém `offset`.
fn indentation(source: &str, offset: usize) -> String {
    let start = source[..offset.min(source.len())].rfind('\n').map_or(0, |i| i + 1);
    source[start..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

impl Project {
    /// As assistências desta lista em `start..end` de `unit`.
    pub(crate) fn rewrite_assists(&self, uri: &str, unit: UnitId, start: usize, end: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let source = u.source.as_str();
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let offset = start;
        let _ = end;

        // A expressão de função mais interna que contém o cursor.
        // O trecho de cada função conta com os metadados da declaração dona
        // (`@override` antes de um método: o `getEnclosingFunctionBody` do
        // Dart sobe da anotação ao método).
        let snippet = |i: usize, f: &ast::Function| -> Span {
            let owner = ast
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x.0 as usize == i))
                .map(|m| m.span)
                .or_else(|| ast.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(x) if x.0 as usize == i)).map(|d| d.span));
            match owner {
                Some(s) if s.start < f.span.start => Span { start: s.start, end: f.span.end },
                _ => f.span,
            }
        };
        let internal = ast
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f, snippet(i, f)))
            .filter(|(_, _, s)| s.start <= offset && offset <= s.end)
            .min_by_key(|(_, _, s)| s.end - s.start)
            .map(|(i, f, _)| (i, f));

        // `Convert to async function body`: `assist_funcoes` (o produtor do Dart).

        // Convert to expression body: a função mais interna.
        if let Some((_, f)) = internal {
            match f.body {
                // `Convert to block body` sai pelo porte do produtor do Dart
                // (`Context::convert_to_block_body`).
                FunctionBody::Block(b) if f.modifier != ast::AsyncModifier::SyncStar && f.modifier != ast::AsyncModifier::AsyncStar => {
                    if let StmtKind::Block(cmds) = &ast.stmt(b).kind
                        && cmds.len() == 1
                    {
                        let single = ast.stmt(cmds[0]);
                        let expr = match &single.kind {
                            StmtKind::Return(Some(e)) | StmtKind::Expression(e) => Some(*e),
                            _ => None,
                        };
                        if let Some(e) = expr
                            && offset < ast.expr(e).span.start
                        {
                            let es = ast.expr(e).span;
                            let bs = ast.stmt(b).span;
                            let async_ = if f.modifier == ast::AsyncModifier::Async { "async " } else { "" };
                            let semicolon = if f.name.is_some() { ";" } else { "" };
                            // O `async` já escrito fica; troca só o bloco.
                            let _ = async_;
                            output.push(action(
                                uri,
                                "Convert to expression body",
                                "refactor.convert.bodyToExpression",
                                vec![(bs, format!("=> {}{semicolon}", &source[es.start..es.end]))],
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Comando local que contém o cursor.
        let command = ast
            .stmts
            .iter()
            .enumerate()
            .filter(|(_, s)| s.span.start <= offset && offset <= s.span.end && !matches!(s.kind, StmtKind::Block(_)))
            .min_by_key(|(_, s)| s.span.end - s.span.start)
            .map(|(i, _)| StmtId(i as u32));

        if let Some(cid) = command {
            let s = ast.stmt(cid);
            match &s.kind {
                StmtKind::Variables(l) => {
                    // `Remove type annotation`: `assist_tipo` (o produtor do Dart).
                    // Split variable declaration.
                    if !l.final_ && !l.const_
                        && l.variables.len() == 1
                        && let Some(v) = l.variables.first()
                        && v.initializer.is_some()
                        && offset <= v.name.span.end
                    {
                        let name = &source[v.name.span.start..v.name.span.end];
                        let prefix = indentation(source, s.span.start);
                        let mut edits = Vec::new();
                        if l.ty.is_none()
                            && let Some(t) = bodies.tipo_local(v.name.span.start)
                            && !matches!(self.query.table.get(t), Type::Dynamic)
                            && let Some(k) = source[s.span.start..v.name.span.start].find("var")
                        {
                            let p = s.span.start + k;
                            edits.push((Span { start: p, end: p + 3 }, self.query.format(t)));
                        }
                        edits.push((Span { start: v.name.span.end, end: v.name.span.end }, format!(";\n{prefix}{name}")));
                        output.push(action(uri, "Split variable declaration", "refactor.splitVariableDeclaration", edits));
                    }
                }
                StmtKind::Expression(e) => {
                    // Assign value to new local variable.
                    let ty = bodies.get_type(*e);
                    let assignment = matches!(ast.expr(*e).kind, ExprKind::Assign { .. } | ExprKind::Throw(_));
                    if !assignment
                        && let Some(t) = ty
                        && !matches!(self.query.table.get(t), Type::Void)
                    {
                        // `AssignToLocalVariable`: as sugestões de
                        // `getVariableNameSuggestionsForExpression` com os
                        // nomes do `ScopedNameFinder` excluídos; a primeira é
                        // escrita, todas vão para o grupo ligado `NAME`.
                        let es = ast.expr(*e).span;
                        let cx = crate::refactorings::Context::new(self, unit);
                        let suggestions = match cx.tree.localizar(es.start, es.end) {
                            Some(n) => {
                                let excluded = cx.names_in_scope(n, es.start);
                                cx.names_to_expression(Some(t), n, &excluded, false)
                            }
                            None => vec![self.suggested_name(unit, *e, Some(t), offset)],
                        };
                        if let Some(name) = suggestions.first().cloned() {
                            let key = if crate::refactorings_exec::enabled_rule(self, unit, "prefer_final_locals") { "final" } else { "var" };
                            let text = format!("{key} {name} = ");
                            let a = action(uri, "Assign value to new local variable", "refactor.assignToVariable", vec![(Span { start: es.start, end: es.start }, text)]);
                            output.push(a.with_group(0, &[key.len() + 1], name.len(), suggestions));
                        }
                    }
                }
                _ => {}
            }
            // Use curly braces (o comando ou o pai).
            if let Some(action_keys) = self.use_keys(uri, unit, cid, offset) {
                output.push(action_keys);
            }
        }

        // Remove type annotation num parâmetro simples com tipo.
        for f in &ast.functions {
            for p in f.parameters.iter().flatten() {
                if let (Some(t), Some(n)) = (p.ty, p.name)
                    && !p.this_
                    && !p.super_
                    && p.function_parameters.is_none()
                    && p.span.start <= offset
                    && offset <= n.span.end
                {
                    let ts = ast.ty(t).span;
                    let rest = &source[ts.end..];
                    let type_end = ts.end + (rest.len() - rest.trim_start().len());
                    let replacement = if p.final_ || p.var_ { String::new() } else { String::new() };
                    output.push(action(uri, "Remove type annotation", "refactor.remove.typeAnnotation", vec![(Span { start: ts.start, end: type_end }, replacement)]));
                }
            }
        }

        // `Inline Local Variable` e `Extract Local Variable` são refatorações
        // por comando (`refactorings.rs`), não assistências.
        output
    }

    /// `Use curly braces` no comando `cid` (ou no pai dele): o porte de
    /// `UseCurlyBraces` (`use_curly_braces.dart`). O comando e o pai são
    /// examinados na ordem do Dart (`do`, `for`, `if`, `while`); cada corpo
    /// sem chaves recebe duas edições: ` {<eol><indent>` no lugar do espaço
    /// entre o token da esquerda e o corpo (os comentários antes do corpo
    /// ficam), e `<eol><prefix>}` depois do corpo (e do comentário na mesma
    /// linha), ou `<eol><prefix>} ` até o `else`/`while` seguinte.
    fn use_keys(&self, uri: &str, unit: UnitId, cid: StmtId, offset: usize) -> Option<CodeAction> {
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let source = u.source.as_str();
        let parent = ast
            .stmts
            .iter()
            .position(|s| match &s.kind {
                StmtKind::If { then, else_, .. } => *then == cid || *else_ == Some(cid),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => *body == cid,
                _ => false,
            })
            .map(|i| StmtId(i as u32));
        let tokens = dartforge_frontend::lexer::lex(source).ok()?;
        // O fim do último token antes de `o` e o início do primeiro depois.
        let before = |o: usize| tokens.iter().take_while(|t| t.span.end <= o).last().map_or(0, |t| t.span.end);
        let after = |o: usize| tokens.iter().find(|t| t.span.start >= o).map(|t| t.span);
        let eol = if source.contains("\r\n") { "\r\n" } else { "\n" };
        let is_block = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::Block(_));
        let is_do = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::DoWhile { .. });
        let is_for = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::For { .. } | StmtKind::ForIn { .. });
        let is_if = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::If { .. });
        let is_while = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::While { .. });
        let body_ = |s: StmtId| match &ast.stmt(s).kind {
            StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => Some(*body),
            _ => None,
        };
        // `_endAfterComments`: o fim do corpo, ou do comentário que começa na
        // mesma linha logo depois dele.
        let end_with_comment = |end: usize| -> usize {
            let rest = &source[end..];
            let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            let c = &rest[spaces..];
            if c.starts_with("//") {
                end + spaces + c.find(['\n', '\r']).unwrap_or(c.len())
            } else if c.starts_with("/*") {
                c.find("*/").map_or(end, |k| end + spaces + k + 2)
            } else {
                end
            }
        };
        let mut edits: Vec<(Span, String)> = Vec::new();
        // `_replaceLeftParenthesis`: do fim do token da esquerda ao corpo (ou
        // ao primeiro comentário antes dele).
        let left = |edits: &mut Vec<(Span, String)>, body_: StmtId, indent: &str| {
            let b = ast.stmt(body_).span.start;
            let a = before(b);
            let between = &source[a..b];
            let start = a + (between.len() - between.trim_start().len());
            edits.push((Span { start: a, end: start.min(b) }, format!(" {{{eol}{indent}")));
        };
        let mut so: Option<StmtId> = None;
        let target = if is_do(cid) {
            cid
        } else if parent.is_some_and(is_do) {
            parent?
        } else if is_for(cid) {
            cid
        } else if parent.is_some_and(is_for) {
            parent?
        } else if is_if(cid) {
            if let StmtKind::If { then, else_: Some(e), .. } = &ast.stmt(cid).kind
                && let Some(k) = after(ast.stmt(*then).span.end)
                && &source[k.start..k.end] == "else"
                && k.start <= offset
                && offset <= k.end
            {
                so = Some(*e);
            }
            cid
        } else if parent.is_some_and(is_if) {
            so = Some(cid);
            parent?
        } else if is_while(cid) {
            cid
        } else if parent.is_some_and(is_while) {
            parent?
        } else {
            return None;
        };
        let target_stmt = ast.stmt(target);
        let prefix = indentation(source, target_stmt.span.start);
        let indent = format!("{prefix}  ");
        match &target_stmt.kind {
            StmtKind::If { then, else_, .. } => {
                // O `else if` de uma cadeia não muda.
                if ast.stmts.iter().any(|s| matches!(&s.kind, StmtKind::If { else_: Some(e), .. } if *e == target)) {
                    return None;
                }
                let else_word = else_.and_then(|_| after(ast.stmt(*then).span.end)).filter(|k| &source[k.start..k.end] == "else");
                if !is_block(*then) && so.is_none_or(|x| x == *then) {
                    left(&mut edits, *then, &indent);
                    let end = end_with_comment(ast.stmt(*then).span.end);
                    match else_word {
                        None => edits.push((Span { start: end, end: end }, format!("{eol}{prefix}}}"))),
                        Some(k) => edits.push((Span { start: end, end: k.start }, format!("{eol}{prefix}}} "))),
                    }
                }
                if let (Some(e), Some(_)) = (else_, else_word)
                    && !is_block(*e)
                    && !is_if(*e)
                    && so.is_none_or(|x| x == *e)
                {
                    left(&mut edits, *e, &indent);
                    let end = end_with_comment(ast.stmt(*e).span.end);
                    edits.push((Span { start: end, end: end }, format!("{eol}{prefix}}}")));
                }
            }
            StmtKind::DoWhile { body, .. } => {
                if is_block(*body) {
                    return None;
                }
                left(&mut edits, *body, &indent);
                let end = end_with_comment(ast.stmt(*body).span.end);
                let k = after(ast.stmt(*body).span.end)?;
                edits.push((Span { start: end, end: k.start.max(end) }, format!("{eol}{prefix}}} ")));
            }
            _ => {
                let b = body_(target)?;
                if is_block(b) {
                    return None;
                }
                left(&mut edits, b, &indent);
                let end = end_with_comment(ast.stmt(b).span.end);
                edits.push((Span { start: end, end: end }, format!("{eol}{prefix}}}")));
            }
        }
        if edits.is_empty() {
            return None;
        }
        Some(action(uri, "Use curly braces", "refactor.surround.curlyBraces", edits))
    }

    /// O nome que o Dart sugere para guardar `expr`.
    fn suggested_name(&self, unit: UnitId, expr: ExprId, ty_: Option<TypeId>, offset: usize) -> String {
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let source = u.source.as_str();
        let text = |s: Span| source[s.start..s.end].to_string();
        let mut e = expr;
        loop {
            match &ast.expr(e).kind {
                ExprKind::Parenthesized(x) | ExprKind::As { value: x, .. } => e = *x,
                _ => break,
            }
        }
        let base: Option<String> = match &ast.expr(e).kind {
            ExprKind::Identifier(n) => Some(text(n.span)),
            ExprKind::Property { name, .. } => Some(text(name.span)),
            ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
                ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                    let n = text(n.span);
                    let sem = ["get", "is", "to"].iter().find_map(|p| n.strip_prefix(p).filter(|r| r.starts_with(char::is_uppercase)).map(str::to_string));
                    Some(sem.unwrap_or(n))
                }
                _ => None,
            },
            ExprKind::InstanceCreation { ty, .. } => match &ast.ty(*ty).kind {
                ast::TypeKind::Named { name, .. } => name.last().map(|n| text(n.span)),
                _ => None,
            },
            _ => None,
        };
        let by_type = || -> Option<String> {
            let t = ty_?;
            if t == self.query.core.int {
                return Some("i".into());
            }
            match self.query.table.get(t) {
                Type::Interface { class, .. } => {
                    let n = self.name(self.program().class(*class).name);
                    Some(match n {
                        "double" => "d".into(),
                        "String" => "s".into(),
                        "int" => "i".into(),
                        _ => n.to_string(),
                    })
                }
                _ => None,
            }
        };
        let base = base.or_else(by_type).unwrap_or_else(|| "value".into());
        let base = base.trim_start_matches('_').to_string();
        let mut name: String = base.chars().next().map(|c| c.to_lowercase().collect::<String>() + &base[c.len_utf8()..]).unwrap_or(base);
        // Os locais e parâmetros declarados antes não podem ser repetidos.
        let visible: HashSet<String> = ast
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Variables(l) => Some(l.variables.iter().map(|v| v.name).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .chain(ast.functions.iter().flat_map(|f| f.parameters.iter().flatten().filter_map(|p| p.name)))
            .filter(|n| n.span.start < offset)
            .map(|n| text(n.span))
            .collect();
        if visible.contains(&name) {
            let base = name.clone();
            let mut k = 2;
            while visible.contains(&name) {
                name = format!("{base}{k}");
                k += 1;
            }
        }
        name
    }
}
