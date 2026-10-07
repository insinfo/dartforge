//! O `ConstantVisitor` e o `_InstanceCreationEvaluator` do analyzer 6.11
//! (`src/dart/constant/evaluation.dart`): avaliação de expressões
//! constantes, com os mesmos erros (`InvalidConstant`), as mesmas posições
//! e as mesmas marcas (`avoidReporting`, `isUnresolved`,
//! `isRuntimeException`).
//!
//! **Pelo lado seguro.** O que depende de informação que a inferência desta
//! passada não produziu (corpos e inicializadores de bibliotecas que não
//! foram inferidas, como as do SDK) vira valor desconhecido válido
//! (`validWithUnknownValue`): nenhum erro é provado sobre ele.

use super::valor::{texto, Campo, Estado, Excecao, Funcao, Valor};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId,
    VariableRef,
};
use dartforge_frontend::ast::{self, BinaryOp, CollectionElement, ExprId, ExprKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `InvalidConstant`.
#[derive(Clone, Debug)]
pub struct Invalida {
    pub codigo: Codigo,
    pub unidade: UnitId,
    pub span: Span,
    pub args: Vec<String>,
    pub evitar_relato: bool,
    pub nao_resolvida: bool,
    pub excecao: bool,
    /// `contextMessages`: unidade, intervalo e texto (a unidade decide o
    /// arquivo da mensagem quando o erro é relatado).
    pub contexto: Vec<(UnitId, Span, String)>,
}

/// `Constant`: um valor válido ou um erro.
#[derive(Clone, Debug)]
pub enum Constante {
    Valor(Valor),
    Invalida(Box<Invalida>),
}

impl Constante {
    pub fn valor(&self) -> Option<&Valor> {
        match self {
            Constante::Valor(v) => Some(v),
            Constante::Invalida(_) => None,
        }
    }
}

/// O estado do `ConstantVisitor`: a unidade da expressão, o ambiente léxico
/// dos parâmetros (`_lexicalEnvironment`) e o dos parâmetros de tipo
/// (`_lexicalTypeEnvironment`, que também faz a substituição da classe).
#[derive(Clone)]
pub struct Ctx {
    pub unidade: UnitId,
    pub lib: LibraryId,
    pub lexico: Option<Rc<HashMap<SymbolId, Valor>>>,
    pub tipos: Option<Rc<HashMap<TypeParamId, TypeId>>>,
    /// O relator do visitante é o do `ConstantVerifier` (e não um que
    /// descarta): os erros de `_valueOf` saem.
    pub relatar: bool,
}

impl Ctx {
    pub fn simples(unidade: UnitId, lib: LibraryId) -> Ctx {
        Ctx { unidade, lib, lexico: None, tipos: None, relatar: false }
    }
}

#[derive(Clone)]
enum EstadoVar {
    EmCurso,
    Pronta(Constante),
}

/// O motor de avaliação (`ConstantEvaluationEngine` com os resultados
/// guardados nos elementos).
pub struct Motor<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub body: &'a BodyTypes,
    /// Bibliotecas cujos corpos (e inicializadores) foram inferidos.
    pub inferidas: &'a HashSet<LibraryId>,
    vars: HashMap<VariableId, EstadoVar>,
    pilha_vars: Vec<VariableId>,
    ciclicos: HashSet<VariableId>,
    /// Valores padrão já avaliados, pela expressão.
    padroes: HashMap<(UnitId, ExprId), Constante>,
    /// Constantes locais: pelo offset do nome, a lista e o índice.
    locais_da_unidade: HashMap<UnitId, HashMap<usize, (ast::StmtId, usize)>>,
    locais: HashMap<(UnitId, usize), Option<Constante>>,
    /// Construtores em avaliação (ciclo: `isCycleFree` falso).
    construtores_em_curso: Vec<FunctionElementId>,
    /// O grafo de dependências de constantes e quem está em ciclo
    /// ([`super::ciclos::computar`], antes do verificador de cada biblioteca).
    pub grafo: super::ciclos::Estado,
    /// Argumentos posicionais diretos de uma criação constante: o
    /// `genericError` deles é `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    args_de_criacao: HashSet<(UnitId, ExprId)>,
    /// Erros relatados pelo próprio visitante (`_valueOf`) quando `relatar`.
    pub relatos: Vec<Invalida>,
    /// Quando `Some`, o verificador guarda aqui, por (unidade, offset do
    /// `switch`), as testemunhas com partes de cada `non_exhaustive_switch_*`
    /// (o `diagnostic.data` do analyzer, usado pelo `AddMissingSwitchCases`).
    pub testemunhas: Option<HashMap<(UnitId, usize), Vec<Vec<super::exaustividade::ParteDeTestemunha>>>>,
    /// Offsets dos nomes de parâmetros formais, por unidade inferida (a
    /// inferência resolve a leitura de um parâmetro como a de um local).
    parametros: HashMap<UnitId, HashSet<usize>>,
    profundidade: u32,
    sym_identical: Option<SymbolId>,
    sym_length: Option<SymbolId>,
    sym_from_environment: Option<SymbolId>,
    sym_has_environment: Option<SymbolId>,
    sym_default_value: Option<SymbolId>,
}

type R = Constante;

impl<'a> Motor<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn novo(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        body: &'a BodyTypes,
        inferidas: &'a HashSet<LibraryId>,
    ) -> Self {
        let mut parametros: HashMap<UnitId, HashSet<usize>> = HashMap::new();
        for (i, u) in program.units.iter().enumerate() {
            if inferidas.contains(&u.library) {
                parametros.insert(UnitId(i as u32), offsets_de_parametros(&u.ast));
            }
        }
        Motor {
            program,
            interner,
            table,
            core,
            outline,
            body,
            inferidas,
            vars: HashMap::new(),
            pilha_vars: Vec::new(),
            ciclicos: HashSet::new(),
            padroes: HashMap::new(),
            locais_da_unidade: HashMap::new(),
            locais: HashMap::new(),
            construtores_em_curso: Vec::new(),
            grafo: super::ciclos::Estado::default(),
            args_de_criacao: HashSet::new(),
            relatos: Vec::new(),
            testemunhas: None,
            parametros,
            profundidade: 0,
            sym_identical: interner.lookup("identical"),
            sym_length: interner.lookup("length"),
            sym_from_environment: interner.lookup("fromEnvironment"),
            sym_has_environment: interner.lookup("hasEnvironment"),
            sym_default_value: interner.lookup("defaultValue"),
        }
    }

    // -- Acesso --------------------------------------------------------------

    pub fn ast(&self, u: UnitId) -> &'a ast::Ast {
        &self.program.unit(u).ast
    }

    pub fn span(&self, u: UnitId, e: ExprId) -> Span {
        self.ast(u).expr(e).span
    }

    fn inferida(&self, u: UnitId) -> bool {
        self.inferidas.contains(&self.program.unit(u).library)
    }

    /// Tipo estático de `e` (`dynamic` numa unidade não inferida).
    pub fn estatico(&self, u: UnitId, e: ExprId) -> TypeId {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_type(e)).unwrap_or(self.core.dynamic_)
    }

    pub fn resolvido(&self, u: UnitId, e: ExprId) -> Option<&'a Resolved> {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_resolved(e))
    }

    /// O tipo estático de `e` é o de recuperação (`InvalidType`): o tipo
    /// registrado inválido, ou o local de inicializador inválido.
    fn tipo_invalido(&self, u: UnitId, e: ExprId) -> bool {
        self.body.units.get(u.0 as usize).is_some_and(|b| {
            b.tipos_invalidos.contains(&e) || b.get_type(e).is_some_and(|t| self.table.e_invalido(t))
        })
    }

    pub fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    pub fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// Sem o alias (`getDisplayString()`): onde o analyzer já passa texto.
    pub fn formatar_sem_alias(&self, t: TypeId) -> String {
        self.table.format_sem_alias(t, self.interner, self.program)
    }

    /// `extensionTypeErasure`.
    pub fn apagar(&mut self, t: TypeId) -> TypeId {
        let outline = self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &mapa, table))
        };
        crate::ops::erase_extension_type(t, self.table, &rep)
    }

    /// `runtimeTypeMatch`: o tipo do objeto é subtipo do tipo apagado. Um
    /// objeto de tipo `dynamic` (expressão não resolvida) casa com tudo.
    pub fn casa(&mut self, v: &Valor, t: TypeId) -> bool {
        if v.tipo == self.core.dynamic_ || matches!(v.estado, Estado::Null { invalido: true }) {
            return true;
        }
        let t = self.apagar(t);
        self.sub(v.tipo, t)
    }

    /// `isAssignableTo` do tipo estático.
    pub fn atribuivel(&mut self, a: TypeId, b: TypeId) -> bool {
        a == self.core.dynamic_ || self.sub(a, b)
    }

    fn valor(&mut self, tipo: TypeId, estado: Estado) -> Valor {
        let tipo = self.apagar(tipo);
        Valor::novo(tipo, estado)
    }

    pub fn desconhecido(&mut self, tipo: TypeId) -> Valor {
        let tipo = self.apagar(tipo);
        let elemento = match self.table.get(tipo) {
            Type::Interface { class, args, .. } if Some(*class) == self.core.list_class && args.len() == 1 => Some(args[0]),
            _ => None,
        };
        Valor::desconhecido(self.table, self.core, tipo, elemento)
    }

    fn interface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(class) => self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    // -- Erros ---------------------------------------------------------------

    pub fn erro(&self, u: UnitId, span: Span, codigo: Codigo) -> Invalida {
        Invalida { codigo, unidade: u, span, args: Vec::new(), evitar_relato: false, nao_resolvida: false, excecao: false, contexto: Vec::new() }
    }

    /// `ConstructorElement.displayName`: `C` ou `C.nome`.
    fn exibicao_do_construtor(&self, f: FunctionElementId) -> String {
        let fe = self.program.function(f);
        let classe = fe.class.map(|k| self.interner.resolve(self.program.class(k).name).to_string()).unwrap_or_default();
        let nome = self.interner.resolve(fe.name);
        if nome.is_empty() || nome == "new" {
            classe
        } else {
            format!("{classe}.{nome}")
        }
    }

    /// `_constructor.source`: a unidade da declaração do construtor (a da
    /// classe, no sintético).
    fn unidade_do_construtor(&self, f: FunctionElementId) -> Option<UnitId> {
        let fe = self.program.function(f);
        match fe.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => fe.class.and_then(|k| self.program.class(k).decl).map(|d| d.unit),
        }
    }

    /// `_checkInitializers`: o erro (que não é de execução) no inicializador
    /// passa ao nó da criação; sem contexto ainda, ganha o de onde ocorre.
    fn erro_no_inicializador(&self, i: &Invalida, f: Option<FunctionElementId>, onde: &str, erro: ErroEm) -> Invalida {
        let mut n = i.clone();
        if n.contexto.is_empty() {
            if let Some(f) = f {
                if let Some(u) = self.unidade_do_construtor(f) {
                    let texto = format!("The error is in the {onde} of '{}', and occurs here.", self.exibicao_do_construtor(f));
                    n.contexto.push((u, i.span, texto));
                }
            }
        }
        n.unidade = erro.unidade;
        n.span = erro.span;
        n
    }

    /// `_stackTraceContextMessage`: no nome do construtor que chama (nenhum
    /// no sintético, cujo `nameOffset` é -1).
    fn pilha_de_construtores(&self, super_: FunctionElementId, f: FunctionElementId) -> Option<(UnitId, Span, String)> {
        if !matches!(self.program.function(f).node, FunctionRef::Constructor { .. }) {
            return None;
        }
        let (u, span) = self.program.nome_nao_sintetico_da_funcao(f)?;
        let quem = self.exibicao_do_construtor(f);
        let texto = format!("The evaluated constructor '{}' is called by '{quem}' and '{quem}' is defined here.", self.exibicao_do_construtor(super_));
        Some((u, span, texto))
    }

    fn inv(&self, u: UnitId, e: ExprId, codigo: Codigo) -> R {
        Constante::Invalida(Box::new(self.erro(u, self.span(u, e), codigo)))
    }

    /// `InvalidConstant.genericError`.
    fn generico(&self, u: UnitId, e: ExprId, nao_resolvida: bool) -> R {
        let codigo = if self.args_de_criacao.contains(&(u, e)) { c::CONST_WITH_NON_CONSTANT_ARGUMENT } else { c::INVALID_CONSTANT };
        let mut i = self.erro(u, self.span(u, e), codigo);
        i.nao_resolvida = nao_resolvida;
        Constante::Invalida(Box::new(i))
    }

    fn excecao(&self, u: UnitId, e: ExprId, x: Excecao) -> R {
        let mut i = self.erro(u, self.span(u, e), x.codigo);
        i.excecao = x.de_execucao;
        Constante::Invalida(Box::new(i))
    }

    // -- Expressões ----------------------------------------------------------

    /// `evaluateConstant`: `em_const` diz se o nó está num contexto
    /// constante (`inConstantContext`).
    pub fn avaliar(&mut self, cx: &Ctx, e: ExprId, em_const: bool) -> R {
        if self.profundidade > 400 {
            return self.generico(cx.unidade, e, true);
        }
        self.profundidade += 1;
        let r = self.avaliar_no(cx, e, em_const);
        self.profundidade -= 1;
        self.instanciar_referencia(cx, e, r)
    }

    /// `visitFunctionReference` sem argumentos escritos (3.6.2
    /// `evaluation.dart:852-882`): a instanciação implícita de qualquer
    /// expressão (`instanciacao_de_tearoff`). Argumento inferido que menciona
    /// parâmetro de tipo é `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`; senão
    /// `_instantiateFunctionType` (`:1939-1959`): o valor cuja função é
    /// genérica (o tipo **do elemento**) fica com esse tipo instanciado pelos
    /// argumentos e depois substituído pelo ambiente do construtor.
    fn instanciar_referencia(&mut self, cx: &Ctx, e: ExprId, r: R) -> R {
        let Constante::Valor(mut val) = r else { return r };
        let u = cx.unidade;
        // O tear-off de construtor é `ConstructorReference`
        // (`visitConstructorReference`), não `FunctionReference`.
        if matches!(self.resolvido(u, e), Some(Resolved::Constructor(_))) {
            return Constante::Valor(val);
        }
        let Some(args) = self.body.units.get(u.0 as usize).and_then(|b| b.instanciacao_de_tearoff(e)).map(|a| a.to_vec()) else {
            return Constante::Valor(val);
        };
        if self.instanciacao_com_parametro(cx, e) {
            return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
        }
        let Estado::Funcao { elemento: Funcao::Elemento(f), .. } = val.estado else { return Constante::Valor(val) };
        let Some(sig) = self.outline.functions.get(f.0 as usize).map(|d| d.signature) else { return Constante::Valor(val) };
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.table.get(sig).clone() else {
            return Constante::Valor(val);
        };
        if type_params.is_empty() || args.is_empty() || type_params.len() != args.len() {
            return Constante::Valor(val);
        }
        let sem = self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable });
        let m: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(args.iter().copied()).collect();
        let t = crate::ops::substitute(sem, &m, self.table);
        val.tipo = match &cx.tipos {
            Some(m) => crate::ops::substitute(t, m, self.table),
            None => t,
        };
        Constante::Valor(val)
    }

    fn avaliar_no(&mut self, cx: &Ctx, e: ExprId, em_const: bool) -> R {
        let u = cx.unidade;
        let a = self.ast(u);
        match &a.expr(e).kind {
            ExprKind::Int(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                let v = crate::constant::inteiro_literal(t);
                if self.estatico(u, e) == self.core.double {
                    let d = v.map(|i| i as f64).or_else(|| t.replace('_', "").parse::<f64>().ok());
                    Constante::Valor(Valor::novo(self.core.double, Estado::Double(d)))
                } else {
                    Constante::Valor(Valor::novo(self.core.int, Estado::Int(v)))
                }
            }
            ExprKind::Double(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                Constante::Valor(Valor::novo(self.core.double, Estado::Double(crate::constant::real_literal(t))))
            }
            ExprKind::Bool(b) => Constante::Valor(Valor::bool_(self.core, Some(*b))),
            ExprKind::Null => Constante::Valor(Valor::nulo(self.core)),
            ExprKind::String(lit) => self.string(cx, e, lit, em_const),
            ExprKind::Symbol(partes) => {
                let s: Vec<&str> = partes.iter().map(|n| self.interner.resolve(n.sym)).collect();
                Constante::Valor(Valor::novo(self.core.symbol, Estado::Simbolo(Some(s.join(".").into()))))
            }
            ExprKind::Identifier(n) => {
                if let Some(v) = cx.lexico.as_ref().and_then(|l| l.get(&n.sym)) {
                    return Constante::Valor(v.clone());
                }
                self.valor_constante(cx, e, e, true)
            }
            ExprKind::Parenthesized(x) => self.avaliar(cx, *x, em_const),
            ExprKind::List { const_, elements, .. } => {
                if !(*const_ || em_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_LIST_LITERAL);
                }
                let tipo = self.estatico(u, e);
                let elemento = match self.table.get(tipo) {
                    Type::Interface { args, .. } if args.len() == 1 => args[0],
                    _ => self.core.dynamic_,
                };
                let elemento = self.apagar(elemento);
                let tipo_lista = self.interface(self.core.list_class, vec![elemento]);
                let mut lista = Vec::new();
                if let Some(i) = self.construir_lista(cx, &mut lista, elements, true) {
                    return match i {
                        Ok(()) => Constante::Valor(self.desconhecido(tipo_lista)),
                        Err(i) => Constante::Invalida(i),
                    };
                }
                Constante::Valor(self.valor(tipo_lista, Estado::Lista { elemento, elementos: Rc::new(lista), desconhecida: false }))
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                let tipo = self.estatico(u, e);
                let (classe, args) = match self.table.get(tipo) {
                    Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                    _ => (None, Vec::new()),
                };
                let e_conjunto = classe.is_some() && classe == self.core.set_class;
                let e_mapa = classe.is_some() && classe == self.core.map_class;
                if !e_conjunto {
                    if !(*const_ || em_const) {
                        return self.inv(u, e, c::MISSING_CONST_IN_MAP_LITERAL);
                    }
                    let (k, v) = if args.len() >= 2 { (args[0], args[1]) } else { (self.core.dynamic_, self.core.dynamic_) };
                    let tipo_mapa = self.interface(self.core.map_class, vec![k, v]);
                    let mut mapa = Vec::new();
                    return match self.construir_mapa(cx, &mut mapa, elements) {
                        Some(Ok(())) => Constante::Valor(self.desconhecido(tipo_mapa)),
                        Some(Err(mut i)) => {
                            if !e_mapa {
                                i.evitar_relato = true;
                            }
                            Constante::Invalida(i)
                        }
                        None => Constante::Valor(self.valor(tipo_mapa, Estado::Mapa { entradas: Rc::new(mapa), desconhecido: false })),
                    };
                }
                if !(*const_ || em_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_SET_LITERAL);
                }
                let el = args.first().copied().unwrap_or(self.core.dynamic_);
                let tipo_conj = self.interface(self.core.set_class, vec![el]);
                let mut conj = Vec::new();
                match self.construir_conjunto(cx, &mut conj, elements) {
                    Some(Ok(())) => Constante::Valor(self.desconhecido(tipo_conj)),
                    Some(Err(i)) => Constante::Invalida(i),
                    None => Constante::Valor(self.valor(tipo_conj, Estado::Conjunto { elementos: Rc::new(conj), desconhecido: false })),
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let filho_const = *const_ || em_const;
                let mut pos = Vec::new();
                for &x in positional.iter() {
                    match self.avaliar(cx, x, filho_const) {
                        Constante::Valor(v) => pos.push(v),
                        i => return i,
                    }
                }
                let mut nom = Vec::new();
                for (n, x) in named.iter() {
                    match self.avaliar(cx, *x, filho_const) {
                        Constante::Valor(v) => nom.push((n.sym, v)),
                        i => return i,
                    }
                }
                let mut nomeados_t: Vec<(SymbolId, TypeId)> = nom.iter().map(|(n, v)| (*n, v.tipo)).collect();
                nomeados_t.sort_by_key(|(n, _)| *n);
                let tipo = self.table.intern(Type::Record {
                    positional: pos.iter().map(|v| v.tipo).collect(),
                    named: nomeados_t.into_boxed_slice(),
                    nullable: false,
                });
                Constante::Valor(Valor::novo(tipo, Estado::Registro { posicionais: Rc::new(pos), nomeados: Rc::new(nom) }))
            }
            ExprKind::InstanceCreation { keyword, arguments, constructor, .. } => {
                if !matches!(keyword, Some(ast::CreationKeyword::Const)) && !(keyword.is_none() && em_const) {
                    return self.generico(u, e, false);
                }
                let Some(Resolved::Constructor(f)) = self.resolvido(u, e) else {
                    // O construtor encaminhado de uma aplicação de mixin.
                    if let Some((f, tipo)) = self.construtor_encaminhado(u, e, constructor.map(|n| n.sym)) {
                        let kw = self.palavra_de_criacao(u, e, keyword.is_some());
                        let r = self.chamar_construtor(cx, e, f, arguments, kw, Some(tipo));
                        return self.formatar_erro_de_construtor(u, e, r);
                    }
                    return self.inv(u, e, c::INVALID_CONSTANT);
                };
                let kw = self.palavra_de_criacao(u, e, keyword.is_some());
                let r = self.chamar_construtor(cx, e, *f, arguments, kw, None);
                self.formatar_erro_de_construtor(u, e, r)
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = self.resolvido(u, e) {
                    // Criação implícita: constante só num contexto constante.
                    if !em_const {
                        return self.generico(u, e, false);
                    }
                    let f = *f;
                    let r = self.chamar_construtor(cx, e, f, arguments, None, None);
                    return self.formatar_erro_de_construtor(u, e, r);
                }
                let alvo = *target;
                // O construtor primário de um tipo de extensão (`A(true)`,
                // `A.n(1)`), que o modelo não guarda como função: a criação
                // vale a representação (`_fieldMap[representation]`, 3.6.2
                // `evaluation.dart:2573-2578`), o argumento avaliado.
                if let Some(r) = self.criacao_por_primario_de_extensao(cx, e, alvo, arguments, em_const) {
                    return r;
                }
                let e_metodo = matches!(a.expr(alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. });
                if !e_metodo {
                    return self.generico(u, e, false);
                }
                // `identical(a, b)` de `dart:core`.
                if let ExprKind::Identifier(n) = &a.expr(alvo).kind {
                    if Some(n.sym) == self.sym_identical && arguments.args.len() == 2 && arguments.args.iter().all(|x| x.name.is_none()) {
                        if let Some(Resolved::Element(Element::Function(f))) = self.resolvido(u, alvo) {
                            if Some(self.program.function(*f).library) == self.core.core_library {
                                let (x, y) = (arguments.args[0].value, arguments.args[1].value);
                                let l = match self.avaliar(cx, x, em_const) {
                                    Constante::Valor(v) => v,
                                    i => return i,
                                };
                                let r = match self.avaliar(cx, y, em_const) {
                                    Constante::Valor(v) => v,
                                    i => return i,
                                };
                                let b = self.identico(&l, &r);
                                return Constante::Valor(Valor::bool_(self.core, b));
                            }
                        }
                    }
                }
                if self.tipo_invalido(u, e) {
                    let mut i = self.erro(u, self.span(u, e), c::INVALID_CONSTANT);
                    i.nao_resolvida = true;
                    return Constante::Invalida(Box::new(i));
                }
                self.inv(u, e, c::CONST_EVAL_METHOD_INVOCATION)
            }
            ExprKind::Property { target, name, .. } => self.propriedade(cx, e, *target, *name, em_const),
            ExprKind::TypeArguments { target, .. } => {
                // Literal de tipo genérico ou instanciação de função: o
                // valor exato do tipo não é reconstruído aqui.
                match self.avaliar(cx, *target, em_const) {
                    Constante::Valor(v) => match v.estado {
                        Estado::Tipo(_) => Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(None))),
                        Estado::Funcao { elemento, .. } => {
                            // `_instantiateFunctionType`: o tipo instanciado
                            // passa pela substituição do construtor.
                            let t = self.estatico(u, e);
                            let t = match &cx.tipos {
                                Some(m) => crate::ops::substitute(t, m, self.table),
                                None => t,
                            };
                            Constante::Valor(self.valor(t, Estado::Funcao { elemento, args: None }))
                        }
                        _ => self.inv(u, e, c::INVALID_CONSTANT),
                    },
                    i => i,
                }
            }
            ExprKind::Unary { op, operand } => self.unario(cx, e, *op, *operand, em_const),
            ExprKind::Binary { op, left, right } => self.binario(cx, e, *op, *left, *right, em_const),
            ExprKind::Conditional { condition, then, else_ } => {
                let cond = match self.avaliar(cx, *condition, em_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                if !cond.estado.e_bool() {
                    return self.inv(u, *condition, c::CONST_EVAL_TYPE_BOOL);
                }
                match cond.como_bool() {
                    Some(true) => {
                        if let Some(i) = self.nao_potencialmente_constante(cx, *else_) {
                            return i;
                        }
                        self.avaliar(cx, *then, em_const)
                    }
                    Some(false) => {
                        if let Some(i) = self.nao_potencialmente_constante(cx, *then) {
                            return i;
                        }
                        self.avaliar(cx, *else_, em_const)
                    }
                    None => {
                        if let i @ Constante::Invalida(_) = self.avaliar(cx, *then, em_const) {
                            return i;
                        }
                        if let i @ Constante::Invalida(_) = self.avaliar(cx, *else_, em_const) {
                            return i;
                        }
                        let t = self.estatico(u, e);
                        Constante::Valor(self.desconhecido(t))
                    }
                }
            }
            ExprKind::Is { value, negated, .. } => {
                let v = match self.avaliar(cx, *value, em_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                // O tipo testado não é reconstruído aqui: o resultado é
                // desconhecido (nunca prova erro).
                let _ = (v, negated);
                Constante::Valor(Valor::bool_(self.core, None))
            }
            ExprKind::As { value, ty } => {
                let ty = *ty;
                let v = match self.avaliar(cx, *value, em_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                // `node.type.type` (`visitAsExpression`): o tipo escrito, não
                // o estático do `as` (que a instanciação implícita muda).
                let alvo = self
                    .body
                    .units
                    .get(u.0 as usize)
                    .and_then(|b| b.tipos_de_anotacoes.get(&ty).copied())
                    .or_else(|| self.outline.tipos_escritos.get(&(u, ty)).copied())
                    .unwrap_or_else(|| self.estatico(u, e));
                // O tipo do `as` com o ambiente léxico do construtor aplicado
                // (`_substitution`, `x as List<T>` em `C<int>`): só o que
                // ainda menciona parâmetro de tipo fica sem conferir.
                let alvo = match &cx.tipos {
                    Some(m) => crate::ops::substitute(alvo, m, self.table),
                    None => alvo,
                };
                if v.tipo == self.core.dynamic_ || alvo == self.core.dynamic_ || self.menciona_parametro(alvo) {
                    return Constante::Valor(v);
                }
                let alvo = self.apagar(alvo);
                if !self.sub(v.tipo, alvo) {
                    return self.inv(u, e, c::CONST_EVAL_THROWS_EXCEPTION);
                }
                Constante::Valor(v)
            }
            ExprKind::DotShorthand { .. } => {
                // Atalho resolvido para um membro estático ou constante de enum.
                match self.resolvido(u, e) {
                    Some(Resolved::Element(Element::Class(d))) => {
                        let d = *d;
                        let ExprKind::DotShorthand { name, .. } = &a.expr(e).kind else { unreachable!() };
                        match self.membro_estatico_variavel(d, name.sym) {
                            Some(v) => self.valor_de_referencia_a_variavel(cx, e, v),
                            None => self.generico(u, e, false),
                        }
                    }
                    _ => self.generico(u, e, true),
                }
            }
            _ => self.generico(u, e, false),
        }
    }

    pub(crate) fn menciona_parametro(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => true,
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| self.menciona_parametro(*a)),
            Type::FutureOr { arg, .. } => self.menciona_parametro(*arg),
            Type::Function { ret, positional, optional, named, .. } => {
                self.menciona_parametro(*ret)
                    || positional.iter().chain(optional.iter()).any(|a| self.menciona_parametro(*a))
                    || named.iter().any(|(_, a, _)| self.menciona_parametro(*a))
            }
            Type::Record { positional, named, .. } => {
                positional.iter().any(|a| self.menciona_parametro(*a)) || named.iter().any(|(_, a)| self.menciona_parametro(*a))
            }
            _ => false,
        }
    }

    /// O span da palavra-chave `new`/`const` de uma criação explícita.
    fn palavra_de_criacao(&self, u: UnitId, e: ExprId, tem: bool) -> Option<Span> {
        if !tem {
            return None;
        }
        let s = self.span(u, e);
        let fonte = &self.program.unit(u).source;
        let texto = fonte.get(s.start..s.end)?;
        let n = texto.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(texto.len());
        Some(Span { start: s.start, end: s.start + n })
    }

    /// `evaluateAndFormatErrorsInConstructorCall`: uma exceção de execução
    /// vira `CONST_EVAL_THROWS_EXCEPTION` na criação.
    pub fn formatar_erro_de_construtor(&self, u: UnitId, e: ExprId, r: R) -> R {
        match r {
            Constante::Invalida(i) if i.excecao => {
                let mut n = self.erro(u, self.span(u, e), c::CONST_EVAL_THROWS_EXCEPTION);
                n.evitar_relato = i.evitar_relato;
                // O contexto da exceção, no arquivo da biblioteca
                // (`library.source`), depois dos que o erro já tinha.
                let texto = dartforge_diagnostics::Diagnostic::com_codigo(i.codigo, i.span, i.args.iter().map(|s| s.as_str())).message;
                let definidora = self.program.library(self.program.unit(u).library).units.first().copied().unwrap_or(u);
                n.contexto = i.contexto.clone();
                n.contexto.push((definidora, i.span, format!("The exception is '{texto}' and occurs here.")));
                Constante::Invalida(Box::new(n))
            }
            r => r,
        }
    }

    // -- Strings ---------------------------------------------------------------

    fn string(&mut self, cx: &Ctx, e: ExprId, lit: &ast::StringLit, em_const: bool) -> R {
        let u = cx.unidade;
        let mut acc: Vec<u16> = Vec::new();
        let mut desconhecida = false;
        for p in lit.parts.iter() {
            match p {
                ast::StringPart::Text(t) => acc.extend(t.code_units()),
                ast::StringPart::Interpolation(x) => {
                    let v = match self.avaliar(cx, *x, em_const) {
                        Constante::Valor(v) => v,
                        i => return i,
                    };
                    if !v.estado.e_bool_num_string_ou_nulo() {
                        let s = self.span_de_interpolacao(u, *x);
                        return Constante::Invalida(Box::new(self.erro(u, s, c::CONST_EVAL_TYPE_BOOL_NUM_STRING)));
                    }
                    match v.estado.para_texto(&|_| String::new(), &|_| String::new()) {
                        Ok(Estado::Str(Some(s))) => acc.extend(s.iter()),
                        Ok(_) => desconhecida = true,
                        Err(x) => return self.excecao(u, e, x),
                    }
                }
            }
        }
        let st = if desconhecida { None } else { Some(acc.into()) };
        Constante::Valor(Valor::novo(self.core.string, Estado::Str(st)))
    }

    /// O intervalo do `InterpolationExpression`: `$x` ou `${e}`.
    fn span_de_interpolacao(&self, u: UnitId, x: ExprId) -> Span {
        let s = self.span(u, x);
        let b = self.program.unit(u).source.as_bytes();
        if s.start >= 1 && b[s.start - 1] == b'$' {
            return Span { start: s.start - 1, end: s.end };
        }
        let mut i = s.start;
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        if i >= 2 && &b[i - 2..i] == b"${" {
            let mut j = s.end;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let fim = if j < b.len() && b[j] == b'}' { j + 1 } else { s.end };
            return Span { start: i - 2, end: fim };
        }
        s
    }

    // -- Coleções --------------------------------------------------------------

    /// `_buildListConstant`: `None` completa; `Some(Ok(()))` desconhecida
    /// (condição de `if` desconhecida); `Some(Err)` erro.
    fn construir_lista(
        &mut self,
        cx: &Ctx,
        lista: &mut Vec<Valor>,
        elementos: &[CollectionElement],
        filho_const: bool,
    ) -> Option<Result<(), Box<Invalida>>> {
        let u = cx.unidade;
        for el in elementos {
            match el {
                CollectionElement::Expression(x) => match self.avaliar(cx, *x, filho_const) {
                    Constante::Valor(v) => lista.push(v),
                    Constante::Invalida(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, filho_const) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if cond.desconhecido_de_fato() {
                        return Some(Ok(()));
                    }
                    match cond.como_bool() {
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION))));
                        }
                        Some(true) => {
                            if let Some(Err(i)) = self.construir_lista(cx, lista, std::slice::from_ref(then), filho_const) {
                                return Some(Err(i));
                            }
                        }
                        Some(false) => {
                            if let Some(el) = else_ {
                                if let Some(Err(i)) = self.construir_lista(cx, lista, std::slice::from_ref(el), filho_const) {
                                    return Some(Err(i));
                                }
                            }
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, filho_const) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if v.estado.e_nulo() && *null_aware {
                        continue;
                    }
                    match v.como_lista().or_else(|| v.como_conjunto()) {
                        Some(xs) => lista.extend(xs.iter().cloned()),
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
                CollectionElement::NullAwareExpression(x) => {
                    let v = match self.avaliar(cx, *x, filho_const) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if v.estado.e_nulo() {
                        continue;
                    }
                    lista.push(v);
                }
            }
        }
        None
    }

    fn construir_conjunto(&mut self, cx: &Ctx, conj: &mut Vec<Valor>, elementos: &[CollectionElement]) -> Option<Result<(), Box<Invalida>>> {
        let u = cx.unidade;
        for el in elementos {
            match el {
                CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => match self.avaliar(cx, *x, true) {
                    Constante::Valor(v) => {
                        if matches!(el, CollectionElement::NullAwareExpression(_)) && v.estado.e_nulo() {
                            continue;
                        }
                        self.inserir_no_conjunto(conj, v);
                    }
                    Constante::Invalida(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if cond.desconhecido_de_fato() {
                        return Some(Ok(()));
                    }
                    let ramo = match cond.como_bool() {
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = ramo {
                        if let Some(Err(i)) = self.construir_conjunto(cx, conj, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if v.estado.e_nulo() && *null_aware {
                        continue;
                    }
                    match v.como_conjunto().or_else(|| v.como_lista()) {
                        Some(xs) => {
                            for x in xs.iter() {
                                self.inserir_no_conjunto(conj, x.clone());
                            }
                        }
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
            }
        }
        None
    }

    fn inserir_no_conjunto(&mut self, conj: &mut Vec<Valor>, v: Valor) {
        if !conj.iter().any(|x| self.iguais(x, &v)) {
            conj.push(v);
        }
    }

    fn construir_mapa(&mut self, cx: &Ctx, mapa: &mut Vec<(Valor, Valor)>, elementos: &[CollectionElement]) -> Option<Result<(), Box<Invalida>>> {
        let u = cx.unidade;
        for el in elementos {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::EXPRESSION_IN_MAP))));
                }
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.span_de_elemento(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if cond.desconhecido_de_fato() {
                        return Some(Ok(()));
                    }
                    let ramo = match cond.como_bool() {
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = ramo {
                        if let Some(Err(i)) = self.construir_mapa(cx, mapa, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
                    let k = self.avaliar(cx, *key, true);
                    let v = self.avaliar(cx, *value, true);
                    let k = match k {
                        Constante::Valor(k) => k,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    let v = match v {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if (*null_aware_key && k.estado.e_nulo()) || (*null_aware_value && v.estado.e_nulo()) {
                        continue;
                    }
                    if let Some(p) = mapa.iter().position(|(x, _)| self.iguais(x, &k)) {
                        mapa[p].1 = v;
                    } else {
                        mapa.push((k, v));
                    }
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalida(i) => return Some(Err(i)),
                    };
                    if v.estado.e_nulo() && *null_aware {
                        continue;
                    }
                    match v.como_mapa() {
                        Some(xs) => {
                            for (k, x) in xs.iter() {
                                if let Some(p) = mapa.iter().position(|(y, _)| self.iguais(y, k)) {
                                    mapa[p].1 = x.clone();
                                } else {
                                    mapa.push((k.clone(), x.clone()));
                                }
                            }
                        }
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_MAP)))),
                    }
                }
            }
        }
        None
    }

    /// O intervalo de um elemento de coleção.
    pub fn span_de_elemento(&self, u: UnitId, el: &CollectionElement) -> Span {
        let a = self.ast(u);
        let fonte = self.program.unit(u).source.as_bytes();
        match el {
            CollectionElement::Expression(x) => a.expr(*x).span,
            CollectionElement::NullAwareExpression(x) => {
                let s = a.expr(*x).span;
                Span { start: recuar_ate(fonte, s.start, b'?'), end: s.end }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let (k, v) = (a.expr(*key).span, a.expr(*value).span);
                Span { start: recuar_ate(fonte, k.start, b'?'), end: v.end }
            }
            CollectionElement::Spread { value, .. } => {
                let s = a.expr(*value).span;
                Span { start: recuar_palavra(fonte, s.start, &["...?", "..."]), end: s.end }
            }
            CollectionElement::If { condition, then, else_, .. } => {
                let s = a.expr(*condition).span;
                let fim = else_.as_ref().map(|x| self.span_de_elemento(u, x)).unwrap_or_else(|| self.span_de_elemento(u, then)).end;
                Span { start: recuar_palavra(fonte, s.start, &["if"]), end: fim }
            }
            CollectionElement::For { body, init, condition, updates, .. } => {
                let fim = self.span_de_elemento(u, body).end;
                let primeiro = condition
                    .map(|x| a.expr(x).span.start)
                    .or_else(|| updates.first().map(|x| a.expr(*x).span.start))
                    .unwrap_or(fim);
                let _ = init;
                Span { start: inicio_de_for(fonte, primeiro), end: fim }
            }
            CollectionElement::ForIn { body, iterable, .. } => {
                let fim = self.span_de_elemento(u, body).end;
                Span { start: inicio_de_for(fonte, a.expr(*iterable).span.start), end: fim }
            }
        }
    }

    // -- Operadores ------------------------------------------------------------

    fn e_de_extensao(&self, u: UnitId, e: ExprId) -> Option<Codigo> {
        match self.resolvido(u, e)? {
            Resolved::ExtensionMember { .. } => Some(c::CONST_EVAL_EXTENSION_METHOD),
            Resolved::Member { class, .. } if self.program.class(*class).kind == ClassKind::ExtensionType => {
                Some(c::CONST_EVAL_EXTENSION_TYPE_METHOD)
            }
            _ => None,
        }
    }

    fn unario(&mut self, cx: &Ctx, e: ExprId, op: UnaryOp, operand: ExprId, em_const: bool) -> R {
        let u = cx.unidade;
        if let Some(codigo) = self.e_de_extensao(u, e) {
            return self.inv(u, e, codigo);
        }
        // Pós-fixos (`a++`, `a!`) não têm visita própria (`visitNode`): o
        // erro genérico no nó. `visitPrefixExpression`: o operando primeiro
        // (o erro dele vence); só então `++`/`--` são o erro genérico.
        if matches!(op, UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert) {
            return self.generico(u, e, false);
        }
        let v = match self.avaliar(cx, operand, em_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        if !matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) {
            return self.generico(u, e, false);
        }
        let r = match op {
            UnaryOp::Not => v.estado.nao_logico().map(|s| (self.core.bool_, s)),
            UnaryOp::BitNot => v.estado.negar_bits().map(|s| (self.core.int, s)),
            _ => v.estado.negar().map(|s| {
                let t = if matches!(s, Estado::Double(_)) { self.core.double } else { self.core.int };
                (t, s)
            }),
        };
        match r {
            Ok((t, s)) => Constante::Valor(Valor::novo(t, s)),
            Err(x) => self.excecao(u, e, x),
        }
    }

    fn binario(&mut self, cx: &Ctx, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, em_const: bool) -> R {
        let u = cx.unidade;
        if let Some(codigo) = self.e_de_extensao(u, e) {
            return self.inv(u, e, codigo);
        }
        let l = match self.avaliar(cx, left, em_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let curto = if op == BinaryOp::And { Some(false) } else { Some(true) };
                if l.como_bool() == curto {
                    if let Some(i) = self.nao_potencialmente_constante(cx, right) {
                        return i;
                    }
                }
                // `lazyAnd`/`lazyOr`.
                let r = (|| -> Result<Estado, Excecao> {
                    match &l.estado {
                        Estado::Bool(v) => {
                            if *v == curto {
                                return Ok(Estado::Bool(*v));
                            }
                            let d = match self.avaliar(cx, right, em_const) {
                                Constante::Valor(d) => d,
                                Constante::Invalida(i) => return Err(Excecao::nova(i.codigo)),
                            };
                            if !d.estado.e_bool() {
                                return Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL));
                            }
                            if v.is_none() {
                                return Ok(Estado::Bool(None));
                            }
                            d.estado.para_bool()
                        }
                        _ => Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL)),
                    }
                })();
                return match r {
                    Ok(s) => Constante::Valor(Valor::novo(self.core.bool_, s)),
                    Err(x) => self.excecao(u, e, x),
                };
            }
            BinaryOp::IfNull => {
                if !l.estado.e_nulo() {
                    if let Some(i) = self.nao_potencialmente_constante(cx, right) {
                        return i;
                    }
                    return Constante::Valor(l);
                }
                return self.avaliar(cx, right, em_const);
            }
            _ => {}
        }
        let r = match self.avaliar(cx, right, em_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        let res: Result<(TypeId, Estado), Excecao> = match op {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                if l.estado.e_bool() && r.estado.e_bool() {
                    let s = match op {
                        BinaryOp::BitAnd => l.estado.e_logico(&r.estado),
                        BinaryOp::BitOr => l.estado.ou_logico(&r.estado),
                        _ => l.estado.xor_logico(&r.estado),
                    };
                    s.map(|s| (self.core.bool_, s))
                } else if l.estado.e_int() && r.estado.e_int() {
                    let s = match op {
                        BinaryOp::BitAnd => l.estado.e_bit(&r.estado),
                        BinaryOp::BitOr => l.estado.ou_bit(&r.estado),
                        _ => l.estado.xor_bit(&r.estado),
                    };
                    s.map(|s| (self.core.int, s))
                } else {
                    Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL_INT))
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let lib = cx.lib;
                self.igual_igual(lib, &l, &r).map(|b| {
                    let b = if op == BinaryOp::NotEq { b.map(|x| !x) } else { b };
                    (self.core.bool_, Estado::Bool(b))
                })
            }
            BinaryOp::Gt => l.estado.maior(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::GtEq => l.estado.maior_ou_igual(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::Lt => l.estado.menor(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::LtEq => l.estado.menor_ou_igual(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::Shl => l.estado.desloca_esquerda(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::Shr => l.estado.desloca_direita(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::UShr => l.estado.desloca_direita_logico(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::TruncDiv => l.estado.dividir_inteiro(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let s = match op {
                    BinaryOp::Add => l.estado.somar(&r.estado),
                    BinaryOp::Sub => l.estado.subtrair(&r.estado),
                    BinaryOp::Mul => l.estado.multiplicar(&r.estado),
                    BinaryOp::Div => l.estado.dividir(&r.estado),
                    _ => l.estado.resto(&r.estado),
                };
                s.map(|s| {
                    let t = match s {
                        Estado::Int(_) => self.core.int,
                        Estado::Double(_) => self.core.double,
                        _ => self.core.string,
                    };
                    (t, s)
                })
            }
            _ => return self.generico(u, e, false),
        };
        match res {
            Ok((t, s)) => Constante::Valor(Valor::novo(t, s)),
            Err(x) => self.excecao(u, e, x),
        }
    }

    /// `equalEqual`: com `patterns` ligado (3.0+), primitiva ou `double`.
    fn igual_igual(&mut self, lib: LibraryId, l: &Valor, r: &Valor) -> Result<Option<bool>, Excecao> {
        if l.estado.e_nulo() || r.estado.e_nulo() {
            return Ok(Some(l.estado.e_nulo() && r.estado.e_nulo()));
        }
        let padroes = self.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
        let pode = if padroes {
            matches!(l.estado, Estado::Double(_)) || self.igualdade_primitiva(l, lib)
        } else {
            l.estado.e_bool_num_string_ou_nulo()
        };
        if !pode {
            return Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL_NUM_STRING));
        }
        Ok(self.identico_estado(&l.estado, &r.estado))
    }

    /// `identical` (`isIdentical2`).
    pub fn identico(&mut self, l: &Valor, r: &Valor) -> Option<bool> {
        let (int, double) = (self.core.int, self.core.double);
        if (l.tipo == int && r.tipo == double) || (l.tipo == double && r.tipo == int) {
            return None;
        }
        if l.tipo != r.tipo {
            return Some(false);
        }
        self.identico_estado(&l.estado, &r.estado)
    }

    /// `InstanceState.isIdentical` (e `equalEqual`, que é o mesmo aqui).
    fn identico_estado(&mut self, l: &Estado, r: &Estado) -> Option<bool> {
        match (l, r) {
            (Estado::Bool(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Bool(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (Estado::Int(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Int(b) => Some(a == (*b)?),
                    Estado::Double(b) => Some((*b)? == a as f64),
                    _ => Some(false),
                }
            }
            (Estado::Double(a), b) => {
                let a = (*a)?;
                if a.is_nan() {
                    return Some(false);
                }
                match b {
                    Estado::Double(b) => {
                        let b = (*b)?;
                        if b.is_nan() {
                            return Some(false);
                        }
                        Some(a.to_bits() == b.to_bits())
                    }
                    Estado::Int(b) => Some(a == (*b)? as f64),
                    _ => Some(false),
                }
            }
            (Estado::Str(a), b) => {
                let a = a.as_ref()?;
                match b {
                    Estado::Str(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (Estado::Null { .. }, b) => Some(b.e_nulo()),
            (Estado::Simbolo(a), b) => {
                let a = a.as_ref()?;
                match b {
                    Estado::Simbolo(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (Estado::Tipo(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Tipo(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (Estado::Funcao { elemento: a, args: x }, b) => match b {
                Estado::Funcao { elemento: b, args: y } => Some(a == b && x == y),
                _ => Some(false),
            },
            (Estado::Lista { desconhecida, .. }, b) | (Estado::Conjunto { desconhecido: desconhecida, .. }, b) | (Estado::Mapa { desconhecido: desconhecida, .. }, b)
                if *desconhecida || b.desconhecido() =>
            {
                None
            }
            (Estado::Registro { .. }, _) => {
                if !self.estados_iguais(l, r) {
                    Some(false)
                } else {
                    None
                }
            }
            _ => Some(self.estados_iguais(l, r)),
        }
    }

    /// `==` de `DartObjectImpl` (a igualdade estrutural das chaves de mapa e
    /// dos elementos de conjunto).
    pub fn iguais(&mut self, a: &Valor, b: &Valor) -> bool {
        a.tipo == b.tipo && self.estados_iguais(&a.estado, &b.estado)
    }

    fn estados_iguais(&mut self, a: &Estado, b: &Estado) -> bool {
        // Valor não conhecido (constante de outra biblioteca, opaca) não é
        // provadamente igual a nada: dois `CompileTimeErrorCode.X` vindos
        // de fora não são o mesmo elemento de conjunto.
        if a.desconhecido() || b.desconhecido() {
            return false;
        }
        match (a, b) {
            (Estado::Bool(x), Estado::Bool(y)) => x == y,
            (Estado::Int(x), Estado::Int(y)) => x == y,
            (Estado::Double(x), Estado::Double(y)) => x.map(f64::to_bits) == y.map(f64::to_bits),
            (Estado::Str(x), Estado::Str(y)) => x == y,
            (Estado::Null { .. }, Estado::Null { .. }) => true,
            (Estado::Simbolo(x), Estado::Simbolo(y)) => x == y,
            (Estado::Tipo(x), Estado::Tipo(y)) => x == y,
            (Estado::Funcao { elemento: x, args: p }, Estado::Funcao { elemento: y, args: q }) => x == y && p == q,
            (Estado::Lista { elementos: x, .. }, Estado::Lista { elementos: y, .. })
            | (Estado::Conjunto { elementos: x, .. }, Estado::Conjunto { elementos: y, .. }) => {
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| self.iguais(p, q))
            }
            (Estado::Mapa { entradas: x, .. }, Estado::Mapa { entradas: y, .. }) => {
                x.len() == y.len()
                    && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| self.iguais(k, k2) && self.iguais(v, v2)))
            }
            (Estado::Registro { posicionais: p1, nomeados: n1 }, Estado::Registro { posicionais: p2, nomeados: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(p2.iter()).all(|(a, b)| self.iguais(a, b))
                    && n1.iter().all(|(n, v)| n2.iter().any(|(m, w)| m == n && self.iguais(v, w)))
            }
            (Estado::Generico { campos: x, .. }, Estado::Generico { campos: y, .. }) => {
                x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && self.iguais(v, v2)))
            }
            _ => false,
        }
    }

    /// `hasPrimitiveEquality`.
    pub fn igualdade_primitiva(&mut self, v: &Valor, lib: LibraryId) -> bool {
        match &v.estado {
            Estado::Registro { posicionais, nomeados } => {
                let (p, n) = (posicionais.clone(), nomeados.clone());
                p.iter().all(|x| self.igualdade_primitiva(x, lib)) && n.iter().all(|(_, x)| self.igualdade_primitiva(x, lib))
            }
            Estado::Generico { .. } => {
                let Type::Interface { class, .. } = self.table.get(v.tipo).clone() else { return false };
                let padroes = self.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
                let eq = self.interner.lookup("==");
                let hash = self.interner.lookup("hashCode");
                let de_object = |m: &Motor<'_>, s: Option<SymbolId>| match s.and_then(|s| m.membro_concreto(class, s)) {
                    Some(c) => Some(c) == m.core.object_class,
                    None => true,
                };
                de_object(self, eq) && (!padroes || de_object(self, hash))
            }
            Estado::Double(_) => false,
            _ => true,
        }
    }

    /// A classe que declara o membro de instância concreto `nome` visto de
    /// `c` (superclasses e mixins, na ordem de busca).
    fn membro_concreto(&self, c: ClassId, nome: SymbolId) -> Option<ClassId> {
        let mut atual = Some(c);
        let mut vistos = 0;
        while let Some(k) = atual {
            vistos += 1;
            if vistos > 100 {
                return None;
            }
            let classe = self.program.class(k);
            if let Some(f) = classe.instance_members.get(&nome) {
                if !self.program.function(*f).abstract_ {
                    return Some(k);
                }
            }
            for m in classe.mixin_classes.iter().rev() {
                if let Some(f) = self.program.class(*m).instance_members.get(&nome) {
                    if !self.program.function(*f).abstract_ {
                        return Some(*m);
                    }
                }
            }
            atual = classe.supertype_class;
        }
        None
    }

    // -- Nomes -----------------------------------------------------------------

    fn propriedade(&mut self, cx: &Ctx, e: ExprId, alvo: ExprId, nome: ast::Name, em_const: bool) -> R {
        let u = cx.unidade;
        let a = self.ast(u);
        let alvo_simples = matches!(a.expr(alvo).kind, ExprKind::Identifier(_));
        let res_alvo = self.resolvido(u, alvo).cloned();
        match res_alvo {
            // Constante de topo importada com prefixo; do prefixo adiado,
            // o erro de biblioteca adiada no nome (`visitPrefixedIdentifier`,
            // 3.6.2 `evaluation.dart:1108-1115`).
            Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => {
                if self.prefixo_adiado(u, alvo) {
                    return self.erro_de_biblioteca_adiada(u, e, nome.span);
                }
                return self.valor_constante(cx, e, e, true);
            }
            // `p.Ext.m` (`visitPropertyAccess`, `:1176-1185`): com `p`
            // adiado, o erro no nome da extensão.
            Some(Resolved::Element(Element::Extension(_))) => {
                if let ExprKind::Property { target: p, name: nome_ext, .. } = &a.expr(alvo).kind
                    && matches!(self.resolvido(u, *p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))))
                    && self.prefixo_adiado(u, *p)
                {
                    let sp = nome_ext.span;
                    return self.erro_de_biblioteca_adiada(u, e, sp);
                }
                return self.valor_constante(cx, e, e, true);
            }
            _ => {}
        }
        if let Some(Resolved::Element(Element::Class(k))) = &res_alvo {
            if self.program.class(*k).kind == ClassKind::ExtensionType && !alvo_simples {
                return self.valor_constante(cx, e, e, true);
            }
        }
        let prefixo = match self.avaliar(cx, alvo, em_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        let e_interface = alvo_simples && matches!(res_alvo, Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))));
        // `C<T>.new`, `C.nome`: o `ConstructorReference` (tear-off de
        // construtor), não um acesso a propriedade do `Type`.
        if matches!(self.resolvido(u, e), Some(Resolved::Constructor(_))) {
            return self.valor_constante(cx, e, e, true);
        }
        if !e_interface {
            if let Some(r) = self.acesso_a_propriedade(u, e, &prefixo, nome) {
                return r;
            }
        }
        self.valor_constante(cx, e, e, true)
    }

    /// `PrefixedIdentifier.isDeferred` (3.6.2 `ast.dart:14386-14396`): o
    /// prefixo `p` (um identificador) é de exatamente um import da unidade, e
    /// esse import é `deferred`.
    fn prefixo_adiado(&self, u: UnitId, p: ExprId) -> bool {
        let ExprKind::Identifier(n) = &self.ast(u).expr(p).kind else {
            return false;
        };
        let lib = self.program.unit(u).library;
        let mut imports = self.program.library(lib).imports.iter().filter(|i| i.unit == u && i.prefix == Some(n.sym));
        match (imports.next(), imports.next()) {
            (Some(i), None) => i.deferred,
            _ => false,
        }
    }

    /// `_getDeferredLibraryError` (3.6.2 `evaluation.dart:1873-1933`): o código
    /// pelo primeiro ancestral de `no` que o decide, no `alvo`; sem nenhum,
    /// `INVALID_CONSTANT` no próprio `no`.
    fn erro_de_biblioteca_adiada(&self, u: UnitId, no: ExprId, alvo: Span) -> R {
        match self.codigo_de_biblioteca_adiada(u, no) {
            Some(codigo) => Constante::Invalida(Box::new(self.erro(u, alvo, codigo))),
            None => self.inv(u, no, c::INVALID_CONSTANT),
        }
    }

    /// A subida do `_getDeferredLibraryError`: `Annotation`, `DefaultFormalParameter`,
    /// `IfElement` cuja condição é o próprio `no`, `InstanceCreationExpression`,
    /// `ListLiteral`, `MapLiteralEntry` (chave ou valor, pelo filho), `RecordLiteral`,
    /// `SetOrMapLiteral`, `SpreadElement`, `SwitchCase` (o antigo), `SwitchPatternCase`
    /// e `VariableDeclaration`. Os pais vêm de `dartforge_frontend::pais`; onde o
    /// pai não é expressão (instrução, padrão, valor padrão…), a subida continua
    /// pela menor expressão que contém o trecho (uma expressão de função, um
    /// `switch` de expressão, a coleção do `if (… case …)`).
    fn codigo_de_biblioteca_adiada(&self, u: UnitId, no: ExprId) -> Option<Codigo> {
        let a = self.ast(u);
        let pais = crate::lints_tipados::pais_da_unidade(self.program, u);
        let lib = self.program.unit(u).library;
        let antes_de_3 = self.program.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(3, 0);
        let contem = |de: Span, sp: Span| de.start <= sp.start && sp.end <= de.end;
        let mut atual = no;
        for _ in 0..a.exprs.len() + 1 {
            let sp = a.expr(atual).span;
            let proximo = match pais.pai(atual) {
                dartforge_frontend::pais::Pai::Expr(p) => Some(p),
                dartforge_frontend::pais::Pai::Anotacao => return Some(c::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                dartforge_frontend::pais::Pai::Variavel { .. } => return Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                // `EnumConstantArguments` → `EnumConstantDeclaration` →
                // `EnumDeclaration`: nenhum decide.
                dartforge_frontend::pais::Pai::ConstanteDeEnum => return None,
                dartforge_frontend::pais::Pai::PadraoConstante { .. } => {
                    // No `case` de um `switch` de instrução, o código do caso.
                    let em_caso = a.stmts.iter().any(|s| match &s.kind {
                        ast::StmtKind::Switch { cases, .. } => {
                            cases.iter().any(|k| k.pattern.is_some_and(|p| contem(a.pattern(p).span, sp)))
                        }
                        _ => false,
                    });
                    if em_caso {
                        return Some(if antes_de_3 {
                            c::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY
                        } else {
                            c::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY
                        });
                    }
                    None
                }
                dartforge_frontend::pais::Pai::Fronteira => {
                    let padrao = |ps: &[ast::Parameter]| ps.iter().any(|p| p.default_value == Some(atual));
                    fn aninhado(ps: &[ast::Parameter], atual: ExprId) -> bool {
                        ps.iter().any(|p| {
                            p.default_value == Some(atual) || p.function_parameters.as_deref().is_some_and(|fs| aninhado(fs, atual))
                        })
                    }
                    let e_padrao = a.members.iter().any(|m| matches!(&m.kind, ast::MemberKind::Constructor(k) if padrao(&k.parameters) || aninhado(&k.parameters, atual)))
                        || a.functions.iter().any(|f| f.parameters.as_deref().is_some_and(|ps| aninhado(ps, atual)))
                        || a.types.iter().any(|t| matches!(&t.kind, ast::TypeKind::Function { parameters, .. } if aninhado(parameters, atual)));
                    if e_padrao {
                        return Some(c::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY);
                    }
                    None
                }
            };
            // O pai que não é expressão: a menor expressão que contém o trecho.
            let p = match proximo {
                Some(p) => p,
                None => {
                    let mut melhor: Option<(usize, ExprId)> = None;
                    for (i, x) in a.exprs.iter().enumerate() {
                        let id = ExprId(i as u32);
                        if id == atual || !contem(x.span, sp) || x.span == sp {
                            continue;
                        }
                        let tam = x.span.end - x.span.start;
                        if melhor.is_none_or(|(t, _)| tam < t) {
                            melhor = Some((tam, id));
                        }
                    }
                    match melhor {
                        Some((_, id)) => id,
                        None => return None,
                    }
                }
            };
            if let Some(codigo) = self.papel_na_subida(u, p, sp, no) {
                return Some(codigo);
            }
            atual = p;
        }
        None
    }

    /// O código que a expressão `p`, pai do trecho `filho`, decide na subida.
    fn papel_na_subida(&self, u: UnitId, p: ExprId, filho: Span, no: ExprId) -> Option<Codigo> {
        let a = self.ast(u);
        let contem = |de: Span, sp: Span| de.start <= sp.start && sp.end <= de.end;
        // O elemento de coleção que contém o filho, do mais fundo ao mais raso.
        fn elemento(a: &ast::Ast, el: &ast::CollectionElement, filho: Span, no: ExprId, contem: &dyn Fn(Span, Span) -> bool) -> Option<Option<Codigo>> {
            let span = |e: ExprId| a.expr(e).span;
            match el {
                ast::CollectionElement::Expression(e) | ast::CollectionElement::NullAwareExpression(e) => {
                    contem(span(*e), filho).then_some(None)
                }
                ast::CollectionElement::MapEntry { key, value, .. } => {
                    if contem(span(*key), filho) {
                        Some(Some(c::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY))
                    } else if contem(span(*value), filho) {
                        Some(Some(c::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY))
                    } else {
                        None
                    }
                }
                ast::CollectionElement::Spread { value, .. } => {
                    contem(span(*value), filho).then_some(Some(c::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                    for sub in std::iter::once(&**then).chain(else_.as_deref()) {
                        if let Some(r) = elemento(a, sub, filho, no, contem) {
                            return Some(r);
                        }
                    }
                    let dentro = contem(span(*condition), filho)
                        || case_pattern.is_some_and(|p| contem(a.pattern(p).span, filho))
                        || guard.is_some_and(|g| contem(span(g), filho));
                    if !dentro {
                        return None;
                    }
                    Some((*condition == no).then_some(c::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::For { condition, updates, body, .. } => {
                    if let Some(r) = elemento(a, body, filho, no, contem) {
                        return Some(r);
                    }
                    let dentro = condition.is_some_and(|x| contem(span(x), filho)) || updates.iter().any(|x| contem(span(*x), filho));
                    dentro.then_some(None)
                }
                ast::CollectionElement::ForIn { iterable, body, .. } => {
                    if let Some(r) = elemento(a, body, filho, no, contem) {
                        return Some(r);
                    }
                    contem(span(*iterable), filho).then_some(None)
                }
            }
        }
        match &a.expr(p).kind {
            ExprKind::InstanceCreation { .. } => Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY),
            ExprKind::Call { .. } if matches!(self.resolvido(u, p), Some(Resolved::Constructor(_))) => {
                Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY)
            }
            ExprKind::Record { .. } => Some(c::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                let do_literal = if matches!(a.expr(p).kind, ExprKind::List { .. }) {
                    c::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY
                } else {
                    c::SET_ELEMENT_FROM_DEFERRED_LIBRARY
                };
                for el in elements.iter() {
                    if let Some(r) = elemento(a, el, filho, no, &contem) {
                        return Some(r.unwrap_or(do_literal));
                    }
                }
                Some(do_literal)
            }
            _ => None,
        }
    }

    /// `_evaluatePropertyAccess`.
    fn acesso_a_propriedade(&mut self, u: UnitId, e: ExprId, alvo: &Valor, nome: ast::Name) -> Option<R> {
        let res = self.resolvido(u, e).cloned();
        let estatico = |m: &Motor<'_>, f: FunctionElementId| m.program.function(f).static_;
        match &res {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) if estatico(self, *f) => return None,
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) if self.program.variable(*v).static_ => return None,
            Some(Resolved::ExtensionMember { .. }) => return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_METHOD)),
            Some(Resolved::Member { class, .. }) if self.program.class(*class).kind == ClassKind::ExtensionType => {
                return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_TYPE_METHOD));
            }
            _ => {}
        }
        if Some(nome.sym) == self.sym_length && alvo.tipo == self.core.string {
            return Some(match alvo.estado.comprimento() {
                Ok(s) => Constante::Valor(Valor::novo(self.core.int, s)),
                Err(x) => self.excecao(u, e, x),
            });
        }
        let mut i = self.erro(u, self.span(u, e), c::CONST_EVAL_PROPERTY_ACCESS);
        i.args = vec![self.interner.resolve(nome.sym).to_string(), self.formatar(alvo.tipo)];
        Some(Constante::Invalida(Box::new(i)))
    }

    /// `alvo(args)` que cria pelo construtor primário de um tipo de extensão:
    /// o alvo é o tipo (`A`) ou `A.nome` com o nome do primário. Fora de
    /// contexto constante é o genérico; o primário sem `const`, também.
    fn criacao_por_primario_de_extensao(&mut self, cx: &Ctx, e: ExprId, alvo: ExprId, arguments: &ast::Arguments, em_const: bool) -> Option<R> {
        let u = cx.unidade;
        let a = self.ast(u);
        let (classe, nome) = match &a.expr(alvo).kind {
            ExprKind::Identifier(_) => (self.resolvido(u, alvo).cloned(), None),
            ExprKind::Property { target, name, .. } => (self.resolvido(u, *target).cloned(), Some(name.sym)),
            _ => return None,
        };
        let Some(Resolved::Element(Element::Class(k))) = classe else { return None };
        let d = self.program.class(k).decl?;
        let ast::DeclKind::ExtensionType(et) = &self.ast(d.unit).decl(d.decl).kind else { return None };
        let do_primario = match (nome, et.constructor) {
            (None, None) => true,
            (Some(n), Some(c)) => n == c.sym,
            (Some(n), None) => self.interner.resolve(n) == "new",
            (None, Some(_)) => false,
        };
        if !do_primario {
            return None;
        }
        if !em_const || !et.const_ {
            return Some(self.generico(u, e, false));
        }
        let x = arguments.args.iter().find(|x| x.name.is_none())?;
        Some(self.avaliar(cx, x.value, true))
    }

    /// A instanciação implícita do tear-off `e` (`typeArgumentTypes` de um
    /// `FunctionReference` sem argumentos escritos) com algum argumento que
    /// menciona parâmetro de tipo depois do ambiente léxico do construtor
    /// (3.6.2 `evaluation.dart:862-876`).
    fn instanciacao_com_parametro(&mut self, cx: &Ctx, e: ExprId) -> bool {
        let inferidos: Option<Vec<TypeId>> = self.body.units.get(cx.unidade.0 as usize).and_then(|b| b.instanciacao_de_tearoff(e)).map(|a| a.to_vec());
        let Some(args) = inferidos else { return false };
        // Só o argumento que é ele mesmo um parâmetro de tipo passa pelo
        // `_lexicalTypeEnvironment` (`:866-871`); `List<U>` fica como está.
        args.iter().any(|&a| {
            let a = match (self.table.get(a), &cx.tipos) {
                (Type::TypeParameter { param, .. }, Some(m)) => m.get(param).copied().unwrap_or(a),
                _ => a,
            };
            self.menciona_parametro(a)
        })
    }

    /// `_getConstantValue` do elemento a que `e` resolve (`erro_em` é o nó
    /// do erro).
    fn valor_constante(&mut self, cx: &Ctx, e: ExprId, erro_em: ExprId, com_identificador: bool) -> R {
        let u = cx.unidade;
        let res = self.resolvido(u, e).cloned();
        let _ = com_identificador;
        match res {
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                // A variável que guarda uma função genérica, instanciada
                // implicitamente: o valor primeiro, depois os argumentos.
                let r = self.valor_de_referencia_a_variavel(cx, erro_em, v);
                if matches!(r, Constante::Valor(_)) && self.instanciacao_com_parametro(cx, e) {
                    return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                }
                return r;
            }
            Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                let fe = self.program.function(f);
                // `values` de um enum: o campo `const` sintético cujo
                // inicializador é a lista das constantes
                // (`LibraryBuilder.buildEnumChildren`); no modelo, um getter
                // estático sem nó.
                if fe.node == FunctionRef::None
                    && fe.static_
                    && fe.kind == FunctionKind::Getter
                    && let Some(k) = fe.class
                    && self.program.class(k).kind == ClassKind::Enum
                    && self.interner.resolve(fe.name) == "values"
                {
                    return self.valores_do_enum(u, erro_em, k);
                }
                if fe.kind == FunctionKind::ImplicitAccessor {
                    if let Some(v) = fe.variable {
                        return self.valor_de_referencia_a_variavel(cx, erro_em, v);
                    }
                }
                let estatica = fe.class.is_none() && fe.extension.is_none() || fe.static_;
                if estatica && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                    // Instanciação implícita do tear-off genérico
                    // (`visitFunctionReference` sem argumentos escritos, 3.6.2
                    // `evaluation.dart:862-876`): um argumento inferido que
                    // menciona parâmetro de tipo (depois do ambiente léxico do
                    // construtor) é `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`.
                    if self.instanciacao_com_parametro(cx, e) {
                        return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                    }
                    let t = self.outline.functions.get(f.0 as usize).map(|d| d.signature).unwrap_or(self.core.dynamic_);
                    let t = if self.inferida(u) { self.estatico(u, e) } else { t };
                    // `_instantiateFunctionType` (`evaluation.dart:1939-1959`):
                    // o tipo instanciado passa pela substituição do construtor.
                    let t = match &cx.tipos {
                        Some(m) => crate::ops::substitute(t, m, self.table),
                        None => t,
                    };
                    return Constante::Valor(self.valor(t, Estado::Funcao { elemento: Funcao::Elemento(f), args: None }));
                }
            }
            Some(Resolved::Constructor(f)) => {
                let t = self.estatico(u, e);
                return Constante::Valor(self.valor(t, Estado::Funcao { elemento: Funcao::Elemento(f), args: None }));
            }
            Some(Resolved::Element(Element::Class(k))) => {
                let n = self.program.class(k).type_params.len();
                let args = vec![self.core.dynamic_; n];
                let t = if self.program.class(k).kind == ClassKind::ExtensionType {
                    self.table.intern(Type::ExtensionType { decl: k, args: args.into_boxed_slice(), nullable: false })
                } else {
                    self.table.intern(Type::Interface { class: k, args: args.into_boxed_slice(), nullable: false })
                };
                let t = self.apagar(t);
                return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(t))));
            }
            Some(Resolved::Element(Element::Typedef(_))) => {
                return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(None)));
            }
            Some(Resolved::TypeParameter(tp)) => {
                if let Some(t) = cx.tipos.as_ref().and_then(|m| m.get(&tp)) {
                    let t = *t;
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(t))));
                }
                return self.inv(u, erro_em, c::CONST_TYPE_PARAMETER);
            }
            Some(Resolved::Local(_)) => {
                if let Some(r) = self.valor_de_local(cx, e, erro_em) {
                    return r;
                }
            }
            _ => {}
        }
        // `dynamic`/`Never`/`void` como literais de tipo.
        if let ExprKind::Identifier(n) = &self.ast(u).expr(e).kind {
            match self.interner.resolve(n.sym) {
                "dynamic" if res.is_none() => {
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(self.core.dynamic_))));
                }
                "Never" if res.is_none() => {
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(self.core.never))));
                }
                _ => {}
            }
        }
        if !self.inferida(u) {
            let t = self.estatico(u, e);
            return Constante::Valor(self.desconhecido(t));
        }
        if self.tipo_invalido(u, e) {
            return self.generico(u, erro_em, true);
        }
        self.generico(u, erro_em, false)
    }

    /// O valor de `E.values`: `List<E>` com o valor de cada constante, na
    /// ordem; uma constante inválida invalida a lista (sem relato próprio).
    fn valores_do_enum(&mut self, u: UnitId, erro_em: ExprId, k: ClassId) -> R {
        let n = self.program.class(k).type_params.len();
        let elemento = self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false });
        let elemento = self.apagar(elemento);
        let tipo = self.interface(self.core.list_class, vec![elemento]);
        let mut lista = Vec::new();
        for v in self.program.class(k).enum_constants.clone() {
            match self.valor_de_variavel(v) {
                Some(Constante::Valor(x)) => lista.push(x),
                // Uma constante inválida (num ciclo com o próprio `values`,
                // `e1(values)`): o `values` fica inválido como variável `const`
                // de resultado inválido, calado (`avoidReporting`,
                // `evaluation.dart:1867-1878`).
                Some(Constante::Invalida(_)) => {
                    let mut i = self.erro(u, self.span(u, erro_em), c::INVALID_CONSTANT);
                    i.nao_resolvida = true;
                    i.evitar_relato = true;
                    return Constante::Invalida(Box::new(i));
                }
                None => return self.generico(u, erro_em, true),
            }
        }
        Constante::Valor(self.valor(tipo, Estado::Lista { elemento, elementos: Rc::new(lista), desconhecida: false }))
    }

    /// O membro estático variável `nome` de `d` (constante de enum ou campo
    /// estático).
    fn membro_estatico_variavel(&self, d: ClassId, nome: SymbolId) -> Option<VariableId> {
        let k = self.program.class(d);
        k.enum_constants
            .iter()
            .chain(k.fields.iter())
            .copied()
            .find(|v| self.program.variable(*v).name == nome && self.program.variable(*v).static_)
    }

    /// Referência a uma variável de topo ou estática: o valor dela, se é
    /// `const`; senão o erro genérico no nó.
    fn valor_de_referencia_a_variavel(&mut self, cx: &Ctx, erro_em: ExprId, v: VariableId) -> R {
        let u = cx.unidade;
        let var = self.program.variable(v);
        if var.const_ {
            return match self.valor_de_variavel(v) {
                Some(Constante::Valor(x)) => Constante::Valor(x),
                Some(Constante::Invalida(_)) => {
                    let mut i = self.erro(u, self.span(u, erro_em), c::INVALID_CONSTANT);
                    i.nao_resolvida = true;
                    i.evitar_relato = true;
                    Constante::Invalida(Box::new(i))
                }
                None => self.generico(u, erro_em, true),
            };
        }
        if self.tipo_invalido(u, erro_em) {
            return self.generico(u, erro_em, true);
        }
        self.generico(u, erro_em, false)
    }

    /// `ElementAnnotation.computeConstantValue` da anotação `m`, escrita na
    /// unidade `u`: a invocação de construtor (`@C(…)`, `@C.n(…)`,
    /// `@p.C(…)`, `@p.C.n(…)`, com os argumentos de tipo que a inferência
    /// registrou para a lista de argumentos) ou a leitura de variável
    /// (`@x`, `@p.x`, `@C.x`). `None` quando não resolve ou não é constante.
    pub fn avaliar_anotacao(&mut self, u: UnitId, m: &ast::Annotation) -> Option<Valor> {
        let lib = self.program.unit(u).library;
        let cx = Ctx::simples(u, lib);
        let nomes: Vec<SymbolId> = m.name.iter().map(|n| n.sym).collect();
        let (alvo, membro) = match nomes.as_slice() {
            [c] => (self.program.lookup_na_unidade(u, *c).and_then(|b| b.getter), None),
            [a, b] => match self.program.lookup_na_unidade(u, *a).and_then(|x| x.getter) {
                Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
                _ => (self.program.lookup_prefixed_na_unidade(u, *a, *b).and_then(|x| x.getter), None),
            },
            [p, c, n] => (self.program.lookup_prefixed_na_unidade(u, *p, *c).and_then(|x| x.getter), Some(*n)),
            _ => (None, None),
        };
        match (alvo?, &m.arguments) {
            (Element::Class(c), Some(args)) => {
                let chave = membro.or_else(|| self.interner.lookup(""))?;
                let f = *self.program.class(c).constructors.get(&chave)?;
                let n = self.program.class(c).type_params.len();
                let argumentos: Vec<TypeId> = match self.body.units.get(u.0 as usize).and_then(|b| b.instanciacao(args.span.start)) {
                    Some(v) if v.len() == n => v.to_vec(),
                    _ => vec![self.core.dynamic_; n],
                };
                let tipo = self.table.intern(Type::Interface { class: c, args: argumentos.into_boxed_slice(), nullable: false });
                let erro = ErroEm { unidade: u, span: m.span };
                match self.avaliar_chamada(&cx, erro, f, tipo, &Argumentos::Ast(args), None, true) {
                    Constante::Valor(v) => Some(v),
                    Constante::Invalida(_) => None,
                }
            }
            (Element::Class(c), None) => {
                let v = self.membro_estatico_variavel(c, membro?)?;
                match self.valor_de_variavel(v)? {
                    Constante::Valor(x) => Some(x),
                    Constante::Invalida(_) => None,
                }
            }
            (Element::Variable(v), None) => match self.valor_de_variavel(v)? {
                Constante::Valor(x) => Some(x),
                Constante::Invalida(_) => None,
            },
            (Element::Function(g), None) => {
                // A leitura de uma variável de topo pelo getter sintético.
                let v = self.program.function(g).variable?;
                match self.valor_de_variavel(v)? {
                    Constante::Valor(x) => Some(x),
                    Constante::Invalida(_) => None,
                }
            }
            _ => None,
        }
    }

    /// O valor de uma variável com inicializador constante (`const`, ou
    /// `final` de instância numa classe com construtor gerador `const`),
    /// calculado uma vez. `None` enquanto está em cálculo (ciclo).
    /// O `_IsSerializableNodeVisitor` sobre `init`: algum nó da subárvore é
    /// `ForElement`, `FunctionExpression`, `PatternAssignment` ou
    /// `SwitchExpression`. Os nós de uma expressão nascem antes dela, em ids
    /// contíguos e dentro do intervalo dela.
    fn nao_serializavel(&self, u: UnitId, init: ExprId) -> bool {
        fn tem_for(el: &ast::CollectionElement) -> bool {
            match el {
                ast::CollectionElement::For { .. } | ast::CollectionElement::ForIn { .. } => true,
                ast::CollectionElement::If { then, else_, .. } => tem_for(then) || else_.as_deref().is_some_and(tem_for),
                _ => false,
            }
        }
        let a = self.ast(u);
        let sp = a.expr(init).span;
        let mut i = init.0 as usize + 1;
        while i > 0 {
            i -= 1;
            let x = &a.exprs[i];
            if x.span.start < sp.start || x.span.end > sp.end {
                break;
            }
            let ruim = match &x.kind {
                ExprKind::FunctionExpression(_) | ExprKind::PatternAssign { .. } | ExprKind::Switch { .. } => true,
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().any(tem_for),
                _ => false,
            };
            if ruim {
                return true;
            }
        }
        false
    }

    pub fn valor_de_variavel(&mut self, v: VariableId) -> Option<Constante> {
        match self.vars.get(&v) {
            Some(EstadoVar::Pronta(r)) => return Some(r.clone()),
            Some(EstadoVar::EmCurso) => {
                if let Some(p) = self.pilha_vars.iter().position(|x| *x == v) {
                    let ciclo: Vec<VariableId> = self.pilha_vars[p..].to_vec();
                    self.ciclicos.extend(ciclo);
                }
                return None;
            }
            None => {}
        }
        // O componente do grafo de dependências a que a variável pertence
        // tem ciclo (`generateCycleError`): o resultado guardado é o
        // inválido no nome dela, e quem a lê recebe um inválido calado.
        if self.grafo.variaveis_em_ciclo.contains(&v) {
            // A constante de enum também: o erro no nome dela.
            let unidade = match self.program.variable(v).node {
                VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } => Some(unit),
                _ => None,
            };
            if let (Some(unidade), Some(nome)) = (unidade, self.span_do_nome_da_variavel(v)) {
                let r = Constante::Invalida(Box::new(self.erro(unidade, nome, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
                self.vars.insert(v, EstadoVar::Pronta(r.clone()));
                return Some(r);
            }
        }
        let var = self.program.variable(v);
        let lib = var.library;
        let tipo_var = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred);
        let tipo_var = tipo_var.unwrap_or(self.core.dynamic_);
        let (unidade, inicial) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                self.vars.insert(v, EstadoVar::EmCurso);
                self.pilha_vars.push(v);
                let r = self.valor_de_enum(v, unit, decl, index);
                self.pilha_vars.pop();
                self.vars.insert(v, EstadoVar::Pronta(r.clone()));
                return Some(r);
            }
            _ => return Some(Constante::Valor(self.desconhecido(tipo_var))),
        };
        let Some(init) = inicial else {
            let r = Constante::Valor(self.desconhecido(tipo_var));
            return Some(r);
        };
        if !self.inferidas.contains(&lib) {
            let r = Constante::Valor(self.desconhecido(tipo_var));
            self.vars.insert(v, EstadoVar::Pronta(r.clone()));
            return Some(r);
        }
        // `DetachNodes._detachConstVariable` (3.6.2 `summary2/detach_nodes.dart:103-115`,
        // `replaceNotSerializableNode` em `:18-51`): o inicializador da
        // constante de topo ou de campo que tem `for` de coleção, expressão de
        // função, atribuição de padrão ou `switch` de expressão vira um
        // identificador sintético; o `evaluationResult` é inválido, no
        // inicializador.
        if self.nao_serializavel(unidade, init) {
            let r = Constante::Invalida(Box::new(self.erro(unidade, self.span(unidade, init), c::INVALID_CONSTANT)));
            self.vars.insert(v, EstadoVar::Pronta(r.clone()));
            return Some(r);
        }
        self.vars.insert(v, EstadoVar::EmCurso);
        self.pilha_vars.push(v);
        let cx = Ctx::simples(unidade, lib);
        let const_ = self.program.variable(v).const_;
        let salvo = std::mem::take(&mut self.args_de_criacao);
        let mut r = self.avaliar(&cx, init, const_);
        self.args_de_criacao = salvo;
        self.pilha_vars.pop();
        if let Constante::Valor(x) = &r {
            if const_ && !self.casa(x, tipo_var) {
                let estatico = self.estatico(unidade, init);
                if self.atribuivel(estatico, tipo_var) {
                    let mut i = self.erro(unidade, self.span(unidade, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.formatar(x.tipo), self.formatar(tipo_var)];
                    r = Constante::Invalida(Box::new(i));
                }
            }
        }
        if let Constante::Valor(x) = &mut r {
            if const_ {
                x.variavel = Some(v);
            }
        }
        if self.ciclicos.contains(&v) {
            let nome = self.span_do_nome_da_variavel(v).unwrap_or(Span { start: 0, end: 0 });
            r = Constante::Invalida(Box::new(self.erro(unidade, nome, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
        }
        self.vars.insert(v, EstadoVar::Pronta(r.clone()));
        Some(r)
    }

    pub fn span_do_nome_da_variavel(&self, v: VariableId) -> Option<Span> {
        match self.program.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Enum(en) => en.constants.get(index).map(|x| x.name.span),
                _ => None,
            },
            _ => None,
        }
    }

    /// Uma constante de enum: o objeto com `index` e `_name`.
    fn valor_de_enum(&mut self, v: VariableId, unit: UnitId, decl: ast::DeclId, index: usize) -> Constante {
        let _ = (unit, decl);
        let Some(k) = self.program.variable(v).class else {
            return Constante::Valor(Valor::novo(self.core.dynamic_, Estado::Generico { campos: Rc::new(Vec::new()), desconhecido: true }));
        };
        // O tipo da constante (a instanciação escrita, `a<int>()`, ou a dos
        // limites), como o `DartObject` do analyzer.
        let vd = &self.outline.variables[v.0 as usize];
        let tipo = match vd.declared_type.or(vd.inferred) {
            Some(t) => t,
            None => {
                let n = self.program.class(k).type_params.len();
                self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        let nome = self.interner.resolve(self.program.variable(v).name);
        let mut campos = vec![
            (Campo::Indice, Valor::novo(self.core.int, Estado::Int(Some(index as i64)))),
            (Campo::NomeDoEnum, Valor::novo(self.core.string, Estado::Str(Some(texto(nome))))),
        ];
        // O inicializador da constante é a criação `E.nome(args)` com a
        // declaração da constante como nó de erro (`_errorNodes`,
        // `evaluation.dart:39-61`); a exceção de avaliação vira
        // `CONST_EVAL_THROWS_EXCEPTION` nela (`:350-376`).
        let lib = self.program.class(k).library;
        if self.inferidas.contains(&lib)
            && let ast::DeclKind::Enum(en) = &self.ast(unit).decl(decl).kind
            && let Some(cst) = en.constants.get(index)
        {
            let chave = match cst.constructor {
                Some(n) if self.interner.resolve(n.sym) != "new" => Some(n.sym),
                _ => self.interner.lookup(""),
            };
            // O primário (3.13) numa biblioteca sem o recurso: o oráculo 3.13.4
            // não o avalia.
            let primario_desligado = |f: FunctionElementId| {
                matches!(self.program.function(f).node, FunctionRef::Constructor { member, .. } if en.primary_constructor == Some(member))
                    && !self.program.library(lib).features.tem(dartforge_frontend::Feature::PrimaryConstructors)
            };
            if let Some(&f) = chave.and_then(|c| self.program.class(k).constructors.get(&c))
                && !primario_desligado(f)
            {
                let span = Span { start: cst.name.span.start, end: cst.span.end };
                let erro = ErroEm { unidade: unit, span };
                let cx = Ctx::simples(unit, lib);
                let vazio = Argumentos::Valores { posicionais: Vec::new(), nomeados: Vec::new() };
                let r = match &cst.arguments {
                    Some(a) => self.avaliar_chamada(&cx, erro, f, tipo, &Argumentos::Ast(a), None, true),
                    None => self.avaliar_chamada(&cx, erro, f, tipo, &vazio, None, true),
                };
                match r {
                    Constante::Invalida(i) if i.excecao => {
                        let mut n = self.erro(unit, span, c::CONST_EVAL_THROWS_EXCEPTION);
                        n.evitar_relato = i.evitar_relato;
                        let msg = dartforge_diagnostics::Diagnostic::com_codigo(i.codigo, i.span, i.args.iter().map(|s| s.as_str())).message;
                        let definidora = self.program.library(self.program.unit(unit).library).units.first().copied().unwrap_or(unit);
                        n.contexto = i.contexto.clone();
                        n.contexto.push((definidora, i.span, format!("The exception is '{msg}' and occurs here.")));
                        return Constante::Invalida(Box::new(n));
                    }
                    Constante::Invalida(i) => return Constante::Invalida(i),
                    Constante::Valor(x) => {
                        if let Estado::Generico { campos: c2, .. } = &x.estado {
                            for par in c2.iter() {
                                if !matches!(par.0, Campo::Indice | Campo::NomeDoEnum) {
                                    campos.push(par.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        Constante::Valor(Valor { tipo, estado: Estado::Generico { campos: Rc::new(campos), desconhecido: false }, variavel: Some(v) })
    }

    /// Constante local: a declaração pelo offset do nome.
    fn valor_de_local(&mut self, cx: &Ctx, e: ExprId, erro_em: ExprId) -> Option<R> {
        let u = cx.unidade;
        let offset = self.body.units.get(u.0 as usize)?.declaracao_local(e)?;
        if !self.locais_da_unidade.contains_key(&u) {
            let mapa = locais_constantes(self.ast(u));
            self.locais_da_unidade.insert(u, mapa);
        }
        let Some(&(stmt, i)) = self.locais_da_unidade[&u].get(&offset) else {
            // Local que não é `const`: o erro genérico.
            return None;
        };
        if let Some(r) = self.locais.get(&(u, offset)) {
            return Some(match r {
                Some(Constante::Valor(v)) => Constante::Valor(v.clone()),
                Some(Constante::Invalida(_)) => {
                    let mut x = self.erro(u, self.span(u, erro_em), c::INVALID_CONSTANT);
                    x.nao_resolvida = true;
                    x.evitar_relato = true;
                    Constante::Invalida(Box::new(x))
                }
                None => self.generico(u, erro_em, true),
            });
        }
        let r = self.valor_de_local_declarado(u, cx.lib, stmt, i);
        Some(match r {
            Constante::Valor(v) => Constante::Valor(v),
            Constante::Invalida(_) => {
                let mut x = self.erro(u, self.span(u, erro_em), c::INVALID_CONSTANT);
                x.nao_resolvida = true;
                x.evitar_relato = true;
                Constante::Invalida(Box::new(x))
            }
        })
    }

    /// O valor da constante local `i` da lista em `stmt` (guardado).
    pub fn valor_de_local_declarado(&mut self, u: UnitId, lib: LibraryId, stmt: ast::StmtId, i: usize) -> Constante {
        let lista = match &self.ast(u).stmt(stmt).kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => return Constante::Valor(Valor::novo(self.core.dynamic_, Estado::Null { invalido: true })),
        };
        let var = &lista.variables[i];
        let offset = var.name.span.start;
        if let Some(Some(r)) = self.locais.get(&(u, offset)) {
            return r.clone();
        }
        if self.grafo.locais_em_ciclo.contains(&(u, offset)) {
            let r = Constante::Invalida(Box::new(self.erro(u, var.name.span, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
            self.locais.insert((u, offset), Some(r.clone()));
            return r;
        }
        let Some(init) = var.initializer else {
            return Constante::Valor(Valor::nulo(self.core));
        };
        self.locais.insert((u, offset), None);
        let cx = Ctx::simples(u, lib);
        let salvo = std::mem::take(&mut self.args_de_criacao);
        let mut r = self.avaliar(&cx, init, true);
        self.args_de_criacao = salvo;
        if let Constante::Valor(x) = &r {
            let tipo = self.body.units[u.0 as usize].tipo_local(offset).unwrap_or(self.core.dynamic_);
            if !self.casa(x, tipo) {
                let estatico = self.estatico(u, init);
                if self.atribuivel(estatico, tipo) {
                    let mut i = self.erro(u, self.span(u, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.formatar(x.tipo), self.formatar(tipo)];
                    r = Constante::Invalida(Box::new(i));
                }
            }
        }
        self.locais.insert((u, offset), Some(r.clone()));
        r
    }

    /// O local lido por `e` é um parâmetro formal.
    pub fn local_parametro(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.declaracao_local(e)) else { return false };
        self.parametros.get(&u).is_some_and(|s| s.contains(&offset))
    }

    /// O local lido por `e` é uma constante (`const`).
    pub fn local_constante(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.declaracao_local(e)) else { return false };
        match self.locais_da_unidade.get(&u) {
            Some(m) => m.contains_key(&offset),
            None => locais_constantes(self.ast(u)).contains_key(&offset),
        }
    }

    // -- Potencialmente constante ---------------------------------------------

    /// `_reportNotPotentialConstants`: o primeiro nó que não é
    /// potencialmente constante, como `INVALID_CONSTANT`.
    fn nao_potencialmente_constante(&self, cx: &Ctx, e: ExprId) -> Option<R> {
        let lib_nnbd = true;
        let _ = lib_nnbd;
        let mut nos = Vec::new();
        super::potencial::coletar(self, cx, e, &mut nos);
        let n = nos.first()?;
        Some(Constante::Invalida(Box::new(self.erro(cx.unidade, *n, c::INVALID_CONSTANT))))
    }

    // -- Construtores ----------------------------------------------------------

    /// O tipo de uma criação: o estático do nó, senão a classe crua.
    fn tipo_de_criacao(&mut self, u: UnitId, e: ExprId, f: FunctionElementId) -> TypeId {
        let t = self.estatico(u, e);
        if matches!(self.table.get(t), Type::Interface { .. } | Type::ExtensionType { .. }) {
            return t;
        }
        let Some(k) = self.program.function(f).class else { return self.core.dynamic_ };
        let n = self.program.class(k).type_params.len();
        self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
    }

    /// `_InstanceCreationEvaluator.evaluate` para uma criação escrita
    /// (`e` é o nó do erro; `palavra`, o `new`/`const` escrito).
    /// O construtor encaminhado de `class B = A with M` (sem elemento no
    /// modelo): o de mesmo nome da primeira superclasse que não é aplicação de
    /// mixin, com o tipo da criação (`B`). A avaliação do encaminhador é a do
    /// construtor da superclasse com os mesmos argumentos.
    fn construtor_encaminhado(&self, u: UnitId, e: ExprId, nome: Option<SymbolId>) -> Option<(FunctionElementId, TypeId)> {
        let tipo = self.estatico(u, e);
        let Type::Interface { class, .. } = self.table.get(tipo) else { return None };
        if self.program.class(*class).kind != ClassKind::MixinApplication {
            return None;
        }
        // `C.new` e `C()` são o construtor sem nome (o símbolo vazio).
        let vazio = self.interner.lookup("");
        let chave = match nome {
            Some(n) if self.interner.resolve(n) != "new" => Some(n),
            _ => vazio,
        };
        let mut atual = self.program.class(*class).supertype_class;
        for _ in 0..64 {
            let k = atual?;
            let classe = self.program.class(k);
            if classe.kind != ClassKind::MixinApplication {
                return chave.and_then(|k| classe.constructors.get(&k).copied()).map(|f| (f, tipo));
            }
            atual = classe.supertype_class;
        }
        None
    }

    fn chamar_construtor(
        &mut self,
        cx: &Ctx,
        e: ExprId,
        f: FunctionElementId,
        args: &ast::Arguments,
        palavra: Option<Span>,
        tipo: Option<TypeId>,
    ) -> R {
        let u = cx.unidade;
        let tipo = match tipo {
            Some(t) => t,
            None => self.tipo_de_criacao(u, e, f),
        };
        let erro = ErroEm { unidade: u, span: self.span(u, e) };
        for arg in args.args.iter() {
            if arg.name.is_none() {
                self.args_de_criacao.insert((u, arg.value));
            }
        }
        self.avaliar_chamada(cx, erro, f, tipo, &Argumentos::Ast(args), palavra, true)
    }

    /// Avalia a chamada do construtor `f` com os argumentos (`ConstantVisitor`
    /// de `cx` para eles), relatando em `erro`.
    fn avaliar_chamada(
        &mut self,
        cx: &Ctx,
        erro: ErroEm,
        f: FunctionElementId,
        tipo: TypeId,
        args: &Argumentos<'_>,
        palavra: Option<Span>,
        em_const_args: bool,
    ) -> R {
        // T3: o `const` é o da declaração pública (no SDK, o patch de
        // `bool.fromEnvironment` e dos irmãos é uma fábrica sem `const`).
        let fe = self.program.function(self.program.publico(f));
        if !fe.const_ {
            let span = palavra.unwrap_or(erro.span);
            return Constante::Invalida(Box::new(self.erro(erro.unidade, span, c::CONST_WITH_NON_CONST)));
        }
        if self.construtores_em_curso.contains(&f) || self.construtores_em_curso.len() > 64 {
            return Constante::Valor(self.desconhecido(tipo));
        }
        // Argumentos (`_valueOf`: o não resolvido é relatado e vira um
        // objeto fictício do tipo do parâmetro).
        let params = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut posicionais: Vec<Valor> = Vec::new();
        let mut nomeados: Vec<(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)> = Vec::new();
        let mut alvos_posicionais: Vec<Option<(UnitId, Span, TypeId)>> = Vec::new();
        match args {
            Argumentos::Ast(a) => {
                let mut i_pos = 0usize;
                let u = cx.unidade;
                for arg in a.args.iter() {
                    let tipo_param = match arg.name {
                        Some(n) => params.iter().find(|p| p.externo.or(p.name) == Some(n.sym)).map(|p| p.ty),
                        None => params.get(i_pos).map(|p| p.ty),
                    }
                    .unwrap_or(self.core.dynamic_);
                    let v = self.valor_de(cx, arg.value, tipo_param, em_const_args);
                    let v = match v {
                        Constante::Valor(v) => v,
                        i => return i,
                    };
                    let alvo = Some((u, self.span(u, arg.value), self.estatico(u, arg.value)));
                    match arg.name {
                        Some(n) => {
                            let s = Span { start: n.span.start, end: self.span(u, arg.value).end };
                            nomeados.push((n.sym, v, Some((u, s, self.estatico(u, arg.value)))));
                        }
                        None => {
                            posicionais.push(v);
                            alvos_posicionais.push(alvo);
                            i_pos += 1;
                        }
                    }
                }
            }
            Argumentos::Valores { posicionais: p, nomeados: n } => {
                posicionais = p.clone();
                alvos_posicionais = vec![None; p.len()];
                nomeados = n.iter().map(|(s, v)| (*s, v.clone(), None)).collect();
            }
        }
        // Redirecionamentos de factory `const` até um construtor que não é.
        let (f, tipo) = self.seguir_redirecionamentos(f, tipo);
        // O construtor não é livre de ciclo (`isCycleFree`,
        // `evaluation.dart:3090-3099`): um desconhecido do tipo, sem erro.
        if self.grafo.construtores_em_ciclo.contains(&f) {
            return Constante::Valor(self.desconhecido(tipo));
        }
        let fe = self.program.function(f);
        let Some(k) = fe.class else { return Constante::Valor(self.desconhecido(tipo)) };
        if fe.factory || !self.inferidas.contains(&fe.library) || fe.kind == FunctionKind::SyntheticConstructor && self.program.class(k).kind == ClassKind::MixinApplication {
            return self.chamar_factory(erro, f, tipo, &posicionais, &nomeados);
        }
        self.construtores_em_curso.push(f);
        let r = self.chamar_gerador(erro, f, k, tipo, posicionais, alvos_posicionais, nomeados);
        self.construtores_em_curso.pop();
        r
    }

    /// `_valueOf`.
    fn valor_de(&mut self, cx: &Ctx, e: ExprId, tipo_padrao: TypeId, em_const: bool) -> R {
        match self.avaliar(cx, e, em_const) {
            Constante::Invalida(i) if i.nao_resolvida => {
                if cx.relatar && !i.evitar_relato {
                    self.relatos.push((*i).clone());
                }
                let t = self.apagar(tipo_padrao);
                Constante::Valor(Valor::novo(t, Estado::Null { invalido: true }))
            }
            r => r,
        }
    }

    /// `_followConstantRedirectionChain`.
    fn seguir_redirecionamentos(&mut self, mut f: FunctionElementId, mut tipo: TypeId) -> (FunctionElementId, TypeId) {
        let mut vistos = vec![f];
        loop {
            let fe = self.program.function(f);
            if !fe.factory {
                break;
            }
            if fe.class.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
                && Some(fe.library) == self.core.core_library
            {
                break;
            }
            let Some((g, t)) = self.alvo_de_redirecionamento(f, tipo) else { break };
            if !self.program.function(self.program.publico(g)).const_ || vistos.contains(&g) {
                break;
            }
            vistos.push(g);
            f = g;
            tipo = t;
        }
        (f, tipo)
    }

    /// O construtor para o qual a factory `f` redireciona (`= C.nome`), e o
    /// tipo instanciado.
    fn alvo_de_redirecionamento(&mut self, f: FunctionElementId, tipo: TypeId) -> Option<(FunctionElementId, TypeId)> {
        let FunctionRef::Constructor { unit, member } = self.program.function(f).node else { return None };
        let ast::MemberKind::Constructor(k) = &self.ast(unit).member(member).kind else { return None };
        let r = k.redirect.as_ref()?;
        let ast::TypeKind::Named { name, .. } = &self.ast(unit).ty(r.ty).kind else { return None };
        // `C.nome` sem prefixo de import é a classe `C` e o construtor `nome`
        // (o parser não distingue de um tipo prefixado).
        let mut construtor = r.constructor.map(|n| n.sym);
        let b = match &name[..] {
            [n] => self.program.lookup_na_unidade(unit, n.sym),
            [p, n] => match self.program.lookup_prefixed_na_unidade(unit, p.sym, n.sym) {
                Some(b) => Some(b),
                None if construtor.is_none() => {
                    construtor = Some(n.sym);
                    self.program.lookup_na_unidade(unit, p.sym)
                }
                None => None,
            },
            _ => None,
        }?;
        let Some(Element::Class(alvo)) = b.getter else { return None };
        let chave = match construtor {
            Some(n) => n,
            None => self.interner.lookup("")?,
        };
        let g = *self.program.class(alvo).constructors.get(&chave)?;
        // Os argumentos de tipo passam pela declaração: o alvo é tratado com
        // os do tipo original quando a classe é a mesma; senão, cru.
        let t = match self.table.get(tipo) {
            Type::Interface { class, .. } if *class == alvo => tipo,
            _ => {
                let n = self.program.class(alvo).type_params.len();
                self.table.intern(Type::Interface { class: alvo, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        Some((g, t))
    }

    /// `evaluateFactoryConstructorCall`.
    fn chamar_factory(
        &mut self,
        erro: ErroEm,
        f: FunctionElementId,
        tipo: TypeId,
        posicionais: &[Valor],
        nomeados: &[(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)],
    ) -> R {
        let fe = self.program.function(f);
        let nome = fe.name;
        let classe = fe.class;
        let de_core = Some(fe.library) == self.core.core_library;
        let primeiro = posicionais.first();
        let n_args = posicionais.len() + nomeados.len();
        let excecao = |m: &Motor<'_>| Constante::Invalida(Box::new(m.erro(erro.unidade, erro.span, c::CONST_EVAL_THROWS_EXCEPTION)));
        if de_core && Some(nome) == self.sym_from_environment {
            // `_checkFromEnvironmentArguments`.
            let ok = (1..=2).contains(&n_args)
                && posicionais.len() == 1
                && primeiro.is_some_and(|p| p.tipo == self.core.string)
                && (nomeados.is_empty()
                    || nomeados.len() == 1
                        && Some(nomeados[0].0) == self.sym_default_value
                        && (nomeados[0].1.tipo == tipo || nomeados[0].1.tipo == self.core.null));
            if !ok {
                return excecao(self);
            }
            return Constante::Valor(self.desconhecido(tipo));
        }
        if de_core && Some(nome) == self.sym_has_environment && classe.is_some() && classe == self.core.bool_class {
            return Constante::Valor(Valor::bool_(self.core, None));
        }
        if de_core
            && self.interner.resolve(nome).is_empty()
            && classe.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
            && n_args == 1
        {
            let ok = posicionais.len() == 1 && primeiro.is_some_and(|p| p.tipo == self.core.string && p.como_texto().is_some());
            if !ok {
                return excecao(self);
            }
            let s = String::from_utf16_lossy(primeiro.unwrap().como_texto().unwrap());
            return Constante::Valor(Valor::novo(tipo, Estado::Simbolo(Some(s.into()))));
        }
        let t = self.apagar(tipo);
        Constante::Valor(self.desconhecido(t))
    }

    /// `evaluateGenerativeConstructorCall`.
    #[allow(clippy::too_many_arguments)]
    fn chamar_gerador(
        &mut self,
        erro: ErroEm,
        f: FunctionElementId,
        k: ClassId,
        tipo: TypeId,
        posicionais: Vec<Valor>,
        alvos_posicionais: Vec<Option<(UnitId, Span, TypeId)>>,
        nomeados: Vec<(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)>,
    ) -> R {
        let lib = self.program.function(f).library;
        // Substituição dos parâmetros de tipo da classe.
        let params_classe = self.outline.classes.get(k.0 as usize).map(|d| d.type_params.clone()).unwrap_or_default();
        let args_tipo: Vec<TypeId> = match self.table.get(tipo) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        let mut mapa_tipos: HashMap<TypeParamId, TypeId> = HashMap::new();
        if params_classe.len() == args_tipo.len() {
            for (p, a) in params_classe.iter().zip(args_tipo.iter()) {
                mapa_tipos.insert(*p, *a);
            }
        }
        let subst = |m: &mut Motor<'_>, t: TypeId| -> TypeId {
            if mapa_tipos.is_empty() { t } else { crate::ops::substitute(t, &mapa_tipos, m.table) }
        };
        let mut campos: Vec<(Campo, Valor)> = Vec::new();
        // `_checkFields`: campos `final` com inicializador na declaração.
        let fields = self.program.class(k).fields.clone();
        for v in fields {
            let var = self.program.variable(v);
            if !(var.final_ || var.const_) || var.static_ || var.late {
                continue;
            }
            let tem_init = match var.node {
                VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                    ast::MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
                    _ => None,
                },
                _ => None,
            };
            let Some((fu, fx)) = tem_init else { continue };
            let Some(Constante::Valor(x)) = self.valor_de_variavel(v) else { continue };
            let tipo_campo = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred).unwrap_or(self.core.dynamic_);
            let tipo_campo = subst(self, tipo_campo);
            if !self.casa(&x, tipo_campo) {
                let excecao = self.menciona_parametro(tipo_campo);
                let (eu, es) = if fu == erro.unidade || true { (fu, self.span(fu, fx)) } else { (erro.unidade, erro.span) };
                let mut i = self.erro(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                i.args = vec![self.formatar(x.tipo), self.interner.resolve(var.name).to_string(), self.formatar(tipo_campo)];
                i.excecao = excecao;
                return Constante::Invalida(Box::new(i));
            }
            campos.push((Campo::Nome(var.name), x));
        }
        // `_checkParameters`.
        let FunctionRef::Constructor { unit: cu, member } = self.program.function(f).node else {
            // Construtor sintético: sem parâmetros nem inicializadores.
            let g = self.completar_gerador(erro, f, k, tipo, lib, campos, None, Vec::new(), HashMap::new(), mapa_tipos);
            return self.terminar_gerador(g);
        };
        let ast::MemberKind::Constructor(ctor) = &self.ast(cu).member(member).kind else {
            return Constante::Valor(self.desconhecido(tipo));
        };
        let params_tipos = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut lexico: HashMap<SymbolId, Valor> = HashMap::new();
        let mut i_pos = 0usize;
        for (i, p) in ctor.parameters.iter().enumerate() {
            let Some(nome) = p.name else { continue };
            let externo = p.nome_externo().map(|n| n.sym).unwrap_or(nome.sym);
            let tipo_p = params_tipos.get(i).map(|d| d.ty).unwrap_or(self.core.dynamic_);
            let tipo_p = subst(self, tipo_p);
            let (mut valor, alvo) = if p.kind == ast::ParameterKind::Named {
                match nomeados.iter().find(|(n, _, _)| *n == externo) {
                    Some((_, v, a)) => (Some(v.clone()), *a),
                    None => (None, None),
                }
            } else {
                let r = (posicionais.get(i_pos).cloned(), alvos_posicionais.get(i_pos).copied().flatten());
                i_pos += 1;
                r
            };
            // Só o opcional (`isOptional`) recebe o padrão; o nomeado
            // `required` ausente fica sem valor.
            if valor.is_none() && p.kind != ast::ParameterKind::Required && !p.required {
                valor = match p.default_value {
                    None => Some(Valor::nulo(self.core)),
                    Some(d) => match self.valor_padrao(cu, lib, d) {
                        Constante::Valor(v) => Some(v),
                        Constante::Invalida(_) => None,
                    },
                };
            }
            let Some(valor) = valor else { continue };
            let (au, aspan, aestatico) = match alvo {
                Some((u, s, t)) => (u, s, Some(t)),
                None => (erro.unidade, erro.span, None),
            };
            let invalido = matches!(valor.estado, Estado::Null { invalido: true });
            if !invalido && !self.casa(&valor, tipo_p) {
                let excecao = aestatico.is_some_and(|t| self.atribuivel(t, tipo_p));
                let mut e = self.erro(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                e.args = vec![self.formatar_sem_alias(valor.tipo), self.formatar_sem_alias(tipo_p)];
                e.excecao = excecao;
                return Constante::Invalida(Box::new(e));
            }
            if p.this_ {
                let campo = self.program.class(k).fields.iter().copied().find(|v| self.program.variable(*v).name == nome.sym && !self.program.variable(*v).static_);
                if let Some(campo) = campo {
                    let tipo_campo = self.outline.variables[campo.0 as usize].declared_type.or(self.outline.variables[campo.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                    let tipo_campo = subst(self, tipo_campo);
                    if tipo_campo != tipo_p && !invalido && !self.casa(&valor, tipo_campo) {
                        let mut e = self.erro(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                        e.args = vec![self.formatar_sem_alias(valor.tipo), self.formatar_sem_alias(tipo_campo)];
                        return Constante::Invalida(Box::new(e));
                    }
                    if campos.iter().any(|(c, _)| *c == Campo::Nome(nome.sym)) {
                        return Constante::Invalida(Box::new(self.erro(erro.unidade, erro.span, c::CONST_EVAL_THROWS_EXCEPTION)));
                    }
                    campos.push((Campo::Nome(nome.sym), valor.clone()));
                }
            }
            lexico.insert(nome.sym, valor);
        }
        // Parâmetros `super.x`: argumentos implícitos da chamada ao super.
        let mut super_implicitos: (Vec<Valor>, Vec<(SymbolId, Valor)>) = (Vec::new(), Vec::new());
        for p in ctor.parameters.iter() {
            if !p.super_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(v) = lexico.get(&n.sym).cloned() else { continue };
            if p.kind == ast::ParameterKind::Named {
                super_implicitos.1.push((n.sym, v));
            } else {
                super_implicitos.0.push(v);
            }
        }
        let inits: Vec<&ast::Initializer> = ctor.initializers.iter().collect();
        let g = self
            .completar_gerador(erro, f, k, tipo, lib, campos, Some((cu, inits)), super_implicitos.0, lexico, mapa_tipos)
            .con_super_nomeados(super_implicitos.1);
        self.terminar_gerador(g)
    }

    /// `_checkInitializers`, `_checkSuperConstructorCall` e o objeto.
    #[allow(clippy::too_many_arguments)]
    fn completar_gerador(
        &mut self,
        erro: ErroEm,
        f: FunctionElementId,
        k: ClassId,
        tipo: TypeId,
        lib: LibraryId,
        mut campos: Vec<(Campo, Valor)>,
        inits: Option<(UnitId, Vec<&ast::Initializer>)>,
        super_posicionais: Vec<Valor>,
        lexico: HashMap<SymbolId, Valor>,
        mapa_tipos: HashMap<TypeParamId, TypeId>,
    ) -> Gerador {
        let lexico = Rc::new(lexico);
        let tipos = Rc::new(mapa_tipos);
        let unidade_ctor = inits.as_ref().map(|(u, _)| *u).unwrap_or(erro.unidade);
        let cxi = Ctx { unidade: unidade_ctor, lib, lexico: Some(lexico.clone()), tipos: Some(tipos.clone()), relatar: false };
        let mut super_nome: Option<SymbolId> = None;
        let mut super_args: Option<(UnitId, Vec<ast::Argument>)> = None;
        if let Some((cu, inits)) = inits {
            for init in inits {
                match init {
                    ast::Initializer::Field { name, value, .. } => {
                        let r = self.avaliar(&cxi, *value, false);
                        match r {
                            Constante::Valor(v) => {
                                if campos.iter().any(|(c, _)| *c == Campo::Nome(name.sym)) {
                                    return Gerador::pronto(Constante::Invalida(Box::new(self.erro(erro.unidade, erro.span, c::CONST_EVAL_THROWS_EXCEPTION))));
                                }
                                let campo = self.program.class(k).fields.iter().copied().find(|x| self.program.variable(*x).name == name.sym && !self.program.variable(*x).static_);
                                if let Some(campo) = campo {
                                    let tc = self.outline.variables[campo.0 as usize].declared_type.or(self.outline.variables[campo.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                                    let tc = if tipos.is_empty() { tc } else { crate::ops::substitute(tc, &tipos, self.table) };
                                    if !self.casa(&v, tc) {
                                        let estatico = self.estatico(cu, *value);
                                        let excecao = self.atribuivel(estatico, tc);
                                        let (eu, es) = if excecao { (cu, self.span(cu, *value)) } else { (erro.unidade, erro.span) };
                                        let mut i = self.erro(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                                        i.args = vec![self.formatar(v.tipo), self.interner.resolve(name.sym).to_string(), self.formatar(tc)];
                                        i.excecao = excecao;
                                        return Gerador::pronto(Constante::Invalida(Box::new(i)));
                                    }
                                }
                                campos.push((Campo::Nome(name.sym), v));
                            }
                            Constante::Invalida(i) if !i.excecao => {
                                let n = self.erro_no_inicializador(&i, Some(f), "field initializer", erro);
                                return Gerador::pronto(Constante::Invalida(Box::new(n)));
                            }
                            r => return Gerador::pronto(r),
                        }
                    }
                    ast::Initializer::Super { constructor, arguments, .. } => {
                        super_nome = constructor.map(|n| n.sym);
                        super_args = Some((cu, arguments.args.to_vec()));
                    }
                    ast::Initializer::Redirect { constructor, arguments, .. } => {
                        let chave = constructor.map(|n| n.sym).or_else(|| self.interner.lookup(""));
                        let alvo = chave.and_then(|c| self.program.class(k).constructors.get(&c).copied());
                        if let Some(g) = alvo {
                            if self.program.function(self.program.publico(g)).const_ {
                                let r = self.avaliar_chamada(&cxi, erro, g, tipo, &Argumentos::Ast(arguments), None, false);
                                return Gerador::pronto(r);
                            }
                        }
                    }
                    ast::Initializer::Assert { condition, message, span } => {
                        let r = self.avaliar(&cxi, *condition, false);
                        match r {
                            Constante::Valor(v) => {
                                if !v.estado.e_bool() || v.como_bool() == Some(false) {
                                    let mut i = None;
                                    if let Some(m) = message {
                                        if let Constante::Valor(mv) = self.avaliar(&cxi, *m, false) {
                                            if let Some(t) = mv.como_texto() {
                                                let mut x = self.erro(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE);
                                                x.args = vec![String::from_utf16_lossy(t)];
                                                x.excecao = true;
                                                i = Some(x);
                                            }
                                        }
                                    }
                                    let mut i = i.unwrap_or_else(|| self.erro(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE));
                                    i.excecao = true;
                                    return Gerador::pronto(Constante::Invalida(Box::new(i)));
                                }
                            }
                            Constante::Invalida(i) if !i.excecao => {
                                let n = self.erro_no_inicializador(&i, Some(f), "assert initializer", erro);
                                return Gerador::pronto(Constante::Invalida(Box::new(n)));
                            }
                            r => return Gerador::pronto(r),
                        }
                    }
                }
            }
        }
        Gerador {
            motor_pronto: None,
            f: Some(f),
            erro,
            k,
            tipo,
            campos,
            super_nome,
            super_args,
            super_posicionais,
            super_nomeados: Vec::new(),
            cxi,
        }
    }

    /// `_checkSuperConstructorCall` e o objeto final.
    fn terminar_gerador(&mut self, g: Gerador) -> R {
        if let Some(r) = g.motor_pronto {
            return r;
        }
        let Gerador { f, erro, k, tipo, mut campos, super_nome, super_args, super_posicionais, super_nomeados, cxi, .. } = g;
        let classe = self.program.class(k);
        if let Some(sup) = classe.supertype_class {
            if Some(sup) != self.core.object_class {
                let chave = super_nome.or_else(|| self.interner.lookup(""));
                let alvo = chave.and_then(|c| self.program.class(sup).constructors.get(&c).copied());
                if let Some(g) = alvo {
                    if self.program.function(self.program.publico(g)).const_ {
                        let tipo_super = self.outline.classes.get(k.0 as usize).and_then(|d| d.supertype).unwrap_or(self.core.dynamic_);
                        let tipo_super = match cxi.tipos.as_ref() {
                            Some(m) if !m.is_empty() => crate::ops::substitute(tipo_super, m, self.table),
                            _ => tipo_super,
                        };
                        let r = match super_args {
                            Some((cu, args)) => {
                                let a = ast::Arguments { span: erro.span, type_args: Box::new([]), args: args.into_boxed_slice() };
                                let cx = Ctx { unidade: cu, ..cxi.clone() };
                                if super_posicionais.is_empty() && super_nomeados.is_empty() {
                                    self.avaliar_chamada(&cx, erro, g, tipo_super, &Argumentos::Ast(&a), None, false)
                                } else {
                                    // Argumentos escritos mais os de `super.x`.
                                    let mut pos = super_posicionais.clone();
                                    let mut nom: Vec<(SymbolId, Valor)> = super_nomeados.clone();
                                    for arg in a.args.iter() {
                                        match self.avaliar(&cx, arg.value, false) {
                                            Constante::Valor(v) => match arg.name {
                                                Some(n) => nom.push((n.sym, v)),
                                                None => pos.push(v),
                                            },
                                            i => return i,
                                        }
                                    }
                                    self.avaliar_chamada(&cx, erro, g, tipo_super, &Argumentos::Valores { posicionais: pos, nomeados: nom }, None, false)
                                }
                            }
                            None => self.avaliar_chamada(
                                &cxi,
                                erro,
                                g,
                                tipo_super,
                                &Argumentos::Valores { posicionais: super_posicionais, nomeados: super_nomeados },
                                None,
                                false,
                            ),
                        };
                        match r {
                            Constante::Valor(v) => campos.push((Campo::Super, v)),
                            Constante::Invalida(i) if !i.excecao => {
                                let mut n = if i.contexto.is_empty() {
                                    self.erro_no_inicializador(&i, f, "super constructor invocation", erro)
                                } else {
                                    let mut n = (*i).clone();
                                    n.unidade = erro.unidade;
                                    n.span = erro.span;
                                    n
                                };
                                if !i.contexto.is_empty() {
                                    if let Some(x) = f.and_then(|f| self.pilha_de_construtores(g, f)) {
                                        n.contexto.push(x);
                                    }
                                }
                                return Constante::Invalida(Box::new(n));
                            }
                            Constante::Invalida(mut i) => {
                                if let Some(x) = f.and_then(|f| self.pilha_de_construtores(g, f)) {
                                    i.contexto.push(x);
                                }
                                return Constante::Invalida(i);
                            }
                        }
                    }
                }
            }
        }
        if classe.kind == ClassKind::ExtensionType {
            if let Some(rep) = classe.representation {
                let nome = self.program.variable(rep).name;
                if let Some((_, v)) = campos.iter().find(|(c, _)| *c == Campo::Nome(nome)) {
                    return Constante::Valor(v.clone());
                }
            }
        }
        let t = self.apagar(tipo);
        Constante::Valor(Valor::novo(t, Estado::Generico { campos: Rc::new(campos), desconhecido: false }))
    }

    /// Valor padrão de um parâmetro (guardado pela expressão).
    fn valor_padrao(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constante {
        if let Some(r) = self.padroes.get(&(u, d)) {
            return r.clone();
        }
        let cx = Ctx::simples(u, lib);
        let salvo = std::mem::take(&mut self.args_de_criacao);
        let r = self.avaliar(&cx, d, false);
        self.args_de_criacao = salvo;
        self.padroes.insert((u, d), r.clone());
        r
    }

    /// O resultado da avaliação do padrão `d` (para o verificador).
    pub fn resultado_padrao(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constante {
        self.valor_padrao(u, lib, d)
    }
}

/// Onde um erro de construtor é relatado (`_errorNode`).
#[derive(Clone, Copy, Debug)]
pub struct ErroEm {
    pub unidade: UnitId,
    pub span: Span,
}

/// Os argumentos de uma chamada de construtor: escritos, ou já avaliados
/// (super implícito, `super.x`).
enum Argumentos<'x> {
    Ast(&'x ast::Arguments),
    Valores { posicionais: Vec<Valor>, nomeados: Vec<(SymbolId, Valor)> },
}

/// O estado entre os inicializadores e a chamada ao super.
struct Gerador {
    motor_pronto: Option<Constante>,
    /// O construtor avaliado (`_constructor`), para as mensagens de contexto.
    f: Option<FunctionElementId>,
    erro: ErroEm,
    k: ClassId,
    tipo: TypeId,
    campos: Vec<(Campo, Valor)>,
    super_nome: Option<SymbolId>,
    super_args: Option<(UnitId, Vec<ast::Argument>)>,
    super_posicionais: Vec<Valor>,
    super_nomeados: Vec<(SymbolId, Valor)>,
    cxi: Ctx,
}

impl Gerador {
    fn pronto(r: Constante) -> Gerador {
        Gerador {
            motor_pronto: Some(r),
            f: None,
            erro: ErroEm { unidade: UnitId(0), span: Span { start: 0, end: 0 } },
            k: ClassId(0),
            tipo: TypeId(0),
            campos: Vec::new(),
            super_nome: None,
            super_args: None,
            super_posicionais: Vec::new(),
            super_nomeados: Vec::new(),
            cxi: Ctx::simples(UnitId(0), LibraryId(0)),
        }
    }

    fn con_super_nomeados(mut self, n: Vec<(SymbolId, Valor)>) -> Gerador {
        self.super_nomeados = n;
        self
    }
}

/// Recua de `i` sobre espaços até o byte `b` (inclusive), se ele vier logo antes.
fn recuar_ate(fonte: &[u8], i: usize, b: u8) -> usize {
    let mut j = i;
    while j > 0 && fonte[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j > 0 && fonte[j - 1] == b { j - 1 } else { i }
}

/// Recua de `i` sobre espaços até uma das palavras (a primeira que casar).
fn recuar_palavra(fonte: &[u8], i: usize, palavras: &[&str]) -> usize {
    let mut j = i;
    while j > 0 && fonte[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    // `if (cond)`: pula o `(`.
    if j > 0 && fonte[j - 1] == b'(' {
        j -= 1;
        while j > 0 && fonte[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
    }
    for p in palavras {
        let p = p.as_bytes();
        if j >= p.len() && &fonte[j - p.len()..j] == p {
            return j - p.len();
        }
    }
    i
}

/// O início do `for` que envolve a posição `i` (o `for` mais próximo antes).
fn inicio_de_for(fonte: &[u8], i: usize) -> usize {
    let mut j = i.min(fonte.len());
    let mut prof = 0i32;
    while j > 0 {
        j -= 1;
        match fonte[j] {
            b')' => prof += 1,
            b'(' => {
                if prof == 0 {
                    let mut k = j;
                    while k > 0 && fonte[k - 1].is_ascii_whitespace() {
                        k -= 1;
                    }
                    if k >= 3 && &fonte[k - 3..k] == b"for" {
                        let mut s = k - 3;
                        // `await for`
                        let mut w = s;
                        while w > 0 && fonte[w - 1].is_ascii_whitespace() {
                            w -= 1;
                        }
                        if w >= 5 && &fonte[w - 5..w] == b"await" {
                            s = w - 5;
                        }
                        return s;
                    }
                    return i;
                }
                prof -= 1;
            }
            _ => {}
        }
    }
    i
}

/// As constantes locais de uma unidade: pelo offset do nome, o comando
/// (`Variables` ou `for`) e o índice na lista.
pub(super) fn locais_constantes(a: &ast::Ast) -> HashMap<usize, (ast::StmtId, usize)> {
    let mut m = HashMap::new();
    for (i, s) in a.stmts.iter().enumerate() {
        let lista = match &s.kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => continue,
        };
        if !lista.const_ {
            continue;
        }
        for (j, v) in lista.variables.iter().enumerate() {
            m.insert(v.name.span.start, (ast::StmtId(i as u32), j));
        }
    }
    m
}

/// Os offsets dos nomes de todos os parâmetros formais de uma unidade
/// (funções, construtores, tipos de função e parâmetros-função).
fn offsets_de_parametros(a: &ast::Ast) -> HashSet<usize> {
    fn params(ps: &[ast::Parameter], s: &mut HashSet<usize>) {
        for p in ps {
            if let Some(n) = p.name {
                s.insert(n.span.start);
            }
            if let Some(f) = &p.function_parameters {
                params(f, s);
            }
        }
    }
    let mut s = HashSet::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            params(ps, &mut s);
        }
    }
    for m in a.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            params(&k.parameters, &mut s);
        }
    }
    s
}
