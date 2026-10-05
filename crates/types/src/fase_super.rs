//! O `MustCallSuperVerifier` do analyzer
//! (`analyzer/lib/src/error/must_call_super_verifier.dart`;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.8): `must_call_super`, no
//! nome de um método, getter ou setter de instância não abstrato que
//! sobrescreve um membro anotado com `@mustCallSuper`, tem implementação
//! concreta herdada e cujo corpo não chama `super.<o mesmo membro>`.
//!
//! A anotação é reconhecida pelo nome (`mustCallSuper`, com ou sem prefixo
//! de import), sem conferir que vem do `package:meta`. A chamada ao `super`
//! é procurada na árvore do corpo, funções aninhadas incluídas.
//! Escrito sem compilar nem executar (2026-10-04).

use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, FunctionElementId, FunctionKind as EspecieDeElemento, FunctionRef, LibraryId, Program, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, AssignOp, DeclKind, ExprKind, FunctionKind, MemberKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashSet, VecDeque};

/// A lista de anotações tem `@mustCallSuper` (ou `@prefixo.mustCallSuper`).
fn anotado(metadata: &[ast::Annotation], sym: SymbolId) -> bool {
    metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| n.sym == sym))
}

/// As anotações da declaração de um membro: as do método, ou as do campo de
/// um acessor implícito.
fn anotacoes_de(program: &Program, f: FunctionElementId) -> &[ast::Annotation] {
    let elemento = program.function(f);
    if let Some(v) = elemento.variable {
        return match program.variable(v).node {
            VariableRef::Field { unit, member, .. } => &program.unit(unit).ast.member(member).metadata,
            _ => &[],
        };
    }
    let (FunctionRef::Function { unit, function }, Some(classe)) = (elemento.node, elemento.class) else { return &[] };
    let Some(decl) = program.class(classe).decl else { return &[] };
    let a = &program.unit(unit).ast;
    if decl.unit != unit {
        return &[];
    }
    let membros: &[ast::MemberId] = match &a.decl(decl.decl).kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        _ => return &[],
    };
    membros
        .iter()
        .map(|m| a.member(*m))
        .find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function))
        .map_or(&[][..], |m| &m.metadata[..])
}

/// `_findOverriddenMemberWithMustCallSuper`: em largura pelos mixins, a
/// superclasse e (num mixin) as restrições `on`, sem as interfaces; o
/// primeiro ancestral com um membro de nome `nome` decide.
fn anotado_acima(program: &Program, interner: &Interner, classe: ClassId, lib: LibraryId, chaves: &[SymbolId], sym: SymbolId) -> Option<ClassId> {
    let privado = chaves.first().is_some_and(|c| interner.resolve(*c).starts_with('_'));
    let mut fila: VecDeque<ClassId> = VecDeque::new();
    let enfileirar = |fila: &mut VecDeque<ClassId>, c: ClassId| {
        let e = program.class(c);
        fila.extend(e.mixin_classes.iter().copied());
        fila.extend(e.supertype_class);
        if e.kind == ClassKind::Mixin {
            fila.extend(e.on_classes.iter().copied());
        }
    };
    let mut vistos: HashSet<ClassId> = HashSet::new();
    enfileirar(&mut fila, classe);
    while let Some(ancestral) = fila.pop_front() {
        if !vistos.insert(ancestral) {
            continue;
        }
        let e = program.class(ancestral);
        if !privado || e.library == lib {
            // `getMethod(name) ?? getGetter(name) ?? getSetter(name)`.
            let membro = chaves.iter().find_map(|c| e.instance_members.get(c).copied());
            if let Some(m) = membro
                && anotado(anotacoes_de(program, m), sym)
            {
                return Some(ancestral);
            }
        }
        enfileirar(&mut fila, ancestral);
    }
    None
}

/// `lookUpConcrete…` a partir de `inicio`: a própria classe (com
/// `com_a_propria`), os mixins dela do último para o primeiro, e assim pela
/// cadeia de superclasses; o primeiro membro não abstrato da espécie pedida.
pub(crate) fn concreto(program: &Program, inicio: ClassId, com_a_propria: bool, chave: SymbolId, lib: LibraryId, privado: bool, metodo: bool) -> bool {
    let serve = |c: ClassId| {
        let e = program.class(c);
        if privado && e.library != lib {
            return false;
        }
        e.instance_members.get(&chave).is_some_and(|f| {
            let m = program.function(*f);
            let e_metodo = matches!(m.kind, EspecieDeElemento::Function | EspecieDeElemento::Operator);
            !m.abstract_ && e_metodo == metodo
        })
    };
    let mut vistos: HashSet<ClassId> = HashSet::new();
    let mut atual = Some(inicio);
    let mut primeira = true;
    while let Some(c) = atual {
        if !vistos.insert(c) {
            break;
        }
        if (!primeira || com_a_propria) && serve(c) {
            return true;
        }
        primeira = false;
        let e = program.class(c);
        if e.mixin_classes.iter().rev().any(|m| serve(*m)) {
            return true;
        }
        atual = e.supertype_class;
    }
    false
}

/// `invokesSuperSelf`: o corpo de `funcao` tem `super.<nome>` no papel
/// certo (leitura ou chamada para método e getter, escrita para setter), ou
/// o operador aplicado a `super`.
fn chama_o_super(a: &ast::Ast, fonte: &str, interner: &Interner, funcao: &ast::Function, nome: &str, especie: FunctionKind) -> bool {
    let dentro = |s: Span| funcao.span.start <= s.start && s.end <= funcao.span.end;
    let e_super = |e: ast::ExprId| matches!(a.expr(e).kind, ExprKind::Super);
    // Os alvos de atribuição simples (`=`): não são leitura.
    let mut so_escritos: HashSet<(usize, usize)> = HashSet::new();
    let mut escritos: Vec<ast::ExprId> = Vec::new();
    for e in a.exprs.iter().filter(|e| dentro(e.span)) {
        if let ExprKind::Assign { op, target, .. } = &e.kind {
            escritos.push(*target);
            if matches!(op, AssignOp::Assign) {
                let s = a.expr(*target).span;
                so_escritos.insert((s.start, s.end));
            }
        }
    }
    match especie {
        FunctionKind::Setter => escritos.iter().any(|t| match &a.expr(*t).kind {
            ExprKind::Property { target, name, .. } => e_super(*target) && interner.resolve(name.sym) == nome,
            _ => false,
        }),
        FunctionKind::Function | FunctionKind::Getter => a.exprs.iter().filter(|e| dentro(e.span)).any(|e| match &e.kind {
            ExprKind::Property { target, name, .. } => {
                e_super(*target) && interner.resolve(name.sym) == nome && !so_escritos.contains(&(e.span.start, e.span.end))
            }
            _ => false,
        }),
        FunctionKind::Operator => {
            if nome == "[]=" {
                return escritos.iter().any(|t| matches!(&a.expr(*t).kind, ExprKind::Index { target, .. } if e_super(*target)));
            }
            a.exprs.iter().filter(|e| dentro(e.span)).any(|e| match &e.kind {
                ExprKind::Index { target, .. } => nome == "[]" && e_super(*target) && !so_escritos.contains(&(e.span.start, e.span.end)),
                ExprKind::Unary { op: UnaryOp::Neg, operand } => nome == "unary-" && e_super(*operand),
                ExprKind::Unary { op: UnaryOp::BitNot, operand } => nome == "~" && e_super(*operand),
                ExprKind::Binary { left, right, .. } if e_super(*left) => {
                    // O texto do operador, entre os operandos; `!=` chama `==`.
                    let (de, ate) = (a.expr(*left).span.end, a.expr(*right).span.start);
                    let texto = fonte.get(de..ate).map_or("", str::trim);
                    texto == nome || (texto == "!=" && nome == "==")
                }
                _ => false,
            })
        }
    }
}

/// Os `must_call_super` da biblioteca `lib`.
pub fn sem_chamada_ao_super(program: &Program, interner: &Interner, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let Some(sym) = interner.lookup("mustCallSuper") else { return saida };
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || !matches!(classe.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let cid = ClassId(i as u32);
        let unidade = program.unit(decl.unit);
        let a = &unidade.ast;
        let membros: &[ast::MemberId] = match &a.decl(decl.decl).kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            _ => continue,
        };
        for &mid in membros {
            let MemberKind::Method(f) = &a.member(mid).kind else { continue };
            let func = a.function(*f);
            // Estático ou abstrato (sem corpo e sem `external`): fora.
            if func.static_ || (matches!(func.body, ast::FunctionBody::Empty) && !func.external) {
                continue;
            }
            let Some(nome) = func.name else { continue };
            let texto = interner.resolve(nome.sym);
            let privado = texto.starts_with('_');
            let chave_do_setter = interner.lookup(&format!("{texto}_="));
            let chaves: Vec<SymbolId> = match func.kind {
                FunctionKind::Setter => chave_do_setter.into_iter().collect(),
                _ => std::iter::once(nome.sym).chain(chave_do_setter).collect(),
            };
            let Some(dona) = anotado_acima(program, interner, cid, lib, &chaves, sym) else { continue };
            // A implementação concreta herdada.
            let tem_concreto = match func.kind {
                // `_hasConcreteSuperMethod`: a superclasse, os mixins ou as
                // restrições, cada um com a sua cadeia.
                FunctionKind::Function | FunctionKind::Operator => classe
                    .supertype_class
                    .iter()
                    .chain(classe.mixin_classes.iter())
                    .chain(classe.on_classes.iter().filter(|_| classe.kind == ClassKind::Mixin))
                    .any(|c| concreto(program, *c, true, nome.sym, lib, privado, true)),
                // `lookUpInheritedConcreteGetter` / `…Setter`.
                FunctionKind::Getter => {
                    concreto(program, cid, false, nome.sym, lib, privado, false)
                        || (classe.kind == ClassKind::Mixin
                            && classe.on_classes.iter().any(|c| concreto(program, *c, true, nome.sym, lib, privado, false)))
                }
                FunctionKind::Setter => chave_do_setter.is_some_and(|k| {
                    concreto(program, cid, false, k, lib, privado, false)
                        || (classe.kind == ClassKind::Mixin
                            && classe.on_classes.iter().any(|c| concreto(program, *c, true, k, lib, privado, false)))
                }),
            };
            if !tem_concreto {
                continue;
            }
            if !chama_o_super(a, &unidade.source, interner, func, texto, func.kind) {
                let nome_da_dona = interner.resolve(program.class(dona).name);
                saida.push((decl.unit, Diagnostic::com_codigo(w::MUST_CALL_SUPER, nome.span, [nome_da_dona])));
            }
        }
    }
    saida
}
