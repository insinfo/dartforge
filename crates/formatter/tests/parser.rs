//! Testes do parser do formatador: as formas de nó que o analyzer produz
//! nos casos em que o `dart_style` as distingue, a cobertura de tokens e os
//! erros que o analyzer relata como sintáticos.

use dartforge_formatter::sintaxe::arvore::{Arvore, No, NoId};
use dartforge_formatter::sintaxe::parser::{OpcoesAnalise, analisar, analisar_com, verificar};
use dartforge_formatter::sintaxe::token::Tokens;

/// Analisa `fonte`, confere a cobertura e devolve tokens e árvore.
fn ler(fonte: &str) -> (Tokens, Arvore) {
    let (t, a) = analisar(fonte).unwrap_or_else(|e| panic!("{fonte:?}: {e:?}"));
    verificar::conferir(&t, &a, fonte).unwrap_or_else(|e| panic!("{fonte:?}: {e}"));
    (t, a)
}

/// Renderiza a subárvore de `id` como `Tipo(filhos...)`; identificadores e
/// literais simples viram o próprio texto.
fn render(fonte: &str, t: &Tokens, a: &Arvore, id: NoId) -> String {
    let texto = |tk: u32| t.texto(fonte, tk).to_string();
    match a.no(id) {
        No::SimpleIdentifier { token } => return texto(*token),
        No::IntegerLiteral { literal }
        | No::SimpleStringLiteral { literal }
        | No::BooleanLiteral { literal } => {
            return texto(*literal);
        }
        _ => {}
    }
    let mut filhos = Vec::new();
    a.no(id)
        .filhos(&mut |f| filhos.push(render(fonte, t, a, f)));
    format!("{}({})", verificar::nome_do_no(a.no(id)), filhos.join(", "))
}

/// A expressão de `f() { return <expr>; }`.
fn expr(e: &str) -> String {
    let fonte = format!("f() {{ return {e}; }}");
    let (t, a) = ler(&fonte);
    let id = a
        .nos
        .iter()
        .position(|d| matches!(d.no, No::ReturnStatement { .. }))
        .expect("comando return");
    let No::ReturnStatement { expression, .. } = a.nos[id].no else {
        unreachable!()
    };
    render(&fonte, &t, &a, expression.expect("expressão"))
}

/// A primeira declaração de topo de `fonte`, renderizada.
fn decl(fonte: &str) -> String {
    let (t, a) = ler(fonte);
    let No::CompilationUnit { declarations, .. } = a.no(a.raiz.unwrap()) else {
        unreachable!()
    };
    render(fonte, &t, &a, declarations[0])
}

/// O primeiro nó do tipo `nome` em `fonte`, renderizado.
fn primeiro(fonte: &str, nome: &str) -> String {
    let (t, a) = ler(fonte);
    let i = a
        .nos
        .iter()
        .position(|d| verificar::nome_do_no(&d.no) == nome)
        .unwrap_or_else(|| panic!("sem {nome} em {fonte:?}"));
    render(fonte, &t, &a, NoId(i as u32))
}

fn recusa(fonte: &str) {
    assert!(analisar(fonte).is_err(), "deveria recusar {fonte:?}");
}

#[test]
fn invocacoes_de_metodo() {
    assert_eq!(expr("foo()"), "MethodInvocation(foo, ArgumentList())");
    assert_eq!(
        expr("a.foo(1)"),
        "MethodInvocation(a, foo, ArgumentList(1))"
    );
    assert_eq!(
        expr("a.b.foo()"),
        "MethodInvocation(PrefixedIdentifier(a, b), foo, ArgumentList())"
    );
    assert_eq!(
        expr("super.foo()"),
        "MethodInvocation(SuperExpression(), foo, ArgumentList())"
    );
    assert_eq!(
        expr("foo<T>()"),
        "MethodInvocation(foo, TypeArgumentList(NamedType()), ArgumentList())"
    );
    assert_eq!(
        expr("A<T>()"),
        "MethodInvocation(A, TypeArgumentList(NamedType()), ArgumentList())"
    );
}

#[test]
fn invocacoes_de_expressao_de_funcao() {
    assert_eq!(
        expr("a()()"),
        "FunctionExpressionInvocation(MethodInvocation(a, ArgumentList()), ArgumentList())"
    );
    assert_eq!(
        expr("(a)()"),
        "FunctionExpressionInvocation(ParenthesizedExpression(a), ArgumentList())"
    );
    assert_eq!(
        expr("a[0]()"),
        "FunctionExpressionInvocation(IndexExpression(a, 0), ArgumentList())"
    );
    assert_eq!(
        expr("a.b()()"),
        "FunctionExpressionInvocation(MethodInvocation(a, b, ArgumentList()), ArgumentList())"
    );
    assert_eq!(
        expr("a!<int>()"),
        "FunctionExpressionInvocation(PostfixExpression(a), TypeArgumentList(NamedType()), ArgumentList())"
    );
}

#[test]
fn criacao_de_instancias() {
    assert_eq!(
        expr("new A()"),
        "InstanceCreationExpression(ConstructorName(NamedType()), ArgumentList())"
    );
    assert_eq!(
        expr("const A.b()"),
        "InstanceCreationExpression(ConstructorName(NamedType()), ArgumentList())"
    );
    assert_eq!(
        expr("new p.A<T>.b()"),
        "InstanceCreationExpression(ConstructorName(NamedType(TypeArgumentList(NamedType())), b), ArgumentList())"
    );
    // `parseImplicitCreationExpression`: só com argumentos de tipo e nome.
    assert_eq!(
        expr("A<T>.named()"),
        "InstanceCreationExpression(ConstructorName(NamedType(TypeArgumentList(NamedType())), named), ArgumentList())"
    );
    assert_eq!(
        expr("p.A<T>.named()"),
        "InstanceCreationExpression(ConstructorName(NamedType(TypeArgumentList(NamedType())), named), ArgumentList())"
    );
    // Sem argumentos de tipo, `A.named()` é uma invocação de método.
    assert_eq!(
        expr("A.named()"),
        "MethodInvocation(A, named, ArgumentList())"
    );
}

#[test]
fn identificadores_prefixados_e_acessos() {
    assert_eq!(expr("a.b"), "PrefixedIdentifier(a, b)");
    assert_eq!(expr("a.b.c"), "PropertyAccess(PrefixedIdentifier(a, b), c)");
    assert_eq!(expr("a?.b"), "PropertyAccess(a, b)");
    assert_eq!(
        expr("(a).b"),
        "PropertyAccess(ParenthesizedExpression(a), b)"
    );
    assert_eq!(expr("this.x"), "PropertyAccess(ThisExpression(), x)");
    assert_eq!(
        expr("a().b"),
        "PropertyAccess(MethodInvocation(a, ArgumentList()), b)"
    );
    assert_eq!(expr("a!.b"), "PropertyAccess(PostfixExpression(a), b)");
    assert_eq!(expr("a!"), "PostfixExpression(a)");
}

#[test]
fn indice_com_interrogacao() {
    let fonte = "f() { return a?[i]; }";
    let (t, a) = ler(fonte);
    let ix = a
        .nos
        .iter()
        .find(|d| matches!(d.no, No::IndexExpression { .. }))
        .unwrap();
    let No::IndexExpression {
        question, period, ..
    } = ix.no
    else {
        unreachable!()
    };
    assert_eq!(t.texto(fonte, question.unwrap()), "?");
    assert!(period.is_none());
    // `a ? [b] : c` é condicional.
    assert_eq!(
        expr("a ? [b] : c"),
        "ConditionalExpression(a, ListLiteral(b), c)"
    );
}

#[test]
fn cascatas() {
    assert_eq!(
        expr("x..a..b()..c[0] = 1"),
        "CascadeExpression(x, PropertyAccess(a), MethodInvocation(b, ArgumentList()), \
         AssignmentExpression(IndexExpression(PropertyAccess(c), 0), 1))"
    );
    assert_eq!(
        expr("x..a.b"),
        "CascadeExpression(x, PropertyAccess(PropertyAccess(a), b))"
    );
    assert_eq!(
        expr("x..a = 1"),
        "CascadeExpression(x, AssignmentExpression(PropertyAccess(a), 1))"
    );
    assert_eq!(
        expr("x..a()()"),
        "CascadeExpression(x, FunctionExpressionInvocation(MethodInvocation(a, ArgumentList()), ArgumentList()))"
    );
    assert_eq!(expr("x?..a"), "CascadeExpression(x, PropertyAccess(a))");
    assert_eq!(expr("x..[0]"), "CascadeExpression(x, IndexExpression(0))");
    // A seção sem alvo começa no operador.
    let fonte = "f() { return x..a; }";
    let (t, a) = ler(fonte);
    let pa = a
        .nos
        .iter()
        .find(|d| matches!(d.no, No::PropertyAccess { .. }))
        .unwrap();
    assert_eq!(t.texto(fonte, pa.inicio), "..");
}

#[test]
fn referencias_de_funcao_e_relacionais() {
    assert_eq!(
        expr("List<int>"),
        "FunctionReference(List, TypeArgumentList(NamedType()))"
    );
    assert_eq!(
        expr("foo<int>"),
        "FunctionReference(foo, TypeArgumentList(NamedType()))"
    );
    assert_eq!(
        expr("a.b<int>"),
        "FunctionReference(PrefixedIdentifier(a, b), TypeArgumentList(NamedType()))"
    );
    // `f(a < b, c > d)`: dois argumentos relacionais (`mayFollowTypeArgs`).
    assert_eq!(
        expr("f(a < b, c > d)"),
        "MethodInvocation(f, ArgumentList(BinaryExpression(a, b), BinaryExpression(c, d)))"
    );
    assert_eq!(
        expr("f(a < b, c > (d))"),
        "MethodInvocation(f, ArgumentList(MethodInvocation(a, TypeArgumentList(NamedType(), NamedType()), ArgumentList(d))))"
    );
}

#[test]
fn operadores_compostos_de_maior() {
    let fonte = "f() { a >> b >>> c; x >>= 1; y >= 2; }";
    let (t, a) = ler(fonte);
    let ops: Vec<&str> =
        a.nos
            .iter()
            .filter_map(|d| match d.no {
                No::BinaryExpression { operator, .. }
                | No::AssignmentExpression { operator, .. } => Some(t.texto(fonte, operator)),
                _ => None,
            })
            .collect();
    assert_eq!(ops, [">>", ">>>", ">>=", ">="]);
    // Em tipo, os `>` ficam separados.
    let (t, a) = ler("List<List<int>> x;");
    let gts = t
        .lista
        .iter()
        .filter(|k| &"List<List<int>> x;"[k.inicio..k.fim] == ">")
        .count();
    assert_eq!(gts, 2);
    assert!(a.raiz.is_some());
}

#[test]
fn literais_de_string() {
    assert_eq!(expr("'a'"), "'a'");
    assert_eq!(
        expr("'a${b}c$d'"),
        "StringInterpolation(InterpolationString(), InterpolationExpression(b), InterpolationString(), \
         InterpolationExpression(d), InterpolationString())"
    );
    assert_eq!(expr("'a' 'b'"), "AdjacentStrings('a', 'b')");
    assert_eq!(
        expr("'$this'"),
        "StringInterpolation(InterpolationString(), InterpolationExpression(ThisExpression()), InterpolationString())"
    );
    recusa("var s = '$class';");
    recusa("var s = '\\x4';");
    recusa("var s = '\\u{110000}';");
}

#[test]
fn records_e_parenteses() {
    assert_eq!(expr("(a)"), "ParenthesizedExpression(a)");
    assert_eq!(expr("(a,)"), "RecordLiteral(a)");
    assert_eq!(
        expr("(x: 1)"),
        "RecordLiteral(NamedExpression(Label(x), 1))"
    );
    assert_eq!(expr("const (1, 2)"), "RecordLiteral(1, 2)");
    assert_eq!(expr("()"), "RecordLiteral()");
    recusa("var x = const (1);");
    recusa("var x = (,);");
}

#[test]
fn colecoes_e_elementos() {
    assert_eq!(
        expr("[...a, ...?b]"),
        "ListLiteral(SpreadElement(a), SpreadElement(b))"
    );
    assert_eq!(expr("{1: 2}"), "SetOrMapLiteral(MapLiteralEntry(1, 2))");
    assert_eq!(
        expr("<int>{}"),
        "SetOrMapLiteral(TypeArgumentList(NamedType()))"
    );
    assert_eq!(
        expr("[if (a case int x when x > 0) x else 0]"),
        "ListLiteral(IfElement(a, CaseClause(GuardedPattern(DeclaredVariablePattern(NamedType()), \
         WhenClause(BinaryExpression(x, 0)))), x, 0))"
    );
    assert_eq!(
        expr("[for (var i in l) i]"),
        "ListLiteral(ForElement(ForEachPartsWithDeclaration(DeclaredIdentifier(), l), i))"
    );
    assert_eq!(
        expr("{for (var i = 0; i < n; i++) i: i}"),
        "SetOrMapLiteral(ForElement(ForPartsWithDeclarations(VariableDeclarationList(VariableDeclaration(0)), \
         BinaryExpression(i, n), PostfixExpression(i)), MapLiteralEntry(i, i)))"
    );
    recusa("var x = [?a];");
}

#[test]
fn funcoes() {
    assert_eq!(
        expr("(a) => a"),
        "FunctionExpression(FormalParameterList(SimpleFormalParameter()), ExpressionFunctionBody(a))"
    );
    assert_eq!(
        expr("<T>(T a) async {}"),
        "FunctionExpression(TypeParameterList(TypeParameter()), FormalParameterList(SimpleFormalParameter(NamedType())), \
         BlockFunctionBody(Block()))"
    );
    assert_eq!(
        decl("int f() => 0;"),
        "FunctionDeclaration(NamedType(), FunctionExpression(FormalParameterList(), ExpressionFunctionBody(0)))"
    );
    assert_eq!(
        primeiro("f() { void g() {} }", "FunctionDeclarationStatement"),
        "FunctionDeclarationStatement(FunctionDeclaration(NamedType(), FunctionExpression(FormalParameterList(), \
         BlockFunctionBody(Block()))))"
    );
    // O `;` do `=>`: presente em declarações, ausente em closures.
    let fonte = "f() => () => 1;";
    let (_, a) = ler(fonte);
    let corpos: Vec<bool> = a
        .nos
        .iter()
        .filter_map(|d| match d.no {
            No::ExpressionFunctionBody { semicolon, .. } => Some(semicolon.is_some()),
            _ => None,
        })
        .collect();
    assert_eq!(corpos, [false, true]);
    recusa("f() { var x = g() {}; }");
}

#[test]
fn parametros_formais() {
    assert_eq!(
        primeiro(
            "f(int a, {b = 1, required int c, this.d}) {}",
            "FormalParameterList"
        ),
        "FormalParameterList(SimpleFormalParameter(NamedType()), DefaultFormalParameter(SimpleFormalParameter(), 1), \
         DefaultFormalParameter(SimpleFormalParameter(NamedType())), DefaultFormalParameter(FieldFormalParameter()))"
    );
    assert_eq!(
        primeiro(
            "class A { A(super.x, void f(int y)?); }",
            "ConstructorDeclaration"
        ),
        "ConstructorDeclaration(A, FormalParameterList(SuperFormalParameter(), FunctionTypedFormalParameter(NamedType(), \
         FormalParameterList(SimpleFormalParameter(NamedType())))), EmptyFunctionBody())"
    );
    let fonte = "f([a]) {}\ng({b}) {}";
    let (_, a) = ler(fonte);
    let tipos: Vec<String> =
        a.nos
            .iter()
            .filter_map(|d| match d.no {
                No::SimpleFormalParameter { kind, .. }
                | No::DefaultFormalParameter { kind, .. } => Some(format!("{kind:?}")),
                _ => None,
            })
            .collect();
    assert_eq!(
        tipos,
        [
            "OpcionalPosicional",
            "OpcionalPosicional",
            "Nomeado",
            "Nomeado"
        ]
    );
    recusa("f(a = 1) {}");
    recusa("f([a], {b}) {}");
    recusa("f([a : 1]) {}");
}

#[test]
fn tipos() {
    assert_eq!(
        decl("p.List<int>? x;"),
        "TopLevelVariableDeclaration(VariableDeclarationList(NamedType(TypeArgumentList(NamedType())), VariableDeclaration()))"
    );
    assert_eq!(
        primeiro("int Function(int)? f;", "GenericFunctionType"),
        "GenericFunctionType(NamedType(), FormalParameterList(SimpleFormalParameter(NamedType())))"
    );
    assert_eq!(
        primeiro("(int, {String s}) r;", "RecordTypeAnnotation"),
        "RecordTypeAnnotation(RecordTypeAnnotationPositionalField(NamedType()), \
         RecordTypeAnnotationNamedFields(RecordTypeAnnotationNamedField(NamedType())))"
    );
    recusa("(int) r;");
}

#[test]
fn padroes() {
    let fonte = "f(o) { switch (o) { case [var a, ...] when a > 0: case {'k': _} || (1, :var b): case Foo(x: > 0): \
                 case int? _: case const [1] as Object: case -1: default: } }";
    let (t, a) = ler(fonte);
    let nomes: Vec<String> = a
        .nos
        .iter()
        .filter_map(|d| match d.no {
            No::SwitchPatternCase {
                guarded_pattern, ..
            } => {
                let No::GuardedPattern { pattern, .. } = a.no(guarded_pattern) else {
                    unreachable!()
                };
                Some(render(fonte, &t, &a, *pattern))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        nomes,
        [
            "ListPattern(DeclaredVariablePattern(), RestPatternElement())",
            "LogicalOrPattern(MapPattern(MapPatternEntry('k', WildcardPattern())), \
             RecordPattern(PatternField(ConstantPattern(1)), PatternField(PatternFieldName(), DeclaredVariablePattern())))",
            "ObjectPattern(NamedType(), PatternField(PatternFieldName(), RelationalPattern(0)))",
            "WildcardPattern(NamedType())",
            "CastPattern(ConstantPattern(ListLiteral(1)), NamedType())",
            "ConstantPattern(PrefixExpression(1))",
        ]
    );
    assert_eq!(
        primeiro(
            "f() { var (a, b) = (1, 2); }",
            "PatternVariableDeclarationStatement"
        ),
        "PatternVariableDeclarationStatement(PatternVariableDeclaration(RecordPattern(PatternField(DeclaredVariablePattern()), \
         PatternField(DeclaredVariablePattern())), RecordLiteral(1, 2)))"
    );
    assert_eq!(
        expr("(a, b) = (b, a)"),
        "PatternAssignment(RecordPattern(PatternField(AssignedVariablePattern()), PatternField(AssignedVariablePattern())), \
         RecordLiteral(b, a))"
    );
    // `WildcardPatternImpl.beginToken` ignora o `var`.
    let fonte = "f(o) { if (o case var _) {} }";
    let (t, a) = ler(fonte);
    let w = a
        .nos
        .iter()
        .find(|d| matches!(d.no, No::WildcardPattern { .. }))
        .unwrap();
    assert_eq!(t.texto(fonte, w.inicio), "_");
    recusa("f(o) { if (o case 1 + 2) {} }");
    recusa("f(o) { if (o case a? as int?) {} }");
}

#[test]
fn comandos() {
    assert_eq!(
        primeiro(
            "f() { a: for (final x in y) { break a; } }",
            "LabeledStatement"
        ),
        "LabeledStatement(Label(a), ForStatement(ForEachPartsWithDeclaration(DeclaredIdentifier(), y), \
         Block(BreakStatement(a))))"
    );
    assert_eq!(
        primeiro(
            "f() { try {} on E catch (e, s) {} finally {} }",
            "TryStatement"
        ),
        "TryStatement(Block(), CatchClause(NamedType(), CatchClauseParameter(), CatchClauseParameter(), Block()), Block())"
    );
    assert_eq!(
        primeiro(
            "f() { switch (x) { l: case 1: case 2: g(); default: } }",
            "SwitchStatement"
        ),
        "SwitchStatement(x, SwitchPatternCase(Label(l), GuardedPattern(ConstantPattern(1))), \
         SwitchPatternCase(GuardedPattern(ConstantPattern(2)), ExpressionStatement(MethodInvocation(g, ArgumentList()))), \
         SwitchDefault())"
    );
    assert_eq!(
        primeiro("f() { assert(a, 'm'); }", "AssertStatement"),
        "AssertStatement(a, 'm')"
    );
    assert_eq!(
        primeiro("f() { throw e; }", "ExpressionStatement"),
        "ExpressionStatement(ThrowExpression(e))"
    );
    assert_eq!(
        primeiro("f() { int? x = 1; }", "VariableDeclarationStatement"),
        "VariableDeclarationStatement(VariableDeclarationList(NamedType(), VariableDeclaration(1)))"
    );
    // `a ? b : c;` não é declaração de `a?`.
    assert_eq!(
        primeiro("f() { a ? b : c; }", "ExpressionStatement"),
        "ExpressionStatement(ConditionalExpression(a, b, c))"
    );
    recusa("f() { break; }");
    recusa("f() { switch (x) { case 1: continue; } }");
    recusa("f() { try {} }");
}

#[test]
fn assert_como_expressao() {
    // O `AstBuilder` monta `assert(...)` fora de comando como invocação.
    assert_eq!(
        primeiro(
            "f() { var x = assert(true); }",
            "FunctionExpressionInvocation"
        ),
        "FunctionExpressionInvocation(assert, ArgumentList(true))"
    );
}

#[test]
fn async_e_geradores() {
    assert_eq!(
        primeiro("f() async { await x; }", "AwaitExpression"),
        "AwaitExpression(x)"
    );
    // Fora de `async`, `await` seguido de expressão vira `AwaitExpression`
    // (o erro do analyzer não é sintático); sozinho é identificador.
    assert_eq!(
        primeiro("f() { await g(); }", "AwaitExpression"),
        "AwaitExpression(MethodInvocation(g, ArgumentList()))"
    );
    assert_eq!(expr("await"), "await");
    assert_eq!(
        primeiro("f() sync* { yield* g(); }", "YieldStatement"),
        "YieldStatement(MethodInvocation(g, ArgumentList()))"
    );
    recusa("f() async { var await = 1; }");
}

#[test]
fn declaracoes() {
    assert_eq!(
        decl(
            "abstract base class A<T> extends B with C implements D { A(this.x) : super(); int x = 0; }"
        ),
        "ClassDeclaration(TypeParameterList(TypeParameter()), ExtendsClause(NamedType()), WithClause(NamedType()), \
         ImplementsClause(NamedType()), ConstructorDeclaration(A, FormalParameterList(FieldFormalParameter()), \
         SuperConstructorInvocation(ArgumentList()), EmptyFunctionBody()), FieldDeclaration(VariableDeclarationList(NamedType(), \
         VariableDeclaration(0))))"
    );
    assert_eq!(
        decl("class A = B with C;"),
        "ClassTypeAlias(NamedType(), WithClause(NamedType()))"
    );
    assert_eq!(
        decl("enum E<T> with M { a, b<int>.c(1); const E(); }"),
        "EnumDeclaration(TypeParameterList(TypeParameter()), WithClause(NamedType()), EnumConstantDeclaration(), \
         EnumConstantDeclaration(EnumConstantArguments(TypeArgumentList(NamedType()), ConstructorSelector(c), ArgumentList(1))), \
         ConstructorDeclaration(E, FormalParameterList(), EmptyFunctionBody()))"
    );
    assert_eq!(
        decl("extension type const E.n(int i) implements I {}"),
        "ExtensionTypeDeclaration(RepresentationDeclaration(RepresentationConstructorName(), NamedType()), \
         ImplementsClause(NamedType()))"
    );
    assert_eq!(
        decl("extension on int {}"),
        "ExtensionDeclaration(ExtensionOnClause(NamedType()))"
    );
    assert_eq!(
        decl("base mixin M on A {}"),
        "MixinDeclaration(MixinOnClause(NamedType()))"
    );
    assert_eq!(
        decl("typedef F = void Function();"),
        "GenericTypeAlias(GenericFunctionType(NamedType(), FormalParameterList()))"
    );
    assert_eq!(
        decl("typedef void F(int x);"),
        "FunctionTypeAlias(NamedType(), FormalParameterList(SimpleFormalParameter(NamedType())))"
    );
    assert_eq!(
        decl("class A { factory A() = B<int>.c; }"),
        "ClassDeclaration(ConstructorDeclaration(A, FormalParameterList(), ConstructorName(NamedType(TypeArgumentList(NamedType())), c), \
         EmptyFunctionBody()))"
    );
    assert_eq!(
        decl(
            "class A { int get x => 1; set x(v) {} bool operator ==(o) => true; static const k = 1; }"
        ),
        "ClassDeclaration(MethodDeclaration(NamedType(), ExpressionFunctionBody(1)), MethodDeclaration(FormalParameterList(\
         SimpleFormalParameter()), BlockFunctionBody(Block())), MethodDeclaration(NamedType(), FormalParameterList(\
         SimpleFormalParameter()), ExpressionFunctionBody(true)), FieldDeclaration(VariableDeclarationList(VariableDeclaration(1))))"
    );
    recusa("class A { const A() {} }");
    recusa("class A { const factory A() { return A(); } }");
    recusa("class A { static abstract int x; }");
}

#[test]
fn diretivas() {
    let fonte = "library a.b;\nimport 'x.dart' if (dart.library.io) 'y.dart' deferred as p show A, B hide C;\n\
                 export 'z.dart';\npart 'w.dart';\n";
    let (t, a) = ler(fonte);
    let No::CompilationUnit { directives, .. } = a.no(a.raiz.unwrap()) else {
        unreachable!()
    };
    let d: Vec<String> = directives
        .iter()
        .map(|&d| render(fonte, &t, &a, d))
        .collect();
    assert_eq!(
        d,
        [
            "LibraryDirective(LibraryIdentifier(a, b))",
            "ImportDirective('x.dart', Configuration(DottedName(dart, library, io), 'y.dart'), p, ShowCombinator(A, B), HideCombinator(C))",
            "ExportDirective('z.dart')",
            "PartDirective('w.dart')",
        ]
    );
    recusa("class A {}\nimport 'a.dart';");
    recusa("import 'a.dart' deferred;");
}

#[test]
fn anotacoes() {
    assert_eq!(
        primeiro("@A.new(10)\nvoid f() {}", "Annotation"),
        "Annotation(PrefixedIdentifier(A, new), ArgumentList(10))"
    );
    assert_eq!(
        primeiro("@p.A<int>.b(1)\nvoid f() {}", "Annotation"),
        "Annotation(PrefixedIdentifier(p, A), TypeArgumentList(NamedType()), b, ArgumentList(1))"
    );
    // Sem colagem, o `(` não é argumento da anotação: é um tipo record.
    assert_eq!(
        primeiro("@a (int, int) f() => (1, 2);", "FunctionDeclaration"),
        "FunctionDeclaration(Annotation(a), RecordTypeAnnotation(RecordTypeAnnotationPositionalField(NamedType()), \
         RecordTypeAnnotationPositionalField(NamedType())), FunctionExpression(FormalParameterList(), ExpressionFunctionBody(RecordLiteral(1, 2))))"
    );
}

#[test]
fn simbolos_e_nomes_de_operador_fundidos() {
    // O analyzer tem um único token `[]`/`[]=`; o nó termina nele.
    let fonte = "f() => g(#[], [n]);";
    let (t, a) = ler(fonte);
    let s = a
        .nos
        .iter()
        .find(|d| matches!(d.no, No::SymbolLiteral { .. }))
        .unwrap();
    assert_eq!(t.texto(fonte, s.fim), "[]");
    let No::SymbolLiteral { components, .. } = &s.no else {
        unreachable!()
    };
    assert_eq!(t.texto(fonte, components[0]), "[]");
    assert_eq!(expr("#a.b.c"), "SymbolLiteral()");
    let fonte = "class A { operator []=(i, v) {} operator >>(o) => 0; }";
    let (t, a) = ler(fonte);
    let nomes: Vec<&str> = a
        .nos
        .iter()
        .filter_map(|d| match d.no {
            No::MethodDeclaration { name, .. } => Some(t.texto(fonte, name)),
            _ => None,
        })
        .collect();
    assert_eq!(nomes, ["[]=", ">>"]);
}

#[test]
fn erros_sintaticos() {
    recusa("f() { a == b == c; }");
    recusa("f() { a < b < c; }");
    recusa("f() { x is int is int; }");
    recusa("f() { x as int as int; }");
    recusa("f() { a + b = 1; }");
    recusa("f() { a++ ++; }");
    recusa("f( {}");
    recusa("var x = 1_;");
    recusa("var x = 0x;");
    recusa("var x = 1e;");
    recusa("var a\u{a0}= 1;");
    recusa("macro class A {}");
}

#[test]
fn erros_que_o_analyzer_nao_trata_como_sintaticos() {
    // Relatados pelo fasta, mas com código de erro não sintático no
    // analyzer: o `dart format` aceita.
    for fonte in [
        "f({_a}) {}",
        "const x;",
        "f() sync* { return 1; }",
        "class A { set x(v) async {} }",
        "f() async { yield 1; }",
        "f() { await for (var x in y) {} }",
        "var x = 5 ~ 3;",
        "class A<as> {}",
        "class A native 'X' {}",
        "f() native;",
        "var x = 1_000;",
    ] {
        ler(fonte);
    }
}

#[test]
fn versao_da_linguagem() {
    // Antes do 3.0, `case` lê uma expressão: `SwitchCase`.
    let fonte = "f(x) { switch (x) { case 1: break; } }";
    let (t, a) = analisar_com(fonte, OpcoesAnalise { versao: (2, 19) }).unwrap();
    verificar::conferir(&t, &a, fonte).unwrap();
    assert!(a.nos.iter().any(|d| matches!(d.no, No::SwitchCase { .. })));
    // O comentário `// @dart=` vence a versão pedida.
    let fonte = "// @dart=2.19\nf(x) { switch (x) { case 1: break; } }";
    let (_, a) = analisar(fonte).unwrap();
    assert!(a.nos.iter().any(|d| matches!(d.no, No::SwitchCase { .. })));
    assert!(analisar("// @dart=2.19\nvar r = (1, 2);").is_err());
    assert!(analisar("// @dart=3.5\nvar x = 1_000;").is_err());
}

#[test]
fn inicio_e_fim_das_declaracoes_ignoram_comentarios() {
    let fonte = "/// doc\n@a\nclass A {}\n// x\nvoid f() {}";
    let (t, a) = ler(fonte);
    let No::CompilationUnit { declarations, .. } = a.no(a.raiz.unwrap()) else {
        unreachable!()
    };
    assert_eq!(t.texto(fonte, a.inicio(declarations[0])), "@");
    assert_eq!(t.texto(fonte, a.inicio(declarations[1])), "void");
    assert_eq!(t.texto(fonte, a.fim(declarations[1])), "}");
}

#[test]
fn desempenho_arquivo_grande() {
    let mut fonte = String::new();
    for i in 0..2000 {
        fonte.push_str(&format!(
            "class C{i} extends B with M {{\n  final Map<String, List<int>> m = {{'a': [1, 2]}};\n  \
             int f(int x, {{String? y}}) => x > 0 ? (g(x - 1).h<int>()..k = 2) : -x;\n  \
             void g() {{ for (var e in m.entries) {{ if (e case MapEntry(:var key)) print('$key ${{e.value}}'); }} }}\n}}\n"
        ));
    }
    let inicio = std::time::Instant::now();
    let (t, a) = analisar(&fonte).unwrap();
    let tempo = inicio.elapsed();
    verificar::conferir(&t, &a, &fonte).unwrap();
    // 10 mil linhas; o limite é folgado para o perfil de testes sem otimização.
    assert!(tempo.as_secs_f64() < 5.0, "{tempo:?}");
}
