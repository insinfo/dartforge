//! Resolução de nós de identificadores e expressões em corpos para suas declarações e alvos semânticos.
//!
//! Toda referência semântica nos corpos de funções ou inicializadores é registrada
//! na tabela lateral [`UnitBodyTypes`], indexada compactamente por [`ast::ExprId`].

use crate::table::{TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, LibraryId, VariableId};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;

/// Identificador de uma variável local declarada em um bloco ou expressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

/// Referência a um membro de classe ou extensão (função/método ou campo/variável).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberRef {
    Function(FunctionElementId),
    Variable(VariableId),
}

/// Alvo resolvido de uma referência ou identificador dentro de um corpo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resolved {
    /// Variável local declarada em escopo de bloco (`var x`, `int x`, `for (var x...)`).
    Local(LocalId),
    /// Parâmetro formal da função, método ou closure envolvente.
    Parameter { index: u32, name: SymbolId },
    /// Parâmetro de tipo genérico local ou da classe/extensão envolvente.
    TypeParameter(TypeParamId),
    /// Elemento de nível superior (classe, função de topo, variável de topo, typedef, extensão).
    Element(Element),
    /// Membro de classe (campo, getter, setter, método) acessado via receptor explícito ou implícito (`this`).
    Member {
        class: ClassId,
        member: MemberRef,
        via_super: bool,
    },
    /// Prefixo de importação (`import '...' as p; p.x`).
    Prefix(LibraryId),
    /// Acesso a membro em receptor cujo tipo estático é `dynamic`.
    Dynamic,
    /// Membro de extensão resolvido para chamada de método/getter/setter estático de extensão.
    ExtensionMember {
        extension: ExtensionId,
        member: FunctionElementId,
    },
    /// Construtor de classe chamado em instanciação (`new C()`, `C.named()`).
    Constructor(FunctionElementId),
}

/// Tabelas laterais de tipos estáticos e resoluções para uma única unidade de compilação.
///
/// Mantém vetores indexados diretamente por [`ast::ExprId::0 as usize`], sem custos de hash
/// ou ponteiros no heap.
#[derive(Debug, Clone, Default)]
pub struct UnitBodyTypes {
    /// Tipo estático inferido de cada expressão na arena `ast.exprs`.
    pub static_types: Vec<TypeId>,
    /// Alvo resolvido de expressões de identificador, propriedade ou chamada.
    pub resolved: Vec<Option<Resolved>>,
    /// Tipo declarado (ou inferido do inicializador) de cada variável local,
    /// pelo offset do nome na declaração. O backend nativo guarda o local
    /// na representação desse tipo (`int?` é caixa, `int` é `i64`), que não
    /// se deduz das leituras: elas têm o tipo promovido pelo fluxo.
    pub tipos_de_locais: std::collections::HashMap<usize, TypeId>,
    /// Declaração (offset do nome) do local que cada expressão resolvida para
    /// [`Resolved::Local`] lê ou escreve. Só é preenchida quando
    /// [`crate::BodyInferrer::registrar_locais`] está ligado (o LSP, para
    /// renomear e achar referências); o compilador não paga por ela.
    pub declaracoes_de_locais: std::collections::HashMap<ast::ExprId, usize>,
    /// Expressões cujo tipo é o de recuperação do analyzer (`InvalidType`,
    /// uma leitura que não resolve): em `static_types` ficam `dynamic`, que
    /// é como o `InvalidType` se comporta; a marca só distingue na exibição.
    pub tipos_invalidos: std::collections::HashSet<ast::ExprId>,
    /// O tipo de execução (a regra do CFE, que dá o `runtimeType`) de uma
    /// função literal ou local geradora (`sync*`/`async*`) sem retorno
    /// escrito, quando difere do tipo estático, que segue o analyzer 3.6.2:
    /// o analyzer junta o `Null` de um `return;` aos `yield` e dá `dynamic`
    /// ao gerador sem nenhum; o CFE ignora o `return;` e dá `Null`. Pela
    /// função (`ast::FunctionId`).
    pub tipos_de_execucao_de_funcoes: std::collections::HashMap<ast::FunctionId, TypeId>,
    /// Argumentos de tipo escolhidos para cada chamada genérica (função,
    /// método, estático, `call`), explícitos ou inferidos, pelo offset do
    /// início da lista de argumentos (`ast::Arguments::span`). Fica fora do
    /// tipo de qualquer nó (`id(5)` tem tipo `Object?`, mas a instanciação
    /// `<Object?>` só aparece aqui) e é o que o backend reifica.
    pub instanciacoes: std::collections::HashMap<usize, Box<[TypeId]>>,
    /// Os argumentos de tipo da instanciação implícita de um tear-off de
    /// função genérica num contexto de função não genérica
    /// (`int Function(int) f = id;`), pela expressão do tear-off: a closure
    /// tem o tipo instanciado e leva os argumentos, como na VM.
    pub instanciacoes_de_tearoff: std::collections::HashMap<ast::ExprId, Box<[TypeId]>>,
    /// O tipo de cada padrão que a exaustividade lê (`PatternConverter` do
    /// analyzer, `an611:src/generated/exhaustiveness.dart:520-680`): o
    /// declarado de uma variável, o escrito de um curinga ou de um cast, o
    /// requerido de um padrão objeto, lista (`List<E>`) ou mapa (`Map<K, V>`).
    pub tipos_de_padroes: std::collections::HashMap<ast::PatternId, TypeId>,
    /// O tipo da propriedade de extensão lida por um campo de padrão objeto
    /// (`ExtensionKey`), pelo subpadrão do campo.
    pub campos_de_extensao: std::collections::HashMap<ast::PatternId, TypeId>,
    /// Padrões cujo tipo escrito não resolve (`InvalidType`): o analyzer não
    /// verifica a exaustividade do `switch` que os contém.
    pub padroes_invalidos: std::collections::HashSet<ast::PatternId>,
    /// O tipo casado (`matchedValueType`) de cada padrão, já promovido pelo
    /// fluxo de padrões (casos anteriores do `switch`, `&&`, `?`): o que o
    /// analyzer guarda em `DartPatternImpl.matchedValueType` e o
    /// `ConstantVerifier` lê (`constant_pattern_never_matches_value_type`).
    pub tipos_casados: std::collections::HashMap<ast::PatternId, TypeId>,
}

impl UnitBodyTypes {
    /// Cria tabelas laterais alocadas com o tamanho exato de expressões da AST da unidade.
    pub fn new(num_exprs: usize, fallback_type: TypeId) -> Self {
        Self {
            static_types: vec![fallback_type; num_exprs],
            resolved: vec![None; num_exprs],
            tipos_de_locais: std::collections::HashMap::new(),
            declaracoes_de_locais: std::collections::HashMap::new(),
            tipos_invalidos: std::collections::HashSet::new(),
            tipos_de_execucao_de_funcoes: std::collections::HashMap::new(),
            instanciacoes: std::collections::HashMap::new(),
            instanciacoes_de_tearoff: std::collections::HashMap::new(),
            tipos_de_padroes: std::collections::HashMap::new(),
            campos_de_extensao: std::collections::HashMap::new(),
            padroes_invalidos: std::collections::HashSet::new(),
            tipos_casados: std::collections::HashMap::new(),
        }
    }

    /// Os argumentos de tipo da instanciação implícita do tear-off `e` (ver
    /// [`UnitBodyTypes::instanciacoes_de_tearoff`]).
    pub fn instanciacao_de_tearoff(&self, e: ast::ExprId) -> Option<&[TypeId]> {
        self.instanciacoes_de_tearoff.get(&e).map(|a| &**a)
    }

    /// Registra os argumentos de tipo da chamada genérica cuja lista de
    /// argumentos começa em `offset`.
    pub fn set_instanciacao(&mut self, offset: usize, args: Box<[TypeId]>) {
        self.instanciacoes.insert(offset, args);
    }

    /// Argumentos de tipo da chamada genérica cuja lista de argumentos
    /// começa em `offset` (ver [`UnitBodyTypes::instanciacoes`]).
    pub fn instanciacao(&self, offset: usize) -> Option<&[TypeId]> {
        self.instanciacoes.get(&offset).map(|a| &**a)
    }

    /// Registra o tipo de uma variável local declarada em `offset`.
    pub fn set_tipo_local(&mut self, offset: usize, ty: TypeId) {
        self.tipos_de_locais.insert(offset, ty);
    }

    /// O tipo de execução de uma função geradora sem retorno escrito, quando
    /// difere do estático (ver [`Self::tipos_de_execucao_de_funcoes`]).
    pub fn tipo_de_execucao_de_funcao(&self, f: ast::FunctionId) -> Option<TypeId> {
        self.tipos_de_execucao_de_funcoes.get(&f).copied()
    }

    /// Tipo da variável local declarada em `offset` (o do nome).
    pub fn tipo_local(&self, offset: usize) -> Option<TypeId> {
        self.tipos_de_locais.get(&offset).copied()
    }

    /// Define o tipo estático de uma expressão.
    pub fn set_type(&mut self, expr: ast::ExprId, ty: TypeId) {
        let idx = expr.0 as usize;
        if idx < self.static_types.len() {
            self.static_types[idx] = ty;
        }
    }

    /// Obtém o tipo estático associado a uma expressão.
    pub fn get_type(&self, expr: ast::ExprId) -> Option<TypeId> {
        self.static_types.get(expr.0 as usize).copied()
    }

    /// Registra a resolução semântica de uma expressão.
    pub fn set_resolved(&mut self, expr: ast::ExprId, target: Resolved) {
        let idx = expr.0 as usize;
        if idx < self.resolved.len() {
            self.resolved[idx] = Some(target);
        }
    }

    /// Offset do nome na declaração do local que `expr` referencia, quando
    /// a inferência registrou locais (ver [`UnitBodyTypes::declaracoes_de_locais`]).
    pub fn declaracao_local(&self, expr: ast::ExprId) -> Option<usize> {
        self.declaracoes_de_locais.get(&expr).copied()
    }

    /// Obtém o alvo semântico resolvido de uma expressão.
    pub fn get_resolved(&self, expr: ast::ExprId) -> Option<&Resolved> {
        self.resolved.get(expr.0 as usize).and_then(|r| r.as_ref())
    }
}

/// Conjunto de todas as tabelas laterais de corpos de todas as unidades do programa.
#[derive(Debug, Clone, Default)]
pub struct BodyTypes {
    /// Tabelas indexadas pelo índice da unidade em `Program::units`.
    pub units: Vec<UnitBodyTypes>,
}

/// Um local (variável, parâmetro ou função local) visível num ponto do corpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVisivel {
    pub nome: SymbolId,
    /// Tipo declarado (escrito ou inferido do inicializador).
    pub tipo: TypeId,
    /// Offset do nome na declaração.
    pub offset: usize,
    /// Declarado como função local (`void f() {}` dentro de um corpo).
    pub funcao: bool,
}

/// O escopo léxico capturado no identificador sondado (ver
/// [`crate::BodyInferrer::sonda_escopo`]): o que o LSP oferece ao completar
/// um nome simples, vindo da mesma pilha de escopos que resolve os nomes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscopoSondado {
    /// Locais visíveis, do mais interno para o mais externo; um nome
    /// sombreado aparece só uma vez (a declaração mais interna).
    pub locais: Vec<LocalVisivel>,
    /// Parâmetros de tipo visíveis pelo nome.
    pub parametros_de_tipo: Vec<(SymbolId, TypeParamId)>,
    /// Classe (ou mixin, enum, extension type) do membro envolvente.
    pub classe: Option<ClassId>,
    /// Extensão do membro envolvente.
    pub extensao: Option<ExtensionId>,
    /// Sem `this`: membro estático, topo ou construtor de fábrica.
    pub estatico: bool,
    /// Tipo de `this`, quando há.
    pub tipo_this: Option<TypeId>,
    /// Biblioteca do corpo.
    pub biblioteca: Option<LibraryId>,
}
