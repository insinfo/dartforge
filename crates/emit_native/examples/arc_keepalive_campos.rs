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

fn modulo(captura: bool) -> Result<Module, String> {
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
    if produzido != (1, 0) || planos["ler_e_substituir"].classes[&ValueId(1)] != Ownership::Owned {
        return Err(format!(
            "keepalive esperado (1, 0), encontrado {produzido:?}"
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
    let captura = controle.as_deref() == Some("--captura");
    if captura { controle = args.next(); }
    if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = modulo(captura)?;
    m.memoria_arc = arc;
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
    match controle.as_deref() {
        None => {}
        Some("sem-retencao") => {
            let emprestado = m.functions[0].blocks[0].instructions[0].0.0;
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
    fn retencao_produzida_antecede_substituicao_da_aresta() {
        let m = modulo(false).unwrap();
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
        let m = modulo(true).unwrap();
        let ir = LlvmEmitter::new(&m).emit_all();
        let ler = ir.find("call i64 @dartforge_arc_env_ref_v1").unwrap();
        let reter = ir.find("call void @dartforge_arc_retain").unwrap();
        let gravar = ir.find("call void @dartforge_arc_gravar_campo_ref_conferido_v1").unwrap();
        assert!(ler < reter && reter < gravar);
    }
}
