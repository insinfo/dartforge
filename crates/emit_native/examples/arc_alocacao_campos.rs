//! Prova AOT da preparação de AllocObject com layout completo de 40 campos.
//! Confere bits escalares, aresta Ref e morte após liberação do retorno Owned.
//! Tracing usa tokens explícitos apenas no harness de comparação.
//! Não certifica lowering de fonte Dart, guardas de getters ou recarga.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo(quadro: bool, tipada: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_alocacao_campos".into());
    m.classes.push(ClassDef {
        id: 123,
        name: "C".into(),
        field_count: 40,
        vtable: vec![],
        to_string_symbol: None,
    });
    let mut layout = vec![Type::I64; 40];
    layout[1] = Type::F64;
    layout[2] = Type::I1;
    layout[3] = Type::I8;
    layout[39] = Type::Ref;
    m.layouts_campos_arc.insert(123, layout);
    let mut fields = vec![Operand::Constant(Constant::Int(85)); 40];
    fields[1] = Operand::Constant(Constant::Double(-0.0));
    fields[2] = Operand::Constant(Constant::Bool(true));
    fields[3] = Operand::Val(ValueId(3));
    fields[39] = Operand::Val(ValueId(0));
    if tipada {
        fields[0] = Operand::Val(ValueId(12));
        fields[1] = Operand::Val(ValueId(11));
        fields[2] = Operand::Val(ValueId(13));
        m.parametros_escalares_dart.insert("criar_objeto".into(), std::collections::HashSet::from([ValueId(12)]));
    }
    if quadro {
        m.classes.clear(); m.layouts_campos_arc.clear();
        fields[0] = Operand::Constant(Constant::Int(0));
        fields[6] = fields[1].clone(); fields[7] = fields[2].clone(); fields[8] = fields[3].clone();
        fields[1..5].fill(Operand::Constant(Constant::Null));
        fields[5] = Operand::Constant(Constant::Int(0));
    }
    m.functions.push(Function {
        symbol: "criar_objeto".into(),
        name: "criar_objeto".into(),
        depuracao: None,
        params: if tipada { vec![(ValueId(10), "contexto".into(), Type::Ref),
            (ValueId(11), "double".into(), Type::F64), (ValueId(12), "int".into(), Type::I64),
            (ValueId(13), "bool".into(), Type::I1)] } else { vec![] },
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)),
                        from: Type::I64,
                        to: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: if quadro { dartforge_emit_native::lower::async_sm::ID_QUADRO_ASYNC } else { 123 },
                        fields,
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    if tipada {
        let f = &mut m.functions[0];
        f.blocks[0].id = BlockId(1);
        f.blocks.insert(0, BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
            cond: Operand::Val(ValueId(13)), then_block: BlockId(1), else_block: BlockId(2),
        } });
        f.blocks.extend([
            BasicBlock { id: BlockId(2), instructions: vec![
                (ValueId(20), Instruction::CallClosure { closure: Operand::Constant(Constant::Null), args: vec![], nomes: vec![],
                    tupla_tipos: Operand::Constant(Constant::Int(0)), ret_ty: Type::Ref }, Type::Ref),
                (ValueId(21), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(22), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(21)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(22)), then_block: BlockId(3), else_block: BlockId(4) } },
            BasicBlock { id: BlockId(3), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(20)))) },
        ]);
    }
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    if quadro {
        m.functions.push(Function { symbol: "ler_quadro".into(), name: "ler_quadro".into(), depuracao: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 39 }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
    }
    let mut planos = HashMap::from([("criar_objeto".into(), plano)]);
    if tipada {
        m.functions.push(Function { symbol: "entrada_objeto".into(), name: "entrada_objeto".into(), depuracao: None,
            params: vec![(ValueId(0), "recv".into(), Type::Ref),
                (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
            return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] });
        m.retornos_uniformes_arc.insert("entrada_objeto".into());
        m.parametros_vetores_dart.insert("entrada_objeto".into(), HashMap::from([
            (ValueId(1), VetorEntradaDart::Argumentos), (ValueId(2), VetorEntradaDart::Descritor),
        ]));
        m.functions.push(Function { symbol: "criar_closure".into(), name: "criar_closure".into(), depuracao: None,
            params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::Box { op: Operand::Constant(Constant::Int(i64::MAX - 1)), from: Type::I64 }, Type::Ref),
                (ValueId(0), Instruction::AllocClosureTipada {
                    code_symbol: "entrada_objeto".into(), env: Operand::Val(ValueId(1)),
                    tipado: "criar_objeto".into(), abi: 1071, direto: false,
                }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
        let mut p = PlanoFuncaoDart::default(); p.tokens.retorno = RetornoTokens::Owned;
        planos.insert("criar_closure".into(), p);
        planos.insert("entrada_objeto".into(), PlanoFuncaoDart::default());
        m.functions.push(Function { symbol: "invocar_closure".into(), name: "invocar_closure".into(), depuracao: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(7), "normal".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_arc_alvo_tipada_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(1071)), Type::I64),
                ], ret_ty: Type::I64 }, Type::I64),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_arc_contexto_closure_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                ], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::ChamadaTipada { alvo: Operand::Val(ValueId(1)), args: vec![
                    (Operand::Val(ValueId(2)), Type::Ref), (Operand::Constant(Constant::Double(-0.0)), Type::F64),
                    (Operand::Constant(Constant::Int(85)), Type::I64), (Operand::Val(ValueId(7)), Type::I1),
                ], ret: Type::Ref }, Type::Ref),
                (ValueId(4), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(5), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(4)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(5)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))) },
            ] });
        let mut p = PlanoFuncaoDart::default(); p.tokens.retorno = RetornoTokens::Owned;
        planos.insert("invocar_closure".into(), p);
    }
    if quadro {
        let mut p = PlanoFuncaoDart::default(); p.tokens.retorno = RetornoTokens::Owned;
        planos.insert("ler_quadro".into(), p);
    }
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    let esperado = if quadro { (1, 1) } else if tipada { (1, 4) } else { (0, 1) };
    if produzido != esperado {
        return Err(format!("esperado {esperado:?}, encontrado {produzido:?}"));
    }
    let antes = format!("{m:?}");
    let planos_antes = planos.clone();
    let repetido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if repetido != (0, 0)
        || format!("{m:?}") != antes || planos != planos_antes
    {
        return Err(format!("preparação não idempotente: {repetido:?}"));
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    if tipada && certificar_portas_arc_aot(&mut m, &planos)? != 2 {
        return Err("imagem tipada sem as duas portas verificadas".into());
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável")?);
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
    let quadro = controle.as_deref() == Some("--quadro");
    let erro_tipado = controle.as_deref() == Some("--tipada-erro");
    let tipada = controle.as_deref() == Some("--tipada") || erro_tipado;
    if quadro || tipada { controle = args.next(); }
    if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = modulo(quadro, tipada)?;
    m.memoria_arc = arc;
    if !arc { m.portas_arc_aot = None; }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    let cid = if quadro { dartforge_emit_native::lower::async_sm::ID_QUADRO_ASYNC } else { 123 };
    let owned = format!("call i64 @dartforge_arc_objeto_owned_v1(i64 {cid}, i64 40)");
    let sem_owner = format!("call i64 @dartforge_object_new(i64 {cid}, i64 40)");
    let reter_leitura = m.functions.iter().find(|f| f.symbol == "ler_quadro")
        .and_then(|f| f.blocks.iter().flat_map(|b| &b.instructions).find_map(|(_, i, _)|
            if let Instruction::ArcCopy { value: Operand::Val(v) } = i {
                Some(format!("  call void @dartforge_arc_retain(i64 %v{})\n", v.0))
            } else { None })).unwrap_or_default();
    if let Some(controle) = controle {
        let (antes, depois) = match controle.as_str() {
            "sem-drop-filho" => ("  call void @dartforge_arc_release(i64 %v0)\n", ""),
            "sem-retencao-leitura" if quadro => (reter_leitura.as_str(), ""),
            "sem-liberar-contexto" if tipada => ("  call void @dartforge_arc_release(i64 %v2)\n", ""),
            "sem-owner-objeto" => (
                owned.as_str(),
                sem_owner.as_str(),
            ),
            _ => return Err(format!("controle desconhecido: {controle}")),
        };
        let quantidade = if controle == "sem-liberar-contexto" { 2 } else { 1 };
        if ir.matches(antes).count() != quantidade {
            return Err(format!("controle sem alvo único: {controle}"));
        }
        ir = ir.replace(antes, depois);
    }
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_alocacao_campos() {
  %obj = call i64 @criar_objeto()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %campos, label %falha
campos:
  %a = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 0)
  %b = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 1)
  %c = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 2)
  %d = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 3)
  %aok = icmp eq i64 %a, 85
  %bok = icmp eq i64 %b, -9223372036854775808
  %cok = icmp eq i64 %c, 1
  %dok = icmp eq i64 %d, 255
  %ab = and i1 %aok, %bok
  %cd = and i1 %cok, %dok
  %bits = and i1 %ab, %cd
  br i1 %bits, label %filho, label %falha
filho:
  %ref = call i64 @dartforge_arc_ler_campo_ref_v1(i64 %obj, i64 39)
  %valor = call i64 @dartforge_unbox_int(i64 %ref)
  %mint = icmp eq i64 %valor, 9223372036854775807
  %ef = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)
  %sem_owner = icmp eq i8 %ef, 1
  %ok = and i1 %mint, %sem_owner
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %eo = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %em = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)
  %morto = icmp eq i8 %eo, 0
  %filho_morto = icmp eq i8 %em, 0
  %fim = and i1 %morto, %filho_morto
  br i1 %fim, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if quadro {
        // O cabeçalho reserva quatro Ref e a tupla RTI; os escalares desta
        // fixture ficam na cauda. O heap observa o mapa real da instância.
        for (de, para) in [(1, 6), (2, 7), (3, 8)] {
            ir = ir.replace(&format!("i64 %obj, i64 {de})"), &format!("i64 %obj, i64 {para})"));
        }
        ir = ir.replace("%aok = icmp eq i64 %a, 85", "%aok = icmp eq i64 %a, 0");
        ir = ir.replace("%ref = call i64 @dartforge_arc_ler_campo_ref_v1(i64 %obj, i64 39)",
            "%ref = call i64 @ler_quadro(i64 %obj)");
        ir = ir.replace("%sem_owner = icmp eq i8 %ef, 1", "%sem_owner = icmp eq i8 %ef, 3");
        ir = ir.replace("  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()",
            "  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()\n  %sobreviveu = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)\n  %ainda_owned = icmp eq i8 %sobreviveu, 3\n  br i1 %ainda_owned, label %soltar_leitura, label %falha\nsoltar_leitura:\n  call void @dartforge_arc_release(i64 %ref)\n  call void @dartforge_arc_collect()");
    }
    if tipada {
        // ARC usa o registro automático do certificado antes da entrada Dart.
        // Tracing registra manualmente apenas para comparar a mesma ABI nativa;
        // esse registro não prepara nem certifica o pipeline tracing.
        let registro_tracing = if arc { "" } else {
            "  %pc = ptrtoint ptr @criar_objeto to i64\n  %versao = call i64 @dartforge_arc_registrar_tipada_v1(i64 %pc, i64 1071)\n"
        };
        ir = ir.replace("  %obj = call i64 @criar_objeto()",
            &format!("{registro_tracing}  %closure = call i64 @criar_closure()\n  %contexto = call i64 @dartforge_arc_contexto_closure_v1(i64 %closure)\n  %obj = call i64 @invocar_closure(i64 %closure, i1 true)\n  call void @dartforge_arc_release(i64 %closure)\n  call void @dartforge_arc_collect()\n  %ec = call i8 @dartforge_arc_observar_heap_v1(i64 %contexto)\n  %contexto_morto = icmp eq i8 %ec, 0\n  br i1 %contexto_morto, label %conferir_objeto, label %falha\nconferir_objeto:"));
        if erro_tipado {
            let inicio = ir.find("define void @prova_alocacao_campos() {").ok_or("harness ausente")?;
            ir.truncate(inicio);
            ir.push_str(&format!("define void @prova_alocacao_campos() {{\n{registro_tracing}"));
            ir.push_str(r#"
  %closure = call i64 @criar_closure()
  %contexto = call i64 @dartforge_arc_contexto_closure_v1(i64 %closure)
  %obj = call i64 @invocar_closure(i64 %closure, i1 false)
  %pending = call i8 @dartforge_exception_pending()
  %erro = icmp ne i8 %pending, 0
  %sem_resultado = icmp eq i64 %obj, 0
  %ok = and i1 %erro, %sem_resultado
  br i1 %ok, label %limpar, label %falha
limpar:
  call void @dartforge_exception_clear()
  call void @dartforge_arc_release(i64 %closure)
  call void @dartforge_arc_collect()
  %ec = call i8 @dartforge_arc_observar_heap_v1(i64 %contexto)
  %contexto_morto = icmp eq i8 %ec, 0
  br i1 %contexto_morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#);
        }
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
    fn preparacao_de_alocacao_e_campos_gera_ir_verificado() {
        for (quadro, tipada) in [(false, false), (true, false), (false, true)] {
            LlvmEmitter::new(&modulo(quadro, tipada).unwrap()).emit_all();
        }
    }
}
