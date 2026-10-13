//! Bibliotecas do SDK compiladas **da fonte** junto com o programa (P6,
//! docs/NATIVO-PLANO.md §7.7).
//!
//! A decisão 1 da rodada 2 (NATIVO-PLANO §7.1) é que o SDK do nativo vem da
//! fonte da seção `vm` com a sobreposição `native_sdk/`. O `dart:async` é a
//! primeira biblioteca a entrar assim, porque o `async`/`await` precisa dele
//! inteiro (`_Future`, `Zone`, microtarefas, `Timer`) e nenhuma classe dele
//! tem representação própria no runtime em Rust: todas são objetos comuns do
//! heap, como as classes do programa.
//!
//! **Como entra.** Uma biblioteca da fonte é tratada pelo lowering como uma
//! biblioteca do programa: as classes ganham id e layout, as funções viram
//! símbolos estáveis (`df.dart$3aasync.…`), os corpos são inferidos. O que
//! faz isso acontecer é desligar `is_sdk` dessa biblioteca na **cópia do
//! `Program` deste compilador**, antes da inferência — o único efeito de
//! `is_sdk` no `crates/types` é pular a inferência dos corpos, e é
//! exatamente o que precisa deixar de acontecer (o pedido de δ ao dono da
//! inferência, docs/NATIVO-PEDIDOS.md, pede a mesma coisa por parâmetro;
//! quando ele existir, a troca é uma linha aqui).
//!
//! **Custo zero para quem não usa** (PLANO.md, regra governante): só o
//! programa que usa `dart:async` (função `async`/gerador, `await`, o nome
//! `Future`/`Stream`/`FutureOr` ou `import 'dart:async'`) a compila; os
//! demais emitem o mesmo IR de antes.
//!
//! **Poda.** A biblioteca inteira é baixada (é o mundo aberto de uma
//! biblioteca do SDK, como no módulo por biblioteca de P5c), e depois o
//! módulo é podado a partir do programa: só fica a função da fonte que o
//! programa alcança (chamada, closure, tear-off, `toString` de classe). O
//! diagnóstico de construto não suportado numa função da fonte só vale se
//! ela ficar — o que o programa não alcança não pode impedi-lo de compilar.

use crate::hir::{Instruction, Module};
use dartforge_elements::model::{LibraryId, Program};
use dartforge_frontend::ast::{AsyncModifier, ExprKind, TypeKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// As bibliotecas do SDK que o programa compila da fonte.
pub fn source_libraries(program: &Program, interner: &Interner) -> Vec<LibraryId> {
    let achar = |uri: &str| program.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32));
    let Some(async_lib) = achar("dart:async") else {
        return Vec::new();
    };
    if !uses_dart_async(program, interner, async_lib) {
        return Vec::new();
    }
    // O `dart:_internal` vem junto: o `dart:async` usa dele `unsafeCast`,
    // `IterableElementError`, `checkNotNullable`… (código Dart puro, fora os
    // intrínsecos, `lower/externals.rs`).
    let mut v = vec![async_lib];
    v.extend(achar("dart:_internal"));
    v
}

/// Os `part` do `dart:core` que o `dart:async` da fonte precisa e que o
/// runtime não representa: código Dart puro, sem `external`. O `Timer` recebe
/// uma `Duration` (`Future()`, `Future.delayed`, `Timer.run` usam
/// `Duration.zero`); o runtime em Rust nunca teve `Duration`.
const CORE_PARTS: &[&str] = &["core/duration.dart"];

/// Tira do `dart:core` as unidades de `CORE_PARTS` para uma biblioteca
/// nova, de mesmo URI e mesmo escopo, que não é do SDK — as classes delas
/// passam a ser compiladas da fonte como as do `dart:async`, e o resto do
/// `dart:core` continua sendo o do runtime. Os elementos não mudam de id
/// (quem os referencia continua referenciando); só a biblioteca dona muda.
/// Devolve a biblioteca nova.
pub fn split_core_parts(program: &mut Program) -> Option<LibraryId> {
    use dartforge_elements::model::{FunctionRef, Library, VariableRef};
    let core = program.core?;
    let units: Vec<dartforge_elements::model::UnitId> = program.libraries[core.0 as usize]
        .units
        .iter()
        .copied()
        .filter(|u| {
            program.unit(*u).path.as_ref().is_some_and(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                CORE_PARTS.iter().any(|suf| s.ends_with(suf))
            })
        })
        .collect();
    if units.is_empty() {
        return None;
    }
    let origin = &program.libraries[core.0 as usize];
    let new_one = Library {
        uri: origin.uri.clone(),
        name: origin.name.clone(),
        units: units.clone(),
        imports: origin.imports.clone(),
        exports: Vec::new(),
        declared: origin.declared.clone(),
        exported: origin.exported.clone(),
        scope: origin.scope.clone(),
        prefixes: origin.prefixes.clone(),
        is_sdk: false,
        // A versão de linguagem e os recursos por biblioteca (Dart moderno, P2).
        features: origin.features,
        // O escopo de cada unidade (A04) e a parte que a incluiu seguem as
        // unidades que vêm para a biblioteca nova.
        pais: origin.pais.iter().filter(|(u, _)| units.contains(u)).map(|(u, p)| (*u, *p)).collect(),
        representante: Default::default(),
        dono_do_grupo: Default::default(),
        escopos_de_unidade: origin
            .escopos_de_unidade
            .iter()
            .filter(|(u, _)| units.contains(u))
            .map(|(u, e)| (*u, e.clone()))
            .collect(),
    };
    let id = LibraryId(program.libraries.len() as u32);
    program.libraries.push(new_one);
    program.libraries[core.0 as usize].units.retain(|u| !units.contains(u));
    for u in &units {
        program.units[u.0 as usize].library = id;
    }
    for c in &mut program.classes {
        if c.decl.is_some_and(|d| units.contains(&d.unit)) {
            c.library = id;
        }
    }
    for f in &mut program.functions {
        let unit = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => None,
        };
        // Construtor sintético: a unidade é a da classe.
        let unit = unit.or_else(|| f.class.and_then(|c| program.classes[c.0 as usize].decl.map(|d| d.unit)));
        if unit.is_some_and(|u| units.contains(&u)) {
            f.library = id;
        }
    }
    for v in &mut program.variables {
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } => {
                Some(unit)
            }
            _ => None,
        };
        if unit.is_some_and(|u| units.contains(&u)) {
            v.library = id;
        }
    }
    Some(id)
}

/// O programa (as bibliotecas que não são do SDK) usa `dart:async`?
fn uses_dart_async(program: &Program, interner: &Interner, async_lib: LibraryId) -> bool {
    let names: HashSet<&str> = ["Future", "Stream", "FutureOr"].into_iter().collect();
    // `dart:isolate` e `dart:io` entregam resultados pela fila do isolado
    // (portas, IOService): quem os importa precisa do laço de eventos.
    let with_loop: Vec<LibraryId> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| l.uri == "dart:isolate" || l.uri == "dart:io")
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    for lib in program.libraries.iter().filter(|l| !l.is_sdk) {
        if lib.imports.iter().any(|i| i.library == async_lib || with_loop.contains(&i.library)) {
            return true;
        }
        for &u in &lib.units {
            let ast = &program.unit(u).ast;
            if ast.functions.iter().any(|f| f.modifier != AsyncModifier::None) {
                return true;
            }
            for e in &ast.exprs {
                match &e.kind {
                    ExprKind::Await(_) => return true,
                    ExprKind::Identifier(n) if names.contains(interner.resolve(n.sym)) => return true,
                    _ => {}
                }
            }
            for t in &ast.types {
                if let TypeKind::Named { name, .. } = &t.kind
                    && name.last().is_some_and(|n| names.contains(interner.resolve(n.sym)))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// A função `fid` tem `@pragma("<text>")` no `Member`/`Decl` que a declara.
pub fn has_pragma(program: &Program, interner: &Interner, fid: usize, text: &str) -> bool {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    let f = &program.functions[fid];
    let FunctionRef::Function { unit, function } = f.node else { return false };
    let a = &program.unit(unit).ast;
    let metadata: &[ast::Annotation] = a
        .members
        .iter()
        .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
        .map(|m| &*m.metadata)
        .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function)).map(|d| &*d.metadata))
        .unwrap_or(&[]);
    metadata.iter().any(|an| pragma_with_string(interner, a, an, text))
}

/// A anotação é `@pragma("<text>", …)`.
pub fn pragma_with_string(interner: &Interner, ast: &dartforge_frontend::ast::Ast, an: &dartforge_frontend::ast::Annotation, text: &str) -> bool {
    if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
        return false;
    }
    let Some(args) = &an.arguments else { return false };
    args.args.first().is_some_and(|a| match &ast.exprs[a.value.0 as usize].kind {
        dartforge_frontend::ast::ExprKind::String(lit) => lit.constant_value().is_some_and(|t| t.as_bytes() == text.as_bytes()),
        _ => false,
    })
}

/// Os pragmas `vm:external-name`/`vm:recognized` de uma função: o nome do
/// native, se há um, e se ela é intrínseco. O pragma mora no `Member`/`Decl`
/// que declara a função (o mesmo que `natives::inventory` lê).
pub fn native_pragma(
    program: &Program,
    interner: &Interner,
    fid: usize,
) -> (Option<String>, bool) {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    let f = &program.functions[fid];
    let (unit, metadata): (_, &[ast::Annotation]) = match f.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let m = a
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                .map(|m| &*m.metadata)
                .or_else(|| {
                    a.decls
                        .iter()
                        .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                        .map(|d| &*d.metadata)
                })
                .unwrap_or(&[]);
            (unit, m)
        }
        FunctionRef::Constructor { unit, member } => (unit, &*program.unit(unit).ast.members[member.0 as usize].metadata),
        FunctionRef::None => return (None, false),
    };
    let mut native = None;
    let mut recognized = false;
    for an in metadata {
        if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
            continue;
        }
        let Some(args) = &an.arguments else { continue };
        let strings: Vec<Option<String>> = args
            .args
            .iter()
            .map(|a| match &program.unit(unit).ast.exprs[a.value.0 as usize].kind {
                ast::ExprKind::String(lit) => {
                    lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned())
                }
                _ => None,
            })
            .collect();
        match strings.first().and_then(|t| t.as_deref()) {
            Some("vm:external-name") => native = strings.get(1).cloned().flatten(),
            Some("vm:recognized") => recognized = true,
            _ => {}
        }
    }
    (native, recognized)
}

/// O símbolo é de uma função de biblioteca do SDK (`df.dart$3a…`, o prefixo
/// que o caminho estável dá a toda função `dart:` e às closures e entradas
/// dela).
pub fn sdk_symbol(sym_name: &str) -> bool {
    sym_name.starts_with("df.dart$3a")
}

/// Poda o módulo a partir do que é do programa: toda função que não é do SDK
/// fica, e toda função do SDK que ela alcança (transitivamente) também. Os
/// diagnósticos de uma função da fonte (`source_errors`, pelo símbolo da
/// função de topo que os produziu) só passam a `errors` se ela ficar.
pub fn prune(module: &mut Module) {
    let index: HashMap<&str, usize> =
        module.functions.iter().enumerate().map(|(i, f)| (f.symbol.as_str(), i)).collect();
    let mut live = vec![false; module.functions.len()];
    let mut stack: Vec<usize> = Vec::new();
    let mark = |s: &str, live: &mut Vec<bool>, stack: &mut Vec<usize>| {
        if let Some(&i) = index.get(s)
            && !live[i]
        {
            live[i] = true;
            stack.push(i);
        }
    };
    // Raízes: as funções do programa; os getters de receita de RTI
    // (`df.rti.*`) só ficam se alguém os chama.
    for f in &module.functions {
        if !sdk_symbol(&f.symbol) && !f.symbol.starts_with("df.rti.") {
            mark(&f.symbol, &mut live, &mut stack);
        }
    }
    for c in &module.classes {
        if let Some(s) = &c.to_string_symbol {
            mark(s, &mut live, &mut stack);
        }
        for (_, s) in &c.vtable {
            mark(s, &mut live, &mut stack);
        }
    }
    for s in &module.source_roots {
        mark(s, &mut live, &mut stack);
    }
    while let Some(i) = stack.pop() {
        for b in &module.functions[i].blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::CallStatic { symbol, .. } => mark(symbol, &mut live, &mut stack),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => {
                        mark(code_symbol, &mut live, &mut stack)
                    }
                    Instruction::AllocClosureTyped { code_symbol, typed, .. } => {
                        mark(code_symbol, &mut live, &mut stack);
                        mark(typed, &mut live, &mut stack);
                    }
                    Instruction::LoadGlobal { symbol: sym_name, .. } | Instruction::StoreGlobal { symbol: sym_name, .. } => {
                        mark(sym_name, &mut live, &mut stack)
                    }
                    _ => {}
                }
            }
        }
    }
    let live_set: HashSet<String> = module
        .functions
        .iter()
        .zip(&live)
        .filter(|(_, v)| **v)
        .map(|(f, _)| f.symbol.clone())
        .collect();
    let mut k = 0;
    module.functions.retain(|_| {
        let v = live[k];
        k += 1;
        v
    });
    module.dart_scalar_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.dart_rti_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.dart_vector_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.arc_uniform_returns.retain(|sym_name| live_set.contains(sym_name));
    module.arc_shared_functions.retain(|sym_name| live_set.contains(sym_name));
    module.dart_ref_returns.retain(|sym_name| live_set.contains(sym_name));
    module.arc_sites.retain(|sym_name, _| live_set.contains(sym_name));
    for (owner, errors) in std::mem::take(&mut module.source_errors) {
        if live_set.contains(&owner) {
            module.errors.extend(errors);
        }
    }
}
