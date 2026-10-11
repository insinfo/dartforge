//! Prova AOT de keepalive produzido antes de substituir uma aresta de campo.
//! A HIR verificada lê BorrowArg e devolve Owned após retirar a aresta forte.
//! O harness LLVM observa os endereços sem raízes, depois que a função saiu.
//! Em tracing, o mesmo corpo com tokens explícitos testa compatibilidade do
//! protocolo runtime; não ativa preparação ARC no pipeline padrão tracing.
//! Não certifica lowering de fonte, guardas pending, recarga ou suspensão.
//! Controles negativos `sem-retencao` e `sem-liberar` alteram somente o IR
//! final do harness, depois da conferência; a execução deve falhar.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo(captura: bool, vizinho: bool, argumento: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_keepalive".into());
    let mut layout = vec![Type::Ref; 40];
    layout[..4].copy_from_slice(&[Type::I64, Type::F64, Type::I1, Type::I8]);
    m.layouts_campos_arc.insert(123, layout.clone());
    if captura { m.layouts_campos_arc.insert(dartforge_runtime::layout::cid::CONTEXTO as u32, layout); }
    for id in m.layouts_campos_arc.keys() {
        m.classes.push(ClassDef { id: *id, name: format!("C{id}"), field_count: 40,
            vtable: vec![], to_string_symbol: None });
    }
    let val = |n| Operand::Val(ValueId(n));
    m.functions.push(Function {
        symbol: "ler_e_substituir".into(),
        name: "ler_e_substituir".into(),
        depuracao: None,
        params: vec![(ValueId(0), "receiver".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::GetField { object: val(0), index: 39 },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::SetField { object: val(0), index: 39,
                        value: Operand::Constant(Constant::Null) },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(val(1))),
        }],
    });
    if captura {
        // Exercita a tradução EnvGet e o mapa real do ambiente, inclusive
        // o campo 39, antes de substituir a aresta que sustenta o empréstimo.
        m.functions[0].blocks[0].instructions[0].1 = Instruction::EnvGet {
            env: val(0), index: 39,
        };
    }
    if vizinho {
        // O filho depende do vizinho e este da raiz. Retirar a aresta da
        // raiz deve preservar ambos até terminar a mutação e devolver o filho.
        m.functions[0].blocks[0].instructions = vec![
            (ValueId(1), Instruction::GetField { object: val(0), index: 39 }, Type::Ref),
            (ValueId(2), Instruction::GetField { object: val(1), index: 39 }, Type::Ref),
            (ValueId(3), Instruction::SetField { object: val(0), index: 39,
                value: Operand::Constant(Constant::Null) }, Type::Void),
            (ValueId(4), Instruction::SetField { object: val(1), index: 39,
                value: Operand::Constant(Constant::Null) }, Type::Void),
        ];
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(val(2)));
        if captura {
            m.functions[0].blocks[0].instructions[0].1 = Instruction::EnvGet { env: val(0), index: 39 };
        }
    }
    if argumento {
        let f = &mut m.functions[0];
        f.params.extend([(ValueId(100), "args".into(), Type::Ptr), (ValueId(101), "desc".into(), Type::Ptr)]);
        for (_, i, _) in &mut f.blocks[0].instructions {
            if let Instruction::GetField { object, .. } | Instruction::SetField { object, .. } = i { *object = val(9); }
        }
        f.blocks[0].instructions.insert(0, (ValueId(9), Instruction::LoadIndexed {
            base: val(100), index: Operand::Constant(Constant::Int(0)),
        }, Type::Ref));
        m.parametros_vetores_dart.insert("ler_e_substituir".into(), HashMap::from([
            (ValueId(100), VetorEntradaDart::Argumentos), (ValueId(101), VetorEntradaDart::Descritor)]));
    }
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    m.functions.push(Function {
        symbol: "inicializar".into(), name: "inicializar".into(), depuracao: None,
        params: vec![(ValueId(0), "this".into(), Type::Ref), (ValueId(1), "filho".into(), Type::Ref)],
        return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0),
            instructions: vec![
                (ValueId(2), Instruction::SetField { object: val(0), index: 39, value: val(1) }, Type::Void),
                (ValueId(3), Instruction::Trunc { op: Operand::Constant(Constant::Int(255)), from: Type::I64, to: Type::I8 }, Type::I8),
                (ValueId(4), Instruction::SetField { object: val(0), index: 0, value: Operand::Constant(Constant::Int(42)) }, Type::Void),
                (ValueId(5), Instruction::SetField { object: val(0), index: 1, value: Operand::Constant(Constant::Double(-0.0)) }, Type::Void),
                (ValueId(6), Instruction::SetField { object: val(0), index: 2, value: Operand::Constant(Constant::Bool(true)) }, Type::Void),
                (ValueId(7), Instruction::SetField { object: val(0), index: 3, value: val(3) }, Type::Void),
            ],
            terminator: Terminator::Return(None) }],
    });
    m.functions.push(Function {
        symbol: "ler_inteiro".into(), name: "ler_inteiro".into(), depuracao: None,
        params: vec![(ValueId(0), "this".into(), Type::Ref)], return_ty: Type::I64,
        blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(1), Instruction::GetField { object: val(0), index: 0 }, Type::I64),
        ], terminator: Terminator::Return(Some(val(1))) }],
    });
    let mut planos = HashMap::from([("ler_e_substituir".into(), plano),
        ("inicializar".into(), PlanoFuncaoDart::default()), ("ler_inteiro".into(), PlanoFuncaoDart::default())]);
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    let esperado = if vizinho { (2, 1) } else if argumento { (1, 1) } else { (1, 0) };
    let retorno = ValueId(if vizinho { 2 } else { 1 });
    if produzido != esperado || planos["ler_e_substituir"].classes[&retorno] != Ownership::Owned {
        return Err(format!(
            "keepalive esperado {esperado:?}, encontrado {produzido:?}"
        ));
    }
    let preparado = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != preparado
    {
        return Err("preparação de keepalive não foi idempotente".into());
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    if argumento {
        m.retornos_uniformes_arc.insert("ler_e_substituir".into());
        certificar_portas_arc_aot(&mut m, &planos)?;
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(outro) => return Err(format!("modo desconhecido: {outro}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(outro) => return Err(format!("nível desconhecido: {outro}")),
    };
    let mut controle = args.next();
    let captura = matches!(controle.as_deref(), Some("--captura" | "--captura-vizinho"));
    let vizinho = matches!(controle.as_deref(), Some("--vizinho" | "--captura-vizinho"));
    let argumento = controle.as_deref() == Some("--argumento");
    if captura || vizinho || argumento { controle = args.next(); }
    if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = modulo(captura, vizinho, argumento)?;
    m.memoria_arc = arc;
    if !arc { m.portas_arc_aot = None; }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // Nenhum Ref observado passa pela HIR do harness: um endereço já morto
    // é apenas diagnóstico nativo. Não há alocação entre morte e observação,
    // evitando confundir reutilização de endereço com sobrevivência.
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_keepalive() {
  %obj = call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)
  %filho = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  call void @inicializar(i64 %obj, i64 %filho)
  call void @dartforge_arc_release(i64 %filho)
  %aresta = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %ok_aresta = icmp eq i8 %aresta, 1
  %inteiro = call i64 @ler_inteiro(i64 %obj)
  %double = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 1)
  %bool = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 2)
  %byte = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 3)
  %ok_int = icmp eq i64 %inteiro, 42
  %ok_double = icmp eq i64 %double, -9223372036854775808
  %ok_bool = icmp eq i64 %bool, 1
  %ok_byte = icmp eq i64 %byte, 255
  %ok_a = and i1 %ok_int, %ok_double
  %ok_b = and i1 %ok_bool, %ok_byte
  %ok_campos = and i1 %ok_a, %ok_b
  %ok_inicio = and i1 %ok_campos, %ok_aresta
  br i1 %ok_inicio, label %ler, label %falha
ler:
  %retorno = call i64 @ler_e_substituir(i64 %obj)
  %mesmo = icmp eq i64 %retorno, %filho
  br i1 %mesmo, label %coletar, label %falha
coletar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %obj_morto = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %filho_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %ok_obj = icmp eq i8 %obj_morto, 0
  %ok_filho = icmp eq i8 %filho_vivo, 3
  %ok = and i1 %ok_obj, %ok_filho
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %retorno)
  call void @dartforge_arc_collect()
  %final = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %morto = icmp eq i8 %final, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if captura {
        ir = ir.replace("@dartforge_arc_objeto_owned_v1(i64 123, i64 40)",
            &format!("@dartforge_arc_objeto_owned_v1(i64 {}, i64 40)",
                dartforge_runtime::layout::cid::CONTEXTO));
    }
    if vizinho {
        ir = ir.replace("  call void @inicializar(i64 %obj, i64 %filho)\n",
            "  %vizinho = call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)\n  call void @inicializar(i64 %vizinho, i64 %filho)\n  call void @inicializar(i64 %obj, i64 %vizinho)\n  call void @dartforge_arc_release(i64 %vizinho)\n");
        ir = ir.replace("  %ok = and i1 %ok_obj, %ok_filho\n",
            "  %vizinho_morto = call i8 @dartforge_arc_observar_heap_v1(i64 %vizinho)\n  %ok_vizinho = icmp eq i8 %vizinho_morto, 0\n  %ok_par = and i1 %ok_obj, %ok_filho\n  %ok = and i1 %ok_par, %ok_vizinho\n");
    }
    if argumento {
        ir.push_str("\n@desc.argumento = private constant [2 x i64] [i64 1, i64 0]\n");
        ir = ir.replace("  %retorno = call i64 @ler_e_substituir(i64 %obj)\n",
            "  %argumentos = alloca [1 x i64], align 8\n  store i64 %obj, ptr %argumentos\n  %retorno = call i64 @ler_e_substituir(i64 %obj, ptr %argumentos, ptr @desc.argumento)\n");
    }
    match controle.as_deref() {
        None => {}
        Some("sem-retencao") => {
            let emprestado = m.functions[0].blocks[0].instructions.iter()
                .find_map(|(v, i, _)| match i {
                    Instruction::ArcCopy { value: Operand::Val(de) }
                        if *v == ValueId(if vizinho { 2 } else { 1 }) => Some(de.0),
                    _ => None,
                }).ok_or("cópia do retorno não encontrada")?;
            let chamada = format!("  call void @dartforge_arc_retain(i64 %v{emprestado})\n");
            if ir.matches(&chamada).count() != 1 {
                return Err("retenção produzida não foi localizada unicamente no IR".into());
            }
            ir = ir.replace(&chamada, "");
        }
        Some("sem-liberar") => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %retorno)\n", "");
        }
        Some(outro) => return Err(format!("controle desconhecido: {outro}")),
    }
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn argumento_owned_sustenta_receiver_e_certifica_vetores_readonly() {
        let m = modulo(false, false, true).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_argumento_owned_v1"));
        let soltar = ops.iter().position(|(_, i, _)| matches!(i,
            Instruction::ArcDrop { value: Operand::Val(ValueId(9)) })).unwrap();
        let gravar = ops.iter().position(|(v, _, _)| *v == ValueId(2)).unwrap();
        assert!(soltar > gravar);
        assert!(m.portas_arc_aot.is_some());
    }

    #[test]
    fn captura_derivada_conserva_owners_antes_de_mutar_ambiente() {
        let m = modulo(true, true, false).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_env_ref_v1"));
        let retirar = ops.iter().position(|(v, _, _)| *v == ValueId(3)).unwrap();
        for id in [ValueId(1), ValueId(2)] {
            let reter = ops.iter().position(|(v, i, _)| *v == id && matches!(i, Instruction::ArcCopy { .. })).unwrap();
            assert!(reter < retirar);
        }
    }

    #[test]
    fn filho_e_vizinho_sobrevivem_a_remocao_da_aresta_da_raiz() {
        let m = modulo(false, true, false).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        let retirar_raiz = ops.iter().position(|(v, _, _)| *v == ValueId(3)).unwrap();
        for id in [ValueId(1), ValueId(2)] {
            let reter = ops.iter().position(|(v, i, _)| *v == id && matches!(i, Instruction::ArcCopy { .. })).unwrap();
            assert!(reter < retirar_raiz);
        }
        let retirar_filho = ops.iter().position(|(v, _, _)| *v == ValueId(4)).unwrap();
        let soltar_vizinho = ops.iter().position(|(_, i, _)| matches!(i,
            Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })).unwrap();
        assert!(soltar_vizinho > retirar_filho);
    }

    #[test]
    fn retencao_produzida_antecede_substituicao_da_aresta() {
        let m = modulo(false, false, false).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(
            ops[1],
            (ValueId(1), Instruction::ArcCopy { .. }, Type::Ref)
        ));
        let ir = LlvmEmitter::new(&m).emit_all();
        assert!(
            ir.find("call void @dartforge_arc_retain").unwrap()
                < ir.find("call void @dartforge_arc_gravar_campo_ref_conferido_v1")
                    .unwrap()
        );
    }
}

#[cfg(test)]
mod testes_captura {
    use super::*;

    #[test]
    fn captura_auditada_e_retida_antes_de_substituir_aresta() {
        let m = modulo(true, false, false).unwrap();
        let ir = LlvmEmitter::new(&m).emit_all();
        let ler = ir.find("call i64 @dartforge_arc_env_ref_v1").unwrap();
        let reter = ir.find("call void @dartforge_arc_retain").unwrap();
        let gravar = ir.find("call void @dartforge_arc_gravar_campo_ref_conferido_v1").unwrap();
        assert!(ler < reter && reter < gravar);
    }
}
