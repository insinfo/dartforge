//! O `ConstantVerifier` do analyzer 6.11
//! (`src/dart/constant/constant_verifier.dart`): percorre as unidades de
//! uma biblioteca e relata os erros de constantes — variáveis `const`,
//! campos `final` de classes com construtor `const`, criações e coleções
//! constantes, registros, valores padrão, padrões constantes e relacionais,
//! inicializadores de construtores `const`.

use super::avaliador::{Constante, Ctx, Invalid, Motor};
use super::exhaustiveness::{Caso, Entrada, Exhaustiveness};
use super::valor::{Estado, Valor};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::{codigos::compile_time_error as c, codigos::warning as w, Codigo, Diagnostic, Span};
use dartforge_elements::model::{ClassId, LibraryId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{
    self, CollectionElement, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind, StmtId, StmtKind,
};
use std::collections::HashMap;

/// Os códigos mais específicos que o padrão (`_reportError`): relatados
/// como estão, no lugar do código padrão do ponto de uso.
const SPECIFIC: &[Codigo] = &[
    c::CONST_EVAL_EXTENSION_METHOD,
    c::CONST_EVAL_EXTENSION_TYPE_METHOD,
    c::CONST_EVAL_FOR_ELEMENT,
    c::CONST_EVAL_METHOD_INVOCATION,
    c::CONST_EVAL_PROPERTY_ACCESS,
    c::CONST_EVAL_THROWS_EXCEPTION,
    c::CONST_EVAL_THROWS_IDBZE,
    c::CONST_EVAL_TYPE_BOOL_NUM_STRING,
    c::CONST_EVAL_TYPE_BOOL,
    c::CONST_EVAL_TYPE_BOOL_INT,
    c::CONST_EVAL_TYPE_INT,
    c::CONST_EVAL_TYPE_NUM,
    c::CONST_EVAL_TYPE_NUM_STRING,
    c::CONST_EVAL_TYPE_STRING,
    c::RECURSIVE_COMPILE_TIME_CONSTANT,
    c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH,
    c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH,
    c::CONST_TYPE_PARAMETER,
    c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF,
    c::CONST_SPREAD_EXPECTED_LIST_OR_SET,
    c::CONST_SPREAD_EXPECTED_MAP,
    c::EXPRESSION_IN_MAP,
    c::VARIABLE_TYPE_MISMATCH,
    c::NON_BOOL_CONDITION,
    c::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY,
    c::SET_ELEMENT_FROM_DEFERRED_LIBRARY,
    c::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY,
    c::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    c::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY,
    c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY,
    c::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY,
    c::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
    c::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION,
];

/// Os códigos cuja verificação é deste módulo (o que a inferência emite
/// deles é descartado por quem mede a paridade).
pub const CODIGOS: &[&str] = &[
    "const_initialized_with_non_constant_value",
    "const_eval_throws_exception",
    "const_with_non_constant_argument",
    "equal_elements_in_const_set",
    "equal_keys_in_const_map",
];

struct Verifier<'m, 'a> {
    m: &'m mut Motor<'a>,
    lib: LibraryId,
    unidade: UnitId,
    saida: Vec<(UnitId, Diagnostic)>,
    /// `(unidade, membro, índice)` → variável; `(unidade, decl, índice)` → variável.
    campos: HashMap<(UnitId, ast::MemberId, usize), VariableId>,
    topo: HashMap<(UnitId, ast::DeclId, usize), VariableId>,
    /// A classe (declaração) de cada membro.
    member_class: HashMap<(UnitId, ast::MemberId), ClassId>,
    patterns_enabled: bool,
    /// Valores dos padrões constantes e das chaves de padrões de mapa da
    /// unidade (`_withConstantPatternValues`), para a exaustividade.
    the_default_values: HashMap<PatternId, Valor>,
    key_values: HashMap<ExprId, Valor>,
    /// `inConstantExpression`: dentro de um valor padrão de parâmetro ou do
    /// inicializador de um campo de instância de uma classe com construtor
    /// gerador `const` (expressão constante sem contexto constante).
    in_constant_expression: bool,
}

/// Os erros que o `_ConstantAnalysisErrorListener` do linter do analyzer
/// 3.6.2 conta como "erro de constante" (`analyzer/lib/src/lint/linter.dart`),
/// pelo nome único.
const CONSTANT_ERRORS: &[&str] = &[
    "CompileTimeErrorCode.CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY",
    "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST",
    "CompileTimeErrorCode.CONST_EVAL_EXTENSION_METHOD",
    "CompileTimeErrorCode.CONST_EVAL_EXTENSION_TYPE_METHOD",
    "CompileTimeErrorCode.CONST_EVAL_METHOD_INVOCATION",
    "CompileTimeErrorCode.CONST_EVAL_PROPERTY_ACCESS",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_INT",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_NUM_STRING",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_INT",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM_STRING",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_STRING",
    "CompileTimeErrorCode.CONST_EVAL_THROWS_EXCEPTION",
    "CompileTimeErrorCode.CONST_EVAL_THROWS_IDBZE",
    "CompileTimeErrorCode.CONST_EVAL_FOR_ELEMENT",
    "CompileTimeErrorCode.CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY",
    "CompileTimeErrorCode.CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY",
    "CompileTimeErrorCode.CONST_TYPE_PARAMETER",
    "CompileTimeErrorCode.CONST_WITH_NON_CONST",
    "CompileTimeErrorCode.CONST_WITH_NON_CONSTANT_ARGUMENT",
    "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS",
    "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF",
    "CompileTimeErrorCode.INVALID_CONSTANT",
    "CompileTimeErrorCode.MISSING_CONST_IN_LIST_LITERAL",
    "CompileTimeErrorCode.MISSING_CONST_IN_MAP_LITERAL",
    "CompileTimeErrorCode.MISSING_CONST_IN_SET_LITERAL",
    "CompileTimeErrorCode.NON_BOOL_CONDITION",
    "CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_ELEMENT",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_KEY",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE",
    "CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD",
    "CompileTimeErrorCode.NON_CONSTANT_SET_ELEMENT",
];

/// `canBeConst` de uma criação de instância (`_canBeConstInstanceCreation`,
/// `analyzer/lib/src/lint/linter.dart`): o construtor resolvido é `const`
/// e o `ConstantVerifier` sobre a criação, como se ela tivesse `const`,
/// não relata nenhum dos `ERROS_DE_CONSTANTE`. A criação constante aqui
/// é o mesmo `visitInstanceCreationExpression` da verificação de sempre
/// (`criacao_constante`), com os argumentos visitados em contexto
/// constante.
pub fn creation_can_be_const(m: &mut Motor<'_>, lib: LibraryId, unit_id: UnitId, e: ExprId) -> bool {
    let program = m.program;
    let a = m.ast(unit_id);
    let args_list = match &a.expr(e).kind {
        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
        _ => return false,
    };
    let Some(Resolved::Constructor(f)) = m.resolvido(unit_id, e) else { return false };
    if !program.function(*f).const_ {
        return false;
    }
    let patterns_enabled = program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verifier {
        m,
        lib,
        unidade: unit_id,
        saida: Vec::new(),
        campos: HashMap::new(),
        topo: HashMap::new(),
        member_class: HashMap::new(),
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    };
    v.constant_creation(a, e, args_list);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unico)))
}

/// Um verificador sem as tabelas da biblioteca (as consultas de lint).
fn standalone_verifier<'v, 'm>(m: &'v mut Motor<'m>, lib: LibraryId, unit_id: UnitId) -> Verifier<'v, 'm> {
    let patterns_enabled = m.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    Verifier {
        m,
        lib,
        unidade: unit_id,
        saida: Vec::new(),
        campos: HashMap::new(),
        topo: HashMap::new(),
        member_class: HashMap::new(),
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    }
}

/// `canBeConst` de um literal tipado (`_canBeConstTypedLiteral`): o
/// `ConstantVerifier` sobre o literal com `const` não relata nenhum dos
/// `ERROS_DE_CONSTANTE`.
pub fn literal_can_be_const(m: &mut Motor<'_>, lib: LibraryId, unit_id: UnitId, e: ExprId) -> bool {
    let a = m.ast(unit_id);
    let mut v = standalone_verifier(m, lib, unit_id);
    v.expr(a, e, true);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unico)))
}

/// `ConstructorDeclaration.canBeConst` (`analyzer/lib/src/lint/linter.dart`):
/// a classe não tem campo de instância que não é `final`, e o
/// `ConstantVerifier` sobre o construtor marcado `const` não relata nenhum
/// dos `ERROS_DE_CONSTANTE`: inicializadores potencialmente constantes,
/// campos de instância com inicializador constante e valores padrão
/// constantes.
pub fn constructor_can_be_const(m: &mut Motor<'_>, lib: LibraryId, unit_id: UnitId, mid: ast::MemberId) -> bool {
    let a = m.ast(unit_id);
    let MemberKind::Constructor(k) = &a.member(mid).kind else { return false };
    let mut v = standalone_verifier(m, lib, unit_id);
    let Some(cls) = v.class_of(a, mid) else { return false };
    let program = v.m.program;
    // `hasNonFinalField`.
    let non_final = program.class(cls).fields.iter().any(|f| {
        let x = program.variable(*f);
        !x.static_ && !x.final_ && !x.const_
    });
    if non_final {
        return false;
    }
    let cx = Ctx { lexical: None, ..v.cx() };
    for init in k.initializers.iter() {
        let exprs: Vec<ExprId> = match init {
            ast::Initializer::Field { value, .. } => vec![*value],
            ast::Initializer::Assert { condition, message, .. } => std::iter::once(*condition).chain(*message).collect(),
            ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => arguments.args.iter().map(|x| x.value).collect(),
        };
        for x in exprs {
            let mut nodes = Vec::new();
            super::potential::collect_into(v.m, &cx, x, true, false, &mut nodes);
            if !nodes.is_empty() {
                return false;
            }
        }
    }
    if !k.factory {
        for fm in v.membros_da_classe(a, cls) {
            let MemberKind::Field(l) = &a.member(fm).kind else { continue };
            if l.static_ {
                continue;
            }
            for var in l.variables.iter() {
                let Some(init) = var.initializer else { continue };
                let cx = v.cx();
                if matches!(v.m.avaliar(&cx, init, l.const_), Constante::Invalid(_)) {
                    return false;
                }
            }
        }
    }
    v.default_values(a, &k.parameters);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unico)))
}

/// Os erros de constantes das unidades de `lib`.
pub fn verificar(m: &mut Motor<'_>, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    // Os ciclos saem do grafo de dependências, antes de qualquer avaliação
    // (`computeConstants`).
    super::cycles::compute(m, lib);
    let program = m.program;
    let mut the_fields = HashMap::new();
    let mut top = HashMap::new();
    let mut member_class = HashMap::new();
    for (i, v) in program.variables.iter().enumerate() {
        if v.library != lib {
            continue;
        }
        match v.node {
            VariableRef::Field { unit, member, index } => {
                the_fields.insert((unit, member, index), VariableId(i as u32));
                if let Some(k) = v.class {
                    member_class.insert((unit, member), k);
                }
            }
            VariableRef::TopLevel { unit, decl, index } => {
                top.insert((unit, decl, index), VariableId(i as u32));
            }
            _ => {}
        }
    }
    let patterns_enabled = program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verifier {
        m,
        lib,
        unidade: UnitId(0),
        saida: Vec::new(),
        campos: the_fields,
        topo: top,
        member_class,
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    };
    for &u in &program.library(lib).units {
        if program.unit(u).role == dartforge_elements::model::UnitRole::Patch {
            continue;
        }
        v.unidade = u;
        v.the_default_values.clear();
        v.key_values.clear();
        let a = v.m.ast(u);
        for &d in &program.unit(u).unit.declarations {
            v.declaracao(a, d);
        }
        // `visitAnnotation`: os argumentos da anotação que cria com um
        // construtor `const` (`_validateConstantArguments`).
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, &program.unit(u).unit) {
            v.anotacao(m);
        }
    }
    v.saida
}

impl Verifier<'_, '_> {
    /// `const C.nome()` com `C` aplicação de mixin (`class C = S with M;`):
    /// o construtor encaminhado é `const` só se o da superclasse for e
    /// nenhum mixin declarar campo (`ConstructorElementImpl.isConst` dos
    /// encaminhados, `analyzer/lib/src/dart/element/element.dart:612-614`);
    /// não sendo, `CONST_WITH_NON_CONST` na palavra `const`.
    fn non_const_forwarded(&mut self, ty: ast::TypeId, mut the_constructor: Option<ast::Name>, span: Span) {
        let program = self.m.program;
        let u = self.unidade;
        let class = match self.annotation_type(ty).map(|t| self.m.table.get(t).clone()) {
            Some(Type::Interface { class, .. }) => class,
            _ => {
                // O tipo da criação não fica registrado: o nome escrito.
                let ast::TypeKind::Named { name, .. } = &program.unit(u).ast.ty(ty).kind else { return };
                // `C.nome` sai do parser como nome de duas partes (`p.C` ou
                // `C.nome`): a primeira parte classe é `C`, com o construtor.
                let (el, ctor) = match &name[..] {
                    [n] => (program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter), the_constructor),
                    [a1, b1] => match program.lookup_na_unidade(u, a1.sym).and_then(|b| b.getter) {
                        Some(el @ dartforge_elements::model::Element::Class(_)) => (Some(el), Some(*b1)),
                        _ => (program.lookup_prefixed_na_unidade(u, a1.sym, b1.sym).and_then(|b| b.getter), the_constructor),
                    },
                    [p, c, n] => (program.lookup_prefixed_na_unidade(u, p.sym, c.sym).and_then(|b| b.getter), Some(*n)),
                    _ => (None, None),
                };
                the_constructor = ctor;
                match el {
                    Some(dartforge_elements::model::Element::Class(c)) => c,
                    _ => return,
                }
            }
        };
        let class = &class;
        if program.class(*class).kind != dartforge_elements::model::ClassKind::MixinApplication {
            return;
        }
        let interner = self.m.interner;
        let the_key = match the_constructor {
            Some(n) if interner.resolve(n.sym) != "new" => Some(n.sym),
            _ => interner.lookup(""),
        };
        let Some(the_key) = the_key else { return };
        let Some(e_const) = self.const_forwarded_constructor(*class, the_key, 0) else { return };
        if !e_const {
            self.relatar(c::CONST_WITH_NON_CONST, Span { start: span.start, end: span.start + 5 }, Vec::new());
        }
    }

    /// `isConst` do construtor `chave` visto pela classe `c` (encaminhado nas
    /// aplicações de mixin); `None` sem construtor.
    fn const_forwarded_constructor(&self, c: ClassId, the_key: dartforge_intern::SymbolId, prof: u32) -> Option<bool> {
        if prof > 32 {
            return None;
        }
        let program = self.m.program;
        let k = program.class(c);
        if k.kind != dartforge_elements::model::ClassKind::MixinApplication {
            let f = *k.constructors.get(&the_key)?;
            let f = program.publico(f);
            if program.function(f).factory {
                return None;
            }
            return Some(program.function(f).const_);
        }
        let s = k.supertype_class?;
        let do_super = self.const_forwarded_constructor(s, the_key, prof + 1)?;
        let mixin_with_field = k.mixin_classes.iter().any(|&m| !program.class(m).fields.is_empty());
        Some(do_super && !mixin_with_field)
    }

    /// `ConstantVerifier.visitAnnotation`: o elemento é um construtor
    /// `const` com lista de argumentos; cada argumento é avaliado, e o que
    /// não é constante é `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    fn anotacao(&mut self, m: &ast::Annotation) {
        let Some(args) = &m.arguments else { return };
        let program = self.m.program;
        let interner = self.m.interner;
        let Some(el) = crate::anotacoes::annotation_element(program, interner, self.unidade, m) else { return };
        let cls = match el {
            dartforge_elements::model::Element::Class(c) => c,
            dartforge_elements::model::Element::Typedef(td) => match self.m.table.get(self.m.outline.typedefs[td.0 as usize].target_type) {
                Type::Interface { class, .. } => *class,
                _ => return,
            },
            _ => return,
        };
        // O construtor: o último nome, se for um construtor da classe; senão
        // o sem nome.
        let last = m.name.last().map(|n| n.sym);
        let named_one = (m.name.len() >= 2)
            .then_some(last)
            .flatten()
            .filter(|&s| interner.resolve(s) != "new" && interner.resolve(s) != interner.resolve(program.class(cls).name))
            .and_then(|s| program.class(cls).constructors.get(&s).copied());
        let f = named_one.or_else(|| interner.lookup("").and_then(|k| program.class(cls).constructors.get(&k).copied()));
        let Some(f) = f else { return };
        if !program.function(program.publico(f)).const_ {
            return;
        }
        for x in args.args.iter() {
            self.evaluate_and_report(x.value, true, c::CONST_WITH_NON_CONSTANT_ARGUMENT);
        }
    }

    fn cx(&self) -> Ctx {
        Ctx::simples(self.unidade, self.lib)
    }

    fn relatar(&mut self, code: Codigo, span: Span, args: Vec<String>) {
        self.saida.push((self.unidade, Diagnostic::com_codigo(code, span, args)));
    }

    /// O erro com o código dele e as `contextMessages` (no arquivo de cada
    /// uma, quando não é o desta unidade).
    fn report_with_context(&mut self, i: &Invalid) {
        let mut d = Diagnostic::com_codigo(i.codigo, i.span, i.args.clone());
        for (u, span, text) in &i.contexto {
            let file = (*u != self.unidade).then(|| self.m.program.caminho_da_unidade(*u).into());
            d.contexto.push(dartforge_diagnostics::Contexto { arquivo: file, span: *span, mensagem: text.as_str().into() });
        }
        self.saida.push((self.unidade, d));
    }

    /// `_reportError`: o código específico leva as mensagens de contexto; o
    /// padrão, não.
    fn report_invalid(&mut self, i: &Invalid, the_pattern: Option<Codigo>) {
        if i.avoid_report || i.unidade != self.unidade {
            return;
        }
        if SPECIFIC.contains(&i.codigo) {
            self.report_with_context(i);
        } else if let Some(p) = the_pattern {
            self.relatar(p, i.span, Vec::new());
        }
    }

    /// `_evaluateAndReportError`.
    fn evaluate_and_report(&mut self, e: ExprId, in_const: bool, the_pattern: Codigo) -> Constante {
        let cx = self.cx();
        let r = self.m.avaliar(&cx, e, in_const);
        if let Constante::Invalid(i) = &r {
            let i = (**i).clone();
            self.report_invalid(&i, Some(the_pattern));
        }
        r
    }

    // -- Declarações -----------------------------------------------------------

    fn declaracao(&mut self, a: &ast::Ast, d: ast::DeclId) {
        let decl = a.decl(d);
        match &decl.kind {
            DeclKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    if let Some(init) = var.initializer {
                        self.expr(a, init, l.const_);
                        if l.const_ {
                            if let Some(&v) = self.topo.get(&(self.unidade, d, i)) {
                                self.variable_result(v, true);
                            }
                        }
                    }
                }
            }
            DeclKind::Function(f) => self.funcao(a, *f),
            DeclKind::Class(k) => self.membros(a, &k.members),
            DeclKind::Mixin(k) => self.membros(a, &k.members),
            DeclKind::Enum(k) => {
                let unit_id = self.unidade;
                let cls = self.m.program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == unit_id && r.decl == d));
                for (i, cst) in k.constants.iter().enumerate() {
                    if let Some(args) = &cst.arguments {
                        for arg in args.args.iter() {
                            self.expr(a, arg.value, true);
                        }
                        // `visitEnumConstantDeclaration`: `_validateConstantArguments`.
                        for arg in args.args.iter() {
                            self.evaluate_and_report(arg.value, true, c::CONST_WITH_NON_CONSTANT_ARGUMENT);
                        }
                    }
                    // `visitEnumConstantDeclaration` (`constant_verifier.dart:204-217`):
                    // o `evaluationResult` inválido da constante, sem código
                    // padrão (a exceção de avaliação vira
                    // `CONST_EVAL_THROWS_EXCEPTION` na constante).
                    if let Some(ci) = cls
                        && let Some(&v) = self.m.program.classes[ci].enum_constants.get(i)
                    {
                        self.variable_result(v, false);
                    }
                }
                self.membros(a, &k.members);
            }
            DeclKind::Extension(k) => self.membros(a, &k.members),
            DeclKind::ExtensionType(k) => self.membros(a, &k.members),
            DeclKind::Typedef(_) => {}
        }
    }

    fn variable_result(&mut self, v: VariableId, const_: bool) {
        if let Some(Constante::Invalid(i)) = self.m.valor_de_variavel(v) {
            let i = (*i).clone();
            let the_pattern = if const_ { Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE) } else { None };
            self.report_invalid(&i, the_pattern);
        }
    }

    fn membros(&mut self, a: &ast::Ast, members: &[ast::MemberId]) {
        for &mid in members {
            let mem = a.member(mid);
            match &mem.kind {
                MemberKind::Field(l) => {
                    for (i, var) in l.variables.iter().enumerate() {
                        let Some(init) = var.initializer else { continue };
                        let const_class = !l.static_
                            && self.member_class.get(&(self.unidade, mid)).copied().is_some_and(|k| {
                                self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Class && self.has_const_generative_constructor(k)
                            });
                        let before = std::mem::replace(&mut self.in_constant_expression, const_class);
                        self.expr(a, init, l.const_);
                        self.in_constant_expression = before;
                        if !(l.const_ || l.final_) {
                            continue;
                        }
                        // Classe com construtor primário (3.13.4): o campo cujo
                        // inicializador não é potencialmente constante fica sem
                        // valor avaliado (o erro dele é o do campo,
                        // `CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST`,
                        // no `const`); o erro da avaliação sai só do
                        // potencialmente constante (medido no oráculo 3.13.4,
                        // `primary_constructors/const/potentially_constant_error_test`).
                        if !l.static_ && Self::class_member_with_primary(a, mid) {
                            let cx = self.cx();
                            let mut nodes = Vec::new();
                            super::potential::collect_into(self.m, &cx, init, true, false, &mut nodes);
                            if !nodes.is_empty() {
                                continue;
                            }
                        }
                        let Some(&v) = self.campos.get(&(self.unidade, mid, i)) else { continue };
                        if !l.static_ {
                            let cls = self.member_class.get(&(self.unidade, mid)).copied();
                            let has = cls.is_some_and(|k| self.has_const_generative_constructor(k));
                            let is_class = cls.is_some_and(|k| self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Class);
                            if is_class && !has {
                                continue;
                            }
                            if !has {
                                continue;
                            }
                        } else if !l.const_ {
                            continue;
                        }
                        self.variable_result(v, l.const_);
                    }
                }
                MemberKind::Method(f) => self.funcao(a, *f),
                MemberKind::Constructor(k) => self.construtor(a, mid, k),
            }
        }
    }

    /// O membro `mid` é de uma classe com construtor primário. O enum fica de
    /// fora: as constantes dele (`e(1)`) avaliam o construtor, e o erro do
    /// inicializador do campo sai dessa avaliação (o `CONST_EVAL_METHOD_INVOCATION`
    /// do `final int x = fn(p)` de `enum const E(int p)` no oráculo 3.13.4).
    fn class_member_with_primary(a: &ast::Ast, mid: ast::MemberId) -> bool {
        a.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(x) => x.primary_constructor.is_some() && x.members.contains(&mid),
            _ => false,
        })
    }

    fn has_const_generative_constructor(&self, k: ClassId) -> bool {
        self.m
            .program
            .class(k)
            .constructors
            .values()
            .any(|f| self.m.program.function(*f).const_ && !self.m.program.function(*f).factory)
    }

    /// `visitConstructorDeclaration`.
    /// As regras do `ErrorVerifier` sobre o cabeçalho de um construtor
    /// (docs/ANALYZER-ESPECIFICACAO.md §D): `non_const_generative_enum_constructor`
    /// (`error_verifier.dart:4710-4719`); e, para o construtor gerador
    /// `const`, nesta ordem e parando no primeiro relato,
    /// `const_constructor_with_mixin_with_field` (`:2807-2856`),
    /// `const_constructor_with_non_const_super` (`:2858-2893`) e
    /// `const_constructor_with_non_final_field` (`:2898-2916`); mais
    /// `const_constructor_throws_exception` (`:2936-2943`) em cada `throw` dos
    /// inicializadores. Escrito sem compilar nem executar (2026-10-04).
    /// O construtor é o primário (3.13) da classe ou do enum, ou a parte de
    /// corpo dele.
    fn is_primary(&self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) -> bool {
        k.parte_primaria
            || a.decls.iter().any(|d| match &d.kind {
                ast::DeclKind::Class(x) => x.primary_constructor == Some(mid),
                ast::DeclKind::Enum(x) => x.primary_constructor == Some(mid),
                _ => false,
            })
    }

    fn constructor_rules(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        use dartforge_elements::model::ClassKind;
        let Some(cls) = self.class_of(a, mid) else { return };
        let program = self.m.program;
        let interner = self.m.interner;
        let data = program.class(cls);
        // `atConstructorDeclaration`: do nome da classe ao fim do nome do
        // construtor, quando há.
        let header = Span { start: k.class_name.span.start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) };
        // Só no `visitConstructorDeclaration`: a parte `this` que sobra (repetida
        // ou sem cabeçalho primário) é um `PrimaryConstructorBody`, não uma
        // declaração de construtor (3.13.4).
        if data.kind == ClassKind::Enum && !k.const_ && !k.factory && !k.parte_primaria {
            self.relatar(c::NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR, header, Vec::new());
        }
        if !k.const_ || k.factory {
            return;
        }
        for init in k.initializers.iter() {
            let sp = match init {
                ast::Initializer::Field { span, .. }
                | ast::Initializer::Super { span, .. }
                | ast::Initializer::Redirect { span, .. }
                | ast::Initializer::Assert { span, .. } => *span,
            };
            let throws: Vec<Span> = a
                .exprs
                .iter()
                .filter(|x| matches!(x.kind, ExprKind::Throw(_)) && x.span.start >= sp.start && x.span.end <= sp.end)
                .map(|x| x.span)
                .collect();
            for s in throws {
                self.relatar(c::CONST_CONSTRUCTOR_THROWS_EXCEPTION, s, Vec::new());
            }
        }
        // Campos de instância dos mixins da cláusula `with`: todo campo que
        // não é estático nem (`abstract` e `final`).
        let mut mixin_fields: Vec<String> = Vec::new();
        for &mx in data.mixin_classes.iter() {
            for &v in program.class(mx).fields.iter() {
                let var = program.variable(v);
                if var.static_ {
                    continue;
                }
                let the_abstract_ = match var.node {
                    VariableRef::Field { unit, member, .. } => {
                        matches!(&program.unit(unit).ast.member(member).kind, MemberKind::Field(l) if l.abstract_)
                    }
                    _ => false,
                };
                if the_abstract_ && var.final_ {
                    continue;
                }
                mixin_fields.push(format!("'{}.{}'", interner.resolve(program.class(mx).name), interner.resolve(var.name)));
            }
        }
        // `implicitErrorRange`: no construtor primário (3.13) é o `errorRange`
        // do `PrimaryConstructorDeclaration` (`ast.dart:45420-45424`), do
        // primeiro token (o `const`, quando escrito) ao nome do construtor, ou
        // só o primeiro token.
        let primary = self.is_primary(a, mid, k);
        let range = if primary {
            let source = &program.unit(self.unidade).source;
            let start = if k.const_ {
                let before = source[..k.class_name.span.start].trim_end();
                if before.ends_with("const") { before.len() - "const".len() } else { k.class_name.span.start }
            } else {
                k.class_name.span.start
            };
            let end = match k.name {
                Some(n) => n.span.end,
                None if start < k.class_name.span.start => start + "const".len(),
                None => k.class_name.span.end,
            };
            Span { start: start, end: end }
        } else {
            k.class_name.span
        };
        if mixin_fields.len() == 1 {
            self.relatar(c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD, range, mixin_fields);
            return;
        }
        if mixin_fields.len() > 1 {
            self.relatar(c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS, range, vec![mixin_fields.join(", ")]);
            return;
        }
        // O construtor da superclasse que este invoca (o escrito, ou o sem
        // nome implícito) não é `const`. Um enum sempre chama um `const`; o
        // redirecionador é conferido em outro lugar.
        let redirects = k.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. }));
        if data.kind != ClassKind::Enum && !redirects {
            let written = k.initializers.iter().find_map(|i| match i {
                ast::Initializer::Super { span, constructor, .. } => Some((*span, constructor.map(|n| n.sym))),
                _ => None,
            });
            if let Some(sup) = super::cycles::declared_superclass(program, cls) {
                if Some(sup) != self.m.core.object_class {
                    let super_name = written.and_then(|(_, n)| n);
                    let the_key = super_name.or_else(|| interner.lookup(""));
                    let tgt = the_key.and_then(|ch| program.class(sup).constructors.get(&ch).copied());
                    let non_const = match tgt {
                        Some(g) => !program.function(program.publico(g)).const_,
                        // Superclasse sem construtor declarado: o padrão
                        // implícito, que não é `const`.
                        None => program.class(sup).constructors.is_empty() && super_name.is_none(),
                    };
                    if non_const {
                        // Sem `super(…)` escrito, o `implicitErrorRange`: no
                        // primário, a faixa do cabeçalho (`class const B()`:
                        // o `const`).
                        let implicit = if primary { range } else { k.class_name.span };
                        let span = written.map_or(implicit, |(s, _)| s);
                        // No 3.6.2 o argumento é a classe DO CONSTRUTOR; no
                        // 3.13.4, a superclasse (o segundo argumento, que só
                        // a variante 3.13 do código usa: T2, caso c04).
                        let the_name = interner.resolve(data.name).to_string();
                        let superior = interner.resolve(program.class(sup).name).to_string();
                        self.relatar(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, span, vec![the_name, superior]);
                        return;
                    }
                }
            }
        }
        // `hasNonFinalField`: a classe, os mixins e as superclasses, em
        // largura — algum campo de instância que não é `final` nem `const`.
        // No 3.13.4 (`link.dart:284-323`, `_computeHasNonFinalField`) só o
        // campo com armazenamento conta: o `abstract int x;` não (no 3.6.2,
        // `element.dart:329-356`, conta).
        let reference_313 = program.referencia(self.unidade) == dartforge_diagnostics::Referencia::V3_13;
        if data.kind == ClassKind::Class {
            let mut queue: Vec<ClassId> = vec![cls];
            let mut seen: Vec<ClassId> = Vec::new();
            let mut found = false;
            while let Some(x) = queue.pop() {
                if seen.contains(&x) {
                    continue;
                }
                seen.push(x);
                let dx = program.class(x);
                if dx.fields.iter().any(|&v| {
                    let var = program.variable(v);
                    let the_abstract_ = var.getter.is_some_and(|g| program.function(g).abstract_);
                    !var.final_ && !var.const_ && !var.static_ && !(reference_313 && the_abstract_)
                }) {
                    found = true;
                    break;
                }
                queue.extend(dx.mixin_classes.iter().copied());
                queue.extend(dx.supertype_class);
            }
            if found {
                // No primário, o `errorRange` do cabeçalho
                // (`class const A()`: `const`; `class const A.n()`:
                // `const A.n`).
                let span = if primary { range } else { header };
                self.relatar(c::CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD, span, Vec::new());
            }
        }
    }

    fn construtor(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        self.constructor_rules(a, mid, k);
        if k.const_ {
            // O construtor gerador de um ciclo (os de fábrica são do
            // `ErrorVerifier`): no nome da classe do cabeçalho.
            let in_cycle = self
                .m
                .grafo
                .construtor_de(self.unidade, mid)
                .is_some_and(|f| self.m.grafo.constructors_in_cycle.contains(&f));
            // O construtor primário (3.13) é um `PrimaryConstructorDeclaration`:
            // o `visitConstructorDeclaration` não passa por ele. No 3.13.4 o
            // relato vai do nome do tipo (ou do `new`) ao fim do nome
            // (`errorRange`); no 3.6.2, só o `returnType`.
            let primary = self.is_primary(a, mid, k);
            if in_cycle && !k.factory && !primary {
                let span = match k.name {
                    Some(n) if self.m.program.referencia(self.unidade) == dartforge_diagnostics::Referencia::V3_13 => {
                        Span { start: k.class_name.span.start, end: n.span.end }
                    }
                    _ => k.class_name.span,
                };
                self.relatar(c::RECURSIVE_CONSTANT_CONSTRUCTOR, span, Vec::new());
            }
            // `_validateConstructorInitializers`: potencialmente constantes.
            let cx = Ctx { lexical: None, ..self.cx() };
            for init in k.initializers.iter() {
                let exprs: Vec<ExprId> = match init {
                    ast::Initializer::Field { value, .. } => vec![*value],
                    ast::Initializer::Assert { condition, message, .. } => std::iter::once(*condition).chain(*message).collect(),
                    ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                        arguments.args.iter().map(|x| x.value).collect()
                    }
                };
                for x in exprs {
                    let mut nodes = Vec::new();
                    super::potential::collect_into(self.m, &cx, x, true, false, &mut nodes);
                    for n in nodes {
                        self.relatar(c::INVALID_CONSTANT, n, Vec::new());
                    }
                }
            }
            if !k.factory {
                let primary = self.is_primary(a, mid, k);
                self.field_initializers(a, mid, primary);
            }
        }
        self.default_values(a, &k.parameters);
        for init in k.initializers.iter() {
            match init {
                ast::Initializer::Field { value, .. } => self.expr(a, *value, false),
                ast::Initializer::Assert { condition, message, .. } => {
                    self.expr(a, *condition, false);
                    if let Some(m) = message {
                        self.expr(a, *m, false);
                    }
                }
                ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                    for x in arguments.args.iter() {
                        self.expr(a, x.value, false);
                    }
                }
            }
        }
        self.corpo(a, &k.body);
    }

    /// `_validateFieldInitializers`: campos de instância com inicializador
    /// que não é constante, relatados no `const` do construtor. Com o
    /// construtor primário (3.13.4, `_validatePrimaryFieldInitializers`), o
    /// inicializador que tem nó não potencialmente constante — os parâmetros
    /// primários valem —, sem avaliar: `final int i = d.length` não é relatado
    /// aqui (o erro da avaliação sai do campo), `final int x = fn(p)` é.
    fn field_initializers(&mut self, a: &ast::Ast, mid: ast::MemberId, primary: bool) {
        let Some(k) = self.class_of(a, mid) else { return };
        let e_enum = self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Enum;
        let Some(the_keyword) = self.const_keyword(a, mid) else { return };
        let members: Vec<ast::MemberId> = self.membros_da_classe(a, k);
        for fm in members {
            let MemberKind::Field(l) = &a.member(fm).kind else { continue };
            if l.static_ {
                continue;
            }
            for var in l.variables.iter() {
                if e_enum && self.m.interner.resolve(var.name.sym) == "values" {
                    continue;
                }
                let Some(init) = var.initializer else { continue };
                let cx = self.cx();
                let invalid = if primary {
                    let mut nodes = Vec::new();
                    super::potential::collect_into(self.m, &cx, init, true, false, &mut nodes);
                    !nodes.is_empty()
                } else {
                    matches!(self.m.avaliar(&cx, init, l.const_), Constante::Invalid(_))
                };
                if invalid {
                    let the_name = self.m.interner.resolve(var.name.sym).to_string();
                    self.relatar(c::CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST, the_keyword, vec![the_name]);
                }
            }
        }
    }

    /// A classe declarada que contém o membro `mid` (pelos construtores).
    fn class_of(&self, _a: &ast::Ast, mid: ast::MemberId) -> Option<ClassId> {
        for (i, f) in self.m.program.functions.iter().enumerate() {
            let _ = i;
            if let dartforge_elements::model::FunctionRef::Constructor { unit, member } = f.node {
                if unit == self.unidade && member == mid {
                    return f.class;
                }
            }
        }
        None
    }

    fn membros_da_classe(&self, a: &ast::Ast, k: ClassId) -> Vec<ast::MemberId> {
        let Some(d) = self.m.program.class(k).decl else { return Vec::new() };
        if d.unit != self.unidade {
            return Vec::new();
        }
        match &a.decl(d.decl).kind {
            DeclKind::Class(x) => x.members.clone(),
            DeclKind::Enum(x) => x.members.clone(),
            DeclKind::Mixin(x) => x.members.clone(),
            DeclKind::ExtensionType(x) => x.members.clone(),
            _ => Vec::new(),
        }
    }

    /// O token `const` do construtor.
    fn const_keyword(&self, a: &ast::Ast, mid: ast::MemberId) -> Option<Span> {
        let s = a.member(mid).span;
        let source = &self.m.program.unit(self.unidade).source;
        let t = source.get(s.start..s.end)?;
        let mut i = 0;
        let b = t.as_bytes();
        while i + 5 <= b.len() {
            if &b[i..i + 5] == b"const" && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) && b.get(i + 5).is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_') {
                return Some(Span { start: s.start + i, end: s.start + i + 5 });
            }
            i += 1;
        }
        None
    }

    fn funcao(&mut self, a: &ast::Ast, f: ast::FunctionId) {
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.default_values(a, ps);
        }
        self.corpo(a, &func.body);
    }

    /// `_validateDefaultValues`.
    fn default_values(&mut self, a: &ast::Ast, ps: &[ast::Parameter]) {
        for p in ps {
            let Some(d) = p.default_value else { continue };
            let before = std::mem::replace(&mut self.in_constant_expression, true);
            self.expr(a, d, false);
            self.in_constant_expression = before;
            // `defaultValue.typeOrThrow is InvalidType` (3.6.2
            // `constant_verifier.dart:825-826`): o erro já saiu (o nome
            // indefinido, por exemplo).
            if self.m.invalid_type(self.unidade, d) {
                continue;
            }
            let r = self.m.default_result(self.unidade, self.lib, d);
            if let Constante::Invalid(i) = r {
                self.report_invalid(&i, Some(c::NON_CONSTANT_DEFAULT_VALUE));
            }
        }
    }

    fn corpo(&mut self, a: &ast::Ast, b: &ast::FunctionBody) {
        match b {
            ast::FunctionBody::Block(s) => self.stmt(a, *s),
            ast::FunctionBody::Expression(e) => self.expr(a, *e, false),
            _ => {}
        }
    }

    // -- Comandos --------------------------------------------------------------

    fn stmt(&mut self, a: &ast::Ast, s: StmtId) {
        match &a.stmt(s).kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(a, *x);
                }
            }
            StmtKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    if let Some(init) = var.initializer {
                        self.expr(a, init, l.const_);
                        if l.const_ {
                            let r = self.m.declared_local_value(self.unidade, self.lib, s, i);
                            if let Constante::Invalid(x) = r {
                                self.report_invalid(&x, Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE));
                            }
                        }
                    }
                }
            }
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(a, *value, false);
                self.padrao(a, *pattern);
            }
            StmtKind::Function(f) => self.funcao(a, *f),
            StmtKind::Expression(e) => self.expr(a, *e, false),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(a, *condition, false);
                if let Some(p) = case_pattern {
                    self.padrao(a, *p);
                }
                if let Some(g) = guard {
                    self.expr(a, *g, false);
                }
                self.stmt(a, *then);
                if let Some(e) = else_ {
                    self.stmt(a, *e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                match init {
                    Some(ast::ForInit::Variables(l)) => {
                        for var in l.variables.iter() {
                            if let Some(i) = var.initializer {
                                self.expr(a, i, l.const_);
                            }
                        }
                    }
                    Some(ast::ForInit::Expression(e)) => self.expr(a, *e, false),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(a, *value, false);
                        self.padrao(a, *pattern);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(a, *c, false);
                }
                for u in updates.iter() {
                    self.expr(a, *u, false);
                }
                self.stmt(a, *body);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                if let ast::ForInTarget::Pattern { pattern, .. } = target {
                    self.padrao(a, *pattern);
                }
                self.expr(a, *iterable, false);
                self.stmt(a, *body);
            }
            StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
                self.expr(a, *condition, false);
                self.stmt(a, *body);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(a, *value, false);
                for case in cases.iter() {
                    if let Some(p) = case.pattern {
                        if self.patterns_enabled {
                            self.padrao(a, p);
                        } else if let PatternKind::Variable { name, .. } = &a.pattern(p).kind
                            && self.m.body.units[self.unidade.0 as usize].non_constant_name_patterns.contains(&p)
                        {
                            // O `case nome:` antigo (a expressão do `SwitchCase`).
                            self.relatar(c::NON_CONSTANT_CASE_EXPRESSION, name.span, Vec::new());
                        } else if let PatternKind::Constant(e) = &a.pattern(p).kind {
                            // `_validateSwitchStatement_nullSafety`.
                            let e = unparenthesize(a, *e);
                            self.expr(a, e, true);
                            // Sem padrões, o valor precisa de igualdade
                            // primitiva: `CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS`
                            // com o tipo do valor (`constant_verifier.dart:1009-1030`).
                            if let Constante::Valor(v) = self.evaluate_and_report(e, true, c::NON_CONSTANT_CASE_EXPRESSION)
                                && !v.actually_unknown()
                                && !self.m.primitive_equality(&v, self.lib)
                            {
                                let sp = self.m.span(self.unidade, e);
                                let typ = self.m.formatar(v.tipo);
                                self.relatar(c::CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS, sp, vec![typ]);
                            }
                        }
                    }
                    if let Some(g) = case.guard {
                        self.expr(a, g, false);
                    }
                    for x in case.body.iter() {
                        self.stmt(a, *x);
                    }
                }
                if self.patterns_enabled {
                    let the_cases: Vec<Caso> = cases.iter().map(|k| Caso { padrao: k.pattern, stored: k.guard.is_some() }).collect();
                    let start = a.stmt(s).span.start;
                    let pos: Vec<usize> = cases.iter().map(|k| self.case_keyword_start(k)).collect();
                    self.exhaustiveness(a, *value, start, &the_cases, &pos, false);
                }
            }
            StmtKind::Return(Some(e)) | StmtKind::Yield { value: e, .. } => self.expr(a, *e, false),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(a, *body);
                for c in catches.iter() {
                    self.stmt(a, c.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(a, *f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(a, *body),
            StmtKind::Assert { condition, message } => {
                self.expr(a, *condition, false);
                if let Some(m) = message {
                    self.expr(a, *m, false);
                }
            }
            _ => {}
        }
    }

    // -- Padrões ---------------------------------------------------------------

    /// `extensionTypeErasure`.
    fn erase_extension(&mut self, t: TypeId) -> TypeId {
        let outline = self.m.outline;
        let program = self.m.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut crate::table::TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, self.m.table, &rep)
    }

    /// `_canBeEqual(constantType, valueType)`
    /// (an611:src/dart/constant/constant_verifier.dart:517-546). `Null` é
    /// `InterfaceType` no analyzer (`NullTypeImpl`).
    fn can_be_equal(&mut self, c: TypeId, v: TypeId, prof: u32) -> bool {
        if prof > 16 {
            return true;
        }
        let interface = |t: &Type| matches!(t, Type::Interface { .. } | Type::Null);
        let tc = self.m.table.get(c).clone();
        if !interface(&tc) {
            return true;
        }
        let tv = self.m.table.get(v).clone();
        match tv {
            Type::Interface { .. } | Type::Null => {
                let c_int = matches!(tc, Type::Interface { class, .. } if Some(class) == self.m.core.int_class);
                let v_double = matches!(tv, Type::Interface { class, .. } if Some(class) == self.m.core.double_class);
                if c_int && v_double {
                    return true;
                }
                let greater = self.greatest_closure_of(v, true, &[]);
                let mut env = crate::subtyping::SubtypeEnv::new(&mut *self.m.table, &self.m.outline.hierarchy, self.m.core);
                crate::subtyping::is_subtype(c, greater, &mut env)
            }
            Type::TypeParameter { param, nullable } => {
                let data = self.m.table.param(param).clone();
                if !data.explicito || parameter_reference(self.m.table, data.bound) {
                    return true;
                }
                let b = if nullable { crate::ops::nullable(data.bound, self.m.table) } else { data.bound };
                self.can_be_equal(c, b, prof + 1)
            }
            // `promotedBound`: o limite promovido (a anulabilidade é a do
            // parâmetro, sem `?`).
            Type::Intersection { bound, .. } => {
                if parameter_reference(self.m.table, bound) {
                    return true;
                }
                self.can_be_equal(c, bound, prof + 1)
            }
            Type::Function { .. } => {
                if matches!(tc, Type::Null) {
                    self.m.table.get(v).is_declared_nullable()
                } else {
                    false
                }
            }
            _ => true,
        }
    }

    /// `PatternGreatestClosureHelper.eliminateToGreatest`: cada parâmetro de
    /// tipo vira `Object?` nas posições covariantes e `Never` nas
    /// contravariantes (com a anulabilidade unida); os formais do próprio
    /// tipo de função ficam.
    fn greatest_closure_of(&mut self, t: TypeId, co: bool, formals: &[crate::table::TypeParamId]) -> TypeId {
        let the_table = &mut *self.m.table;
        match the_table.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } if !formals.contains(&param) => {
                let is_nullable = matches!(the_table.get(t), Type::TypeParameter { nullable: true, .. });
                if co {
                    self.m.core.object_nullable
                } else if is_nullable {
                    self.m.core.null
                } else {
                    self.m.core.never
                }
            }
            Type::Interface { class, args, nullable } if !args.is_empty() => {
                let new_: Vec<TypeId> = args.iter().map(|&x| self.greatest_closure_of(x, co, formals)).collect();
                self.m.table.intern(Type::Interface { class, args: new_.into_boxed_slice(), nullable })
            }
            Type::ExtensionType { decl, args, nullable } if !args.is_empty() => {
                let new_: Vec<TypeId> = args.iter().map(|&x| self.greatest_closure_of(x, co, formals)).collect();
                self.m.table.intern(Type::ExtensionType { decl, args: new_.into_boxed_slice(), nullable })
            }
            Type::FutureOr { arg, nullable } => {
                let arg = self.greatest_closure_of(arg, co, formals);
                self.m.table.intern(Type::FutureOr { arg, nullable })
            }
            Type::Record { positional, named, nullable } => {
                let pos: Vec<TypeId> = positional.iter().map(|&x| self.greatest_closure_of(x, co, formals)).collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId)> = named.iter().map(|&(n, x)| (n, self.greatest_closure_of(x, co, formals))).collect();
                self.m.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable })
            }
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let mut inside: Vec<crate::table::TypeParamId> = formals.to_vec();
                inside.extend(type_params.iter().copied());
                let ret = self.greatest_closure_of(ret, co, &inside);
                let pos: Vec<TypeId> = positional.iter().map(|&x| self.greatest_closure_of(x, !co, &inside)).collect();
                let opt: Vec<TypeId> = optional.iter().map(|&x| self.greatest_closure_of(x, !co, &inside)).collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|&(n, x, r)| (n, self.greatest_closure_of(x, !co, &inside), r)).collect();
                self.m.table.intern(Type::Function {
                    type_params,
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: nm.into_boxed_slice(),
                    nullable,
                })
            }
            _ => t,
        }
    }

    fn padrao(&mut self, a: &ast::Ast, p: PatternId) {
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => {
                // `const (List<T>)`: o literal de tipo logo dentro dos
                // parênteses do padrão constante
                // (`isTypeLiteralInConstantPattern`, 3.6.2
                // `evaluation.dart:1066-1072`, `:3197-3201`) que menciona
                // parâmetro de tipo é `CONST_TYPE_PARAMETER` no tipo.
                if let ExprKind::Parenthesized(x) = a.expr(*e).kind
                    && let ExprKind::TypeArguments { target, type_args } = &a.expr(x).kind
                    && matches!(
                        self.m.body.units[self.unidade.0 as usize].get_resolved(*target),
                        Some(crate::resolved::Resolved::Element(dartforge_elements::model::Element::Class(_) | dartforge_elements::model::Element::Typedef(_)))
                    )
                    && type_args.iter().any(|&t| self.annotation_type(t).is_some_and(|r| self.m.mentions_parameter(r)))
                {
                    self.relatar(c::CONST_TYPE_PARAMETER, a.expr(x).span, Vec::new());
                    return;
                }
                let e = unparenthesize(a, *e);
                // `expression.typeOrThrow is InvalidType` (3.6.2
                // `constant_verifier.dart:133-135`).
                if self.m.invalid_type(self.unidade, e) {
                    return;
                }
                let r = self.evaluate_and_report(e, false, c::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION);
                if let Constante::Valor(v) = r {
                    // `CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE`
                    // (an611:src/dart/constant/constant_verifier.dart:141-158):
                    // valor de igualdade primitiva cujo tipo não pode ser
                    // igual a um valor do tipo casado (sem os tipos de
                    // extensão).
                    if self.patterns_enabled
                        && self.m.primitive_equality(&v, self.lib)
                        && let Some(&matched) = self.m.body.units[self.unidade.0 as usize].tipos_casados.get(&p)
                    {
                        let matched = self.erase_extension(matched);
                        if !self.can_be_equal(v.tipo, matched, 0) {
                            let displayer = crate::exibicao::Exibidor { table: &*self.m.table, interner: self.m.interner, program: self.m.program };
                            let (texts, context) = displayer.arguments_and_context(&[crate::exibicao::Arg::Tipo(matched), crate::exibicao::Arg::Tipo(v.tipo)]);
                            let mut d = Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE, a.pattern(p).span, texts);
                            d.contexto.extend(context);
                            self.saida.push((self.unidade, d));
                            self.the_default_values.insert(p, v);
                            return;
                        }
                    }
                    self.the_default_values.insert(p, v);
                    self.expr(a, e, false);
                }
            }
            // `case nome:` (o parser guarda como variável): a constante de
            // topo com esse nome, para a exaustividade.
            PatternKind::Variable { final_: false, var_: false, ty: None, name } if self.patterns_enabled => {
                if self.m.body.units[self.unidade.0 as usize].non_constant_name_patterns.contains(&p) {
                    self.relatar(c::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION, name.span, Vec::new());
                    return;
                }
                let program = self.m.program;
                if let Some(dartforge_elements::model::Element::Variable(v)) =
                    program.lookup_na_unidade(self.unidade, name.sym).and_then(|b| b.getter)
                {
                    if let Some(Constante::Valor(val)) = self.m.valor_de_variavel(v) {
                        self.the_default_values.insert(p, val);
                    }
                }
            }
            PatternKind::Relational { value, .. } => {
                self.expr(a, *value, false);
                self.evaluate_and_report(*value, false, c::NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION);
            }
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(a, *x);
                self.padrao(a, *y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.padrao(a, *x),
            PatternKind::Cast { pattern, .. } => self.padrao(a, *pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.padrao(a, *x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                // `visitMapPattern` (`constant_verifier.dart:311-358`): cada
                // chave avaliada; uma igual (idêntica, ou `==` com igualdade
                // primitiva) a uma anterior é `EQUAL_KEYS_IN_MAP_PATTERN`, na
                // chave repetida, depois de todas. O `isIdentical` do record
                // nunca é verdadeiro (`RecordState.isIdentical`: falso ou
                // desconhecido), então dois records só são iguais com a
                // igualdade primitiva dos dois — um campo com `==` próprio
                // tira a repetição.
                let mut unique: Vec<(Valor, Span)> = Vec::new();
                let mut the_repeated: Vec<(Span, Span)> = Vec::new();
                for en in entries.iter() {
                    self.padrao(a, en.value);
                    if let Constante::Valor(v) = self.evaluate_and_report(en.key, false, c::NON_CONSTANT_MAP_PATTERN_KEY) {
                        let known = !(v.actually_unknown() || matches!(v.estado, Estado::Null { invalid: true }));
                        if known {
                            let span = a.expr(en.key).span;
                            let lib = self.lib;
                            let record = |x: &Valor| matches!(x.estado, Estado::Registro { .. });
                            let primitive_v = !record(&v) || self.m.primitive_equality(&v, lib);
                            let m = &mut *self.m;
                            let the_found = unique.iter().find(|(x, _)| (!record(x) || (primitive_v && m.primitive_equality(x, lib))) && m.iguais(x, &v));
                            match the_found {
                                Some(&(_, original)) => the_repeated.push((span, original)),
                                None => unique.push((v.clone(), span)),
                            }
                        }
                        self.key_values.insert(en.key, v);
                    }
                }
                // `equalKeysInMapPattern`: o contexto é a primeira chave.
                for (sp, original) in the_repeated {
                    let d = Diagnostic::com_codigo(c::EQUAL_KEYS_IN_MAP_PATTERN, sp, Vec::<String>::new()).com_contexto(original, "The first key with this value.");
                    self.saida.push((self.unidade, d));
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(a, f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressões ------------------------------------------------------------

    fn expr(&mut self, a: &ast::Ast, e: ExprId, in_const: bool) {
        let u = self.unidade;
        match &a.expr(e).kind {
            ExprKind::InstanceCreation { keyword, arguments, ty, constructor } => {
                let const_ = matches!(keyword, Some(ast::CreationKeyword::Const));
                // `_checkForConstWithNonConst` com o construtor encaminhado de
                // uma aplicação de mixin (a inferência não o resolve).
                if const_ && !matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    self.non_const_forwarded(*ty, *constructor, a.expr(e).span);
                }
                // `visitInstanceCreationExpression` (`isConst`: `const`, ou
                // sem palavra num contexto constante).
                if const_ || (keyword.is_none() && in_const) {
                    self.without_type_parameters(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                if const_ {
                    self.constant_creation(a, e, arguments);
                } else {
                    for x in arguments.args.iter() {
                        self.expr(a, x.value, in_const);
                    }
                }
            }
            ExprKind::Call { target, arguments } => {
                if in_const && matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    // O `NamedType` da criação sem `new`: os argumentos de
                    // tipo da chamada ou os do `C<T>` antes de `.nome`.
                    let mut written_ones: Vec<ast::TypeId> = arguments.type_args.to_vec();
                    if let ExprKind::Property { target: r, .. } = &a.expr(*target).kind
                        && let ExprKind::TypeArguments { type_args, .. } = &a.expr(*r).kind
                    {
                        written_ones.extend(type_args.iter().copied());
                    }
                    for x in written_ones {
                        self.without_type_parameters(a, x, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                    }
                    self.constant_creation(a, e, arguments);
                    return;
                }
                self.expr(a, *target, in_const);
                for x in arguments.args.iter() {
                    self.expr(a, x.value, in_const);
                }
            }
            ExprKind::List { const_, elements, type_args } => {
                let c = *const_ || in_const;
                for el in elements.iter() {
                    self.child_element(a, el, c);
                }
                if c {
                    for &x in type_args.iter() {
                        self.const_type_argument(a, x, c::INVALID_TYPE_ARGUMENT_IN_CONST_LIST);
                    }
                }
                if c {
                    let typ = self.m.estatico(u, e);
                    let element = match self.m.table.get(typ) {
                        Type::Interface { args, .. } if args.len() == 1 => args[0],
                        _ => self.m.core.dynamic_,
                    };
                    let mut lv = Literal { tipo: LiteralType::Lista(element), the_unique: Vec::new(), duplicados: Vec::new() };
                    for el in elements.iter() {
                        self.verify_element(a, &mut lv, el);
                    }
                }
            }
            ExprKind::SetOrMap { const_, elements, type_args } => {
                let c = *const_ || in_const;
                for el in elements.iter() {
                    self.child_element(a, el, c);
                }
                if c {
                    let typ = self.m.estatico(u, e);
                    let (cls, args) = match self.m.table.get(typ) {
                        Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                        _ => (None, Vec::new()),
                    };
                    // `TypeArgumentsVerifier.checkMapLiteral`/`checkSetLiteral`:
                    // só com a forma decidida.
                    let code = if cls.is_some() && cls == self.m.core.map_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_MAP)
                    } else if cls.is_some() && cls == self.m.core.set_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_SET)
                    } else {
                        None
                    };
                    if let Some(code) = code {
                        for &x in type_args.iter() {
                            self.const_type_argument(a, x, code);
                        }
                    }
                    let tl = if cls.is_some() && cls == self.m.core.set_class && args.len() == 1 {
                        Some(LiteralType::Conjunto(args[0]))
                    } else if cls.is_some() && cls == self.m.core.map_class && args.len() == 2 {
                        Some(LiteralType::Mapa(args[0], args[1]))
                    } else {
                        None
                    };
                    if let Some(tl) = tl {
                        let mut lv = Literal { tipo: tl, the_unique: Vec::new(), duplicados: Vec::new() };
                        for el in elements.iter() {
                            self.verify_element(a, &mut lv, el);
                        }
                        // `equalKeysInConstMap`/`equalElementsInConstSet`: o
                        // contexto é o primeiro com o mesmo valor.
                        let (code, context) = if matches!(lv.tipo, LiteralType::Mapa(..)) {
                            (c::EQUAL_KEYS_IN_CONST_MAP, "The first key with this value.")
                        } else {
                            (c::EQUAL_ELEMENTS_IN_CONST_SET, "The first element with this value.")
                        };
                        for (dup, original) in std::mem::take(&mut lv.duplicados) {
                            let d = Diagnostic::com_codigo(code, dup, Vec::<String>::new()).com_contexto(original, context);
                            self.saida.push((self.unidade, d));
                        }
                    }
                } else {
                    self.duplicates_in_non_constant_literal(a, e, elements);
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let c = *const_ || in_const;
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(a, *x, c);
                }
                if *const_ {
                    for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                        self.evaluate_and_report(*x, true, c::NON_CONSTANT_RECORD_FIELD);
                    }
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(a, *f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(a, *x, in_const);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(a, *x, in_const),
            ExprKind::Property { target, name, .. } => {
                // `visitConstructorReference`: `C<T>.nome` de um construtor.
                if (in_const || self.in_constant_expression)
                    && let ExprKind::TypeArguments { target: base, type_args } = &a.expr(*target).kind
                    && self.referenced_constructor(a, *base, *name)
                {
                    for &x in type_args.iter() {
                        self.without_type_parameters(a, x, c::CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF, &[]);
                    }
                    self.expr(a, *base, in_const);
                    return;
                }
                self.expr(a, *target, in_const)
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(a, *target, in_const);
                self.expr(a, *index, in_const);
            }
            ExprKind::TypeArguments { target, type_args } => {
                // `visitFunctionReference`: a instanciação de uma função (não
                // o literal de tipo, de tipo `Type`).
                if (in_const || self.in_constant_expression) && self.m.estatico(u, e) != self.m.core.type_ {
                    for &x in type_args.iter() {
                        self.without_type_parameters(a, x, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF, &[]);
                    }
                }
                self.expr(a, *target, in_const)
            }
            ExprKind::Unary { operand, .. } => self.expr(a, *operand, in_const),
            ExprKind::Binary { left, right, .. } => {
                self.expr(a, *left, in_const);
                self.expr(a, *right, in_const);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(a, *condition, in_const);
                self.expr(a, *then, in_const);
                self.expr(a, *else_, in_const);
            }
            ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
                // `visitGenericFunctionType` sob `is`/`as` num contexto
                // constante.
                if in_const && matches!(a.ty(*ty).kind, ast::TypeKind::Function { .. }) {
                    self.without_type_parameters(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                self.expr(a, *value, in_const)
            }
            ExprKind::Assign { target, value, .. } => {
                self.expr(a, *target, in_const);
                self.expr(a, *value, in_const);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(a, *pattern);
                self.expr(a, *value, in_const);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(a, *target, in_const);
                for s in sections.iter() {
                    self.expr(a, *s, in_const);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(a, *value, in_const);
                for case in cases.iter() {
                    self.padrao(a, case.pattern);
                    if let Some(g) = case.guard {
                        self.expr(a, g, false);
                    }
                    self.expr(a, case.body, in_const);
                }
                let the_cases: Vec<Caso> = cases.iter().map(|k| Caso { padrao: Some(k.pattern), stored: k.guard.is_some() }).collect();
                let start = a.expr(e).span.start;
                let pos: Vec<usize> = cases.iter().map(|k| self.case_arrow(a, k)).collect();
                self.exhaustiveness(a, *value, start, &the_cases, &pos, true);
            }
            _ => {}
        }
    }

    // -- Exaustividade (`_validateSwitchExhaustiveness`, `:883-1000`) ----------

    /// Relata `UNREACHABLE_SWITCH_CASE` (na palavra `case` ou na seta),
    /// `NON_EXHAUSTIVE_SWITCH_*` (na palavra `switch`) e
    /// `UNREACHABLE_SWITCH_DEFAULT` (na palavra `default`). `pos` é o
    /// início do token de cada caso.
    fn exhaustiveness(&mut self, a: &ast::Ast, the_value: ExprId, switch_start: usize, the_cases: &[Caso], pos: &[usize], expression: bool) {
        let u = self.unidade;
        let body = self.m.body;
        let the_body = &body.units[u.0 as usize];
        if the_body.invalid_types.contains(&the_value) {
            return;
        }
        let Some(t) = the_body.get_type(the_value) else { return };
        let program = self.m.program;
        let source = program.unit(u).source.as_str();
        let version_3_3 = program.library(self.lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 3);
        let the_default_values = std::mem::take(&mut self.the_default_values);
        let key_values = std::mem::take(&mut self.key_values);
        let entry = Entrada {
            ast: a,
            source: source,
            tipos_de_padroes: &the_body.tipos_de_padroes,
            extension_fields: &the_body.extension_fields,
            invalid_patterns: &the_body.invalid_patterns,
            the_default_values: &the_default_values,
            key_values: &key_values,
            version_3_3,
        };
        let collect = self.m.witnesses.is_some();
        let mut ex = Exhaustiveness::nova(self.m, self.lib);
        ex.collect_parts = collect;
        let must = expression || ex.always_exhaustive(t, 0);
        let r = ex.verificar(&entry, t, the_cases);
        drop(ex);
        self.the_default_values = the_default_values;
        self.key_values = key_values;
        let Some(r) = r else { return };
        let mut parts = r.partes;
        for i in r.unreachable {
            let n = if expression { 2 } else { 4 };
            self.relatar(w::UNREACHABLE_SWITCH_CASE, Span { start: pos[i], end: pos[i] + n }, Vec::new());
        }
        let default = the_cases.iter().position(|k| k.padrao.is_none());
        match (r.witness, r.correcao) {
            (Some(tw), Some(co)) => {
                if must && default.is_none() {
                    let code = if expression { c::NON_EXHAUSTIVE_SWITCH_EXPRESSION } else { c::NON_EXHAUSTIVE_SWITCH_STATEMENT };
                    let typ = self.m.formatar(t);
                    // O quarto argumento só existe para a variante "privado"
                    // do 3.13.4 (`constant_verifier.dart` do main, caso c29
                    // de `corpus/especificacao/t2/v`): o valor é de um enum
                    // de outra biblioteca e o que falta é privado. O
                    // original olha todas as testemunhas; aqui, a primeira.
                    let private = match self.m.table.get(t) {
                        Type::Interface { class, .. } => {
                            let k = self.m.program.class(*class);
                            k.kind == dartforge_elements::model::ClassKind::Enum
                                && k.library != self.lib
                                && (self.m.interner.resolve(k.name).starts_with('_') || tw.contains("._"))
                        }
                        _ => false,
                    };
                    let mut args = vec![typ, tw, co];
                    if private {
                        args.push(String::new());
                    }
                    self.relatar(code, Span { start: switch_start, end: switch_start + 6 }, args);
                    let u = self.unidade;
                    if let Some(map) = self.m.witnesses.as_mut() {
                        map.insert((u, switch_start), std::mem::take(&mut parts));
                    }
                }
            }
            _ => {
                if let Some(d) = default {
                    if must {
                        self.relatar(w::UNREACHABLE_SWITCH_DEFAULT, Span { start: pos[d], end: pos[d] + 7 }, Vec::new());
                    }
                }
            }
        }
    }

    /// O início da palavra `case`/`default` de um caso de `switch` (depois
    /// dos rótulos).
    fn case_keyword_start(&self, k: &ast::SwitchCase) -> usize {
        let source = self.m.program.unit(self.unidade).source.as_bytes();
        let mut i = k.span.start;
        if let Some(l) = k.labels.last() {
            i = pular_brancos(source, l.span.end);
            if source.get(i) == Some(&b':') {
                i += 1;
            }
            i = pular_brancos(source, i);
        }
        i
    }

    /// O início da seta `=>` de um caso de expressão `switch`.
    fn case_arrow(&self, a: &ast::Ast, k: &ast::SwitchExprCase) -> usize {
        let source = self.m.program.unit(self.unidade).source.as_bytes();
        let end = match k.guard {
            Some(g) => a.expr(g).span.end,
            None => a.pattern(k.pattern).span.end,
        };
        let bnd = a.expr(k.body).span.start;
        let mut i = end;
        while i + 1 < source.len() && i < bnd && !(source[i] == b'=' && source[i + 1] == b'>') {
            let j = pular_brancos(source, i);
            i = if j > i { j } else { i + 1 };
        }
        i
    }

    fn child_element(&mut self, a: &ast::Ast, el: &CollectionElement, in_const: bool) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(a, *x, in_const),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(a, *key, in_const);
                self.expr(a, *value, in_const);
            }
            CollectionElement::Spread { value, .. } => self.expr(a, *value, in_const),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(a, *condition, in_const);
                if let Some(p) = case_pattern {
                    self.padrao(a, *p);
                }
                if let Some(g) = guard {
                    self.expr(a, *g, in_const);
                }
                self.child_element(a, then, in_const);
                if let Some(x) = else_ {
                    self.child_element(a, x, in_const);
                }
            }
            CollectionElement::For { condition, updates, body, .. } => {
                if let Some(c) = condition {
                    self.expr(a, *c, in_const);
                }
                for u in updates.iter() {
                    self.expr(a, *u, in_const);
                }
                self.child_element(a, body, in_const);
            }
            CollectionElement::ForIn { iterable, body, .. } => {
                self.expr(a, *iterable, in_const);
                self.child_element(a, body, in_const);
            }
        }
    }

    /// `visitInstanceCreationExpression` de uma criação constante: avalia e
    /// relata (com o relator verdadeiro: os argumentos não resolvidos
    /// também); com valor, segue nos argumentos.
    fn constant_creation(&mut self, a: &ast::Ast, e: ExprId, arguments: &ast::Arguments) {
        if !matches!(self.m.resolvido(self.unidade, e), Some(Resolved::Constructor(_))) {
            return;
        }
        let cx = Ctx { relatar: true, ..self.cx() };
        let before = self.m.relatos.len();
        let r = self.m.avaliar(&cx, e, true);
        let reports: Vec<Invalid> = self.m.relatos.drain(before..).collect();
        for i in reports {
            if i.unidade == self.unidade && !i.avoid_report {
                self.report_with_context(&i);
            }
        }
        match r {
            Constante::Invalid(i) => {
                if !i.avoid_report && i.unidade == self.unidade {
                    self.report_with_context(&i);
                }
            }
            Constante::Valor(_) => {
                for x in arguments.args.iter() {
                    self.expr(a, x.value, true);
                }
            }
        }
    }

    // -- Literais constantes (`_ConstLiteralVerifier`) -------------------------

    fn literal_code(t: &LiteralType) -> Codigo {
        match t {
            LiteralType::Lista(_) => c::NON_CONSTANT_LIST_ELEMENT,
            LiteralType::Conjunto(_) => c::NON_CONSTANT_SET_ELEMENT,
            LiteralType::Mapa(..) => c::NON_CONSTANT_MAP_ELEMENT,
        }
    }

    fn verify_element(&mut self, a: &ast::Ast, lv: &mut Literal, el: &CollectionElement) -> bool {
        let u = self.unidade;
        let code = Self::literal_code(&lv.tipo);
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                let Constante::Valor(v) = self.evaluate_and_report(*x, true, code) else { return false };
                let span = self.m.span(u, *x);
                match lv.tipo {
                    LiteralType::Lista(t) => {
                        if !self.m.casa(&v, t) {
                            let args = vec![self.m.formatar(v.tipo), self.m.formatar(t)];
                            self.relatar(c::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        true
                    }
                    LiteralType::Conjunto(t) => {
                        if !self.m.casa(&v, t) {
                            let args = vec![self.m.formatar(v.tipo), self.m.formatar(t)];
                            self.relatar(c::SET_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        if !self.m.primitive_equality(&v, self.lib) {
                            let args = vec![self.m.formatar(v.tipo)];
                            self.relatar(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, span, args);
                            return false;
                        }
                        self.record_unique(lv, v, span);
                        true
                    }
                    LiteralType::Mapa(..) => true,
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                let s = self.m.element_span(u, el);
                self.relatar(c::CONST_EVAL_FOR_ELEMENT, s, Vec::new());
                false
            }
            CollectionElement::If { condition, then, else_, case_pattern, .. } => {
                if case_pattern.is_some() {
                    return false;
                }
                let Constante::Valor(cond) = self.evaluate_and_report(*condition, true, code) else { return false };
                if !cond.estado.e_bool() {
                    return false;
                }
                match cond.as_bool() {
                    None => {
                        let a1 = self.potential_in_literal(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.potential_in_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(true) => {
                        let a1 = self.verify_element(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.potential_in_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(false) => {
                        let a1 = self.potential_in_literal(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.verify_element(a, lv, x));
                        a1 && a2
                    }
                }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let LiteralType::Mapa(tk, tv) = lv.tipo else { return false };
                let k = self.evaluate_and_report(*key, true, c::NON_CONSTANT_MAP_KEY);
                let v = self.evaluate_and_report(*value, true, c::NON_CONSTANT_MAP_VALUE);
                if let Constante::Valor(k) = k {
                    let span = self.m.span(u, *key);
                    if !self.m.casa(&k, tk) {
                        let args = vec![self.m.formatar(k.tipo), self.m.formatar(tk)];
                        self.relatar(c::MAP_KEY_TYPE_NOT_ASSIGNABLE, span, args);
                    }
                    if !self.m.primitive_equality(&k, self.lib) {
                        let args = vec![self.m.formatar(k.tipo)];
                        self.relatar(c::CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY, span, args);
                    }
                    self.record_unique(lv, k, span);
                }
                if let Constante::Valor(v) = v {
                    if !self.m.casa(&v, tv) {
                        let span = self.m.span(u, *value);
                        let args = vec![self.m.formatar(v.tipo), self.m.formatar(tv)];
                        self.relatar(c::MAP_VALUE_TYPE_NOT_ASSIGNABLE, span, args);
                    }
                }
                true
            }
            CollectionElement::Spread { value, null_aware } => {
                let Constante::Valor(v) = self.evaluate_and_report(*value, true, code) else { return false };
                let span = self.m.span(u, *value);
                match lv.tipo {
                    LiteralType::Lista(_) | LiteralType::Conjunto(_) => {
                        let list = v.as_list().cloned();
                        let set = v.as_set().cloned();
                        let Some(items) = list.clone().or(set) else {
                            if v.estado.is_null() && *null_aware {
                                return true;
                            }
                            self.relatar(c::CONST_SPREAD_EXPECTED_LIST_OR_SET, span, Vec::new());
                            return false;
                        };
                        if matches!(lv.tipo, LiteralType::Lista(_)) {
                            return true;
                        }
                        if list.is_some() && !items.iter().all(|x| self.m.primitive_equality(x, self.lib)) {
                            let s = self.m.element_span(u, el);
                            let args = vec![self.m.formatar(v.tipo)];
                            self.relatar(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, s, args);
                            return false;
                        }
                        for x in items.iter() {
                            self.record_unique(lv, x.clone(), span);
                        }
                        true
                    }
                    LiteralType::Mapa(..) => {
                        if v.estado.is_null() && *null_aware {
                            return true;
                        }
                        match v.as_map().cloned() {
                            Some(m) => {
                                for (k, _) in m.iter() {
                                    self.record_unique(lv, k.clone(), span);
                                }
                                true
                            }
                            None => {
                                self.relatar(c::CONST_SPREAD_EXPECTED_MAP, span, Vec::new());
                                false
                            }
                        }
                    }
                }
            }
        }
    }

    /// O tipo resolvido de uma anotação de tipo da unidade.
    /// `_checkForConstWithTypeParameters`
    /// (an611:src/dart/constant/constant_verifier.dart:554-605): um
    /// `NamedType` que nomeia um parâmetro de tipo de fora (no nó, sem
    /// descer), e os argumentos dele; num tipo de função, os formais dele
    /// são permitidos, e os limites, o retorno e os parâmetros simples são
    /// conferidos.
    fn without_type_parameters(&mut self, a: &ast::Ast, x: ast::TypeId, code: Codigo, allowed: &[crate::table::TypeParamId]) {
        let annotation = a.ty(x);
        match &annotation.kind {
            ast::TypeKind::Named { args, .. } => {
                let parameter = self.annotation_type(x).and_then(|r| {
                    if matches!(self.m.table.exibicao(r), Some(crate::table::Exibicao::Alias { .. })) {
                        return None;
                    }
                    match self.m.table.get(r) {
                        Type::TypeParameter { param, .. } => Some(*param),
                        _ => None,
                    }
                });
                if let Some(p) = parameter
                    && !allowed.contains(&p)
                {
                    self.relatar(code, annotation.span, Vec::new());
                    return;
                }
                for &y in args.iter() {
                    self.without_type_parameters(a, y, code, allowed);
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                let mut inside: Vec<crate::table::TypeParamId> = allowed.to_vec();
                if let Some(r) = self.annotation_type(x)
                    && let Type::Function { type_params: formals, .. } = self.m.table.get(r)
                {
                    inside.extend(formals.iter().copied());
                }
                for tp in type_params.iter() {
                    if let Some(b) = tp.bound {
                        self.without_type_parameters(a, b, code, &inside);
                    }
                }
                if let Some(r) = return_type {
                    self.without_type_parameters(a, *r, code, &inside);
                }
                for p in parameters.iter() {
                    if p.function_parameters.is_none()
                        && let Some(y) = p.ty
                    {
                        self.without_type_parameters(a, y, code, &inside);
                    }
                }
            }
            ast::TypeKind::Record { .. } | ast::TypeKind::Void => {}
        }
    }

    /// `base.nome` designa um construtor (`C.new`, `C.nome`, `p.C.nome`,
    /// pelo alias também).
    fn referenced_constructor(&self, a: &ast::Ast, base: ExprId, the_name: ast::Name) -> bool {
        let program = self.m.program;
        let u = self.unidade;
        let el = match &a.expr(base).kind {
            ExprKind::Identifier(n) => program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter),
            ExprKind::Property { target: p, name: n, .. } => match &a.expr(*p).kind {
                ExprKind::Identifier(p) => program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter),
                _ => None,
            },
            _ => None,
        };
        let cls = match el {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            Some(dartforge_elements::model::Element::Typedef(td)) => match self.m.table.get(self.m.outline.typedefs[td.0 as usize].target_type) {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
                _ => None,
            },
            _ => None,
        };
        let Some(c) = cls else { return false };
        let text = self.m.interner.resolve(the_name.sym);
        let the_key = if text == "new" { self.m.interner.lookup("") } else { Some(the_name.sym) };
        the_key.is_some_and(|k| program.class(c).constructors.contains_key(&k))
    }

    fn annotation_type(&self, x: ast::TypeId) -> Option<TypeId> {
        let u = self.unidade;
        self.m.body.units.get(u.0 as usize).and_then(|b| b.tipos_de_anotacoes.get(&x).copied()).or_else(|| self.m.outline.tipos_escritos.get(&(u, x)).copied())
    }

    /// `_checkTypeArgumentConst` (`type_arguments_verifier.dart:501-544`): um
    /// argumento de tipo de literal constante que é parâmetro de tipo, no nó
    /// do tipo; no tipo nomeado com o lexema, nos de função e registro com o
    /// tipo exibido. Desce nos argumentos, nos parâmetros simples e no
    /// retorno de um tipo de função e nos campos de um registro.
    fn const_type_argument(&mut self, a: &ast::Ast, x: ast::TypeId, code: Codigo) {
        let is_parameter = |s: &Self, t: ast::TypeId| s.annotation_type(t).filter(|r| matches!(s.m.table.get(*r), Type::TypeParameter { .. }));
        let annotation = a.ty(x);
        match &annotation.kind {
            ast::TypeKind::Named { name, args } => {
                if is_parameter(self, x).is_some() {
                    let lexeme = name.last().map(|n| self.m.interner.resolve(n.sym).to_string()).unwrap_or_default();
                    self.relatar(code, annotation.span, vec![lexeme]);
                } else {
                    for &y in args.iter() {
                        self.const_type_argument(a, y, code);
                    }
                }
            }
            ast::TypeKind::Function { return_type, parameters, .. } => {
                let mut children: Vec<ast::TypeId> = parameters.iter().filter(|p| p.function_parameters.is_none()).filter_map(|p| p.ty).collect();
                children.extend(return_type.iter().copied());
                for y in children {
                    self.const_type_field(a, y, code);
                }
            }
            ast::TypeKind::Record { positional, named } => {
                let children: Vec<ast::TypeId> = positional.iter().copied().chain(named.iter().map(|(_, t)| *t)).collect();
                for y in children {
                    self.const_type_field(a, y, code);
                }
            }
            ast::TypeKind::Void => {}
        }
    }

    /// Um parâmetro, retorno ou campo de registro: parâmetro de tipo relata
    /// com o tipo exibido; senão desce.
    fn const_type_field(&mut self, a: &ast::Ast, y: ast::TypeId, code: Codigo) {
        match self.annotation_type(y) {
            Some(r) if matches!(self.m.table.get(r), Type::TypeParameter { .. }) => {
                let displayed = self.m.formatar(r);
                self.relatar(code, a.ty(y).span, vec![displayed]);
            }
            _ => self.const_type_argument(a, y, code),
        }
    }

    /// `BestPracticesVerifier._checkForDuplications`
    /// (`best_practices_verifier.dart:851-875`): num literal não constante,
    /// os elementos (conjunto) ou as chaves (mapa) de primeiro nível que a
    /// avaliação constante dá sem erro, iguais a um anterior:
    /// `EQUAL_ELEMENTS_IN_SET`/`EQUAL_KEYS_IN_MAP` no repetido.
    fn duplicates_in_non_constant_literal(&mut self, _a: &ast::Ast, e: ExprId, elements: &[CollectionElement]) {
        let u = self.unidade;
        let typ = self.m.estatico(u, e);
        let cls = match self.m.table.get(typ) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let (map, code) = if cls.is_some() && cls == self.m.core.set_class {
            (false, w::EQUAL_ELEMENTS_IN_SET)
        } else if cls.is_some() && cls == self.m.core.map_class {
            (true, w::EQUAL_KEYS_IN_MAP)
        } else {
            return;
        };
        let expressions: Vec<ExprId> = elements
            .iter()
            .filter_map(|el| match el {
                CollectionElement::Expression(x) if !map => Some(*x),
                CollectionElement::MapEntry { key, .. } if map => Some(*key),
                _ => None,
            })
            .collect();
        let mut seen: Vec<Valor> = Vec::new();
        for x in expressions {
            let cx = self.cx();
            let Constante::Valor(v) = self.m.avaliar(&cx, x, false) else { continue };
            if v.actually_unknown() {
                continue;
            }
            let repeated = seen.iter().any(|y| self.m.iguais(y, &v));
            if repeated {
                let sp = self.m.span(u, x);
                self.relatar(code, sp, Vec::new());
            } else {
                seen.push(v);
            }
        }
    }

    fn record_unique(&mut self, lv: &mut Literal, v: Valor, span: Span) {
        if v.actually_unknown() || matches!(v.estado, Estado::Null { invalid: true }) {
            return;
        }
        for (x, s) in lv.the_unique.iter() {
            if self.m.iguais(x, &v) {
                let original = *s;
                if !lv.duplicados.iter().any(|(d, _)| *d == span) {
                    lv.duplicados.push((span, original));
                }
                return;
            }
        }
        lv.the_unique.push((v, span));
    }

    /// `_reportNotPotentialConstants` do verificador de literais.
    fn potential_in_literal(&mut self, a: &ast::Ast, lv: &Literal, el: &CollectionElement) -> bool {
        let _ = a;
        let cx = self.cx();
        let mut nodes: Vec<(Span, Codigo)> = Vec::new();
        let base = Self::literal_code(&lv.tipo);
        self.collect_element(&cx, el, base, &mut nodes);
        if nodes.is_empty() {
            return true;
        }
        for (s, code) in nodes {
            self.relatar(code, s, Vec::new());
        }
        false
    }

    fn collect_element(&mut self, cx: &Ctx, el: &CollectionElement, base: Codigo, nodes: &mut Vec<(Span, Codigo)>) {
        let map = base == c::NON_CONSTANT_MAP_ELEMENT;
        let one = |m: &Motor<'_>, x: ExprId, code: Codigo, nodes: &mut Vec<(Span, Codigo)>| {
            let mut v = Vec::new();
            super::potential::collect_into(m, cx, x, false, true, &mut v);
            nodes.extend(v.into_iter().map(|s| (s, code)));
        };
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) | CollectionElement::Spread { value: x, .. } => {
                one(self.m, *x, base, nodes)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let (ck, cv) = if map { (c::NON_CONSTANT_MAP_KEY, c::NON_CONSTANT_MAP_VALUE) } else { (base, base) };
                one(self.m, *key, ck, nodes);
                one(self.m, *value, cv, nodes);
            }
            CollectionElement::If { condition, then, else_, .. } => {
                one(self.m, *condition, base, nodes);
                self.collect_element(cx, then, base, nodes);
                if let Some(x) = else_ {
                    self.collect_element(cx, x, base, nodes);
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                nodes.push((self.m.element_span(cx.unidade, el), base));
            }
        }
    }
}

enum LiteralType {
    Lista(TypeId),
    Conjunto(TypeId),
    Mapa(TypeId, TypeId),
}

struct Literal {
    tipo: LiteralType,
    the_unique: Vec<(Valor, Span)>,
    duplicados: Vec<(Span, Span)>,
}

/// Pula espaços e comentários a partir de `i`.
fn pular_brancos(f: &[u8], mut i: usize) -> usize {
    loop {
        while i < f.len() && f[i].is_ascii_whitespace() {
            i += 1;
        }
        if f.get(i) == Some(&b'/') && f.get(i + 1) == Some(&b'/') {
            while i < f.len() && f[i] != b'\n' {
                i += 1;
            }
        } else if f.get(i) == Some(&b'/') && f.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < f.len() && !(f[i] == b'*' && f[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(f.len());
        } else {
            return i;
        }
    }
}

fn unparenthesize(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// `hasTypeParameterReference`.
fn parameter_reference(table: &crate::table::TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::TypeParameter { .. } | Type::Intersection { .. } => true,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|&a| parameter_reference(table, a)),
        Type::FutureOr { arg, .. } => parameter_reference(table, *arg),
        Type::Function { ret, positional, optional, named, .. } => {
            parameter_reference(table, *ret)
                || positional.iter().chain(optional.iter()).any(|&a| parameter_reference(table, a))
                || named.iter().any(|(_, a, _)| parameter_reference(table, *a))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|&a| parameter_reference(table, a)) || named.iter().any(|(_, a)| parameter_reference(table, *a))
        }
        _ => false,
    }
}
