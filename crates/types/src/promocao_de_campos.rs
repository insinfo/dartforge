//! Promoção de campos (Dart 3.2, `inference-update-2`): o
//! `FieldPromotability` de `_fe_analyzer_shared`
//! (`src/field_promotability.dart`) com o `_FieldPromotability` do analyzer
//! (`src/summary2/library_builder.dart:1466-1581`).
//!
//! Por biblioteca: cada classe, enum e mixin registra os campos e getters de
//! instância não sintéticos de nome privado. Um campo não final ou
//! `external`, ou um getter concreto, torna o nome não promovível; uma classe
//! concreta cuja interface tem o nome sem que a implementação o tenha (um
//! encaminhador de `noSuchMethod`) também. O resultado é o
//! `fieldNameNonPromotabilityInfo` da biblioteca (que o why-not-promoted
//! mostra) e os campos promovíveis (`isPromotable`).

use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// `FieldNameNonPromotabilityInfo`: as declarações que impedem a promoção de
/// um nome, na ordem em que o analyzer as encontra.
#[derive(Debug, Default, Clone)]
pub struct InfoDeNaoPromocao {
    /// Campos não finais ou `external` (`conflictingFields`).
    pub campos: Vec<VariableId>,
    /// Getters concretos (`conflictingGetters`).
    pub getters: Vec<FunctionElementId>,
    /// Classes concretas com encaminhador de `noSuchMethod`
    /// (`conflictingNsmClasses`).
    pub classes_nsm: Vec<ClassId>,
}

/// O resultado de uma biblioteca.
#[derive(Debug, Default, Clone)]
pub struct PromocaoDaBiblioteca {
    /// `fieldNameNonPromotabilityInfo`, pelo nome.
    pub info: HashMap<SymbolId, InfoDeNaoPromocao>,
    /// Campos com `isPromotable` (os de representação privados de tipos de
    /// extensão inclusive).
    pub campos: HashSet<VariableId>,
    /// Getters abstratos privados cujo campo sintético é promovível (o
    /// `accessor.variable2` do analyzer).
    pub getters: HashSet<FunctionElementId>,
}

/// `PropertyNonPromotabilityReason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PorQueNaoPromove {
    NaoECampo,
    NaoEPrivado,
    Externo,
    NaoFinal,
}

/// Se o recurso vale numa biblioteca (versão de linguagem 3.2 ou mais).
pub fn habilitada(program: &Program, lib: LibraryId) -> bool {
    program.library(lib).features.versao() >= dartforge_frontend::LanguageVersion::new(3, 2)
}

/// As classes, enums e mixins de uma unidade na ordem do analyzer: as
/// classes (e aliases de classe), depois os enums, depois os mixins, cada
/// grupo na ordem de declaração.
fn declaracoes_da_unidade(program: &Program, u: dartforge_elements::model::UnitId) -> Vec<(ClassId, bool)> {
    let mut classes = Vec::new();
    let mut enums = Vec::new();
    let mut mixins = Vec::new();
    for (i, c) in program.classes.iter().enumerate() {
        let Some(d) = c.decl else { continue };
        if d.unit != u {
            continue;
        }
        let id = ClassId(i as u32);
        match c.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                // `sealed` também é abstrata no elemento do analyzer.
                let abstrata = c.modifiers.abstract_ || c.modifiers.sealed;
                classes.push((d.decl.0, id, abstrata));
            }
            ClassKind::Enum => enums.push((d.decl.0, id, false)),
            ClassKind::Mixin => mixins.push((d.decl.0, id, true)),
            ClassKind::ExtensionType => {}
        }
    }
    classes.sort_by_key(|x| x.0);
    enums.sort_by_key(|x| x.0);
    mixins.sort_by_key(|x| x.0);
    classes.into_iter().chain(enums).chain(mixins).map(|(_, id, a)| (id, a)).collect()
}

/// `getSuperclasses`: o supertipo e os mixins; com `implements` (e o `on`
/// dos mixins) quando não os ignora.
fn superclasses(program: &Program, c: ClassId, ignorar_implements: bool) -> Vec<ClassId> {
    let ce = program.class(c);
    let mut r: Vec<ClassId> = ce.supertype_class.into_iter().collect();
    r.extend(ce.mixin_classes.iter().copied());
    if !ignorar_implements {
        r.extend(ce.interface_classes.iter().copied());
        if ce.kind == ClassKind::Mixin {
            r.extend(ce.on_classes.iter().copied());
        }
    }
    r
}

/// Os nomes de `_directNames` de cada nó alcançável a partir de `c` (o
/// `_ClassHierarchyWalker`: a união transitiva).
fn nomes_transitivos(program: &Program, c: ClassId, diretos: &HashMap<ClassId, HashSet<SymbolId>>, ignorar_implements: bool) -> HashSet<SymbolId> {
    let mut vistos: HashSet<ClassId> = HashSet::new();
    let mut pilha = vec![c];
    let mut nomes = HashSet::new();
    while let Some(x) = pilha.pop() {
        if !vistos.insert(x) {
            continue;
        }
        if let Some(d) = diretos.get(&x) {
            nomes.extend(d.iter().copied());
        }
        pilha.extend(superclasses(program, x, ignorar_implements));
    }
    nomes
}

/// O `abstract` escrito de um campo.
fn campo_abstrato(program: &Program, v: VariableId) -> bool {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => l.abstract_,
            _ => false,
        },
        _ => false,
    }
}

/// `_FieldPromotability.perform` de uma biblioteca.
pub fn calcular(program: &Program, interner: &Interner, lib: LibraryId) -> PromocaoDaBiblioteca {
    let ligada = habilitada(program, lib);
    let mut info: HashMap<SymbolId, InfoDeNaoPromocao> = HashMap::new();
    let mut interface: HashMap<ClassId, HashSet<SymbolId>> = HashMap::new();
    let mut implementados: HashMap<ClassId, HashSet<SymbolId>> = HashMap::new();
    let mut concretas: Vec<ClassId> = Vec::new();
    let mut possiveis_campos: Vec<VariableId> = Vec::new();
    let mut possiveis_getters: Vec<FunctionElementId> = Vec::new();
    let mut r = PromocaoDaBiblioteca::default();
    let privado = |s: SymbolId| interner.resolve(s).starts_with('_');
    for &u in &program.library(lib).units {
        for (c, abstrata) in declaracoes_da_unidade(program, u) {
            if !abstrata {
                concretas.push(c);
            }
            interface.entry(c).or_default();
            implementados.entry(c).or_default();
            let ce = program.class(c);
            // Os campos de instância não sintéticos, na ordem de declaração.
            for &v in &ce.fields {
                let ve = program.variable(v);
                if ve.static_ {
                    continue;
                }
                if !privado(ve.name) {
                    continue;
                }
                let abstrato = campo_abstrato(program, v);
                interface.get_mut(&c).unwrap().insert(ve.name);
                if !abstrato {
                    implementados.get_mut(&c).unwrap().insert(ve.name);
                }
                if ve.external || !ve.final_ {
                    info.entry(ve.name).or_default().campos.push(v);
                } else if ligada {
                    possiveis_campos.push(v);
                }
            }
            // Os getters de instância não sintéticos, na ordem de declaração.
            let mut getters: Vec<FunctionElementId> = ce
                .instance_members
                .values()
                .copied()
                .filter(|&f| {
                    let fe = program.function(f);
                    fe.kind == FunctionKind::Getter && !fe.static_ && fe.class == Some(c)
                })
                .collect();
            getters.sort();
            for f in getters {
                let fe = program.function(f);
                if !privado(fe.name) {
                    continue;
                }
                interface.get_mut(&c).unwrap().insert(fe.name);
                if !fe.abstract_ {
                    implementados.get_mut(&c).unwrap().insert(fe.name);
                    info.entry(fe.name).or_default().getters.push(f);
                } else if ligada {
                    possiveis_getters.push(f);
                }
            }
        }
        // Representação privada de tipo de extensão: sempre promovível.
        for c in program.classes.iter() {
            if c.kind != ClassKind::ExtensionType || c.decl.is_none_or(|d| d.unit != u) {
                continue;
            }
            if let Some(rep) = c.representation {
                if privado(program.variable(rep).name) {
                    r.campos.insert(rep);
                }
            }
        }
    }
    // `computeNonPromotabilityInfo`: as classes concretas cuja interface tem
    // um nome que a implementação não tem.
    for &c in &concretas {
        let nomes_da_interface = nomes_transitivos(program, c, &interface, false);
        let nomes_implementados = nomes_transitivos(program, c, &implementados, true);
        let mut faltam: Vec<SymbolId> = nomes_da_interface.into_iter().filter(|n| !nomes_implementados.contains(n)).collect();
        faltam.sort();
        for n in faltam {
            info.entry(n).or_default().classes_nsm.push(c);
        }
    }
    for v in possiveis_campos {
        if !info.contains_key(&program.variable(v).name) {
            r.campos.insert(v);
        }
    }
    for f in possiveis_getters {
        if !info.contains_key(&program.function(f).name) {
            r.getters.insert(f);
        }
    }
    r.info = info;
    r
}
