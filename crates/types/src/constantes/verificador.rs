//! O `ConstantVerifier` do analyzer 6.11
//! (`src/dart/constant/constant_verifier.dart`): percorre as unidades de
//! uma biblioteca e relata os erros de constantes — variáveis `const`,
//! campos `final` de classes com construtor `const`, criações e coleções
//! constantes, registros, valores padrão, padrões constantes e relacionais,
//! inicializadores de construtores `const`.

use super::avaliador::{Constante, Ctx, Invalida, Motor};
use super::exaustividade::{Caso, Entrada, Exaustividade};
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
const ESPECIFICOS: &[Codigo] = &[
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

struct Verificador<'m, 'a> {
    m: &'m mut Motor<'a>,
    lib: LibraryId,
    unidade: UnitId,
    saida: Vec<(UnitId, Diagnostic)>,
    /// `(unidade, membro, índice)` → variável; `(unidade, decl, índice)` → variável.
    campos: HashMap<(UnitId, ast::MemberId, usize), VariableId>,
    topo: HashMap<(UnitId, ast::DeclId, usize), VariableId>,
    /// A classe (declaração) de cada membro.
    classe_de_membro: HashMap<(UnitId, ast::MemberId), ClassId>,
    padroes_ligados: bool,
    /// Valores dos padrões constantes e das chaves de padrões de mapa da
    /// unidade (`_withConstantPatternValues`), para a exaustividade.
    valores_de_padroes: HashMap<PatternId, Valor>,
    valores_de_chaves: HashMap<ExprId, Valor>,
    /// `inConstantExpression`: dentro de um valor padrão de parâmetro ou do
    /// inicializador de um campo de instância de uma classe com construtor
    /// gerador `const` (expressão constante sem contexto constante).
    em_expressao_constante: bool,
}

/// Os erros que o `_ConstantAnalysisErrorListener` do linter do analyzer
/// 3.6.2 conta como "erro de constante" (`analyzer/lib/src/lint/linter.dart`),
/// pelo nome único.
const ERROS_DE_CONSTANTE: &[&str] = &[
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
/// não relata nenhum dos [`ERROS_DE_CONSTANTE`]. A criação constante aqui
/// é o mesmo `visitInstanceCreationExpression` da verificação de sempre
/// (`criacao_constante`), com os argumentos visitados em contexto
/// constante.
pub fn criacao_pode_ser_const(m: &mut Motor<'_>, lib: LibraryId, unidade: UnitId, e: ExprId) -> bool {
    let program = m.program;
    let a = m.ast(unidade);
    let argumentos = match &a.expr(e).kind {
        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
        _ => return false,
    };
    let Some(Resolved::Constructor(f)) = m.resolvido(unidade, e) else { return false };
    if !program.function(*f).const_ {
        return false;
    }
    let padroes_ligados = program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verificador {
        m,
        lib,
        unidade,
        saida: Vec::new(),
        campos: HashMap::new(),
        topo: HashMap::new(),
        classe_de_membro: HashMap::new(),
        padroes_ligados,
        valores_de_padroes: HashMap::new(),
        valores_de_chaves: HashMap::new(),
        em_expressao_constante: false,
    };
    v.criacao_constante(a, e, argumentos);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| ERROS_DE_CONSTANTE.contains(&c.info().unico)))
}

/// Um verificador sem as tabelas da biblioteca (as consultas de lint).
fn verificador_avulso<'v, 'm>(m: &'v mut Motor<'m>, lib: LibraryId, unidade: UnitId) -> Verificador<'v, 'm> {
    let padroes_ligados = m.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    Verificador {
        m,
        lib,
        unidade,
        saida: Vec::new(),
        campos: HashMap::new(),
        topo: HashMap::new(),
        classe_de_membro: HashMap::new(),
        padroes_ligados,
        valores_de_padroes: HashMap::new(),
        valores_de_chaves: HashMap::new(),
        em_expressao_constante: false,
    }
}

/// `canBeConst` de um literal tipado (`_canBeConstTypedLiteral`): o
/// `ConstantVerifier` sobre o literal com `const` não relata nenhum dos
/// [`ERROS_DE_CONSTANTE`].
pub fn literal_pode_ser_const(m: &mut Motor<'_>, lib: LibraryId, unidade: UnitId, e: ExprId) -> bool {
    let a = m.ast(unidade);
    let mut v = verificador_avulso(m, lib, unidade);
    v.expr(a, e, true);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| ERROS_DE_CONSTANTE.contains(&c.info().unico)))
}

/// `ConstructorDeclaration.canBeConst` (`analyzer/lib/src/lint/linter.dart`):
/// a classe não tem campo de instância que não é `final`, e o
/// `ConstantVerifier` sobre o construtor marcado `const` não relata nenhum
/// dos [`ERROS_DE_CONSTANTE`]: inicializadores potencialmente constantes,
/// campos de instância com inicializador constante e valores padrão
/// constantes.
pub fn construtor_pode_ser_const(m: &mut Motor<'_>, lib: LibraryId, unidade: UnitId, mid: ast::MemberId) -> bool {
    let a = m.ast(unidade);
    let MemberKind::Constructor(k) = &a.member(mid).kind else { return false };
    let mut v = verificador_avulso(m, lib, unidade);
    let Some(classe) = v.classe_de(a, mid) else { return false };
    let program = v.m.program;
    // `hasNonFinalField`.
    let nao_final = program.class(classe).fields.iter().any(|f| {
        let x = program.variable(*f);
        !x.static_ && !x.final_ && !x.const_
    });
    if nao_final {
        return false;
    }
    let cx = Ctx { lexico: None, ..v.cx() };
    for init in k.initializers.iter() {
        let exprs: Vec<ExprId> = match init {
            ast::Initializer::Field { value, .. } => vec![*value],
            ast::Initializer::Assert { condition, message, .. } => std::iter::once(*condition).chain(*message).collect(),
            ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => arguments.args.iter().map(|x| x.value).collect(),
        };
        for x in exprs {
            let mut nos = Vec::new();
            super::potencial::coletar_em(v.m, &cx, x, true, false, &mut nos);
            if !nos.is_empty() {
                return false;
            }
        }
    }
    if !k.factory {
        for fm in v.membros_da_classe(a, classe) {
            let MemberKind::Field(l) = &a.member(fm).kind else { continue };
            if l.static_ {
                continue;
            }
            for var in l.variables.iter() {
                let Some(init) = var.initializer else { continue };
                let cx = v.cx();
                if matches!(v.m.avaliar(&cx, init, l.const_), Constante::Invalida(_)) {
                    return false;
                }
            }
        }
    }
    v.valores_padrao(a, &k.parameters);
    !v.saida.iter().any(|(_, d)| d.code.is_some_and(|c| ERROS_DE_CONSTANTE.contains(&c.info().unico)))
}

/// Os erros de constantes das unidades de `lib`.
pub fn verificar(m: &mut Motor<'_>, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    // Os ciclos saem do grafo de dependências, antes de qualquer avaliação
    // (`computeConstants`).
    super::ciclos::computar(m, lib);
    let program = m.program;
    let mut campos = HashMap::new();
    let mut topo = HashMap::new();
    let mut classe_de_membro = HashMap::new();
    for (i, v) in program.variables.iter().enumerate() {
        if v.library != lib {
            continue;
        }
        match v.node {
            VariableRef::Field { unit, member, index } => {
                campos.insert((unit, member, index), VariableId(i as u32));
                if let Some(k) = v.class {
                    classe_de_membro.insert((unit, member), k);
                }
            }
            VariableRef::TopLevel { unit, decl, index } => {
                topo.insert((unit, decl, index), VariableId(i as u32));
            }
            _ => {}
        }
    }
    let padroes_ligados = program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verificador {
        m,
        lib,
        unidade: UnitId(0),
        saida: Vec::new(),
        campos,
        topo,
        classe_de_membro,
        padroes_ligados,
        valores_de_padroes: HashMap::new(),
        valores_de_chaves: HashMap::new(),
        em_expressao_constante: false,
    };
    for &u in &program.library(lib).units {
        if program.unit(u).role == dartforge_elements::model::UnitRole::Patch {
            continue;
        }
        v.unidade = u;
        v.valores_de_padroes.clear();
        v.valores_de_chaves.clear();
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

impl Verificador<'_, '_> {
    /// `const C.nome()` com `C` aplicação de mixin (`class C = S with M;`):
    /// o construtor encaminhado é `const` só se o da superclasse for e
    /// nenhum mixin declarar campo (`ConstructorElementImpl.isConst` dos
    /// encaminhados, `analyzer/lib/src/dart/element/element.dart:612-614`);
    /// não sendo, `CONST_WITH_NON_CONST` na palavra `const`.
    fn encaminhado_nao_const(&mut self, ty: ast::TypeId, mut construtor: Option<ast::Name>, span: Span) {
        let program = self.m.program;
        let u = self.unidade;
        let class = match self.tipo_da_anotacao(ty).map(|t| self.m.table.get(t).clone()) {
            Some(Type::Interface { class, .. }) => class,
            _ => {
                // O tipo da criação não fica registrado: o nome escrito.
                let ast::TypeKind::Named { name, .. } = &program.unit(u).ast.ty(ty).kind else { return };
                // `C.nome` sai do parser como nome de duas partes (`p.C` ou
                // `C.nome`): a primeira parte classe é `C`, com o construtor.
                let (el, ctor) = match &name[..] {
                    [n] => (program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter), construtor),
                    [a1, b1] => match program.lookup_na_unidade(u, a1.sym).and_then(|b| b.getter) {
                        Some(el @ dartforge_elements::model::Element::Class(_)) => (Some(el), Some(*b1)),
                        _ => (program.lookup_prefixed_na_unidade(u, a1.sym, b1.sym).and_then(|b| b.getter), construtor),
                    },
                    [p, c, n] => (program.lookup_prefixed_na_unidade(u, p.sym, c.sym).and_then(|b| b.getter), Some(*n)),
                    _ => (None, None),
                };
                construtor = ctor;
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
        let chave = match construtor {
            Some(n) if interner.resolve(n.sym) != "new" => Some(n.sym),
            _ => interner.lookup(""),
        };
        let Some(chave) = chave else { return };
        let Some(e_const) = self.construtor_encaminhado_const(*class, chave, 0) else { return };
        if !e_const {
            self.relatar(c::CONST_WITH_NON_CONST, Span { start: span.start, end: span.start + 5 }, Vec::new());
        }
    }

    /// `isConst` do construtor `chave` visto pela classe `c` (encaminhado nas
    /// aplicações de mixin); `None` sem construtor.
    fn construtor_encaminhado_const(&self, c: ClassId, chave: dartforge_intern::SymbolId, prof: u32) -> Option<bool> {
        if prof > 32 {
            return None;
        }
        let program = self.m.program;
        let k = program.class(c);
        if k.kind != dartforge_elements::model::ClassKind::MixinApplication {
            let f = *k.constructors.get(&chave)?;
            let f = program.publico(f);
            if program.function(f).factory {
                return None;
            }
            return Some(program.function(f).const_);
        }
        let s = k.supertype_class?;
        let do_super = self.construtor_encaminhado_const(s, chave, prof + 1)?;
        let mixin_com_campo = k.mixin_classes.iter().any(|&m| !program.class(m).fields.is_empty());
        Some(do_super && !mixin_com_campo)
    }

    /// `ConstantVerifier.visitAnnotation`: o elemento é um construtor
    /// `const` com lista de argumentos; cada argumento é avaliado, e o que
    /// não é constante é `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    fn anotacao(&mut self, m: &ast::Annotation) {
        let Some(args) = &m.arguments else { return };
        let program = self.m.program;
        let interner = self.m.interner;
        let Some(el) = crate::anotacoes::elemento_da_anotacao(program, interner, self.unidade, m) else { return };
        let classe = match el {
            dartforge_elements::model::Element::Class(c) => c,
            dartforge_elements::model::Element::Typedef(td) => match self.m.table.get(self.m.outline.typedefs[td.0 as usize].target_type) {
                Type::Interface { class, .. } => *class,
                _ => return,
            },
            _ => return,
        };
        // O construtor: o último nome, se for um construtor da classe; senão
        // o sem nome.
        let ultimo = m.name.last().map(|n| n.sym);
        let nomeado = (m.name.len() >= 2)
            .then_some(ultimo)
            .flatten()
            .filter(|&s| interner.resolve(s) != "new" && interner.resolve(s) != interner.resolve(program.class(classe).name))
            .and_then(|s| program.class(classe).constructors.get(&s).copied());
        let f = nomeado.or_else(|| interner.lookup("").and_then(|k| program.class(classe).constructors.get(&k).copied()));
        let Some(f) = f else { return };
        if !program.function(program.publico(f)).const_ {
            return;
        }
        for x in args.args.iter() {
            self.avaliar_e_relatar(x.value, true, c::CONST_WITH_NON_CONSTANT_ARGUMENT);
        }
    }

    fn cx(&self) -> Ctx {
        Ctx::simples(self.unidade, self.lib)
    }

    fn relatar(&mut self, codigo: Codigo, span: Span, args: Vec<String>) {
        self.saida.push((self.unidade, Diagnostic::com_codigo(codigo, span, args)));
    }

    /// O erro com o código dele e as `contextMessages` (no arquivo de cada
    /// uma, quando não é o desta unidade).
    fn relatar_com_contexto(&mut self, i: &Invalida) {
        let mut d = Diagnostic::com_codigo(i.codigo, i.span, i.args.clone());
        for (u, span, texto) in &i.contexto {
            let arquivo = (*u != self.unidade).then(|| self.m.program.caminho_da_unidade(*u).into());
            d.contexto.push(dartforge_diagnostics::Contexto { arquivo, span: *span, mensagem: texto.as_str().into() });
        }
        self.saida.push((self.unidade, d));
    }

    /// `_reportError`: o código específico leva as mensagens de contexto; o
    /// padrão, não.
    fn relatar_invalida(&mut self, i: &Invalida, padrao: Option<Codigo>) {
        if i.evitar_relato || i.unidade != self.unidade {
            return;
        }
        if ESPECIFICOS.contains(&i.codigo) {
            self.relatar_com_contexto(i);
        } else if let Some(p) = padrao {
            self.relatar(p, i.span, Vec::new());
        }
    }

    /// `_evaluateAndReportError`.
    fn avaliar_e_relatar(&mut self, e: ExprId, em_const: bool, padrao: Codigo) -> Constante {
        let cx = self.cx();
        let r = self.m.avaliar(&cx, e, em_const);
        if let Constante::Invalida(i) = &r {
            let i = (**i).clone();
            self.relatar_invalida(&i, Some(padrao));
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
                                self.resultado_de_variavel(v, true);
                            }
                        }
                    }
                }
            }
            DeclKind::Function(f) => self.funcao(a, *f),
            DeclKind::Class(k) => self.membros(a, &k.members),
            DeclKind::Mixin(k) => self.membros(a, &k.members),
            DeclKind::Enum(k) => {
                let unidade = self.unidade;
                let classe = self.m.program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == unidade && r.decl == d));
                for (i, cst) in k.constants.iter().enumerate() {
                    if let Some(args) = &cst.arguments {
                        for arg in args.args.iter() {
                            self.expr(a, arg.value, true);
                        }
                        // `visitEnumConstantDeclaration`: `_validateConstantArguments`.
                        for arg in args.args.iter() {
                            self.avaliar_e_relatar(arg.value, true, c::CONST_WITH_NON_CONSTANT_ARGUMENT);
                        }
                    }
                    // `visitEnumConstantDeclaration` (`constant_verifier.dart:204-217`):
                    // o `evaluationResult` inválido da constante, sem código
                    // padrão (a exceção de avaliação vira
                    // `CONST_EVAL_THROWS_EXCEPTION` na constante).
                    if let Some(ci) = classe
                        && let Some(&v) = self.m.program.classes[ci].enum_constants.get(i)
                    {
                        self.resultado_de_variavel(v, false);
                    }
                }
                self.membros(a, &k.members);
            }
            DeclKind::Extension(k) => self.membros(a, &k.members),
            DeclKind::ExtensionType(k) => self.membros(a, &k.members),
            DeclKind::Typedef(_) => {}
        }
    }

    fn resultado_de_variavel(&mut self, v: VariableId, const_: bool) {
        if let Some(Constante::Invalida(i)) = self.m.valor_de_variavel(v) {
            let i = (*i).clone();
            let padrao = if const_ { Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE) } else { None };
            self.relatar_invalida(&i, padrao);
        }
    }

    fn membros(&mut self, a: &ast::Ast, membros: &[ast::MemberId]) {
        for &mid in membros {
            let membro = a.member(mid);
            match &membro.kind {
                MemberKind::Field(l) => {
                    for (i, var) in l.variables.iter().enumerate() {
                        let Some(init) = var.initializer else { continue };
                        let classe_const = !l.static_
                            && self.classe_de_membro.get(&(self.unidade, mid)).copied().is_some_and(|k| {
                                self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Class && self.tem_construtor_gerador_const(k)
                            });
                        let antes = std::mem::replace(&mut self.em_expressao_constante, classe_const);
                        self.expr(a, init, l.const_);
                        self.em_expressao_constante = antes;
                        if !(l.const_ || l.final_) {
                            continue;
                        }
                        let Some(&v) = self.campos.get(&(self.unidade, mid, i)) else { continue };
                        if !l.static_ {
                            let classe = self.classe_de_membro.get(&(self.unidade, mid)).copied();
                            let tem = classe.is_some_and(|k| self.tem_construtor_gerador_const(k));
                            let e_classe = classe.is_some_and(|k| self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Class);
                            if e_classe && !tem {
                                continue;
                            }
                            if !tem {
                                continue;
                            }
                        } else if !l.const_ {
                            continue;
                        }
                        self.resultado_de_variavel(v, l.const_);
                    }
                }
                MemberKind::Method(f) => self.funcao(a, *f),
                MemberKind::Constructor(k) => self.construtor(a, mid, k),
            }
        }
    }

    fn tem_construtor_gerador_const(&self, k: ClassId) -> bool {
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
    fn regras_do_construtor(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        use dartforge_elements::model::ClassKind;
        let Some(classe) = self.classe_de(a, mid) else { return };
        let program = self.m.program;
        let interner = self.m.interner;
        let dados = program.class(classe);
        // `atConstructorDeclaration`: do nome da classe ao fim do nome do
        // construtor, quando há.
        let cabecalho = Span { start: k.class_name.span.start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) };
        if dados.kind == ClassKind::Enum && !k.const_ && !k.factory {
            self.relatar(c::NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR, cabecalho, Vec::new());
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
            let lancamentos: Vec<Span> = a
                .exprs
                .iter()
                .filter(|x| matches!(x.kind, ExprKind::Throw(_)) && x.span.start >= sp.start && x.span.end <= sp.end)
                .map(|x| x.span)
                .collect();
            for s in lancamentos {
                self.relatar(c::CONST_CONSTRUCTOR_THROWS_EXCEPTION, s, Vec::new());
            }
        }
        // Campos de instância dos mixins da cláusula `with`: todo campo que
        // não é estático nem (`abstract` e `final`).
        let mut campos_de_mixin: Vec<String> = Vec::new();
        for &mx in dados.mixin_classes.iter() {
            for &v in program.class(mx).fields.iter() {
                let var = program.variable(v);
                if var.static_ {
                    continue;
                }
                let abstrato = match var.node {
                    VariableRef::Field { unit, member, .. } => {
                        matches!(&program.unit(unit).ast.member(member).kind, MemberKind::Field(l) if l.abstract_)
                    }
                    _ => false,
                };
                if abstrato && var.final_ {
                    continue;
                }
                campos_de_mixin.push(format!("'{}.{}'", interner.resolve(program.class(mx).name), interner.resolve(var.name)));
            }
        }
        if campos_de_mixin.len() == 1 {
            self.relatar(c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD, k.class_name.span, campos_de_mixin);
            return;
        }
        if campos_de_mixin.len() > 1 {
            self.relatar(c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS, k.class_name.span, vec![campos_de_mixin.join(", ")]);
            return;
        }
        // O construtor da superclasse que este invoca (o escrito, ou o sem
        // nome implícito) não é `const`. Um enum sempre chama um `const`; o
        // redirecionador é conferido em outro lugar.
        let redireciona = k.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. }));
        if dados.kind != ClassKind::Enum && !redireciona {
            let escrito = k.initializers.iter().find_map(|i| match i {
                ast::Initializer::Super { span, constructor, .. } => Some((*span, constructor.map(|n| n.sym))),
                _ => None,
            });
            if let Some(sup) = super::ciclos::superclasse_declarada(program, classe) {
                if Some(sup) != self.m.core.object_class {
                    let nome_do_super = escrito.and_then(|(_, n)| n);
                    let chave = nome_do_super.or_else(|| interner.lookup(""));
                    let alvo = chave.and_then(|ch| program.class(sup).constructors.get(&ch).copied());
                    let nao_const = match alvo {
                        Some(g) => !program.function(program.publico(g)).const_,
                        // Superclasse sem construtor declarado: o padrão
                        // implícito, que não é `const`.
                        None => program.class(sup).constructors.is_empty() && nome_do_super.is_none(),
                    };
                    if nao_const {
                        let span = escrito.map_or(k.class_name.span, |(s, _)| s);
                        // No 3.6.2 o argumento é a classe DO CONSTRUTOR; no
                        // 3.13.4, a superclasse (o segundo argumento, que só
                        // a variante 3.13 do código usa: T2, caso c04).
                        let nome = interner.resolve(dados.name).to_string();
                        let superior = interner.resolve(program.class(sup).name).to_string();
                        self.relatar(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, span, vec![nome, superior]);
                        return;
                    }
                }
            }
        }
        // `hasNonFinalField`: a classe, os mixins e as superclasses, em
        // largura — algum campo de instância que não é `final` nem `const`.
        if dados.kind == ClassKind::Class {
            let mut fila: Vec<ClassId> = vec![classe];
            let mut vistos: Vec<ClassId> = Vec::new();
            let mut achou = false;
            while let Some(x) = fila.pop() {
                if vistos.contains(&x) {
                    continue;
                }
                vistos.push(x);
                let dx = program.class(x);
                if dx.fields.iter().any(|&v| {
                    let var = program.variable(v);
                    !var.final_ && !var.const_ && !var.static_
                }) {
                    achou = true;
                    break;
                }
                fila.extend(dx.mixin_classes.iter().copied());
                fila.extend(dx.supertype_class);
            }
            if achou {
                self.relatar(c::CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD, cabecalho, Vec::new());
            }
        }
    }

    fn construtor(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        self.regras_do_construtor(a, mid, k);
        if k.const_ {
            // O construtor gerador de um ciclo (os de fábrica são do
            // `ErrorVerifier`): no nome da classe do cabeçalho.
            let em_ciclo = self
                .m
                .grafo
                .construtor_de(self.unidade, mid)
                .is_some_and(|f| self.m.grafo.construtores_em_ciclo.contains(&f));
            if em_ciclo && !k.factory {
                self.relatar(c::RECURSIVE_CONSTANT_CONSTRUCTOR, k.class_name.span, Vec::new());
            }
            // `_validateConstructorInitializers`: potencialmente constantes.
            let cx = Ctx { lexico: None, ..self.cx() };
            for init in k.initializers.iter() {
                let exprs: Vec<ExprId> = match init {
                    ast::Initializer::Field { value, .. } => vec![*value],
                    ast::Initializer::Assert { condition, message, .. } => std::iter::once(*condition).chain(*message).collect(),
                    ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                        arguments.args.iter().map(|x| x.value).collect()
                    }
                };
                for x in exprs {
                    let mut nos = Vec::new();
                    super::potencial::coletar_em(self.m, &cx, x, true, false, &mut nos);
                    for n in nos {
                        self.relatar(c::INVALID_CONSTANT, n, Vec::new());
                    }
                }
            }
            if !k.factory {
                self.inicializadores_de_campo(a, mid);
            }
        }
        self.valores_padrao(a, &k.parameters);
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
    /// que não é constante, relatados no `const` do construtor.
    fn inicializadores_de_campo(&mut self, a: &ast::Ast, mid: ast::MemberId) {
        let Some(k) = self.classe_de(a, mid) else { return };
        let e_enum = self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Enum;
        let Some(palavra) = self.palavra_const(a, mid) else { return };
        let membros: Vec<ast::MemberId> = self.membros_da_classe(a, k);
        for fm in membros {
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
                let r = self.m.avaliar(&cx, init, l.const_);
                if matches!(r, Constante::Invalida(_)) {
                    let nome = self.m.interner.resolve(var.name.sym).to_string();
                    self.relatar(c::CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST, palavra, vec![nome]);
                }
            }
        }
    }

    /// A classe declarada que contém o membro `mid` (pelos construtores).
    fn classe_de(&self, _a: &ast::Ast, mid: ast::MemberId) -> Option<ClassId> {
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
    fn palavra_const(&self, a: &ast::Ast, mid: ast::MemberId) -> Option<Span> {
        let s = a.member(mid).span;
        let fonte = &self.m.program.unit(self.unidade).source;
        let t = fonte.get(s.start..s.end)?;
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
            self.valores_padrao(a, ps);
        }
        self.corpo(a, &func.body);
    }

    /// `_validateDefaultValues`.
    fn valores_padrao(&mut self, a: &ast::Ast, ps: &[ast::Parameter]) {
        for p in ps {
            let Some(d) = p.default_value else { continue };
            let antes = std::mem::replace(&mut self.em_expressao_constante, true);
            self.expr(a, d, false);
            self.em_expressao_constante = antes;
            if self.m.body.units[self.unidade.0 as usize].tipos_invalidos.contains(&d) {
                continue;
            }
            let r = self.m.resultado_padrao(self.unidade, self.lib, d);
            if let Constante::Invalida(i) = r {
                self.relatar_invalida(&i, Some(c::NON_CONSTANT_DEFAULT_VALUE));
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
                            let r = self.m.valor_de_local_declarado(self.unidade, self.lib, s, i);
                            if let Constante::Invalida(x) = r {
                                self.relatar_invalida(&x, Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE));
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
                for caso in cases.iter() {
                    if let Some(p) = caso.pattern {
                        if self.padroes_ligados {
                            self.padrao(a, p);
                        } else if let PatternKind::Constant(e) = &a.pattern(p).kind {
                            // `_validateSwitchStatement_nullSafety`.
                            let e = desparentizar(a, *e);
                            self.expr(a, e, true);
                            // Sem padrões, o valor precisa de igualdade
                            // primitiva: `CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS`
                            // com o tipo do valor (`constant_verifier.dart:1009-1030`).
                            if let Constante::Valor(v) = self.avaliar_e_relatar(e, true, c::NON_CONSTANT_CASE_EXPRESSION)
                                && !v.desconhecido_de_fato()
                                && !self.m.igualdade_primitiva(&v, self.lib)
                            {
                                let sp = self.m.span(self.unidade, e);
                                let tipo = self.m.formatar(v.tipo);
                                self.relatar(c::CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS, sp, vec![tipo]);
                            }
                        }
                    }
                    if let Some(g) = caso.guard {
                        self.expr(a, g, false);
                    }
                    for x in caso.body.iter() {
                        self.stmt(a, *x);
                    }
                }
                if self.padroes_ligados {
                    let casos: Vec<Caso> = cases.iter().map(|k| Caso { padrao: k.pattern, guardado: k.guard.is_some() }).collect();
                    let inicio = a.stmt(s).span.start;
                    let pos: Vec<usize> = cases.iter().map(|k| self.inicio_da_palavra_do_caso(k)).collect();
                    self.exaustividade(a, *value, inicio, &casos, &pos, false);
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
    fn apagar_extensao(&mut self, t: TypeId) -> TypeId {
        let outline = self.m.outline;
        let program = self.m.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut crate::table::TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let mapa: HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &mapa, table))
        };
        crate::ops::erase_extension_type(t, self.m.table, &rep)
    }

    /// `_canBeEqual(constantType, valueType)`
    /// (an611:src/dart/constant/constant_verifier.dart:517-546). `Null` é
    /// `InterfaceType` no analyzer (`NullTypeImpl`).
    fn pode_ser_igual(&mut self, c: TypeId, v: TypeId, prof: u32) -> bool {
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
                let maior = self.fecho_maior(v, true, &[]);
                let mut env = crate::subtyping::SubtypeEnv::new(&mut *self.m.table, &self.m.outline.hierarchy, self.m.core);
                crate::subtyping::is_subtype(c, maior, &mut env)
            }
            Type::TypeParameter { param, nullable } => {
                let dados = self.m.table.param(param).clone();
                if !dados.explicito || referencia_parametro(self.m.table, dados.bound) {
                    return true;
                }
                let b = if nullable { crate::ops::nullable(dados.bound, self.m.table) } else { dados.bound };
                self.pode_ser_igual(c, b, prof + 1)
            }
            // `promotedBound`: o limite promovido (a anulabilidade é a do
            // parâmetro, sem `?`).
            Type::Intersection { bound, .. } => {
                if referencia_parametro(self.m.table, bound) {
                    return true;
                }
                self.pode_ser_igual(c, bound, prof + 1)
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
    fn fecho_maior(&mut self, t: TypeId, co: bool, formais: &[crate::table::TypeParamId]) -> TypeId {
        let tabela = &mut *self.m.table;
        match tabela.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } if !formais.contains(&param) => {
                let anulavel = matches!(tabela.get(t), Type::TypeParameter { nullable: true, .. });
                if co {
                    self.m.core.object_nullable
                } else if anulavel {
                    self.m.core.null
                } else {
                    self.m.core.never
                }
            }
            Type::Interface { class, args, nullable } if !args.is_empty() => {
                let novos: Vec<TypeId> = args.iter().map(|&x| self.fecho_maior(x, co, formais)).collect();
                self.m.table.intern(Type::Interface { class, args: novos.into_boxed_slice(), nullable })
            }
            Type::ExtensionType { decl, args, nullable } if !args.is_empty() => {
                let novos: Vec<TypeId> = args.iter().map(|&x| self.fecho_maior(x, co, formais)).collect();
                self.m.table.intern(Type::ExtensionType { decl, args: novos.into_boxed_slice(), nullable })
            }
            Type::FutureOr { arg, nullable } => {
                let arg = self.fecho_maior(arg, co, formais);
                self.m.table.intern(Type::FutureOr { arg, nullable })
            }
            Type::Record { positional, named, nullable } => {
                let pos: Vec<TypeId> = positional.iter().map(|&x| self.fecho_maior(x, co, formais)).collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId)> = named.iter().map(|&(n, x)| (n, self.fecho_maior(x, co, formais))).collect();
                self.m.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable })
            }
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let mut dentro: Vec<crate::table::TypeParamId> = formais.to_vec();
                dentro.extend(type_params.iter().copied());
                let ret = self.fecho_maior(ret, co, &dentro);
                let pos: Vec<TypeId> = positional.iter().map(|&x| self.fecho_maior(x, !co, &dentro)).collect();
                let opt: Vec<TypeId> = optional.iter().map(|&x| self.fecho_maior(x, !co, &dentro)).collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|&(n, x, r)| (n, self.fecho_maior(x, !co, &dentro), r)).collect();
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
                    && type_args.iter().any(|&t| self.tipo_da_anotacao(t).is_some_and(|r| self.m.menciona_parametro(r)))
                {
                    self.relatar(c::CONST_TYPE_PARAMETER, a.expr(x).span, Vec::new());
                    return;
                }
                let e = desparentizar(a, *e);
                if self.m.body.units[self.unidade.0 as usize].tipos_invalidos.contains(&e) {
                    return;
                }
                let r = self.avaliar_e_relatar(e, false, c::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION);
                if let Constante::Valor(v) = r {
                    // `CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE`
                    // (an611:src/dart/constant/constant_verifier.dart:141-158):
                    // valor de igualdade primitiva cujo tipo não pode ser
                    // igual a um valor do tipo casado (sem os tipos de
                    // extensão).
                    if self.padroes_ligados
                        && self.m.igualdade_primitiva(&v, self.lib)
                        && let Some(&casado) = self.m.body.units[self.unidade.0 as usize].tipos_casados.get(&p)
                    {
                        let casado = self.apagar_extensao(casado);
                        if !self.pode_ser_igual(v.tipo, casado, 0) {
                            let exibidor = crate::exibicao::Exibidor { table: &*self.m.table, interner: self.m.interner, program: self.m.program };
                            let (textos, contexto) = exibidor.argumentos_e_contexto(&[crate::exibicao::Arg::Tipo(casado), crate::exibicao::Arg::Tipo(v.tipo)]);
                            let mut d = Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE, a.pattern(p).span, textos);
                            d.contexto.extend(contexto);
                            self.saida.push((self.unidade, d));
                            self.valores_de_padroes.insert(p, v);
                            return;
                        }
                    }
                    self.valores_de_padroes.insert(p, v);
                    self.expr(a, e, false);
                }
            }
            // `case nome:` (o parser guarda como variável): a constante de
            // topo com esse nome, para a exaustividade.
            PatternKind::Variable { final_: false, var_: false, ty: None, name } if self.padroes_ligados => {
                let program = self.m.program;
                if let Some(dartforge_elements::model::Element::Variable(v)) =
                    program.lookup_na_unidade(self.unidade, name.sym).and_then(|b| b.getter)
                {
                    if let Some(Constante::Valor(val)) = self.m.valor_de_variavel(v) {
                        self.valores_de_padroes.insert(p, val);
                    }
                }
            }
            PatternKind::Relational { value, .. } => {
                self.expr(a, *value, false);
                self.avaliar_e_relatar(*value, false, c::NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION);
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
                // chave repetida, depois de todas.
                let mut unicas: Vec<(Valor, Span)> = Vec::new();
                let mut repetidas: Vec<(Span, Span)> = Vec::new();
                for en in entries.iter() {
                    self.padrao(a, en.value);
                    if let Constante::Valor(v) = self.avaliar_e_relatar(en.key, false, c::NON_CONSTANT_MAP_PATTERN_KEY) {
                        let conhecida = !(v.desconhecido_de_fato() || matches!(v.estado, Estado::Null { invalido: true }));
                        if conhecida {
                            let span = a.expr(en.key).span;
                            match unicas.iter().find(|(x, _)| self.m.iguais(x, &v)) {
                                Some(&(_, original)) => repetidas.push((span, original)),
                                None => unicas.push((v.clone(), span)),
                            }
                        }
                        self.valores_de_chaves.insert(en.key, v);
                    }
                }
                // `equalKeysInMapPattern`: o contexto é a primeira chave.
                for (sp, original) in repetidas {
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

    fn expr(&mut self, a: &ast::Ast, e: ExprId, em_const: bool) {
        let u = self.unidade;
        match &a.expr(e).kind {
            ExprKind::InstanceCreation { keyword, arguments, ty, constructor } => {
                let const_ = matches!(keyword, Some(ast::CreationKeyword::Const));
                // `_checkForConstWithNonConst` com o construtor encaminhado de
                // uma aplicação de mixin (a inferência não o resolve).
                if const_ && !matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    self.encaminhado_nao_const(*ty, *constructor, a.expr(e).span);
                }
                // `visitInstanceCreationExpression` (`isConst`: `const`, ou
                // sem palavra num contexto constante).
                if const_ || (keyword.is_none() && em_const) {
                    self.sem_parametros_de_tipo(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                if const_ {
                    self.criacao_constante(a, e, arguments);
                } else {
                    for x in arguments.args.iter() {
                        self.expr(a, x.value, em_const);
                    }
                }
            }
            ExprKind::Call { target, arguments } => {
                if em_const && matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    // O `NamedType` da criação sem `new`: os argumentos de
                    // tipo da chamada ou os do `C<T>` antes de `.nome`.
                    let mut escritos: Vec<ast::TypeId> = arguments.type_args.to_vec();
                    if let ExprKind::Property { target: r, .. } = &a.expr(*target).kind
                        && let ExprKind::TypeArguments { type_args, .. } = &a.expr(*r).kind
                    {
                        escritos.extend(type_args.iter().copied());
                    }
                    for x in escritos {
                        self.sem_parametros_de_tipo(a, x, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                    }
                    self.criacao_constante(a, e, arguments);
                    return;
                }
                self.expr(a, *target, em_const);
                for x in arguments.args.iter() {
                    self.expr(a, x.value, em_const);
                }
            }
            ExprKind::List { const_, elements, type_args } => {
                let c = *const_ || em_const;
                for el in elements.iter() {
                    self.elemento_filho(a, el, c);
                }
                if c {
                    for &x in type_args.iter() {
                        self.argumento_de_tipo_const(a, x, c::INVALID_TYPE_ARGUMENT_IN_CONST_LIST);
                    }
                }
                if c {
                    let tipo = self.m.estatico(u, e);
                    let elemento = match self.m.table.get(tipo) {
                        Type::Interface { args, .. } if args.len() == 1 => args[0],
                        _ => self.m.core.dynamic_,
                    };
                    let mut lv = Literal { tipo: TipoLiteral::Lista(elemento), unicos: Vec::new(), duplicados: Vec::new() };
                    for el in elements.iter() {
                        self.verificar_elemento(a, &mut lv, el);
                    }
                }
            }
            ExprKind::SetOrMap { const_, elements, type_args } => {
                let c = *const_ || em_const;
                for el in elements.iter() {
                    self.elemento_filho(a, el, c);
                }
                if c {
                    let tipo = self.m.estatico(u, e);
                    let (classe, args) = match self.m.table.get(tipo) {
                        Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                        _ => (None, Vec::new()),
                    };
                    // `TypeArgumentsVerifier.checkMapLiteral`/`checkSetLiteral`:
                    // só com a forma decidida.
                    let codigo = if classe.is_some() && classe == self.m.core.map_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_MAP)
                    } else if classe.is_some() && classe == self.m.core.set_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_SET)
                    } else {
                        None
                    };
                    if let Some(codigo) = codigo {
                        for &x in type_args.iter() {
                            self.argumento_de_tipo_const(a, x, codigo);
                        }
                    }
                    let tl = if classe.is_some() && classe == self.m.core.set_class && args.len() == 1 {
                        Some(TipoLiteral::Conjunto(args[0]))
                    } else if classe.is_some() && classe == self.m.core.map_class && args.len() == 2 {
                        Some(TipoLiteral::Mapa(args[0], args[1]))
                    } else {
                        None
                    };
                    if let Some(tl) = tl {
                        let mut lv = Literal { tipo: tl, unicos: Vec::new(), duplicados: Vec::new() };
                        for el in elements.iter() {
                            self.verificar_elemento(a, &mut lv, el);
                        }
                        // `equalKeysInConstMap`/`equalElementsInConstSet`: o
                        // contexto é o primeiro com o mesmo valor.
                        let (codigo, contexto) = if matches!(lv.tipo, TipoLiteral::Mapa(..)) {
                            (c::EQUAL_KEYS_IN_CONST_MAP, "The first key with this value.")
                        } else {
                            (c::EQUAL_ELEMENTS_IN_CONST_SET, "The first element with this value.")
                        };
                        for (dup, original) in std::mem::take(&mut lv.duplicados) {
                            let d = Diagnostic::com_codigo(codigo, dup, Vec::<String>::new()).com_contexto(original, contexto);
                            self.saida.push((self.unidade, d));
                        }
                    }
                } else {
                    self.duplicados_em_literal_nao_constante(a, e, elements);
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let c = *const_ || em_const;
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(a, *x, c);
                }
                if *const_ {
                    for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                        self.avaliar_e_relatar(*x, true, c::NON_CONSTANT_RECORD_FIELD);
                    }
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(a, *f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(a, *x, em_const);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(a, *x, em_const),
            ExprKind::Property { target, name, .. } => {
                // `visitConstructorReference`: `C<T>.nome` de um construtor.
                if (em_const || self.em_expressao_constante)
                    && let ExprKind::TypeArguments { target: base, type_args } = &a.expr(*target).kind
                    && self.construtor_referido(a, *base, *name)
                {
                    for &x in type_args.iter() {
                        self.sem_parametros_de_tipo(a, x, c::CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF, &[]);
                    }
                    self.expr(a, *base, em_const);
                    return;
                }
                self.expr(a, *target, em_const)
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(a, *target, em_const);
                self.expr(a, *index, em_const);
            }
            ExprKind::TypeArguments { target, type_args } => {
                // `visitFunctionReference`: a instanciação de uma função (não
                // o literal de tipo, de tipo `Type`).
                if (em_const || self.em_expressao_constante) && self.m.estatico(u, e) != self.m.core.type_ {
                    for &x in type_args.iter() {
                        self.sem_parametros_de_tipo(a, x, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF, &[]);
                    }
                }
                self.expr(a, *target, em_const)
            }
            ExprKind::Unary { operand, .. } => self.expr(a, *operand, em_const),
            ExprKind::Binary { left, right, .. } => {
                self.expr(a, *left, em_const);
                self.expr(a, *right, em_const);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(a, *condition, em_const);
                self.expr(a, *then, em_const);
                self.expr(a, *else_, em_const);
            }
            ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
                // `visitGenericFunctionType` sob `is`/`as` num contexto
                // constante.
                if em_const && matches!(a.ty(*ty).kind, ast::TypeKind::Function { .. }) {
                    self.sem_parametros_de_tipo(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                self.expr(a, *value, em_const)
            }
            ExprKind::Assign { target, value, .. } => {
                self.expr(a, *target, em_const);
                self.expr(a, *value, em_const);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(a, *pattern);
                self.expr(a, *value, em_const);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(a, *target, em_const);
                for s in sections.iter() {
                    self.expr(a, *s, em_const);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(a, *value, em_const);
                for caso in cases.iter() {
                    self.padrao(a, caso.pattern);
                    if let Some(g) = caso.guard {
                        self.expr(a, g, false);
                    }
                    self.expr(a, caso.body, em_const);
                }
                let casos: Vec<Caso> = cases.iter().map(|k| Caso { padrao: Some(k.pattern), guardado: k.guard.is_some() }).collect();
                let inicio = a.expr(e).span.start;
                let pos: Vec<usize> = cases.iter().map(|k| self.seta_do_caso(a, k)).collect();
                self.exaustividade(a, *value, inicio, &casos, &pos, true);
            }
            _ => {}
        }
    }

    // -- Exaustividade (`_validateSwitchExhaustiveness`, `:883-1000`) ----------

    /// Relata `UNREACHABLE_SWITCH_CASE` (na palavra `case` ou na seta),
    /// `NON_EXHAUSTIVE_SWITCH_*` (na palavra `switch`) e
    /// `UNREACHABLE_SWITCH_DEFAULT` (na palavra `default`). `pos` é o
    /// início do token de cada caso.
    fn exaustividade(&mut self, a: &ast::Ast, valor: ExprId, inicio_switch: usize, casos: &[Caso], pos: &[usize], expressao: bool) {
        let u = self.unidade;
        let body = self.m.body;
        let corpo = &body.units[u.0 as usize];
        if corpo.tipos_invalidos.contains(&valor) {
            return;
        }
        let Some(t) = corpo.get_type(valor) else { return };
        let program = self.m.program;
        let fonte = program.unit(u).source.as_str();
        let versao_3_3 = program.library(self.lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 3);
        let valores_de_padroes = std::mem::take(&mut self.valores_de_padroes);
        let valores_de_chaves = std::mem::take(&mut self.valores_de_chaves);
        let entrada = Entrada {
            ast: a,
            source: fonte,
            tipos_de_padroes: &corpo.tipos_de_padroes,
            campos_de_extensao: &corpo.campos_de_extensao,
            padroes_invalidos: &corpo.padroes_invalidos,
            valores_de_padroes: &valores_de_padroes,
            valores_de_chaves: &valores_de_chaves,
            versao_3_3,
        };
        let coletar = self.m.testemunhas.is_some();
        let mut ex = Exaustividade::nova(self.m, self.lib);
        ex.coletar_partes = coletar;
        let deve = expressao || ex.sempre_exaustivo(t, 0);
        let r = ex.verificar(&entrada, t, casos);
        drop(ex);
        self.valores_de_padroes = valores_de_padroes;
        self.valores_de_chaves = valores_de_chaves;
        let Some(r) = r else { return };
        let mut partes = r.partes;
        for i in r.inalcancaveis {
            let n = if expressao { 2 } else { 4 };
            self.relatar(w::UNREACHABLE_SWITCH_CASE, Span { start: pos[i], end: pos[i] + n }, Vec::new());
        }
        let default = casos.iter().position(|k| k.padrao.is_none());
        match (r.testemunha, r.correcao) {
            (Some(tw), Some(co)) => {
                if deve && default.is_none() {
                    let codigo = if expressao { c::NON_EXHAUSTIVE_SWITCH_EXPRESSION } else { c::NON_EXHAUSTIVE_SWITCH_STATEMENT };
                    let tipo = self.m.formatar(t);
                    // O quarto argumento só existe para a variante "privado"
                    // do 3.13.4 (`constant_verifier.dart` do main, caso c29
                    // de `corpus/especificacao/t2/v`): o valor é de um enum
                    // de outra biblioteca e o que falta é privado. O
                    // original olha todas as testemunhas; aqui, a primeira.
                    let privado = match self.m.table.get(t) {
                        Type::Interface { class, .. } => {
                            let k = self.m.program.class(*class);
                            k.kind == dartforge_elements::model::ClassKind::Enum
                                && k.library != self.lib
                                && (self.m.interner.resolve(k.name).starts_with('_') || tw.contains("._"))
                        }
                        _ => false,
                    };
                    let mut args = vec![tipo, tw, co];
                    if privado {
                        args.push(String::new());
                    }
                    self.relatar(codigo, Span { start: inicio_switch, end: inicio_switch + 6 }, args);
                    let u = self.unidade;
                    if let Some(mapa) = self.m.testemunhas.as_mut() {
                        mapa.insert((u, inicio_switch), std::mem::take(&mut partes));
                    }
                }
            }
            _ => {
                if let Some(d) = default {
                    if deve {
                        self.relatar(w::UNREACHABLE_SWITCH_DEFAULT, Span { start: pos[d], end: pos[d] + 7 }, Vec::new());
                    }
                }
            }
        }
    }

    /// O início da palavra `case`/`default` de um caso de `switch` (depois
    /// dos rótulos).
    fn inicio_da_palavra_do_caso(&self, k: &ast::SwitchCase) -> usize {
        let fonte = self.m.program.unit(self.unidade).source.as_bytes();
        let mut i = k.span.start;
        if let Some(l) = k.labels.last() {
            i = pular_brancos(fonte, l.span.end);
            if fonte.get(i) == Some(&b':') {
                i += 1;
            }
            i = pular_brancos(fonte, i);
        }
        i
    }

    /// O início da seta `=>` de um caso de expressão `switch`.
    fn seta_do_caso(&self, a: &ast::Ast, k: &ast::SwitchExprCase) -> usize {
        let fonte = self.m.program.unit(self.unidade).source.as_bytes();
        let fim = match k.guard {
            Some(g) => a.expr(g).span.end,
            None => a.pattern(k.pattern).span.end,
        };
        let limite = a.expr(k.body).span.start;
        let mut i = fim;
        while i + 1 < fonte.len() && i < limite && !(fonte[i] == b'=' && fonte[i + 1] == b'>') {
            let j = pular_brancos(fonte, i);
            i = if j > i { j } else { i + 1 };
        }
        i
    }

    fn elemento_filho(&mut self, a: &ast::Ast, el: &CollectionElement, em_const: bool) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(a, *x, em_const),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(a, *key, em_const);
                self.expr(a, *value, em_const);
            }
            CollectionElement::Spread { value, .. } => self.expr(a, *value, em_const),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(a, *condition, em_const);
                if let Some(p) = case_pattern {
                    self.padrao(a, *p);
                }
                if let Some(g) = guard {
                    self.expr(a, *g, em_const);
                }
                self.elemento_filho(a, then, em_const);
                if let Some(x) = else_ {
                    self.elemento_filho(a, x, em_const);
                }
            }
            CollectionElement::For { condition, updates, body, .. } => {
                if let Some(c) = condition {
                    self.expr(a, *c, em_const);
                }
                for u in updates.iter() {
                    self.expr(a, *u, em_const);
                }
                self.elemento_filho(a, body, em_const);
            }
            CollectionElement::ForIn { iterable, body, .. } => {
                self.expr(a, *iterable, em_const);
                self.elemento_filho(a, body, em_const);
            }
        }
    }

    /// `visitInstanceCreationExpression` de uma criação constante: avalia e
    /// relata (com o relator verdadeiro: os argumentos não resolvidos
    /// também); com valor, segue nos argumentos.
    fn criacao_constante(&mut self, a: &ast::Ast, e: ExprId, arguments: &ast::Arguments) {
        if !matches!(self.m.resolvido(self.unidade, e), Some(Resolved::Constructor(_))) {
            return;
        }
        let cx = Ctx { relatar: true, ..self.cx() };
        let antes = self.m.relatos.len();
        let r = self.m.avaliar(&cx, e, true);
        let relatos: Vec<Invalida> = self.m.relatos.drain(antes..).collect();
        for i in relatos {
            if i.unidade == self.unidade && !i.evitar_relato {
                self.relatar_com_contexto(&i);
            }
        }
        match r {
            Constante::Invalida(i) => {
                if !i.evitar_relato && i.unidade == self.unidade {
                    self.relatar_com_contexto(&i);
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

    fn codigo_do_literal(t: &TipoLiteral) -> Codigo {
        match t {
            TipoLiteral::Lista(_) => c::NON_CONSTANT_LIST_ELEMENT,
            TipoLiteral::Conjunto(_) => c::NON_CONSTANT_SET_ELEMENT,
            TipoLiteral::Mapa(..) => c::NON_CONSTANT_MAP_ELEMENT,
        }
    }

    fn verificar_elemento(&mut self, a: &ast::Ast, lv: &mut Literal, el: &CollectionElement) -> bool {
        let u = self.unidade;
        let codigo = Self::codigo_do_literal(&lv.tipo);
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                let Constante::Valor(v) = self.avaliar_e_relatar(*x, true, codigo) else { return false };
                let span = self.m.span(u, *x);
                match lv.tipo {
                    TipoLiteral::Lista(t) => {
                        if !self.m.casa(&v, t) {
                            let args = vec![self.m.formatar(v.tipo), self.m.formatar(t)];
                            self.relatar(c::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        true
                    }
                    TipoLiteral::Conjunto(t) => {
                        if !self.m.casa(&v, t) {
                            let args = vec![self.m.formatar(v.tipo), self.m.formatar(t)];
                            self.relatar(c::SET_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        if !self.m.igualdade_primitiva(&v, self.lib) {
                            let args = vec![self.m.formatar(v.tipo)];
                            self.relatar(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, span, args);
                            return false;
                        }
                        self.registrar_unico(lv, v, span);
                        true
                    }
                    TipoLiteral::Mapa(..) => true,
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                let s = self.m.span_de_elemento(u, el);
                self.relatar(c::CONST_EVAL_FOR_ELEMENT, s, Vec::new());
                false
            }
            CollectionElement::If { condition, then, else_, case_pattern, .. } => {
                if case_pattern.is_some() {
                    return false;
                }
                let Constante::Valor(cond) = self.avaliar_e_relatar(*condition, true, codigo) else { return false };
                if !cond.estado.e_bool() {
                    return false;
                }
                match cond.como_bool() {
                    None => {
                        let a1 = self.potenciais_no_literal(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.potenciais_no_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(true) => {
                        let a1 = self.verificar_elemento(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.potenciais_no_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(false) => {
                        let a1 = self.potenciais_no_literal(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.verificar_elemento(a, lv, x));
                        a1 && a2
                    }
                }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let TipoLiteral::Mapa(tk, tv) = lv.tipo else { return false };
                let k = self.avaliar_e_relatar(*key, true, c::NON_CONSTANT_MAP_KEY);
                let v = self.avaliar_e_relatar(*value, true, c::NON_CONSTANT_MAP_VALUE);
                if let Constante::Valor(k) = k {
                    let span = self.m.span(u, *key);
                    if !self.m.casa(&k, tk) {
                        let args = vec![self.m.formatar(k.tipo), self.m.formatar(tk)];
                        self.relatar(c::MAP_KEY_TYPE_NOT_ASSIGNABLE, span, args);
                    }
                    if !self.m.igualdade_primitiva(&k, self.lib) {
                        let args = vec![self.m.formatar(k.tipo)];
                        self.relatar(c::CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY, span, args);
                    }
                    self.registrar_unico(lv, k, span);
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
                let Constante::Valor(v) = self.avaliar_e_relatar(*value, true, codigo) else { return false };
                let span = self.m.span(u, *value);
                match lv.tipo {
                    TipoLiteral::Lista(_) | TipoLiteral::Conjunto(_) => {
                        let lista = v.como_lista().cloned();
                        let conj = v.como_conjunto().cloned();
                        let Some(itens) = lista.clone().or(conj) else {
                            if v.estado.e_nulo() && *null_aware {
                                return true;
                            }
                            self.relatar(c::CONST_SPREAD_EXPECTED_LIST_OR_SET, span, Vec::new());
                            return false;
                        };
                        if matches!(lv.tipo, TipoLiteral::Lista(_)) {
                            return true;
                        }
                        if lista.is_some() && !itens.iter().all(|x| self.m.igualdade_primitiva(x, self.lib)) {
                            let s = self.m.span_de_elemento(u, el);
                            let args = vec![self.m.formatar(v.tipo)];
                            self.relatar(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, s, args);
                            return false;
                        }
                        for x in itens.iter() {
                            self.registrar_unico(lv, x.clone(), span);
                        }
                        true
                    }
                    TipoLiteral::Mapa(..) => {
                        if v.estado.e_nulo() && *null_aware {
                            return true;
                        }
                        match v.como_mapa().cloned() {
                            Some(m) => {
                                for (k, _) in m.iter() {
                                    self.registrar_unico(lv, k.clone(), span);
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
    fn sem_parametros_de_tipo(&mut self, a: &ast::Ast, x: ast::TypeId, codigo: Codigo, permitidos: &[crate::table::TypeParamId]) {
        let anotacao = a.ty(x);
        match &anotacao.kind {
            ast::TypeKind::Named { args, .. } => {
                let parametro = self.tipo_da_anotacao(x).and_then(|r| {
                    if matches!(self.m.table.exibicao(r), Some(crate::table::Exibicao::Alias { .. })) {
                        return None;
                    }
                    match self.m.table.get(r) {
                        Type::TypeParameter { param, .. } => Some(*param),
                        _ => None,
                    }
                });
                if let Some(p) = parametro
                    && !permitidos.contains(&p)
                {
                    self.relatar(codigo, anotacao.span, Vec::new());
                    return;
                }
                for &y in args.iter() {
                    self.sem_parametros_de_tipo(a, y, codigo, permitidos);
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                let mut dentro: Vec<crate::table::TypeParamId> = permitidos.to_vec();
                if let Some(r) = self.tipo_da_anotacao(x)
                    && let Type::Function { type_params: formais, .. } = self.m.table.get(r)
                {
                    dentro.extend(formais.iter().copied());
                }
                for tp in type_params.iter() {
                    if let Some(b) = tp.bound {
                        self.sem_parametros_de_tipo(a, b, codigo, &dentro);
                    }
                }
                if let Some(r) = return_type {
                    self.sem_parametros_de_tipo(a, *r, codigo, &dentro);
                }
                for p in parameters.iter() {
                    if p.function_parameters.is_none()
                        && let Some(y) = p.ty
                    {
                        self.sem_parametros_de_tipo(a, y, codigo, &dentro);
                    }
                }
            }
            ast::TypeKind::Record { .. } | ast::TypeKind::Void => {}
        }
    }

    /// `base.nome` designa um construtor (`C.new`, `C.nome`, `p.C.nome`,
    /// pelo alias também).
    fn construtor_referido(&self, a: &ast::Ast, base: ExprId, nome: ast::Name) -> bool {
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
        let classe = match el {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            Some(dartforge_elements::model::Element::Typedef(td)) => match self.m.table.get(self.m.outline.typedefs[td.0 as usize].target_type) {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
                _ => None,
            },
            _ => None,
        };
        let Some(c) = classe else { return false };
        let texto = self.m.interner.resolve(nome.sym);
        let chave = if texto == "new" { self.m.interner.lookup("") } else { Some(nome.sym) };
        chave.is_some_and(|k| program.class(c).constructors.contains_key(&k))
    }

    fn tipo_da_anotacao(&self, x: ast::TypeId) -> Option<TypeId> {
        let u = self.unidade;
        self.m.body.units.get(u.0 as usize).and_then(|b| b.tipos_de_anotacoes.get(&x).copied()).or_else(|| self.m.outline.tipos_escritos.get(&(u, x)).copied())
    }

    /// `_checkTypeArgumentConst` (`type_arguments_verifier.dart:501-544`): um
    /// argumento de tipo de literal constante que é parâmetro de tipo, no nó
    /// do tipo; no tipo nomeado com o lexema, nos de função e registro com o
    /// tipo exibido. Desce nos argumentos, nos parâmetros simples e no
    /// retorno de um tipo de função e nos campos de um registro.
    fn argumento_de_tipo_const(&mut self, a: &ast::Ast, x: ast::TypeId, codigo: Codigo) {
        let e_parametro = |s: &Self, t: ast::TypeId| s.tipo_da_anotacao(t).filter(|r| matches!(s.m.table.get(*r), Type::TypeParameter { .. }));
        let anotacao = a.ty(x);
        match &anotacao.kind {
            ast::TypeKind::Named { name, args } => {
                if e_parametro(self, x).is_some() {
                    let lexema = name.last().map(|n| self.m.interner.resolve(n.sym).to_string()).unwrap_or_default();
                    self.relatar(codigo, anotacao.span, vec![lexema]);
                } else {
                    for &y in args.iter() {
                        self.argumento_de_tipo_const(a, y, codigo);
                    }
                }
            }
            ast::TypeKind::Function { return_type, parameters, .. } => {
                let mut filhos: Vec<ast::TypeId> = parameters.iter().filter(|p| p.function_parameters.is_none()).filter_map(|p| p.ty).collect();
                filhos.extend(return_type.iter().copied());
                for y in filhos {
                    self.campo_de_tipo_const(a, y, codigo);
                }
            }
            ast::TypeKind::Record { positional, named } => {
                let filhos: Vec<ast::TypeId> = positional.iter().copied().chain(named.iter().map(|(_, t)| *t)).collect();
                for y in filhos {
                    self.campo_de_tipo_const(a, y, codigo);
                }
            }
            ast::TypeKind::Void => {}
        }
    }

    /// Um parâmetro, retorno ou campo de registro: parâmetro de tipo relata
    /// com o tipo exibido; senão desce.
    fn campo_de_tipo_const(&mut self, a: &ast::Ast, y: ast::TypeId, codigo: Codigo) {
        match self.tipo_da_anotacao(y) {
            Some(r) if matches!(self.m.table.get(r), Type::TypeParameter { .. }) => {
                let exibido = self.m.formatar(r);
                self.relatar(codigo, a.ty(y).span, vec![exibido]);
            }
            _ => self.argumento_de_tipo_const(a, y, codigo),
        }
    }

    /// `BestPracticesVerifier._checkForDuplications`
    /// (`best_practices_verifier.dart:851-875`): num literal não constante,
    /// os elementos (conjunto) ou as chaves (mapa) de primeiro nível que a
    /// avaliação constante dá sem erro, iguais a um anterior:
    /// `EQUAL_ELEMENTS_IN_SET`/`EQUAL_KEYS_IN_MAP` no repetido.
    fn duplicados_em_literal_nao_constante(&mut self, _a: &ast::Ast, e: ExprId, elements: &[CollectionElement]) {
        let u = self.unidade;
        let tipo = self.m.estatico(u, e);
        let classe = match self.m.table.get(tipo) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let (mapa, codigo) = if classe.is_some() && classe == self.m.core.set_class {
            (false, w::EQUAL_ELEMENTS_IN_SET)
        } else if classe.is_some() && classe == self.m.core.map_class {
            (true, w::EQUAL_KEYS_IN_MAP)
        } else {
            return;
        };
        let expressoes: Vec<ExprId> = elements
            .iter()
            .filter_map(|el| match el {
                CollectionElement::Expression(x) if !mapa => Some(*x),
                CollectionElement::MapEntry { key, .. } if mapa => Some(*key),
                _ => None,
            })
            .collect();
        let mut vistos: Vec<Valor> = Vec::new();
        for x in expressoes {
            let cx = self.cx();
            let Constante::Valor(v) = self.m.avaliar(&cx, x, false) else { continue };
            if v.desconhecido_de_fato() {
                continue;
            }
            let repetido = vistos.iter().any(|y| self.m.iguais(y, &v));
            if repetido {
                let sp = self.m.span(u, x);
                self.relatar(codigo, sp, Vec::new());
            } else {
                vistos.push(v);
            }
        }
    }

    fn registrar_unico(&mut self, lv: &mut Literal, v: Valor, span: Span) {
        if v.desconhecido_de_fato() || matches!(v.estado, Estado::Null { invalido: true }) {
            return;
        }
        for (x, s) in lv.unicos.iter() {
            if self.m.iguais(x, &v) {
                let original = *s;
                if !lv.duplicados.iter().any(|(d, _)| *d == span) {
                    lv.duplicados.push((span, original));
                }
                return;
            }
        }
        lv.unicos.push((v, span));
    }

    /// `_reportNotPotentialConstants` do verificador de literais.
    fn potenciais_no_literal(&mut self, a: &ast::Ast, lv: &Literal, el: &CollectionElement) -> bool {
        let _ = a;
        let cx = self.cx();
        let mut nos: Vec<(Span, Codigo)> = Vec::new();
        let base = Self::codigo_do_literal(&lv.tipo);
        self.coletar_elemento(&cx, el, base, &mut nos);
        if nos.is_empty() {
            return true;
        }
        for (s, codigo) in nos {
            self.relatar(codigo, s, Vec::new());
        }
        false
    }

    fn coletar_elemento(&mut self, cx: &Ctx, el: &CollectionElement, base: Codigo, nos: &mut Vec<(Span, Codigo)>) {
        let mapa = base == c::NON_CONSTANT_MAP_ELEMENT;
        let um = |m: &Motor<'_>, x: ExprId, codigo: Codigo, nos: &mut Vec<(Span, Codigo)>| {
            let mut v = Vec::new();
            super::potencial::coletar_em(m, cx, x, false, true, &mut v);
            nos.extend(v.into_iter().map(|s| (s, codigo)));
        };
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) | CollectionElement::Spread { value: x, .. } => {
                um(self.m, *x, base, nos)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let (ck, cv) = if mapa { (c::NON_CONSTANT_MAP_KEY, c::NON_CONSTANT_MAP_VALUE) } else { (base, base) };
                um(self.m, *key, ck, nos);
                um(self.m, *value, cv, nos);
            }
            CollectionElement::If { condition, then, else_, .. } => {
                um(self.m, *condition, base, nos);
                self.coletar_elemento(cx, then, base, nos);
                if let Some(x) = else_ {
                    self.coletar_elemento(cx, x, base, nos);
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                nos.push((self.m.span_de_elemento(cx.unidade, el), base));
            }
        }
    }
}

enum TipoLiteral {
    Lista(TypeId),
    Conjunto(TypeId),
    Mapa(TypeId, TypeId),
}

struct Literal {
    tipo: TipoLiteral,
    unicos: Vec<(Valor, Span)>,
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

fn desparentizar(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// `hasTypeParameterReference`.
fn referencia_parametro(table: &crate::table::TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::TypeParameter { .. } | Type::Intersection { .. } => true,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|&a| referencia_parametro(table, a)),
        Type::FutureOr { arg, .. } => referencia_parametro(table, *arg),
        Type::Function { ret, positional, optional, named, .. } => {
            referencia_parametro(table, *ret)
                || positional.iter().chain(optional.iter()).any(|&a| referencia_parametro(table, a))
                || named.iter().any(|(_, a, _)| referencia_parametro(table, *a))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|&a| referencia_parametro(table, a)) || named.iter().any(|(_, a)| referencia_parametro(table, *a))
        }
        _ => false,
    }
}
