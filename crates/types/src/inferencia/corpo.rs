//! Estado de um corpo em inferência: locais, escopos léxicos, parâmetros de
//! tipo visíveis, pilha de funções (para `return`/`yield`), receptores de
//! cascata e o modelo de fluxo corrente.

use super::fluxo::Fluxo;
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
    TipoParam(TypeParamId),
    /// Local declarado mais adiante no mesmo bloco: o escopo do bloco já o
    /// contém, e usá-lo antes da declaração é erro. O intervalo do nome na
    /// declaração.
    Adiante(dartforge_diagnostics::Span),
}

/// Contexto da função (ou expressão de função) que envolve o ponto atual.
#[derive(Debug, Clone)]
pub(crate) struct CtxFuncao {
    pub modificador: ast::AsyncModifier,
    /// Tipo de retorno declarado (ou imposto pelo contexto); `None` quando
    /// o retorno está sendo inferido.
    pub retorno: Option<TypeId>,
    /// Esquema de contexto para as expressões de `return`/`yield`.
    pub contexto_retorno: TypeId,
    /// Tipos das expressões retornadas (ou `yield`adas), para inferência.
    pub retornados: Vec<TypeId>,
    /// Há `return;` sem valor.
    pub retorno_vazio: bool,
    /// As expressões de `return e;` (e o corpo `=> e`) com os tipos, para a
    /// conferência de uma closure cujo retorno acaba sendo o do contexto.
    pub expressoes_retornadas: Vec<(ast::ExprId, TypeId)>,
    /// O executável que declara o retorno, para `return_of_invalid_type`;
    /// `None` em closures e construtores geradores (outras regras).
    pub executavel: Option<Executavel>,
    /// `hasLegalReturnType`: falso depois de `illegal_*_return_type`, que
    /// cala a conferência das expressões retornadas.
    pub retorno_legal: bool,
}

/// Espécie e nome de exibição do executável (`EnclosingExecutableContext`
/// do analyzer): o `return_of_invalid_type` diz "function", "method" ou
/// "constructor" e o nome.
#[derive(Debug, Clone)]
pub(crate) struct Executavel {
    pub especie: EspecieExecutavel,
    pub nome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EspecieExecutavel {
    /// Função de topo ou local com nome, e getters/setters (de topo ou de
    /// classe): `PropertyAccessorElement` conta como função.
    Funcao,
    /// Método ou operador de classe ou extensão.
    Metodo,
    /// Construtor factory.
    Construtor,
}

/// Estado de inferência de um corpo.
pub(crate) struct Corpo {
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
    pub fluxo: Fluxo,
    pub funcoes: Vec<CtxFuncao>,
    pub cascatas: Vec<TypeId>,
    /// O alvo de cada cascata aberta (o `realTarget` das seções).
    pub alvos_de_cascata: Vec<ast::ExprId>,
    /// Tipando o padrão de um `case`/`if-case` (refutável): o identificador
    /// solto é uma constante (`case _padrao:`), não uma variável nova.
    pub padrao_refutavel: bool,
    /// Um padrão refutável já relatado num contexto irrefutável
    /// (`refutablePatternInIrrefutableContext`): o que está abaixo dele é
    /// analisado como refutável (`context.makeRefutable()`), sem relatar de
    /// novo nem conferir o tipo requerido.
    pub refutavel_forcado: bool,
    /// O literal inteiro operando de um `-` unário em análise
    /// (`IntegerLiteral.immediatelyNegated`).
    pub literal_negado: Option<ast::ExprId>,
    /// As variáveis do padrão guardado cuja cláusula `when` está em análise
    /// (`isVisitingWhenClause`): escrever nelas é
    /// `PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD`.
    pub variaveis_em_guarda: Vec<LocalId>,
    /// As variáveis do padrão guardado em curso (nome e declaração), que
    /// ficam escondidas durante todo o padrão
    /// (`HiddenElements.forGuardedPattern`, `error_verifier.dart:6570-6575`):
    /// lê-las dentro do padrão é `REFERENCED_BEFORE_DECLARATION`.
    pub ocultas_do_padrao: Vec<(SymbolId, dartforge_diagnostics::Span)>,
    /// Os locais declarados pelo padrão do último `case` (sem os da
    /// guarda), para as variáveis de junção dos casos que dividem o corpo.
    pub locais_do_ultimo_padrao: std::ops::Range<usize>,
    /// Variáveis de junção inconsistentes dos casos que dividem um corpo,
    /// com o código que cada referência relata (`finishJoinedPatternVariable`).
    pub juncoes_inconsistentes: HashMap<LocalId, dartforge_diagnostics::Codigo>,
    /// O casamento de padrão em curso (T6: a referência do valor casado e o
    /// estado "não casou"); `None` fora de padrão.
    pub casamento: Option<super::padroes::Casamento>,
    /// Dentro de um `switch` (instrução ou expressão): o escrutínio já
    /// casado por um caso — a expressão, a chave de promoção do valor e a
    /// versão de escrita da variável escrutinada —, que os casos seguintes
    /// reusam. `None` fora de `switch`; `Some(None)` antes do primeiro caso.
    pub escrutinio_de_switch: Option<Option<(ast::ExprId, LocalId, Option<u32>)>>,
    /// Corpo de um método ou inicializador de campo estático: os parâmetros
    /// de tipo da classe não valem ali (`TYPE_PARAMETER_REFERENCED_BY_STATIC`).
    pub membro_estatico: bool,
    /// Pilha de alvos de `break`/`continue` (rótulos e laços): modelos de
    /// fluxo acumulados nos saltos.
    pub saltos: Vec<AlvoSalto>,
    /// Tipo de `this` (classe, mixin, enum, tipo de extensão ou `on` da extensão).
    pub tipo_this: Option<TypeId>,
    /// Nomes escritos dentro de closures (captura de escrita, que impede promoção).
    pub escritos_em_closure: Vec<super::instrucoes::Escrita>,
    /// Rótulos da instrução rotulada cujo corpo é o próximo laço/`switch`.
    pub rotulos_pendentes: Vec<SymbolId>,
    /// O símbolo `_` quando a biblioteca tem curingas (Dart 3.7): declarar
    /// `_` cria o local (tem `LocalId` e tipo) mas não liga o nome.
    pub curinga: Option<SymbolId>,
    /// Atalhos de ponto (3.10): o contexto da cadeia de seletores, pela
    /// expressão `DotShorthand` da raiz (ver `atalhos`).
    pub contexto_atalho: HashMap<u32, TypeId>,
    /// Raízes `DotShorthand` de uma invocação cujo erro já foi relatado
    /// (`atalhos::construcao`): o [`super::atalhos::valor`] do alvo não relata de novo.
    pub atalhos_relatados: std::collections::HashSet<u32>,
    /// Nomes escritos em qualquer ponto do corpo de topo: dentro de uma
    /// closure eles não ficam promovidos (`functionExpression_begin` faz a
    /// junção conservadora com `assignedVariables.anywhere`).
    /// Calculado na primeira closure (a maioria dos corpos não tem nenhuma).
    /// `(escritos fora de literais, escritos dentro de literais)` do membro.
    pub escritos_no_corpo: Option<(Vec<super::instrucoes::Escrita>, Vec<super::instrucoes::Escrita>)>,
    /// O corpo de topo, para calcular `escritos_no_corpo` sob demanda.
    pub raiz: Raiz,
    /// Propriedades promovíveis (Dart 3.2) já referidas: `(base, versão de
    /// escrita da base, nome) -> local sintético` que carrega o modelo de
    /// fluxo da propriedade (o `_promotableProperties` do nó SSA da base).
    pub campos: HashMap<(Base, u32, SymbolId), LocalId>,
    /// Propriedades não promovíveis: as gerações (uma por leitura que virou
    /// alvo de promoção), da mais antiga à mais nova (o
    /// `_nonPromotableProperties` com o `previousSsaNode` de cada nó).
    pub geracoes: HashMap<(Base, u32, SymbolId), Vec<LocalId>>,
    /// A geração de cada leitura de propriedade não promovível.
    pub geracao_da_leitura: HashMap<ast::ExprId, LocalId>,
    /// O local sintético de `this` (a `thisPromotionKey`), que guarda as
    /// promoções que `this` nunca usa (só o why-not-promoted as vê).
    pub local_this: Option<LocalId>,
    /// Fluxos de antes de cada `?.` das cadeias em curso.
    pub cadeias: Vec<super::fluxo::Fluxo>,
    /// Variáveis de condição (§7.10): `(verdadeiro, falso, versão)` do
    /// valor escrito na variável, restaurados na leitura como condição.
    pub condicoes: HashMap<LocalId, (Fluxo, Fluxo, u32)>,
    /// Sobreposições explícitas de extensão (`E(x)`, R-EXT-02): a chamada
    /// `E(x)` → a extensão e os argumentos de tipo dela.
    pub sobreposicoes: HashMap<ast::ExprId, (ExtensionId, Vec<TypeId>)>,
    /// Fins dos blocos básicos em curso (o `flowEnd` do
    /// `NullSafetyDeadCodeVerifier`: corpo de função, ramo de `if`, corpo de
    /// laço, `try`/`catch`), já aparados na última instrução do bloco.
    pub fins_de_fluxo: Vec<usize>,
    /// O fim da última instrução de cada bloco em curso: sem bloco básico
    /// aberto, é onde o trecho morto de uma expressão termina (o
    /// `flowEnd` do corpo, aparado na última instrução).
    pub fins_de_bloco: Vec<usize>,
    /// Profundidade em `fins_de_fluxo` onde começou o trecho morto em curso
    /// (`_firstDeadNode`): enquanto houver um, outro nó inalcançável é parte
    /// do mesmo trecho.
    pub trecho_morto: Option<usize>,
    /// Onde o trecho morto em curso começou: a própria instrução (ramo
    /// inalcançável) ou o bloco que a contém. O `for` relata as
    /// atualizações quando é o corpo dele (`_reportForUpdaters`).
    pub origem_do_morto: Option<ast::StmtId>,
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
pub(crate) struct AlvoSalto {
    pub rotulos: Vec<SymbolId>,
    /// É laço ou `switch` (alvo de `break`/`continue` sem rótulo).
    pub laco: bool,
    pub e_switch: bool,
    pub breaks: Vec<Fluxo>,
    pub continues: Vec<Fluxo>,
}

impl Corpo {
    pub fn novo(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, extensao: Option<ExtensionId>, estatico: bool) -> Self {
        let lib = inf.program.unit(unit).library;
        inf.unidade_corrente = Some(unit);
        let mut cx = Corpo {
            unit,
            lib,
            classe,
            extensao,
            estatico,
            locais: Vec::new(),
            funcoes_locais: HashSet::new(),
            escopos: vec![Vec::new()],
            fluxo: Fluxo::alcancavel(),
            funcoes: Vec::new(),
            cascatas: Vec::new(),
            alvos_de_cascata: Vec::new(),
            padrao_refutavel: false,
            refutavel_forcado: false,
            literal_negado: None,
            variaveis_em_guarda: Vec::new(),
            ocultas_do_padrao: Vec::new(),
            locais_do_ultimo_padrao: 0..0,
            juncoes_inconsistentes: HashMap::new(),
            casamento: None,
            escrutinio_de_switch: None,
            membro_estatico: false,
            saltos: Vec::new(),
            tipo_this: None,
            escritos_em_closure: Vec::new(),
            rotulos_pendentes: Vec::new(),
            curinga: None,
            contexto_atalho: HashMap::new(),
            atalhos_relatados: std::collections::HashSet::new(),
            escritos_no_corpo: None,
            raiz: Raiz::Nada,
            campos: HashMap::new(),
            geracoes: HashMap::new(),
            geracao_da_leitura: HashMap::new(),
            local_this: None,
            cadeias: Vec::new(),
            condicoes: HashMap::new(),
            sobreposicoes: HashMap::new(),
            fins_de_fluxo: Vec::new(),
            fins_de_bloco: Vec::new(),
            trecho_morto: None,
            origem_do_morto: None,
        };
        if inf.program.library(lib).features.tem(dartforge_frontend::Feature::WildcardVariables) {
            cx.curinga = inf.interner.lookup("_");
        }
        // Parâmetros de tipo da classe/extensão estão sempre em escopo
        // (mesmo em membros estáticos, onde usá-los é erro).
        if let Some(c) = classe {
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            for p in ps.iter() {
                let nome = inf.table.param(*p).name;
                cx.escopos[0].push((nome, Nome::TipoParam(*p)));
            }
            if !estatico {
                cx.tipo_this = Some(inf.tipo_this_classe(c));
            }
        } else if let Some(e) = extensao {
            let ps = inf.outline.extensions[e.0 as usize].type_params.clone();
            for p in ps.iter() {
                let nome = inf.table.param(*p).name;
                cx.escopos[0].push((nome, Nome::TipoParam(*p)));
            }
            if !estatico {
                cx.tipo_this = Some(inf.outline.extensions[e.0 as usize].on);
            }
        }
        cx.escopos.push(Vec::new());
        cx
    }

    /// Corpo para o inicializador de uma variável de topo ou campo.
    pub fn para_variavel(inf: &mut BodyInferrer<'_>, vid: VariableId, unit: UnitId) -> Self {
        let v = inf.program.variable(vid);
        let (classe, extensao) = (v.class, v.extension);
        // Inicializadores de campos de instância não veem `this` (só os `late`).
        let estatico = v.static_ || (classe.is_none() && extensao.is_none()) || !v.late;
        let mut cx = Corpo::novo(inf, unit, classe, extensao, estatico);
        cx.membro_estatico = v.static_ && (classe.is_some() || extensao.is_some());
        if !v.static_ && v.late {
            cx.estatico = false;
            if let Some(c) = classe {
                cx.tipo_this = Some(inf.tipo_this_classe(c));
            }
        }
        cx
    }

    /// Corpo de uma função declarada (método, função de topo, construtor).
    pub fn para_funcao(inf: &mut BodyInferrer<'_>, f: FunctionElementId, unit: UnitId) -> Self {
        let fe = inf.program.function(f);
        let (classe, extensao) = (fe.class, fe.extension);
        let estatico = fe.static_ && !matches!(fe.kind, dartforge_elements::model::FunctionKind::Constructor) || fe.factory;
        let estatico = estatico || (classe.is_none() && extensao.is_none());
        let membro_estatico = fe.static_
            && !fe.factory
            && !matches!(fe.kind, dartforge_elements::model::FunctionKind::Constructor)
            && (classe.is_some() || extensao.is_some());
        let mut cx = Corpo::novo(inf, unit, classe, extensao, estatico);
        cx.membro_estatico = membro_estatico;
        cx
    }

    pub fn empurrar_escopo(&mut self) {
        self.escopos.push(Vec::new());
    }

    pub fn tirar_escopo(&mut self) {
        if self.escopos.len() > 1 {
            self.escopos.pop();
        }
    }

    /// Declara um local no escopo mais interno.
    pub fn declarar(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locais.len() as u32);
        let nome = local.nome;
        self.locais.push(local);
        if self.curinga == Some(nome) {
            self.fluxo.declarar(id);
            return id;
        }
        if let Some(e) = self.escopos.last_mut() {
            e.push((nome, Nome::Local(id)));
        }
        self.fluxo.declarar(id);
        id
    }

    /// Registra um nome declarado adiante no bloco corrente.
    pub fn declarar_adiante(&mut self, nome: SymbolId, span: dartforge_diagnostics::Span) {
        if self.curinga == Some(nome) {
            return;
        }
        if let Some(e) = self.escopos.last_mut() {
            e.push((nome, Nome::Adiante(span)));
        }
    }

    /// Local sintético (sem nome no escopo): alvo de promoção de um campo.
    pub fn declarar_sintetico(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locais.len() as u32);
        self.locais.push(local);
        self.fluxo.declarar(id);
        self.fluxo.inicializar(id);
        id
    }

    /// Esquece os campos promovidos de uma local reatribuída.
    pub fn esquecer_campos_de(&mut self, base: LocalId) {
        self.campos.retain(|(b, _, _), _| *b != Base::Local(base));
    }

    /// O local é o sintético de uma propriedade (estável ou geração).
    pub fn e_propriedade(&self, id: LocalId) -> bool {
        self.campos.values().any(|&v| v == id) || self.geracao_da_leitura.values().any(|&v| v == id)
    }

    /// `infoFor`: um sintético cujo modelo se perdeu numa junção (criado num
    /// só dos ramos) volta como novo.
    pub fn garantir_modelo(&mut self, id: LocalId) {
        if self.fluxo.modelo(id).is_none() {
            self.fluxo.declarar(id);
            self.fluxo.inicializar(id);
        }
    }

    /// A versão de escrita da base de uma propriedade: a da local (o nó SSA
    /// dela); `this`, `super` e as propriedades estáveis não mudam.
    pub fn versao_da_base(&self, base: Base) -> u32 {
        match base {
            Base::Local(id) if !self.e_propriedade(id) => self.fluxo.versao(id).unwrap_or(0),
            _ => 0,
        }
    }

    pub fn declarar_tipo_param(&mut self, nome: SymbolId, p: TypeParamId) {
        if let Some(e) = self.escopos.last_mut() {
            e.push((nome, Nome::TipoParam(p)));
        }
    }

    /// Nome nos escopos de bloco (do mais interno para fora).
    pub fn buscar(&self, nome: SymbolId) -> Option<Nome> {
        for e in self.escopos.iter().rev() {
            for (n, r) in e.iter().rev() {
                if *n == nome {
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
    pub fn locais_visiveis(&self, interner: &dartforge_intern::Interner) -> HashMap<SymbolId, (dartforge_diagnostics::Span, bool)> {
        let mut m = HashMap::new();
        for e in self.escopos.iter() {
            for (n, r) in e.iter() {
                match r {
                    Nome::TipoParam(_) => {
                        m.remove(n);
                    }
                    Nome::Local(id) => {
                        let l = &self.locais[id.0 as usize];
                        let inicio = l.offset;
                        m.insert(*n, (dartforge_diagnostics::Span { start: inicio, end: inicio + interner.resolve(*n).len() }, false));
                    }
                    Nome::Adiante(s) => {
                        m.insert(*n, (*s, true));
                    }
                }
            }
        }
        m
    }

    /// Parâmetros de tipo visíveis por nome (para resolver anotações).
    pub fn parametros_de_tipo_visiveis(&self) -> HashMap<SymbolId, TypeParamId> {
        let mut m = HashMap::new();
        for e in self.escopos.iter() {
            for (n, r) in e.iter() {
                match r {
                    Nome::TipoParam(p) => {
                        m.insert(*n, *p);
                    }
                    Nome::Local(_) => {
                        m.remove(n);
                    }
                    Nome::Adiante(_) => {}
                }
            }
        }
        m
    }

    /// O escopo léxico deste ponto, para a sonda do LSP: cada nome uma vez,
    /// pela declaração mais interna (a mesma ordem de [`Corpo::buscar`]).
    pub fn escopo_visivel(&self) -> crate::resolved::EscopoSondado {
        let mut vistos: HashSet<SymbolId> = HashSet::new();
        let mut saida = crate::resolved::EscopoSondado {
            classe: self.classe,
            extensao: self.extensao,
            estatico: self.estatico,
            tipo_this: self.tipo_this,
            biblioteca: Some(self.lib),
            ..Default::default()
        };
        for e in self.escopos.iter().rev() {
            for (n, r) in e.iter().rev() {
                if !vistos.insert(*n) {
                    continue;
                }
                match r {
                    Nome::Local(id) => {
                        let l = self.local(*id);
                        saida.locais.push(crate::resolved::LocalVisivel {
                            nome: l.nome,
                            tipo: l.tipo,
                            offset: l.offset,
                            funcao: self.funcoes_locais.contains(id),
                        });
                    }
                    Nome::TipoParam(p) => saida.parametros_de_tipo.push((*n, *p)),
                    // Declarado adiante: ainda não pode ser usado aqui.
                    Nome::Adiante(_) => {}
                }
            }
        }
        saida
    }
}
