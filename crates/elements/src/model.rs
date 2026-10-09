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
use dartforge_diagnostics::Span;
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
    /// As declarações `augment class`/`augment mixin` de cada classe, na
    /// ordem de aplicação (docs/AUGMENTATIONS.md). Vazio num programa sem
    /// augmentations.
    pub augmentacoes: HashMap<ClassId, Vec<DeclRef>>,
    /// As classes declaradas com `macro` (experimento `macros`), anotadas na
    /// mesma passada que cria os elementos. Vazio ⇒ nenhuma anotação pode ser
    /// aplicação de macro, e o hospedeiro de macros nem é consultado
    /// (regra de custo zero, docs/MACROS-PROTOCOLO.md §2).
    pub classes_macro: Vec<ClassId>,
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
    /// Biblioteca de augmentation da forma 3.6 (`import augment 'x'` +
    /// `augment library 'y'`): pertence à biblioteca, como uma parte, e as
    /// suas declarações `augment` se fundem nas da biblioteca
    /// (docs/AUGMENTATIONS.md). Na forma atual da spec a augmentation vive
    /// numa [`UnitRole::Part`] comum.
    Augmentation,
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
    /// Os trechos da fonte que a recuperação de erro do parser descartou
    /// (`dartforge_frontend::parser::Parsed::pulados`); vazio numa unidade
    /// sem erro de sintaxe e nas que vêm do cache do SDK.
    pub pulados: Vec<dartforge_diagnostics::Span>,
    /// Qual analyzer é a referência de nomes de código e de textos deste
    /// arquivo (`dartforge_frontend::parser::Parsed::referencia`). As
    /// unidades do SDK são do 3.6.2.
    pub referencia: dartforge_diagnostics::Referencia,
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
    /// O arquivo que incluiu cada parte (ou augmentation) da biblioteca: a
    /// árvore de partes (`part` dentro de `part`, `parts-with-imports`).
    pub pais: HashMap<UnitId, UnitId>,
    /// O escopo de cada unidade quando alguma parte tem imports próprios
    /// (`parts-with-imports`): os imports de um arquivo valem nele e nas
    /// partes dele, e os de dentro escondem os de fora. Vazio quando só o
    /// arquivo da biblioteca importa — aí vale [`Library::scope`] para
    /// todas. Consulte por [`Program::lookup_na_unidade`].
    pub escopos_de_unidade: HashMap<UnitId, EscopoDeUnidade>,
    /// Para cada elemento de topo com homônimo da MESMA espécie na
    /// biblioteca (duas classes `A`, duas funções `f`): o representante do
    /// grupo, o primeiro em ordem de fonte — ele próprio inclusive. Sem
    /// homônimo, ausente. É a "chave de mapa" do analyzer, que iguala
    /// elementos pela espécie e pela localização: os caches por elemento
    /// (interface, hierarquia, nomes de membros já vistos) são um só para o
    /// grupo (docs/ANALYZER-ESPECIFICACAO.md §G, T1.1 c–d). A identidade de
    /// tipo continua sendo a de cada elemento.
    pub representante: HashMap<Element, Element>,
    /// T1.1 d: para cada classe com homônimo da mesma espécie, a classe cujo
    /// cálculo de interface e de hierarquia vale para o grupo inteiro — a
    /// que o analyzer consulta primeiro (`outline::calcular_donos_de_homonimos`).
    /// Sem homônimo, ausente.
    pub dono_do_grupo: HashMap<ClassId, ClassId>,
}

/// Escopo de nomes de topo de uma unidade (ver [`Library::escopos_de_unidade`]).
#[derive(Debug, Clone, Default)]
pub struct EscopoDeUnidade {
    /// Como [`Library::scope`], com os imports da cadeia da unidade.
    pub scope: Namespace,
    /// Como [`Library::prefixes`], com os prefixos da cadeia da unidade.
    pub prefixes: HashMap<SymbolId, Namespace>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// separadas (`x` e `x_=`), operadores pelo texto (`+`, `[]=`).
    pub instance_members: HashMap<SymbolId, FunctionElementId>,
    /// Os membros de instância que um homônimo posterior tirou de
    /// [`ClassElement::instance_members`] (declaração duplicada), em ordem.
    /// O mapa declarado da interface (`_getTypeMembers`) não segue a ordem
    /// da fonte: métodos, depois os acessores dos campos, depois os acessores
    /// escritos (`_visitPropertyFirst`), com o último vencendo.
    pub instancia_sobrepostos: Vec<(SymbolId, FunctionElementId)>,
    pub static_members: HashMap<SymbolId, FunctionElementId>,
    /// Construtores por nome; o sem nome usa o símbolo vazio `""`. A chave é
    /// o `SymbolId`, cuja ordem é a da **internação**: numa sessão residente
    /// (`dartforge dev`) ela depende da ordem das edições, então percorrer o
    /// mapa direto faz a saída de uma recompilação divergir da de uma
    /// compilação limpa. Quem emite percorre [`ClassElement::construtores`].
    pub constructors: BTreeMap<SymbolId, FunctionElementId>,
    /// Campos de instância e estáticos, na ordem de declaração.
    pub fields: Vec<VariableId>,
    /// Constantes de um `enum`, na ordem.
    pub enum_constants: Vec<VariableId>,
    /// Tipo de representação de um extension type.
    pub representation: Option<VariableId>,
}

impl ClassElement {
    /// Os construtores na ordem de declaração na fonte (o outline numera as
    /// funções nessa ordem), que não depende da ordem de internação dos nomes.
    pub fn construtores(&self) -> Vec<(SymbolId, FunctionElementId)> {
        let mut v: Vec<(SymbolId, FunctionElementId)> = self.constructors.iter().map(|(&s, &f)| (s, f)).collect();
        v.sort_by_key(|&(_, f)| f);
        v
    }
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

#[derive(Debug, Clone)]
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
    /// Membro de patch do SDK → a declaração pública que ele substitui (o
    /// inverso de `patched_by`, sempre a de origem, mesmo com dois patches
    /// do mesmo membro). O analyzer não aplica patches: a assinatura e os
    /// modificadores (`const`, `factory`, `external`) que a análise vê são
    /// os da declaração pública — o patch só dá o corpo
    /// (docs/ANALYZER-ESPECIFICACAO.md §G, T3). `None` em código do usuário e
    /// em membros que só existem no patch.
    pub declaracao_publica: Option<FunctionElementId>,
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

#[derive(Debug, Clone)]
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
    /// A referência do arquivo `u` (docs/ANALYZER-ESPECIFICACAO.md, T2): a
    /// regra do corpus é por arquivo.
    pub fn referencia(&self, u: UnitId) -> dartforge_diagnostics::Referencia {
        self.unit(u).referencia
    }
    /// A referência de uma biblioteca, para as regras que são da biblioteca
    /// inteira (juntar classes homônimas): a maior entre as das unidades.
    pub fn referencia_da_biblioteca(&self, l: LibraryId) -> dartforge_diagnostics::Referencia {
        self.library(l).units.iter().map(|&u| self.unit(u).referencia).max().unwrap_or_default()
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
    /// O elemento cuja ASSINATURA a análise vê: a declaração pública quando
    /// `f` é um membro de patch do SDK ([`FunctionElement::declaracao_publica`]);
    /// senão o próprio `f`. Os emissores continuam com `f` (o corpo é o do
    /// patch).
    pub fn publico(&self, f: FunctionElementId) -> FunctionElementId {
        self.function(f).declaracao_publica.unwrap_or(f)
    }
    pub fn variable(&self, id: VariableId) -> &VariableElement {
        &self.variables[id.0 as usize]
    }

    /// T1 (docs/ANALYZER-ESPECIFICACAO.md §G): o representante do grupo de
    /// homônimos da classe `c` — a primeira declaração de classe, mixin,
    /// enum ou tipo de extensão com o mesmo nome e a mesma espécie na
    /// biblioteca; sem homônimo, a própria `c`. É a classe cuja interface e
    /// cuja hierarquia o analyzer guarda em cache para o grupo inteiro
    /// (`ElementImpl.==` por espécie e localização): os verificadores
    /// calculam a interface do representante e percorrem a árvore de cada
    /// declaração.
    pub fn representante_da_classe(&self, c: ClassId) -> ClassId {
        match self.library(self.class(c).library).representante.get(&Element::Class(c)) {
            Some(Element::Class(r)) => *r,
            _ => c,
        }
    }

    /// T1.1 d: a classe cuja interface e cuja hierarquia valem para `c` (o
    /// dono do cache do grupo de homônimos); sem homônimo, a própria `c`.
    /// Os tipos continuam distintos: só os supertipos e os membros da
    /// interface vêm do dono.
    pub fn dono_da_classe(&self, c: ClassId) -> ClassId {
        let lib = self.library(self.class(c).library);
        if lib.dono_do_grupo.is_empty() {
            return c;
        }
        lib.dono_do_grupo.get(&c).copied().unwrap_or(c)
    }

    /// T1: a classe `c` tem homônimo da mesma espécie na biblioteca (ela é
    /// uma das declarações repetidas, a primeira inclusive)?
    pub fn classe_com_homonimo(&self, c: ClassId) -> bool {
        self.library(self.class(c).library).representante.contains_key(&Element::Class(c))
    }

    /// A biblioteca tem augmentations de verdade: o experimento
    /// `augmentations` (ou `macros`) está ligado nela. Sem ele, `augment`
    /// não é modificador — o parser lê `augment class A {}` como a variável
    /// de topo `augment` seguida de uma segunda declaração `A` — e nenhum
    /// filtro de "declaração aumentada" se aplica (T1.1 f).
    pub fn biblioteca_com_augmentations(&self, lib: LibraryId) -> bool {
        let f = self.library(lib).features;
        f.tem(dartforge_frontend::Feature::Augmentations) || f.tem(dartforge_frontend::Feature::Macros)
    }

    /// Os membros sintáticos de uma classe: os da declaração introdutória
    /// seguidos dos de cada augmentation, na ordem de aplicação, cada um com
    /// a sua unidade. Um membro cuja declaração foi completada por outra da
    /// cadeia continua na lista; quem emite pega o elemento efetivo pelo mapa
    /// de membros e pula o membro cujo elemento não é o efetivo.
    /// `source.fullName`: o caminho nativo do arquivo da unidade (a URI, sem
    /// arquivo).
    pub fn caminho_da_unidade(&self, u: UnitId) -> String {
        let unit = self.unit(u);
        match &unit.path {
            Some(p) => p.display().to_string(),
            None => unit.uri.clone(),
        }
    }

    /// O nome declarado de uma classe (`nameOffset`/`nameLength`); nenhum na
    /// sintética.
    pub fn nome_da_classe(&self, c: ClassId) -> Option<(UnitId, Span)> {
        let d = self.class(c).decl?;
        let n = match &self.unit(d.unit).ast.decl(d.decl).kind {
            ast::DeclKind::Class(x) => x.name,
            ast::DeclKind::Mixin(x) => x.name,
            ast::DeclKind::Enum(x) => x.name,
            ast::DeclKind::ExtensionType(x) => x.name,
            _ => return None,
        };
        Some((d.unit, n.span))
    }

    /// O nome declarado de uma variável (de topo, campo, constante de enum,
    /// representação).
    pub fn nome_da_variavel(&self, v: VariableId) -> Option<(UnitId, Span)> {
        match self.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &self.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(l) => Some((unit, l.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &self.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => Some((unit, l.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => match &self.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::Representation { unit, decl } => match &self.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::ExtensionType(x) => Some((unit, x.representation_name.span)),
                _ => None,
            },
            VariableRef::None => None,
        }
    }

    /// `element.nonSynthetic` + `nameOffset`/`nameLength` de uma função
    /// (docs/ANALYZER-ESPECIFICACAO-INFRA.md III.3 item 4, o
    /// `intervalo_de_relato`): o acessor implícito vale pela variável; o
    /// construtor sintético, pela classe; o construtor sem nome, pelo nome da
    /// classe escrito nele.
    pub fn nome_nao_sintetico_da_funcao(&self, f: FunctionElementId) -> Option<(UnitId, Span)> {
        let fe = self.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => Some((unit, self.unit(unit).ast.function(function).name?.span)),
            FunctionRef::Constructor { unit, member } => match &self.unit(unit).ast.member(member).kind {
                ast::MemberKind::Constructor(k) => Some((unit, k.name.unwrap_or(k.class_name).span)),
                _ => None,
            },
            FunctionRef::None => match (fe.variable, fe.class) {
                (Some(v), _) => self.nome_da_variavel(v),
                (None, Some(c)) => self.nome_da_classe(c),
                _ => None,
            },
        }
    }

    pub fn membros_da_classe(&self, id: ClassId) -> Vec<(UnitId, MemberId)> {
        let class = self.class(id);
        let Some(decl) = class.decl else { return Vec::new() };
        let mut out = Vec::new();
        let decls = std::iter::once(decl).chain(self.augmentacoes.get(&id).into_iter().flatten().copied());
        for d in decls {
            let ast = &self.unit(d.unit).ast;
            let membros: &[MemberId] = match &ast.decl(d.decl).kind {
                ast::DeclKind::Class(c) => &c.members,
                ast::DeclKind::Mixin(m) => &m.members,
                ast::DeclKind::Enum(e) => &e.members,
                ast::DeclKind::ExtensionType(e) => &e.members,
                _ => &[],
            };
            out.extend(membros.iter().map(|&m| (d.unit, m)));
        }
        out
    }

    /// Resolve um nome de topo no escopo de uma biblioteca (sem prefixo).
    pub fn lookup(&self, library: LibraryId, name: SymbolId) -> Option<Binding> {
        self.library(library).scope.get(&name).copied()
    }

    /// Resolve um nome de topo no escopo de uma **unidade**: com
    /// `parts-with-imports`, cada arquivo vê os próprios imports (e os dos
    /// arquivos que o incluíram, que os de dentro escondem); sem imports em
    /// partes, é [`Program::lookup`] da biblioteca.
    pub fn lookup_na_unidade(&self, unit: UnitId, name: SymbolId) -> Option<Binding> {
        let lib = self.library(self.unit(unit).library);
        match lib.escopos_de_unidade.get(&unit) {
            Some(e) => e.scope.get(&name).copied(),
            None => lib.scope.get(&name).copied(),
        }
    }

    /// `prefixo.nome` no escopo de uma unidade (ver [`Program::lookup_na_unidade`]).
    pub fn lookup_prefixed_na_unidade(&self, unit: UnitId, prefix: SymbolId, name: SymbolId) -> Option<Binding> {
        self.prefixos_na_unidade(unit).get(&prefix).and_then(|ns| ns.get(&name).copied())
    }

    /// Os prefixos de import visíveis numa unidade.
    pub fn prefixos_na_unidade(&self, unit: UnitId) -> &HashMap<SymbolId, Namespace> {
        let lib = self.library(self.unit(unit).library);
        match lib.escopos_de_unidade.get(&unit) {
            Some(e) => &e.prefixes,
            None => &lib.prefixes,
        }
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
