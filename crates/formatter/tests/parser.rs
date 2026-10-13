//! Tests of the formatter's parser: the node shapes the analyzer produces in
//! the cases where `dart_style` tells them apart, token coverage and the
//! errors the analyzer reports as syntactic.

use dartforge_formatter::syntax::tree::{Tree, Node, NodeId};
use dartforge_formatter::syntax::parser::{ParseOptions, parse, parse_with, verify};
use dartforge_formatter::syntax::token::Tokens;

/// Parses `source`, checks coverage and returns the tokens and the tree.
fn read(source: &str) -> (Tokens, Tree) {
    let (t, a) = parse(source).unwrap_or_else(|e| panic!("{source:?}: {e:?}"));
    verify::check(&t, &a, source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
    (t, a)
}

/// Renders the subtree of `id` as `Type(children...)`; identifiers and
/// simple literals become their own text.
fn render(source: &str, t: &Tokens, a: &Tree, id: NodeId) -> String {
    let text = |tk: u32| t.text(source, tk).to_string();
    match a.node(id) {
        Node::SimpleIdentifier { token } => return text(*token),
        Node::IntegerLiteral { literal }
        | Node::SimpleStringLiteral { literal }
        | Node::BooleanLiteral { literal } => {
            return text(*literal);
        }
        _ => {}
    }
    let mut children = Vec::new();
    a.node(id)
        .children(&mut |f| children.push(render(source, t, a, f)));
    format!("{}({})", verify::node_name(a.node(id)), children.join(", "))
}

/// The expression of `f() { return <expr>; }`.
fn expr(e: &str) -> String {
    let source = format!("f() {{ return {e}; }}");
    let (t, a) = read(&source);
    let id = a
        .nodes
        .iter()
        .position(|d| matches!(d.node, Node::ReturnStatement { .. }))
        .expect("return statement");
    let Node::ReturnStatement { expression, .. } = a.nodes[id].node else {
        unreachable!()
    };
    render(&source, &t, &a, expression.expect("expression"))
}

/// The first top-level declaration of `source`, rendered.
fn decl(source: &str) -> String {
    let (t, a) = read(source);
    let Node::CompilationUnit { declarations, .. } = a.node(a.root.unwrap()) else {
        unreachable!()
    };
    render(source, &t, &a, declarations[0])
}

/// The first node of kind `name_` in `source`, rendered.
fn first(source: &str, name_: &str) -> String {
    let (t, a) = read(source);
    let i = a
        .nodes
        .iter()
        .position(|d| verify::node_name(&d.node) == name_)
        .unwrap_or_else(|| panic!("no {name_} in {source:?}"));
    render(source, &t, &a, NodeId(i as u32))
}

fn rejection(source: &str) {
    assert!(parse(source).is_err(), "deveria recusar {source:?}");
}

#[test]
fn method_invocations() {
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
fn function_expression_invocations() {
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
fn instance_creation() {
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
    // `parseImplicitCreationExpression`: only with type arguments and a name.
    assert_eq!(
        expr("A<T>.named()"),
        "InstanceCreationExpression(ConstructorName(NamedType(TypeArgumentList(NamedType())), named), ArgumentList())"
    );
    assert_eq!(
        expr("p.A<T>.named()"),
        "InstanceCreationExpression(ConstructorName(NamedType(TypeArgumentList(NamedType())), named), ArgumentList())"
    );
    // Without type arguments, `A.named()` is a method invocation.
    assert_eq!(
        expr("A.named()"),
        "MethodInvocation(A, named, ArgumentList())"
    );
}

#[test]
fn prefixed_identifiers_and_accesses() {
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
fn index_with_question() {
    let source = "f() { return a?[i]; }";
    let (t, a) = read(source);
    let ix = a
        .nodes
        .iter()
        .find(|d| matches!(d.node, Node::IndexExpression { .. }))
        .unwrap();
    let Node::IndexExpression {
        question, period, ..
    } = ix.node
    else {
        unreachable!()
    };
    assert_eq!(t.text(source, question.unwrap()), "?");
    assert!(period.is_none());
    // `a ? [b] : c` is a conditional.
    assert_eq!(
        expr("a ? [b] : c"),
        "ConditionalExpression(a, ListLiteral(b), c)"
    );
}

#[test]
fn cascades() {
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
    // The section without a target starts at the operator.
    let source = "f() { return x..a; }";
    let (t, a) = read(source);
    let pa = a
        .nodes
        .iter()
        .find(|d| matches!(d.node, Node::PropertyAccess { .. }))
        .unwrap();
    assert_eq!(t.text(source, pa.start), "..");
}

#[test]
fn function_references_and_relationals() {
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
    // `f(a < b, c > d)`: two relational arguments (`mayFollowTypeArgs`).
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
fn composed_greater_operators() {
    let source = "f() { a >> b >>> c; x >>= 1; y >= 2; }";
    let (t, a) = read(source);
    let ops: Vec<&str> =
        a.nodes
            .iter()
            .filter_map(|d| match d.node {
                Node::BinaryExpression { operator, .. }
                | Node::AssignmentExpression { operator, .. } => Some(t.text(source, operator)),
                _ => None,
            })
            .collect();
    assert_eq!(ops, [">>", ">>>", ">>=", ">="]);
    // In a type, the `>` stay separate.
    let (t, a) = read("List<List<int>> x;");
    let gts = t
        .list
        .iter()
        .filter(|k| &"List<List<int>> x;"[k.start..k.end] == ">")
        .count();
    assert_eq!(gts, 2);
    assert!(a.root.is_some());
}

#[test]
fn string_literals() {
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
    rejection("var s = '$class';");
    rejection("var s = '\\x4';");
    rejection("var s = '\\u{110000}';");
}

#[test]
fn records_and_parentheses() {
    assert_eq!(expr("(a)"), "ParenthesizedExpression(a)");
    assert_eq!(expr("(a,)"), "RecordLiteral(a)");
    assert_eq!(
        expr("(x: 1)"),
        "RecordLiteral(NamedExpression(Label(x), 1))"
    );
    assert_eq!(expr("const (1, 2)"), "RecordLiteral(1, 2)");
    assert_eq!(expr("()"), "RecordLiteral()");
    rejection("var x = const (1);");
    rejection("var x = (,);");
}

#[test]
fn collections_and_elements() {
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
    rejection("var x = [?a];");
}

#[test]
fn functions() {
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
        first("f() { void g() {} }", "FunctionDeclarationStatement"),
        "FunctionDeclarationStatement(FunctionDeclaration(NamedType(), FunctionExpression(FormalParameterList(), \
         BlockFunctionBody(Block()))))"
    );
    // The `;` of `=>`: present in declarations, absent in closures.
    let source = "f() => () => 1;";
    let (_, a) = read(source);
    let bodies: Vec<bool> = a
        .nodes
        .iter()
        .filter_map(|d| match d.node {
            Node::ExpressionFunctionBody { semicolon, .. } => Some(semicolon.is_some()),
            _ => None,
        })
        .collect();
    assert_eq!(bodies, [false, true]);
    rejection("f() { var x = g() {}; }");
}

#[test]
fn formal_parameters_() {
    assert_eq!(
        first(
            "f(int a, {b = 1, required int c, this.d}) {}",
            "FormalParameterList"
        ),
        "FormalParameterList(SimpleFormalParameter(NamedType()), DefaultFormalParameter(SimpleFormalParameter(), 1), \
         DefaultFormalParameter(SimpleFormalParameter(NamedType())), DefaultFormalParameter(FieldFormalParameter()))"
    );
    assert_eq!(
        first(
            "class A { A(super.x, void f(int y)?); }",
            "ConstructorDeclaration"
        ),
        "ConstructorDeclaration(A, FormalParameterList(SuperFormalParameter(), FunctionTypedFormalParameter(NamedType(), \
         FormalParameterList(SimpleFormalParameter(NamedType())))), EmptyFunctionBody())"
    );
    let source = "f([a]) {}\ng({b}) {}";
    let (_, a) = read(source);
    let types_: Vec<String> =
        a.nodes
            .iter()
            .filter_map(|d| match d.node {
                Node::SimpleFormalParameter { kind, .. }
                | Node::DefaultFormalParameter { kind, .. } => Some(format!("{kind:?}")),
                _ => None,
            })
            .collect();
    assert_eq!(
        types_,
        [
            "OptionalPositional",
            "OptionalPositional",
            "Named",
            "Named"
        ]
    );
    rejection("f(a = 1) {}");
    rejection("f([a], {b}) {}");
    rejection("f([a : 1]) {}");
}

#[test]
fn types_() {
    assert_eq!(
        decl("p.List<int>? x;"),
        "TopLevelVariableDeclaration(VariableDeclarationList(NamedType(TypeArgumentList(NamedType())), VariableDeclaration()))"
    );
    assert_eq!(
        first("int Function(int)? f;", "GenericFunctionType"),
        "GenericFunctionType(NamedType(), FormalParameterList(SimpleFormalParameter(NamedType())))"
    );
    assert_eq!(
        first("(int, {String s}) r;", "RecordTypeAnnotation"),
        "RecordTypeAnnotation(RecordTypeAnnotationPositionalField(NamedType()), \
         RecordTypeAnnotationNamedFields(RecordTypeAnnotationNamedField(NamedType())))"
    );
    rejection("(int) r;");
}

#[test]
fn patterns_() {
    let source = "f(o) { switch (o) { case [var a, ...] when a > 0: case {'k': _} || (1, :var b): case Foo(x: > 0): \
                 case int? _: case const [1] as Object: case -1: default: } }";
    let (t, a) = read(source);
    let names: Vec<String> = a
        .nodes
        .iter()
        .filter_map(|d| match d.node {
            Node::SwitchPatternCase {
                guarded_pattern, ..
            } => {
                let Node::GuardedPattern { pattern, .. } = a.node(guarded_pattern) else {
                    unreachable!()
                };
                Some(render(source, &t, &a, *pattern))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
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
        first(
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
    // `WildcardPatternImpl.beginToken` ignores the `var`.
    let source = "f(o) { if (o case var _) {} }";
    let (t, a) = read(source);
    let w = a
        .nodes
        .iter()
        .find(|d| matches!(d.node, Node::WildcardPattern { .. }))
        .unwrap();
    assert_eq!(t.text(source, w.start), "_");
    rejection("f(o) { if (o case 1 + 2) {} }");
    rejection("f(o) { if (o case a? as int?) {} }");
}

#[test]
fn statements_() {
    assert_eq!(
        first(
            "f() { a: for (final x in y) { break a; } }",
            "LabeledStatement"
        ),
        "LabeledStatement(Label(a), ForStatement(ForEachPartsWithDeclaration(DeclaredIdentifier(), y), \
         Block(BreakStatement(a))))"
    );
    assert_eq!(
        first(
            "f() { try {} on E catch (e, s) {} finally {} }",
            "TryStatement"
        ),
        "TryStatement(Block(), CatchClause(NamedType(), CatchClauseParameter(), CatchClauseParameter(), Block()), Block())"
    );
    assert_eq!(
        first(
            "f() { switch (x) { l: case 1: case 2: g(); default: } }",
            "SwitchStatement"
        ),
        "SwitchStatement(x, SwitchPatternCase(Label(l), GuardedPattern(ConstantPattern(1))), \
         SwitchPatternCase(GuardedPattern(ConstantPattern(2)), ExpressionStatement(MethodInvocation(g, ArgumentList()))), \
         SwitchDefault())"
    );
    assert_eq!(
        first("f() { assert(a, 'm'); }", "AssertStatement"),
        "AssertStatement(a, 'm')"
    );
    assert_eq!(
        first("f() { throw e; }", "ExpressionStatement"),
        "ExpressionStatement(ThrowExpression(e))"
    );
    assert_eq!(
        first("f() { int? x = 1; }", "VariableDeclarationStatement"),
        "VariableDeclarationStatement(VariableDeclarationList(NamedType(), VariableDeclaration(1)))"
    );
    // `a ? b : c;` is not a declaration of `a?`.
    assert_eq!(
        first("f() { a ? b : c; }", "ExpressionStatement"),
        "ExpressionStatement(ConditionalExpression(a, b, c))"
    );
    rejection("f() { break; }");
    rejection("f() { switch (x) { case 1: continue; } }");
    rejection("f() { try {} }");
}

#[test]
fn assert_as_expression() {
    // The `AstBuilder` builds `assert(...)` outside a statement as an
    // invocation.
    assert_eq!(
        first(
            "f() { var x = assert(true); }",
            "FunctionExpressionInvocation"
        ),
        "FunctionExpressionInvocation(assert, ArgumentList(true))"
    );
}

#[test]
fn async_and_generators() {
    assert_eq!(
        first("f() async { await x; }", "AwaitExpression"),
        "AwaitExpression(x)"
    );
    // Outside `async`, `await` followed by an expression becomes an
    // `AwaitExpression` (the analyzer's error is not syntactic); alone it is an
    // identifier.
    assert_eq!(
        first("f() { await g(); }", "AwaitExpression"),
        "AwaitExpression(MethodInvocation(g, ArgumentList()))"
    );
    assert_eq!(expr("await"), "await");
    assert_eq!(
        first("f() sync* { yield* g(); }", "YieldStatement"),
        "YieldStatement(MethodInvocation(g, ArgumentList()))"
    );
    rejection("f() async { var await = 1; }");
}

#[test]
fn declarations_() {
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
    rejection("class A { const A() {} }");
    rejection("class A { const factory A() { return A(); } }");
    rejection("class A { static abstract int x; }");
}

#[test]
fn directives_() {
    let source = "library a.b;\nimport 'x.dart' if (dart.library.io) 'y.dart' deferred as p show A, B hide C;\n\
                 export 'z.dart';\npart 'w.dart';\n";
    let (t, a) = read(source);
    let Node::CompilationUnit { directives, .. } = a.node(a.root.unwrap()) else {
        unreachable!()
    };
    let d: Vec<String> = directives
        .iter()
        .map(|&d| render(source, &t, &a, d))
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
    rejection("class A {}\nimport 'a.dart';");
    rejection("import 'a.dart' deferred;");
}

#[test]
fn annotations() {
    assert_eq!(
        first("@A.new(10)\nvoid f() {}", "Annotation"),
        "Annotation(PrefixedIdentifier(A, new), ArgumentList(10))"
    );
    assert_eq!(
        first("@p.A<int>.b(1)\nvoid f() {}", "Annotation"),
        "Annotation(PrefixedIdentifier(p, A), TypeArgumentList(NamedType()), b, ArgumentList(1))"
    );
    // Without gluing, the `(` is not an argument of the annotation: it is a
    // record type.
    assert_eq!(
        first("@a (int, int) f() => (1, 2);", "FunctionDeclaration"),
        "FunctionDeclaration(Annotation(a), RecordTypeAnnotation(RecordTypeAnnotationPositionalField(NamedType()), \
         RecordTypeAnnotationPositionalField(NamedType())), FunctionExpression(FormalParameterList(), ExpressionFunctionBody(RecordLiteral(1, 2))))"
    );
}

#[test]
fn symbols_and_merged_operator_names() {
    // The analyzer has a single `[]`/`[]=` token; the node ends at it.
    let source = "f() => g(#[], [n]);";
    let (t, a) = read(source);
    let s = a
        .nodes
        .iter()
        .find(|d| matches!(d.node, Node::SymbolLiteral { .. }))
        .unwrap();
    assert_eq!(t.text(source, s.end), "[]");
    let Node::SymbolLiteral { components, .. } = &s.node else {
        unreachable!()
    };
    assert_eq!(t.text(source, components[0]), "[]");
    assert_eq!(expr("#a.b.c"), "SymbolLiteral()");
    let source = "class A { operator []=(i, v) {} operator >>(o) => 0; }";
    let (t, a) = read(source);
    let names: Vec<&str> = a
        .nodes
        .iter()
        .filter_map(|d| match d.node {
            Node::MethodDeclaration { name, .. } => Some(t.text(source, name)),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["[]=", ">>"]);
}

#[test]
fn syntax_errors() {
    rejection("f() { a == b == c; }");
    rejection("f() { a < b < c; }");
    rejection("f() { x is int is int; }");
    rejection("f() { x as int as int; }");
    rejection("f() { a + b = 1; }");
    rejection("f() { a++ ++; }");
    rejection("f( {}");
    rejection("var x = 1_;");
    rejection("var x = 0x;");
    rejection("var x = 1e;");
    rejection("var a\u{a0}= 1;");
    rejection("macro class A {}");
}

#[test]
fn errors_the_analyzer_does_not_treat_as_syntactic() {
    // Reported by fasta, but with a non-syntactic error code in the analyzer:
    // `dart format` accepts them.
    for source in [
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
        read(source);
    }
}

#[test]
fn language_version() {
    // Before 3.0, `case` reads an expression: `SwitchCase`.
    let source = "f(x) { switch (x) { case 1: break; } }";
    let (t, a) = parse_with(source, ParseOptions { version: (2, 19) }).unwrap();
    verify::check(&t, &a, source).unwrap();
    assert!(a.nodes.iter().any(|d| matches!(d.node, Node::SwitchCase { .. })));
    // The `// @dart=` comment wins over the requested version.
    let source = "// @dart=2.19\nf(x) { switch (x) { case 1: break; } }";
    let (_, a) = parse(source).unwrap();
    assert!(a.nodes.iter().any(|d| matches!(d.node, Node::SwitchCase { .. })));
    assert!(parse("// @dart=2.19\nvar r = (1, 2);").is_err());
    assert!(parse("// @dart=3.5\nvar x = 1_000;").is_err());
}

#[test]
fn declaration_start_and_end_ignore_comments() {
    let source = "/// doc\n@a\nclass A {}\n// x\nvoid f() {}";
    let (t, a) = read(source);
    let Node::CompilationUnit { declarations, .. } = a.node(a.root.unwrap()) else {
        unreachable!()
    };
    assert_eq!(t.text(source, a.start(declarations[0])), "@");
    assert_eq!(t.text(source, a.start(declarations[1])), "void");
    assert_eq!(t.text(source, a.end(declarations[1])), "}");
}

#[test]
fn large_file_performance() {
    let mut source = String::new();
    for i in 0..2000 {
        source.push_str(&format!(
            "class C{i} extends B with M {{\n  final Map<String, List<int>> m = {{'a': [1, 2]}};\n  \
             int f(int x, {{String? y}}) => x > 0 ? (g(x - 1).h<int>()..k = 2) : -x;\n  \
             void g() {{ for (var e in m.entries) {{ if (e case MapEntry(:var key)) print('$key ${{e.value}}'); }} }}\n}}\n"
        ));
    }
    let start = std::time::Instant::now();
    let (t, a) = parse(&source).unwrap();
    let elapsed_time = start.elapsed();
    verify::check(&t, &a, &source).unwrap();
    // 10 thousand lines; the limit is generous for the unoptimized test profile.
    assert!(elapsed_time.as_secs_f64() < 5.0, "{elapsed_time:?}");
}
