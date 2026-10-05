//! `FfiCode.NON_CONSTANT_TYPE_ARGUMENT` (`FfiVerifier`,
//! `analyzer/lib/src/generated/ffi_verifier.dart` do analyzer 3.6.2), a
//! primeira parte do verificador de FFI: um argumento de tipo de uma API de
//! `dart:ffi` que não é conhecido em tempo de compilação.
//!
//! Versão conservadora: só relata quando o tipo em questão é um parâmetro de
//! tipo (`T`, `Pointer<T>`), caso em que o `_isValidFfiNativeType` do
//! original sempre recusa. Os outros tipos inválidos (uma classe que não é
//! nativa) ficam sem relato, e o resto do `FfiVerifier` (48 códigos) não
//! está aqui. Os nove pontos do original:
//!
//! | ponto | forma | lugar | `{0}` |
//! |---|---|---|---|
//! | `_validateTypeArgument` | `p.asFunction<R>()` com `R` parâmetro | o `R` | `asFunction` |
//! | `_validateAsFunction` | `p.asFunction()` com `p: Pointer<NativeFunction<T>>` | o alvo | `asFunction` |
//! | `_validateRefPrefixedIdentifier`, `_validateRefPropertyAccess` | `p.ref` | o acesso | `ref` |
//! | `_validateRefIndexed` | `p[i]` | o índice | `[]` |
//! | `_validateSizeOf` | `sizeOf<T>()` | a chamada | `sizeOf` |
//! | `_validateAllocate` | `alocador<T>()` | a chamada | `AllocatorAlloc.call` |
//! | `_validateCreate` | `Struct.create<T>()` | a chamada | `Struct.create`/`Union.create` |
//! | `_validateElementAt` | `p.elementAt(i)` | a chamada | `elementAt` |
//!
//! Diferenças conhecidas: o `ref` e o `elementAt` são reconhecidos pela
//! resolução quando ela existe e, no índice (que a inferência não
//! registra), pelo tipo: `Pointer<T>`/`Array<T>` de `dart:ffi` com `T`
//! parâmetro limitado por `Struct` ou `Union`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::table::{Type, TypeId, TypeTable};
use dartforge_diagnostics::{codigos::ffi, Diagnostic, Span};
use dartforge_elements::model::{ClassId, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::Interner;

/// Os relatos de `NON_CONSTANT_TYPE_ARGUMENT` na unidade `u`.
pub fn argumentos_nao_constantes(program: &Program, interner: &Interner, table: &TypeTable, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(ffi_lib) = program.libraries.iter().position(|l| l.uri == "dart:ffi").map(|i| LibraryId(i as u32)) else {
        return out;
    };
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let relatar = |out: &mut Vec<Diagnostic>, span: Span, nome: &str| {
        out.push(Diagnostic::com_codigo(ffi::NON_CONSTANT_TYPE_ARGUMENT, span, [nome]));
    };

    // A classe de `dart:ffi` com o nome dado.
    let classe_ffi = |c: ClassId, nome: &str| {
        let k = program.class(c);
        k.library == ffi_lib && interner.resolve(k.name) == nome
    };
    let parametro = |t: TypeId| matches!(table.get(t), Type::TypeParameter { .. });
    // O argumento de `Pointer<X>` (ou de outra classe de `dart:ffi`).
    let argumento_de = |t: TypeId, nome: &str| match table.get(t) {
        Type::Interface { class, args, .. } if classe_ffi(*class, nome) && args.len() == 1 => Some(args[0]),
        _ => None,
    };
    // O parâmetro de tipo limitado (direto ou por outro parâmetro) por uma
    // subclasse de `Struct` ou `Union`.
    let de_composto = |t: TypeId| {
        let mut atual = t;
        for _ in 0..8 {
            match table.get(atual) {
                Type::TypeParameter { param, .. } => atual = table.param(*param).bound,
                Type::Interface { class, .. } => {
                    let mut c = Some(*class);
                    while let Some(k) = c {
                        if classe_ffi(k, "Struct") || classe_ffi(k, "Union") {
                            return true;
                        }
                        c = program.class(k).supertype_class;
                    }
                    return false;
                }
                _ => return false,
            }
        }
        false
    };
    // O membro de extensão de `dart:ffi` com nome de extensão e de membro.
    let extensao_ffi = |r: Option<&Resolved>, extensoes: &[&str], membro: &str| match r {
        Some(Resolved::ExtensionMember { extension, member }) => {
            let x = program.extension(*extension);
            x.library == ffi_lib
                && x.name.is_some_and(|n| extensoes.contains(&interner.resolve(n)))
                && interner.resolve(program.function(*member).name) == membro
        }
        _ => false,
    };
    let tipo = |e: ExprId| corpo.get_type(e);
    // O primeiro argumento de tipo escolhido para a chamada (escrito ou
    // inferido), pelo início da lista de argumentos.
    let instanciado = |inicio: usize| corpo.instanciacao(inicio).and_then(|v| (v.len() == 1).then_some(v[0]));

    for (i, e) in a.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        match &e.kind {
            // `p.ref`.
            ExprKind::Property { target, name, .. } if interner.resolve(name.sym) == "ref" => {
                if extensao_ffi(corpo.get_resolved(id), &["StructPointer", "UnionPointer"], "ref")
                    && tipo(*target).is_some_and(|t| parametro(t) || argumento_de(t, "Pointer").is_some_and(parametro))
                {
                    relatar(&mut out, e.span, "ref");
                }
            }
            // `p[i]`.
            ExprKind::Index { target, .. } => {
                let Some(t) = tipo(*target) else { continue };
                let x = argumento_de(t, "Pointer").or_else(|| argumento_de(t, "Array"));
                if x.is_some_and(|x| parametro(x) && de_composto(x)) {
                    relatar(&mut out, e.span, "[]");
                }
            }
            ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                ExprKind::Property { target: receptor, name, .. } => {
                    let r = corpo.get_resolved(*target);
                    match interner.resolve(name.sym) {
                        "asFunction" if extensao_ffi(r, &["NativeFunctionPointer"], "asFunction") => {
                            // O argumento escrito vem antes e, se relatado, encerra.
                            if let [escrito] = &arguments.type_args[..]
                                && instanciado(arguments.span.start).is_some_and(parametro)
                            {
                                relatar(&mut out, a.ty(*escrito).span, "asFunction");
                                continue;
                            }
                            let nativo = tipo(*receptor).and_then(|t| argumento_de(t, "Pointer")).and_then(|n| argumento_de(n, "NativeFunction"));
                            if nativo.is_some_and(parametro) {
                                relatar(&mut out, a.expr(*receptor).span, "asFunction");
                            }
                        }
                        "elementAt" => {
                            let de_pointer = matches!(r, Some(Resolved::Member { class, member: MemberRef::Function(_), .. }) if classe_ffi(*class, "Pointer"));
                            if de_pointer && tipo(*receptor).and_then(|t| argumento_de(t, "Pointer")).is_some_and(parametro) {
                                relatar(&mut out, e.span, "elementAt");
                            }
                        }
                        "create" => {
                            // `Struct.create<T>()`: o receptor é a classe.
                            let classe = match corpo.get_resolved(*receptor) {
                                Some(Resolved::Element(Element::Class(c))) => Some(*c),
                                _ => None,
                            };
                            if let Some(c) = classe.filter(|c| classe_ffi(*c, "Struct") || classe_ffi(*c, "Union"))
                                && instanciado(arguments.span.start).is_some_and(parametro)
                            {
                                let nome = format!("{}.create", interner.resolve(program.class(c).name));
                                relatar(&mut out, e.span, &nome);
                            }
                        }
                        _ => {}
                    }
                }
                // `sizeOf<T>()`.
                ExprKind::Identifier(n) if interner.resolve(n.sym) == "sizeOf" => {
                    let de_ffi = matches!(
                        corpo.get_resolved(*target),
                        Some(Resolved::Element(Element::Function(f))) if program.function(*f).library == ffi_lib
                    );
                    if de_ffi && instanciado(arguments.span.start).is_some_and(parametro) {
                        relatar(&mut out, e.span, "sizeOf");
                    }
                }
                // `alocador<T>(n)`: o `call` de `AllocatorAlloc` sobre um
                // valor cujo tipo implementa `Allocator`.
                _ => {
                    let alocador = tipo(*target).is_some_and(|t| match table.get(t) {
                        Type::Interface { class, .. } => {
                            classe_ffi(*class, "Allocator")
                                || program.class(*class).interface_classes.iter().any(|k| classe_ffi(*k, "Allocator"))
                                || program.class(*class).supertype_class.is_some_and(|k| classe_ffi(k, "Allocator"))
                        }
                        _ => false,
                    });
                    if alocador && !arguments.type_args.is_empty() && instanciado(arguments.span.start).is_some_and(parametro) {
                        relatar(&mut out, e.span, "AllocatorAlloc.call");
                    }
                }
            },
            _ => {}
        }
    }
    out
}
