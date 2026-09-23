//! Fusão das declarações `augment` no outline (docs/AUGMENTATIONS.md).
//!
//! Fonte normativa: `working/augmentations/feature-specification.md` v1.46,
//! com a divergência do CFE 3.6.2 (D9): uma declaração `external` pode ser
//! completada por uma augmentation (é o que a saída das macros do 3.6.2 faz:
//! `external C.fromJson(...)` e depois `augment C.fromJson(...) : ...`), e não
//! há `augmented()` nem embrulho de corpo.
//!
//! É a generalização do mecanismo de patch do SDK (o CFE reusou a mesma
//! infraestrutura): a declaração `augment` não cria nome novo; ela se liga à
//! declaração de mesmo nome que vem **antes** dela na ordem de aplicação (a
//! pré-ordem da árvore de partes, que o carregador deixa em
//! [`Library::units`]).
//!
//! Invariante da cadeia de um membro, a mesma dos patches: a entrada do mapa
//! de membros (e do `declared` da biblioteca) aponta sempre para o elemento
//! **efetivo** — a última declaração completa da cadeia, ou a introdutória se
//! nenhuma é completa —, e todo outro elemento da cadeia tem `patched_by`
//! apontando para ele. Quem emite percorre só os elementos com
//! `patched_by == None`.
//!
//! O que os emissores veem de uma classe aumentada: os membros das
//! declarações da cadeia ([`Program::membros_da_classe`]), um elemento por
//! membro, e os supertipos acrescentados (`implements`/`with`/`extends`).
use crate::model::*;
use crate::outline::{extract_members, ElementPools};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, FunctionBody, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{BTreeMap, HashMap};

/// O que a fusão precisa ler e escrever, além dos elementos.
pub(crate) struct Fusao<'a> {
    pub units: &'a [Unit],
    pub library: &'a mut Library,
    pub lib_id: LibraryId,
    pub empty_sym: SymbolId,
    pub interner: &'a mut Interner,
    pub diagnostics: &'a mut Vec<Diagnostic>,
    /// Declarações de augmentation de cada classe, na ordem de aplicação.
    pub cadeias: &'a mut Vec<(ClassId, DeclRef)>,
}

impl Fusao<'_> {
    fn erro(&mut self, unit: UnitId, msg: impl Into<String>, span: Span) {
        let u = &self.units[unit.0 as usize];
        self.diagnostics.push(Diagnostic::new(format!("{}: {}", u.uri, msg.into()), span));
    }
}

/// Aplica a declaração de topo `augment` `decl_id` da unidade `unit_id`.
pub(crate) fn aplicar(f: &mut Fusao<'_>, pools: &mut ElementPools, unit_id: UnitId, decl_id: ast::DeclId) {
    let ast = &f.units[unit_id.0 as usize].ast;
    let decl = ast.decl(decl_id);
    let span = decl.span;
    match &decl.kind {
        DeclKind::Class(c) => {
            if c.mixin_application {
                f.erro(unit_id, "uma aplicação de mixin ('class C = S with M') não pode ser aumentada", span);
                return;
            }
            let Some(cid) = classe_introdutoria(f, pools, unit_id, c.name, ClassKind::Class) else { return };
            let classe = &mut pools.classes[cid.0 as usize];
            if let Some(t) = c.extends {
                if classe.supertype.is_some() {
                    f.erro(unit_id, format!("a classe '{}' já tem cláusula 'extends'", nome(f, c.name.sym)), span);
                } else {
                    classe.supertype = Some((unit_id, t));
                }
            }
            classe.mixins.extend(c.with.iter().map(|t| (unit_id, *t)));
            classe.interfaces.extend(c.implements.iter().map(|t| (unit_id, *t)));
            f.cadeias.push((cid, DeclRef { unit: unit_id, decl: decl_id }));
            aplicar_membros(f, pools, cid, unit_id, &c.members);
        }
        DeclKind::Mixin(m) => {
            let Some(cid) = classe_introdutoria(f, pools, unit_id, m.name, ClassKind::Mixin) else { return };
            let classe = &mut pools.classes[cid.0 as usize];
            classe.on.extend(m.on.iter().map(|t| (unit_id, *t)));
            classe.interfaces.extend(m.implements.iter().map(|t| (unit_id, *t)));
            f.cadeias.push((cid, DeclRef { unit: unit_id, decl: decl_id }));
            aplicar_membros(f, pools, cid, unit_id, &m.members);
        }
        DeclKind::Function(fid) => {
            let func = ast.function(*fid);
            let Some(n) = func.name else { return };
            let kind = tipo_de_funcao(func.kind);
            let entrada = f.library.declared.get(&n.sym).copied().unwrap_or_default();
            let anterior = match kind {
                FunctionKind::Setter => entrada.setter,
                _ => entrada.getter,
            };
            let Some(Element::Function(antigo)) = anterior else {
                f.erro(
                    unit_id,
                    format!("'augment' de '{}' sem declaração introdutória antes dele", nome(f, n.sym)),
                    n.span,
                );
                return;
            };
            let novo = FunctionElementId(pools.functions.len() as u32);
            pools.functions.push(FunctionElement {
                name: n.sym,
                library: f.lib_id,
                class: None,
                extension: None,
                kind,
                static_: func.static_,
                abstract_: false,
                external: func.external,
                const_: false,
                factory: false,
                node: FunctionRef::Function { unit: unit_id, function: *fid },
                variable: None,
                patched_by: None,
            });
            if ligar(f, pools, unit_id, antigo, novo, funcao_completa(func), n.span) {
                let e = f.library.declared.entry(n.sym).or_default();
                match kind {
                    FunctionKind::Setter => e.setter = Some(Element::Function(novo)),
                    _ => e.getter = Some(Element::Function(novo)),
                }
            }
        }
        DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) => {
            f.erro(unit_id, "augmentation de enum, extension e extension type ainda não é suportada", span);
        }
        DeclKind::Variables(_) => {
            f.erro(unit_id, "augmentation de variável de topo ainda não é suportada", span);
        }
        DeclKind::Typedef(_) => {
            f.erro(unit_id, "um typedef não pode ser aumentado", span);
        }
    }
}

/// A classe (ou mixin) introdutória de mesmo nome, já declarada antes.
fn classe_introdutoria(
    f: &mut Fusao<'_>,
    pools: &ElementPools,
    unit_id: UnitId,
    n: ast::Name,
    kind: ClassKind,
) -> Option<ClassId> {
    let achada = f.library.declared.get(&n.sym).and_then(|b| b.getter);
    match achada {
        Some(Element::Class(cid)) if pools.classes[cid.0 as usize].kind == kind => Some(cid),
        Some(_) => {
            f.erro(unit_id, format!("'augment' de '{}' com outro tipo de declaração", nome(f, n.sym)), n.span);
            None
        }
        None => {
            f.erro(unit_id, format!("'augment' de '{}' sem declaração introdutória antes dele", nome(f, n.sym)), n.span);
            None
        }
    }
}

/// Os membros do corpo de uma `augment class`/`augment mixin`: os comuns são
/// acrescentados; os `augment` se ligam ao membro de mesmo nome.
fn aplicar_membros(f: &mut Fusao<'_>, pools: &mut ElementPools, cid: ClassId, unit_id: UnitId, membros: &[ast::MemberId]) {
    let ast = &f.units[unit_id.0 as usize].ast;
    // Na ordem do texto: um membro novo pode ser aumentado logo depois, no
    // mesmo corpo (é o que a saída das macros faz: `external C.x(...)` e
    // `augment C.x(...)` na mesma `augment class`).
    for &mid in membros {
        let m = ast.member(mid);
        if !m.augment {
            if let Some((sym, span)) = nome_do_membro(ast, m) {
                let c = &pools.classes[cid.0 as usize];
                let existe = match &m.kind {
                    MemberKind::Constructor(k) => {
                        let s = k.name.map(|n| n.sym).unwrap_or(f.empty_sym);
                        c.constructors
                            .get(&s)
                            .is_some_and(|id| pools.functions[id.0 as usize].kind != FunctionKind::SyntheticConstructor)
                    }
                    _ => c.instance_members.contains_key(&sym) || c.static_members.contains_key(&sym),
                };
                if existe {
                    let n = nome(f, sym);
                    f.erro(unit_id, format!("'{n}' já está declarado; para aumentá-lo, use 'augment'"), span);
                    continue;
                }
            }
            acrescentar_membro(f, pools, cid, unit_id, mid);
            continue;
        }
        match &m.kind {
            MemberKind::Method(fid) => {
                let func = ast.function(*fid);
                let Some(n) = func.name else { continue };
                let kind = tipo_de_funcao(func.kind);
                let mut sym = n.sym;
                if kind == FunctionKind::Operator
                    && f.interner.resolve(sym) == "-"
                    && func.parameters.as_ref().is_some_and(|p| p.is_empty())
                {
                    sym = f.interner.intern("unary-");
                }
                let chave = if kind == FunctionKind::Setter {
                    let s = format!("{}_=", f.interner.resolve(sym));
                    f.interner.intern(&s)
                } else {
                    sym
                };
                let c = &pools.classes[cid.0 as usize];
                let mapa = if func.static_ { &c.static_members } else { &c.instance_members };
                let Some(&antigo) = mapa.get(&chave) else {
                    let t = nome(f, n.sym);
                    f.erro(unit_id, format!("'augment' de '{t}' sem membro introdutório antes dele"), n.span);
                    continue;
                };
                let novo = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: sym,
                    library: f.lib_id,
                    class: Some(cid),
                    extension: None,
                    kind,
                    static_: func.static_,
                    abstract_: matches!(func.body, FunctionBody::Empty) && !func.external,
                    external: func.external,
                    const_: false,
                    factory: false,
                    node: FunctionRef::Function { unit: unit_id, function: *fid },
                    variable: None,
                    patched_by: None,
                });
                if ligar(f, pools, unit_id, antigo, novo, funcao_completa(func), n.span) {
                    let c = &mut pools.classes[cid.0 as usize];
                    let mapa = if func.static_ { &mut c.static_members } else { &mut c.instance_members };
                    mapa.insert(chave, novo);
                }
            }
            MemberKind::Constructor(k) => {
                let sym = k.name.map(|n| n.sym).unwrap_or(f.empty_sym);
                let span = k.name.map(|n| n.span).unwrap_or(k.class_name.span);
                let Some(&antigo) = pools.classes[cid.0 as usize].constructors.get(&sym) else {
                    f.erro(unit_id, "'augment' de construtor sem construtor introdutório antes dele", span);
                    continue;
                };
                let novo = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: sym,
                    library: f.lib_id,
                    class: Some(cid),
                    extension: None,
                    kind: FunctionKind::Constructor,
                    static_: true,
                    abstract_: false,
                    external: k.external,
                    const_: k.const_,
                    factory: k.factory,
                    node: FunctionRef::Constructor { unit: unit_id, member: mid },
                    variable: None,
                    patched_by: None,
                });
                if ligar(f, pools, unit_id, antigo, novo, construtor_completo(k), span) {
                    pools.classes[cid.0 as usize].constructors.insert(sym, novo);
                }
            }
            MemberKind::Field(_) => {
                f.erro(unit_id, "augmentation de campo ainda não é suportada", m.span);
            }
        }
    }
}

/// Um membro novo: os mesmos elementos de uma declaração comum. Um
/// construtor declarado na augmentation tira o construtor sintético (a
/// classe passa a declarar construtor).
fn acrescentar_membro(f: &mut Fusao<'_>, pools: &mut ElementPools, cid: ClassId, unit_id: UnitId, mid: ast::MemberId) {
    let ast = &f.units[unit_id.0 as usize].ast;
    let mut classe = std::mem::replace(&mut pools.classes[cid.0 as usize], classe_vazia(f.lib_id, f.empty_sym));
    if matches!(ast.member(mid).kind, MemberKind::Constructor(_)) {
        classe
            .constructors
            .retain(|_, id| pools.functions[id.0 as usize].kind != FunctionKind::SyntheticConstructor);
    }
    extract_members(pools, ast, &mut classe, cid, unit_id, &[mid], f.empty_sym, f.interner);
    pools.classes[cid.0 as usize] = classe;
}

/// Liga `novo` à cadeia cujo elemento efetivo é `antigo`. Devolve `true` se
/// `novo` passou a ser o efetivo (quem chama troca a entrada do mapa).
fn ligar(
    f: &mut Fusao<'_>,
    pools: &mut ElementPools,
    unit_id: UnitId,
    antigo: FunctionElementId,
    novo: FunctionElementId,
    novo_completo: bool,
    span: Span,
) -> bool {
    if !novo_completo {
        // Só metadata (ou nada): o corpo continua o do efetivo.
        pools.functions[novo.0 as usize].patched_by = Some(antigo);
        return false;
    }
    if tem_corpo(f.units, &pools.functions[antigo.0 as usize]) {
        f.erro(unit_id, "a declaração aumentada já tem corpo; uma augmentation não pode substituí-lo", span);
        pools.functions[novo.0 as usize].patched_by = Some(antigo);
        return false;
    }
    for e in pools.functions.iter_mut() {
        if e.patched_by == Some(antigo) {
            e.patched_by = Some(novo);
        }
    }
    pools.functions[antigo.0 as usize].patched_by = Some(novo);
    true
}

/// A declaração tem corpo em Dart (não `external`, não `;`)?
fn tem_corpo(units: &[Unit], e: &FunctionElement) -> bool {
    match e.node {
        FunctionRef::Function { unit, function } => {
            let func = units[unit.0 as usize].ast.function(function);
            !func.external && !matches!(func.body, FunctionBody::Empty)
        }
        FunctionRef::Constructor { unit, member } => {
            let MemberKind::Constructor(k) = &units[unit.0 as usize].ast.member(member).kind else { return false };
            !k.external && construtor_completo(k)
        }
        // Acessor implícito de campo não abstrato: tem o corpo sintético.
        FunctionRef::None => !e.abstract_ && !e.external,
    }
}

/// "Complete" da spec (`:547-560`), com `external` contando como completa.
fn funcao_completa(func: &ast::Function) -> bool {
    func.external || !matches!(func.body, FunctionBody::Empty)
}

fn construtor_completo(k: &ast::Constructor) -> bool {
    k.external
        || !matches!(k.body, FunctionBody::Empty)
        || !k.initializers.is_empty()
        || k.redirect.is_some()
        || k.parameters.iter().any(|p| p.this_ || p.super_)
}

fn tipo_de_funcao(k: ast::FunctionKind) -> FunctionKind {
    match k {
        ast::FunctionKind::Getter => FunctionKind::Getter,
        ast::FunctionKind::Setter => FunctionKind::Setter,
        ast::FunctionKind::Operator => FunctionKind::Operator,
        _ => FunctionKind::Function,
    }
}

fn nome_do_membro(ast: &ast::Ast, m: &ast::Member) -> Option<(SymbolId, Span)> {
    match &m.kind {
        MemberKind::Method(fid) => ast.function(*fid).name.map(|n| (n.sym, n.span)),
        MemberKind::Constructor(k) => Some(k.name.map(|n| (n.sym, n.span)).unwrap_or((k.class_name.sym, k.class_name.span))),
        MemberKind::Field(v) => v.variables.first().map(|x| (x.name.sym, x.name.span)),
    }
}

fn nome(f: &Fusao<'_>, s: SymbolId) -> String {
    f.interner.resolve(s).to_string()
}

/// Marcador que ocupa o lugar de uma classe enquanto os membros novos são
/// extraídos para ela (a extração trabalha sobre um `ClassElement` solto).
fn classe_vazia(library: LibraryId, name: SymbolId) -> ClassElement {
    ClassElement {
        name,
        library,
        decl: None,
        kind: ClassKind::Class,
        modifiers: ast::ClassModifiers::default(),
        type_params: Vec::new(),
        supertype: None,
        mixins: Vec::new(),
        interfaces: Vec::new(),
        on: Vec::new(),
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        static_members: HashMap::new(),
        constructors: BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    }
}
