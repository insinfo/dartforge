//! Seleciona entradas auditadas para operações nativas ARC.
//! Os contratos conferem SSA, pendência e callbacks na preparação completa.

use crate::hir::*;

pub(super) fn prepare(f: &mut Function) -> Result<(), String> {
    // O tear-off canônico, com owner do código (`dartforge_arc_tearoff_owned_v1`).
    for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::TearOff { code_symbol } = inst {
            if *ty != Type::Ref {
                return Err(format!("tear-off ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            *inst = Instruction::CallRuntime {
                name: "dartforge_arc_tearoff_owned_v1".into(),
                args: vec![(Operand::Constant(Constant::Function(std::mem::take(code_symbol))), Type::I64)],
                ret_ty: Type::Ref,
            };
        }
    }
    for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        let Instruction::CallRuntime { name, args, ret_ty } = inst else {
            continue;
        };
        // A exceção e o rastro pendentes com owner independente do `clear`.
        let pending = match name.as_str() {
            "dartforge_exception_peek_ref" => Some("dartforge_arc_excecao_owned_v1"),
            "dartforge_stack_trace_get" => Some("dartforge_arc_rastro_owned_v1"),
            _ => None,
        };
        if let Some(owned) = pending {
            if *ty != Type::Ref || *ret_ty != Type::Ref || !args.is_empty() {
                return Err(format!("exceção pendente ARC em {} v{}: ABI incompatível", f.symbol, v.0));
            }
            *name = owned.into();
            continue;
        }
        // Os acessos físicos por índice (o quadro de `lower/async_sm.rs`, os
        // campos de record, o `name` de enum, o campo por índice dinâmico):
        // as entradas auditadas, que conferem o objeto, o índice e o mapa do
        // campo (a leitura escalar recusa um campo Ref). A marca da gravação
        // é a representação estática do valor.
        if name == "dartforge_object_get" {
            if *ty != *ret_ty || !matches!(ret_ty, Type::I64 | Type::Ref)
                || !matches!(args.as_slice(), [(_, Type::Ref), (_, Type::I64)])
            {
                return Err(format!("leitura de campo ARC em {} v{}: ABI incompatível", f.symbol, v.0));
            }
            // Ref: um empréstimo do receiver, sem retenção.
            *name = if *ret_ty == Type::Ref { "dartforge_arc_ler_campo_ref_v1" } else { "dartforge_arc_ler_campo_escalar_v1" }.into();
            continue;
        }
        if name == "dartforge_object_set" {
            let [(object, Type::Ref), (index, Type::I64), (value, Type::I64),
                (Operand::Constant(Constant::Int(mark @ (0 | 1))), Type::I8)] = args.as_slice()
            else {
                return Err(format!("gravação de campo ARC em {} v{}: ABI ou marca incompatível", f.symbol, v.0));
            };
            let (name_str, repr) = if *mark == 1 {
                ("dartforge_arc_gravar_campo_ref_v1", Type::Ref)
            } else {
                ("dartforge_arc_gravar_campo_escalar_v1", Type::I64)
            };
            *args = vec![(object.clone(), Type::Ref), (index.clone(), Type::I64), (value.clone(), repr)];
            *name = name_str.into();
            continue;
        }
        // A string nova (`@df.texto_alocar`) com owner do código.
        if name == "df.texto_alocar" {
            let [(len, Type::I64), (Operand::Constant(Constant::Bool(two)), Type::I1)] = args.as_slice() else {
                return Err(format!("alocação de texto ARC em {} v{}: ABI incompatível", f.symbol, v.0));
            };
            if *ty != Type::Ref || *ret_ty != Type::Ref {
                return Err(format!("alocação de texto ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            *args = vec![(len.clone(), Type::I64), (Operand::Constant(Constant::Int(i64::from(*two))), Type::I64)];
            *name = "dartforge_arc_texto_novo_owned_v1".into();
            continue;
        }
        // Os construtores de erro do runtime: a variante Owned (o par
        // bits/é-referência vira o valor `Ref`).
        if let Some(base) = name.strip_prefix("dartforge_")
            && ERRORS_WITH_ARC_VARIANT.contains(&base)
        {
            if *ty != Type::Ref || *ret_ty != Type::Ref {
                return Err(format!("erro do runtime ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            if matches!(base, "exception_new" | "assertion_error_new" | "argument_error_value") {
                let valid_ref = |o: &Operand| matches!(o, Operand::Val(_) | Operand::Constant(Constant::Null));
                if args.len() < 2 || !valid_ref(&args[0].0) || !matches!(args[1].0, Operand::Constant(Constant::Int(1))) {
                    return Err(format!("erro do runtime ARC em {} v{}: valor sem referência", f.symbol, v.0));
                }
                args.remove(1);
                args[0].1 = Type::Ref;
            }
            *name = format!("dartforge_arc_{base}_owned_v1");
            continue;
        }
        if name == "df.lista_len" {
            if *ty != Type::I64
                || *ret_ty != Type::I64
                || !matches!(args.as_slice(), [(_, Type::Ref)])
            {
                return Err(format!(
                    "comprimento ARC em {} v{}: ABI Ref/I64 incompatível",
                    f.symbol, v.0
                ));
            }
            *name = "dartforge_arc_lista_len_v1".into();
            continue;
        }
        // Callback FFI: a closure do contexto e o `Pointer` do argumento
        // ganham owner do código, pela porta Owned no caso do ponteiro.
        let callback = match name.as_str() {
            "dartforge_ffi_callback_closure" => Some(("dartforge_arc_ffi_callback_closure_owned_v1", 1)),
            "dartforge_ffi_callback_ponteiro" => Some(("dartforge_arc_ffi_callback_ponteiro_owned_v1", 3)),
            _ => None,
        };
        if let Some((owned, n)) = callback {
            if *ty != Type::Ref || *ret_ty != Type::Ref || args.len() != n || args.iter().any(|(_, t)| *t != Type::I64) {
                return Err(format!("callback FFI ARC em {} v{}: ABI I64/Ref incompatível", f.symbol, v.0));
            }
            *name = owned.into();
            continue;
        }
        // O objeto de um `Handle`: argumento de callback ou retorno de uma
        // chamada nativa (lido antes de fechar o escopo das células).
        let handle = match name.as_str() {
            "dartforge_ffi_objeto_do_handle" => Some(("dartforge_arc_ffi_objeto_do_handle_owned_v1", 1)),
            "dartforge_ffi_handles_fechar_com_objeto" => Some(("dartforge_arc_ffi_handles_fechar_com_objeto_owned_v1", 2)),
            _ => None,
        };
        if let Some((owned, n)) = handle {
            if *ty != Type::Ref || *ret_ty != Type::Ref || args.len() != n || args.iter().any(|(_, t)| *t != Type::I64) {
                return Err(format!("handle FFI ARC em {} v{}: ABI I64/Ref incompatível", f.symbol, v.0));
            }
            *name = owned.into();
            continue;
        }
        // O `Pointer` devolvido por uma chamada nativa: endereço e closure
        // emprestada, owner do código pela porta Owned do ajudante.
        if name == "dartforge_ffi_ponteiro_de_retorno" {
            if *ty != Type::Ref || *ret_ty != Type::Ref || !matches!(args.as_slice(), [(_, Type::I64), (_, Type::Ref)]) {
                return Err(format!("ponteiro de retorno ARC em {} v{}: ABI I64,Ref/Ref incompatível", f.symbol, v.0));
            }
            *name = "dartforge_arc_ffi_ponteiro_de_retorno_owned_v1".into();
            continue;
        }
        let rti = match name.as_str() {
            "dartforge_rti_texto" => Some("dartforge_arc_rti_texto_owned_v1"),
            "dartforge_rti_objeto_tipo" => Some("dartforge_arc_rti_objeto_tipo_owned_v1"),
            _ => None,
        };
        if let Some(owned) = rti {
            if *ty != Type::Ref || *ret_ty != Type::Ref || !matches!(args.as_slice(), [(_, Type::I64)]) {
                return Err(format!("texto/objeto RTI ARC em {} v{}: ABI I64/Ref incompatível", f.symbol, v.0));
            }
            *name = owned.into();
            continue;
        }
        if name == "dartforge_nativo_Socket_RecvFrom" {
            if *ty != Type::Ref || *ret_ty != Type::Ref || !matches!(args.as_slice(), [(_, Type::Ref)])
            {
                return Err(format!(
                    "recepção ARC em {} v{}: ABI Ref incompatível",
                    f.symbol, v.0
                ));
            }
            *name = "dartforge_arc_socket_recvfrom_owned_v1".into();
            continue;
        }
        // Uma função do runtime que devolve um objeto vivo sem owner do
        // código: o thunk Owned genérico (escopo nativo, guarda, retain).
        if *ret_ty == Type::Ref && generic_owned(name) && super::contract(name).is_err() {
            if *ty != Type::Ref || args.iter().any(|(_, t)| !matches!(t, Type::Ref | Type::I64 | Type::F64 | Type::I8 | Type::I1 | Type::Ptr)) {
                return Err(format!("thunk Owned em {} v{}: ABI incompatível", f.symbol, v.0));
            }
            *name = format!("{}{name}", super::GENERIC_OWNED_PREFIX);
        }
    }
    Ok(())
}

/// Cada chamada a uma função do runtime que pode rodar Dart, com contrato
/// próprio, passa a rodar num escopo nativo (`NATIVE_SCOPE_PREFIX`). Roda
/// depois das preparações que criam chamadas (`boxes`, `fields`…). As ABIs
/// `dartforge_arc_*` já escopam por dentro (`em_escopo_nativo`).
pub(super) fn scope(f: &mut Function) {
    for (_, inst, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::CallRuntime { name, .. } = inst
            && !name.starts_with("dartforge_arc_")
            && !name.starts_with("df.")
            && crate::llvm::externs::effects_of(name).calls_dart
            && super::contract(name).is_ok()
        {
            *name = format!("{}{name}", super::NATIVE_SCOPE_PREFIX);
        }
    }
}

/// Os construtores de erro do runtime com variante Owned (o nome sem o
/// prefixo `dartforge_`; `lower/runtime_errors.rs` os emite).
pub(crate) const ERRORS_WITH_ARC_VARIANT: &[&str] = &[
    "exception_new", "assertion_error_new", "state_error_new", "unsupported_error_new",
    "unimplemented_error_new", "argument_error_new", "argument_error_value",
    "argument_error_not_null", "range_error_range", "type_error_new", "null_check_error_new",
    "late_error_new", "stack_overflow_error_new", "no_such_method_error_new",
];

/// As funções do runtime cujo resultado `Ref` está vivo ao voltar, sem owner
/// do código e sem consumir argumentos: os natives do SDK e os construtores
/// do runtime chamados pelo lowering. Leitura de campo, alocação de objeto e
/// as de endereço nativo têm produtores próprios.
fn generic_owned(name_str: &str) -> bool {
    name_str.starts_with("dartforge_nativo_")
        || matches!(
            name_str,
            "dartforge_collection_mark_unmodifiable"
                | "dartforge_constante_canonica"
                | "dartforge_encaminhar_nsm"
                | "dartforge_ffi_composto"
                | "dartforge_ffi_composto_copia"
                | "dartforge_ffi_composto_novo"
                | "dartforge_ffi_ponteiro_novo"
                | "dartforge_lista_de_tabela"
                | "dartforge_lista_de_tabela_g"
                | "dartforge_rastro_texto"
                | "dartforge_string_concat"
                | "dartforge_to_string_bool"
                | "dartforge_to_string_f64"
                | "dartforge_typed_externo"
                | "dartforge_typed_novo"
                | "dartforge_view_nova"
        )
}

#[cfg(test)]
mod tests {
    use crate::optimize::arc::*;
    use std::collections::HashMap;

    #[test]
    fn local_selector_requires_refs_descriptor_rti_and_pending_without_changing_tracing() {
        for case in 0..9 {
            let mut m = Module::new();
            m.arc_memory = case != 8;
            let mut insts = vec![
                (ValueId(2), Instruction::CallSelector { selector: "tc:start".into(),
                    recv: Operand::Val(ValueId(0)), args: vec![Operand::Val(ValueId(1))],
                    names: if case == 3 { vec!["x".into(), "x".into()] } else { vec![] },
                    tuple_types: Operand::Constant(Constant::Int(if case == 4 { 7 } else { 0 })) },
                    if case == 5 { Type::I64 } else { Type::Ref }),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            if case == 6 { insts.pop(); }
            if case == 7 && let Instruction::CallSelector { args, .. } = &mut insts[0].1 {
                args[0] = Operand::Val(ValueId(99));
            }
            m.functions.push(Function { symbol: "seletor".into(), name: "seletor".into(), debug_info: None,
                params: vec![(ValueId(0), "recv".into(), if case == 1 { Type::I64 } else { Type::Ref }),
                    (ValueId(1), "arg".into(), if case == 2 { Type::I64 } else { Type::Ref })], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                    terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.dart_ref_returns.insert("seletor".into());
            let mut plans = HashMap::from([("seletor".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}"); let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 0 {
                r.unwrap();
                assert_eq!(plans["seletor"].classes[&ValueId(2)], Ownership::Owned);
                assert_eq!(plans["seletor"].tokens.pendings[&ValueId(2)], BlockId(1));
                certify_arc_aot_ports(&mut m, &plans).unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc2"));
                assert!(!ir.contains("call i64 %sf2("));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                if case == 8 {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert!(crate::llvm::LlvmEmitter::new(&m).emit_all().contains("call i64 %sf2("));
                } else { assert!(r.is_err(), "caso {case}"); }
                assert_eq!(format!("{m:?}"), before); assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn selector_accepts_proven_rti_getter_tuple_and_rejects_constant() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = case != 3;
            m.globals.push((0, Type::I64, "cache".into()));
            m.arc_rti_caches.insert("cache".into());
            let ret = if case == 1 { ValueId(9) } else { ValueId(1) };
            m.functions.push(Function { symbol: "getter".into(), name: "getter".into(), debug_info: None,
                params: vec![], return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::Const(Constant::String("C201".into())), Type::Ref),
                    (ValueId(1), Instruction::CallRuntime { name: "dartforge_rti_receita".into(),
                        args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::StoreGlobal { symbol: "cache".into(), val: Operand::Val(ValueId(1)),
                        ty: Type::I64, root: None }, Type::Void),
                    (ValueId(3), Instruction::StoreGlobal { symbol: "cache$ok".into(),
                        val: Operand::Constant(Constant::Int(1)), ty: Type::I8, root: None }, Type::Void),
                    (ValueId(9), Instruction::Const(Constant::Int(42)), Type::I64)],
                    terminator: Terminator::Return(Some(Operand::Val(ret))) }] });
            let getter_args = if case == 2 { vec![Operand::Constant(Constant::Int(0))] } else { vec![] };
            m.functions.push(Function { symbol: "seletor".into(), name: "seletor".into(), debug_info: None,
                params: vec![(ValueId(0), "recv".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::CallStatic { symbol: "getter".into(), args: getter_args, ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::CallSelector { selector: "tc:fold".into(), recv: Operand::Val(ValueId(0)),
                        args: vec![], names: vec![], tuple_types: Operand::Val(ValueId(1)) }, Type::Ref),
                    (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1)],
                    terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.dart_ref_returns.insert("seletor".into());
            let mut plans = HashMap::from([("getter".into(), DartFunctionPlan::default()), ("seletor".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}"); let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    assert_eq!(plans["seletor"].classes[&ValueId(2)], Ownership::Owned);
                    certify_arc_aot_ports(&mut m, &plans).unwrap();
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc2"));
                }
                3 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert!(crate::llvm::LlvmEmitter::new(&m).emit_all().contains("call i64 %sf2("));
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before); assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn literal_produces_owner_by_real_types_and_rejects_incompatible_abi() {
        for case in 0..8 {
            let mut m = Module::new();
            m.arc_memory = case != 7;
            let t = match case { 2 => Type::Ptr, 3 => Type::V4I32, 4 => Type::I8, _ => Type::Ref };
            let mut elements = if case == 0 { vec![] } else { vec![(Operand::Val(ValueId(0)), 255),
                (Operand::Constant(Constant::Int(i64::MAX)), 255),
                (Operand::Constant(Constant::Double(-0.0)), 255),
                (Operand::Constant(Constant::Bool(true)), 255)] };
            if case == 5 { elements.push((Operand::Constant(Constant::Function("alvo".into())), 1)); }
            m.functions.push(Function { symbol: "literal".into(), name: "literal".into(), debug_info: None,
                params: vec![(ValueId(0), "elemento".into(), t)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1),
                    Instruction::AllocList { elements }, if case == 6 { Type::I64 } else { Type::Ref })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
            let mut p = DartFunctionPlan::default();
            p.tokens.ret = TokenReturn::Owned;
            let mut plans = HashMap::from([("literal".into(), p)]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case < 2 {
                r.unwrap();
                assert_eq!(plans["literal"].classes[&ValueId(1)], Ownership::Owned);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_lista_nova_owned_v1(ptr %list_buf_1"));
                if case == 1 { assert!(ir.contains("store i8 3") && ir.contains("store i8 4")); }
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                if case == 7 {
                    assert_eq!(r.unwrap(), (0, 0));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_lista_nova(ptr %list_buf_1"));
                } else { assert!(r.is_err(), "caso {case}"); }
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn interpolation_produces_owner_by_real_types_and_rejects_parts_without_abi() {
        for case in 0..7 {
            let mut m = Module::new();
            m.arc_memory = case != 6;
            let t = match case { 2 => Type::F64, 3 => Type::I1, _ => Type::Ref };
            let mut parts = vec![Operand::Val(ValueId(0)), Operand::Constant(Constant::Int(i64::MIN)),
                Operand::Val(ValueId(1)), Operand::Val(ValueId(3))];
            if case == 4 { parts.push(Operand::Constant(Constant::Null)); }
            if case == 5 { parts.push(Operand::Constant(Constant::StringWtf8(b" fim".to_vec()))); }
            m.functions.push(Function { symbol: "juntar".into(), name: "juntar".into(), debug_info: None,
                params: vec![(ValueId(0), "texto".into(), t)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Const(Constant::Int(42)), Type::I64),
                    (ValueId(3), Instruction::Const(Constant::StringWtf8(b" fim".to_vec())), Type::Ref),
                    (ValueId(2), Instruction::ConcatStrings { parts }, if case == 1 { Type::I64 } else { Type::Ref })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            let mut p = DartFunctionPlan::default();
            p.tokens.ret = TokenReturn::Owned;
            let mut plans = HashMap::from([("juntar".into(), p)]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    assert_eq!(plans["juntar"].classes[&ValueId(2)], Ownership::Owned);
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_string_juntar_owned_v1(ptr %jbuf2, i64 4)"));
                    assert!(!ir.contains("call i64 @dartforge_string_juntar_tipado("));
                    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                }
                6 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_string_juntar_tipado(ptr %jbuf2, i64 4)"));
                    assert_eq!(format!("{m:?}"), before);
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before);
                    assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn scalar_native_call_is_trivial_reentrant_and_rejects_ref() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = true;
            let arg = if case == 1 { Operand::Val(ValueId(0)) } else { Operand::Val(ValueId(1)) };
            let ret = if case == 2 { CType::Ptr } else { CType::I64 };
            m.functions.push(Function { symbol: "nativa".into(), name: "nativa".into(), debug_info: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64),
                    (ValueId(2), Instruction::NativeCall { alvo: Operand::Constant(Constant::Int(0x1000)),
                        args: vec![(arg, CType::I64), (Operand::Constant(Constant::Double(0.5)), CType::F64)], ret },
                        if case == 3 { Type::Ref } else { Type::I64 })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
            m.dart_ref_returns.insert("nativa".into());
            let mut plans = HashMap::from([("nativa".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 0 || case == 2 {
                r.unwrap_or_else(|e| panic!("caso {case}: {e}"));
                assert_eq!(plans["nativa"].classes[&ValueId(2)], Ownership::Trivial);
                assert_eq!(plans["nativa"].tokens.instructions[&ValueId(2)], TokenEffect::default());
            } else {
                assert!(r.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn generic_owned_thunk_wraps_constructor_and_native_with_guard() {
        for case in 0..3 {
            let mut m = Module::new();
            m.arc_memory = case != 2;
            let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let i64c = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
            let insts = vec![
                (ValueId(2), rt("dartforge_typed_novo", vec![i64c(23), i64c(1), (Operand::Val(ValueId(1)), Type::I64)], Type::Ref), Type::Ref),
                (ValueId(3), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            let native = if case == 1 {
                // Um argumento sem representação no thunk: recusado.
                rt("dartforge_nativo_X_y", vec![(Operand::Val(ValueId(0)), Type::Ptr)], Type::Ref)
            } else {
                rt("dartforge_nativo_X_y", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Double(1.5)), Type::F64)], Type::Ref)
            };
            let insts2 = vec![
                (ValueId(5), native, Type::Ref),
                (ValueId(6), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(7), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(6)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "g".into(), name: "g".into(), debug_info: None,
                params: vec![(ValueId(0), "o".into(), Type::Ref), (ValueId(1), "n".into(), Type::I64)],
                return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock { id: BlockId(0), instructions: insts,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: insts2,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(7)), then_block: BlockId(3), else_block: BlockId(4) } },
                    BasicBlock { id: BlockId(3), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) },
                ] });
            m.dart_ref_returns.insert("g".into());
            m.dart_scalar_parameters.insert("g".into(), [ValueId(1)].into());
            let mut plans = HashMap::from([("g".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    for (v, block) in [(2, 1), (5, 3)] {
                        assert_eq!(plans["g"].classes[&ValueId(v)], Ownership::Owned);
                        assert_eq!(plans["g"].tokens.pendings[&ValueId(v)], BlockId(block));
                    }
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @df.arc.owned.dartforge_typed_novo(i64 23, i64 1, i64 %v1)"), "{ir}");
                    assert!(ir.contains("define internal i64 @df.arc.owned.dartforge_typed_novo(i64 %a0, i64 %a1, i64 %a2)"));
                    assert!(ir.contains("define internal i64 @df.arc.owned.dartforge_nativo_X_y(i64 %a0, double %a1)"));
                    assert!(ir.contains("declare i64 @dartforge_nativo_X_y(i64, double)"));
                    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), before);
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before);
                    assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn handle_scope_borrows_argument_and_reads_return_with_owner_before_close() {
        for case in 0..3 {
            let mut m = Module::new();
            m.arc_memory = case != 2;
            let handle = if case == 1 { (Operand::Val(ValueId(0)), Type::Ref) } else { (Operand::Val(ValueId(1)), Type::I64) };
            let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let insts = vec![
                (ValueId(2), rt("dartforge_ffi_handles_abrir", vec![], Type::I64), Type::I64),
                (ValueId(3), rt("dartforge_ffi_handle_novo", vec![(Operand::Val(ValueId(0)), Type::Ref)], Type::I64), Type::I64),
                (ValueId(4), rt("dartforge_ffi_handles_fechar_com_objeto", vec![(Operand::Val(ValueId(2)), Type::I64), handle], Type::Ref), Type::Ref),
                (ValueId(5), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(5)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "nat".into(), name: "nat".into(), debug_info: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref), (ValueId(1), "devolvido".into(), Type::I64)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(6)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))) }] });
            m.dart_ref_returns.insert("nat".into());
            m.dart_scalar_parameters.insert("nat".into(), [ValueId(1)].into());
            let mut plans = HashMap::from([("nat".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    assert_eq!(plans["nat"].classes[&ValueId(3)], Ownership::Trivial);
                    assert_eq!(plans["nat"].classes[&ValueId(4)], Ownership::Owned);
                    assert_eq!(plans["nat"].tokens.pendings[&ValueId(4)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_handles_fechar_com_objeto_owned_v1("));
                    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), before);
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before);
                    assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn ffi_return_pointer_switches_to_owned_entry_with_borrowed_closure() {
        for case in 0..3 {
            let mut m = Module::new();
            m.arc_memory = case != 2;
            let clo = if case == 1 { (Operand::Val(ValueId(1)), Type::I64) } else { (Operand::Val(ValueId(0)), Type::Ref) };
            let insts = vec![
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_de_retorno".into(),
                    args: vec![(Operand::Val(ValueId(1)), Type::I64), clo], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "ent".into(), name: "ent".into(), debug_info: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "endereco".into(), Type::I64)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.dart_ref_returns.insert("ent".into());
            m.dart_scalar_parameters.insert("ent".into(), [ValueId(1)].into());
            let mut plans = HashMap::from([("ent".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    assert_eq!(plans["ent"].classes[&ValueId(2)], Ownership::Owned);
                    assert_eq!(plans["ent"].tokens.pendings[&ValueId(2)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_ponteiro_de_retorno_owned_v1("));
                    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), before);
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before);
                    assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn ffi_callback_switches_to_owned_entries_and_checks_pointer_pending() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = case != 3;
            let i64c = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
            let mut insts = vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_ffi_callback_closure".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::I64)], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_ffi_callback_ponteiro".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::I64), i64c(0), i64c(0x1000)],
                    ret_ty: if case == 2 { Type::I64 } else { Type::Ref } }, if case == 2 { Type::I64 } else { Type::Ref }),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            if case == 1 { insts.truncate(2); }
            let terminator = if case == 1 { Terminator::Return(Some(Operand::Val(ValueId(1)))) } else {
                Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } };
            m.functions.push(Function { symbol: "cb".into(), name: "cb".into(), debug_info: None,
                params: vec![(ValueId(0), "ctx".into(), Type::I64)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts, terminator: terminator },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
            m.dart_ref_returns.insert("cb".into());
            m.dart_scalar_parameters.insert("cb".into(), [ValueId(0)].into());
            let mut plans = HashMap::from([("cb".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            match case {
                0 => {
                    r.unwrap();
                    assert_eq!(plans["cb"].classes[&ValueId(1)], Ownership::Owned);
                    assert_eq!(plans["cb"].classes[&ValueId(2)], Ownership::Owned);
                    assert_eq!(plans["cb"].tokens.pendings[&ValueId(2)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_callback_closure_owned_v1("));
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_callback_ponteiro_owned_v1("));
                    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                }
                3 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), before);
                }
                _ => {
                    assert!(r.is_err(), "caso {case}");
                    assert_eq!(format!("{m:?}"), before);
                    assert_eq!(plans, ps);
                }
            }
        }
    }

    #[test]
    fn null_phi_awaits_owned_join_and_rejects_incompatible_trivial_premise() {
        for case in 0..3 {
            let val = |n| Operand::Val(ValueId(n));
            let phi = |entries| Instruction::Phi {
                incoming: entries,
                ty: Type::Ref,
            };
            let boxed = |n| {
                if case == 1 {
                    Instruction::Const(Constant::Null)
                } else {
                    Instruction::CallRuntime {
                        name: "dartforge_arc_box_int_owned_v1".into(),
                        args: vec![(Operand::Constant(Constant::Int(n)), Type::I64)],
                        ret_ty: Type::Ref,
                    }
                }
            };
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(Function {
                symbol: "phi_misto".into(),
                name: "phi_misto".into(),
                debug_info: None,
                params: vec![
                    (ValueId(100), "externo".into(), Type::I1),
                    (ValueId(101), "interno".into(), Type::I1),
                ],
                return_ty: Type::I64,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: val(100),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: val(101),
                            then_block: BlockId(4),
                            else_block: BlockId(5),
                        },
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                    // Ordem física: o Phi externo é visitado antes do join interno.
                    BasicBlock {
                        id: BlockId(3),
                        instructions: vec![
                            (
                                ValueId(3),
                                phi(vec![
                                    (BlockId(2), Operand::Constant(Constant::Null)),
                                    (BlockId(6), val(2)),
                                ]),
                                Type::Ref,
                            ),
                            (
                                ValueId(4),
                                Instruction::CallRuntime {
                                    name: "dartforge_value_class".into(),
                                    args: vec![(val(3), Type::Ref)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            ),
                        ],
                        terminator: Terminator::Return(Some(val(4))),
                    },
                    BasicBlock {
                        id: BlockId(4),
                        instructions: vec![(ValueId(0), boxed(i64::MAX), Type::Ref)],
                        terminator: Terminator::Branch(BlockId(6)),
                    },
                    BasicBlock {
                        id: BlockId(5),
                        instructions: vec![(ValueId(1), boxed(i64::MIN), Type::Ref)],
                        terminator: Terminator::Branch(BlockId(6)),
                    },
                    BasicBlock {
                        id: BlockId(6),
                        instructions: vec![(
                            ValueId(2),
                            phi(vec![(BlockId(4), val(0)), (BlockId(5), val(1))]),
                            Type::Ref,
                        )],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                ],
            });
            let mut plan = DartFunctionPlan::default();
            if case == 2 {
                plan.classes.insert(ValueId(3), Ownership::Trivial);
            }
            let mut plans = HashMap::from([("phi_misto".into(), plan)]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 2 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
                continue;
            }
            r.unwrap();
            let expected = if case == 1 {
                Ownership::Trivial
            } else {
                Ownership::Owned
            };
            for v in [ValueId(2), ValueId(3)] {
                assert_eq!(plans["phi_misto"].classes[&v], expected);
            }
            if case == 0 {
                assert!(
                    m.functions[0]
                        .blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| matches!(
                            i,
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(3))
                            }
                        ))
                );
            }
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
        }
    }

    #[test]
    fn append_preserves_receiver_borrow_and_protects_storage_before_gc() {
        for case in 0..6 {
            let mut m = Module::new();
            m.arc_memory = case != 5;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "crescer".into(),
                name: "crescer".into(),
                debug_info: None,
                params: vec![
                    (
                        ValueId(0),
                        "lista".into(),
                        if case == 2 { Type::I64 } else { Type::Ref },
                    ),
                    (
                        ValueId(1),
                        "valor".into(),
                        if case == 3 { Type::I64 } else { Type::Ref },
                    ),
                ],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_armazenamento_ref_v1".into(),
                                args: vec![(val(0), Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_lista_acrescentar".into(),
                                args: vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                                ret_ty: if case == 4 { Type::I64 } else { Type::Ref },
                            },
                            Type::Ref,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(if case == 1 { 2 } else { 3 }))),
                }],
            });
            let contract = runtime_call_contract(&m.functions[0].blocks[0].instructions[1].1);
            if case != 4 {
                assert_eq!(
                    contract.unwrap().result,
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Value(ValueId(0)),
                        scope: 0,
                    }
                );
            } else {
                assert!(contract.is_err());
            }
            let mut plan = DartFunctionPlan::default();
            plan.tokens.ret = TokenReturn::Owned;
            let mut plans = HashMap::from([("crescer".into(), plan)]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case <= 1 {
                assert_eq!(r.unwrap(), (1, 0));
                let insts = &m.functions[0].blocks[0].instructions;
                let copy_inst = insts
                    .iter()
                    .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                    .unwrap();
                let call = insts
                    .iter()
                    .position(|(_, i, _)| {
                        matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_lista_acrescentar")
                    })
                    .unwrap();
                assert_eq!(copy_inst < call, case == 1);
                let Terminator::Return(Some(Operand::Val(ret))) =
                    m.functions[0].blocks[0].terminator
                else {
                    panic!("retorno SSA esperado");
                };
                assert_eq!(plans["crescer"].classes[&ret], Ownership::Owned);
                if case == 0 {
                    assert_ne!(ret, ValueId(3));
                    assert!(matches!(
                        plans["crescer"].classes[&ValueId(3)],
                        Ownership::Borrowed { .. }
                    ));
                }
                let prepared = format!("{m:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), prepared);
            } else {
                if case == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn ref_storage_gets_owner_before_mutation_and_rejects_native_bits() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.arc_memory = true;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "armazenamento".into(),
                name: "armazenamento".into(),
                debug_info: None,
                params: vec![(ValueId(0), "lista".into(), ty)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_armazenamento_ref_v1".into(),
                                args: vec![(val(0), Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_forma_v1".into(),
                                args: vec![(val(1), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_gravar_len_v1".into(),
                                args: vec![
                                    (val(0), Type::Ref),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(1))),
                }],
            });
            let mut plans = HashMap::from([("armazenamento".into(), DartFunctionPlan::default())]);
            plans.get_mut("armazenamento").unwrap().tokens.ret = TokenReturn::Owned;
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if ty == Type::I64 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
                continue;
            }
            assert_eq!(r.unwrap().0, 1);
            let insts = &m.functions[0].blocks[0].instructions;
            let copy_inst = insts
                .iter()
                .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                .unwrap();
            let mutation = insts.iter().position(|(_, i, _)| matches!(i,
                Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_gravar_len_v1")).unwrap();
            assert!(copy_inst < mutation);
            assert_eq!(
                plans["armazenamento"].classes[&insts[copy_inst].0],
                Ownership::Owned
            );
            let prepared = format!("{m:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}"), prepared);
        }
    }
    #[test]
    fn length_borrows_ref_and_rejects_invalid_abi_without_changing_tracing() {
        for case in 0..6 {
            let mut m = Module::new();
            m.arc_memory = case != 5;
            m.functions.push(Function {
                symbol: "comprimento".into(),
                name: "comprimento".into(),
                debug_info: None,
                params: vec![(
                    ValueId(0),
                    "lista".into(),
                    if case == 1 { Type::I64 } else { Type::Ref },
                )],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "df.lista_len".into(),
                            args: if case == 2 {
                                vec![]
                            } else {
                                vec![(
                                    Operand::Val(ValueId(0)),
                                    if case == 3 { Type::I64 } else { Type::Ref },
                                )]
                            },
                            ret_ty: if case == 4 { Type::Ref } else { Type::I64 },
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut plans = HashMap::from([("comprimento".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 0 {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(
                    plans["comprimento"].classes[&ValueId(1)],
                    Ownership::Trivial
                );
                assert!(plans["comprimento"].tokens.pendings.is_empty());
                assert!(
                    matches!(&m.functions[0].blocks[0].instructions[0].1,                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_len_v1")
                );
                let prepared = format!("{m:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), prepared);
            } else {
                if case == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn class_query_borrows_ref_without_creating_token_or_accepting_native_bits() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(Function {
                symbol: "classe".into(),
                name: "classe".into(),
                debug_info: None,
                params: vec![(ValueId(0), "valor".into(), ty)],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_value_class".into(),
                            args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut plans = HashMap::from([("classe".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if ty == Type::Ref {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(plans["classe"].classes[&ValueId(1)], Ownership::Trivial);
                assert!(plans["classe"].tokens.pendings.is_empty());
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
            } else {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }

    #[test]
    fn owned_reception_preserves_pending_without_promoting_invalid_abi_or_tracing() {
        for case in 0..5 {
            let mut m = Module::new();
            m.arc_memory = case != 4;
            m.functions.push(Function {
                symbol: "receber".into(),
                name: "receber".into(),
                debug_info: None,
                params: vec![(
                    ValueId(0),
                    "socket".into(),
                    if case == 1 { Type::I64 } else { Type::Ref },
                )],
                return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: "dartforge_nativo_Socket_RecvFrom".into(),
                                    args: if case == 2 {
                                        vec![]
                                    } else {
                                        vec![(Operand::Val(ValueId(0)), Type::Ref)]
                                    },
                                    ret_ty: if case == 3 { Type::I64 } else { Type::Ref },
                                },
                                Type::Ref,
                            ),
                            (
                                ValueId(2),
                                Instruction::CallRuntime {
                                    name: "dartforge_exception_pending".into(),
                                    args: vec![],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            ),
                            (
                                ValueId(3),
                                Instruction::ICmp(
                                    ICmpOp::Ne,
                                    Operand::Val(ValueId(2)),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            });
            let mut p = DartFunctionPlan::default();
            p.tokens.ret = TokenReturn::Owned;
            let mut plans = HashMap::from([("receber".into(), p)]);
            let before = format!("{m:?}");
            let ps = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 0 {
                r.unwrap();
                assert_eq!(plans["receber"].classes[&ValueId(1)], Ownership::Owned);
                assert!(
                    plans["receber"]
                        .tokens
                        .pendings
                        .contains_key(&ValueId(1))
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_socket_recvfrom_owned_v1"));
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
            } else {
                if case == 4 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err(), "caso {case}");
                }
                assert_eq!(format!("{m:?}"), before);
                assert_eq!(plans, ps);
            }
        }
    }
}
