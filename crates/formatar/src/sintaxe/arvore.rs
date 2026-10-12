//! Árvore sintática concreta no formato do AST do analyzer 3.6
//! (`package:analyzer/dart/ast/ast.dart`), que é o que o `dart_style`
//! percorre.
//!
//! Os nomes das variantes e dos campos são os das classes e getters do
//! analyzer (em `snake_case`), para que o porte do `SourceVisitor` possa ser
//! conferido linha a linha contra o original. Cada nó guarda os índices dos
//! seus tokens ([`Tk`]) e dos filhos ([`NoId`]); o pai, o primeiro e o último
//! token ficam em [`NoDados`].
//!
//! Formas que o parser do analyzer (fasta + `AstBuilder`, sem resolução)
//! produz e que o formatador distingue:
//!
//! * `foo()` e `a.foo()` são [`No::MethodInvocation`]; `a()()` e `(a)()` são
//!   [`No::FunctionExpressionInvocation`]; `new A()`, `const A()` e
//!   `A<T>.nome()` são [`No::InstanceCreationExpression`].
//! * `a.b` com `a` identificador simples é [`No::PrefixedIdentifier`];
//!   qualquer outro acesso (`a.b.c`, `a?.b`, `(a).b`, `this.x`) é
//!   [`No::PropertyAccess`].
//! * Numa cascata, cada seção é a expressão com o alvo vazio: `..a` é
//!   `PropertyAccess(target: None, operator: ..)`, `..f()` é
//!   `MethodInvocation(target: None)`, `..[0]` é `IndexExpression(target:
//!   None, period: ..)`.
//! * `List<int>` em posição de expressão é [`No::FunctionReference`].
//! * Parâmetros dentro de `[...]`/`{...}` são sempre embrulhados em
//!   [`No::DefaultFormalParameter`], com ou sem valor padrão.

use super::token::Tk;

/// Índice de um nó em [`Arvore::nos`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoId(pub u32);

/// Tipo de parâmetro formal, pela posição na lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoParametro {
    /// Posicional obrigatório.
    Obrigatorio,
    /// Posicional opcional (`[...]`).
    OpcionalPosicional,
    /// Nomeado (`{...}`), obrigatório ou não.
    Nomeado,
}

/// Um nó da árvore e suas ligações.
#[derive(Debug, Clone)]
pub struct NoDados {
    pub no: No,
    pub pai: Option<NoId>,
    /// `beginToken` do analyzer (sem o comentário de documentação).
    pub inicio: Tk,
    /// `endToken` do analyzer.
    pub fim: Tk,
}

/// A árvore de uma unidade de compilação; a raiz é o último nó criado.
#[derive(Debug, Clone, Default)]
pub struct Arvore {
    pub nos: Vec<NoDados>,
    pub raiz: Option<NoId>,
}

impl Arvore {
    /// O nó `id`.
    pub fn no(&self, id: NoId) -> &No {
        &self.nos[id.0 as usize].no
    }

    /// Os dados de ligação de `id`.
    pub fn dados(&self, id: NoId) -> &NoDados {
        &self.nos[id.0 as usize]
    }

    /// O pai de `id`.
    pub fn pai(&self, id: NoId) -> Option<NoId> {
        self.nos[id.0 as usize].pai
    }

    /// `beginToken`.
    pub fn inicio(&self, id: NoId) -> Tk {
        self.nos[id.0 as usize].inicio
    }

    /// `endToken`.
    pub fn fim(&self, id: NoId) -> Tk {
        self.nos[id.0 as usize].fim
    }

    /// Acrescenta um nó (sem pai ainda) e devolve seu índice.
    pub fn novo(&mut self, no: No, inicio: Tk, fim: Tk) -> NoId {
        self.nos.push(NoDados {
            no,
            pai: None,
            inicio,
            fim,
        });
        NoId(self.nos.len() as u32 - 1)
    }

    /// Preenche o pai de todos os nós a partir dos filhos de cada um.
    pub fn ligar_pais(&mut self) {
        for i in 0..self.nos.len() {
            let mut filhos = Vec::new();
            self.nos[i].no.filhos(&mut |f| filhos.push(f));
            for f in filhos {
                self.nos[f.0 as usize].pai = Some(NoId(i as u32));
            }
        }
    }
}

/// Os nós do AST do analyzer que o formatador visita.
///
/// Campos `Option<Tk>` são tokens opcionais; `Vec<NoId>` são `NodeList`s.
/// `metadata` são as anotações ([`No::Annotation`]) do nó.
#[derive(Debug, Clone)]
pub enum No {
    // ---------------------------------------------------------------- unidade
    CompilationUnit {
        script_tag: Option<Tk>,
        directives: Vec<NoId>,
        declarations: Vec<NoId>,
    },

    // -------------------------------------------------------------- diretivas
    LibraryDirective {
        metadata: Vec<NoId>,
        library_keyword: Tk,
        /// [`No::LibraryIdentifier`]; ausente em `library;`.
        name2: Option<NoId>,
        semicolon: Tk,
    },
    ImportDirective {
        metadata: Vec<NoId>,
        import_keyword: Tk,
        /// Literal de string.
        uri: NoId,
        configurations: Vec<NoId>,
        deferred_keyword: Option<Tk>,
        as_keyword: Option<Tk>,
        /// [`No::SimpleIdentifier`].
        prefix: Option<NoId>,
        combinators: Vec<NoId>,
        semicolon: Tk,
    },
    ExportDirective {
        metadata: Vec<NoId>,
        export_keyword: Tk,
        uri: NoId,
        configurations: Vec<NoId>,
        combinators: Vec<NoId>,
        semicolon: Tk,
    },
    PartDirective {
        metadata: Vec<NoId>,
        part_keyword: Tk,
        uri: NoId,
        semicolon: Tk,
    },
    PartOfDirective {
        metadata: Vec<NoId>,
        part_keyword: Tk,
        of_keyword: Tk,
        /// [`No::LibraryIdentifier`] em `part of a.b;`.
        library_name: Option<NoId>,
        /// Literal de string em `part of 'a.dart';`.
        uri: Option<NoId>,
        semicolon: Tk,
    },
    Configuration {
        if_keyword: Tk,
        left_parenthesis: Tk,
        /// [`No::DottedName`].
        name: NoId,
        equal_token: Option<Tk>,
        value: Option<NoId>,
        right_parenthesis: Tk,
        uri: NoId,
    },
    DottedName {
        /// [`No::SimpleIdentifier`]s; os `.` ficam entre eles no fluxo.
        components: Vec<NoId>,
    },
    LibraryIdentifier {
        components: Vec<NoId>,
    },
    ShowCombinator {
        keyword: Tk,
        shown_names: Vec<NoId>,
    },
    HideCombinator {
        keyword: Tk,
        hidden_names: Vec<NoId>,
    },

    // ------------------------------------------------------------ declarações
    ClassDeclaration {
        metadata: Vec<NoId>,
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
        type_parameters: Option<NoId>,
        extends_clause: Option<NoId>,
        with_clause: Option<NoId>,
        implements_clause: Option<NoId>,
        native_clause: Option<NoId>,
        left_bracket: Tk,
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    ExtendsClause {
        extends_keyword: Tk,
        /// [`No::NamedType`].
        superclass: NoId,
    },
    WithClause {
        with_keyword: Tk,
        mixin_types: Vec<NoId>,
    },
    ImplementsClause {
        implements_keyword: Tk,
        interfaces: Vec<NoId>,
    },
    NativeClause {
        native_keyword: Tk,
        name: Option<NoId>,
    },
    MixinDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        base_keyword: Option<Tk>,
        mixin_keyword: Tk,
        name: Tk,
        type_parameters: Option<NoId>,
        /// [`No::MixinOnClause`].
        on_clause: Option<NoId>,
        implements_clause: Option<NoId>,
        left_bracket: Tk,
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    MixinOnClause {
        on_keyword: Tk,
        superclass_constraints: Vec<NoId>,
    },
    ExtensionDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        extension_keyword: Tk,
        name: Option<Tk>,
        type_parameters: Option<NoId>,
        /// [`No::ExtensionOnClause`]; ausente só com erro.
        on_clause: Option<NoId>,
        left_bracket: Tk,
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    ExtensionOnClause {
        on_keyword: Tk,
        extended_type: NoId,
    },
    ExtensionTypeDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        extension_keyword: Tk,
        type_keyword: Tk,
        const_keyword: Option<Tk>,
        name: Tk,
        type_parameters: Option<NoId>,
        /// [`No::RepresentationDeclaration`].
        representation: NoId,
        implements_clause: Option<NoId>,
        left_bracket: Tk,
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    RepresentationDeclaration {
        /// [`No::RepresentationConstructorName`].
        constructor_name: Option<NoId>,
        left_parenthesis: Tk,
        field_metadata: Vec<NoId>,
        field_type: NoId,
        field_name: Tk,
        right_parenthesis: Tk,
    },
    RepresentationConstructorName {
        period: Tk,
        name: Tk,
    },
    EnumDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        enum_keyword: Tk,
        name: Tk,
        type_parameters: Option<NoId>,
        with_clause: Option<NoId>,
        implements_clause: Option<NoId>,
        left_bracket: Tk,
        /// [`No::EnumConstantDeclaration`]s.
        constants: Vec<NoId>,
        /// O `;` depois das constantes, se houver.
        semicolon: Option<Tk>,
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    EnumConstantDeclaration {
        metadata: Vec<NoId>,
        name: Tk,
        /// [`No::EnumConstantArguments`].
        arguments: Option<NoId>,
    },
    EnumConstantArguments {
        type_arguments: Option<NoId>,
        /// [`No::ConstructorSelector`].
        constructor_selector: Option<NoId>,
        argument_list: NoId,
    },
    ConstructorSelector {
        period: Tk,
        /// [`No::SimpleIdentifier`].
        name: NoId,
    },
    /// `class A = B with C;` (o "typedef_keyword" do analyzer é o `class`).
    ClassTypeAlias {
        metadata: Vec<NoId>,
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
        type_parameters: Option<NoId>,
        equals: Tk,
        superclass: NoId,
        with_clause: Option<NoId>,
        implements_clause: Option<NoId>,
        semicolon: Tk,
    },
    /// `typedef int F(int a);` (sintaxe antiga).
    FunctionTypeAlias {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        typedef_keyword: Tk,
        return_type: Option<NoId>,
        name: Tk,
        type_parameters: Option<NoId>,
        parameters: NoId,
        semicolon: Tk,
    },
    /// `typedef F = int Function(int);`
    GenericTypeAlias {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        typedef_keyword: Tk,
        name: Tk,
        type_parameters: Option<NoId>,
        equals: Tk,
        type_: NoId,
        semicolon: Tk,
    },
    FunctionDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        return_type: Option<NoId>,
        /// `get` ou `set`.
        property_keyword: Option<Tk>,
        name: Tk,
        /// [`No::FunctionExpression`].
        function_expression: NoId,
    },
    TopLevelVariableDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        /// [`No::VariableDeclarationList`].
        variables: NoId,
        semicolon: Tk,
    },

    // ---------------------------------------------------------------- membros
    ConstructorDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        const_keyword: Option<Tk>,
        factory_keyword: Option<Tk>,
        /// [`No::SimpleIdentifier`] com o nome da classe.
        return_type: NoId,
        period: Option<Tk>,
        name: Option<Tk>,
        parameters: NoId,
        /// `:` antes dos inicializadores ou `=` do redirecionamento.
        separator: Option<Tk>,
        initializers: Vec<NoId>,
        /// [`No::ConstructorName`] de `factory A() = B;`.
        redirected_constructor: Option<NoId>,
        body: NoId,
    },
    MethodDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        /// `static` ou `abstract`.
        modifier_keyword: Option<Tk>,
        return_type: Option<NoId>,
        property_keyword: Option<Tk>,
        operator_keyword: Option<Tk>,
        name: Tk,
        type_parameters: Option<NoId>,
        /// Ausente em getters.
        parameters: Option<NoId>,
        body: NoId,
    },
    FieldDeclaration {
        metadata: Vec<NoId>,
        augment_keyword: Option<Tk>,
        abstract_keyword: Option<Tk>,
        covariant_keyword: Option<Tk>,
        external_keyword: Option<Tk>,
        static_keyword: Option<Tk>,
        /// [`No::VariableDeclarationList`].
        fields: NoId,
        semicolon: Tk,
    },
    ConstructorFieldInitializer {
        this_keyword: Option<Tk>,
        period: Option<Tk>,
        /// [`No::SimpleIdentifier`].
        field_name: NoId,
        equals: Tk,
        expression: NoId,
    },
    SuperConstructorInvocation {
        super_keyword: Tk,
        period: Option<Tk>,
        constructor_name: Option<NoId>,
        argument_list: NoId,
    },
    RedirectingConstructorInvocation {
        this_keyword: Tk,
        period: Option<Tk>,
        constructor_name: Option<NoId>,
        argument_list: NoId,
    },
    AssertInitializer {
        assert_keyword: Tk,
        left_parenthesis: Tk,
        condition: NoId,
        comma: Option<Tk>,
        message: Option<NoId>,
        right_parenthesis: Tk,
    },

    // ------------------------------------------------------------- corpos
    BlockFunctionBody {
        /// `async` ou `sync`.
        keyword: Option<Tk>,
        star: Option<Tk>,
        /// [`No::Block`].
        block: NoId,
    },
    ExpressionFunctionBody {
        keyword: Option<Tk>,
        star: Option<Tk>,
        function_definition: Tk,
        expression: NoId,
        semicolon: Option<Tk>,
    },
    EmptyFunctionBody {
        semicolon: Tk,
    },
    NativeFunctionBody {
        native_keyword: Tk,
        string_literal: Option<NoId>,
        semicolon: Tk,
    },

    // ----------------------------------------------------------- parâmetros
    FormalParameterList {
        left_parenthesis: Tk,
        parameters: Vec<NoId>,
        left_delimiter: Option<Tk>,
        right_delimiter: Option<Tk>,
        right_parenthesis: Tk,
    },
    SimpleFormalParameter {
        metadata: Vec<NoId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        /// `final`, `const` ou `var`.
        keyword: Option<Tk>,
        type_: Option<NoId>,
        name: Option<Tk>,
        kind: TipoParametro,
    },
    FieldFormalParameter {
        metadata: Vec<NoId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        keyword: Option<Tk>,
        type_: Option<NoId>,
        this_keyword: Tk,
        period: Tk,
        name: Tk,
        type_parameters: Option<NoId>,
        parameters: Option<NoId>,
        question: Option<Tk>,
        kind: TipoParametro,
    },
    SuperFormalParameter {
        metadata: Vec<NoId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        keyword: Option<Tk>,
        type_: Option<NoId>,
        super_keyword: Tk,
        period: Tk,
        name: Tk,
        type_parameters: Option<NoId>,
        parameters: Option<NoId>,
        question: Option<Tk>,
        kind: TipoParametro,
    },
    FunctionTypedFormalParameter {
        metadata: Vec<NoId>,
        covariant_keyword: Option<Tk>,
        required_keyword: Option<Tk>,
        return_type: Option<NoId>,
        name: Tk,
        type_parameters: Option<NoId>,
        parameters: NoId,
        question: Option<Tk>,
        kind: TipoParametro,
    },
    DefaultFormalParameter {
        parameter: NoId,
        /// `=` ou `:`.
        separator: Option<Tk>,
        default_value: Option<NoId>,
        kind: TipoParametro,
    },

    // ---------------------------------------------------------------- tipos
    NamedType {
        /// `prefixo` e `.` de `p.Tipo`.
        import_prefix: Option<(Tk, Tk)>,
        name2: Tk,
        type_arguments: Option<NoId>,
        question: Option<Tk>,
    },
    GenericFunctionType {
        return_type: Option<NoId>,
        function_keyword: Tk,
        type_parameters: Option<NoId>,
        parameters: NoId,
        question: Option<Tk>,
    },
    RecordTypeAnnotation {
        left_parenthesis: Tk,
        positional_fields: Vec<NoId>,
        /// [`No::RecordTypeAnnotationNamedFields`].
        named_fields: Option<NoId>,
        right_parenthesis: Tk,
        question: Option<Tk>,
    },
    RecordTypeAnnotationNamedFields {
        left_bracket: Tk,
        fields: Vec<NoId>,
        right_bracket: Tk,
    },
    RecordTypeAnnotationPositionalField {
        metadata: Vec<NoId>,
        type_: NoId,
        name: Option<Tk>,
    },
    RecordTypeAnnotationNamedField {
        metadata: Vec<NoId>,
        type_: NoId,
        name: Tk,
    },
    TypeArgumentList {
        left_bracket: Tk,
        arguments: Vec<NoId>,
        right_bracket: Tk,
    },
    TypeParameterList {
        left_bracket: Tk,
        type_parameters: Vec<NoId>,
        right_bracket: Tk,
    },
    TypeParameter {
        metadata: Vec<NoId>,
        name: Tk,
        extends_keyword: Option<Tk>,
        bound: Option<NoId>,
    },

    // ----------------------------------------------------------- anotações
    Annotation {
        at_sign: Tk,
        /// [`No::SimpleIdentifier`] ou [`No::PrefixedIdentifier`].
        name: NoId,
        type_arguments: Option<NoId>,
        period: Option<Tk>,
        constructor_name: Option<NoId>,
        arguments: Option<NoId>,
    },

    // ----------------------------------------------------------- instruções
    Block {
        left_bracket: Tk,
        statements: Vec<NoId>,
        right_bracket: Tk,
    },
    ExpressionStatement {
        expression: NoId,
        /// Ausente só no corpo `=>`... nunca em código válido.
        semicolon: Option<Tk>,
    },
    VariableDeclarationStatement {
        variables: NoId,
        semicolon: Tk,
    },
    VariableDeclarationList {
        /// Anotações de declarações locais; vazio em campos e variáveis de
        /// topo (lá elas ficam na declaração).
        metadata: Vec<NoId>,
        late_keyword: Option<Tk>,
        /// `final`, `const` ou `var`.
        keyword: Option<Tk>,
        type_: Option<NoId>,
        /// [`No::VariableDeclaration`]s.
        variables: Vec<NoId>,
    },
    VariableDeclaration {
        name: Tk,
        equals: Option<Tk>,
        initializer: Option<NoId>,
    },
    FunctionDeclarationStatement {
        function_declaration: NoId,
    },
    IfStatement {
        if_keyword: Tk,
        left_parenthesis: Tk,
        expression: NoId,
        /// [`No::CaseClause`] de `if (x case P)`.
        case_clause: Option<NoId>,
        right_parenthesis: Tk,
        then_statement: NoId,
        else_keyword: Option<Tk>,
        else_statement: Option<NoId>,
    },
    CaseClause {
        case_keyword: Tk,
        /// [`No::GuardedPattern`].
        guarded_pattern: NoId,
    },
    GuardedPattern {
        pattern: NoId,
        /// [`No::WhenClause`].
        when_clause: Option<NoId>,
    },
    WhenClause {
        when_keyword: Tk,
        expression: NoId,
    },
    ForStatement {
        await_keyword: Option<Tk>,
        for_keyword: Tk,
        left_parenthesis: Tk,
        for_loop_parts: NoId,
        right_parenthesis: Tk,
        body: NoId,
    },
    ForPartsWithDeclarations {
        /// [`No::VariableDeclarationList`].
        variables: NoId,
        left_separator: Tk,
        condition: Option<NoId>,
        right_separator: Tk,
        updaters: Vec<NoId>,
    },
    ForPartsWithExpression {
        initialization: Option<NoId>,
        left_separator: Tk,
        condition: Option<NoId>,
        right_separator: Tk,
        updaters: Vec<NoId>,
    },
    ForPartsWithPattern {
        /// [`No::PatternVariableDeclaration`].
        variables: NoId,
        left_separator: Tk,
        condition: Option<NoId>,
        right_separator: Tk,
        updaters: Vec<NoId>,
    },
    ForEachPartsWithDeclaration {
        /// [`No::DeclaredIdentifier`].
        loop_variable: NoId,
        in_keyword: Tk,
        iterable: NoId,
    },
    ForEachPartsWithIdentifier {
        /// [`No::SimpleIdentifier`].
        identifier: NoId,
        in_keyword: Tk,
        iterable: NoId,
    },
    ForEachPartsWithPattern {
        metadata: Vec<NoId>,
        /// `var` ou `final`.
        keyword: Tk,
        pattern: NoId,
        in_keyword: Tk,
        iterable: NoId,
    },
    DeclaredIdentifier {
        metadata: Vec<NoId>,
        keyword: Option<Tk>,
        type_: Option<NoId>,
        name: Tk,
    },
    WhileStatement {
        while_keyword: Tk,
        left_parenthesis: Tk,
        condition: NoId,
        right_parenthesis: Tk,
        body: NoId,
    },
    DoStatement {
        do_keyword: Tk,
        body: NoId,
        while_keyword: Tk,
        left_parenthesis: Tk,
        condition: NoId,
        right_parenthesis: Tk,
        semicolon: Tk,
    },
    SwitchStatement {
        switch_keyword: Tk,
        left_parenthesis: Tk,
        expression: NoId,
        right_parenthesis: Tk,
        left_bracket: Tk,
        /// [`No::SwitchCase`], [`No::SwitchPatternCase`] e
        /// [`No::SwitchDefault`].
        members: Vec<NoId>,
        right_bracket: Tk,
    },
    /// `case expr:` em biblioteca sem padrões (não ocorre em 3.6).
    SwitchCase {
        labels: Vec<NoId>,
        keyword: Tk,
        expression: NoId,
        colon: Tk,
        statements: Vec<NoId>,
    },
    SwitchPatternCase {
        labels: Vec<NoId>,
        keyword: Tk,
        guarded_pattern: NoId,
        colon: Tk,
        statements: Vec<NoId>,
    },
    SwitchDefault {
        labels: Vec<NoId>,
        keyword: Tk,
        colon: Tk,
        statements: Vec<NoId>,
    },
    TryStatement {
        try_keyword: Tk,
        body: NoId,
        catch_clauses: Vec<NoId>,
        finally_keyword: Option<Tk>,
        finally_block: Option<NoId>,
    },
    CatchClause {
        on_keyword: Option<Tk>,
        exception_type: Option<NoId>,
        catch_keyword: Option<Tk>,
        left_parenthesis: Option<Tk>,
        /// [`No::CatchClauseParameter`].
        exception_parameter: Option<NoId>,
        comma: Option<Tk>,
        stack_trace_parameter: Option<NoId>,
        right_parenthesis: Option<Tk>,
        body: NoId,
    },
    CatchClauseParameter {
        name: Tk,
    },
    ReturnStatement {
        return_keyword: Tk,
        expression: Option<NoId>,
        semicolon: Tk,
    },
    BreakStatement {
        break_keyword: Tk,
        /// [`No::SimpleIdentifier`].
        label: Option<NoId>,
        semicolon: Tk,
    },
    ContinueStatement {
        continue_keyword: Tk,
        label: Option<NoId>,
        semicolon: Tk,
    },
    YieldStatement {
        yield_keyword: Tk,
        star: Option<Tk>,
        expression: NoId,
        semicolon: Tk,
    },
    LabeledStatement {
        /// [`No::Label`]s.
        labels: Vec<NoId>,
        statement: NoId,
    },
    Label {
        /// [`No::SimpleIdentifier`].
        label: NoId,
        colon: Tk,
    },
    AssertStatement {
        assert_keyword: Tk,
        left_parenthesis: Tk,
        condition: NoId,
        comma: Option<Tk>,
        message: Option<NoId>,
        right_parenthesis: Tk,
        semicolon: Tk,
    },
    EmptyStatement {
        semicolon: Tk,
    },
    PatternVariableDeclarationStatement {
        /// [`No::PatternVariableDeclaration`].
        declaration: NoId,
        semicolon: Tk,
    },
    PatternVariableDeclaration {
        metadata: Vec<NoId>,
        /// `var` ou `final`.
        keyword: Tk,
        pattern: NoId,
        equals: Tk,
        expression: NoId,
    },

    // ---------------------------------------------------------- expressões
    AdjacentStrings {
        strings: Vec<NoId>,
    },
    AsExpression {
        expression: NoId,
        as_operator: Tk,
        type_: NoId,
    },
    AssignmentExpression {
        left_hand_side: NoId,
        operator: Tk,
        right_hand_side: NoId,
    },
    AwaitExpression {
        await_keyword: Tk,
        expression: NoId,
    },
    BinaryExpression {
        left_operand: NoId,
        operator: Tk,
        right_operand: NoId,
    },
    BooleanLiteral {
        literal: Tk,
    },
    CascadeExpression {
        target: NoId,
        cascade_sections: Vec<NoId>,
    },
    ConditionalExpression {
        condition: NoId,
        question: Tk,
        then_expression: NoId,
        colon: Tk,
        else_expression: NoId,
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
        type_parameters: Option<NoId>,
        parameters: Option<NoId>,
        body: NoId,
    },
    FunctionExpressionInvocation {
        function: NoId,
        type_arguments: Option<NoId>,
        argument_list: NoId,
    },
    FunctionReference {
        function: NoId,
        type_arguments: Option<NoId>,
    },
    IndexExpression {
        target: Option<NoId>,
        /// `..` ou `?..` quando é seção de cascata.
        period: Option<Tk>,
        /// `?` de `a?[i]`.
        question: Option<Tk>,
        left_bracket: Tk,
        index: NoId,
        right_bracket: Tk,
    },
    InstanceCreationExpression {
        /// `new` ou `const`.
        keyword: Option<Tk>,
        /// [`No::ConstructorName`].
        constructor_name: NoId,
        argument_list: NoId,
    },
    ConstructorName {
        /// [`No::NamedType`].
        type_: NoId,
        period: Option<Tk>,
        /// [`No::SimpleIdentifier`].
        name: Option<NoId>,
    },
    IsExpression {
        expression: NoId,
        is_operator: Tk,
        not_operator: Option<Tk>,
        type_: NoId,
    },
    ListLiteral {
        const_keyword: Option<Tk>,
        type_arguments: Option<NoId>,
        left_bracket: Tk,
        elements: Vec<NoId>,
        right_bracket: Tk,
    },
    SetOrMapLiteral {
        const_keyword: Option<Tk>,
        type_arguments: Option<NoId>,
        left_bracket: Tk,
        elements: Vec<NoId>,
        right_bracket: Tk,
    },
    MapLiteralEntry {
        key: NoId,
        separator: Tk,
        value: NoId,
    },
    MethodInvocation {
        target: Option<NoId>,
        /// `.`, `?.`, `..` ou `?..`.
        operator: Option<Tk>,
        /// [`No::SimpleIdentifier`].
        method_name: NoId,
        type_arguments: Option<NoId>,
        argument_list: NoId,
    },
    NamedExpression {
        /// [`No::Label`].
        name: NoId,
        expression: NoId,
    },
    ParenthesizedExpression {
        left_parenthesis: Tk,
        expression: NoId,
        right_parenthesis: Tk,
    },
    PostfixExpression {
        operand: NoId,
        operator: Tk,
    },
    PrefixExpression {
        operator: Tk,
        operand: NoId,
    },
    PrefixedIdentifier {
        /// [`No::SimpleIdentifier`].
        prefix: NoId,
        period: Tk,
        /// [`No::SimpleIdentifier`].
        identifier: NoId,
    },
    PropertyAccess {
        target: Option<NoId>,
        operator: Tk,
        /// [`No::SimpleIdentifier`].
        property_name: NoId,
    },
    RecordLiteral {
        const_keyword: Option<Tk>,
        left_parenthesis: Tk,
        fields: Vec<NoId>,
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
        /// [`No::InterpolationString`] e [`No::InterpolationExpression`]
        /// alternados, começando e terminando em texto.
        elements: Vec<NoId>,
    },
    InterpolationExpression {
        /// `${` ou `$`.
        left_bracket: Tk,
        expression: NoId,
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
        expression: NoId,
    },
    SwitchExpression {
        switch_keyword: Tk,
        left_parenthesis: Tk,
        expression: NoId,
        right_parenthesis: Tk,
        left_bracket: Tk,
        /// [`No::SwitchExpressionCase`]s.
        cases: Vec<NoId>,
        right_bracket: Tk,
    },
    SwitchExpressionCase {
        guarded_pattern: NoId,
        arrow: Tk,
        expression: NoId,
    },
    ArgumentList {
        left_parenthesis: Tk,
        arguments: Vec<NoId>,
        right_parenthesis: Tk,
    },

    // ------------------------------------------------- elementos de coleção
    SpreadElement {
        /// `...` ou `...?`.
        spread_operator: Tk,
        expression: NoId,
    },
    IfElement {
        if_keyword: Tk,
        left_parenthesis: Tk,
        expression: NoId,
        case_clause: Option<NoId>,
        right_parenthesis: Tk,
        then_element: NoId,
        else_keyword: Option<Tk>,
        else_element: Option<NoId>,
    },
    ForElement {
        await_keyword: Option<Tk>,
        for_keyword: Tk,
        left_parenthesis: Tk,
        for_loop_parts: NoId,
        right_parenthesis: Tk,
        body: NoId,
    },

    // -------------------------------------------------------------- padrões
    AssignedVariablePattern {
        name: Tk,
    },
    CastPattern {
        pattern: NoId,
        as_token: Tk,
        type_: NoId,
    },
    ConstantPattern {
        const_keyword: Option<Tk>,
        expression: NoId,
    },
    DeclaredVariablePattern {
        keyword: Option<Tk>,
        type_: Option<NoId>,
        name: Tk,
    },
    ListPattern {
        type_arguments: Option<NoId>,
        left_bracket: Tk,
        elements: Vec<NoId>,
        right_bracket: Tk,
    },
    LogicalAndPattern {
        left_operand: NoId,
        operator: Tk,
        right_operand: NoId,
    },
    LogicalOrPattern {
        left_operand: NoId,
        operator: Tk,
        right_operand: NoId,
    },
    MapPattern {
        type_arguments: Option<NoId>,
        left_bracket: Tk,
        elements: Vec<NoId>,
        right_bracket: Tk,
    },
    MapPatternEntry {
        key: NoId,
        separator: Tk,
        value: NoId,
    },
    NullAssertPattern {
        pattern: NoId,
        operator: Tk,
    },
    NullCheckPattern {
        pattern: NoId,
        operator: Tk,
    },
    ObjectPattern {
        /// [`No::NamedType`].
        type_: NoId,
        left_parenthesis: Tk,
        /// [`No::PatternField`]s.
        fields: Vec<NoId>,
        right_parenthesis: Tk,
    },
    ParenthesizedPattern {
        left_parenthesis: Tk,
        pattern: NoId,
        right_parenthesis: Tk,
    },
    PatternField {
        /// [`No::PatternFieldName`].
        name: Option<NoId>,
        pattern: NoId,
    },
    PatternFieldName {
        name: Option<Tk>,
        colon: Tk,
    },
    RecordPattern {
        left_parenthesis: Tk,
        fields: Vec<NoId>,
        right_parenthesis: Tk,
    },
    RelationalPattern {
        operator: Tk,
        operand: NoId,
    },
    RestPatternElement {
        operator: Tk,
        pattern: Option<NoId>,
    },
    WildcardPattern {
        keyword: Option<Tk>,
        type_: Option<NoId>,
        name: Tk,
    },
    PatternAssignment {
        pattern: NoId,
        equals: Tk,
        expression: NoId,
    },
}

impl No {
    /// Chama `f` para cada filho, na ordem do texto.
    pub fn filhos(&self, f: &mut dyn FnMut(NoId)) {
        let um = |o: &Option<NoId>, f: &mut dyn FnMut(NoId)| {
            if let Some(n) = o {
                f(*n);
            }
        };
        let todos = |v: &Vec<NoId>, f: &mut dyn FnMut(NoId)| {
            for n in v {
                f(*n);
            }
        };
        use No::*;
        match self {
            CompilationUnit { directives, declarations, .. } => {
                todos(directives, f);
                todos(declarations, f);
            }
            LibraryDirective { metadata, name2, .. } => {
                todos(metadata, f);
                um(name2, f);
            }
            ImportDirective {
                metadata,
                uri,
                configurations,
                prefix,
                combinators,
                ..
            } => {
                todos(metadata, f);
                f(*uri);
                todos(configurations, f);
                um(prefix, f);
                todos(combinators, f);
            }
            ExportDirective {
                metadata,
                uri,
                configurations,
                combinators,
                ..
            } => {
                todos(metadata, f);
                f(*uri);
                todos(configurations, f);
                todos(combinators, f);
            }
            PartDirective { metadata, uri, .. } => {
                todos(metadata, f);
                f(*uri);
            }
            PartOfDirective {
                metadata,
                library_name,
                uri,
                ..
            } => {
                todos(metadata, f);
                um(library_name, f);
                um(uri, f);
            }
            Configuration { name, value, uri, .. } => {
                f(*name);
                um(value, f);
                f(*uri);
            }
            DottedName { components } | LibraryIdentifier { components } => todos(components, f),
            ShowCombinator { shown_names: v, .. } | HideCombinator { hidden_names: v, .. } => todos(v, f),
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
                todos(metadata, f);
                um(type_parameters, f);
                um(extends_clause, f);
                um(with_clause, f);
                um(implements_clause, f);
                um(native_clause, f);
                todos(members, f);
            }
            ExtendsClause { superclass, .. } => f(*superclass),
            WithClause { mixin_types: v, .. }
            | ImplementsClause { interfaces: v, .. }
            | MixinOnClause {
                superclass_constraints: v,
                ..
            } => todos(v, f),
            NativeClause { name, .. } => um(name, f),
            MixinDeclaration {
                metadata,
                type_parameters,
                on_clause,
                implements_clause,
                members,
                ..
            } => {
                todos(metadata, f);
                um(type_parameters, f);
                um(on_clause, f);
                um(implements_clause, f);
                todos(members, f);
            }
            ExtensionDeclaration {
                metadata,
                type_parameters,
                on_clause,
                members,
                ..
            } => {
                todos(metadata, f);
                um(type_parameters, f);
                um(on_clause, f);
                todos(members, f);
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
                todos(metadata, f);
                um(type_parameters, f);
                f(*representation);
                um(implements_clause, f);
                todos(members, f);
            }
            RepresentationDeclaration {
                constructor_name,
                field_metadata,
                field_type,
                ..
            } => {
                um(constructor_name, f);
                todos(field_metadata, f);
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
                todos(metadata, f);
                um(type_parameters, f);
                um(with_clause, f);
                um(implements_clause, f);
                todos(constants, f);
                todos(members, f);
            }
            EnumConstantDeclaration { metadata, arguments, .. } => {
                todos(metadata, f);
                um(arguments, f);
            }
            EnumConstantArguments {
                type_arguments,
                constructor_selector,
                argument_list,
            } => {
                um(type_arguments, f);
                um(constructor_selector, f);
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
                todos(metadata, f);
                um(type_parameters, f);
                f(*superclass);
                um(with_clause, f);
                um(implements_clause, f);
            }
            FunctionTypeAlias {
                metadata,
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                todos(metadata, f);
                um(return_type, f);
                um(type_parameters, f);
                f(*parameters);
            }
            GenericTypeAlias {
                metadata,
                type_parameters,
                type_,
                ..
            } => {
                todos(metadata, f);
                um(type_parameters, f);
                f(*type_);
            }
            FunctionDeclaration {
                metadata,
                return_type,
                function_expression,
                ..
            } => {
                todos(metadata, f);
                um(return_type, f);
                f(*function_expression);
            }
            TopLevelVariableDeclaration { metadata, variables, .. } => {
                todos(metadata, f);
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
                todos(metadata, f);
                f(*return_type);
                f(*parameters);
                todos(initializers, f);
                um(redirected_constructor, f);
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
                todos(metadata, f);
                um(return_type, f);
                um(type_parameters, f);
                um(parameters, f);
                f(*body);
            }
            FieldDeclaration { metadata, fields, .. } => {
                todos(metadata, f);
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
                um(constructor_name, f);
                f(*argument_list);
            }
            AssertInitializer { condition, message, .. } | AssertStatement { condition, message, .. } => {
                f(*condition);
                um(message, f);
            }
            BlockFunctionBody { block, .. } => f(*block),
            ExpressionFunctionBody { expression, .. } => f(*expression),
            EmptyFunctionBody { .. } => {}
            NativeFunctionBody { string_literal, .. } => um(string_literal, f),
            FormalParameterList { parameters, .. } => todos(parameters, f),
            SimpleFormalParameter { metadata, type_, .. } => {
                todos(metadata, f);
                um(type_, f);
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
                todos(metadata, f);
                um(type_, f);
                um(type_parameters, f);
                um(parameters, f);
            }
            FunctionTypedFormalParameter {
                metadata,
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                todos(metadata, f);
                um(return_type, f);
                um(type_parameters, f);
                f(*parameters);
            }
            DefaultFormalParameter {
                parameter,
                default_value,
                ..
            } => {
                f(*parameter);
                um(default_value, f);
            }
            NamedType { type_arguments, .. } => um(type_arguments, f),
            GenericFunctionType {
                return_type,
                type_parameters,
                parameters,
                ..
            } => {
                um(return_type, f);
                um(type_parameters, f);
                f(*parameters);
            }
            RecordTypeAnnotation {
                positional_fields,
                named_fields,
                ..
            } => {
                todos(positional_fields, f);
                um(named_fields, f);
            }
            RecordTypeAnnotationNamedFields { fields, .. } => todos(fields, f),
            RecordTypeAnnotationPositionalField { metadata, type_, .. }
            | RecordTypeAnnotationNamedField { metadata, type_, .. } => {
                todos(metadata, f);
                f(*type_);
            }
            TypeArgumentList { arguments: v, .. }
            | TypeParameterList {
                type_parameters: v, ..
            } => todos(v, f),
            TypeParameter { metadata, bound, .. } => {
                todos(metadata, f);
                um(bound, f);
            }
            Annotation {
                name,
                type_arguments,
                constructor_name,
                arguments,
                ..
            } => {
                f(*name);
                um(type_arguments, f);
                um(constructor_name, f);
                um(arguments, f);
            }
            Block { statements, .. } => todos(statements, f),
            ExpressionStatement { expression, .. } => f(*expression),
            VariableDeclarationStatement { variables, .. } => f(*variables),
            VariableDeclarationList {
                metadata,
                type_,
                variables,
                ..
            } => {
                todos(metadata, f);
                um(type_, f);
                todos(variables, f);
            }
            VariableDeclaration { initializer, .. } => um(initializer, f),
            FunctionDeclarationStatement { function_declaration } => f(*function_declaration),
            IfStatement {
                expression,
                case_clause,
                then_statement,
                else_statement,
                ..
            } => {
                f(*expression);
                um(case_clause, f);
                f(*then_statement);
                um(else_statement, f);
            }
            CaseClause { guarded_pattern, .. } => f(*guarded_pattern),
            GuardedPattern { pattern, when_clause } => {
                f(*pattern);
                um(when_clause, f);
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
                um(condition, f);
                todos(updaters, f);
            }
            ForPartsWithExpression {
                initialization,
                condition,
                updaters,
                ..
            } => {
                um(initialization, f);
                um(condition, f);
                todos(updaters, f);
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
                todos(metadata, f);
                f(*pattern);
                f(*iterable);
            }
            DeclaredIdentifier { metadata, type_, .. } => {
                todos(metadata, f);
                um(type_, f);
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
                todos(members, f);
            }
            SwitchCase {
                labels,
                expression,
                statements,
                ..
            } => {
                todos(labels, f);
                f(*expression);
                todos(statements, f);
            }
            SwitchPatternCase {
                labels,
                guarded_pattern,
                statements,
                ..
            } => {
                todos(labels, f);
                f(*guarded_pattern);
                todos(statements, f);
            }
            SwitchDefault { labels, statements, .. } => {
                todos(labels, f);
                todos(statements, f);
            }
            TryStatement {
                body,
                catch_clauses,
                finally_block,
                ..
            } => {
                f(*body);
                todos(catch_clauses, f);
                um(finally_block, f);
            }
            CatchClause {
                exception_type,
                exception_parameter,
                stack_trace_parameter,
                body,
                ..
            } => {
                um(exception_type, f);
                um(exception_parameter, f);
                um(stack_trace_parameter, f);
                f(*body);
            }
            CatchClauseParameter { .. } => {}
            ReturnStatement { expression, .. } => um(expression, f),
            BreakStatement { label, .. } | ContinueStatement { label, .. } => um(label, f),
            YieldStatement { expression, .. } => f(*expression),
            LabeledStatement { labels, statement } => {
                todos(labels, f);
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
                todos(metadata, f);
                f(*pattern);
                f(*expression);
            }
            AdjacentStrings { strings } => todos(strings, f),
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
                todos(cascade_sections, f);
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
                um(type_parameters, f);
                um(parameters, f);
                f(*body);
            }
            FunctionExpressionInvocation {
                function,
                type_arguments,
                argument_list,
            } => {
                f(*function);
                um(type_arguments, f);
                f(*argument_list);
            }
            FunctionReference {
                function,
                type_arguments,
            } => {
                f(*function);
                um(type_arguments, f);
            }
            IndexExpression { target, index, .. } => {
                um(target, f);
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
                um(name, f);
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
                um(type_arguments, f);
                todos(elements, f);
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
                um(target, f);
                f(*method_name);
                um(type_arguments, f);
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
                um(target, f);
                f(*property_name);
            }
            RecordLiteral { fields, .. } | RecordPattern { fields, .. } => todos(fields, f),
            RethrowExpression { .. } | SimpleIdentifier { .. } | SimpleStringLiteral { .. } => {}
            StringInterpolation { elements } => todos(elements, f),
            InterpolationExpression { expression, .. } => f(*expression),
            InterpolationString { .. } | SuperExpression { .. } | ThisExpression { .. } | SymbolLiteral { .. } => {}
            SwitchExpression { expression, cases, .. } => {
                f(*expression);
                todos(cases, f);
            }
            SwitchExpressionCase {
                guarded_pattern,
                expression,
                ..
            } => {
                f(*guarded_pattern);
                f(*expression);
            }
            ArgumentList { arguments, .. } => todos(arguments, f),
            SpreadElement { expression, .. } => f(*expression),
            IfElement {
                expression,
                case_clause,
                then_element,
                else_element,
                ..
            } => {
                f(*expression);
                um(case_clause, f);
                f(*then_element);
                um(else_element, f);
            }
            AssignedVariablePattern { .. } => {}
            CastPattern { pattern, type_, .. } => {
                f(*pattern);
                f(*type_);
            }
            ConstantPattern { expression, .. } => f(*expression),
            DeclaredVariablePattern { type_, .. } | WildcardPattern { type_, .. } => um(type_, f),
            NullAssertPattern { pattern, .. }
            | NullCheckPattern { pattern, .. }
            | ParenthesizedPattern { pattern, .. } => f(*pattern),
            ObjectPattern { type_, fields, .. } => {
                f(*type_);
                todos(fields, f);
            }
            PatternField { name, pattern } => {
                um(name, f);
                f(*pattern);
            }
            PatternFieldName { .. } => {}
            RelationalPattern { operand, .. } => f(*operand),
            RestPatternElement { pattern, .. } => um(pattern, f),
            PatternAssignment { pattern, expression, .. } => {
                f(*pattern);
                f(*expression);
            }
        }
    }
}
