//! Element model: the *outline* of a Dart program, without analyzed bodies.
//!
//! It is what the CFE calls the outline and the `analyzer` calls the element
//! model: for each library, what it declares and what it sees; for each
//! class, its members by name and its supertypes by class. No function body
//! is touched here — resolving `a.b` inside a body depends on the type of
//! `a`, and that belongs to the `types` phase.
//!
//! Memory rules (docs/FRONTEND-ARQUITETURA.md §2): everything in `Vec`s
//! indexed by `u32` ids; names are [`SymbolId`]; tree nodes are referenced
//! by `(UnitId, id in the arena)`, never copied.
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

id!(/// Index into [`Program::units`].
    UnitId);
id!(/// Index into [`Program::libraries`].
    LibraryId);
id!(/// Index into [`Program::classes`] (class, mixin, enum or extension type).
    ClassId);
id!(/// Index into [`Program::extensions`].
    ExtensionId);
id!(/// Index into [`Program::typedefs`].
    TypedefId);
id!(/// Index into [`Program::functions`] (top-level functions, methods, getters,
    /// setters, operators and constructors).
    FunctionElementId);
id!(/// Index into [`Program::variables`] (top-level variables and fields).
    VariableId);

/// Timings of the `load_lenient` sub-phases, for `compile-js --timings`.
#[derive(Debug, Default, Clone)]
pub struct LoadTimes {
    /// Serial reading and lexing (parts and SDK from files), plus the cost of
    /// taking from the prefetch what already came ready.
    pub read: std::time::Duration,
    /// Parallel read + lex waves (wall time).
    pub parallel_read_lex: std::time::Duration,
    pub waves: usize,
    /// Parsing (sequential: it needs the `Interner`).
    pub parse: std::time::Duration,
    /// Decoding of the SDK units coming from the cache.
    pub sdk_cache: std::time::Duration,
    /// Directive resolution: URIs, `canonicalize`, `verify_part_of`, queue.
    pub directives: std::time::Duration,
    /// `build_outline` phase 1: declarations and members.
    pub outline_declarations: std::time::Duration,
    /// `build_outline` phase 2: `exported` with the reexport fixpoint.
    pub outline_reexports: std::time::Duration,
    /// `build_outline` phase 3: lexical scope and prefixes per library.
    pub outline_scopes: std::time::Duration,
    /// `build_outline` phase 4: supertypes and cycles.
    pub outline_supertypes: std::time::Duration,
    /// Files read and source bytes (user and packages).
    pub files_read: usize,
    pub bytes_read: usize,
    /// Units that came ready from the resident session's cache.
    pub reused_units: usize,
}

/// The whole program: SDK, packages and the project, in a single library
/// graph.
#[derive(Debug, Default)]
pub struct Program {
    /// Timings of the load sub-phases (filled in by `load_lenient`).
    pub times: LoadTimes,
    pub units: Vec<Unit>,
    pub libraries: Vec<Library>,
    pub classes: Vec<ClassElement>,
    pub extensions: Vec<ExtensionElement>,
    pub typedefs: Vec<TypedefElement>,
    pub functions: Vec<FunctionElement>,
    pub variables: Vec<VariableElement>,
    /// The `dart:core` library, implicitly imported by all.
    pub core: Option<LibraryId>,
    /// The library of the program's entry point (the one declaring `main`),
    /// when there is one.
    pub entry: Option<LibraryId>,
    /// The `augment class`/`augment mixin` declarations of each class, in
    /// application order (docs/AUGMENTATIONS.md). Empty in a program without
    /// augmentations.
    pub augmentations: HashMap<ClassId, Vec<DeclRef>>,
    /// The classes declared with `macro` (`macros` experiment), recorded in the
    /// same pass that creates the elements. Empty ⇒ no annotation can be a macro
    /// application, and the macro host is not even consulted (zero-cost rule,
    /// docs/MACROS-PROTOCOLO.md §2).
    pub classes_macro: Vec<ClassId>,
    /// The elements by unit and by library, built on the first question
    /// ([`Program::index`]): the verifiers that run once per library or per
    /// unit would otherwise walk every element of the program each time.
    pub element_index: std::sync::OnceLock<ElementIndex>,
}

/// The program's elements grouped by unit and by library, each list in
/// increasing id order (a walk over a list visits the elements in the same
/// order as a filtered walk over the whole arena).
#[derive(Debug, Default)]
pub struct ElementIndex {
    /// The functions whose node ([`FunctionRef`]) is in each unit.
    pub unit_functions: Vec<Vec<FunctionElementId>>,
    /// The functions of each library ([`FunctionElement::library`]).
    pub library_functions: Vec<Vec<FunctionElementId>>,
    /// The variables of each library ([`VariableElement::library`]).
    pub library_variables: Vec<Vec<VariableId>>,
    /// The variables whose node ([`VariableRef`]) is in each unit.
    pub unit_variables: Vec<Vec<VariableId>>,
    /// The class declared by each declaration node, by unit.
    pub unit_classes: Vec<HashMap<DeclId, ClassId>>,
    /// The typedef declared by each declaration node, by unit.
    pub unit_typedefs: Vec<HashMap<DeclId, TypedefId>>,
    /// The extension declared by each declaration node, by unit.
    pub unit_extensions: Vec<HashMap<DeclId, ExtensionId>>,
}

/// Role of a file within its library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnitRole {
    /// File that declares the library (it has the imports).
    Library,
    /// A `part of` of the library.
    Part,
    /// SDK patch file (`libraries.json`), merged into the source library.
    Patch,
    /// Augmentation library in the 3.6 form (`import augment 'x'` +
    /// `augment library 'y'`): it belongs to the library, like a part, and its
    /// `augment` declarations merge into the library's (docs/AUGMENTATIONS.md).
    /// In the current form of the spec the augmentation lives in a plain
    /// [`UnitRole::Part`].
    Augmentation,
}

/// A `.dart` file already parsed.
#[derive(Debug)]
pub struct Unit {
    /// Canonical URI: `dart:core`, `package:x/y.dart` or `file:///…`.
    pub uri: String,
    pub path: Option<PathBuf>,
    pub source: String,
    pub ast: Ast,
    pub unit: CompilationUnit,
    pub library: LibraryId,
    pub role: UnitRole,
    /// Features the unit was parsed with (the library's). The resident session
    /// only reuses the unit if they stay the same.
    pub features: LibraryFeatures,
    /// The source ranges the parser's error recovery discarded
    /// (`dartforge_frontend::parser::Parsed::skipped`); empty in a unit without
    /// syntax errors and in those coming from the SDK cache.
    pub skipped: Vec<dartforge_diagnostics::Span>,
    /// Which analyzer is the reference for this file's code names and texts
    /// (`dartforge_frontend::parser::Parsed::reference`). SDK units are
    /// 3.6.2's.
    pub reference: dartforge_diagnostics::AnalyzerReference,
}

/// An import already resolved to a library of the program.
#[derive(Debug)]
pub struct Import {
    pub unit: UnitId,
    /// Index of the directive in `Unit::unit.directives`.
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

/// Reference to a nameable element in a library's scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    /// Top-level function, getter or setter.
    Function(FunctionElementId),
    /// Top-level variable (with implicit getter/setter).
    Variable(VariableId),
    /// Import prefix (`import 'x' as p;`): resolves to the owning library.
    Prefix(LibraryId, SymbolId),
}

/// Namespace entry: one name may map to a distinct getter and setter
/// (top-level `get x` and `set x` are separate elements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Binding {
    pub getter: Option<Element>,
    pub setter: Option<Element>,
    /// Two imports brought different elements with this name; using the name is
    /// an error, but the import itself is not (a Dart rule).
    pub ambiguous: bool,
}

/// Map of names → elements.
pub type Namespace = HashMap<SymbolId, Binding>;

#[derive(Debug)]
pub struct Library {
    pub uri: String,
    pub name: Option<Vec<SymbolId>>,
    /// Main unit, then parts and patches, in discovery order.
    pub units: Vec<UnitId>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    /// Elements declared in this library (including those from patches),
    /// private ones included.
    pub declared: Namespace,
    /// What the library exports: `declared` without private names, plus the
    /// exports.
    pub exported: Namespace,
    /// Resolution scope for top-level names: `declared` shadows the unprefixed
    /// imports; the implicit `dart:core` has the lowest precedence.
    pub scope: Namespace,
    /// Namespaces reachable through a prefix (`p.name`).
    pub prefixes: HashMap<SymbolId, Namespace>,
    /// SDK library (`dart:`), which may use `JS()`, `@patch`, `native`.
    pub is_sdk: bool,
    /// Language version and enabled features (`docs/VERSOES-LINGUAGEM.md`): the
    /// `// @dart = x.y` marker, otherwise the package's `languageVersion`,
    /// otherwise the current version; `dart:*` at the 3.6 floor. Resolved once,
    /// at load time, before parsing.
    pub features: LibraryFeatures,
    /// The file that included each part (or augmentation) of the library: the
    /// parts tree (`part` inside `part`, `parts-with-imports`).
    pub unit_parents: HashMap<UnitId, UnitId>,
    /// The scope of each unit when some part has its own imports
    /// (`parts-with-imports`): a file's imports apply to it and to its parts,
    /// and the inner ones hide the outer ones. Empty when only the library file
    /// imports — then [`Library::scope`] applies to all. Query it through
    /// [`Program::lookup_in_unit`].
    pub unit_scopes: HashMap<UnitId, UnitScope>,
    /// For each top-level element with a homonym of the SAME kind in the
    /// library (two classes `A`, two functions `f`): the group's representative,
    /// the first in source order — itself included. Without a homonym, absent.
    /// It is the analyzer's "map key", which equates elements by kind and
    /// location: the per-element caches (interface, hierarchy, member names
    /// already seen) are a single one for the group
    /// (docs/ANALYZER-ESPECIFICACAO.md §G, T1.1 c–d). Type identity remains that
    /// of each element.
    pub representative: HashMap<Element, Element>,
    /// T1.1 d: for each class with a homonym of the same kind, the class whose
    /// interface and hierarchy computation applies to the whole group — the one
    /// the analyzer consults first (`outline::compute_homonym_owners`). Without
    /// a homonym, absent.
    pub group_owner: HashMap<ClassId, ClassId>,
}

/// Top-level name scope of a unit (see [`Library::unit_scopes`]).
#[derive(Debug, Clone, Default)]
pub struct UnitScope {
    /// Like [`Library::scope`], with the imports of the unit's chain.
    pub scope: Namespace,
    /// Like [`Library::prefixes`], with the prefixes of the unit's chain.
    pub prefixes: HashMap<SymbolId, Namespace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Mixin,
    Enum,
    ExtensionType,
    /// `class C = S with M;` — or the synthetic class `S with M` generated for
    /// each mixin in a `with` clause (`S&M`).
    MixinApplication,
}

/// Where the declaration is in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeclRef {
    pub unit: UnitId,
    pub decl: DeclId,
}

#[derive(Debug)]
pub struct TypeParameterElement {
    pub name: SymbolId,
    /// `(unit, kind in the arena)` of the written bound, resolved in `types`.
    pub bound: Option<(UnitId, ast::TypeId)>,
}

#[derive(Debug)]
pub struct ClassElement {
    pub name: SymbolId,
    pub library: LibraryId,
    /// Source declaration; `None` for synthetic mixin classes.
    pub decl: Option<DeclRef>,
    pub kind: ClassKind,
    pub modifiers: ast::ClassModifiers,
    pub type_params: Vec<TypeParameterElement>,
    /// Supertypes as written, for `types` to instantiate with arguments.
    pub supertype: Option<(UnitId, ast::TypeId)>,
    pub mixins: Vec<(UnitId, ast::TypeId)>,
    pub interfaces: Vec<(UnitId, ast::TypeId)>,
    /// The `on` of mixins.
    pub on: Vec<(UnitId, ast::TypeId)>,
    /// The supertype's class, already resolved by name (`Object` when omitted,
    /// `None` only for `Object` itself). Type arguments stay in `types`.
    pub supertype_class: Option<ClassId>,
    pub mixin_classes: Vec<ClassId>,
    pub interface_classes: Vec<ClassId>,
    pub on_classes: Vec<ClassId>,
    /// Instance members by name; getters and setters are separate entries (`x`
    /// and `x_=`), operators by their text (`+`, `[]=`).
    pub instance_members: HashMap<SymbolId, FunctionElementId>,
    /// The instance members a later homonym removed from
    /// [`ClassElement::instance_members`] (duplicate declaration), in order. The
    /// interface's declared map (`_getTypeMembers`) does not follow source
    /// order: methods, then the fields' accessors, then the written accessors
    /// (`_visitPropertyFirst`), with the last one winning.
    pub shadowed_instance_members: Vec<(SymbolId, FunctionElementId)>,
    pub static_members: HashMap<SymbolId, FunctionElementId>,
    /// Constructors by name; the unnamed one uses the empty symbol `""`. The key
    /// is the `SymbolId`, whose order is that of **interning**: in a resident
    /// session (`dartforge dev`) it depends on the order of the edits, so
    /// walking the map directly makes a recompilation's output diverge from a
    /// clean compilation's. Emitters walk [`ClassElement::constructors`].
    pub constructors: BTreeMap<SymbolId, FunctionElementId>,
    /// Instance and static fields, in declaration order.
    pub fields: Vec<VariableId>,
    /// Constants of an `enum`, in order.
    pub enum_constants: Vec<VariableId>,
    /// Representation type of an extension type.
    pub representation: Option<VariableId>,
}

impl ClassElement {
    /// The constructors in source declaration order (the outline numbers the
    /// functions in that order), which does not depend on the interning order
    /// of the names.
    pub fn constructors(&self) -> Vec<(SymbolId, FunctionElementId)> {
        let mut v: Vec<(SymbolId, FunctionElementId)> =
            self.constructors.iter().map(|(&s, &f)| (s, f)).collect();
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
    /// Synthetic constructor (default unnamed, or a mixin application's).
    SyntheticConstructor,
    /// Implicit getter/setter of a field or top-level variable.
    ImplicitAccessor,
}

/// Where the function is in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRef {
    /// Top-level function or method: `Ast::functions[id]`.
    Function { unit: UnitId, function: FunctionId },
    /// Constructor: `Ast::members[id]` with `MemberKind::Constructor`.
    Constructor { unit: UnitId, member: MemberId },
    /// No node (synthetic).
    None,
}

#[derive(Debug, Clone)]
pub struct FunctionElement {
    pub name: SymbolId,
    pub library: LibraryId,
    /// Owning class/mixin/enum, for members; `None` for top-level functions.
    pub class: Option<ClassId>,
    pub extension: Option<ExtensionId>,
    pub kind: FunctionKind,
    pub static_: bool,
    pub abstract_: bool,
    pub external: bool,
    pub const_: bool,
    pub factory: bool,
    pub node: FunctionRef,
    /// For implicit accessors, the source variable.
    pub variable: Option<VariableId>,
    /// Patch member that replaced this `external` one (the body to compile is
    /// its); `None` when there is no patch.
    pub patched_by: Option<FunctionElementId>,
    /// SDK patch member → the public declaration it replaces (the inverse of
    /// `patched_by`, always the source one, even with two patches of the same
    /// member). The analyzer does not apply patches: the signature and the
    /// modifiers (`const`, `factory`, `external`) analysis sees are those of the
    /// public declaration — the patch only provides the body
    /// (docs/ANALYZER-ESPECIFICACAO.md §G, T3). `None` in user code and in
    /// members that only exist in the patch.
    pub public_declaration: Option<FunctionElementId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableRef {
    /// Top-level variable: `Ast::decls[decl]` is a `DeclKind::Variables` and
    /// `index` is the position in the list.
    TopLevel {
        unit: UnitId,
        decl: DeclId,
        index: usize,
    },
    /// Field: `Ast::members[member]` is a `MemberKind::Field`.
    Field {
        unit: UnitId,
        member: MemberId,
        index: usize,
    },
    /// Enum constant: `EnumDecl::constants[index]`.
    EnumConstant {
        unit: UnitId,
        decl: DeclId,
        index: usize,
    },
    /// Extension type representation field.
    Representation {
        unit: UnitId,
        decl: DeclId,
    },
    None,
}

impl VariableRef {
    /// The unit of the node, when there is one.
    pub fn unit(&self) -> Option<UnitId> {
        match *self {
            VariableRef::TopLevel { unit, .. }
            | VariableRef::Field { unit, .. }
            | VariableRef::EnumConstant { unit, .. }
            | VariableRef::Representation { unit, .. } => Some(unit),
            VariableRef::None => None,
        }
    }
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

    /// The elements by unit and by library ([`ElementIndex`]), built on the
    /// first call. Whoever changes the element arenas or the nodes calls
    /// [`Program::forget_index`].
    pub fn index(&self) -> &ElementIndex {
        self.element_index.get_or_init(|| {
            let mut x = ElementIndex {
                unit_functions: vec![Vec::new(); self.units.len()],
                library_functions: vec![Vec::new(); self.libraries.len()],
                library_variables: vec![Vec::new(); self.libraries.len()],
                unit_variables: vec![Vec::new(); self.units.len()],
                unit_classes: vec![HashMap::new(); self.units.len()],
                unit_typedefs: vec![HashMap::new(); self.units.len()],
                unit_extensions: vec![HashMap::new(); self.units.len()],
            };
            for (i, e) in self.extensions.iter().enumerate() {
                if let Some(m) = x.unit_extensions.get_mut(e.decl.unit.0 as usize) {
                    m.entry(e.decl.decl).or_insert(ExtensionId(i as u32));
                }
            }
            // The first element of each node, like a `position` over the arena.
            for (i, c) in self.classes.iter().enumerate() {
                if let Some(d) = c.decl
                    && let Some(m) = x.unit_classes.get_mut(d.unit.0 as usize)
                {
                    m.entry(d.decl).or_insert(ClassId(i as u32));
                }
            }
            for (i, t) in self.typedefs.iter().enumerate() {
                if let Some(m) = x.unit_typedefs.get_mut(t.decl.unit.0 as usize) {
                    m.entry(t.decl.decl).or_insert(TypedefId(i as u32));
                }
            }
            for (i, f) in self.functions.iter().enumerate() {
                let id = FunctionElementId(i as u32);
                if let Some(l) = x.library_functions.get_mut(f.library.0 as usize) {
                    l.push(id);
                }
                let unit = match f.node {
                    FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => {
                        unit
                    }
                    FunctionRef::None => continue,
                };
                if let Some(l) = x.unit_functions.get_mut(unit.0 as usize) {
                    l.push(id);
                }
            }
            for (i, v) in self.variables.iter().enumerate() {
                let id = VariableId(i as u32);
                if let Some(l) = x.library_variables.get_mut(v.library.0 as usize) {
                    l.push(id);
                }
                if let Some(unit) = v.node.unit()
                    && let Some(l) = x.unit_variables.get_mut(unit.0 as usize)
                {
                    l.push(id);
                }
            }
            x
        })
    }

    /// Drops the [`ElementIndex`] (after a change to the arenas or nodes).
    pub fn forget_index(&mut self) {
        self.element_index = std::sync::OnceLock::new();
    }

    /// The element of the function (or method) declared by node `function`
    /// of unit `unit`: the first one, in id order.
    pub fn function_of_node(
        &self,
        unit: UnitId,
        function: FunctionId,
    ) -> Option<FunctionElementId> {
        let l = self.index().unit_functions.get(unit.0 as usize)?;
        l.iter().copied().find(|&f| matches!(self.function(f).node, FunctionRef::Function { unit: u, function: g } if u == unit && g == function))
    }

    /// The class declared by node `decl` of unit `unit` (the first one, in id
    /// order).
    pub fn class_of_decl(&self, unit: UnitId, decl: DeclId) -> Option<ClassId> {
        self.index()
            .unit_classes
            .get(unit.0 as usize)?
            .get(&decl)
            .copied()
    }

    /// The typedef declared by node `decl` of unit `unit` (the first one, in
    /// id order).
    pub fn typedef_of_decl(&self, unit: UnitId, decl: DeclId) -> Option<TypedefId> {
        self.index()
            .unit_typedefs
            .get(unit.0 as usize)?
            .get(&decl)
            .copied()
    }

    /// The extension declared by node `decl` of unit `unit` (the first one,
    /// in id order).
    pub fn extension_of_decl(&self, unit: UnitId, decl: DeclId) -> Option<ExtensionId> {
        self.index()
            .unit_extensions
            .get(unit.0 as usize)?
            .get(&decl)
            .copied()
    }

    /// The variable whose node is `node` (the first one, in id order).
    pub fn variable_of_node(&self, node: VariableRef) -> Option<VariableId> {
        let unit = node.unit()?;
        let l = self.index().unit_variables.get(unit.0 as usize)?;
        l.iter().copied().find(|&v| self.variable(v).node == node)
    }

    /// The element of the constructor declared by member `member` of unit
    /// `unit`: the first one, in id order.
    pub fn constructor_of_member(
        &self,
        unit: UnitId,
        member: MemberId,
    ) -> Option<FunctionElementId> {
        let l = self.index().unit_functions.get(unit.0 as usize)?;
        l.iter().copied().find(|&f| matches!(self.function(f).node, FunctionRef::Constructor { unit: u, member: m } if u == unit && m == member))
    }
    /// The reference of file `u` (docs/ANALYZER-ESPECIFICACAO.md, T2): the
    /// corpus rule is per file.
    pub fn reference(&self, u: UnitId) -> dartforge_diagnostics::AnalyzerReference {
        self.unit(u).reference
    }
    /// The reference of a library, for the rules that belong to the whole
    /// library (merging homonymous classes): the highest among its units'.
    pub fn library_reference(&self, l: LibraryId) -> dartforge_diagnostics::AnalyzerReference {
        self.library(l)
            .units
            .iter()
            .map(|&u| self.unit(u).reference)
            .max()
            .unwrap_or_default()
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
    /// The element whose SIGNATURE analysis sees: the public declaration when
    /// `f` is an SDK patch member ([`FunctionElement::public_declaration`]);
    /// otherwise `f` itself. Emitters keep `f` (the body is the patch's).
    pub fn public(&self, f: FunctionElementId) -> FunctionElementId {
        self.function(f).public_declaration.unwrap_or(f)
    }
    pub fn variable(&self, id: VariableId) -> &VariableElement {
        &self.variables[id.0 as usize]
    }

    /// T1 (docs/ANALYZER-ESPECIFICACAO.md §G): the representative of class `c`'s
    /// homonym group — the first class, mixin, enum or extension type
    /// declaration with the same name and kind in the library; without a
    /// homonym, `c` itself. It is the class whose interface and hierarchy the
    /// analyzer caches for the whole group (`ElementImpl.==` by kind and
    /// location): the checkers compute the representative's interface and walk
    /// each declaration's tree.
    pub fn class_representative(&self, c: ClassId) -> ClassId {
        match self
            .library(self.class(c).library)
            .representative
            .get(&Element::Class(c))
        {
            Some(Element::Class(r)) => *r,
            _ => c,
        }
    }

    /// T1.1 d: the class whose interface and hierarchy apply to `c` (the owner
    /// of the homonym group's cache); without a homonym, `c` itself. The types
    /// stay distinct: only the supertypes and the interface members come from
    /// the owner.
    pub fn class_owner(&self, c: ClassId) -> ClassId {
        let lib = self.library(self.class(c).library);
        if lib.group_owner.is_empty() {
            return c;
        }
        lib.group_owner.get(&c).copied().unwrap_or(c)
    }

    /// T1: does class `c` have a homonym of the same kind in the library (is it
    /// one of the repeated declarations, the first included)?
    pub fn class_with_homonym(&self, c: ClassId) -> bool {
        self.library(self.class(c).library)
            .representative
            .contains_key(&Element::Class(c))
    }

    /// The library really has augmentations: the `augmentations` (or `macros`)
    /// experiment is on in it. Without it, `augment` is not a modifier — the
    /// parser reads `augment class A {}` as the top-level variable `augment`
    /// followed by a second declaration `A` — and no "augmented declaration"
    /// filter applies (T1.1 f).
    pub fn library_has_augmentations(&self, lib: LibraryId) -> bool {
        let f = self.library(lib).features;
        f.has(dartforge_frontend::Feature::Augmentations)
            || f.has(dartforge_frontend::Feature::Macros)
    }

    /// The syntactic members of a class: those of the introductory declaration
    /// followed by those of each augmentation, in application order, each with
    /// its unit. A member whose declaration was completed by another one of the
    /// chain stays in the list; emitters take the effective element from the
    /// member map and skip the member whose element is not the effective one.
    /// `source.fullName`: the native path of the unit's file (the URI, without
    /// a file).
    pub fn unit_path(&self, u: UnitId) -> String {
        let unit = self.unit(u);
        match &unit.path {
            Some(p) => p.display().to_string(),
            None => unit.uri.clone(),
        }
    }

    /// The declared name of a class (`nameOffset`/`nameLength`); none for a
    /// synthetic one.
    pub fn class_name(&self, c: ClassId) -> Option<(UnitId, Span)> {
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

    /// The declared name of a variable (top-level, field, enum constant,
    /// representation).
    pub fn variable_name(&self, v: VariableId) -> Option<(UnitId, Span)> {
        match self.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => {
                match &self.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Variables(l) => Some((unit, l.variables.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &self.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => Some((unit, l.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                match &self.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Representation { unit, decl } => {
                match &self.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::ExtensionType(x) => Some((unit, x.representation_name.span)),
                    _ => None,
                }
            }
            VariableRef::None => None,
        }
    }

    /// `element.nonSynthetic` + `nameOffset`/`nameLength` of a function
    /// (docs/ANALYZER-ESPECIFICACAO-INFRA.md III.3 item 4, the
    /// `intervalo_de_relato`): an implicit accessor counts through the variable;
    /// a synthetic constructor, through the class; the unnamed constructor,
    /// through the class name written in it.
    pub fn non_synthetic_function_name(&self, f: FunctionElementId) -> Option<(UnitId, Span)> {
        let fe = self.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => {
                Some((unit, self.unit(unit).ast.function(function).name?.span))
            }
            FunctionRef::Constructor { unit, member } => {
                match &self.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Constructor(k) => {
                        Some((unit, k.name.unwrap_or(k.class_name).span))
                    }
                    _ => None,
                }
            }
            FunctionRef::None => match (fe.variable, fe.class) {
                (Some(v), _) => self.variable_name(v),
                (None, Some(c)) => self.class_name(c),
                _ => None,
            },
        }
    }

    pub fn class_members(&self, id: ClassId) -> Vec<(UnitId, MemberId)> {
        let class = self.class(id);
        let Some(decl) = class.decl else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let decls =
            std::iter::once(decl).chain(self.augmentations.get(&id).into_iter().flatten().copied());
        for d in decls {
            let ast = &self.unit(d.unit).ast;
            let members_: &[MemberId] = match &ast.decl(d.decl).kind {
                ast::DeclKind::Class(c) => &c.members,
                ast::DeclKind::Mixin(m) => &m.members,
                ast::DeclKind::Enum(e) => &e.members,
                ast::DeclKind::ExtensionType(e) => &e.members,
                _ => &[],
            };
            out.extend(members_.iter().map(|&m| (d.unit, m)));
        }
        out
    }

    /// Resolves a top-level name in a library's scope (without a prefix).
    pub fn lookup(&self, library: LibraryId, name: SymbolId) -> Option<Binding> {
        self.library(library).scope.get(&name).copied()
    }

    /// Resolves a top-level name in the scope of a **unit**: with
    /// `parts-with-imports`, each file sees its own imports (and those of the
    /// files that included it, which the inner ones hide); without imports in
    /// parts, it is the library's [`Program::lookup`].
    pub fn lookup_in_unit(&self, unit: UnitId, name: SymbolId) -> Option<Binding> {
        let lib = self.library(self.unit(unit).library);
        match lib.unit_scopes.get(&unit) {
            Some(e) => e.scope.get(&name).copied(),
            None => lib.scope.get(&name).copied(),
        }
    }

    /// `prefix.name` in a unit's scope (see [`Program::lookup_in_unit`]).
    pub fn lookup_prefixed_in_unit(
        &self,
        unit: UnitId,
        prefix: SymbolId,
        name: SymbolId,
    ) -> Option<Binding> {
        self.prefixes_in_unit(unit)
            .get(&prefix)
            .and_then(|ns| ns.get(&name).copied())
    }

    /// The import prefixes visible in a unit.
    pub fn prefixes_in_unit(&self, unit: UnitId) -> &HashMap<SymbolId, Namespace> {
        let lib = self.library(self.unit(unit).library);
        match lib.unit_scopes.get(&unit) {
            Some(e) => &e.prefixes,
            None => &lib.prefixes,
        }
    }

    /// Resolve `prefix.name_`.
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
