//! Metaprogramação experimental: expande macros incorporadas em AST antes da análise.
//!
//! `JsonCodable` e `DataClass` são implementadas em Rust e geram declarações Dart
//! comuns, verificadas pelo mesmo sistema de tipos da aplicação. Esta fase não
//! executa programas Dart arbitrários nem implementa o protocolo histórico de macros.
//!
//! # Limites do subconjunto que moldam o código gerado
//!
//! O subconjunto aceito pelo parser não tem parâmetros nomeados ou opcionais,
//! sobrecarga de `operator ==`, `toString()` nem interpolação de strings. As três
//! ausências aparecem diretamente na forma dos membros gerados por `DataClass`:
//!
//! - `copyWith` recebe todos os campos como parâmetros posicionais anuláveis e
//!   trata `null` como "mantém o valor atual"; não há como atribuir `null` a um
//!   campo anulável por meio dele.
//! - A igualdade estrutural é exposta como `bool igualA(Nome outro)`, um método
//!   comum, porque `operator ==` não é declarável.
//! - `descrever()` só imprime o valor de campos `String`/`String?`; campos `int` e
//!   `bool` aparecem como marcadores de tipo, porque `+` exige dois operandos
//!   `int` ou dois `String` e não existe conversão numérica para texto.
//!
//! Esses limites estão descritos em `docs/IMPLEMENTACAO-24.md` junto com o contrato
//! completo e as medições.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::*;
mod augment;
mod cache;
mod data_class;
pub use augment::{AugmentedMember, ClassAugmentation, augmentation_library, augmentations};
pub use cache::{MacroCacheStats, MacroSession};

/// Macro incorporada reconhecida por esta fase da compilação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MacroKind {
    /// Serialização por mapa: `fromJson` e `toJson`.
    JsonCodable,
    /// Classe de dados: `copyWith`, `igualA` e `descrever`.
    DataClass,
}
impl MacroKind {
    /// Devolve o nome escrito na anotação, usado em diagnósticos e no texto de aumento.
    ///
    /// ```
    /// assert_eq!(dartforge_macros::MacroKind::DataClass.name(), "DataClass");
    /// ```
    pub const fn name(self) -> &'static str {
        match self {
            Self::JsonCodable => "JsonCodable",
            Self::DataClass => "DataClass",
        }
    }
}

/// Anotação que autorizou a geração, junto da macro correspondente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Origin {
    pub span: Span,
    pub kind: MacroKind,
}

/// Proveniência de um nó sintético, com origem na anotação que pediu a expansão.
#[derive(Debug, Clone)]
pub struct GeneratedOrigin {
    pub generated: Span,
    pub annotation: Span,
    /// Macro responsável pelo nó; prefixa a mensagem devolvida por `remap`.
    pub kind: MacroKind,
}
/// Resultado determinístico da expansão; spans sintéticos nunca colidem com a fonte.
#[derive(Debug, Clone)]
pub struct ExpansionReport {
    pub plan_hits: usize,
    pub plan_misses: usize,
    pub plan_evictions: usize,
    pub materialized_nodes: usize,
    pub phases: [PhaseStats; 3],
    pub applications: usize,
    pub generated_declarations: usize,
    pub origins: Vec<GeneratedOrigin>,
    pub extent: usize,
}
impl ExpansionReport {
    /// Traduz um diagnóstico gerado de volta à anotação da biblioteca original.
    pub fn remap(&self, mut diagnostic: Diagnostic) -> Diagnostic {
        if let Some(origin) = self
            .origins
            .iter()
            .find(|origin| origin.generated == diagnostic.span)
        {
            diagnostic.span = origin.annotation;
            diagnostic.message = format!("{}: {}", origin.kind.name(), diagnostic.message);
        }
        diagnostic
    }
    /// Reserva um intervalo exclusivo para cada expressão, parâmetro ou declaração.
    pub(crate) fn span(&mut self, origin: Origin) -> Result<Span, Diagnostic> {
        let start = self
            .extent
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new("Macro span space exhausted", origin.span))?;
        self.extent = start
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new("Macro span space exhausted", origin.span))?;
        let generated = Span {
            start,
            end: self.extent,
        };
        self.origins.push(GeneratedOrigin {
            generated,
            annotation: origin.span,
            kind: origin.kind,
        });
        Ok(generated)
    }
    /// Cria expressão comum com identidade própria na tabela de resolução.
    pub(crate) fn expr<'a>(
        &mut self,
        kind: ExprKind<'a>,
        origin: Origin,
    ) -> Result<Expr<'a>, Diagnostic> {
        Ok(Expr {
            kind,
            span: self.span(origin)?,
        })
    }
    /// Cria um `return` sintético com intervalo próprio.
    pub(crate) fn ret<'a>(
        &mut self,
        value: Expr<'a>,
        origin: Origin,
    ) -> Result<Vec<Statement<'a>>, Diagnostic> {
        Ok(vec![Statement {
            kind: StatementKind::Return(Some(value)),
            span: self.span(origin)?,
        }])
    }
    /// Registra as declarações reservadas por uma macro na barreira Declarations.
    pub(crate) fn declared(&mut self, count: usize) {
        self.generated_declarations += count;
        self.phases[1].generated_declarations += count;
    }
}

/// Barreiras globais, executadas nesta ordem para todas as aplicações.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroPhase {
    Types,
    Declarations,
    Definitions,
}
/// Contadores por fase; Types é no-op real para as macros incorporadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseStats {
    pub phase: MacroPhase,
    pub applications: usize,
    pub generated_declarations: usize,
}
impl Default for ExpansionReport {
    /// Mantém o relatório vazio sem reserva de memória para programas comuns.
    fn default() -> Self {
        Self {
            plan_hits: 0,
            plan_misses: 0,
            plan_evictions: 0,
            materialized_nodes: 0,
            applications: 0,
            generated_declarations: 0,
            origins: Vec::new(),
            extent: 0,
            phases: [
                PhaseStats {
                    phase: MacroPhase::Types,
                    applications: 0,
                    generated_declarations: 0,
                },
                PhaseStats {
                    phase: MacroPhase::Declarations,
                    applications: 0,
                    generated_declarations: 0,
                },
                PhaseStats {
                    phase: MacroPhase::Definitions,
                    applications: 0,
                    generated_declarations: 0,
                },
            ],
        }
    }
}
/// Campo normalizado: somente tipos escalares sem IDs ou referências ao programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedField {
    pub name: String,
    pub ty: Type,
    pub is_final: bool,
}
/// Parte do esquema que não vem dos campos; distingue as macros aplicadas à classe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlanShape {
    pub generate_constructor: bool,
    pub json_codable: bool,
    pub data_class: bool,
}
/// Plano reutilizável de geração, independente de spans e identidade da biblioteca.
///
/// As duas macros compartilham um único plano por classe porque compartilham o
/// construtor posicional; os conjuntos de membros gerados são disjuntos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroPlan {
    pub fields: Vec<PlannedField>,
    pub generate_constructor: bool,
    /// Reserva `fromJson`/`toJson` quando a classe aplica `@JsonCodable()`.
    pub json_codable: bool,
    /// Reserva `copyWith`/`igualA`/`descrever` quando a classe aplica `@DataClass()`.
    pub data_class: bool,
}
/// Versão explícita do contrato do plano para invalidar caches persistentes.
///
/// A versão 2 acrescentou as macros aplicadas ao esquema: planos gravados pela
/// versão 1 descrevem apenas `JsonCodable` e não podem ser reutilizados.
pub const PLAN_VERSION: u32 = 2;

/// Nomes de membros reservados por `@JsonCodable()`.
const JSON_MEMBERS: [&str; 2] = ["fromJson", "toJson"];

/// Anotações de macro válidas encontradas em uma classe, com o span de cada uma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Targets {
    pub json: Option<Span>,
    pub data: Option<Span>,
}
impl Targets {
    /// Anotação usada nos erros compartilhados e no construtor comum às duas macros.
    pub(crate) fn primary(self) -> Origin {
        match self.json {
            Some(span) => Origin {
                span,
                kind: MacroKind::JsonCodable,
            },
            None => Origin {
                span: self.data.expect("alvo com ao menos uma anotação"),
                kind: MacroKind::DataClass,
            },
        }
    }
    /// Conta aplicações de macro; uma classe pode receber as duas simultaneamente.
    pub(crate) fn count(self) -> usize {
        usize::from(self.json.is_some()) + usize::from(self.data.is_some())
    }
}

/// Torna anulável um dos seis tipos escalares aceitos; formas já anuláveis não mudam.
pub(crate) fn nullable(ty: Type) -> Type {
    match ty {
        Type::Int => Type::NullableInt,
        Type::Bool => Type::NullableBool,
        Type::String => Type::NullableString,
        other => other,
    }
}
/// Escreve um dos seis tipos escalares aceitos como fonte Dart.
///
/// A validação restringe os campos a esse conjunto; o ramo final existe apenas
/// para manter a função total e nunca é alcançado por uma classe validada.
pub(crate) fn type_text(ty: Type) -> &'static str {
    match ty {
        Type::Int => "int",
        Type::NullableInt => "int?",
        Type::Bool => "bool",
        Type::NullableBool => "bool?",
        Type::String => "String",
        Type::NullableString => "String?",
        _ => "Object?",
    }
}

/// Localiza a anotação de uma macro e rejeita aplicações repetidas.
fn single(class: &Class<'_>, kind: MacroKind) -> Result<Option<Span>, Diagnostic> {
    let mut found = class.annotations.iter().filter(|a| {
        matches!(
            (&a.kind, kind),
            (AnnotationKind::JsonCodable, MacroKind::JsonCodable)
                | (AnnotationKind::DataClass, MacroKind::DataClass)
        )
    });
    let Some(annotation) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(Diagnostic::new(
            format!("Duplicate {} application", kind.name()),
            annotation.span,
        ));
    }
    Ok(Some(annotation.span))
}
/// Rejeita qualquer membro já declarado com um dos nomes que a macro reserva.
fn collision(
    class: &Class<'_>,
    origin: Origin,
    names: &[&str],
    listed: &str,
) -> Result<(), Diagnostic> {
    let taken = |name: &str| names.contains(&name);
    if class.fields.iter().any(|f| taken(f.name))
        || class
            .methods
            .iter()
            .chain(&class.abstract_methods)
            .chain(&class.factories)
            .any(|f| taken(f.name))
    {
        return Err(Diagnostic::new(
            format!(
                "{} conflicts with existing {listed} member",
                origin.kind.name()
            ),
            origin.span,
        ));
    }
    Ok(())
}

/// Valida toda a elegibilidade antes de oferecer um plano ao cache.
///
/// Corpos alheios à macro não participam do plano, mas colisões são sempre
/// verificadas. `JsonCodable` e `DataClass` coexistem: exigem a mesma forma de
/// classe e o mesmo construtor, e reservam conjuntos de nomes disjuntos. Cada
/// conjunto é conferido contra a anotação que o pediu, de modo que o diagnóstico
/// aponta a macro responsável.
/// # Erros
/// Retorna diagnóstico na anotação para qualquer contrato de geração inválido.
pub(crate) fn validate_class(class: &Class<'_>) -> Result<Option<Targets>, Diagnostic> {
    let targets = Targets {
        json: single(class, MacroKind::JsonCodable)?,
        data: single(class, MacroKind::DataClass)?,
    };
    if targets.count() == 0 {
        return Ok(None);
    }
    let primary = targets.primary();
    let name = primary.kind.name();
    if class.kind != ClassKind::Class
        || class.is_abstract
        || class.superclass.is_some()
        || !class.mixins.is_empty()
        || !class.interfaces.is_empty()
        || !class.enum_values.is_empty()
    {
        return Err(Diagnostic::new(
            format!(
                "{name} currently requires a concrete class without inheritance, mixins or interfaces"
            ),
            primary.span,
        ));
    }
    if class.fields.iter().any(|f| {
        !matches!(
            f.ty,
            Type::Int
                | Type::Bool
                | Type::String
                | Type::NullableInt
                | Type::NullableBool
                | Type::NullableString
        ) || f.initializer.is_some()
    }) {
        return Err(Diagnostic::new(
            format!(
                "{name} requires scalar fields without initializers (int/bool/String, optionally nullable)"
            ),
            primary.span,
        ));
    }
    if let Some(span) = targets.json {
        collision(
            class,
            Origin {
                span,
                kind: MacroKind::JsonCodable,
            },
            &JSON_MEMBERS,
            "toJson/fromJson",
        )?;
    }
    if let Some(span) = targets.data {
        collision(
            class,
            Origin {
                span,
                kind: MacroKind::DataClass,
            },
            &data_class::MEMBERS,
            "copyWith/igualA/descrever",
        )?;
    }
    if let Some(constructor) = &class.constructor
        && (!constructor.body.is_empty()
            || constructor.parameters.len() != class.fields.len()
            || !constructor
                .parameters
                .iter()
                .zip(&class.fields)
                .all(|(p, f)| p.field == Some(f.name) && p.ty == f.ty))
    {
        return Err(Diagnostic::new(
            format!(
                "{name} requires an empty constructor initializing every field in declaration order"
            ),
            primary.span,
        ));
    }

    Ok(Some(targets))
}
/// Copia o esquema de uma classe já validada para um plano independente da fonte.
pub(crate) fn plan_from(class: &Class<'_>, targets: Targets) -> MacroPlan {
    MacroPlan {
        fields: class
            .fields
            .iter()
            .map(|f| PlannedField {
                name: f.name.to_owned(),
                ty: f.ty,
                is_final: f.is_final,
            })
            .collect(),
        generate_constructor: class.constructor.is_none(),
        json_codable: targets.json.is_some(),
        data_class: targets.data.is_some(),
    }
}
/// Constrói um plano owned após validar a declaração; sessões podem reutilizar outro plano igual.
///
/// ```
/// let source = "@DataClass() class Ponto{final int x;}void main(){}";
/// let tokens = dartforge_lexer::lex(source).unwrap();
/// let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
/// let plan = dartforge_macros::plan_class(&program.classes[0]).unwrap().unwrap();
/// assert!(plan.data_class && !plan.json_codable);
/// assert_eq!(plan.fields[0].name, "x");
/// ```
/// # Erros
/// Retorna os mesmos diagnósticos de elegibilidade da expansão completa.
pub fn plan_class(class: &Class<'_>) -> Result<Option<MacroPlan>, Diagnostic> {
    Ok(validate_class(class)?.map(|targets| plan_from(class, targets)))
}

/// Gera o construtor posicional comum às duas macros, com initializing formals.
fn reserve_constructor(
    class: &mut Class<'_>,
    plan: &MacroPlan,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    let parameters = class
        .fields
        .iter()
        .zip(&plan.fields)
        .map(|(f, planned)| {
            Ok(ConstructorParameter::required(
                f.name,
                planned.ty,
                Some(f.name),
                report.span(origin)?,
            ))
        })
        .collect::<Result<_, Diagnostic>>()?;
    class.constructor = Some(Constructor {
        parameters,
        body: vec![],
        span: report.span(origin)?,
    });
    report.declared(1);
    Ok(())
}

/// Reserva as assinaturas de `JsonCodable` sem gerar nenhum corpo de método.
fn reserve_json(
    class: &mut Class<'_>,
    map_type: Type,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(usize, usize), Diagnostic> {
    let class_id = class.id;
    let factory = class.factories.len();
    let method = class.methods.len();
    class.factories.push(Function {
        is_arrow: false,
        is_async: false,
        annotations: vec![],
        native_binding: None,
        type_parameters: vec![],
        is_getter: false,
        name: "fromJson",
        return_type: Type::Class(class_id),
        parameters: vec![Parameter::required("json", map_type, report.span(origin)?)],
        body: vec![],
        span: report.span(origin)?,
    });
    class.methods.push(Function {
        is_arrow: false,
        is_async: false,
        annotations: vec![],
        native_binding: None,
        type_parameters: vec![],
        is_getter: false,
        name: "toJson",
        return_type: map_type,
        parameters: vec![],
        body: vec![],
        span: report.span(origin)?,
    });
    report.declared(2);
    Ok((factory, method))
}

/// Preenche definições somente após todas as assinaturas de todas as classes existirem.
fn define_json(
    class: &mut Class<'_>,
    plan: &MacroPlan,
    factory: usize,
    method: usize,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    let class_id = class.id;
    let mut arguments = Vec::with_capacity(class.fields.len());
    let mut entries = Vec::with_capacity(class.fields.len());
    for (field, planned) in class.fields.iter().zip(&plan.fields) {
        let receiver = report.expr(ExprKind::Identifier("json"), origin)?;
        let index = report.expr(ExprKind::OwnedString(planned.name.clone()), origin)?;
        let operand = report.expr(
            ExprKind::Index {
                receiver: Box::new(receiver),
                index: Box::new(index),
            },
            origin,
        )?;
        arguments.push(report.expr(
            ExprKind::Cast {
                operand: Box::new(operand),
                ty: planned.ty,
            },
            origin,
        )?);
        let key = report.expr(ExprKind::OwnedString(planned.name.clone()), origin)?;
        let this = report.expr(ExprKind::This, origin)?;
        let value = report.expr(
            ExprKind::Member {
                receiver: Box::new(this),
                name: field.name,
            },
            origin,
        )?;
        entries.push((key, value));
    }
    let value = report.expr(
        ExprKind::Construct {
            class_id,
            arguments,
        },
        origin,
    )?;
    class.factories[factory].body = report.ret(value, origin)?;
    let value = report.expr(
        ExprKind::Map {
            key_type: Some(Type::String),
            value_type: Some(Type::NullableObject),
            entries,
        },
        origin,
    )?;
    class.methods[method].body = report.ret(value, origin)?;
    Ok(())
}

/// Expande atomicamente por barreiras globais Types, Declarations e Definitions.
/// Types não gera declarações nestas macros; nenhum código Dart arbitrário é executado.
/// # Erros
/// Falhas de validação ou reserva deixam a AST original completamente intacta.
pub fn expand(program: &mut Program<'_>, source_len: usize) -> Result<ExpansionReport, Diagnostic> {
    MacroSession::with_limits(0, 0).expand(program, source_len)
}

impl MacroSession {
    /// Reutiliza somente planos próprios e materializa a expansão no contexto atual.
    /// # Erros
    /// Falhas preservam a AST original; planos válidos já consultados podem permanecer no cache.
    pub fn expand(
        &mut self,
        program: &mut Program<'_>,
        source_len: usize,
    ) -> Result<ExpansionReport, Diagnostic> {
        expand_cached(program, source_len, self)
    }
}

/// Membros reservados por uma classe durante a barreira Declarations.
struct Reserved {
    json: Option<(usize, usize)>,
    data: Option<data_class::Reserved>,
}

/// Aplica as barreiras globais e observa o cache sem reutilizar nós ou proveniência.
fn expand_cached(
    program: &mut Program<'_>,
    source_len: usize,
    session: &mut MacroSession,
) -> Result<ExpansionReport, Diagnostic> {
    let before = session.stats();
    let mut report = ExpansionReport {
        extent: source_len,
        ..Default::default()
    };
    if !program.classes.iter().any(|c| {
        c.annotations.iter().any(|a| {
            matches!(
                a.kind,
                AnnotationKind::JsonCodable | AnnotationKind::DataClass
            )
        })
    }) {
        return Ok(report);
    }
    // Types: nenhuma ação para as macros incorporadas. A barreira existe antes de Declarations.
    // Declarations: valida todos os alvos antes de reservar a primeira assinatura.
    let mut plans = Vec::new();
    for (index, class) in program.classes.iter().enumerate() {
        if let Some(targets) = validate_class(class)? {
            let plan = session.intern_validated(
                class,
                PlanShape {
                    generate_constructor: class.constructor.is_none(),
                    json_codable: targets.json.is_some(),
                    data_class: targets.data.is_some(),
                },
            );
            plans.push((index, targets, plan));
        }
    }
    // Apenas JsonCodable precisa da forma Map<String, Object?> na tabela de tipos.
    let needs_map = plans.iter().any(|(_, targets, _)| targets.json.is_some());
    let shape = TypeShape::Map {
        key: Type::String,
        value: Type::NullableObject,
    };
    let existing = program.types.iter().position(|s| *s == shape);
    let map_id = existing.unwrap_or(program.types.len());
    let map_type = Type::Applied(
        u32::try_from(map_id)
            .map_err(|_| Diagnostic::new("Too many generated types", plans[0].1.primary().span))?,
    );
    let mut pending = Vec::with_capacity(plans.len());
    for (index, targets, plan) in plans {
        // Apenas classes aumentadas são copiadas; corpos alheios não entram na transação.
        let mut class = program.classes[index].clone();
        if plan.generate_constructor {
            reserve_constructor(&mut class, &plan, targets.primary(), &mut report)?;
        }
        let json = match targets.json {
            Some(span) => Some(reserve_json(
                &mut class,
                map_type,
                Origin {
                    span,
                    kind: MacroKind::JsonCodable,
                },
                &mut report,
            )?),
            None => None,
        };
        let data = match targets.data {
            Some(span) => Some(data_class::reserve(
                &mut class,
                &plan,
                Origin {
                    span,
                    kind: MacroKind::DataClass,
                },
                &mut report,
            )?),
            None => None,
        };
        report.phases[1].applications += targets.count();
        pending.push((index, class, targets, Reserved { json, data }, plan));
    }
    // Definitions: todas as assinaturas já estão disponíveis, sem expansão intercalada.
    for (_, class, targets, reserved, plan) in &mut pending {
        if let (Some(span), Some((factory, method))) = (targets.json, reserved.json) {
            define_json(
                class,
                plan,
                factory,
                method,
                Origin {
                    span,
                    kind: MacroKind::JsonCodable,
                },
                &mut report,
            )?;
        }
        if let (Some(span), Some(members)) = (targets.data, reserved.data.as_ref()) {
            data_class::define(
                class,
                plan,
                members,
                Origin {
                    span,
                    kind: MacroKind::DataClass,
                },
                &mut report,
            )?;
        }
        class.annotations.retain(|a| {
            !matches!(
                a.kind,
                AnnotationKind::JsonCodable | AnnotationKind::DataClass
            )
        });
        report.phases[2].applications += targets.count();
        report.applications += targets.count();
    }
    // Publica somente depois de todas as reservas e definições terem sucesso.
    if needs_map && existing.is_none() {
        program.types.push(shape);
    }
    for (index, class, ..) in pending {
        program.classes[index] = class;
    }
    let after = session.stats();
    report.plan_hits = after.hits.saturating_sub(before.hits);
    report.plan_misses = after.misses.saturating_sub(before.misses);
    report.plan_evictions = after.evictions.saturating_sub(before.evictions);
    report.materialized_nodes = report.origins.len();
    Ok(report)
}
