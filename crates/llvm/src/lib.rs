//! Emissão textual de LLVM IR 17+ com ponteiros opacos, sem bindings ou unsafe.
//!
//! O subconjunto nativo usa int de 64 bits com transbordamento modular, bool e
//! void. Difere deliberadamente do backend JavaScript Number. Não fixa target
//! triple/data layout: a ferramenta nativa escolhe o alvo. A ABI externa contém
//! dartforge_print_i64(i64), dartforge_print_bool(i8) e dartforge_entry().
//! Consulte LLVM 17 LangRef (alloca, phi, br, add) e SDK Dart 3.6.2 sdk/lib/core/int.dart.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use dartforge_syntax::{BinaryOp, Expr, ExprKind, Statement, StatementKind, Type, UnaryOp};
use std::collections::HashMap;
use std::fmt::Write;

/// Tipo escalar interno da ABI e das operações LLVM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    Int,
    Bool,
    Void,
}
impl Ty {
    /// Nome do tipo na IR; bool usa i1 internamente e i8 na impressão externa.
    fn ir(self) -> &'static str {
        match self {
            Self::Int => "i64",
            Self::Bool => "i1",
            Self::Void => "void",
        }
    }
}

/// Produz LLVM IR de um módulo previamente validado semanticamente.
///
/// Valida todo o subconjunto antes de emitir, inclusive código morto. Identificadores
/// do usuário nunca são interpolados na IR. Locais usam alloca no bloco de entrada,
/// permitindo promoção por mem2reg. Não fornece runtime, linker ou objeto nativo.
///
/// # Erros
/// Rejeita classes, extensions, strings, null, tipos nullable e outras expressões
/// fora do contrato. Diagnósticos conservam o span da AST. Nomes/tipos incorretos de
/// AST construída manualmente também podem produzir diagnóstico.
///
/// ```
/// use dartforge_syntax::Program;
/// let module = dartforge_hir::lower(Program { classes: vec![], extensions: vec![], functions: vec![], statements: vec![] });
/// let ir = dartforge_llvm::emit(&module)?;
/// assert!(ir.contains("define void @dartforge_entry()"));
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn emit(module: &Module<'_>) -> Result<String, Diagnostic> {
    if let Some(class) = module.classes.first() {
        return Err(error(class.span, "classes"));
    }
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
        "; DartForge LLVM: inteiros i64 modulares, sem target fixo\ndeclare void @dartforge_print_i64(i64)\ndeclare void @dartforge_print_bool(i8)\n\n",
    );
    for function in &module.functions {
        let signature = &signatures[function.name];
        let mut emitter = FunctionEmitter::new(&signatures, signature.result);
        let mut params = vec![];
        for (index, parameter) in function.parameters.iter().enumerate() {
            let parameter_ty = signature.parameters[index];
            params.push(format!("{} %a{index}", parameter_ty.ir()));
            let pointer = emitter.local(parameter.name, parameter_ty);
            emitter.line(format!(
                "store {} %a{index}, ptr {pointer}",
                parameter_ty.ir()
            ));
        }
        emitter.block(&function.body)?;
        output.push_str(&emitter.finish(&signature.symbol, &params.join(", ")));
    }
    let mut emitter = FunctionEmitter::new(&signatures, Ty::Void);
    emitter.block(&module.statements)?;
    output.push_str(&emitter.finish("dartforge_entry", ""));
    Ok(output)
}

/// Diagnóstico do limite de recursos do backend, não erro genérico de parsing.
fn error(span: Span, feature: &str) -> Diagnostic {
    Diagnostic::new(format!("LLVM AOT ainda não suporta {feature}"), span)
}
/// Converte somente os tipos públicos do subconjunto nativo.
fn ty(value: Type, span: Span) -> Result<Ty, Diagnostic> {
    match value {
        Type::Int => Ok(Ty::Int),
        Type::Bool => Ok(Ty::Bool),
        Type::Void => Ok(Ty::Void),
        _ => Err(error(span, "este tipo")),
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
        StatementKind::Break | StatementKind::Continue => {}
        StatementKind::FieldAssign { .. } => return Err(error(statement.span, "campos")),
    }
    Ok(())
}
/// Rejeita expressões incompatíveis antes de qualquer simplificação ou emissão.
fn validate_expression(value: &Expr<'_>) -> Result<(), Diagnostic> {
    match &value.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Identifier(_) => {}
        ExprKind::Call { arguments, .. } => {
            for argument in arguments {
                validate_expression(argument)?;
            }
        }
        ExprKind::Unary { op, operand } => {
            if *op == UnaryOp::NullAssert {
                return Err(error(value.span, "asserção nullable"));
            }
            validate_expression(operand)?;
        }
        ExprKind::Binary { op, left, right } => {
            if *op == BinaryOp::IfNull {
                return Err(error(value.span, "operador ??"));
            }
            validate_expression(left)?;
            validate_expression(right)?;
        }
        _ => {
            return Err(error(
                value.span,
                "strings, null ou objetos nesta expressão",
            ));
        }
    }
    Ok(())
}

/// Assinatura nominal traduzida para símbolo numérico seguro.
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
    result: Ty,
    scopes: Vec<HashMap<String, (Ty, String)>>,
    loops: Vec<(String, String)>,
    allocas: String,
    code: String,
    next_value: usize,
    next_block: usize,
    current: String,
    terminated: bool,
}
impl<'a> FunctionEmitter<'a> {
    /// Inicia a função sem fixar alinhamento ou arquitetura de destino.
    fn new(signatures: &'a HashMap<String, Signature>, result: Ty) -> Self {
        Self {
            signatures,
            result,
            scopes: vec![HashMap::new()],
            loops: vec![],
            allocas: String::new(),
            code: String::new(),
            next_value: 0,
            next_block: 0,
            current: "entry".into(),
            terminated: false,
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
            self.line(if self.result == Ty::Void {
                "ret void".into()
            } else {
                "unreachable".into()
            });
        }
        format!(
            "define {} @{symbol}({parameters}) {{\nentry:\n{}{} }}\n\n",
            self.result.ir(),
            self.allocas,
            self.code
        )
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
            StatementKind::Variable {
                name, initializer, ..
            } => {
                let value = self.expression(initializer)?;
                if value.ty == Ty::Void {
                    return Err(error(statement.span, "variável void"));
                }
                let pointer = self.local(name, value.ty);
                self.line(format!(
                    "store {} {}, ptr {pointer}",
                    value.ty.ir(),
                    value.text
                ));
            }
            StatementKind::Assign { name, value } => {
                let (ty, pointer) = self.lookup(name, statement.span)?;
                let value = self.expression(value)?;
                Self::require(&value, ty, statement.span)?;
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
                    Self::require(&value, self.result, statement.span)?;
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
                    self.line("ret void".into());
                }
                self.terminated = true;
            }
            StatementKind::Block(body) => self.block(body)?,
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                let condition = self.expression(condition)?;
                Self::require(&condition, Ty::Bool, statement.span)?;
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
                let (end, next) = self.loops.last().ok_or_else(|| {
                    Diagnostic::new("controle de laço fora do laço", statement.span)
                })?;
                let target = if matches!(statement.kind, StatementKind::Break) {
                    end.clone()
                } else {
                    next.clone()
                };
                self.jump(&target);
            }
            _ => return Err(error(statement.span, "instrução")),
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
        Self::require(
            &value,
            Ty::Bool,
            condition.map_or(Span { start: 0, end: 0 }, |value| value.span),
        )?;
        self.branch(&value.text, &work, &end);
        self.start(&work);
        self.loops.push((end.clone(), step.clone()));
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
            Ty::Void => return Err(error(span, "impressão de void")),
        }
        Ok(())
    }
    /// Emite expressão em ordem; && e || produzem CFG e phi, nunca avaliação ávida.
    fn expression(&mut self, expression: &Expr<'_>) -> Result<Value, Diagnostic> {
        let value = match &expression.kind {
            ExprKind::Int(value) => Value {
                ty: Ty::Int,
                text: value.to_string(),
            },
            ExprKind::Bool(value) => Value {
                ty: Ty::Bool,
                text: value.to_string(),
            },
            ExprKind::Identifier(name) => {
                let (ty, pointer) = self.lookup(name, expression.span)?;
                let register = self.register();
                self.line(format!("{register} = load {}, ptr {pointer}", ty.ir()));
                Value { ty, text: register }
            }
            ExprKind::Call { name, arguments } => {
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
                    for (value, expected) in values.iter().zip(&signature.parameters) {
                        Self::require(value, *expected, expression.span)?;
                    }
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
                let right = self.expression(right)?;
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
            _ => return Err(error(expression.span, "expressão")),
        };
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
        Self::require(&left, Ty::Bool, span)?;
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
        Self::require(&right, Ty::Bool, span)?;
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
        assert!(ir.ends_with(" }\n\n"));
    }

    /// Recursos inválidos são rejeitados mesmo em funções ou ramos nunca executados.
    #[test]
    fn unsupported_dead_code_has_original_span() {
        for source in [
            "void main(){return;print('dead');}",
            "void unused(){print('dead');} void main(){}",
            "void main(){if(false){int? n=null;}}",
            "class C{} void main(){}",
            "extension E on int{int x(){return this;}} void main(){}",
        ] {
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
}
