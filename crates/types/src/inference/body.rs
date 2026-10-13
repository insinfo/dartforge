//! Estado de um corpo em inferência: locais, escopos léxicos, parâmetros de
//! tipo visíveis, pilha de funções (para `return`/`yield`), receptores de
//! cascata e o modelo de fluxo corrente.

use super::flow::Fluxo;
use super::BodyInferrer;
use crate::resolved::LocalId;
use crate::table::{TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ExtensionId, FunctionElementId, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// Uma variável local (inclusive parâmetros, variáveis de padrão e funções locais).
#[derive(Debug, Clone)]
pub(crate) struct Local {
    pub nome: SymbolId,
    /// Tipo declarado (escrito ou inferido do inicializador).
    pub tipo: TypeId,
    pub final_: bool,
    pub late: bool,
    pub const_: bool,
    /// Offset do nome na declaração.
    pub offset: usize,
    /// Tipo ainda não conhecido (`var x;` sem inicializador tem `dynamic`,
    /// mas uma função local sendo inferida não está disponível).
    pub funcao_local: bool,
}

/// O que um nome resolve num escopo de bloco.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Nome {
    Local(LocalId),
    TypeParam(TypeParamId),
    /// Local declarado mais adiante no mesmo bloco: o escopo do bloco já o
    /// contém, e usá-lo antes da declaração é erro. O intervalo do nome na
    /// declaração.
    Forward(dartforge_diagnostics::Span),
}

/// Contexto da função (ou expressão de função) que envolve o ponto atual.
#[derive(Debug, Clone)]
pub(crate) struct FunctionCtx {
    pub modificador: ast::AsyncModifier,
    /// O `imposedType` do `BodyInferenceContext` de um literal de função: o
    /// retorno do tipo de função do contexto (salvo `dynamic`/`_`), contra o
    /// qual o `yield` é conferido (`function_expression_resolver.dart:37-48`).
    pub closure_imposed: Option<TypeId>,
    /// Tipo de retorno declarado (ou imposto pelo contexto); `None` quando
    /// o retorno está sendo inferido.
    pub retorno: Option<TypeId>,
    /// Esquema de contexto para as expressões de `return`/`yield`.
    pub return_context: TypeId,
    /// Tipos das expressões retornadas (ou `yield`adas), para inferência.
    pub returned: Vec<TypeId>,
    /// Há `return;` sem valor.
    pub empty_return: bool,
    /// As palavras `return` dos `return;` sem valor de uma closure, para o
    /// `_checkReturnWithoutValue` depois de inferido o tipo de retorno dela.
    pub valueless_returns: Vec<dartforge_diagnostics::Span>,
    /// As expressões de `return e;` (e o corpo `=> e`) com os tipos, para a
    /// conferência de uma closure cujo retorno acaba sendo o do contexto.
    pub returned_expressions: Vec<(ast::ExprId, TypeId)>,
    /// O executável que declara o retorno, para `return_of_invalid_type`;
    /// `None` em closures e construtores geradores (outras regras).
    pub executavel: Option<Executavel>,
    /// `hasLegalReturnType`: falso depois de `illegal_*_return_type`, que
    /// cala a conferência das expressões retornadas.
    pub legal_return: bool,
}

/// Espécie e nome de exibição do executável (`EnclosingExecutableContext`
/// do analyzer): o `return_of_invalid_type` diz "function", "method" ou
/// "constructor" e o nome.
#[derive(Debug, Clone)]
pub(crate) struct Executavel {
    pub especie: ExecutableKind,
    pub nome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutableKind {
    /// Função de topo ou local com nome, e getters/setters (de topo ou de
    /// classe): `PropertyAccessorElement` conta como função.
    Funcao,
    /// Método ou operador de classe ou extensão.
    Metodo,
    /// Construtor factory.
    Construtor,
}

/// Estado de inferência de um corpo.
pub(crate) struct Body {
    pub unit: UnitId,
    pub lib: LibraryId,
    pub classe: Option<ClassId>,
    pub extensao: Option<ExtensionId>,
    /// Sem `this` (membro estático, topo, construtor de fábrica...).
    pub estatico: bool,
    pub locais: Vec<Local>,
    /// Declarações de funções locais (distintas de variáveis finais com tipo de função).
    pub funcoes_locais: HashSet<LocalId>,
    pub escopos: Vec<Vec<(SymbolId, Nome)>>,
    pub flow: Fluxo,
    pub funcoes: Vec<FunctionCtx>,
    pub cascades: Vec<TypeId>,
    /// O alvo de cada cascata aberta (o `realTarget` das seções).
    pub cascade_targets: Vec<ast::ExprId>,
    /// A base de promoção de cada alvo de cascata e a versão dela no início
    /// da cascata: o alvo vive num temporário implícito com o nó SSA da
    /// expressão (`cascadeExpression_afterTarget`), que uma escrita na
    /// variável durante as seções não muda.
    pub cascade_bases: Vec<Option<(Base, u32)>>,
    /// No inicializador de um campo de instância não `late` ou de uma
    /// variável estática de classe ou extensão (`_isInInstanceNotLateVariableDeclaration`,
    /// `_isInStaticVariableDeclaration`): o membro de instância sem `this` é
    /// `IMPLICIT_THIS_REFERENCE_IN_INITIALIZER`.
    pub variable_initializer: bool,
    /// A versão forçada de uma base enquanto se lê uma propriedade pelo
    /// alvo de cascata ([`Corpo::versao_da_base`]).
    pub forced_version: Option<(Base, u32)>,
    /// Tipando o padrão de um `case`/`if-case` (refutável): o identificador
    /// solto é uma constante (`case _padrao:`), não uma variável nova.
    pub padrao_refutavel: bool,
    /// Um padrão refutável já relatado num contexto irrefutável
    /// (`refutablePatternInIrrefutableContext`): o que está abaixo dele é
    /// analisado como refutável (`context.makeRefutable()`), sem relatar de
    /// novo nem conferir o tipo requerido.
    pub forced_refutable: bool,
    /// O literal inteiro operando de um `-` unário em análise
    /// (`IntegerLiteral.immediatelyNegated`).
    pub negated_literal: Option<ast::ExprId>,
    /// As variáveis do padrão guardado cuja cláusula `when` está em análise
    /// (`isVisitingWhenClause`): escrever nelas é
    /// `PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD`.
    pub variables_in_guard: Vec<LocalId>,
    /// As variáveis do padrão guardado em curso (nome e declaração), que
    /// ficam escondidas durante todo o padrão
    /// (`HiddenElements.forGuardedPattern`, `error_verifier.dart:6570-6575`):
    /// lê-las dentro do padrão é `REFERENCED_BEFORE_DECLARATION`.
    pub pattern_hidden: Vec<(SymbolId, dartforge_diagnostics::Span)>,
    /// Os locais declarados pelo padrão do último `case` (sem os da
    /// guarda), para as variáveis de junção dos casos que dividem o corpo.
    pub last_pattern_locals: std::ops::Range<usize>,
    /// Variáveis de junção inconsistentes dos casos que dividem um corpo,
    /// com o código que cada referência relata (`finishJoinedPatternVariable`).
    pub inconsistent_joins: HashMap<LocalId, dartforge_diagnostics::Codigo>,
    /// O casamento de padrão em curso (T6: a referência do valor casado e o
    /// estado "não casou"); `None` fora de padrão.
    pub matching: Option<super::patterns::Matching>,
    /// Dentro de um `switch` (instrução ou expressão): o escrutínio já
    /// casado por um caso — a expressão, a chave de promoção do valor e a
    /// versão de escrita da variável escrutinada —, que os casos seguintes
    /// reusam. `None` fora de `switch`; `Some(None)` antes do primeiro caso.
    pub switch_scrutinee: Option<Option<(ast::ExprId, LocalId, Option<u32>)>>,
    /// Corpo de um método ou inicializador de campo estático: os parâmetros
    /// de tipo da classe não valem ali (`TYPE_PARAMETER_REFERENCED_BY_STATIC`).
    pub static_member: bool,
    /// Pilha de alvos de `break`/`continue` (rótulos e laços): modelos de
    /// fluxo acumulados nos saltos.
    pub saltos: Vec<JumpTarget>,
    /// Tipo de `this` (classe, mixin, enum, tipo de extensão ou `on` da extensão).
    pub tipo_this: Option<TypeId>,
    /// Nomes escritos dentro de closures (captura de escrita, que impede promoção).
    pub written_in_closure: Vec<super::statements::Escrita>,
    /// Rótulos da instrução rotulada cujo corpo é o próximo laço/`switch`.
    pub pending_labels: Vec<SymbolId>,
    /// O símbolo `_` quando a biblioteca tem curingas (Dart 3.7): declarar
    /// `_` cria o local (tem `LocalId` e tipo) mas não liga o nome.
    pub curinga: Option<SymbolId>,
    /// Atalhos de ponto (3.10): o contexto da cadeia de seletores, pela
    /// expressão `DotShorthand` da raiz (ver `atalhos`).
    pub shorthand_context: HashMap<u32, TypeId>,
    /// Raízes `DotShorthand` de uma invocação cujo erro já foi relatado
    /// (`atalhos::construcao`): o [`super::atalhos::valor`] do alvo não relata de novo.
    pub reported_shorthands: std::collections::HashSet<u32>,
    /// Nomes escritos em qualquer ponto do corpo de topo: dentro de uma
    /// closure eles não ficam promovidos (`functionExpression_begin` faz a
    /// junção conservadora com `assignedVariables.anywhere`).
    /// Calculado na primeira closure (a maioria dos corpos não tem nenhuma).
    /// `(escritos fora de literais, escritos dentro de literais)` do membro.
    pub written_in_body: Option<(Vec<super::statements::Escrita>, Vec<super::statements::Escrita>)>,
    /// O corpo de topo, para calcular `escritos_no_corpo` sob demanda.
    pub raiz: Raiz,
    /// Propriedades promovíveis (Dart 3.2) já referidas: `(base, versão de
    /// escrita da base, nome) -> local sintético` que carrega o modelo de
    /// fluxo da propriedade (o `_promotableProperties` do nó SSA da base).
    pub campos: HashMap<(Base, u32, SymbolId), LocalId>,
    /// Propriedades não promovíveis: as gerações (uma por leitura que virou
    /// alvo de promoção), da mais antiga à mais nova (o
    /// `_nonPromotableProperties` com o `previousSsaNode` de cada nó).
    pub generations: HashMap<(Base, u32, SymbolId), Vec<LocalId>>,
    /// A geração de cada leitura de propriedade não promovível.
    pub read_generation: HashMap<ast::ExprId, LocalId>,
    /// O local sintético de `this` (a `thisPromotionKey`), que guarda as
    /// promoções que `this` nunca usa (só o why-not-promoted as vê).
    pub local_this: Option<LocalId>,
    /// Fluxos de antes de cada `?.` das cadeias em curso.
    pub cadeias: Vec<super::flow::Fluxo>,
    /// Variáveis de condição (§7.10): `(verdadeiro, falso, versão)` do
    /// valor escrito na variável, restaurados na leitura como condição.
    pub conditions: HashMap<LocalId, (Fluxo, Fluxo, u32)>,
    /// Sobreposições explícitas de extensão (`E(x)`, R-EXT-02): a chamada
    /// `E(x)` → a extensão e os argumentos de tipo dela.
    pub overlays: HashMap<ast::ExprId, (ExtensionId, Vec<TypeId>)>,
    /// Fins dos blocos básicos em curso (o `flowEnd` do
    /// `NullSafetyDeadCodeVerifier`: corpo de função, ramo de `if`, corpo de
    /// laço, `try`/`catch`), já aparados na última instrução do bloco.
    pub flow_ends: Vec<usize>,
    /// O fim da última instrução de cada bloco em curso: sem bloco básico
    /// aberto, é onde o trecho morto de uma expressão termina (o
    /// `flowEnd` do corpo, aparado na última instrução).
    pub block_ends: Vec<usize>,
    /// Os parâmetros do construtor primário vistos nos inicializadores dele
    /// e nos de campo não `late` (o `_thisContext` `constructorInitializers`
    /// e `instanceFieldDeclaration` do `ErrorVerifier` 3.13): escrever num
    /// deles ali é `ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER`.
    pub primary_parameters: HashSet<LocalId>,
    /// O `this` sem o tipo da declaração: o cabeçalho de um construtor
    /// primário (3.13, fora do corpo da classe) e a lista de inicialização
    /// (onde a recuperação do parser do analyzer difere da nossa em
    /// `this?.x()`).
    pub untyped_this: bool,
    /// Profundidade em `fins_de_fluxo` onde começou o trecho morto em curso
    /// (`_firstDeadNode`): enquanto houver um, outro nó inalcançável é parte
    /// do mesmo trecho.
    pub dead_code: Option<usize>,
    /// Onde o trecho morto em curso começou: a própria instrução (ramo
    /// inalcançável) ou o bloco que a contém. O `for` relata as
    /// atualizações quando é o corpo dele (`_reportForUpdaters`).
    pub dead_origin: Option<ast::StmtId>,
}

/// Base de uma referência a campo promovível.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Base {
    This,
    /// `super` (o `_superSsaNode`: as propriedades dele são outras que as
    /// de `this`).
    Super,
    Local(LocalId),
}

/// Corpo de topo em inferência.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Raiz {
    Nada,
    Funcao(ast::FunctionId),
    Construtor(ast::MemberId),
}

/// Destino de saltos.
pub(crate) struct JumpTarget {
    pub rotulos: Vec<SymbolId>,
    /// É laço ou `switch` (alvo de `break`/`continue` sem rótulo).
    pub laco: bool,
    pub e_switch: bool,
    pub breaks: Vec<Fluxo>,
    pub continues: Vec<Fluxo>,
}

impl Body {
    pub fn novo(inf: &mut BodyInferrer<'_>, unit: UnitId, class: Option<ClassId>, extension: Option<ExtensionId>, static_: bool) -> Self {
        let lib = inf.program.unit(unit).library;
        inf.current_unit = Some(unit);
        let mut cx = Body {
            unit,
            lib,
            classe: class,
            extensao: extension,
            estatico: static_,
            locais: Vec::new(),
            funcoes_locais: HashSet::new(),
            escopos: vec![Vec::new()],
            flow: Fluxo::alcancavel(),
            funcoes: Vec::new(),
            cascades: Vec::new(),
            cascade_targets: Vec::new(),
            cascade_bases: Vec::new(),
            variable_initializer: false,
            forced_version: None,
            padrao_refutavel: false,
            forced_refutable: false,
            negated_literal: None,
            variables_in_guard: Vec::new(),
            pattern_hidden: Vec::new(),
            last_pattern_locals: 0..0,
            inconsistent_joins: HashMap::new(),
            matching: None,
            switch_scrutinee: None,
            static_member: false,
            saltos: Vec::new(),
            tipo_this: None,
            written_in_closure: Vec::new(),
            pending_labels: Vec::new(),
            curinga: None,
            shorthand_context: HashMap::new(),
            reported_shorthands: std::collections::HashSet::new(),
            written_in_body: None,
            raiz: Raiz::Nada,
            campos: HashMap::new(),
            generations: HashMap::new(),
            read_generation: HashMap::new(),
            local_this: None,
            cadeias: Vec::new(),
            conditions: HashMap::new(),
            overlays: HashMap::new(),
            flow_ends: Vec::new(),
            block_ends: Vec::new(),
            primary_parameters: HashSet::new(),
            untyped_this: false,
            dead_code: None,
            dead_origin: None,
        };
        if inf.program.library(lib).features.tem(dartforge_frontend::Feature::WildcardVariables) {
            cx.curinga = inf.interner.lookup("_");
        }
        // Parâmetros de tipo da classe/extensão estão sempre em escopo
        // (mesmo em membros estáticos, onde usá-los é erro).
        if let Some(c) = class {
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            for p in ps.iter() {
                let name = inf.table.param(*p).name;
                cx.escopos[0].push((name, Nome::TypeParam(*p)));
            }
            if !static_ {
                cx.tipo_this = Some(inf.class_this_type(c));
            }
        } else if let Some(e) = extension {
            let ps = inf.outline.extensions[e.0 as usize].type_params.clone();
            for p in ps.iter() {
                let name = inf.table.param(*p).name;
                cx.escopos[0].push((name, Nome::TypeParam(*p)));
            }
            if !static_ {
                cx.tipo_this = Some(inf.outline.extensions[e.0 as usize].on);
            }
        }
        cx.escopos.push(Vec::new());
        cx
    }

    /// Corpo para o inicializador de uma variável de topo ou campo.
    pub fn to_variable(inf: &mut BodyInferrer<'_>, vid: VariableId, unit: UnitId) -> Self {
        let v = inf.program.variable(vid);
        let (class, extension) = (v.class, v.extension);
        // Inicializadores de campos de instância não veem `this` (só os `late`).
        let static_ = v.static_ || (class.is_none() && extension.is_none()) || !v.late;
        let mut cx = Body::novo(inf, unit, class, extension, static_);
        cx.static_member = v.static_ && (class.is_some() || extension.is_some());
        cx.variable_initializer = (class.is_some() || extension.is_some()) && (v.static_ || !v.late);
        if !v.static_ && v.late {
            cx.estatico = false;
            if let Some(c) = class {
                cx.tipo_this = Some(inf.class_this_type(c));
            }
        }
        // O campo de instância não `late` vê os parâmetros do construtor
        // primário.
        if !v.static_ && !v.late
            && let Some(c) = class
        {
            inf.declare_primary_parameters(&mut cx, c);
        }
        cx
    }

    /// Corpo de uma função declarada (método, função de topo, construtor).
    pub fn to_function(inf: &mut BodyInferrer<'_>, f: FunctionElementId, unit: UnitId) -> Self {
        let fe = inf.program.function(f);
        let (class, extension) = (fe.class, fe.extension);
        let static_ = fe.static_ && !matches!(fe.kind, dartforge_elements::model::FunctionKind::Constructor) || fe.factory;
        let static_ = static_ || (class.is_none() && extension.is_none());
        let static_member = fe.static_
            && !fe.factory
            && !matches!(fe.kind, dartforge_elements::model::FunctionKind::Constructor)
            && (class.is_some() || extension.is_some());
        let mut cx = Body::novo(inf, unit, class, extension, static_);
        cx.static_member = static_member;
        cx
    }

    pub fn push_scope(&mut self) {
        self.escopos.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        if self.escopos.len() > 1 {
            self.escopos.pop();
        }
    }

    /// Declara um local no escopo mais interno.
    pub fn declarar(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locais.len() as u32);
        let name = local.nome;
        self.locais.push(local);
        if self.curinga == Some(name) {
            self.flow.declarar(id);
            return id;
        }
        if let Some(e) = self.escopos.last_mut() {
            e.push((name, Nome::Local(id)));
        }
        self.flow.declarar(id);
        id
    }

    /// Registra um nome declarado adiante no bloco corrente.
    pub fn declare_forward(&mut self, name: SymbolId, span: dartforge_diagnostics::Span) {
        if self.curinga == Some(name) {
            return;
        }
        if let Some(e) = self.escopos.last_mut() {
            e.push((name, Nome::Forward(span)));
        }
    }

    /// Local sintético (sem nome no escopo): alvo de promoção de um campo.
    pub fn declare_synthetic(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locais.len() as u32);
        self.locais.push(local);
        self.flow.declarar(id);
        self.flow.inicializar(id);
        id
    }

    /// Esquece os campos promovidos de uma local reatribuída.
    pub fn forget_fields_of(&mut self, base: LocalId) {
        // Numa cascata sobre esta local, o temporário do alvo continua com o
        // nó SSA antigo: os campos da versão de antes ficam para as seções.
        if self.cascade_bases.iter().any(|x| matches!(x, Some((Base::Local(b), _)) if *b == base)) {
            let current = self.base_version(Base::Local(base));
            let stored: Vec<u32> = self.cascade_bases.iter().filter_map(|x| match x {
                Some((Base::Local(b), v)) if *b == base => Some(*v),
                _ => None,
            }).collect();
            self.campos.retain(|(b, v, _), _| *b != Base::Local(base) || (stored.contains(v) && *v != current));
            return;
        }
        self.campos.retain(|(b, _, _), _| *b != Base::Local(base));
    }

    /// O local é o sintético de uma propriedade (estável ou geração).
    pub fn is_property(&self, id: LocalId) -> bool {
        self.campos.values().any(|&v| v == id) || self.read_generation.values().any(|&v| v == id)
    }

    /// `infoFor`: um sintético cujo modelo se perdeu numa junção (criado num
    /// só dos ramos) volta como novo.
    pub fn ensure_model(&mut self, id: LocalId) {
        if self.flow.modelo(id).is_none() {
            self.flow.declarar(id);
            self.flow.inicializar(id);
        }
    }

    /// A versão de escrita da base de uma propriedade: a da local (o nó SSA
    /// dela); `this`, `super` e as propriedades estáveis não mudam.
    pub fn base_version(&self, base: Base) -> u32 {
        if let Some((b, v)) = self.forced_version
            && b == base
        {
            return v;
        }
        match base {
            Base::Local(id) if !self.is_property(id) => self.flow.versao(id).unwrap_or(0),
            _ => 0,
        }
    }

    pub fn declare_type_param(&mut self, name: SymbolId, p: TypeParamId) {
        if let Some(e) = self.escopos.last_mut() {
            e.push((name, Nome::TypeParam(p)));
        }
    }

    /// Nome nos escopos de bloco (do mais interno para fora).
    pub fn buscar(&self, name: SymbolId) -> Option<Nome> {
        for e in self.escopos.iter().rev() {
            for (n, r) in e.iter().rev() {
                if *n == name {
                    return Some(*r);
                }
            }
        }
        None
    }

    pub fn local(&self, id: LocalId) -> &Local {
        &self.locais[id.0 as usize]
    }

    /// Os locais visíveis (declarados ou adiante no bloco) por nome, com o
    /// nome da declaração e se ainda não foi declarado (escondido): um tipo escrito que nomeia um deles é
    /// `REFERENCED_BEFORE_DECLARATION` (`NamedTypeResolver`,
    /// `named_type_resolver.dart:611-621`).
    pub fn visible_locals(&self, interner: &dartforge_intern::Interner) -> HashMap<SymbolId, (dartforge_diagnostics::Span, bool)> {
        let mut m = HashMap::new();
        for e in self.escopos.iter() {
            for (n, r) in e.iter() {
                match r {
                    Nome::TypeParam(_) => {
                        m.remove(n);
                    }
                    Nome::Local(id) => {
                        let l = &self.locais[id.0 as usize];
                        let start = l.offset;
                        m.insert(*n, (dartforge_diagnostics::Span { start: start, end: start + interner.resolve(*n).len() }, false));
                    }
                    Nome::Forward(s) => {
                        m.insert(*n, (*s, true));
                    }
                }
            }
        }
        m
    }

    /// Parâmetros de tipo visíveis por nome (para resolver anotações).
    pub fn visible_type_parameters(&self) -> HashMap<SymbolId, TypeParamId> {
        let mut m = HashMap::new();
        for e in self.escopos.iter() {
            for (n, r) in e.iter() {
                match r {
                    Nome::TypeParam(p) => {
                        m.insert(*n, *p);
                    }
                    Nome::Local(_) => {
                        m.remove(n);
                    }
                    Nome::Forward(_) => {}
                }
            }
        }
        m
    }

    /// O escopo léxico deste ponto, para a sonda do LSP: cada nome uma vez,
    /// pela declaração mais interna (a mesma ordem de [`Corpo::buscar`]).
    pub fn visible_scope(&self) -> crate::resolved::EscopoSondado {
        let mut seen: HashSet<SymbolId> = HashSet::new();
        let mut output = crate::resolved::EscopoSondado {
            classe: self.classe,
            extensao: self.extensao,
            estatico: self.estatico,
            tipo_this: self.tipo_this,
            biblioteca: Some(self.lib),
            ..Default::default()
        };
        for e in self.escopos.iter().rev() {
            for (n, r) in e.iter().rev() {
                if !seen.insert(*n) {
                    continue;
                }
                match r {
                    Nome::Local(id) => {
                        let l = self.local(*id);
                        output.locais.push(crate::resolved::VisibleLocal {
                            nome: l.nome,
                            tipo: l.tipo,
                            offset: l.offset,
                            funcao: self.funcoes_locais.contains(id),
                        });
                    }
                    Nome::TypeParam(p) => output.parametros_de_tipo.push((*n, *p)),
                    // Declarado adiante: ainda não pode ser usado aqui.
                    Nome::Forward(_) => {}
                }
            }
        }
        output
    }
}
