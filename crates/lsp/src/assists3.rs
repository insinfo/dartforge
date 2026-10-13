//! Três assistências de condição do servidor do Dart 3.6.2, pelas regras de
//! docs/LSP-ESPECIFICACAO.md §13.10:
//!
//! | Assistência | Espécie |
//! | --- | --- |
//! | `Invert 'if' statement` | `refactor.invertIf` |
//! | `Invert conditional expression` | `refactor.invertConditional` |
//! | `Join 'if' statement with inner 'if' statement` | `refactor.joinWithInnerIf` |
//!
//! As três usam o `invertCondition` do `CorrectionUtils`
//! (`correction_utils.dart:345-410`), aqui `Project::invert`. Diferença
//! conhecida: no `Invert conditional expression` o cursor vale em qualquer
//! ponto da condicional mais interna que não esteja dentro dos argumentos
//! de uma chamada aninhada (o original sobe um número fixo de nós).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::actions::CodeAction;
use crate::project::Project;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};

fn action(uri: &str, title: &str, kind: &str, edits: Vec<(Span, String)>) -> CodeAction {
    CodeAction {
        title: title.into(),
        kind: kind.into(),
        edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
        diagnostic: None,
        create_file: None, linked: Default::default(),
    }
}

const OR_PRECEDENCE: u32 = 1;
const AND_PRECEDENCE: u32 = 2;
const MAX_PRECEDENCE: u32 = 1 << 20;

/// A indentação da linha que contém `offset`.
fn indentation(source: &str, offset: usize) -> &str {
    let start = source[..offset.min(source.len())].rfind('\n').map_or(0, |i| i + 1);
    let rest = &source[start..];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}

impl Project {
    /// `invertCondition`: o texto da condição negada e a precedência dele.
    fn invert(&self, unit: UnitId, e: ExprId) -> (String, u32) {
        let u = self.program().unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let text = |x: ExprId| {
            let s = a.expr(x).span;
            source.get(s.start..s.end).unwrap_or("").to_string()
        };
        match &a.expr(e).kind {
            ExprKind::Bool(v) => ((!*v).to_string(), MAX_PRECEDENCE),
            ExprKind::Binary { op, left, right } => {
                let comparison = match op {
                    BinaryOp::Lt => Some(">="),
                    BinaryOp::Gt => Some("<="),
                    BinaryOp::LtEq => Some(">"),
                    BinaryOp::GtEq => Some("<"),
                    BinaryOp::Eq => Some("!="),
                    BinaryOp::NotEq => Some("=="),
                    _ => None,
                };
                if let Some(new) = comparison {
                    return (format!("{} {new} {}", text(*left), text(*right)), MAX_PRECEDENCE);
                }
                // De Morgan; cada lado entre parênteses se liga mais fraco
                // que o operador novo.
                let (junta, precedence) = match op {
                    BinaryOp::And => ("||", OR_PRECEDENCE),
                    BinaryOp::Or => ("&&", AND_PRECEDENCE),
                    _ => return self.negate_other(unit, e),
                };
                let side = |x: ExprId| {
                    let (t, p) = self.invert(unit, x);
                    if p < precedence { format!("({t})") } else { t }
                };
                (format!("{} {junta} {}", side(*left), side(*right)), precedence)
            }
            ExprKind::Is { value, ty, negated } => {
                let ty_ = a.types[ty.0 as usize].span;
                let word = if *negated { "is" } else { "is!" };
                (format!("{} {word} {}", text(*value), source.get(ty_.start..ty_.end).unwrap_or("")), MAX_PRECEDENCE)
            }
            // `!e`: o operando sem os parênteses de fora.
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                let mut inside = *operand;
                while let ExprKind::Parenthesized(x) = &a.expr(inside).kind {
                    inside = *x;
                }
                (text(inside), MAX_PRECEDENCE)
            }
            ExprKind::Parenthesized(x) => self.invert(unit, *x),
            _ => self.negate_other(unit, e),
        }
    }

    /// As duas últimas linhas da tabela: `!text` para uma expressão de tipo
    /// estático `bool`, o texto inalterado para as outras.
    fn negate_other(&self, unit: UnitId, e: ExprId) -> (String, u32) {
        let u = self.program().unit(unit);
        let s = u.ast.expr(e).span;
        let text = u.source.get(s.start..s.end).unwrap_or("");
        let is_bool = self.query.bodies.units[unit.0 as usize].get_type(e) == Some(self.query.core.bool_);
        (if is_bool { format!("!{text}") } else { text.to_string() }, MAX_PRECEDENCE)
    }

    /// As assistências desta lista com o cursor em `offset` de `unit`.
    pub(crate) fn condition_assists(&self, uri: &str, unit: UnitId, offset: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        let u = self.program().unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let command_text = |s: StmtId| {
            let span = a.stmt(s).span;
            source.get(span.start..span.end).unwrap_or("").to_string()
        };
        let is_block = |s: StmtId| matches!(a.stmt(s).kind, StmtKind::Block(_));

        // O `if` mais interno que contém o cursor.
        let the_if = a
            .stmts
            .iter()
            .filter(|s| matches!(s.kind, StmtKind::If { .. }) && s.span.start <= offset && offset <= s.span.end)
            .min_by_key(|s| s.span.end - s.span.start);
        if let Some(s) = the_if
            && let StmtKind::If { condition, case_pattern, then, else_, .. } = &s.kind
        {
            let cond = a.expr(*condition).span;
            let then_stmt = a.stmt(*then).span;
            // `Invert 'if' statement`: o cursor no próprio comando (no `if`,
            // nos parênteses ou no `else`), fora da condição e dos blocos.
            if let Some(else_stmt) = else_
                && case_pattern.is_none()
                && is_block(*then)
                && is_block(*else_stmt)
            {
                let in_command = offset < cond.start || (offset > cond.end && offset <= then_stmt.start) || (offset >= then_stmt.end && offset <= a.stmt(*else_stmt).span.start);
                if in_command {
                    let (inverted, _) = self.invert(unit, *condition);
                    output.push(action(
                        uri,
                        "Invert 'if' statement",
                        "refactor.invertIf",
                        vec![(cond, inverted), (then_stmt, command_text(*else_stmt)), (a.stmt(*else_stmt).span, command_text(*then))],
                    ));
                }
            }
            // `Join 'if' statement with inner 'if' statement`: o cursor no
            // comando ou na condição; sem `else`; o corpo é um `if` sem `else`.
            if else_.is_none() && offset <= then_stmt.start {
                // `getSingleStatement`: o comando, ou o único de um bloco.
                let single = match &a.stmt(*then).kind {
                    StmtKind::Block(l) if l.len() == 1 => Some(l[0]),
                    StmtKind::Block(_) => None,
                    _ => Some(*then),
                };
                if let Some(internal) = single
                    && let StmtKind::If { condition: internal_cond, then: body, else_: None, .. } = &a.stmt(internal).kind
                {
                    // `shouldWrapParenthesisBeforeAnd`: binária mais fraca que `&&`.
                    let to_is = |x: ExprId| {
                        let sp = a.expr(x).span;
                        let t = source.get(sp.start..sp.end).unwrap_or("");
                        if matches!(a.expr(x).kind, ExprKind::Binary { op: BinaryOp::Or | BinaryOp::IfNull, .. }) { format!("({t})") } else { t.to_string() }
                    };
                    let prefix = indentation(source, s.span.start);
                    let eol = if source.contains("\r\n") { "\r\n" } else { "\n" };
                    // Os comandos do corpo interno, por linhas inteiras, com
                    // um nível de indentação a menos.
                    let commands: Vec<StmtId> = match &a.stmt(*body).kind {
                        StmtKind::Block(l) => l.to_vec(),
                        _ => vec![*body],
                    };
                    let mut new = String::new();
                    if let (Some(first), Some(last)) = (commands.first(), commands.last()) {
                        let de = source[..a.stmt(*first).span.start].rfind('\n').map_or(0, |i| i + 1);
                        let last_end = a.stmt(*last).span.end;
                        let until = source[last_end..].find('\n').map_or(source.len(), |i| last_end + i + 1);
                        for line in source[de..until].split_inclusive('\n') {
                            new.push_str(line.strip_prefix("  ").unwrap_or(line));
                        }
                    }
                    let joined = format!("{} && {}", to_is(*condition), to_is(*internal_cond));
                    output.push(action(
                        uri,
                        "Join 'if' statement with inner 'if' statement",
                        "refactor.joinWithInnerIf",
                        vec![(s.span, format!("if ({joined}) {{{eol}{new}{prefix}}}"))],
                    ));
                }
            }
        }

        // `Invert conditional expression`: a condicional do
        // `_getConditionalExpressionAncestor`, sobre a árvore do analyzer.
        let conditional = {
            let cx = crate::refactorings::Context::new(self, unit);
            cx.next_conditional(offset).and_then(|n| cx.node_expr(n))
        };
        if let Some(e) = conditional.map(|x| a.expr(x))
            && let ExprKind::Conditional { condition, then, else_ } = &e.kind
        {
            {
                let snippet = |x: ExprId| {
                    let s = a.expr(x).span;
                    (s, source.get(s.start..s.end).unwrap_or("").to_string())
                };
                let (inverted, _) = self.invert(unit, *condition);
                let ((span_then, text_then), (span_else, text_else)) = (snippet(*then), snippet(*else_));
                output.push(action(
                    uri,
                    "Invert conditional expression",
                    "refactor.invertConditional",
                    vec![(a.expr(*condition).span, inverted), (span_then, text_else), (span_else, text_then)],
                ));
            }
        }
        output
    }
}

impl Project {
    /// Mais três assistências sintáticas (docs/LSP-ESPECIFICACAO.md §13.10):
    /// `Join 'if' statement with outer 'if' statement`
    /// (`refactor.joinWithOuterIf`), `Replace 'if-else' with conditional
    /// ('c ? x : y')` (`refactor.convert.ifElseToConditional`) e `Join
    /// variable declaration` (`refactor.joinVariableDeclaration`). Na
    /// última, "a mesma variável" é o mesmo nome no comando vizinho do mesmo
    /// bloco. Escrito sem compilar nem executar (2026-10-05).
    pub(crate) fn join_assists(&self, uri: &str, unit: UnitId, offset: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        let u = self.program().unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let eol = if source.contains("\r\n") { "\r\n" } else { "\n" };
        let expression_text = |x: ExprId| {
            let s = a.expr(x).span;
            source.get(s.start..s.end).unwrap_or("")
        };
        // `getSingleStatement`: o comando, ou o único de um bloco.
        let single = |s: StmtId| match &a.stmt(s).kind {
            StmtKind::Block(l) if l.len() == 1 => Some(l[0]),
            StmtKind::Block(_) => None,
            _ => Some(s),
        };

        // O `if` mais interno que contém o cursor.
        let the_if = a
            .stmts
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, StmtKind::If { .. }) && s.span.start <= offset && offset <= s.span.end)
            .min_by_key(|(_, s)| s.span.end - s.span.start);
        if let Some((index, s)) = the_if
            && let StmtKind::If { condition, case_pattern, then, else_, .. } = &s.kind
        {
            let id = StmtId(index as u32);
            let cond = a.expr(*condition).span;
            let then_stmt = a.stmt(*then).span;
            // `Join … with outer`: sem `else`; o pai (pulando um bloco de um
            // comando só) é um `if` sem `else`.
            if else_.is_none() && offset <= then_stmt.start {
                let external = a.stmts.iter().find(|x| match &x.kind {
                    StmtKind::If { then: t, else_: None, .. } => *t == id || (*t != id && single(*t) == Some(id)),
                    _ => false,
                });
                if let Some(external) = external
                    && let StmtKind::If { condition: external_cond, .. } = &external.kind
                {
                    let to_is = |x: ExprId| {
                        let t = expression_text(x);
                        if matches!(a.expr(x).kind, ExprKind::Binary { op: BinaryOp::Or | BinaryOp::IfNull, .. }) { format!("({t})") } else { t.to_string() }
                    };
                    let prefix = indentation(source, external.span.start);
                    let commands: Vec<StmtId> = match &a.stmt(*then).kind {
                        StmtKind::Block(l) => l.to_vec(),
                        _ => vec![*then],
                    };
                    let mut new = String::new();
                    if let (Some(first), Some(last)) = (commands.first(), commands.last()) {
                        let de = source[..a.stmt(*first).span.start].rfind('\n').map_or(0, |i| i + 1);
                        let last_end = a.stmt(*last).span.end;
                        let until = source[last_end..].find('\n').map_or(source.len(), |i| last_end + i + 1);
                        for line in source[de..until].split_inclusive('\n') {
                            new.push_str(line.strip_prefix("  ").unwrap_or(line));
                        }
                    }
                    let joined = format!("{} && {}", to_is(*external_cond), to_is(*condition));
                    output.push(action(
                        uri,
                        "Join 'if' statement with outer 'if' statement",
                        "refactor.joinWithOuterIf",
                        vec![(external.span, format!("if ({joined}) {{{eol}{new}{prefix}}}"))],
                    ));
                }
            }
            // `Replace 'if-else' with conditional`: o cursor no próprio
            // comando; cada ramo é um comando só.
            if let Some(else_stmt) = else_
                && case_pattern.is_none()
            {
                let in_command = offset < cond.start || (offset > cond.end && offset <= then_stmt.start) || (offset >= then_stmt.end && offset <= a.stmt(*else_stmt).span.start);
                if in_command && let (Some(x), Some(y)) = (single(*then), single(*else_stmt)) {
                    let c = expression_text(*condition);
                    let new = match (&a.stmt(x).kind, &a.stmt(y).kind) {
                        (StmtKind::Return(Some(p)), StmtKind::Return(Some(q))) => {
                            Some(format!("return {c} ? {} : {};", expression_text(*p), expression_text(*q)))
                        }
                        (StmtKind::Expression(p), StmtKind::Expression(q)) => match (&a.expr(*p).kind, &a.expr(*q).kind) {
                            (
                                ExprKind::Assign { op: ast::AssignOp::Assign, target: t1, value: v1 },
                                ExprKind::Assign { op: ast::AssignOp::Assign, target: t2, value: v2 },
                            ) if expression_text(*t1) == expression_text(*t2) => {
                                Some(format!("{} = {c} ? {} : {};", expression_text(*t1), expression_text(*v1), expression_text(*v2)))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(new) = new {
                        output.push(action(uri, "Replace 'if-else' with conditional ('c ? x : y')", "refactor.convert.ifElseToConditional", vec![(s.span, new)]));
                    }
                }
            }
        }

        // `Join variable declaration`: `T v;` seguido de `v = e;` no mesmo
        // bloco, com o cursor no nome atribuído ou na declaração.
        'blocos: for block in a.stmts.iter() {
            let StmtKind::Block(list) = &block.kind else { continue };
            for pair in list.windows(2) {
                let (declaration, assignment) = (a.stmt(pair[0]), a.stmt(pair[1]));
                let StmtKind::Variables(l) = &declaration.kind else { continue };
                let [variable] = &l.variables[..] else { continue };
                if variable.initializer.is_some() {
                    continue;
                }
                let StmtKind::Expression(e) = &assignment.kind else { continue };
                let ExprKind::Assign { op: ast::AssignOp::Assign, target, value } = &a.expr(*e).kind else { continue };
                let ExprKind::Identifier(n) = &a.expr(*target).kind else { continue };
                if n.sym != variable.name.sym {
                    continue;
                }
                let target_ = a.expr(*target).span;
                let assigned_name_node = target_.start <= offset && offset <= target_.end;
                let in_declaration = declaration.span.start <= offset && offset < declaration.span.end;
                if !(assigned_name_node || in_declaration) {
                    continue;
                }
                // O `=` fica entre o alvo e o valor.
                let Some(k) = source.get(target_.end..a.expr(*value).span.start).and_then(|t| t.find('=')) else { continue };
                output.push(action(
                    uri,
                    "Join variable declaration",
                    "refactor.joinVariableDeclaration",
                    vec![(Span { start: variable.name.span.end, end: target_.end + k }, " ".to_string())],
                ));
                break 'blocos;
            }
        }
        output
    }
}

impl crate::refactorings::Context<'_> {
    /// `_thisOrParentOfType`: o nó, ou o pai, se for de uma das espécies.
    fn this_or_parent_of(&self, n: usize, kinds: &[&str]) -> Option<usize> {
        if kinds.contains(&self.kind(n)) {
            return Some(n);
        }
        self.parent(n).filter(|&p| kinds.contains(&self.kind(p)))
    }

    /// `InvertConditionalExpression._getConditionalExpressionAncestor`
    /// (invert_conditional_expression.dart): a condicional "perto o
    /// bastante" do nó da seleção.
    pub(crate) fn next_conditional(&self, offset: usize) -> Option<usize> {
        let mut no = self.tree.localizar(offset, offset)?;
        no = self.this_or_parent_of(no, &["MethodInvocation"]).unwrap_or(no);
        no = self.this_or_parent_of(no, &["AwaitExpression"]).unwrap_or(no);
        let boolean = ["IsExpression", "BinaryExpression", "BooleanLiteral"];
        let boolean_ = if boolean.contains(&self.kind(no)) {
            Some(no)
        } else if let Some(p) = self.parent(no).filter(|&p| boolean.contains(&self.kind(p))) {
            Some(p)
        } else {
            self.this_or_parent_of(no, &["PrefixExpression"])
        };
        no = boolean_.unwrap_or(no);
        no = self.this_or_parent_of(no, &["ParenthesizedExpression"]).unwrap_or(no);
        no = self.this_or_parent_of(no, &["AwaitExpression"]).unwrap_or(no);
        self.this_or_parent_of(no, &["ConditionalExpression"])
    }
}
