//! Fusão opcional e conservadora de funções top-level estruturalmente idênticas.
//! Uma passagem escolhe a primeira definição elegível em ordem de fonte; não há
//! fusão de métodos, equivalência algébrica ou ponto fixo. Bindings lexicais recebem
//! IDs canônicos, permitindo equivalência alfa sem alterar seus escopos.
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
/// Identidades lexicais de bindings, tipos e destinos resolvidos são significativos.
/// Nomes locais e spans não são; nomes globais e membros permanecem exatos.
/// Representantes que poderiam ser capturados por locais/parâmetros são excluídos.
/// Closures ou referências a funções desativam a fusão neste módulo: identidade é observável.
pub fn merge_identical_functions(program: &mut Program<'_>, resolution: &Resolution) -> MergeStats {
    let function_names: BTreeSet<_> = program.functions.iter().map(|f| f.name).collect();
    let mut identity_observable = false;
    visit_program(program, &mut |_| {}, &mut |e| {
        if matches!(&e.kind, ExprKind::Closure { .. })
            || matches!(&e.kind, ExprKind::Identifier(n) if function_names.contains(n))
        {
            identity_observable = true;
        }
    });
    if identity_observable {
        return MergeStats::default();
    }
    let mut forbidden = BTreeSet::new();
    for class in &program.classes {
        for field in &class.fields {
            forbidden.insert(field.name);
        }
        for method in class.methods.iter().chain(&class.abstract_methods) {
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
        let Some(key) = function_key(f, resolution) else {
            continue;
        };
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
/// Escopos da chave canônica; bool indica declaração já inicializada.
struct Canonical<'a, 'r> {
    scopes: Vec<BTreeMap<&'a str, (usize, bool)>>,
    next_id: usize,
    valid: bool,
    resolution: &'r Resolution,
}
/// Canonicaliza parâmetros por posição e locais por ordem de declaração.
/// Retorna None se o AST não permitir resolver um binding com segurança.
fn function_key(f: &Function<'_>, resolution: &Resolution) -> Option<String> {
    let mut c = Canonical {
        scopes: vec![BTreeMap::new()],
        next_id: 0,
        valid: true,
        resolution,
    };
    for p in &f.parameters {
        c.declare(p.name, true);
    }
    let body = c.body(&f.body);
    c.valid.then(|| {
        pack(
            "function",
            &[
                format!("{:?}", f.return_type),
                format!(
                    "{:?}",
                    f.parameters.iter().map(|p| p.ty).collect::<Vec<_>>()
                ),
                body,
            ],
        )
    })
}
impl<'a> Canonical<'a, '_> {
    /// Reserva um ID estável; declarações duplicadas tornam a comparação inelegível.
    fn declare(&mut self, name: &'a str, ready: bool) {
        let id = self.next_id;
        self.next_id += 1;
        if self
            .scopes
            .last_mut()
            .expect("escopo ativo")
            .insert(name, (id, ready))
            .is_some()
        {
            self.valid = false;
        }
    }
    /// Resolve sempre o binding lexical mais próximo, nunca um homônimo externo.
    fn binding(&mut self, name: &str) -> String {
        for scope in self.scopes.iter().rev() {
            if let Some(&(id, ready)) = scope.get(name) {
                self.valid &= ready;
                return format!("local{id}");
            }
        }
        // O subconjunto não admite valores globais nem tear-offs de funções.
        self.valid = false;
        "unresolved".into()
    }
    /// Predeclara locais em todo o bloco, como a análise semântica Dart.
    fn body(&mut self, body: &[Statement<'a>]) -> String {
        self.scopes.push(BTreeMap::new());
        for s in body {
            if let StatementKind::Variable { name, .. } = &s.kind {
                self.declare(name, false);
            }
        }
        let result = pack(
            "body",
            &body.iter().map(|s| self.statement(s)).collect::<Vec<_>>(),
        );
        self.scopes.pop();
        result
    }
    /// Preserva escopos, inicializadores, atribuições e ordem de efeitos.
    fn statement(&mut self, s: &Statement<'a>) -> String {
        use StatementKind::*;
        match &s.kind {
            IndexAssign { .. } => {
                self.valid = false;
                "index-assignment".into()
            }
            Variable {
                name,
                annotation,
                is_final,
                initializer,
            } => {
                // A declaração oculta a externa inclusive no seu inicializador.
                let value = self.expression(initializer);
                if let Some(binding) = self.scopes.last_mut().and_then(|scope| scope.get_mut(name))
                {
                    binding.1 = true;
                } else {
                    self.valid = false;
                }
                pack(
                    "var",
                    &[
                        self.binding(name),
                        format!("{annotation:?}:{is_final}"),
                        value,
                    ],
                )
            }
            Assign { name, value } => pack("assign", &[self.binding(name), self.expression(value)]),
            FieldAssign {
                receiver,
                name,
                value,
            } => pack(
                "field",
                &[
                    self.expression(receiver),
                    (*name).into(),
                    self.expression(value),
                ],
            ),
            Print(e) => pack("print", &[self.expression(e)]),
            Expression(e) => pack("expr", &[self.expression(e)]),
            Return(e) => pack(
                "return",
                &e.iter().map(|e| self.expression(e)).collect::<Vec<_>>(),
            ),
            If {
                condition,
                then_body,
                else_body,
            } => pack(
                "if",
                &[
                    self.expression(condition),
                    self.body(then_body),
                    else_body.as_ref().map(|b| self.body(b)).unwrap_or_default(),
                ],
            ),
            While { condition, body } => {
                pack("while", &[self.expression(condition), self.body(body)])
            }
            DoWhile { body, condition } => {
                pack("do", &[self.body(body), self.expression(condition)])
            }
            For {
                initializer,
                condition,
                update,
                body,
            } => {
                // Cabeçalho tem escopo próprio; corpo possui um escopo filho.
                self.scopes.push(BTreeMap::new());
                if let Some(Statement {
                    kind: Variable { name, .. },
                    ..
                }) = initializer.as_deref()
                {
                    self.declare(name, false);
                }
                let result = pack(
                    "for",
                    &[
                        initializer
                            .as_ref()
                            .map(|s| self.statement(s))
                            .unwrap_or_default(),
                        condition
                            .as_ref()
                            .map(|e| self.expression(e))
                            .unwrap_or_default(),
                        update
                            .as_ref()
                            .map(|s| self.statement(s))
                            .unwrap_or_default(),
                        self.body(body),
                    ],
                );
                self.scopes.pop();
                result
            }
            Block(body) => pack("block", &[self.body(body)]),
            Break => "break".into(),
            Continue => "continue".into(),
        }
    }
    /// Canonicaliza apenas identificadores locais; destinos globais e tipos não mudam.
    fn expression(&mut self, e: &Expr<'a>) -> String {
        use ExprKind::*;
        match &e.kind {
            Closure { .. } | List { .. } | Index { .. } | Invoke { .. } => {
                self.valid = false;
                "allocation-or-indirect".into()
            }
            Null => "null".into(),
            This => {
                self.valid = false;
                "this".into()
            }
            Int(n) => format!("int{n}"),
            Bool(b) => format!("bool{b}"),
            String(s) => pack("str", &[(*s).into()]),
            OwnedString(s) => pack("str", std::slice::from_ref(s)),
            Identifier(n) => self.binding(n),
            Construct { class_id } => format!("new{class_id}"),
            EnumValue { class_id, name } => pack("enum", &[class_id.to_string(), (*name).into()]),
            Call { name, arguments } => {
                // Chamadas a locais não têm representação no subconjunto validado.
                if self
                    .scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.contains_key(name))
                {
                    self.valid = false;
                }
                pack(
                    "call",
                    &[
                        (*name).into(),
                        pack(
                            "args",
                            &arguments
                                .iter()
                                .map(|e| self.expression(e))
                                .collect::<Vec<_>>(),
                        ),
                    ],
                )
            }
            Member { receiver, name } => {
                pack("member", &[self.expression(receiver), (*name).into()])
            }
            MethodCall {
                receiver,
                name,
                arguments,
            } => pack(
                "method",
                &[
                    self.expression(receiver),
                    (*name).into(),
                    pack(
                        "args",
                        &arguments
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Vec<_>>(),
                    ),
                    format!(
                        "{:?}",
                        self.resolution
                            .extension_calls
                            .get(&(e.span.start, e.span.end))
                    ),
                ],
            ),
            Unary { op, operand } => pack("unary", &[format!("{op:?}"), self.expression(operand)]),
            Binary { op, left, right } => pack(
                "binary",
                &[
                    format!("{op:?}"),
                    self.expression(left),
                    self.expression(right),
                ],
            ),
        }
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
        IndexAssign {
            receiver,
            index,
            value,
        } => {
            visit_expr(receiver, e);
            visit_expr(index, e);
            visit_expr(value, e);
        }
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
        Closure { body, .. } => visit_body(body, &mut |_| {}, e),
        List { elements, .. } => {
            for x in elements {
                visit_expr(x, e);
            }
        }
        Index { receiver, index } => {
            visit_expr(receiver, e);
            visit_expr(index, e);
        }
        Invoke { callee, arguments } => {
            visit_expr(callee, e);
            for x in arguments {
                visit_expr(x, e);
            }
        }
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
        | Construct { .. }
        | EnumValue { .. } => {}
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
    /// Tipos e destinos diferentes permanecem significativos; nomes locais são alfa-equivalentes.
    #[test]
    fn distinct_semantics_stay_distinct() {
        check(
            "int a(int x) { return x + 1; } int b(int x) { return x + 2; } int c(int y) { return y + 1; } void main() {}",
            1,
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

    /// Parâmetros seguem posição, não grafia; ordem dos operandos permanece significativa.
    #[test]
    fn alpha_parameters_preserve_operand_order_and_types() {
        check(
            "int soma(int a, int b) => a + b; int outra(int x, int y) => x + y; int reversa(int x, int y) => y + x; void main() {}",
            1,
        );
        check("int a(int x) => 1; int b(bool x) => 1; void main() {}", 0);
    }

    /// IDs lexicais distinguem variáveis internas de referências ao escopo externo.
    #[test]
    fn alpha_nested_shadowing_and_initializers() {
        check(
            "int f(int n) { var result = n; { var n = 2; result = n; } return result; } int g(int input) { var output = input; { var inner = 2; output = inner; } return output; } void main() {}",
            1,
        );
        check(
            "int f(int n) { var result = n; { var n = 2; result = n; } return result; } int g(int input) { var output = input; { var inner = 2; output = input; } return output; } void main() {}",
            0,
        );
        check(
            "int f(int n) { var n = 2; return n; } int g(int m) { var other = 2; return other; } void main() {}",
            1,
        );
        check(
            "int f(int n) { var local = n; return local; } int g(int m) { var local = 1; return local; } void main() {}",
            0,
        );
    }

    /// Cabeçalho de for e corpo têm escopos distintos; o update resolve o cabeçalho.
    #[test]
    fn alpha_for_scope_and_restored_outer_binding() {
        check(
            "int f(int n) { var i = 10; for (var i = 0; i < n; i++) { var i = 2; print(i); } return i; } int g(int m) { var outer = 10; for (var j = 0; j < m; j++) { var inner = 2; print(inner); } return outer; } void main() {}",
            1,
        );
        check(
            "int f(int n) { var i = 10; for (var i = 0; i < n; i++) { print(i); } return i; } int g(int m) { var outer = 10; for (var j = 0; j < m; j++) { print(outer); } return outer; } void main() {}",
            0,
        );
    }

    /// Recursão e outros destinos globais não são normalizados como parâmetros locais.
    #[test]
    fn recursion_and_global_effect_targets_remain_exact() {
        check(
            "int f(int n) { if (n == 0) { return 1; } return f(n - 1); } int g(int m) { if (m == 0) { return 1; } return g(m - 1); } void main() {}",
            0,
        );
        check(
            "int left(int x) { print(1); return x; } int right(int x) { print(2); return x; } int f(int n) => left(n); int g(int m) => right(m); void main() {}",
            0,
        );
        check(
            "int f(int n) { print(n); return n; } int g(int m) { print(m); return m; } int h(int v) { print(1); return v; } void main() {}",
            1,
        );
    }

    /// Construtores e assinaturas nominais continuam distinguindo classes diferentes.
    #[test]
    fn alpha_keeps_nominal_classes_and_members() {
        check(
            "class A { int x = 1; } class B { int x = 1; } int f(A a) => a.x; int g(B b) => b.x; void main() {}",
            0,
        );
        check(
            "class C { int x = 1; int y = 2; } int f(C a) => a.x; int g(C b) => b.y; void main() {}",
            0,
        );
        check(
            "class C { int x = 1; } int f(C a) => a.x; int g(C b) => b.x; void main() {}",
            1,
        );
    }

    /// Uma chamada de extension conserva sua resolução ao remover a outra definição.
    #[test]
    fn alpha_preserves_extension_resolution() {
        check(
            "extension E on int { int twice() => this + this; } int f(int x) => x.twice(); int g(int y) => y.twice(); void main() { print(g(2)); }",
            1,
        );
    }

    /// Árvores não validadas com auto-referência ou uso anterior são inelegíveis.
    #[test]
    fn uncertain_bindings_are_not_merged() {
        for source in [
            "int f(int x) { var x = x; return x; } void main() {}",
            "int f(int x) { { print(x); var x = 1; } return x; } void main() {}",
        ] {
            let tokens = dartforge_lexer::lex(source).unwrap();
            let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
            assert!(function_key(&program.functions[0], &Resolution::default()).is_none());
        }
    }

    /// Valores de enum conservam tipo e identidade, mesmo sem alocação explícita.
    #[test]
    fn enum_values_keep_nominal_identity() {
        check(
            "enum E { a, b } E f() => E.a; E g() => E.a; E h() => E.b; void main() { print(g() == h()); }",
            1,
        );
        check(
            "enum E { a } enum F { a } E f() => E.a; F g() => F.a; void main() {}",
            0,
        );
    }

    /// Contratos abstratos também podem ocultar nomes globais no contexto de métodos.
    #[test]
    fn abstract_member_names_cannot_capture_representative() {
        check(
            "int a() => 1; int b() => 1; abstract class I { int a(); } abstract class C implements I { int run() => b(); } void main() {}",
            0,
        );
    }
}
