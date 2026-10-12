//! Árvore sintática concreta no formato do AST do analyzer 3.6
//! (`package:analyzer/dart/ast/ast.dart`), que é o que o `dart_style`
//! percorre.
//!
//! Os nomes das variantes e dos campos são os das classes e getters do
//! analyzer (em `snake_case`), para que o porte do `SourceVisitor` possa ser
//! conferido linha a linha contra o original. Cada nó guarda os índices dos
//! seus tokens ([`Tk`]) e dos filhos ([`NodeId`]); o pai, o primeiro e o último
//! token ficam em [`NodeData`].
//!
//! Formas que o parser do analyzer (fasta + `AstBuilder`, sem resolução)
//! produz e que o formatador distingue:
//!
//! * `foo()` e `a.foo()` são [`Node::MethodInvocation`]; `a()()` e `(a)()` são
//!   [`Node::FunctionExpressionInvocation`]; `new A()`, `const A()` e
//!   `A<T>.nome()` são [`Node::InstanceCreationExpression`].
//! * `a.b` com `a` identificador simples é [`Node::PrefixedIdentifier`];
//!   qualquer outro acesso (`a.b.c`, `a?.b`, `(a).b`, `this.x`) é
//!   [`Node::PropertyAccess`].
//! * Numa cascata, cada seção é a expressão com o alvo vazio: `..a` é
//!   `PropertyAccess(target: None, operator: ..)`, `..f()` é
//!   `MethodInvocation(target: None)`, `..[0]` é `IndexExpression(target:
//!   None, period: ..)`.
//! * `List<int>` em posição de expressão é [`Node::FunctionReference`].
//! * Parâmetros dentro de `[...]`/`{...}` são sempre embrulhados em
//!   [`Node::DefaultFormalParameter`], com ou sem valor padrão.

use super::token::Tk;

/// Índice de um nó em [`Tree::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

/// Tipo de parâmetro formal, pela posição na lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    /// Posicional obrigatório.
    Required,
    /// Posicional opcional (`[...]`).
    OptionalPositional,
    /// Nomeado (`{...}`), obrigatório ou não.
    Named,
}

/// Um nó da árvore e suas ligações.
#[derive(Debug, Clone)]
pub struct NodeData {
    pub node: Node,
    pub parent: Option<NodeId>,
    /// `beginToken` do analyzer (sem o comentário de documentação).
    pub start: Tk,
    /// `endToken` do analyzer.
    pub end: Tk,
}

/// A árvore de uma unidade de compilação; a raiz é o último nó criado.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    pub nodes: Vec<NodeData>,
    pub root: Option<NodeId>,
}

impl Tree {
    /// O nó `id`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize].node
    }

    /// Os dados de ligação de `id`.
    pub fn data(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.0 as usize]
    }

    /// O pai de `id`.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0 as usize].parent
    }

    /// `beginToken`.
    pub fn start(&self, id: NodeId) -> Tk {
        self.nodes[id.0 as usize].start
    }

    /// `endToken`.
    pub fn end(&self, id: NodeId) -> Tk {
        self.nodes[id.0 as usize].end
    }

    /// Acrescenta um nó (sem pai ainda) e devolve seu índice.
    pub fn new(&mut self, node: Node, start: Tk, end: Tk) -> NodeId {
        self.nodes.push(NodeData {
            node,
            parent: None,
            start,
            end,
        });
        NodeId(self.nodes.len() as u32 - 1)
    }

    /// Preenche o pai de todos os nós a partir dos filhos de cada um.
    pub fn link_parents(&mut self) {
        for i in 0..self.nodes.len() {
            let mut children = Vec::new();
            self.nodes[i].node.children(&mut |f| children.push(f));
            for f in children {
                self.nodes[f.0 as usize].parent = Some(NodeId(i as u32));
            }
        }
    }
}

/// Os nós do AST do analyzer que o formatador visita.
///
/// Campos `Option<Tk>` são tokens opcionais; `Vec<NodeId>` são `NodeList`s.
/// `metadata` são as anotações ([`Node::Annotation`]) do nó.
#[derive(Debug, Clone)]
pub enum Node {
    // ---------------------------------------------------------------- unidade
    CompilationUnit {
        script_tag: Option<Tk>,
        directives: Vec<NodeId>,
        declarations: Vec<NodeId>,
    },

    // -------------------------------------------------------------- diretivas
    LibraryDirective {
        metadata: Vec<NodeId>,
        library_keyword: Tk,
        /// [`Node::LibraryIdentifier`]; ausente em `library;`.
        name2: Option<NodeId>,
        semicolon: Tk,
    },
    ImportDirective {
        metadata: Vec<NodeId>,
        import_keyword: Tk,
        /// Literal de string.
        uri: NodeId,
        configurations: Vec<NodeId>,
        deferred_keyword: Option<Tk>,
        as_keyword: Option<Tk>,
        /// [`Node::SimpleIdentifier`].
        prefix: Option<NodeId>,
        combinators: Vec<NodeId>,
        semicolon: Tk,
    },
    ExportDirective {
        metadata: Vec<NodeId>,
        export_keyword: Tk,
        uri: NodeId,
        configurations: Vec<NodeId>,
        combinators: Vec<NodeId>,
        semicolon: Tk,
    },
    PartDirective {
        metadata: Vec<NodeId>,
        part_keyword: Tk,
        uri: NodeId,
        semicolon: Tk,
    },
    PartOfDirective {
        metadata: Vec<NodeId>,
        part_keyword: Tk,
        of_keyword: Tk,
        /// [`Node::LibraryIdentifier`] em `part of a.b;`.
        library_name: Option<NodeId>,
        /// Literal de string em `part of 'a.dart';`.
        uri: Option<NodeId>,
        semicolon: Tk,
    },
    Configuration {
        if_keyword: Tk,
        left_parenthesis: Tk,
        /// [`Node::DottedName`].
        name: NodeId,
        equal_token: Option<Tk>,
        value: Option<NodeId>,
        right_parenthesis: Tk,
        uri: NodeId,
    },
    DottedName {
        /// [`Node::SimpleIdentifier`]s; os `.` ficam entre eles no fluxo.
        components: Vec<NodeId>,
    },
    LibraryIdentifier {
        components: Vec<NodeId>,
    },
    ShowCombinator {
        keyword: Tk,
        shown_names: Vec<NodeId>,
    },
    HideCombinator {
        keyword: Tk,
        hidden_names: Vec<NodeId>,
    },

    // ------------------------------------------------------------ declarações
    ClassDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        abstract_keyword: Option<Tk>,
        macro_keyword: Option<Tk>,
        sealed_keyword: Option<Tk>,
        base_keyword: Option<Tk>,
        interface_keyword: Option<Tk>,
        final_keyword: Option<Tk>,
        mixin_keyword: Option<Tk>,
        class_keyword: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        extends_clause: Option<NodeId>,
        with_clause: Option<NodeId>,
        implements_clause: Option<NodeId>,
        native_clause: Option<NodeId>,
        left_bracket: Tk,
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    ExtendsClause {
        extends_keyword: Tk,
        /// [`Node::NamedType`].
        superclass: NodeId,
    },
    WithClause {
        with_keyword: Tk,
        mixin_types: Vec<NodeId>,
    },
    ImplementsClause {
        implements_keyword: Tk,
        interfaces: Vec<NodeId>,
    },
    NativeClause {
        native_keyword: Tk,
        name: Option<NodeId>,
    },
    MixinDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        base_keyword: Option<Tk>,
        mixin_keyword: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        /// [`Node::MixinOnClause`].
        on_clause: Option<NodeId>,
        implements_clause: Option<NodeId>,
        left_bracket: Tk,
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    MixinOnClause {
        on_keyword: Tk,
        superclass_constraints: Vec<NodeId>,
    },
    ExtensionDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        extension_keyword: Tk,
        name: Option<Tk>,
        type_parameters: Option<NodeId>,
        /// [`Node::ExtensionOnClause`]; ausente só com erro.
        on_clause: Option<NodeId>,
        left_bracket: Tk,
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    ExtensionOnClause {
        on_keyword: Tk,
        extended_type: NodeId,
    },
    ExtensionTypeDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        extension_keyword: Tk,
        type_keyword: Tk,
        const_keyword: Option<Tk>,
        name: Tk,
        type_parameters: Option<NodeId>,
        /// [`Node::RepresentationDeclaration`].
        representation: NodeId,
        implements_clause: Option<NodeId>,
        left_bracket: Tk,
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    RepresentationDeclaration {
        /// [`Node::RepresentationConstructorName`].
        constructor_name: Option<NodeId>,
        left_parenthesis: Tk,
        field_metadata: Vec<NodeId>,
        field_type: NodeId,
        field_name: Tk,
        right_parenthesis: Tk,
    },
    RepresentationConstructorName {
        period: Tk,
        name: Tk,
    },
    EnumDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        enum_keyword: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        with_clause: Option<NodeId>,
        implements_clause: Option<NodeId>,
        left_bracket: Tk,
        /// [`Node::EnumConstantDeclaration`]s.
        constants: Vec<NodeId>,
        /// O `;` depois das constantes, se houver.
        semicolon: Option<Tk>,
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    EnumConstantDeclaration {
        metadata: Vec<NodeId>,
        name: Tk,
        /// [`Node::EnumConstantArguments`].
        arguments: Option<NodeId>,
    },
    EnumConstantArguments {
        type_arguments: Option<NodeId>,
        /// [`Node::ConstructorSelector`].
        constructor_selector: Option<NodeId>,
        argument_list: NodeId,
    },
    ConstructorSelector {
        period: Tk,
        /// [`Node::SimpleIdentifier`].
        name: NodeId,
    },
    /// `class A = B with C;` (o "typedef_keyword" do analyzer é o `class`).
    ClassTypeAlias {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        abstract_keyword: Option<Tk>,
        macro_keyword: Option<Tk>,
        sealed_keyword: Option<Tk>,
        base_keyword: Option<Tk>,
        interface_keyword: Option<Tk>,
        final_keyword: Option<Tk>,
        mixin_keyword: Option<Tk>,
        typedef_keyword: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        equals: Tk,
        superclass: NodeId,
        with_clause: Option<NodeId>,
        implements_clause: Option<NodeId>,
        semicolon: Tk,
    },
    /// `typedef int F(int a);` (sintaxe antiga).
    FunctionTypeAlias {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        typedef_keyword: Tk,
        return_type: Option<NodeId>,
        name: Tk,
        type_parameters: Option<NodeId>,
        parameters: NodeId,
        semicolon: Tk,
    },
    /// `typedef F = int Function(int);`
    GenericTypeAlias {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        typedef_keyword: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        equals: Tk,
        type_: NodeId,
        semicolon: Tk,
    },
    FunctionDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        return_type: Option<NodeId>,
        /// `get` ou `set`.
        property_keyword: Option<Tk>,
        name: Tk,
        /// [`Node::FunctionExpression`].
        function_expression: NodeId,
    },
    TopLevelVariableDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        /// [`Node::VariableDeclarationList`].
        variables: NodeId,
        semicolon: Tk,
    },

    // ---------------------------------------------------------------- membros
    ConstructorDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        const_keyword: Option<Tk>,
        factory_keyword: Option<Tk>,
        /// [`Node::SimpleIdentifier`] com o nome da classe.
        return_type: NodeId,
        period: Option<Tk>,
        name: Option<Tk>,
        parameters: NodeId,
        /// `:` antes dos inicializadores ou `=` do redirecionamento.
        separator: Option<Tk>,
        initializers: Vec<NodeId>,
        /// [`Node::ConstructorName`] de `factory A() = B;`.
        redirected_constructor: Option<NodeId>,
        body: NodeId,
    },
    MethodDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        /// `static` ou `abstract`.
        modifier_keyword: Option<Tk>,
        return_type: Option<NodeId>,
        property_keyword: Option<Tk>,
        operator_keyword: Option<Tk>,
        name: Tk,
        type_parameters: Option<NodeId>,
        /// Ausente em getters.
        parameters: Option<NodeId>,
        body: NodeId,
    },
    FieldDeclaration {
        metadata: Vec<NodeId>,
        augment_keyword: Option<Tk>,
        abstract_keyword: Option<Tk>,
        covariant_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        static_keyword: Option<Tk>,
        /// [`Node::VariableDeclarationList`].
        fields: NodeId,
        semicolon: Tk,
    },
    ConstructorFieldInitializer {
        this_keyword: Option<Tk>,
        period: Option<Tk>,
        /// [`Node::SimpleIdentifier`].
        field_name: NodeId,
        equals: Tk,
        expression: NodeId,
    },
    SuperConstructorInvocation {
        super_keyword: Tk,
        period: Option<Tk>,
        constructor_name: Option<NodeId>,
        argument_list: NodeId,
    },
    RedirectingConstructorInvocation {
        this_keyword: Tk,
        period: Option<Tk>,
        constructor_name: Option<NodeId>,
        argument_list: NodeId,
    },
    AssertInitializer {
        assert_keyword: Tk,
        left_parenthesis: Tk,
        condition: NodeId,
        comma: Option<Tk>,
        message: Option<NodeId>,
        right_parenthesis: Tk,
    },

    // ------------------------------------------------------------- corpos
    BlockFunctionBody {
        /// `async` ou `sync`.
        keyword: Option<Tk>,
        star: Option<Tk>,
        /// [`Node::Block`].
        block: NodeId,
    },
    ExpressionFunctionBody {
        keyword: Option<Tk>,
        star: Option<Tk>,
        function_definition: Tk,
        expression: NodeId,
        semicolon: Option<Tk>,
    },
    EmptyFunctionBody {
        semicolon: Tk,
    },
    NativeFunctionBody {
        native_keyword: Tk,
        string_literal: Option<NodeId>,
        semicolon: Tk,
    },

    // ----------------------------------------------------------- parâmetros
    FormalParameterList {
        left_parenthesis: Tk,
        parameters: Vec<NodeId>,
        left_delimiter: Option<Tk>,
        right_delimiter: Option<Tk>,
        right_parenthesis: Tk,
    },
    SimpleFormalParameter {
        metadata: Vec<NodeId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        /// `final`, `const` ou `var`.
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        name: Option<Tk>,
        kind: ParameterKind,
    },
    FieldFormalParameter {
        metadata: Vec<NodeId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        this_keyword: Tk,
        period: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        parameters: Option<NodeId>,
        question: Option<Tk>,
        kind: ParameterKind,
    },
    SuperFormalParameter {
        metadata: Vec<NodeId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        super_keyword: Tk,
        period: Tk,
        name: Tk,
        type_parameters: Option<NodeId>,
        parameters: Option<NodeId>,
        question: Option<Tk>,
        kind: ParameterKind,
    },
    FunctionTypedFormalParameter {
        metadata: Vec<NodeId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        return_type: Option<NodeId>,
        name: Tk,
        type_parameters: Option<NodeId>,
        parameters: NodeId,
        question: Option<Tk>,
        kind: ParameterKind,
    },
    DefaultFormalParameter {
        parameter: NodeId,
        /// `=` ou `:`.
        separator: Option<Tk>,
        default_value: Option<NodeId>,
        kind: ParameterKind,
    },

    // ---------------------------------------------------------------- tipos
    NamedType {
        /// `prefix` e `.` de `p.Type`.
        import_prefix: Option<(Tk, Tk)>,
        name2: Tk,
        type_arguments: Option<NodeId>,
        question: Option<Tk>,
    },
    GenericFunctionType {
        return_type: Option<NodeId>,
        function_keyword: Tk,
        type_parameters: Option<NodeId>,
        parameters: NodeId,
        question: Option<Tk>,
    },
    RecordTypeAnnotation {
        left_parenthesis: Tk,
        positional_fields: Vec<NodeId>,
        /// [`Node::RecordTypeAnnotationNamedFields`].
        named_fields: Option<NodeId>,
        right_parenthesis: Tk,
        question: Option<Tk>,
    },
    RecordTypeAnnotationNamedFields {
        left_bracket: Tk,
        fields: Vec<NodeId>,
        right_bracket: Tk,
    },
    RecordTypeAnnotationPositionalField {
        metadata: Vec<NodeId>,
        type_: NodeId,
        name: Option<Tk>,
    },
    RecordTypeAnnotationNamedField {
        metadata: Vec<NodeId>,
        type_: NodeId,
        name: Tk,
    },
    TypeArgumentList {
        left_bracket: Tk,
        arguments: Vec<NodeId>,
        right_bracket: Tk,
    },
    TypeParameterList {
        left_bracket: Tk,
        type_parameters: Vec<NodeId>,
        right_bracket: Tk,
    },
    TypeParameter {
        metadata: Vec<NodeId>,
        name: Tk,
        extends_keyword: Option<Tk>,
        bound: Option<NodeId>,
    },

    // ----------------------------------------------------------- anotações
    Annotation {
        at_sign: Tk,
        /// [`Node::SimpleIdentifier`] ou [`Node::PrefixedIdentifier`].
        name: NodeId,
        type_arguments: Option<NodeId>,
        period: Option<Tk>,
        constructor_name: Option<NodeId>,
        arguments: Option<NodeId>,
    },

    // ----------------------------------------------------------- instruções
    Block {
        left_bracket: Tk,
        statements: Vec<NodeId>,
        right_bracket: Tk,
    },
    ExpressionStatement {
        expression: NodeId,
        /// Ausente só no corpo `=>`... nunca em código válido.
        semicolon: Option<Tk>,
    },
    VariableDeclarationStatement {
        variables: NodeId,
        semicolon: Tk,
    },
    VariableDeclarationList {
        /// Anotações de declarações locais; vazio em campos e variáveis de
        /// topo (lá elas ficam na declaração).
        metadata: Vec<NodeId>,
        late_keyword: Option<Tk>,
        /// `final`, `const` ou `var`.
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        /// [`Node::VariableDeclaration`]s.
        variables: Vec<NodeId>,
    },
    VariableDeclaration {
        name: Tk,
        equals: Option<Tk>,
        initializer: Option<NodeId>,
    },
    FunctionDeclarationStatement {
        function_declaration: NodeId,
    },
    IfStatement {
        if_keyword: Tk,
        left_parenthesis: Tk,
        expression: NodeId,
        /// [`Node::CaseClause`] de `if (x case P)`.
        case_clause: Option<NodeId>,
        right_parenthesis: Tk,
        then_statement: NodeId,
        else_keyword: Option<Tk>,
        else_statement: Option<NodeId>,
    },
    CaseClause {
        case_keyword: Tk,
        /// [`Node::GuardedPattern`].
        guarded_pattern: NodeId,
    },
    GuardedPattern {
        pattern: NodeId,
        /// [`Node::WhenClause`].
        when_clause: Option<NodeId>,
    },
    WhenClause {
        when_keyword: Tk,
        expression: NodeId,
    },
    ForStatement {
        await_keyword: Option<Tk>,
        for_keyword: Tk,
        left_parenthesis: Tk,
        for_loop_parts: NodeId,
        right_parenthesis: Tk,
        body: NodeId,
    },
    ForPartsWithDeclarations {
        /// [`Node::VariableDeclarationList`].
        variables: NodeId,
        left_separator: Tk,
        condition: Option<NodeId>,
        right_separator: Tk,
        updaters: Vec<NodeId>,
    },
    ForPartsWithExpression {
        initialization: Option<NodeId>,
        left_separator: Tk,
        condition: Option<NodeId>,
        right_separator: Tk,
        updaters: Vec<NodeId>,
    },
    ForPartsWithPattern {
        /// [`Node::PatternVariableDeclaration`].
        variables: NodeId,
        left_separator: Tk,
        condition: Option<NodeId>,
        right_separator: Tk,
        updaters: Vec<NodeId>,
    },
    ForEachPartsWithDeclaration {
        /// [`Node::DeclaredIdentifier`].
        loop_variable: NodeId,
        in_keyword: Tk,
        iterable: NodeId,
    },
    ForEachPartsWithIdentifier {
        /// [`Node::SimpleIdentifier`].
        identifier: NodeId,
        in_keyword: Tk,
        iterable: NodeId,
    },
    ForEachPartsWithPattern {
        metadata: Vec<NodeId>,
        /// `var` ou `final`.
        keyword: Tk,
        pattern: NodeId,
        in_keyword: Tk,
        iterable: NodeId,
    },
    DeclaredIdentifier {
        metadata: Vec<NodeId>,
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        name: Tk,
    },
    WhileStatement {
        while_keyword: Tk,
        left_parenthesis: Tk,
        condition: NodeId,
        right_parenthesis: Tk,
        body: NodeId,
    },
    DoStatement {
        do_keyword: Tk,
        body: NodeId,
        while_keyword: Tk,
        left_parenthesis: Tk,
        condition: NodeId,
        right_parenthesis: Tk,
        semicolon: Tk,
    },
    SwitchStatement {
        switch_keyword: Tk,
        left_parenthesis: Tk,
        expression: NodeId,
        right_parenthesis: Tk,
        left_bracket: Tk,
        /// [`Node::SwitchCase`], [`Node::SwitchPatternCase`] e
        /// [`Node::SwitchDefault`].
        members: Vec<NodeId>,
        right_bracket: Tk,
    },
    /// `case expr:` em biblioteca sem padrões (não ocorre em 3.6).
    SwitchCase {
        labels: Vec<NodeId>,
        keyword: Tk,
        expression: NodeId,
        colon: Tk,
        statements: Vec<NodeId>,
    },
    SwitchPatternCase {
        labels: Vec<NodeId>,
        keyword: Tk,
        guarded_pattern: NodeId,
        colon: Tk,
        statements: Vec<NodeId>,
    },
    SwitchDefault {
        labels: Vec<NodeId>,
        keyword: Tk,
        colon: Tk,
        statements: Vec<NodeId>,
    },
    TryStatement {
        try_keyword: Tk,
        body: NodeId,
        catch_clauses: Vec<NodeId>,
        finally_keyword: Option<Tk>,
        finally_block: Option<NodeId>,
    },
    CatchClause {
        on_keyword: Option<Tk>,
        exception_type: Option<NodeId>,
        catch_keyword: Option<Tk>,
        left_parenthesis: Option<Tk>,
        /// [`Node::CatchClauseParameter`].
        exception_parameter: Option<NodeId>,
        comma: Option<Tk>,
        stack_trace_parameter: Option<NodeId>,
        right_parenthesis: Option<Tk>,
        body: NodeId,
    },
    CatchClauseParameter {
        name: Tk,
    },
    ReturnStatement {
        return_keyword: Tk,
        expression: Option<NodeId>,
        semicolon: Tk,
    },
    BreakStatement {
        break_keyword: Tk,
        /// [`Node::SimpleIdentifier`].
        label: Option<NodeId>,
        semicolon: Tk,
    },
    ContinueStatement {
        continue_keyword: Tk,
        label: Option<NodeId>,
        semicolon: Tk,
    },
    YieldStatement {
        yield_keyword: Tk,
        star: Option<Tk>,
        expression: NodeId,
        semicolon: Tk,
    },
    LabeledStatement {
        /// [`Node::Label`]s.
        labels: Vec<NodeId>,
        statement: NodeId,
    },
    Label {
        /// [`Node::SimpleIdentifier`].
        label: NodeId,
        colon: Tk,
    },
    AssertStatement {
        assert_keyword: Tk,
        left_parenthesis: Tk,
        condition: NodeId,
        comma: Option<Tk>,
        message: Option<NodeId>,
        right_parenthesis: Tk,
        semicolon: Tk,
    },
    EmptyStatement {
        semicolon: Tk,
    },
    PatternVariableDeclarationStatement {
        /// [`Node::PatternVariableDeclaration`].
        declaration: NodeId,
        semicolon: Tk,
    },
    PatternVariableDeclaration {
        metadata: Vec<NodeId>,
        /// `var` ou `final`.
        keyword: Tk,
        pattern: NodeId,
        equals: Tk,
        expression: NodeId,
    },

    // ---------------------------------------------------------- expressões
    AdjacentStrings {
        strings: Vec<NodeId>,
    },
    AsExpression {
        expression: NodeId,
        as_operator: Tk,
        type_: NodeId,
    },
    AssignmentExpression {
        left_hand_side: NodeId,
        operator: Tk,
        right_hand_side: NodeId,
    },
    AwaitExpression {
        await_keyword: Tk,
        expression: NodeId,
    },
    BinaryExpression {
        left_operand: NodeId,
        operator: Tk,
        right_operand: NodeId,
    },
    BooleanLiteral {
        literal: Tk,
    },
    CascadeExpression {
        target: NodeId,
        cascade_sections: Vec<NodeId>,
    },
    ConditionalExpression {
        condition: NodeId,
        question: Tk,
        then_expression: NodeId,
        colon: Tk,
        else_expression: NodeId,
    },
    DoubleLiteral {
        literal: Tk,
    },
    IntegerLiteral {
        literal: Tk,
    },
    NullLiteral {
        literal: Tk,
    },
    FunctionExpression {
        type_parameters: Option<NodeId>,
        parameters: Option<NodeId>,
        body: NodeId,
    },
    FunctionExpressionInvocation {
        function: NodeId,
        type_arguments: Option<NodeId>,
        argument_list: NodeId,
    },
    FunctionReference {
        function: NodeId,
        type_arguments: Option<NodeId>,
    },
    IndexExpression {
        target: Option<NodeId>,
        /// `..` ou `?..` quando é seção de cascata.
        period: Option<Tk>,
        /// `?` de `a?[i]`.
        question: Option<Tk>,
        left_bracket: Tk,
        index: NodeId,
        right_bracket: Tk,
    },
    InstanceCreationExpression {
        /// `new` ou `const`.
        keyword: Option<Tk>,
        /// [`Node::ConstructorName`].
        constructor_name: NodeId,
        argument_list: NodeId,
    },
    ConstructorName {
        /// [`Node::NamedType`].
        type_: NodeId,
        period: Option<Tk>,
        /// [`Node::SimpleIdentifier`].
        name: Option<NodeId>,
    },
    IsExpression {
        expression: NodeId,
        is_operator: Tk,
        not_operator: Option<Tk>,
        type_: NodeId,
    },
    ListLiteral {
        const_keyword: Option<Tk>,
        type_arguments: Option<NodeId>,
        left_bracket: Tk,
        elements: Vec<NodeId>,
        right_bracket: Tk,
    },
    SetOrMapLiteral {
        const_keyword: Option<Tk>,
        type_arguments: Option<NodeId>,
        left_bracket: Tk,
        elements: Vec<NodeId>,
        right_bracket: Tk,
    },
    MapLiteralEntry {
        key: NodeId,
        separator: Tk,
        value: NodeId,
    },
    MethodInvocation {
        target: Option<NodeId>,
        /// `.`, `?.`, `..` ou `?..`.
        operator: Option<Tk>,
        /// [`Node::SimpleIdentifier`].
        method_name: NodeId,
        type_arguments: Option<NodeId>,
        argument_list: NodeId,
    },
    NamedExpression {
        /// [`Node::Label`].
        name: NodeId,
        expression: NodeId,
    },
    ParenthesizedExpression {
        left_parenthesis: Tk,
        expression: NodeId,
        right_parenthesis: Tk,
    },
    PostfixExpression {
        operand: NodeId,
        operator: Tk,
    },
    PrefixExpression {
        operator: Tk,
        operand: NodeId,
    },
    PrefixedIdentifier {
        /// [`Node::SimpleIdentifier`].
        prefix: NodeId,
        period: Tk,
        /// [`Node::SimpleIdentifier`].
        identifier: NodeId,
    },
    PropertyAccess {
        target: Option<NodeId>,
        operator: Tk,
        /// [`Node::SimpleIdentifier`].
        property_name: NodeId,
    },
    RecordLiteral {
        const_keyword: Option<Tk>,
        left_parenthesis: Tk,
        fields: Vec<NodeId>,
        right_parenthesis: Tk,
    },
    RethrowExpression {
        rethrow_keyword: Tk,
    },
    SimpleIdentifier {
        token: Tk,
    },
    SimpleStringLiteral {
        literal: Tk,
    },
    StringInterpolation {
        /// [`Node::InterpolationString`] e [`Node::InterpolationExpression`]
        /// alternados, começando e terminando em texto.
        elements: Vec<NodeId>,
    },
    InterpolationExpression {
        /// `${` ou `$`.
        left_bracket: Tk,
        expression: NodeId,
        right_bracket: Option<Tk>,
    },
    InterpolationString {
        contents: Tk,
    },
    SuperExpression {
        super_keyword: Tk,
    },
    ThisExpression {
        this_keyword: Tk,
    },
    SymbolLiteral {
        pound_sign: Tk,
        /// Os componentes; os `.` ficam entre eles no fluxo.
        components: Vec<Tk>,
    },
    ThrowExpression {
        throw_keyword: Tk,
        expression: NodeId,
    },
    SwitchExpression {
        switch_keyword: Tk,
        left_parenthesis: Tk,
        expression: NodeId,
        right_parenthesis: Tk,
        left_bracket: Tk,
        /// [`Node::SwitchExpressionCase`]s.
        cases: Vec<NodeId>,
        right_bracket: Tk,
    },
    SwitchExpressionCase {
        guarded_pattern: NodeId,
        arrow: Tk,
        expression: NodeId,
    },
    ArgumentList {
        left_parenthesis: Tk,
        arguments: Vec<NodeId>,
        right_parenthesis: Tk,
    },

    // ------------------------------------------------- elementos de coleção
    SpreadElement {
        /// `...` ou `...?`.
        spread_operator: Tk,
        expression: NodeId,
    },
    IfElement {
        if_keyword: Tk,
        left_parenthesis: Tk,
        expression: NodeId,
        case_clause: Option<NodeId>,
        right_parenthesis: Tk,
        then_element: NodeId,
        else_keyword: Option<Tk>,
        else_element: Option<NodeId>,
    },
    ForElement {
        await_keyword: Option<Tk>,
        for_keyword: Tk,
        left_parenthesis: Tk,
        for_loop_parts: NodeId,
        right_parenthesis: Tk,
        body: NodeId,
    },

    // -------------------------------------------------------------- padrões
    AssignedVariablePattern {
        name: Tk,
    },
    CastPattern {
        pattern: NodeId,
        as_token: Tk,
        type_: NodeId,
    },
    ConstantPattern {
        const_keyword: Option<Tk>,
        expression: NodeId,
    },
    DeclaredVariablePattern {
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        name: Tk,
    },
    ListPattern {
        type_arguments: Option<NodeId>,
        left_bracket: Tk,
        elements: Vec<NodeId>,
        right_bracket: Tk,
    },
    LogicalAndPattern {
        left_operand: NodeId,
        operator: Tk,
        right_operand: NodeId,
    },
    LogicalOrPattern {
        left_operand: NodeId,
        operator: Tk,
        right_operand: NodeId,
    },
    MapPattern {
        type_arguments: Option<NodeId>,
        left_bracket: Tk,
        elements: Vec<NodeId>,
        right_bracket: Tk,
    },
    MapPatternEntry {
        key: NodeId,
        separator: Tk,
        value: NodeId,
    },
    NullAssertPattern {
        pattern: NodeId,
        operator: Tk,
    },
    NullCheckPattern {
        pattern: NodeId,
        operator: Tk,
    },
    ObjectPattern {
        /// [`Node::NamedType`].
        type_: NodeId,
        left_parenthesis: Tk,
        /// [`Node::PatternField`]s.
        fields: Vec<NodeId>,
        right_parenthesis: Tk,
    },
    ParenthesizedPattern {
        left_parenthesis: Tk,
        pattern: NodeId,
        right_parenthesis: Tk,
    },
    PatternField {
        /// [`Node::PatternFieldName`].
        name: Option<NodeId>,
        pattern: NodeId,
    },
    PatternFieldName {
        name: Option<Tk>,
        colon: Tk,
    },
    RecordPattern {
        left_parenthesis: Tk,
        fields: Vec<NodeId>,
        right_parenthesis: Tk,
    },
    RelationalPattern {
        operator: Tk,
        operand: NodeId,
    },
    RestPatternElement {
        operator: Tk,
        pattern: Option<NodeId>,
    },
    WildcardPattern {
        keyword: Option<Tk>,
        type_: Option<NodeId>,
        name: Tk,
    },
    PatternAssignment {
        pattern: NodeId,
        equals: Tk,
        expression: NodeId,
    },
}

impl Node {
    /// Chama `f` para cada filho, na ordem do texto.
    pub fn children(&self, f: &mut dyn FnMut(NodeId)) {
        let one = |o: &Option<NodeId>, f: &mut dyn FnMut(NodeId)| {
            if let Some(n) = o {
                f(*n);
            }
        };
        let all = |v: &Vec<NodeId>, f: &mut dyn FnMut(NodeId)| {
            for n in v {
                f(*n);
            }
        };
        use Node::*;
        match self {
            CompilationUnit { directives, declarations, .. } => {
                all(directives, f);
                all(declarations, f);
            }
            LibraryDirective { metadata, name2, .. } => {
                all(metadata, f);
                one(name2, f);
            }
            ImportDirective {
                metadata,
                uri,
                configurations,
                prefix,
                combinators,
                ..
            } => {
                all(metadata, f);
                f(*uri);
                all(configurations, f);
                one(prefix, f);
                all(combinators, f);
            }
            ExportDirective {
                metadata,
                uri,
                configurations,
                combinators,
                ..
            } => {
                all(metadata, f);
                f(*uri);
                all(configurations, f);
                all(combinators, f);
            }
            PartDirective { metadata, uri, .. } => {
                all(metadata, f);
                f(*uri);
            }
            PartOfDirective {
                metadata,
                library_name,
                uri,
                ..
            } => {
                all(metadata, f);
                one(library_name, f);
                one(uri, f);
            }
            Configuration { name, value, uri, .. } => {
                f(*name);
                one(value, f);
                f(*uri);
            }
            DottedName { components } | LibraryIdentifier { components } => all(components, f),
            ShowCombinator { shown_names: v, .. } | HideCombinator { hidden_names: v, .. } => all(v, f),
            ClassDeclaration {
                metadata,
                type_parameters,
                extends_clause,
                with_clause,
                implements_clause,
                native_clause,
                members,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                one(extends_clause, f);
                one(with_clause, f);
                one(implements_clause, f);
                one(native_clause, f);
                all(members, f);
            }
            ExtendsClause { superclass, .. } => f(*superclass),
            WithClause { mixin_types: v, .. }
            | ImplementsClause { interfaces: v, .. }
            | MixinOnClause {
                superclass_constraints: v,
                ..
            } => all(v, f),
            NativeClause { name, .. } => one(name, f),
            MixinDeclaration {
                metadata,
                type_parameters,
                on_clause,
                implements_clause,
                members,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                one(on_clause, f);
                one(implements_clause, f);
                all(members, f);
            }
            ExtensionDeclaration {
                metadata,
                type_parameters,
                on_clause,
                members,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                one(on_clause, f);
                all(members, f);
            }
            ExtensionOnClause { extended_type, .. } => f(*extended_type),
            ExtensionTypeDeclaration {
                metadata,
                type_parameters,
                representation,
                implements_clause,
                members,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                f(*representation);
                one(implements_clause, f);
                all(members, f);
            }
            RepresentationDeclaration {
                constructor_name,
                field_metadata,
                field_type,
                ..
            } => {
                one(constructor_name, f);
                all(field_metadata, f);
                f(*field_type);
            }
            RepresentationConstructorName { .. } => {}
            EnumDeclaration {
                metadata,
                type_parameters,
                with_clause,
                implements_clause,
                constants,
                members,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                one(with_clause, f);
                one(implements_clause, f);
                all(constants, f);
                all(members, f);
            }
            EnumConstantDeclaration { metadata, arguments, .. } => {
                all(metadata, f);
                one(arguments, f);
            }
            EnumConstantArguments {
                type_arguments,
                constructor_selector,
                argument_list,
            } => {
                one(type_arguments, f);
                one(constructor_selector, f);
                f(*argument_list);
            }
            ConstructorSelector { name, .. } => f(*name),
            ClassTypeAlias {
                metadata,
                type_parameters,
                superclass,
                with_clause,
                implements_clause,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                f(*superclass);
                one(with_clause, f);
                one(implements_clause, f);
            }
            FunctionTypeAlias {
                metadata,
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                all(metadata, f);
                one(return_type, f);
                one(type_parameters, f);
                f(*parameters);
            }
            GenericTypeAlias {
                metadata,
                type_parameters,
                type_,
                ..
            } => {
                all(metadata, f);
                one(type_parameters, f);
                f(*type_);
            }
            FunctionDeclaration {
                metadata,
                return_type,
                function_expression,
                ..
            } => {
                all(metadata, f);
                one(return_type, f);
                f(*function_expression);
            }
            TopLevelVariableDeclaration { metadata, variables, .. } => {
                all(metadata, f);
                f(*variables);
            }
            ConstructorDeclaration {
                metadata,
                return_type,
                parameters,
                initializers,
                redirected_constructor,
                body,
                ..
            } => {
                all(metadata, f);
                f(*return_type);
                f(*parameters);
                all(initializers, f);
                one(redirected_constructor, f);
                f(*body);
            }
            MethodDeclaration {
                metadata,
                return_type,
                type_parameters,
                parameters,
                body,
                ..
            } => {
                all(metadata, f);
                one(return_type, f);
                one(type_parameters, f);
                one(parameters, f);
                f(*body);
            }
            FieldDeclaration { metadata, fields, .. } => {
                all(metadata, f);
                f(*fields);
            }
            ConstructorFieldInitializer { field_name, expression, .. } => {
                f(*field_name);
                f(*expression);
            }
            SuperConstructorInvocation {
                constructor_name,
                argument_list,
                ..
            }
            | RedirectingConstructorInvocation {
                constructor_name,
                argument_list,
                ..
            } => {
                one(constructor_name, f);
                f(*argument_list);
            }
            AssertInitializer { condition, message, .. } | AssertStatement { condition, message, .. } => {
                f(*condition);
                one(message, f);
            }
            BlockFunctionBody { block, .. } => f(*block),
            ExpressionFunctionBody { expression, .. } => f(*expression),
            EmptyFunctionBody { .. } => {}
            NativeFunctionBody { string_literal, .. } => one(string_literal, f),
            FormalParameterList { parameters, .. } => all(parameters, f),
            SimpleFormalParameter { metadata, type_, .. } => {
                all(metadata, f);
                one(type_, f);
            }
            FieldFormalParameter {
                metadata,
                type_,
                type_parameters,
                parameters,
                ..
            }
            | SuperFormalParameter {
                metadata,
                type_,
                type_parameters,
                parameters,
                ..
            } => {
                all(metadata, f);
                one(type_, f);
                one(type_parameters, f);
                one(parameters, f);
            }
            FunctionTypedFormalParameter {
                metadata,
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                all(metadata, f);
                one(return_type, f);
                one(type_parameters, f);
                f(*parameters);
            }
            DefaultFormalParameter {
                parameter,
                default_value,
                ..
            } => {
                f(*parameter);
                one(default_value, f);
            }
            NamedType { type_arguments, .. } => one(type_arguments, f),
            GenericFunctionType {
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                one(return_type, f);
                one(type_parameters, f);
                f(*parameters);
            }
            RecordTypeAnnotation {
                positional_fields,
                named_fields,
                ..
            } => {
                all(positional_fields, f);
                one(named_fields, f);
            }
            RecordTypeAnnotationNamedFields { fields, .. } => all(fields, f),
            RecordTypeAnnotationPositionalField { metadata, type_, .. }
            | RecordTypeAnnotationNamedField { metadata, type_, .. } => {
                all(metadata, f);
                f(*type_);
            }
            TypeArgumentList { arguments: v, .. }
            | TypeParameterList {
                type_parameters: v, ..
            } => all(v, f),
            TypeParameter { metadata, bound, .. } => {
                all(metadata, f);
                one(bound, f);
            }
            Annotation {
                name,
                type_arguments,
                constructor_name,
                arguments,
                ..
            } => {
                f(*name);
                one(type_arguments, f);
                one(constructor_name, f);
                one(arguments, f);
            }
            Block { statements, .. } => all(statements, f),
            ExpressionStatement { expression, .. } => f(*expression),
            VariableDeclarationStatement { variables, .. } => f(*variables),
            VariableDeclarationList {
                metadata,
                type_,
                variables,
                ..
            } => {
                all(metadata, f);
                one(type_, f);
                all(variables, f);
            }
            VariableDeclaration { initializer, .. } => one(initializer, f),
            FunctionDeclarationStatement { function_declaration } => f(*function_declaration),
            IfStatement {
                expression,
                case_clause,
                then_statement,
                else_statement,
                ..
            } => {
                f(*expression);
                one(case_clause, f);
                f(*then_statement);
                one(else_statement, f);
            }
            CaseClause { guarded_pattern, .. } => f(*guarded_pattern),
            GuardedPattern { pattern, when_clause } => {
                f(*pattern);
                one(when_clause, f);
            }
            WhenClause { expression, .. } => f(*expression),
            ForStatement {
                for_loop_parts, body, ..
            }
            | ForElement {
                for_loop_parts, body, ..
            } => {
                f(*for_loop_parts);
                f(*body);
            }
            ForPartsWithDeclarations {
                variables,
                condition,
                updaters,
                ..
            }
            | ForPartsWithPattern {
                variables,
                condition,
                updaters,
                ..
            } => {
                f(*variables);
                one(condition, f);
                all(updaters, f);
            }
            ForPartsWithExpression {
                initialization,
                condition,
                updaters,
                ..
            } => {
                one(initialization, f);
                one(condition, f);
                all(updaters, f);
            }
            ForEachPartsWithDeclaration {
                loop_variable,
                iterable,
                ..
            } => {
                f(*loop_variable);
                f(*iterable);
            }
            ForEachPartsWithIdentifier { identifier, iterable, .. } => {
                f(*identifier);
                f(*iterable);
            }
            ForEachPartsWithPattern {
                metadata,
                pattern,
                iterable,
                ..
            } => {
                all(metadata, f);
                f(*pattern);
                f(*iterable);
            }
            DeclaredIdentifier { metadata, type_, .. } => {
                all(metadata, f);
                one(type_, f);
            }
            WhileStatement { condition, body, .. } => {
                f(*condition);
                f(*body);
            }
            DoStatement { body, condition, .. } => {
                f(*body);
                f(*condition);
            }
            SwitchStatement { expression, members, .. } => {
                f(*expression);
                all(members, f);
            }
            SwitchCase {
                labels,
                expression,
                statements,
                ..
            } => {
                all(labels, f);
                f(*expression);
                all(statements, f);
            }
            SwitchPatternCase {
                labels,
                guarded_pattern,
                statements,
                ..
            } => {
                all(labels, f);
                f(*guarded_pattern);
                all(statements, f);
            }
            SwitchDefault { labels, statements, .. } => {
                all(labels, f);
                all(statements, f);
            }
            TryStatement {
                body,
                catch_clauses,
                finally_block,
                ..
            } => {
                f(*body);
                all(catch_clauses, f);
                one(finally_block, f);
            }
            CatchClause {
                exception_type,
                exception_parameter,
                stack_trace_parameter,
                body,
                ..
            } => {
                one(exception_type, f);
                one(exception_parameter, f);
                one(stack_trace_parameter, f);
                f(*body);
            }
            CatchClauseParameter { .. } => {}
            ReturnStatement { expression, .. } => one(expression, f),
            BreakStatement { label, .. } | ContinueStatement { label, .. } => one(label, f),
            YieldStatement { expression, .. } => f(*expression),
            LabeledStatement { labels, statement } => {
                all(labels, f);
                f(*statement);
            }
            Label { label, .. } => f(*label),
            EmptyStatement { .. } => {}
            PatternVariableDeclarationStatement { declaration, .. } => f(*declaration),
            PatternVariableDeclaration {
                metadata,
                pattern,
                expression,
                ..
            } => {
                all(metadata, f);
                f(*pattern);
                f(*expression);
            }
            AdjacentStrings { strings } => all(strings, f),
            AsExpression { expression, type_, .. } | IsExpression { expression, type_, .. } => {
                f(*expression);
                f(*type_);
            }
            AssignmentExpression {
                left_hand_side,
                right_hand_side,
                ..
            } => {
                f(*left_hand_side);
                f(*right_hand_side);
            }
            AwaitExpression { expression, .. } | ThrowExpression { expression, .. } => f(*expression),
            BinaryExpression {
                left_operand,
                right_operand,
                ..
            }
            | LogicalAndPattern {
                left_operand,
                right_operand,
                ..
            }
            | LogicalOrPattern {
                left_operand,
                right_operand,
                ..
            } => {
                f(*left_operand);
                f(*right_operand);
            }
            BooleanLiteral { .. } | DoubleLiteral { .. } | IntegerLiteral { .. } | NullLiteral { .. } => {}
            CascadeExpression {
                target,
                cascade_sections,
            } => {
                f(*target);
                all(cascade_sections, f);
            }
            ConditionalExpression {
                condition,
                then_expression,
                else_expression,
                ..
            } => {
                f(*condition);
                f(*then_expression);
                f(*else_expression);
            }
            FunctionExpression {
                type_parameters,
                parameters,
                body,
            } => {
                one(type_parameters, f);
                one(parameters, f);
                f(*body);
            }
            FunctionExpressionInvocation {
                function,
                type_arguments,
                argument_list,
            } => {
                f(*function);
                one(type_arguments, f);
                f(*argument_list);
            }
            FunctionReference {
                function,
                type_arguments,
            } => {
                f(*function);
                one(type_arguments, f);
            }
            IndexExpression { target, index, .. } => {
                one(target, f);
                f(*index);
            }
            InstanceCreationExpression {
                constructor_name,
                argument_list,
                ..
            } => {
                f(*constructor_name);
                f(*argument_list);
            }
            ConstructorName { type_, name, .. } => {
                f(*type_);
                one(name, f);
            }
            ListLiteral {
                type_arguments,
                elements,
                ..
            }
            | SetOrMapLiteral {
                type_arguments,
                elements,
                ..
            }
            | ListPattern {
                type_arguments,
                elements,
                ..
            }
            | MapPattern {
                type_arguments,
                elements,
                ..
            } => {
                one(type_arguments, f);
                all(elements, f);
            }
            MapLiteralEntry { key, value, .. } | MapPatternEntry { key, value, .. } => {
                f(*key);
                f(*value);
            }
            MethodInvocation {
                target,
                method_name,
                type_arguments,
                argument_list,
                ..
            } => {
                one(target, f);
                f(*method_name);
                one(type_arguments, f);
                f(*argument_list);
            }
            NamedExpression { name, expression } => {
                f(*name);
                f(*expression);
            }
            ParenthesizedExpression { expression, .. } => f(*expression),
            PostfixExpression { operand, .. } | PrefixExpression { operand, .. } => f(*operand),
            PrefixedIdentifier { prefix, identifier, .. } => {
                f(*prefix);
                f(*identifier);
            }
            PropertyAccess {
                target, property_name, ..
            } => {
                one(target, f);
                f(*property_name);
            }
            RecordLiteral { fields, .. } | RecordPattern { fields, .. } => all(fields, f),
            RethrowExpression { .. } | SimpleIdentifier { .. } | SimpleStringLiteral { .. } => {}
            StringInterpolation { elements } => all(elements, f),
            InterpolationExpression { expression, .. } => f(*expression),
            InterpolationString { .. } | SuperExpression { .. } | ThisExpression { .. } | SymbolLiteral { .. } => {}
            SwitchExpression { expression, cases, .. } => {
                f(*expression);
                all(cases, f);
            }
            SwitchExpressionCase {
                guarded_pattern,
                expression,
                ..
            } => {
                f(*guarded_pattern);
                f(*expression);
            }
            ArgumentList { arguments, .. } => all(arguments, f),
            SpreadElement { expression, .. } => f(*expression),
            IfElement {
                expression,
                case_clause,
                then_element,
                else_element,
                ..
            } => {
                f(*expression);
                one(case_clause, f);
                f(*then_element);
                one(else_element, f);
            }
            AssignedVariablePattern { .. } => {}
            CastPattern { pattern, type_, .. } => {
                f(*pattern);
                f(*type_);
            }
            ConstantPattern { expression, .. } => f(*expression),
            DeclaredVariablePattern { type_, .. } | WildcardPattern { type_, .. } => one(type_, f),
            NullAssertPattern { pattern, .. }
            | NullCheckPattern { pattern, .. }
            | ParenthesizedPattern { pattern, .. } => f(*pattern),
            ObjectPattern { type_, fields, .. } => {
                f(*type_);
                all(fields, f);
            }
            PatternField { name, pattern } => {
                one(name, f);
                f(*pattern);
            }
            PatternFieldName { .. } => {}
            RelationalPattern { operand, .. } => f(*operand),
            RestPatternElement { pattern, .. } => one(pattern, f),
            PatternAssignment { pattern, expression, .. } => {
                f(*pattern);
                f(*expression);
            }
        }
    }
}
