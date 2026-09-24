//! Bibliotecas do SDK compiladas **da fonte** junto com o programa (P6,
//! docs/NATIVO-PLANO.md §7.7).
//!
//! A decisão 1 da rodada 2 (NATIVO-PLANO §7.1) é que o SDK do nativo vem da
//! fonte da seção `vm` com a sobreposição `sdk_nativo/`. O `dart:async` é a
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
pub fn bibliotecas_da_fonte(program: &Program, interner: &Interner) -> Vec<LibraryId> {
    let achar = |uri: &str| program.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32));
    let Some(async_lib) = achar("dart:async") else {
        return Vec::new();
    };
    if !usa_dart_async(program, interner, async_lib) {
        return Vec::new();
    }
    // O `dart:_internal` vem junto: o `dart:async` usa dele `unsafeCast`,
    // `IterableElementError`, `checkNotNullable`… (código Dart puro, fora os
    // intrínsecos, `lower/externos.rs`).
    let mut v = vec![async_lib];
    v.extend(achar("dart:_internal"));
    v
}

/// Os `part` do `dart:core` que o `dart:async` da fonte precisa e que o
/// runtime não representa: código Dart puro, sem `external`. O `Timer` recebe
/// uma `Duration` (`Future()`, `Future.delayed`, `Timer.run` usam
/// `Duration.zero`); o runtime em Rust nunca teve `Duration`.
const PARTES_DO_CORE: &[&str] = &["core/duration.dart"];

/// Tira do `dart:core` as unidades de [`PARTES_DO_CORE`] para uma biblioteca
/// nova, de mesmo URI e mesmo escopo, que não é do SDK — as classes delas
/// passam a ser compiladas da fonte como as do `dart:async`, e o resto do
/// `dart:core` continua sendo o do runtime. Os elementos não mudam de id
/// (quem os referencia continua referenciando); só a biblioteca dona muda.
/// Devolve a biblioteca nova.
pub fn separar_partes_do_core(program: &mut Program) -> Option<LibraryId> {
    use dartforge_elements::model::{FunctionRef, Library, VariableRef};
    let core = program.core?;
    let unidades: Vec<dartforge_elements::model::UnitId> = program.libraries[core.0 as usize]
        .units
        .iter()
        .copied()
        .filter(|u| {
            program.unit(*u).path.as_ref().is_some_and(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                PARTES_DO_CORE.iter().any(|suf| s.ends_with(suf))
            })
        })
        .collect();
    if unidades.is_empty() {
        return None;
    }
    let origem = &program.libraries[core.0 as usize];
    let nova = Library {
        uri: origem.uri.clone(),
        name: origem.name.clone(),
        units: unidades.clone(),
        imports: origem.imports.clone(),
        exports: Vec::new(),
        declared: origem.declared.clone(),
        exported: origem.exported.clone(),
        scope: origem.scope.clone(),
        prefixes: origem.prefixes.clone(),
        is_sdk: false,
        // A versão de linguagem e os recursos por biblioteca (Dart moderno, P2).
        features: origem.features,
    };
    let id = LibraryId(program.libraries.len() as u32);
    program.libraries.push(nova);
    program.libraries[core.0 as usize].units.retain(|u| !unidades.contains(u));
    for u in &unidades {
        program.units[u.0 as usize].library = id;
    }
    for c in &mut program.classes {
        if c.decl.is_some_and(|d| unidades.contains(&d.unit)) {
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
        if unit.is_some_and(|u| unidades.contains(&u)) {
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
        if unit.is_some_and(|u| unidades.contains(&u)) {
            v.library = id;
        }
    }
    Some(id)
}

/// O programa (as bibliotecas que não são do SDK) usa `dart:async`?
fn usa_dart_async(program: &Program, interner: &Interner, async_lib: LibraryId) -> bool {
    let nomes: HashSet<&str> = ["Future", "Stream", "FutureOr"].into_iter().collect();
    for lib in program.libraries.iter().filter(|l| !l.is_sdk) {
        if lib.imports.iter().any(|i| i.library == async_lib) {
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
                    ExprKind::Identifier(n) if nomes.contains(interner.resolve(n.sym)) => return true,
                    _ => {}
                }
            }
            for t in &ast.types {
                if let TypeKind::Named { name, .. } = &t.kind
                    && name.last().is_some_and(|n| nomes.contains(interner.resolve(n.sym)))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Os pragmas `vm:external-name`/`vm:recognized` de uma função: o nome do
/// native, se há um, e se ela é intrínseco. O pragma mora no `Member`/`Decl`
/// que declara a função (o mesmo que `nativos::inventario` lê).
pub fn pragma_nativo(
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
    let mut reconhecido = false;
    for an in metadata {
        if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
            continue;
        }
        let Some(args) = &an.arguments else { continue };
        let textos: Vec<Option<String>> = args
            .args
            .iter()
            .map(|a| match &program.unit(unit).ast.exprs[a.value.0 as usize].kind {
                ast::ExprKind::String(lit) => {
                    lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned())
                }
                _ => None,
            })
            .collect();
        match textos.first().and_then(|t| t.as_deref()) {
            Some("vm:external-name") => native = textos.get(1).cloned().flatten(),
            Some("vm:recognized") => reconhecido = true,
            _ => {}
        }
    }
    (native, reconhecido)
}

/// O símbolo é de uma função de biblioteca do SDK (`df.dart$3a…`, o prefixo
/// que o caminho estável dá a toda função `dart:` e às closures e entradas
/// dela).
pub fn simbolo_do_sdk(simbolo: &str) -> bool {
    simbolo.starts_with("df.dart$3a")
}

/// Poda o módulo a partir do que é do programa: toda função que não é do SDK
/// fica, e toda função do SDK que ela alcança (transitivamente) também. Os
/// diagnósticos de uma função da fonte (`erros_da_fonte`, pelo símbolo da
/// função de topo que os produziu) só passam a `erros` se ela ficar.
pub fn podar(module: &mut Module) {
    let indice: HashMap<&str, usize> =
        module.functions.iter().enumerate().map(|(i, f)| (f.symbol.as_str(), i)).collect();
    let mut vivo = vec![false; module.functions.len()];
    let mut pilha: Vec<usize> = Vec::new();
    let marcar = |s: &str, vivo: &mut Vec<bool>, pilha: &mut Vec<usize>| {
        if let Some(&i) = indice.get(s)
            && !vivo[i]
        {
            vivo[i] = true;
            pilha.push(i);
        }
    };
    // Raízes: as funções do programa; os getters de receita de RTI
    // (`df.rti.*`) só ficam se alguém os chama.
    for f in &module.functions {
        if !simbolo_do_sdk(&f.symbol) && !f.symbol.starts_with("df.rti.") {
            marcar(&f.symbol, &mut vivo, &mut pilha);
        }
    }
    for c in &module.classes {
        if let Some(s) = &c.to_string_symbol {
            marcar(s, &mut vivo, &mut pilha);
        }
        for (_, s) in &c.vtable {
            marcar(s, &mut vivo, &mut pilha);
        }
    }
    for s in &module.raizes_da_fonte {
        marcar(s, &mut vivo, &mut pilha);
    }
    while let Some(i) = pilha.pop() {
        for b in &module.functions[i].blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::CallStatic { symbol, .. } => marcar(symbol, &mut vivo, &mut pilha),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => {
                        marcar(code_symbol, &mut vivo, &mut pilha)
                    }
                    Instruction::LoadGlobal { simbolo, .. } | Instruction::StoreGlobal { simbolo, .. } => {
                        marcar(simbolo, &mut vivo, &mut pilha)
                    }
                    _ => {}
                }
            }
        }
    }
    let vivos: HashSet<String> = module
        .functions
        .iter()
        .zip(&vivo)
        .filter(|(_, v)| **v)
        .map(|(f, _)| f.symbol.clone())
        .collect();
    let mut k = 0;
    module.functions.retain(|_| {
        let v = vivo[k];
        k += 1;
        v
    });
    for (dono, erros) in std::mem::take(&mut module.erros_da_fonte) {
        if vivos.contains(&dono) {
            module.erros.extend(erros);
        }
    }
}
