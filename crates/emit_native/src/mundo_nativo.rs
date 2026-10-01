//! Poda do código do programa pelo mundo fechado, antes do lowering
//! (docs/NATIVO-PROJETOS-REAIS.md, causa C7).
//!
//! O módulo do programa baixava TODA função de toda biblioteca que o
//! programa carrega: no new_sali/backend, 311 mil funções e 2,8 GB de LLVM IR
//! (as tabelas Unicode do `unorm_dart`, os 180 MB das ligações FFI geradas do
//! `openssl_bindings`, as gramáticas do `highlight`…), quase tudo
//! inalcançável a partir do `main`. O Dart AOT só compila o que a análise de
//! fluxo de tipos retém (`VM/pkg/vm/lib/transformations/type_flow/`,
//! `VM/runtime/vm/compiler/aot/precompiler.cc` `DropFunctions`).
//!
//! Aqui a alcançabilidade é a do `crates/mundo` — o RTA do JS de produção:
//! classes instanciadas × seletores invocados, com o SDK como fronteira
//! (os membros que implementam um supertipo do SDK, os de `Object` e `call`
//! vivem em toda classe instanciada). O que o SDK compilado (fora do mundo)
//! pode chamar num objeto do programa entra como seletor externo: TODO nome
//! de membro escrito nas bibliotecas do SDK (com a sobreposição
//! `sdk_nativo/`) — `x.nome`, `nome` solto (o `this.nome()` implícito de uma
//! classe do SDK estendida pelo programa), símbolos `#nome` e campos de
//! padrão de objeto. É um superconjunto sintático: o SDK só chama um membro
//! escrevendo o nome dele (sem espelhos), ou pelo `noSuchMethod`/`call`, que
//! vivem sempre. Mais os nomes que o próprio lowering chama por seletor
//! (`iterator`, `moveNext`, `current`, `[]=`…).
//!
//! A função de instância, de topo ou estática que o mundo não alcança vira um
//! corpo que lança (`podada`); a variável global, um getter que lança. Os
//! símbolos continuam definidos (as tabelas de métodos, as entradas
//! uniformes e o registro da classe os citam), mas o corpo — e o que ele
//! carregaria consigo — some. Se o mundo errar, o erro é alto, com o nome:
//! `UnsupportedError` "código podado como inalcançável foi chamado: <símbolo>".
//! `DARTFORGE_SEM_PODA_DO_PROGRAMA=1` desliga (medida).

use dartforge_elements::model::{Element, FunctionElementId, Program};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::TypeTable;

/// Os nomes que o lowering chama por seletor em valores do programa, além
/// dos que o SDK escreve (`lower/*.rs`: `chamar_por_nome`,
/// `metodo_por_seletor`, `propriedade_por_seletor`).
const NOMES_DO_LOWERING: &[&str] = &[
    "iterator", "moveNext", "current", "[]", "[]=", "==", "add", "addAll", "length", "hashCode",
    "toString", "fillRange", "containsKey", "cancel", "*", "call", "noSuchMethod", "then",
];

/// Calcula o mundo do programa. `None` quando a poda está desligada.
pub fn calcular(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
) -> Option<dartforge_mundo::Mundo> {
    if std::env::var_os("DARTFORGE_SEM_PODA_DO_PROGRAMA").is_some_and(|v| v != "0") {
        return None;
    }
    let mut r = dartforge_mundo::Raizes::default();
    let lib = program.entry?;
    let main = interner.lookup("main")?;
    match program.library(lib).declared.get(&main).and_then(|b| b.getter) {
        Some(Element::Function(f)) => r.funcoes.push(f),
        _ => return None,
    }
    entradas_marcadas(program, interner, &mut r);
    let mut nomes = std::collections::BTreeSet::new();
    for n in NOMES_DO_LOWERING {
        nomes.insert((*n).to_string());
    }
    nomes_escritos_no_sdk(program, interner, &mut nomes);
    r.seletores = nomes.into_iter().collect();
    let e = dartforge_mundo::Entrada { program, interner, table, outline, bodies };
    Some(dartforge_mundo::calcular(e, &r))
}

/// `@pragma('vm:entry-point')` no programa: a VM retém o que a anotação
/// marca (o embutidor chama por nome); aqui vira raiz — a função (de topo
/// ou método) e a classe.
fn entradas_marcadas(program: &Program, interner: &Interner, r: &mut dartforge_mundo::Raizes) {
    use dartforge_elements::model::{ClassId, FunctionRef};
    let marcada = |a: &ast::Ast, meta: &[ast::Annotation]| {
        meta.iter().any(|m| {
            m.name.last().is_some_and(|n| interner.resolve(n.sym) == "pragma")
                && m.arguments.as_ref().and_then(|args| args.args.first()).is_some_and(|arg| {
                    matches!(&a.expr(arg.value).kind, ast::ExprKind::String(s)
                        if s.constant_value().is_some_and(|v| v.to_string_lossy().starts_with("vm:entry-point")))
                })
        })
    };
    let mut por_no = std::collections::HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = f.node
            && !program.library(f.library).is_sdk
        {
            por_no.insert((unit, function), FunctionElementId(i as u32));
        }
    }
    for lib in &program.libraries {
        if lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let a = &program.unit(u).ast;
            for d in &a.decls {
                if let ast::DeclKind::Function(fid) = d.kind
                    && marcada(a, &d.metadata)
                    && let Some(&f) = por_no.get(&(u, fid))
                {
                    r.funcoes.push(f);
                }
            }
            for m in &a.members {
                if let ast::MemberKind::Method(fid) = m.kind
                    && marcada(a, &m.metadata)
                    && let Some(&f) = por_no.get(&(u, fid))
                {
                    r.funcoes.push(f);
                }
            }
        }
    }
    for (i, c) in program.classes.iter().enumerate() {
        if program.library(c.library).is_sdk {
            continue;
        }
        let Some(d) = c.decl else { continue };
        let a = &program.unit(d.unit).ast;
        if marcada(a, &a.decl(d.decl).metadata) {
            r.classes_instanciadas.push(ClassId(i as u32));
        }
    }
}

/// Todo nome de membro escrito nas bibliotecas do SDK.
fn nomes_escritos_no_sdk(program: &Program, interner: &Interner, nomes: &mut std::collections::BTreeSet<String>) {
    for lib in &program.libraries {
        if !lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let a = &program.unit(u).ast;
            for e in &a.exprs {
                match &e.kind {
                    ast::ExprKind::Identifier(n) | ast::ExprKind::Property { name: n, .. } => {
                        nomes.insert(interner.resolve(n.sym).to_string());
                    }
                    ast::ExprKind::Symbol(ns) => {
                        for n in ns.iter() {
                            nomes.insert(interner.resolve(n.sym).to_string());
                        }
                    }
                    _ => {}
                }
            }
            for p in &a.patterns {
                if let ast::PatternKind::Object { fields, .. } = &p.kind {
                    for f in fields.iter() {
                        if let Some(n) = &f.name {
                            nomes.insert(interner.resolve(n.sym).to_string());
                        }
                    }
                }
            }
        }
    }
}
