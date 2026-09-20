//! Fusão opcional e conservadora de funções top-level estruturalmente idênticas.
//! Uma passagem escolhe a primeira definição elegível em ordem de fonte; não há
//! renomeação alfa, fusão de métodos, equivalência algébrica ou ponto fixo.
//! A chave canônica completa inclui tipos, bindings e destinos de chamadas.
//! Construí-la exige alocações; este passe privilegia redução de código, não latência.
//! Nomes/spans vistos em stack traces podem mudar quando uma chamada é redirecionada.
use dartforge_syntax::*;
use std::collections::{BTreeMap, BTreeSet};

/// Contagem de definições removidas; main e métodos nunca são removidos.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MergeStats {
    pub merged_functions: usize,
}

/// Funde definições validadas usando uma chave estrutural completa, sem hashes truncados.
/// Nomes de bindings, assinaturas e destinos resolvidos são significativos. Spans não são.
/// Representantes que poderiam ser capturados por locais/parâmetros são excluídos.
/// O subconjunto não admite tear-offs; somente main é exportado pelo backend JS.
pub fn merge_identical_functions(program: &mut Program<'_>, resolution: &Resolution) -> MergeStats {
    let mut forbidden = BTreeSet::new();
    for class in &program.classes {
        for field in &class.fields {
            forbidden.insert(field.name);
        }
        for method in &class.methods {
            forbidden.insert(method.name);
        }
    }
    for f in program
        .functions
        .iter()
        .chain(program.classes.iter().flat_map(|c| &c.methods))
        .chain(program.extensions.iter().flat_map(|e| &e.methods))
    {
        for p in &f.parameters {
            forbidden.insert(p.name);
        }
    }
    visit_program(
        program,
        &mut |s| {
            if let StatementKind::Variable { name, .. } = &s.kind {
                forbidden.insert(*name);
            }
        },
        &mut |_| {},
    );
    let mut representatives = BTreeMap::new();
    let mut replacements = BTreeMap::new();
    for f in &program.functions {
        if f.name == "main" {
            continue;
        }
        let key = format!(
            "{:?}|{:?}|{}",
            f.return_type,
            f.parameters
                .iter()
                .map(|p| (p.name, p.ty))
                .collect::<Vec<_>>(),
            statements_key(&f.body, resolution)
        );
        if let Some(&name) = representatives.get(&key) {
            replacements.insert(f.name, name);
        } else if !forbidden.contains(f.name) && f.name != "main" {
            representatives.insert(key, f.name);
        }
    }
    let stats = MergeStats {
        merged_functions: replacements.len(),
    };
    program
        .functions
        .retain(|f| !replacements.contains_key(f.name));
    visit_program(program, &mut |_| {}, &mut |e| {
        if let ExprKind::Call { name, .. } = &mut e.kind
            && let Some(&target) = replacements.get(name)
        {
            *name = target;
        }
    });
    stats
}
/// Delimita cada componente por comprimento em bytes, sem escape recursivo.
fn pack(tag: &str, parts: &[String]) -> String {
    let mut result = format!("{tag}:{}:", parts.len());
    for p in parts {
        result.push_str(&format!("{}:", p.len()));
        result.push_str(p);
    }
    result
}
/// Serializa sequência com fronteiras inequívocas entre instruções.
fn statements_key(body: &[Statement<'_>], r: &Resolution) -> String {
    pack(
        "body",
        &body.iter().map(|s| statement_key(s, r)).collect::<Vec<_>>(),
    )
}
/// Serializa instrução sem localização, mantendo ordem, bindings e alternativas.
fn statement_key(s: &Statement<'_>, r: &Resolution) -> String {
    use StatementKind::*;
    match &s.kind {
        Variable {
            name,
            annotation,
            is_final,
            initializer,
        } => pack(
            "var",
            &[
                format!("{name:?}:{annotation:?}:{is_final}"),
                expression_key(initializer, r),
            ],
        ),
        Assign { name, value } => pack("assign", &[(*name).into(), expression_key(value, r)]),
        FieldAssign {
            receiver,
            name,
            value,
        } => pack(
            "field",
            &[
                expression_key(receiver, r),
                (*name).into(),
                expression_key(value, r),
            ],
        ),
        Print(e) => pack("print", &[expression_key(e, r)]),
        Expression(e) => pack("expr", &[expression_key(e, r)]),
        Return(e) => pack(
            "return",
            &e.iter().map(|e| expression_key(e, r)).collect::<Vec<_>>(),
        ),
        If {
            condition,
            then_body,
            else_body,
        } => pack(
            "if",
            &[
                expression_key(condition, r),
                statements_key(then_body, r),
                else_body
                    .as_ref()
                    .map(|b| statements_key(b, r))
                    .unwrap_or_default(),
            ],
        ),
        While { condition, body } => pack(
            "while",
            &[expression_key(condition, r), statements_key(body, r)],
        ),
        DoWhile { body, condition } => pack(
            "do",
            &[statements_key(body, r), expression_key(condition, r)],
        ),
        For {
            initializer,
            condition,
            update,
            body,
        } => pack(
            "for",
            &[
                initializer
                    .as_ref()
                    .map(|s| statement_key(s, r))
                    .unwrap_or_default(),
                condition
                    .as_ref()
                    .map(|e| expression_key(e, r))
                    .unwrap_or_default(),
                update
                    .as_ref()
                    .map(|s| statement_key(s, r))
                    .unwrap_or_default(),
                statements_key(body, r),
            ],
        ),
        Block(body) => pack("block", &[statements_key(body, r)]),
        Break => "break".into(),
        Continue => "continue".into(),
    }
}
/// Serializa expressão incluindo o destino estático de chamadas de extension.
fn expression_key(e: &Expr<'_>, r: &Resolution) -> String {
    use ExprKind::*;
    match &e.kind {
        Null => "null".into(),
        This => "this".into(),
        Int(n) => format!("int{n}"),
        Bool(b) => format!("bool{b}"),
        String(s) => pack("str", &[(*s).into()]),
        OwnedString(s) => pack("str", std::slice::from_ref(s)),
        Identifier(n) => pack("id", &[(*n).into()]),
        Construct { class_id } => format!("new{class_id}"),
        Call { name, arguments } => pack(
            "call",
            &[
                (*name).into(),
                pack(
                    "args",
                    &arguments
                        .iter()
                        .map(|e| expression_key(e, r))
                        .collect::<Vec<_>>(),
                ),
            ],
        ),
        Member { receiver, name } => pack("member", &[expression_key(receiver, r), (*name).into()]),
        MethodCall {
            receiver,
            name,
            arguments,
        } => pack(
            "method",
            &[
                expression_key(receiver, r),
                (*name).into(),
                pack(
                    "args",
                    &arguments
                        .iter()
                        .map(|e| expression_key(e, r))
                        .collect::<Vec<_>>(),
                ),
                format!("{:?}", r.extension_calls.get(&(e.span.start, e.span.end))),
            ],
        ),
        Unary { op, operand } => pack("unary", &[format!("{op:?}"), expression_key(operand, r)]),
        Binary { op, left, right } => pack(
            "binary",
            &[
                format!("{op:?}"),
                expression_key(left, r),
                expression_key(right, r),
            ],
        ),
    }
}
/// Visita todas as instruções e expressões, inclusive inicializadores de campos.
fn visit_program<'a>(
    p: &mut Program<'a>,
    s: &mut impl FnMut(&mut Statement<'a>),
    e: &mut impl FnMut(&mut Expr<'a>),
) {
    for c in &mut p.classes {
        for f in &mut c.fields {
            visit_expr(&mut f.initializer, e);
        }
        for m in &mut c.methods {
            visit_body(&mut m.body, s, e);
        }
    }
    for x in &mut p.extensions {
        for m in &mut x.methods {
            visit_body(&mut m.body, s, e);
        }
    }
    for f in &mut p.functions {
        visit_body(&mut f.body, s, e);
    }
    visit_body(&mut p.statements, s, e);
}
/// Visita um bloco sem alterar a ordem das instruções.
fn visit_body<'a>(
    b: &mut [Statement<'a>],
    s: &mut impl FnMut(&mut Statement<'a>),
    e: &mut impl FnMut(&mut Expr<'a>),
) {
    for x in b {
        visit_stmt(x, s, e);
    }
}
/// Percorre todos os filhos de uma instrução, incluindo cabeçalhos de for.
fn visit_stmt<'a>(
    x: &mut Statement<'a>,
    s: &mut impl FnMut(&mut Statement<'a>),
    e: &mut impl FnMut(&mut Expr<'a>),
) {
    s(x);
    use StatementKind::*;
    match &mut x.kind {
        Variable { initializer, .. } => visit_expr(initializer, e),
        Assign { value, .. } | Print(value) | Expression(value) => visit_expr(value, e),
        FieldAssign {
            receiver, value, ..
        } => {
            visit_expr(receiver, e);
            visit_expr(value, e);
        }
        Return(v) => {
            if let Some(v) = v {
                visit_expr(v, e);
            }
        }
        If {
            condition,
            then_body,
            else_body,
        } => {
            visit_expr(condition, e);
            visit_body(then_body, s, e);
            if let Some(b) = else_body {
                visit_body(b, s, e);
            }
        }
        While { condition, body } | DoWhile { condition, body } => {
            visit_expr(condition, e);
            visit_body(body, s, e);
        }
        For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(x) = initializer {
                visit_stmt(x, s, e);
            }
            if let Some(x) = condition {
                visit_expr(x, e);
            }
            if let Some(x) = update {
                visit_stmt(x, s, e);
            }
            visit_body(body, s, e);
        }
        Block(b) => visit_body(b, s, e),
        Break | Continue => {}
    }
}
/// Reescreve chamadas diretas em qualquer posição de expressão.
fn visit_expr<'a>(x: &mut Expr<'a>, e: &mut impl FnMut(&mut Expr<'a>)) {
    e(x);
    use ExprKind::*;
    match &mut x.kind {
        Call { arguments, .. } => {
            for x in arguments {
                visit_expr(x, e);
            }
        }
        MethodCall {
            receiver,
            arguments,
            ..
        } => {
            visit_expr(receiver, e);
            for x in arguments {
                visit_expr(x, e);
            }
        }
        Member { receiver, .. } => visit_expr(receiver, e),
        Unary { operand, .. } => visit_expr(operand, e),
        Binary { left, right, .. } => {
            visit_expr(left, e);
            visit_expr(right, e);
        }
        Null
        | This
        | Int(_)
        | Bool(_)
        | String(_)
        | OwnedString(_)
        | Identifier(_)
        | Construct { .. } => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Analisa uma fonte válida e entrega sua árvore ao teste sem arenas persistentes.
    fn check(source: &str, expected: usize) {
        let tokens = dartforge_lexer::lex(source).unwrap();
        let mut p = dartforge_parser::parse(&tokens, source.len()).unwrap();
        let resolution = dartforge_semantic::analyze(&p).unwrap();
        assert_eq!(
            merge_identical_functions(&mut p, &resolution).merged_functions,
            expected
        );
        // A reanálise detecta captura acidental de nomes após reescrever chamadas.
        dartforge_semantic::analyze(&p).unwrap();
    }
    /// Corpos e assinaturas iguais permitem remover somente a segunda definição.
    #[test]
    fn exact_bodies_merge() {
        check(
            "int a(int x) { return x + 1; } int b(int x) { return x + 1; } void main() { print(a(1)); print(b(2)); }",
            1,
        );
    }
    /// Tipos, nomes locais e destinos diferentes nunca são ignorados pela chave.
    #[test]
    fn distinct_semantics_stay_distinct() {
        check(
            "int a(int x) { return x + 1; } int b(int x) { return x + 2; } int c(int y) { return y + 1; } void main() {}",
            0,
        );
        check(
            "int a() { return 1; } int? b() { return 1; } void main() {}",
            0,
        );
        check(
            "int a() { return 1; } int b() { return 2; } int c() { return a(); } int d() { return b(); } void main() {}",
            0,
        );
    }
    /// Não redireciona para um representante capturado por um parâmetro de outro corpo.
    #[test]
    fn representative_cannot_be_shadowed() {
        check(
            "int a() { return 1; } int b() { return 1; } int run(int a) { return b(); } void main() { print(run(8)); }",
            0,
        );
    }
    /// O percurso cobre classes, extensions, campos e cabeçalhos de laços.
    #[test]
    fn rewrites_every_direct_call_position() {
        check(
            "int a() { return 1; } int b() { return 1; } class C { int x = b(); int f() { return b(); } } extension E on int { int f() { return b(); } } void main() { var c = C(); for (var i = b(); i < b(); i = b()) { print(b()); } while (b() < 0) { print(b()); } do { print(b()); } while (b() < 0); }",
            1,
        );
    }
    /// Chaves de strings não confundem delimitadores e escapes com estrutura.
    #[test]
    fn literal_boundaries_are_unambiguous() {
        check(
            "String a() { return 'a:b'; } String b() { return 'a'; } String c() { return 'a:b'; } void main() {}",
            1,
        );
    }
}
