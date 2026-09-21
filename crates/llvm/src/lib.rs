//! Emissão textual de LLVM IR 17+ com ponteiros opacos, sem bindings ou unsafe.
//!
//! O subconjunto nativo usa int de 64 bits com transbordamento modular, bool,
//! void, strings, classes com herança simples e suas formas nullable. Int? e
//! bool? usam {i1, payload}: presença e valor. Strings e objetos usam handles
//! i64 rastreados pelo GC, reservando zero para null. Métodos têm despacho virtual
//! e adaptadores de assinatura; campos herdados conservam seus slots de memória.
//! Cada ativação reserva slots de raízes por local e ponto estático de expressão.
//! Laços reutilizam esses slots; referências antigas ainda podem permanecer até
//! sobrescrita ou retorno. Não há análise completa de vivacidade.
//! Promoções semânticas usam unwrap checado; essa ABI interna não imita o SDK.
//! SDK 3.6.2 tests/language/if_null/behavior_test.dart fundamenta RHS condicional.
//! Falhas ! chamam dartforge_null_assert_fail() noreturn; null usa print_null().
//! Difere deliberadamente do backend JavaScript Number. Não fixa target
//! triple/data layout: a ferramenta nativa escolhe o alvo. O runtime fornece
//! impressão, falhas de null, frames de raízes, objetos e strings; o módulo define
//! dartforge_entry(). Strings internas UTF-8 ainda não oferecem indexação UTF-16.
//! Interfaces de métodos e classes abstratas participam do despacho; enums simples
//! oferecem identidade, index, name e nullabilidade por singletons gerenciados.
//! @Native aceita somente Int32/Int64/Void por ligação estática de símbolos C.
//! Adaptadores truncam Int32 e estendem seu sinal; isLeaf não altera a ABI nem habilita callbacks.
//! Switch avalia o discriminante uma vez e testa os braços na ordem declarada, sem
//! fallthrough implícito: cada caso encerra e a expressão une os braços em um phi.
//! Padrões constantes reutilizam a semântica de ==; padrões de tipo e bindings usam
//! dartforge_object_class contra as classes concretas do cone, sempre com handle não nulo.
//! break sai do switch mais próximo e continue segue pertencendo ao laço que o envolve.
//! Consulte LLVM 17 LangRef (alloca, phi, br, add) e SDK Dart 3.6.2 sdk/lib/core/int.dart.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, ParameterKind, Pattern, Statement, StatementKind, SwitchArm,
    SwitchCase, Type, UnaryOp,
};
use std::collections::HashMap;
use std::fmt::Write;
mod native;
mod objects;
use objects::Objects;

/// Tipo interno da ABI: escalares, agregados nullable e referências nominais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    Int,
    Bool,
    Void,
    Null,
    NullableInt,
    NullableBool,
    String,
    NullableString,
    Class(u32),
    NullableClass(u32),
}
impl Ty {
    /// Nome do tipo na IR; bool usa i1 internamente e i8 na impressão externa.
    fn ir(self) -> &'static str {
        match self {
            Self::Int
            | Self::String
            | Self::NullableString
            | Self::Class(_)
            | Self::NullableClass(_) => "i64",
            Self::Bool => "i1",
            Self::Void => "void",
            Self::Null | Self::NullableInt => "{ i1, i64 }",
            Self::NullableBool => "{ i1, i1 }",
        }
    }
    /// Payload escalar; Null não possui payload observável.
    fn base(self) -> Self {
        match self {
            Self::NullableInt => Self::Int,
            Self::NullableBool => Self::Bool,
            Self::NullableString => Self::String,
            Self::NullableClass(id) => Self::Class(id),
            _ => self,
        }
    }
    /// Referências são handles rastreados pelo runtime, com zero reservado a null.
    fn reference(self) -> bool {
        matches!(
            self,
            Self::String | Self::NullableString | Self::Class(_) | Self::NullableClass(_)
        )
    }
    /// Tipos que permitem ausência, representada por tag ou handle zero.
    fn nullable(self) -> bool {
        matches!(
            self,
            Self::Null
                | Self::NullableInt
                | Self::NullableBool
                | Self::NullableString
                | Self::NullableClass(_)
        )
    }
}

/// Produz LLVM IR de um módulo previamente validado semanticamente.
///
/// Valida todo o subconjunto antes de emitir, inclusive código morto. Identificadores
/// do usuário nunca são interpolados na IR. Locais usam alloca no bloco de entrada,
/// permitindo promoção por mem2reg. Não fornece runtime, linker ou objeto nativo.
///
/// # Erros
/// Rejeita extensions e outras expressões
/// fora do contrato. Diagnósticos conservam o span da AST. Nomes/tipos incorretos de
/// AST construída manualmente também podem produzir diagnóstico.
///
/// ```
/// use dartforge_syntax::Program;
/// let module = dartforge_hir::lower(Program { main_is_arrow: false, main_is_async: false, types: vec![], classes: vec![], extensions: vec![], functions: vec![], statements: vec![] });
/// let ir = dartforge_llvm::emit(&module)?;
/// assert!(ir.contains("define void @dartforge_entry()"));
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn emit(module: &Module<'_>) -> Result<String, Diagnostic> {
    if module.main_is_async
        || module
            .functions
            .iter()
            .chain(module.classes.iter().flat_map(|c| &c.methods))
            .chain(module.extensions.iter().flat_map(|e| &e.methods))
            .any(|f| f.is_async)
    {
        return Err(error(
            Span { start: 0, end: 0 },
            "async/await (máquina de estados nativa pendente)",
        ));
    }

    if let Some(factory) = module.classes.iter().flat_map(|c| &c.factories).next() {
        return Err(error(
            factory.span,
            "fábricas nomeadas (lowering nativo pendente)",
        ));
    }
    if module
        .resolution
        .expr_types
        .values()
        .any(|t| matches!(t, Type::Applied(_)))
    {
        let key = module
            .resolution
            .expr_types
            .iter()
            .find(|(_, t)| matches!(t, Type::Applied(_)))
            .unwrap()
            .0;
        return Err(error(
            Span {
                start: key.0,
                end: key.1,
            },
            "coleções e funções como valores (lowering nativo pendente)",
        ));
    }
    if let Some(f) = module
        .functions
        .iter()
        .find(|f| !f.type_parameters.is_empty())
    {
        return Err(error(f.span, "funções genéricas"));
    }
    if let Some(c) = module
        .classes
        .iter()
        .find(|c| !c.enum_values.is_empty() && (!c.fields.is_empty() || !c.methods.is_empty()))
    {
        return Err(error(c.span, "enums avançadas"));
    }
    native::validate(module)?;
    let objects = Objects::new(module)?;
    if let Some(extension) = module.extensions.first() {
        return Err(error(extension.span, "extensions"));
    }
    let mut signatures = HashMap::new();
    signatures.insert(
        "main".to_owned(),
        Signature {
            symbol: "dartforge_entry".into(),
            result: Ty::Void,
            parameters: vec![],
        },
    );
    for (index, function) in module.functions.iter().enumerate() {
        // Parâmetros opcionais e nomeados exigem um prólogo que o AOT não emite.
        if let Some(parameter) = function
            .parameters
            .iter()
            .find(|parameter| parameter.kind != ParameterKind::RequiredPositional)
        {
            return Err(error(
                parameter.span,
                "parâmetros opcionais ou nomeados no backend nativo",
            ));
        }
        let result = ty(function.return_type, function.span)?;
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| value_ty(parameter.ty, parameter.span))
            .collect::<Result<Vec<_>, _>>()?;
        if signatures
            .insert(
                function.name.to_owned(),
                Signature {
                    symbol: format!("df_fn_{index}"),
                    result,
                    parameters,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::new(
                "função duplicada na HIR LLVM",
                function.span,
            ));
        }
        validate_statements(&function.body)?;
    }
    validate_statements(&module.statements)?;
    let mut output = String::from(
        "; DartForge LLVM: inteiros i64 modulares, sem target fixo\ndeclare void @dartforge_print_i64(i64)\ndeclare void @dartforge_print_bool(i8)\ndeclare void @dartforge_print_null()\ndeclare void @dartforge_null_assert_fail() noreturn\n\n",
    );
    output.push_str(&native::declarations(module));
    for function in &module.functions {
        let signature = &signatures[function.name];
        if let Some(binding) = &function.native_binding {
            output.push_str(&native::wrapper(binding, &signature.symbol));
            continue;
        }
        let mut emitter = FunctionEmitter::new(&signatures, &objects, signature.result);
        let mut params = vec![];
        for (index, parameter) in function.parameters.iter().enumerate() {
            let parameter_ty = signature.parameters[index];
            params.push(format!("{} %a{index}", parameter_ty.ir()));
            emitter.root(&Value {
                ty: parameter_ty,
                text: format!("%a{index}"),
            });
            let pointer = emitter.local(parameter.name, parameter_ty);
            emitter.root_local(
                &pointer,
                &Value {
                    ty: parameter_ty,
                    text: format!("%a{index}"),
                },
            );
            emitter.line(format!(
                "store {} %a{index}, ptr {pointer}",
                parameter_ty.ir()
            ));
        }
        emitter.block(&function.body)?;
        output.push_str(&emitter.finish(&signature.symbol, &params.join(", ")));
    }
    let mut emitter = FunctionEmitter::new(&signatures, &objects, Ty::Void);
    emitter.block(&module.statements)?;
    output.push_str(&emitter.finish("dartforge_entry", ""));
    output.push_str(&objects.emit(module, &signatures)?);
    output.push_str(&objects.globals.borrow().join("\n"));
    output.push_str(objects::DECLARATIONS);
    Ok(output)
}

/// Diagnóstico do limite de recursos do backend, não erro genérico de parsing.
fn error(span: Span, feature: &str) -> Diagnostic {
    Diagnostic::new(format!("LLVM AOT ainda não suporta {feature}"), span)
}
/// Converte somente os tipos públicos do subconjunto nativo.
fn ty(value: Type, _span: Span) -> Result<Ty, Diagnostic> {
    match value {
        Type::Duration | Type::Timer => Err(error(_span, "Duration e Timer")),
        Type::Object | Type::NullableObject | Type::NullableParameter(_) => {
            Err(error(_span, "Object e parâmetros de tipo reificados"))
        }
        Type::Parameter(_) | Type::Applied(_) | Type::Inferred => Err(error(
            _span,
            "tipos estruturais e genéricos (coleções, funções e records)",
        )),
        Type::Int => Ok(Ty::Int),
        Type::Bool => Ok(Ty::Bool),
        Type::Void => Ok(Ty::Void),
        Type::Null => Ok(Ty::Null),
        Type::NullableInt => Ok(Ty::NullableInt),
        Type::NullableBool => Ok(Ty::NullableBool),
        Type::String => Ok(Ty::String),
        Type::NullableString => Ok(Ty::NullableString),
        Type::Class(id) => Ok(Ty::Class(id)),
        Type::NullableClass(id) => Ok(Ty::NullableClass(id)),
    }
}
/// Evita variáveis e parâmetros void mesmo em HIR criada manualmente.
fn value_ty(value: Type, span: Span) -> Result<Ty, Diagnostic> {
    let value = ty(value, span)?;
    if value == Ty::Void {
        Err(error(span, "valor void"))
    } else {
        Ok(value)
    }
}
/// Percorre todas as instruções, mesmo após return, break ou continue.
fn validate_statements(body: &[Statement<'_>]) -> Result<(), Diagnostic> {
    for statement in body {
        validate_statement(statement)?;
    }
    Ok(())
}
/// Valida recursos em cada posição, incluindo cabeçalhos e blocos não executados.
fn validate_statement(statement: &Statement<'_>) -> Result<(), Diagnostic> {
    match &statement.kind {
        StatementKind::RecordDestructure { .. } => {
            return Err(error(statement.span, "desestruturação de records"));
        }
        StatementKind::Switch { scrutinee, cases } => {
            validate_expression(scrutinee)?;
            for case in cases {
                validate_pattern(&case.pattern, case.span)?;
                if let Some(guard) = &case.guard {
                    validate_expression(guard)?;
                }
                validate_statements(&case.body)?;
            }
        }
        StatementKind::IndexAssign { .. } => {
            return Err(error(statement.span, "atribuição por índice"));
        }
        StatementKind::Variable {
            annotation,
            initializer,
            ..
        } => {
            if let Some(annotation) = annotation {
                value_ty(*annotation, statement.span)?;
            }
            validate_expression(initializer)?;
        }
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => validate_expression(value)?,
        StatementKind::Return(value) => {
            if let Some(value) = value {
                validate_expression(value)?;
            }
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            validate_expression(condition)?;
            validate_statements(then_body)?;
            if let Some(body) = else_body {
                validate_statements(body)?;
            }
        }
        StatementKind::While { condition, body } | StatementKind::DoWhile { condition, body } => {
            validate_expression(condition)?;
            validate_statements(body)?;
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(value) = initializer {
                validate_statement(value)?;
            }
            if let Some(value) = condition {
                validate_expression(value)?;
            }
            if let Some(value) = update {
                validate_statement(value)?;
            }
            validate_statements(body)?;
        }
        StatementKind::Block(body) => validate_statements(body)?,
        // O backend nativo ainda não tem desenrolamento de pilha, rótulos nem
        // iteração sobre Iterable; a rejeição acontece antes do driver.
        StatementKind::Try { .. } | StatementKind::Rethrow => {
            return Err(error(statement.span, "try, catch, finally e rethrow"));
        }
        StatementKind::Assert { .. } => {
            return Err(error(statement.span, "assert"));
        }
        StatementKind::ForIn { .. } => {
            return Err(error(statement.span, "for-in"));
        }
        StatementKind::Labeled { .. }
        | StatementKind::BreakLabel(_)
        | StatementKind::ContinueLabel(_) => {
            return Err(error(statement.span, "rótulos de laço"));
        }
        StatementKind::Break | StatementKind::Continue => {}
        StatementKind::FieldAssign {
            receiver, value, ..
        } => {
            validate_expression(receiver)?;
            validate_expression(value)?;
        }
    }
    Ok(())
}
/// Confere o padrão antes da emissão, inclusive em braços nunca selecionados.
///
/// `Pattern::Binding` sem anotação herda o tipo do discriminante e por isso não é
/// convertido aqui; a escolha depende do valor emitido e acontece em `bind_pattern`.
fn validate_pattern(pattern: &Pattern<'_>, span: Span) -> Result<(), Diagnostic> {
    match pattern {
        Pattern::Wildcard => Ok(()),
        Pattern::Constant(value) => validate_expression(value),
        Pattern::Type(annotation) => {
            value_ty(*annotation, span)?;
            Ok(())
        }
        Pattern::Binding { ty: annotation, .. } => {
            if *annotation != Type::Inferred {
                value_ty(*annotation, span)?;
            }
            Ok(())
        }
    }
}
/// Rejeita expressões incompatíveis antes de qualquer simplificação ou emissão.
fn validate_expression(value: &Expr<'_>) -> Result<(), Diagnostic> {
    match &value.kind {
        // O backend AOT ainda não modela passagem por rótulo.
        ExprKind::NamedArgument { .. } => {
            return Err(error(value.span, "argumentos nomeados"));
        }
        ExprKind::Await(_)
        | ExprKind::FutureValue { .. }
        | ExprKind::FutureDelayed { .. }
        | ExprKind::Duration { .. } => return Err(error(value.span, "operações assíncronas")),
        ExprKind::Cascade { .. } | ExprKind::CascadeReceiver => {
            return Err(error(value.span, "cascatas (lowering nativo pendente)"));
        }
        ExprKind::Map { .. } | ExprKind::NamedConstruct { .. } => {
            return Err(error(value.span, "mapas e fábricas nomeadas"));
        }
        ExprKind::Record { .. } => return Err(error(value.span, "records")),
        ExprKind::Conditional { .. } => {
            return Err(error(value.span, "o operador condicional"));
        }
        ExprKind::Throw(_) => return Err(error(value.span, "throw")),
        ExprKind::Const(e) => validate_expression(e)?,
        ExprKind::TypeTest { .. } | ExprKind::Cast { .. } => {
            return Err(error(value.span, "testes e casts de tipos reificados"));
        }
        ExprKind::GenericCall { .. } => {
            return Err(error(value.span, "chamadas genéricas"));
        }
        ExprKind::Switch { scrutinee, arms } => {
            validate_expression(scrutinee)?;
            for arm in arms {
                validate_pattern(&arm.pattern, arm.span)?;
                if let Some(guard) = &arm.guard {
                    validate_expression(guard)?;
                }
                validate_expression(&arm.value)?;
            }
        }
        ExprKind::NullAwareElement(_) => {
            return Err(error(value.span, "elementos null-aware de coleções"));
        }
        // A conversão toString ainda é função do runtime JavaScript.
        ExprKind::Interpolation(_) => {
            return Err(error(value.span, "interpolação de strings"));
        }
        ExprKind::DotShorthand { .. } => {
            return Err(error(value.span, "atalhos de ponto"));
        }
        ExprKind::Closure { .. }
        | ExprKind::List { .. }
        | ExprKind::Index { .. }
        | ExprKind::Invoke { .. } => return Err(error(value.span, "coleções e closures")),
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Identifier(_) | ExprKind::Null => {}
        ExprKind::This
        | ExprKind::EnumValue { .. }
        | ExprKind::String(_)
        | ExprKind::OwnedString(_) => {}
        ExprKind::Construct { arguments, .. } => {
            for argument in arguments {
                validate_expression(argument)?;
            }
        }
        ExprKind::Member { receiver, .. } => validate_expression(receiver)?,
        ExprKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            validate_expression(receiver)?;
            for arg in arguments {
                validate_expression(arg)?;
            }
        }
        ExprKind::Call { arguments, .. } => {
            for argument in arguments {
                validate_expression(argument)?;
            }
        }
        ExprKind::Unary { operand, .. } => {
            validate_expression(operand)?;
        }
        ExprKind::Binary {
            op: BinaryOp::Remainder,
            ..
        } => return Err(error(value.span, "módulo euclidiano")),
        ExprKind::Binary { left, right, .. } => {
            validate_expression(left)?;
            validate_expression(right)?;
        }
    }
    Ok(())
}

/// Assinatura nominal traduzida para símbolo numérico seguro.
#[derive(Clone)]
struct Signature {
    symbol: String,
    result: Ty,
    parameters: Vec<Ty>,
}
/// Operando SSA ou constante tipada.
struct Value {
    ty: Ty,
    text: String,
}
/// Estado de uma função, incluindo escopos léxicos e destinos de laços.
struct FunctionEmitter<'a> {
    signatures: &'a HashMap<String, Signature>,
    objects: &'a Objects,
    this_class: Option<u32>,
    result: Ty,
    scopes: Vec<HashMap<String, (Ty, String)>>,
    /// Destino de break e, somente para laços, destino de continue.
    loops: Vec<(String, Option<String>)>,
    allocas: String,
    code: String,
    next_value: usize,
    next_block: usize,
    current: String,
    terminated: bool,
    has_roots: bool,
    root_slots: usize,
    local_roots: HashMap<String, usize>,
    frame_exits: Vec<usize>,
}
impl<'a> FunctionEmitter<'a> {
    /// Inicia a função sem fixar alinhamento ou arquitetura de destino.
    fn new(signatures: &'a HashMap<String, Signature>, objects: &'a Objects, result: Ty) -> Self {
        Self {
            signatures,
            objects,
            this_class: None,
            result,
            scopes: vec![HashMap::new()],
            loops: vec![],
            allocas: String::new(),
            code: String::new(),
            next_value: 0,
            next_block: 0,
            current: "entry".into(),
            terminated: false,
            has_roots: false,
            root_slots: 0,
            local_roots: HashMap::new(),
            frame_exits: vec![],
        }
    }
    /// Reserva um nome SSA independente do texto do usuário.
    fn register(&mut self) -> String {
        let name = format!("%v{}", self.next_value);
        self.next_value += 1;
        name
    }
    /// Reserva um rótulo de bloco numericamente identificado.
    fn label(&mut self) -> String {
        let label = format!("b{}", self.next_block);
        self.next_block += 1;
        label
    }
    /// Acrescenta uma instrução no bloco aberto.
    fn line(&mut self, line: String) {
        writeln!(self.code, "  {line}").unwrap();
    }
    /// Termina o bloco atual com salto incondicional.
    fn jump(&mut self, label: &str) {
        self.line(format!("br label %{label}"));
        self.terminated = true;
    }
    /// Termina o bloco atual com dois sucessores.
    fn branch(&mut self, condition: &str, yes: &str, no: &str) {
        self.line(format!("br i1 {condition}, label %{yes}, label %{no}"));
        self.terminated = true;
    }
    /// Abre um bloco depois de terminar o predecessor.
    fn start(&mut self, label: &str) {
        writeln!(self.code, "{label}:").unwrap();
        self.current = label.into();
        self.terminated = false;
    }
    /// Reserva armazenamento no entry, embora a declaração esteja dentro de um laço.
    fn local(&mut self, name: &str, ty: Ty) -> String {
        let pointer = self.register();
        if ty.reference() {
            let slot = self.reserve_root();
            self.local_roots.insert(pointer.clone(), slot);
        }
        writeln!(self.allocas, "  {pointer} = alloca {}", ty.ir()).unwrap();
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.into(), (ty, pointer.clone()));
        pointer
    }
    /// Resolve o local mais próximo preservando sombreamento.
    fn lookup(&self, name: &str, span: Span) -> Result<(Ty, String), Diagnostic> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
            .ok_or_else(|| Diagnostic::new("local não resolvido na HIR LLVM", span))
    }
    /// Confere tipo de operando para evitar IR textual inválida.
    fn require(value: &Value, expected: Ty, span: Span) -> Result<(), Diagnostic> {
        if value.ty == expected {
            Ok(())
        } else {
            Err(Diagnostic::new("tipo incompatível na HIR LLVM", span))
        }
    }
    /// Fecha a função com terminador adequado também em bloco inalcançável.
    fn finish(mut self, symbol: &str, parameters: &str) -> String {
        if !self.terminated {
            self.end_frame();
            self.line(if self.result == Ty::Void {
                "ret void".into()
            } else if self.result.nullable() {
                format!("ret {} zeroinitializer", self.result.ir())
            } else {
                "unreachable".into()
            });
        }
        // Posições são offsets de bytes capturados durante a emissão; inserir em
        // ordem reversa preserva todos os offsets sem procurar texto na IR.
        if self.has_roots {
            for offset in self.frame_exits.iter().rev() {
                self.code.insert_str(
                    *offset,
                    "  call void @dartforge_gc_pop_frame(i64 %gcframe)\n",
                );
            }
            self.code.insert_str(
                0,
                &format!(
                    "  %gcframe = call i64 @dartforge_gc_push_frame(i64 {})\n",
                    self.root_slots
                ),
            );
        }
        format!(
            "define {} @{symbol}({parameters}) {{\nentry:\n{}{} }}\n\n",
            self.result.ir(),
            self.allocas,
            self.code
        )
    }
    /// Registra uma saída; funções sem raízes não precisam criar ou remover frame.
    fn end_frame(&mut self) {
        self.frame_exits.push(self.code.len());
    }
    /// Cria escopo lexical novo; instruções após terminador não são emitidas.
    fn block(&mut self, body: &[Statement<'_>]) -> Result<(), Diagnostic> {
        self.scopes.push(HashMap::new());
        for statement in body {
            if self.terminated {
                break;
            }
            self.statement(statement)?;
        }
        self.scopes.pop();
        Ok(())
    }
    /// Emite uma instrução e liga os blocos de controle correspondentes.
    fn statement(&mut self, statement: &Statement<'_>) -> Result<(), Diagnostic> {
        match &statement.kind {
            StatementKind::RecordDestructure { .. } => {
                return Err(error(statement.span, "desestruturação de records"));
            }
            StatementKind::Switch { scrutinee, cases } => {
                self.switch_statement(scrutinee, cases)?;
            }
            StatementKind::IndexAssign { .. } => {
                return Err(error(statement.span, "atribuição por índice"));
            }
            StatementKind::Variable {
                name,
                annotation,
                initializer,
                ..
            } => {
                let value = self.expression(initializer)?;
                if value.ty == Ty::Void {
                    return Err(error(statement.span, "variável void"));
                }
                let storage = annotation
                    .map(|t| value_ty(t, statement.span))
                    .transpose()?
                    .unwrap_or(value.ty);
                let value = self.coerce(value, storage, statement.span)?;
                let pointer = self.local(name, value.ty);
                self.root_local(&pointer, &value);
                self.line(format!(
                    "store {} {}, ptr {pointer}",
                    value.ty.ir(),
                    value.text
                ));
            }
            StatementKind::Assign { name, value } => {
                if self
                    .objects
                    .implicit_members
                    .contains(&(statement.span.start, statement.span.end))
                {
                    let receiver = self.this_value(statement.span)?;
                    let field = self.objects.field(receiver.ty, name, statement.span)?;
                    let value = self.expression(value)?;
                    self.store_field(&receiver, &field, value, statement.span)?;
                    return Ok(());
                }
                let (ty, pointer) = self.lookup(name, statement.span)?;
                let value = self.expression(value)?;
                let value = self.coerce(value, ty, statement.span)?;
                self.root_local(&pointer, &value);
                self.line(format!("store {} {}, ptr {pointer}", ty.ir(), value.text));
            }
            StatementKind::Print(value) => {
                let value = self.expression(value)?;
                self.print(value, statement.span)?;
            }
            StatementKind::Expression(value) => {
                self.expression(value)?;
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    let value = self.expression(value)?;
                    let value = self.coerce(value, self.result, statement.span)?;
                    self.end_frame();
                    self.line(if value.ty == Ty::Void {
                        "ret void".into()
                    } else {
                        format!("ret {} {}", value.ty.ir(), value.text)
                    });
                } else {
                    if self.result != Ty::Void {
                        return Err(Diagnostic::new(
                            "retorno sem valor na HIR LLVM",
                            statement.span,
                        ));
                    }
                    self.end_frame();
                    self.line("ret void".into());
                }
                self.terminated = true;
            }
            StatementKind::Block(body) => self.block(body)?,
            // A validação anterior já rejeitou estas formas; o braço existe
            // para manter a exaustividade com uma mensagem própria.
            StatementKind::Try { .. }
            | StatementKind::Rethrow
            | StatementKind::Assert { .. }
            | StatementKind::ForIn { .. }
            | StatementKind::Labeled { .. }
            | StatementKind::BreakLabel(_)
            | StatementKind::ContinueLabel(_) => {
                return Err(error(statement.span, "este controle de fluxo"));
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                let condition = self.expression(condition)?;
                let condition = self.coerce(condition, Ty::Bool, statement.span)?;
                let yes = self.label();
                let no = self.label();
                let end = self.label();
                self.branch(&condition.text, &yes, &no);
                self.start(&yes);
                self.block(then_body)?;
                if !self.terminated {
                    self.jump(&end);
                }
                self.start(&no);
                if let Some(body) = else_body {
                    self.block(body)?;
                }
                if !self.terminated {
                    self.jump(&end);
                }
                self.start(&end);
            }
            StatementKind::While { condition, body } => {
                self.loop_statement(None, Some(condition), None, body, false)?;
            }
            StatementKind::DoWhile { condition, body } => {
                self.loop_statement(None, Some(condition), None, body, true)?;
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                self.loop_statement(
                    initializer.as_deref(),
                    condition.as_ref(),
                    update.as_deref(),
                    body,
                    false,
                )?;
            }
            StatementKind::Break | StatementKind::Continue => {
                // break encerra o switch ou o laço mais próximo; continue ignora
                // switches e sempre retoma o laço textualmente mais interno.
                let target = if matches!(statement.kind, StatementKind::Break) {
                    self.loops.last().map(|(end, _)| end.clone())
                } else {
                    self.loops.iter().rev().find_map(|(_, next)| next.clone())
                };
                let target = target.ok_or_else(|| {
                    Diagnostic::new("controle de laço fora do laço", statement.span)
                })?;
                self.jump(&target);
            }
            StatementKind::FieldAssign {
                receiver,
                name,
                value,
            } => {
                let receiver = self.expression(receiver)?;
                let field = self.objects.field(receiver.ty, name, statement.span)?;
                let value = self.expression(value)?;
                self.store_field(&receiver, &field, value, statement.span)?;
            }
        }
        Ok(())
    }
    /// Liga condição, corpo e atualização; continue de for passa pela atualização.
    fn loop_statement(
        &mut self,
        initializer: Option<&Statement<'_>>,
        condition: Option<&Expr<'_>>,
        update: Option<&Statement<'_>>,
        body: &[Statement<'_>],
        first_body: bool,
    ) -> Result<(), Diagnostic> {
        self.scopes.push(HashMap::new());
        if let Some(initializer) = initializer {
            self.statement(initializer)?;
        }
        let test = self.label();
        let work = self.label();
        let step = self.label();
        let end = self.label();
        self.jump(if first_body { &work } else { &test });
        self.start(&test);
        let value = if let Some(condition) = condition {
            self.expression(condition)?
        } else {
            Value {
                ty: Ty::Bool,
                text: "true".into(),
            }
        };
        let value = self.coerce(
            value,
            Ty::Bool,
            condition.map_or(Span { start: 0, end: 0 }, |value| value.span),
        )?;
        self.branch(&value.text, &work, &end);
        self.start(&work);
        self.loops.push((end.clone(), Some(step.clone())));
        self.block(body)?;
        self.loops.pop();
        if !self.terminated {
            self.jump(&step);
        }
        self.start(&step);
        if let Some(update) = update {
            self.statement(update)?;
        }
        if !self.terminated {
            self.jump(&test);
        }
        self.start(&end);
        self.scopes.pop();
        Ok(())
    }
    /// Adapta bool i1 à ABI i8, sem varargs dependentes da plataforma.
    fn print(&mut self, value: Value, span: Span) -> Result<(), Diagnostic> {
        match value.ty {
            Ty::Int => self.line(format!(
                "call void @dartforge_print_i64(i64 {})",
                value.text
            )),
            Ty::Bool => {
                let register = self.register();
                self.line(format!("{register} = zext i1 {} to i8", value.text));
                self.line(format!("call void @dartforge_print_bool(i8 {register})"));
            }
            Ty::String | Ty::NullableString => self.line(format!(
                "call void @dartforge_print_string(i64 {})",
                value.text
            )),
            Ty::Class(_) | Ty::NullableClass(_) => return Err(error(span, "impressao de objetos")),
            Ty::Null => self.line("call void @dartforge_print_null()".into()),
            Ty::NullableInt | Ty::NullableBool => {
                let present = self.present(&value);
                let yes = self.label();
                let no = self.label();
                let end = self.label();
                self.branch(&present, &yes, &no);
                self.start(&yes);
                let payload = self.payload(&value);
                self.print(payload, span)?;
                self.jump(&end);
                self.start(&no);
                self.line("call void @dartforge_print_null()".into());
                self.jump(&end);
                self.start(&end);
            }
            Ty::Void => return Err(error(span, "impressão de void")),
        }
        Ok(())
    }
    /// Extrai a presença; payloads ausentes são inicializados, nunca poison.
    fn present(&mut self, value: &Value) -> String {
        if value.ty == Ty::Null {
            return "false".into();
        }
        if !value.ty.nullable() {
            return "true".into();
        }
        if value.ty.reference() {
            let r = self.register();
            self.line(format!("{r} = icmp ne i64 {}, 0", value.text));
            return r;
        }
        let register = self.register();
        self.line(format!(
            "{register} = extractvalue {} {}, 0",
            value.ty.ir(),
            value.text
        ));
        register
    }
    /// Extrai payload sem teste em bloco cuja presença já foi verificada.
    fn payload(&mut self, value: &Value) -> Value {
        if value.ty.reference() {
            return Value {
                ty: value.ty.base(),
                text: value.text.clone(),
            };
        }
        if !value.ty.nullable() || value.ty == Ty::Null {
            return Value {
                ty: value.ty,
                text: value.text.clone(),
            };
        }
        let register = self.register();
        self.line(format!(
            "{register} = extractvalue {} {}, 1",
            value.ty.ir(),
            value.text
        ));
        Value {
            ty: value.ty.base(),
            text: register,
        }
    }
    /// Implementa ! com avaliação única e falha explícita, sem comportamento indefinido.
    fn assert_present(&mut self, value: Value) -> Result<Value, Diagnostic> {
        if !value.ty.nullable() {
            return Ok(value);
        }
        let present = self.present(&value);
        let yes = self.label();
        let no = self.label();
        self.branch(&present, &yes, &no);
        self.start(&no);
        self.line("call void @dartforge_null_assert_fail()".into());
        self.line("unreachable".into());
        self.terminated = true;
        self.start(&yes);
        Ok(self.payload(&value))
    }
    /// Adapta armazenamento, argumentos e promoções já validadas pelo frontend.
    fn coerce(&mut self, value: Value, expected: Ty, span: Span) -> Result<Value, Diagnostic> {
        if value.ty == expected {
            return Ok(value);
        }
        if value.ty.reference()
            && expected.reference()
            && self.objects.assignable(value.ty.base(), expected.base())
        {
            let value = if value.ty.nullable() && !expected.nullable() {
                self.assert_present(value)?
            } else {
                value
            };
            return Ok(Value {
                ty: expected,
                text: value.text,
            });
        }
        if value.ty == Ty::Null && expected.nullable() {
            return Ok(Value {
                ty: expected,
                text: "zeroinitializer".into(),
            });
        }
        if expected.nullable() && expected != Ty::Null && value.ty == expected.base() {
            let tag = self.register();
            let payload = self.register();
            self.line(format!(
                "{tag} = insertvalue {} zeroinitializer, i1 true, 0",
                expected.ir()
            ));
            self.line(format!(
                "{payload} = insertvalue {} {tag}, {} {}, 1",
                expected.ir(),
                value.ty.ir(),
                value.text
            ));
            return Ok(Value {
                ty: expected,
                text: payload,
            });
        }
        if value.ty.nullable() && value.ty.base() == expected {
            return self.assert_present(value);
        }
        Err(Diagnostic::new("tipo incompatível na HIR LLVM", span))
    }
    /// Compara tags e payloads sem confundir int zero com bool false ou null.
    fn equality(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if left.ty == Ty::Void || right.ty == Ty::Void {
            return Err(error(span, "comparação void"));
        }
        if (left.ty.base() == Ty::String && right.ty.base() == Ty::String)
            || (matches!(left.ty.base(), Ty::Class(_)) && matches!(right.ty.base(), Ty::Class(_)))
        {
            let r = self.register();
            if left.ty.base() == Ty::String {
                let byte = self.register();
                self.line(format!(
                    "{byte} = call i8 @dartforge_string_equal(i64 {}, i64 {})",
                    left.text, right.text
                ));
                self.line(format!("{r} = trunc i8 {byte} to i1"));
            } else {
                self.line(format!("{r} = icmp eq i64 {}, {}", left.text, right.text));
            }
            let text = if op == BinaryOp::NotEqual {
                let inv = self.register();
                self.line(format!("{inv} = xor i1 {r}, true"));
                inv
            } else {
                r
            };
            return Ok(Value { ty: Ty::Bool, text });
        }
        let lp = self.present(&left);
        let rp = self.present(&right);
        let any = self.register();
        let absent = self.register();
        self.line(format!("{any} = or i1 {lp}, {rp}"));
        self.line(format!("{absent} = xor i1 {any}, true"));
        let mut equal = absent;
        if left.ty.base() == right.ty.base() && left.ty != Ty::Null {
            let l = self.payload(&left);
            let r = self.payload(&right);
            let payload = self.register();
            let both = self.register();
            let same = self.register();
            let total = self.register();
            self.line(format!(
                "{payload} = icmp eq {} {}, {}",
                l.ty.ir(),
                l.text,
                r.text
            ));
            self.line(format!("{both} = and i1 {lp}, {rp}"));
            self.line(format!("{same} = and i1 {both}, {payload}"));
            self.line(format!("{total} = or i1 {equal}, {same}"));
            equal = total;
        }
        if op == BinaryOp::NotEqual {
            let inverse = self.register();
            self.line(format!("{inverse} = xor i1 {equal}, true"));
            equal = inverse;
        }
        Ok(Value {
            ty: Ty::Bool,
            text: equal,
        })
    }
    /// RHS de ?? fica em bloco exclusivo; phi usa valores e predecessores reais.
    fn coalesce(&mut self, left: Value, right: &Expr<'_>, span: Span) -> Result<Value, Diagnostic> {
        if !left.ty.nullable() {
            return Ok(left);
        }
        let present = self.present(&left);
        let yes = self.label();
        let no = self.label();
        let end = self.label();
        self.branch(&present, &yes, &no);
        self.start(&no);
        let right = self.expression(right)?;
        let base = left.ty.base();
        // A análise permite RHS de outro tipo quando lhs está promovido e não
        // pode ser null. A HIR conserva o slot nullable: esse ramo é morto para
        // programas válidos. Falha explícita evita UB em HIR que viole o contrato.
        if left.ty != Ty::Null
            && right.ty != Ty::Null
            && !self.objects.assignable(right.ty.base(), base)
        {
            self.line("call void @dartforge_null_assert_fail()".into());
            self.line("unreachable".into());
            self.terminated = true;
            self.start(&yes);
            let value = self.payload(&left);
            self.jump(&end);
            self.start(&end);
            return Ok(value);
        }
        let result = if left.ty == Ty::Null {
            right.ty
        } else if right.ty == Ty::Null || right.ty.nullable() {
            match base {
                Ty::Int => Ty::NullableInt,
                Ty::Bool => Ty::NullableBool,
                Ty::String => Ty::NullableString,
                Ty::Class(id) => Ty::NullableClass(id),
                _ => return Err(error(span, "tipo de ??")),
            }
        } else {
            base
        };
        let right = self.coerce(right, result, span)?;
        let no_predecessor = self.current.clone();
        self.jump(&end);
        self.start(&yes);
        let left = if left.ty == Ty::Null {
            Value {
                ty: result,
                text: if result.nullable() {
                    "zeroinitializer".into()
                } else if result == Ty::Bool {
                    "false".into()
                } else {
                    "0".into()
                },
            }
        } else {
            self.payload(&left)
        };
        let left = self.coerce(left, result, span)?;
        let yes_predecessor = self.current.clone();
        self.jump(&end);
        self.start(&end);
        let register = self.register();
        self.line(format!(
            "{register} = phi {} [ {}, %{yes_predecessor} ], [ {}, %{no_predecessor} ]",
            result.ir(),
            left.text,
            right.text
        ));
        Ok(Value {
            ty: result,
            text: register,
        })
    }
    /// Emite expressão em ordem; && e || produzem CFG e phi, nunca avaliação ávida.
    fn expression(&mut self, expression: &Expr<'_>) -> Result<Value, Diagnostic> {
        let value = match &expression.kind {
            ExprKind::NamedArgument { .. } => {
                return Err(error(expression.span, "argumentos nomeados"));
            }
            ExprKind::Cascade { .. } | ExprKind::CascadeReceiver => {
                return Err(error(
                    expression.span,
                    "cascatas (lowering nativo pendente)",
                ));
            }
            ExprKind::Conditional { .. } => {
                return Err(error(expression.span, "o operador condicional"));
            }
            ExprKind::Throw(_) => return Err(error(expression.span, "throw")),
            ExprKind::Map { .. } | ExprKind::NamedConstruct { .. } => {
                return Err(error(expression.span, "mapas e fábricas nomeadas"));
            }
            ExprKind::Record { .. } => return Err(error(expression.span, "records")),
            ExprKind::Const(e) => self.expression(e)?,
            ExprKind::TypeTest { .. } | ExprKind::Cast { .. } => {
                return Err(error(expression.span, "testes e casts de tipos reificados"));
            }
            ExprKind::GenericCall { .. } => {
                return Err(error(expression.span, "chamadas genéricas"));
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.switch_expression(scrutinee, arms, expression.span)?
            }
            ExprKind::Await(_)
            | ExprKind::FutureValue { .. }
            | ExprKind::FutureDelayed { .. }
            | ExprKind::Duration { .. } => {
                return Err(error(expression.span, "operações assíncronas"));
            }
            ExprKind::Closure { .. }
            | ExprKind::List { .. }
            | ExprKind::Index { .. }
            | ExprKind::Invoke { .. } => return Err(error(expression.span, "coleções e closures")),
            ExprKind::Interpolation(_) => {
                return Err(error(expression.span, "interpolação de strings"));
            }
            ExprKind::NullAwareElement(_) => {
                return Err(error(expression.span, "elementos null-aware de coleções"));
            }
            ExprKind::DotShorthand { .. } => {
                return Err(error(expression.span, "atalhos de ponto"));
            }
            ExprKind::String(s) => self.string(s),
            ExprKind::OwnedString(s) => self.string(s),
            ExprKind::This => Value {
                ty: Ty::Class(
                    self.this_class
                        .ok_or_else(|| error(expression.span, "this fora de classe"))?,
                ),
                text: "%this".into(),
            },
            ExprKind::Construct {
                class_id,
                arguments,
            } => self.construct(*class_id, arguments, expression.span)?,
            ExprKind::EnumValue { class_id, name } => {
                self.enum_value(*class_id, name, expression.span)?
            }
            ExprKind::Member { receiver, name } => {
                let receiver = self.expression(receiver)?;
                self.read_member(receiver, name, expression.span)?
            }
            ExprKind::MethodCall {
                receiver,
                name,
                arguments,
            } => {
                let receiver = self.expression(receiver)?;
                self.method_call(receiver, name, arguments, expression.span)?
            }
            ExprKind::Null => Value {
                ty: Ty::Null,
                text: "zeroinitializer".into(),
            },
            ExprKind::Int(value) => Value {
                ty: Ty::Int,
                text: value.to_string(),
            },
            ExprKind::Bool(value) => Value {
                ty: Ty::Bool,
                text: value.to_string(),
            },
            ExprKind::Identifier(name) => {
                if self
                    .objects
                    .implicit_members
                    .contains(&(expression.span.start, expression.span.end))
                {
                    let receiver = self.this_value(expression.span)?;
                    let value = self.read_member(receiver, name, expression.span)?;
                    self.root(&value);
                    return Ok(value);
                }
                let (ty, pointer) = self.lookup(name, expression.span)?;
                let register = self.register();
                self.line(format!("{register} = load {}, ptr {pointer}", ty.ir()));
                Value { ty, text: register }
            }
            ExprKind::Call { name, arguments } => {
                if self
                    .objects
                    .implicit_members
                    .contains(&(expression.span.start, expression.span.end))
                {
                    let receiver = self.this_value(expression.span)?;
                    let value = self.method_call(receiver, name, arguments, expression.span)?;
                    self.root(&value);
                    return Ok(value);
                }
                let values = arguments
                    .iter()
                    .map(|argument| self.expression(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                if *name == "print" {
                    if values.len() != 1 {
                        return Err(Diagnostic::new("print exige um argumento", expression.span));
                    }
                    self.print(values.into_iter().next().unwrap(), expression.span)?;
                    Value {
                        ty: Ty::Void,
                        text: String::new(),
                    }
                } else {
                    let signature = self.signatures.get(*name).ok_or_else(|| {
                        Diagnostic::new("função não resolvida na HIR LLVM", expression.span)
                    })?;
                    if values.len() != signature.parameters.len() {
                        return Err(Diagnostic::new(
                            "aridade incorreta na HIR LLVM",
                            expression.span,
                        ));
                    }
                    let values = values
                        .into_iter()
                        .zip(&signature.parameters)
                        .map(|(value, expected)| self.coerce(value, *expected, expression.span))
                        .collect::<Result<Vec<_>, _>>()?;
                    let parameters = values
                        .iter()
                        .map(|value| format!("{} {}", value.ty.ir(), value.text))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let (result, symbol) = (signature.result, signature.symbol.clone());
                    if result == Ty::Void {
                        self.line(format!("call void @{symbol}({parameters})"));
                        Value {
                            ty: result,
                            text: String::new(),
                        }
                    } else {
                        let register = self.register();
                        self.line(format!(
                            "{register} = call {} @{symbol}({parameters})",
                            result.ir()
                        ));
                        Value {
                            ty: result,
                            text: register,
                        }
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                let value = self.expression(operand)?;
                if *op == UnaryOp::NullAssert {
                    return self.assert_present(value);
                }
                let expected = if *op == UnaryOp::Negate {
                    Ty::Int
                } else {
                    Ty::Bool
                };
                let value = self.coerce(value, expected, expression.span)?;
                let register = self.register();
                match op {
                    UnaryOp::Negate => {
                        Self::require(&value, Ty::Int, expression.span)?;
                        self.line(format!("{register} = sub i64 0, {}", value.text));
                    }
                    UnaryOp::Not => {
                        Self::require(&value, Ty::Bool, expression.span)?;
                        self.line(format!("{register} = xor i1 {}, true", value.text));
                    }
                    _ => return Err(error(expression.span, "operador unário")),
                }
                Value {
                    ty: value.ty,
                    text: register,
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.expression(left)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.lazy(*op, left, right, expression.span);
                }
                if *op == BinaryOp::IfNull {
                    return self.coalesce(left, right, expression.span);
                }
                let right = self.expression(right)?;
                if *op == BinaryOp::Add
                    && left.ty.base() == Ty::String
                    && right.ty.base() == Ty::String
                {
                    let left = self.coerce(left, Ty::String, expression.span)?;
                    let right = self.coerce(right, Ty::String, expression.span)?;
                    let r = self.register();
                    self.line(format!(
                        "{r} = call i64 @dartforge_string_concat(i64 {}, i64 {})",
                        left.text, right.text
                    ));
                    let v = Value {
                        ty: Ty::String,
                        text: r,
                    };
                    self.root(&v);
                    return Ok(v);
                }
                if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
                    && (left.ty.nullable()
                        || right.ty.nullable()
                        || left.ty.reference()
                        || right.ty.reference())
                {
                    return self.equality(*op, left, right, expression.span);
                }
                let (left, right) = if !matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
                    (
                        self.coerce(left, Ty::Int, expression.span)?,
                        self.coerce(right, Ty::Int, expression.span)?,
                    )
                } else {
                    (left, right)
                };
                if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) && left.ty != right.ty {
                    if left.ty == Ty::Void || right.ty == Ty::Void {
                        return Err(error(expression.span, "comparação void"));
                    }
                    return Ok(Value {
                        ty: Ty::Bool,
                        text: (*op == BinaryOp::NotEqual).to_string(),
                    });
                }
                let (instruction, result) = match op {
                    BinaryOp::Add => ("add", Ty::Int),
                    BinaryOp::Subtract => ("sub", Ty::Int),
                    BinaryOp::Multiply => ("mul", Ty::Int),
                    BinaryOp::Equal => ("icmp eq", Ty::Bool),
                    BinaryOp::NotEqual => ("icmp ne", Ty::Bool),
                    BinaryOp::Less => ("icmp slt", Ty::Bool),
                    BinaryOp::LessEqual => ("icmp sle", Ty::Bool),
                    BinaryOp::Greater => ("icmp sgt", Ty::Bool),
                    BinaryOp::GreaterEqual => ("icmp sge", Ty::Bool),
                    _ => return Err(error(expression.span, "operador binário")),
                };
                if !matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
                    Self::require(&left, Ty::Int, expression.span)?;
                }
                Self::require(&right, left.ty, expression.span)?;
                if left.ty == Ty::Void {
                    return Err(error(expression.span, "operando void"));
                }
                let register = self.register();
                self.line(format!(
                    "{register} = {instruction} {} {}, {}",
                    left.ty.ir(),
                    left.text,
                    right.text
                ));
                Value {
                    ty: result,
                    text: register,
                }
            }
        };
        self.root(&value);
        Ok(value)
    }
    /// Registra os predecessores reais após avaliar subexpressões com seus próprios blocos.
    fn lazy(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: &Expr<'_>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let left = self.coerce(left, Ty::Bool, span)?;
        let origin = self.current.clone();
        let rhs = self.label();
        let end = self.label();
        let is_and = op == BinaryOp::And;
        if is_and {
            self.branch(&left.text, &rhs, &end);
        } else {
            self.branch(&left.text, &end, &rhs);
        }
        self.start(&rhs);
        let right = self.expression(right)?;
        let right = self.coerce(right, Ty::Bool, span)?;
        let predecessor = self.current.clone();
        self.jump(&end);
        self.start(&end);
        let register = self.register();
        self.line(format!(
            "{register} = phi i1 [ {}, %{origin} ], [ {}, %{predecessor} ]",
            !is_and, right.text
        ));
        Ok(Value {
            ty: Ty::Bool,
            text: register,
        })
    }
    /// Compara discriminante e constante do padrão com a mesma semântica de `==`.
    ///
    /// Strings usam comparação de conteúdo do runtime, objetos e enums usam
    /// identidade de handle e escalares de tipos diferentes nunca são iguais.
    fn equal_values(&mut self, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
        if left.ty == Ty::Void || right.ty == Ty::Void {
            return Err(error(span, "comparação void"));
        }
        if left.ty.nullable() || right.ty.nullable() || left.ty.reference() || right.ty.reference()
        {
            return self.equality(BinaryOp::Equal, left, right, span);
        }
        if left.ty != right.ty {
            return Ok(Value {
                ty: Ty::Bool,
                text: "false".into(),
            });
        }
        let register = self.register();
        self.line(format!(
            "{register} = icmp eq {} {}, {}",
            left.ty.ir(),
            left.text,
            right.text
        ));
        Ok(Value {
            ty: Ty::Bool,
            text: register,
        })
    }
    /// Testa a identidade nominal concreta do handle contra um tipo alvo.
    ///
    /// O handle precisa ser não nulo: `dartforge_object_class` só aceita objetos.
    fn class_test(&mut self, handle: &str, id: u32, span: Span) -> Result<String, Diagnostic> {
        let descendants = self.objects.concrete_descendants(id);
        if descendants.is_empty() {
            return Ok("false".into());
        }
        let class = self.register();
        self.line(format!(
            "{class} = call i64 @dartforge_object_class(i64 {handle})"
        ));
        let mut accumulated: Option<String> = None;
        for child in descendants {
            let same = self.register();
            self.line(format!("{same} = icmp eq i64 {class}, {child}"));
            accumulated = Some(match accumulated {
                None => same,
                Some(previous) => {
                    let any = self.register();
                    self.line(format!("{any} = or i1 {previous}, {same}"));
                    any
                }
            });
        }
        accumulated.ok_or_else(|| Diagnostic::new("teste nominal vazio na HIR LLVM", span))
    }
    /// Produz um i1 que decide se o discriminante satisfaz o tipo do padrão.
    ///
    /// # Erros
    /// Rejeita alvos sem relação nominal representável nesta ABI.
    fn type_test(&mut self, value: &Value, target: Ty, span: Span) -> Result<String, Diagnostic> {
        if value.ty == Ty::Null {
            return Ok(if target.nullable() { "true" } else { "false" }.into());
        }
        if self.objects.assignable(value.ty.base(), target.base()) {
            // O tipo estático já garante a forma: só a ausência pode reprovar.
            if value.ty.nullable() && !target.nullable() {
                return Ok(self.present(value));
            }
            return Ok("true".into());
        }
        let (Ty::Class(_), Ty::Class(id)) = (value.ty.base(), target.base()) else {
            return Err(error(
                span,
                "padrão de tipo sem relação nominal representável",
            ));
        };
        if !self.objects.assignable(target.base(), value.ty.base()) {
            return Err(error(
                span,
                "padrão de tipo sem relação nominal representável",
            ));
        }
        if !value.ty.nullable() {
            return self.class_test(&value.text, id, span);
        }
        // Handle zero não é objeto: a consulta de classe fica em bloco protegido.
        let present = self.present(value);
        let check = self.label();
        let end = self.label();
        let origin = self.current.clone();
        self.branch(&present, &check, &end);
        self.start(&check);
        let matched = self.class_test(&value.text, id, span)?;
        let checked = self.current.clone();
        self.jump(&end);
        self.start(&end);
        let register = self.register();
        self.line(format!(
            "{register} = phi i1 [ {}, %{origin} ], [ {matched}, %{checked} ]",
            target.nullable()
        ));
        Ok(register)
    }
    /// Avalia o teste do padrão sem introduzir bindings nem avaliar a guarda.
    fn pattern_test(
        &mut self,
        pattern: &Pattern<'_>,
        value: &Value,
        span: Span,
    ) -> Result<String, Diagnostic> {
        match pattern {
            Pattern::Wildcard => Ok("true".into()),
            Pattern::Constant(constant) => {
                let constant = self.expression(constant)?;
                let scrutinee = Value {
                    ty: value.ty,
                    text: value.text.clone(),
                };
                Ok(self.equal_values(scrutinee, constant, span)?.text)
            }
            Pattern::Type(annotation) => {
                let target = value_ty(*annotation, span)?;
                self.type_test(value, target, span)
            }
            Pattern::Binding {
                ty: annotation,
                name: _,
            } => {
                let target = self.pattern_binding_ty(*annotation, value, span)?;
                self.type_test(value, target, span)
            }
        }
    }
    /// Binding sem anotação assume o tipo estático do discriminante.
    fn pattern_binding_ty(
        &self,
        annotation: Type,
        value: &Value,
        span: Span,
    ) -> Result<Ty, Diagnostic> {
        if annotation == Type::Inferred {
            Ok(value.ty)
        } else {
            value_ty(annotation, span)
        }
    }
    /// Adapta o discriminante já testado ao tipo do binding, sem repetir a checagem.
    fn bind_value(&mut self, value: &Value, target: Ty, span: Span) -> Result<Value, Diagnostic> {
        let scrutinee = Value {
            ty: value.ty,
            text: value.text.clone(),
        };
        if scrutinee.ty == target {
            return Ok(scrutinee);
        }
        // Referências compartilham a representação i64: o teste já fixou a classe.
        if scrutinee.ty.reference() && target.reference() {
            return Ok(Value {
                ty: target,
                text: scrutinee.text,
            });
        }
        if scrutinee.ty.nullable() && !target.nullable() && scrutinee.ty.base() == target {
            return Ok(self.payload(&scrutinee));
        }
        self.coerce(scrutinee, target, span)
    }
    /// Declara o local do binding no bloco do braço, já protegido contra o GC.
    fn bind_pattern(
        &mut self,
        pattern: &Pattern<'_>,
        value: &Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Pattern::Binding {
            ty: annotation,
            name,
        } = pattern
        else {
            return Ok(());
        };
        let target = self.pattern_binding_ty(*annotation, value, span)?;
        let bound = self.bind_value(value, target, span)?;
        let pointer = self.local(name, bound.ty);
        self.root_local(&pointer, &bound);
        self.line(format!(
            "store {} {}, ptr {pointer}",
            bound.ty.ir(),
            bound.text
        ));
        Ok(())
    }
    /// Abre o braço na ordem do Dart: padrão, binding e por fim a guarda.
    ///
    /// Devolve o rótulo do próximo candidato; o bloco aberto recebe o corpo do braço.
    fn arm_open(
        &mut self,
        pattern: &Pattern<'_>,
        guard: Option<&Expr<'_>>,
        value: &Value,
        span: Span,
    ) -> Result<String, Diagnostic> {
        let test = self.pattern_test(pattern, value, span)?;
        let body = self.label();
        let next = self.label();
        self.branch(&test, &body, &next);
        self.start(&body);
        self.bind_pattern(pattern, value, span)?;
        if let Some(guard) = guard {
            let condition = self.expression(guard)?;
            let condition = self.coerce(condition, Ty::Bool, guard.span)?;
            let run = self.label();
            self.branch(&condition.text, &run, &next);
            self.start(&run);
        }
        Ok(next)
    }
    /// Emite o switch instrução: discriminante único, sem fallthrough implícito.
    ///
    /// Cada caso termina saltando para o fim; `break` no corpo usa o mesmo destino
    /// e `continue` continua pertencendo ao laço que envolve o switch.
    fn switch_statement(
        &mut self,
        scrutinee: &Expr<'_>,
        cases: &[SwitchCase<'_>],
    ) -> Result<(), Diagnostic> {
        let value = self.expression(scrutinee)?;
        let end = self.label();
        self.loops.push((end.clone(), None));
        for case in cases {
            self.scopes.push(HashMap::new());
            let next = self.arm_open(&case.pattern, case.guard.as_ref(), &value, case.span)?;
            self.block(&case.body)?;
            if !self.terminated {
                self.jump(&end);
            }
            self.scopes.pop();
            self.start(&next);
        }
        self.loops.pop();
        // Switch instrução não precisa ser exaustivo: nenhum caso apenas segue adiante.
        self.jump(&end);
        self.start(&end);
        Ok(())
    }
    /// Emite o switch expressão com phi sobre os braços realmente selecionáveis.
    ///
    /// O tipo do resultado vem da análise semântica; a exaustividade já foi provada
    /// lá, então a queda final é um erro de runtime explícito, nunca um valor poison.
    ///
    /// # Erros
    /// Rejeita braços cujo padrão ou tipo resultante fica fora do subconjunto nativo.
    fn switch_expression(
        &mut self,
        scrutinee: &Expr<'_>,
        arms: &[SwitchArm<'_>],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let resolved = *self
            .objects
            .expr_types
            .get(&(span.start, span.end))
            .ok_or_else(|| Diagnostic::new("switch expressão sem tipo resolvido", span))?;
        let result = value_ty(resolved, span)?;
        if arms.is_empty() {
            return Err(Diagnostic::new("switch expressão sem braços", span));
        }
        let value = self.expression(scrutinee)?;
        let end = self.label();
        let mut incoming = vec![];
        for arm in arms {
            self.scopes.push(HashMap::new());
            let next = self.arm_open(&arm.pattern, arm.guard.as_ref(), &value, arm.span)?;
            let produced = self.expression(&arm.value)?;
            let produced = self.coerce(produced, result, arm.value.span)?;
            incoming.push(format!("[ {}, %{} ]", produced.text, self.current));
            self.jump(&end);
            self.scopes.pop();
            self.start(&next);
        }
        self.line("call void @dartforge_null_assert_fail()".into());
        self.line("unreachable".into());
        self.terminated = true;
        self.start(&end);
        let register = self.register();
        self.line(format!(
            "{register} = phi {} {}",
            result.ir(),
            incoming.join(", ")
        ));
        Ok(Value {
            ty: result,
            text: register,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Percorre o frontend real para exercitar o contrato HIR validado.
    fn compile(source: &str) -> Result<String, Diagnostic> {
        let tokens = dartforge_lexer::lex(source)?;
        let program = dartforge_parser::parse(&tokens, source.len())?;
        let resolution = dartforge_semantic::analyze(&program)?;
        emit(&dartforge_hir::lower_resolved(program, resolution))
    }

    /// A aritmética modular não recebe flags que transformam overflow em poison.
    #[test]
    fn emits_wrapping_i64_and_print_abi() {
        let ir = compile("void main(){var x=1073741824;x=x*x;x=x*8;print(x);print(x-1);print(-x);print(x*2);print(true);}").unwrap();
        assert!(ir.contains("mul i64"));
        assert!(ir.contains("sub i64 0,"));
        assert!(ir.contains("zext i1 true to i8"));
        assert!(!ir.contains("nsw"));
        assert!(!ir.contains("nuw"));
        assert!(!ir.contains("target triple"));
    }

    /// Funções recursivas usam símbolos numéricos, parâmetros e chamadas tipadas.
    #[test]
    fn emits_recursion_scopes_and_numeric_symbols() {
        let ir = compile("int factorial(int n){if(n<2){return 1;}else{return n*factorial(n-1);}} int shadow(int n){var n=3;return n;} void main(){print(factorial(5));print(shadow(9));}").unwrap();
        assert!(ir.contains("define i64 @df_fn_0(i64 %a0)"));
        assert!(ir.contains("call i64 @df_fn_0("));
        assert!(!ir.contains("factorial"));
        assert!(!ir.contains("shadow"));
        assert!(ir.contains("unreachable"));
    }

    /// Cada curto-circuito usa phi e blocos, inclusive em combinações aninhadas.
    #[test]
    fn short_circuit_uses_cfg_and_real_predecessors() {
        let ir = compile("bool effect(){print(99);return true;} void main(){print(false&&effect());print(true||effect());print((true&&effect())||(false&&effect()));}").unwrap();
        assert_eq!(ir.matches("phi i1").count(), 5);
        assert!(!ir.contains(" = and i1"));
        assert!(!ir.contains(" = or i1"));
        assert!(ir.contains("[ false, %entry ]"));
    }

    /// Allocas de laços permanecem no entry e as instruções mortas não são emitidas.
    #[test]
    fn loops_have_entry_allocas_and_terminators() {
        let ir = compile("void main(){var n=0;for(var i=0;i<3;i++){if(i==1){continue;}n+=i;}while(n<8){n++;if(n==6){break;}}do{n--;continue;}while(n>0);print(n);return;print(9);}").unwrap();
        let last_alloca = ir.rfind(" = alloca ").unwrap();
        let first_branch = ir.find("br label %").unwrap();
        assert!(last_alloca < first_branch);
        assert!(!ir.contains("@dartforge_print_i64(i64 9)"));
        assert!(ir.contains("ret void\n }"));
    }

    /// Recursos inválidos são rejeitados mesmo em funções ou ramos nunca executados.
    #[test]
    fn unsupported_dead_code_has_original_span() {
        {
            let source = "extension E on int{int x(){return this;}} void main(){}";
            let error = compile(source).unwrap_err();
            assert!(error.message.contains("LLVM AOT"), "{}", error.message);
            assert!(error.span.start < error.span.end && error.span.end <= source.len());
        }
    }

    /// Igualdade entre int e bool avalia ambos operandos e produz resultado constante.
    #[test]
    fn mixed_primitive_equality_does_not_emit_invalid_icmp() {
        let ir = compile("int effect(){print(1);return 1;} void main(){print(effect()==true);}")
            .unwrap();
        assert!(ir.contains("call i64 @df_fn_0()"));
        assert!(ir.contains("zext i1 false to i8"));
        assert!(!ir.contains("icmp eq"));
    }

    /// Coerções atravessam argumentos, retornos, slots e promoção após guarda.
    #[test]
    fn nullable_storage_calls_and_promotions() {
        let ir = compile("int? id(int? x){return x;} bool? absent(){} int inc(int? x){if(x==null){return 0;}return x+1;} void main(){int? n=id(3); n=null; print(n); n=5; print(inc(n)); print(absent());}").unwrap();
        assert!(ir.contains("define { i1, i64 } @df_fn_0({ i1, i64 } %a0)"));
        assert!(ir.contains("ret { i1, i1 } zeroinitializer"));
        assert!(ir.contains("insertvalue { i1, i64 }"));
        assert!(ir.contains("call void @dartforge_print_null()"));
    }

    /// ?? aninhado mantém phi agregado e ! verifica exatamente o resultado da chamada.
    #[test]
    fn nullable_lazy_and_checked_assertion() {
        let ir = compile("int? effect(){print(7);return null;} void main(){print(null ?? (effect() ?? effect())); print(effect()!);}").unwrap();
        assert_eq!(ir.matches("call { i1, i64 } @df_fn_0()").count(), 3);
        assert_eq!(ir.matches("phi { i1, i64 }").count(), 2);
        assert!(ir.contains("call void @dartforge_null_assert_fail()\n  unreachable"));
    }

    /// Int? e bool? têm representação distinta e igualdade comum somente em null.
    #[test]
    fn nullable_heterogeneous_equality() {
        let ir=compile("void compare(int? a,bool? b){print(a==b);print(a!=b);print(a==null);print(a==2);} void main(){compare(null,null);compare(0,false);}").unwrap();
        assert!(ir.contains("icmp eq i64"));
        assert!(!ir.contains("icmp eq {"));
        assert!(ir.contains(" = or i1"));
    }

    /// Corpus compartilhado exerce laços, reatribuições e fallthrough nullable.
    #[test]
    fn native_nullable_corpus_emits() {
        compile(include_str!("../../../tests/native/cases/null_safety.dart")).unwrap();
        compile(include_str!(
            "../../../tests/native/cases/null_flow_loops.dart"
        ))
        .unwrap();
    }
    /// RHS heterogeneo exige prova semantica de presenca do operando esquerdo.
    #[test]
    fn promoted_coalesce_keeps_dead_rhs_types_out_of_phi() {
        let ir = compile("void main(){int? n=1; print(n ?? false); print(2 ?? true);}").unwrap();
        assert!(!ir.contains("phi i64 [ false"));
        assert!(ir.contains("@dartforge_print_i64(i64 2)"));
        assert!(
            compile("int? maybe(){return null;} void main(){print(maybe() ?? true);}").is_err()
        );
    }

    /// Objetos exercitam despacho virtual, campos herdados, raízes e ABI adaptada.
    #[test]
    fn managed_objects_and_strings() {
        let ir = compile(include_str!(
            "../../../tests/native/cases/managed_objects.dart"
        ))
        .unwrap();
        assert!(ir.contains("switch i64 %class"));
        assert!(ir.contains("@dartforge_gc_set_root(i64 %gcframe"));
        assert!(ir.contains("call i64 @dartforge_string_concat"));
        assert!(ir.contains("call i8 @dartforge_string_equal"));
        assert!(ir.contains("insertvalue { i1, i64 }"));
    }

    /// Recursão e chamadores escalares não pagam pelo frame de um callee gerenciado.
    #[test]
    fn scalar_functions_omit_gc_frames() {
        let ir = compile("int fib(int n){if(n<2){return n;}return fib(n-1)+fib(n-2);} void main(){print(fib(8));}").unwrap();
        assert!(!ir.contains("call i64 @dartforge_gc_push_frame"));
        assert!(!ir.contains("call void @dartforge_gc_pop_frame"));
        let managed = compile("int text(){print('managed');return 1;} int caller(){return text();} void main(){print(caller());}").unwrap();
        assert_eq!(
            managed.matches("call i64 @dartforge_gc_push_frame").count(),
            1
        );
        assert_eq!(
            managed.matches("call void @dartforge_gc_pop_frame").count(),
            1
        );
        assert!(managed.contains("call void @dartforge_gc_set_root"));
    }

    /// Retornos emitidos antes da primeira raiz também encerram o frame da função.
    #[test]
    fn early_return_before_managed_branch_has_frame_exit() {
        let ir = compile(
            "void f(bool early){if(early){return;} print('later');} void main(){f(true);f(false);}",
        )
        .unwrap();
        assert_eq!(ir.matches("call i64 @dartforge_gc_push_frame").count(), 1);
        assert_eq!(ir.matches("call void @dartforge_gc_pop_frame").count(), 2);
    }

    /// O limite de iterações não altera slots, e referências locais têm raiz separada.
    #[test]
    fn loop_root_slots_are_static_and_locals_are_independent() {
        let source = "void main(){String saved=''; for(var i=0;i<8;i++){String current='new'+'!';if(i==0){saved=current;}}print(saved);}";
        let short = compile(source).unwrap();
        let long = compile(&source.replace("i<8", "i<2000")).unwrap();
        let frame = |ir: &str| {
            ir.lines()
                .find(|line| line.contains("call i64 @dartforge_gc_push_frame"))
                .unwrap()
                .to_owned()
        };
        assert_eq!(frame(&short), frame(&long));
        assert!(!short.contains("@dartforge_gc_root("));
        // Atualizar um local reutiliza o mesmo slot emitido na sua inicialização.
        let mut slots = HashMap::new();
        for line in short
            .lines()
            .filter(|line| line.contains("call void @dartforge_gc_set_root"))
        {
            let slot = line
                .split("i64 ")
                .nth(2)
                .unwrap()
                .split(',')
                .next()
                .unwrap();
            *slots.entry(slot).or_insert(0usize) += 1;
        }
        assert!(slots.values().any(|count| *count > 1));
        compile(include_str!(
            "../../../tests/native/cases/gc_root_slots.dart"
        ))
        .unwrap();
        compile(include_str!("../../../tests/native/cases/root_slots.dart")).unwrap();
    }

    /// Interfaces despacham para implementadores e enum_get preserva singletons.
    #[test]
    fn interfaces_abstract_redeclarations_and_enums() {
        let ir = compile(include_str!(
            "../../../tests/native/cases/interfaces_enums.dart"
        ))
        .unwrap();
        assert!(ir.contains("call i64 @dartforge_enum_get"));
        assert!(ir.contains("@df_dispatch_"));
        // As classes Named e NamedAgain são abstratas: nenhum construtor é emitido.
        assert!(!ir.contains("define i64 @df_new_2()"));
        assert!(!ir.contains("define i64 @df_new_3()"));
        // Enums são obtidos do runtime, nunca construídos como objetos comuns.
        assert!(!ir.contains("define i64 @df_new_0()"));
        assert!(!ir.contains("define i64 @df_new_1()"));
    }

    /// Adaptador void executa a implementação escalar e descarta somente o resultado.
    #[test]
    fn interface_void_discards_concrete_result() {
        let ir=compile("abstract class I{void run();} class C implements I{int run(){print(7);return 9;}} void main(){I item=C();item.run();}").unwrap();
        let adapter = ir
            .split("define void @df_dispatch_0_")
            .nth(1)
            .unwrap()
            .split("}\n")
            .next()
            .unwrap();
        assert!(adapter.contains("call i64 @df_method_1_0(i64 %this)"));
        assert!(adapter.contains("ret void"));
    }

    /// O discriminante é avaliado uma vez e cada caso encerra no mesmo bloco final.
    #[test]
    fn switch_statement_evaluates_scrutinee_once_without_fallthrough() {
        let ir = compile("int pick(){print(0);return 2;} void main(){switch(pick()){case 1: print(10); case 2: print(20); default: print(30);}}").unwrap();
        assert_eq!(ir.matches("call i64 @df_fn_0()").count(), 1);
        assert_eq!(ir.matches("icmp eq i64 %v0,").count(), 2);
        // Nenhum corpo continua no caso seguinte: todos saltam para o fim do switch.
        for printed in [10, 20, 30] {
            assert!(ir.contains(&format!(
                "call void @dartforge_print_i64(i64 {printed})\n  br label %b0\n"
            )));
        }
        // default vira um teste sempre verdadeiro, preservando a ordem dos casos.
        assert!(ir.contains("br i1 true, label %b5, label %b6"));
    }

    /// A guarda roda depois do binding e reprova para o próximo caso, não para o fim.
    #[test]
    fn switch_guard_runs_after_binding_and_falls_to_the_next_case() {
        let ir = compile("void main(){int n=3; switch(n){case int v when v>2: print(1); case 3: print(2); default: print(3);}}").unwrap();
        let arm = ir
            .split("br i1 true, label %b1, label %b2\n")
            .nth(1)
            .unwrap();
        // O binding é gravado antes de qualquer leitura feita pela guarda.
        assert!(arm.starts_with("b1:\n  store i64 %v1, ptr %v2\n"));
        assert!(arm.contains("icmp sgt i64 %v3, 2"));
        assert!(arm.contains("label %b3, label %b2\n"));
        // O destino reprovado é o teste do caso seguinte sobre o mesmo discriminante.
        assert!(ir.contains("b2:\n  %v5 = icmp eq i64 %v1, 3\n"));
    }

    /// Switch expressão junta os braços em um único phi e nunca produz valor poison.
    #[test]
    fn switch_expression_joins_arms_with_phi_and_traps_when_unmatched() {
        let ir = compile("void main(){print(switch(2){1=>10,2=>20,_=>30});}").unwrap();
        assert_eq!(ir.matches("phi i64").count(), 1);
        assert!(ir.contains("phi i64 [ 10, %b1 ], [ 20, %b3 ], [ 30, %b5 ]"));
        assert!(ir.contains("call void @dartforge_null_assert_fail()\n  unreachable"));
    }

    /// Strings comparam conteúdo, enums e classes seladas comparam identidade nominal.
    #[test]
    fn switch_over_strings_enums_and_sealed_hierarchies() {
        let text = compile(
            "void main(){String s='b'; switch(s){case 'a': print(1); case 'b': print(2);}}",
        )
        .unwrap();
        assert_eq!(text.matches("call i8 @dartforge_string_equal").count(), 2);
        let enums = compile("enum C{red,green} void main(){C c=C.red; switch(c){case C.red: print(1); case C.green: print(2);} print(switch(c){C.red=>1,C.green=>2});}").unwrap();
        assert_eq!(enums.matches("call i64 @dartforge_enum_get").count(), 5);
        assert!(!enums.contains("call i64 @dartforge_object_class"));
        let sealed = compile("sealed class S{} class A extends S{} class B extends S{} void main(){S v=A(); switch(v){case A a: print(1); case B(): print(2);}}").unwrap();
        assert_eq!(
            sealed.matches("call i64 @dartforge_object_class").count(),
            2
        );
        // Handle ausente nunca chega à consulta de classe do runtime.
        let optional = compile("sealed class S{} class A extends S{} class B extends S{} void main(){S? v=A(); switch(v){case A a: print(1); case B(): print(2); case null: print(0);}}").unwrap();
        assert!(optional.contains("icmp ne i64 %v2, 0\n  br i1 %v3, label %b1, label %b2"));
        assert!(optional.contains("phi i1 [ false, %entry ], [ %v5, %b1 ]"));
    }

    /// Discriminante nullable escolhe o caso null antes de expor o payload promovido.
    #[test]
    fn switch_on_nullable_int_binds_only_the_present_payload() {
        let ir =
            compile("void main(){int? n=1; switch(n){case null: print(0); case int v: print(v);}}")
                .unwrap();
        assert!(ir.contains("extractvalue { i1, i64 } %v3, 0"));
        assert!(ir.contains("b3:\n  %v8 = extractvalue { i1, i64 } %v3, 1\n"));
    }

    /// break encerra somente o switch; continue continua pertencendo ao laço externo.
    #[test]
    fn switch_break_ends_the_case_and_continue_reaches_the_loop() {
        let ir = compile("void main(){for(var i=0;i<3;i++){switch(i){case 0: continue; case 1: break; default: print(i);} print(9);}}").unwrap();
        // b2 é o passo do for e b4 é o fim do switch.
        assert!(ir.contains("b5:\n  br label %b2\n"));
        assert!(ir.contains("b7:\n  br label %b4\n"));
        assert!(ir.contains("b4:\n  call void @dartforge_print_i64(i64 9)\n  br label %b2\n"));
    }

    /// Recursos sem lowering dentro de padrões, guardas e corpos conservam seu span.
    #[test]
    fn unsupported_switch_members_keep_original_span_and_reason() {
        for (source, fragment) in [
            (
                "void main(){int x=1; switch(x){case 1: print(x as int); default: print(2);}}",
                "x as int",
            ),
            (
                "void main(){int x=1; switch(x){case 1 when x is int: print(1); default: print(2);}}",
                "x is int",
            ),
            (
                "void main(){int x=1; print(switch(x){1 when x is int=>1,_=>0});}",
                "x is int",
            ),
            (
                "void main(){int x=1; print(switch(x){1=>x as int,_=>0});}",
                "x as int",
            ),
        ] {
            let error = compile(source).unwrap_err();
            assert_eq!(
                error.message,
                "LLVM AOT ainda não suporta testes e casts de tipos reificados"
            );
            assert_eq!(&source[error.span.start..error.span.end], fragment);
        }
    }
}
