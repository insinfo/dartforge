//! Resolve nomes, escopos e tipos do subconjunto de Dart 3.6.2.
//! Rejeita programas incompatíveis antes da geração de JavaScript.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Program, Statement, StatementKind, Type, UnaryOp,
};
use dartforge_syntax::{
    ClassKind, ClassModifier, ConstValue, ExtensionTarget, Resolution, TypeShape,
};
mod collections;
mod constants;
mod generics;
mod modifiers;
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
#[derive(Clone, PartialEq, Eq)]
struct Signature {
    generic_count: usize,
    is_getter: bool,
    parameters: Vec<Type>,
    result: Type,
}

#[derive(Clone, Copy)]
struct FieldInfo {
    ty: Type,
    is_final: bool,
}
#[derive(Clone)]
struct ClassInfo<'a> {
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
    methods: HashMap<&'a str, Signature>,
}
#[derive(Clone)]
struct ExtensionInfo<'a> {
    id: u32,
    on_type: Type,
    methods: HashMap<&'a str, (usize, Signature)>,
}
#[derive(Clone)]
struct Validator<'a> {
    exhaustive_switches: Rc<RefCell<HashSet<(usize, usize)>>>,
    type_parameters: Vec<&'a str>,
    switch_depth: usize,
    captured_writes: Rc<HashSet<&'a str>>,
    inferred_returns: Option<Rc<RefCell<Vec<Type>>>>,
    scopes: Vec<HashMap<&'a str, Binding>>,
    functions: HashMap<&'a str, Signature>,
    return_type: Type,
    loop_depth: usize,
    classes: HashMap<u32, ClassInfo<'a>>,
    current_class: Option<u32>,
    in_field_initializer: bool,
    extensions: Vec<ExtensionInfo<'a>>,
    current_extension: Option<usize>,
    resolution: Rc<RefCell<Resolution>>,
}

/// Valida nomes, tipos, chamadas, retornos e controle de laços antes da geração de código.
/// Aplicações `with` devem ser normalizadas pelo lowering de mixins antes desta etapa.
///
/// # Exemplos
/// ```
/// use dartforge_syntax::Program;
/// let programa = Program { types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] };
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
/// let programa = dartforge_syntax::Program { types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] };
/// assert!(dartforge_semantic::analyze(&programa).unwrap().extension_calls.is_empty());
/// ```
pub fn analyze(program: &Program<'_>) -> Result<Resolution, Diagnostic> {
    let mut validator = Validator {
        exhaustive_switches: Rc::new(RefCell::new(HashSet::new())),
        type_parameters: vec![],
        switch_depth: 0,
        captured_writes: Rc::new(collections::captured_writes(program)),
        inferred_returns: None,
        scopes: Vec::new(),
        functions: HashMap::new(),
        return_type: Type::Void,
        loop_depth: 0,
        classes: HashMap::new(),
        current_class: None,
        in_field_initializer: false,
        extensions: vec![],
        current_extension: None,
        resolution: Rc::new(RefCell::new(Resolution {
            types: program.types.clone(),
            ..Default::default()
        })),
    };
    validator.validate_shapes()?;
    validator.functions.insert(
        "main",
        Signature {
            generic_count: 0,
            is_getter: false,
            parameters: vec![],
            result: Type::Void,
        },
    );
    let mut class_names = HashSet::new();
    for class in &program.classes {
        if !class.is_mixin_application
            && (!class_names.insert(class.name)
                || matches!(class.name, "main" | "print" | "int" | "String" | "bool"))
        {
            return Err(Diagnostic::new(
                "Duplicate or reserved class name",
                class.span,
            ));
        }
        let mut info = ClassInfo {
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
                    .insert(
                        method.name,
                        Signature {
                            generic_count: method.type_parameters.len(),
                            is_getter: method.is_getter,
                            parameters: method.parameters.iter().map(|p| p.ty).collect(),
                            result: method.return_type,
                        },
                    )
                    .is_some()
            {
                return Err(Diagnostic::new("Duplicate class member", method.span));
            }
        }
        if validator.classes.insert(class.id, info).is_some() {
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
        if validator
            .functions
            .insert(
                function.name,
                Signature {
                    generic_count: function.type_parameters.len(),
                    is_getter: function.is_getter,
                    parameters: function
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect(),
                    result: function.return_type,
                },
            )
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
                .insert(
                    method.name,
                    (
                        index,
                        Signature {
                            generic_count: method.type_parameters.len(),
                            is_getter: method.is_getter,
                            parameters: method.parameters.iter().map(|p| p.ty).collect(),
                            result: method.return_type,
                        },
                    ),
                )
                .is_some()
            {
                return Err(Diagnostic::new("Duplicate extension method", method.span));
            }
        }
        validator.extensions.push(ExtensionInfo {
            id: extension.id,
            on_type: extension.on_type,
            methods,
        });
    }
    for function in &program.functions {
        validator.function(function)?;
    }
    validator.type_parameters.clear();
    for class in &program.classes {
        validator.validate_enum(class)?;
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
            validator.require_type(
                validator.value_expected(&field.initializer, Some(field.ty))?,
                field.ty,
                field.span,
            )?;
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
                    if base.parameters.len() != method.parameters.len() {
                        return Err(Diagnostic::new("Incompatible override arity", method.span));
                    }
                    for (expected, actual) in base.parameters.iter().zip(&method.parameters) {
                        validator.require_type(*expected, actual.ty, actual.span)?;
                    }
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
    }
    for class in &program.classes {
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
    validator.type_parameters.clear();
    validator.return_type = Type::Void;
    validator.loop_depth = 0;
    validator.block(&program.statements)?;
    let resolution = std::mem::take(&mut *validator.resolution.borrow_mut());
    Ok(resolution)
}
impl<'a> Validator<'a> {
    /// Valida arestas nominais antes de qualquer busca recursiva de membros.
    fn validate_class_graph(&self, program: &Program<'a>) -> Result<(), Diagnostic> {
        for class in &program.classes {
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
    /// Procura apenas corpos concretos herdados via extends, ignorando redeclarações abstratas.
    fn implementation(&self, id: u32, name: &str) -> Option<&Signature> {
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
        actual: &Signature,
        expected: &Signature,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if actual.is_getter != expected.is_getter {
            return Err(Diagnostic::new(
                "Getter and method contracts are incompatible",
                span,
            ));
        }
        if actual.parameters.len() != expected.parameters.len() {
            return Err(Diagnostic::new("Incompatible method contract arity", span));
        }
        for (&actual, &expected) in actual.parameters.iter().zip(&expected.parameters) {
            self.require_type(expected, actual, span)?;
        }
        if expected.result != Type::Void {
            self.require_type(actual.result, expected.result, span)?;
        }
        Ok(())
    }
    /// Confere todos os contratos, inclusive requisitos transitivos de interfaces.
    fn validate_contracts(&self, id: u32, span: Span) -> Result<(), Diagnostic> {
        let class = &self.classes[&id];
        let mut required = std::collections::BTreeMap::<&str, Vec<&Signature>>::new();
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
        if function.is_getter && !function.parameters.is_empty()
            || !function.type_parameters.is_empty()
        {
            return Err(Diagnostic::new(
                "Unsupported abstract getter or generic method signature",
                function.span,
            ));
        }
        self.check_type_name(function.return_type, function.span)?;
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
    /// Detecta membros que ocultariam uma referência global sem receptor explícito.
    fn has_implicit_member(&self, name: &str) -> bool {
        self.current_extension
            .is_some_and(|index| self.extensions[index].methods.contains_key(name))
            || self
                .current_class
                .is_some_and(|id| self.field(id, name).is_some() || self.method(id, name).is_some())
    }
    /// Valida uma função ou método com parâmetros mutáveis em escopo externo ao corpo.
    fn function(&mut self, function: &dartforge_syntax::Function<'a>) -> Result<(), Diagnostic> {
        self.type_parameters = function.type_parameters.clone();
        if function
            .type_parameters
            .iter()
            .collect::<HashSet<_>>()
            .len()
            != function.type_parameters.len()
        {
            return Err(Diagnostic::new("Duplicate type parameter", function.span));
        }
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
        let mut parameters = HashMap::new();
        for parameter in &function.parameters {
            self.check_type_name(parameter.ty, parameter.span)?;
            if parameter.ty == Type::Void {
                return Err(Diagnostic::new(
                    "Void parameters are unsupported",
                    parameter.span,
                ));
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
        self.scopes.push(parameters);
        self.return_type = function.return_type;
        self.loop_depth = 0;
        self.block(&function.body)?;
        self.scopes.pop();
        if !matches!(
            function.return_type,
            Type::Void
                | Type::Null
                | Type::NullableInt
                | Type::NullableString
                | Type::NullableBool
                | Type::NullableClass(_)
        ) && !self.returns(&function.body)
        {
            return Err(Diagnostic::new(
                format!(
                    "Function '{}' may complete without returning a value",
                    function.name
                ),
                function.span,
            ));
        }
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
    fn method(&self, id: u32, name: &str) -> Option<&Signature> {
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
    fn require_type(&self, actual: Type, expected: Type, span: Span) -> Result<(), Diagnostic> {
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
        match self.value(receiver)? {
            Type::Class(id) => Ok(id),
            _ => Err(Diagnostic::new(
                "Member access requires a non-null class instance",
                receiver.span,
            )),
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
            if let StatementKind::Variable {
                name,
                is_final,
                is_const,
                ..
            } = statement.kind
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
                let binding = self
                    .scopes
                    .last_mut()
                    .expect("current block scope")
                    .get_mut(name)
                    .expect("predeclared local");
                binding.constant = constant;
                binding.ty = Some(annotation.unwrap_or(actual));
                binding.promoted = if !self.captured_writes.contains(name)
                    && actual != Type::Null
                    && !is_nullable(actual)
                    && is_nullable(annotation.unwrap_or(actual))
                {
                    Some(non_null(annotation.unwrap_or(actual)))
                } else {
                    None
                };
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
                let binding = self.initialized(name, statement.span)?;
                if binding.is_final {
                    return Err(Diagnostic::new(
                        format!("Cannot assign to final variable '{name}'"),
                        statement.span,
                    ));
                }
                let actual = self.value_expected(value, binding.ty)?;
                self.require_type(actual, binding.ty.expect("initialized binding"), value.span)?;
                // A escrita remove a promoção anterior; valores não nulos estabelecem uma nova.
                let target = self
                    .scopes
                    .iter_mut()
                    .rev()
                    .find_map(|scope| scope.get_mut(name))
                    .expect("resolved binding");
                target.promoted = if !self.captured_writes.contains(name)
                    && actual != Type::Null
                    && !is_nullable(actual)
                    && is_nullable(target.ty.expect("initialized binding"))
                {
                    Some(non_null(target.ty.expect("initialized binding")))
                } else {
                    None
                };
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
                let ty = self.value(receiver)?;
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
                self.inferred_returns
                    .as_ref()
                    .expect("inferência ativa")
                    .borrow_mut()
                    .push(ty);
                Ok(())
            }
            StatementKind::Return(value) => match (self.return_type, value) {
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
                    && let Some(binding) = self
                        .scopes
                        .iter_mut()
                        .rev()
                        .find_map(|scope| scope.get_mut(name))
                    && let Some(ty) = binding.ty
                    && is_nullable(ty)
                {
                    binding.promoted = Some(non_null(ty));
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
                | StatementKind::DoWhile { body, .. } => self.invalidate_writes(body),
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
            Type::Parameter(id) => {
                return if (id as usize) < self.type_parameters.len()
                    && self.lookup(self.type_parameters[id as usize]).is_none()
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
            Type::String | Type::NullableString => "String",
            Type::Bool | Type::NullableBool => "bool",
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
            ExprKind::Const(inner) => {
                let ty = self.expression(inner)?;
                self.evaluate_constant(expression)?;
                Ok(ty)
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.switch_expression(scrutinee, arms, expression.span, None)
            }
            ExprKind::GenericCall {
                name,
                type_arguments,
                arguments,
            } => self.generic_call(name, type_arguments, arguments, expression.span, None),
            ExprKind::Closure { .. } | ExprKind::List { .. } => {
                unreachable!("expressões contextuais são tratadas no wrapper")
            }
            ExprKind::Index { receiver, index } => {
                let ty = self.value(receiver)?;
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
            ExprKind::This => {
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
                if !class.enum_values.contains(name) {
                    return Err(Diagnostic::new("Unknown enum value", expression.span));
                }
                Ok(Type::Class(*class_id))
            }
            ExprKind::Construct { class_id } => {
                let class = self
                    .classes
                    .get(class_id)
                    .ok_or_else(|| Diagnostic::new("Unknown class", expression.span))?;
                if class.is_abstract
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
                Ok(Type::Class(*class_id))
            }
            ExprKind::Member { receiver, name } => {
                let receiver_type = self.value(receiver)?;
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
                let receiver_type = self.value(receiver)?;
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
                if signature.parameters.len() != arguments.len() {
                    return Err(Diagnostic::new(
                        "Incorrect method argument count",
                        expression.span,
                    ));
                }
                for (argument, expected) in arguments.iter().zip(&signature.parameters) {
                    self.require_type(
                        self.value_expected(argument, Some(*expected))?,
                        *expected,
                        argument.span,
                    )?;
                }
                Ok(signature.result)
            }
            ExprKind::Null => Ok(Type::Null),
            ExprKind::Int(_) => Ok(Type::Int),
            ExprKind::String(_) | ExprKind::OwnedString(_) => Ok(Type::String),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Identifier(name) => {
                if self.lookup(name).is_none()
                    && let Some(id) = self.current_class
                {
                    if let Some(field) = self.field(id, name) {
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
                    return Ok(self.intern(TypeShape::Function {
                        result: signature.result,
                        parameters: signature.parameters.clone(),
                    }));
                }
                let binding = self.initialized(name, expression.span)?;
                Ok(binding
                    .promoted
                    .or(binding.ty)
                    .expect("initialized binding"))
            }
            ExprKind::Call { name, arguments } => {
                if self.lookup(name).is_some() {
                    let ty = self
                        .initialized(name, expression.span)?
                        .ty
                        .expect("initialized binding");
                    return self.invoke(ty, arguments, expression.span);
                }
                if self.has_implicit_member(name) {
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
                            if signature.parameters.len() != arguments.len() {
                                return Err(Diagnostic::new(
                                    "Incorrect implicit method argument count",
                                    expression.span,
                                ));
                            }
                            for (arg, expected) in arguments.iter().zip(&signature.parameters) {
                                self.require_type(
                                    self.value_expected(arg, Some(*expected))?,
                                    *expected,
                                    arg.span,
                                )?;
                            }
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
                if arguments.len() != signature.parameters.len() {
                    return Err(Diagnostic::new(
                        format!(
                            "Function '{name}' expects {} arguments, received {}",
                            signature.parameters.len(),
                            arguments.len()
                        ),
                        expression.span,
                    ));
                }
                for (argument, expected) in arguments.iter().zip(&signature.parameters) {
                    self.require_type(
                        self.value_expected(argument, Some(*expected))?,
                        *expected,
                        argument.span,
                    )?;
                }
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
                        Ok(non_null(ty))
                    };
                }
                let expected = match op {
                    UnaryOp::Negate => Type::Int,
                    UnaryOp::Not => Type::Bool,
                    UnaryOp::NullAssert => unreachable!(),
                };
                self.require_type(self.value(operand)?, expected, operand.span)?;
                Ok(expected)
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
                        let base = non_null(lhs);
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
                        if matches!(lhs, Type::Int | Type::String) && lhs == rhs {
                            Ok(lhs)
                        } else {
                            Err(Diagnostic::new(
                                "Operator '+' requires two int operands or two String operands",
                                expression.span,
                            ))
                        }
                    }
                    BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Remainder => {
                        self.require_type(lhs, Type::Int, left.span)?;
                        self.require_type(rhs, Type::Int, right.span)?;
                        Ok(Type::Int)
                    }
                    BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        self.require_type(lhs, Type::Int, left.span)?;
                        self.require_type(rhs, Type::Int, right.span)?;
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
fn is_nullable(ty: Type) -> bool {
    matches!(
        ty,
        Type::NullableInt | Type::NullableString | Type::NullableBool | Type::NullableClass(_)
    )
}
/// Remove a possibilidade de null de um tipo primitivo.
fn non_null(ty: Type) -> Type {
    match ty {
        Type::NullableInt => Type::Int,
        Type::NullableString => Type::String,
        Type::NullableBool => Type::Bool,
        Type::NullableClass(id) => Type::Class(id),
        other => other,
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
            type_parameters: vec![],
            is_getter: false,
            name,
            return_type: result,
            parameters: parameters
                .into_iter()
                .map(|(name, ty)| dartforge_syntax::Parameter {
                    name,
                    ty,
                    span: SPAN,
                })
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
                types: vec![],
                extensions: vec![extension(0, "E", Type::Class(0))],
                classes: vec![c],
                functions: vec![],
                statements: vec![print(extension_call(expr(ExprKind::Construct {
                    class_id: 0
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
                types: vec![],
                extensions: vec![extension(0, "E", Type::Class(0))],
                classes: vec![c],
                functions: vec![],
                statements: vec![print(extension_call(expr(ExprKind::Construct {
                    class_id: 0
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
                        initializer: expr(ExprKind::Construct { class_id: 1 }),
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
                        initializer: expr(ExprKind::Construct { class_id: 1 })
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
            initializer: int(),
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
                        initializer: expr(ExprKind::Construct { class_id: 1 })
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
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "Base", None), class(1, "Child", Some(0))],
                functions: vec![],
                statements: vec![stmt(StatementKind::Variable {
                    is_const: false,
                    name: "c",
                    annotation: Some(Type::Class(1)),
                    is_final: false,
                    initializer: expr(ExprKind::Construct { class_id: 0 })
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
                types: vec![],
                extensions: vec![],
                classes: vec![c],
                functions: vec![],
                statements: vec![
                    local("a", expr(ExprKind::Construct { class_id: 0 })),
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
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "A", None)],
                functions: vec![],
                statements: vec![print(expr(ExprKind::Construct { class_id: 0 }))]
            })
            .is_err()
        );
        assert!(
            validate(&Program {
                types: vec![],
                extensions: vec![],
                classes: vec![class(0, "A", None)],
                functions: vec![],
                statements: vec![
                    local("A", int()),
                    local("a", expr(ExprKind::Construct { class_id: 0 }))
                ]
            })
            .is_err()
        );
        let mut c = class(0, "A", None);
        c.fields.push(dartforge_syntax::Field {
            name: "self",
            ty: Type::Class(0),
            is_final: true,
            initializer: expr(ExprKind::This),
            span: SPAN,
        });
        assert!(
            validate(&Program {
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
            initializer: call("f", vec![]),
            span: SPAN,
        });
        assert!(
            validate(&Program {
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
            vec![ret(expr(ExprKind::Construct { class_id: 0 }))],
        ));
        assert!(
            validate(&Program {
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
            vec![ret(expr(ExprKind::Construct { class_id: 1 }))],
        ));
        assert!(
            validate(&Program {
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
            initializer: expr(ExprKind::Null),
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
                Type::Applied(_) | Type::Inferred | Type::Parameter(_) => unreachable!(),
                Type::Void
                | Type::Null
                | Type::NullableInt
                | Type::NullableString
                | Type::NullableBool
                | Type::Class(_)
                | Type::NullableClass(_) => unreachable!(),
                Type::Int => int(),
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
