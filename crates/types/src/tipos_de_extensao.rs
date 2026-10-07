//! As declarações `extension type` (docs/ANALYZER-ESPECIFICACAO.md §A, grupo
//! 4):
//!
//! * os ciclos do link (`summary2/extension_type.dart`): a representação que
//!   depende de si (`extension_type_representation_depends_on_itself`, no
//!   nome de cada tipo da componente) e o `implements` em ciclo
//!   (`extension_type_implements_itself`), pelo `DependencyWalker` (Tarjan,
//!   com o auto-laço);
//! * `_checkForExtensionTypeRepresentationTypeBottom`
//!   (`error_verifier.dart:3503-3513`), no tipo da representação;
//! * `_checkForExtensionTypeMemberConflicts` (`:3448-3485`): um relato por
//!   conflito da interface (candidatos, membro de classe e de tipo de
//!   extensão, membro de tipo de extensão não único), no nome;
//! * `ResolutionVisitor._verifyExtensionElementImplements`
//!   (`resolution_visitor.dart:1786-1838`), em cada tipo do `implements`:
//!   `extension_type_implements_disallowed_type`,
//!   `extension_type_implements_representation_not_supertype` e
//!   `extension_type_implements_not_supertype`.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::heranca::{Conflito, Heranca, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// Os tipos de extensão citados num tipo (`_DependenciesCollector`: os
/// argumentos de tipo, o retorno, os limites dos parâmetros de tipo e os
/// parâmetros de uma função, os campos de um registro; não o limite de um
/// parâmetro de tipo).
fn citados(table: &TypeTable, t: TypeId, saida: &mut Vec<ClassId>, prof: u32) {
    if prof > 64 {
        return;
    }
    match table.get(t).clone() {
        Type::ExtensionType { decl, args, .. } => {
            saida.push(decl);
            for a in args.iter() {
                citados(table, *a, saida, prof + 1);
            }
        }
        Type::Interface { args, .. } => {
            for a in args.iter() {
                citados(table, *a, saida, prof + 1);
            }
        }
        Type::FutureOr { arg, .. } => citados(table, arg, saida, prof + 1),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            citados(table, ret, saida, prof + 1);
            for p in type_params.iter() {
                let b = table.param(*p).bound;
                if table.param(*p).explicito {
                    citados(table, b, saida, prof + 1);
                }
            }
            for x in positional.iter().chain(optional.iter()) {
                citados(table, *x, saida, prof + 1);
            }
            for (_, x, _) in named.iter() {
                citados(table, *x, saida, prof + 1);
            }
        }
        Type::Record { positional, named, .. } => {
            for x in positional.iter() {
                citados(table, *x, saida, prof + 1);
            }
            for (_, x) in named.iter() {
                citados(table, *x, saida, prof + 1);
            }
        }
        _ => {}
    }
}

/// Os nós de componentes fortemente conexas com mais de um nó ou com
/// auto-laço (`DependencyWalker.evaluateScc`).
fn em_ciclo(nos: &[ClassId], deps: &HashMap<ClassId, Vec<ClassId>>) -> HashSet<ClassId> {
    struct Estado<'a> {
        deps: &'a HashMap<ClassId, Vec<ClassId>>,
        indice: HashMap<ClassId, usize>,
        baixo: HashMap<ClassId, usize>,
        pilha: Vec<ClassId>,
        na_pilha: HashSet<ClassId>,
        proximo: usize,
        marcados: HashSet<ClassId>,
    }
    fn conectar(e: &mut Estado<'_>, v: ClassId) {
        e.indice.insert(v, e.proximo);
        e.baixo.insert(v, e.proximo);
        e.proximo += 1;
        e.pilha.push(v);
        e.na_pilha.insert(v);
        let mut auto = false;
        let vizinhos = e.deps.get(&v).cloned().unwrap_or_default();
        for w in vizinhos {
            if !e.deps.contains_key(&w) {
                continue;
            }
            if w == v {
                auto = true;
            } else if !e.indice.contains_key(&w) {
                conectar(e, w);
                let bw = e.baixo[&w];
                if bw < e.baixo[&v] {
                    e.baixo.insert(v, bw);
                }
            } else if e.na_pilha.contains(&w) {
                let iw = e.indice[&w];
                if iw < e.baixo[&v] {
                    e.baixo.insert(v, iw);
                }
            }
        }
        if e.baixo[&v] == e.indice[&v] {
            let mut componente = Vec::new();
            while let Some(x) = e.pilha.pop() {
                e.na_pilha.remove(&x);
                componente.push(x);
                if x == v {
                    break;
                }
            }
            if componente.len() > 1 || auto {
                e.marcados.extend(componente);
            }
        }
    }
    let mut e = Estado { deps, indice: HashMap::new(), baixo: HashMap::new(), pilha: Vec::new(), na_pilha: HashSet::new(), proximo: 1, marcados: HashSet::new() };
    for &v in nos {
        if !e.indice.contains_key(&v) {
            conectar(&mut e, v);
        }
    }
    e.marcados
}

/// `isBottom`: `Never`, ou parâmetro de tipo não anulável cujo limite é
/// fundo.
fn e_fundo(table: &TypeTable, t: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return false;
    }
    match table.get(t) {
        Type::Never => true,
        Type::TypeParameter { param, nullable: false } => e_fundo(table, table.param(*param).bound, prof + 1),
        Type::Intersection { bound, .. } => e_fundo(table, *bound, prof + 1),
        _ => false,
    }
}

/// O tipo escrito é literalmente `dynamic` (e não um nome que não resolveu,
/// que o outline também deixa `dynamic`).
fn escrito_dynamic(interner: &Interner, a: &ast::Ast, t: ast::TypeId) -> bool {
    matches!(&a.ty(t).kind, ast::TypeKind::Named { name, .. } if name.len() == 1 && interner.resolve(name[0].sym) == "dynamic")
}

/// O tipo da representação declarada (o `InvalidType` fica `None`).
fn representacao(program: &Program, outline: &OutlineTypes, table: &TypeTable, interner: &Interner, c: ClassId) -> Option<TypeId> {
    let v = program.class(c).representation?;
    let t = outline.variables[v.0 as usize].declared_type?;
    if table.e_invalido(t) {
        return None;
    }
    if matches!(table.get(t), Type::Dynamic) {
        let d = program.class(c).decl?;
        let a = &program.unit(d.unit).ast;
        let DeclKind::ExtensionType(et) = &a.decl(d.decl).kind else { return None };
        if !escrito_dynamic(interner, a, et.representation_type) {
            return None;
        }
    }
    Some(t)
}

pub fn verificar(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let tipos: Vec<ClassId> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, ce)| ce.kind == ClassKind::ExtensionType && ce.decl.is_some_and(|d| !program.unit(d.unit).ast.decl(d.decl).augment))
        .map(|(i, _)| ClassId(i as u32))
        .collect();
    if !tipos.iter().any(|&t| program.class(t).library == lib) {
        return saida;
    }
    // O grafo da representação e o do `implements` (só os tipos de extensão
    // não anuláveis: os outros saem do `interfaces` do elemento).
    let mut deps_rep: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
    let mut deps_impl: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
    for &t in &tipos {
        let mut v = Vec::new();
        if let Some(rep) = program.class(t).representation
            && let Some(r) = outline.variables[rep.0 as usize].declared_type
        {
            citados(table, r, &mut v, 0);
        }
        deps_rep.insert(t, v);
        let interfaces = outline.classes[t.0 as usize].interfaces.clone();
        let mut w = Vec::new();
        for i in interfaces.iter() {
            if let Type::ExtensionType { decl, nullable: false, .. } = table.get(*i) {
                w.push(*decl);
            }
        }
        deps_impl.insert(t, w);
    }
    let rep_em_ciclo = em_ciclo(&tipos, &deps_rep);
    let impl_em_ciclo = em_ciclo(&tipos, &deps_impl);
    let vazio: [&str; 0] = [];
    let mut heranca = Heranca::default();
    for &t in tipos.iter().filter(|&&t| program.class(t).library == lib) {
        let ce = program.class(t);
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let DeclKind::ExtensionType(et) = &a.decl(d.decl).kind else { continue };
        let u = d.unit;
        let nome = interner.resolve(et.name.sym).to_string();
        let rep = if rep_em_ciclo.contains(&t) { None } else { representacao(program, outline, table, interner, t) };

        // `ResolutionVisitor` (antes do `ErrorVerifier`): o `implements`.
        if let Some(r) = rep {
            let interfaces = outline.classes[t.0 as usize].interfaces.clone();
            for (k, &escrito) in et.implements.iter().enumerate() {
                let Some(&tipo) = interfaces.get(k) else { continue };
                // `_verifyNullability` (`named_type_resolver.dart:388-414`): o
                // tipo de hierarquia perde o `?` depois do
                // `NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE`. O `hasErrorReported`
                // só vale para o alias que expande a parâmetro de tipo: o nome
                // indefinido é `InvalidType`, e `isValidExtensionTypeSuperinterface`
                // (`type_system.dart:1361-1378`) o recusa.
                let invalido = table.e_invalido(tipo);
                let tipo = if invalido { tipo } else { crate::ops::non_nullable(tipo, table) };
                let sp = a.ty(escrito).span;
                let valido = !invalido
                    && match table.get(tipo).clone() {
                        Type::Interface { class, nullable: false, .. } => {
                            Some(class) != core.function_class && Some(class) != core.null_class && Some(class) != core.record_class
                        }
                        Type::ExtensionType { nullable: false, .. } => true,
                        _ => false,
                    };
                if !valido {
                    // O tipo como o analyzer o escreve: sem o alias (`void`,
                    // `dynamic`), e `InvalidType` para o que não resolveu.
                    let texto = if invalido {
                        "InvalidType".to_string()
                    } else {
                        let s = crate::ops::sem_exibicao(tipo, table);
                        table.format(s, interner, program)
                    };
                    saida.push((u, Diagnostic::com_codigo(c::EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE, sp, [texto.as_str()])));
                    continue;
                }
                let cabe = {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(r, tipo, &mut env)
                };
                if cabe {
                    continue;
                }
                if let Type::ExtensionType { decl, args, .. } = table.get(tipo).clone() {
                    let Some(rep_impl) = representacao(program, outline, table, interner, decl) else { continue };
                    let params = outline.classes[decl.0 as usize].type_params.clone();
                    let rep_impl = if params.len() == args.len() {
                        let mapa: HashMap<_, _> = params.iter().copied().zip(args.iter().copied()).collect();
                        crate::ops::substitute(rep_impl, &mapa, table)
                    } else {
                        rep_impl
                    };
                    let cabe_na_rep = {
                        let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                        is_subtype(r, rep_impl, &mut env)
                    };
                    if !cabe_na_rep {
                        let ri = table.format(rep_impl, interner, program);
                        let ni = interner.resolve(program.class(decl).name).to_string();
                        let rd = table.format(r, interner, program);
                        saida.push((
                            u,
                            Diagnostic::com_codigo(
                                c::EXTENSION_TYPE_IMPLEMENTS_REPRESENTATION_NOT_SUPERTYPE,
                                sp,
                                [ri.as_str(), ni.as_str(), rd.as_str(), nome.as_str()],
                            ),
                        ));
                    }
                    continue;
                }
                let ti = table.format(tipo, interner, program);
                let rd = table.format(r, interner, program);
                saida.push((u, Diagnostic::com_codigo(c::EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE, sp, [ti.as_str(), rd.as_str()])));
            }
        }

        // `ErrorVerifier.visitExtensionTypeDeclaration`, na ordem.
        if rep_em_ciclo.contains(&t) {
            saida.push((u, Diagnostic::com_codigo(c::EXTENSION_TYPE_REPRESENTATION_DEPENDS_ON_ITSELF, et.name.span, vazio.iter().copied())));
        } else if let Some(r) = rep
            && e_fundo(table, r, 0)
        {
            let sp = a.ty(et.representation_type).span;
            saida.push((u, Diagnostic::com_codigo(c::EXTENSION_TYPE_REPRESENTATION_TYPE_BOTTOM, sp, vazio.iter().copied())));
        }
        if impl_em_ciclo.contains(&t) {
            saida.push((u, Diagnostic::com_codigo(c::EXTENSION_TYPE_IMPLEMENTS_ITSELF, et.name.span, vazio.iter().copied())));
        }
        let interface = {
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            heranca.interface(&mut p, t)
        };
        for conflito in interface.conflitos.iter() {
            // `_checkForExtensionTypeMemberConflicts`: os candidatos (os sem
            // extensão antes dos de extensão) viram as mensagens de contexto.
            let (n, candidatos): (_, Vec<&crate::heranca::Membro>) = match conflito {
                Conflito::Candidatos { nome, candidatos } | Conflito::ExtensaoNaoUnica { nome, candidatos } => (nome, candidatos.iter().collect()),
                Conflito::ExtensaoENaoExtensao { nome, nao_extensao, extensao } => (nome, nao_extensao.iter().chain(extensao.iter()).collect()),
                Conflito::GetterMetodo { .. } => continue,
            };
            // O `Name.name` do setter é `x=`; a chave do modelo, `x_=`.
            let texto = interner.resolve(n.chave);
            let membro = texto.strip_suffix("_=").map_or_else(|| texto.to_string(), |s| format!("{s}="));
            let mut d = Diagnostic::com_codigo(c::EXTENSION_TYPE_INHERITED_MEMBER_CONFLICT, et.name.span, [nome.as_str(), membro.as_str()]);
            for m in candidatos {
                let Some((mu, span)) = program.nome_nao_sintetico_da_funcao(m.funcao) else { continue };
                let arquivo = (mu != u).then(|| program.caminho_da_unidade(mu).into());
                let mensagem = format!("Inherited from '{}'", interner.resolve(program.class(m.classe).name));
                d.contexto.push(dartforge_diagnostics::Contexto { arquivo, span, mensagem: mensagem.into() });
            }
            saida.push((u, d));
        }
    }
    saida
}
