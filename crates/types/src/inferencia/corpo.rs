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
use std::collections::HashMap;

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
    /// contém, e usá-lo antes da declaração é erro.
    Adiante,
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
    pub escopos: Vec<Vec<(SymbolId, Nome)>>,
    pub fluxo: Fluxo,
    pub funcoes: Vec<CtxFuncao>,
    pub cascatas: Vec<TypeId>,
    /// Pilha de alvos de `break`/`continue` (rótulos e laços): modelos de
    /// fluxo acumulados nos saltos.
    pub saltos: Vec<AlvoSalto>,
    /// Tipo de `this` (classe, mixin, enum, tipo de extensão ou `on` da extensão).
    pub tipo_this: Option<TypeId>,
    /// Nomes escritos dentro de closures (captura de escrita, que impede promoção).
    pub escritos_em_closure: Vec<SymbolId>,
    /// Rótulos da instrução rotulada cujo corpo é o próximo laço/`switch`.
    pub rotulos_pendentes: Vec<SymbolId>,
    /// Nomes escritos em qualquer ponto do corpo de topo: dentro de uma
    /// closure eles não ficam promovidos (`functionExpression_begin` faz a
    /// junção conservadora com `assignedVariables.anywhere`).
    /// Calculado na primeira closure (a maioria dos corpos não tem nenhuma).
    pub escritos_no_corpo: Option<Vec<SymbolId>>,
    /// O corpo de topo, para calcular `escritos_no_corpo` sob demanda.
    pub raiz: Raiz,
    /// Campos promovíveis (Dart 3.2) já referidos: `(base, campo) -> local
    /// sintético` que carrega o modelo de fluxo do campo.
    pub campos: HashMap<(Base, VariableId), LocalId>,
    /// Fluxos de antes de cada `?.` das cadeias em curso.
    pub cadeias: Vec<super::fluxo::Fluxo>,
}

/// Base de uma referência a campo promovível.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Base {
    This,
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
            escopos: vec![Vec::new()],
            fluxo: Fluxo::alcancavel(),
            funcoes: Vec::new(),
            cascatas: Vec::new(),
            saltos: Vec::new(),
            tipo_this: None,
            escritos_em_closure: Vec::new(),
            rotulos_pendentes: Vec::new(),
            escritos_no_corpo: None,
            raiz: Raiz::Nada,
            campos: HashMap::new(),
            cadeias: Vec::new(),
        };
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
        Corpo::novo(inf, unit, classe, extensao, estatico)
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
        if let Some(e) = self.escopos.last_mut() {
            e.push((nome, Nome::Local(id)));
        }
        self.fluxo.declarar(id);
        id
    }

    /// Registra um nome declarado adiante no bloco corrente.
    pub fn declarar_adiante(&mut self, nome: SymbolId) {
        if let Some(e) = self.escopos.last_mut() {
            e.push((nome, Nome::Adiante));
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
        self.campos.retain(|(b, _), _| *b != Base::Local(base));
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
                    Nome::Adiante => {}
                }
            }
        }
        m
    }

}
