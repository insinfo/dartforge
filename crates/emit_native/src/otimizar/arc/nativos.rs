//! Seleciona entradas auditadas para operações nativas ARC.
//! Os contratos conferem SSA, pendência e callbacks na preparação completa.

use crate::hir::*;

pub(super) fn preparar(f: &mut Function) -> Result<(), String> {
    // O tear-off canônico, com owner do código (`dartforge_arc_tearoff_owned_v1`).
    for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::TearOff { code_symbol } = inst {
            if *ty != Type::Ref {
                return Err(format!("tear-off ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            *inst = Instruction::CallRuntime {
                name: "dartforge_arc_tearoff_owned_v1".into(),
                args: vec![(Operand::Constant(Constant::Funcao(std::mem::take(code_symbol))), Type::I64)],
                ret_ty: Type::Ref,
            };
        }
    }
    for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        let Instruction::CallRuntime { name, args, ret_ty } = inst else {
            continue;
        };
        // A exceção e o rastro pendentes com owner independente do `clear`.
        let pendente = match name.as_str() {
            "dartforge_exception_peek_ref" => Some("dartforge_arc_excecao_owned_v1"),
            "dartforge_stack_trace_get" => Some("dartforge_arc_rastro_owned_v1"),
            _ => None,
        };
        if let Some(owned) = pendente {
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
            let [(objeto, Type::Ref), (indice, Type::I64), (valor, Type::I64),
                (Operand::Constant(Constant::Int(marca @ (0 | 1))), Type::I8)] = args.as_slice()
            else {
                return Err(format!("gravação de campo ARC em {} v{}: ABI ou marca incompatível", f.symbol, v.0));
            };
            let (nome, repr) = if *marca == 1 {
                ("dartforge_arc_gravar_campo_ref_v1", Type::Ref)
            } else {
                ("dartforge_arc_gravar_campo_escalar_v1", Type::I64)
            };
            *args = vec![(objeto.clone(), Type::Ref), (indice.clone(), Type::I64), (valor.clone(), repr)];
            *name = nome.into();
            continue;
        }
        // A string nova (`@df.texto_alocar`) com owner do código.
        if name == "df.texto_alocar" {
            let [(len, Type::I64), (Operand::Constant(Constant::Bool(dois)), Type::I1)] = args.as_slice() else {
                return Err(format!("alocação de texto ARC em {} v{}: ABI incompatível", f.symbol, v.0));
            };
            if *ty != Type::Ref || *ret_ty != Type::Ref {
                return Err(format!("alocação de texto ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            *args = vec![(len.clone(), Type::I64), (Operand::Constant(Constant::Int(i64::from(*dois))), Type::I64)];
            *name = "dartforge_arc_texto_novo_owned_v1".into();
            continue;
        }
        // Os construtores de erro do runtime: a variante Owned (o par
        // bits/é-referência vira o valor `Ref`).
        if let Some(base) = name.strip_prefix("dartforge_")
            && ERROS_COM_VARIANTE_ARC.contains(&base)
        {
            if *ty != Type::Ref || *ret_ty != Type::Ref {
                return Err(format!("erro do runtime ARC em {} v{}: resultado não Ref", f.symbol, v.0));
            }
            if matches!(base, "exception_new" | "assertion_error_new" | "argument_error_value") {
                let ref_valida = |o: &Operand| matches!(o, Operand::Val(_) | Operand::Constant(Constant::Null));
                if args.len() < 2 || !ref_valida(&args[0].0) || !matches!(args[1].0, Operand::Constant(Constant::Int(1))) {
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
        if *ret_ty == Type::Ref && owned_generico(name) && super::contrato(name).is_err() {
            if *ty != Type::Ref || args.iter().any(|(_, t)| !matches!(t, Type::Ref | Type::I64 | Type::F64 | Type::I8 | Type::I1 | Type::Ptr)) {
                return Err(format!("thunk Owned em {} v{}: ABI incompatível", f.symbol, v.0));
            }
            *name = format!("{}{name}", super::PREFIXO_OWNED_GENERICO);
        }
    }
    Ok(())
}

/// Os construtores de erro do runtime com variante Owned (o nome sem o
/// prefixo `dartforge_`; `lower/erros_do_runtime.rs` os emite).
pub(crate) const ERROS_COM_VARIANTE_ARC: &[&str] = &[
    "exception_new", "assertion_error_new", "state_error_new", "unsupported_error_new",
    "unimplemented_error_new", "argument_error_new", "argument_error_value",
    "argument_error_not_null", "range_error_range", "type_error_new", "null_check_error_new",
    "late_error_new", "stack_overflow_error_new", "no_such_method_error_new",
];

/// As funções do runtime cujo resultado `Ref` está vivo ao voltar, sem owner
/// do código e sem consumir argumentos: os natives do SDK e os construtores
/// do runtime chamados pelo lowering. Leitura de campo, alocação de objeto e
/// as de endereço nativo têm produtores próprios.
fn owned_generico(nome: &str) -> bool {
    nome.starts_with("dartforge_nativo_")
        || matches!(
            nome,
            "dartforge_collection_mark_unmodifiable"
                | "dartforge_constante_canonica"
                | "dartforge_encaminhar_nsm"
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
mod testes {
    use crate::otimizar::arc::*;
    use std::collections::HashMap;

    #[test]
    fn seletor_local_exige_refs_descritor_rti_e_pending_sem_alterar_tracing() {
        for caso in 0..9 {
            let mut m = Module::new();
            m.memoria_arc = caso != 8;
            let mut insts = vec![
                (ValueId(2), Instruction::CallSeletor { seletor: "tc:start".into(),
                    recv: Operand::Val(ValueId(0)), args: vec![Operand::Val(ValueId(1))],
                    nomes: if caso == 3 { vec!["x".into(), "x".into()] } else { vec![] },
                    tupla_tipos: Operand::Constant(Constant::Int(if caso == 4 { 7 } else { 0 })) },
                    if caso == 5 { Type::I64 } else { Type::Ref }),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            if caso == 6 { insts.pop(); }
            if caso == 7 && let Instruction::CallSeletor { args, .. } = &mut insts[0].1 {
                args[0] = Operand::Val(ValueId(99));
            }
            m.functions.push(Function { symbol: "seletor".into(), name: "seletor".into(), depuracao: None,
                params: vec![(ValueId(0), "recv".into(), if caso == 1 { Type::I64 } else { Type::Ref }),
                    (ValueId(1), "arg".into(), if caso == 2 { Type::I64 } else { Type::Ref })], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                    terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.retornos_ref_dart.insert("seletor".into());
            let mut planos = HashMap::from([("seletor".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}"); let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 {
                r.unwrap();
                assert_eq!(planos["seletor"].classes[&ValueId(2)], Ownership::Owned);
                assert_eq!(planos["seletor"].tokens.pendencias[&ValueId(2)], BlockId(1));
                certificar_portas_arc_aot(&mut m, &planos).unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc2"));
                assert!(!ir.contains("call i64 %sf2("));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                if caso == 8 {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert!(crate::llvm::LlvmEmitter::new(&m).emit_all().contains("call i64 %sf2("));
                } else { assert!(r.is_err(), "caso {caso}"); }
                assert_eq!(format!("{m:?}"), antes); assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn seletor_aceita_tupla_de_getter_rti_comprovado_e_recusa_constante() {
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = caso != 3;
            m.globais.push((0, Type::I64, "cache".into()));
            m.caches_rti_arc.insert("cache".into());
            let retorno = if caso == 1 { ValueId(9) } else { ValueId(1) };
            m.functions.push(Function { symbol: "getter".into(), name: "getter".into(), depuracao: None,
                params: vec![], return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::Const(Constant::String("C201".into())), Type::Ref),
                    (ValueId(1), Instruction::CallRuntime { name: "dartforge_rti_receita".into(),
                        args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::StoreGlobal { simbolo: "cache".into(), val: Operand::Val(ValueId(1)),
                        ty: Type::I64, raiz: None }, Type::Void),
                    (ValueId(3), Instruction::StoreGlobal { simbolo: "cache$ok".into(),
                        val: Operand::Constant(Constant::Int(1)), ty: Type::I8, raiz: None }, Type::Void),
                    (ValueId(9), Instruction::Const(Constant::Int(42)), Type::I64)],
                    terminator: Terminator::Return(Some(Operand::Val(retorno))) }] });
            let getter_args = if caso == 2 { vec![Operand::Constant(Constant::Int(0))] } else { vec![] };
            m.functions.push(Function { symbol: "seletor".into(), name: "seletor".into(), depuracao: None,
                params: vec![(ValueId(0), "recv".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::CallStatic { symbol: "getter".into(), args: getter_args, ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::CallSeletor { seletor: "tc:fold".into(), recv: Operand::Val(ValueId(0)),
                        args: vec![], nomes: vec![], tupla_tipos: Operand::Val(ValueId(1)) }, Type::Ref),
                    (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1)],
                    terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.retornos_ref_dart.insert("seletor".into());
            let mut planos = HashMap::from([("getter".into(), PlanoFuncaoDart::default()), ("seletor".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}"); let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    assert_eq!(planos["seletor"].classes[&ValueId(2)], Ownership::Owned);
                    certificar_portas_arc_aot(&mut m, &planos).unwrap();
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc2"));
                }
                3 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert!(crate::llvm::LlvmEmitter::new(&m).emit_all().contains("call i64 %sf2("));
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes); assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn literal_produz_owner_por_tipos_reais_e_recusa_abi_incompativel() {
        for caso in 0..8 {
            let mut m = Module::new();
            m.memoria_arc = caso != 7;
            let t = match caso { 2 => Type::Ptr, 3 => Type::V4I32, 4 => Type::I8, _ => Type::Ref };
            let mut elements = if caso == 0 { vec![] } else { vec![(Operand::Val(ValueId(0)), 255),
                (Operand::Constant(Constant::Int(i64::MAX)), 255),
                (Operand::Constant(Constant::Double(-0.0)), 255),
                (Operand::Constant(Constant::Bool(true)), 255)] };
            if caso == 5 { elements.push((Operand::Constant(Constant::Funcao("alvo".into())), 1)); }
            m.functions.push(Function { symbol: "literal".into(), name: "literal".into(), depuracao: None,
                params: vec![(ValueId(0), "elemento".into(), t)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1),
                    Instruction::AllocList { elements }, if caso == 6 { Type::I64 } else { Type::Ref })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("literal".into(), p)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso < 2 {
                r.unwrap();
                assert_eq!(planos["literal"].classes[&ValueId(1)], Ownership::Owned);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_lista_nova_owned_v1(ptr %list_buf_1"));
                if caso == 1 { assert!(ir.contains("store i8 3") && ir.contains("store i8 4")); }
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                if caso == 7 {
                    assert_eq!(r.unwrap(), (0, 0));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_lista_nova(ptr %list_buf_1"));
                } else { assert!(r.is_err(), "caso {caso}"); }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn interpolacao_produz_owner_por_tipos_reais_e_recusa_partes_sem_abi() {
        for caso in 0..7 {
            let mut m = Module::new();
            m.memoria_arc = caso != 6;
            let t = match caso { 2 => Type::F64, 3 => Type::I1, _ => Type::Ref };
            let mut partes = vec![Operand::Val(ValueId(0)), Operand::Constant(Constant::Int(i64::MIN)),
                Operand::Val(ValueId(1)), Operand::Val(ValueId(3))];
            if caso == 4 { partes.push(Operand::Constant(Constant::Null)); }
            if caso == 5 { partes.push(Operand::Constant(Constant::StringWtf8(b" fim".to_vec()))); }
            m.functions.push(Function { symbol: "juntar".into(), name: "juntar".into(), depuracao: None,
                params: vec![(ValueId(0), "texto".into(), t)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Const(Constant::Int(42)), Type::I64),
                    (ValueId(3), Instruction::Const(Constant::StringWtf8(b" fim".to_vec())), Type::Ref),
                    (ValueId(2), Instruction::JuntarTextos { partes }, if caso == 1 { Type::I64 } else { Type::Ref })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("juntar".into(), p)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    assert_eq!(planos["juntar"].classes[&ValueId(2)], Ownership::Owned);
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_string_juntar_owned_v1(ptr %jbuf2, i64 4)"));
                    assert!(!ir.contains("call i64 @dartforge_string_juntar_tipado("));
                    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                }
                6 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_string_juntar_tipado(ptr %jbuf2, i64 4)"));
                    assert_eq!(format!("{m:?}"), antes);
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes);
                    assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn chamada_nativa_escalar_e_trivial_reentrante_e_recusa_ref() {
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = true;
            let arg = if caso == 1 { Operand::Val(ValueId(0)) } else { Operand::Val(ValueId(1)) };
            let ret = if caso == 2 { TipoC::Ptr } else { TipoC::I64 };
            m.functions.push(Function { symbol: "nativa".into(), name: "nativa".into(), depuracao: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64),
                    (ValueId(2), Instruction::ChamadaNativa { alvo: Operand::Constant(Constant::Int(0x1000)),
                        args: vec![(arg, TipoC::I64), (Operand::Constant(Constant::Double(0.5)), TipoC::F64)], ret },
                        if caso == 3 { Type::Ref } else { Type::I64 })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
            m.retornos_ref_dart.insert("nativa".into());
            let mut planos = HashMap::from([("nativa".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 || caso == 2 {
                r.unwrap_or_else(|e| panic!("caso {caso}: {e}"));
                assert_eq!(planos["nativa"].classes[&ValueId(2)], Ownership::Trivial);
                assert_eq!(planos["nativa"].tokens.instrucoes[&ValueId(2)], EfeitoTokens::default());
            } else {
                assert!(r.is_err(), "caso {caso}");
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn thunk_owned_generico_envolve_construtor_e_native_com_guarda() {
        for caso in 0..3 {
            let mut m = Module::new();
            m.memoria_arc = caso != 2;
            let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let i64c = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
            let insts = vec![
                (ValueId(2), rt("dartforge_typed_novo", vec![i64c(23), i64c(1), (Operand::Val(ValueId(1)), Type::I64)], Type::Ref), Type::Ref),
                (ValueId(3), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            let nativo = if caso == 1 {
                // Um argumento sem representação no thunk: recusado.
                rt("dartforge_nativo_X_y", vec![(Operand::Val(ValueId(0)), Type::Ptr)], Type::Ref)
            } else {
                rt("dartforge_nativo_X_y", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Double(1.5)), Type::F64)], Type::Ref)
            };
            let insts2 = vec![
                (ValueId(5), nativo, Type::Ref),
                (ValueId(6), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(7), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(6)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "g".into(), name: "g".into(), depuracao: None,
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
            m.retornos_ref_dart.insert("g".into());
            m.parametros_escalares_dart.insert("g".into(), [ValueId(1)].into());
            let mut planos = HashMap::from([("g".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    for (v, bloco) in [(2, 1), (5, 3)] {
                        assert_eq!(planos["g"].classes[&ValueId(v)], Ownership::Owned);
                        assert_eq!(planos["g"].tokens.pendencias[&ValueId(v)], BlockId(bloco));
                    }
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @df.arc.owned.dartforge_typed_novo(i64 23, i64 1, i64 %v1)"), "{ir}");
                    assert!(ir.contains("define internal i64 @df.arc.owned.dartforge_typed_novo(i64 %a0, i64 %a1, i64 %a2)"));
                    assert!(ir.contains("define internal i64 @df.arc.owned.dartforge_nativo_X_y(i64 %a0, double %a1)"));
                    assert!(ir.contains("declare i64 @dartforge_nativo_X_y(i64, double)"));
                    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), antes);
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes);
                    assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn escopo_de_handles_empresta_argumento_e_le_retorno_com_owner_antes_de_fechar() {
        for caso in 0..3 {
            let mut m = Module::new();
            m.memoria_arc = caso != 2;
            let handle = if caso == 1 { (Operand::Val(ValueId(0)), Type::Ref) } else { (Operand::Val(ValueId(1)), Type::I64) };
            let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let insts = vec![
                (ValueId(2), rt("dartforge_ffi_handles_abrir", vec![], Type::I64), Type::I64),
                (ValueId(3), rt("dartforge_ffi_handle_novo", vec![(Operand::Val(ValueId(0)), Type::Ref)], Type::I64), Type::I64),
                (ValueId(4), rt("dartforge_ffi_handles_fechar_com_objeto", vec![(Operand::Val(ValueId(2)), Type::I64), handle], Type::Ref), Type::Ref),
                (ValueId(5), rt("dartforge_exception_pending", vec![], Type::I8), Type::I8),
                (ValueId(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(5)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "nat".into(), name: "nat".into(), depuracao: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref), (ValueId(1), "devolvido".into(), Type::I64)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(6)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))) }] });
            m.retornos_ref_dart.insert("nat".into());
            m.parametros_escalares_dart.insert("nat".into(), [ValueId(1)].into());
            let mut planos = HashMap::from([("nat".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    assert_eq!(planos["nat"].classes[&ValueId(3)], Ownership::Trivial);
                    assert_eq!(planos["nat"].classes[&ValueId(4)], Ownership::Owned);
                    assert_eq!(planos["nat"].tokens.pendencias[&ValueId(4)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_handles_fechar_com_objeto_owned_v1("));
                    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), antes);
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes);
                    assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn ponteiro_de_retorno_ffi_troca_por_entrada_owned_com_closure_emprestada() {
        for caso in 0..3 {
            let mut m = Module::new();
            m.memoria_arc = caso != 2;
            let clo = if caso == 1 { (Operand::Val(ValueId(1)), Type::I64) } else { (Operand::Val(ValueId(0)), Type::Ref) };
            let insts = vec![
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_de_retorno".into(),
                    args: vec![(Operand::Val(ValueId(1)), Type::I64), clo], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            m.functions.push(Function { symbol: "ent".into(), name: "ent".into(), depuracao: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "endereco".into(), Type::I64)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts,
                        terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.retornos_ref_dart.insert("ent".into());
            m.parametros_escalares_dart.insert("ent".into(), [ValueId(1)].into());
            let mut planos = HashMap::from([("ent".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    assert_eq!(planos["ent"].classes[&ValueId(2)], Ownership::Owned);
                    assert_eq!(planos["ent"].tokens.pendencias[&ValueId(2)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_ponteiro_de_retorno_owned_v1("));
                    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                }
                2 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), antes);
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes);
                    assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn callback_ffi_troca_por_entradas_owned_e_confere_pendencia_do_ponteiro() {
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = caso != 3;
            let i64c = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
            let mut insts = vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_ffi_callback_closure".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::I64)], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_ffi_callback_ponteiro".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::I64), i64c(0), i64c(0x1000)],
                    ret_ty: if caso == 2 { Type::I64 } else { Type::Ref } }, if caso == 2 { Type::I64 } else { Type::Ref }),
                (ValueId(3), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0))), Type::I1),
            ];
            if caso == 1 { insts.truncate(2); }
            let terminador = if caso == 1 { Terminator::Return(Some(Operand::Val(ValueId(1)))) } else {
                Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2) } };
            m.functions.push(Function { symbol: "cb".into(), name: "cb".into(), depuracao: None,
                params: vec![(ValueId(0), "ctx".into(), Type::I64)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: insts, terminator: terminador },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
            m.retornos_ref_dart.insert("cb".into());
            m.parametros_escalares_dart.insert("cb".into(), [ValueId(0)].into());
            let mut planos = HashMap::from([("cb".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            match caso {
                0 => {
                    r.unwrap();
                    assert_eq!(planos["cb"].classes[&ValueId(1)], Ownership::Owned);
                    assert_eq!(planos["cb"].classes[&ValueId(2)], Ownership::Owned);
                    assert_eq!(planos["cb"].tokens.pendencias[&ValueId(2)], BlockId(1));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_callback_closure_owned_v1("));
                    assert!(ir.contains("call i64 @dartforge_arc_ffi_callback_ponteiro_owned_v1("));
                    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                }
                3 => {
                    assert_eq!(r.unwrap(), (0, 0));
                    assert_eq!(format!("{m:?}"), antes);
                }
                _ => {
                    assert!(r.is_err(), "caso {caso}");
                    assert_eq!(format!("{m:?}"), antes);
                    assert_eq!(planos, ps);
                }
            }
        }
    }

    #[test]
    fn phi_null_aguarda_join_owned_e_recusa_premissa_trivial_incompativel() {
        for caso in 0..3 {
            let val = |n| Operand::Val(ValueId(n));
            let phi = |entradas| Instruction::Phi {
                incoming: entradas,
                ty: Type::Ref,
            };
            let caixa = |n| {
                if caso == 1 {
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
            m.memoria_arc = true;
            m.functions.push(Function {
                symbol: "phi_misto".into(),
                name: "phi_misto".into(),
                depuracao: None,
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
                        instructions: vec![(ValueId(0), caixa(i64::MAX), Type::Ref)],
                        terminator: Terminator::Branch(BlockId(6)),
                    },
                    BasicBlock {
                        id: BlockId(5),
                        instructions: vec![(ValueId(1), caixa(i64::MIN), Type::Ref)],
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
            let mut plano = PlanoFuncaoDart::default();
            if caso == 2 {
                plano.classes.insert(ValueId(3), Ownership::Trivial);
            }
            let mut planos = HashMap::from([("phi_misto".into(), plano)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 2 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
                continue;
            }
            r.unwrap();
            let esperada = if caso == 1 {
                Ownership::Trivial
            } else {
                Ownership::Owned
            };
            for v in [ValueId(2), ValueId(3)] {
                assert_eq!(planos["phi_misto"].classes[&v], esperada);
            }
            if caso == 0 {
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
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
        }
    }

    #[test]
    fn acrescentar_preserva_borrow_do_receiver_e_protege_storage_anterior_ao_gc() {
        for caso in 0..6 {
            let mut m = Module::new();
            m.memoria_arc = caso != 5;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "crescer".into(),
                name: "crescer".into(),
                depuracao: None,
                params: vec![
                    (
                        ValueId(0),
                        "lista".into(),
                        if caso == 2 { Type::I64 } else { Type::Ref },
                    ),
                    (
                        ValueId(1),
                        "valor".into(),
                        if caso == 3 { Type::I64 } else { Type::Ref },
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
                                ret_ty: if caso == 4 { Type::I64 } else { Type::Ref },
                            },
                            Type::Ref,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(if caso == 1 { 2 } else { 3 }))),
                }],
            });
            let contrato = contrato_chamada_runtime(&m.functions[0].blocks[0].instructions[1].1);
            if caso != 4 {
                assert_eq!(
                    contrato.unwrap().resultado,
                    Ownership::Borrowed {
                        owner: OrigemOwner::Valor(ValueId(0)),
                        escopo: 0,
                    }
                );
            } else {
                assert!(contrato.is_err());
            }
            let mut plano = PlanoFuncaoDart::default();
            plano.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("crescer".into(), plano)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso <= 1 {
                assert_eq!(r.unwrap(), (1, 0));
                let insts = &m.functions[0].blocks[0].instructions;
                let copia = insts
                    .iter()
                    .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                    .unwrap();
                let chamada = insts
                    .iter()
                    .position(|(_, i, _)| {
                        matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_lista_acrescentar")
                    })
                    .unwrap();
                assert_eq!(copia < chamada, caso == 1);
                let Terminator::Return(Some(Operand::Val(retorno))) =
                    m.functions[0].blocks[0].terminator
                else {
                    panic!("retorno SSA esperado");
                };
                assert_eq!(planos["crescer"].classes[&retorno], Ownership::Owned);
                if caso == 0 {
                    assert_ne!(retorno, ValueId(3));
                    assert!(matches!(
                        planos["crescer"].classes[&ValueId(3)],
                        Ownership::Borrowed { .. }
                    ));
                }
                let preparado = format!("{m:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), preparado);
            } else {
                if caso == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn armazenamento_ref_recebe_owner_antes_de_mutacao_e_recusa_bits_nativos() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.memoria_arc = true;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "armazenamento".into(),
                name: "armazenamento".into(),
                depuracao: None,
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
            let mut planos = HashMap::from([("armazenamento".into(), PlanoFuncaoDart::default())]);
            planos.get_mut("armazenamento").unwrap().tokens.retorno = RetornoTokens::Owned;
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if ty == Type::I64 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
                continue;
            }
            assert_eq!(r.unwrap().0, 1);
            let insts = &m.functions[0].blocks[0].instructions;
            let copia = insts
                .iter()
                .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                .unwrap();
            let mutacao = insts.iter().position(|(_, i, _)| matches!(i,
                Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_gravar_len_v1")).unwrap();
            assert!(copia < mutacao);
            assert_eq!(
                planos["armazenamento"].classes[&insts[copia].0],
                Ownership::Owned
            );
            let preparado = format!("{m:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}"), preparado);
        }
    }
    #[test]
    fn comprimento_empresta_ref_e_recusa_abi_invalida_sem_alterar_tracing() {
        for caso in 0..6 {
            let mut m = Module::new();
            m.memoria_arc = caso != 5;
            m.functions.push(Function {
                symbol: "comprimento".into(),
                name: "comprimento".into(),
                depuracao: None,
                params: vec![(
                    ValueId(0),
                    "lista".into(),
                    if caso == 1 { Type::I64 } else { Type::Ref },
                )],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "df.lista_len".into(),
                            args: if caso == 2 {
                                vec![]
                            } else {
                                vec![(
                                    Operand::Val(ValueId(0)),
                                    if caso == 3 { Type::I64 } else { Type::Ref },
                                )]
                            },
                            ret_ty: if caso == 4 { Type::Ref } else { Type::I64 },
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut planos = HashMap::from([("comprimento".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(
                    planos["comprimento"].classes[&ValueId(1)],
                    Ownership::Trivial
                );
                assert!(planos["comprimento"].tokens.pendencias.is_empty());
                assert!(
                    matches!(&m.functions[0].blocks[0].instructions[0].1,                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_len_v1")
                );
                let preparado = format!("{m:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), preparado);
            } else {
                if caso == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn consulta_de_classe_empresta_ref_sem_criar_token_ou_aceitar_bits_nativos() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(Function {
                symbol: "classe".into(),
                name: "classe".into(),
                depuracao: None,
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
            let mut planos = HashMap::from([("classe".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if ty == Type::Ref {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(planos["classe"].classes[&ValueId(1)], Ownership::Trivial);
                assert!(planos["classe"].tokens.pendencias.is_empty());
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            } else {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn recepcao_owned_preserva_pending_sem_promover_abi_invalida_ou_tracing() {
        for caso in 0..5 {
            let mut m = Module::new();
            m.memoria_arc = caso != 4;
            m.functions.push(Function {
                symbol: "receber".into(),
                name: "receber".into(),
                depuracao: None,
                params: vec![(
                    ValueId(0),
                    "socket".into(),
                    if caso == 1 { Type::I64 } else { Type::Ref },
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
                                    args: if caso == 2 {
                                        vec![]
                                    } else {
                                        vec![(Operand::Val(ValueId(0)), Type::Ref)]
                                    },
                                    ret_ty: if caso == 3 { Type::I64 } else { Type::Ref },
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
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("receber".into(), p)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 {
                r.unwrap();
                assert_eq!(planos["receber"].classes[&ValueId(1)], Ownership::Owned);
                assert!(
                    planos["receber"]
                        .tokens
                        .pendencias
                        .contains_key(&ValueId(1))
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_socket_recvfrom_owned_v1"));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            } else {
                if caso == 4 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err(), "caso {caso}");
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }
}
