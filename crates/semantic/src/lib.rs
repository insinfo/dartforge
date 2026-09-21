//! Resolve nomes, escopos e tipos do subconjunto de Dart 3.6.2.
//! Rejeita programas incompatíveis antes da geração de JavaScript.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Program, Statement, StatementKind, Type, UnaryOp,
};
use dartforge_syntax::{
    ClassKind, ClassModifier, ConstValue, ExtensionTarget, ParameterKind, Resolution, TypeShape,
};
mod asynchronous;
mod cascades;
mod collections;
mod constants;
mod constructors;
mod fluxo;
mod generics;
mod maps;
mod modifiers;
mod native;
mod records;
mod reified;
mod shorthands;
mod statics;
mod switches;
use std::collections::{HashMap, HashSet};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct Binding {
    constant: Option<Rc<ConstValue>>,
    ty: Option<Type>,
    is_final: bool,
    promoted: Option<Type>,
}
/// Parâmetro nomeado de uma assinatura, já com o rótulo externo resolvido.
///
/// A lista que os guarda permanece ordenada por `label` para busca binária:
/// uma assinatura é compartilhada por `Rc` e não pode carregar um `HashMap`.
#[derive(Clone, PartialEq, Eq)]
struct NamedParameter<'a> {
    label: &'a str,
    ty: Type,
    is_required: bool,
}
#[derive(Clone, PartialEq, Eq)]
struct Signature<'a> {
    generic_count: usize,
    bounds: Vec<Type>,
    is_getter: bool,
    /// Posicionais na ordem declarada: os obrigatórios precedem os opcionais.
    parameters: Vec<Type>,
    /// Quantos dos primeiros `parameters` a chamada precisa fornecer.
    required_positional: usize,
    /// Nomeados ordenados por rótulo; vazio no caminho comum e sem alocação.
    named: Vec<NamedParameter<'a>>,
    result: Type,
}
impl<'a> Signature<'a> {
    /// Procura um nomeado por rótulo; a lista é pequena e sempre ordenada.
    fn named(&self, label: &str) -> Option<&NamedParameter<'a>> {
        let index = self
            .named
            .binary_search_by(|candidate| candidate.label.cmp(label))
            .ok()?;
        Some(&self.named[index])
    }
}
/// Extrai a forma compacta de uma lista de parâmetros declarada.
///
/// Devolve os tipos posicionais, quantos deles são obrigatórios e os nomeados
/// ordenados. O caminho comum — só posicionais obrigatórios — não aloca nada
/// além do `Vec` de tipos que a assinatura já mantinha.
fn signature_parts<'a>(
    parameters: &[dartforge_syntax::Parameter<'a>],
) -> (Vec<Type>, usize, Vec<NamedParameter<'a>>) {
    // Os nomeados sempre encerram a lista, então uma única varredura basta e o
    // `collect` continua recebendo um iterador de tamanho exato: uma alocação.
    let (split, required) = split_kinds(parameters.iter().map(|parameter| parameter.kind));
    let positional = parameters[..split]
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    if split == parameters.len() {
        return (positional, required, Vec::new());
    }
    let mut named = parameters[split..]
        .iter()
        .map(|parameter| NamedParameter {
            label: parameter.label(),
            ty: parameter.ty,
            is_required: parameter.kind.is_required(),
        })
        .collect::<Vec<_>>();
    named.sort_unstable_by_key(|parameter| parameter.label);
    (positional, required, named)
}
/// Localiza o início do grupo nomeado e o total de posicionais obrigatórios.
///
/// Uma única varredura, sem alocar, usada pelas duas formas de parâmetro.
fn split_kinds(kinds: impl Iterator<Item = ParameterKind>) -> (usize, usize) {
    let mut split = 0;
    let mut required = 0;
    let mut contiguous = true;
    for (index, kind) in kinds.enumerate() {
        if kind.is_named() {
            continue;
        }
        split = index + 1;
        if contiguous && kind == ParameterKind::RequiredPositional {
            required += 1;
        } else {
            contiguous = false;
        }
    }
    (split, required)
}
/// Indica se a expressão é um literal escalar aceito como valor padrão.
///
/// Apenas `int`, `double`, `String`, `bool`, `null` e a negação de um literal
/// numérico. A restrição mantém a emissão do padrão independente de qualquer tabela
/// indexada por span, que o linker não reescreve dentro de um parâmetro.
fn is_scalar_literal(expression: &Expr<'_>) -> bool {
    match &expression.kind {
        ExprKind::Int(_)
        | ExprKind::Double(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::OwnedString(_)
        | ExprKind::Null => true,
        ExprKind::Unary {
            op: UnaryOp::Negate,
            operand,
        } => matches!(operand.kind, ExprKind::Int(_) | ExprKind::Double(_)),
        _ => false,
    }
}
/// Monta a assinatura compacta de uma função, método ou fábrica declarada.
fn signature_of<'a>(function: &dartforge_syntax::Function<'a>) -> Signature<'a> {
    let (parameters, required_positional, named) = signature_parts(&function.parameters);
    Signature {
        generic_count: function.type_parameters.len(),
        bounds: function
            .type_parameters
            .iter()
            .map(|parameter| parameter.bound)
            .collect(),
        is_getter: function.is_getter,
        parameters,
        required_positional,
        named,
        result: function.return_type,
    }
}
/// Mesma extração para a lista de um construtor generativo.
fn constructor_parts<'a>(
    parameters: &[dartforge_syntax::ConstructorParameter<'a>],
) -> (Vec<Type>, usize, Vec<NamedParameter<'a>>) {
    let (split, required) = split_kinds(parameters.iter().map(|parameter| parameter.kind));
    let positional = parameters[..split]
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    if split == parameters.len() {
        return (positional, required, Vec::new());
    }
    let mut named = parameters[split..]
        .iter()
        .map(|parameter| NamedParameter {
            label: parameter.label(),
            ty: parameter.ty,
            is_required: parameter.kind.is_required(),
        })
        .collect::<Vec<_>>();
    named.sort_unstable_by_key(|parameter| parameter.label);
    (positional, required, named)
}
/// Separa a lista escrita em posicionais e nomeados sem alocar.
///
/// Os nomeados só podem aparecer no fim da lista; o parser já garante isso e
/// esta verificação protege árvores construídas por macros ou por testes.
fn split_arguments<'e, 'a>(
    arguments: &'e [Expr<'a>],
    span: Span,
) -> Result<(&'e [Expr<'a>], &'e [Expr<'a>]), Diagnostic> {
    let first = arguments
        .iter()
        .position(|argument| matches!(argument.kind, ExprKind::NamedArgument { .. }))
        .unwrap_or(arguments.len());
    if arguments[first..]
        .iter()
        .any(|argument| !matches!(argument.kind, ExprKind::NamedArgument { .. }))
    {
        return Err(Diagnostic::new(
            "Positional arguments must precede named arguments",
            span,
        ));
    }
    Ok(arguments.split_at(first))
}

#[derive(Clone, Copy)]
struct FieldInfo {
    ty: Type,
    is_final: bool,
}
/// Assinatura de um construtor generativo nomeado, na forma compacta.
///
/// As listas seguem o mesmo contrato de [`Signature`]: posicionais na ordem
/// declarada e nomeados ordenados por rótulo, sem tabela associativa alguma.
#[derive(Clone, PartialEq, Eq)]
struct ConstructorInfo<'a> {
    name: &'a str,
    parameters: Vec<Type>,
    required: usize,
    named: Vec<NamedParameter<'a>>,
    is_const: bool,
}
/// Membro estático de classe ou variável de topo já tipado.
///
/// `constant` só existe para declarações `const`, cujo valor é canônico e
/// utilizável dentro de outras expressões constantes.
#[derive(Clone)]
struct StaticInfo<'a> {
    name: &'a str,
    ty: Type,
    /// `final` ou `const`: a escrita é recusada depois da inicialização.
    is_final: bool,
    constant: Option<Rc<ConstValue>>,
}
/// Formal `this.campo` de um construtor `const`, com padrão já avaliado.
#[derive(Clone)]
struct ConstFormal<'a> {
    field: &'a str,
    label: &'a str,
    kind: ParameterKind,
    default: Option<ConstValue>,
}
/// Receita para montar `const C(...)` sem executar código do usuário.
///
/// Só existe para o recorte aceito de construtores const: sem superclasse, sem
/// corpo, sem lista de inicialização e com todo campo vindo de um formal
/// `this.campo` ou de um inicializador de declaração já constante.
#[derive(Clone)]
struct ConstPlan<'a> {
    formals: Vec<ConstFormal<'a>>,
    /// Campos na ordem de declaração; `None` indica valor vindo do formal.
    fields: Vec<(&'a str, Option<ConstValue>)>,
}
/// Procura por nome numa lista ordenada, sem construir tabela associativa.
fn find_by_name<'a, T>(items: &'a [T], name: &str, key: impl Fn(&T) -> &'a str) -> Option<&'a T> {
    let index = items.binary_search_by(|item| key(item).cmp(name)).ok()?;
    Some(&items[index])
}
#[derive(Clone)]
struct ClassInfo<'a> {
    has_generative: bool,
    factories: HashMap<&'a str, Signature<'a>>,
    constructor_parameters: Vec<Type>,
    /// Quantos posicionais do construtor a chamada precisa fornecer.
    constructor_required: usize,
    /// Nomeados do construtor, ordenados por rótulo como em Signature.
    constructor_named: Vec<NamedParameter<'a>>,
    /// Construtores nomeados ordenados por nome; vazio no caminho comum.
    named_constructors: Vec<ConstructorInfo<'a>>,
    /// Campos estáticos ordenados por nome; não participam de herança.
    static_fields: Vec<StaticInfo<'a>>,
    /// Métodos estáticos ordenados por nome; resolvidos pelo nome da classe.
    static_methods: Vec<(&'a str, Signature<'a>)>,
    /// Receitas de construtores const, ordenadas por nome (`""` para o sem nome).
    const_plans: Vec<(&'a str, Rc<ConstPlan<'a>>)>,
    modifier: ClassModifier,
    kind: ClassKind,
    is_mixin_application: bool,
    is_interface: bool,
    library_id: usize,
    is_abstract: bool,
    interfaces: Vec<u32>,
    abstract_methods: HashSet<&'a str>,
    enum_values: Vec<&'a str>,
    name: &'a str,
    superclass: Option<u32>,
    fields: HashMap<&'a str, FieldInfo>,
    methods: HashMap<&'a str, Signature<'a>>,
}
#[derive(Clone)]
struct ExtensionInfo<'a> {
    id: u32,
    on_type: Type,
    methods: HashMap<&'a str, (usize, Signature<'a>)>,
}
#[derive(Clone)]
struct Validator<'a> {
    in_arrow: bool,
    async_library: bool,
    in_async: bool,
    factory_class: Option<u32>,
    cascade_receiver: Option<Type>,
    in_constructor: bool,
    exhaustive_switches: Rc<RefCell<HashSet<(usize, usize)>>>,
    /// Ambiente genérico da declaração corrente; trocado por inteiro, nunca em partes.
    type_parameters: Rc<Vec<dartforge_syntax::GenericParameter<'a>>>,
    switch_depth: usize,
    /// Cláusulas catch ativas; só `rethrow` consulta, e um contador basta.
    catch_depth: usize,
    /// Rótulos de laços visíveis, do mais externo ao mais interno.
    ///
    /// São fatias emprestadas da fonte num `Vec` vazio no caminho comum; um
    /// `Vec` vazio não aloca, então clonar o validador a cada ramificação de
    /// fluxo continua sem custo algum para funções sem rótulo.
    labels: Vec<&'a str>,
    captured_writes: Rc<HashSet<&'a str>>,
    inferred_returns: Option<Rc<RefCell<Vec<Type>>>>,
    scopes: Vec<HashMap<&'a str, Binding>>,
    /// Tabela global construída antes da análise; compartilhada, nunca por fluxo.
    functions: Rc<HashMap<&'a str, Signature<'a>>>,
    return_type: Type,
    loop_depth: usize,
    /// Tabela global construída antes da análise; compartilhada, nunca por fluxo.
    classes: Rc<HashMap<u32, ClassInfo<'a>>>,
    /// Variáveis de topo ordenadas por nome; vazia sem declaração alguma.
    globals: Rc<Vec<StaticInfo<'a>>>,
    current_class: Option<u32>,
    in_field_initializer: bool,
    /// Lista de inicialização em curso: `this` ainda não pode ser lido.
    in_initializer_list: bool,
    /// Tabela global construída antes da análise; compartilhada, nunca por fluxo.
    extensions: Rc<Vec<ExtensionInfo<'a>>>,
    current_extension: Option<usize>,
    resolution: Rc<RefCell<Resolution>>,
}

/// Valida nomes, tipos, chamadas, retornos e controle de laços antes da geração de código.
/// Aplicações `with` devem ser normalizadas pelo lowering de mixins antes desta etapa.
///
/// # Exemplos
/// ```
/// use dartforge_syntax::Program;
/// let programa = Program { main_is_arrow: false, main_is_async: false, types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] };
/// assert!(dartforge_semantic::validate(&programa).is_ok());
/// ```
///
/// # Erros
/// Retorna o primeiro diagnóstico de nome desconhecido, incompatibilidade de tipos,
/// retorno ausente ou controle de fluxo inválido. A análise conservadora de retorno
/// não considera laços, mesmo infinitos, como prova de retorno obrigatório.
pub fn validate(program: &Program<'_>) -> Result<(), Diagnostic> {
    analyze(program).map(|_| ())
}

/// Valida o programa e registra destinos estáticos das chamadas de extensions.
/// Recebe aplicações de mixins já expandidas em classes sintéticas pelo pipeline;
/// uma AST com listas `Class::mixins` ainda preenchidas produz diagnóstico explícito.
///
/// # Erros
/// Retorna os mesmos diagnósticos de validate, incluindo extensions ambíguas,
/// tipos on não suportados e métodos incompatíveis com o receptor estático.
///
/// # Exemplos
/// ```
/// let programa = dartforge_syntax::Program { main_is_arrow: false, main_is_async: false, types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] };
/// assert!(dartforge_semantic::analyze(&programa).unwrap().extension_calls.is_empty());
/// ```
pub fn analyze(program: &Program<'_>) -> Result<Resolution, Diagnostic> {
    analyze_with_async_library(program, false)
}
/// Analisa após o linker verificar a disponibilidade dos nomes de dart:async por biblioteca.
/// # Erros
/// Retorna diagnósticos de tipos ou uso de intrínsecos sem a biblioteca habilitada.
pub fn analyze_with_async_library(
    program: &Program<'_>,
    async_library: bool,
) -> Result<Resolution, Diagnostic> {
    let mut validator = Validator {
        in_arrow: false,
        async_library,
        in_async: false,
        factory_class: None,
        cascade_receiver: None,
        in_constructor: false,
        exhaustive_switches: Rc::new(RefCell::new(HashSet::new())),
        type_parameters: Rc::new(vec![]),
        switch_depth: 0,
        catch_depth: 0,
        labels: Vec::new(),
        captured_writes: Rc::new(collections::captured_writes(program)),
        inferred_returns: None,
        scopes: Vec::new(),
        functions: Rc::new(HashMap::new()),
        return_type: Type::Void,
        loop_depth: 0,
        classes: Rc::new(HashMap::new()),
        globals: Rc::new(Vec::new()),
        current_class: None,
        in_field_initializer: false,
        in_initializer_list: false,
        extensions: Rc::new(vec![]),
        current_extension: None,
        resolution: Rc::new(RefCell::new(Resolution {
            types: program.types.clone(),
            ..Default::default()
        })),
    };
    validator.validate_shapes()?;
    let main_result = if program.main_is_async {
        validator.intern(TypeShape::Future(Type::Void))
    } else {
        Type::Void
    };
    Rc::make_mut(&mut validator.functions).insert(
        "main",
        Signature {
            generic_count: 0,
            bounds: vec![],
            is_getter: false,
            parameters: vec![],
            required_positional: 0,
            named: vec![],
            result: main_result,
        },
    );
    let mut class_names = HashSet::new();
    let mut globals_class = None;
    for class in &program.classes {
        // A declaração sintética de variáveis de topo não é um tipo nominal:
        // não entra na tabela de classes nem participa de herança.
        if class.is_library_globals {
            if globals_class.is_some() {
                return Err(Diagnostic::new(
                    "Duplicate library globals declaration",
                    class.span,
                ));
            }
            globals_class = Some(class);
            continue;
        }
        if class
            .annotations
            .iter()
            .any(|a| matches!(a.kind, dartforge_syntax::AnnotationKind::Override))
        {
            return Err(Diagnostic::new(
                "@override requires an instance member",
                class.span,
            ));
        }
        if !class.is_mixin_application
            && (!class_names.insert(class.name)
                || matches!(
                    class.name,
                    "main" | "print" | "int" | "String" | "bool" | "Object" | "Null"
                ))
        {
            return Err(Diagnostic::new(
                "Duplicate or reserved class name",
                class.span,
            ));
        }
        let (constructor_positional, constructor_required, constructor_named) =
            class.constructor.as_ref().map_or_else(
                || (Vec::new(), 0, Vec::new()),
                |constructor| constructor_parts(&constructor.parameters),
            );
        let mut named_constructors = class
            .named_constructors
            .iter()
            .map(|declared| {
                let (parameters, required, named) =
                    constructor_parts(&declared.constructor.parameters);
                ConstructorInfo {
                    name: declared.name,
                    parameters,
                    required,
                    named,
                    is_const: declared.extras.is_const,
                }
            })
            .collect::<Vec<_>>();
        named_constructors.sort_unstable_by_key(|declared| declared.name);
        if named_constructors
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(Diagnostic::new(
                "Duplicate named constructor",
                class.span,
            ));
        }
        let mut info = ClassInfo {
            // Declarar qualquer construtor remove o construtor implícito sem
            // nome, exatamente como em Dart.
            has_generative: class.constructor.is_some()
                || (class.factories.is_empty() && class.named_constructors.is_empty()),
            factories: HashMap::new(),
            constructor_parameters: constructor_positional,
            constructor_required,
            constructor_named,
            named_constructors,
            static_fields: Vec::new(),
            static_methods: Vec::new(),
            const_plans: Vec::new(),
            modifier: class.modifier,
            kind: class.kind,
            is_mixin_application: class.is_mixin_application,
            is_interface: class.is_interface,
            library_id: class.library_id,
            is_abstract: class.is_abstract
                || class.kind == ClassKind::Mixin
                || class.modifier == ClassModifier::Sealed,
            interfaces: class.interfaces.clone(),
            abstract_methods: class.abstract_methods.iter().map(|m| m.name).collect(),
            enum_values: class.enum_values.clone(),
            name: class.name,
            superclass: class.superclass,
            fields: HashMap::new(),
            methods: HashMap::new(),
        };
        for field in &class.fields {
            if field.name == class.name
                || matches!(
                    field.name,
                    "toString" | "hashCode" | "runtimeType" | "noSuchMethod"
                )
            {
                return Err(Diagnostic::new(
                    "Member name requires unsupported Object or constructor semantics",
                    field.span,
                ));
            }
            if info
                .fields
                .insert(
                    field.name,
                    FieldInfo {
                        ty: field.ty,
                        is_final: field.is_final,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new("Duplicate field", field.span));
            }
        }
        for method in class.methods.iter().chain(&class.abstract_methods) {
            if method.name == class.name
                || matches!(
                    method.name,
                    "toString" | "hashCode" | "runtimeType" | "noSuchMethod"
                )
            {
                return Err(Diagnostic::new(
                    "Member name requires unsupported Object or constructor semantics",
                    method.span,
                ));
            }
            if info.fields.contains_key(method.name)
                || info
                    .methods
                    .insert(method.name, signature_of(method))
                    .is_some()
            {
                return Err(Diagnostic::new("Duplicate class member", method.span));
            }
        }
        for factory in &class.factories {
            if factory.return_type != Type::Class(class.id)
                || factory.is_getter
                || !factory.type_parameters.is_empty()
                || factory.native_binding.is_some()
                || class.kind != ClassKind::Class
                || !class.enum_values.is_empty()
            {
                return Err(Diagnostic::new(
                    "Unsupported factory signature",
                    factory.span,
                ));
            }
            if info.fields.contains_key(factory.name)
                || info.methods.contains_key(factory.name)
                || info
                    .factories
                    .insert(factory.name, signature_of(factory))
                    .is_some()
            {
                return Err(Diagnostic::new(
                    "Duplicate factory or conflicting member",
                    factory.span,
                ));
            }
        }
        for member in &class.static_fields {
            if info.fields.contains_key(member.name)
                || info.methods.contains_key(member.name)
                || info.factories.contains_key(member.name)
                || info
                    .static_fields
                    .iter()
                    .any(|existing| existing.name == member.name)
            {
                return Err(Diagnostic::new(
                    "Duplicate static member or conflicting instance member",
                    member.span,
                ));
            }
            info.static_fields.push(StaticInfo {
                name: member.name,
                ty: member.ty,
                is_final: member.is_final || member.is_const,
                constant: None,
            });
        }
        for method in &class.static_methods {
            if info.fields.contains_key(method.name)
                || info.methods.contains_key(method.name)
                || info.factories.contains_key(method.name)
                || info
                    .static_fields
                    .iter()
                    .any(|existing| existing.name == method.name)
                || info
                    .static_methods
                    .iter()
                    .any(|(name, _)| *name == method.name)
            {
                return Err(Diagnostic::new(
                    "Duplicate static member or conflicting instance member",
                    method.span,
                ));
            }
            if method.is_getter || !method.type_parameters.is_empty() {
                return Err(Diagnostic::new(
                    "Static getters and generic static methods are unsupported",
                    method.span,
                ));
            }
            info.static_methods.push((method.name, signature_of(method)));
        }
        info.static_fields
            .sort_unstable_by_key(|member| member.name);
        info.static_methods.sort_unstable_by_key(|(name, _)| *name);
        if Rc::make_mut(&mut validator.classes)
            .insert(class.id, info)
            .is_some()
        {
            return Err(Diagnostic::new("Duplicate class ID", class.span));
        }
    }
    validator.validate_class_graph(program)?;
    for function in &program.functions {
        if function.is_getter {
            return Err(Diagnostic::new(
                "Top-level getters are unsupported",
                function.span,
            ));
        }
        if function.name == "print" || class_names.contains(function.name) {
            return Err(Diagnostic::new(
                "Declaring a top-level function named 'print' is unsupported",
                function.span,
            ));
        }
        if Rc::make_mut(&mut validator.functions)
            .insert(function.name, signature_of(function))
            .is_some()
        {
            return Err(Diagnostic::new(
                format!("Duplicate function '{}'", function.name),
                function.span,
            ));
        }
    }
    let mut extension_ids = HashSet::new();
    let mut extension_names = HashSet::new();
    for extension in &program.extensions {
        if !extension_ids.insert(extension.id)
            || !extension_names.insert(extension.name)
            || class_names.contains(extension.name)
            || validator.functions.contains_key(extension.name)
            || extension.name == "print"
        {
            return Err(Diagnostic::new(
                "Duplicate extension name or ID",
                extension.span,
            ));
        }
        if !matches!(
            extension.on_type,
            Type::Int | Type::String | Type::Bool | Type::Class(_)
        ) {
            return Err(Diagnostic::new(
                "Unsupported nullable or special extension on type",
                extension.span,
            ));
        }
        validator.check_type_name(extension.on_type, extension.span)?;
        let mut methods = HashMap::new();
        for (index, method) in extension.methods.iter().enumerate() {
            if matches!(
                method.name,
                "toString" | "hashCode" | "runtimeType" | "noSuchMethod"
            ) {
                return Err(Diagnostic::new(
                    "Extensions of Object members are unsupported",
                    method.span,
                ));
            }
            if methods
                .insert(method.name, (index, signature_of(method)))
                .is_some()
            {
                return Err(Diagnostic::new("Duplicate extension method", method.span));
            }
        }
        Rc::make_mut(&mut validator.extensions).push(ExtensionInfo {
            id: extension.id,
            on_type: extension.on_type,
            methods,
        });
    }
    if let Some(class) = globals_class {
        // Os nomes entram primeiro para que qualquer inicializador enxergue toda
        // a biblioteca; só os valores `const` dependem da ordem escrita.
        let mut globals: Vec<StaticInfo<'_>> = Vec::with_capacity(class.static_fields.len());
        for member in &class.static_fields {
            if validator.functions.contains_key(member.name)
                || class_names.contains(member.name)
                || extension_names.contains(member.name)
                || member.name == "print"
            {
                return Err(Diagnostic::new(
                    format!("Duplicate or reserved top-level name '{}'", member.name),
                    member.span,
                ));
            }
            if globals.iter().any(|global| global.name == member.name) {
                return Err(Diagnostic::new(
                    format!("Duplicate top-level variable '{}'", member.name),
                    member.span,
                ));
            }
            validator.check_type_name(member.ty, member.span)?;
            if matches!(member.ty, Type::Void | Type::Null | Type::Inferred) {
                return Err(Diagnostic::new(
                    "Unsupported top-level variable type",
                    member.span,
                ));
            }
            globals.push(StaticInfo {
                name: member.name,
                ty: member.ty,
                is_final: member.is_final || member.is_const,
                constant: None,
            });
        }
        globals.sort_unstable_by_key(|global| global.name);
        validator.globals = Rc::new(globals);
        for member in &class.static_fields {
            validator.validate_static_initializer(member)?;
            if member.is_const {
                let value = validator.constant_initializer(member)?;
                let table = Rc::make_mut(&mut validator.globals);
                let index = table
                    .binary_search_by(|global| global.name.cmp(member.name))
                    .expect("variável de topo registrada antes da avaliação");
                table[index].constant = Some(Rc::new(value));
            }
        }
    }
    for class in &program.classes {
        if class.is_library_globals {
            continue;
        }
        for member in &class.static_fields {
            validator.validate_static_initializer(member)?;
            if member.is_const {
                let value = validator.constant_initializer(member)?;
                let info = Rc::make_mut(&mut validator.classes)
                    .get_mut(&class.id)
                    .expect("classe registrada antes da avaliação de estáticos");
                let index = info
                    .static_fields
                    .binary_search_by(|field| field.name.cmp(member.name))
                    .expect("campo estático registrado antes da avaliação");
                info.static_fields[index].constant = Some(Rc::new(value));
            }
        }
        for method in &class.static_methods {
            validator.function(method)?;
        }
    }
    // As receitas const ficam prontas antes de qualquer corpo, porque `const
    // C(1)` pode aparecer em qualquer ponto do programa.
    for class in &program.classes {
        if class.is_library_globals {
            continue;
        }
        validator.build_const_plans(class)?;
    }
    for function in &program.functions {
        validator.function(function)?;
    }
    validator.type_parameters = Rc::new(Vec::new());
    for class in &program.classes {
        if class.is_library_globals {
            continue;
        }
        validator.validate_enum(class)?;
        validator.current_class = None;
        for factory in &class.factories {
            validator.factory_class = Some(class.id);
            validator.function(factory)?;
            validator.factory_class = None;
        }
        validator.current_class = Some(class.id);
        validator.validate_constructor_fields(class)?;
        validator.current_class = None;
        for field in &class.fields {
            validator.current_class = Some(class.id);
            validator.in_field_initializer = true;
            validator.check_type_name(field.ty, field.span)?;
            if matches!(field.ty, Type::Void | Type::Null) {
                return Err(Diagnostic::new("Unsupported field type", field.span));
            }
            if let Some(parent) = class.superclass
                && (validator.field(parent, field.name).is_some()
                    || validator.method(parent, field.name).is_some())
            {
                return Err(Diagnostic::new(
                    "Redeclaring an inherited field or replacing a method with a field is unsupported",
                    field.span,
                ));
            }
            if !class.enum_values.is_empty() {
                validator.in_field_initializer = false;
                validator.current_class = None;
                continue;
            }
            if class.is_mixin_application {
                validator.in_field_initializer = false;
                validator.current_class = None;
                continue;
            }
            if let Some(initializer) = &field.initializer {
                validator.require_type(
                    validator.value_expected(initializer, Some(field.ty))?,
                    field.ty,
                    field.span,
                )?;
            }
            validator.in_field_initializer = false;
            validator.current_class = None;
        }
        for method in &class.methods {
            if let Some(parent) = class.superclass {
                if validator.field(parent, method.name).is_some() {
                    return Err(Diagnostic::new(
                        "Method conflicts with inherited field",
                        method.span,
                    ));
                }
                if let Some(base) = validator.method(parent, method.name) {
                    let actual = validator.classes[&class.id]
                        .methods
                        .get(method.name)
                        .expect("método próprio registrado na tabela da classe");
                    validator.compatible_parameters(
                        actual,
                        base,
                        method.span,
                        "Incompatible override arity",
                    )?;
                    if base.result != Type::Void {
                        validator.require_type(method.return_type, base.result, method.span)?;
                    }
                }
            }
            validator.current_class = Some(class.id);
            if !class.is_mixin_application {
                validator.function(method)?;
            }
            validator.current_class = None;
        }
        for constructor in class
            .constructor
            .iter()
            .chain(class.named_constructors.iter().map(|c| &c.constructor))
        {
            validator.current_class = Some(class.id);
            validator.constructor_body(constructor)?;
            validator.current_class = None;
        }
    }
    for class in &program.classes {
        if class.is_library_globals {
            continue;
        }
        validator.current_class = Some(class.id);
        for method in &class.abstract_methods {
            if class
                .superclass
                .is_some_and(|parent| validator.field(parent, method.name).is_some())
            {
                return Err(Diagnostic::new(
                    "Abstract method conflicts with inherited field",
                    method.span,
                ));
            }
            validator.signature_only(method)?;
            if !method.body.is_empty() {
                return Err(Diagnostic::new(
                    "Abstract method must not have a body",
                    method.span,
                ));
            }
        }
        validator.validate_contracts(class.id, class.span)?;
        validator.current_class = None;
    }
    for (index, extension) in program.extensions.iter().enumerate() {
        validator.current_extension = Some(index);
        validator.current_class = if let Type::Class(id) = extension.on_type {
            Some(id)
        } else {
            None
        };
        for method in &extension.methods {
            validator.function(method)?;
        }
        validator.current_class = None;
        validator.current_extension = None;
    }
    validator.type_parameters = Rc::new(Vec::new());
    validator.return_type = Type::Void;
    validator.in_async = program.main_is_async;
    validator.in_arrow = program.main_is_arrow;
    validator.loop_depth = 0;
    validator.labels.clear();
    validator.catch_depth = 0;
    validator.block(&program.statements)?;
    let resolution = std::mem::take(&mut *validator.resolution.borrow_mut());
    Ok(resolution)
}
impl<'a> Validator<'a> {
    /// Valida arestas nominais antes de qualquer busca recursiva de membros.
    fn validate_class_graph(&self, program: &Program<'a>) -> Result<(), Diagnostic> {
        for class in &program.classes {
            if class.is_library_globals {
                continue;
            }
            let mut active = HashSet::new();
            let mut finished = HashSet::new();
            let mut stack = vec![(class.id, false)];
            while let Some((id, exit)) = stack.pop() {
                if exit {
                    active.remove(&id);
                    finished.insert(id);
                    continue;
                }
                if finished.contains(&id) {
                    continue;
                }
                if !active.insert(id) {
                    return Err(Diagnostic::new(
                        "Inheritance cycle (extends/implements)",
                        class.span,
                    ));
                }
                let info = self.classes.get(&id).ok_or_else(|| {
                    Diagnostic::new("Unknown superclass or interface", class.span)
                })?;
                stack.push((id, true));
                for parent in info.superclass.iter().chain(&info.interfaces) {
                    stack.push((*parent, false));
                }
            }
            let mut edges = HashSet::new();
            self.validate_modifiers(class)?;
            if let Some(parent) = class.superclass {
                let base = &self.classes[&parent];
                if base.is_interface && base.library_id != class.library_id {
                    return Err(Diagnostic::new(
                        "Cannot extend an interface class from another library",
                        class.span,
                    ));
                }
            }
            for &id in &class.interfaces {
                if !edges.insert(id) {
                    return Err(Diagnostic::new(
                        "Duplicate implemented interface",
                        class.span,
                    ));
                }
                if self
                    .ancestors(id)
                    .iter()
                    .any(|id| !self.classes[id].fields.is_empty())
                    && !(class.is_mixin_application && class.mixin_origin == Some(id))
                {
                    return Err(Diagnostic::new(
                        "Interfaces with fields require unsupported getter/setter dispatch",
                        class.span,
                    ));
                }
            }
            for parent in class.superclass.iter().chain(&class.interfaces) {
                if !self.classes[parent].enum_values.is_empty() {
                    return Err(Diagnostic::new(
                        "Enums cannot be extended or implemented",
                        class.span,
                    ));
                }
            }
            if !class.enum_values.is_empty() {
                if class.is_abstract
                    || class.superclass.is_some()
                    || !class.abstract_methods.is_empty()
                {
                    return Err(Diagnostic::new(
                        "Enhanced enums are unsupported",
                        class.span,
                    ));
                }
                let mut values = HashSet::new();
                for &name in &class.enum_values {
                    if !values.insert(name)
                        || matches!(
                            name,
                            "index"
                                | "values"
                                | "toString"
                                | "hashCode"
                                | "runtimeType"
                                | "noSuchMethod"
                        )
                    {
                        return Err(Diagnostic::new(
                            "Duplicate or reserved enum value",
                            class.span,
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    /// Reúne o fecho nominal transitivo; implements produz subtipo, não implementação.
    fn ancestors(&self, id: u32) -> HashSet<u32> {
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if seen.insert(id)
                && let Some(info) = self.classes.get(&id)
            {
                stack.extend(info.superclass);
                stack.extend(info.interfaces.iter().copied());
            }
        }
        seen
    }
    /// Confere os argumentos nomeados de uma chamada contra a assinatura.
    ///
    /// As listas são pequenas e percorridas na ordem escrita: duplicatas
    /// comparam-se com os rótulos já vistos e a ausência de um obrigatório
    /// varre a lista ordenada da assinatura. Nenhuma tabela é construída.
    fn check_named_arguments(
        &self,
        arguments: &[Expr<'a>],
        named: &[NamedParameter<'a>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        for (index, argument) in arguments.iter().enumerate() {
            let ExprKind::NamedArgument { label, value } = &argument.kind else {
                return Err(Diagnostic::new(
                    "Positional arguments must precede named arguments",
                    argument.span,
                ));
            };
            if arguments[..index].iter().any(|previous| {
                matches!(&previous.kind, ExprKind::NamedArgument { label: seen, .. } if seen == label)
            }) {
                return Err(Diagnostic::new(
                    format!("Duplicate named argument '{label}'"),
                    argument.span,
                ));
            }
            let Ok(position) = named.binary_search_by(|candidate| candidate.label.cmp(label))
            else {
                return Err(Diagnostic::new(
                    format!("Unknown named argument '{label}'"),
                    argument.span,
                ));
            };
            let expected = named[position].ty;
            self.require_type(
                self.value_expected(value, Some(expected))?,
                expected,
                value.span,
            )?;
        }
        for parameter in named.iter().filter(|parameter| parameter.is_required) {
            if !arguments.iter().any(|argument| {
                matches!(&argument.kind, ExprKind::NamedArgument { label, .. } if *label == parameter.label)
            }) {
                return Err(Diagnostic::new(
                    format!("Missing required named argument '{}'", parameter.label),
                    span,
                ));
            }
        }
        Ok(())
    }
    /// Confere posicionais e nomeados de uma chamada contra a forma compacta.
    ///
    /// `arity` só é consultado no caminho de erro, para que cada forma de
    /// chamada preserve sua própria mensagem de contagem de argumentos.
    fn check_call_arguments(
        &self,
        arguments: &[Expr<'a>],
        positional: &[Type],
        required_positional: usize,
        named: &[NamedParameter<'a>],
        span: Span,
        arity: impl Fn(usize) -> Diagnostic,
    ) -> Result<(), Diagnostic> {
        let (written, labelled) = split_arguments(arguments, span)?;
        if written.len() < required_positional || written.len() > positional.len() {
            return Err(arity(written.len()));
        }
        for (argument, expected) in written.iter().zip(positional) {
            self.require_type(
                self.value_expected(argument, Some(*expected))?,
                *expected,
                argument.span,
            )?;
        }
        self.check_named_arguments(labelled, named, span)
    }
    /// Exige que a assinatura aceite toda chamada válida para o contrato herdado.
    ///
    /// Posicionais são contravariantes; obrigatoriedade só pode afrouxar. Um
    /// nomeado do contrato não pode desaparecer nem passar a ser obrigatório.
    fn compatible_parameters(
        &self,
        actual: &Signature<'a>,
        expected: &Signature<'a>,
        span: Span,
        arity: &str,
    ) -> Result<(), Diagnostic> {
        if actual.required_positional > expected.required_positional
            || actual.parameters.len() < expected.parameters.len()
        {
            return Err(Diagnostic::new(arity, span));
        }
        for (&expected_ty, &actual_ty) in expected.parameters.iter().zip(&actual.parameters) {
            self.require_type(expected_ty, actual_ty, span)?;
        }
        for parameter in &expected.named {
            let Some(overridden) = actual.named(parameter.label) else {
                return Err(Diagnostic::new(
                    if parameter.is_required {
                        format!(
                            "Override drops required named parameter '{}'",
                            parameter.label
                        )
                    } else {
                        format!("Override drops named parameter '{}'", parameter.label)
                    },
                    span,
                ));
            };
            self.require_type(parameter.ty, overridden.ty, span)?;
        }
        for parameter in &actual.named {
            if parameter.is_required
                && !expected
                    .named(parameter.label)
                    .is_some_and(|base| base.is_required)
            {
                return Err(Diagnostic::new(
                    format!(
                        "Override adds required named parameter '{}'",
                        parameter.label
                    ),
                    span,
                ));
            }
        }
        Ok(())
    }
    /// Procura apenas corpos concretos herdados via extends, ignorando redeclarações abstratas.
    fn implementation(&self, id: u32, name: &str) -> Option<&Signature<'a>> {
        let class = self.classes.get(&id)?;
        if !class.abstract_methods.contains(name)
            && let Some(method) = class.methods.get(name)
        {
            return Some(method);
        }
        class
            .superclass
            .and_then(|parent| self.implementation(parent, name))
    }
    /// Exige parâmetros contravariantes e retorno covariante; void aceita resultado descartado.
    fn compatible_signature(
        &self,
        actual: &Signature<'a>,
        expected: &Signature<'a>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if actual.is_getter != expected.is_getter {
            return Err(Diagnostic::new(
                "Getter and method contracts are incompatible",
                span,
            ));
        }
        self.compatible_parameters(actual, expected, span, "Incompatible method contract arity")?;
        if expected.result != Type::Void {
            // Covariância estrita: sem promoção (int não é subtipo de double).
            self.require_subtype(actual.result, expected.result, span)?;
        }
        Ok(())
    }
    /// Confere todos os contratos, inclusive requisitos transitivos de interfaces.
    fn validate_contracts(&self, id: u32, span: Span) -> Result<(), Diagnostic> {
        let class = &self.classes[&id];
        let mut required = std::collections::BTreeMap::<&str, Vec<&Signature<'a>>>::new();
        let mut ids = self.ancestors(id).into_iter().collect::<Vec<_>>();
        ids.sort_unstable();
        for ancestor in ids {
            for (&name, signature) in &self.classes[&ancestor].methods {
                required.entry(name).or_default().push(signature);
            }
        }
        for (name, contracts) in required {
            // A assinatura estática escolhida deve atender todos os contratos.
            // Combinações sem uma assinatura herdada utilizável exigem declaração explícita.
            let selected = self.method(id, name).expect("contrato possui assinatura");
            for contract in &contracts {
                self.compatible_signature(selected, contract, span)?;
            }
            if class.is_abstract {
                continue;
            }
            if let Some(implementation) = self.implementation(id, name) {
                for contract in &contracts {
                    self.compatible_signature(implementation, contract, span)?;
                }
            } else if !class.is_abstract {
                return Err(Diagnostic::new(
                    format!("Missing concrete implementation of '{name}'"),
                    span,
                ));
            }
        }
        Ok(())
    }
    /// Valida assinatura abstrata sem exigir corpo ou retorno executável.
    fn signature_only(&self, function: &dartforge_syntax::Function<'a>) -> Result<(), Diagnostic> {
        if function.native_binding.is_some() {
            return Err(Diagnostic::new(
                "@Native is supported only on external top-level functions",
                function.span,
            ));
        }
        if function.is_getter && !function.parameters.is_empty()
            || !function.type_parameters.is_empty()
        {
            return Err(Diagnostic::new(
                "Unsupported abstract getter or generic method signature",
                function.span,
            ));
        }
        self.check_type_name(function.return_type, function.span)?;
        self.validate_parameters(&function.parameters, None)?;
        let mut names = HashSet::new();
        for p in &function.parameters {
            self.check_type_name(p.ty, p.span)?;
            if p.ty == Type::Void || !names.insert(p.name) {
                return Err(Diagnostic::new(
                    "Invalid or duplicate abstract parameter",
                    p.span,
                ));
            }
        }
        Ok(())
    }
    /// Valida a forma e o valor padrão de um parâmetro isolado.
    ///
    /// O padrão precisa ser uma expressão constante escalar: listas const são
    /// canônicas em Dart e a emissão JavaScript recriaria o valor por chamada.
    fn validate_parameter_default(
        &self,
        kind: ParameterKind,
        ty: Type,
        default: Option<&Expr<'a>>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (kind, default) {
            (ParameterKind::RequiredPositional, Some(_)) => {
                return Err(Diagnostic::new(
                    "Only optional or named parameters accept a default value",
                    span,
                ));
            }
            (ParameterKind::Named { required: true }, Some(_)) => {
                return Err(Diagnostic::new(
                    "A required named parameter cannot have a default value",
                    span,
                ));
            }
            (ParameterKind::OptionalPositional, None) if !self.may_be_null(ty) => {
                return Err(Diagnostic::new(
                    "Optional positional parameter requires a default value or a nullable type",
                    span,
                ));
            }
            (ParameterKind::Named { required: false }, None) if !self.may_be_null(ty) => {
                return Err(Diagnostic::new(
                    "Optional named parameter requires a default value or a nullable type",
                    span,
                ));
            }
            _ => {}
        }
        let Some(default) = default else {
            return Ok(());
        };
        // O avaliador de constantes decide se a expressão é constante; nenhum
        // binding local é visível na posição de um parâmetro.
        let value = constants::evaluate(default, &self.resolution.borrow(), &|_| None, &|inner| {
            Err(Diagnostic::new(
                "A default value must be a literal scalar constant",
                inner.span,
            ))
        })?;
        // O linker não reescreve o span de um padrão, então o valor não pode
        // depender de tabela indexada por span: só literais escalares passam,
        // e a emissão os escreve diretamente.
        if !is_scalar_literal(default) {
            return Err(Diagnostic::new(
                "A default value must be a literal scalar constant",
                default.span,
            ));
        }
        let actual = match value {
            ConstValue::Int(_) => Type::Int,
            ConstValue::Double(_) => Type::Double,
            ConstValue::String(_) => Type::String,
            ConstValue::Bool(_) => Type::Bool,
            // `is_scalar_literal` já rejeitou enum e lista; resta apenas null.
            _ => Type::Null,
        };
        self.require_type(actual, ty, default.span)
    }
    /// Valida grupos, rótulos e padrões da lista de parâmetros de uma função.
    ///
    /// `positional_only` recebe a mensagem do contexto que ainda não emite
    /// prólogo de opcionais; None aceita as três formas de passagem.
    fn validate_parameters(
        &self,
        parameters: &[dartforge_syntax::Parameter<'a>],
        positional_only: Option<&str>,
    ) -> Result<(), Diagnostic> {
        for (index, parameter) in parameters.iter().enumerate() {
            if parameter.kind == ParameterKind::RequiredPositional {
                // Caminho comum: só resta rejeitar um padrão sem grupo opcional.
                if parameter.default.is_some() {
                    return Err(Diagnostic::new(
                        "Only optional or named parameters accept a default value",
                        parameter.span,
                    ));
                }
                continue;
            }
            if let Some(reason) = positional_only {
                return Err(Diagnostic::new(reason, parameter.span));
            }
            if parameter.kind.is_named() && parameter.name.starts_with('_') {
                return Err(Diagnostic::new(
                    "A named parameter accepts a private name only as an initializing formal",
                    parameter.span,
                ));
            }
            if parameter.kind.is_named()
                && parameters[..index].iter().any(|previous| {
                    previous.kind.is_named() && previous.label() == parameter.label()
                })
            {
                return Err(Diagnostic::new(
                    format!("Duplicate named parameter '{}'", parameter.label()),
                    parameter.span,
                ));
            }
            self.validate_parameter_default(
                parameter.kind,
                parameter.ty,
                parameter.default.as_deref(),
                parameter.span,
            )?;
        }
        Ok(())
    }
    /// Detecta membros que ocultariam uma referência global sem receptor explícito.
    fn has_implicit_member(&self, name: &str) -> bool {
        self.factory_class
            .is_some_and(|id| self.field(id, name).is_some() || self.method(id, name).is_some())
            || self
                .current_extension
                .is_some_and(|index| self.extensions[index].methods.contains_key(name))
            || self
                .current_class
                .is_some_and(|id| self.field(id, name).is_some() || self.method(id, name).is_some())
    }
    /// Valida uma função ou método com parâmetros mutáveis em escopo externo ao corpo.
    fn function(&mut self, function: &dartforge_syntax::Function<'a>) -> Result<(), Diagnostic> {
        self.in_arrow = function.is_arrow;
        self.in_async = function.is_async;
        self.in_constructor = false;
        self.type_parameters = Rc::new(function.type_parameters.clone());
        if function
            .type_parameters
            .iter()
            .map(|p| p.name)
            .collect::<HashSet<_>>()
            .len()
            != function.type_parameters.len()
        {
            return Err(Diagnostic::new("Duplicate type parameter", function.span));
        }
        self.validate_bounds()?;
        self.switch_depth = 0;
        self.inferred_returns = None;
        if function.is_getter && !function.parameters.is_empty() {
            return Err(Diagnostic::new(
                "Getters cannot have parameters",
                function.span,
            ));
        }
        if (self.current_class.is_some() || self.current_extension.is_some())
            && !function.type_parameters.is_empty()
        {
            return Err(Diagnostic::new(
                "Generic methods are unsupported",
                function.span,
            ));
        }
        self.check_type_name(function.return_type, function.span)?;
        self.validate_parameters(
            &function.parameters,
            if self.current_extension.is_some() {
                Some("Extension methods support only required positional parameters")
            } else if !function.type_parameters.is_empty() {
                Some("Generic functions support only required positional parameters")
            } else if function.native_binding.is_some() {
                Some("@Native functions support only required positional parameters")
            } else {
                None
            },
        )?;
        let mut parameters = HashMap::new();
        for parameter in &function.parameters {
            self.check_type_name(parameter.ty, parameter.span)?;
            if parameter.ty == Type::Void {
                return Err(Diagnostic::new(
                    "Void parameters are unsupported",
                    parameter.span,
                ));
            }
            if is_wildcard(parameter.name) {
                // Dart 3.7: parâmetros `_` não declaram nome e podem repetir.
                continue;
            }
            if parameters
                .insert(
                    parameter.name,
                    Binding {
                        constant: None,
                        ty: Some(parameter.ty),
                        is_final: false,
                        promoted: None,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new(
                    format!("Duplicate parameter '{}'", parameter.name),
                    parameter.span,
                ));
            }
        }
        if function
            .annotations
            .iter()
            .any(|a| matches!(a.kind, dartforge_syntax::AnnotationKind::Override))
            && self.current_class.is_none()
            && self.current_extension.is_none()
        {
            return Err(Diagnostic::new(
                "@override requires an instance member in this subset",
                function.span,
            ));
        }
        if function.native_binding.is_some() {
            if function.is_async {
                return Err(Diagnostic::new(
                    "Native functions cannot be async",
                    function.span,
                ));
            }
            return self.validate_native(function);
        }
        self.scopes.push(parameters);
        self.return_type = if function.is_async {
            self.async_result(function.return_type, function.span)?
        } else {
            function.return_type
        };
        self.loop_depth = 0;
        self.labels.clear();
        self.catch_depth = 0;
        self.block(&function.body)?;
        self.scopes.pop();
        if self.return_type != Type::Void
            && self
                .require_type(Type::Null, self.return_type, function.span)
                .is_err()
            && !self.returns(&function.body)
        {
            return Err(Diagnostic::new(
                format!(
                    "Function '{}' may complete without returning a value",
                    function.name
                ),
                function.span,
            ));
        }
        self.in_async = false;
        self.in_arrow = false;
        Ok(())
    }
    /// Procura um campo na classe nominal e em suas bases já verificadas.
    fn field(&self, id: u32, name: &str) -> Option<FieldInfo> {
        let class = self.classes.get(&id)?;
        if !class.enum_values.is_empty() {
            return match name {
                "name" if !class.methods.contains_key("name") => Some(FieldInfo {
                    ty: Type::String,
                    is_final: true,
                }),
                "index" => Some(FieldInfo {
                    ty: Type::Int,
                    is_final: true,
                }),
                _ => class.fields.get(name).copied(),
            };
        }
        class
            .fields
            .get(name)
            .copied()
            .or_else(|| class.superclass.and_then(|base| self.field(base, name)))
    }
    /// Procura assinatura nominal sem revisitar caminhos compartilhados de interfaces.
    fn method(&self, id: u32, name: &str) -> Option<&Signature<'a>> {
        let mut stack = vec![id];
        let mut visited = HashSet::new();
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            let class = self.classes.get(&id)?;
            if let Some(method) = class.methods.get(name) {
                return Some(method);
            }
            stack.extend(class.interfaces.iter().rev().copied());
            stack.extend(class.superclass);
        }
        None
    }
    /// Compara tipos primitivos e relações nominais de subtipo entre classes.
    ///
    /// Inclui a promoção de atribuição do oráculo Dart 3.6.2 (`int→double`,
    /// `int|double→num`); contratos de override que exigem subtipagem estrita
    /// usam [`Validator::require_subtype`].
    fn require_type(&self, actual: Type, expected: Type, span: Span) -> Result<(), Diagnostic> {
        self.require_inner(actual, expected, span, true)
    }
    /// Subtipagem estrita sem promoção numérica, para covariância de retorno.
    ///
    /// `int` não é subtipo de `double` no Dart: um override não pode
    /// estreitar `double` para `int` no retorno, embora `return 1;` seja
    /// válido num corpo com retorno `double` (posição de atribuição).
    fn require_subtype(&self, actual: Type, expected: Type, span: Span) -> Result<(), Diagnostic> {
        // int e double são subtipos reais de num, mesmo sem promoção.
        if numeric_subtype(actual, expected) {
            return Ok(());
        }
        self.require_inner(actual, expected, span, false)
    }
    /// Núcleo compartilhado das duas comparações; `promote` libera int→double.
    fn require_inner(
        &self,
        actual: Type,
        expected: Type,
        span: Span,
        promote: bool,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            return Ok(());
        }
        if expected == Type::NullableObject && actual != Type::Void && actual != Type::Inferred {
            return Ok(());
        }
        if expected == Type::Object
            && actual != Type::Void
            && actual != Type::Inferred
            && !self.may_be_null(actual)
        {
            return Ok(());
        }
        if let Type::NullableParameter(id) = expected
            && (actual == Type::Null || actual == Type::Parameter(id))
        {
            return Ok(());
        }
        if matches!(actual, Type::Parameter(_) | Type::NullableParameter(_)) {
            return self.require_inner(self.upper_bound(actual), expected, span, promote);
        }
        // Promoção de atribuição (oráculo Dart 3.6.2, semântica WEB/JS Number):
        // int vale onde double ou num é esperado; double vale onde num é
        // esperado. Anulabilidade continua exigida dos dois lados.
        if promote && numeric_promotion(actual, expected) {
            return Ok(());
        }
        if let Some(TypeShape::Nullable(inner)) = self.shape(expected) {
            if actual == Type::Null {
                return Ok(());
            }
            return self.require_inner(self.without_null(actual), inner, span, promote);
        }
        if matches!(actual, Type::Applied(_)) || matches!(expected, Type::Applied(_)) {
            return self.require_structural(actual, expected, span);
        }
        let source = match actual {
            Type::Class(id) | Type::NullableClass(id) => Some(id),
            _ => None,
        };
        let target = match expected {
            Type::Class(id) | Type::NullableClass(id) => Some(id),
            _ => None,
        };
        if let (Some(source), Some(target)) = (source, target)
            && (!is_nullable(actual) || is_nullable(expected))
            && self.ancestors(source).contains(&target)
        {
            return Ok(());
        }
        require_type(actual, expected, span)
    }
    /// Obtém a classe de um receptor comprovadamente não anulável.
    fn receiver_class(&self, receiver: &Expr<'a>) -> Result<u32, Diagnostic> {
        match self.upper_bound(self.value(receiver)?) {
            Type::Class(id) => Ok(id),
            _ => Err(Diagnostic::new(
                "Member access requires a non-null class instance",
                receiver.span,
            )),
        }
    }
    /// Rejeita interpolar valores sem `toString` representável no subconjunto.
    ///
    /// O conjunto é o mesmo de `print`: int, String, bool, Null e as coleções e
    /// records cujos elementos também são representáveis. Instâncias de classe,
    /// enums, funções, Future, Duration e Timer ficam de fora porque o
    /// subconjunto ainda não tem o protocolo `toString` do Dart — emitir
    /// qualquer coisa para eles produziria texto diferente do Dart 3.6.2.
    fn interpolable(&self, expression: &Expr<'a>) -> Result<(), Diagnostic> {
        let ty = self.value(expression)?;
        if self.printable_type(ty) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                "String interpolation requires unsupported toString semantics for this value",
                expression.span,
            ))
        }
    }
    /// Rejeita impressão de objetos até existir o protocolo Dart de toString.
    fn printable(&self, expression: &Expr<'a>) -> Result<(), Diagnostic> {
        let ty = self.value(expression)?;
        if self.printable_type(ty) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                "Printing objects or function values requires unsupported toString semantics",
                expression.span,
            ))
        }
    }

    /// Recusa ler `this` antes de a superclasse concluir sua inicialização.
    ///
    /// # Erros
    /// Devolve diagnóstico enquanto a lista de inicialização estiver em curso.
    fn reject_early_this(&self, span: Span) -> Result<(), Diagnostic> {
        if self.in_initializer_list {
            return Err(Diagnostic::new(
                "Cannot read 'this' before the superclass constructor runs",
                span,
            ));
        }
        Ok(())
    }
    /// Procura o nome do escopo mais interno até o mais externo.
    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
    /// Exige que o nome exista e tenha concluído sua inicialização.
    fn initialized(&self, name: &str, span: Span) -> Result<Binding, Diagnostic> {
        let binding = self
            .lookup(name)
            .ok_or_else(|| Diagnostic::new(format!("Unknown identifier '{name}'"), span))?;
        if binding.ty.is_none() {
            return Err(Diagnostic::new(
                format!(
                    "Local variable '{name}' is referenced before its declaration or in its own initializer"
                ),
                span,
            ));
        }
        Ok(binding)
    }
    /// Pré-declara os locais e valida as instruções em um novo escopo.
    fn block(&mut self, statements: &[Statement<'a>]) -> Result<(), Diagnostic> {
        let mut scope = HashMap::new();
        // Locais ocultam nomes externos em todo o bloco, inclusive antes da declaração.
        for statement in statements {
            if let StatementKind::RecordDestructure {
                is_final,
                positional,
                named,
                ..
            } = &statement.kind
            {
                let names = positional
                    .iter()
                    .copied()
                    .chain(named.iter().map(|(_, name, span)| (*name, *span)));
                for (name, span) in names {
                    if name != "_"
                        && scope
                            .insert(
                                name,
                                Binding {
                                    constant: None,
                                    ty: None,
                                    is_final: *is_final,
                                    promoted: None,
                                },
                            )
                            .is_some()
                    {
                        return Err(Diagnostic::new(
                            format!("Duplicate local variable '{name}'"),
                            span,
                        ));
                    }
                }
            }
            if let StatementKind::Variable {
                name,
                is_final,
                is_const,
                ..
            } = statement.kind
                && !is_wildcard(name)
                && scope
                    .insert(
                        name,
                        Binding {
                            constant: None,
                            ty: None,
                            is_final: is_final || is_const,
                            promoted: None,
                        },
                    )
                    .is_some()
            {
                return Err(Diagnostic::new(
                    format!("Duplicate local variable '{name}'"),
                    statement.span,
                ));
            }
        }
        self.scopes.push(scope);
        let result = statements
            .iter()
            .try_for_each(|statement| self.statement(statement));
        self.scopes.pop();
        result
    }
    /// Valida uma instrução e seus efeitos sobre o escopo semântico.
    fn statement(&mut self, statement: &Statement<'a>) -> Result<(), Diagnostic> {
        match &statement.kind {
            StatementKind::RecordDestructure {
                positional,
                named,
                initializer,
                ..
            } => self.record_destructure(positional, named, initializer, statement.span),
            StatementKind::Variable {
                is_const,
                name,
                annotation,
                initializer,
                ..
            } => {
                let actual = self.value_expected(initializer, *annotation)?;
                if let Some(expected) = annotation {
                    self.check_type_name(*expected, statement.span)?;
                    self.require_type(actual, *expected, initializer.span)?;
                }
                if annotation.is_none() && actual == Type::Null {
                    return Err(Diagnostic::new(
                        "Inference from null requires dynamic, which is unsupported; use an explicit nullable type",
                        initializer.span,
                    ));
                }
                let constant = if *is_const {
                    Some(Rc::new(self.evaluate_constant(initializer)?))
                } else {
                    None
                };
                let promoted = if !self.captured_writes.contains(name)
                    && !self.may_be_null(actual)
                    && self.may_be_null(annotation.unwrap_or(actual))
                {
                    Some(self.without_null(annotation.unwrap_or(actual)))
                } else {
                    None
                };
                if is_wildcard(name) {
                    // Dart 3.7: `_` avalia o inicializador e descarta a ligação.
                    return Ok(());
                }
                let binding = self
                    .scopes
                    .last_mut()
                    .expect("current block scope")
                    .get_mut(name)
                    .expect("predeclared local");
                binding.constant = constant;
                binding.ty = Some(annotation.unwrap_or(actual));
                binding.promoted = promoted;
                Ok(())
            }
            StatementKind::FieldAssign {
                receiver,
                name,
                value,
            } => {
                let id = self.receiver_class(receiver)?;
                let field = self
                    .field(id, name)
                    .ok_or_else(|| Diagnostic::new("Unknown field", statement.span))?;
                if field.is_final {
                    return Err(Diagnostic::new(
                        "Cannot assign to final field",
                        statement.span,
                    ));
                }
                self.require_type(
                    self.value_expected(value, Some(field.ty))?,
                    field.ty,
                    value.span,
                )
            }
            StatementKind::Assign { name, value } => {
                self.reject_factory_instance(name, statement.span)?;
                if self.lookup(name).is_none()
                    && let Some(id) = self.current_class
                    && let Some(field) = self.field(id, name)
                {
                    if field.is_final {
                        return Err(Diagnostic::new(
                            "Cannot assign to final field",
                            statement.span,
                        ));
                    }
                    self.implicit(statement.span);
                    return self.require_type(
                        self.value_expected(value, Some(field.ty))?,
                        field.ty,
                        value.span,
                    );
                }
                if self.lookup(name).is_none()
                    && !self.has_implicit_member(name)
                    && let Some(global) = self.global(name)
                {
                    let (ty, is_final) = (global.ty, global.is_final);
                    if is_final {
                        return Err(Diagnostic::new(
                            format!("Cannot assign to final top-level variable '{name}'"),
                            statement.span,
                        ));
                    }
                    self.global_access(statement.span);
                    return self.require_type(
                        self.value_expected(value, Some(ty))?,
                        ty,
                        value.span,
                    );
                }
                let binding = self.initialized(name, statement.span)?;
                if binding.is_final {
                    return Err(Diagnostic::new(
                        format!("Cannot assign to final variable '{name}'"),
                        statement.span,
                    ));
                }
                let actual = self.value_expected(value, binding.ty)?;
                self.require_type(actual, binding.ty.expect("initialized binding"), value.span)?;
                let promoted = if !self.captured_writes.contains(name)
                    && !self.may_be_null(actual)
                    && self.may_be_null(binding.ty.expect("initialized binding"))
                {
                    Some(self.without_null(binding.ty.expect("initialized binding")))
                } else {
                    None
                };
                // A escrita remove a promoção anterior; valores não nulos estabelecem uma nova.
                let target = self
                    .scopes
                    .iter_mut()
                    .rev()
                    .find_map(|scope| scope.get_mut(name))
                    .expect("resolved binding");
                target.promoted = promoted;
                Ok(())
            }
            StatementKind::Print(expression) => {
                if self.lookup("print").is_some() || self.has_implicit_member("print") {
                    return Err(Diagnostic::new(
                        "Invocation of local 'print' is unsupported; it shadows the built-in function",
                        statement.span,
                    ));
                }
                self.printable(expression)
            }
            StatementKind::Switch { scrutinee, cases } => {
                self.switch_statement(scrutinee, cases, statement.span)
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                let ty = self.upper_bound(self.value(receiver)?);
                if let Some(TypeShape::Map {
                    key,
                    value: element,
                }) = self.shape(ty)
                {
                    self.require_type(self.value(index)?, key, index.span)?;
                    return self.require_type(
                        self.value_expected(value, Some(element))?,
                        element,
                        value.span,
                    );
                }
                let Some(TypeShape::List(element)) = self.shape(ty) else {
                    return Err(Diagnostic::new(
                        "Index assignment requires a List",
                        receiver.span,
                    ));
                };
                self.require_type(self.value(index)?, Type::Int, index.span)?;
                self.require_type(
                    self.value_expected(value, Some(element))?,
                    element,
                    value.span,
                )
            }
            StatementKind::Return(Some(_)) if self.in_constructor => Err(Diagnostic::new(
                "A generative constructor cannot return a value",
                statement.span,
            )),
            StatementKind::Return(value) if self.inferred_returns.is_some() => {
                let ty = if let Some(value) = value {
                    self.expression_expected(
                        value,
                        if self.return_type == Type::Inferred || self.return_type == Type::Void {
                            None
                        } else {
                            Some(self.return_type)
                        },
                    )?
                } else {
                    Type::Void
                };
                let ty = if self.in_async
                    && self.return_type != Type::Void
                    && self
                        .require_type(ty, self.return_type, statement.span)
                        .is_err()
                {
                    self.await_type(ty)
                } else {
                    ty
                };
                let ty = if self.in_async
                    && self.return_type == Type::Void
                    && matches!(self.shape(ty), Some(TypeShape::Future(Type::Void)))
                {
                    Type::Void
                } else {
                    ty
                };
                self.inferred_returns
                    .as_ref()
                    .expect("inferência ativa")
                    .borrow_mut()
                    .push(ty);
                Ok(())
            }
            StatementKind::Return(value) => match (self.return_type, value) {
                (expected, Some(expression)) if self.in_async => {
                    self.async_return(expression, expected)
                }
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(expression)) => {
                    self.require_type(self.expression(expression)?, Type::Void, expression.span)
                }
                (_, None) => Err(Diagnostic::new(
                    "A value must be returned from this function",
                    statement.span,
                )),
                (expected, Some(expression)) => self.require_type(
                    self.value_expected(expression, Some(expected))?,
                    expected,
                    expression.span,
                ),
            },
            StatementKind::Expression(expression) => self.expression(expression).map(|_| ()),
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
                let before = self.clone();
                self.promote(condition, true);
                self.block(then_body)?;
                let then_state = self.clone();
                *self = before.clone();
                self.promote(condition, false);
                if let Some(body) = else_body {
                    self.block(body)?;
                }
                let else_returns = else_body.as_ref().is_some_and(|body| self.returns(body));
                if self.returns(then_body) { /* Somente o outro ramo alcança a próxima instrução. */
                } else if else_returns {
                    *self = then_state;
                } else {
                    self.merge(&then_state);
                }
                Ok(())
            }
            StatementKind::While { condition, body } => {
                self.invalidate_writes(body);
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
                let before = self.clone();
                self.promote(condition, true);
                self.loop_body(body)?;
                *self = before;
                self.invalidate_writes(body);
                Ok(())
            }
            StatementKind::DoWhile { body, condition } => {
                self.invalidate_writes(body);
                let before = self.clone();
                self.loop_body(body)?;
                self.invalidate_writes(body);
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
                *self = before;
                self.invalidate_writes(body);
                Ok(())
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => self.for_loop(
                initializer.as_deref(),
                condition.as_ref(),
                update.as_deref(),
                body,
            ),
            StatementKind::Break | StatementKind::Continue => {
                if self.loop_depth == 0
                    && !(matches!(statement.kind, StatementKind::Break) && self.switch_depth > 0)
                {
                    Err(Diagnostic::new(
                        "break and continue require an enclosing loop",
                        statement.span,
                    ))
                } else {
                    Ok(())
                }
            }
            StatementKind::BreakLabel(label) | StatementKind::ContinueLabel(label) => {
                // A fronteira de closure preserva os rótulos externos após um
                // sentinela vazio (ver `collections.rs`): o que casa até ele
                // foi declarado numa função externa, como `label_in_outer_scope`
                // do Dart 3.6.2; o resto da função não tem sentinela alguma.
                let boundary = self
                    .labels
                    .iter()
                    .rposition(|candidate| candidate.is_empty());
                match self
                    .labels
                    .iter()
                    .rposition(|candidate| *candidate == *label)
                {
                    Some(position) if boundary.map_or(true, |index| position > index) => Ok(()),
                    Some(_) => Err(Diagnostic::new(
                        format!("Can't reference label '{label}' declared in an outer method."),
                        statement.span,
                    )),
                    None => Err(Diagnostic::new(
                        format!("Unknown loop label '{label}'"),
                        statement.span,
                    )),
                }
            }
            StatementKind::Labeled { label, body } => {
                // Rótulos repetem-se apenas dentro da mesma função: o que está
                // até o sentinela pertence a uma função externa e não conflita.
                let current = match self
                    .labels
                    .iter()
                    .rposition(|candidate| candidate.is_empty())
                {
                    Some(index) => &self.labels[index + 1..],
                    None => &self.labels[..],
                };
                if current.contains(label) {
                    return Err(Diagnostic::new(
                        format!("Duplicate loop label '{label}'"),
                        statement.span,
                    ));
                }
                self.labels.push(label);
                let result = self.statement(body);
                self.labels.pop();
                result
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => self.try_statement(body, catches, finally_body.as_ref()),
            StatementKind::Rethrow => {
                if self.catch_depth == 0 {
                    Err(Diagnostic::new(
                        "rethrow requires an enclosing catch clause",
                        statement.span,
                    ))
                } else {
                    Ok(())
                }
            }
            StatementKind::Assert { condition, message } => {
                self.assert_statement(condition, message.as_ref())
            }
            StatementKind::ForIn {
                is_final,
                name,
                annotation,
                iterable,
                body,
            } => self.for_in(
                name,
                *annotation,
                *is_final,
                iterable,
                body,
                statement.span,
            ),
            StatementKind::Block(statements) => self.block(statements),
        }
    }
    /// Valida o corpo em escopo próprio e restaura a profundidade inclusive em caso de erro.
    fn loop_body(&mut self, body: &[Statement<'a>]) -> Result<(), Diagnostic> {
        self.loop_depth += 1;
        let result = self.block(body);
        self.loop_depth -= 1;
        result
    }
    /// Mantém o inicializador visível à condição, atualização e corpo do laço clássico.
    fn for_loop(
        &mut self,
        initializer: Option<&Statement<'a>>,
        condition: Option<&Expr<'a>>,
        update: Option<&Statement<'a>>,
        body: &[Statement<'a>],
    ) -> Result<(), Diagnostic> {
        if let Some(initializer) = initializer
            && !matches!(
                initializer.kind,
                StatementKind::Variable { .. }
                    | StatementKind::Assign { .. }
                    | StatementKind::Expression(_)
            )
        {
            return Err(Diagnostic::new(
                "Unsupported for initializer statement",
                initializer.span,
            ));
        }
        if let Some(update) = update
            && !matches!(
                update.kind,
                StatementKind::Assign { .. } | StatementKind::Expression(_)
            )
        {
            return Err(Diagnostic::new(
                "Unsupported for update statement",
                update.span,
            ));
        }
        let mut scope = HashMap::new();
        if let Some(Statement {
            kind:
                StatementKind::Variable {
                    name,
                    is_final,
                    is_const,
                    ..
                },
            ..
        }) = initializer
            && !is_wildcard(name)
        {
            scope.insert(
                *name,
                Binding {
                    constant: None,
                    ty: None,
                    is_final: *is_final || *is_const,
                    promoted: None,
                },
            );
        }
        self.scopes.push(scope);
        let result = (|| {
            if let Some(initializer) = initializer {
                self.statement(initializer)?;
            }
            self.invalidate_writes(body);
            if let Some(update) = update {
                self.invalidate_writes(std::slice::from_ref(update));
            }
            if let Some(condition) = condition {
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
            }
            let before = self.clone();
            if let Some(condition) = condition {
                self.promote(condition, true);
            }
            self.loop_body(body)?;
            // Continue pode saltar qualquer promoção produzida no corpo.
            *self = before.clone();
            if let Some(condition) = condition {
                self.promote(condition, true);
            }
            self.invalidate_writes(body);
            if let Some(update) = update {
                self.statement(update)?;
            }
            *self = before;
            self.invalidate_writes(body);
            if let Some(update) = update {
                self.invalidate_writes(std::slice::from_ref(update));
            }
            Ok(())
        })();
        self.scopes.pop();
        result
    }
    /// Promove nomes quando uma condição simples comprova ausência de null.
    fn promote(&mut self, condition: &Expr<'a>, truth: bool) {
        match &condition.kind {
            ExprKind::TypeTest {
                operand,
                ty,
                negated,
            } if truth != *negated => {
                if let ExprKind::Identifier(name) = operand.kind
                    && !self.captured_writes.contains(name)
                    && let Some(declared) = self.lookup(name).and_then(|b| b.ty)
                    && self.require_type(*ty, declared, condition.span).is_ok()
                    && let Some(binding) =
                        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
                {
                    binding.promoted = Some(*ty);
                }
            }
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => self.promote(operand, !truth),
            ExprKind::Binary { op, left, right }
                if (*op == BinaryOp::And && truth) || (*op == BinaryOp::Or && !truth) =>
            {
                self.promote(left, truth);
                self.promote(right, truth);
            }
            ExprKind::Binary { op, left, right }
                if (*op == BinaryOp::NotEqual && truth) || (*op == BinaryOp::Equal && !truth) =>
            {
                let name = match (&left.kind, &right.kind) {
                    (ExprKind::Identifier(name), ExprKind::Null)
                    | (ExprKind::Null, ExprKind::Identifier(name)) => Some(name),
                    _ => None,
                };
                if let Some(name) = name
                    && !self.captured_writes.contains(name)
                    && let Some(ty) = self.lookup(name).and_then(|b| b.promoted.or(b.ty))
                {
                    let promoted = self.without_null(ty);
                    if let Some(binding) =
                        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
                    {
                        binding.promoted = Some(promoted);
                    }
                }
            }
            _ => {}
        }
    }
    /// Mantém somente promoções idênticas nos dois caminhos alcançáveis.
    fn merge(&mut self, other: &Self) {
        for (scope, other_scope) in self.scopes.iter_mut().zip(&other.scopes) {
            for (name, binding) in scope {
                if other_scope
                    .get(name)
                    .is_none_or(|other| other.promoted != binding.promoted)
                {
                    binding.promoted = None;
                }
            }
        }
    }
    /// Invalida conservadoramente nomes escritos em qualquer ramo de um laço.
    fn invalidate_writes(&mut self, statements: &[Statement<'a>]) {
        for statement in statements {
            match &statement.kind {
                StatementKind::Switch { cases, .. } => {
                    for case in cases {
                        self.invalidate_writes(&case.body);
                    }
                }
                StatementKind::Assign { name, .. } => {
                    // A invalidação por nome pode descartar promoções externas mesmo com sombreamento.
                    for scope in &mut self.scopes {
                        if let Some(binding) = scope.get_mut(name) {
                            binding.promoted = None;
                        }
                    }
                }
                StatementKind::Block(body)
                | StatementKind::While { body, .. }
                | StatementKind::DoWhile { body, .. }
                | StatementKind::ForIn { body, .. } => self.invalidate_writes(body),
                StatementKind::Labeled { body, .. } => {
                    self.invalidate_writes(std::slice::from_ref(body));
                }
                StatementKind::Try {
                    body,
                    catches,
                    finally_body,
                } => {
                    self.invalidate_writes(body);
                    for clause in catches {
                        self.invalidate_writes(&clause.body);
                    }
                    if let Some(body) = finally_body {
                        self.invalidate_writes(body);
                    }
                }
                StatementKind::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.invalidate_writes(then_body);
                    if let Some(body) = else_body {
                        self.invalidate_writes(body);
                    }
                }
                StatementKind::For {
                    initializer,
                    update,
                    body,
                    ..
                } => {
                    if let Some(init) = initializer {
                        self.invalidate_writes(std::slice::from_ref(init));
                    }
                    if let Some(update) = update {
                        self.invalidate_writes(std::slice::from_ref(update));
                    }
                    self.invalidate_writes(body);
                }
                _ => {}
            }
        }
    }
    /// Rejeita anotações cujo nome de tipo foi ocultado por uma declaração.
    fn check_type_name(&self, ty: Type, span: Span) -> Result<(), Diagnostic> {
        let name = match ty {
            Type::Parameter(id) | Type::NullableParameter(id) => {
                return if (id as usize) < self.type_parameters.len()
                    && self
                        .lookup(self.type_parameters[id as usize].name)
                        .is_none()
                {
                    Ok(())
                } else {
                    Err(Diagnostic::new("Unknown type parameter", span))
                };
            }
            Type::Inferred => {
                return Err(Diagnostic::new(
                    "Type requires unsupported dynamic inference",
                    span,
                ));
            }
            Type::Applied(_) => return self.check_shape_name(ty, span),
            Type::Void | Type::Null => return Ok(()),
            Type::Class(id) | Type::NullableClass(id) => {
                let name = self
                    .classes
                    .get(&id)
                    .ok_or_else(|| Diagnostic::new("Unknown class type", span))?
                    .name;
                if self.lookup(name).is_some() || self.has_implicit_member(name) {
                    return Err(Diagnostic::new(
                        "Local declaration shadows class type",
                        span,
                    ));
                }
                return Ok(());
            }
            Type::Int | Type::NullableInt => "int",
            Type::Double | Type::NullableDouble => "double",
            Type::Num | Type::NullableNum => "num",
            Type::String | Type::NullableString => "String",
            Type::Bool | Type::NullableBool => "bool",
            Type::Object | Type::NullableObject => "Object",
            Type::Duration => "Duration",
            Type::Timer => {
                if !self.async_library {
                    return Err(Diagnostic::new("Timer requires dart:async", span));
                }
                "Timer"
            }
        };
        if self.lookup(name).is_some()
            || self.functions.contains_key(name)
            || self.has_implicit_member(name)
        {
            Err(Diagnostic::new(
                format!(
                    "Declaration '{name}' shadows the type name; using it as a type is unsupported"
                ),
                span,
            ))
        } else {
            Ok(())
        }
    }
    /// Exige uma expressão que produza um valor diferente de void.
    fn value(&self, expression: &Expr<'a>) -> Result<Type, Diagnostic> {
        let ty = self.expression(expression)?;
        if ty == Type::Void {
            Err(Diagnostic::new(
                "A void expression cannot be used as a value",
                expression.span,
            ))
        } else {
            Ok(ty)
        }
    }
    /// Determina o tipo da expressão e valida operadores e chamadas.
    fn expression(&self, expression: &Expr<'a>) -> Result<Type, Diagnostic> {
        self.expression_expected(expression, None)
    }
    /// Determina o tipo sem contexto adicional; o wrapper registra o resultado para os backends.
    fn expression_inner(&self, expression: &Expr<'a>) -> Result<Type, Diagnostic> {
        match &expression.kind {
            ExprKind::NamedArgument { .. } => Err(Diagnostic::new(
                "A named argument is valid only in an argument list",
                expression.span,
            )),
            ExprKind::TypeTest { operand, ty, .. } => {
                self.value(operand)?;
                self.runtime_type(*ty, expression.span)?;
                Ok(Type::Bool)
            }
            ExprKind::Cast { operand, ty } => {
                let actual = self.value(operand)?;
                self.runtime_type(*ty, expression.span)?;
                // Apagamento Number: `as double`/`as num` só vale quando a
                // conversão é estaticamente verificável (`1 as double` lança
                // no oráculo e seria invisível em `typeof x === 'number'`).
                if matches!(
                    ty,
                    Type::Double | Type::Num | Type::NullableDouble | Type::NullableNum
                ) && self.require_subtype(actual, *ty, expression.span).is_err() {
                    return Err(Diagnostic::new(
                        "Casts to double or num require a statically known numeric operand",
                        expression.span,
                    ));
                }
                Ok(*ty)
            }
            ExprKind::Const(inner) => {
                let ty = self.expression(inner)?;
                self.evaluate_constant(expression)?;
                Ok(ty)
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.switch_expression(scrutinee, arms, expression.span, None)
            }
            ExprKind::Conditional {
                condition,
                then_value,
                else_value,
            } => self.conditional(condition, then_value, else_value, None, expression.span),
            ExprKind::Throw(value) => self.throw_expression(value, None),
            ExprKind::GenericCall {
                name,
                type_arguments,
                arguments,
            } => self.generic_call(name, type_arguments, arguments, expression.span, None),
            ExprKind::CascadeReceiver => self.cascade_receiver.ok_or_else(|| {
                Diagnostic::new("Cascade receiver outside a cascade", expression.span)
            }),
            ExprKind::Map { .. }
            | ExprKind::Cascade { .. }
            | ExprKind::Closure { .. }
            | ExprKind::List { .. }
            | ExprKind::Record { .. } => {
                unreachable!("expressões contextuais são tratadas no wrapper")
            }
            ExprKind::DotShorthand { .. } => Err(Diagnostic::new(
                "Dot shorthand requires a context type in this position",
                expression.span,
            )),
            ExprKind::NullAwareElement(_) => Err(Diagnostic::new(
                "Null-aware elements are only valid directly inside list or map literals",
                expression.span,
            )),
            ExprKind::Index { receiver, index } => {
                let ty = self.upper_bound(self.value(receiver)?);
                if let Some(TypeShape::Map { value, .. }) = self.shape(ty) {
                    self.value(index)?;
                    return Ok(self.nullable(value));
                }
                let Some(TypeShape::List(element)) = self.shape(ty) else {
                    return Err(Diagnostic::new(
                        "Index access requires a List",
                        receiver.span,
                    ));
                };
                self.require_type(self.value(index)?, Type::Int, index.span)?;
                Ok(element)
            }
            ExprKind::Invoke { callee, arguments } => {
                let ty = self.value(callee)?;
                self.invoke(ty, arguments, expression.span)
            }
            ExprKind::Await(operand) => {
                if !self.in_async {
                    return Err(Diagnostic::new(
                        "await requires an async body",
                        expression.span,
                    ));
                }
                Ok(self.await_type(self.expression(operand)?))
            }
            ExprKind::FutureValue { value, value_type } => {
                self.future_value(value.as_deref(), *value_type, expression.span)
            }
            ExprKind::FutureDelayed {
                duration,
                computation,
                value_type,
            } => self.future_delayed(
                duration,
                computation.as_deref(),
                *value_type,
                expression.span,
            ),
            ExprKind::Duration { parts } => {
                self.check_type_name(Type::Duration, expression.span)?;
                for (_, part) in parts {
                    self.require_type(self.value(part)?, Type::Int, part.span)?;
                }
                Ok(Type::Duration)
            }
            ExprKind::This => {
                self.reject_early_this(expression.span)?;
                if self.in_field_initializer {
                    return Err(Diagnostic::new(
                        "this is unavailable in field initializers",
                        expression.span,
                    ));
                }
                self.current_extension
                    .map(|index| self.extensions[index].on_type)
                    .or_else(|| self.current_class.map(Type::Class))
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "this is only supported inside instance or extension methods",
                            expression.span,
                        )
                    })
            }
            ExprKind::EnumValue { class_id, name } => {
                let class = self
                    .classes
                    .get(class_id)
                    .ok_or_else(|| Diagnostic::new("Unknown enum", expression.span))?;
                if self.lookup(class.name).is_some() || self.has_implicit_member(class.name) {
                    return Err(Diagnostic::new(
                        "Local declaration shadows enum",
                        expression.span,
                    ));
                }
                // `C.v` alcança um campo estático da própria classe; a busca
                // nunca sobe para a superclasse, porque Dart não herda estáticos.
                if let Some(member) = self.static_field(*class_id, name) {
                    return Ok(member.ty);
                }
                if !class.enum_values.contains(name) {
                    self.reject_inherited_static(*class_id, name, expression.span)?;
                    return Err(Diagnostic::new(
                        "Unknown enum value or static field",
                        expression.span,
                    ));
                }
                Ok(Type::Class(*class_id))
            }
            ExprKind::Construct {
                class_id,
                arguments,
            } => {
                let class = self
                    .classes
                    .get(class_id)
                    .ok_or_else(|| Diagnostic::new("Unknown class", expression.span))?;
                if class.is_abstract
                    || !class.has_generative
                    || class.kind == ClassKind::Mixin
                    || class.is_mixin_application
                    || !class.enum_values.is_empty()
                {
                    return Err(Diagnostic::new(
                        "Cannot construct an abstract class or enum",
                        expression.span,
                    ));
                }
                if self.lookup(class.name).is_some() || self.has_implicit_member(class.name) {
                    return Err(Diagnostic::new(
                        "Local declaration shadows constructor",
                        expression.span,
                    ));
                }
                self.check_call_arguments(
                    arguments,
                    &class.constructor_parameters,
                    class.constructor_required,
                    &class.constructor_named,
                    expression.span,
                    |_| Diagnostic::new("Incorrect constructor argument count", expression.span),
                )?;
                Ok(Type::Class(*class_id))
            }
            ExprKind::NamedConstruct {
                class_id,
                name,
                arguments,
            } => self.factory_call(*class_id, name, arguments, expression.span),
            ExprKind::Member { receiver, name } => {
                let receiver_type = self.upper_bound(self.value(receiver)?);
                if receiver_type == Type::Duration {
                    return match *name {
                        "inDays" | "inHours" | "inMinutes" | "inSeconds" | "inMilliseconds"
                        | "inMicroseconds" => Ok(Type::Int),
                        "isNegative" => Ok(Type::Bool),
                        _ => Err(Diagnostic::new(
                            "Unsupported Duration property",
                            expression.span,
                        )),
                    };
                }
                if receiver_type == Type::Timer {
                    return match *name {
                        "isActive" => Ok(Type::Bool),
                        "tick" => Ok(Type::Int),
                        _ => Err(Diagnostic::new(
                            "Unsupported Timer property",
                            expression.span,
                        )),
                    };
                }
                if matches!(self.shape(receiver_type), Some(TypeShape::Map { .. })) {
                    return match *name {
                        "length" => Ok(Type::Int),
                        _ => Err(Diagnostic::new("Unsupported Map property", expression.span)),
                    };
                }
                if matches!(self.shape(receiver_type), Some(TypeShape::Record { .. })) {
                    return self.record_field(receiver_type, name, expression.span);
                }
                if let Some(element) = self.element(receiver_type) {
                    return match *name {
                        "length" => Ok(Type::Int),
                        "isEmpty" | "isNotEmpty" => Ok(Type::Bool),
                        "first" | "last" => Ok(element),
                        _ => Err(Diagnostic::new(
                            "Unsupported collection property",
                            expression.span,
                        )),
                    };
                }
                let id = self.receiver_class(receiver)?;
                if let Some(method) = self.method(id, name)
                    && method.is_getter
                {
                    self.getter(expression.span);
                    return Ok(method.result);
                }
                self.field(id, name).map(|field| field.ty).ok_or_else(|| {
                    Diagnostic::new(
                        "Unknown field or unsupported method tear-off",
                        expression.span,
                    )
                })
            }
            ExprKind::MethodCall {
                receiver,
                name,
                arguments,
            } => {
                let receiver_type = self.upper_bound(self.value(receiver)?);
                if receiver_type == Type::Timer {
                    if *name == "cancel" && arguments.is_empty() {
                        return Ok(Type::Void);
                    }
                    return Err(Diagnostic::new("Unsupported Timer method", expression.span));
                }
                if matches!(self.shape(receiver_type), Some(TypeShape::Record { .. })) {
                    return self.invoke(
                        self.record_field(receiver_type, name, expression.span)?,
                        arguments,
                        expression.span,
                    );
                }
                if self.element(receiver_type).is_some() {
                    return self.collection_call(receiver_type, name, arguments, expression.span);
                }
                if is_nullable(receiver_type) || matches!(receiver_type, Type::Null | Type::Void) {
                    return Err(Diagnostic::new(
                        "Method calls require a non-null receiver",
                        receiver.span,
                    ));
                }
                let instance = if let Type::Class(id) = receiver_type {
                    if let Some(field) = self.field(id, name) {
                        return self.invoke(field.ty, arguments, expression.span);
                    }
                    self.method(id, name)
                } else {
                    None
                };
                let signature = if let Some(signature) = instance {
                    signature
                } else {
                    let candidates = self
                        .extensions
                        .iter()
                        .filter(|extension| {
                            extension.methods.contains_key(name)
                                && self
                                    .require_type(receiver_type, extension.on_type, expression.span)
                                    .is_ok()
                        })
                        .collect::<Vec<_>>();
                    if candidates.is_empty() {
                        return Err(Diagnostic::new(
                            "Unknown instance or extension method",
                            expression.span,
                        ));
                    }
                    let best = candidates
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            candidates.iter().all(|other| {
                                candidate.id == other.id
                                    || (self
                                        .require_type(
                                            candidate.on_type,
                                            other.on_type,
                                            expression.span,
                                        )
                                        .is_ok()
                                        && self
                                            .require_type(
                                                other.on_type,
                                                candidate.on_type,
                                                expression.span,
                                            )
                                            .is_err())
                            })
                        })
                        .collect::<Vec<_>>();
                    if best.len() != 1 {
                        return Err(Diagnostic::new(
                            "Ambiguous extension method",
                            expression.span,
                        ));
                    }
                    let extension = best[0];
                    let (method_index, signature) = &extension.methods[name];
                    self.resolution.borrow_mut().extension_calls.insert(
                        (expression.span.start, expression.span.end),
                        ExtensionTarget {
                            extension_id: extension.id,
                            method_index: *method_index,
                        },
                    );
                    signature
                };
                if signature.is_getter {
                    self.getter(expression.span);
                    return self.invoke(signature.result, arguments, expression.span);
                }
                self.check_call_arguments(
                    arguments,
                    &signature.parameters,
                    signature.required_positional,
                    &signature.named,
                    expression.span,
                    |_| Diagnostic::new("Incorrect method argument count", expression.span),
                )?;
                Ok(signature.result)
            }
            ExprKind::Null => Ok(Type::Null),
            ExprKind::Int(_) => Ok(Type::Int),
            ExprKind::Double(_) => Ok(Type::Double),
            ExprKind::String(_) | ExprKind::OwnedString(_) => Ok(Type::String),
            ExprKind::Interpolation(parts) => {
                // A ordem da lista é a ordem escrita: analisar aqui já fixa a
                // ordem de avaliação que o emissor precisa preservar.
                for part in parts {
                    if let dartforge_syntax::StringPart::Expression(value) = part {
                        self.interpolable(value)?;
                    }
                }
                Ok(Type::String)
            }
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Identifier(name) => {
                if self.lookup(name).is_none()
                    && let Some(id) = self.current_class
                {
                    if let Some(field) = self.field(id, name) {
                        self.reject_early_this(expression.span)?;
                        if self.in_field_initializer {
                            return Err(Diagnostic::new(
                                "Implicit this unavailable in field initializer",
                                expression.span,
                            ));
                        }
                        self.implicit(expression.span);
                        return Ok(field.ty);
                    }
                    if let Some(method) = self.method(id, name)
                        && method.is_getter
                    {
                        self.reject_early_this(expression.span)?;
                        if self.in_field_initializer {
                            return Err(Diagnostic::new(
                                "Implicit this unavailable in field initializer",
                                expression.span,
                            ));
                        }
                        self.implicit(expression.span);
                        self.getter(expression.span);
                        return Ok(method.result);
                    }
                }
                if self.lookup(name).is_none()
                    && !self.has_implicit_member(name)
                    && let Some(signature) = self.functions.get(name)
                {
                    if signature.generic_count > 0 {
                        return Err(Diagnostic::new(
                            "Generic function tear-offs are unsupported; call with arguments",
                            expression.span,
                        ));
                    }
                    if !signature.named.is_empty()
                        || signature.required_positional != signature.parameters.len()
                    {
                        // O tipo de função do subconjunto só descreve posicionais
                        // obrigatórios; um tear-off perderia a forma de passagem.
                        return Err(Diagnostic::new(
                            "Tear-offs of functions with optional or named parameters are unsupported",
                            expression.span,
                        ));
                    }
                    return Ok(self.intern(TypeShape::Function {
                        result: signature.result,
                        parameters: signature.parameters.clone(),
                    }));
                }
                // Locais e membros da instância têm precedência; só depois deles
                // uma variável de topo entra na resolução.
                if self.lookup(name).is_none()
                    && !self.has_implicit_member(name)
                    && let Some(global) = self.global(name)
                {
                    let ty = global.ty;
                    self.global_access(expression.span);
                    return Ok(ty);
                }
                let binding = self.initialized(name, expression.span)?;
                Ok(binding
                    .promoted
                    .or(binding.ty)
                    .expect("initialized binding"))
            }
            ExprKind::Call { name, arguments } => {
                if matches!(*name, "Timer" | "scheduleMicrotask")
                    && self.lookup(name).is_none()
                    && !self.has_implicit_member(name)
                    && !self.functions.contains_key(name)
                {
                    return self.async_builtin(name, arguments, expression.span);
                }
                if self.lookup(name).is_some() {
                    let ty = self
                        .initialized(name, expression.span)?
                        .ty
                        .expect("initialized binding");
                    return self.invoke(ty, arguments, expression.span);
                }
                if self.has_implicit_member(name) {
                    self.reject_early_this(expression.span)?;
                    if self.in_field_initializer {
                        return Err(Diagnostic::new(
                            "Implicit this unavailable in field initializer",
                            expression.span,
                        ));
                    }
                    if let Some(id) = self.current_class {
                        self.implicit(expression.span);
                        if let Some(field) = self.field(id, name) {
                            return self.invoke(field.ty, arguments, expression.span);
                        }
                        if let Some(signature) = self.method(id, name) {
                            if signature.is_getter {
                                self.getter(expression.span);
                                return self.invoke(signature.result, arguments, expression.span);
                            }
                            self.check_call_arguments(
                                arguments,
                                &signature.parameters,
                                signature.required_positional,
                                &signature.named,
                                expression.span,
                                |_| {
                                    Diagnostic::new(
                                        "Incorrect implicit method argument count",
                                        expression.span,
                                    )
                                },
                            )?;
                            return Ok(signature.result);
                        }
                    }
                    return Err(Diagnostic::new(
                        "Implicit extension methods remain unsupported",
                        expression.span,
                    ));
                }
                if *name == "print" {
                    if arguments.len() != 1 {
                        return Err(Diagnostic::new(
                            "print expects one argument",
                            expression.span,
                        ));
                    }
                    self.printable(&arguments[0])?;
                    return Ok(Type::Void);
                }
                let signature = self.functions.get(name).ok_or_else(|| {
                    Diagnostic::new(format!("Unknown function '{name}'"), expression.span)
                })?;
                self.check_call_arguments(
                    arguments,
                    &signature.parameters,
                    signature.required_positional,
                    &signature.named,
                    expression.span,
                    |written| {
                        // Aridades exata e mínima coincidem quando não há opcionais.
                        let expected = if written < signature.required_positional {
                            signature.required_positional
                        } else {
                            signature.parameters.len()
                        };
                        Diagnostic::new(
                            format!(
                                "Function '{name}' expects {expected} arguments, received {written}"
                            ),
                            expression.span,
                        )
                    },
                )?;
                Ok(signature.result)
            }
            ExprKind::Unary { op, operand } => {
                if *op == UnaryOp::NullAssert {
                    let ty = self.value(operand)?;
                    if matches!(ty, Type::Parameter(_)) {
                        return Err(Diagnostic::new(
                            "Null assertion of an unconstrained type parameter is unsupported",
                            expression.span,
                        ));
                    }
                    return if ty == Type::Null {
                        Err(Diagnostic::new(
                            "Null assertion on a null-only value is unsupported",
                            expression.span,
                        ))
                    } else {
                        Ok(self.without_null(ty))
                    };
                }
                let expected = match op {
                    UnaryOp::Negate => None,
                    UnaryOp::Not => Some(Type::Bool),
                    UnaryOp::NullAssert => unreachable!(),
                };
                // Negação preserva o tipo numérico (oráculo Dart 3.6.2):
                // `-1` é int, `-1.5` é double e `-n` (num) é num.
                if let Some(expected) = expected {
                    self.require_type(self.value(operand)?, expected, operand.span)?;
                    return Ok(expected);
                }
                let actual = self.value(operand)?;
                match self.upper_bound(actual) {
                    ty @ (Type::Int | Type::Double | Type::Num) => Ok(ty),
                    _ => Err(Diagnostic::new(
                        format!("Type mismatch: expected Int, found {actual:?}"),
                        operand.span,
                    )),
                }
            }
            ExprKind::Binary { op, left, right } => {
                let lhs = self.value(left)?;
                let rhs = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let mut rhs_state = self.clone();
                    rhs_state.promote(left, *op == BinaryOp::And);
                    rhs_state.value(right)?
                } else {
                    self.value(right)?
                };
                match op {
                    // `a ?? (throw e)` nunca produz o valor do lado direito:
                    // o tipo do resultado é o do lado esquerdo sem null.
                    BinaryOp::IfNull if fluxo::is_throw(right) => Ok(self.without_null(lhs)),
                    BinaryOp::IfNull => {
                        if matches!(lhs, Type::Parameter(_)) {
                            return Err(Diagnostic::new(
                                "Null coalescing of an unconstrained type parameter is unsupported",
                                expression.span,
                            ));
                        }
                        if lhs == Type::Null {
                            return Ok(rhs);
                        }
                        let base = self.without_null(lhs);
                        if lhs == base {
                            return Ok(lhs);
                        }
                        if rhs == base {
                            Ok(base)
                        } else if rhs == lhs || rhs == Type::Null {
                            Ok(lhs)
                        } else {
                            Err(Diagnostic::new(
                                "The common type of ?? operands is unsupported",
                                expression.span,
                            ))
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => Ok(Type::Bool),
                    BinaryOp::Add => {
                        let lhs = self.upper_bound(lhs);
                        let rhs = self.upper_bound(rhs);
                        if matches!(lhs, Type::String) && lhs == rhs {
                            Ok(lhs)
                        } else if is_number(lhs)
                            && is_number(rhs)
                            && !self.may_be_null(lhs)
                            && !self.may_be_null(rhs)
                        {
                            // int+int→int; com double→double; com num→num.
                            Ok(numeric_result(lhs, rhs))
                        } else {
                            Err(Diagnostic::new(
                                "Operator '+' requires numeric operands of compatible type or two String operands",
                                expression.span,
                            ))
                        }
                    }
                    BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Remainder => {
                        let lhs = self.upper_bound(lhs);
                        let rhs = self.upper_bound(rhs);
                        if !is_number(lhs) || self.may_be_null(lhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {lhs:?}"),
                                left.span,
                            ));
                        }
                        if !is_number(rhs) || self.may_be_null(rhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {rhs:?}"),
                                right.span,
                            ));
                        }
                        Ok(numeric_result(lhs, rhs))
                    }
                    BinaryOp::Divide => {
                        // `/` sempre produz double, mesmo entre inteiros (oráculo Dart 3.6.2).
                        let lhs = self.upper_bound(lhs);
                        let rhs = self.upper_bound(rhs);
                        if !is_number(lhs) || self.may_be_null(lhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Double, found {lhs:?}"),
                                left.span,
                            ));
                        }
                        if !is_number(rhs) || self.may_be_null(rhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Double, found {rhs:?}"),
                                right.span,
                            ));
                        }
                        Ok(Type::Double)
                    }
                    BinaryOp::TruncDivide => {
                        // `~/` sempre produz int, mesmo entre doubles.
                        let lhs = self.upper_bound(lhs);
                        let rhs = self.upper_bound(rhs);
                        if !is_number(lhs) || self.may_be_null(lhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {lhs:?}"),
                                left.span,
                            ));
                        }
                        if !is_number(rhs) || self.may_be_null(rhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {rhs:?}"),
                                right.span,
                            ));
                        }
                        Ok(Type::Int)
                    }
                    BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        // Comparações aceitam int, double e num misturados.
                        let lhs = self.upper_bound(lhs);
                        let rhs = self.upper_bound(rhs);
                        if !is_number(lhs) || self.may_be_null(lhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {lhs:?}"),
                                left.span,
                            ));
                        }
                        if !is_number(rhs) || self.may_be_null(rhs) {
                            return Err(Diagnostic::new(
                                format!("Type mismatch: expected Int, found {rhs:?}"),
                                right.span,
                            ));
                        }
                        Ok(Type::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        self.require_type(lhs, Type::Bool, left.span)?;
                        self.require_type(rhs, Type::Bool, right.span)?;
                        Ok(Type::Bool)
                    }
                }
            }
        }
    }
}
/// Identifica os tipos primitivos que admitem null.
/// Identifica o curinga `_`, que a partir do Dart 3.7 não declara ligação alguma.
///
/// Campos, membros e declarações de topo chamados `_` continuam sendo nomes
/// comuns; esta função só é consultada em locais, parâmetros e padrões.
pub(crate) fn is_wildcard(name: &str) -> bool {
    name == "_"
}
fn is_nullable(ty: Type) -> bool {
    matches!(
        ty,
        Type::NullableInt
            | Type::NullableDouble
            | Type::NullableNum
            | Type::NullableString
            | Type::NullableBool
            | Type::NullableClass(_)
            | Type::NullableObject
            | Type::NullableParameter(_)
    )
}
/// Remove a possibilidade de null de um tipo primitivo.
fn non_null(ty: Type) -> Type {
    match ty {
        Type::NullableInt => Type::Int,
        Type::NullableDouble => Type::Double,
        Type::NullableNum => Type::Num,
        Type::NullableObject => Type::Object,
        Type::NullableParameter(id) => Type::Parameter(id),
        Type::NullableString => Type::String,
        Type::NullableBool => Type::Bool,
        Type::NullableClass(id) => Type::Class(id),
        other => other,
    }
}
/// Indica se o tipo é numérico não estrutural (int, double ou num, anuláveis ou não).
fn is_number(ty: Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::Double
            | Type::Num
            | Type::NullableInt
            | Type::NullableDouble
            | Type::NullableNum
    )
}
/// Posto numérico para promoção: int (0) → double (1) → num (2).
fn numeric_rank(ty: Type) -> Option<(u8, bool)> {
    match ty {
        Type::Int => Some((0, false)),
        Type::Double => Some((1, false)),
        Type::Num => Some((2, false)),
        Type::NullableInt => Some((0, true)),
        Type::NullableDouble => Some((1, true)),
        Type::NullableNum => Some((2, true)),
        _ => None,
    }
}
/// Promoção de atribuição do oráculo Dart 3.6.2: int vale onde double ou num
/// é esperado, e double vale onde num é esperado, sem perder null.
fn numeric_promotion(actual: Type, expected: Type) -> bool {
    match (numeric_rank(actual), numeric_rank(expected)) {
        (Some((a, a_null)), Some((e, e_null))) if a < e => !a_null || e_null,
        _ => false,
    }
}
/// Subtipagem numérica real do Dart: int e double são subtipos de num
/// (vale também na variante estrita usada por covariância e casts).
fn numeric_subtype(actual: Type, expected: Type) -> bool {
    match (numeric_rank(actual), numeric_rank(expected)) {
        (Some((_, a_null)), Some((2, e_null))) => !a_null || e_null,
        _ => false,
    }
}
/// Resultado de `+ - * %` entre numéricos: int+int→int, com double→double,
/// com num→num; anulabilidade contamina o resultado como no restante da análise.
fn numeric_result(left: Type, right: Type) -> Type {
    let ((a, a_null), (e, e_null)) = (
        numeric_rank(left).expect("operando numérico"),
        numeric_rank(right).expect("operando numérico"),
    );
    let rank = a.max(e);
    let nullable = a_null || e_null;
    match (rank, nullable) {
        (0, false) => Type::Int,
        (0, true) => Type::NullableInt,
        (1, false) => Type::Double,
        (1, true) => Type::NullableDouble,
        (_, false) => Type::Num,
        (_, true) => Type::NullableNum,
    }
}
/// Compara tipos e associa a incompatibilidade ao trecho indicado.
fn require_type(actual: Type, expected: Type, span: Span) -> Result<(), Diagnostic> {
    if actual == expected
        || (is_nullable(expected) && (actual == Type::Null || actual == non_null(expected)))
    {
        Ok(())
    } else {
        Err(Diagnostic::new(
            format!("Type mismatch: expected {expected:?}, found {actual:?}"),
            span,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SPAN: Span = Span { start: 4, end: 9 };
    /// Constrói uma expressão para os testes semânticos.
    fn expr(kind: ExprKind<'static>) -> Expr<'static> {
        Expr { kind, span: SPAN }
    }
    /// Constrói um literal inteiro para os testes.
    fn int() -> Expr<'static> {
        expr(ExprKind::Int(1))
    }
    /// Constrói uma referência a nome para os testes.
    fn id(name: &'static str) -> Expr<'static> {
        expr(ExprKind::Identifier(name))
    }
    /// Constrói uma instrução com posição conhecida.
    fn stmt(kind: StatementKind<'static>) -> Statement<'static> {
        Statement { kind, span: SPAN }
    }
    /// Constrói uma declaração local mutável sem anotação.
    fn local(name: &'static str, initializer: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Variable {
            is_const: false,
            name,
            annotation: None,
            is_final: false,
            initializer,
        })
    }
    /// Constrói uma chamada de impressão para os testes.
    fn print(value: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Print(value))
    }
    /// Valida um corpo de main sem funções auxiliares.
    fn check(statements: Vec<Statement<'static>>) -> Result<(), Diagnostic> {
        validate(&Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![],
            statements,
        })
    }
    /// Constrói uma operação binária para os testes.
    fn binary(op: BinaryOp, left: Expr<'static>, right: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    /// Constrói uma chamada com argumentos posicionais.
    fn call(name: &'static str, arguments: Vec<Expr<'static>>) -> Expr<'static> {
        expr(ExprKind::Call { name, arguments })
    }
    /// Constrói uma função tipada para os testes.
    fn function(
        name: &'static str,
        result: Type,
        parameters: Vec<(&'static str, Type)>,
        body: Vec<Statement<'static>>,
    ) -> dartforge_syntax::Function<'static> {
        dartforge_syntax::Function {
            is_arrow: false,
            is_async: false,
            annotations: vec![],
            native_binding: None,
            type_parameters: vec![],
            is_getter: false,
            name,
            return_type: result,
            parameters: parameters
                .into_iter()
                .map(|(name, ty)| dartforge_syntax::Parameter::required(name, ty, SPAN))
                .collect(),
            body,
            span: SPAN,
        }
    }
    /// Constrói uma instrução de retorno com valor.
    fn ret(value: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Return(Some(value)))
    }
    /// Constrói uma extension de teste com um método inteiro constante.
    fn extension(
        id: u32,
        name: &'static str,
        on_type: Type,
    ) -> dartforge_syntax::Extension<'static> {
        dartforge_syntax::Extension {
            id,
            name,
            on_type,
            methods: vec![function("value", Type::Int, vec![], vec![ret(int())])],
            span: SPAN,
        }
    }
    /// Constrói uma chamada de extension com posição conhecida para a tabela lateral.
    fn extension_call(receiver: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: "value",
            arguments: vec![],
        })
    }
    /// Verifica this primitivo, tipos de argumentos e gravação da resolução estática.
    #[test]
    fn primitive_extensions_record_resolved_calls() {
        let mut ext = extension(7, "Numbers", Type::Int);
        ext.methods[0].body = vec![ret(binary(BinaryOp::Add, expr(ExprKind::This), int()))];
        let program = Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![ext],
            classes: vec![],
            functions: vec![],
            statements: vec![print(extension_call(int()))],
        };
        let resolution = analyze(&program).unwrap();
        assert_eq!(
            resolution.extension_calls[&(SPAN.start, SPAN.end)].extension_id,
            7
        );
        let program = Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![extension(7, "Numbers", Type::Int)],
            classes: vec![],
            functions: vec![],
            statements: vec![print(extension_call(expr(ExprKind::Bool(true))))],
        };
        assert!(analyze(&program).is_err());
    }
    /// Rejeita ambiguidade e receptor anulável sem promoção comprovada.
    #[test]
    fn extensions_require_unique_applicable_nonnullable_receiver() {
        assert!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![extension(0, "A", Type::Int), extension(1, "B", Type::Int)],
                classes: vec![],
                functions: vec![],
                statements: vec![print(extension_call(int()))]
            })
            .is_err()
        );
        let nullable = function(
            "f",
            Type::Void,
            vec![("x", Type::NullableInt)],
            vec![print(extension_call(id("x")))],
        );
        assert!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![extension(0, "A", Type::Int)],
                classes: vec![],
                functions: vec![nullable],
                statements: vec![]
            })
            .is_err()
        );
        let promoted = function(
            "f",
            Type::Void,
            vec![("x", Type::NullableInt)],
            vec![stmt(StatementKind::If {
                condition: present("x"),
                then_body: vec![print(extension_call(id("x")))],
                else_body: None,
            })],
        );
        assert_eq!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![extension(0, "A", Type::Int)],
                classes: vec![],
                functions: vec![promoted],
                statements: vec![]
            })
            .unwrap()
            .extension_calls
            .len(),
            1
        );
    }
    /// Membros reais prevalecem sobre extensions, inclusive campos não invocáveis.
    #[test]
    fn instance_members_take_precedence_over_extensions() {
        let mut c = class(0, "C", None);
        c.methods
            .push(function("value", Type::Int, vec![], vec![ret(int())]));
        assert!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![extension(0, "E", Type::Class(0))],
                classes: vec![c],
                functions: vec![],
                statements: vec![print(extension_call(expr(ExprKind::Construct {
                    class_id: 0,
                    arguments: vec![]
                })))]
            })
            .unwrap()
            .extension_calls
            .is_empty()
        );
        let mut c = class(0, "C", None);
        c.fields.push(field("value", false));
        assert!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![extension(0, "E", Type::Class(0))],
                classes: vec![c],
                functions: vec![],
                statements: vec![print(extension_call(expr(ExprKind::Construct {
                    class_id: 0,
                    arguments: vec![]
                })))]
            })
            .is_err()
        );
    }
    /// Seleciona o on mais específico sem trocar o tipo declarado pelo tipo construído.
    #[test]
    fn extension_specificity_uses_static_declared_type() {
        for (annotation, expected) in [
            (None, 1),
            (Some(Type::Class(0)), 0),
            (Some(Type::NullableClass(0)), 0),
        ] {
            let program = Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![
                    extension(0, "EA", Type::Class(0)),
                    extension(1, "EB", Type::Class(1)),
                ],
                classes: vec![class(0, "A", None), class(1, "B", Some(0))],
                functions: vec![],
                statements: vec![
                    stmt(StatementKind::Variable {
                        is_const: false,
                        name: "x",
                        annotation,
                        is_final: false,
                        initializer: expr(ExprKind::Construct {
                            class_id: 1,
                            arguments: vec![],
                        }),
                    }),
                    print(extension_call(id("x"))),
                ],
            };
            assert_eq!(
                analyze(&program).unwrap().extension_calls[&(SPAN.start, SPAN.end)].extension_id,
                expected
            );
        }
        let mut b = class(1, "B", Some(0));
        b.methods
            .push(function("onlyChild", Type::Int, vec![], vec![ret(int())]));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "A", None), b],
                functions: vec![],
                statements: vec![
                    stmt(StatementKind::Variable {
                        is_const: false,
                        name: "x",
                        annotation: Some(Type::Class(0)),
                        is_final: false,
                        initializer: expr(ExprKind::Construct {
                            class_id: 1,
                            arguments: vec![]
                        })
                    }),
                    print(expr(ExprKind::MethodCall {
                        receiver: Box::new(id("x")),
                        name: "onlyChild",
                        arguments: vec![]
                    }))
                ]
            })
            .is_err()
        );
    }
    /// Constrói uma classe de teste sem membros e com base opcional.
    fn class(
        id: u32,
        name: &'static str,
        superclass: Option<u32>,
    ) -> dartforge_syntax::Class<'static> {
        dartforge_syntax::Class {
            constructor: None,
            factories: vec![],
            annotations: vec![],
            modifier: ClassModifier::None,
            kind: ClassKind::Class,
            mixins: vec![],
            is_mixin_application: false,
            mixin_origin: None,
            enum_arguments: vec![],
            enum_constructor_fields: vec![],
            id,
            name,
            superclass,
            is_abstract: false,
            is_interface: false,
            library_id: 0,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            fields: vec![],
            methods: vec![],
            span: SPAN,
        }
    }
    /// Constrói um campo inteiro para testar inicialização e mutabilidade.
    fn field(name: &'static str, is_final: bool) -> dartforge_syntax::Field<'static> {
        dartforge_syntax::Field {
            name,
            ty: Type::Int,
            is_final,
            initializer: Some(int()),
            span: SPAN,
        }
    }
    /// Constrói a leitura de um campo de instância.
    fn member(receiver: Expr<'static>, name: &'static str) -> Expr<'static> {
        expr(ExprKind::Member {
            receiver: Box::new(receiver),
            name,
        })
    }
    /// Verifica herança, chamada dinâmica tipada e atribuição nominal de subtipo.
    #[test]
    fn classes_inherit_fields_methods_and_nominal_types() {
        let mut base = class(0, "Base", None);
        base.fields.push(field("value", false));
        base.methods.push(function(
            "get",
            Type::Int,
            vec![],
            vec![ret(member(expr(ExprKind::This), "value"))],
        ));
        let mut child = class(1, "Child", Some(0));
        child
            .methods
            .push(function("get", Type::Int, vec![], vec![ret(int())]));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![base, child],
                functions: vec![],
                statements: vec![
                    stmt(StatementKind::Variable {
                        is_const: false,
                        name: "b",
                        annotation: Some(Type::Class(0)),
                        is_final: false,
                        initializer: expr(ExprKind::Construct {
                            class_id: 1,
                            arguments: vec![]
                        })
                    }),
                    stmt(StatementKind::FieldAssign {
                        receiver: id("b"),
                        name: "value",
                        value: int()
                    }),
                    print(expr(ExprKind::MethodCall {
                        receiver: Box::new(id("b")),
                        name: "get",
                        arguments: vec![]
                    })),
                ]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "Base", None), class(1, "Child", Some(0))],
                functions: vec![],
                statements: vec![stmt(StatementKind::Variable {
                    is_const: false,
                    name: "c",
                    annotation: Some(Type::Class(1)),
                    is_final: false,
                    initializer: expr(ExprKind::Construct {
                        class_id: 0,
                        arguments: vec![]
                    })
                })]
            })
            .is_err()
        );
    }
    /// Rejeita ciclos, bases ausentes, colisões de membros e sobrescritas incompatíveis.
    #[test]
    fn invalid_class_hierarchies_and_overrides_are_rejected() {
        for classes in [
            vec![class(0, "A", Some(1)), class(1, "B", Some(0))],
            vec![class(0, "A", Some(9))],
            vec![class(0, "A", None), class(0, "B", None)],
        ] {
            assert!(
                validate(&Program {
                    main_is_arrow: false,
                    main_is_async: false,
                    types: vec![],
                    extensions: vec![],
                    classes,
                    functions: vec![],
                    statements: vec![]
                })
                .is_err()
            );
        }
        let mut base = class(0, "A", None);
        base.methods
            .push(function("f", Type::Int, vec![], vec![ret(int())]));
        let mut child = class(1, "B", Some(0));
        child.methods.push(function(
            "f",
            Type::Bool,
            vec![],
            vec![ret(expr(ExprKind::Bool(true)))],
        ));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![base, child],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
        let mut base = class(0, "A", None);
        base.fields.push(field("x", false));
        let mut child = class(1, "B", Some(0));
        child.fields.push(field("x", false));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![base, child],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
        let mut base = class(0, "A", None);
        base.methods.push(function(
            "f",
            Type::Void,
            vec![("x", Type::NullableInt)],
            vec![],
        ));
        let mut child = class(1, "B", Some(0));
        child
            .methods
            .push(function("f", Type::Void, vec![("x", Type::Int)], vec![]));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![base, child],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Verifica this, campos final, impressão de objetos e construtores ocultados.
    #[test]
    fn class_access_respects_context_and_final_fields() {
        assert!(check(vec![print(expr(ExprKind::This))]).is_err());
        let mut c = class(0, "A", None);
        c.fields.push(field("x", true));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![],
                statements: vec![
                    local(
                        "a",
                        expr(ExprKind::Construct {
                            class_id: 0,
                            arguments: vec![]
                        })
                    ),
                    stmt(StatementKind::FieldAssign {
                        receiver: id("a"),
                        name: "x",
                        value: int()
                    })
                ]
            })
            .is_err()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "A", None)],
                functions: vec![],
                statements: vec![print(expr(ExprKind::Construct {
                    class_id: 0,
                    arguments: vec![]
                }))]
            })
            .is_err()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "A", None)],
                functions: vec![],
                statements: vec![
                    local("A", int()),
                    local(
                        "a",
                        expr(ExprKind::Construct {
                            class_id: 0,
                            arguments: vec![]
                        })
                    )
                ]
            })
            .is_err()
        );
        let mut c = class(0, "A", None);
        c.fields.push(dartforge_syntax::Field {
            name: "self",
            ty: Type::Class(0),
            is_final: true,
            initializer: Some(expr(ExprKind::This)),
            span: SPAN,
        });
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Rejeita resolução global quando membros ocultam funções, tipos ou construtores.
    #[test]
    fn implicit_member_shadowing_never_resolves_to_globals() {
        let mut c = class(0, "C", None);
        c.fields.push(field("f", false));
        c.fields.push(dartforge_syntax::Field {
            name: "x",
            ty: Type::Int,
            is_final: false,
            initializer: Some(call("f", vec![])),
            span: SPAN,
        });
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![function("f", Type::Int, vec![], vec![ret(int())])],
                statements: vec![]
            })
            .is_err()
        );
        let mut a = class(1, "A", None);
        a.fields.push(field("C", false));
        a.methods.push(function(
            "make",
            Type::Class(0),
            vec![],
            vec![ret(expr(ExprKind::Construct {
                class_id: 0,
                arguments: vec![],
            }))],
        ));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "C", None), a],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
        let mut c = class(0, "C", None);
        c.methods
            .push(function("f", Type::Int, vec![], vec![ret(int())]));
        c.methods.push(function(
            "g",
            Type::Int,
            vec![],
            vec![ret(call("f", vec![]))],
        ));
        assert!(
            analyze(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![function("f", Type::Int, vec![], vec![ret(int())])],
                statements: vec![]
            })
            .unwrap()
            .implicit_members
            .contains(&(SPAN.start, SPAN.end))
        );
    }
    /// Aceita parâmetros contravariantes e retorno covariante em sobrescritas nominais.
    #[test]
    fn class_override_variance_is_checked_nominally() {
        let base = class(0, "Base", None);
        let child = class(1, "Child", Some(0));
        let mut a = class(2, "A", None);
        a.methods.push(function(
            "f",
            Type::Class(0),
            vec![("x", Type::Class(1))],
            vec![ret(id("x"))],
        ));
        let mut b = class(3, "B", Some(2));
        b.methods.push(function(
            "f",
            Type::Class(1),
            vec![("x", Type::Class(0))],
            vec![ret(expr(ExprKind::Construct {
                class_id: 1,
                arguments: vec![],
            }))],
        ));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![base, child, a, b],
                functions: vec![],
                statements: vec![]
            })
            .is_ok()
        );
    }
    /// Promove variáveis de classe anuláveis, mas nunca a leitura de campos.
    #[test]
    fn nullable_class_promotion_does_not_promote_fields() {
        let mut c = class(0, "A", None);
        c.fields.push(field("x", false));
        let f = function(
            "f",
            Type::Void,
            vec![("a", Type::NullableClass(0))],
            vec![stmt(StatementKind::If {
                condition: present("a"),
                then_body: vec![print(member(id("a"), "x"))],
                else_body: None,
            })],
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![f],
                statements: vec![]
            })
            .is_ok()
        );
        let mut c = class(0, "A", None);
        c.fields.push(dartforge_syntax::Field {
            name: "x",
            ty: Type::NullableInt,
            is_final: false,
            initializer: Some(expr(ExprKind::Null)),
            span: SPAN,
        });
        c.methods.push(function(
            "f",
            Type::Void,
            vec![],
            vec![stmt(StatementKind::If {
                condition: binary(
                    BinaryOp::NotEqual,
                    member(expr(ExprKind::This), "x"),
                    expr(ExprKind::Null),
                ),
                then_body: vec![print(binary(
                    BinaryOp::Add,
                    member(expr(ExprKind::This), "x"),
                    int(),
                ))],
                else_body: None,
            })],
        ));
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Constrói uma comparação de presença de valor para testes de fluxo.
    fn present(name: &'static str) -> Expr<'static> {
        binary(BinaryOp::NotEqual, id(name), expr(ExprKind::Null))
    }
    /// Valida um corpo com um parâmetro inteiro anulável.
    fn nullable_body(body: Vec<Statement<'static>>) -> Result<(), Diagnostic> {
        validate(&Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "f",
                Type::Void,
                vec![("x", Type::NullableInt)],
                body,
            )],
            statements: vec![],
        })
    }
    /// Verifica atribuição anulável, inferência segura e incompatibilidades primitivas.
    #[test]
    fn nullable_assignments_preserve_declared_type() {
        assert!(
            check(vec![
                stmt(StatementKind::Variable {
                    is_const: false,
                    name: "x",
                    annotation: Some(Type::NullableInt),
                    is_final: false,
                    initializer: int()
                }),
                print(binary(BinaryOp::Add, id("x"), int())),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Null)
                }),
                print(binary(BinaryOp::IfNull, id("x"), int()))
            ])
            .is_ok()
        );
        assert!(
            nullable_body(vec![stmt(StatementKind::Assign {
                name: "x",
                value: expr(ExprKind::String("bad"))
            })])
            .is_err()
        );
        assert!(check(vec![local("x", expr(ExprKind::Null))]).is_err());
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Null)
                })
            ])
            .is_err()
        );
        assert!(nullable_body(vec![print(binary(BinaryOp::Add, id("x"), int()))]).is_err());
    }
    /// Verifica promoção em if, retorno antecipado e curto circuito booleano.
    #[test]
    fn null_checks_promote_only_reachable_branches() {
        assert!(
            nullable_body(vec![stmt(StatementKind::If {
                condition: present("x"),
                then_body: vec![print(binary(BinaryOp::Add, id("x"), int()))],
                else_body: None
            })])
            .is_ok()
        );
        assert!(
            nullable_body(vec![
                stmt(StatementKind::If {
                    condition: binary(BinaryOp::Equal, id("x"), expr(ExprKind::Null)),
                    then_body: vec![stmt(StatementKind::Return(None))],
                    else_body: None
                }),
                print(binary(BinaryOp::Add, id("x"), int()))
            ])
            .is_ok()
        );
        assert!(
            nullable_body(vec![
                print(binary(
                    BinaryOp::And,
                    present("x"),
                    binary(BinaryOp::Greater, id("x"), int())
                )),
                print(binary(
                    BinaryOp::Or,
                    binary(BinaryOp::Equal, id("x"), expr(ExprKind::Null)),
                    binary(BinaryOp::Greater, id("x"), int())
                ))
            ])
            .is_ok()
        );
        assert!(
            nullable_body(vec![print(binary(
                BinaryOp::Or,
                present("x"),
                binary(BinaryOp::Greater, id("x"), int())
            ))])
            .is_err()
        );
        assert!(
            nullable_body(vec![
                stmt(StatementKind::If {
                    condition: present("x"),
                    then_body: vec![],
                    else_body: None
                }),
                print(binary(BinaryOp::Add, id("x"), int()))
            ])
            .is_err()
        );
    }
    /// Rejeita uso promovido após escrita null, mescla de ramo e repetição com escrita.
    #[test]
    fn null_writes_invalidate_branch_and_loop_promotions() {
        let assign_null = || {
            stmt(StatementKind::Assign {
                name: "x",
                value: expr(ExprKind::Null),
            })
        };
        assert!(
            nullable_body(vec![stmt(StatementKind::If {
                condition: present("x"),
                then_body: vec![assign_null(), print(binary(BinaryOp::Add, id("x"), int()))],
                else_body: None
            })])
            .is_err()
        );
        assert!(
            nullable_body(vec![stmt(StatementKind::If {
                condition: present("x"),
                then_body: vec![stmt(StatementKind::While {
                    condition: expr(ExprKind::Bool(true)),
                    body: vec![print(binary(BinaryOp::Add, id("x"), int())), assign_null()]
                })],
                else_body: None
            })])
            .is_err()
        );
        assert!(
            nullable_body(vec![stmt(StatementKind::While {
                condition: present("x"),
                body: vec![
                    print(binary(BinaryOp::Add, id("x"), int())),
                    assign_null(),
                    stmt(StatementKind::Continue)
                ]
            })])
            .is_ok()
        );
        assert!(
            nullable_body(vec![stmt(StatementKind::For {
                initializer: None,
                condition: Some(present("x")),
                update: Some(Box::new(stmt(StatementKind::Assign {
                    name: "x",
                    value: binary(BinaryOp::Add, id("x"), int())
                }))),
                body: vec![assign_null(), stmt(StatementKind::Continue)]
            })])
            .is_err()
        );
    }
    /// Verifica remoção explícita de null, coalescência e retornos anuláveis.
    #[test]
    fn null_assert_and_coalesce_have_nonnullable_results() {
        assert!(
            nullable_body(vec![
                print(binary(
                    BinaryOp::Add,
                    expr(ExprKind::Unary {
                        op: UnaryOp::NullAssert,
                        operand: Box::new(id("x"))
                    }),
                    int()
                )),
                print(binary(
                    BinaryOp::Add,
                    binary(BinaryOp::IfNull, id("x"), int()),
                    int()
                ))
            ])
            .is_ok()
        );
        assert!(
            nullable_body(vec![print(binary(
                BinaryOp::IfNull,
                id("x"),
                expr(ExprKind::String("bad"))
            ))])
            .is_err()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![
                    function(
                        "f",
                        Type::NullableInt,
                        vec![],
                        vec![ret(expr(ExprKind::Null))]
                    ),
                    function("g", Type::NullableString, vec![], vec![])
                ],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![],
                    vec![ret(expr(ExprKind::Null))]
                )],
                statements: vec![]
            })
            .is_err()
        );
        assert!(
            nullable_body(vec![stmt(StatementKind::If {
                condition: present("x"),
                then_body: vec![stmt(StatementKind::Block(vec![
                    stmt(StatementKind::Variable {
                        is_const: false,
                        name: "x",
                        annotation: Some(Type::NullableInt),
                        is_final: false,
                        initializer: expr(ExprKind::Null)
                    }),
                    print(binary(BinaryOp::Add, id("x"), int()))
                ]))],
                else_body: None
            })])
            .is_err()
        );
    }
    /// Constrói um laço clássico com inicializador local e condição booleana.
    fn for_statement(
        initializer: Statement<'static>,
        update: Option<Statement<'static>>,
        body: Vec<Statement<'static>>,
    ) -> Statement<'static> {
        stmt(StatementKind::For {
            initializer: Some(Box::new(initializer)),
            condition: Some(expr(ExprKind::Bool(true))),
            update: update.map(Box::new),
            body,
        })
    }
    /// Verifica controle de laços aninhados e restauração da profundidade fora deles.
    #[test]
    fn loop_control_requires_enclosing_loop() {
        for kind in [StatementKind::Break, StatementKind::Continue] {
            assert!(check(vec![stmt(kind)]).is_err());
        }
        assert!(
            check(vec![stmt(StatementKind::While {
                condition: expr(ExprKind::Bool(true)),
                body: vec![
                    stmt(StatementKind::DoWhile {
                        body: vec![stmt(StatementKind::Continue)],
                        condition: expr(ExprKind::Bool(false))
                    }),
                    stmt(StatementKind::Break),
                ]
            })])
            .is_ok()
        );
        assert!(
            check(vec![
                stmt(StatementKind::While {
                    condition: expr(ExprKind::Bool(true)),
                    body: vec![]
                }),
                stmt(StatementKind::Break)
            ])
            .is_err()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![
                    function(
                        "f",
                        Type::Void,
                        vec![],
                        vec![stmt(StatementKind::While {
                            condition: expr(ExprKind::Bool(true)),
                            body: vec![stmt(StatementKind::Break)]
                        })]
                    ),
                    function("g", Type::Void, vec![], vec![stmt(StatementKind::Continue)])
                ],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Verifica condições booleanas em todas as formas de laço.
    #[test]
    fn loop_conditions_require_bool() {
        for kind in [
            StatementKind::While {
                condition: int(),
                body: vec![],
            },
            StatementKind::DoWhile {
                condition: int(),
                body: vec![],
            },
            StatementKind::For {
                initializer: None,
                condition: Some(int()),
                update: None,
                body: vec![],
            },
        ] {
            assert!(check(vec![stmt(kind)]).is_err());
        }
        assert!(
            check(vec![stmt(StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                body: vec![stmt(StatementKind::Break)]
            })])
            .is_ok()
        );
    }
    /// Verifica o alcance do inicializador e separação entre o corpo e a atualização.
    #[test]
    fn for_scope_keeps_body_shadow_out_of_update() {
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })),
                vec![local("x", expr(ExprKind::String("body"))), print(id("x"))]
            )])
            .is_ok()
        );
        assert!(
            check(vec![
                for_statement(local("x", int()), None, vec![]),
                print(id("x"))
            ])
            .is_err()
        );
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "body",
                    value: int()
                })),
                vec![local("body", int())]
            )])
            .is_err()
        );
        assert!(
            check(vec![
                local("x", int()),
                for_statement(local("x", id("x")), None, vec![])
            ])
            .is_err()
        );
        assert!(
            check(vec![stmt(StatementKind::DoWhile {
                body: vec![local("flag", expr(ExprKind::Bool(true)))],
                condition: id("flag")
            })])
            .is_err()
        );
    }
    /// Verifica tipos e imutabilidade das atribuições na atualização.
    #[test]
    fn for_update_obeys_final_and_type_rules() {
        assert!(
            check(vec![for_statement(
                stmt(StatementKind::Variable {
                    is_const: false,
                    name: "x",
                    annotation: None,
                    is_final: true,
                    initializer: int()
                }),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })),
                vec![]
            )])
            .is_err()
        );
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Bool(false))
                })),
                vec![]
            )])
            .is_err()
        );
    }
    /// Impede que ASTs construídas diretamente insiram controle de fluxo nos cabeçalhos.
    #[test]
    fn for_headers_reject_non_expression_statements() {
        for initializer in [
            stmt(StatementKind::Break),
            ret(int()),
            stmt(StatementKind::Block(vec![])),
            print(int()),
        ] {
            assert!(check(vec![for_statement(initializer, None, vec![])]).is_err());
        }
        for update in [
            local("y", int()),
            stmt(StatementKind::Continue),
            stmt(StatementKind::Return(None)),
            print(int()),
        ] {
            assert!(check(vec![for_statement(local("x", int()), Some(update), vec![])]).is_err());
        }
        // O parser representa print nos cabeçalhos como expressão de chamada.
        assert!(
            check(vec![for_statement(
                stmt(StatementKind::Expression(call("print", vec![int()]))),
                Some(stmt(StatementKind::Expression(call("print", vec![int()])))),
                vec![stmt(StatementKind::Break)],
            )])
            .is_ok()
        );
    }
    /// Não infere retorno obrigatório a partir de laços nem de retornos após break.
    #[test]
    fn loop_returns_remain_conservative() {
        for body in [
            vec![stmt(StatementKind::While {
                condition: expr(ExprKind::Bool(true)),
                body: vec![ret(int())],
            })],
            vec![stmt(StatementKind::DoWhile {
                condition: expr(ExprKind::Bool(true)),
                body: vec![stmt(StatementKind::Break), ret(int())],
            })],
        ] {
            assert!(
                validate(&Program {
                    main_is_arrow: false,
                    main_is_async: false,
                    types: vec![],
                    extensions: vec![],
                    classes: vec![],
                    functions: vec![function("f", Type::Int, vec![], body)],
                    statements: vec![]
                })
                .is_err()
            );
        }
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![],
                    vec![
                        stmt(StatementKind::While {
                            condition: expr(ExprKind::Bool(true)),
                            body: vec![stmt(StatementKind::Break)]
                        }),
                        ret(int())
                    ]
                )],
                statements: vec![]
            })
            .is_ok()
        );
    }
    #[test]
    /// Verifica referências antecipadas, recursão e parâmetros mutáveis.
    fn forward_calls_recursion_and_mutable_parameters() {
        let program = Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![
                function(
                    "first",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![ret(call("second", vec![id("x")]))],
                ),
                function(
                    "second",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![
                        stmt(StatementKind::Assign {
                            name: "x",
                            value: int(),
                        }),
                        stmt(StatementKind::If {
                            condition: expr(ExprKind::Bool(true)),
                            then_body: vec![ret(id("x"))],
                            else_body: Some(vec![ret(call("first", vec![id("x")]))]),
                        }),
                    ],
                ),
            ],
            statements: vec![print(call("first", vec![int()]))],
        };
        assert!(validate(&program).is_ok());
    }
    #[test]
    /// Verifica nomes de funções, quantidade e tipos dos argumentos.
    fn calls_require_known_function_correct_arity_and_types() {
        for arguments in [vec![], vec![int(), int()], vec![expr(ExprKind::Bool(true))]] {
            assert!(
                validate(&Program {
                    main_is_arrow: false,
                    main_is_async: false,
                    types: vec![],
                    extensions: vec![],
                    classes: vec![],
                    functions: vec![function(
                        "f",
                        Type::Int,
                        vec![("x", Type::Int)],
                        vec![ret(id("x"))]
                    )],
                    statements: vec![print(call("f", arguments))]
                })
                .is_err()
            );
        }
        assert!(check(vec![print(call("missing", vec![]))]).is_err());
        assert!(check(vec![stmt(StatementKind::Expression(call("main", vec![])))]).is_ok());
        assert!(
            check(vec![
                local("main", int()),
                stmt(StatementKind::Expression(call("main", vec![])))
            ])
            .is_err()
        );
    }
    #[test]
    /// Verifica compatibilidade e presença de retorno em todos os caminhos.
    fn returns_are_typed_and_required_on_every_path() {
        for body in [
            vec![],
            vec![stmt(StatementKind::Return(None))],
            vec![ret(expr(ExprKind::Bool(false)))],
            vec![stmt(StatementKind::If {
                condition: expr(ExprKind::Bool(true)),
                then_body: vec![ret(int())],
                else_body: None,
            })],
        ] {
            assert!(
                validate(&Program {
                    main_is_arrow: false,
                    main_is_async: false,
                    types: vec![],
                    extensions: vec![],
                    classes: vec![],
                    functions: vec![function("f", Type::Int, vec![], body)],
                    statements: vec![]
                })
                .is_err()
            );
        }
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![],
                    vec![stmt(StatementKind::Block(vec![ret(int())]))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(check(vec![ret(int())]).is_err());
        assert!(check(vec![stmt(StatementKind::Return(None))]).is_ok());
    }
    #[test]
    /// Verifica que chamadas void não escapam para contextos de valor.
    fn void_calls_are_statements_not_values() {
        assert!(
            check(vec![
                stmt(StatementKind::Expression(call("print", vec![int()]))),
                ret(call("print", vec![int()]))
            ])
            .is_ok()
        );
        for statement in [
            print(call("main", vec![])),
            local("x", call("main", vec![])),
            print(binary(
                BinaryOp::Equal,
                call("main", vec![]),
                call("main", vec![]),
            )),
        ] {
            assert!(check(vec![statement]).is_err());
        }
    }
    #[test]
    /// Rejeita funções, parâmetros duplicados e nomes especiais.
    fn duplicate_functions_parameters_and_reserved_main_are_rejected() {
        for functions in [
            vec![
                function("f", Type::Void, vec![], vec![]),
                function("f", Type::Void, vec![], vec![]),
            ],
            vec![function("main", Type::Void, vec![], vec![])],
            vec![function("print", Type::Void, vec![], vec![])],
            vec![function(
                "f",
                Type::Void,
                vec![("x", Type::Int), ("x", Type::Bool)],
                vec![],
            )],
        ] {
            assert!(
                validate(&Program {
                    main_is_arrow: false,
                    main_is_async: false,
                    types: vec![],
                    extensions: vec![],
                    classes: vec![],
                    functions,
                    statements: vec![]
                })
                .is_err()
            );
        }
    }
    #[test]
    /// Verifica o alcance dos parâmetros sobre nomes de tipos.
    fn parameters_shadow_body_types_but_not_signature_types() {
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("int", Type::Int), ("x", Type::Int)],
                    vec![ret(id("int"))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![local("x", int()), ret(id("x"))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("int", Type::Int)],
                    vec![
                        stmt(StatementKind::Variable {
                            is_const: false,
                            name: "x",
                            annotation: Some(Type::Int),
                            is_final: false,
                            initializer: int()
                        }),
                        ret(int())
                    ]
                )],
                statements: vec![]
            })
            .is_err()
        );
        assert!(
            check(vec![stmt(StatementKind::If {
                condition: int(),
                then_body: vec![],
                else_body: None
            })])
            .is_err()
        );
        assert!(
            check(vec![
                stmt(StatementKind::If {
                    condition: expr(ExprKind::Bool(true)),
                    then_body: vec![local("x", int())],
                    else_body: None
                }),
                print(id("x"))
            ])
            .is_err()
        );
    }
    #[test]
    /// Verifica ocultação de nomes e atribuições a escopos externos.
    fn nested_scopes_allow_shadowing_and_outer_assignment() {
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Block(vec![
                    stmt(StatementKind::Assign {
                        name: "x",
                        value: int()
                    }),
                    local("y", id("x")),
                    stmt(StatementKind::Block(vec![
                        local("x", expr(ExprKind::String("s"))),
                        print(id("x"))
                    ])),
                ])),
                print(id("x"))
            ])
            .is_ok()
        );
    }
    #[test]
    /// Rejeita uso antecipado de local que oculta um nome externo.
    fn later_local_shadows_outer_even_before_declaration() {
        let error = check(vec![
            local("x", int()),
            stmt(StatementKind::Block(vec![
                print(id("x")),
                local("x", int()),
            ])),
        ])
        .unwrap_err();
        assert!(error.message.contains("before its declaration"));
        assert_eq!(error.span, SPAN);
    }
    #[test]
    /// Rejeita leitura do próprio nome durante sua inicialização.
    fn self_initializer_cannot_read_outer_with_same_name() {
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Block(vec![local("x", id("x"))]))
            ])
            .unwrap_err()
            .message
            .contains("own initializer")
        );
    }
    #[test]
    /// Rejeita nomes duplicados e referências fora do escopo.
    fn duplicate_and_out_of_scope_names_are_rejected() {
        assert!(
            check(vec![local("x", int()), local("x", int())])
                .unwrap_err()
                .message
                .contains("Duplicate")
        );
        assert!(
            check(vec![
                stmt(StatementKind::Block(vec![local("x", int())])),
                print(id("x"))
            ])
            .unwrap_err()
            .message
            .contains("Unknown")
        );
    }
    #[test]
    /// Verifica imutabilidade de final e compatibilidade nas atribuições.
    fn final_and_assignment_types_are_enforced() {
        assert!(
            check(vec![
                stmt(StatementKind::Variable {
                    is_const: false,
                    name: "x",
                    annotation: None,
                    is_final: true,
                    initializer: int()
                }),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })
            ])
            .unwrap_err()
            .message
            .contains("final")
        );
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Bool(true))
                })
            ])
            .unwrap_err()
            .message
            .contains("Type mismatch")
        );
        assert!(
            check(vec![stmt(StatementKind::Variable {
                is_const: false,
                name: "x",
                annotation: Some(Type::Bool),
                is_final: false,
                initializer: int()
            })])
            .is_err()
        );
    }
    #[test]
    /// Verifica que strings decodificadas e emprestadas compartilham o mesmo tipo.
    fn owned_strings_match_borrowed_strings_in_operators_and_calls() {
        assert!(
            check(vec![
                print(binary(
                    BinaryOp::Add,
                    expr(ExprKind::String("texto")),
                    expr(ExprKind::OwnedString("\n🦀".into()))
                )),
                print(binary(
                    BinaryOp::Equal,
                    expr(ExprKind::OwnedString("a".into())),
                    expr(ExprKind::String("a"))
                )),
                local("s", expr(ExprKind::OwnedString("b".into()))),
                stmt(StatementKind::Assign {
                    name: "s",
                    value: expr(ExprKind::String("c"))
                }),
            ])
            .is_ok()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![
                    function(
                        "identity",
                        Type::String,
                        vec![("s", Type::String)],
                        vec![ret(id("s"))]
                    ),
                    function(
                        "decoded",
                        Type::String,
                        vec![],
                        vec![ret(expr(ExprKind::OwnedString("\n".into())))]
                    ),
                ],
                statements: vec![
                    print(call("identity", vec![expr(ExprKind::String("borrowed"))])),
                    print(call(
                        "identity",
                        vec![expr(ExprKind::OwnedString("owned".into()))]
                    )),
                    print(call("decoded", vec![])),
                ],
            })
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                expr(ExprKind::OwnedString("text".into())),
                int()
            ))])
            .is_err()
        );
        assert!(
            validate(&Program {
                main_is_arrow: false,
                main_is_async: false,
                types: vec![],
                extensions: vec![],
                classes: vec![],
                functions: vec![function(
                    "wrong",
                    Type::Int,
                    vec![],
                    vec![ret(expr(ExprKind::OwnedString("text".into())))]
                )],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Verifica tipos dos operadores e igualdade entre tipos distintos.
    #[test]
    fn typed_operators_and_cross_type_equality() {
        for op in [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::Less,
            BinaryOp::LessEqual,
            BinaryOp::Greater,
            BinaryOp::GreaterEqual,
            BinaryOp::Equal,
            BinaryOp::NotEqual,
        ] {
            assert!(check(vec![print(binary(op, int(), int()))]).is_ok());
        }
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                expr(ExprKind::String("a")),
                expr(ExprKind::String("b"))
            ))])
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Equal,
                int(),
                expr(ExprKind::Bool(false))
            ))])
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                int(),
                expr(ExprKind::String("b"))
            ))])
            .is_err()
        );
        for op in [BinaryOp::And, BinaryOp::Or] {
            assert!(
                check(vec![print(binary(
                    op,
                    expr(ExprKind::Bool(true)),
                    expr(ExprKind::Bool(false))
                ))])
                .is_ok()
            );
            assert!(check(vec![print(binary(op, int(), int()))]).is_err());
        }
        for (op, valid, invalid) in [
            (UnaryOp::Negate, int(), expr(ExprKind::Bool(false))),
            (UnaryOp::Not, expr(ExprKind::Bool(false)), int()),
        ] {
            assert!(
                check(vec![print(expr(ExprKind::Unary {
                    op,
                    operand: Box::new(valid)
                }))])
                .is_ok()
            );
            assert!(
                check(vec![print(expr(ExprKind::Unary {
                    op,
                    operand: Box::new(invalid)
                }))])
                .is_err()
            );
        }
    }
    #[test]
    /// Verifica anotações quando variáveis ocultam nomes de tipos.
    fn annotations_respect_shadowed_type_names() {
        /// Constrói uma declaração com anotação e inicializador compatíveis.
        fn annotated(ty: Type) -> Statement<'static> {
            let initializer = match ty {
                Type::Applied(_)
                | Type::Inferred
                | Type::Parameter(_)
                | Type::NullableParameter(_)
                | Type::Object
                | Type::NullableObject => unreachable!(),
                Type::Duration | Type::Timer => unreachable!(),
                Type::Void
                | Type::Null
                | Type::NullableInt
                | Type::NullableDouble
                | Type::NullableNum
                | Type::NullableString
                | Type::NullableBool
                | Type::Class(_)
                | Type::NullableClass(_) => unreachable!(),
                Type::Int => int(),
                Type::Double => expr(ExprKind::Double(1.5)),
                Type::Num => int(),
                Type::String => expr(ExprKind::String("text")),
                Type::Bool => expr(ExprKind::Bool(true)),
            };
            stmt(StatementKind::Variable {
                is_const: false,
                name: "value",
                annotation: Some(ty),
                is_final: false,
                initializer,
            })
        }
        for (name, ty) in [
            ("int", Type::Int),
            ("String", Type::String),
            ("bool", Type::Bool),
        ] {
            for statements in [
                vec![local(name, int()), annotated(ty)],
                vec![annotated(ty), local(name, int())],
                vec![
                    local(name, int()),
                    stmt(StatementKind::Block(vec![annotated(ty)])),
                ],
            ] {
                assert!(
                    check(statements)
                        .unwrap_err()
                        .message
                        .contains("shadows the type name")
                );
            }
            // Um local aninhado não oculta o tipo usado na anotação externa.
            assert!(
                check(vec![
                    annotated(ty),
                    stmt(StatementKind::Block(vec![local(name, int())]))
                ])
                .is_ok()
            );
            assert!(check(vec![local(name, int()), print(id(name))]).is_ok());
        }
    }
    #[test]
    /// Impede que print local seja tratado como a função nativa.
    fn shadowed_print_is_never_treated_as_builtin() {
        assert!(
            check(vec![local("print", int()), print(int())])
                .unwrap_err()
                .message
                .contains("shadows")
        );
        assert!(check(vec![print(int()), local("print", int())]).is_err());
        assert!(
            check(vec![
                stmt(StatementKind::Block(vec![local("print", int())])),
                print(int())
            ])
            .is_ok()
        );
    }
}
