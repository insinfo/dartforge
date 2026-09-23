//! Modelo de elementos: o *outline* de um programa Dart, sem corpos analisados.
//!
//! É o que o CFE chama de outline e o `analyzer` chama de element model: para
//! cada biblioteca, o que ela declara e o que ela vê; para cada classe, seus
//! membros por nome e seus supertipos por classe. Nenhum corpo de função é
//! tocado aqui — resolver `a.b` dentro de um corpo depende do tipo de `a`, e
//! isso é da fase `types`.
//!
//! Regras de memória (docs/FRONTEND-ARQUITETURA.md §2): tudo em `Vec` indexado
//! por id `u32`; nomes são [`SymbolId`]; nós da árvore são referenciados por
//! `(UnitId, id na arena)`, nunca copiados.
use dartforge_frontend::LibraryFeatures;
use dartforge_frontend::ast::{self, Ast, CompilationUnit, DeclId, FunctionId, MemberId};
use dartforge_intern::SymbolId;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u32);
    };
}

id!(/// Índice em [`Program::units`].
    UnitId);
id!(/// Índice em [`Program::libraries`].
    LibraryId);
id!(/// Índice em [`Program::classes`] (classe, mixin, enum ou extension type).
    ClassId);
id!(/// Índice em [`Program::extensions`].
    ExtensionId);
id!(/// Índice em [`Program::typedefs`].
    TypedefId);
id!(/// Índice em [`Program::functions`] (funções de topo, métodos, getters,
    /// setters, operadores e construtores).
    FunctionElementId);
id!(/// Índice em [`Program::variables`] (variáveis de topo e campos).
    VariableId);

/// Tempos das sub-fases de `load_lenient`, para o `--timings` do `compile-js`.
#[derive(Debug, Default, Clone)]
pub struct TemposCarga {
    /// Leitura e lexing em série (partes e SDK dos arquivos), mais o custo
    /// de tirar do prefetch o que já veio pronto.
    pub leitura: std::time::Duration,
    /// Ondas de leitura + lexing em paralelo (tempo de parede).
    pub leitura_lex_paralelo: std::time::Duration,
    pub ondas: usize,
    /// Análise sintática (sequencial: precisa do `Interner`).
    pub parse: std::time::Duration,
    /// Decodificação das unidades do SDK vindas do cache.
    pub sdk_cache: std::time::Duration,
    /// Resolução de diretivas: URIs, `canonicalize`, `verify_part_of`, fila.
    pub diretivas: std::time::Duration,
    /// `build_outline` fase 1: declarações e membros.
    pub outline_declaracoes: std::time::Duration,
    /// `build_outline` fase 2: `exported` com ponto fixo de reexports.
    pub outline_reexports: std::time::Duration,
    /// `build_outline` fase 3: escopo léxico e prefixos por biblioteca.
    pub outline_escopos: std::time::Duration,
    /// `build_outline` fase 4: supertipos e ciclos.
    pub outline_supertipos: std::time::Duration,
    /// Arquivos lidos e bytes de fonte (usuário e pacotes).
    pub arquivos_lidos: usize,
    pub bytes_lidos: usize,
    /// Unidades que vieram prontas do cache da sessão residente.
    pub unidades_reaproveitadas: usize,
}

/// Programa inteiro: SDK, pacotes e o projeto, num só grafo de bibliotecas.
#[derive(Debug, Default)]
pub struct Program {
    /// Tempos das sub-fases da carga (preenchido por `load_lenient`).
    pub tempos: TemposCarga,
    pub units: Vec<Unit>,
    pub libraries: Vec<Library>,
    pub classes: Vec<ClassElement>,
    pub extensions: Vec<ExtensionElement>,
    pub typedefs: Vec<TypedefElement>,
    pub functions: Vec<FunctionElement>,
    pub variables: Vec<VariableElement>,
    /// Biblioteca de `dart:core`, importada implicitamente por todas.
    pub core: Option<LibraryId>,
    /// Biblioteca da entrada do programa (a que declara `main`), quando há.
    pub entry: Option<LibraryId>,
}

/// Papel de um arquivo dentro da sua biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnitRole {
    /// Arquivo que declara a biblioteca (tem os imports).
    Library,
    /// `part of` da biblioteca.
    Part,
    /// Patch file do SDK (`libraries.json`), fundido na biblioteca de origem.
    Patch,
}

/// Um arquivo `.dart` já analisado sintaticamente.
#[derive(Debug)]
pub struct Unit {
    /// URI canônica: `dart:core`, `package:x/y.dart` ou `file:///…`.
    pub uri: String,
    pub path: Option<PathBuf>,
    pub source: String,
    pub ast: Ast,
    pub unit: CompilationUnit,
    pub library: LibraryId,
    pub role: UnitRole,
    /// Recursos com que a unidade foi analisada (os da biblioteca). A sessão
    /// residente só reaproveita a unidade se continuarem os mesmos.
    pub features: LibraryFeatures,
}

/// Importação já resolvida para uma biblioteca do programa.
#[derive(Debug)]
pub struct Import {
    pub unit: UnitId,
    /// Índice da diretiva em `Unit::unit.directives`.
    pub directive: usize,
    pub library: LibraryId,
    pub prefix: Option<SymbolId>,
    pub deferred: bool,
    pub combinators: Vec<ast::Combinator>,
}

impl Clone for Import {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            directive: self.directive,
            library: self.library,
            prefix: self.prefix,
            deferred: self.deferred,
            combinators: self
                .combinators
                .iter()
                .map(|c| match c {
                    ast::Combinator::Show(n) => ast::Combinator::Show(n.clone()),
                    ast::Combinator::Hide(n) => ast::Combinator::Hide(n.clone()),
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct Export {
    pub unit: UnitId,
    pub directive: usize,
    pub library: LibraryId,
    pub combinators: Vec<ast::Combinator>,
}

impl Clone for Export {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            directive: self.directive,
            library: self.library,
            combinators: self
                .combinators
                .iter()
                .map(|c| match c {
                    ast::Combinator::Show(n) => ast::Combinator::Show(n.clone()),
                    ast::Combinator::Hide(n) => ast::Combinator::Hide(n.clone()),
                })
                .collect(),
        }
    }
}

/// Referência a um elemento nomeável no escopo de uma biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    /// Função, getter ou setter de topo.
    Function(FunctionElementId),
    /// Variável de topo (com getter/setter implícitos).
    Variable(VariableId),
    /// Prefixo de import (`import 'x' as p;`): resolve para a biblioteca dona.
    Prefix(LibraryId, SymbolId),
}

/// Entrada de namespace: um nome pode mapear para um getter e um setter
/// distintos (`get x` e `set x` de topo são elementos separados).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Binding {
    pub getter: Option<Element>,
    pub setter: Option<Element>,
    /// Dois imports trouxeram elementos diferentes com este nome; o uso do
    /// nome é erro, mas a importação em si não (regra do Dart).
    pub ambiguous: bool,
}

/// Mapa de nomes → elementos.
pub type Namespace = HashMap<SymbolId, Binding>;

#[derive(Debug)]
pub struct Library {
    pub uri: String,
    pub name: Option<Vec<SymbolId>>,
    /// Unidade principal, depois partes e patches, na ordem de descoberta.
    pub units: Vec<UnitId>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    /// Elementos declarados nesta biblioteca (inclusive os de patches),
    /// privados incluídos.
    pub declared: Namespace,
    /// O que a biblioteca exporta: `declared` sem privados, mais os exports.
    pub exported: Namespace,
    /// Escopo de resolução de nomes de topo: `declared` sombreia os imports
    /// sem prefixo; `dart:core` implícito tem a menor precedência.
    pub scope: Namespace,
    /// Namespaces alcançáveis por prefixo (`p.nome`).
    pub prefixes: HashMap<SymbolId, Namespace>,
    /// Biblioteca do SDK (`dart:`), que pode usar `JS()`, `@patch`, `native`.
    pub is_sdk: bool,
    /// Versão de linguagem e recursos ligados (`docs/VERSOES-LINGUAGEM.md`):
    /// o marcador `// @dart = x.y`, senão o `languageVersion` do pacote,
    /// senão a versão corrente; `dart:*` no piso 3.6. Resolvida uma vez, na
    /// carga, antes do parse.
    pub features: LibraryFeatures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Mixin,
    Enum,
    ExtensionType,
    /// `class C = S with M;` — ou a classe sintética `S with M` gerada para
    /// cada mixin numa cláusula `with` (`S&M`).
    MixinApplication,
}

/// Onde a declaração está na árvore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclRef {
    pub unit: UnitId,
    pub decl: DeclId,
}

#[derive(Debug)]
pub struct TypeParameterElement {
    pub name: SymbolId,
    /// `(unidade, tipo na arena)` do bound escrito, resolvido em `types`.
    pub bound: Option<(UnitId, ast::TypeId)>,
}

#[derive(Debug)]
pub struct ClassElement {
    pub name: SymbolId,
    pub library: LibraryId,
    /// Declaração de origem; `None` em classes sintéticas de mixin.
    pub decl: Option<DeclRef>,
    pub kind: ClassKind,
    pub modifiers: ast::ClassModifiers,
    pub type_params: Vec<TypeParameterElement>,
    /// Supertipos como escritos, para `types` instanciar com argumentos.
    pub supertype: Option<(UnitId, ast::TypeId)>,
    pub mixins: Vec<(UnitId, ast::TypeId)>,
    pub interfaces: Vec<(UnitId, ast::TypeId)>,
    /// `on` de mixins.
    pub on: Vec<(UnitId, ast::TypeId)>,
    /// Classe do supertipo, já resolvida pelo nome (`Object` quando omitida,
    /// `None` só para `Object` mesmo). Argumentos de tipo ficam em `types`.
    pub supertype_class: Option<ClassId>,
    pub mixin_classes: Vec<ClassId>,
    pub interface_classes: Vec<ClassId>,
    pub on_classes: Vec<ClassId>,
    /// Membros de instância por nome; getters e setters são entradas
    /// separadas (`x` e `x=`), operadores pelo texto (`+`, `[]=`).
    pub instance_members: HashMap<SymbolId, FunctionElementId>,
    pub static_members: HashMap<SymbolId, FunctionElementId>,
    /// Construtores por nome; o sem nome usa o símbolo vazio `""`. `BTreeMap`
    /// para a iteração ser determinística (a emissão percorre o mapa).
    pub constructors: BTreeMap<SymbolId, FunctionElementId>,
    /// Campos de instância e estáticos, na ordem de declaração.
    pub fields: Vec<VariableId>,
    /// Constantes de um `enum`, na ordem.
    pub enum_constants: Vec<VariableId>,
    /// Tipo de representação de um extension type.
    pub representation: Option<VariableId>,
}

#[derive(Debug)]
pub struct ExtensionElement {
    pub name: Option<SymbolId>,
    pub library: LibraryId,
    pub decl: DeclRef,
    pub type_params: Vec<TypeParameterElement>,
    pub on: (UnitId, ast::TypeId),
    pub instance_members: HashMap<SymbolId, FunctionElementId>,
    pub static_members: HashMap<SymbolId, FunctionElementId>,
    pub fields: Vec<VariableId>,
}

#[derive(Debug)]
pub struct TypedefElement {
    pub name: SymbolId,
    pub library: LibraryId,
    pub decl: DeclRef,
    pub type_params: Vec<TypeParameterElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Function,
    Getter,
    Setter,
    Operator,
    Constructor,
    /// Construtor sintético (padrão sem nome, ou de aplicação de mixin).
    SyntheticConstructor,
    /// Getter/setter implícitos de um campo ou variável de topo.
    ImplicitAccessor,
}

/// Onde a função está na árvore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRef {
    /// Função de topo ou método: `Ast::functions[id]`.
    Function { unit: UnitId, function: FunctionId },
    /// Construtor: `Ast::members[id]` com `MemberKind::Constructor`.
    Constructor { unit: UnitId, member: MemberId },
    /// Sem nó (sintético).
    None,
}

#[derive(Debug)]
pub struct FunctionElement {
    pub name: SymbolId,
    pub library: LibraryId,
    /// Classe/mixin/enum dona, para membros; `None` em funções de topo.
    pub class: Option<ClassId>,
    pub extension: Option<ExtensionId>,
    pub kind: FunctionKind,
    pub static_: bool,
    pub abstract_: bool,
    pub external: bool,
    pub const_: bool,
    pub factory: bool,
    pub node: FunctionRef,
    /// Para acessores implícitos, a variável de origem.
    pub variable: Option<VariableId>,
    /// Membro de patch que substituiu este `external` (o corpo a compilar é
    /// o dele); `None` quando não há patch.
    pub patched_by: Option<FunctionElementId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableRef {
    /// Variável de topo: `Ast::decls[decl]` é `DeclKind::Variables` e
    /// `index` é a posição na lista.
    TopLevel {
        unit: UnitId,
        decl: DeclId,
        index: usize,
    },
    /// Campo: `Ast::members[member]` é `MemberKind::Field`.
    Field {
        unit: UnitId,
        member: MemberId,
        index: usize,
    },
    /// Constante de enum: `EnumDecl::constants[index]`.
    EnumConstant {
        unit: UnitId,
        decl: DeclId,
        index: usize,
    },
    /// Campo de representação de extension type.
    Representation {
        unit: UnitId,
        decl: DeclId,
    },
    None,
}

#[derive(Debug)]
pub struct VariableElement {
    pub name: SymbolId,
    pub library: LibraryId,
    pub class: Option<ClassId>,
    pub extension: Option<ExtensionId>,
    pub static_: bool,
    pub final_: bool,
    pub const_: bool,
    pub late: bool,
    pub external: bool,
    pub node: VariableRef,
    pub getter: Option<FunctionElementId>,
    pub setter: Option<FunctionElementId>,
}

impl Program {
    pub fn unit(&self, id: UnitId) -> &Unit {
        &self.units[id.0 as usize]
    }
    pub fn library(&self, id: LibraryId) -> &Library {
        &self.libraries[id.0 as usize]
    }
    pub fn class(&self, id: ClassId) -> &ClassElement {
        &self.classes[id.0 as usize]
    }
    pub fn extension(&self, id: ExtensionId) -> &ExtensionElement {
        &self.extensions[id.0 as usize]
    }
    pub fn typedef(&self, id: TypedefId) -> &TypedefElement {
        &self.typedefs[id.0 as usize]
    }
    pub fn function(&self, id: FunctionElementId) -> &FunctionElement {
        &self.functions[id.0 as usize]
    }
    pub fn variable(&self, id: VariableId) -> &VariableElement {
        &self.variables[id.0 as usize]
    }

    /// Resolve um nome de topo no escopo de uma biblioteca (sem prefixo).
    pub fn lookup(&self, library: LibraryId, name: SymbolId) -> Option<Binding> {
        self.library(library).scope.get(&name).copied()
    }

    /// Resolve `prefixo.nome`.
    pub fn lookup_prefixed(
        &self,
        library: LibraryId,
        prefix: SymbolId,
        name: SymbolId,
    ) -> Option<Binding> {
        self.library(library)
            .prefixes
            .get(&prefix)
            .and_then(|namespace| namespace.get(&name).copied())
    }
}
